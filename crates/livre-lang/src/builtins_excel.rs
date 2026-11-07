//! Spreadsheets (`.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.ods`): `readtable`, `readmatrix`,
//! `readcell`, `readvars`, `writetable`, `writematrix` and `writecell` on spreadsheet files,
//! and `xlsread`, `xlswrite`, `xlsfinfo` and `sheetnames`.
//!
//! The text-file readers and writers already implement MATLAB's type detection, headers and
//! options, so a sheet goes through them as CSV: reading converts the sheet (or `'Range'`) to a
//! temporary CSV file, writing writes one and copies its cells into the workbook (keeping the
//! other sheets).

use std::path::{Path, PathBuf};

use calamine::{open_workbook_auto, Data, Reader};
use rust_xlsxwriter::Workbook;

use crate::builtins::{arg, one, Ret};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["xlsread", "xlswrite", "xlsfinfo", "sheetnames"];

/// Readers and writers that take spreadsheet files as well as text.
pub const FILE_FUNCS: &[&str] = &["readtable", "readmatrix", "readcell", "readvars", "writetable", "writematrix", "writecell"];

#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Empty,
    Num(f64),
    Text(String),
}

pub fn is_sheet_path(p: &str) -> bool {
    let ext = Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "xlsx" | "xlsm" | "xlsb" | "xls" | "ods")
}

fn err(e: impl std::fmt::Display) -> LangError {
    LangError::new(e.to_string(), 0)
}

/// `'A1'` as a 0-based (row, column).
fn cell_ref(s: &str) -> R<(usize, usize)> {
    let s = s.trim().to_uppercase();
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &s[letters.len()..];
    if letters.is_empty() || digits.is_empty() {
        return rt(format!("Invalid cell reference '{s}'."));
    }
    let col = letters.bytes().fold(0usize, |a, b| a * 26 + (b - b'A' + 1) as usize) - 1;
    let row: usize = digits.parse::<usize>().map_err(|_| LangError::new(format!("Invalid cell reference '{s}'."), 0))?;
    Ok((row.max(1) - 1, col))
}

/// `'A1:B2'` (or a start cell) as 0-based (r0, c0, r1, c1) with open ends as `usize::MAX`.
fn range_ref(s: &str) -> R<(usize, usize, usize, usize)> {
    match s.split_once(':') {
        Some((a, b)) => {
            let (r0, c0) = cell_ref(a)?;
            let (r1, c1) = cell_ref(b)?;
            Ok((r0.min(r1), c0.min(c1), r0.max(r1), c0.max(c1)))
        }
        None => {
            let (r0, c0) = cell_ref(s)?;
            Ok((r0, c0, usize::MAX, usize::MAX))
        }
    }
}

/// A sheet given by name or 1-based index.
fn sheet_name(path: &str, sheet: Option<&Value>) -> R<String> {
    let wb = open_workbook_auto(path).map_err(|e| err(format!("Unable to open file '{path}': {e}")))?;
    let names = wb.sheet_names();
    match sheet {
        None => names.first().cloned().ok_or_else(|| err("The workbook has no sheets.")),
        Some(v) if v.is_text() || matches!(v, Value::Str(_)) => {
            let n = v.text()?;
            names.iter().find(|x| x.eq_ignore_ascii_case(&n)).cloned().ok_or_else(|| err(format!("Worksheet '{n}' not found.")))
        }
        Some(v) => {
            let k = v.scalar()? as usize;
            names.get(k.wrapping_sub(1)).cloned().ok_or_else(|| err(format!("Worksheet number {k} is out of range.")))
        }
    }
}

