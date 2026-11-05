//! `timeseries` and `tscollection`: data samples over a numeric time vector, with metadata
//! (`tsdata.timemetadata`, `tsdata.datametadata`), events (`tsdata.event`), resampling,
//! sample access and editing, statistics, arithmetic, event-relative extraction, absolute
//! times, `idealfilter` and conversion to a timetable.
//!
//! The classes are generated MATLAB source; their methods forward to the hidden builtin
//! `matlivre_ts_method`. Samples run along the first dimension when `IsTimeFirst` (column
//! data), otherwise along the last (a row vector becomes 1-by-1-by-n, as in MATLAB).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::{arg, one, Ret};
use crate::class::ClassDef;
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "timeseries", "tscollection", "tsdata.event", "matlivre_ts_method", "timeseries2timetable", "ts2timetable", "resample", "getdatasamples", "getsamples",
    "getsampleusingtime", "addsample", "delsample", "getinterpmethod", "setinterpmethod", "getdatasamplesize", "addevent", "delevent",
    "gettsafteratevent", "gettsafterevent", "gettsatevent", "gettsbeforeatevent", "gettsbeforeevent", "gettsbetweenevents", "setuniformtime",
    "getabstime", "setabstime", "getqualitydesc", "idealfilter", "gettimeseriesnames", "removets", "addts", "settimeseriesnames",
    "addsampletocollection", "delsamplefromcollection",
];

const TS_PROPS: &[&str] = &["Name", "Time", "TimeInfo", "Data", "DataInfo", "Events", "Quality", "QualityInfo", "IsTimeFirst", "TreatNaNasMissing"];
const TS_METHODS: &[&str] = &[
    "resample", "getdatasamples", "getsamples", "getsampleusingtime", "addsample", "delsample", "getinterpmethod", "setinterpmethod", "getdatasamplesize",
    "addevent", "delevent", "gettsafteratevent", "gettsafterevent", "gettsatevent", "gettsbeforeatevent", "gettsbeforeevent", "gettsbetweenevents",
    "setuniformtime", "getabstime", "setabstime", "getqualitydesc", "idealfilter", "timeseries2timetable", "plot", "mean", "median", "max", "min",
    "std", "var", "sum", "mode", "iqr", "plus", "minus", "times", "rdivide", "mtimes", "mrdivide", "uminus", "detrend",
];
const TIME_PROPS: &[&str] = &["Units", "Format", "StartDate", "Start", "End", "Increment", "Length", "UserData"];
const DATA_PROPS: &[&str] = &["Units", "Interpolation", "UserData"];
const EVENT_PROPS: &[&str] = &["EventData", "Name", "Time", "Units", "StartDate"];
const TC_METHODS: &[&str] = &["gettimeseriesnames", "removets", "addts", "settimeseriesnames", "addsampletocollection", "delsamplefromcollection"];

