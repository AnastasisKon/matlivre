//! Graphics utilities: `imtile`, `imformats`, `orient`, `objbounds`, `fontname`,
//! `rendererinfo`, `opengl` and `printopt`.

use std::cell::RefCell;

use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["imtile", "imformats", "orient", "objbounds", "fontname", "rendererinfo", "opengl", "printopt"];

/// The file format registry: extensions, the isa/info/read/write functions, alpha, description.
const FORMATS: &[(&[&str], &str, &str, &str, &str, bool, &str)] = &[
    (&["bmp"], "isbmp", "imbmpinfo", "readbmp", "writebmp", false, "Windows Bitmap"),
    (&["cur"], "iscur", "imcurinfo", "readcur", "", true, "Windows Cursor resources"),
    (&["fts", "fits"], "isfits", "imfitsinfo", "readfits", "", false, "Flexible Image Transport System"),
    (&["gif"], "isgif", "imgifinfo", "readgif", "writegif", false, "Graphics Interchange Format"),
    (&["hdf"], "ishdf", "imhdfinfo", "readhdf", "writehdf", false, "Hierarchical Data Format"),
    (&["ico"], "isico", "imicoinfo", "readico", "", true, "Windows Icon resources"),
    (&["j2c", "j2k"], "isjp2", "imjp2info", "readjp2", "writej2c", false, "JPEG 2000 (raw codestream)"),
    (&["jp2"], "isjp2", "imjp2info", "readjp2", "writejp2", false, "JPEG 2000 (Part 1)"),
    (&["jpf", "jpx"], "isjp2", "imjp2info", "readjp2", "", false, "JPEG 2000 (Part 2)"),
    (&["jpg", "jpeg"], "isjpg", "imjpginfo", "readjpg", "writejpg", false, "Joint Photographic Experts Group"),
    (&["pbm"], "ispbm", "impnminfo", "readpnm", "writepnm", false, "Portable Bitmap"),
    (&["pcx"], "ispcx", "impcxinfo", "readpcx", "writepcx", false, "Windows Paintbrush"),
    (&["pgm"], "ispgm", "impnminfo", "readpnm", "writepnm", false, "Portable Graymap"),
    (&["png"], "ispng", "impnginfo", "readpng", "writepng", true, "Portable Network Graphics"),
    (&["pnm"], "ispnm", "impnminfo", "readpnm", "writepnm", false, "Portable Any Map"),
    (&["ppm"], "isppm", "impnminfo", "readpnm", "writepnm", false, "Portable Pixmap"),
    (&["ras"], "isras", "imrasinfo", "readras", "writeras", true, "Sun Raster"),
    (&["svs"], "istif", "imtifinfo", "readtif", "", false, "Aperio ScanScope Virtual Slide"),
    (&["tif", "tiff"], "istif", "imtifinfo", "readtif", "writetif", false, "Tagged Image File Format"),
    (&["xwd"], "isxwd", "imxwdinfo", "readxwd", "writexwd", false, "X Window Dump"),
];

const FORMAT_FIELDS: &[&str] = &["ext", "isa", "info", "read", "write", "alpha", "description"];

thread_local! {
    /// The registry as `imformats` returns it (changed by 'add', 'remove' and 'update').
    static REGISTRY: RefCell<Option<Vec<Vec<Value>>>> = const { RefCell::new(None) };
}

fn factory() -> Vec<Vec<Value>> {
    let handle = |n: &str| if n.is_empty() { Value::empty() } else { Value::Func(FuncValue::Named(n.into())) };
    FORMATS
        .iter()
        .map(|(ext, isa, info, read, write, alpha, desc)| {
            vec![
                Value::Cell(CellArray::row(ext.iter().map(|e| Value::chars(e)).collect())),
                handle(isa),
                handle(info),
                handle(read),
                handle(write),
                Value::num(if *alpha { 1.0 } else { 0.0 }),
                Value::chars(desc),
            ]
        })
        .collect()
}

fn registry() -> Vec<Vec<Value>> {
    REGISTRY.with(|r| r.borrow_mut().get_or_insert_with(factory).clone())
}

fn struct_of(rows: Vec<Vec<Value>>) -> Value {
    let n = rows.len();
    Value::Struct(StructArray { dims: vec![1, n], fields: FORMAT_FIELDS.iter().map(|s| s.to_string()).collect(), elems: rows })
}