/// The cells of a sheet from A1 (so ranges index absolutely), cut to `range`.
fn read_sheet(path: &str, sheet: &str, range: Option<&str>) -> R<Vec<Vec<Cell>>> {
    let mut wb = open_workbook_auto(path).map_err(|e| err(format!("Unable to open file '{path}': {e}")))?;
    let r = wb.worksheet_range(sheet).map_err(err)?;
    let (sr, sc) = r.start().map(|(a, b)| (a as usize, b as usize)).unwrap_or((0, 0));
    let mut grid: Vec<Vec<Cell>> = Vec::new();
    for (i, row) in r.rows().enumerate() {
        let ri = sr + i;
        while grid.len() <= ri {
            grid.push(Vec::new());
        }
        for (j, d) in row.iter().enumerate() {
            let cj = sc + j;
            let line = &mut grid[ri];
            while line.len() <= cj {
                line.push(Cell::Empty);
            }
            line[cj] = match d {
                Data::Int(x) => Cell::Num(*x as f64),
                Data::Float(x) => Cell::Num(*x),
                Data::Bool(b) => Cell::Num(if *b { 1.0 } else { 0.0 }),
                Data::String(s) if s.is_empty() => Cell::Empty,
                Data::String(s) => Cell::Text(s.clone()),
                Data::DateTime(t) => Cell::Num(t.as_f64()),
                Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.clone()),
                Data::Error(_) => Cell::Num(f64::NAN),
                Data::Empty => Cell::Empty,
            };
        }
    }
    let width = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    for r in grid.iter_mut() {
        r.resize(width, Cell::Empty);
    }
    let (r0, c0, r1, c1) = match range {
        Some(s) => range_ref(s)?,
        // Without a range, the used area (from its first row and column with data).
        None => {
            let r0 = grid.iter().position(|r| r.iter().any(|c| *c != Cell::Empty)).unwrap_or(0);
            let c0 = (0..width).find(|&j| grid.iter().any(|r| r[j] != Cell::Empty)).unwrap_or(0);
            (r0, c0, usize::MAX, usize::MAX)
        }
    };
    let rows: Vec<Vec<Cell>> = grid
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= r0 && *i <= r1)
        .map(|(_, r)| r.iter().enumerate().filter(|(j, _)| *j >= c0 && *j <= c1).map(|(_, c)| c.clone()).collect())
        .collect();
    if let (Some(_), true) = (range, r1 != usize::MAX) {
        // A closed range keeps its full size, padded with empty cells.
        let (h, w) = (r1 - r0 + 1, c1 - c0 + 1);
        let mut out = vec![vec![Cell::Empty; w]; h];
        for (i, r) in rows.into_iter().enumerate().take(h) {
            for (j, c) in r.into_iter().enumerate().take(w) {
                out[i][j] = c;
            }
        }
        return Ok(out);
    }
    Ok(rows)
}

fn csv_field(c: &Cell) -> String {
    match c {
        Cell::Empty => String::new(),
        Cell::Num(x) if x.is_nan() => "NaN".into(),
        Cell::Num(x) => format!("{x}"),
        Cell::Text(t) if t.contains([',', '"', '\n', '\r']) || t.trim() != t => format!("\"{}\"", t.replace('"', "\"\"")),
        Cell::Text(t) => t.clone(),
    }
}

