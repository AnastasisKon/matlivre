//! Volume and vector field visualization: isosurfaces (`isosurface`, `isocaps`, `isonormals`,
//! `isocolors`), volume utilities (`smooth3`, `subvolume`, `reducevolume`, `volumebounds`),
//! patch utilities (`surf2patch`, `shrinkfaces`, `reducepatch`, and `patch` with faces and
//! vertices), stream lines (`stream2`, `stream3`, `streamline`, `streamtube`, `streamribbon`,
//! `streamslice`, `streamparticles`, `interpstreamspeed`), `slice`, `contourslice` and
//! `coneplot`.
//!
//! Isosurfaces are extracted by marching tetrahedra (six per cell), so their triangles differ
//! from MATLAB's marching cubes while lying on the same surface. Stream lines follow MATLAB:
//! Euler steps of `step` cells along the velocity scaled to a unit largest component.

use std::collections::HashMap;

use crate::builtins::{arg, none, one, Ret};
use crate::builtins_plot::handles_out;
use crate::error::{rt, LangError};
use crate::gfx;
use crate::graphics::{LineStyle, Series, SeriesKind};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "isosurface", "isocaps", "isonormals", "isocolors", "smooth3", "subvolume", "reducevolume", "volumebounds", "surf2patch", "shrinkfaces", "reducepatch",
    "stream2", "stream3", "streamline", "streamtube", "streamribbon", "streamslice", "streamparticles", "interpstreamspeed", "slice", "contourslice",
    "coneplot",
];

fn num(v: &Value) -> bool {
    matches!(v, Value::Num(m) if m.kind != Kind::Char)
}

fn mat(v: &Value) -> R<Matrix> {
    v.to_matrix()
}

fn rows3(pts: &[[f64; 3]]) -> Value {
    let n = pts.len();
    let mut d = vec![0.0; n * 3];
    for (i, p) in pts.iter().enumerate() {
        for k in 0..3 {
            d[k * n + i] = p[k];
        }
    }
    Value::Num(Matrix::new(n, 3, d))
}

fn index_rows(faces: &[Vec<usize>]) -> Value {
    let n = faces.len();
    let w = faces.iter().map(|f| f.len()).max().unwrap_or(3);
    let mut d = vec![f64::NAN; n * w];
    for (i, f) in faces.iter().enumerate() {
        for (k, &v) in f.iter().enumerate() {
            d[k * n + i] = (v + 1) as f64;
        }
    }
    Value::Num(Matrix::new(n, w, d))
}

fn read_rows(m: &Matrix) -> Vec<Vec<f64>> {
    let (r, c) = (m.rows(), m.cols());
    (0..r).map(|i| (0..c).map(|j| m.data[j * r + i]).collect()).collect()
}

// ---- volumes -----------------------------------------------------------------------------------

/// A sampled volume: grid vectors (x along columns, y along rows, z along pages) and values.
#[derive(Clone)]
struct Vol {
    xs: Vec<f64>,
    ys: Vec<f64>,
    zs: Vec<f64>,
    v: Vec<f64>,
}

impl Vol {
    fn dims(&self) -> (usize, usize, usize) {
        (self.ys.len(), self.xs.len(), self.zs.len())
    }

    fn at(&self, i: usize, j: usize, k: usize) -> f64 {
        let (ny, nx, _) = self.dims();
        self.v[k * ny * nx + j * ny + i]
    }

    /// Trilinear interpolation (NaN outside the grid).
    fn interp(&self, p: [f64; 3]) -> f64 {
        let (Some(fx), Some(fy), Some(fz)) = (frac(&self.xs, p[0]), frac(&self.ys, p[1]), frac(&self.zs, p[2])) else { return f64::NAN };
        self.interp_idx(fy, fx, fz)
    }

    /// Interpolation at fractional (0-based) row, column and page indices.
    fn interp_idx(&self, fi: f64, fj: f64, fk: f64) -> f64 {
        let (ny, nx, nz) = self.dims();
        let split = |f: f64, n: usize| -> (usize, usize, f64) {
            if n == 1 {
                return (0, 0, 0.0);
            }
            let a = (f.floor() as usize).min(n - 2);
            (a, a + 1, f - a as f64)
        };
        let ((i0, i1, ti), (j0, j1, tj), (k0, k1, tk)) = (split(fi, ny), split(fj, nx), split(fk, nz));
        let lerp = |a: f64, b: f64, t: f64| if t == 0.0 { a } else { a + (b - a) * t };
        let c00 = lerp(self.at(i0, j0, k0), self.at(i0, j1, k0), tj);
        let c10 = lerp(self.at(i1, j0, k0), self.at(i1, j1, k0), tj);
        let c01 = lerp(self.at(i0, j0, k1), self.at(i0, j1, k1), tj);
        let c11 = lerp(self.at(i1, j0, k1), self.at(i1, j1, k1), tj);
        lerp(lerp(c00, c10, ti), lerp(c01, c11, ti), tk)
    }

    fn point(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [self.xs[j], self.ys[i], self.zs[k]]
    }
}

/// Fractional 0-based position of `x` in increasing grid values, or None outside.
fn frac(g: &[f64], x: f64) -> Option<f64> {
    let n = g.len();
    if n == 1 {
        return (x == g[0]).then_some(0.0);
    }
    if !(x >= g[0] && x <= g[n - 1]) {
        return None;
    }
    let k = (0..n - 1).find(|&k| x <= g[k + 1]).unwrap_or(n - 2);
    let d = g[k + 1] - g[k];
    Some(k as f64 + if d == 0.0 { 0.0 } else { (x - g[k]) / d })
}

/// Grid vectors from meshgrid arrays X, Y, Z (or 1..n for missing ones).
fn grid_vectors(dims: &[usize], coords: Option<(&Matrix, &Matrix, &Matrix)>) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (ny, nx, nz) = (dims[0], dims.get(1).copied().unwrap_or(1), dims.get(2).copied().unwrap_or(1));
    match coords {
        Some((x, y, z)) => {
            let at = |m: &Matrix, i: usize, j: usize, k: usize| m.data.get(k * ny * nx + j * ny + i).copied().unwrap_or(f64::NAN);
            // Vectors may also be given directly.
            let vec_or = |m: &Matrix, n: usize, f: &dyn Fn(usize) -> f64| if m.numel() == n { m.data.clone() } else { (0..n).map(f).collect() };
            (vec_or(x, nx, &|j| at(x, 0, j, 0)), vec_or(y, ny, &|i| at(y, i, 0, 0)), vec_or(z, nz, &|k| at(z, 0, 0, k)))
        }
        None => ((1..=nx).map(|v| v as f64).collect(), (1..=ny).map(|v| v as f64).collect(), (1..=nz).map(|v| v as f64).collect()),
    }
}

/// (X, Y, Z, V, rest...) or (V, rest...): the volume and the remaining arguments.
fn vol_args<'a>(a: &'a [Value], name: &str) -> R<(Vol, &'a [Value])> {
    let lead = a.iter().take_while(|v| num(v) && v.numel() > 1).count();
    let (coords, vi) = if lead >= 4 { (Some((mat(&a[0])?, mat(&a[1])?, mat(&a[2])?)), 3) } else { (None, 0) };
    let v = mat(arg(a, vi, name)?)?;
    let (xs, ys, zs) = grid_vectors(&v.dims, coords.as_ref().map(|(x, y, z)| (x, y, z)));
    Ok((Vol { xs, ys, zs, v: v.data }, &a[vi + 1..]))
}

/// A vector field: (X, Y, Z, U, V, W, rest...) or (U, V, W, rest...) (2-D: X, Y, U, V).
struct Field {
    xs: Vec<f64>,
    ys: Vec<f64>,
    zs: Vec<f64>,
    comps: Vec<Vol>,
}

