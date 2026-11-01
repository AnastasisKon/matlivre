//! Timetables: a [`Table`] with row times. Construction (`timetable`, `array2timetable`,
//! `table2timetable`), conversion back (`timetable2table`), the row and variable subscripts
//! `timerange`, `withtol` and `vartype`, resampling (`retime`, `synchronize`, `lag`), range
//! tests (`isregular`, `containsrange`, `overlapsrange`, `withinrange`) and file I/O
//! (`readtimetable`, `writetimetable`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::{arg, one, Ret};
use crate::class::ClassDef;
use crate::error::{rt, LangError};
use crate::indexing;
use crate::interp::Interpreter;
use crate::nd::IndexArg;
use crate::table::Table;
use crate::time::{self, TimeArray, TimeKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "timetable", "array2timetable", "table2timetable", "timetable2table", "istimetable", "isregular", "timerange", "withtol", "vartype", "retime",
    "synchronize", "lag", "readtimetable", "writetimetable", "containsrange", "overlapsrange", "withinrange",
];

const MS_SEC: f64 = 1000.0;
const MS_MIN: f64 = 60_000.0;
const MS_HOUR: f64 = 3_600_000.0;
const MS_DAY: f64 = 86_400_000.0;

// ---- subscript objects ---------------------------------------------------------------------

fn class_def(name: &'static str, props: &[&str]) -> Rc<ClassDef> {
    thread_local! {
        static DEFS: RefCell<HashMap<&'static str, Rc<ClassDef>>> = RefCell::new(HashMap::new());
    }
    DEFS.with(|d| {
        if let Some(def) = d.borrow().get(name) {
            return def.clone();
        }
        let mut src = format!("classdef {name}\n  properties (Hidden)\n");
        for p in props {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        d.borrow_mut().insert(name, def.clone());
        def
    })
}

fn object(name: &'static str, props: &[&str], values: Vec<Value>) -> Value {
    Value::Object(Object { class: class_def(name, props), props: Rc::new(RefCell::new(values)) })
}

fn obj_of<'a>(v: &'a Value, class: &str) -> Option<&'a Object> {
    match v {
        Value::Object(o) if o.class.name == class => Some(o),
        _ => None,
    }
}

/// The type a `vartype` subscript selects.
pub fn vartype_of(v: &Value) -> Option<String> {
    obj_of(v, "vartype").and_then(|o| o.get("Type")).and_then(|t| t.text().ok())
}

/// Whether a table variable has the type a `vartype` names.
pub fn matches_type(v: &Value, ty: &str) -> bool {
    match ty {
        "numeric" => matches!(v, Value::Num(m) if !matches!(m.kind, Kind::Char | Kind::Logical)),
        "float" => matches!(v, Value::Num(m) if matches!(m.kind, Kind::Double | Kind::Single)),
        "integer" => matches!(v, Value::Num(m) if matches!(m.kind, Kind::Int(_))),
        "cellstr" => matches!(v, Value::Cell(c) if c.data.iter().all(|x| matches!(x, Value::Num(m) if m.kind == Kind::Char))),
        other => v.class_name() == other,
    }
}

fn times_of(v: &Value) -> R<&TimeArray> {
    match v {
        Value::Time(t) if t.kind != TimeKind::CalDuration => Ok(t),
        _ => rt("Row times must be a datetime or duration vector."),
    }
}

/// A time as shown in subscript displays (datetimes with their time of day).
fn time_text(t: &TimeArray, i: usize) -> String {
    match t.kind {
        TimeKind::DateTime => {
            let pattern = if t.format.is_empty() || t.format == "dd-MMM-yyyy" { time::DEFAULT_DT_FORMAT } else { &t.format };
            time::format_datetime(t.wall(i), pattern)
        }
        _ => t.format_elem(i),
    }
}

/// `x = ...` display of a `timerange`, `withtol` or `vartype` subscript.
pub fn display(name: &str, v: &Value) -> Option<String> {
    let Value::Object(o) = v else { return None };
    match o.class.name.as_str() {
        "timerange" => {
            let (start, stop) = (o.get("Start")?, o.get("Stop")?);
            let ty = o.get("Type").and_then(|t| t.text().ok()).unwrap_or_default();
            let end = |v: &Value| match v {
                Value::Time(t) if !t.data.is_empty() => time_text(t, 0),
                Value::Num(m) if !m.data.is_empty() => crate::display::fmt_g(m.data[0], 5).replace("inf", "Inf"),
                _ => String::new(),
            };
            let (kind, l, r) = match ty.as_str() {
                "closed" => ("closed interval", '[', ']'),
                "open" => ("open interval", '(', ')'),
                "openleft" | "closedright" => ("half-open interval", '(', ']'),
                _ => ("half-open interval", '[', ')'),
            };
            Some(format!(
                "{name} = \n\ttimetable timerange subscript:\n\n\t\tSelect timetable rows with times in the {kind}:\n\t\t{l}{}, {}{r}",
                end(&start),
                end(&stop)
            ))
        }
        "withtol" => {
            let Value::Time(times) = o.get("Times")? else { return None };
            let Value::Time(tol) = o.get("Tol")? else { return None };
            let lines: Vec<String> = (0..times.data.len()).map(|i| format!("\t\t{}", time_text(&times, i))).collect();
            Some(format!(
                "{name} = \n\ttimetable withtol subscript:\n\n\t\tSelect timetable rows matching the following times:\n\n{}\n\t\twith tolerance of +/- {}",
                lines.join("\n"),
                tol.format_elem(0)
            ))
        }
        "vartype" => {
            let ty = o.get("Type")?.text().ok()?;
            Some(format!("{name} = \n\ttable vartype subscript:\n\n\t\tSelect table variables matching the type '{ty}'"))
        }
        _ => None,
    }
}