/// A generated class; `methods` forward to matlivre_ts_method, `dependent` adds a
/// computed Length.
fn class_def(name: &str, props: &[String], methods: &[&str], length: bool) -> Rc<ClassDef> {
    thread_local! {
        static DEFS: RefCell<HashMap<String, Rc<ClassDef>>> = RefCell::new(HashMap::new());
    }
    let key = format!("{name}|{}", props.join(","));
    DEFS.with(|d| {
        if let Some(def) = d.borrow().get(&key) {
            return def.clone();
        }
        let mut src = String::from("classdef MatlivreGenerated\n  properties\n");
        for p in props {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\n");
        if length {
            src.push_str("  properties (Dependent)\n    Length\n  end\n");
        }
        src.push_str("  methods\n");
        if length {
            src.push_str("    function v = get.Length(obj)\n      v = numel(obj.Time);\n    end\n");
        }
        for m in methods {
            src.push_str(&format!(
                "    function varargout = {m}(varargin)\n      varargout = matlivre_ts_method('{m}', max(nargout, 1), varargin{{:}});\n    end\n"
            ));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let mut def = ClassDef::build(ast, &[]);
        def.name = name.to_string();
        let def = Rc::new(def);
        d.borrow_mut().insert(key, def.clone());
        def
    })
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn object(class: Rc<ClassDef>, values: Vec<Value>) -> Value {
    Value::Object(Object { class, props: Rc::new(RefCell::new(values)) })
}

fn is_class(v: &Value, name: &str) -> bool {
    matches!(v, Value::Object(o) if o.class.name == name)
}

fn get(v: &Value, p: &str) -> Value {
    match v {
        Value::Object(o) => o.get(p).unwrap_or_else(Value::empty),
        _ => Value::empty(),
    }
}

fn text(v: &Value, p: &str) -> String {
    get(v, p).text().unwrap_or_default()
}

// ---- the timeseries value ------------------------------------------------------------------

/// A timeseries as plain data: samples as rows (each sample flattened), and the sample shape.
#[derive(Clone)]
struct Ts {
    name: String,
    time: Vec<f64>,
    rows: Vec<Vec<f64>>,
    /// Shape of one sample (MATLAB's getdatasamplesize).
    shape: Vec<usize>,
    time_first: bool,
    units: String,
    format: String,
    start_date: String,
    interp: String,
    data_units: String,
    events: Vec<Value>,
    quality: Value,
    quality_info: Value,
    nan_missing: bool,
}

impl Ts {
    fn of(v: &Value) -> R<Ts> {
        if !is_class(v, "timeseries") {
            return rt("Expected a timeseries object.");
        }
        let time = get(v, "Time").to_matrix()?.data;
        let data = get(v, "Data").to_matrix()?;
        let time_first = get(v, "IsTimeFirst").scalar().unwrap_or(1.0) != 0.0;
        let n = time.len();
        let (rows, shape) = split_samples(&data, n, time_first);
        let ti = get(v, "TimeInfo");
        let di = get(v, "DataInfo");
        let events = match get(v, "Events") {
            Value::Cell(c) => c.data,
            _ => Vec::new(),
        };
        Ok(Ts {
            name: text(v, "Name"),
            time,
            rows,
            shape,
            time_first,
            units: text(&ti, "Units"),
            format: text(&ti, "Format"),
            start_date: text(&ti, "StartDate"),
            interp: text(&di, "Interpolation"),
            data_units: text(&di, "Units"),
            events,
            quality: get(v, "Quality"),
            quality_info: get(v, "QualityInfo"),
            nan_missing: get(v, "TreatNaNasMissing").scalar().unwrap_or(1.0) != 0.0,
        })
    }

    fn new(name: &str, time: Vec<f64>, rows: Vec<Vec<f64>>, shape: Vec<usize>, time_first: bool) -> Ts {
        Ts {
            name: name.into(),
            time,
            rows,
            shape,
            time_first,
            units: "seconds".into(),
            format: String::new(),
            start_date: String::new(),
            interp: "linear".into(),
            data_units: String::new(),
            events: Vec::new(),
            quality: Value::empty(),
            quality_info: Value::empty(),
            nan_missing: true,
        }
    }

    fn with(&self, time: Vec<f64>, rows: Vec<Vec<f64>>) -> Ts {
        Ts { time, rows, ..self.clone() }
    }

    fn data(&self) -> Matrix {
        // No samples: an empty 0-by-0 array, as MATLAB returns.
        if self.rows.is_empty() {
            return Matrix::new(0, 0, Vec::new());
        }
        join_samples(&self.rows, &self.shape, self.time_first)
    }

    fn value(&self) -> Value {
        let n = self.time.len();
        let uniform = uniform_step(&self.time);
        let time_info = object(
            class_def("tsdata.timemetadata", &strs(TIME_PROPS), &[], false),
            vec![
                Value::chars(&self.units),
                Value::chars(&self.format),
                Value::chars(&self.start_date),
                Value::num(self.time.first().copied().unwrap_or(f64::NAN)),
                Value::num(self.time.last().copied().unwrap_or(f64::NAN)),
                Value::num(uniform.unwrap_or(f64::NAN)),
                Value::num(n as f64),
                Value::empty(),
            ],
        );
        let data_info = object(class_def("tsdata.datametadata", &strs(DATA_PROPS), &[], false), vec![Value::chars(&self.data_units), Value::chars(&self.interp), Value::empty()]);
        let quality_info = if matches!(&self.quality_info, Value::Object(_)) {
            self.quality_info.clone()
        } else {
            object(
                class_def("tsdata.qualmetadata", &strs(&["Code", "Description", "UserData"]), &[], false),
                vec![Value::empty(), Value::Cell(CellArray::from_dims(vec![0, 0], Vec::new())), Value::empty()],
            )
        };
        object(
            class_def("timeseries", &strs(TS_PROPS), TS_METHODS, true),
            vec![
                Value::chars(&self.name),
                Value::Num(Matrix::col(self.time.clone())),
                time_info,
                Value::Num(self.data()),
                data_info,
                Value::Cell(CellArray::row(self.events.clone())),
                self.quality.clone(),
                quality_info,
                Value::bool(self.time_first),
                Value::bool(self.nan_missing),
            ],
        )
    }

    /// Samples whose index is in `keep` (order kept).
    fn pick(&self, keep: &[usize]) -> Ts {
        let mut t = self.with(keep.iter().map(|&k| self.time[k]).collect(), keep.iter().map(|&k| self.rows[k].clone()).collect());
        if let Value::Num(q) = &self.quality {
            if q.numel() == self.time.len() {
                t.quality = Value::Num(Matrix::col(keep.iter().map(|&k| q.data[k]).collect()));
            }
        }
        t
    }

    fn width(&self) -> usize {
        self.shape.iter().product::<usize>().max(1)
    }
}

/// Samples of data with `n` time points: (rows, shape of one sample).
fn split_samples(data: &Matrix, n: usize, time_first: bool) -> (Vec<Vec<f64>>, Vec<usize>) {
    if n == 0 || data.numel() == 0 {
        return (Vec::new(), vec![1, 1]);
    }
    let w = data.numel() / n;
    if time_first {
        let rows = (0..n).map(|i| (0..w).map(|j| data.data[j * n + i]).collect()).collect();
        let mut shape: Vec<usize> = data.dims.clone();
        shape[0] = 1;
        (rows, shape)
    } else {
        let rows = (0..n).map(|i| data.data[i * w..(i + 1) * w].to_vec()).collect();
        let mut shape = data.dims[..data.dims.len().saturating_sub(1)].to_vec();
        while shape.len() < 2 {
            shape.push(1);
        }
        (rows, shape)
    }
}

fn join_samples(rows: &[Vec<f64>], shape: &[usize], time_first: bool) -> Matrix {
    let n = rows.len();
    let w = shape.iter().product::<usize>().max(1);
    if time_first {
        let mut d = vec![0.0; n * w];
        for (i, r) in rows.iter().enumerate() {
            for (j, &v) in r.iter().enumerate() {
                d[j * n + i] = v;
            }
        }
        let mut dims = shape.to_vec();
        dims[0] = n;
        return Matrix::from_dims(dims, d);
    }
    let d: Vec<f64> = rows.iter().flatten().copied().collect();
    let mut dims = shape.to_vec();
    dims.push(n);
    Matrix::from_dims(dims, d)
}

fn uniform_step(t: &[f64]) -> Option<f64> {
    if t.len() < 2 {
        return None;
    }
    let d = t[1] - t[0];
    t.windows(2).all(|w| ((w[1] - w[0]) - d).abs() <= 1e-10 * d.abs().max(1.0)).then_some(d)
}

/// Name-value pairs (case-insensitive names).
fn pairs(a: &[Value]) -> R<Vec<(String, Value)>> {
    if a.len() % 2 != 0 {
        return rt("Name-value arguments must come in pairs.");
    }
    a.chunks(2).map(|p| Ok((p[0].text()?.to_lowercase(), p[1].clone()))).collect()
}

fn opt<'a>(p: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    p.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

fn constructor(a: &[Value]) -> Ret {
    // timeseries(data[, time][, Name, Value]) or timeseries(name).
    let (pos, nv): (Vec<&Value>, &[Value]) = {
        let k = a.iter().position(|v| v.is_text()).unwrap_or(a.len());
        if k == 0 && a.len() == 1 {
            return one(Ts::new(&a[0].text()?, Vec::new(), Vec::new(), vec![1, 1], true).value());
        }
        (a[..k].iter().collect(), &a[k..])
    };
    let opts = pairs(nv)?;
    let data = match pos.first() {
        Some(v) => v.to_matrix()?,
        None => Matrix::new(0, 0, Vec::new()),
    };
    // Row vectors run along the last dimension; otherwise samples are rows.
    let (time_first, n) = if data.rows() == 1 && data.dims.len() == 2 && data.cols() > 1 && pos.get(1).is_some_and(|t| t.numel() == data.cols()) {
        (false, data.cols())
    } else {
        (true, data.rows())
    };
    let data = if time_first { data } else { Matrix::from_dims(vec![1, 1, n], data.data.clone()) };
    let time: Vec<f64> = match pos.get(1) {
        Some(t) => t.to_matrix()?.data,
        None => (0..n).map(|k| k as f64).collect(),
    };
    if time.len() != n {
        return rt("The size of the time vector must match the number of samples in the data.");
    }
    let name = opt(&opts, "name").map(|v| v.text()).transpose()?.unwrap_or_else(|| "unnamed".into());
    let (rows, shape) = split_samples(&data, n, time_first);
    let mut ts = Ts::new(&name, time, rows, shape, time_first);
    if let Some(q) = opt(&opts, "quality") {
        ts.quality = q.clone();
    }
    one(ts.value())
}

// ---- events ------------------------------------------------------------------------------------

fn event(name: &str, time: f64) -> Value {
    object(
        class_def("tsdata.event", &strs(EVENT_PROPS), &[], false),
        vec![Value::empty(), Value::chars(name), Value::num(time), Value::chars("seconds"), Value::chars("")],
    )
}

/// The time of the n-th (1-based) event named `e`, or of an event object.
fn event_time(ts: &Ts, e: &Value, n: usize) -> R<f64> {
    if is_class(e, "tsdata.event") {
        return get(e, "Time").scalar();
    }
    let name = e.text()?;
    let times: Vec<f64> = ts.events.iter().filter(|v| text(v, "Name") == name).filter_map(|v| get(v, "Time").scalar().ok()).collect();
    times.get(n.saturating_sub(1)).copied().ok_or_else(|| LangError::new(format!("Event '{name}' is not defined for this timeseries."), 0))
}

// ---- interpolation -----------------------------------------------------------------------------

fn resample_rows(ts: &Ts, q: &[f64], method: &str) -> Vec<Vec<f64>> {
    let w = ts.width();
    let n = ts.time.len();
    q.iter()
        .map(|&t| {
            if n == 0 || !(t >= ts.time[0] && t <= ts.time[n - 1]) {
                return vec![f64::NAN; w];
            }
            let k = (0..n - 1).find(|&k| t <= ts.time[k + 1]).unwrap_or(n.saturating_sub(2));
            if n == 1 || t == ts.time[k] {
                return ts.rows[k].clone();
            }
            if t == ts.time[k + 1] {
                return ts.rows[k + 1].clone();
            }
            if method == "zoh" {
                return ts.rows[k].clone();
            }
            let f = (t - ts.time[k]) / (ts.time[k + 1] - ts.time[k]);
            (0..w).map(|j| ts.rows[k][j] + f * (ts.rows[k + 1][j] - ts.rows[k][j])).collect()
        })
        .collect()
}

// ---- methods -----------------------------------------------------------------------------------

fn stats(it: &mut Interpreter, name: &str, ts: &Ts, a: &[Value]) -> Ret {
    // Over time (the samples as rows), skipping NaN samples (TreatNaNasMissing).
    let _ = a;
    let n = ts.rows.len();
    let w = ts.width();
    let mut d = vec![0.0; n * w];
    for (i, r) in ts.rows.iter().enumerate() {
        for (j, &v) in r.iter().enumerate() {
            d[j * n + i] = v;
        }
    }
    let mut args = vec![Value::Num(Matrix::new(n, w, d))];
    if matches!(name, "std" | "var" | "max" | "min") {
        args.push(Value::empty());
    }
    args.push(Value::num(1.0));
    if ts.nan_missing && matches!(name, "mean" | "median" | "max" | "min" | "std" | "var" | "sum") {
        args.push(Value::chars("omitnan"));
    }
    Ok(Some(it.call_function(name, args, 1)?))
}

fn arith(it: &mut Interpreter, op: &str, a: &[Value]) -> Ret {
    let (x, y) = (arg(a, 0, op)?, arg(a, 1, op)?);
    let template = if is_class(x, "timeseries") { Ts::of(x)? } else { Ts::of(y)? };
    let operand = |v: &Value| -> R<Value> { if is_class(v, "timeseries") { Ok(Value::Num(Ts::of(v)?.data())) } else { Ok(v.clone()) } };
    let fname = match op {
        "mtimes" => "times",
        "mrdivide" => "rdivide",
        o => o,
    };
    let r = it.call_function(fname, vec![operand(x)?, operand(y)?], 1)?.remove(0).to_matrix()?;
    let (rows, shape) = split_samples(&r, template.time.len(), template.time_first);
    let mut out = template.with(template.time.clone(), rows);
    out.shape = shape;
    out.name = "unnamed".into();
    one(out.value())
}

fn ts_method(it: &mut Interpreter, m: &str, a: &[Value], nargout: usize) -> Ret {
    let tsv = arg(a, 0, m)?;
    let ts = Ts::of(tsv)?;
    let rest = &a[1..];
    let out = |t: Ts| one(t.value());
    match m {
        "resample" => {
            let q = arg(rest, 0, m)?.to_matrix()?.data;
            let method = rest.get(1).map(|v| v.text()).transpose()?.unwrap_or_else(|| ts.interp.clone());
            let rows = resample_rows(&ts, &q, &method);
            out(ts.with(q, rows))
        }
        "getdatasamples" => {
            let idx: Vec<usize> = arg(rest, 0, m)?.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect();
            one(Value::Num(ts.pick(&idx).data()))
        }
        "getsamples" => {
            let idx: Vec<usize> = arg(rest, 0, m)?.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect();
            if idx.iter().any(|&k| k >= ts.time.len()) {
                return rt("Index exceeds the number of samples.");
            }
            out(ts.pick(&idx))
        }
        "getsampleusingtime" => {
            let t0 = arg(rest, 0, m)?.scalar()?;
            let t1 = rest.get(1).map(|v| v.scalar()).transpose()?;
            let keep: Vec<usize> = (0..ts.time.len()).filter(|&k| match t1 { Some(t1) => ts.time[k] >= t0 && ts.time[k] <= t1, None => ts.time[k] == t0 }).collect();
            out(ts.pick(&keep))
        }
        "addsample" => {
            let p = pairs(rest)?;
            let d = opt(&p, "data").ok_or_else(|| LangError::new("Specify 'Data' for the new samples.", 0))?.to_matrix()?;
            let t = opt(&p, "time").ok_or_else(|| LangError::new("Specify 'Time' for the new samples.", 0))?.to_matrix()?.data;
            let (new_rows, _) = split_samples(&d, t.len(), ts.time_first || d.rows() == t.len());
            let mut pts: Vec<(f64, Vec<f64>)> = ts.time.iter().copied().zip(ts.rows.iter().cloned()).collect();
            pts.extend(t.into_iter().zip(new_rows));
            pts.sort_by(|x, y| x.0.total_cmp(&y.0));
            let (time, rows): (Vec<f64>, Vec<Vec<f64>>) = pts.into_iter().unzip();
            out(ts.with(time, rows))
        }
        "delsample" => {
            let p = pairs(rest)?;
            let drop: Vec<usize> = if let Some(i) = opt(&p, "index") {
                i.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect()
            } else if let Some(v) = opt(&p, "value") {
                let tv = v.to_matrix()?.data;
                (0..ts.time.len()).filter(|&k| tv.contains(&ts.time[k])).collect()
            } else {
                return rt("Specify 'Index' or 'Value'.");
            };
            let keep: Vec<usize> = (0..ts.time.len()).filter(|k| !drop.contains(k)).collect();
            out(ts.pick(&keep))
        }
        "getinterpmethod" => one(Value::chars(&ts.interp)),
        "setinterpmethod" => {
            let method = arg(rest, 0, m)?.text()?.to_lowercase();
            if method != "linear" && method != "zoh" {
                return rt("The interpolation method must be 'linear' or 'zoh'.");
            }
            out(Ts { interp: method, ..ts })
        }
        "getdatasamplesize" => one(Value::Num(Matrix::row(ts.shape.iter().map(|&s| s as f64).collect()))),
        "addevent" => {
            let mut t = ts.clone();
            match rest.first() {
                Some(e) if is_class(e, "tsdata.event") => t.events.push(e.clone()),
                Some(e) => t.events.push(event(&e.text()?, arg(rest, 1, m)?.scalar()?)),
                None => return rt("Not enough input arguments."),
            }
            out(t)
        }
        "delevent" => {
            let name = arg(rest, 0, m)?.text()?;
            let mut t = ts.clone();
            t.events.retain(|e| text(e, "Name") != name);
            out(t)
        }
        "gettsafteratevent" | "gettsafterevent" | "gettsatevent" | "gettsbeforeatevent" | "gettsbeforeevent" => {
            let n = rest.get(1).map(|v| v.scalar()).transpose()?.unwrap_or(1.0) as usize;
            let te = event_time(&ts, arg(rest, 0, m)?, n)?;
            let keep: Vec<usize> = (0..ts.time.len())
                .filter(|&k| {
                    let t = ts.time[k];
                    match m {
                        "gettsafteratevent" => t >= te,
                        "gettsafterevent" => t > te,
                        "gettsatevent" => t == te,
                        "gettsbeforeatevent" => t <= te,
                        _ => t < te,
                    }
                })
                .collect();
            out(ts.pick(&keep))
        }
        "gettsbetweenevents" => {
            let (n1, n2) = (rest.get(2).map(|v| v.scalar()).transpose()?.unwrap_or(1.0) as usize, rest.get(3).map(|v| v.scalar()).transpose()?.unwrap_or(1.0) as usize);
            let (t1, t2) = (event_time(&ts, arg(rest, 0, m)?, n1)?, event_time(&ts, arg(rest, 1, m)?, n2)?);
            let keep: Vec<usize> = (0..ts.time.len()).filter(|&k| ts.time[k] > t1 && ts.time[k] < t2).collect();
            out(ts.pick(&keep))
        }
        "setuniformtime" => {
            let p = pairs(rest)?;
            let n = ts.time.len();
            let start = opt(&p, "starttime").map(|v| v.scalar()).transpose()?;
            let end = opt(&p, "endtime").map(|v| v.scalar()).transpose()?;
            let step = opt(&p, "interval").map(|v| v.scalar()).transpose()?;
            let (s, d) = match (start, end, step) {
                (Some(s), _, Some(d)) => (s, d),
                (Some(s), Some(e), None) => (s, if n > 1 { (e - s) / (n - 1) as f64 } else { 0.0 }),
                (None, Some(e), Some(d)) => (e - d * (n.saturating_sub(1)) as f64, d),
                (None, None, Some(d)) => (ts.time.first().copied().unwrap_or(0.0), d),
                _ => return rt("Specify two of 'StartTime', 'EndTime' and 'Interval'."),
            };
            let time = (0..n).map(|k| s + d * k as f64).collect();
            out(ts.with(time, ts.rows.clone()))
        }
        "getabstime" => {
            if ts.start_date.is_empty() {
                return rt("The time vector is relative; set TimeInfo.StartDate to get absolute times.");
            }
            let base = it.call_function("datenum", vec![Value::chars(&ts.start_date)], 1)?.remove(0).scalar()?;
            let per_day = unit_seconds(&ts.units) / 86400.0;
            let nums = Value::Num(Matrix::col(ts.time.iter().map(|&t| base + t * per_day).collect()));
            let fmt = if ts.format.is_empty() { "dd-mmm-yyyy HH:MM:SS".to_string() } else { ts.format.clone() };
            let s = it.call_function("datestr", vec![nums, Value::chars(&fmt)], 1)?.remove(0);
            one(it.call_function("cellstr", vec![s], 1)?.remove(0))
        }
        "setabstime" => {
            let times = arg(rest, 0, m)?;
            let nums = it.call_function("datenum", vec![times.clone()], 1)?.remove(0).to_matrix()?.data;
            if nums.len() != ts.time.len() {
                return rt("Length of time and data do not match");
            }
            let first = nums[0];
            let per_day = unit_seconds(&ts.units) / 86400.0;
            // Date numbers resolve about 1e-5 s, so offsets are rounded to 0.1 ms.
            let time = nums.iter().map(|&d| ((d - first) * 86400.0 * 1e4).round() / 1e4 / (per_day * 86400.0)).collect();
            let start = it.call_function("datestr", vec![Value::num(first), Value::chars("dd-mmm-yyyy HH:MM:SS")], 1)?.remove(0).text()?;
            let mut t = ts.with(time, ts.rows.clone());
            t.start_date = start;
            out(t)
        }
        "getqualitydesc" => {
            let codes = ts.quality.to_matrix().map(|q| q.data).unwrap_or_default();
            if codes.is_empty() {
                return one(Value::Cell(CellArray::from_dims(vec![0, 0], Vec::new())));
            }
            let (known, desc): (Vec<f64>, Vec<String>) = match &ts.quality_info {
                Value::Object(_) => (
                    get(&ts.quality_info, "Code").to_matrix().map(|m| m.data).unwrap_or_default(),
                    match get(&ts.quality_info, "Description") {
                        Value::Cell(c) => c.data.iter().map(|v| v.text().unwrap_or_default()).collect(),
                        _ => Vec::new(),
                    },
                ),
                _ => (Vec::new(), Vec::new()),
            };
            let out: Vec<Value> = codes.iter().map(|c| Value::chars(&known.iter().position(|k| k == c).and_then(|i| desc.get(i).cloned()).unwrap_or_default())).collect();
            one(Value::Cell(CellArray::col(out)))
        }
        "idealfilter" => idealfilter(it, &ts, rest),
        "timeseries2timetable" => to_timetable(it, &[ts]),
        "plot" => {
            let mut args = vec![Value::Num(Matrix::col(ts.time.clone())), Value::Num(ts.data())];
            args.extend(rest.iter().cloned());
            let h = it.call_function("plot", args, 1)?;
            it.call_function("title", vec![Value::chars(&format!("Time Series Plot:{}", ts.name))], 0)?;
            it.call_function("xlabel", vec![Value::chars(&format!("Time ({})", ts.units))], 0)?;
            it.call_function("ylabel", vec![Value::chars(&ts.name)], 0)?;
            Ok(Some(if nargout > 0 { h } else { Vec::new() }))
        }
        "mean" | "median" | "max" | "min" | "std" | "var" | "sum" | "mode" | "iqr" => stats(it, m, &ts, rest),
        "uminus" => {
            let rows = ts.rows.iter().map(|r| r.iter().map(|v| -v).collect()).collect();
            out(ts.with(ts.time.clone(), rows))
        }
        "detrend" => {
            let kind = rest.first().map(|v| v.text()).transpose()?.unwrap_or_else(|| "constant".into());
            let d = Value::Num(ts.data());
            let r = it.call_function("detrend", vec![d, Value::chars(&kind)], 1)?.remove(0).to_matrix()?;
            let (rows, _) = split_samples(&r, ts.time.len(), ts.time_first);
            out(ts.with(ts.time.clone(), rows))
        }
        _ => rt(format!("Undefined function '{m}' for input arguments of type 'timeseries'.")),
    }
}

fn unit_seconds(u: &str) -> f64 {
    match u.to_lowercase().as_str() {
        "weeks" => 604800.0,
        "days" => 86400.0,
        "hours" => 3600.0,
        "minutes" => 60.0,
        "milliseconds" => 1e-3,
        "microseconds" => 1e-6,
        "nanoseconds" => 1e-9,
        _ => 1.0,
    }
}

/// `idealfilter(ts, [lo hi], 'pass'|'notch')`: the mean is removed and frequencies (cycles per
/// time unit) inside each interval kept or removed through the FFT.
fn idealfilter(it: &mut Interpreter, ts: &Ts, a: &[Value]) -> Ret {
    let iv = arg(a, 0, "idealfilter")?.to_matrix()?;
    let kind = arg(a, 1, "idealfilter")?.text()?.to_lowercase();
    let dt = uniform_step(&ts.time).ok_or_else(|| LangError::new("idealfilter requires uniformly sampled data.", 0))?;
    let n = ts.time.len();
    let bands: Vec<(f64, f64)> = {
        let r = iv.rows();
        (0..r).map(|i| (iv.data[i], iv.data[r + i])).collect()
    };
    let in_band = |f: f64| bands.iter().any(|&(lo, hi)| f >= lo && f <= hi);
    let w = ts.width();
    let mut rows = vec![vec![0.0; w]; n];
    for j in 0..w {
        let col: Vec<f64> = ts.rows.iter().map(|r| r[j]).collect();
        let mean = col.iter().sum::<f64>() / n as f64;
        let x = Value::Num(Matrix::col(col.iter().map(|v| v - mean).collect()));
        let spec = it.call_function("fft", vec![x], 1)?.remove(0).to_matrix()?;
        let mut re = spec.data.clone();
        let mut im = spec.im.clone().unwrap_or_else(|| vec![0.0; n]);
        for k in 0..n {
            let f = k.min(n - k) as f64 / (n as f64 * dt);
            let keep = if kind == "pass" { in_band(f) } else { !in_band(f) };
            if !keep {
                re[k] = 0.0;
                im[k] = 0.0;
            }
        }
        let mut m = Matrix::col(re);
        m.im = Some(im);
        let back = it.call_function("ifft", vec![Value::Num(m), Value::chars("symmetric")], 1)?.remove(0).to_matrix()?;
        // Rounding residue of the transforms is cleared.
        let scale = back.data.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        for i in 0..n {
            let v = back.data[i];
            rows[i][j] = if v.abs() <= 1e-12 * scale.max(1.0) { 0.0 } else { v };
        }
    }
    one(ts.with(ts.time.clone(), rows).value())
}

fn to_timetable(it: &mut Interpreter, list: &[Ts]) -> Ret {
    let first = list.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
    let secs = it.call_function("seconds", vec![Value::Num(Matrix::col(first.time.iter().map(|t| t * unit_seconds(&first.units)).collect()))], 1)?.remove(0);
    let mut args = vec![Value::chars("RowTimes"), secs];
    let mut data = Vec::new();
    let mut names = Vec::new();
    for t in list {
        if t.time != first.time {
            return rt("All timeseries must have the same time vector.");
        }
        data.push(Value::Num(join_samples(&t.rows, &t.shape, true)));
        names.push(Value::chars(if t.name == "unnamed" || t.name.is_empty() { "Data" } else { &t.name }));
    }
    args.push(Value::chars("VariableNames"));
    args.push(Value::Cell(CellArray::row(names)));
    let mut full = data;
    full.extend(args);
    Ok(Some(it.call_function("timetable", full, 1)?))
}

// ---- tscollection ------------------------------------------------------------------------------

fn collection(name: &str, members: Vec<(String, Value)>) -> R<Value> {
    let (time, units) = match members.first() {
        Some((_, ts)) => {
            let t = Ts::of(ts)?;
            (t.time.clone(), t.units.clone())
        }
        None => (Vec::new(), "seconds".to_string()),
    };
    let mut props = strs(&["Name", "Time", "TimeInfo"]);
    props.extend(members.iter().map(|(n, _)| n.clone()));
    let info = Ts { units: units.clone(), ..Ts::new("", time.clone(), Vec::new(), vec![1, 1], true) }.value();
    let mut values = vec![Value::chars(name), Value::Num(Matrix::col(time)), get(&info, "TimeInfo")];
    values.extend(members.into_iter().map(|(_, v)| v));
    Ok(object(class_def("tscollection", &props, TC_METHODS, true), values))
}

fn members(v: &Value) -> R<(String, Vec<(String, Value)>)> {
    let Value::Object(o) = v else { return rt("Expected a tscollection object.") };
    if o.class.name != "tscollection" {
        return rt("Expected a tscollection object.");
    }
    let names: Vec<String> = o.class.props.iter().map(|p| p.name.clone()).filter(|n| !["Name", "Time", "TimeInfo", "Length"].contains(&n.as_str())).collect();
    Ok((text(v, "Name"), names.into_iter().map(|n| (n.clone(), o.get(&n).unwrap_or_else(Value::empty))).collect()))
}

fn new_collection(a: &[Value]) -> Ret {
    let k = a.iter().position(|v| v.is_text()).unwrap_or(a.len());
    let opts = pairs(&a[k..])?;
    let name = opt(&opts, "name").map(|v| v.text()).transpose()?.unwrap_or_else(|| "unnamed".into());
    let list: Vec<Value> = match a.first() {
        Some(Value::Cell(c)) => c.data.clone(),
        Some(v) if is_class(v, "timeseries") => a[..k].to_vec(),
        _ => Vec::new(),
    };
    let mut ms = Vec::new();
    for v in list {
        let t = Ts::of(&v)?;
        if let Some((_, f)) = ms.first() {
            if Ts::of(f)?.time != t.time {
                return rt("The time vectors of all members must be the same.");
            }
        }
        ms.push((t.name.clone(), v));
    }
    one(collection(&name, ms)?)
}

fn tc_method(m: &str, a: &[Value]) -> Ret {
    let (name, mut ms) = members(arg(a, 0, m)?)?;
    let rest = &a[1..];
    let names = |ms: &[(String, Value)]| Value::Cell(CellArray::row(ms.iter().map(|(n, _)| Value::chars(n)).collect()));
    match m {
        "gettimeseriesnames" => one(names(&ms)),
        "removets" => {
            let drop: Vec<String> = match arg(rest, 0, m)? {
                Value::Cell(c) => c.data.iter().map(|v| v.text()).collect::<R<_>>()?,
                v => vec![v.text()?],
            };
            ms.retain(|(n, _)| !drop.contains(n));
            one(collection(&name, ms)?)
        }
        "addts" => {
            let v = arg(rest, 0, m)?;
            let mut t = Ts::of(v)?;
            if let Some(n) = rest.get(1) {
                t.name = n.text()?;
            }
            ms.push((t.name.clone(), t.value()));
            one(collection(&name, ms)?)
        }
        "settimeseriesnames" => {
            let (old, new) = (arg(rest, 0, m)?.text()?, arg(rest, 1, m)?.text()?);
            for (n, v) in ms.iter_mut() {
                if *n == old {
                    *n = new.clone();
                    let mut t = Ts::of(v)?;
                    t.name = new.clone();
                    *v = t.value();
                }
            }
            one(collection(&name, ms)?)
        }
        "addsampletocollection" => {
            let p: Vec<(String, Value)> = rest.chunks(2).map(|c| Ok((c[0].text()?, c[1].clone()))).collect::<R<_>>()?;
            let t = p.iter().find(|(k, _)| k.eq_ignore_ascii_case("time")).map(|(_, v)| v.to_matrix()).transpose()?.ok_or_else(|| LangError::new("Specify 'Time'.", 0))?.data;
            for (n, v) in ms.iter_mut() {
                let ts = Ts::of(v)?;
                let d = match p.iter().find(|(k, _)| k == n) {
                    Some((_, d)) => d.to_matrix()?,
                    None => Matrix::from_dims(vec![t.len(), ts.width()], vec![f64::NAN; t.len() * ts.width()]),
                };
                let (new_rows, _) = split_samples(&d, t.len(), true);
                let mut pts: Vec<(f64, Vec<f64>)> = ts.time.iter().copied().zip(ts.rows.iter().cloned()).collect();
                pts.extend(t.iter().copied().zip(new_rows));
                pts.sort_by(|x, y| x.0.total_cmp(&y.0));
                let (time, rows): (Vec<f64>, Vec<Vec<f64>>) = pts.into_iter().unzip();
                *v = ts.with(time, rows).value();
            }
            one(collection(&name, ms)?)
        }
        "delsamplefromcollection" => {
            let p = pairs(rest)?;
            for (_, v) in ms.iter_mut() {
                let ts = Ts::of(v)?;
                let drop: Vec<usize> = if let Some(i) = opt(&p, "index") {
                    i.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect()
                } else {
                    let tv = opt(&p, "value").ok_or_else(|| LangError::new("Specify 'Index' or 'Value'.", 0))?.to_matrix()?.data;
                    (0..ts.time.len()).filter(|&k| tv.contains(&ts.time[k])).collect()
                };
                let keep: Vec<usize> = (0..ts.time.len()).filter(|k| !drop.contains(k)).collect();
                *v = ts.pick(&keep).value();
            }
            one(collection(&name, ms)?)
        }
        _ => rt(format!("Undefined function '{m}' for input arguments of type 'tscollection'.")),
    }
}

// ---- display -------------------------------------------------------------------------------

fn units_text(v: f64, units: &str) -> String {
    format!("{} {units}", crate::display::fmt_g(v, 5))
}

/// Displays of timeseries, tscollection and the tsdata metadata and event objects.
pub fn display(_name: &str, v: &Value) -> Option<String> {
    let Value::Object(o) = v else { return None };
    let size_of = |p: &str| {
        let x = o.get(p).unwrap_or_else(Value::empty);
        format!("[{} {}]", crate::nd::size_text(&x.dims()).replace('×', "x"), x.class_name())
    };
    match o.class.name.as_str() {
        "timeseries" => Some(format!(
            "  timeseries\n\n  Common Properties:\n            Name: '{}'\n            Time: {}\n        TimeInfo: tsdata.timemetadata\n            Data: {}\n        DataInfo: tsdata.datametadata\n",
            text(v, "Name"),
            size_of("Time"),
            size_of("Data")
        )),
        "tscollection" => {
            let (cname, ms) = members(v).ok()?;
            let t = get(v, "Time").to_matrix().ok()?.data;
            let units = text(&get(v, "TimeInfo"), "Units");
            let (s, e) = (t.first().copied().unwrap_or(f64::NAN), t.last().copied().unwrap_or(f64::NAN));
            let list: String = ms.iter().map(|(n, _)| format!("      {n}\n")).collect();
            Some(format!(
                "Time Series Collection Object: {cname}\n\nTime vector characteristics\n\n      Start time            {}\n      End time              {}\n\nMember Time Series Objects:\n\n{list}\n",
                units_text(s, &units),
                units_text(e, &units)
            ))
        }
        "tsdata.timemetadata" => {
            let units = text(v, "Units");
            let inc = get(v, "Increment").scalar().unwrap_or(f64::NAN);
            let uniform = if inc.is_nan() {
                "  Non-Uniform Time:\n".to_string() + &format!("    Length       {}\n", get(v, "Length").scalar().unwrap_or(0.0))
            } else {
                format!("  Uniform Time:\n    Length       {}\n    Increment    {}\n", get(v, "Length").scalar().unwrap_or(0.0), units_text(inc, &units))
            };
            Some(format!(
                "  tsdata.timemetadata\n  Package: tsdata\n\n{uniform}\n  Time Range:\n    Start        {}\n    End          {}\n\n  Common Properties:\n          Units: '{units}'\n         Format: '{}'\n      StartDate: '{}'\n",
                units_text(get(v, "Start").scalar().unwrap_or(f64::NAN), &units),
                units_text(get(v, "End").scalar().unwrap_or(f64::NAN), &units),
                text(v, "Format"),
                text(v, "StartDate")
            ))
        }
        "tsdata.datametadata" => Some(format!(
            "  tsdata.datametadata\n  Package: tsdata\n\n  Common Properties:\n               Units: '{}'\n       Interpolation: {} (tsdata.interpolation)\n",
            text(v, "Units"),
            text(v, "Interpolation")
        )),
        "tsdata.event" => {
            let ed = o.get("EventData").unwrap_or_else(Value::empty);
            let ed_text = crate::display::summary(&ed, crate::display::NumFormat::Short);
            Some(format!(
                "    EventData: {ed_text}\n         Name: '{}'\n         Time: {}\n        Units: '{}'\n    StartDate: '{}'\n",
                text(v, "Name"),
                crate::display::fmt_g(get(v, "Time").scalar().unwrap_or(f64::NAN), 5),
                text(v, "Units"),
                text(v, "StartDate")
            ))
        }
        _ => None,
    }
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "timeseries" => constructor(a),
        "tscollection" => new_collection(a),
        "tsdata.event" => one(event(&arg(a, 0, name)?.text()?, arg(a, 1, name)?.scalar()?)),
        "timeseries2timetable" | "ts2timetable" => {
            let list: Vec<Ts> = a.iter().map(Ts::of).collect::<R<_>>()?;
            to_timetable(it, &list)
        }
        "matlivre_ts_method" => {
            let m = arg(a, 0, name)?.text()?;
            let n = arg(a, 1, name)?.scalar()? as usize;
            let rest = &a[2..];
            let first = rest.iter().find(|v| matches!(v, Value::Object(_))).cloned().unwrap_or_else(Value::empty);
            let r = if is_class(&first, "tscollection") {
                tc_method(&m, rest)?
            } else if matches!(m.as_str(), "plus" | "minus" | "times" | "rdivide" | "mtimes" | "mrdivide") {
                arith(it, &m, rest)?
            } else {
                ts_method(it, &m, rest, n)?
            };
            one(Value::Cell(CellArray::row(r.unwrap_or_default())))
        }
        // The function forms reach here only for timeseries and collection arguments.
        _ => {
            let Some(first) = a.first() else { return Ok(None) };
            if is_class(first, "tscollection") {
                return tc_method(name, a);
            }
            if is_class(first, "timeseries") {
                return ts_method(it, name, a, nargout);
            }
            Ok(None)
        }
    }
}
