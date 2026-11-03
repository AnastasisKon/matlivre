//! Image and colormap utilities: `imresize`, `rgb2ind`, `dither`, `cmunique`, `cmpermute`,
//! `cmap2gray`, `getrangefromclass`, and the lighting helpers `diffuse` and `specular`.

use crate::builtins::{arg, one, Ret};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["imresize", "rgb2ind", "dither", "cmunique", "cmpermute", "cmap2gray", "getrangefromclass", "diffuse", "specular"];

const GRAY_WEIGHTS: [f64; 3] = [0.298936021293775, 0.587043074451121, 0.114020904255103];

/// Rows of an n×3 colormap.
fn map_rows(v: &Value) -> R<Vec<[f64; 3]>> {
    let m = v.to_matrix()?;
    if m.dims.len() != 2 || m.cols() != 3 {
        return rt("The colormap must be an m-by-3 array.");
    }
    let n = m.rows();
    Ok((0..n).map(|i| [m.data[i], m.data[n + i], m.data[2 * n + i]]).collect())
}

fn map_value(rows: &[[f64; 3]]) -> Value {
    let n = rows.len();
    let mut data = vec![0.0; n * 3];
    for (i, r) in rows.iter().enumerate() {
        for k in 0..3 {
            data[k * n + i] = r[k];
        }
    }
    Value::Num(Matrix::from_dims(vec![n, 3], data))
}

/// Scale factor taking an image's values to [0, 1].
fn unit_scale(m: &Matrix) -> f64 {
    match m.kind {
        Kind::Int(c) => c.range().1,
        _ => 1.0,
    }
}

/// Zero-based indices in the smallest integer class that holds them, as MATLAB returns.
fn index_image(dims: Vec<usize>, idx: Vec<f64>, colors: usize) -> Value {
    let m = Matrix::from_dims(dims, idx);
    if colors <= 256 {
        Value::Num(m.cast_to(Kind::Int(IntClass::U8)))
    } else if colors <= 65_536 {
        Value::Num(m.cast_to(Kind::Int(IntClass::U16)))
    } else {
        Value::Num(Matrix::from_dims(m.dims.clone(), m.data.iter().map(|x| x + 1.0).collect()))
    }
}

/// The closest map entry; ties go to the later entry, as in MATLAB.
fn nearest(map: &[[f64; 3]], c: [f64; 3]) -> usize {
    let d = |m: &[f64; 3]| (0..3).map(|k| (m[k] - c[k]).powi(2)).sum::<f64>();
    let mut best = (0, f64::INFINITY);
    for (k, m) in map.iter().enumerate() {
        let dk = d(m);
        if dk <= best.1 {
            best = (k, dk);
        }
    }
    best.0
}

/// Pixels of an M×N×3 image, scaled to [0, 1].
fn rgb_pixels(m: &Matrix) -> R<(usize, usize, Vec<[f64; 3]>)> {
    if m.dims.len() != 3 || m.dims[2] != 3 {
        return rt("RGB must be an M-by-N-by-3 array.");
    }
    let (r, c) = (m.dims[0], m.dims[1]);
    let n = r * c;
    let s = unit_scale(m);
    Ok((r, c, (0..n).map(|i| [m.data[i] / s, m.data[n + i] / s, m.data[2 * n + i] / s]).collect()))
}

/// Indices into `map`, with Floyd–Steinberg error diffusion when `dither` is set.
fn quantize(rows: usize, cols: usize, mut px: Vec<[f64; 3]>, map: &[[f64; 3]], dither: bool) -> Vec<f64> {
    let mut out = vec![0.0; rows * cols];
    for i in 0..rows {
        for j in 0..cols {
            let p = i + rows * j;
            let c = px[p].map(|x| x.clamp(0.0, 1.0));
            let k = nearest(map, c);
            out[p] = k as f64;
            if !dither {
                continue;
            }
            let err = [0, 1, 2].map(|ch| c[ch] - map[k][ch]);
            let mut spread = |di: isize, dj: isize, w: f64| {
                let (ii, jj) = (i as isize + di, j as isize + dj);
                if ii >= 0 && (ii as usize) < rows && jj >= 0 && (jj as usize) < cols {
                    let q = ii as usize + rows * jj as usize;
                    for ch in 0..3 {
                        px[q][ch] += err[ch] * w;
                    }
                }
            };
            spread(0, 1, 7.0 / 16.0);
            spread(1, -1, 3.0 / 16.0);
            spread(1, 0, 5.0 / 16.0);
            spread(1, 1, 1.0 / 16.0);
        }
    }
    out
}

