//! `pattern` objects (`digitsPattern`, `wildcardPattern`, boundaries, `+` and `|`, ...) and the
//! text functions that accept them (`contains`, `extract`, `replace`, `split`, ...).
//!
//! A pattern is a value object holding a regular expression (for `fancy-regex`) and the text
//! MATLAB displays for it. Combining patterns builds both; matching compiles the expression.

use std::cell::RefCell;
use std::rc::Rc;

use fancy_regex::Regex;

use crate::builtins::{arg, one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "pattern", "digitsPattern", "lettersPattern", "alphanumericsPattern", "whitespacePattern", "wildcardPattern", "characterListPattern", "optionalPattern",
    "asManyOfPattern", "asFewOfPattern", "possessivePattern", "caseInsensitivePattern", "caseSensitivePattern", "regexpPattern", "namedPattern",
    "maskedPattern", "letterBoundary", "digitBoundary", "alphanumericBoundary", "whitespaceBoundary", "lineBoundary", "textBoundary",
    "lookAheadBoundary", "lookBehindBoundary", "matlivre_pattern_method",
];

/// Text functions that take patterns in place of text.
pub const TEXT_FUNCS: &[&str] = &[
    "contains", "startsWith", "endsWith", "extract", "replace", "split", "count", "erase", "matches", "extractBefore", "extractAfter", "extractBetween",
    "insertBefore", "insertAfter", "strsplit",
];

const CLASS: &str = "pattern";

/// Operator precedence of a pattern's display text: alternation binds loosest.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Prec {
    Or,
    Plus,
    Atom,
}

fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let mut src = String::from("classdef pattern\n  properties (Hidden)\n    Regex\n    Display\n    Prec\n    Name\n  end\n  methods\n");
        for m in ["plus", "or"] {
            src.push_str(&format!("    function varargout = {m}(varargin)\n      varargout = matlivre_pattern_method('{m}', varargin{{:}});\n    end\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

#[derive(Clone)]
struct Pat {
    regex: String,
    display: String,
    prec: Prec,
    name: String,
}

impl Pat {
    fn atom(regex: impl Into<String>, display: impl Into<String>) -> Pat {
        Pat { regex: regex.into(), display: display.into(), prec: Prec::Atom, name: String::new() }
    }

    fn value(self) -> Value {
        let prec = match self.prec {
            Prec::Or => 0.0,
            Prec::Plus => 1.0,
            Prec::Atom => 2.0,
        };
        Value::Object(Object {
            class: class_def(),
            props: Rc::new(RefCell::new(vec![Value::chars(&self.regex), Value::chars(&self.display), Value::num(prec), Value::chars(&self.name)])),
        })
    }

    fn of(o: &Object) -> Pat {
        let get = |n: &str| o.get(n).and_then(|v| v.text().ok()).unwrap_or_default();
        let prec = match o.get("Prec").and_then(|v| v.scalar().ok()).unwrap_or(2.0) as i64 {
            0 => Prec::Or,
            1 => Prec::Plus,
            _ => Prec::Atom,
        };
        Pat { regex: get("Regex"), display: get("Display"), prec, name: get("Name") }
    }

    /// Display text wrapped in parentheses when it binds looser than `at`.
    fn shown_at(&self, at: Prec) -> String {
        if self.prec < at { format!("({})", self.display) } else { self.display.clone() }
    }
}

pub fn is_pattern(v: &Value) -> bool {
    matches!(v, Value::Object(o) if o.class.name == CLASS)
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// A pattern operand: a pattern, or text matched literally (a string array or cellstr gives
/// the alternatives).
fn operand(v: &Value) -> R<Pat> {
    if let Value::Object(o) = v {
        if o.class.name == CLASS {
            return Ok(Pat::of(o));
        }
    }
    let items: Vec<String> = match v {
        Value::Str(s) => s.data.iter().map(|x| x.clone().unwrap_or_default()).collect(),
        Value::Cell(c) => c.data.iter().map(|x| x.text()).collect::<R<_>>()?,
        Value::Num(m) if m.kind == Kind::Char => vec![m.to_string_lossy()],
        other => return rt(format!("Invalid pattern operand of class '{}'.", other.class_name())),
    };
    if items.len() == 1 {
        return Ok(Pat::atom(fancy_regex::escape(&items[0]).into_owned(), quote(&items[0])));
    }
    let regex = items.iter().map(|t| fancy_regex::escape(t).into_owned()).collect::<Vec<_>>().join("|");
    let display = items.iter().map(|t| quote(t)).collect::<Vec<_>>().join(" | ");
    Ok(Pat { regex: format!("(?:{regex})"), display, prec: Prec::Or, name: String::new() })
}

/// `(n)` or `(min, max)` arguments of the counted patterns, as a quantifier and display args.
fn counts(a: &[Value], fname: &str) -> R<Option<(String, String)>> {
    let nums: Vec<f64> = a.iter().filter(|v| !v.is_text()).map(|v| v.scalar()).collect::<R<_>>()?;
    Ok(match nums.as_slice() {
        [] => None,
        [n] => Some((format!("{{{}}}", *n as u64), format!("({})", crate::builtins::num2str_scalar(*n)))),
        [lo, hi] => {
            let hi_q = if hi.is_infinite() { String::new() } else { (*hi as u64).to_string() };
            Some((format!("{{{},{hi_q}}}", *lo as u64), format!("({},{})", crate::builtins::num2str_scalar(*lo), crate::builtins::num2str_scalar(*hi))))
        }
        _ => return rt(format!("Too many input arguments to {fname}.")),
    })
}

/// `digitsPattern`, `lettersPattern`, ...: a character class repeated (one or more by default).
fn class_pattern(name: &str, class: &str, a: &[Value]) -> Ret {
    let pat = match counts(a, name)? {
        None => Pat::atom(format!("{class}+"), name),
        Some((q, args)) => Pat::atom(format!("{class}{q}"), format!("{name}{args}")),
    };
    one(pat.value())
}

/// Characters of `text`, escaped for use inside a regex character class.
fn class_chars(text: &str) -> String {
    text.chars().map(|c| if "\\]^-[".contains(c) { format!("\\{c}") } else { c.to_string() }).collect()
}

fn wildcard(a: &[Value]) -> Ret {
    let mut except: Option<String> = None;
    let mut nums = Vec::new();
    let mut i = 0;
    while i < a.len() {
        if a[i].is_text() && a[i].text()?.eq_ignore_ascii_case("Except") {
            except = Some(match a.get(i + 1) {
                Some(v) => operand_chars(v)?,
                None => return rt("Expected a value after 'Except'."),
            });
            i += 2;
            continue;
        }
        nums.push(a[i].clone());
        i += 1;
    }
    let class = match &except {
        Some(e) => format!("[^{}]", class_chars(e)),
        None => "(?s:.)".into(),
    };
    let except_text = except.as_ref().map(|e| format!("\"Except\",{}", quote(e))).unwrap_or_default();
    let pat = match counts(&nums, "wildcardPattern")? {
        // As few characters as possible.
        None => Pat::atom(format!("{class}*?"), if except.is_some() { format!("wildcardPattern({except_text})") } else { "wildcardPattern".into() }),
        Some((q, args)) => {
            let shown = if except.is_some() { format!("wildcardPattern({},{except_text})", &args[1..args.len() - 1]) } else { format!("wildcardPattern{args}") };
            Pat::atom(format!("{class}{q}?"), shown)
        }
    };
    one(pat.value())
}

fn operand_chars(v: &Value) -> R<String> {
    match v {
        Value::Str(s) => Ok(s.data.iter().map(|x| x.clone().unwrap_or_default()).collect()),
        Value::Cell(c) => Ok(c.data.iter().map(|x| x.text()).collect::<R<Vec<_>>>()?.concat()),
        other => other.text(),
    }
}

/// Boundary patterns: `start`, `end` or `either` side of a run of `class`.
fn boundary(name: &str, class: &str, a: &[Value]) -> Ret {
    let side = match a.first() {
        Some(v) => v.text()?.to_lowercase(),
        None => "either".into(),
    };
    let (s, e) = (format!("(?<!{class})(?={class})"), format!("(?<={class})(?!{class})"));
    let regex = match side.as_str() {
        "start" => s,
        "end" => e,
        "either" => format!("(?:{s}|{e})"),
        _ => return rt(format!("Side must be \"start\", \"end\", or \"either\".")),
    };
    let shown = if a.is_empty() { name.to_string() } else { format!("{name}({})", quote(&side)) };
    one(Pat::atom(regex, shown).value())
}

fn wrap(name: &str, a: &[Value], f: impl Fn(&Pat, &[Value]) -> R<(String, String)>) -> Ret {
    let p = operand(arg(a, 0, name)?)?;
    let (regex, args) = f(&p, &a[1..])?;
    let inner = p.display.clone();
    one(Pat::atom(regex, format!("{name}({inner}{args})")).value())
}

fn repeat(name: &str, a: &[Value], lazy: bool) -> Ret {
    wrap(name, a, |p, rest| {
        let lo = rest.first().map(|v| v.scalar()).transpose()?.unwrap_or(0.0);
        let hi = rest.get(1).map(|v| v.scalar()).transpose()?.unwrap_or(f64::INFINITY);
        let q = if hi.is_infinite() { format!("{{{},}}", lo as u64) } else { format!("{{{},{}}}", lo as u64, hi as u64) };
        let args: String = rest.iter().map(|v| v.scalar().map(|x| format!(",{}", crate::builtins::num2str_scalar(x)))).collect::<R<Vec<_>>>()?.concat();
        Ok((format!("(?:{}){q}{}", p.regex, if lazy { "?" } else { "" }), args))
    })
}

fn combine(op: &str, a: &[Value]) -> Ret {
    let l = operand(arg(a, 0, op)?)?;
    let r = operand(arg(a, 1, op)?)?;
    let pat = if op == "plus" {
        Pat {
            regex: format!("(?:{})(?:{})", l.regex, r.regex),
            display: format!("{} + {}", l.shown_at(Prec::Plus), r.shown_at(Prec::Plus)),
            prec: Prec::Plus,
            name: String::new(),
        }
    } else {
        Pat { regex: format!("(?:{}|{})", l.regex, r.regex), display: format!("{} | {}", l.display, r.display), prec: Prec::Or, name: String::new() }
    };
    one(pat.value())
}

/// `pat = ...` display: `pattern` and the pattern's text.
pub fn display(name: &str, v: &Value) -> Option<String> {
    let Value::Object(o) = v else { return None };
    if o.class.name != CLASS {
        return None;
    }
    let p = Pat::of(o);
    let head = if p.name.is_empty() { "Matching:".to_string() } else { format!("Matching {}:", p.name) };
    Some(format!("{name} = pattern\n  {head}\n\n    {}", p.display))
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "pattern" => {
            let p = operand(arg(a, 0, name)?)?;
            one(p.value())
        }
        "digitsPattern" => class_pattern(name, r"\d", a),
        "lettersPattern" => class_pattern(name, r"\p{L}", a),
        "alphanumericsPattern" => class_pattern(name, r"[\p{L}\p{N}]", a),
        "whitespacePattern" => class_pattern(name, r"\s", a),
        "wildcardPattern" => wildcard(a),
        "characterListPattern" => {
            let chars = operand_chars(arg(a, 0, name)?)?;
            let class = format!("[{}]", class_chars(&chars));
            let pat = match counts(&a[1..], name)? {
                None => Pat::atom(class, format!("{name}({})", quote(&chars))),
                Some((q, args)) => Pat::atom(format!("{class}{q}"), format!("{name}({},{}", quote(&chars), &args[1..])),
            };
            one(pat.value())
        }
        "optionalPattern" => wrap(name, a, |p, _| Ok((format!("(?:{})?", p.regex), String::new()))),
        "asManyOfPattern" => repeat(name, a, false),
        "asFewOfPattern" => repeat(name, a, true),
        "possessivePattern" => wrap(name, a, |p, _| Ok((format!("(?>{})", p.regex), String::new()))),
        "caseInsensitivePattern" => wrap(name, a, |p, _| Ok((format!("(?i:{})", p.regex), String::new()))),
        "caseSensitivePattern" => wrap(name, a, |p, _| Ok((format!("(?-i:{})", p.regex), String::new()))),
        "regexpPattern" => {
            let expr = arg(a, 0, name)?.text()?;
            one(Pat::atom(format!("(?:{})", crate::builtins_strings::translate(&expr)), format!("{name}({})", quote(&expr))).value())
        }
        "namedPattern" | "maskedPattern" => {
            let p = operand(arg(a, 0, name)?)?;
            let label = match a.get(1) {
                Some(v) => v.text()?,
                None => p.display.clone(),
            };
            if name == "maskedPattern" {
                return one(Pat::atom(format!("(?:{})", p.regex), label).value());
            }
            let ok = label.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ok {
                return rt("Pattern names must be valid MATLAB identifiers.");
            }
            one(Pat { regex: format!("(?P<{label}>{})", p.regex), name: label, ..p }.value())
        }
        "letterBoundary" => boundary(name, r"\p{L}", a),
        "digitBoundary" => boundary(name, r"\d", a),
        "alphanumericBoundary" => boundary(name, r"[\p{L}\p{N}]", a),
        "whitespaceBoundary" => boundary(name, r"\s", a),
        "lineBoundary" | "textBoundary" => {
            let side = match a.first() {
                Some(v) => v.text()?.to_lowercase(),
                None => "either".into(),
            };
            let (s, e) = if name == "lineBoundary" { ("(?m:^)", "(?m:$)") } else { (r"\A", r"\z") };
            let regex = match side.as_str() {
                "start" => s.to_string(),
                "end" => e.to_string(),
                "either" => format!("(?:{s}|{e})"),
                _ => return rt("Side must be \"start\", \"end\", or \"either\"."),
            };
            let shown = if a.is_empty() { name.to_string() } else { format!("{name}({})", quote(&side)) };
            one(Pat::atom(regex, shown).value())
        }
        "lookAheadBoundary" => wrap(name, a, |p, _| Ok((format!("(?={})", p.regex), String::new()))),
        "lookBehindBoundary" => wrap(name, a, |p, _| Ok((format!("(?<={})", p.regex), String::new()))),
        "matlivre_pattern_method" => {
            let op = arg(a, 0, name)?.text()?;
            let r = combine(&op, &a[1..])?.unwrap_or_default();
            one(Value::Cell(CellArray::row(r)))
        }
        _ => Ok(None),
    }
}

// ---- text functions with pattern arguments -------------------------------------------------

/// The texts of the first argument, with how to rebuild a result like it.
struct Texts {
    items: Vec<Option<String>>,
    dims: Vec<usize>,
    kind: Container,
}

#[derive(Clone, Copy, PartialEq)]
enum Container {
    Char,
    Str,
    Cell,
}

impl Texts {
    fn of(v: &Value) -> R<Texts> {
        Ok(match v {
            Value::Num(m) if m.kind == Kind::Char || m.is_empty() => Texts { items: vec![Some(m.to_string_lossy())], dims: vec![1, 1], kind: Container::Char },
            Value::Str(s) => Texts { items: s.data.clone(), dims: s.dims.clone(), kind: Container::Str },
            Value::Cell(c) => Texts { items: c.data.iter().map(|x| x.text().map(Some)).collect::<R<_>>()?, dims: c.dims.clone(), kind: Container::Cell },
            other => return rt(format!("First argument must be text, not a value of class '{}'.", other.class_name())),
        })
    }

    /// Text results in the input's container (char stays char for a single text).
    fn rebuild(&self, out: Vec<Option<String>>) -> Value {
        match self.kind {
            Container::Char => Value::chars(out.into_iter().next().flatten().as_deref().unwrap_or("")),
            Container::Str => Value::Str(StrArray::from_dims(self.dims.clone(), out)),
            Container::Cell => Value::Cell(CellArray::from_dims(self.dims.clone(), out.into_iter().map(|t| Value::chars(t.as_deref().unwrap_or(""))).collect())),
        }
    }

    fn logical(&self, out: Vec<bool>) -> Value {
        Value::Num(Matrix::from_dims(self.dims.clone(), out.into_iter().map(|b| if b { 1.0 } else { 0.0 }).collect()).with_kind(Kind::Logical))
    }
}

fn compile(regex: &str, ignore_case: bool) -> R<Regex> {
    let src = if ignore_case { format!("(?i:{regex})") } else { regex.to_string() };
    Regex::new(&src).map_err(|e| LangError::new(format!("Invalid pattern: {e}"), 0))
}

/// Non-overlapping matches (start, end) of `re` in `text`, as byte offsets.
fn find_all(re: &Regex, text: &str) -> R<Vec<(usize, usize)>> {
    let mut out = Vec::new();
    for m in re.find_iter(text) {
        let m = m.map_err(|e| LangError::new(format!("Pattern matching failed: {e}"), 0))?;
        out.push((m.start(), m.end()));
    }
    Ok(out)
}

fn ignore_case_opt(a: &[Value]) -> R<bool> {
    let mut i = 0;
    while i + 1 < a.len() {
        if a[i].is_text() && a[i].text()?.eq_ignore_ascii_case("IgnoreCase") {
            return a[i + 1].is_true();
        }
        i += 1;
    }
    Ok(false)
}

/// `contains(str, pat)` and the other text functions when a pattern is among the arguments.
pub fn text_call(name: &str, a: &[Value]) -> Ret {
    let t = Texts::of(arg(a, 0, name)?)?;
    let pat = |i: usize| -> R<Pat> { operand(arg(a, i, name)?) };
    let ic = ignore_case_opt(a)?;
    match name {
        "contains" | "startsWith" | "endsWith" | "matches" => {
            let p = pat(1)?;
            let regex = match name {
                "contains" => p.regex,
                "startsWith" => format!(r"\A(?:{})", p.regex),
                "endsWith" => format!(r"(?:{})\z", p.regex),
                _ => format!(r"\A(?:{})\z", p.regex),
            };
            let re = compile(&regex, ic)?;
            let out = t.items.iter().map(|s| s.as_deref().map(|s| re.is_match(s).unwrap_or(false)).unwrap_or(false)).collect();
            one(t.logical(out))
        }
        "count" => {
            let re = compile(&pat(1)?.regex, ic)?;
            let out: Vec<f64> = t.items.iter().map(|s| s.as_deref().map(|s| find_all(&re, s).map(|m| m.len() as f64)).transpose().map(|n| n.unwrap_or(0.0))).collect::<R<_>>()?;
            one(Value::Num(Matrix::from_dims(t.dims.clone(), out)))
        }
        "extract" => {
            let re = compile(&pat(1)?.regex, false)?;
            let per: Vec<Vec<String>> = t
                .items
                .iter()
                .map(|s| Ok(s.as_deref().map(|s| find_all(&re, s).map(|m| m.into_iter().map(|(b, e)| s[b..e].to_string()).collect())).transpose()?.unwrap_or_default()))
                .collect::<R<_>>()?;
            if t.items.len() == 1 {
                let found = per.into_iter().next().unwrap_or_default();
                let n = found.len();
                // Char input gives strings too, as MATLAB's extract does; a cell gives a cell.
                return one(match t.kind {
                    Container::Str | Container::Char if n == 1 => Value::string(found[0].clone()),
                    Container::Str | Container::Char => Value::Str(StrArray::from_dims(vec![n, 1], found.into_iter().map(Some).collect())),
                    _ if n == 1 => Value::Cell(CellArray::new(1, 1, vec![Value::chars(&found[0])])),
                    _ => Value::Cell(CellArray::from_dims(vec![n, 1], found.iter().map(|s| Value::chars(s)).collect())),
                });
            }
            if per.iter().any(|m| m.len() != 1) {
                return rt("Unable to extract from a text array where elements have different numbers of matches. Extract from one element at a time.");
            }
            one(t.rebuild(per.into_iter().map(|m| m.into_iter().next()).collect()))
        }
        "replace" | "erase" => {
            let re = compile(&pat(1)?.regex, false)?;
            let new = if name == "erase" { String::new() } else { arg(a, 2, name)?.text()? };
            let out = t
                .items
                .iter()
                .map(|s| {
                    let Some(s) = s else { return Ok(None) };
                    let mut r = String::new();
                    let mut last = 0;
                    for (b, e) in find_all(&re, s)? {
                        r.push_str(&s[last..b]);
                        r.push_str(&new);
                        last = e;
                    }
                    r.push_str(&s[last..]);
                    Ok(Some(r))
                })
                .collect::<R<_>>()?;
            one(t.rebuild(out))
        }
        "split" | "strsplit" => {
            if t.items.len() != 1 {
                return rt("Splitting a text array by a pattern is supported one element at a time.");
            }
            let re = compile(&pat(1)?.regex, false)?;
            let s = t.items[0].clone().unwrap_or_default();
            let mut parts = Vec::new();
            let mut last = 0;
            for (b, e) in find_all(&re, &s)? {
                if e == b {
                    continue;
                }
                parts.push(s[last..b].to_string());
                last = e;
            }
            parts.push(s[last..].to_string());
            let n = parts.len();
            one(match (t.kind, name) {
                (_, "strsplit") => Value::Cell(CellArray::row(parts.iter().map(|p| Value::chars(p)).collect())),
                (Container::Str, _) => Value::Str(StrArray::from_dims(vec![n, 1], parts.into_iter().map(Some).collect())),
                _ => Value::Cell(CellArray::from_dims(vec![n, 1], parts.iter().map(|p| Value::chars(p)).collect())),
            })
        }
        "extractBefore" | "extractAfter" | "insertBefore" | "insertAfter" => {
            let re = compile(&pat(1)?.regex, false)?;
            let ins = if name.starts_with("insert") { Some(arg(a, 2, name)?.text()?) } else { None };
            let out = t
                .items
                .iter()
                .map(|s| {
                    let Some(s) = s else { return Ok(None) };
                    let m = find_all(&re, s)?;
                    Ok(match (name, m.first()) {
                        ("extractBefore", Some(&(b, _))) => Some(s[..b].to_string()),
                        ("extractAfter", Some(&(_, e))) => Some(s[e..].to_string()),
                        ("insertBefore", _) => {
                            let ins = ins.as_deref().unwrap_or("");
                            let mut r = String::new();
                            let mut last = 0;
                            for &(b, e) in &m {
                                r.push_str(&s[last..b]);
                                r.push_str(ins);
                                r.push_str(&s[b..e]);
                                last = e;
                            }
                            r.push_str(&s[last..]);
                            Some(r)
                        }
                        ("insertAfter", _) => {
                            let ins = ins.as_deref().unwrap_or("");
                            let mut r = String::new();
                            let mut last = 0;
                            for &(_, e) in &m {
                                r.push_str(&s[last..e]);
                                r.push_str(ins);
                                last = e;
                            }
                            r.push_str(&s[last..]);
                            Some(r)
                        }
                        _ => None,
                    })
                })
                .collect::<R<Vec<_>>>()?;
            if t.kind == Container::Char {
                return one(Value::chars(out.into_iter().next().flatten().as_deref().unwrap_or("")));
            }
            one(t.rebuild(out))
        }
        "extractBetween" => {
            let start = compile(&pat(1)?.regex, false)?;
            let end = compile(&pat(2)?.regex, false)?;
            let inclusive = a.iter().skip(3).any(|v| v.text().map(|t| t.eq_ignore_ascii_case("inclusive")).unwrap_or(false));
            let per: Vec<Vec<String>> = t
                .items
                .iter()
                .map(|s| {
                    let Some(s) = s else { return Ok(Vec::new()) };
                    let mut found = Vec::new();
                    let mut pos = 0;
                    while pos <= s.len() {
                        let Some(m) = start.find_from_pos(s, pos).map_err(|e| LangError::new(e.to_string(), 0))? else { break };
                        let Some(n) = end.find_from_pos(s, m.end()).map_err(|e| LangError::new(e.to_string(), 0))? else { break };
                        found.push(if inclusive { s[m.start()..n.end()].to_string() } else { s[m.end()..n.start()].to_string() });
                        pos = n.end().max(m.end() + 1);
                    }
                    Ok(found)
                })
                .collect::<R<_>>()?;
            let found = per.into_iter().next().unwrap_or_default();
            let n = found.len();
            one(match t.kind {
                Container::Str if n == 1 => Value::string(found[0].clone()),
                Container::Str => Value::Str(StrArray::from_dims(vec![n, 1], found.into_iter().map(Some).collect())),
                _ => Value::Cell(CellArray::from_dims(vec![n, 1], found.iter().map(|s| Value::chars(s)).collect())),
            })
        }
        _ => Ok(None),
    }
}