fn temp_csv() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!("matlivre-sheet-{}-{}.csv", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// The arguments without a name-value pair, and its value.
fn take_opt(a: &[Value], name: &str) -> (Vec<Value>, Option<Value>) {
    let mut out = Vec::new();
    let mut found = None;
    let mut i = 0;
    while i < a.len() {
        if i > 0 && a[i].text().map(|t| t.eq_ignore_ascii_case(name)).unwrap_or(false) && i + 1 < a.len() {
            found = Some(a[i + 1].clone());
            i += 2;
            continue;
        }
        out.push(a[i].clone());
        i += 1;
    }
    (out, found)
}

/// Splits CSV text into rows of fields (quotes and doubled quotes as RFC 4180).
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if field.is_empty() => quoted = true,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (c, _) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// The workbook's sheets as cell grids (to rewrite it with one sheet replaced).
fn read_all(path: &str) -> R<Vec<(String, Vec<Vec<Cell>>)>> {
    if !Path::new(path).is_file() {
        return Ok(Vec::new());
    }
    let wb = open_workbook_auto(path).map_err(|e| err(format!("Unable to open file '{path}': {e}")))?;
    let names = wb.sheet_names();
    names.iter().map(|n| Ok((n.clone(), read_sheet(path, n, Some("A1"))?))).collect()
}

/// Writes `cells` into `sheet` of the workbook at `path`, starting at (r0, c0), keeping the
/// other sheets. Returns whether the sheet was new.
fn write_cells(path: &str, sheet: Option<&Value>, start: (usize, usize), cells: &[Vec<Cell>]) -> R<bool> {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "xlsx" && ext != "xlsm" {
        return rt(format!("Writing .{ext} files is not supported; use .xlsx."));
    }
    let mut sheets = read_all(path)?;
    let target = match sheet {
        None => sheets.first().map(|s| s.0.clone()).unwrap_or_else(|| "Sheet1".into()),
        Some(v) if v.is_text() || matches!(v, Value::Str(_)) => v.text()?,
        Some(v) => {
            let k = v.scalar()? as usize;
            sheets.get(k.wrapping_sub(1)).map(|s| s.0.clone()).unwrap_or_else(|| format!("Sheet{k}"))
        }
    };
    let added = !sheets.iter().any(|s| s.0.eq_ignore_ascii_case(&target));
    if added {
        sheets.push((target.clone(), Vec::new()));
    }
    let grid = &mut sheets.iter_mut().find(|s| s.0.eq_ignore_ascii_case(&target)).unwrap().1;
    for (i, row) in cells.iter().enumerate() {
        let r = start.0 + i;
        while grid.len() <= r {
            grid.push(Vec::new());
        }
        for (j, c) in row.iter().enumerate() {
            let k = start.1 + j;
            while grid[r].len() <= k {
                grid[r].push(Cell::Empty);
            }
            grid[r][k] = c.clone();
        }
    }
    let mut wb = Workbook::new();
    for (name, grid) in &sheets {
        let ws = wb.add_worksheet();
        ws.set_name(name).map_err(err)?;
        for (i, row) in grid.iter().enumerate() {
            for (j, c) in row.iter().enumerate() {
                match c {
                    Cell::Empty => {}
                    Cell::Num(x) => {
                        ws.write_number(i as u32, j as u16, *x).map_err(err)?;
                    }
                    Cell::Text(t) => {
                        ws.write_string(i as u32, j as u16, t).map_err(err)?;
                    }
                }
            }
        }
    }
    wb.save(path).map_err(err)?;
    Ok(added)
}

fn cell_of_text(t: &str) -> Cell {
    if t.is_empty() {
        return Cell::Empty;
    }
    match t.trim().parse::<f64>() {
        Ok(x) if !t.trim().is_empty() => Cell::Num(x),
        _ => match t.trim() {
            "NaN" => Cell::Num(f64::NAN),
            "Inf" => Cell::Num(f64::INFINITY),
            "-Inf" => Cell::Num(f64::NEG_INFINITY),
            _ => Cell::Text(t.to_string()),
        },
    }
}

/// `readtable` and friends on a spreadsheet: the sheet through a temporary CSV file.
pub fn file_call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    if name.starts_with("read") {
        let path = arg(a, 0, name)?.text()?;
        let (rest, sheet) = take_opt(a, "Sheet");
        let (mut rest, range) = take_opt(&rest, "Range");
        let sheet = sheet_name(&path, sheet.as_ref())?;
        let range = range.map(|r| r.text()).transpose()?;
        let rows = read_sheet(&path, &sheet, range.as_deref())?;
        let text: String = rows.iter().map(|r| r.iter().map(csv_field).collect::<Vec<_>>().join(",") + "\n").collect();
        let tmp = temp_csv();
        std::fs::write(&tmp, text).map_err(err)?;
        rest[0] = Value::chars(&tmp.display().to_string());
        let out = it.call_function(name, rest, nargout);
        let _ = std::fs::remove_file(&tmp);
        return out.map(Some);
    }
    // Writers: the data goes to CSV first, then into the workbook.
    let path_at = 1;
    let path = arg(a, path_at, name)?.text()?;
    let (rest, sheet) = take_opt(a, "Sheet");
    let (mut rest, range) = take_opt(&rest, "Range");
    let start = match range {
        Some(r) => {
            let (r0, c0, _, _) = range_ref(&r.text()?)?;
            (r0, c0)
        }
        None => (0, 0),
    };
    let tmp = temp_csv();
    rest[path_at] = Value::chars(&tmp.display().to_string());
    it.call_function(name, rest, 0)?;
    let text = std::fs::read_to_string(&tmp).map_err(err)?;
    let _ = std::fs::remove_file(&tmp);
    let cells: Vec<Vec<Cell>> = parse_csv(&text).iter().map(|r| r.iter().map(|f| cell_of_text(f)).collect()).collect();
    write_cells(&path, sheet.as_ref(), start, &cells)?;
    Ok(Some(Vec::new()))
}