/// The interpolation kernel and its half-width.
fn kernel(method: &str) -> R<(fn(f64) -> f64, f64)> {
    fn cubic(x: f64) -> f64 {
        let a = x.abs();
        let (a2, a3) = (a * a, a * a * a);
        if a <= 1.0 {
            1.5 * a3 - 2.5 * a2 + 1.0
        } else if a <= 2.0 {
            -0.5 * a3 + 2.5 * a2 - 4.0 * a + 2.0
        } else {
            0.0
        }
    }
    fn triangle(x: f64) -> f64 {
        (1.0 - x.abs()).max(0.0)
    }
    fn boxk(x: f64) -> f64 {
        if (-0.5..0.5).contains(&x) {
            1.0
        } else {
            0.0
        }
    }
    Ok(match method {
        "bicubic" | "cubic" => (cubic, 2.0),
        "bilinear" | "linear" | "triangle" => (triangle, 1.0),
        "nearest" | "box" => (boxk, 0.5),
        other => return rt(format!("Unknown interpolation method '{other}'.")),
    })
}

/// Weights resampling a length-`n` axis to `m` samples; edges mirror (symmetric padding).
fn axis_weights(n: usize, m: usize, scale: f64, method: &str, antialias: bool) -> R<Vec<Vec<(usize, f64)>>> {
    let (k, half) = kernel(method)?;
    let shrink = scale < 1.0 && antialias && method != "nearest";
    let (width, s) = if shrink { (half / scale, scale) } else { (half, 1.0) };
    let mirror = |j: i64| -> usize {
        let period = 2 * n as i64;
        let r = j.rem_euclid(period);
        (if r < n as i64 { r } else { period - 1 - r }) as usize
    };
    let mut out = Vec::with_capacity(m);
    for x in 1..=m {
        let u = x as f64 / scale + 0.5 * (1.0 - 1.0 / scale);
        if method == "nearest" {
            let j = (u + 0.5).floor() as i64 - 1;
            out.push(vec![(mirror(j), 1.0)]);
            continue;
        }
        let left = (u - width).floor() as i64;
        let taps = (width * 2.0).ceil() as i64 + 2;
        let mut w: Vec<(usize, f64)> = Vec::new();
        let mut total = 0.0;
        for t in 0..taps {
            let j = left + t;
            let wt = s * k(s * (u - j as f64));
            if wt != 0.0 {
                w.push((mirror(j - 1), wt));
                total += wt;
            }
        }
        for e in w.iter_mut() {
            e.1 /= total;
        }
        out.push(w);
    }
    Ok(out)
}

