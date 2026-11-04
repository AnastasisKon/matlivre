//! Function objects: `inline` (with `argnames`, `formula`, `fcnchk`), memoized functions
//! (`memoize`, `matlab.lang.MemoizedFunction` with `stats` and `clearCache`,
//! `clearAllMemoizedCaches`) and `timeit`.

use crate::builtins::{arg, Ret};
use crate::class::ClassDef;
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["inline", "fcnchk", "memoize", "clearAllMemoizedCaches", "timeit", "matlivre_funcobj_method"];

const MEMO_CLASS: &str = "matlab.lang.MemoizedFunction";

/// Names `inline` never takes as arguments.
const CONSTANTS: &[&str] = &["i", "j", "pi", "Inf", "inf", "NaN", "nan", "eps"];

fn class_def(class: &str) -> Rc<ClassDef> {
    thread_local! {
        static DEFS: RefCell<HashMap<String, Rc<ClassDef>>> = RefCell::new(HashMap::new());
    }
    DEFS.with(|defs| {
        if let Some(d) = defs.borrow().get(class) {
            return d.clone();
        }
        let (short, header, props, methods): (&str, &str, &[&str], &[&str]) = if class == "inline" {
            ("inline", "classdef inline\n  properties (Hidden)\n", &["expr", "args"], &["subsref", "argnames", "formula", "char", "vectorize", "nargin", "feval"])
        } else {
            ("MemoizedFunction", "classdef MemoizedFunction < handle\n  properties\n", &["Function", "Enabled", "CacheSize"], &["subsref", "stats", "clearCache"])
        };
        let mut src = header.to_string();
        for p in props {
            src.push_str(&format!("    {p}\n"));
        }
        src.push_str("  end\n  methods\n");
        for m in methods {
            src.push_str(&format!(
                "    function varargout = {m}(obj, varargin)\n      varargout = matlivre_funcobj_method('{short}.{m}', max(nargout, 1), obj, varargin{{:}});\n    end\n"
            ));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let mut def = ClassDef::build(ast, &[]);
        def.name = class.to_string();
        let def = Rc::new(def);
        defs.borrow_mut().insert(class.to_string(), def.clone());
        def
    })
}

fn make(class: &str, values: Vec<Value>) -> Value {
    Value::Object(Object { class: class_def(class), props: Rc::new(RefCell::new(values)) })
}

fn prop(o: &Object, name: &str) -> Value {
    o.get(name).unwrap_or_else(Value::empty)
}

// ---- inline ----------------------------------------------------------------------------------

/// The variables of an expression, as `inline` finds them: identifiers that are not field
/// names, not called with parentheses and not constants, in alphabetical order; `x` if none.
fn find_args(expr: &str) -> Vec<String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut found: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let field = start > 0 && chars[start - 1] == '.';
            let mut k = i;
            while k < chars.len() && chars[k] == ' ' {
                k += 1;
            }
            let called = k < chars.len() && chars[k] == '(';
            if !field && !called && !CONSTANTS.contains(&word.as_str()) && !found.contains(&word) {
                found.push(word);
            }
        } else if c.is_ascii_digit() || c == '.' {
            // Skip numbers, including exponents such as 1e5.
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
        } else if c == '\'' || c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != c {
                i += 1;
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    found.sort();
    if found.is_empty() {
        found.push("x".into());
    }
    found
}

fn make_inline(expr: &str, args: Vec<String>) -> Value {
    let cells = args.iter().map(|a| Value::chars(a)).collect();
    make("inline", vec![Value::chars(expr), Value::Cell(CellArray::row(cells))])
}

fn inline_parts(o: &Object) -> R<(String, Vec<String>)> {
    let expr = prop(o, "expr").text()?;
    let args = match prop(o, "args") {
        Value::Cell(c) => c.data.iter().map(|v| v.text()).collect::<R<Vec<_>>>()?,
        _ => Vec::new(),
    };
    Ok((expr, args))
}