fn exts(row: &[Value]) -> Vec<String> {
    match &row[0] {
        Value::Cell(c) => c.data.iter().filter_map(|v| v.text().ok()).collect(),
        _ => Vec::new(),
    }
}

fn imformats(a: &[Value]) -> R<Value> {
    let Some(first) = a.first() else { return Ok(struct_of(registry())) };
    match first {
        Value::Struct(s) => {
            // imformats(fmts) replaces the registry.
            let rows: Vec<Vec<Value>> = s.elems.iter().map(|e| FORMAT_FIELDS.iter().map(|f| s.fields.iter().position(|x| x == f).map(|k| e[k].clone()).unwrap_or_else(Value::empty)).collect()).collect();
            let old = registry();
            REGISTRY.with(|r| *r.borrow_mut() = Some(rows));
            Ok(struct_of(old))
        }
        v => {
            let cmd = v.text()?.to_lowercase();
            match cmd.as_str() {
                "factory" => {
                    REGISTRY.with(|r| *r.borrow_mut() = Some(factory()));
                    Ok(struct_of(registry()))
                }
                "add" | "update" => {
                    let Some(Value::Struct(s)) = a.get(1) else { return rt("A format struct is required.") };
                    let mut reg = registry();
                    for e in &s.elems {
                        let row: Vec<Value> = FORMAT_FIELDS.iter().map(|f| s.fields.iter().position(|x| x == f).map(|k| e[k].clone()).unwrap_or_else(Value::empty)).collect();
                        let ext = exts(&row);
                        match reg.iter().position(|r| exts(r).iter().any(|x| ext.contains(x))) {
                            Some(k) if cmd == "update" => reg[k] = row,
                            Some(_) => return rt(format!("Format '{}' already exists.", ext.join(", "))),
                            None if cmd == "add" => reg.push(row),
                            None => return rt(format!("Format '{}' does not exist.", ext.join(", "))),
                        }
                    }
                    REGISTRY.with(|r| *r.borrow_mut() = Some(reg.clone()));
                    Ok(struct_of(reg))
                }
                "remove" => {
                    let e = a.get(1).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?.to_lowercase();
                    let mut reg = registry();
                    reg.retain(|r| !exts(r).contains(&e));
                    REGISTRY.with(|r| *r.borrow_mut() = Some(reg.clone()));
                    Ok(struct_of(reg))
                }
                ext => {
                    let found: Vec<Vec<Value>> = registry().into_iter().filter(|r| exts(r).iter().any(|x| x == ext)).collect();
                    if found.is_empty() {
                        return Ok(Value::Struct(StructArray { dims: vec![0, 0], fields: Vec::new(), elems: Vec::new() }));
                    }
                    Ok(Value::Struct(StructArray { dims: vec![1, 1], fields: FORMAT_FIELDS.iter().map(|s| s.to_string()).collect(), elems: found }))
                }
            }
        }
    }
}

// ---- imtile -----------------------------------------------------------------------------------

/// The background color as one value per channel in the image's range.
fn background(v: Option<&Value>, kind: Kind) -> R<(Vec<f64>, bool)> {
    let full = match kind {
        Kind::Int(IntClass::U8) => 255.0,
        Kind::Int(IntClass::U16) => 65535.0,
        Kind::Int(IntClass::I16) => 32767.0,
        _ => 1.0,
    };
    let Some(v) = v else { return Ok((vec![0.0; 3], false)) };
    let rgb: Vec<f64> = if v.is_text() {
        match v.text()?.to_lowercase().as_str() {
            "w" | "white" => vec![1.0, 1.0, 1.0],
            "k" | "black" => vec![0.0, 0.0, 0.0],
            "r" | "red" => vec![1.0, 0.0, 0.0],
            "g" | "green" => vec![0.0, 1.0, 0.0],
            "b" | "blue" => vec![0.0, 0.0, 1.0],
            "y" | "yellow" => vec![1.0, 1.0, 0.0],
            "m" | "magenta" => vec![1.0, 0.0, 1.0],
            "c" | "cyan" => vec![0.0, 1.0, 1.0],
            other => return rt(format!("Invalid color name '{other}'.")),
        }
    } else {
        v.to_matrix()?.data
    };
    let is_black = rgb.iter().all(|&x| x == 0.0);
    Ok((rgb.iter().map(|x| (x * full).round().min(full)).collect(), !is_black))
}

