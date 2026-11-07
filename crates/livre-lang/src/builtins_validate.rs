//! Argument validation and error state: the `mustBe*` validators, `inputParser`,
//! `lasterr`/`lastwarn`/`lasterror`, `rethrow`, `matlab.lang.makeValidName` and
//! `makeUniqueStrings`, `namedargs2cell`, `onCleanup`, `validatecolor` and `mlock`.
//!
//! Validator messages and identifiers (`MATLAB:validators:mustBeX`) follow MATLAB R2023b.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::{arg, num2str_scalar, one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, rt_id, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "mustBeA", "mustBeFile", "mustBeFinite", "mustBeFloat", "mustBeFolder", "mustBeGreaterThan", "mustBeGreaterThanOrEqual", "mustBeInRange",
    "mustBeInteger", "mustBeLessThan", "mustBeLessThanOrEqual", "mustBeMember", "mustBeNegative", "mustBeNonempty", "mustBeNonmissing", "mustBeNonNan",
    "mustBeNonnegative", "mustBeNonpositive", "mustBeNonsparse", "mustBeNonzero", "mustBeNonzeroLengthText", "mustBeNumeric", "mustBeNumericOrLogical",
    "mustBePositive", "mustBeReal", "mustBeScalarOrEmpty", "mustBeSparse", "mustBeText", "mustBeTextScalar", "mustBeUnderlyingType",
    "mustBeValidVariableName", "mustBeVector", "lasterr", "lasterror", "lastwarn", "rethrow", "matlab.lang.makeValidName",
    "matlab.lang.makeUniqueStrings", "namedargs2cell", "onCleanup", "validatecolor", "mlock", "munlock", "mislocked", "inputParser",
    "matlivre_inputparser_method", "MException", "MException.last", "throw", "throwAsCaller", "getReport", "addCause", "addCorrection",
    "matlivre_mexception_method",
];

// ---- MException ----------------------------------------------------------------------------

const ME_PROPS: &[&str] = &["identifier", "message", "cause", "stack", "Correction"];
const ME_METHODS: &[&str] = &["throw", "rethrow", "throwAsCaller", "getReport", "addCause", "addCorrection"];