/// The bounds of a time range as (lo, hi, lo closed, hi closed) in ms.
fn range_bounds(v: &Value) -> R<Option<(f64, f64, bool, bool)>> {
    if let Some(o) = obj_of(v, "timerange") {
        let ms = |x: Option<Value>| -> R<f64> {
            match x {
                Some(Value::Time(t)) => Ok(t.data.first().map(|d| d[0]).unwrap_or(f64::NAN)),
                Some(Value::Num(m)) => Ok(m.data.first().copied().unwrap_or(f64::NAN)),
                _ => rt("Invalid timerange."),
            }
        };
        let ty = o.get("Type").and_then(|t| t.text().ok()).unwrap_or_default();
        let (lc, hc) = match ty.as_str() {
            "closed" => (true, true),
            "open" => (false, false),
            "openleft" | "closedright" => (false, true),
            _ => (true, false),
        };
        return Ok(Some((ms(o.get("Start"))?, ms(o.get("Stop"))?, lc, hc)));
    }
    if let Value::Time(t) = v {
        let ms = t.ms();
        let lo = ms.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = ms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        return Ok(Some((lo, hi, true, true)));
    }
    Ok(None)
}

fn check_kinds(times: &TimeArray, other: &Value) -> R<()> {
    if let Value::Time(o) = other {
        if o.kind != times.kind {
            return rt(format!("Cannot compare {} row times with {} values.", times.class_name(), o.class_name()));
        }
    }
    Ok(())
}

/// Rows of a timetable selected by a time subscript (row times, `timerange` or `withtol`).
pub fn time_rows(times: &Value, v: &Value) -> R<Option<Vec<usize>>> {
    let times = times_of(times)?;
    let ms = times.ms();
    if let Value::Time(q) = v {
        check_kinds(times, v)?;
        let mut out = Vec::new();
        for x in q.ms() {
            out.extend((0..ms.len()).filter(|&i| ms[i] == x));
        }
        return Ok(Some(out));
    }
    if let Some(o) = obj_of(v, "timerange") {
        if let Some(s) = o.get("Start") {
            check_kinds(times, &s)?;
        }
        let (lo, hi, lc, hc) = range_bounds(v)?.unwrap();
        let inside = |t: f64| (if lc { t >= lo } else { t > lo }) && (if hc { t <= hi } else { t < hi });
        return Ok(Some((0..ms.len()).filter(|&i| inside(ms[i])).collect()));
    }
    if let Some(o) = obj_of(v, "withtol") {
        let (Some(Value::Time(q)), Some(Value::Time(tol))) = (o.get("Times"), o.get("Tol")) else { return rt("Invalid withtol subscript.") };
        check_kinds(times, &Value::Time(q.clone()))?;
        let (q, tol) = (q.ms(), tol.data[0][0]);
        return Ok(Some((0..ms.len()).filter(|&i| q.iter().any(|&x| (ms[i] - x).abs() <= tol)).collect()));
    }
    Ok(None)
}

// ---- helpers ---------------------------------------------------------------------------------

/// The step of evenly spaced row times (ms), if they are regular.
pub fn regular_step(times: &Value) -> Option<f64> {
    let Value::Time(t) = times else { return None };
    let ms = t.ms();
    if ms.len() < 2 {
        return None;
    }
    let d = ms[1] - ms[0];
    let tol = 1e-9 * d.abs().max(1.0);
    (d != 0.0 && ms.windows(2).all(|w| ((w[1] - w[0]) - d).abs() <= tol)).then_some(d)
}

/// A step (ms) as a duration in the row times' units.
pub fn step_value(times: &Value, ms: f64) -> Value {
    let format = match times {
        Value::Time(t) if t.kind == TimeKind::Duration => t.format.clone(),
        _ => "hh:mm:ss".to_string(),
    };
    Value::Time(TimeArray::durations(vec![1, 1], vec![ms], &format))
}

fn times_like(template: &Value, ms: Vec<f64>) -> R<Value> {
    let t = times_of(template)?;
    let n = ms.len();
    Ok(Value::Time(t.with_ms(vec![n, 1], ms)))
}

fn ms_of(v: &Value) -> R<Vec<f64>> {
    Ok(times_of(v)?.ms())
}

