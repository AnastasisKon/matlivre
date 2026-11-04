//! `timer` objects: `timer`, `start`, `startat`, `stop`, `wait`, `timerfind`, `timerfindall`.
//!
//! There is no event loop, so timers fire where MATLAB also lets them run while code is
//! executing: in `pause`, `wait` and `drawnow`. A running timer's callbacks are called with the
//! timer and an event struct (`Type`, `Data.time`), like MATLAB's.

use crate::builtins::{arg, one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;
use crate::display::NumFormat;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["timer", "timerfind", "timerfindall", "matlivre_timer_method"];

/// The timer methods (they are not functions on the path, as in MATLAB).
#[allow(dead_code)]
const TIMER_METHODS: &[&str] = &["start", "startat", "stop", "wait"];

/// The public properties, in MATLAB's order.
const PROPS: &[&str] = &[
    "AveragePeriod", "BusyMode", "ErrorFcn", "ExecutionMode", "InstantPeriod", "Name", "ObjectVisibility", "Period", "Running", "StartDelay",
    "StartFcn", "StopFcn", "Tag", "TasksExecuted", "TasksToExecute", "TimerFcn", "Type", "UserData",
];

const READ_ONLY: &[&str] = &["AveragePeriod", "InstantPeriod", "Running", "TasksExecuted", "Type"];

/// The live timers, in creation order, and when each running one fires next.
#[derive(Default)]
pub struct Timers {
    live: Vec<(u64, Object, Option<Instant>)>,
    next_id: u64,
    /// When the running timer last fired (for AveragePeriod / InstantPeriod).
    last: Vec<(u64, Instant, f64, u32)>,
}

fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let mut src = String::from("classdef timer < handle\n  properties\n");
        for p in PROPS {
            src += &format!("    {p}\n");
        }
        src += "  end\n  properties (Hidden)\n    Id\n    Members\n  end\n  methods\n";
        for m in ["start", "startat", "stop", "wait", "delete", "isvalid", "get", "set", "subsref", "numel", "length", "size", "isempty"] {
            src += &format!("    function varargout = {m}(obj, varargin)\n      varargout = matlivre_timer_method('{m}', max(nargout, 1), obj, varargin{{:}});\n    end\n");
        }
        src += "  end\nend\n";
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

fn idx(name: &str) -> usize {
    PROPS.iter().position(|p| *p == name).expect("timer property")
}

fn prop(o: &Object, name: &str) -> Value {
    o.props.borrow()[idx(name)].clone()
}

fn put(o: &Object, name: &str, v: Value) {
    o.props.borrow_mut()[idx(name)] = v;
}

fn text_prop(o: &Object, name: &str) -> String {
    prop(o, name).text().unwrap_or_default()
}

fn num_prop(o: &Object, name: &str) -> f64 {
    prop(o, name).scalar().unwrap_or(0.0)
}

fn id_of(o: &Object) -> u64 {
    o.props.borrow()[PROPS.len()].scalar().unwrap_or(0.0) as u64
}

/// The single timers a value stands for: itself, or the members of a timer array.
fn members(v: &Value) -> Vec<Object> {
    match v {
        Value::Object(o) if o.class.name == "timer" => match &o.props.borrow()[PROPS.len() + 1] {
            Value::Cell(c) if !c.data.is_empty() => c.data.iter().filter_map(|m| if let Value::Object(x) = m { Some(x.clone()) } else { None }).collect(),
            _ => vec![o.clone()],
        },
        _ => Vec::new(),
    }
}

fn alive(it: &Interpreter, o: &Object) -> bool {
    let id = id_of(o);
    it.timers.live.iter().any(|(i, _, _)| *i == id)
}

/// A value of timers: `[]` for none, the timer for one, a timer array for more.
fn timers_value(list: Vec<Object>) -> Value {
    match list.len() {
        0 => Value::empty(),
        1 => Value::Object(list.into_iter().next().unwrap()),
        _ => {
            let mut props: Vec<Value> = PROPS.iter().map(|_| Value::empty()).collect();
            props.push(Value::num(0.0));
            props.push(Value::Cell(CellArray::row(list.into_iter().map(Value::Object).collect())));
            Value::Object(Object { class: class_def(), props: Rc::new(RefCell::new(props)) })
        }
    }
}

/// The canonical spelling of a property name (case-insensitive, as in MATLAB).
fn prop_name(name: &str) -> R<&'static str> {
    PROPS
        .iter()
        .find(|p| p.eq_ignore_ascii_case(name))
        .copied()
        .ok_or_else(|| LangError::new(format!("The name '{name}' is not an accessible property for an instance of class 'timer'."), 0))
}