fn imtile(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let src = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
    let mut images: Vec<Matrix> = match src {
        Value::Cell(c) => c.data.iter().map(|v| v.to_matrix()).collect::<R<_>>()?,
        Value::Str(s) => {
            let mut out = Vec::new();
            for f in s.data.iter().flatten() {
                out.push(it.call_function("imread", vec![Value::chars(f)], 1)?.remove(0).to_matrix()?);
            }
            out
        }
        v => {
            // H x W x C x N (or H x W x N for single-channel frames)
            let m = v.to_matrix()?;
            let d = m.dims.clone();
            let (h, w) = (d[0], d.get(1).copied().unwrap_or(1));
            let (c, n) = match d.len() {
                4 => (d[2], d[3]),
                3 if d[2] == 3 => (3, 1),
                3 => (1, d[2]),
                _ => (1, 1),
            };
            let per = h * w * c;
            (0..n)
                .map(|k| {
                    let mut x = Matrix::from_dims(if c == 1 { vec![h, w] } else { vec![h, w, c] }, m.data[k * per..(k + 1) * per].to_vec());
                    x.kind = m.kind;
                    x
                })
                .collect()
        }
    };
    if images.is_empty() {
        return rt("No images to tile.");
    }
    let mut grid: Option<(f64, f64)> = None;
    let mut border = (0usize, 0usize);
    let mut bg: Option<Value> = None;
    let mut frames: Option<Vec<usize>> = None;
    let mut i = 1;
    while i + 1 < a.len() {
        let key = a[i].text()?.to_lowercase();
        let v = &a[i + 1];
        match key.as_str() {
            "gridsize" => {
                let m = v.to_matrix()?;
                grid = Some((m.data[0], m.data.get(1).copied().unwrap_or(f64::NAN)));
            }
            "bordersize" => {
                let m = v.to_matrix()?;
                let b0 = m.data[0] as usize;
                border = (b0, m.data.get(1).map(|&x| x as usize).unwrap_or(b0));
            }
            "backgroundcolor" => bg = Some(v.clone()),
            "frames" => frames = Some(v.to_matrix()?.data.iter().map(|&x| (x as usize).saturating_sub(1)).collect()),
            "thumbnailsize" => {}
            other => return rt(format!("'{other}' is not a recognized parameter.")),
        }
        i += 2;
    }
    if let Some(f) = frames {
        images = f.iter().filter_map(|&k| images.get(k).cloned()).collect();
    }
    let kind = images[0].kind;
    let (h, w) = (images[0].dims[0], images[0].dims.get(1).copied().unwrap_or(1));
    if images.iter().any(|m| m.dims[0] != h || m.dims.get(1).copied().unwrap_or(1) != w) {
        return rt("All images must have the same size (ThumbnailSize resizing is not supported in this version).");
    }
    let n = images.len();
    let (rows, cols) = match grid {
        Some((r, c)) if r.is_nan() => ((n as f64 / c).ceil() as usize, c as usize),
        Some((r, c)) if c.is_nan() => (r as usize, (n as f64 / r).ceil() as usize),
        Some((r, c)) => (r as usize, c as usize),
        None => {
            // A roughly square layout for the image shape.
            let c = ((n as f64 * h as f64 / w as f64).sqrt()).ceil().max(1.0) as usize;
            ((n + c - 1) / c, c)
        }
    };
    let (bgv, colored) = background(bg.as_ref(), kind)?;
    let channels = if images.iter().any(|m| m.dims.len() == 3 && m.dims[2] == 3) || colored { 3 } else { 1 };
    let (th, tw) = (h + 2 * border.0, w + 2 * border.1);
    let (oh, ow) = (rows * th, cols * tw);
    let mut out = vec![0.0; oh * ow * channels];
    for ch in 0..channels {
        let v = if channels == 1 { bgv[0] } else { bgv[ch.min(bgv.len() - 1)] };
        for k in 0..oh * ow {
            out[ch * oh * ow + k] = v;
        }
    }
    for (k, img) in images.iter().enumerate().take(rows * cols) {
        let (r0, c0) = ((k / cols) * th + border.0, (k % cols) * tw + border.1);
        let img_channels = if img.dims.len() == 3 { img.dims[2] } else { 1 };
        for ch in 0..channels {
            let sc = if img_channels == 1 { 0 } else { ch.min(img_channels - 1) };
            for x in 0..w {
                for y in 0..h {
                    out[ch * oh * ow + (c0 + x) * oh + r0 + y] = img.data[sc * h * w + x * h + y];
                }
            }
        }
    }
    let mut m = Matrix::from_dims(if channels == 1 { vec![oh, ow] } else { vec![oh, ow, 3] }, out);
    m.kind = kind;
    Ok(Value::Num(m))
}