/// Rows `idx` of a variable, with missing values where `idx` is None.
fn gather_opt(v: &Value, idx: &[Option<usize>]) -> R<Value> {
    let n = idx.len();
    let rows = v.dims()[0];
    if rows == 0 {
        let cols = v.dims().get(1).copied().unwrap_or(1).max(1);
        return Ok(match v {
            Value::Cell(_) => Value::Cell(CellArray::from_dims(vec![n, 1], vec![Value::chars(""); n])),
            Value::Str(_) => Value::Str(StrArray::from_dims(vec![n, 1], vec![None; n])),
            Value::Time(t) => Value::Time(t.with_ms(vec![n, 1], vec![f64::NAN; n])),
            _ => Value::Num(Matrix::from_dims(vec![n, cols], vec![f64::NAN; n * cols])),
        });
    }
    let ix: Vec<usize> = idx.iter().map(|i| i.unwrap_or(0)).collect();
    let mut out = indexing::index(v, &[IndexArg::List { ix, dims: vec![n, 1], from_mask: false }, IndexArg::All])?;
    let missing: Vec<usize> = (0..n).filter(|&r| idx[r].is_none()).collect();
    if missing.is_empty() {
        return Ok(out);
    }
    match &mut out {
        Value::Num(m) => {
            if !matches!(m.kind, Kind::Double | Kind::Single) {
                *m = m.clone().cast_to(Kind::Double);
            }
            let cols = m.data.len() / n.max(1);
            for &r in &missing {
                for c in 0..cols {
                    m.data[c * n + r] = f64::NAN;
                }
            }
        }
        Value::Cell(c) => {
            for &r in &missing {
                c.data[r] = Value::chars("");
            }
        }
        Value::Str(s) => {
            for &r in &missing {
                s.data[r] = None;
            }
        }
        Value::Time(t) => {
            for &r in &missing {
                t.data[r] = [f64::NAN, 0.0, 0.0];
            }
        }
        _ => {}
    }
    Ok(out)
}

fn table_of<'a>(v: &'a Value, fname: &str) -> R<&'a Table> {
    match v {
        Value::Table(t) => Ok(t),
        _ => rt(format!("The first input to '{fname}' must be a table or timetable.")),
    }
}

fn timetable_of<'a>(v: &'a Value, fname: &str) -> R<&'a Table> {
    match v {
        Value::Table(t) if t.is_timetable() => Ok(t),
        _ => rt(format!("The first input to '{fname}' must be a timetable.")),
    }
}

fn names_of(v: &Value) -> R<Vec<String>> {
    match v {
        Value::Cell(c) => c.data.iter().map(|x| x.text()).collect(),
        Value::Str(s) => Ok(s.data.iter().map(|x| x.clone().unwrap_or_default()).collect()),
        other => Ok(vec![other.text()?]),
    }
}

/// Positional arguments and recognized name-value options.
fn split_opts(args: &[Value], names: &[&str]) -> (Vec<Value>, Vec<(String, Value)>) {
    let mut data = Vec::new();
    let mut opts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if let Ok(t) = args[i].text() {
            if args[i].is_text() {
                if let Some(n) = names.iter().find(|n| n.eq_ignore_ascii_case(&t)) {
                    if let Some(v) = args.get(i + 1) {
                        opts.push((n.to_string(), v.clone()));
                        i += 2;
                        continue;
                    }
                }
            }
        }
        data.push(args[i].clone());
        i += 1;
    }
    (data, opts)
}

fn opt<'a>(opts: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    opts.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// Row times from the RowTimes, TimeStep, SampleRate and StartTime options.
fn generated_times(opts: &[(String, Value)], n: usize) -> R<Option<Value>> {
    if let Some(rt_) = opt(opts, "RowTimes") {
        times_of(rt_)?;
        if rt_.numel() != n {
            return rt(format!("The number of row times must match the number of rows ({n})."));
        }
        let ms = ms_of(rt_)?;
        return Ok(Some(times_like(rt_, ms)?));
    }
    let start = opt(opts, "StartTime").cloned();
    let (step_ms, default_format) = if let Some(ts) = opt(opts, "TimeStep") {
        let Value::Time(t) = ts else { return rt("TimeStep must be a duration.") };
        if t.kind != TimeKind::Duration {
            return rt("TimeStep must be a duration in this version.");
        }
        (t.data[0][0], "hh:mm:ss")
    } else if let Some(sr) = opt(opts, "SampleRate") {
        (MS_SEC / sr.scalar()?, "s")
    } else {
        return Ok(None);
    };
    let template = start.unwrap_or_else(|| Value::Time(TimeArray::durations(vec![1, 1], vec![0.0], default_format)));
    let t0 = ms_of(&template)?.first().copied().unwrap_or(0.0);
    Ok(Some(times_like(&template, (0..n).map(|k| t0 + k as f64 * step_ms).collect())?))
}

fn make_timetable(names: Vec<String>, vars: Vec<Value>, times: Value, dim: &str) -> R<Table> {
    let n = times.numel();
    let mut t = Table::new(names, vars)?;
    if !t.vars.is_empty() && t.nrows != n {
        return rt(format!("The number of row times ({n}) must match the number of rows ({}).", t.nrows));
    }
    t.nrows = n;
    t.row_times = Some(times);
    t.dim_names = [dim.to_string(), "Variables".to_string()];
    Ok(t)
}