fn set_error(p: &str, why: &str) -> LangError {
    LangError::new(format!("Error setting property '{p}' of class 'timer'. {why}"), 0)
}

/// Checks a value assigned to timer property `p`, returning what is stored.
pub fn check_prop(it: &mut Interpreter, p: &str, v: Value) -> R<Value> {
    let p = prop_name(p)?;
    if READ_ONLY.contains(&p) {
        return Err(LangError::new(format!("Unable to set the '{p}' property of class ''timer'' because it is read-only."), 0));
    }
    let choice = |opts: &[&str]| -> R<Value> {
        let t = v.text().map_err(|_| set_error(p, "Value must be a character vector or string scalar."))?;
        match opts.iter().find(|o| o.eq_ignore_ascii_case(&t)) {
            Some(o) => Ok(Value::chars(o)),
            None => {
                let list = match opts.len() {
                    2 => format!("'{}' or '{}'", opts[0], opts[1]),
                    _ => {
                        let head: Vec<String> = opts[..opts.len() - 1].iter().map(|o| format!("'{o}'")).collect();
                        format!("{}, or '{}'", head.join(", "), opts[opts.len() - 1])
                    }
                };
                Err(set_error(p, &format!("'{t}' is invalid. Value must be {list}.")))
            }
        }
    };
    match p {
        "ExecutionMode" => choice(&["singleShot", "fixedSpacing", "fixedDelay", "fixedRate"]),
        "BusyMode" => choice(&["drop", "queue", "error"]),
        "ObjectVisibility" => choice(&["on", "off"]),
        "Period" => match v.scalar() {
            Ok(x) if x > 0.0 => {
                if (x * 1000.0).fract() != 0.0 {
                    it.emit(OutputKind::Warning("Period property is limited to 1 millisecond precision. Sub-millisecond precision will be ignored.".into()));
                }
                Ok(Value::num((x * 1000.0).round() / 1000.0))
            }
            _ => Err(set_error(p, "Value must be positive.")),
        },
        "StartDelay" => match v.scalar() {
            Ok(x) if x >= 0.0 => {
                if (x * 1000.0).fract() != 0.0 {
                    it.emit(OutputKind::Warning("StartDelay property is limited to 1 millisecond precision. Sub-millisecond precision will be ignored.".into()));
                }
                Ok(Value::num((x * 1000.0).round() / 1000.0))
            }
            _ => Err(set_error(p, "Value must be nonnegative.")),
        },
        "TasksToExecute" => match v.scalar() {
            Ok(x) if x > 0.0 => Ok(Value::num(x)),
            _ => Err(set_error(p, "Value must be a positive integer.")),
        },
        "Name" | "Tag" => {
            v.text().map_err(|_| set_error(p, "Value must be a character vector or string scalar."))?;
            Ok(Value::chars(&v.text()?))
        }
        _ => Ok(v),
    }
}

fn new_timer(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    if a.len() % 2 != 0 {
        return rt("Invalid parameter/value pair arguments.");
    }
    let n = it.timers.live.len() + 1;
    it.timers.next_id += 1;
    let id = it.timers.next_id;
    let mut props: Vec<Value> = PROPS
        .iter()
        .map(|p| match *p {
            "AveragePeriod" | "InstantPeriod" => Value::num(f64::NAN),
            "BusyMode" => Value::chars("drop"),
            "ExecutionMode" => Value::chars("singleShot"),
            "Name" => Value::chars(&format!("timer-{n}")),
            "ObjectVisibility" => Value::chars("on"),
            "Period" => Value::num(1.0),
            "Running" => Value::chars("off"),
            "StartDelay" | "TasksExecuted" => Value::num(0.0),
            "TasksToExecute" => Value::num(f64::INFINITY),
            "Type" => Value::chars("timer"),
            "UserData" => Value::empty(),
            _ => Value::chars(""),
        })
        .collect();
    props.push(Value::num(id as f64));
    props.push(Value::Cell(CellArray::from_dims(vec![0, 0], Vec::new())));
    let o = Object { class: class_def(), props: Rc::new(RefCell::new(props)) };
    for pair in a.chunks(2) {
        let p = prop_name(&pair[0].text()?)?;
        let v = check_prop(it, p, pair[1].clone())?;
        put(&o, p, v);
    }
    it.timers.live.push((id, o.clone(), None));
    Ok(Value::Object(o))
}

fn event(it: &mut Interpreter, ty: &str) -> R<Value> {
    let clock = it.call_function("clock", Vec::new(), 1)?.into_iter().next().unwrap_or_else(Value::empty);
    let mut data = StructArray::scalar();
    data.set("time", clock);
    let mut e = StructArray::scalar();
    e.set("Type", Value::chars(ty));
    e.set("Data", Value::Struct(data));
    Ok(Value::Struct(e))
}