fn imresize(a: &[Value]) -> Ret {
    let img = arg(a, 0, "imresize")?.to_matrix()?;
    let size_arg = arg(a, 1, "imresize")?.to_matrix()?;
    let mut method = "bicubic".to_string();
    let mut antialias = true;
    let mut i = 2;
    while i < a.len() {
        let key = a[i].text()?.to_lowercase();
        if key == "antialiasing" {
            antialias = a.get(i + 1).map(|v| v.is_true()).transpose()?.unwrap_or(true);
            i += 2;
            continue;
        }
        if key == "method" {
            method = a.get(i + 1).map(|v| v.text()).transpose()?.unwrap_or_default().to_lowercase();
            i += 2;
            continue;
        }
        method = key;
        i += 1;
    }
    if method == "nearest" {
        antialias = false;
    }
    let (rows, cols) = (img.dims[0], img.dims.get(1).copied().unwrap_or(1));
    let planes: usize = img.dims[2..].iter().product();
    let (out_r, out_c, sr, sc) = if size_arg.numel() == 1 {
        let s = size_arg.data[0];
        ((rows as f64 * s).ceil() as usize, (cols as f64 * s).ceil() as usize, s, s)
    } else {
        let (r, c) = (size_arg.data[0], size_arg.data[1]);
        let (r, c) = match (r.is_nan(), c.is_nan()) {
            (true, false) => ((rows as f64 * c / cols as f64).ceil(), c),
            (false, true) => (r, (cols as f64 * r / rows as f64).ceil()),
            _ => (r, c),
        };
        (r as usize, c as usize, r / rows as f64, c / cols as f64)
    };
    let wr = axis_weights(rows, out_r, sr, &method, antialias)?;
    let wc = axis_weights(cols, out_c, sc, &method, antialias)?;
    let mut data = Vec::with_capacity(out_r * out_c * planes);
    for p in 0..planes {
        let base = p * rows * cols;
        // Columns first (along dimension 2), then rows.
        let mut tmp = vec![0.0; rows * out_c];
        for (j, w) in wc.iter().enumerate() {
            for r in 0..rows {
                tmp[r + rows * j] = w.iter().map(|&(c, wt)| img.data[base + r + rows * c] * wt).sum();
            }
        }
        for j in 0..out_c {
            for w in &wr {
                data.push(w.iter().map(|&(r, wt)| tmp[r + rows * j] * wt).sum::<f64>());
            }
        }
    }
    let mut dims = vec![out_r, out_c];
    dims.extend_from_slice(&img.dims[2..]);
    let out = Matrix::from_dims(dims, data);
    one(Value::Num(match img.kind {
        Kind::Double => out,
        k => out.cast_to(k),
    }))
}

/// Normalized direction from `[x y z]` or `[az el]` in degrees.
fn direction(v: &Value) -> R<[f64; 3]> {
    let m = v.to_matrix()?;
    let d = match m.data.as_slice() {
        [az, el] => {
            let (az, el) = (az.to_radians(), el.to_radians());
            [el.cos() * az.cos(), el.cos() * az.sin(), el.sin()]
        }
        [x, y, z] => [*x, *y, *z],
        _ => return rt("Light and view directions must be [x y z] or [az el]."),
    };
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    Ok(d.map(|x| x / n))
}

fn lighting(name: &str, a: &[Value]) -> Ret {
    let nx = arg(a, 0, name)?.to_matrix()?;
    let ny = arg(a, 1, name)?.to_matrix()?;
    let nz = arg(a, 2, name)?.to_matrix()?;
    if nx.numel() != ny.numel() || nx.numel() != nz.numel() {
        return rt("Nx, Ny and Nz must be the same size.");
    }
    let s = direction(arg(a, 3, name)?)?;
    let specular = name == "specular";
    let (v, spread) = if specular {
        (direction(arg(a, 4, name)?)?, a.get(5).map(|x| x.scalar()).transpose()?.unwrap_or(10.0))
    } else {
        ([0.0; 3], 0.0)
    };
    let out = (0..nx.numel())
        .map(|i| {
            let n = [nx.data[i], ny.data[i], nz.data[i]];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let n = n.map(|x| x / len);
            let ns = n[0] * s[0] + n[1] * s[1] + n[2] * s[2];
            if !specular {
                return ns.max(0.0);
            }
            // The light reflected about the normal, against the view direction.
            let r = [0, 1, 2].map(|k| 2.0 * ns * n[k] - s[k]);
            let rv = r[0] * v[0] + r[1] * v[1] + r[2] * v[2];
            rv.max(0.0).powf(spread)
        })
        .collect();
    one(Value::Num(Matrix::from_dims(nx.dims.clone(), out)))
}