fn inline(a: &[Value]) -> R<Value> {
    let expr = match a.first() {
        Some(v @ Value::Num(m)) if m.kind == Kind::Char => v.text()?,
        Some(Value::Str(s)) if s.data.len() == 1 => s.data[0].clone().unwrap_or_default(),
        _ => return rt("Input must be a character vector."),
    };
    let args = match a.get(1) {
        None => find_args(&expr),
        // inline(expr, n): x and the parameters P1, ..., Pn.
        Some(Value::Num(m)) if m.kind != Kind::Char => {
            let n = m.data.first().copied().unwrap_or(0.0) as usize;
            std::iter::once("x".to_string()).chain((1..=n).map(|k| format!("P{k}"))).collect()
        }
        Some(_) => a[1..].iter().map(|v| v.text()).collect::<R<Vec<_>>>()?,
    };
    Ok(make_inline(&expr, args))
}

fn call_inline(it: &mut Interpreter, o: &Object, args: Vec<Value>, nargout: usize) -> R<Vec<Value>> {
    let (expr, names) = inline_parts(o)?;
    if args.len() < names.len() {
        return rt("Not enough inputs to inline function.");
    }
    if args.len() > names.len() {
        return rt("Too many inputs to inline function.");
    }
    let f = it.call_function("str2func", vec![Value::chars(&format!("@({}) {expr}", names.join(",")))], 1)?.remove(0);
    let Value::Func(fv) = f else { return rt("Invalid inline function.") };
    it.call_value(&fv, args, nargout)
}

/// `     Inline function:\n     name(x,y) = expr`, as `display` and `disp` show it.
pub fn inline_text(name: &str, o: &Object) -> Option<String> {
    if o.class.name != "inline" {
        return None;
    }
    let (expr, args) = inline_parts(o).ok()?;
    Some(format!("     Inline function:\n     {name}({}) = {expr}", args.join(",")))
}

fn inline_method(it: &mut Interpreter, m: &str, o: &Object, rest: &[Value], nargout: usize) -> R<Vec<Value>> {
    let (expr, args) = inline_parts(o)?;
    Ok(match m {
        "subsref" => {
            let Some(Value::Struct(s)) = rest.first() else { return rt("Invalid subscript structure.") };
            let subs = match s.get("subs") {
                Some(Value::Cell(c)) => c.data.clone(),
                _ => Vec::new(),
            };
            return call_inline(it, o, subs, nargout);
        }
        "feval" => return call_inline(it, o, rest.to_vec(), nargout),
        "argnames" => vec![Value::Cell(CellArray::col(args.iter().map(|a| Value::chars(a)).collect()))],
        "formula" | "char" => vec![Value::chars(&expr)],
        "vectorize" => vec![make_inline(&crate::math_more::vectorize(&expr), args)],
        "nargin" => vec![Value::num(args.len() as f64)],
        _ => return rt(format!("Undefined function '{m}' for input arguments of type 'inline'.")),
    })
}

fn fcnchk(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let f = arg(a, 0, "fcnchk")?;
    if let Value::Func(_) | Value::Object(_) = f {
        return Ok(f.clone());
    }
    let text = f.text()?;
    let vectorized = a.get(1).map(|v| v.text().map(|t| t.eq_ignore_ascii_case("vectorized"))).transpose()?.unwrap_or(false);
    let is_name = text.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if is_name {
        return Ok(it.call_function("str2func", vec![Value::chars(&text)], 1)?.remove(0));
    }
    let args = find_args(&text);
    if vectorized {
        // Constant terms still return one value per element of the first argument.
        let expr = format!("{}.*ones(size({}))", crate::math_more::vectorize(&text), args[0]);
        return Ok(make_inline(&expr, args));
    }
    Ok(make_inline(&text, args))
}

// ---- memoize ---------------------------------------------------------------------------------

struct Entry {
    inputs: Vec<Value>,
    nargout: usize,
    outputs: Vec<Value>,
    hits: f64,
}

#[derive(Default)]
struct Cache {
    entries: Vec<Entry>,
    hits: f64,
    misses: f64,
}

thread_local! {
    /// The MemoizedFunction made for each function, so memoizing it again shares the cache.
    static MEMOIZED: RefCell<Vec<(FuncValue, Value)>> = const { RefCell::new(Vec::new()) };
    static CACHES: RefCell<HashMap<usize, Cache>> = RefCell::new(HashMap::new());
}

