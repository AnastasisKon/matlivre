//! Formula tools: `combine`, `root`, `sympref`, `mathml`, `eliminate`, `displayFormula`,
//! `changeIntegrationVariable` and `integrateByParts`.

use std::cell::RefCell;

use super::expr::{Const, Num, Rel};
use super::{algebra, calculus, print, Expr, SymArray};
use crate::cplx::C;
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["combine", "root", "sympref", "mathml", "eliminate", "displayFormula", "changeIntegrationVariable", "integrateByParts"];

fn not_enough() -> LangError {
    LangError::new("Not enough input arguments.", 0)
}

fn sym_arg(a: &[Value], i: usize) -> R<SymArray> {
    SymArray::from_value(a.get(i).ok_or_else(not_enough)?)
}

fn scalar(a: &[Value], i: usize) -> R<Expr> {
    let s = sym_arg(a, i)?;
    if s.data.len() != 1 {
        return rt("A scalar symbolic expression is required.");
    }
    Ok(s.data.into_iter().next().unwrap())
}

fn var_of(v: &Value) -> R<String> {
    match SymArray::from_value(v)?.data.as_slice() {
        [Expr::Var(n)] => Ok(n.clone()),
        _ => rt("The variable must be a symbolic variable."),
    }
}

fn sym(e: Expr) -> Value {
    Value::Sym(SymArray::scalar(e))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> R<Option<Vec<Value>>> {
    let one = |v: Value| Ok(Some(vec![v]));
    match name {
        "combine" => {
            let s = sym_arg(a, 0)?;
            let mut target = "powers".to_string();
            let mut iac = false;
            let mut i = 1;
            while i < a.len() {
                let t = a[i].text()?;
                if t.eq_ignore_ascii_case("IgnoreAnalyticConstraints") {
                    iac = a.get(i + 1).map(|v| v.is_true().unwrap_or(false)).unwrap_or(false);
                    i += 2;
                } else {
                    target = t;
                    i += 1;
                }
            }
            if !matches!(target.as_str(), "powers" | "log" | "exp" | "sincos" | "sinhcosh" | "gamma" | "atan" | "int") {
                return rt(format!("Invalid target '{target}'."));
            }
            one(Value::Sym(s.map(|e| combine(e, &target, iac))))
        }
        "root" => {
            let p = scalar(a, 0)?;
            let v = match a.get(1) {
                Some(v) => var_of(v)?,
                None => super::default_var(&p).unwrap_or_else(|| "x".into()),
            };
            let deg = algebra::poly_coeffs(&algebra::expand(&p), &v).map(|c| c.len().saturating_sub(1)).ok_or_else(|| LangError::new("The first argument must be a polynomial.", 0))?;
            let make = |k: usize| Expr::Fn("root".into(), vec![p.clone(), Expr::var(&v), Expr::int(k as i128)]);
            if let Some(k) = a.get(2) {
                let k = k.as_usize()?;
                if k < 1 || k > deg {
                    return rt("Root index must be a positive integer not exceeding the polynomial degree.");
                }
                return one(sym(make(k)));
            }
            one(Value::Sym(SymArray::from_dims(vec![deg, if deg == 0 { 0 } else { 1 }], (1..=deg).map(make).collect())))
        }
        "sympref" => sympref(a),
        "mathml" => {
            let e = scalar(a, 0)?;
            let mut lines = vec!["<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>".to_string()];
            mml(&e, 1, &mut lines);
            lines.push("</math>".into());
            one(Value::chars(&(lines.join("\n") + "\n")))
        }
        "eliminate" => {
            let eqs = sym_arg(a, 0)?;
            let elim = SymArray::from_value(a.get(1).ok_or_else(not_enough)?)?
                .data
                .iter()
                .map(|e| match e {
                    Expr::Var(n) => Ok(n.clone()),
                    _ => rt("Variables must be symbolic variables."),
                })
                .collect::<R<Vec<_>>>()?;
            let polys: Vec<Expr> = eqs
                .data
                .iter()
                .map(|e| match e {
                    Expr::Rel(Rel::Eq, l, r) => Expr::sub((**l).clone(), (**r).clone()),
                    other => other.clone(),
                })
                .collect();
            let mut rest = Vec::new();
            for p in &polys {
                p.free_vars(&mut rest);
            }
            rest.retain(|v| !elim.contains(v));
            rest.sort();
            let mut order = elim.clone();
            order.extend(rest);
            let basis = super::forms::gbasis_lex(&polys, &order)?;
            let kept: Vec<Expr> = basis.into_iter().filter(|g| !elim.iter().any(|v| g.contains_var(v))).collect();
            let n = kept.len();
            one(Value::Sym(SymArray::from_dims(vec![1, n], kept)))
        }
        "displayFormula" => {
            let v = a.first().ok_or_else(not_enough)?;
            let texts: Vec<String> = match v {
                Value::Str(s) => s.data.iter().map(|x| x.clone().unwrap_or_default()).collect(),
                other => vec![other.text()?],
            };
            for t in texts {
                let e = super::parse_sym_text(&t)?;
                it.emit(OutputKind::Text(format!("{e}\n")));
            }
            Ok(Some(Vec::new()))
        }
        "changeIntegrationVariable" => {
            let f = scalar(a, 0)?;
            let old = scalar(a, 1)?;
            let new = scalar(a, 2)?;
            one(sym(change_variable(&f, &old, &new)?))
        }
        "integrateByParts" => {
            let f = scalar(a, 0)?;
            let du = scalar(a, 1)?;
            one(sym(by_parts(&f, &du)))
        }
        _ => Ok(None),
    }
}

// ---- combine ----------------------------------------------------------------------------------

fn combine(e: &Expr, target: &str, iac: bool) -> Expr {
    match target {
        "log" if iac => e.map(&|x| match x {
            Expr::Add(ts) => {
                let (logs, rest): (Vec<Expr>, Vec<Expr>) = ts.iter().cloned().partition(|t| log_term(t).is_some());
                if logs.len() < 2 {
                    return None;
                }
                let arg = Expr::mul(logs.iter().map(|t| log_term(t).unwrap()).collect());
                let mut out = rest;
                out.push(Expr::func("log", vec![arg]));
                Some(Expr::add(out))
            }
            _ => None,
        }),
        "exp" => e.map(&|x| match x {
            Expr::Mul(fs) => {
                let (exps, rest): (Vec<Expr>, Vec<Expr>) = fs.iter().cloned().partition(|f| matches!(f, Expr::Fn(n, a) if n == "exp" && a.len() == 1));
                if exps.len() < 2 {
                    return None;
                }
                let arg = Expr::add(exps.iter().map(|f| if let Expr::Fn(_, a) = f { a[0].clone() } else { unreachable!() }).collect());
                let mut out = rest;
                out.push(Expr::func("exp", vec![arg]));
                Some(Expr::mul(out))
            }
            _ => None,
        }),
        "sincos" => {
            let ex = algebra::expand(e);
            let terms = match ex {
                Expr::Add(ts) => ts,
                other => vec![other],
            };
            algebra::expand(&Expr::add(terms.iter().map(sincos_term).collect()))
        }
        _ => e.clone(),
    }
}

/// c*log(u) with a numeric c as log(u^c) (an integer c only), or log(u).
fn log_term(t: &Expr) -> Option<Expr> {
    match t {
        Expr::Fn(n, a) if n == "log" && a.len() == 1 => Some(a[0].clone()),
        Expr::Mul(fs) if fs.len() == 2 => match (&fs[0], &fs[1]) {
            (Expr::Num(c), Expr::Fn(n, a)) if n == "log" && c.is_int() => Some(Expr::pow(a[0].clone(), Expr::Num(c.clone()))),
            _ => None,
        },
        _ => None,
    }
}

/// Products and powers of sines and cosines as sums (2 sin x cos x = sin 2x).
fn sincos_term(t: &Expr) -> Expr {
    let fs: Vec<Expr> = match t {
        Expr::Mul(fs) => fs.clone(),
        other => vec![other.clone()],
    };
    // Expand powers of sin/cos into repeated factors.
    let mut trig: Vec<(bool, Expr)> = Vec::new(); // (is_sin, argument)
    let mut rest = Vec::new();
    for f in fs {
        match &f {
            Expr::Fn(n, a) if (n == "sin" || n == "cos") && a.len() == 1 => trig.push((n == "sin", a[0].clone())),
            Expr::Pow(b, x) => match (&**b, x.as_num().and_then(|k| k.as_int())) {
                (Expr::Fn(n, a), Some(k)) if (n == "sin" || n == "cos") && a.len() == 1 && (1..=8).contains(&k) => {
                    for _ in 0..k {
                        trig.push((n == "sin", a[0].clone()));
                    }
                }
                _ => rest.push(f),
            },
            _ => rest.push(f),
        }
    }
    if trig.len() < 2 {
        return t.clone();
    }
    // Reduce pairwise: the product of two trig factors is half a sum.
    let mut acc: Vec<(Expr, bool, Expr)> = vec![(Expr::one(), trig[0].0, trig[0].1.clone())]; // coeff * (sin|cos)(arg)
    for (is_sin, b) in trig.into_iter().skip(1) {
        let mut next = Vec::new();
        for (c, s, a) in acc {
            let half = Expr::mul(vec![Expr::rat(1, 2), c]);
            let (sum, diff) = (algebra::simplify(&Expr::add(vec![a.clone(), b.clone()])), algebra::simplify(&Expr::sub(a.clone(), b.clone())));
            match (s, is_sin) {
                // sin a sin b = (cos(a-b) - cos(a+b))/2
                (true, true) => {
                    next.push((half.clone(), false, diff));
                    next.push((Expr::neg(half), false, sum));
                }
                // cos a cos b = (cos(a-b) + cos(a+b))/2
                (false, false) => {
                    next.push((half.clone(), false, diff));
                    next.push((half, false, sum));
                }
                // sin a cos b = (sin(a+b) + sin(a-b))/2
                (true, false) => {
                    next.push((half.clone(), true, sum));
                    next.push((half, true, diff));
                }
                // cos a sin b = (sin(a+b) - sin(a-b))/2
                (false, true) => {
                    next.push((half.clone(), true, sum));
                    next.push((Expr::neg(half), true, diff));
                }
            }
        }
        acc = next;
    }
    let mut out = Vec::new();
    for (c, s, a) in acc {
        out.push(Expr::mul(vec![c, Expr::func(if s { "sin" } else { "cos" }, vec![a])]));
    }
    Expr::mul(vec![Expr::mul(rest), Expr::add(out)])
}

// ---- root -------------------------------------------------------------------------------------

/// The numeric value of root(p, x, k): roots ordered real first (ascending), then complex by
/// real and imaginary part.
pub fn root_value(xs: &[Expr], env: &dyn Fn(&str) -> Option<C>) -> Option<C> {
    let [p, Expr::Var(v), k] = xs else { return None };
    let k = k.as_num()?.as_int()? as usize;
    let coeffs = algebra::poly_coeffs(&algebra::expand(p), v)?;
    let c: Vec<C> = coeffs.iter().map(|e| e.eval_c(env)).collect::<Option<_>>()?;
    let desc: Vec<C> = c.iter().rev().copied().collect();
    let mut roots = crate::cplx::poly_roots(&desc);
    roots.sort_by(|a, b| {
        let ra = a.im.abs() < 1e-12;
        let rb = b.im.abs() < 1e-12;
        rb.cmp(&ra).then(a.re.partial_cmp(&b.re).unwrap_or(std::cmp::Ordering::Equal)).then(a.im.partial_cmp(&b.im).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut z = *roots.get(k.checked_sub(1)?)?;
    if z.im.abs() < 1e-12 * z.re.abs().max(1.0) {
        z.im = 0.0;
    }
    Some(z)
}

/// vpa(root(p, x, k)) for a real root of a polynomial with rational coefficients: the double
/// root refined by Newton steps in exact arithmetic.
pub fn root_vpa(e: &Expr) -> Option<Expr> {
    let Expr::Fn(n, xs) = e else { return None };
    if n != "root" || xs.len() != 3 {
        return None;
    }
    let z = root_value(xs, &|_| None)?;
    if z.im != 0.0 {
        return None;
    }
    let Expr::Var(v) = &xs[1] else { return None };
    let c: Vec<num_rational::BigRational> = algebra::poly_coeffs(&algebra::expand(&xs[0]), v)?.iter().map(|e| e.as_num().and_then(|n| n.exact())).collect::<Option<_>>()?;
    let d: Vec<num_rational::BigRational> = c.iter().enumerate().skip(1).map(|(k, a)| a * num_rational::BigRational::from_integer((k as i64).into())).collect();
    let eval = |p: &[num_rational::BigRational], x: &num_rational::BigRational| p.iter().rev().fold(num_rational::BigRational::from_integer(0.into()), |acc, a| acc * x + a);
    let digits = super::expr::digits();
    let mut x = num_rational::BigRational::from_float(z.re)?;
    for _ in 0..12 {
        let dp = eval(&d, &x);
        if dp == num_rational::BigRational::from_integer(0.into()) {
            break;
        }
        x = super::expr::round_sig(&(&x - eval(&c, &x) / dp), digits + 20);
    }
    Some(Expr::Num(Num::vpa(&x)))
}

// ---- sympref ----------------------------------------------------------------------------------

const PREFS: &[&str] = &["FourierParameters", "HeavisideAtOrigin", "AbbreviateOutput", "TypesetOutput", "FloatingPointOutput", "PolynomialDisplayStyle", "MatrixWithSquareBrackets"];

thread_local! {
    static PREF_VALUES: RefCell<Option<Vec<Value>>> = const { RefCell::new(None) };
}

fn defaults() -> Vec<Value> {
    vec![
        Value::Num(Matrix::row(vec![1.0, -1.0])),
        sym(Expr::rat(1, 2)),
        Value::bool(true),
        Value::bool(true),
        Value::bool(false),
        Value::chars("default"),
        Value::bool(false),
    ]
}

fn with_prefs<T>(f: impl FnOnce(&mut Vec<Value>) -> T) -> T {
    PREF_VALUES.with(|p| f(p.borrow_mut().get_or_insert_with(defaults)))
}

/// Whether `sympref('FloatingPointOutput')` is on: numbers display in format short.
pub fn floating_point_output() -> bool {
    with_prefs(|p| p[4].is_true().unwrap_or(false))
}

/// The value of `sympref('HeavisideAtOrigin')`.
pub fn heaviside_at_origin() -> Expr {
    with_prefs(|p| SymArray::from_value(&p[1]).ok().and_then(|s| s.data.into_iter().next()).unwrap_or_else(|| Expr::rat(1, 2)))
}

fn prefs_struct(vals: &[Value]) -> Value {
    let mut s = StructArray::scalar();
    for (n, v) in PREFS.iter().zip(vals) {
        s.set(n, v.clone());
    }
    Value::Struct(s)
}

fn sympref(a: &[Value]) -> R<Option<Vec<Value>>> {
    match a {
        [] => Ok(Some(vec![with_prefs(|p| prefs_struct(p))])),
        [v] if matches!(v, Value::Struct(_)) => {
            let Value::Struct(s) = v else { unreachable!() };
            let old = with_prefs(|p| prefs_struct(p));
            for (k, n) in PREFS.iter().enumerate() {
                if let Some(x) = s.get(n) {
                    with_prefs(|p| p[k] = x.clone());
                }
            }
            Ok(Some(vec![old]))
        }
        [name, rest @ ..] => {
            let n = name.text()?;
            if n.eq_ignore_ascii_case("default") {
                let old = with_prefs(|p| prefs_struct(p));
                with_prefs(|p| *p = defaults());
                return Ok(Some(vec![old]));
            }
            let k = PREFS.iter().position(|p| p.eq_ignore_ascii_case(&n)).ok_or_else(|| LangError::new(format!("'{n}' is not a valid symbolic preference."), 0))?;
            let old = with_prefs(|p| p[k].clone());
            if let Some(v) = rest.first() {
                let value = match v.text().ok().as_deref() {
                    Some("default") if !matches!(v, Value::Sym(_)) => defaults()[k].clone(),
                    _ => match k {
                        1 => Value::Sym(SymArray::from_value(v)?),
                        2 | 3 | 4 | 6 => Value::bool(v.is_true()?),
                        _ => v.clone(),
                    },
                };
                with_prefs(|p| p[k] = value);
            }
            Ok(Some(vec![old]))
        }
    }
}

// ---- mathml -----------------------------------------------------------------------------------

fn put(lines: &mut Vec<String>, depth: usize, s: &str) {
    lines.push(format!("{}{s}", "  ".repeat(depth)));
}

fn open_close(lines: &mut Vec<String>, depth: usize, tag: &str, body: impl FnOnce(&mut Vec<String>)) {
    put(lines, depth, &format!("<{tag}>"));
    body(lines);
    put(lines, depth, &format!("</{tag}>"));
}

const MI_FUNCTIONS: &[&str] = &["sin", "cos", "tan", "cot", "sec", "csc", "sinh", "cosh", "tanh", "coth", "log", "ln", "exp", "arcsin", "arccos", "arctan"];

fn mml(e: &Expr, d: usize, out: &mut Vec<String>) {
    match e {
        Expr::Var(v) => put(out, d, &format!("<mi>{v}</mi>")),
        Expr::Const(Const::Pi) => put(out, d, "<mi>&pi;</mi>"),
        Expr::Const(Const::I) => put(out, d, "<mo>&ii;</mo>"),
        Expr::Const(Const::Inf) => put(out, d, "<mi>&infin;</mi>"),
        Expr::Const(Const::NaN) => put(out, d, "<mi>NaN</mi>"),
        Expr::Num(n) => {
            if n.is_neg() {
                open_close(out, d, "mrow", |o| {
                    put(o, d + 1, "<mo>-</mo>");
                    mml(&Expr::Num(n.neg()), d + 1, o);
                });
            } else if n.is_fraction() {
                let t = e.to_string();
                let (p, q) = t.split_once('/').unwrap_or((&t, "1"));
                let (p, q) = (p.to_string(), q.to_string());
                open_close(out, d, "mfrac", |o| {
                    put(o, d + 1, &format!("<mn>{p}</mn>"));
                    put(o, d + 1, &format!("<mn>{q}</mn>"));
                });
            } else {
                put(out, d, &format!("<mn>{e}</mn>"));
            }
        }
        Expr::Add(ts) => {
            let ordered = print::ordered_terms(ts);
            open_close(out, d, "mrow", |o| {
                for (k, t) in ordered.iter().enumerate() {
                    let negative = t.split_coeff().0.is_neg();
                    let body = if negative { Expr::neg(t.clone()) } else { t.clone() };
                    if k > 0 || negative {
                        put(o, d + 1, if negative { "<mo>-</mo>" } else { "<mo>+</mo>" });
                    }
                    mml(&body, d + 1, o);
                }
            });
        }
        Expr::Rel(op, l, r) => {
            let sym = match op {
                Rel::Eq => "&equals;",
                Rel::Ne => "&ne;",
                Rel::Lt => "&lt;",
                Rel::Le => "&le;",
                Rel::Gt => "&gt;",
                Rel::Ge => "&ge;",
            };
            open_close(out, d, "mrow", |o| {
                mml(l, d + 1, o);
                put(o, d + 1, &format!("<mo form='infix'>{sym}</mo>"));
                mml(r, d + 1, o);
            });
        }
        Expr::Pow(b, x) => {
            if let Some(n) = x.as_num() {
                if n.is_neg() {
                    let pos = Expr::pow((**b).clone(), Expr::Num(n.neg()));
                    return open_close(out, d, "mfrac", |o| {
                        put(o, d + 1, "<mn>1</mn>");
                        mml(&pos, d + 1, o);
                    });
                }
                if matches!(n, Num::Rat(1, 2)) {
                    return open_close(out, d, "msqrt", |o| mml(b, d + 1, o));
                }
            }
            open_close(out, d, "msup", |o| {
                fenced(b, d + 1, o);
                match x.as_num() {
                    Some(n) if n.is_fraction() => {
                        let t = x.to_string();
                        let (p, q) = t.split_once('/').unwrap_or((&t, "1"));
                        let (p, q) = (p.to_string(), q.to_string());
                        open_close(o, d + 1, "mrow", |o2| {
                            put(o2, d + 2, &format!("<mn>{p}</mn>"));
                            put(o2, d + 2, "<mo>/</mo>");
                            put(o2, d + 2, &format!("<mn>{q}</mn>"));
                        });
                    }
                    _ => mml(x, d + 1, o),
                }
            });
        }
        Expr::Mul(_) => {
            let (coeff, num, den) = print::fraction(e);
            if coeff.is_neg() {
                let pos = Expr::neg(e.clone());
                return open_close(out, d, "mrow", |o| {
                    put(o, d + 1, "<mo>-</mo>");
                    mml(&pos, d + 1, o);
                });
            }
            let (cp, cq) = match &coeff {
                Num::Rat(p, q) => (*p, *q),
                _ => (1, 1),
            };
            let mut nf: Vec<Expr> = Vec::new();
            if cp != 1 || num.is_empty() || !matches!(coeff, Num::Rat(..)) {
                nf.push(if matches!(coeff, Num::Rat(..)) { Expr::int(cp) } else { Expr::Num(coeff.clone()) });
            }
            nf.extend(num);
            let mut df: Vec<Expr> = Vec::new();
            if cq != 1 {
                df.push(Expr::int(cq));
            }
            df.extend(den);
            let product = |fs: &[Expr], d: usize, o: &mut Vec<String>| {
                if fs.len() == 1 {
                    fenced(&fs[0], d, o);
                    return;
                }
                open_close(o, d, "mrow", |o2| {
                    for (k, f) in fs.iter().enumerate() {
                        if k > 0 {
                            put(o2, d + 1, "<mo form='infix'>&InvisibleTimes;</mo>");
                        }
                        fenced(f, d + 1, o2);
                    }
                });
            };
            if df.is_empty() {
                product(&nf, d, out);
            } else {
                open_close(out, d, "mfrac", |o| {
                    product(&nf, d + 1, o);
                    product(&df, d + 1, o);
                });
            }
        }
        Expr::Fn(name, args) => {
            if name == "exp" && args.len() == 1 {
                return open_close(out, d, "msup", |o| {
                    put(o, d + 1, "<mo>&ee;</mo>");
                    mml(&args[0], d + 1, o);
                });
            }
            let tag = if MI_FUNCTIONS.contains(&name.as_str()) { "mi" } else { "mtext" };
            open_close(out, d, "mrow", |o| {
                put(o, d + 1, &format!("<{tag}>{name}</{tag}>"));
                open_close(o, d + 1, "mrow", |o2| {
                    put(o2, d + 2, "<mo form='prefix'>(</mo>");
                    open_close(o2, d + 2, "mrow", |o3| {
                        for (k, x) in args.iter().enumerate() {
                            if k > 0 {
                                put(o3, d + 3, "<mo form='infix'>,</mo>");
                            }
                            mml(x, d + 3, o3);
                        }
                    });
                    put(o2, d + 2, "<mo form='postfix'>)</mo>");
                });
            });
        }
    }
}

/// A sum inside a product or a power is fenced.
fn fenced(e: &Expr, d: usize, out: &mut Vec<String>) {
    if matches!(e, Expr::Add(_)) {
        open_close(out, d, "mfenced", |o| mml(e, d + 1, o));
    } else {
        mml(e, d, out);
    }
}

// ---- integrals --------------------------------------------------------------------------------

fn change_variable(f: &Expr, old: &Expr, new: &Expr) -> R<Expr> {
    let Expr::Fn(n, args) = f else { return Ok(f.clone()) };
    if n != "int" || args.len() < 2 {
        return Ok(f.clone());
    }
    let Expr::Var(x) = &args[1] else { return rt("The integration variable must be a symbolic variable.") };
    let Expr::Var(o) = old else { return rt("The old variable must be a symbolic variable.") };
    if o != x {
        return rt("The second argument must be the integration variable.");
    }
    let mut vs = Vec::new();
    new.free_vars(&mut vs);
    vs.retain(|v| v != x);
    let z = vs.into_iter().next().ok_or_else(|| LangError::new("The new expression must contain a new variable.", 0))?;
    // x = g(z): f(g(z)) g'(z) dz, limits through the inverse of g.
    let integrand = Expr::mul(vec![args[0].subs(x, new), calculus::diff(new, &z)]);
    let mut out = vec![integrand, Expr::var(&z)];
    if args.len() == 4 {
        for lim in &args[2..4] {
            let r = algebra::solve_one(&Expr::sub(new.clone(), lim.clone()), &z).and_then(|r| r.into_iter().next()).ok_or_else(|| LangError::new("Unable to change the integration limits.", 0))?;
            out.push(algebra::simplify(&r));
        }
    }
    Ok(Expr::Fn("int".into(), out))
}

fn by_parts(f: &Expr, du: &Expr) -> Expr {
    let Expr::Fn(n, args) = f else { return f.clone() };
    if n != "int" || args.len() < 2 {
        return f.clone();
    }
    let Expr::Var(x) = &args[1] else { return f.clone() };
    // ∫ u dv = u v - ∫ u' v, with dv = du (the part integrated) and u = f/du.
    let u = algebra::simplify(&Expr::div(args[0].clone(), du.clone()));
    let v = calculus::int_indef(du, x);
    let rest = algebra::simplify(&Expr::mul(vec![calculus::diff(&u, x), v.clone()]));
    let uv = Expr::mul(vec![u, v]);
    if args.len() == 4 {
        let (a, b) = (&args[2], &args[3]);
        let bound = Expr::sub(uv.subs(x, b), uv.subs(x, a));
        return Expr::sub(algebra::simplify(&bound), Expr::Fn("int".into(), vec![rest, args[1].clone(), a.clone(), b.clone()]));
    }
    Expr::sub(uv, Expr::Fn("int".into(), vec![rest, args[1].clone()]))
}