/// Calls callback property `which` (`TimerFcn`, `StartFcn`, …) of `o`. Errors are reported
/// like MATLAB's; the result says whether the callback failed.
fn callback(it: &mut Interpreter, o: &Object, which: &str) -> bool {
    let cb = prop(o, which);
    let r: R<()> = (|| {
        match &cb {
            Value::Func(f) => {
                let e = event(it, which)?;
                it.call_value(f, vec![Value::Object(o.clone()), e], 0)?;
            }
            Value::Cell(c) if !c.data.is_empty() => {
                let Value::Func(f) = &c.data[0] else { return rt("The callback must be a function handle.") };
                let e = event(it, which)?;
                let mut args = vec![Value::Object(o.clone()), e];
                args.extend(c.data[1..].iter().cloned());
                it.call_value(f, args, 0)?;
            }
            v if v.is_text() => {
                let code = v.text()?;
                if !code.trim().is_empty() {
                    it.in_workspace("base", |it| it.eval_text(&code, 0))?;
                }
            }
            _ => {}
        }
        Ok(())
    })();
    match r {
        Ok(()) => false,
        Err(e) => {
            let name = text_prop(o, "Name");
            it.emit(OutputKind::Text(format!("Error while evaluating {which} for timer '{name}' \n\n{}\n", e.message)));
            true
        }
    }
}

fn set_next(it: &mut Interpreter, id: u64, next: Option<Instant>) {
    if let Some(e) = it.timers.live.iter_mut().find(|e| e.0 == id) {
        e.2 = next;
    }
}

fn start_one(it: &mut Interpreter, o: &Object, delay: Option<f64>) -> R<()> {
    if !alive(it, o) {
        return rt("Invalid timer object. This object has been deleted and should be removed from your workspace using CLEAR.");
    }
    if text_prop(o, "Running") == "on" {
        return rt("Cannot start timer because it is already running.");
    }
    let delay = delay.unwrap_or_else(|| num_prop(o, "StartDelay")).max(0.0);
    put(o, "Running", Value::chars("on"));
    put(o, "TasksExecuted", Value::num(0.0));
    let id = id_of(o);
    it.timers.last.retain(|l| l.0 != id);
    set_next(it, id, Some(Instant::now() + Duration::from_secs_f64(delay)));
    callback(it, o, "StartFcn");
    Ok(())
}

fn stop_one(it: &mut Interpreter, o: &Object) {
    if text_prop(o, "Running") != "on" {
        return;
    }
    put(o, "Running", Value::chars("off"));
    set_next(it, id_of(o), None);
    callback(it, o, "StopFcn");
}

/// Runs the callback of timer `id`, which is due, and schedules its next run.
fn fire(it: &mut Interpreter, id: u64) {
    let Some((_, o, Some(due))) = it.timers.live.iter().find(|e| e.0 == id).cloned() else { return };
    let now = Instant::now();
    let done = num_prop(&o, "TasksExecuted") + 1.0;
    put(&o, "TasksExecuted", Value::num(done));
    match it.timers.last.iter_mut().find(|l| l.0 == id) {
        Some(l) => {
            let p = now.duration_since(l.1).as_secs_f64();
            l.1 = now;
            l.2 += p;
            l.3 += 1;
            put(&o, "InstantPeriod", Value::num((p * 1000.0).round() / 1000.0));
            put(&o, "AveragePeriod", Value::num((l.2 / l.3 as f64 * 1000.0).round() / 1000.0));
        }
        None => it.timers.last.push((id, now, 0.0, 0)),
    }
    let mode = text_prop(&o, "ExecutionMode");
    let period = num_prop(&o, "Period").max(0.001);
    let last = mode == "singleShot" || done >= num_prop(&o, "TasksToExecute");
    let next = due + Duration::from_secs_f64(period);
    set_next(it, id, if last { None } else { Some(if mode == "fixedRate" { next.max(now) } else { now + Duration::from_secs_f64(period) }) });
    let failed = callback(it, &o, "TimerFcn");
    if failed {
        callback(it, &o, "ErrorFcn");
    }
    if !alive(it, &o) {
        return;
    }
    if last || failed {
        stop_one(it, &o);
    } else if mode != "fixedRate" {
        // fixedSpacing / fixedDelay count the period from the end of the callback.
        set_next(it, id, Some(Instant::now() + Duration::from_secs_f64(period)));
    }
}

