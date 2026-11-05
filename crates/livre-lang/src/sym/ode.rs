//! Differential equations with abstract functions (`syms y(t)`): `dsolve` for linear equations
//! (constant coefficients, or first order with variable coefficients), `odeToVectorField` and
//! `functionalDerivative`.

use super::expr::{Const, Rel};
use super::{algebra, calculus, Expr, SymArray};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["dsolve", "odeToVectorField", "functionalDerivative"];

fn not_enough() -> LangError {
    LangError::new("Not enough input arguments.", 0)
}

fn is_zero(e: &Expr) -> bool {
    e.is_zero() || algebra::simplify(e).is_zero()
}

/// The abstract function in an equation: (name, variable) from y(t) or diff(y(t), t, ...).
fn unknown(e: &Expr) -> Option<(String, String)> {
    match e {
        Expr::Fn(n, a) if n == "diff" => match a.first() {
            Some(Expr::Fn(f, fa)) if fa.len() == 1 => match &fa[0] {
                Expr::Var(t) => Some((f.clone(), t.clone())),
                _ => None,
            },
            _ => None,
        },
        Expr::Add(xs) | Expr::Mul(xs) | Expr::Fn(_, xs) => xs.iter().find_map(unknown),
        Expr::Pow(a, b) | Expr::Rel(_, a, b) => unknown(a).or_else(|| unknown(b)),
        _ => None,
    }
}

fn slot(k: usize) -> String {
    format!("\u{0}d{k}")
}

fn is_fn_of(e: &Expr, y: &str, t: &str) -> bool {
    matches!(e, Expr::Fn(n, a) if n == y && a.len() == 1 && matches!(&a[0], Expr::Var(v) if v == t))
}

/// y(t), diff(y(t), t), ... replaced by placeholders d0, d1, ...; also the highest order.
fn to_slots(e: &Expr, y: &str, t: &str) -> (Expr, usize) {
    let order = std::cell::Cell::new(0usize);
    let out = e.map(&|x| match x {
        Expr::Fn(n, a) if n == "diff" && a.first().is_some_and(|f| is_fn_of(f, y, t)) && a[1..].iter().all(|v| matches!(v, Expr::Var(s) if s == t)) => {
            let k = a.len() - 1;
            order.set(order.get().max(k));
            Some(Expr::var(&slot(k)))
        }
        other if is_fn_of(other, y, t) => Some(Expr::var(&slot(0))),
        _ => None,
    });
    (out, order.get())
}

fn deriv_expr(y: &str, t: &str, k: usize) -> Expr {
    let f = Expr::Fn(y.into(), vec![Expr::var(t)]);
    if k == 0 {
        return f;
    }
    let mut a = vec![f];
    a.extend((0..k).map(|_| Expr::var(t)));
    Expr::Fn("diff".into(), a)
}

fn from_slots(e: &Expr, y: &str, t: &str, n: usize) -> Expr {
    let mut out = e.clone();
    for k in 0..=n {
        out = out.subs(&slot(k), &deriv_expr(y, t, k));
    }
    out
}

/// Coefficients a_0..a_n of y, y', ..., y^(n) and the rest f: sum a_k y^(k) + f = 0.
fn linear_form(e: &Expr, n: usize) -> Option<(Vec<Expr>, Expr)> {
    let ex = algebra::expand(e);
    let mut coeffs = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let c = algebra::simplify(&calculus::diff(&ex, &slot(k)));
        if (0..=n).any(|j| c.contains_var(&slot(j))) {
            return None;
        }
        coeffs.push(c);
    }
    let mut rest = ex;
    for k in 0..=n {
        rest = rest.subs(&slot(k), &Expr::zero());
    }
    Some((coeffs, algebra::simplify(&rest)))
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> R<Option<Vec<Value>>> {
    match name {
        "dsolve" => {
            let eqs = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            if eqs.data.len() != 1 || a.iter().skip(1).any(|v| SymArray::from_value(v).map(|s| s.data.iter().any(|e| unknown(e).is_some() && !is_condition(e))).unwrap_or(false)) {
                return rt("Systems of differential equations are not supported in this version.");
            }
            let mut conds = Vec::new();
            for v in &a[1..] {
                conds.extend(SymArray::from_value(v)?.data);
            }
            Ok(Some(vec![Value::Sym(SymArray::scalar(dsolve(&eqs.data[0], &conds)?))]))
        }
        "odeToVectorField" => {
            let eqs = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            if eqs.data.len() != 1 || a.len() > 1 {
                return rt("Systems of differential equations are not supported in this version.");
            }
            let (field, names) = vector_field(&eqs.data[0])?;
            let n = field.len();
            Ok(Some(vec![Value::Sym(SymArray::from_dims(vec![n, 1], field)), Value::Sym(SymArray::from_dims(vec![n, 1], names))]))
        }
        "functionalDerivative" => {
            let l = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            let y = SymArray::from_value(a.get(1).ok_or_else(not_enough)?)?;
            let Some(Expr::Fn(yname, yargs)) = y.data.first() else { return rt("The second argument must be a symbolic function.") };
            let Some(Expr::Var(t)) = yargs.first() else { return rt("The second argument must be a symbolic function.") };
            let out = l.map(|e| functional_derivative(e, yname, t));
            let mut out = out;
            out.args = vec![t.clone()];
            Ok(Some(vec![Value::Sym(out)]))
        }
        _ => Ok(None),
    }
}