fn columns_of(m: &Value) -> R<Vec<Value>> {
    let (_, cols) = m.dims2();
    (0..cols).map(|c| indexing::index(m, &[IndexArg::All, IndexArg::scalar(c)])).collect()
}

// ---- resampling ------------------------------------------------------------------------------

const AGGREGATES: &[&str] = &["sum", "mean", "prod", "min", "max", "count", "firstvalue", "lastvalue", "median", "mode", "nnz"];

fn floor_calendar(ms: f64, unit: &str, tz_ms: f64) -> R<f64> {
    let w = ms + tz_ms;
    let (y, mo, d, h, mi, s) = time::components(w);
    let wall = match unit {
        "yearly" => time::wall_ms(y as f64, 1.0, 1.0, 0.0, 0.0, 0.0),
        "quarterly" => time::wall_ms(y as f64, (((mo - 1) / 3) * 3 + 1) as f64, 1.0, 0.0, 0.0, 0.0),
        "monthly" => time::wall_ms(y as f64, mo as f64, 1.0, 0.0, 0.0, 0.0),
        "weekly" => {
            let days = (w / MS_DAY).floor();
            let weekday = (days as i64 + 4).rem_euclid(7) as f64;
            (days - weekday) * MS_DAY
        }
        "daily" => time::wall_ms(y as f64, mo as f64, d as f64, 0.0, 0.0, 0.0),
        "hourly" => time::wall_ms(y as f64, mo as f64, d as f64, h as f64, 0.0, 0.0),
        "minutely" => time::wall_ms(y as f64, mo as f64, d as f64, h as f64, mi as f64, 0.0),
        "secondly" => time::wall_ms(y as f64, mo as f64, d as f64, h as f64, mi as f64, s.floor()),
        other => return rt(format!("'{other}' is not a valid time step.")),
    };
    Ok(wall - tz_ms)
}

fn next_calendar(ms: f64, unit: &str, tz_ms: f64) -> f64 {
    let w = ms + tz_ms;
    let (y, mo, d, h, mi, s) = time::components(w);
    let wall = match unit {
        "yearly" => time::wall_ms(y as f64 + 1.0, mo as f64, d as f64, h as f64, mi as f64, s),
        "quarterly" => time::wall_ms(y as f64, mo as f64 + 3.0, d as f64, h as f64, mi as f64, s),
        "monthly" => time::wall_ms(y as f64, mo as f64 + 1.0, d as f64, h as f64, mi as f64, s),
        "weekly" => w + 7.0 * MS_DAY,
        "daily" => w + MS_DAY,
        "hourly" => w + MS_HOUR,
        "minutely" => w + MS_MIN,
        _ => w + MS_SEC,
    };
    wall - tz_ms
}

/// The new time vector of `retime`/`synchronize` and whether its last bin is a full period.
fn new_times(times: &[f64], spec: &Value, template: &Value, opts: &[(String, Value)]) -> R<(Vec<f64>, Option<f64>, Option<String>)> {
    if let Value::Time(_) = spec {
        return Ok((ms_of(spec)?, None, None));
    }
    let unit = spec.text()?.to_lowercase();
    let lo = times.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !lo.is_finite() {
        return Ok((Vec::new(), None, None));
    }
    if unit == "regular" {
        let step = if let Some(Value::Time(t)) = opt(opts, "TimeStep") {
            t.data[0][0]
        } else if let Some(sr) = opt(opts, "SampleRate") {
            MS_SEC / sr.scalar()?
        } else {
            return rt("Specify 'TimeStep' or 'SampleRate' with 'regular'.");
        };
        let first = (lo / step).floor() * step;
        let mut out = Vec::new();
        let mut k = 0.0;
        while first + k * step <= hi + 1e-9 * step.abs() {
            out.push(first + k * step);
            k += 1.0;
        }
        return Ok((out, Some(step), None));
    }
    let tz = match template {
        Value::Time(t) if t.kind == TimeKind::DateTime => t.tz.unwrap_or(0.0) * MS_MIN,
        Value::Time(_) => {
            // Durations: fixed periods.
            let step = match unit.as_str() {
                "daily" => MS_DAY,
                "hourly" => MS_HOUR,
                "minutely" => MS_MIN,
                "secondly" => MS_SEC,
                other => return rt(format!("'{other}' is not a valid time step for duration row times.")),
            };
            let first = (lo / step).floor() * step;
            let mut out = Vec::new();
            let mut t = first;
            while t <= hi {
                out.push(t);
                t += step;
            }
            return Ok((out, Some(step), None));
        }
        _ => 0.0,
    };
    let mut t = floor_calendar(lo, &unit, tz)?;
    let mut out = Vec::new();
    while t <= hi {
        out.push(t);
        t = next_calendar(t, &unit, tz);
    }
    Ok((out, None, Some(unit)))
}