fn cache_key(o: &Object) -> usize {
    Rc::as_ptr(&o.props) as *const () as usize
}

fn memoize(a: &[Value]) -> R<Value> {
    let Some(Value::Func(f)) = a.first() else {
        return rt("Input must be a function handle.");
    };
    let same = |g: &FuncValue| match (f, g) {
        (FuncValue::Named(x), FuncValue::Named(y)) => x == y,
        (FuncValue::Anon { f: x, .. }, FuncValue::Anon { f: y, .. }) => Rc::ptr_eq(x, y),
        _ => false,
    };
    if let Some(v) = MEMOIZED.with(|m| m.borrow().iter().find(|(g, _)| same(g)).map(|(_, v)| v.clone())) {
        return Ok(v);
    }
    let v = make(MEMO_CLASS, vec![Value::Func(f.clone()), Value::bool(true), Value::num(10.0)]);
    MEMOIZED.with(|m| m.borrow_mut().push((f.clone(), v.clone())));
    Ok(v)
}

fn call_memoized(it: &mut Interpreter, o: &Object, args: Vec<Value>, nargout: usize) -> R<Vec<Value>> {
    let Value::Func(f) = prop(o, "Function") else { return rt("Invalid MemoizedFunction.") };
    let enabled = prop(o, "Enabled").scalar().unwrap_or(1.0) != 0.0;
    if !enabled {
        return it.call_value(&f, args, nargout);
    }
    let key = cache_key(o);
    let hit = CACHES.with(|c| {
        let mut c = c.borrow_mut();
        let cache = c.entry(key).or_default();
        let found = cache.entries.iter_mut().find(|e| e.inputs == args && e.nargout == nargout)?;
        found.hits += 1.0;
        let out = found.outputs.clone();
        cache.hits += 1.0;
        Some(out)
    });
    if let Some(out) = hit {
        return Ok(out);
    }
    let outputs = it.call_value(&f, args.clone(), nargout)?;
    let size = prop(o, "CacheSize").scalar().unwrap_or(10.0).max(0.0) as usize;
    CACHES.with(|c| {
        let mut c = c.borrow_mut();
        let cache = c.entry(key).or_default();
        cache.misses += 1.0;
        cache.entries.push(Entry { inputs: args, nargout, outputs: outputs.clone(), hits: 0.0 });
        // The oldest entries leave first when the cache is full.
        while cache.entries.len() > size {
            cache.entries.remove(0);
        }
    });
    Ok(outputs)
}

fn struct_of(fields: Vec<(&str, Value)>) -> Value {
    let mut s = StructArray::scalar();
    for (k, v) in fields {
        s.set(k, v);
    }
    Value::Struct(s)
}

fn memo_stats(o: &Object) -> Value {
    let size = prop(o, "CacheSize").scalar().unwrap_or(10.0);
    CACHES.with(|c| {
        let c = c.borrow();
        let empty = Cache::default();
        let cache = c.get(&cache_key(o)).unwrap_or(&empty);
        let n = cache.entries.len();
        let cells = |f: &dyn Fn(&Entry) -> Vec<Value>| -> Value {
            if n == 0 {
                return Value::Cell(CellArray::empty(0, 0));
            }
            Value::Cell(CellArray::row(cache.entries.iter().map(|e| Value::Cell(CellArray::row(f(e)))).collect()))
        };
        let row = |f: &dyn Fn(&Entry) -> f64| -> Value {
            if n == 0 {
                return Value::empty();
            }
            Value::Num(Matrix::row(cache.entries.iter().map(f).collect()))
        };
        let cache_struct = struct_of(vec![
            ("Inputs", cells(&|e| e.inputs.clone())),
            ("Nargout", row(&|e| e.nargout as f64)),
            ("Outputs", cells(&|e| e.outputs.clone())),
            ("HitCount", row(&|e| e.hits)),
            ("TotalHits", Value::num(cache.hits)),
            ("TotalMisses", Value::num(cache.misses)),
        ]);
        let most = cache.entries.iter().filter(|e| e.hits > 0.0).fold(None::<&Entry>, |best, e| match best {
            Some(b) if b.hits >= e.hits => Some(b),
            _ => Some(e),
        });
        let most = match most {
            Some(e) => struct_of(vec![("Hits", Value::num(e.hits)), ("Input", Value::Cell(CellArray::row(e.inputs.clone())))]),
            None => Value::empty(),
        };
        let calls = cache.hits + cache.misses;
        struct_of(vec![
            ("Cache", cache_struct),
            ("MostHitCachedInput", most),
            ("CacheHitRatePercent", Value::num(if calls == 0.0 { 0.0 } else { 100.0 * cache.hits / calls })),
            ("CacheOccupancyPercent", Value::num(if size == 0.0 { 0.0 } else { 100.0 * n as f64 / size })),
        ])
    })
}