/// Lets the running timers fire: until `deadline` (pause), until timer `until` stops (wait),
/// or only the ones already due (drawnow).
pub fn service(it: &mut Interpreter, deadline: Option<Instant>, until: Option<u64>) {
    loop {
        if let Some(u) = until {
            if !it.timers.live.iter().any(|e| e.0 == u && e.2.is_some()) {
                return;
            }
        }
        let next = it.timers.live.iter().filter_map(|e| e.2.map(|t| (t, e.0))).min();
        let now = Instant::now();
        match next {
            Some((t, id)) if deadline.is_none_or(|d| t <= d) && (t <= now || deadline.is_some() || until.is_some()) => {
                if t > now {
                    std::thread::sleep(t - now);
                }
                fire(it, id);
            }
            _ => {
                if let Some(d) = deadline {
                    if d > now {
                        std::thread::sleep(d - now);
                    }
                }
                return;
            }
        }
    }
}

/// Whether any timer is running (pause only waits when one is).
pub fn any_running(it: &Interpreter) -> bool {
    it.timers.live.iter().any(|e| e.2.is_some())
}

fn callback_text(v: &Value) -> String {
    match v {
        Value::Func(f) => crate::display::func_text(f),
        Value::Cell(c) => {
            let items: Vec<String> = c
                .data
                .iter()
                .map(|x| match x {
                    Value::Func(f) => format!("{{{}}}", crate::display::func_text(f)),
                    Value::Num(m) if m.kind != Kind::Char && m.is_scalar() => format!("{{[{}]}}", crate::display::fmt_short_scalar(m.data[0])),
                    other => format!("{{{}}}", crate::display::summary(other, NumFormat::Short)),
                })
                .collect();
            format!("{{{}}}", items.join(" "))
        }
        other => format!("'{}'", other.text().unwrap_or_default()),
    }
}

fn num_text(v: &Value) -> String {
    format!("{}", v.scalar().unwrap_or(0.0))
}

/// The display of a timer or timer array.
pub fn timer_text(o: &Object) -> Option<String> {
    if o.class.name != "timer" {
        return None;
    }
    let list = members(&Value::Object(o.clone()));
    if list.len() > 1 {
        let mut s = String::from("Timer Object Array\n\n   Index:  ExecutionMode:  Period:  TimerFcn:               Name:\n");
        for (i, m) in list.iter().enumerate() {
            s += &format!(
                "   {:<8}{:<16}{:<9}{:<24}{}\n",
                i + 1,
                text_prop(m, "ExecutionMode"),
                num_text(&prop(m, "Period")),
                callback_text(&prop(m, "TimerFcn")),
                text_prop(m, "Name")
            );
        }
        return Some(s.trim_end().to_string());
    }
    Some(format!(
        "   Timer Object: {}\n\n   Timer Settings\n      ExecutionMode: {}\n             Period: {}\n           BusyMode: {}\n            Running: {}\n\n   Callbacks\n           TimerFcn: {}\n           ErrorFcn: {}\n           StartFcn: {}\n            StopFcn: {}",
        text_prop(o, "Name"),
        text_prop(o, "ExecutionMode"),
        num_text(&prop(o, "Period")),
        text_prop(o, "BusyMode"),
        text_prop(o, "Running"),
        callback_text(&prop(o, "TimerFcn")),
        callback_text(&prop(o, "ErrorFcn")),
        callback_text(&prop(o, "StartFcn")),
        callback_text(&prop(o, "StopFcn")),
    ))
}

/// `timerfind(Name, Value, ...)`: the live timers whose properties match.
fn find(it: &mut Interpreter, a: &[Value], all: bool) -> R<Value> {
    let mut conds = Vec::new();
    for pair in a.chunks(2) {
        let p = prop_name(&pair[0].text()?)?;
        let Some(v) = pair.get(1) else { return rt("Invalid parameter/value pair arguments.") };
        conds.push((p, v.clone()));
    }
    let list: Vec<Object> = it
        .timers
        .live
        .iter()
        .map(|e| e.1.clone())
        .filter(|o| all || text_prop(o, "ObjectVisibility") == "on")
        .filter(|o| {
            conds.iter().all(|(p, v)| {
                let have = prop(o, p);
                match (have.is_text(), v.is_text()) {
                    (true, true) => have.text().ok() == v.text().ok(),
                    _ => crate::builtins::values_equal(&have, v),
                }
            })
        })
        .collect();
    Ok(timers_value(list))
}

