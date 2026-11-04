//! The search path (`path`, `matlabpath`, `addpath`, `rmpath`, `restoredefaultpath`,
//! `rehash`) and folder and file queries (`what`, `fileattrib`, `recycle`, `toolboxdir`).

use crate::builtins::{none, one, Ret};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;
use std::cell::Cell;
use std::path::{Path, PathBuf};

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["path", "matlabpath", "addpath", "rmpath", "restoredefaultpath", "rehash", "what", "fileattrib", "recycle", "toolboxdir"];

const SEP: &str = if cfg!(windows) { ";" } else { ":" };

thread_local! {
    static RECYCLE: Cell<bool> = const { Cell::new(false) };
}

fn path_text(it: &Interpreter) -> String {
    it.search_path().iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(SEP)
}

fn split_dirs(text: &str) -> Vec<PathBuf> {
    text.split(SEP).map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from).collect()
}

fn texts(a: &[Value]) -> R<Vec<String>> {
    a.iter().map(|v| v.text()).collect()
}

/// `path`: the search path; with arguments it sets the path first and returns the new one.
fn path_fn(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    match a.len() {
        0 => {
            if nargout == 0 {
                let mut out = String::from("\n\t\tMATLABPATH\n\n");
                for p in it.search_path() {
                    out.push_str(&format!("\t{}\n", p.display()));
                }
                it.emit(OutputKind::Text(out));
                return none();
            }
        }
        _ => {
            // path(p) sets the path; path(p1, p2) sets it to p1 followed by p2.
            let joined = texts(a)?.join(SEP);
            it.replace_search_path(split_dirs(&joined));
        }
    }
    if nargout == 0 && !a.is_empty() {
        return none();
    }
    one(Value::chars(&path_text(it)))
}

fn addpath(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let old = path_text(it);
    let mut items = texts(a)?;
    let mut at_end = false;
    items.retain(|t| match t.to_ascii_lowercase().as_str() {
        "-end" | "1" => {
            at_end = true;
            false
        }
        "-begin" | "0" | "-frozen" => false,
        _ => true,
    });
    let mut dirs: Vec<PathBuf> = items.iter().flat_map(|t| split_dirs(t)).collect();
    for d in &dirs {
        if !d.is_dir() {
            it.emit(OutputKind::Warning(format!("Name is nonexistent or not a directory: {}", d.display())));
        }
    }
    dirs.retain(|d| d.is_dir());
    let mut path: Vec<PathBuf> = it.search_path().iter().filter(|p| !dirs.contains(p)).cloned().collect();
    if at_end {
        path.extend(dirs);
    } else {
        dirs.extend(path);
        path = dirs;
    }
    it.replace_search_path(path);
    if nargout == 0 { none() } else { one(Value::chars(&old)) }
}

fn rmpath(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let old = path_text(it);
    let mut path = it.search_path().to_vec();
    for t in texts(a)? {
        for d in split_dirs(&t) {
            match path.iter().position(|p| *p == d) {
                Some(i) => {
                    path.remove(i);
                }
                None => it.emit(OutputKind::Warning(format!("\"{}\" not found in path.", d.display()))),
            }
        }
    }
    it.replace_search_path(path);
    if nargout == 0 { none() } else { one(Value::chars(&old)) }
}

// ---- what --------------------------------------------------------------------------------------

/// The folder `what` lists: a path, or a folder name found on the search path.
fn resolve_folder(it: &Interpreter, name: Option<String>) -> R<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let Some(name) = name else { return Ok(cwd) };
    let direct = cwd.join(&name);
    if direct.is_dir() {
        return Ok(std::path::absolute(&direct).unwrap_or(direct));
    }
    let tail = Path::new(&name);
    it.search_path().iter().find(|p| p.ends_with(tail) && p.is_dir()).cloned().ok_or_else(|| LangError::new(format!("Unable to find folder \"{name}\"."), 0))
}