fn me_class() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let mut src = String::from("classdef MException\n  properties\n");
        for p in ME_PROPS {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\n  methods\n");
        for m in ME_METHODS {
            src.push_str(&format!("    function varargout = {m}(varargin)\n      varargout = matlivre_mexception_method('{m}', varargin{{:}});\n    end\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

/// The identifier MATLAB gives its own errors, from their message.
pub fn builtin_identifier(message: &str) -> &'static str {
    const KNOWN: &[(&str, &str)] = &[
        ("Index exceeds", "MATLAB:badsubscript"),
        ("Index in position", "MATLAB:badsubscript"),
        ("Array indices must be positive integers", "MATLAB:badsubscript"),
        ("Unrecognized function or variable", "MATLAB:UndefinedFunction"),
        ("Undefined function", "MATLAB:UndefinedFunction"),
        ("Arrays have incompatible sizes", "MATLAB:sizeDimensionsMustMatch"),
        ("Incorrect dimensions for matrix multiplication", "MATLAB:innerdim"),
        ("Not enough input arguments", "MATLAB:minrhs"),
        ("Too many input arguments", "MATLAB:TooManyInputs"),
        ("Too many output arguments", "MATLAB:TooManyOutputs"),
        ("Unrecognized field name", "MATLAB:nonExistentField"),
        ("Dimensions of arrays being concatenated are not consistent", "MATLAB:catenate:dimensionMismatch"),
        ("Unable to perform assignment because the left and right sides have a different number of elements", "MATLAB:matrix:singleSubscriptNumelMismatch"),
    ];
    KNOWN.iter().find(|(p, _)| message.starts_with(p)).map(|(_, id)| *id).unwrap_or("")
}

/// An MException value for an error (as `catch e` sees it).
pub fn mexception(message: &str, identifier: &str) -> Value {
    let id = if identifier.is_empty() { builtin_identifier(message) } else { identifier };
    let stack = StructArray { dims: vec![0, 1], ..StructArray::empty_with(vec!["file".into(), "name".into(), "line".into()]) };
    let values = vec![
        Value::chars(id),
        Value::chars(message),
        Value::Cell(CellArray::from_dims(vec![0, 0], Vec::new())),
        Value::Struct(stack),
        Value::empty(),
    ];
    Value::Object(Object { class: me_class(), props: Rc::new(RefCell::new(values)) })
}

/// (message, identifier) of an error value: an MException or an error struct.
pub fn error_parts(v: &Value) -> Option<(String, String)> {
    let get = |name: &str| -> Option<String> {
        match v {
            Value::Object(o) if o.class.name == "MException" => o.get(name).and_then(|x| x.text().ok()),
            Value::Struct(s) => s.get(name).and_then(|x| x.text().ok()),
            _ => None,
        }
    };
    if !matches!(v, Value::Struct(_)) && !matches!(v, Value::Object(o) if o.class.name == "MException") {
        return None;
    }
    Some((get("message").unwrap_or_default(), get("identifier").unwrap_or_default()))
}

fn me_method(it: &mut Interpreter, a: &[Value]) -> Ret {
    let m = arg(a, 0, "matlivre_mexception_method")?.text()?;
    let me = arg(a, 1, &m)?;
    let Some((msg, id)) = error_parts(me) else { return rt("Expected an MException.") };
    let out = match m.as_str() {
        "throw" | "rethrow" | "throwAsCaller" => {
            it.last_error = (msg.clone(), id.clone());
            return rt_id(&id, msg);
        }
        "getReport" => vec![Value::chars(&msg)],
        "addCause" | "addCorrection" => {
            let Value::Object(o) = me else { return rt("Expected an MException.") };
            let copy = Object { class: o.class.clone(), props: Rc::new(RefCell::new(o.props.borrow().clone())) };
            let prop = if m == "addCause" { "cause" } else { "Correction" };
            let add = arg(a, 2, &m)?.clone();
            let mut items = match copy.get(prop) {
                Some(Value::Cell(c)) => c.data,
                _ => Vec::new(),
            };
            items.push(add);
            let n = items.len();
            let v = if m == "addCause" { Value::Cell(CellArray::from_dims(vec![n, 1], items)) } else { items.pop().unwrap() };
            set(&copy, prop, v);
            vec![Value::Object(copy)]
        }
        _ => return rt(format!("Undefined method '{m}' for MException.")),
    };
    one(Value::Cell(CellArray::row(out)))
}

/// `ME = ...` display of an MException.
pub fn display_mexception(name: &str, v: &Value, f: crate::display::NumFormat) -> Option<String> {
    let Value::Object(o) = v else { return None };
    if o.class.name != "MException" {
        return None;
    }
    let mut s = StructArray::scalar();
    for p in ME_PROPS {
        s.set(p, o.get(p).unwrap_or_else(Value::empty));
    }
    let body = crate::display::struct_fields_body(&s, f).replace("cause: {0×0 cell}", "cause: {}");
    Some(format!("{name} = \n  MException with properties:\n\n{}", body.trim_end_matches('\n')))
}

const IP_METHODS: &[&str] = &["addRequired", "addOptional", "addParameter", "addParamValue", "parse"];

fn fail<T>(name: &str, msg: impl Into<String>) -> R<T> {
    rt_id(&format!("MATLAB:validators:{name}"), msg)
}

fn nums(v: &Value) -> Option<&Matrix> {
    match v {
        Value::Num(m) if m.kind != Kind::Char => Some(m),
        _ => None,
    }
}

/// The values of a numeric (or logical) argument, or the "numeric or logical" error.
fn real_values<'a>(name: &str, v: &'a Value) -> R<&'a Matrix> {
    match nums(v) {
        Some(m) => Ok(m),
        None => fail(name, "Value must be numeric or logical."),
    }
}

fn all(m: &Matrix, f: impl Fn(f64) -> bool) -> bool {
    m.data.iter().all(|&x| f(x))
}

fn texts(v: &Value) -> R<Vec<String>> {
    match v {
        Value::Str(s) => Ok(s.data.iter().map(|x| x.clone().unwrap_or_default()).collect()),
        Value::Cell(c) => c.data.iter().map(|x| x.text()).collect(),
        other => Ok(vec![other.text()?]),
    }
}

fn is_text(v: &Value) -> bool {
    match v {
        Value::Num(m) => m.kind == Kind::Char,
        Value::Str(_) => true,
        Value::Cell(c) => c.data.iter().all(|x| matches!(x, Value::Num(m) if m.kind == Kind::Char && m.rows() <= 1)),
        _ => false,
    }
}

/// `mustBeX(value, ...)`: `Ok(())` when the value passes.
fn validate(it: &mut Interpreter, name: &str, a: &[Value]) -> R<()> {
    let v = arg(a, 0, name)?;
    let bound = |i: usize| -> R<f64> { arg(a, i, name)?.scalar() };
    match name {
        "mustBeA" => {
            let classes = texts(arg(a, 1, name)?)?;
            let cls = v.class_name();
            let ok = classes.iter().any(|c| *c == cls || crate::builtins::call(it, "isa", vec![v.clone(), Value::chars(c)], 1).ok().flatten().and_then(|r| r.into_iter().next()).map(|r| r.is_true().unwrap_or(false)).unwrap_or(false));
            if !ok {
                let list = classes.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ");
                return fail(name, format!("Value must be one of the following types: {list}."));
            }
        }
        "mustBeFinite" => {
            if !all(real_values(name, v)?, f64::is_finite) {
                return fail(name, "Value must be finite.");
            }
        }
        "mustBeFloat" => {
            if !matches!(v, Value::Num(m) if matches!(m.kind, Kind::Double | Kind::Single)) {
                return fail(name, "Value must be a floating-point array.");
            }
        }
        "mustBeGreaterThan" | "mustBeGreaterThanOrEqual" | "mustBeLessThan" | "mustBeLessThanOrEqual" => {
            let c = bound(1)?;
            let m = real_values(name, v)?;
            let (ok, words) = match name {
                "mustBeGreaterThan" => (all(m, |x| x > c), "greater than"),
                "mustBeGreaterThanOrEqual" => (all(m, |x| x >= c), "greater than or equal to"),
                "mustBeLessThan" => (all(m, |x| x < c), "less than"),
                _ => (all(m, |x| x <= c), "less than or equal to"),
            };
            if !ok {
                return fail(name, format!("Value must be {words} {}.", num2str_scalar(c)));
            }
        }
        "mustBeInRange" => {
            let (lo, hi) = (bound(1)?, bound(2)?);
            let bounds = a.get(3).map(|b| b.text()).transpose()?.unwrap_or_else(|| "inclusive".into()).to_lowercase();
            let (lo_open, hi_open) = match bounds.as_str() {
                "inclusive" => (false, false),
                "exclusive" => (true, true),
                "exclude-lower" => (true, false),
                "exclude-upper" => (false, true),
                _ => return rt("Bounds must be 'inclusive', 'exclusive', 'exclude-lower', or 'exclude-upper'."),
            };
            let m = real_values(name, v)?;
            if !all(m, |x| (if lo_open { x > lo } else { x >= lo }) && (if hi_open { x < hi } else { x <= hi })) {
                let l = if lo_open { "greater than" } else { "greater than or equal to" };
                let h = if hi_open { "less than" } else { "less than or equal to" };
                return fail(name, format!("Value must be {l} {}, and {h} {}.", num2str_scalar(lo), num2str_scalar(hi)));
            }
        }
        "mustBeInteger" => {
            if !all(real_values(name, v)?, |x| x.is_finite() && x.fract() == 0.0) {
                return fail(name, "Value must be integer.");
            }
        }
        "mustBeMember" => {
            let set = arg(a, 1, name)?;
            let r = crate::builtins::call(it, "ismember", vec![v.clone(), set.clone()], 1)?.and_then(|r| r.into_iter().next()).unwrap_or_else(|| Value::bool(false));
            let ok = nums(&r).map(|m| all(m, |x| x != 0.0)).unwrap_or(false);
            if !ok {
                let items: Vec<String> = match set {
                    Value::Num(m) if m.kind != Kind::Char => m.data.iter().map(|&x| num2str_scalar(x)).collect(),
                    other => texts(other)?.into_iter().map(|t| format!("'{t}'")).collect(),
                };
                let list: String = items.iter().map(|i| format!("    {i}\n")).collect();
                return fail(name, format!("Value must be a member of this set:\n{list}"));
            }
        }
        "mustBeNegative" | "mustBeNonnegative" | "mustBeNonpositive" | "mustBePositive" | "mustBeNonzero" => {
            let m = real_values(name, v)?;
            let (ok, msg) = match name {
                "mustBeNegative" => (all(m, |x| x < 0.0), "Value must be negative."),
                "mustBeNonnegative" => (all(m, |x| x >= 0.0), "Value must be nonnegative."),
                "mustBeNonpositive" => (all(m, |x| x <= 0.0), "Value must not be positive."),
                "mustBePositive" => (all(m, |x| x > 0.0), "Value must be positive."),
                _ => (all(m, |x| x != 0.0), "Value must not be zero."),
            };
            if !ok {
                return fail(name, msg);
            }
        }
        "mustBeNonempty" => {
            if v.numel() == 0 {
                return fail(name, "Value must not be empty.");
            }
        }
        "mustBeNonmissing" => {
            let r = crate::builtins::call(it, "ismissing", vec![v.clone()], 1)?.and_then(|r| r.into_iter().next());
            if r.and_then(|r| nums(&r).map(|m| m.data.iter().any(|&x| x != 0.0))).unwrap_or(false) {
                return fail(name, "Value must not have missing data.");
            }
        }
        "mustBeNonNan" => {
            if nums(v).is_some_and(|m| m.data.iter().any(|x| x.is_nan())) {
                return fail(name, "Value must not be NaN.");
            }
        }
        "mustBeNonsparse" => {}
        "mustBeSparse" => return fail(name, "Value must be sparse."),
        "mustBeNonzeroLengthText" => {
            let ok = is_text(v) && texts(v).map(|t| t.iter().all(|s| !s.is_empty())).unwrap_or(false);
            if !ok {
                return fail(name, "Value must be text with one or more characters.");
            }
        }
        "mustBeNumeric" => {
            if !matches!(v, Value::Num(m) if !matches!(m.kind, Kind::Char | Kind::Logical)) {
                return fail(name, "Value must be numeric.");
            }
        }
        "mustBeNumericOrLogical" => {
            if nums(v).is_none() {
                return fail(name, "Value must be numeric or logical.");
            }
        }
        "mustBeReal" => {
            if !matches!(v, Value::Num(m) if m.kind != Kind::Char && !m.is_complex()) {
                return fail(name, "Value must be real.");
            }
        }
        "mustBeScalarOrEmpty" => {
            if v.numel() > 1 {
                return fail(name, "Value must be scalar or empty.");
            }
        }
        "mustBeText" => {
            if !is_text(v) {
                return fail(name, "Value must be a character vector, string array, or cell array of character vectors.");
            }
        }
        "mustBeTextScalar" => {
            let ok = matches!(v, Value::Num(m) if m.kind == Kind::Char && m.rows() <= 1) || matches!(v, Value::Str(s) if s.data.len() == 1);
            if !ok {
                return fail(name, "Value must be a character vector or string scalar.");
            }
        }
        "mustBeUnderlyingType" => {
            let want = arg(a, 1, name)?.text()?;
            if v.class_name() != want {
                return fail(name, format!("Value must have underlying type '{want}'."));
            }
        }
        "mustBeValidVariableName" => {
            let bad: Vec<String> = texts(v)?.into_iter().filter(|s| !crate::builtins_lang::is_varname(s)).collect();
            if !bad.is_empty() {
                let list = bad.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ");
                return fail(name, format!("The following are not valid variable names: {list}."));
            }
        }
        "mustBeVector" => {
            let opt = a.get(1).map(|o| o.text()).transpose()?.unwrap_or_default().to_lowercase();
            let d = v.dims();
            let empty = v.numel() == 0;
            let vector = d.len() == 2 && (d[0] == 1 || d[1] == 1);
            let ok = vector || (empty && opt == "allow-all-empties");
            if !ok {
                return fail(name, "Value must be a 1-by-n vector or an n-by-1 vector.");
            }
        }
        "mustBeFile" | "mustBeFolder" => {
            let file = name == "mustBeFile";
            let missing: Vec<String> = texts(v)?
                .into_iter()
                .filter(|p| if file { !std::path::Path::new(p).is_file() } else { !std::path::Path::new(p).is_dir() })
                .collect();
            if !missing.is_empty() {
                let list = missing.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ");
                let what = if file { "files" } else { "folders" };
                return fail(name, format!("The following {what} do not exist: {list}."));
            }
        }
        _ => return rt(format!("Unknown validator '{name}'.")),
    }
    Ok(())
}

// ---- matlab.lang names -----------------------------------------------------------------------

/// Applies `f` to each text of `v`, keeping its container type.
fn map_texts(v: &Value, mut f: impl FnMut(&str) -> String) -> R<Value> {
    Ok(match v {
        Value::Str(s) => Value::Str(StrArray::from_dims(s.dims.clone(), s.data.iter().map(|x| x.as_deref().map(&mut f)).collect())),
        Value::Cell(c) => Value::Cell(CellArray::from_dims(c.dims.clone(), c.data.iter().map(|x| x.text().map(|t| Value::chars(&f(&t)))).collect::<R<_>>()?)),
        other => Value::chars(&f(&other.text()?)),
    })
}

/// matlab.lang.makeValidName for a name that is not valid: whitespace dropped with the next
/// letter capitalised, other invalid characters replaced as `style` says, `prefix` in front of
/// a name that does not start with a letter or that is a keyword.
fn valid_name(s: &str, style: &str, prefix: &str) -> String {
    if crate::builtins_lang::KEYWORDS.contains(&s) {
        let mut c = s.chars();
        let first: String = c.next().map(|f| f.to_uppercase().collect()).unwrap_or_default();
        return format!("{prefix}{first}{}", c.as_str());
    }
    let mut out = String::new();
    let mut cap = false;
    for ch in s.trim_start().chars() {
        if ch.is_whitespace() {
            cap = true;
            continue;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if cap {
                out.extend(ch.to_uppercase());
            } else {
                out.push(ch);
            }
        } else {
            match style {
                "delete" => {}
                "hex" => out.push_str(&format!("0x{:X}", ch as u32)),
                _ => out.push('_'),
            }
        }
        cap = false;
    }
    if !out.starts_with(|c: char| c.is_ascii_alphabetic()) {
        out = format!("{prefix}{out}");
    }
    out.chars().take(63).collect()
}

fn make_unique(a: &[Value]) -> Ret {
    let v = arg(a, 0, "matlab.lang.makeUniqueStrings")?;
    let items = texts(v)?;
    let exclude: Vec<String> = match a.get(1) {
        Some(e) if is_text(e) => texts(e)?,
        _ => Vec::new(),
    };
    let mut used: Vec<String> = exclude.clone();
    let mut out = Vec::with_capacity(items.len());
    let mut first: HashMap<String, bool> = HashMap::new();
    for s in &items {
        // The first occurrence keeps its name unless it is excluded.
        let keep = !exclude.contains(s) && !first.contains_key(s);
        first.insert(s.clone(), true);
        let name = if keep && !used.contains(s) { s.clone() } else { (1..).map(|k| format!("{s}_{k}")).find(|n| !used.contains(n) && !items.contains(n)).unwrap() };
        used.push(name.clone());
        out.push(name);
    }
    let changed: Vec<f64> = items.iter().zip(&out).map(|(p, q)| (p != q) as u8 as f64).collect();
    let mut it = out.into_iter();
    let made = map_texts(v, |_| it.next().unwrap_or_default())?;
    let dims = match &made {
        Value::Num(_) => vec![1, 1],
        other => other.dims(),
    };
    Ok(Some(vec![made, Value::Num(Matrix::from_dims(dims, changed).with_kind(Kind::Logical))]))
}

// ---- inputParser ---------------------------------------------------------------------------

const IP_PROPS: &[&str] = &["FunctionName", "CaseSensitive", "KeepUnmatched", "PartialMatching", "StructExpand", "Parameters", "Results", "Unmatched", "UsingDefaults"];

fn ip_class() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let mut src = String::from("classdef inputParser < handle\n  properties\n");
        for p in IP_PROPS {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\n  properties (Hidden)\n    Specs\n  end\n  methods\n");
        for m in IP_METHODS {
            src.push_str(&format!("    function varargout = {m}(varargin)\n      varargout = matlivre_inputparser_method('{m}', varargin{{:}});\n    end\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

/// One declared argument: kind (0 required, 1 optional, 2 parameter), name, default, validator.
struct Spec {
    kind: u8,
    name: String,
    default: Value,
    validator: Option<Value>,
}

fn specs_of(o: &Object) -> Vec<Spec> {
    let Some(Value::Cell(c)) = o.get("Specs") else { return Vec::new() };
    c.data
        .chunks(4)
        .map(|q| Spec {
            kind: q[0].scalar().unwrap_or(0.0) as u8,
            name: q[1].text().unwrap_or_default(),
            default: q[2].clone(),
            validator: if q[3].numel() == 0 && !matches!(q[3], Value::Func(_)) { None } else { Some(q[3].clone()) },
        })
        .collect()
}

fn store_specs(o: &Object, specs: &[Spec]) {
    let data: Vec<Value> = specs
        .iter()
        .flat_map(|s| [Value::num(s.kind as f64), Value::chars(&s.name), s.default.clone(), s.validator.clone().unwrap_or_else(Value::empty)])
        .collect();
    set(o, "Specs", Value::Cell(CellArray::row(data)));
    let mut names: Vec<String> = specs.iter().map(|s| s.name.clone()).collect();
    names.sort();
    set(o, "Parameters", Value::Cell(CellArray::row(names.iter().map(|n| Value::chars(n)).collect())));
}

fn set(o: &Object, name: &str, v: Value) {
    if let Some(i) = o.class.prop_index(name) {
        o.props.borrow_mut()[i] = v;
    }
}

fn flag(o: &Object, name: &str) -> bool {
    o.get(name).map(|v| v.is_true().unwrap_or(false)).unwrap_or(false)
}

fn new_input_parser() -> Ret {
    let empty_struct = Value::Struct(StructArray::scalar());
    let values = vec![
        Value::chars(""),
        Value::bool(false),
        Value::bool(false),
        Value::bool(true),
        Value::bool(true),
        Value::Cell(CellArray::from_dims(vec![1, 0], Vec::new())),
        empty_struct.clone(),
        empty_struct,
        Value::Cell(CellArray::from_dims(vec![1, 0], Vec::new())),
        Value::Cell(CellArray::row(Vec::new())),
    ];
    one(Value::Object(Object { class: ip_class(), props: Rc::new(RefCell::new(values)) }))
}

/// Runs a validator: false or an error makes the value invalid (with the reason).
fn check(it: &mut Interpreter, spec: &Spec, v: &Value) -> R<()> {
    let Some(f) = &spec.validator else { return Ok(()) };
    let shown = match f {
        // A named function shows without its @ (isnumeric), an anonymous one in full.
        Value::Func(fv) => crate::display::func_text(fv).trim_start_matches('@').to_string(),
        other => other.text().unwrap_or_default(),
    };
    let shown = if matches!(f, Value::Func(FuncValue::Anon { .. })) { format!("@{shown}") } else { shown };
    match it.call_func_value(f, vec![v.clone()], 1) {
        Ok(out) => {
            // Validators that return nothing (mustBe*) pass by not throwing.
            let ok = out.first().map(|r| r.is_true().unwrap_or(false)).unwrap_or(true);
            if ok {
                Ok(())
            } else {
                rt_id("MATLAB:InputParser:ArgumentFailedValidation", format!("The value of '{}' is invalid. It must satisfy the function: {shown}.", spec.name))
            }
        }
        Err(e) if e.message.starts_with("Too many output arguments") || e.message.contains("output") => {
            it.call_func_value(f, vec![v.clone()], 0).map(|_| ()).map_err(|e| LangError::new(format!("The value of '{}' is invalid. {}", spec.name, e.message), 0).with_id("MATLAB:InputParser:ArgumentFailedValidation"))
        }
        Err(e) => rt_id("MATLAB:InputParser:ArgumentFailedValidation", format!("The value of '{}' is invalid. {}", spec.name, e.message)),
    }
}

fn parse(it: &mut Interpreter, o: &Object, args: &[Value]) -> R<()> {
    let specs = specs_of(o);
    let case = flag(o, "CaseSensitive");
    let partial = flag(o, "PartialMatching");
    let keep = flag(o, "KeepUnmatched");
    let expand = flag(o, "StructExpand");
    let mut results: Vec<(String, Value)> = Vec::new();
    let mut given: Vec<String> = Vec::new();
    let mut unmatched: Vec<(String, Value)> = Vec::new();
    let params: Vec<&Spec> = specs.iter().filter(|s| s.kind == 2).collect();
    let find_param = |name: &str| -> Option<&Spec> {
        let eq = |a: &str, b: &str| if case { a == b } else { a.eq_ignore_ascii_case(b) };
        if let Some(p) = params.iter().find(|p| eq(&p.name, name)) {
            return Some(p);
        }
        if partial {
            let hits: Vec<&&Spec> = params.iter().filter(|p| p.name.len() >= name.len() && eq(&p.name[..name.len()], name)).collect();
            if hits.len() == 1 {
                return Some(hits[0]);
            }
        }
        None
    };
    let mut i = 0;
    for s in specs.iter().filter(|s| s.kind == 0) {
        let Some(v) = args.get(i) else { return rt_id("MATLAB:minrhs", "Not enough input arguments.") };
        check(it, s, v)?;
        results.push((s.name.clone(), v.clone()));
        given.push(s.name.clone());
        i += 1;
    }
    for s in specs.iter().filter(|s| s.kind == 1) {
        let Some(v) = args.get(i) else { break };
        // Text naming a parameter starts the name-value pairs instead; so does any text when
        // the optional argument has no validator to claim it.
        if v.is_text() && (v.text().ok().is_some_and(|t| find_param(&t).is_some()) || (s.validator.is_none() && !params.is_empty())) {
            break;
        }
        if check(it, s, v).is_err() && v.is_text() {
            break;
        }
        check(it, s, v)?;
        results.push((s.name.clone(), v.clone()));
        given.push(s.name.clone());
        i += 1;
    }
    let mut pairs: Vec<(String, Value)> = Vec::new();
    while i < args.len() {
        if let (true, Value::Struct(st)) = (expand, &args[i]) {
            if st.elems.len() == 1 {
                for (k, f) in st.fields.iter().enumerate() {
                    pairs.push((f.clone(), st.elems[0][k].clone()));
                }
                i += 1;
                continue;
            }
        }
        let name = args[i].text().map_err(|_| LangError::new("Expected a parameter name.", 0).with_id("MATLAB:InputParser:ParamMustBeChar"))?;
        let Some(v) = args.get(i + 1) else { return rt_id("MATLAB:InputParser:ParamMissingValue", format!("No value was given for '{name}'. Name-value pair arguments require a name followed by a value.")) };
        pairs.push((name, v.clone()));
        i += 2;
    }
    for (name, v) in pairs {
        match find_param(&name) {
            Some(s) => {
                check(it, s, &v)?;
                results.retain(|(n, _)| *n != s.name);
                results.push((s.name.clone(), v));
                given.push(s.name.clone());
            }
            None if keep => unmatched.push((name, v)),
            None => {
                return rt_id(
                    "MATLAB:InputParser:UnmatchedParameter",
                    format!("'{name}' is not a recognized parameter. For a list of valid name-value pair arguments, see the documentation for this function."),
                )
            }
        }
    }
    let mut defaults: Vec<String> = Vec::new();
    for s in &specs {
        if !given.contains(&s.name) {
            results.push((s.name.clone(), s.default.clone()));
            defaults.push(s.name.clone());
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    defaults.sort();
    let mut r = StructArray::scalar();
    for (n, v) in results {
        r.set(&n, v);
    }
    let mut u = StructArray::scalar();
    for (n, v) in unmatched {
        u.set(&n, v);
    }
    set(o, "Results", Value::Struct(r));
    set(o, "Unmatched", Value::Struct(u));
    set(o, "UsingDefaults", Value::Cell(CellArray::row(defaults.iter().map(|d| Value::chars(d)).collect())));
    Ok(())
}

fn ip_method(it: &mut Interpreter, a: &[Value]) -> Ret {
    let m = arg(a, 0, "matlivre_inputparser_method")?.text()?;
    let Some(Value::Object(o)) = a.get(1) else { return rt("Expected an inputParser.") };
    let o = o.clone();
    let rest = &a[2..];
    if m == "parse" {
        parse(it, &o, rest)?;
        return one(Value::Cell(CellArray::row(Vec::new())));
    }
    let name = arg(rest, 0, &m)?.text()?;
    if !crate::builtins_lang::is_varname(&name) {
        return rt(format!("'{name}' is not a valid argument name."));
    }
    let mut specs = specs_of(&o);
    if specs.iter().any(|s| s.name.eq_ignore_ascii_case(&name)) {
        return rt_id("MATLAB:InputParser:DuplicateParameter", format!("An argument named '{name}' already exists in the schema. Use a unique name for each argument."));
    }
    let (kind, default, validator) = match m.as_str() {
        "addRequired" => (0, Value::empty(), rest.get(1).cloned()),
        "addOptional" => (1, arg(rest, 1, &m)?.clone(), rest.get(2).filter(|v| !v.is_text()).cloned()),
        _ => (2, arg(rest, 1, &m)?.clone(), rest.get(2).filter(|v| !v.is_text()).cloned()),
    };
    if kind == 1 && specs.iter().any(|s| s.kind == 2) {
        return rt("Optional arguments must be added before parameters.");
    }
    specs.push(Spec { kind, name, default, validator });
    store_specs(&o, &specs);
    one(Value::Cell(CellArray::row(Vec::new())))
}

/// `p = ...` display of an inputParser: its properties as a struct shows them.
pub fn display(name: &str, v: &Value, f: crate::display::NumFormat) -> Option<String> {
    let Value::Object(o) = v else { return None };
    if o.class.name != "inputParser" {
        return None;
    }
    let mut s = StructArray::scalar();
    for p in IP_PROPS {
        s.set(p, o.get(p).unwrap_or_else(Value::empty));
    }
    Some(format!("{name} = \n  inputParser with properties:\n\n{}", crate::display::struct_fields_body(&s, f).trim_end_matches('\n')))
}

// ---- onCleanup -----------------------------------------------------------------------------

fn cleanup_class() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let ast = crate::class::parse_classdef("classdef onCleanup < handle\n  properties\n    task\n  end\nend\n").expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

/// Runs the task of an `onCleanup` object whose last reference is going away.
pub fn run_cleanup(it: &mut Interpreter, v: &Value) {
    if let Value::Object(o) = v {
        if o.class.name == "onCleanup" && Rc::strong_count(&o.props) == 1 {
            if let Some(task) = o.get("task") {
                let _ = it.call_func_value(&task, Vec::new(), 0);
            }
        }
    }
}

// ---- colors --------------------------------------------------------------------------------

fn validatecolor(a: &[Value]) -> Ret {
    let v = arg(a, 0, "validatecolor")?;
    let multiple = a.get(1).map(|o| o.text()).transpose()?.is_some_and(|t| t.eq_ignore_ascii_case("multiple"));
    let bad = |s: &str| -> Ret {
        rt_id(
            "MATLAB:graphics:validatecolor:InvalidColorString",
            format!("'{s}' is not a valid color specification. Valid names include: 'red', 'green', 'blue', 'cyan', 'magenta', 'yellow', 'black', and 'white'. Valid hexadecimal color codes consist of '#' followed by three or six hexadecimal digits."),
        )
    };
    let rows: Vec<[f64; 3]> = match v {
        Value::Num(m) if m.kind != Kind::Char => {
            let scale = match m.kind {
                Kind::Double | Kind::Single => 1.0,
                Kind::Int(_) => {
                    let max = m.kind.cast(f64::INFINITY);
                    if max.is_finite() { max } else { 1.0 }
                }
                _ => return rt("Color values must be numeric."),
            };
            if m.cols() != 3 {
                return rt_id("MATLAB:graphics:validatecolor:InvalidRGBSize", "RGB triplets must be specified as a 1-by-3 vector or an m-by-3 matrix.");
            }
            let n = m.rows();
            let rows: Vec<[f64; 3]> = (0..n).map(|i| [m.data[i] / scale, m.data[n + i] / scale, m.data[2 * n + i] / scale]).collect();
            if rows.iter().flatten().any(|x| !(0.0..=1.0).contains(x)) {
                return rt_id("MATLAB:graphics:validatecolor:OutOfRange", "Values of RGB triplets must be in the range [0, 1].");
            }
            rows
        }
        other => {
            let mut rows = Vec::new();
            for t in texts(other)? {
                match crate::gfx::color_arg(&Value::chars(&t)) {
                    Ok(Some((_, rgb))) => rows.push(rgb),
                    _ => return bad(&t),
                }
            }
            rows
        }
    };
    if rows.len() > 1 && !multiple {
        return rt_id("MATLAB:graphics:validatecolor:MultipleColors", "Specify 'multiple' to validate more than one color.");
    }
    let n = rows.len();
    let mut data = vec![0.0; n * 3];
    for (i, r) in rows.iter().enumerate() {
        for k in 0..3 {
            data[k * n + i] = r[k];
        }
    }
    one(Value::Num(Matrix::new(n, 3, data)))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    if name.starts_with("mustBe") {
        validate(it, name, a)?;
        return Ok(Some(Vec::new()));
    }
    match name {
        "lasterr" | "lastwarn" => {
            let cur = if name == "lasterr" { it.last_error.clone() } else { it.last_warning.clone() };
            if let Some(m) = a.first() {
                let id = a.get(1).map(|v| v.text()).transpose()?.unwrap_or_default();
                let new = (m.text()?, id);
                if name == "lasterr" { it.last_error = new } else { it.last_warning = new }
            }
            let mut out = vec![Value::chars(&cur.0)];
            if nargout > 1 {
                out.push(Value::chars(&cur.1));
            }
            Ok(Some(out))
        }
        "lasterror" => {
            if a.first().map(|v| v.text().map(|t| t == "reset").unwrap_or(false)).unwrap_or(false) {
                it.last_error = (String::new(), String::new());
            }
            let mut s = StructArray::scalar();
            s.set("message", Value::chars(&it.last_error.0));
            s.set("identifier", Value::chars(&it.last_error.1));
            s.set("stack", Value::Struct(StructArray::empty_with(vec!["file".into(), "name".into(), "line".into()])));
            one(Value::Struct(s))
        }
        "rethrow" | "throw" | "throwAsCaller" => {
            let Some((msg, id)) = a.first().and_then(error_parts) else { return rt(format!("{name} requires an error structure or MException.")) };
            it.last_error = (msg.clone(), id.clone());
            rt_id(&id, msg)
        }
        "MException" => {
            if let Some(Value::Object(o)) = a.first() {
                if let Some(p) = crate::messages::parts(o) {
                    let (msg, id) = p?;
                    return one(mexception(&msg, &id));
                }
            }
            let id = arg(a, 0, name)?.text()?;
            let fmt = arg(a, 1, name)?.text()?;
            let msg = crate::format::sprintf(&fmt, &a[2..])?;
            if !id.is_empty() && !id.contains(':') {
                return rt("The identifier must have at least one colon (e.g. 'component:message').");
            }
            one(mexception(&msg, &id))
        }
        "MException.last" => {
            if a.first().map(|v| v.text().map(|t| t == "reset").unwrap_or(false)).unwrap_or(false) {
                it.last_error = (String::new(), String::new());
                return Ok(Some(Vec::new()));
            }
            let (msg, id) = it.last_error.clone();
            one(mexception(&msg, &id))
        }
        "getReport" | "addCause" | "addCorrection" => {
            let mut args = vec![Value::chars(name)];
            args.extend(a.iter().cloned());
            let r = me_method(it, &args)?.and_then(|mut v| v.pop());
            Ok(r.and_then(|c| match c {
                Value::Cell(c) => Some(c.data),
                _ => None,
            }))
        }
        "matlivre_mexception_method" => me_method(it, a),
        "matlab.lang.makeValidName" => {
            let v = arg(a, 0, name)?;
            let (mut style, mut prefix) = ("underscore".to_string(), "x".to_string());
            let mut k = 1;
            while k + 1 < a.len() {
                let key = a[k].text()?;
                if key.eq_ignore_ascii_case("ReplacementStyle") {
                    style = a[k + 1].text()?.to_lowercase();
                    if !matches!(style.as_str(), "underscore" | "delete" | "hex") {
                        return rt("Expected ReplacementStyle to match one of these values:\n\n'underscore', 'delete', 'hex'");
                    }
                } else if key.eq_ignore_ascii_case("Prefix") {
                    prefix = a[k + 1].text()?;
                    if !crate::builtins_lang::is_varname(&prefix) {
                        return rt("Invalid value for 'Prefix'. It must be a valid variable name.");
                    }
                }
                k += 2;
            }
            let made = map_texts(v, |s| if crate::builtins_lang::is_varname(s) { s.to_string() } else { valid_name(s, &style, &prefix) })?;
            let changed: Vec<f64> = texts(v)?.iter().map(|s| if crate::builtins_lang::is_varname(s) { 0.0 } else { 1.0 }).collect();
            let dims = match v {
                Value::Num(_) => vec![1, 1],
                other => other.dims(),
            };
            Ok(Some(vec![made, Value::Num(Matrix::from_dims(dims, changed).with_kind(Kind::Logical))]))
        }
        "matlab.lang.makeUniqueStrings" => make_unique(a),
        "namedargs2cell" => {
            let Some(Value::Struct(s)) = a.first() else { return rt("Input must be a scalar structure.") };
            if s.elems.len() != 1 {
                return rt("Input must be a scalar structure.");
            }
            let data: Vec<Value> = s.fields.iter().enumerate().flat_map(|(k, f)| [Value::chars(f), s.elems[0][k].clone()]).collect();
            one(Value::Cell(CellArray::row(data)))
        }
        "onCleanup" => {
            let f = arg(a, 0, name)?.clone();
            if !matches!(f, Value::Func(_)) {
                return rt("The input to onCleanup must be a function handle.");
            }
            one(Value::Object(Object { class: cleanup_class(), props: Rc::new(RefCell::new(vec![f])) }))
        }
        "validatecolor" => validatecolor(a),
        "mlock" | "munlock" => Ok(Some(Vec::new())),
        "mislocked" => one(Value::bool(false)),
        "inputParser" => new_input_parser(),
        "matlivre_inputparser_method" => ip_method(it, a),
        _ => Ok(None),
    }
}