fn raw_value(c: &Cell) -> Value {
    match c {
        Cell::Empty => Value::num(f64::NAN),
        Cell::Num(x) => Value::num(*x),
        Cell::Text(t) => Value::chars(t),
    }
}

fn xlsread(a: &[Value], nargout: usize) -> Ret {
    let path = arg(a, 0, "xlsread")?.text()?;
    // xlsread(file, sheet, range) or xlsread(file, range).
    let (sheet, range) = match (a.get(1), a.get(2)) {
        (Some(s), Some(r)) => (Some(s.clone()), Some(r.text()?)),
        (Some(v), None) if v.is_text() && v.text().map(|t| t.contains(':') || (t.len() <= 7 && cell_ref(&t).is_ok() && t.chars().any(|c| c.is_ascii_digit()))).unwrap_or(false) => (None, Some(v.text()?)),
        (Some(s), None) => (Some(s.clone()), None),
        _ => (None, None),
    };
    let sheet = sheet_name(&path, sheet.as_ref())?;
    let rows = read_sheet(&path, &sheet, range.as_deref())?;
    let h = rows.len();
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let is_num = |c: &Cell| matches!(c, Cell::Num(_));
    // num: the numbers, without leading and trailing rows and columns that hold none.
    let rows_with: Vec<usize> = (0..h).filter(|&i| rows[i].iter().any(is_num)).collect();
    let cols_with: Vec<usize> = (0..w).filter(|&j| rows.iter().any(|r| r.get(j).is_some_and(is_num))).collect();
    let num = match (rows_with.first(), rows_with.last(), cols_with.first(), cols_with.last()) {
        (Some(&r0), Some(&r1), Some(&c0), Some(&c1)) => {
            let (nr, nc) = (r1 - r0 + 1, c1 - c0 + 1);
            let mut data = vec![f64::NAN; nr * nc];
            for i in 0..nr {
                for j in 0..nc {
                    if let Some(Cell::Num(x)) = rows[r0 + i].get(c0 + j) {
                        data[j * nr + i] = *x;
                    }
                }
            }
            Matrix::new(nr, nc, data)
        }
        _ => Matrix::new(0, 0, Vec::new()),
    };
    let mut out = vec![Value::Num(num)];
    if nargout > 1 {
        let txt: Vec<Value> = (0..w)
            .flat_map(|j| (0..h).map(move |i| (i, j)))
            .map(|(i, j)| match rows[i].get(j) {
                Some(Cell::Text(t)) => Value::chars(t),
                _ => Value::chars(""),
            })
            .collect();
        out.push(Value::Cell(CellArray::new(h, w, txt)));
    }
    if nargout > 2 {
        let raw: Vec<Value> = (0..w).flat_map(|j| (0..h).map(move |i| (i, j))).map(|(i, j)| raw_value(rows[i].get(j).unwrap_or(&Cell::Empty))).collect();
        out.push(Value::Cell(CellArray::new(h, w, raw)));
    }
    Ok(Some(out))
}