fn is_condition(e: &Expr) -> bool {
    match e {
        Expr::Rel(_, l, _) => matches!(&**l, Expr::Fn(n, _) if n == "subs") || matches!(&**l, Expr::Fn(_, a) if a.len() == 1 && a[0].as_num().is_some()),
        _ => false,
    }
}

fn as_zero(e: &Expr) -> Expr {
    match e {
        Expr::Rel(Rel::Eq, l, r) => Expr::sub((**l).clone(), (**r).clone()),
        other => other.clone(),
    }
}

/// (real part, imaginary part) of a root such as -1 + 2i.
fn re_im(e: &Expr) -> (Expr, Expr) {
    let terms = match e {
        Expr::Add(ts) => ts.clone(),
        other => vec![other.clone()],
    };
    let (mut re, mut im) = (Vec::new(), Vec::new());
    for t in terms {
        let fs = match &t {
            Expr::Mul(fs) => fs.clone(),
            other => vec![other.clone()],
        };
        if let Some(k) = fs.iter().position(|f| matches!(f, Expr::Const(Const::I))) {
            let mut rest = fs.clone();
            rest.remove(k);
            im.push(Expr::mul(rest));
        } else {
            re.push(t);
        }
    }
    (algebra::simplify(&Expr::add(re)), algebra::simplify(&Expr::add(im)))
}

fn exp_of(a: &Expr, t: &str) -> Expr {
    if a.is_zero() {
        return Expr::one();
    }
    Expr::func("exp", vec![Expr::mul(vec![a.clone(), Expr::var(t)])])
}

/// Multiplicity of x as a root of the polynomial with coefficients `p` (ascending) in r.
fn multiplicity(p: &[Expr], x: &Expr) -> usize {
    let mut q = algebra::from_coeffs(p, "\u{0}r");
    for m in 0..p.len() {
        if !is_zero(&q.subs("\u{0}r", x)) {
            return m;
        }
        q = calculus::diff(&q, "\u{0}r");
    }
    p.len()
}

fn constant(k: usize) -> Expr {
    Expr::var(&format!("C{k}"))
}