fn memo_method(it: &mut Interpreter, m: &str, o: &Object, rest: &[Value], nargout: usize) -> R<Vec<Value>> {
    Ok(match m {
        "subsref" => {
            let Some(Value::Struct(s)) = rest.first() else { return rt("Invalid subscript structure.") };
            let subs = match s.get("subs") {
                Some(Value::Cell(c)) => c.data.clone(),
                _ => Vec::new(),
            };
            return call_memoized(it, o, subs, nargout);
        }
        "stats" => vec![memo_stats(o)],
        "clearCache" => {
            CACHES.with(|c| c.borrow_mut().remove(&cache_key(o)));
            Vec::new()
        }
        _ => return rt(format!("Undefined function '{m}' for input arguments of type '{MEMO_CLASS}'.")),
    })
}

// ---- timeit ----------------------------------------------------------------------------------

/// `timeit(f, nout)`: the median time of repeated calls of `f`, in seconds.
fn timeit(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let Some(Value::Func(f)) = a.first() else {
        return rt("The first input must be a function handle.");
    };
    let nout = match a.get(1) {
        Some(v) => v.scalar()? as usize,
        None => 1,
    };
    let once = |it: &mut Interpreter| -> R<f64> {
        let t = std::time::Instant::now();
        match it.call_value(f, Vec::new(), nout) {
            Ok(_) => {}
            // A function with no outputs is timed with none requested.
            Err(_) if nout == 1 => {
                it.call_value(f, Vec::new(), 0)?;
            }
            Err(e) => return Err(e),
        }
        Ok(t.elapsed().as_secs_f64())
    };
    let first = once(it)?;
    // About a second of runs at most, between 3 and 11 of them.
    let runs = ((1.0 / first.max(1e-9)) as usize).clamp(3, 11);
    let mut times: Vec<f64> = (0..runs).map(|_| once(it)).collect::<R<_>>()?;
    times.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Value::num(times[times.len() / 2]))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    let outs = match name {
        "inline" => vec![inline(a)?],
        "fcnchk" => vec![fcnchk(it, a)?],
        "memoize" => vec![memoize(a)?],
        "clearAllMemoizedCaches" => {
            CACHES.with(|c| c.borrow_mut().clear());
            Vec::new()
        }
        "timeit" => vec![timeit(it, a)?],
        "matlivre_funcobj_method" => {
            let which = arg(a, 0, name)?.text()?;
            let n = arg(a, 1, name)?.scalar()? as usize;
            let Some(Value::Object(o)) = a.get(2) else { return rt("Invalid object.") };
            let (class, m) = which.split_once('.').unwrap_or(("", ""));
            let out = if class == "inline" { inline_method(it, m, o, &a[3..], n)? } else { memo_method(it, m, o, &a[3..], n)? };
            vec![Value::Cell(CellArray::row(out))]
        }
        _ => return Ok(None),
    };
    Ok(Some(outs))
}

/// Methods of the built-in classes here (and the MemoizedFunction class itself), for the
/// coverage checklist.
#[allow(dead_code)]
const MEMO_METHODS: &[&str] = &["stats", "clearCache", "MemoizedFunction"];

/// The display text of the built-in objects shown in their own format (inline functions and
/// random streams); `global` is the current global stream.
pub fn object_text(name: &str, o: &Object, global: Option<u64>) -> Option<String> {
    inline_text(name, o).or_else(|| crate::random::stream_text(global, o)).or_else(|| crate::timers::timer_text(o))
}