fn cells_of_value(v: &Value) -> R<Vec<Vec<Cell>>> {
    Ok(match v {
        Value::Cell(c) => {
            let (r, k) = (c.rows(), c.cols());
            (0..r)
                .map(|i| {
                    (0..k)
                        .map(|j| match &c.data[j * r + i] {
                            Value::Num(m) if m.kind != Kind::Char && m.numel() >= 1 => Cell::Num(m.data[0]),
                            Value::Num(m) if m.numel() == 0 => Cell::Empty,
                            other => other.text().map(|t| Cell::Text(t)).unwrap_or(Cell::Empty),
                        })
                        .collect()
                })
                .collect()
        }
        Value::Num(m) if m.kind == Kind::Char => vec![vec![Cell::Text(m.to_string_lossy())]],
        Value::Str(s) => {
            let (r, k) = (s.dims[0], s.data.len() / s.dims[0].max(1));
            (0..r).map(|i| (0..k).map(|j| s.data[j * r + i].clone().map(Cell::Text).unwrap_or(Cell::Empty)).collect()).collect()
        }
        other => {
            let m = other.to_matrix()?;
            let (r, k) = (m.rows(), m.cols());
            (0..r).map(|i| (0..k).map(|j| Cell::Num(m.data[j * r + i])).collect()).collect()
        }
    })
}

fn xlswrite(it: &mut Interpreter, a: &[Value]) -> Ret {
    let path = arg(a, 0, "xlswrite")?.text()?;
    let cells = cells_of_value(arg(a, 1, "xlswrite")?)?;
    let (sheet, range) = match (a.get(2), a.get(3)) {
        (Some(s), Some(r)) => (Some(s.clone()), Some(r.text()?)),
        (Some(v), None) if v.is_text() && v.text().map(|t| t.contains(':') || cell_ref(&t).is_ok() && t.chars().any(|c| c.is_ascii_digit())).unwrap_or(false) => (None, Some(v.text()?)),
        (Some(s), None) => (Some(s.clone()), None),
        _ => (None, None),
    };
    let start = match range {
        Some(r) => {
            let (r0, c0, _, _) = range_ref(&r)?;
            (r0, c0)
        }
        None => (0, 0),
    };
    let existed = Path::new(&path).is_file();
    if write_cells(&path, sheet.as_ref(), start, &cells)? && existed {
        it.emit(OutputKind::Warning("Added specified worksheet.".into()));
    }
    Ok(Some(vec![Value::bool(true)]))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "xlsread" => xlsread(a, nargout),
        "xlswrite" => xlswrite(it, a),
        "xlsfinfo" | "sheetnames" => {
            let path = arg(a, 0, name)?.text()?;
            if name == "xlsfinfo" && !std::path::Path::new(&path).exists() {
                let full = std::env::current_dir().map(|d| d.join(&path).display().to_string()).unwrap_or_else(|_| path.clone());
                return Err(err(format!("File '{full}' not found.")));
            }
            let wb = open_workbook_auto(&path).map_err(|e| err(format!("Unable to open file '{path}': {e}")))?;
            let names = wb.sheet_names();
            if name == "sheetnames" {
                return one(Value::Str(StrArray::col(names)));
            }
            // [status, sheets, xlFormat]: Excel's name of the file format.
            let ext = std::path::Path::new(&path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let format = match ext.as_str() {
                "xlsx" => "xlOpenXMLWorkbook",
                "xlsm" => "xlOpenXMLWorkbookMacroEnabled",
                "xlsb" => "xlExcel12",
                "xls" => "xlWorkbookNormal",
                _ => "",
            };
            Ok(Some(vec![
                Value::chars("Microsoft Excel Spreadsheet"),
                Value::Cell(CellArray::row(names.iter().map(|n| Value::chars(n)).collect())),
                Value::chars(format),
            ]))
        }
        _ => Ok(None),
    }
}
