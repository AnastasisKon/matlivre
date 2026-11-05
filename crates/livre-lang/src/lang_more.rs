//! Events and listeners (`addlistener`, `listener`, `notify`, `event.hasListener`, `events`,
//! with the built-in `event.EventData` and `event.listener` classes), enumeration support
//! (`enumeration`, comparing members, converting them to text), class queries
//! (`superclasses`, `meta.class.fromName`) and tolerant set functions (`ismembertol`,
//! `uniquetol`), plus `powermod`.
//!
//! Object arrays are not supported, so `enumeration` returns its members in a cell column.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::BinOp;
use crate::builtins::{arg, none, one, Ret};
use crate::class::{enum_name, ClassDef};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "addlistener", "listener", "notify", "event.hasListener", "events", "superclasses", "enumeration", "meta.class.fromName", "ismembertol", "uniquetol",
    "powermod",
];

/// A registered listener: the source object's storage, the event, the callback and the
/// listener object's storage (for `Enabled` and deletion).
pub struct Listener {
    source: Rc<RefCell<Vec<Value>>>,
    event: String,
    callback: Value,
    handle: Rc<RefCell<Vec<Value>>>,
}

/// The built-in `event.EventData` and `event.listener` classes (handle classes).
pub fn builtin_class(name: &str) -> Option<Rc<ClassDef>> {
    if name == crate::sysobj::CLASS {
        return Some(crate::sysobj::base_class());
    }
    if name == crate::tiff::CLASS {
        return Some(crate::tiff::class_def());
    }
    if name == crate::sysobj::FINITE {
        return Some(crate::sysobj::finite_class());
    }
    let props: &[&str] = match name {
        "event.EventData" => &["Source", "EventName"],
        "event.listener" => &["Source", "EventName", "Callback", "Enabled", "Recursive"],
        _ => return None,
    };
    thread_local! {
        static DEFS: RefCell<HashMap<String, Rc<ClassDef>>> = RefCell::new(HashMap::new());
    }
    DEFS.with(|d| {
        if let Some(c) = d.borrow().get(name) {
            return Some(c.clone());
        }
        let mut src = String::from("classdef MatlivreBuiltin < handle\n  properties\n");
        for p in props {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).ok()?;
        let mut def = ClassDef::build(ast, &[]);
        def.name = name.to_string();
        let def = Rc::new(def);
        d.borrow_mut().insert(name.to_string(), def.clone());
        Some(def)
    })
}

fn new_object(c: &Rc<ClassDef>, values: Vec<Value>) -> Value {
    Value::Object(Object { class: c.clone(), props: Rc::new(RefCell::new(values)) })
}

/// The class named by a text argument or of an object.
fn class_arg(it: &mut Interpreter, v: &Value, fname: &str) -> R<Rc<ClassDef>> {
    match v {
        Value::Object(o) => Ok(o.class.clone()),
        v if v.is_text() => {
            let n = v.text()?;
            it.find_class(&n).ok_or_else(|| LangError::new(format!("Unable to find class '{n}'."), 0))
        }
        _ => rt(format!("The input to '{fname}' must be a class name or an object.")),
    }
}

/// A class's events, with `ObjectBeingDestroyed` for handle classes.
pub(crate) fn event_list(c: &ClassDef) -> Vec<String> {
    event_names(c)
}

fn event_names(c: &ClassDef) -> Vec<String> {
    let mut e = c.events.clone();
    if c.is_handle && !e.iter().any(|x| x == "ObjectBeingDestroyed") {
        e.push("ObjectBeingDestroyed".into());
    }
    e
}

fn cell_col(items: &[String]) -> Value {
    Value::Cell(CellArray::col(items.iter().map(|s| Value::chars(s)).collect()))
}

fn add_listener(it: &mut Interpreter, name: &str, a: &[Value]) -> Ret {
    let Value::Object(src) = arg(a, 0, name)? else { return rt("Listeners can only be added to handle objects.") };
    if !src.class.is_handle {
        return rt(format!("Listeners can only be added to handle objects; '{}' is a value class.", src.class.name));
    }
    let event = arg(a, 1, name)?.text()?;
    let callback = arg(a, 2, name)?.clone();
    if !event_names(&src.class).contains(&event) {
        return rt(format!("Event '{event}' is not defined for class '{}'.", src.class.name));
    }
    let class = builtin_class("event.listener").unwrap();
    let source = Value::Cell(CellArray::row(vec![Value::Object(src.clone())]));
    let obj = new_object(&class, vec![source, Value::chars(&event), callback.clone(), Value::bool(true), Value::bool(false)]);
    let Value::Object(lo) = &obj else { unreachable!() };
    it.listeners.push(Listener { source: src.props.clone(), event, callback, handle: lo.props.clone() });
    one(obj)
}

