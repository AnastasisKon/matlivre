//! Integral transforms: `ztrans`/`iztrans`, `fourier`/`ifourier` and `htrans`/`ihtrans`, by
//! rules (linearity, shifts, multiplication by the variable) and tables of transform pairs.
//! What no rule covers stays as an unevaluated call, as in MATLAB.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use super::expr::{Const, Num};
use super::extra::{factors, linear_in};
use super::{algebra, calculus, Expr, SymArray};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;
type Q = BigRational;

pub const NAMES: &[&str] = &["ztrans", "iztrans", "fourier", "ifourier", "htrans", "ihtrans"];

fn var_of(v: &Value) -> R<String> {
    match SymArray::from_value(v)?.data.as_slice() {
        [Expr::Var(n)] => Ok(n.clone()),
        _ => rt("The variable must be a symbolic variable."),
    }
}

/// (independent variable, transformation variable) from the optional arguments, with
/// MATLAB's defaults: the independent variable is `dflt_in` when present, otherwise symvar; the
/// transformation variable is `dflt_out` (or `alt` when the expression already uses it).
fn vars(e: &Expr, rest: &[Value], dflt_in: &str, dflt_out: &str, alt: &str) -> R<(String, String)> {
    let auto_in = || {
        if e.contains_var(dflt_in) {
            dflt_in.to_string()
        } else {
            super::default_var(e).unwrap_or_else(|| dflt_in.to_string())
        }
    };
    let auto_out = |v: &str| if e.contains_var(dflt_out) || v == dflt_out { alt.to_string() } else { dflt_out.to_string() };
    Ok(match rest {
        [] => {
            let v = auto_in();
            let o = auto_out(&v);
            (v, o)
        }
        [o] => (auto_in(), var_of(o)?),
        [v, o, ..] => (var_of(v)?, var_of(o)?),
    })
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> R<Option<Vec<Value>>> {
    let s = SymArray::from_value(a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?)?;
    let rest = &a[1..];
    let mut out = Vec::with_capacity(s.data.len());
    for e in &s.data {
        let r = match name {
            "ztrans" => {
                let (n, z) = vars(e, rest, "n", "z", "w")?;
                ztrans(e, &n, &z).unwrap_or_else(|| Expr::func("ztrans", vec![e.clone(), Expr::var(&n), Expr::var(&z)]))
            }
            "iztrans" => {
                let (z, n) = vars(e, rest, "z", "n", "k")?;
                iztrans(e, &z, &n).unwrap_or_else(|| Expr::func("iztrans", vec![e.clone(), Expr::var(&z), Expr::var(&n)]))
            }
            "fourier" => {
                let (t, w) = vars(e, rest, "x", "w", "v")?;
                fourier(e, &t, &w).map(|r| tidy(&r)).unwrap_or_else(|| Expr::func("fourier", vec![e.clone(), Expr::var(&t), Expr::var(&w)]))
            }
            "ifourier" => {
                let (w, x) = vars(e, rest, "w", "x", "t")?;
                match fourier(e, &w, &x) {
                    Some(f) => {
                        let flipped = f.subs(&x, &Expr::neg(Expr::var(&x)));
                        tidy(&Expr::div(flipped, Expr::mul(vec![Expr::int(2), Expr::Const(Const::Pi)])))
                    }
                    None => Expr::func("ifourier", vec![e.clone(), Expr::var(&w), Expr::var(&x)]),
                }
            }
            "htrans" => {
                let (t, x) = vars(e, rest, "t", "x", "z")?;
                htrans(e, &t, &x).map(|r| tidy(&r)).unwrap_or_else(|| Expr::func("htrans", vec![e.clone(), Expr::var(&t), Expr::var(&x)]))
            }
            "ihtrans" => {
                let (x, t) = vars(e, rest, "x", "t", "z")?;
                htrans(e, &x, &t).map(|r| tidy(&Expr::neg(r))).unwrap_or_else(|| Expr::func("ihtrans", vec![e.clone(), Expr::var(&x), Expr::var(&t)]))
            }
            _ => return Ok(None),
        };
        out.push(r);
    }
    Ok(Some(vec![Value::Sym(SymArray::from_dims(s.dims.clone(), out))]))
}

/// abs and dirac are even: abs(-x) = abs(x), dirac(-x) = dirac(x).
fn tidy(e: &Expr) -> Expr {
    e.map(&|x| match x {
        Expr::Fn(n, args) if (n == "abs" || n == "dirac") && args.len() == 1 && args[0].split_coeff().0.is_neg() => Some(Expr::func(n, vec![Expr::neg(args[0].clone())])),
        _ => None,
    })
}

fn split_consts(f: &Expr, v: &str) -> (Expr, Vec<Expr>) {
    let (c, rest): (Vec<Expr>, Vec<Expr>) = factors(f).into_iter().partition(|x| !x.contains_var(v));
    (Expr::mul(c), rest)
}

// ---- Z-transform ------------------------------------------------------------------------------

/// Eulerian numbers A(k, j), j = 0..k-1 (A(0, 0) = 1).
fn eulerian(k: usize) -> Vec<i128> {
    if k == 0 {
        return vec![1];
    }
    let mut row = vec![1i128];
    for m in 2..=k {
        let mut next = vec![0i128; m];
        for (j, slot) in next.iter_mut().enumerate() {
            let a = if j < row.len() { (j as i128 + 1) * row[j] } else { 0 };
            let b = if j >= 1 && j - 1 < row.len() { (m as i128 - j as i128) * row[j - 1] } else { 0 };
            *slot = a + b;
        }
        row = next;
    }
    row
}

fn ztrans(f: &Expr, n: &str, z: &str) -> Option<Expr> {
    let zv = Expr::var(z);
    if !f.contains_var(n) {
        return Some(Expr::mul(vec![f.clone(), zv.clone(), Expr::pow(Expr::sub(zv, Expr::one()), Expr::int(-1))]));
    }
    if let Expr::Add(ts) = f {
        return ts.iter().map(|t| ztrans(t, n, z)).collect::<Option<Vec<_>>>().map(Expr::add);
    }
    let (c, rest) = split_consts(f, n);
    // Split the factors: a^n (or exp(c*n)), n^k, and at most one other factor.
    let mut base = Expr::one();
    let mut k: usize = 0;
    let mut other: Option<Expr> = None;
    for x in rest {
        match &x {
            Expr::Var(v) if v == n => k += 1,
            Expr::Pow(b, e) if matches!(&**b, Expr::Var(v) if v == n) => k += e.as_num()?.as_int().filter(|&p| p >= 0)? as usize,
            Expr::Pow(b, e) if !b.contains_var(n) => {
                let (s, off) = linear_in(e, n)?;
                if !off.is_zero() {
                    return None;
                }
                base = Expr::mul(vec![base, Expr::pow((**b).clone(), s)]);
            }
            Expr::Fn(name, args) if name == "exp" => {
                let (s, off) = linear_in(&args[0], n)?;
                if !off.is_zero() {
                    return None;
                }
                base = Expr::mul(vec![base, Expr::func("exp", vec![s])]);
            }
            _ => {
                if other.is_some() {
                    return None;
                }
                other = Some(x.clone());
            }
        }
    }
    let core = match other {
        None => {
            // n^k a^n = z * sum_j A(k, j) a^(k-j) z^j / (z - a)^(k+1)
            let e = eulerian(k);
            let coeffs: Vec<Expr> = e.iter().enumerate().map(|(j, &aj)| Expr::mul(vec![Expr::int(aj), Expr::pow(base.clone(), Expr::int((k - j) as i128))])).collect();
            let poly = algebra::from_coeffs(&coeffs, z);
            let num = Expr::mul(vec![zv.clone(), poly]);
            return Some(Expr::mul(vec![c, num, Expr::pow(Expr::sub(zv, base), Expr::int(-(k as i128) - 1))]));
        }
        Some(o) => o,
    };
    if k != 0 || !base.is_one() {
        return None;
    }
    match &core {
        Expr::Fn(name, args) if (name == "sin" || name == "cos") && args.len() == 1 => {
            let (b, off) = linear_in(&args[0], n)?;
            if !off.is_zero() {
                return None;
            }
            let den = Expr::add(vec![Expr::pow(zv.clone(), Expr::int(2)), Expr::mul(vec![Expr::int(-2), Expr::func("cos", vec![b.clone()]), zv.clone()]), Expr::one()]);
            let num = if name == "sin" { Expr::mul(vec![zv.clone(), Expr::func("sin", vec![b])]) } else { Expr::mul(vec![zv.clone(), Expr::sub(zv.clone(), Expr::func("cos", vec![b]))]) };
            Some(Expr::mul(vec![c, num, Expr::pow(den, Expr::int(-1))]))
        }
        Expr::Fn(name, args) if name == "kroneckerDelta" => {
            let d = algebra::simplify(&Expr::sub(args[0].clone(), args.get(1).cloned().unwrap_or_else(Expr::zero)));
            let (a, b) = linear_in(&d, n)?;
            // delta(a*n + b): nonzero at n = -b/a (a = ±1).
            let at = algebra::simplify(&Expr::neg(Expr::div(b, a)));
            let m = at.as_num()?.as_int()?;
            if m < 0 {
                return Some(Expr::zero());
            }
            Some(Expr::mul(vec![c, Expr::pow(zv, Expr::int(-m))]))
        }
        _ => None,
    }
}

// rational polynomials, ascending coefficients

fn trim(mut p: Vec<Q>) -> Vec<Q> {
    while p.len() > 1 && p.last().is_some_and(|x| x.is_zero()) {
        p.pop();
    }
    p
}

fn peval(p: &[Q], x: &Q) -> Q {
    p.iter().rev().fold(Q::zero(), |acc, c| acc * x + c)
}

/// p / (z - r), assuming r is a root.
fn deflate(p: &[Q], r: &Q) -> Vec<Q> {
    let n = p.len() - 1;
    let mut q = vec![Q::zero(); n];
    let mut carry = Q::zero();
    for i in (0..n).rev() {
        carry = &p[i + 1] + carry * r;
        q[i] = carry.clone();
    }
    q
}

fn divisors(n: &BigInt) -> Vec<BigInt> {
    let n = n.abs();
    let mut out = Vec::new();
    let mut d = BigInt::one();
    while &d * &d <= n {
        if (&n % &d).is_zero() {
            out.push(d.clone());
            out.push(&n / &d);
        }
        d += 1;
        if d > BigInt::from(100000) {
            break;
        }
    }
    out
}

/// Rational roots with multiplicities, and the remaining factor.
fn rational_roots(p: &[Q]) -> (Vec<(Q, usize)>, Vec<Q>) {
    let mut p = trim(p.to_vec());
    let mut roots: Vec<(Q, usize)> = Vec::new();
    // Zero roots first.
    let mut zeros = 0;
    while p.len() > 1 && p[0].is_zero() {
        p.remove(0);
        zeros += 1;
    }
    if zeros > 0 {
        roots.push((Q::zero(), zeros));
    }
    loop {
        if p.len() <= 1 {
            break;
        }
        // Integer coefficients for the rational root theorem.
        let l = p.iter().fold(BigInt::one(), |acc, c| num_integer::Integer::lcm(&acc, c.denom()));
        let ints: Vec<BigInt> = p.iter().map(|c| (c * Q::from_integer(l.clone())).to_integer()).collect();
        let mut found = None;
        'search: for pn in divisors(&ints[0]) {
            for qd in divisors(ints.last().unwrap()) {
                for sign in [1, -1] {
                    let r = Q::new(&pn * sign, qd.clone());
                    if peval(&p, &r).is_zero() {
                        found = Some(r);
                        break 'search;
                    }
                }
            }
        }
        let Some(r) = found else { break };
        p = deflate(&p, &r);
        match roots.iter_mut().find(|(x, _)| *x == r) {
            Some((_, m)) => *m += 1,
            None => roots.push((r, 1)),
        }
    }
    (roots, p)
}