fn method(it: &mut Interpreter, a: &[Value]) -> Ret {
    let m = arg(a, 0, "timer")?.text()?;
    let obj = arg(a, 2, "timer")?.clone();
    let rest = &a[3..];
    let list = members(&obj);
    let out = |v: Vec<Value>| one(Value::Cell(CellArray::row(v)));
    match m.as_str() {
        "start" => {
            for o in &list {
                start_one(it, o, None)?;
            }
            out(Vec::new())
        }
        "startat" => {
            let when = match rest.len() {
                0 => return rt("Not enough input arguments."),
                1 => it.call_function("datenum", vec![rest[0].clone()], 1)?,
                _ => it.call_function("datenum", rest.to_vec(), 1)?,
            };
            let when = when.first().map(|v| v.scalar()).transpose()?.unwrap_or(0.0);
            let now = it.call_function("now", Vec::new(), 1)?[0].scalar()?;
            let delay = ((when - now) * 86400.0).max(0.0);
            for o in &list {
                start_one(it, o, Some(delay))?;
            }
            out(Vec::new())
        }
        "stop" => {
            for o in &list {
                stop_one(it, o);
            }
            out(Vec::new())
        }
        "wait" => {
            for o in &list {
                service(it, None, Some(id_of(o)));
            }
            out(Vec::new())
        }
        "delete" => {
            for o in &list {
                if text_prop(o, "Running") == "on" {
                    stop_one(it, o);
                }
                let id = id_of(o);
                it.timers.live.retain(|e| e.0 != id);
            }
            out(Vec::new())
        }
        "isvalid" => {
            let flags: Vec<f64> = list.iter().map(|o| if alive(it, o) { 1.0 } else { 0.0 }).collect();
            let n = flags.len();
            let dims = vec![1, n];
            out(vec![Value::Num(Matrix::from_dims(dims, flags).cast_to(Kind::Logical))])
        }
        "numel" | "length" => out(vec![Value::num(list.len() as f64)]),
        "isempty" => out(vec![Value::bool(list.is_empty())]),
        "size" => {
            let n = list.len() as f64;
            let dims = [1.0, n];
            match rest.first() {
                Some(d) => out(vec![Value::num(if d.scalar()? == 2.0 { n } else { 1.0 })]),
                None => out(vec![Value::Num(Matrix::row(dims.to_vec()))]),
            }
        }
        "subsref" => {
            let Some(Value::Struct(s)) = rest.first() else { return rt("Invalid indexing.") };
            let subs = s.elems.first().and_then(|e| e.get(1).cloned());
            let Some(Value::Cell(c)) = subs else { return rt("Invalid indexing.") };
            let i = c.data.first().map(|x| x.scalar()).transpose()?.unwrap_or(1.0);
            if i < 1.0 || i as usize > list.len() || i.fract() != 0.0 {
                return rt(format!("Index exceeds the number of array elements. Index must not exceed {}.", list.len()));
            }
            out(vec![Value::Object(list[i as usize - 1].clone())])
        }
        "get" => {
            let get_one = |o: &Object, names: &Option<Vec<String>>| -> R<Value> {
                match names {
                    None => {
                        let mut s = StructArray::scalar();
                        for p in PROPS {
                            s.set(p, prop(o, p));
                        }
                        Ok(Value::Struct(s))
                    }
                    Some(ns) if ns.len() == 1 => Ok(prop(o, prop_name(&ns[0])?)),
                    Some(ns) => Ok(Value::Cell(CellArray::row(ns.iter().map(|n| prop_name(n).map(|p| prop(o, p))).collect::<R<_>>()?))),
                }
            };
            let names = match rest.first() {
                None => None,
                Some(Value::Cell(c)) => Some(c.data.iter().map(|x| x.text()).collect::<R<Vec<_>>>()?),
                Some(v) => Some(vec![v.text()?]),
            };
            if list.len() == 1 {
                return out(vec![get_one(&list[0], &names)?]);
            }
            let vals: Vec<Value> = list.iter().map(|o| get_one(o, &names)).collect::<R<_>>()?;
            out(vec![Value::Cell(CellArray::from_dims(vec![vals.len(), 1], vals))])
        }
        "set" => {
            if rest.len() % 2 != 0 {
                return rt("Invalid parameter/value pair arguments.");
            }
            for o in &list {
                for pair in rest.chunks(2) {
                    let p = prop_name(&pair[0].text()?)?;
                    let v = check_prop(it, p, pair[1].clone())?;
                    put(o, p, v);
                }
            }
            out(Vec::new())
        }
        _ => rt(format!("Unknown timer method '{m}'.")),
    }
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "timer" => one(new_timer(it, a)?),
        "timerfind" => one(find(it, a, false)?),
        "timerfindall" => one(find(it, a, true)?),
        "matlivre_timer_method" => method(it, a),
        _ => Ok(None),
    }
}