fn notify(it: &mut Interpreter, a: &[Value]) -> Ret {
    let Value::Object(src) = arg(a, 0, "notify")? else { return rt("notify requires a handle object.") };
    let event = arg(a, 1, "notify")?.text()?;
    if !event_names(&src.class).contains(&event) {
        return rt(format!("Event '{event}' is not defined for class '{}'.", src.class.name));
    }
    // The event data: the given event.EventData (subclass) object, or a new one.
    let data = match a.get(2) {
        Some(Value::Object(d)) if d.class.is_a("event.EventData") => {
            let d = d.clone().set("Source", Value::Object(src.clone()))?.set("EventName", Value::chars(&event))?;
            Value::Object(d)
        }
        Some(_) => return rt("Event data must be an event.EventData object."),
        None => new_object(&builtin_class("event.EventData").unwrap(), vec![Value::Object(src.clone()), Value::chars(&event)]),
    };
    let targets: Vec<(Value, Rc<RefCell<Vec<Value>>>)> =
        it.listeners.iter().filter(|l| Rc::ptr_eq(&l.source, &src.props) && l.event == event).map(|l| (l.callback.clone(), l.handle.clone())).collect();
    for (cb, handle) in targets {
        // Enabled is the listener object's fourth property.
        let enabled = handle.borrow().get(3).map(|v| v.scalar().unwrap_or(1.0) != 0.0).unwrap_or(true);
        if enabled {
            it.call_func_value(&cb, vec![Value::Object(src.clone()), data.clone()], 0)?;
        }
    }
    none()
}

/// `delete(listener)`: unregisters it. Returns false for other values.
pub fn delete_listener(it: &mut Interpreter, v: &Value) -> bool {
    match v {
        Value::Object(o) if o.class.name == "event.listener" => {
            it.listeners.retain(|l| !Rc::ptr_eq(&l.handle, &o.props));
            true
        }
        _ => false,
    }
}

// ---- enumerations ----------------------------------------------------------------------------

/// `==` and `~=` with enumeration members (or member names as text).
pub fn enum_compare(op: BinOp, a: &Value, b: &Value) -> R<Option<Value>> {
    if !matches!(op, BinOp::Eq | BinOp::Ne) {
        return Ok(None);
    }
    let name = |v: &Value| -> Option<(Option<String>, String)> {
        match v {
            Value::Object(o) => enum_name(o).map(|n| (Some(o.class.name.clone()), n)),
            v if v.is_text() => v.text().ok().map(|t| (None, t)),
            _ => None,
        }
    };
    let (Some((ca, na)), Some((cb, nb))) = (name(a), name(b)) else { return Ok(None) };
    if ca.is_none() && cb.is_none() {
        return Ok(None);
    }
    if let (Some(x), Some(y)) = (&ca, &cb) {
        if x != y {
            return rt(format!("Cannot compare enumeration members of classes '{x}' and '{y}'."));
        }
    }
    let eq = na == nb;
    Ok(Some(Value::bool(if op == BinOp::Eq { eq } else { !eq })))
}

/// `char`, `string` and `cellstr` of an enumeration member: its name.
pub fn enum_text(name: &str, a: &[Value]) -> Option<Value> {
    let Some(Value::Object(o)) = a.first() else { return None };
    let n = enum_name(o)?;
    Some(match name {
        "char" => Value::chars(&n),
        "string" => Value::Str(StrArray::scalar(n)),
        _ => Value::Cell(CellArray::row(vec![Value::chars(&n)])),
    })
}

fn enumeration(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let c = class_arg(it, arg(a, 0, "enumeration")?, "enumeration")?;
    let names: Vec<String> = c.enums.iter().map(|(n, _)| n.clone()).collect();
    if nargout == 0 {
        if names.is_empty() {
            it.emit(OutputKind::Text(format!("No enumeration members for class '{}'.\n\n", c.name)));
        } else {
            let lines: String = names.iter().map(|n| format!("    {n}\n")).collect();
            it.emit(OutputKind::Text(format!("Enumeration members for class '{}':\n\n{lines}\n", c.name)));
        }
        return none();
    }
    let members: Vec<Value> = names.iter().map(|n| it.enum_member(&c, n)).collect::<R<_>>()?;
    Ok(Some(vec![Value::Cell(CellArray::col(members)), cell_col(&names)]))
}