fn aggregate(it: &mut Interpreter, method: &Value, vals: &[f64]) -> R<f64> {
    if let Value::Func(_) = method {
        let r = it.call_func_value(method, vec![Value::Num(Matrix::col(vals.to_vec()))], 1)?;
        return r.first().map(|v| v.scalar()).transpose()?.ok_or_else(|| LangError::new("The aggregation function returned nothing.", 0));
    }
    let m = method.text()?.to_lowercase();
    let n = vals.len();
    Ok(match m.as_str() {
        "sum" => vals.iter().sum(),
        "prod" => vals.iter().product(),
        "count" => n as f64,
        "nnz" => vals.iter().filter(|&&v| v != 0.0).count() as f64,
        _ if n == 0 => f64::NAN,
        "mean" => vals.iter().sum::<f64>() / n as f64,
        "min" => vals.iter().copied().fold(f64::INFINITY, f64::min),
        "max" => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "firstvalue" => vals[0],
        "lastvalue" => vals[n - 1],
        "median" | "mode" => {
            let mut s = vals.to_vec();
            s.sort_by(f64::total_cmp);
            if m == "median" {
                if n % 2 == 1 { s[n / 2] } else { 0.5 * (s[n / 2 - 1] + s[n / 2]) }
            } else {
                let mut best = (s[0], 0usize);
                let mut k = 0;
                while k < n {
                    let j = (k..n).take_while(|&j| s[j] == s[k]).count();
                    if j > best.1 {
                        best = (s[k], j);
                    }
                    k += j;
                }
                best.0
            }
        }
        other => return rt(format!("'{other}' is not a valid method.")),
    })
}