/// `coords`: whether the grid is given (None: when 2*n leading arrays are present).
fn field_args<'a>(a: &'a [Value], two_d: bool, coords: Option<bool>, name: &str) -> R<(Field, &'a [Value])> {
    let need = if two_d { 2 } else { 3 };
    let big = a.iter().take_while(|v| num(v) && v.numel() > 1).count();
    let with_coords = coords.unwrap_or(big >= 2 * need);
    let start = if with_coords { need } else { 0 };
    let comps_m: Vec<Matrix> = (0..need).map(|k| mat(arg(a, start + k, name)?)).collect::<R<_>>()?;
    let dims = comps_m[0].dims.clone();
    let (xs, ys, zs) = if with_coords {
        let (x, y) = (mat(&a[0])?, mat(&a[1])?);
        let z = if two_d { Matrix::row(vec![1.0]) } else { mat(&a[2])? };
        grid_vectors(&dims, Some((&x, &y, &z)))
    } else {
        grid_vectors(&dims, None)
    };
    let comps = comps_m.into_iter().map(|m| Vol { xs: xs.clone(), ys: ys.clone(), zs: zs.clone(), v: m.data }).collect();
    Ok((Field { xs, ys, zs, comps }, &a[start + need..]))
}

// ---- isosurfaces -------------------------------------------------------------------------------