// ---- class queries ---------------------------------------------------------------------------

fn superclasses(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let c = class_arg(it, arg(a, 0, "superclasses")?, "superclasses")?;
    let mut names = c.ancestors.clone();
    if c.is_handle && !names.iter().any(|n| n == "handle") {
        names.push("handle".into());
    }
    if nargout == 0 {
        if names.is_empty() {
            it.emit(OutputKind::Text(format!("No superclasses for class {}.\n\n", c.name)));
        } else {
            let lines: String = names.iter().map(|n| format!("    {n}\n")).collect();
            it.emit(OutputKind::Text(format!("Superclasses for class {}:\n\n{lines}\n", c.name)));
        }
        return none();
    }
    one(cell_col(&names))
}

fn events(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let c = class_arg(it, arg(a, 0, "events")?, "events")?;
    let names = event_names(&c);
    if nargout == 0 {
        let lines: String = names.iter().map(|n| format!("    {n}\n")).collect();
        it.emit(OutputKind::Text(format!("Events for class {}:\n\n{lines}\n", c.name)));
        return none();
    }
    one(cell_col(&names))
}

fn meta_class(it: &mut Interpreter, a: &[Value]) -> Ret {
    let name = arg(a, 0, "meta.class.fromName")?.text()?;
    match it.find_class(&name) {
        Some(c) => one(crate::builtins_ops::metaclass_for(it, Some(c), &name)),
        None => one(Value::empty()),
    }
}

// ---- tolerant set functions ------------------------------------------------------------------

struct TolOpts {
    tol: f64,
    by_rows: bool,
    scale: Option<Vec<f64>>,
}

fn tol_opts(a: &[Value], from: usize, single: bool) -> R<TolOpts> {
    let mut o = TolOpts { tol: if single { 1e-6 } else { 1e-12 }, by_rows: false, scale: None };
    let mut i = from;
    if let Some(v) = a.get(i).filter(|v| !v.is_text()) {
        o.tol = v.scalar()?;
        i += 1;
    }
    while i < a.len() {
        let k = a[i].text()?.to_lowercase();
        let v = a.get(i + 1).ok_or_else(|| LangError::new("Name-value arguments must come in pairs.", 0))?;
        match k.as_str() {
            "byrows" => o.by_rows = v.scalar()? != 0.0,
            "datascale" => o.scale = Some(v.to_matrix()?.data),
            "outputallindices" => {}
            other => return rt(format!("'{other}' is not a recognized parameter.")),
        }
        i += 2;
    }
    Ok(o)
}

/// Rows (or elements) of a matrix as vectors.
fn items(m: &Matrix, by_rows: bool) -> Vec<Vec<f64>> {
    if !by_rows {
        return m.data.iter().map(|&x| vec![x]).collect();
    }
    let (r, c) = (m.rows(), m.cols());
    (0..r).map(|i| (0..c).map(|j| m.data[j * r + i]).collect()).collect()
}

fn within(u: &[f64], v: &[f64], tol: f64, scale: &[f64]) -> bool {
    u.iter().zip(v).enumerate().all(|(k, (x, y))| (x - y).abs() <= tol * scale[k.min(scale.len() - 1)])
}

fn ismembertol(a: &[Value]) -> Ret {
    let am = arg(a, 0, "ismembertol")?.to_matrix()?;
    let bm = arg(a, 1, "ismembertol")?.to_matrix()?;
    let o = tol_opts(a, 2, am.kind == Kind::Single || bm.kind == Kind::Single)?;
    let (ai, bi) = (items(&am, o.by_rows), items(&bm, o.by_rows));
    let width = ai.first().or(bi.first()).map(|v| v.len()).unwrap_or(1);
    let scale = o.scale.unwrap_or_else(|| {
        if o.by_rows {
            (0..width).map(|k| ai.iter().chain(&bi).map(|v| v[k].abs()).fold(0.0, f64::max)).collect()
        } else {
            vec![am.data.iter().chain(&bm.data).map(|x| x.abs()).fold(0.0, f64::max)]
        }
    });
    let (mut tf, mut loc) = (Vec::new(), Vec::new());
    for u in &ai {
        match bi.iter().position(|v| within(u, v, o.tol, &scale)) {
            Some(k) => {
                tf.push(1.0);
                loc.push((k + 1) as f64);
            }
            None => {
                tf.push(0.0);
                loc.push(0.0);
            }
        }
    }
    let dims = if o.by_rows { vec![ai.len(), 1] } else { am.dims.clone() };
    Ok(Some(vec![Value::Num(Matrix::from_dims(dims.clone(), tf).cast_to(Kind::Logical)), Value::Num(Matrix::from_dims(dims, loc))]))
}