fn dsolve(eq: &Expr, conds: &[Expr]) -> R<Expr> {
    let no = || LangError::new("Unable to find an explicit solution.", 0);
    let (y, t) = unknown(eq).ok_or_else(|| LangError::new("The equation must contain a derivative of a symbolic function.", 0))?;
    let (e, n) = to_slots(&as_zero(eq), &y, &t);
    let (a, f) = linear_form(&e, n).ok_or_else(no)?;
    if is_zero(&a[n]) {
        return Err(no());
    }
    let g = algebra::simplify(&Expr::neg(f)); // L[y] = g
    let constant_coeffs = a.iter().all(|c| !c.contains_var(&t));
    let mut used = 0;
    let general = if constant_coeffs && a[..n].iter().all(is_zero) {
        // y^(n) = g/a_n: integrate n times, adding a constant each time.
        let mut s = algebra::simplify(&Expr::div(g.clone(), a[n].clone()));
        let mut consts = Vec::new();
        for _ in 0..n {
            s = calculus::int_indef(&s, &t);
            used += 1;
            let c = constant(used);
            consts.push(c.clone());
            s = Expr::add(vec![s, c]);
        }
        let _ = consts;
        s
    } else if constant_coeffs {
        let r = "\u{0}r";
        let poly = algebra::from_coeffs(&a, r);
        let roots = algebra::solve_one(&algebra::expand(&poly), r).ok_or_else(no)?;
        let mut distinct: Vec<Expr> = Vec::new();
        for x in roots {
            let x = algebra::simplify(&x);
            if !distinct.iter().any(|d| is_zero(&Expr::sub(d.clone(), x.clone()))) {
                distinct.push(x);
            }
        }
        // Real roots and one root of each complex pair, in ascending order of real part.
        let mut groups: Vec<(Expr, Expr, usize)> = Vec::new(); // (alpha, beta, multiplicity)
        for x in &distinct {
            let (re, im) = re_im(x);
            if im.is_zero() {
                groups.push((x.clone(), Expr::zero(), multiplicity(&a, x)));
            } else if im.to_f64().map(|v| v > 0.0).unwrap_or(true) {
                groups.push((re, im, multiplicity(&a, x)));
            }
        }
        let key = |e: &Expr| e.to_f64().unwrap_or(f64::INFINITY);
        groups.sort_by(|p, q| key(&p.0).partial_cmp(&key(&q.0)).unwrap_or(std::cmp::Ordering::Equal));
        let mut terms = Vec::new();
        for (alpha, beta, m) in &groups {
            for j in 0..*m {
                let tp = if j == 0 { Expr::one() } else { Expr::pow(Expr::var(&t), Expr::int(j as i128)) };
                if beta.is_zero() {
                    used += 1;
                    terms.push(Expr::mul(vec![constant(used), tp, exp_of(alpha, &t)]));
                } else {
                    let arg = Expr::mul(vec![beta.clone(), Expr::var(&t)]);
                    used += 1;
                    terms.push(Expr::mul(vec![constant(used), tp.clone(), Expr::func("cos", vec![arg.clone()]), exp_of(alpha, &t)]));
                    used += 1;
                    terms.push(Expr::mul(vec![Expr::int(-1), constant(used), tp, Expr::func("sin", vec![arg]), exp_of(alpha, &t)]));
                }
            }
        }
        let homog = Expr::add(terms);
        if is_zero(&g) {
            homog
        } else {
            Expr::add(vec![particular(&a, &g, &t).ok_or_else(no)?, homog])
        }
    } else if n == 1 {
        // y' + p y = q with p = a0/a1, q = g/a1.
        let p = algebra::simplify(&Expr::div(a[0].clone(), a[1].clone()));
        let q = algebra::simplify(&Expr::div(g.clone(), a[1].clone()));
        let ip = calculus::int_indef(&p, &t);
        let mu = Expr::func("exp", vec![ip.clone()]);
        let inv = Expr::func("exp", vec![Expr::neg(ip)]);
        used += 1;
        if is_zero(&q) {
            Expr::mul(vec![constant(used), inv])
        } else {
            let iq = calculus::int_indef(&algebra::simplify(&Expr::mul(vec![q, mu])), &t);
            algebra::simplify(&Expr::mul(vec![inv, Expr::add(vec![iq, constant(used)])]))
        }
    } else {
        return Err(no());
    };
    if conds.is_empty() {
        return Ok(general);
    }
    // Initial conditions: a linear system in the constants.
    let mut eqs = Vec::new();
    for c in conds {
        let z = as_zero(c);
        let at = z.map(&|x| match x {
            Expr::Fn(nm, args) if nm == "subs" && args.len() == 3 => match &args[0] {
                Expr::Fn(d, da) if d == "diff" && da.first().is_some_and(|f| is_fn_of(f, &y, &t)) => {
                    let mut s = general.clone();
                    for _ in 1..da.len() {
                        s = calculus::diff(&s, &t);
                    }
                    Some(s.subs(&t, &args[2]))
                }
                _ => None,
            },
            Expr::Fn(nm, args) if *nm == y && args.len() == 1 => Some(general.subs(&t, &args[0])),
            _ => None,
        });
        eqs.push(at);
    }
    let vars: Vec<String> = (1..=used).map(|k| format!("C{k}")).collect();
    let mut free = Vec::new();
    for e in &eqs {
        e.free_vars(&mut free);
    }
    let unknowns: Vec<String> = vars.iter().filter(|v| free.contains(v)).cloned().collect();
    let sol = algebra::solve_linear(&eqs, &unknowns).ok_or_else(|| LangError::new("No solution satisfies the initial conditions.", 0))?;
    let mut out = general;
    for (v, s) in unknowns.iter().zip(sol) {
        out = out.subs(v, &s);
    }
    Ok(out)
}