fn what(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let folder = resolve_folder(it, a.first().map(|v| v.text()).transpose()?)?;
    let mut groups: Vec<(&str, Vec<String>)> = ["m", "mlapp", "mlx", "mat", "mex", "mdl", "slx", "sfx", "p", "classes", "packages"].iter().map(|k| (*k, Vec::new())).collect();
    let mex = if cfg!(windows) { "mexw64" } else if cfg!(target_os = "macos") { "mexmaca64" } else { "mexa64" };
    let mut entries: Vec<_> = std::fs::read_dir(&folder).map_err(|e| LangError::new(format!("Unable to read folder: {e}"), 0))?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().to_string();
        let is_dir = e.path().is_dir();
        let key = if is_dir {
            match name.chars().next() {
                Some('@') => Some(("classes", name[1..].to_string())),
                Some('+') => Some(("packages", name[1..].to_string())),
                _ => None,
            }
        } else {
            let ext = Path::new(&name).extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            let k = match ext.as_str() {
                "m" => Some("m"),
                "mlapp" => Some("mlapp"),
                "mlx" => Some("mlx"),
                "mat" => Some("mat"),
                "mdl" => Some("mdl"),
                "slx" => Some("slx"),
                "sfx" => Some("sfx"),
                "p" => Some("p"),
                x if x == mex => Some("mex"),
                _ => None,
            };
            k.map(|k| (k, name.clone()))
        };
        if let Some((k, n)) = key {
            if let Some(g) = groups.iter_mut().find(|g| g.0 == k) {
                g.1.push(n);
            }
        }
    }
    let folder_text = folder.display().to_string();
    if nargout == 0 {
        let titles = [
            ("m", "MATLAB Code files"),
            ("mlapp", "MATLAB App files"),
            ("mlx", "Live Code files"),
            ("mat", "MAT-files"),
            ("mex", "MEX-files"),
            ("mdl", "Simulink models"),
            ("slx", "Simulink models"),
            ("p", "P-files"),
            ("classes", "Classes"),
            ("packages", "Packages"),
        ];
        let mut out = String::new();
        for (k, title) in titles {
            let names: Vec<String> = groups.iter().find(|g| g.0 == k).map(|g| g.1.clone()).unwrap_or_default();
            if names.is_empty() {
                continue;
            }
            // Files are listed without their extensions.
            let shown: Vec<String> = names.iter().map(|n| if k == "classes" || k == "packages" { n.clone() } else { Path::new(n).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() }).collect();
            let w = shown.iter().map(|s| s.chars().count()).max().unwrap_or(0) + 2;
            let per_line = (80 / w).max(1);
            out.push_str(&format!("\n{title} in folder {folder_text}\n\n"));
            for chunk in shown.chunks(per_line) {
                out.push_str(&chunk.iter().map(|s| format!("{s:<w$}")).collect::<String>());
                out.push('\n');
            }
        }
        if !out.is_empty() {
            out.push('\n');
            it.emit(OutputKind::Text(out));
        }
        return none();
    }
    let mut s = StructArray::scalar();
    s.set("path", Value::chars(&folder_text));
    for (k, names) in groups {
        let n = names.len();
        s.set(k, Value::Cell(CellArray::from_dims(vec![n, 1], names.iter().map(|x| Value::chars(x)).collect())));
    }
    one(Value::Struct(s))
}

// ---- fileattrib ------------------------------------------------------------------------------

fn bool_or_nan(b: Option<bool>) -> Value {
    match b {
        Some(b) => Value::bool(b),
        None => Value::num(f64::NAN),
    }
}

fn attributes(p: &Path) -> std::io::Result<StructArray> {
    let md = std::fs::metadata(p)?;
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut s = StructArray::scalar();
    s.set("Name", Value::chars(&abs.display().to_string()));
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let a = md.file_attributes();
        let ro = a & 0x1 != 0;
        s.set("archive", Value::bool(a & 0x20 != 0));
        s.set("system", Value::bool(a & 0x4 != 0));
        s.set("hidden", Value::bool(a & 0x2 != 0));
        s.set("directory", Value::bool(md.is_dir()));
        s.set("UserRead", Value::bool(true));
        s.set("UserWrite", Value::bool(!ro));
        s.set("UserExecute", Value::bool(true));
        for k in ["GroupRead", "GroupWrite", "GroupExecute", "OtherRead", "OtherWrite", "OtherExecute"] {
            s.set(k, bool_or_nan(None));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = md.permissions().mode();
        for k in ["archive", "system", "hidden"] {
            s.set(k, bool_or_nan(None));
        }
        s.set("directory", Value::bool(md.is_dir()));
        let bits = [("UserRead", 0o400), ("UserWrite", 0o200), ("UserExecute", 0o100), ("GroupRead", 0o40), ("GroupWrite", 0o20), ("GroupExecute", 0o10), ("OtherRead", 0o4), ("OtherWrite", 0o2), ("OtherExecute", 0o1)];
        for (k, b) in bits {
            s.set(k, Value::bool(mode & b != 0));
        }
    }
    Ok(s)
}

