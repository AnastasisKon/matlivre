//! System objects: classes derived from `matlab.System`, run with `step` (or by calling the
//! object), `setup`, `reset`, `release`, `isLocked`, `isDone` and `clone`. The `*Impl`
//! methods a class defines receive only as many inputs as they declare.

use std::cell::RefCell;
use std::rc::Rc;

use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["step", "setup", "release", "isLocked", "isDone", "clone", "reset"];

pub const CLASS: &str = "matlab.System";
/// The mixin that gives a System object `isDone`.
pub const FINITE: &str = "matlab.system.mixin.FiniteSource";

/// `matlab.system.mixin.FiniteSource`: no state of its own.
pub fn finite_class() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| {
                let ast = crate::class::parse_classdef("classdef FiniteSource < handle
end
").expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = FINITE.into();
                Rc::new(def)
            })
            .clone()
    })
}
const LOCKED: &str = "MatlivreLocked";

/// The `matlab.System` base class.
pub fn base_class() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| {
                let src = format!("classdef System < handle\n  properties (Hidden)\n    {LOCKED} = false\n  end\nend\n");
                let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = CLASS.into();
                Rc::new(def)
            })
            .clone()
    })
}

pub fn is_system(v: &Value) -> bool {
    matches!(v, Value::Object(o) if o.class.is_a(CLASS))
}

fn object(v: &Value) -> R<&Object> {
    match v {
        Value::Object(o) if o.class.is_a(CLASS) => Ok(o),
        _ => rt("Expected a System object."),
    }
}

fn locked(o: &Object) -> bool {
    o.class.prop_index(LOCKED).and_then(|i| o.props.borrow()[i].is_true().ok()).unwrap_or(false)
}

fn set_locked(o: &Object, v: bool) {
    if let Some(i) = o.class.prop_index(LOCKED) {
        o.props.borrow_mut()[i] = Value::bool(v);
    }
}

/// Calls `obj.name(inputs...)` with the inputs the method declares; None when the class
/// does not define it.
fn call_impl(it: &mut Interpreter, obj: &Value, name: &str, inputs: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let Value::Object(o) = obj else { return Ok(None) };
    let Some(f) = o.class.method(name).cloned() else { return Ok(None) };
    let takes_all = f.params.last().is_some_and(|p| p == "varargin");
    let n = if takes_all { inputs.len() } else { f.params.len().saturating_sub(1).min(inputs.len()) };
    let mut args = vec![obj.clone()];
    args.extend(inputs[..n].iter().cloned());
    let class = o.class.clone();
    let nout = if f.outputs.is_empty() { 0 } else { nargout.max(1) };
    Ok(Some(it.call_method(&f, &class, args, nout)?))
}

fn do_setup(it: &mut Interpreter, obj: &Value, inputs: &[Value], with_reset: bool) -> R<()> {
    call_impl(it, obj, "setupImpl", inputs, 0)?;
    set_locked(object(obj)?, true);
    if with_reset {
        call_impl(it, obj, "resetImpl", &[], 0)?;
    }
    Ok(())
}

/// `step(obj, inputs...)`, also `obj(inputs...)`.
pub fn step(it: &mut Interpreter, obj: &Value, inputs: &[Value], nargout: usize) -> R<Vec<Value>> {
    let o = object(obj)?;
    if !locked(o) {
        do_setup(it, obj, inputs, true)?;
    }
    Ok(call_impl(it, obj, "stepImpl", inputs, nargout)?.unwrap_or_default())
}

fn release(it: &mut Interpreter, obj: &Value) -> R<()> {
    let o = object(obj)?;
    if locked(o) {
        call_impl(it, obj, "releaseImpl", &[], 0)?;
        set_locked(o, false);
    }
    Ok(())
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let Some(obj) = a.first() else { return Ok(None) };
    if !is_system(obj) {
        // reset of other objects (graphics) has nothing to restore here.
        return Ok(if name == "reset" { Some(Vec::new()) } else { None });
    }
    let o = object(obj)?;
    let class = o.class.name.clone();
    match name {
        "step" => Ok(Some(step(it, obj, &a[1..], nargout)?)),
        "setup" => {
            if locked(o) {
                it.emit(OutputKind::Warning(format!(
                    "Reported by {class}: The System object has reinitialized because the setup method was called on an initialized System object. This will affect performance."
                )));
                release(it, obj)?;
            }
            do_setup(it, obj, &a[1..], false)?;
            Ok(Some(Vec::new()))
        }
        "release" => {
            release(it, obj)?;
            Ok(Some(Vec::new()))
        }
        "reset" => {
            if locked(o) {
                call_impl(it, obj, "resetImpl", &[], 0)?;
            }
            Ok(Some(Vec::new()))
        }
        "isLocked" => Ok(Some(vec![Value::bool(locked(o))])),
        "isDone" => {
            if !o.class.is_a(FINITE) {
                return rt(format!("Undefined function 'isDone' for input arguments of type '{class}'."));
            }
            Ok(Some(call_impl(it, obj, "isDoneImpl", &[], 1)?.unwrap_or_else(|| vec![Value::bool(false)])))
        }
        "clone" => {
            let mut props = o.props.borrow().clone();
            // A locked object's private state is not copied without saveObjectImpl/loadObjectImpl.
            let private: Vec<usize> = (0..o.class.props.len()).filter(|&i| o.class.props[i].hidden && o.class.props[i].name != LOCKED).collect();
            if locked(o) && !private.is_empty() && (o.class.method("saveObjectImpl").is_none() || o.class.method("loadObjectImpl").is_none()) {
                it.emit(OutputKind::Warning(format!(
                    "The {class} System object has private or protected properties, but does not implement both the saveObjectImpl and loadObjectImpl methods. The save, load, and clone methods may not copy the object exactly when it is locked."
                )));
                for i in private {
                    props[i] = Value::empty();
                }
            }
            Ok(Some(vec![Value::Object(Object { class: o.class.clone(), props: Rc::new(RefCell::new(props)) })]))
        }
        _ => Ok(None),
    }
}