/// Marching tetrahedra: (faces, vertices) of the surface V = iso.
fn iso_extract(vol: &Vol, iso: f64) -> (Vec<Vec<usize>>, Vec<[f64; 3]>) {
    let (ny, nx, nz) = vol.dims();
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut key: HashMap<(usize, usize), usize> = HashMap::new();
    let mut faces = Vec::new();
    let gid = |i: usize, j: usize, k: usize| k * ny * nx + j * ny + i;
    const TETS: [[usize; 4]; 6] = [[0, 1, 3, 7], [0, 3, 2, 7], [0, 2, 6, 7], [0, 6, 4, 7], [0, 4, 5, 7], [0, 5, 1, 7]];
    for k in 0..nz.saturating_sub(1) {
        for j in 0..nx.saturating_sub(1) {
            for i in 0..ny.saturating_sub(1) {
                // Corner c: bit 0 along x (column), bit 1 along y (row), bit 2 along z.
                let corner = |c: usize| (i + (c >> 1 & 1), j + (c & 1), k + (c >> 2 & 1));
                for t in TETS {
                    let ids: Vec<(usize, usize, usize)> = t.iter().map(|&c| corner(c)).collect();
                    let vals: Vec<f64> = ids.iter().map(|&(a, b, c)| vol.at(a, b, c)).collect();
                    if vals.iter().any(|v| v.is_nan()) {
                        continue;
                    }
                    let inside: Vec<usize> = (0..4).filter(|&q| vals[q] < iso).collect();
                    let outside: Vec<usize> = (0..4).filter(|&q| vals[q] >= iso).collect();
                    let mut cut = |p: usize, q: usize| -> usize {
                        let (gp, gq) = (gid(ids[p].0, ids[p].1, ids[p].2), gid(ids[q].0, ids[q].1, ids[q].2));
                        let k2 = (gp.min(gq), gp.max(gq));
                        if let Some(&v) = key.get(&k2) {
                            return v;
                        }
                        let t = (iso - vals[p]) / (vals[q] - vals[p]);
                        let (a, b) = (vol.point(ids[p].0, ids[p].1, ids[p].2), vol.point(ids[q].0, ids[q].1, ids[q].2));
                        verts.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), a[2] + t * (b[2] - a[2])]);
                        key.insert(k2, verts.len() - 1);
                        verts.len() - 1
                    };
                    match (inside.len(), outside.len()) {
                        (1, 3) => {
                            let p = inside[0];
                            faces.push(outside.iter().map(|&q| cut(p, q)).collect());
                        }
                        (3, 1) => {
                            let q = outside[0];
                            faces.push(inside.iter().map(|&p| cut(p, q)).collect());
                        }
                        (2, 2) => {
                            let (a, b, c, d) = (inside[0], inside[1], outside[0], outside[1]);
                            let (e1, e2, e3, e4) = (cut(a, c), cut(a, d), cut(b, d), cut(b, c));
                            faces.push(vec![e1, e2, e3]);
                            faces.push(vec![e1, e3, e4]);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // Triangles collapsed onto a grid point go.
    faces.retain(|f: &Vec<usize>| f[0] != f[1] && f[1] != f[2] && f[0] != f[2] && !(verts[f[0]] == verts[f[1]] || verts[f[1]] == verts[f[2]] || verts[f[0]] == verts[f[2]]));
    (faces, verts)
}

fn fv_struct(faces: &[Vec<usize>], verts: &[[f64; 3]], cdata: Option<Vec<f64>>, vertices_first: bool) -> Value {
    let mut s = StructArray::scalar();
    if vertices_first {
        s.set("vertices", rows3(verts));
        s.set("faces", index_rows(faces));
    } else {
        s.set("faces", index_rows(faces));
        s.set("vertices", rows3(verts));
    }
    if let Some(c) = cdata {
        s.set("facevertexcdata", Value::Num(Matrix::col(c)));
    }
    Value::Struct(s)
}

/// Draws faces and vertices as one patch (colored per face from `cdata`, or by one value).
fn draw_patch(it: &mut Interpreter, faces: &[Vec<usize>], verts: &[[f64; 3]], color: Option<Vec<f64>>, rgb: Option<[u8; 3]>, nargout: usize) -> Ret {
    let (mut x, mut y, mut z) = (Vec::new(), Vec::new(), Vec::new());
    for (n, f) in faces.iter().enumerate() {
        if n > 0 {
            x.push(f64::NAN);
            y.push(f64::NAN);
            z.push(f64::NAN);
        }
        for &v in f {
            x.push(verts[v][0]);
            y.push(verts[v][1]);
            z.push(verts[v][2]);
        }
    }
    let mut s = Series::new(SeriesKind::Patch, x, y);
    s.z = z;
    s.line_style = LineStyle::Solid;
    s.edge_color = None;
    match (color, rgb) {
        (_, Some(c)) => s.color = Some(c),
        (Some(c), None) => {
            s.c = c;
            s.face_mapped = true;
        }
        (None, None) => {}
    }
    crate::builtins_graph3d::add_3d(it, vec![s], &[], nargout)
}

fn isosurface(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "isosurface")?;
    let nums: Vec<&Value> = rest.iter().filter(|v| num(v)).collect();
    // The isovalue: given, or the mean of the data.
    let iso = match nums.first().filter(|v| v.numel() == 1) {
        Some(v) => v.scalar()?,
        None => {
            let f: Vec<f64> = vol.v.iter().copied().filter(|v| v.is_finite()).collect();
            f.iter().sum::<f64>() / f.len().max(1) as f64
        }
    };
    let colors = nums.iter().find(|v| v.numel() == vol.v.len()).map(|v| mat(v)).transpose()?;
    let (faces, verts) = iso_extract(&vol, iso);
    let cdata = colors.map(|c| {
        let cv = Vol { v: c.data, ..vol.clone() };
        verts.iter().map(|&p| cv.interp(p)).collect::<Vec<f64>>()
    });
    match nargout {
        0 => {
            let per_face = cdata.as_ref().map(|c| faces.iter().map(|f| f.iter().map(|&v| c[v]).sum::<f64>() / f.len() as f64).collect());
            draw_patch(it, &faces, &verts, per_face.or(Some(vec![iso])), None, 0)?;
            none()
        }
        1 => one(fv_struct(&faces, &verts, cdata, true)),
        _ => {
            let mut out = vec![index_rows(&faces), rows3(&verts)];
            if let Some(c) = cdata {
                out.push(Value::Num(Matrix::col(c)));
            }
            Ok(Some(out))
        }
    }
}

/// The part of a square cell (corners in order) on the kept side of `iso`, as a polygon of
/// (corner, None) and (corner, Some(next corner)) crossing points.
fn clip_cell(vals: [f64; 4], iso: f64, above: bool) -> Vec<(usize, Option<usize>)> {
    let keep = |v: f64| if above { v >= iso } else { v <= iso };
    let mut out = Vec::new();
    for c in 0..4 {
        let n = (c + 1) % 4;
        if keep(vals[c]) {
            out.push((c, None));
        }
        if keep(vals[c]) != keep(vals[n]) {
            out.push((c, Some(n)));
        }
    }
    out
}

fn isocaps(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "isocaps")?;
    let iso = rest.iter().find(|v| num(v)).map(|v| v.scalar()).transpose()?.unwrap_or(0.0);
    let words: Vec<String> = rest.iter().filter(|v| v.is_text()).map(|v| v.text().map(|t| t.to_lowercase())).collect::<R<_>>()?;
    let above = !words.iter().any(|w| w == "below");
    let which: Vec<&str> = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"].into_iter().filter(|w| words.iter().any(|x| x == w)).collect();
    let planes: Vec<&str> = if which.is_empty() { vec!["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"] } else { which };
    let (ny, nx, nz) = vol.dims();
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut cdata = Vec::new();
    let mut index: HashMap<[u64; 3], usize> = HashMap::new();
    let mut faces = Vec::new();
    for plane in planes {
        // The plane as a 2-D grid of (row, col, page) triples.
        let (n1, n2): (usize, usize);
        let pick: Box<dyn Fn(usize, usize) -> (usize, usize, usize)> = match plane {
            "xmin" | "xmax" => {
                let j = if plane == "xmin" { 0 } else { nx - 1 };
                (n1, n2) = (ny, nz);
                Box::new(move |u, w| (u, j, w))
            }
            "ymin" | "ymax" => {
                let i = if plane == "ymin" { 0 } else { ny - 1 };
                (n1, n2) = (nx, nz);
                Box::new(move |u, w| (i, u, w))
            }
            _ => {
                let k = if plane == "zmin" { 0 } else { nz - 1 };
                (n1, n2) = (ny, nx);
                Box::new(move |u, w| (u, w, k))
            }
        };
        for u in 0..n1.saturating_sub(1) {
            for w in 0..n2.saturating_sub(1) {
                let corners = [pick(u, w), pick(u + 1, w), pick(u + 1, w + 1), pick(u, w + 1)];
                let vals = corners.map(|(i, j, k)| vol.at(i, j, k));
                if vals.iter().any(|v| v.is_nan()) {
                    continue;
                }
                let poly = clip_cell(vals, iso, above);
                if poly.len() < 3 {
                    continue;
                }
                let mut ids = Vec::new();
                for (c, cross) in poly {
                    let (i, j, k) = corners[c];
                    let p0 = vol.point(i, j, k);
                    let (p, val) = match cross {
                        None => (p0, vals[c]),
                        Some(n) => {
                            let (i2, j2, k2) = corners[n];
                            let p1 = vol.point(i2, j2, k2);
                            let t = (iso - vals[c]) / (vals[n] - vals[c]);
                            ([p0[0] + t * (p1[0] - p0[0]), p0[1] + t * (p1[1] - p0[1]), p0[2] + t * (p1[2] - p0[2])], iso)
                        }
                    };
                    let k3 = p.map(f64::to_bits);
                    let id = *index.entry(k3).or_insert_with(|| {
                        verts.push(p);
                        cdata.push(val);
                        verts.len() - 1
                    });
                    ids.push(id);
                }
                for t in 1..ids.len() - 1 {
                    faces.push(vec![ids[0], ids[t], ids[t + 1]]);
                }
            }
        }
    }
    if nargout == 0 {
        let per_face: Vec<f64> = faces.iter().map(|f: &Vec<usize>| f.iter().map(|&v| cdata[v]).sum::<f64>() / 3.0).collect();
        draw_patch(it, &faces, &verts, Some(per_face), None, 0)?;
        return none();
    }
    if nargout == 1 {
        return one(fv_struct(&faces, &verts, Some(cdata), true));
    }
    Ok(Some(vec![index_rows(&faces), rows3(&verts), Value::Num(Matrix::col(cdata))]))
}

/// Gradient of a volume along x, y and z (MATLAB's `gradient` with the grid spacing).
fn gradient3(it: &mut Interpreter, vol: &Vol) -> R<[Vol; 3]> {
    let (ny, nx, nz) = vol.dims();
    let m = Value::Num(Matrix::from_dims(vec![ny, nx, nz], vol.v.clone()));
    let row = |v: &[f64]| Value::Num(Matrix::row(v.to_vec()));
    let mut args = vec![m, row(&vol.xs), row(&vol.ys)];
    if nz > 1 {
        args.push(row(&vol.zs));
    }
    let g = it.call_function("gradient", args, 3)?;
    let comp = |k: usize| -> R<Vol> {
        Ok(match g.get(k) {
            Some(v) => Vol { v: v.to_matrix()?.data, ..vol.clone() },
            None => Vol { v: vec![0.0; vol.v.len()], ..vol.clone() },
        })
    };
    Ok([comp(0)?, comp(1)?, comp(2)?])
}

fn isonormals(it: &mut Interpreter, a: &[Value]) -> Ret {
    let (vol, rest) = vol_args(a, "isonormals")?;
    let negate = rest.iter().any(|v| v.text().is_ok_and(|t| t.eq_ignore_ascii_case("negate")));
    let verts = match rest.iter().find(|v| !v.is_text()) {
        Some(Value::Struct(s)) => s.get("vertices").ok_or_else(|| LangError::new("The patch structure has no vertices.", 0))?.to_matrix()?,
        Some(v) if gfx::is_gfx(v) => it.call_function("get", vec![v.clone(), Value::chars("Vertices")], 1)?.remove(0).to_matrix()?,
        Some(v) => v.to_matrix()?,
        None => return rt("Not enough input arguments."),
    };
    let g = gradient3(it, &vol)?;
    let sign = if negate { 1.0 } else { -1.0 };
    let n: Vec<[f64; 3]> = read_rows(&verts).iter().map(|p| {
        let q = [p[0], p[1], *p.get(2).unwrap_or(&0.0)];
        [sign * g[0].interp(q), sign * g[1].interp(q), sign * g[2].interp(q)]
    }).collect();
    one(rows3(&n))
}

fn isocolors(a: &[Value]) -> Ret {
    // (X, Y, Z, C, verts), (C, verts), and the RGB forms (X, Y, Z, R, G, B, verts), (R, G, B, verts).
    let verts_v = a.last().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
    let verts = match verts_v {
        Value::Struct(s) => s.get("vertices").ok_or_else(|| LangError::new("The patch structure has no vertices.", 0))?.to_matrix()?,
        v => v.to_matrix()?,
    };
    let body = &a[..a.len() - 1];
    let (coords, chans): (Option<(Matrix, Matrix, Matrix)>, Vec<Matrix>) = match body.len() {
        1 | 3 => (None, body.iter().map(mat).collect::<R<_>>()?),
        4 | 6 => (Some((mat(&body[0])?, mat(&body[1])?, mat(&body[2])?)), body[3..].iter().map(mat).collect::<R<_>>()?),
        _ => return rt("Wrong number of input arguments."),
    };
    let dims = chans[0].dims.clone();
    let (xs, ys, zs) = grid_vectors(&dims, coords.as_ref().map(|(x, y, z)| (x, y, z)));
    let pts = read_rows(&verts);
    let n = pts.len();
    let mut out = vec![0.0; n * chans.len()];
    for (c, ch) in chans.iter().enumerate() {
        let vol = Vol { xs: xs.clone(), ys: ys.clone(), zs: zs.clone(), v: ch.data.clone() };
        for (i, p) in pts.iter().enumerate() {
            out[c * n + i] = vol.interp([p[0], p[1], *p.get(2).unwrap_or(&1.0)]);
        }
    }
    one(Value::Num(Matrix::new(n, chans.len(), out)))
}

// ---- volume utilities ----------------------------------------------------------------------

fn smooth3(a: &[Value]) -> Ret {
    let v = mat(arg(a, 0, "smooth3")?)?;
    let filt = a.get(1).map(|f| f.text()).transpose()?.unwrap_or_else(|| "box".into()).to_lowercase();
    let size: Vec<usize> = match a.get(2) {
        Some(s) => {
            let d = s.to_matrix()?.data;
            if d.len() == 1 { vec![d[0] as usize; 3] } else { d.iter().map(|&x| x as usize).collect() }
        }
        None => vec![3; 3],
    };
    if size.iter().any(|&s| s % 2 == 0) {
        return rt("The size must be odd.");
    }
    let sd = a.get(3).map(|s| s.scalar()).transpose()?.unwrap_or(0.65);
    let dims = [v.dims[0], v.dims.get(1).copied().unwrap_or(1), v.dims.get(2).copied().unwrap_or(1)];
    // Separable 1-D kernels along rows, columns and pages.
    let kernel = |n: usize| -> Vec<f64> {
        let h = (n / 2) as i64;
        let w: Vec<f64> = (-h..=h).map(|t| if filt == "gaussian" { (-(t * t) as f64 / (2.0 * sd * sd)).exp() } else { 1.0 }).collect();
        let s: f64 = w.iter().sum();
        w.iter().map(|x| x / s).collect()
    };
    if filt != "box" && filt != "gaussian" {
        return rt("The filter must be 'box' or 'gaussian'.");
    }
    let mut data = v.data.clone();
    for axis in 0..3 {
        let k = kernel(size[axis]);
        let h = (k.len() / 2) as i64;
        let mut out = vec![0.0; data.len()];
        let stride = [1, dims[0], dims[0] * dims[1]][axis];
        for p in 0..data.len() {
            let pos = (p / stride) % dims[axis];
            let base = p - pos * stride;
            // Replicate padding at the edges.
            out[p] = k.iter().enumerate().map(|(t, w)| {
                let q = (pos as i64 + t as i64 - h).clamp(0, dims[axis] as i64 - 1) as usize;
                w * data[base + q * stride]
            }).sum();
        }
        data = out;
    }
    one(Value::Num(Matrix::from_dims(v.dims.clone(), data)))
}

/// Indices of the grid kept along one axis (limits are inclusive; NaN means no limit).
fn keep_range(g: &[f64], lo: f64, hi: f64) -> Vec<usize> {
    (0..g.len()).filter(|&k| (lo.is_nan() || g[k] >= lo) && (hi.is_nan() || g[k] <= hi)).collect()
}

fn gather(vol: &Vol, ri: &[usize], ci: &[usize], pi: &[usize]) -> Vec<f64> {
    let mut out = Vec::with_capacity(ri.len() * ci.len() * pi.len());
    for &k in pi {
        for &j in ci {
            for &i in ri {
                out.push(vol.at(i, j, k));
            }
        }
    }
    out
}

/// Meshgrid arrays and values of the kept part of a volume.
fn sub_outputs(vol: &Vol, ri: &[usize], ci: &[usize], pi: &[usize], coords: bool, nargout: usize) -> Ret {
    let dims = vec![ri.len(), ci.len(), pi.len()];
    let v = Value::Num(Matrix::from_dims(dims.clone(), gather(vol, ri, ci, pi)));
    if nargout <= 1 {
        return one(v);
    }
    let _ = coords;
    let mut gx = Vec::new();
    let mut gy = Vec::new();
    let mut gz = Vec::new();
    for &k in pi {
        for &j in ci {
            for &i in ri {
                gx.push(vol.xs[j]);
                gy.push(vol.ys[i]);
                gz.push(vol.zs[k]);
            }
        }
    }
    let m = |d: Vec<f64>| Value::Num(Matrix::from_dims(dims.clone(), d));
    Ok(Some(vec![m(gx), m(gy), m(gz), v]))
}

fn subvolume(a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "subvolume")?;
    let l = mat(arg(rest, 0, "subvolume")?)?.data;
    if l.len() != 6 {
        return rt("LIMITS must be [xmin xmax ymin ymax zmin zmax].");
    }
    let (ci, ri, pi) = (keep_range(&vol.xs, l[0], l[1]), keep_range(&vol.ys, l[2], l[3]), keep_range(&vol.zs, l[4], l[5]));
    sub_outputs(&vol, &ri, &ci, &pi, true, nargout)
}