/// Resamples one timetable onto `target` times.
#[allow(clippy::too_many_arguments)]
fn resample(it: &mut Interpreter, t: &Table, target: &[f64], last_bin: Option<f64>, calendar: Option<&str>, method: &Value, opts: &[(String, Value)]) -> R<Table> {
    let times = t.row_times.as_ref().unwrap();
    let ms = ms_of(times)?;
    let mut order: Vec<usize> = (0..ms.len()).filter(|&i| !ms[i].is_nan()).collect();
    order.sort_by(|&a, &b| ms[a].total_cmp(&ms[b]));
    let sorted: Vec<f64> = order.iter().map(|&i| ms[i]).collect();
    let n = target.len();
    let mname = if let Value::Func(_) = method { "function".to_string() } else { method.text()?.to_lowercase() };
    let mut out = t.clone();
    out.row_times = Some(times_like(times, target.to_vec())?);
    out.nrows = n;
    out.row_names = None;
    let constant = opt(opts, "Constant").cloned();
    let end_values = opt(opts, "EndValues").cloned();
    let tz = match times {
        Value::Time(tt) => tt.tz.unwrap_or(0.0) * MS_MIN,
        _ => 0.0,
    };
    if AGGREGATES.contains(&mname.as_str()) || mname == "function" {
        let bins: Vec<Vec<usize>> = (0..n)
            .map(|i| {
                let lo = target[i];
                let hi = if i + 1 < n {
                    target[i + 1]
                } else if let Some(step) = last_bin {
                    lo + step
                } else if let Some(u) = calendar {
                    next_calendar(lo, u, tz)
                } else {
                    lo
                };
                (0..sorted.len()).filter(|&k| if hi > lo { sorted[k] >= lo && sorted[k] < hi } else { sorted[k] == lo }).map(|k| order[k]).collect()
            })
            .collect();
        for k in 0..t.vars.len() {
            let Value::Num(m) = &t.vars[k] else { return rt(format!("Unable to aggregate the variable '{}' with '{mname}'.", t.names[k])) };
            let rows = m.rows();
            let cols = m.data.len() / rows.max(1);
            let mut data = vec![0.0; n * cols];
            for c in 0..cols {
                for (i, bin) in bins.iter().enumerate() {
                    let vals: Vec<f64> = bin.iter().map(|&r| m.data[c * rows + r]).collect();
                    data[c * n + i] = aggregate(it, method, &vals)?;
                }
            }
            out.vars[k] = Value::Num(Matrix::from_dims(vec![n, cols], data));
        }
        return Ok(out);
    }
    // Index-based and interpolating methods.
    let lo = sorted.first().copied().unwrap_or(f64::NAN);
    let hi = sorted.last().copied().unwrap_or(f64::NAN);
    let pick = |q: f64| -> Option<usize> {
        match mname.as_str() {
            "previous" => sorted.iter().rposition(|&x| x <= q),
            "next" => sorted.iter().position(|&x| x >= q),
            "nearest" => (0..sorted.len()).min_by(|&a, &b| (sorted[a] - q).abs().total_cmp(&(sorted[b] - q).abs())),
            _ => sorted.iter().position(|&x| x == q),
        }
        .map(|k| order[k])
    };
    let idx: Vec<Option<usize>> = target.iter().map(|&q| pick(q)).collect();
    for k in 0..t.vars.len() {
        let v = &t.vars[k];
        let mut nv = match mname.as_str() {
            "linear" | "spline" | "pchip" | "makima" => {
                let Value::Num(m) = v else { return rt(format!("Unable to interpolate the variable '{}' with '{mname}'.", t.names[k])) };
                let rows = m.rows();
                let cols = m.data.len() / rows.max(1);
                let x = Value::Num(Matrix::col(sorted.clone()));
                let ys: Vec<f64> = (0..cols).flat_map(|c| order.iter().map(move |&r| (c, r))).map(|(c, r)| m.data[c * rows + r]).collect();
                let y = Value::Num(Matrix::new(sorted.len(), cols, ys));
                let q = Value::Num(Matrix::col(target.to_vec()));
                it.call_function("interp1", vec![x, y, q, Value::chars(&mname), Value::chars("extrap")], 1)?.remove(0)
            }
            "fillwithmissing" | "fillwithconstant" | "previous" | "next" | "nearest" | "default" => gather_opt(v, &idx)?,
            other => return rt(format!("'{other}' is not a valid method.")),
        };
        // Fill values: the constant, and EndValues outside the data's time range.
        if let Value::Num(m) = &mut nv {
            let cols = m.data.len() / n.max(1);
            for i in 0..n {
                let outside = target[i] < lo || target[i] > hi;
                let fill = if mname == "fillwithconstant" && idx[i].is_none() {
                    Some(constant.as_ref().map(|c| c.scalar()).transpose()?.unwrap_or(0.0))
                } else if outside {
                    match &end_values {
                        Some(e) if !e.is_text() => Some(e.scalar()?),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some(fv) = fill {
                    for c in 0..cols {
                        m.data[c * n + i] = fv;
                    }
                }
            }
        }
        out.vars[k] = nv;
    }
    Ok(out)
}

fn retime(it: &mut Interpreter, a: &[Value]) -> Ret {
    let t = timetable_of(arg(a, 0, "retime")?, "retime")?;
    let (data, opts) = split_opts(&a[1..], &["TimeStep", "SampleRate", "Constant", "EndValues", "IncludedEdge"]);
    let spec = data.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
    let method = data.get(1).cloned().unwrap_or_else(|| Value::chars("fillwithmissing"));
    let times = t.row_times.as_ref().unwrap();
    let (target, step, cal) = new_times(&ms_of(times)?, spec, times, &opts)?;
    one(Value::Table(Box::new(resample(it, t, &target, step, cal.as_deref(), &method, &opts)?)))
}

fn synchronize(it: &mut Interpreter, a: &[Value]) -> Ret {
    let arg_names = std::mem::take(&mut it.arg_names);
    let tables: Vec<(usize, &Table)> = a.iter().enumerate().filter_map(|(i, v)| if let Value::Table(t) = v { Some((i, &**t)) } else { None }).collect();
    if tables.is_empty() {
        return rt("synchronize requires timetable inputs.");
    }
    if tables.iter().any(|(_, t)| !t.is_timetable()) {
        return rt("All inputs to synchronize must be timetables.");
    }
    let rest: Vec<Value> = a[tables.last().unwrap().0 + 1..].to_vec();
    let (data, opts) = split_opts(&rest, &["TimeStep", "SampleRate", "Constant", "EndValues", "IncludedEdge"]);
    let basis = data.first().cloned().unwrap_or_else(|| Value::chars("union"));
    let method = data.get(1).cloned().unwrap_or_else(|| Value::chars("fillwithmissing"));
    let all: Vec<Vec<f64>> = tables.iter().map(|(_, t)| ms_of(t.row_times.as_ref().unwrap())).collect::<R<_>>()?;
    let uniq = |mut v: Vec<f64>| {
        v.retain(|x| !x.is_nan());
        v.sort_by(f64::total_cmp);
        v.dedup();
        v
    };
    let template = tables[0].1.row_times.clone().unwrap();
    let (target, step, cal) = match basis.text().ok().map(|s| s.to_lowercase()).as_deref() {
        Some("union") => (uniq(all.concat()), None, None),
        Some("intersection") => (uniq(all[0].iter().copied().filter(|x| all.iter().all(|v| v.contains(x))).collect()), None, None),
        Some("first") => (all[0].clone(), None, None),
        Some("last") => (all[all.len() - 1].clone(), None, None),
        Some("commonrange") => {
            let lo = all.iter().map(|v| v.iter().copied().fold(f64::INFINITY, f64::min)).fold(f64::NEG_INFINITY, f64::max);
            let hi = all.iter().map(|v| v.iter().copied().fold(f64::NEG_INFINITY, f64::max)).fold(f64::INFINITY, f64::min);
            (uniq(all.concat().into_iter().filter(|&x| x >= lo && x <= hi).collect()), None, None)
        }
        _ => new_times(&all.concat(), &basis, &template, &opts)?,
    };
    let mut parts = Vec::new();
    for (i, t) in &tables {
        parts.push((*i, resample(it, t, &target, step, cal.as_deref(), &method, &opts)?));
    }
    // Variable names shared by several inputs get the input's name as a suffix.
    let mut count: HashMap<String, usize> = HashMap::new();
    for (_, p) in &parts {
        for n in &p.names {
            *count.entry(n.clone()).or_default() += 1;
        }
    }
    let mut out = parts[0].1.clone();
    out.names.clear();
    out.vars.clear();
    for (k, (i, p)) in parts.into_iter().enumerate() {
        let suffix = arg_names.get(i).cloned().flatten().unwrap_or_else(|| format!("{}", k + 1));
        for (n, v) in p.names.into_iter().zip(p.vars) {
            let name = if count[&n] > 1 { format!("{n}_{suffix}") } else { n };
            out.names.push(name);
            out.vars.push(v);
        }
    }
    one(Value::Table(Box::new(out)))
}

fn lag(a: &[Value]) -> Ret {
    let t = timetable_of(arg(a, 0, "lag")?, "lag")?;
    let n = match a.get(1) {
        Some(v) if !v.is_text() && !matches!(v, Value::Time(_)) => v.scalar()? as i64,
        Some(Value::Time(d)) => {
            let step = regular_step(t.row_times.as_ref().unwrap()).ok_or_else(|| LangError::new("Lagging by a duration requires a regular timetable.", 0))?;
            (d.data[0][0] / step).round() as i64
        }
        _ => 1,
    };
    let rows = t.nrows as i64;
    let idx: Vec<Option<usize>> = (0..rows).map(|r| { let s = r - n; (s >= 0 && s < rows).then_some(s as usize) }).collect();
    let mut out = t.clone();
    for k in 0..out.vars.len() {
        out.vars[k] = gather_opt(&t.vars[k], &idx)?;
    }
    one(Value::Table(Box::new(out)))
}

fn range_test(name: &str, a: &[Value]) -> Ret {
    let t = timetable_of(arg(a, 0, name)?, name)?;
    let r = arg(a, 1, name)?;
    let ms = ms_of(t.row_times.as_ref().unwrap())?;
    let lo = ms.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = ms.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let Some((a0, b0, lc, hc)) = range_bounds(r)? else { return rt(format!("The second input to '{name}' must be a timerange or a datetime or duration vector.")) };
    let inside = |x: f64| (if lc { x >= a0 } else { x > a0 }) && (if hc { x <= b0 } else { x < b0 });
    let tf = match name {
        "withinrange" => ms.is_empty() || (inside(lo) && inside(hi)),
        "containsrange" => lo <= a0 && hi >= b0,
        _ => ms.iter().any(|&x| inside(x)) || (lo <= a0 && hi >= b0),
    };
    one(Value::bool(tf))
}

fn timetable2table(t: &Table) -> R<Table> {
    let mut out = t.clone();
    if let Some(times) = out.row_times.take() {
        out.names.insert(0, t.dim_names[0].clone());
        out.vars.insert(0, times);
        out.dim_names = ["Row".into(), "Variables".into()];
    }
    Ok(out)
}

fn table2timetable(t: &Table, opts: &[(String, Value)]) -> R<Table> {
    let mut out = t.clone();
    let (times, dim) = match opt(opts, "RowTimes") {
        Some(v) if v.is_text() => {
            let name = v.text()?;
            let k = t.names.iter().position(|n| *n == name).ok_or_else(|| LangError::new(format!("Unrecognized table variable name '{name}'."), 0))?;
            out.names.remove(k);
            (out.vars.remove(k), name)
        }
        Some(v) => (v.clone(), "Time".to_string()),
        None => match generated_times(opts, t.nrows)? {
            Some(g) => (g, "Time".to_string()),
            None => {
                let k = t
                    .vars
                    .iter()
                    .position(|v| matches!(v, Value::Time(x) if x.kind != TimeKind::CalDuration))
                    .ok_or_else(|| LangError::new("Input table must contain datetime or duration vector for row times.", 0))?;
                out.names.remove(k);
                (out.vars.remove(k), t.names[k].clone())
            }
        },
    };
    times_of(&times)?;
    if times.numel() != t.nrows {
        return rt("The number of row times must match the number of rows.");
    }
    out.row_times = Some(times);
    out.row_names = None;
    out.dim_names = [dim, "Variables".into()];
    Ok(out)
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    let opt_names = &["RowTimes", "TimeStep", "SampleRate", "StartTime", "VariableNames", "ConvertRowTimes"];
    match name {
        "timetable" => {
            let arg_names = std::mem::take(&mut it.arg_names);
            let mut positions = Vec::new();
            let mut opts = Vec::new();
            let mut i = 0;
            while i < a.len() {
                if a[i].is_text() {
                    if let Some(n) = opt_names.iter().find(|n| a[i].text().is_ok_and(|t| n.eq_ignore_ascii_case(&t))) {
                        if let Some(v) = a.get(i + 1) {
                            opts.push((n.to_string(), v.clone()));
                            i += 2;
                            continue;
                        }
                    }
                }
                positions.push(i);
                i += 1;
            }
            let mut data: Vec<(usize, Value)> = positions.iter().map(|&i| (i, a[i].clone())).collect();
            let nrows = data.iter().find(|(_, v)| !matches!(v, Value::Time(_))).map(|(_, v)| v.dims()[0]).or_else(|| data.first().map(|(_, v)| v.dims()[0])).unwrap_or(0);
            let (times, dim) = match generated_times(&opts, nrows)? {
                Some(g) => (g, "Time".to_string()),
                None => {
                    if data.is_empty() || !matches!(&data[0].1, Value::Time(t) if t.kind != TimeKind::CalDuration) {
                        return rt("Provide row times as the first input, or with 'RowTimes', 'TimeStep' or 'SampleRate'.");
                    }
                    let (i, v) = data.remove(0);
                    let dim = arg_names.get(i).cloned().flatten().unwrap_or_else(|| "Time".to_string());
                    let ms = ms_of(&v)?;
                    (times_like(&v, ms)?, dim)
                }
            };
            let mut names: Vec<String> = data.iter().enumerate().map(|(k, (i, _))| arg_names.get(*i).cloned().flatten().unwrap_or_else(|| format!("Var{}", k + 1))).collect();
            if let Some(v) = opt(&opts, "VariableNames") {
                names = names_of(v)?;
            }
            if names.len() != data.len() {
                return rt("The VariableNames property must contain one name for each variable in the table.");
            }
            let vars = data.into_iter().map(|(_, v)| v).collect();
            one(Value::Table(Box::new(make_timetable(names, vars, times, &dim)?)))
        }
        "array2timetable" => {
            let (data, opts) = split_opts(a, opt_names);
            let src = data.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
            let cols = columns_of(src)?;
            let names = match opt(&opts, "VariableNames") {
                Some(v) => names_of(v)?,
                None => (1..=cols.len()).map(|i| format!("Var{i}")).collect(),
            };
            let n = src.dims()[0];
            let times = generated_times(&opts, n)?.ok_or_else(|| LangError::new("Specify the row times with 'RowTimes', 'TimeStep' or 'SampleRate'.", 0))?;
            one(Value::Table(Box::new(make_timetable(names, cols, times, "Time")?)))
        }
        "table2timetable" => {
            let t = table_of(arg(a, 0, name)?, name)?;
            let (_, opts) = split_opts(&a[1..], opt_names);
            one(Value::Table(Box::new(table2timetable(t, &opts)?)))
        }
        "timetable2table" => {
            let t = timetable_of(arg(a, 0, name)?, name)?;
            let (_, opts) = split_opts(&a[1..], opt_names);
            if let Some(c) = opt(&opts, "ConvertRowTimes") {
                if c.scalar()? == 0.0 {
                    let mut out = t.clone();
                    out.row_times = None;
                    out.dim_names = ["Row".into(), "Variables".into()];
                    return one(Value::Table(Box::new(out)));
                }
            }
            one(Value::Table(Box::new(timetable2table(t)?)))
        }
        "istimetable" => one(Value::bool(matches!(a.first(), Some(Value::Table(t)) if t.is_timetable()))),
        "isregular" => {
            let t = timetable_of(arg(a, 0, name)?, name)?;
            let times = t.row_times.as_ref().unwrap();
            let step = regular_step(times);
            Ok(Some(vec![Value::bool(step.is_some()), step_value(times, step.unwrap_or(f64::NAN))]))
        }
        "timerange" => {
            let start = arg(a, 0, name)?.clone();
            let stop = arg(a, 1, name)?.clone();
            let ty = a.get(2).map(|v| v.text()).transpose()?.unwrap_or_else(|| "openright".into()).to_lowercase();
            if !["openright", "closedleft", "openleft", "closedright", "open", "closed"].contains(&ty.as_str()) {
                return rt(format!("'{ty}' is not a valid interval type."));
            }
            for v in [&start, &stop] {
                if !matches!(v, Value::Time(_) | Value::Num(_)) {
                    return rt("The start and end times must be datetime or duration values.");
                }
            }
            one(object("timerange", &["Start", "Stop", "Type"], vec![start, stop, Value::chars(&ty)]))
        }
        "withtol" => {
            let times = arg(a, 0, name)?.clone();
            let tol = arg(a, 1, name)?.clone();
            if !matches!(&tol, Value::Time(t) if t.kind == TimeKind::Duration) {
                return rt("The tolerance must be a duration.");
            }
            times_of(&times)?;
            one(object("withtol", &["Times", "Tol"], vec![times, tol]))
        }
        "vartype" => {
            let ty = arg(a, 0, name)?.text()?;
            one(object("vartype", &["Type"], vec![Value::chars(&ty)]))
        }
        "retime" => retime(it, a),
        "synchronize" => synchronize(it, a),
        "lag" => lag(a),
        "containsrange" | "overlapsrange" | "withinrange" => range_test(name, a),
        "readtimetable" => {
            let (data, opts) = split_opts(a, opt_names);
            let t = it.call_function("readtable", data, 1)?.remove(0);
            let t = table_of(&t, name)?;
            let tt = table2timetable(t, &opts).map_err(|_| {
                LangError::new("Expected a datetime or duration variable to use as RowTimes. Check that the data can be converted to datetime or duration, or supply a \"TimeStep\" or \"SampleRate\".", 0)
            })?;
            one(Value::Table(Box::new(tt)))
        }
        "writetimetable" => {
            let t = timetable_of(arg(a, 0, name)?, name)?;
            let mut args = vec![Value::Table(Box::new(timetable2table(t)?))];
            args.extend(a[1..].iter().cloned());
            it.call_function("writetable", args, 0)?;
            Ok(Some(Vec::new()))
        }
        _ => Ok(None),
    }
}