/// Distinct colors sorted by blue, then green, then red, with doubles quantized to 1/1024.
fn cmunique(a: &[Value]) -> Ret {
    let first = arg(a, 0, "cmunique")?.to_matrix()?;
    let q = |x: f64| (x * 1024.0).round() / 1024.0;
    let (dims, px): (Vec<usize>, Vec<[f64; 3]>) = if a.len() >= 2 {
        let map = map_rows(&a[1])?;
        let offset = if matches!(first.kind, Kind::Int(_)) { 0.0 } else { 1.0 };
        let px = first
            .data
            .iter()
            .map(|&x| map.get((x - offset) as usize).copied().ok_or_else(|| LangError::new("Index exceeds the number of colormap entries.", 0)))
            .collect::<R<_>>()?;
        (first.dims.clone(), px)
    } else if first.dims.len() == 3 && first.dims[2] == 3 {
        let (r, c, px) = rgb_pixels(&first)?;
        (vec![r, c], px.into_iter().map(|p| p.map(q)).collect())
    } else {
        let s = unit_scale(&first);
        (first.dims.clone(), first.data.iter().map(|&x| [q(x / s); 3]).collect())
    };
    let key = |c: &[f64; 3]| (c[2], c[1], c[0]);
    let mut colors: Vec<[f64; 3]> = px.clone();
    colors.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap());
    colors.dedup();
    let idx = px.iter().map(|p| colors.binary_search_by(|c| key(c).partial_cmp(&key(p)).unwrap()).unwrap_or(0) as f64).collect();
    let n = colors.len();
    Ok(Some(vec![index_image(dims, idx, n), map_value(&colors)]))
}

fn cmpermute(a: &[Value]) -> Ret {
    let x = arg(a, 0, "cmpermute")?.to_matrix()?;
    let map = map_rows(arg(a, 1, "cmpermute")?)?;
    let order: Vec<usize> = match a.get(2) {
        Some(v) => v.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect(),
        None => {
            // Without an order, sort by gray level.
            let mut o: Vec<usize> = (0..map.len()).collect();
            let gray = |c: &[f64; 3]| (0..3).map(|k| GRAY_WEIGHTS[k] * c[k]).sum::<f64>();
            o.sort_by(|&p, &q| gray(&map[p]).partial_cmp(&gray(&map[q])).unwrap());
            o
        }
    };
    if order.iter().any(|&k| k >= map.len()) {
        return rt("Index exceeds the number of colormap entries.");
    }
    let mut inverse = vec![0usize; map.len()];
    for (new, &old) in order.iter().enumerate() {
        inverse[old] = new;
    }
    let offset = if matches!(x.kind, Kind::Int(_)) { 0.0 } else { 1.0 };
    let y = x.data.iter().map(|&v| inverse[(v - offset) as usize] as f64 + offset).collect();
    let newmap: Vec<[f64; 3]> = order.iter().map(|&k| map[k]).collect();
    Ok(Some(vec![Value::Num(Matrix::from_dims(x.dims.clone(), y).cast_to(x.kind)), map_value(&newmap)]))
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    let outs = match name {
        "imresize" => imresize(a)?,
        "diffuse" | "specular" => lighting(name, a)?,
        "cmunique" => cmunique(a)?,
        "cmpermute" => cmpermute(a)?,
        "cmap2gray" => {
            let map = map_rows(arg(a, 0, name)?)?;
            let gray: Vec<[f64; 3]> = map.iter().map(|c| [(0..3).map(|k| GRAY_WEIGHTS[k] * c[k]).sum::<f64>(); 3]).collect();
            one(map_value(&gray))?
        }
        "getrangefromclass" => {
            let m = arg(a, 0, name)?.to_matrix()?;
            let (lo, hi) = match m.kind {
                Kind::Int(c) => c.range(),
                _ => (0.0, 1.0),
            };
            one(Value::Num(Matrix::row(vec![lo, hi])))?
        }
        "rgb2ind" | "dither" => {
            let img = arg(a, 0, name)?.to_matrix()?;
            let map_arg = arg(a, 1, name)?;
            let map = map_rows(map_arg).map_err(|_| LangError::new(format!("{name} needs a colormap; color quantization to a number of colors or a tolerance is not supported."), 0))?;
            let dither = match a.get(2) {
                Some(v) => v.text()?.to_lowercase() != "nodither",
                None => true,
            };
            let (r, c, px) = rgb_pixels(&img)?;
            let idx = quantize(r, c, px, &map, dither);
            let n = map.len();
            Some(vec![index_image(vec![r, c], idx, n), map_value(&map)])
        }
        _ => return Ok(None),
    };
    Ok(outs.map(|mut v| {
        v.truncate(nargout.max(1));
        v
    }))
}