// ---- figure helpers ---------------------------------------------------------------------------

fn get(it: &mut Interpreter, h: &Value, prop: &str) -> R<Value> {
    Ok(it.call_function("get", vec![h.clone(), Value::chars(prop)], 1)?.into_iter().next().unwrap_or_else(Value::empty))
}

fn set(it: &mut Interpreter, h: &Value, prop: &str, v: Value) -> R<()> {
    it.call_function("set", vec![h.clone(), Value::chars(prop), v], 0)?;
    Ok(())
}

fn current_figure(it: &mut Interpreter) -> R<Value> {
    Ok(it.call_function("gcf", Vec::new(), 1)?.remove(0))
}

const TALL: [f64; 4] = [0.25, 0.25, 8.0, 10.5];

fn orient(it: &mut Interpreter, a: &[Value]) -> R<Option<Value>> {
    let (fig, mode) = match a {
        [] => (current_figure(it)?, None),
        [v] if v.is_text() => (current_figure(it)?, Some(v.text()?)),
        [f] => (f.clone(), None),
        [f, m, ..] => (f.clone(), Some(m.text()?)),
    };
    match mode {
        None => {
            let o = get(it, &fig, "PaperOrientation")?.text()?;
            if o == "landscape" {
                return Ok(Some(Value::chars("landscape")));
            }
            let pos = get(it, &fig, "PaperPosition")?.to_matrix()?.data;
            Ok(Some(Value::chars(if pos == TALL { "tall" } else { "portrait" })))
        }
        Some(m) => {
            match m.to_lowercase().as_str() {
                "landscape" => set(it, &fig, "PaperOrientation", Value::chars("landscape"))?,
                "portrait" => set(it, &fig, "PaperOrientation", Value::chars("portrait"))?,
                "tall" => {
                    set(it, &fig, "PaperOrientation", Value::chars("portrait"))?;
                    set(it, &fig, "PaperPosition", Value::Num(Matrix::row(TALL.to_vec())))?;
                }
                other => return rt(format!("Unknown orientation '{other}'. Use 'portrait', 'landscape' or 'tall'.")),
            }
            Ok(None)
        }
    }
}

fn objbounds(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let h = match a.first() {
        Some(h) => h.clone(),
        None => it.call_function("gca", Vec::new(), 1)?.remove(0),
    };
    // The data objects: the children, recursively (groups hold their own children).
    let mut stack = vec![h];
    let mut kids_list: Vec<Value> = Vec::new();
    while let Some(x) = stack.pop() {
        let Ok(c) = get(it, &x, "Children") else { continue };
        for k in 0..c.numel() {
            let one = crate::indexing::index(&c, &[crate::nd::IndexArg::scalar(k)])?;
            kids_list.push(one.clone());
            stack.push(one);
        }
    }
    let n = kids_list.len();
    let mut lim = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
    for k in 0..n {
        let one = kids_list[k].clone();
        for (axis, prop) in ["XData", "YData", "ZData"].iter().enumerate() {
            let Ok(v) = get(it, &one, prop) else { continue };
            let Ok(m) = v.to_matrix() else { continue };
            for &x in m.data.iter().filter(|x| x.is_finite()) {
                lim[2 * axis] = lim[2 * axis].min(x);
                lim[2 * axis + 1] = lim[2 * axis + 1].max(x);
            }
        }
    }
    let out: Vec<f64> = lim.iter().map(|&x| if x.is_finite() { x } else { 0.0 }).collect();
    Ok(Value::Num(Matrix::row(out)))
}