fn pmul(a: &[Q], b: &[Q]) -> Vec<Q> {
    let mut out = vec![Q::zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

/// Taylor coefficients of p(x + r) (ascending).
fn shift(p: &[Q], r: &Q) -> Vec<Q> {
    let mut out = vec![Q::zero(); p.len()];
    for (i, c) in p.iter().enumerate().rev() {
        // out = out*(x + r) + c
        let mut next = vec![Q::zero(); p.len()];
        for (j, o) in out.iter().enumerate() {
            if j + 1 < next.len() {
                next[j + 1] += o;
            }
            next[j] += o * r;
        }
        next[0] += c;
        out = next;
        let _ = i;
    }
    out
}

/// The first `m` power-series coefficients of a(x)/b(x) at x = 0.
fn series_div(a: &[Q], b: &[Q], m: usize) -> Vec<Q> {
    let mut out = Vec::with_capacity(m);
    let get = |p: &[Q], i: usize| p.get(i).cloned().unwrap_or_else(Q::zero);
    for k in 0..m {
        let mut s = get(a, k);
        for (j, o) in out.iter().enumerate() {
            s -= o * get(b, k - j);
        }
        out.push(s / get(b, 0));
    }
    out
}

fn q_expr(q: &Q) -> Expr {
    Expr::Num(Num::from_big(q.clone()))
}

fn binom_expr(n: &Expr, k: usize) -> Expr {
    let mut fs = Vec::new();
    let mut den: i128 = 1;
    for i in 0..k {
        fs.push(Expr::sub(n.clone(), Expr::int(i as i128)));
        den *= i as i128 + 1;
    }
    fs.push(Expr::rat(1, den));
    Expr::mul(fs)
}

fn iztrans(f: &Expr, z: &str, n: &str) -> Option<Expr> {
    let (num, den) = algebra::numden(&algebra::together(f));
    let to_q = |e: &Expr| -> Option<Vec<Q>> { algebra::poly_coeffs(&algebra::expand(e), z)?.iter().map(|c| c.as_num().and_then(|x| x.exact())).collect() };
    let nq = trim(to_q(&num)?);
    // F(z)/z: one more power of z in the denominator.
    let mut dq = vec![Q::zero()];
    dq.extend(trim(to_q(&den)?));
    let (roots, rest) = rational_roots(&dq);
    if rest.len() != 1 {
        return None;
    }
    let lead = rest[0].clone();
    if nq.len() >= dq.len() {
        return None;
    }
    let nv = Expr::var(n);
    let mut terms = Vec::new();
    for (r, m) in &roots {
        // Laurent coefficients at r: g(x) = N(x + r) / (lead * prod_{other} (x + r - s)^k)
        let mut rest_den = vec![lead.clone()];
        for (s, k) in &roots {
            if s != r {
                for _ in 0..*k {
                    rest_den = pmul(&rest_den, &[-s.clone(), Q::one()]);
                }
            }
        }
        let g = series_div(&shift(&nq, r), &shift(&rest_den, r), *m);
        // c_j / (z - r)^(m - j), so F has c z / (z - r)^p with p = m - j.
        for (j, c) in g.iter().enumerate() {
            if c.is_zero() {
                continue;
            }
            let p = m - j;
            let ce = q_expr(c);
            if r.is_zero() {
                // c z / z^p = c z^(1 - p): delta(n - (p - 1)).
                let shiftn = if p == 1 { nv.clone() } else { Expr::sub(nv.clone(), Expr::int(p as i128 - 1)) };
                terms.push(Expr::mul(vec![ce, Expr::Fn("kroneckerDelta".into(), vec![shiftn, Expr::zero()])]));
            } else {
                // z/(z - r)^p <-> binomial(n, p - 1) r^(n - p + 1)
                let re = q_expr(r);
                let power = if p == 1 { Expr::pow(re, nv.clone()) } else { Expr::pow(re, Expr::sub(nv.clone(), Expr::int(p as i128 - 1))) };
                terms.push(Expr::mul(vec![ce, binom_expr(&nv, p - 1), power]));
            }
        }
    }
    Some(Expr::add(terms))
}

// ---- Fourier transform ------------------------------------------------------------------------

fn i() -> Expr {
    Expr::Const(Const::I)
}

fn pi() -> Expr {
    Expr::Const(Const::Pi)
}

/// F(w) = ∫ f(t) exp(-i w t) dt.
fn fourier(f: &Expr, t: &str, w: &str) -> Option<Expr> {
    let wv = Expr::var(w);
    if !f.contains_var(t) {
        return Some(Expr::mul(vec![Expr::int(2), pi(), f.clone(), Expr::func("dirac", vec![wv])]));
    }
    if let Expr::Add(ts) = f {
        return ts.iter().map(|x| fourier(x, t, w)).collect::<Option<Vec<_>>>().map(Expr::add);
    }
    let (c, rest) = split_consts(f, t);
    let g = Expr::mul(rest.clone());
    // t^k g(t) <-> i^k d^k/dw^k G(w)
    let mut k = 0usize;
    let mut others = Vec::new();
    for x in &rest {
        match x {
            Expr::Var(v) if v == t => k += 1,
            Expr::Pow(b, e) if matches!(&**b, Expr::Var(v) if v == t) && e.as_num().and_then(|n| n.as_int()).is_some_and(|p| p > 0) => k += e.as_num()?.as_int()? as usize,
            _ => others.push(x.clone()),
        }
    }
    if k > 0 && !others.is_empty() {
        let mut gw = fourier(&Expr::mul(others), t, w)?;
        for _ in 0..k {
            gw = calculus::diff(&gw, w);
        }
        return Some(Expr::mul(vec![c, Expr::pow(i(), Expr::int(k as i128)), algebra::simplify(&gw)]));
    }
    let base = pair(&g, t, w)?;
    Some(Expr::mul(vec![c, base]))
}

/// Coefficient c of a quadratic -c t^2 (no linear or constant part).
fn neg_square_coeff(e: &Expr, t: &str) -> Option<Expr> {
    let p = algebra::poly_coeffs(&algebra::expand(e), t)?;
    if p.len() == 3 && p[0].is_zero() && p[1].is_zero() {
        return Some(Expr::neg(p[2].clone()));
    }
    None
}

fn positive_number(e: &Expr) -> bool {
    e.to_f64().is_some_and(|x| x > 0.0)
}

fn pair(g: &Expr, t: &str, w: &str) -> Option<Expr> {
    let wv = Expr::var(w);
    let fs = factors(g);
    match fs.as_slice() {
        [Expr::Fn(n, a)] if n == "dirac" && a.len() == 1 => {
            let (s, off) = linear_in(&a[0], t)?;
            if !s.is_one() {
                return None;
            }
            // dirac(t + b) <-> exp(b w i)
            Some(if off.is_zero() { Expr::one() } else { Expr::func("exp", vec![Expr::mul(vec![off, wv, i()])]) })
        }
        [Expr::Fn(n, a)] if n == "heaviside" && a.len() == 1 && matches!(&a[0], Expr::Var(v) if v == t) => {
            Some(Expr::sub(Expr::mul(vec![pi(), Expr::func("dirac", vec![wv.clone()])]), Expr::div(i(), wv)))
        }
        [Expr::Fn(n, a)] if n == "exp" => {
            // exp(-c t^2), c > 0: sqrt(pi/c) exp(-w^2/(4c))
            if let Some(cq) = neg_square_coeff(&a[0], t) {
                if !positive_number(&cq) {
                    return None;
                }
                let amp = Expr::pow(Expr::div(pi(), cq.clone()), Expr::rat(1, 2));
                return Some(Expr::mul(vec![amp, Expr::func("exp", vec![Expr::neg(Expr::div(Expr::pow(wv, Expr::int(2)), Expr::mul(vec![Expr::int(4), cq])))])]));
            }
            // exp(-c abs(t)): 2c/(w^2 + c^2)
            let (cf, rest) = split_consts(&a[0], t);
            if let [Expr::Fn(abs, aa)] = rest.as_slice() {
                if abs == "abs" && matches!(&aa[0], Expr::Var(v) if v == t) {
                    let cq = Expr::neg(cf);
                    if positive_number(&cq) {
                        return Some(Expr::div(Expr::mul(vec![Expr::int(2), cq.clone()]), Expr::add(vec![Expr::pow(wv, Expr::int(2)), Expr::pow(cq, Expr::int(2))])));
                    }
                }
            }
            None
        }
        [Expr::Fn(h, ha), Expr::Fn(e, ea)] | [Expr::Fn(e, ea), Expr::Fn(h, ha)] if h == "heaviside" && e == "exp" && matches!(&ha[0], Expr::Var(v) if v == t) => {
            // heaviside(t) exp(-c t): 1/(c + w i)
            let (s, off) = linear_in(&ea[0], t)?;
            let cq = Expr::neg(s);
            if !off.is_zero() || !positive_number(&cq) {
                return None;
            }
            Some(Expr::pow(Expr::add(vec![cq, Expr::mul(vec![wv, i()])]), Expr::int(-1)))
        }
        [Expr::Fn(n, a)] if (n == "sin" || n == "cos") && a.len() == 1 => {
            let (b, off) = linear_in(&a[0], t)?;
            if !off.is_zero() {
                return None;
            }
            let dm = Expr::func("dirac", vec![Expr::sub(wv.clone(), b.clone())]);
            let dp = Expr::func("dirac", vec![Expr::add(vec![wv, b])]);
            Some(if n == "cos" {
                Expr::mul(vec![pi(), Expr::add(vec![dm, dp])])
            } else {
                Expr::mul(vec![Expr::int(-1), pi(), Expr::sub(dm, dp), i()])
            })
        }
        _ => {
            // c2/(t^2 + c^2) and the like: (pi/c) exp(-c abs(w)) times the numerator.
            let (num, den) = algebra::numden(&algebra::together(g));
            if num.contains_var(t) {
                return None;
            }
            let p = algebra::poly_coeffs(&algebra::expand(&den), t)?;
            if p.len() != 3 || !p[1].is_zero() {
                return None;
            }
            let c2 = algebra::simplify(&Expr::div(p[0].clone(), p[2].clone()));
            if !positive_number(&c2) {
                return None;
            }
            let cq = algebra::simplify(&Expr::pow(c2, Expr::rat(1, 2)));
            let k = algebra::simplify(&Expr::div(num, p[2].clone()));
            Some(Expr::mul(vec![k, pi(), Expr::pow(cq.clone(), Expr::int(-1)), Expr::func("exp", vec![Expr::neg(Expr::mul(vec![cq, Expr::func("abs", vec![Expr::var(w)])]))])]))
        }
    }
}

// ---- Hilbert transform ------------------------------------------------------------------------

fn htrans(f: &Expr, t: &str, x: &str) -> Option<Expr> {
    let xv = Expr::var(x);
    if !f.contains_var(t) {
        return Some(Expr::zero());
    }
    if let Expr::Add(ts) = f {
        return ts.iter().map(|e| htrans(e, t, x)).collect::<Option<Vec<_>>>().map(Expr::add);
    }
    let (c, rest) = split_consts(f, t);
    let g = Expr::mul(rest);
    let r = match &g {
        Expr::Fn(n, a) if (n == "sin" || n == "cos") && a.len() == 1 => {
            let (b, off) = linear_in(&a[0], t)?;
            if !off.is_zero() || !positive_number(&b) {
                return None;
            }
            let arg = Expr::mul(vec![b, xv]);
            if n == "sin" { Expr::neg(Expr::func("cos", vec![arg])) } else { Expr::func("sin", vec![arg]) }
        }
        Expr::Fn(n, a) if n == "dirac" && matches!(&a[0], Expr::Var(v) if v == t) => Expr::pow(Expr::mul(vec![xv, pi()]), Expr::int(-1)),
        Expr::Pow(b, e) if matches!(&**b, Expr::Var(v) if v == t) && e.as_num().and_then(|n| n.as_int()) == Some(-1) => {
            Expr::mul(vec![Expr::int(-1), pi(), Expr::func("dirac", vec![xv])])
        }
        _ => {
            // 1/(t^2 + 1): x/(x^2 + 1)
            let (num, den) = algebra::numden(&algebra::together(&g));
            if num.contains_var(t) {
                return None;
            }
            let p = algebra::poly_coeffs(&algebra::expand(&den), t)?;
            if p.len() != 3 || !p[1].is_zero() || !p[0].is_one() || !p[2].is_one() {
                return None;
            }
            Expr::mul(vec![num, xv.clone(), Expr::pow(Expr::add(vec![Expr::pow(xv, Expr::int(2)), Expr::one()]), Expr::int(-1))])
        }
    };
    Some(Expr::mul(vec![c, r]))
}