fn uniquetol(a: &[Value]) -> Ret {
    let am = arg(a, 0, "uniquetol")?.to_matrix()?;
    let o = tol_opts(a, 1, am.kind == Kind::Single)?;
    let ai = items(&am, o.by_rows);
    let width = ai.first().map(|v| v.len()).unwrap_or(1);
    let scale = o.scale.unwrap_or_else(|| (0..width).map(|k| ai.iter().map(|v| v[if o.by_rows { k } else { 0 }].abs()).fold(0.0, f64::max)).collect());
    let mut order: Vec<usize> = (0..ai.len()).collect();
    order.sort_by(|&x, &y| ai[x].iter().zip(&ai[y]).map(|(p, q)| p.total_cmp(q)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal).then(x.cmp(&y)));
    // Each element joins the group of the latest kept element within tolerance.
    let mut kept: Vec<usize> = Vec::new();
    let mut ic = vec![0.0; ai.len()];
    for &k in &order {
        match kept.last() {
            Some(&last) if within(&ai[k], &ai[last], o.tol, &scale) => ic[k] = kept.len() as f64,
            _ => {
                kept.push(k);
                ic[k] = kept.len() as f64;
            }
        }
    }
    let c = if o.by_rows {
        let n = kept.len();
        let mut d = vec![0.0; n * width];
        for (i, &k) in kept.iter().enumerate() {
            for j in 0..width {
                d[j * n + i] = ai[k][j];
            }
        }
        Matrix::new(n, width, d)
    } else {
        let v: Vec<f64> = kept.iter().map(|&k| ai[k][0]).collect();
        if am.rows() == 1 { Matrix::row(v) } else { Matrix::col(v) }
    };
    let ia = Matrix::col(kept.iter().map(|&k| (k + 1) as f64).collect());
    Ok(Some(vec![Value::Num(c.cast_to(am.kind)), Value::Num(ia), Value::Num(Matrix::col(ic))]))
}

fn powermod(a: &[Value]) -> Ret {
    let (b, e, m) = (arg(a, 0, "powermod")?.to_matrix()?, arg(a, 1, "powermod")?.to_matrix()?, arg(a, 2, "powermod")?.to_matrix()?);
    let n = b.numel().max(e.numel()).max(m.numel());
    let pick = |x: &Matrix, k: usize| x.data[if x.numel() == 1 { 0 } else { k }];
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let (base, exp, md) = (pick(&b, k), pick(&e, k), pick(&m, k));
        if [base, exp, md].iter().any(|x| x.fract() != 0.0) || exp < 0.0 || md <= 0.0 {
            return rt("Inputs must be integers, the exponent nonnegative and the modulus positive.");
        }
        let md = md as u128;
        let (mut r, mut x, mut p) = (1u128 % md, (base.rem_euclid(md as f64)) as u128, exp as u128);
        while p > 0 {
            if p & 1 == 1 {
                r = r * x % md;
            }
            x = x * x % md;
            p >>= 1;
        }
        out.push(r as f64);
    }
    let dims = if b.numel() == n { b.dims.clone() } else if e.numel() == n { e.dims.clone() } else { m.dims.clone() };
    one(Value::Num(Matrix::from_dims(dims, out)))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "addlistener" | "listener" => add_listener(it, name, a),
        "notify" => notify(it, a),
        "event.hasListener" => {
            let Value::Object(src) = arg(a, 0, name)? else { return one(Value::bool(false)) };
            let event = arg(a, 1, name)?.text()?;
            one(Value::bool(it.listeners.iter().any(|l| Rc::ptr_eq(&l.source, &src.props) && l.event == event)))
        }
        "events" => events(it, a, nargout),
        "superclasses" => superclasses(it, a, nargout),
        "enumeration" => enumeration(it, a, nargout),
        "meta.class.fromName" => meta_class(it, a),
        "ismembertol" => ismembertol(a),
        "uniquetol" => uniquetol(a),
        "powermod" => powermod(a),
        _ => Ok(None),
    }
}