/// A particular solution of L[y] = g for g a sum of polynomials times exponentials, by
/// undetermined coefficients.
fn particular(a: &[Expr], g: &Expr, t: &str) -> Option<Expr> {
    let terms = match algebra::expand(g) {
        Expr::Add(ts) => ts,
        other => vec![other],
    };
    // Group by the exponential rate.
    let mut groups: Vec<(Expr, Vec<Expr>)> = Vec::new();
    for term in terms {
        let fs = match &term {
            Expr::Mul(fs) => fs.clone(),
            other => vec![other.clone()],
        };
        let mut rate = Expr::zero();
        let mut rest = Vec::new();
        for f in fs {
            match &f {
                Expr::Fn(n, args) if n == "exp" && args[0].contains_var(t) => {
                    let (s, off) = super::extra::linear_in(&args[0], t)?;
                    rate = algebra::simplify(&Expr::add(vec![rate, s]));
                    if !off.is_zero() {
                        rest.push(Expr::func("exp", vec![off]));
                    }
                }
                _ => rest.push(f),
            }
        }
        let poly = Expr::mul(rest);
        match groups.iter_mut().find(|(r, _)| is_zero(&Expr::sub(r.clone(), rate.clone()))) {
            Some((_, ps)) => ps.push(poly),
            None => groups.push((rate, vec![poly])),
        }
    }
    let mut out = Vec::new();
    for (rate, polys) in groups {
        let target = algebra::poly_coeffs(&algebra::expand(&Expr::add(polys)), t)?;
        let d = target.len() - 1;
        let s = multiplicity(a, &rate);
        let unknowns: Vec<String> = (0..=d).map(|j| format!("\u{0}b{j}")).collect();
        let guess_poly = Expr::add((0..=d).map(|j| Expr::mul(vec![Expr::var(&unknowns[j]), Expr::pow(Expr::var(t), Expr::int((j + s) as i128))])).collect());
        // L[p e^(r t)] = e^(r t) sum_k a_k (D + r)^k p.
        let mut lhs = Vec::new();
        let mut dk = guess_poly.clone();
        for (k, ak) in a.iter().enumerate() {
            if k > 0 {
                dk = algebra::expand(&Expr::add(vec![calculus::diff(&dk, t), Expr::mul(vec![rate.clone(), dk.clone()])]));
            }
            lhs.push(Expr::mul(vec![ak.clone(), dk.clone()]));
        }
        let reduced = algebra::expand(&Expr::add(lhs));
        let got = algebra::poly_coeffs(&reduced, t)?;
        let m = got.len().max(target.len());
        let eqs: Vec<Expr> = (0..m)
            .map(|j| Expr::sub(got.get(j).cloned().unwrap_or_else(Expr::zero), target.get(j).cloned().unwrap_or_else(Expr::zero)))
            .collect();
        let sol = algebra::solve_linear(&eqs, &unknowns)?;
        let mut p = guess_poly;
        for (v, x) in unknowns.iter().zip(sol) {
            p = p.subs(v, &x);
        }
        out.push(Expr::mul(vec![algebra::simplify(&p), exp_of(&rate, t)]));
    }
    Some(Expr::add(out))
}

/// y^(n) = F(t, y, ..., y^(n-1)) as the first-order system Y' = [Y[2]; ...; F].
fn vector_field(eq: &Expr) -> R<(Vec<Expr>, Vec<Expr>)> {
    let no = || LangError::new("Unable to convert the equation to a first-order system.", 0);
    let (y, t) = unknown(eq).ok_or_else(no)?;
    let (e, n) = to_slots(&as_zero(eq), &y, &t);
    let (a, f) = linear_form(&e, n).ok_or_else(no)?;
    if is_zero(&a[n]) {
        return Err(no());
    }
    let mut rhs = Expr::neg(Expr::div(Expr::add((0..n).map(|k| Expr::mul(vec![a[k].clone(), Expr::var(&slot(k))])).chain([f]).collect()), a[n].clone()));
    for k in 0..n {
        rhs = rhs.subs(&slot(k), &Expr::var(&format!("Y[{}]", k + 1)));
    }
    let mut field: Vec<Expr> = (2..=n).map(|k| Expr::var(&format!("Y[{k}]"))).collect();
    field.push(algebra::simplify(&rhs));
    let names = (0..n).map(|k| Expr::var(&match k { 0 => y.clone(), 1 => format!("D{y}"), _ => format!("D{k}{y}") })).collect();
    Ok((field, names))
}

/// Euler–Lagrange: sum_k (-1)^k d^k/dt^k dL/d(y^(k)).
fn functional_derivative(l: &Expr, y: &str, t: &str) -> Expr {
    let (e, n) = to_slots(l, y, t);
    let mut terms = Vec::new();
    for k in 0..=n {
        let p = calculus::diff(&e, &slot(k));
        let mut q = from_slots(&p, y, t, n);
        for _ in 0..k {
            q = calculus::diff(&q, t);
        }
        terms.push(if k % 2 == 1 { Expr::neg(q) } else { q });
    }
    algebra::expand(&Expr::add(terms))
}