fn reducevolume(a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "reducevolume")?;
    let r = mat(arg(rest, 0, "reducevolume")?)?.data;
    let (rx, ry, rz) = if r.len() == 3 { (r[0], r[1], r[2]) } else { (r[0], r[0], r[0]) };
    let step = |n: usize, s: f64| -> Vec<usize> { (0..n).step_by((s.max(1.0)) as usize).collect() };
    let (ny, nx, nz) = vol.dims();
    sub_outputs(&vol, &step(ny, ry), &step(nx, rx), &step(nz, rz), true, nargout)
}

fn volumebounds(a: &[Value]) -> Ret {
    let ms: Vec<Matrix> = a.iter().filter(|v| num(v)).map(mat).collect::<R<_>>()?;
    let lo_hi = |d: &[f64]| {
        let f: Vec<f64> = d.iter().copied().filter(|v| v.is_finite()).collect();
        (f.iter().copied().fold(f64::INFINITY, f64::min), f.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    };
    let (coords, values): (Vec<Matrix>, Vec<Matrix>) = match ms.len() {
        1 => {
            let (xs, ys, zs) = grid_vectors(&ms[0].dims, None);
            (vec![Matrix::row(xs), Matrix::row(ys), Matrix::row(zs)], vec![ms[0].clone()])
        }
        3 => {
            let (xs, ys, zs) = grid_vectors(&ms[0].dims, None);
            (vec![Matrix::row(xs), Matrix::row(ys), Matrix::row(zs)], Vec::new())
        }
        4 => (ms[..3].to_vec(), vec![ms[3].clone()]),
        6 => (ms[..3].to_vec(), Vec::new()),
        _ => return rt("Wrong number of input arguments."),
    };
    let mut out = Vec::new();
    for c in &coords {
        let (lo, hi) = lo_hi(&c.data);
        out.extend([lo, hi]);
    }
    if let Some(v) = values.first() {
        let (lo, hi) = lo_hi(&v.data);
        out.extend([lo, hi]);
    }
    one(Value::Num(Matrix::row(out)))
}

// ---- patch utilities ---------------------------------------------------------------------------

/// Faces and vertices of a patch given as a struct, (F, V), or a patch handle.
fn fv_args(it: &mut Interpreter, a: &[Value]) -> R<(Vec<Vec<usize>>, Vec<[f64; 3]>, usize)> {
    let to_faces = |m: &Matrix| -> Vec<Vec<usize>> { read_rows(m).into_iter().map(|r| r.into_iter().filter(|v| v.is_finite()).map(|v| v as usize - 1).collect()).collect() };
    let to_verts = |m: &Matrix| -> Vec<[f64; 3]> { read_rows(m).into_iter().map(|r| [r[0], *r.get(1).unwrap_or(&0.0), *r.get(2).unwrap_or(&0.0)]).collect() };
    match a.first() {
        Some(Value::Struct(s)) => {
            let f = s.get("faces").ok_or_else(|| LangError::new("The structure must have faces and vertices fields.", 0))?.to_matrix()?;
            let v = s.get("vertices").ok_or_else(|| LangError::new("The structure must have faces and vertices fields.", 0))?.to_matrix()?;
            Ok((to_faces(&f), to_verts(&v), 1))
        }
        Some(h) if gfx::is_gfx(h) => {
            let f = it.call_function("get", vec![h.clone(), Value::chars("Faces")], 1)?.remove(0).to_matrix()?;
            let v = it.call_function("get", vec![h.clone(), Value::chars("Vertices")], 1)?.remove(0).to_matrix()?;
            Ok((to_faces(&f), to_verts(&v), 1))
        }
        Some(f) => {
            let v = mat(arg(a, 1, "patch")?)?;
            Ok((to_faces(&mat(f)?), to_verts(&v), 2))
        }
        None => rt("Not enough input arguments."),
    }
}

fn fv_out(faces: &[Vec<usize>], verts: &[[f64; 3]], two_d: bool, nargout: usize) -> Ret {
    let vv = if two_d {
        let n = verts.len();
        let mut d = vec![0.0; n * 2];
        for (i, p) in verts.iter().enumerate() {
            d[i] = p[0];
            d[n + i] = p[1];
        }
        Value::Num(Matrix::new(n, 2, d))
    } else {
        rows3(verts)
    };
    if nargout <= 1 {
        let mut s = StructArray::scalar();
        s.set("faces", index_rows(faces));
        s.set("vertices", vv);
        return one(Value::Struct(s));
    }
    Ok(Some(vec![index_rows(faces), vv]))
}

fn surf2patch(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let tri = a.iter().any(|v| v.text().is_ok_and(|t| t.eq_ignore_ascii_case("triangles")));
    let nums: Vec<Matrix> = if let Some(h) = a.first().filter(|v| gfx::is_gfx(v)) {
        ["XData", "YData", "ZData", "CData"].iter().map(|p| it.call_function("get", vec![h.clone(), Value::chars(p)], 1).and_then(|mut r| r.remove(0).to_matrix())).collect::<R<_>>()?
    } else {
        a.iter().filter(|v| num(v)).map(mat).collect::<R<_>>()?
    };
    let (x, y, z, c) = match nums.len() {
        1 | 2 => {
            let z = nums[0].clone();
            let (m, n) = (z.rows(), z.cols());
            let x = Matrix::new(m, n, (0..m * n).map(|k| (k / m + 1) as f64).collect());
            let y = Matrix::new(m, n, (0..m * n).map(|k| (k % m + 1) as f64).collect());
            (x, y, z, nums.get(1).cloned())
        }
        _ => (nums[0].clone(), nums[1].clone(), nums[2].clone(), nums.get(3).cloned()),
    };
    let (m, n) = (z.rows(), z.cols());
    let grid = |v: &Matrix, k: usize, along_cols: bool| -> f64 {
        if v.numel() == m * n {
            v.data[k]
        } else if along_cols {
            v.data[k / m]
        } else {
            v.data[k % m]
        }
    };
    let verts: Vec<[f64; 3]> = (0..m * n).map(|k| [grid(&x, k, true), grid(&y, k, false), z.data[k]]).collect();
    let idx = |i: usize, j: usize| j * m + i;
    let mut faces = Vec::new();
    for j in 0..n.saturating_sub(1) {
        for i in 0..m.saturating_sub(1) {
            let (a0, b0, c0, d0) = (idx(i, j), idx(i, j + 1), idx(i + 1, j + 1), idx(i + 1, j));
            if tri {
                faces.push(vec![a0, b0, c0]);
                faces.push(vec![a0, c0, d0]);
            } else {
                faces.push(vec![a0, b0, c0, d0]);
            }
        }
    }
    let cdata = c.filter(|c| c.numel() == m * n).map(|c| c.data);
    if nargout <= 1 {
        let mut s = StructArray::scalar();
        s.set("faces", index_rows(&faces));
        s.set("vertices", rows3(&verts));
        if let Some(c) = cdata {
            s.set("facevertexcdata", Value::Num(Matrix::col(c)));
        }
        return one(Value::Struct(s));
    }
    let mut out = vec![index_rows(&faces), rows3(&verts)];
    if let Some(c) = cdata {
        out.push(Value::Num(Matrix::col(c)));
    }
    Ok(Some(out))
}

fn shrinkfaces(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (faces, verts, used) = fv_args(it, a)?;
    let two_d = match a.first() {
        Some(Value::Struct(s)) => s.get("vertices").is_some_and(|v| v.dims().get(1) == Some(&2)),
        _ => a.get(1).is_some_and(|v| v.dims().get(1) == Some(&2)),
    };
    let sf = a.get(used).map(|v| v.scalar()).transpose()?.unwrap_or(0.3);
    let k = sf.sqrt();
    let (mut nf, mut nv) = (Vec::new(), Vec::new());
    for f in &faces {
        let n = f.len() as f64;
        let c = [0, 1, 2].map(|d| f.iter().map(|&v| verts[v][d]).sum::<f64>() / n);
        let mut ids = Vec::new();
        for &v in f {
            nv.push([0, 1, 2].map(|d| c[d] + (verts[v][d] - c[d]) * k));
            ids.push(nv.len() - 1);
        }
        nf.push(ids);
    }
    if nargout == 0 {
        return draw_patch(it, &nf, &nv, None, None, 0).map(|_| Some(Vec::new()));
    }
    fv_out(&nf, &nv, two_d, nargout)
}

/// Vertex clustering on a grid of `cells` per side: the faces that remain and the merged
/// vertices.
fn cluster(faces: &[Vec<usize>], verts: &[[f64; 3]], cells: f64) -> (Vec<Vec<usize>>, Vec<[f64; 3]>) {
    let lo = [0, 1, 2].map(|d| verts.iter().map(|p| p[d]).fold(f64::INFINITY, f64::min));
    let hi = [0, 1, 2].map(|d| verts.iter().map(|p| p[d]).fold(f64::NEG_INFINITY, f64::max));
    let size = (0..3).map(|d| hi[d] - lo[d]).fold(0.0f64, f64::max).max(f64::MIN_POSITIVE) / cells;
    let mut cell_of: HashMap<[i64; 3], usize> = HashMap::new();
    let mut sums: Vec<([f64; 3], f64)> = Vec::new();
    let map: Vec<usize> = verts
        .iter()
        .map(|p| {
            let key = [0, 1, 2].map(|d| ((p[d] - lo[d]) / size).floor() as i64);
            let id = *cell_of.entry(key).or_insert_with(|| {
                sums.push(([0.0; 3], 0.0));
                sums.len() - 1
            });
            for d in 0..3 {
                sums[id].0[d] += p[d];
            }
            sums[id].1 += 1.0;
            id
        })
        .collect();
    let nv: Vec<[f64; 3]> = sums.iter().map(|(s, n)| s.map(|x| x / n)).collect();
    let mut seen = std::collections::HashSet::new();
    let nf: Vec<Vec<usize>> = faces
        .iter()
        .filter_map(|f| {
            let mut g: Vec<usize> = Vec::new();
            for &v in f {
                let c = map[v];
                if g.last() != Some(&c) && g.first() != Some(&c) {
                    g.push(c);
                }
            }
            if g.len() < 3 {
                return None;
            }
            let mut key = g.clone();
            key.sort_unstable();
            seen.insert(key).then_some(g)
        })
        .collect();
    (nf, nv)
}

fn reducepatch(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (faces, verts, used) = fv_args(it, a)?;
    let r = a.get(used).map(|v| v.scalar()).transpose()?.unwrap_or(0.5);
    let target = if r < 1.0 { (r * faces.len() as f64).floor() as usize } else { r as usize };
    let mut cells = (verts.len() as f64).cbrt() * 4.0;
    let mut best = (faces.clone(), verts.clone());
    while best.0.len() > target && cells >= 1.0 {
        best = cluster(&faces, &verts, cells);
        cells *= 0.85;
    }
    // Unused vertices are dropped and the faces renumbered.
    let mut used_v: Vec<usize> = best.0.iter().flatten().copied().collect();
    used_v.sort_unstable();
    used_v.dedup();
    let renum: HashMap<usize, usize> = used_v.iter().enumerate().map(|(n, &v)| (v, n)).collect();
    let nf: Vec<Vec<usize>> = best.0.iter().map(|f| f.iter().map(|v| renum[v]).collect()).collect();
    let nv: Vec<[f64; 3]> = used_v.iter().map(|&v| best.1[v]).collect();
    if nargout == 0 {
        return draw_patch(it, &nf, &nv, None, None, 0).map(|_| Some(Vec::new()));
    }
    fv_out(&nf, &nv, false, nargout)
}

/// `patch(fv)` and `patch('Faces', F, 'Vertices', V, ...)`: None when the call has neither.
pub fn patch_fv(it: &mut Interpreter, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let (fv, props): (Option<(Matrix, Matrix, Option<Matrix>)>, Vec<Value>) = match a.first() {
        Some(Value::Struct(s)) => {
            let get = |n: &str| s.fields.iter().position(|f| f.eq_ignore_ascii_case(n)).and_then(|i| s.elems.first().map(|e| e[i].clone()));
            let (Some(f), Some(v)) = (get("faces"), get("vertices")) else { return Ok(None) };
            (Some((f.to_matrix()?, v.to_matrix()?, get("facevertexcdata").map(|c| c.to_matrix()).transpose()?)), a[1..].to_vec())
        }
        _ => {
            let pos = |n: &str| (0..a.len()).step_by(1).find(|&i| a[i].text().is_ok_and(|t| t.eq_ignore_ascii_case(n)) && i + 1 < a.len());
            let (Some(fi), Some(vi)) = (pos("Faces"), pos("Vertices")) else { return Ok(None) };
            let ci = pos("FaceVertexCData");
            let rest: Vec<Value> = a
                .chunks(2)
                .filter(|p| p[0].text().map(|t| !["faces", "vertices", "facevertexcdata"].contains(&t.to_lowercase().as_str())).unwrap_or(true))
                .flatten()
                .cloned()
                .collect();
            (Some((a[fi + 1].to_matrix()?, a[vi + 1].to_matrix()?, ci.map(|c| a[c + 1].to_matrix()).transpose()?)), rest)
        }
    };
    let Some((f, v, c)) = fv else { return Ok(None) };
    let faces: Vec<Vec<usize>> = read_rows(&f).into_iter().map(|r| r.into_iter().filter(|x| x.is_finite()).map(|x| x as usize - 1).collect()).collect();
    let verts: Vec<[f64; 3]> = read_rows(&v).into_iter().map(|r| [r[0], *r.get(1).unwrap_or(&0.0), *r.get(2).unwrap_or(&0.0)]).collect();
    if faces.iter().flatten().any(|&k| k >= verts.len()) {
        return rt("Faces refers to a vertex that does not exist.");
    }
    // Per-face color: one value per face, or the mean of the face's vertex values.
    let per_face = c.filter(|c| c.cols() == 1).map(|c| {
        if c.numel() == faces.len() && c.numel() != verts.len() {
            c.data.clone()
        } else {
            faces.iter().map(|fc| fc.iter().map(|&k| c.data.get(k).copied().unwrap_or(f64::NAN)).sum::<f64>() / fc.len() as f64).collect()
        }
    });
    let h = draw_patch(it, &faces, &verts, per_face, None, 1)?;
    let id = h.as_ref().and_then(|v| v.first()).and_then(gfx::ids_of).and_then(|i| i.first().copied()).unwrap_or(0);
    it.gfx_store.entry(id).or_default().extend([("Faces".to_string(), Value::Num(f)), ("Vertices".to_string(), Value::Num(v))]);
    if !props.is_empty() {
        let mut args = vec![gfx::handle(it, id)];
        args.extend(props);
        it.call_function("set", args, 0)?;
    }
    Ok(Some(if nargout > 0 { vec![gfx::handle(it, id)] } else { Vec::new() }))
}

// ---- stream lines ------------------------------------------------------------------------------

/// Index-space velocity at fractional (row, col, page) indices.
fn vel(f: &Field, fi: f64, fj: f64, fk: f64) -> Vec<f64> {
    let sp = |g: &[f64], t: f64| -> f64 {
        let n = g.len();
        if n < 2 {
            return 1.0;
        }
        let a = (t.floor() as usize).min(n - 2);
        g[a + 1] - g[a]
    };
    let dx = sp(&f.xs, fj);
    let dy = sp(&f.ys, fi);
    let dz = sp(&f.zs, fk);
    let u = f.comps[0].interp_idx(fi, fj, fk) / dx;
    let v = f.comps[1].interp_idx(fi, fj, fk) / dy;
    if f.comps.len() == 3 {
        vec![u, v, f.comps[2].interp_idx(fi, fj, fk) / dz]
    } else {
        vec![u, v]
    }
}

/// Coordinates of a fractional index along a grid.
fn coord(g: &[f64], t: f64) -> f64 {
    let n = g.len();
    if n < 2 {
        return g.first().copied().unwrap_or(t);
    }
    let a = (t.floor() as usize).min(n - 2);
    g[a] + (t - a as f64) * (g[a + 1] - g[a])
}

/// One stream line from `start` (data coordinates).
fn trace(f: &Field, start: &[f64], step: f64, maxvert: usize) -> Vec<Vec<f64>> {
    let three = f.comps.len() == 3;
    let (Some(fx), Some(fy)) = (frac(&f.xs, start[0]), frac(&f.ys, start[1])) else { return Vec::new() };
    let fz = if three { match frac(&f.zs, start[2]) { Some(z) => z, None => return Vec::new() } } else { 0.0 };
    // 0-based index space (the rounding of MATLAB's steps follows from it).
    let mut p = vec![fy, fx, fz];
    let (ny, nx, nz) = ((f.ys.len() - 1) as f64, (f.xs.len() - 1) as f64, (f.zs.len() - 1) as f64);
    let inside = |p: &[f64]| p[0] >= 0.0 && p[0] <= ny && p[1] >= 0.0 && p[1] <= nx && (!three || (p[2] >= 0.0 && p[2] <= nz));
    let mut out = Vec::new();
    while out.len() < maxvert && inside(&p) {
        let pt = if three { vec![coord(&f.xs, p[1]), coord(&f.ys, p[0]), coord(&f.zs, p[2])] } else { vec![coord(&f.xs, p[1]), coord(&f.ys, p[0])] };
        out.push(pt);
        let v = vel(f, p[0], p[1], p[2]);
        let m = v.iter().fold(0.0f64, |a, b| a.max(b.abs()));
        if !(m > 0.0) || v.iter().any(|x| x.is_nan()) {
            break;
        }
        // (u, v, w) move columns, rows and pages.
        p[1] += step * v[0] / m;
        p[0] += step * v[1] / m;
        if three {
            p[2] += step * v[2] / m;
        }
    }
    out
}

fn stream(it: &mut Interpreter, a: &[Value], three: bool) -> R<Vec<Vec<Vec<f64>>>> {
    let name = if three { "stream3" } else { "stream2" };
    let (f, rest) = field_args(a, !three, Some(a.len() >= if three { 9 } else { 6 }), name)?;
    let n_start = if three { 3 } else { 2 };
    let starts: Vec<Matrix> = (0..n_start).map(|k| mat(arg(rest, k, name)?)).collect::<R<_>>()?;
    let opts = rest.get(n_start).map(mat).transpose()?.map(|m| m.data).unwrap_or_default();
    let step = opts.first().copied().unwrap_or(0.1);
    let maxvert = opts.get(1).copied().unwrap_or(10000.0) as usize;
    let _ = it;
    let n = starts[0].numel();
    Ok((0..n).map(|k| {
        let s: Vec<f64> = starts.iter().map(|m| m.data[if m.numel() == 1 { 0 } else { k }]).collect();
        trace(&f, &s, step, maxvert)
    }).collect())
}

fn verts_value(v: &[Vec<f64>]) -> Value {
    let n = v.len();
    let w = v.first().map(|p| p.len()).unwrap_or(2);
    let mut d = vec![0.0; n * w];
    for (i, p) in v.iter().enumerate() {
        for k in 0..w {
            d[k * n + i] = p[k];
        }
    }
    Value::Num(if n == 0 { Matrix::new(0, w, Vec::new()) } else { Matrix::new(n, w, d) })
}

fn cell_of(lines: &[Vec<Vec<f64>>]) -> Value {
    Value::Cell(CellArray::row(lines.iter().map(|l| verts_value(l)).collect()))
}

/// Stream vertices from a cell argument (each n-by-2 or n-by-3).
fn lines_arg(v: &Value) -> R<Vec<Vec<Vec<f64>>>> {
    let items: Vec<Value> = match v {
        Value::Cell(c) => c.data.clone(),
        other => vec![other.clone()],
    };
    items.iter().map(|x| Ok(read_rows(&x.to_matrix()?))).collect()
}

fn streamline(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let lines = match a.first() {
        Some(v @ Value::Cell(_)) => lines_arg(v)?,
        _ => {
            let big = a.iter().take_while(|v| num(v) && v.numel() > 1).count();
            let three = big == 6 || (big == 3 && a.len() >= 6);
            stream(it, a, three)?
        }
    };
    let mut ids = Vec::new();
    for l in &lines {
        let mut s = Series::new(SeriesKind::Line, l.iter().map(|p| p[0]).collect(), l.iter().map(|p| p[1]).collect());
        s.z = if l.first().is_some_and(|p| p.len() == 3) { l.iter().map(|p| p[2]).collect() } else { Vec::new() };
        s.class_hint = Some("matlab.graphics.primitive.Line");
        let three = !s.z.is_empty();
        if three {
            let r = crate::builtins_graph3d::add_3d(it, vec![s], &[], 1)?;
            ids.extend(r.as_ref().and_then(|v| v.first()).and_then(gfx::ids_of).unwrap_or_default());
        } else {
            ids.extend(it.add_series(vec![s], crate::graphics::XAxisKind::Numeric));
        }
    }
    handles_out(it, &ids, nargout)
}

/// A frame perpendicular to direction `d`: two unit vectors.
fn frame(d: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(f64::MIN_POSITIVE);
    let t = d.map(|x| x / l);
    let a = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let cross = |p: [f64; 3], q: [f64; 3]| [p[1] * q[2] - p[2] * q[1], p[2] * q[0] - p[0] * q[2], p[0] * q[1] - p[1] * q[0]];
    let n = cross(t, a);
    let ln = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let n = n.map(|x| x / ln);
    (n, cross(t, n))
}

fn tangent(l: &[Vec<f64>], i: usize) -> [f64; 3] {
    let p = |k: usize| [l[k][0], l[k][1], *l[k].get(2).unwrap_or(&0.0)];
    let (a, b) = (p(i.saturating_sub(1)), p((i + 1).min(l.len() - 1)));
    [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
}

/// A surface series from (n+1)-by-m grids of points (rows along the line).
fn grid_surface(pts: Vec<Vec<[f64; 3]>>, class: &'static str) -> Series {
    let rows = pts.len();
    let cols = pts.first().map(|r| r.len()).unwrap_or(0);
    let at = |d: usize| -> Vec<f64> { (0..cols).flat_map(|j| (0..rows).map(move |i| (i, j))).map(|(i, j)| pts[i][j][d]).collect() };
    let mut s = Series::new(SeriesKind::Surface, at(0), at(1));
    s.z = at(2);
    s.grid = (rows, cols);
    s.c = s.z.clone();
    s.face_mapped = true;
    s.edge_color = None;
    s.class_hint = Some(class);
    s
}

fn streamtube(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let lines = lines_arg(arg(a, 0, "streamtube")?)?;
    let width = a.get(1).filter(|v| num(v)).map(|v| v.scalar()).transpose()?.unwrap_or(1.0);
    let span = lines.iter().flatten().flat_map(|p| p.iter().copied()).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let r = width * 0.05 * (span.1 - span.0).max(1.0) / 2.0;
    let mut series = Vec::new();
    for l in lines.iter().filter(|l| l.len() >= 2) {
        let ring: Vec<Vec<[f64; 3]>> = (0..l.len())
            .map(|i| {
                let (n, b) = frame(tangent(l, i));
                let c = [l[i][0], l[i][1], *l[i].get(2).unwrap_or(&0.0)];
                (0..=20).map(|k| {
                    let t = k as f64 / 20.0 * std::f64::consts::TAU;
                    [0, 1, 2].map(|d| c[d] + r * (t.cos() * n[d] + t.sin() * b[d]))
                }).collect()
            })
            .collect();
        series.push(grid_surface(ring, "matlab.graphics.primitive.Surface"));
    }
    crate::builtins_graph3d::add_3d(it, series, &[], nargout)
}

fn streamribbon(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let lines = lines_arg(arg(a, 0, "streamribbon")?)?;
    let twists: Vec<Vec<f64>> = match a.get(1) {
        Some(Value::Cell(c)) => c.data.iter().map(|v| v.to_matrix().map(|m| m.data)).collect::<R<_>>()?,
        _ => Vec::new(),
    };
    let width = a.iter().skip(1).filter(|v| num(v) && v.numel() == 1).map(|v| v.scalar()).next().transpose()?.unwrap_or(1.0);
    let span = lines.iter().flatten().flat_map(|p| p.iter().copied()).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
    let half = width * 0.05 * (span.1 - span.0).max(1.0);
    let mut series = Vec::new();
    for (li, l) in lines.iter().enumerate().filter(|(_, l)| l.len() >= 2) {
        let rows: Vec<Vec<[f64; 3]>> = (0..l.len())
            .map(|i| {
                let (n, b) = frame(tangent(l, i));
                let tw = twists.get(li).and_then(|t| t.get(i)).copied().unwrap_or(0.0);
                let dir = [0, 1, 2].map(|d| tw.cos() * n[d] + tw.sin() * b[d]);
                let c = [l[i][0], l[i][1], *l[i].get(2).unwrap_or(&0.0)];
                vec![[0, 1, 2].map(|d| c[d] - half * dir[d]), [0, 1, 2].map(|d| c[d] + half * dir[d])]
            })
            .collect();
        series.push(grid_surface(rows, "matlab.graphics.primitive.Surface"));
    }
    crate::builtins_graph3d::add_3d(it, series, &[], nargout)
}

fn streamslice(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (f, _) = field_args(a, true, None, "streamslice")?;
    let (ny, nx) = (f.ys.len(), f.xs.len());
    // Seeds on a coarse grid; each traced both ways for a limited length.
    let every = ((nx.max(ny) as f64) / 8.0).ceil().max(1.0) as usize;
    let back = Field { xs: f.xs.clone(), ys: f.ys.clone(), zs: f.zs.clone(), comps: f.comps.iter().map(|c| Vol { v: c.v.iter().map(|x| -x).collect(), ..c.clone() }).collect() };
    let mut lines = Vec::new();
    for i in (0..ny).step_by(every) {
        for j in (0..nx).step_by(every) {
            let s = [f.xs[j], f.ys[i]];
            let mut fwd = trace(&f, &s, 0.1, (every * 12).max(20));
            let bwd = trace(&back, &s, 0.1, (every * 12).max(20));
            let mut l: Vec<Vec<f64>> = bwd.into_iter().rev().collect();
            if !fwd.is_empty() {
                fwd.remove(0);
            }
            l.extend(fwd);
            if l.len() > 1 {
                lines.push(l);
            }
        }
    }
    let mut ids = Vec::new();
    for l in &lines {
        let mut s = Series::new(SeriesKind::Line, l.iter().map(|p| p[0]).collect(), l.iter().map(|p| p[1]).collect());
        s.class_hint = Some("matlab.graphics.primitive.Line");
        s.color = Some([0, 0, 255]);
        s.line_width = 0.5;
        ids.extend(it.add_series(vec![s], crate::graphics::XAxisKind::Numeric));
    }
    handles_out(it, &ids, nargout)
}

fn streamparticles(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let lines = lines_arg(arg(a, 0, "streamparticles")?)?;
    let n = a.get(1).filter(|v| num(v)).map(|v| v.scalar()).transpose()?.unwrap_or(1.0).max(1.0) as usize;
    let (mut x, mut y, mut z) = (Vec::new(), Vec::new(), Vec::new());
    for l in &lines {
        if l.is_empty() {
            continue;
        }
        let stride = (l.len() / n).max(1);
        for p in l.iter().step_by(stride).take(n) {
            x.push(p[0]);
            y.push(p[1]);
            z.push(*p.get(2).unwrap_or(&0.0));
        }
    }
    let mut s = Series::new(SeriesKind::Line, x, y);
    s.z = z;
    s.line_style = LineStyle::None;
    s.marker = Some(crate::graphics::Marker::Circle);
    s.class_hint = Some("matlab.graphics.primitive.Line");
    crate::builtins_graph3d::add_3d(it, vec![s], &[], nargout)
}

fn interpstreamspeed(a: &[Value]) -> Ret {
    // (X, Y, Z, U, V, W, verts[, sf]) or (X, Y, U, V, verts[, sf]) or without the grid.
    let cell_at = a.iter().position(|v| matches!(v, Value::Cell(_))).ok_or_else(|| LangError::new("The stream vertices must be a cell array.", 0))?;
    let three = matches!(cell_at, 3 | 6);
    let (f, _) = field_args(&a[..cell_at], !three, Some(matches!(cell_at, 4 | 6)), "interpstreamspeed")?;
    let sf = a.get(cell_at + 1).map(|v| v.scalar()).transpose()?.unwrap_or(1.0);
    let lines = lines_arg(&a[cell_at])?;
    let speed = |p: &[f64]| -> f64 {
        let q = [p[0], p[1], *p.get(2).unwrap_or(&f.zs[0])];
        let comps: Vec<f64> = f.comps.iter().map(|c| c.interp(q)).collect();
        comps.iter().map(|c| c * c).sum::<f64>().sqrt()
    };
    let out: Vec<Vec<Vec<f64>>> = lines
        .iter()
        .map(|l| {
            if l.len() < 2 {
                return l.clone();
            }
            // Walk the polyline, taking steps of sf * speed.
            let mut res = vec![l[0].clone()];
            let (mut seg, mut t) = (0usize, 0.0f64);
            let mut cur = l[0].clone();
            for _ in 0..100_000 {
                let s = sf * speed(&cur);
                if !(s > 0.0) {
                    break;
                }
                let mut left = s;
                loop {
                    let (a, b) = (&l[seg], &l[seg + 1]);
                    let len = a.iter().zip(b).map(|(x, y)| (y - x).powi(2)).sum::<f64>().sqrt();
                    let rem = len * (1.0 - t);
                    if left <= rem || seg + 2 >= l.len() {
                        let tt = if len > 0.0 { (t + left / len).min(1.0) } else { 1.0 };
                        t = tt;
                        cur = a.iter().zip(b).map(|(x, y)| x + tt * (y - x)).collect();
                        break;
                    }
                    left -= rem;
                    seg += 1;
                    t = 0.0;
                }
                if seg + 2 >= l.len() && t >= 1.0 {
                    break;
                }
                res.push(cur.clone());
            }
            res
        })
        .collect();
    one(cell_of(&out))
}

// ---- slices and cones ------------------------------------------------------------------------

fn slice(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "slice")?;
    let planes: Vec<Vec<f64>> = (0..3).map(|k| rest.get(k).map(|v| v.to_matrix().map(|m| m.data)).transpose().map(|o| o.unwrap_or_default())).collect::<R<_>>()?;
    let (ny, nx, nz) = vol.dims();
    let mut series = Vec::new();
    for (axis, vals) in planes.iter().enumerate() {
        for &c in vals {
            // Rows and columns of the plane: (y, z) for x, (x, z) for y, (y, x) for z.
            let (rows, cols) = match axis {
                0 => (ny, nz),
                1 => (nx, nz),
                _ => (ny, nx),
            };
            let mut pts = vec![vec![[0.0; 3]; cols]; rows];
            for (i, row) in pts.iter_mut().enumerate() {
                for (j, p) in row.iter_mut().enumerate() {
                    *p = match axis {
                        0 => [c, vol.ys[i], vol.zs[j]],
                        1 => [vol.xs[i], c, vol.zs[j]],
                        _ => [vol.xs[j], vol.ys[i], c],
                    };
                }
            }
            let mut s = grid_surface(pts.clone(), "matlab.graphics.primitive.Surface");
            s.c = (0..cols).flat_map(|j| (0..rows).map(move |i| (i, j))).map(|(i, j)| vol.interp(pts[i][j])).collect();
            s.edge_color = Some([0, 0, 0]);
            series.push(s);
        }
    }
    crate::builtins_graph3d::add_3d(it, series, &[], nargout)
}

fn contourslice(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (vol, rest) = vol_args(a, "contourslice")?;
    let planes: Vec<Vec<f64>> = (0..3).map(|k| rest.get(k).map(|v| v.to_matrix().map(|m| m.data)).transpose().map(|o| o.unwrap_or_default())).collect::<R<_>>()?;
    let levels = rest.get(3).cloned();
    let (ny, nx, nz) = vol.dims();
    let mut series = Vec::new();
    for (axis, vals) in planes.iter().enumerate() {
        for &c in vals {
            let (u, w, n1, n2): (&[f64], &[f64], usize, usize) = match axis {
                0 => (&vol.ys, &vol.zs, ny, nz),
                1 => (&vol.xs, &vol.zs, nx, nz),
                _ => (&vol.xs, &vol.ys, nx, ny),
            };
            // A 2-D slice: rows along w, columns along u, as contourc expects.
            let mut z = vec![0.0; n1 * n2];
            for b in 0..n2 {
                for a2 in 0..n1 {
                    let p = match axis {
                        0 => [c, u[a2], w[b]],
                        1 => [u[a2], c, w[b]],
                        _ => [u[a2], w[b], c],
                    };
                    z[a2 * n2 + b] = vol.interp(p);
                }
            }
            let mut args = vec![Value::Num(Matrix::row(u.to_vec())), Value::Num(Matrix::row(w.to_vec())), Value::Num(Matrix::new(n2, n1, z))];
            if let Some(l) = &levels {
                args.push(if l.numel() == 1 && !l.is_text() { Value::Num(Matrix::row(vec![l.scalar()?; 2])) } else { l.clone() });
            }
            let cm = it.call_function("contourc", args, 1)?.remove(0).to_matrix()?;
            let (mut xs, mut ys, mut zs, mut cs) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let mut k = 0;
            while k < cm.cols() {
                let (lev, cnt) = (cm.data[k * 2], cm.data[k * 2 + 1] as usize);
                if !xs.is_empty() {
                    xs.push(f64::NAN);
                    ys.push(f64::NAN);
                    zs.push(f64::NAN);
                    cs.push(f64::NAN);
                }
                for j in k + 1..=k + cnt {
                    let (p, q) = (cm.data[j * 2], cm.data[j * 2 + 1]);
                    let pt = match axis {
                        0 => [c, p, q],
                        1 => [p, c, q],
                        _ => [p, q, c],
                    };
                    xs.push(pt[0]);
                    ys.push(pt[1]);
                    zs.push(pt[2]);
                    cs.push(lev);
                }
                k += cnt + 1;
            }
            let mut s = Series::new(SeriesKind::Contour, xs, ys);
            s.z = zs;
            s.c = cs;
            s.edge_mapped = true;
            s.class_hint = Some("matlab.graphics.primitive.Patch");
            series.push(s);
        }
    }
    crate::builtins_graph3d::add_3d(it, series, &[], nargout)
}

fn coneplot(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let (f, rest) = field_args(a, false, Some(a.len() >= 9), "coneplot")?;
    let nums: Vec<Matrix> = rest.iter().filter(|v| num(v)).map(mat).collect::<R<_>>()?;
    let (cx, cy, cz) = match nums.as_slice() {
        [x, y, z, ..] => (x.data.clone(), y.data.clone(), z.data.clone()),
        _ => return rt("Specify the cone positions Cx, Cy and Cz."),
    };
    let scale = nums.get(3).filter(|m| m.numel() == 1).map(|m| m.data[0]).unwrap_or(1.0);
    let n = cx.len();
    let vels: Vec<[f64; 3]> = (0..n).map(|k| {
        let q = [cx[k], cy[k], cz[k]];
        [f.comps[0].interp(q), f.comps[1].interp(q), f.comps[2].interp(q)]
    }).collect();
    let speed = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let vmax = vels.iter().map(speed).filter(|s| s.is_finite()).fold(0.0f64, f64::max).max(f64::MIN_POSITIVE);
    let cell = [&f.xs, &f.ys, &f.zs].iter().filter(|g| g.len() > 1).map(|g| (g[1] - g[0]).abs()).fold(f64::INFINITY, f64::min);
    let cell = if cell.is_finite() { cell } else { 1.0 };
    let (mut faces, mut verts, mut colors) = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..n {
        let v = vels[k];
        let s = speed(&v);
        if !(s > 0.0) {
            continue;
        }
        let len = scale * cell * s / vmax;
        let d = v.map(|x| x / s);
        let (nv, bv) = frame(d);
        let base = [cx[k], cy[k], cz[k]];
        let tip = [0, 1, 2].map(|i| base[i] + len * d[i]);
        let r = len * 0.35;
        let start = verts.len();
        verts.push(tip);
        for t in 0..8 {
            let ang = t as f64 / 8.0 * std::f64::consts::TAU;
            verts.push([0, 1, 2].map(|i| base[i] + r * (ang.cos() * nv[i] + ang.sin() * bv[i])));
        }
        for t in 0..8 {
            faces.push(vec![start, start + 1 + t, start + 1 + (t + 1) % 8]);
            colors.push(s);
        }
        faces.push((0..8).map(|t| start + 1 + t).collect());
        colors.push(s);
    }
    let r = draw_patch(it, &faces, &verts, Some(colors), None, 1)?;
    let ids = r.as_ref().and_then(|v| v.first()).and_then(gfx::ids_of).unwrap_or_default();
    handles_out(it, &ids, nargout)
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "isosurface" => isosurface(it, a, nargout),
        "isocaps" => isocaps(it, a, nargout),
        "isonormals" => isonormals(it, a),
        "isocolors" => isocolors(a),
        "smooth3" => smooth3(a),
        "subvolume" => subvolume(a, nargout),
        "reducevolume" => reducevolume(a, nargout),
        "volumebounds" => volumebounds(a),
        "surf2patch" => surf2patch(it, a, nargout),
        "shrinkfaces" => shrinkfaces(it, a, nargout),
        "reducepatch" => reducepatch(it, a, nargout),
        "stream2" => Ok(Some(vec![cell_of(&stream(it, a, false)?)])),
        "stream3" => Ok(Some(vec![cell_of(&stream(it, a, true)?)])),
        "streamline" => streamline(it, a, nargout),
        "streamtube" => streamtube(it, a, nargout),
        "streamribbon" => streamribbon(it, a, nargout),
        "streamslice" => streamslice(it, a, nargout),
        "streamparticles" => streamparticles(it, a, nargout),
        "interpstreamspeed" => interpstreamspeed(a),
        "slice" => slice(it, a, nargout),
        "contourslice" => contourslice(it, a, nargout),
        "coneplot" => coneplot(it, a, nargout),
        _ => Ok(None),
    }
}