fn fontname(it: &mut Interpreter, a: &[Value]) -> R<()> {
    let (target, name) = match a {
        [n] => (current_figure(it)?, n.clone()),
        [t, n, ..] => (t.clone(), n.clone()),
        [] => return rt("Not enough input arguments."),
    };
    let name = if name.text()?.eq_ignore_ascii_case("default") { Value::chars("Helvetica") } else { name };
    let all = it.call_function("findall", vec![target], 1)?.remove(0);
    for k in 0..all.numel() {
        let h = crate::indexing::index(&all, &[crate::nd::IndexArg::scalar(k)])?;
        // Objects without a FontName property are skipped.
        let _ = set(it, &h, "FontName", name.clone());
    }
    Ok(())
}

fn renderer_struct() -> Value {
    let mut d = StructArray::scalar();
    d.set("HardwareSupportLevel", Value::chars("full"));
    d.set("SupportsDepthPeelTransparency", Value::bool(true));
    d.set("SupportsAlignVertexCenters", Value::bool(true));
    d.set("SupportsGraphicsSmoothing", Value::bool(true));
    d.set("MaxTextureSize", Value::num(8192.0));
    let mut s = StructArray::scalar();
    s.set("GraphicsRenderer", Value::chars("MatLivre renderer"));
    s.set("Vendor", Value::chars("MatLivre"));
    s.set("Version", Value::chars(env!("CARGO_PKG_VERSION")));
    s.set("RendererDevice", Value::chars("egui"));
    s.set("Details", Value::Struct(d));
    Value::Struct(s)
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let one = |v: Value| Ok(Some(vec![v]));
    match name {
        "imtile" => one(imtile(it, a)?),
        "imformats" => {
            let v = imformats(a)?;
            if nargout == 0 && a.is_empty() {
                // imformats without outputs lists the formats.
                let mut lines = vec![format!("{:<10} {:<10} {:<10} {:<10} {:<10} {:<6} {}", "EXT", "ISA", "INFO", "READ", "WRITE", "ALPHA", "DESCRIPTION")];
                lines.push("-".repeat(lines[0].len() + 20));
                for r in registry() {
                    let f = |v: &Value| match v {
                        Value::Func(FuncValue::Named(n)) => n.clone(),
                        _ => String::new(),
                    };
                    lines.push(format!("{:<10} {:<10} {:<10} {:<10} {:<10} {:<6} {}", exts(&r).join(" "), f(&r[1]), f(&r[2]), f(&r[3]), f(&r[4]), r[5].scalar().unwrap_or(0.0), r[6].text().unwrap_or_default()));
                }
                it.emit(OutputKind::Text(lines.join("\n") + "\n"));
                return Ok(Some(Vec::new()));
            }
            one(v)
        }
        "orient" => Ok(Some(orient(it, a)?.into_iter().collect())),
        "objbounds" => one(objbounds(it, a)?),
        "fontname" => {
            fontname(it, a)?;
            Ok(Some(Vec::new()))
        }
        "rendererinfo" => one(renderer_struct()),
        "opengl" => {
            let cmd = a.first().map(|v| v.text()).transpose()?.unwrap_or_else(|| "info".into());
            match cmd.as_str() {
                "info" => {
                    if nargout > 0 {
                        return one(renderer_struct());
                    }
                    it.emit(OutputKind::Text(format!("          Version: '{}'\n           Vendor: 'MatLivre'\n         Renderer: 'MatLivre renderer'\n   RendererDriverVersion: '{}'\n", env!("CARGO_PKG_VERSION"), env!("CARGO_PKG_VERSION"))));
                    Ok(Some(Vec::new()))
                }
                "data" => one(renderer_struct()),
                _ => Ok(Some(Vec::new())),
            }
        }
        "printopt" => {
            it.emit(OutputKind::Warning("printopt will be removed in a future release. To configure printer defaults, use the options provided by your operating system.".into()));
            let (cmd, dev) = if cfg!(windows) { ("COPY /B %s PORTPROMPT:", "-dwin") } else { ("lpr -r", "-dps2") };
            Ok(Some(vec![Value::chars(cmd), Value::chars(dev)]))
        }
        _ => Ok(None),
    }
}