fn set_attributes(p: &Path, mode: &str) -> Result<(), String> {
    let (on, letters) = match mode.chars().next() {
        Some('+') => (true, &mode[1..]),
        Some('-') => (false, &mode[1..]),
        _ => return Err("Illegal file mode characters on the current platform.".into()),
    };
    for c in letters.chars() {
        let md = std::fs::metadata(p).map_err(|_| "No such file or directory.".to_string())?;
        let mut perm = md.permissions();
        match c {
            'w' => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let m = perm.mode();
                    perm.set_mode(if on { m | 0o200 } else { m & !0o222 });
                }
                #[cfg(not(unix))]
                perm.set_readonly(!on);
            }
            #[cfg(unix)]
            'r' | 'x' => {
                use std::os::unix::fs::PermissionsExt;
                let bit = if c == 'r' { 0o400 } else { 0o100 };
                let m = perm.mode();
                perm.set_mode(if on { m | bit } else { m & !bit });
            }
            _ => return Err("Illegal file mode characters on the current platform.".into()),
        }
        std::fs::set_permissions(p, perm).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn fileattrib(a: &[Value], nargout: usize) -> Ret {
    let target = match a.first() {
        Some(v) => v.text()?,
        None => ".".into(),
    };
    let p = PathBuf::from(&target);
    if let Some(mode) = a.get(1) {
        let r = set_attributes(&p, &mode.text()?);
        return match (r, nargout) {
            (Ok(()), 0) => none(),
            (Err(e), 0) => rt(e),
            (Ok(()), _) => Ok(Some(vec![Value::bool(true), Value::chars(""), Value::chars("")])),
            (Err(e), _) => Ok(Some(vec![Value::bool(false), Value::chars(&e), Value::chars("")])),
        };
    }
    match (attributes(&p), nargout) {
        (Ok(s), 0) => one(Value::Struct(s)),
        (Ok(s), _) => Ok(Some(vec![Value::bool(true), Value::Struct(s), Value::chars("")])),
        (Err(_), 0) => rt("No such file or directory."),
        (Err(_), _) => Ok(Some(vec![Value::bool(false), Value::chars("No such file or directory."), Value::chars("")])),
    }
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "path" | "matlabpath" => path_fn(it, a, nargout),
        "addpath" => addpath(it, a, nargout),
        "rmpath" => rmpath(it, a, nargout),
        "restoredefaultpath" => {
            let d = it.default_search_path();
            it.replace_search_path(d);
            none()
        }
        "rehash" => {
            it.reset_path_cache();
            none()
        }
        "what" => what(it, a, nargout),
        "fileattrib" => fileattrib(a, nargout),
        "recycle" => {
            let old = if RECYCLE.with(Cell::get) { "on" } else { "off" };
            if let Some(v) = a.first() {
                match v.text()?.to_lowercase().as_str() {
                    "on" => {
                        RECYCLE.with(|r| r.set(true));
                        it.emit(OutputKind::Warning("MatLivre has no recycle bin: delete still removes files permanently.".into()));
                    }
                    "off" => RECYCLE.with(|r| r.set(false)),
                    _ => return rt("Argument must be 'on' or 'off'."),
                }
                if nargout == 0 {
                    return none();
                }
            }
            one(Value::chars(old))
        }
        "toolboxdir" => {
            let t = crate::builtins::arg(a, 0, name)?.text()?;
            if !["matlab", "symbolic"].contains(&t.to_lowercase().as_str()) {
                return rt(format!("Could not locate the base directory for {t}."));
            }
            let root = it.call_function("matlabroot", Vec::new(), 1)?.remove(0).text()?;
            one(Value::chars(&Path::new(&root).join("toolbox").join(t.to_lowercase()).display().to_string()))
        }
        _ => Ok(None),
    }
}
