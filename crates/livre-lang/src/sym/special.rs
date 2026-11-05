//! Symbolic Math Toolbox special functions, orthogonal polynomials and number theory:
//! `heaviside`, `dirac`, the pulses, the exponential/trigonometric/logarithmic and Fresnel
//! integrals, polylogarithms, `igamma`, `hurwitzZeta`, `wrightOmega`, the hypergeometric
//! family, the elliptic integrals and Jacobi elliptic functions, `chebyshevT` and the other
//! classical polynomials, `bernstein`, `euler`, `factorIntegerPower`, `isPrimitiveRoot` and
//! `jacobiSymbol`. Double inputs are evaluated numerically (see `specnum`); sym inputs stay
//! symbolic apart from their exact special values.

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

use super::expr::{Const, Num};
use super::specnum as sn;
use super::{algebra, Expr, SymArray};
use crate::cplx::C;
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::nd::Dims;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "heaviside", "dirac", "rectangularPulse", "triangularPulse", "signIm", "catalan", "eulergamma", "pochhammer", "erfi", "dawson", "sinint", "cosint",
    "sinhint", "coshint", "ssinint", "ei", "logint", "dilog", "polylog", "fresnelc", "fresnels", "igamma", "hurwitzZeta", "wrightOmega", "kummerU",
    "whittakerM", "whittakerW", "hypergeom", "meijerG", "ellipticK", "ellipticE", "ellipticF", "ellipticPi", "ellipticCK", "ellipticCE", "ellipticCPi",
    "ellipticNome", "jacobiAM", "jacobiSN", "jacobiCN", "jacobiDN", "jacobiSC", "jacobiSD", "jacobiCD", "jacobiCS", "jacobiDS", "jacobiDC", "jacobiNC",
    "jacobiND", "jacobiNS", "jacobiZeta", "chebyshevT", "chebyshevU", "gegenbauerC", "hermiteH", "jacobiP", "laguerreL", "legendreP", "bernstein",
    "euler", "factorIntegerPower", "isPrimitiveRoot", "jacobiSymbol",
];

const JACOBI: &[&str] = &["jacobiSN", "jacobiCN", "jacobiDN", "jacobiSC", "jacobiSD", "jacobiCD", "jacobiCS", "jacobiDS", "jacobiDC", "jacobiNC", "jacobiND", "jacobiNS"];

fn not_enough() -> LangError {
    LangError::new("Not enough input arguments.", 0)
}

fn pi() -> Expr {
    Expr::Const(Const::Pi)
}

fn inf() -> Expr {
    Expr::Const(Const::Inf)
}

// ---- numeric evaluation shared by double inputs and double(sym) ---------------------------------

fn cr(x: f64) -> C {
    C::new(x, 0.0)
}

/// The value of `name` at real arguments (None when the arity is wrong).
fn eval_real(name: &str, x: &[f64]) -> Option<C> {
    let a = |i: usize| x.get(i).copied();
    Some(match (name, x.len()) {
        ("catalan", 0) => cr(sn::CATALAN),
        ("eulergamma", 0) => cr(sn::EULER),
        ("heaviside", 1) => cr(sn::heaviside(a(0)?)),
        ("dirac", 1) => cr(dirac_num(0.0, a(0)?)),
        ("dirac", 2) => cr(dirac_num(a(0)?, a(1)?)),
        ("rectangularPulse", 1) => cr(sn::rect_pulse(-0.5, 0.5, a(0)?)),
        ("rectangularPulse", 3) => cr(sn::rect_pulse(a(0)?, a(1)?, a(2)?)),
        ("triangularPulse", 1) => cr(sn::tri_pulse(-1.0, 0.0, 1.0, a(0)?)),
        ("triangularPulse", 3) => cr(sn::tri_pulse(a(0)?, (a(0)? + a(1)?) / 2.0, a(1)?, a(2)?)),
        ("triangularPulse", 4) => cr(sn::tri_pulse(a(0)?, a(1)?, a(2)?, a(3)?)),
        ("signIm", 1) => cr(sn::sign_im(cr(a(0)?))),
        ("pochhammer", 2) => cr(sn::pochhammer(a(0)?, a(1)?)),
        ("erfi", 1) => cr(sn::erfi(a(0)?)),
        ("dawson", 1) => cr(sn::dawson(a(0)?)),
        ("sinint", 1) => cr(sn::sinint(a(0)?)),
        ("cosint", 1) => sn::cosint(a(0)?),
        ("sinhint", 1) => cr(sn::sinhint(a(0)?)),
        ("coshint", 1) => sn::coshint(a(0)?),
        ("ssinint", 1) => cr(sn::sinint(a(0)?) - std::f64::consts::FRAC_PI_2),
        ("ei", 1) => cr(sn::ei(a(0)?)),
        ("expint", 1) if a(0)? >= 0.0 => cr(sn::e1(a(0)?)),
        ("logint", 1) => cr(sn::logint(a(0)?)),
        ("dilog", 1) => sn::dilog(a(0)?),
        ("polylog", 2) => sn::polylog(a(0)?, a(1)?),
        ("fresnelc", 1) => cr(sn::fresnel(a(0)?).0),
        ("fresnels", 1) => cr(sn::fresnel(a(0)?).1),
        ("igamma", 2) => cr(sn::igamma(a(0)?, a(1)?)),
        ("hurwitzZeta", 2) => cr(sn::hurwitz_zeta(a(0)?, a(1)?)),
        ("wrightOmega", 1) => cr(sn::wright_omega(a(0)?)),
        ("kummerU", 3) => cr(sn::kummer_u(a(0)?, a(1)?, a(2)?)),
        ("whittakerM", 3) => cr(sn::whittaker_m(a(0)?, a(1)?, a(2)?)),
        ("whittakerW", 3) => cr(sn::whittaker_w(a(0)?, a(1)?, a(2)?)),
        ("ellipticK", 1) => sn::ellip_k(a(0)?),
        ("ellipticE", 1) => sn::ellip_e(a(0)?),
        ("ellipticE", 2) => cr(sn::ellip_e_inc(a(0)?, a(1)?)),
        ("ellipticF", 2) => cr(sn::ellip_f(a(0)?, a(1)?)),
        ("ellipticPi", 2) => cr(sn::ellip_pi(a(0)?, a(1)?)),
        ("ellipticPi", 3) => cr(sn::ellip_pi_inc(a(0)?, a(1)?, a(2)?)),
        ("ellipticCK", 1) => sn::ellip_k(1.0 - a(0)?),
        ("ellipticCE", 1) => sn::ellip_e(1.0 - a(0)?),
        ("ellipticCPi", 2) => cr(sn::ellip_pi(a(0)?, 1.0 - a(1)?)),
        ("ellipticNome", 1) => cr(sn::ellip_nome(a(0)?)),
        ("jacobiAM", 2) => cr(sn::jacobi(a(0)?, a(1)?).3),
        ("jacobiZeta", 2) => cr(sn::jacobi_zeta(a(0)?, a(1)?)),
        (n, 2) if JACOBI.contains(&n) => {
            let (s, c, d, _) = sn::jacobi(a(0)?, a(1)?);
            let (p, q) = (&n[6..7], &n[7..8]);
            let part = |l: &str| match l {
                "S" => s,
                "C" => c,
                "D" => d,
                _ => 1.0,
            };
            cr(part(p) / part(q))
        }
        _ => return None,
    })
}

fn dirac_num(n: f64, x: f64) -> f64 {
    if x.is_nan() || n.is_nan() {
        f64::NAN
    } else if x == 0.0 {
        if n as i64 % 2 == 0 { f64::INFINITY } else { f64::NEG_INFINITY }
    } else {
        0.0
    }
}

/// `eval_c` hook: Some(value) when `name` is one of these functions.
pub fn eval_hook(name: &str, xs: &[Expr], env: &dyn Fn(&str) -> Option<C>) -> Option<Option<C>> {
    if !NAMES.contains(&name) && name != "expint" {
        return None;
    }
    let real_vec = |e: &Expr| -> Option<Vec<f64>> {
        match e {
            Expr::Fn(n, items) if n == "__vec" => items.iter().map(|i| i.eval_c(env).filter(|z| z.im == 0.0).map(|z| z.re)).collect(),
            other => other.eval_c(env).filter(|z| z.im == 0.0).map(|z| vec![z.re]),
        }
    };
    if name == "hypergeom" && xs.len() == 3 {
        let (a, b, z) = (real_vec(&xs[0]), real_vec(&xs[1]), real_vec(&xs[2]));
        return Some(match (a, b, z) {
            (Some(a), Some(b), Some(z)) if z.len() == 1 => Some(cr(sn::hypergeom(&a, &b, z[0]))),
            _ => None,
        });
    }
    if name == "meijerG" && xs.len() == 5 {
        let v: Option<Vec<Vec<f64>>> = xs.iter().map(real_vec).collect();
        return Some(v.filter(|v| v[4].len() == 1).map(|v| cr(sn::meijer_g(&v[0], &v[1], &v[2], &v[3], v[4][0]))));
    }
    if name == "signIm" && xs.len() == 1 {
        return Some(xs[0].eval_c(env).map(|z| cr(sn::sign_im(z))));
    }
    let mut vals = Vec::with_capacity(xs.len());
    for x in xs {
        match x.eval_c(env) {
            Some(z) if z.im == 0.0 => vals.push(z.re),
            _ => return Some(None),
        }
    }
    if let Some(r) = poly_eval(name, &vals) {
        return Some(r.map(cr));
    }
    Some(eval_real(name, &vals))
}

fn poly_eval(name: &str, v: &[f64]) -> Option<Option<f64>> {
    let n = |i: usize| -> Option<usize> {
        let x = *v.get(i)?;
        (x >= 0.0 && x.fract() == 0.0).then_some(x as usize)
    };
    Some(match (name, v.len()) {
        ("chebyshevT", 2) => n(0).map(|k| cheb_t(k, &v[1])),
        ("chebyshevU", 2) => n(0).map(|k| cheb_u(k, &v[1])),
        ("hermiteH", 2) => n(0).map(|k| hermite(k, &v[1])),
        ("legendreP", 2) => n(0).map(|k| legendre(k, &v[1])),
        ("laguerreL", 2) => n(0).map(|k| laguerre(k, &0.0, &v[1])),
        ("laguerreL", 3) => n(0).map(|k| laguerre(k, &v[1], &v[2])),
        ("gegenbauerC", 3) => n(0).map(|k| gegenbauer(k, &v[1], &v[2])),
        ("jacobiP", 4) => n(0).map(|k| jacobi_p(k, &v[1], &v[2], &v[3])),
        _ => return None,
    })
}

// ---- argument handling ------------------------------------------------------------------------

/// Common size of elementwise arguments (scalars expand).
fn common_dims(dims: &[Dims]) -> R<Dims> {
    let mut out: Option<Dims> = None;
    for d in dims {
        if d.iter().product::<usize>() == 1 {
            continue;
        }
        match &out {
            None => out = Some(d.clone()),
            Some(o) if o == d => {}
            Some(_) => return rt("Input arguments must be scalars or arrays of the same size."),
        }
    }
    Ok(out.unwrap_or_else(|| vec![1, 1]))
}

fn numeric(args: &[Value], f: &dyn Fn(&[f64]) -> R<C>) -> R<Value> {
    let ms = args.iter().map(|a| a.to_matrix()).collect::<R<Vec<_>>>()?;
    let dims = common_dims(&ms.iter().map(|m| m.dims.clone()).collect::<Vec<_>>())?;
    let n: usize = dims.iter().product();
    let (mut re, mut im) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut buf = vec![0.0; ms.len()];
    for i in 0..n {
        for (j, m) in ms.iter().enumerate() {
            buf[j] = if m.numel() == 1 { m.data[0] } else { m.data[i] };
        }
        let z = f(&buf)?;
        re.push(z.re);
        im.push(z.im);
    }
    Ok(Value::Num(if im.iter().all(|&v| v == 0.0) { Matrix::from_dims(dims, re) } else { Matrix::complex(dims, re, im) }))
}

fn symbolic(args: &[Value], f: &dyn Fn(&[Expr]) -> R<Expr>) -> R<Value> {
    let ss = args.iter().map(SymArray::from_value).collect::<R<Vec<_>>>()?;
    let dims = common_dims(&ss.iter().map(|s| s.dims.clone()).collect::<Vec<_>>())?;
    let n: usize = dims.iter().product();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let row: Vec<Expr> = ss.iter().map(|s| if s.data.len() == 1 { s.data[0].clone() } else { s.data[i].clone() }).collect();
        out.push(f(&row)?);
    }
    Ok(Value::Sym(SymArray::from_dims(dims, out)))
}

fn any_sym(args: &[Value]) -> bool {
    args.iter().any(|a| matches!(a, Value::Sym(_)))
}

fn real_value(e: &Expr) -> Option<f64> {
    let mut vars = Vec::new();
    e.free_vars(&mut vars);
    if !vars.is_empty() {
        return None;
    }
    super::mp::to_f64(e).or_else(|| e.to_f64())
}

fn num_int(e: &Expr) -> Option<i128> {
    e.as_num().and_then(|n| n.as_int())
}

/// -f(x) for odd f when x carries a negative coefficient.
fn odd(name: &str, x: &Expr) -> Expr {
    let (c, _) = x.split_coeff();
    if c.is_neg() {
        Expr::neg(Expr::Fn(name.into(), vec![Expr::neg(x.clone())]))
    } else {
        Expr::Fn(name.into(), vec![x.clone()])
    }
}

fn f(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Fn(name.into(), args)
}

/// Symbolic value with MATLAB's exact special cases.
fn sym_call(name: &str, a: &[Expr]) -> R<Expr> {
    let z0 = |e: &Expr| e.is_zero();
    let one = |e: &Expr| num_int(e) == Some(1);
    Ok(match (name, a.len()) {
        ("catalan" | "eulergamma", 0) => f(name, vec![]),
        ("heaviside", 1) => match real_value(&a[0]) {
            Some(v) if v > 0.0 => Expr::one(),
            Some(v) if v < 0.0 => Expr::zero(),
            Some(v) if v == 0.0 => Expr::rat(1, 2),
            _ => f(name, a.to_vec()),
        },
        ("dirac", 1) => match real_value(&a[0]) {
            Some(v) if v == 0.0 => inf(),
            Some(v) if !v.is_nan() => Expr::zero(),
            _ => f(name, a.to_vec()),
        },
        ("dirac", 2) => match (num_int(&a[0]), real_value(&a[1])) {
            (Some(0), _) => sym_call("dirac", &a[1..])?,
            (Some(n), Some(v)) if v == 0.0 => if n % 2 == 0 { inf() } else { Expr::neg(inf()) },
            (Some(_), Some(v)) if !v.is_nan() => Expr::zero(),
            _ => f(name, a.to_vec()),
        },
        ("rectangularPulse", 1) => sym_call(name, &[Expr::rat(-1, 2), Expr::rat(1, 2), a[0].clone()])?,
        ("rectangularPulse", 3) => {
            let (l, r) = (real_value(&a[0]), real_value(&a[1]));
            if let (Some(l), Some(r)) = (l, r) {
                if l > r {
                    return rt("Left border must not exceed right border.");
                }
                if let Some(x) = real_value(&a[2]) {
                    return Ok(Expr::from_f64(sn::rect_pulse(l, r, x)));
                }
            }
            f(name, a.to_vec())
        }
        ("triangularPulse", 1) => sym_call(name, &[Expr::int(-1), Expr::zero(), Expr::one(), a[0].clone()])?,
        ("triangularPulse", 3) => {
            let mid = algebra::simplify(&Expr::div(Expr::add(vec![a[0].clone(), a[1].clone()]), Expr::int(2)));
            sym_call(name, &[a[0].clone(), mid, a[1].clone(), a[2].clone()])?
        }
        ("triangularPulse", 4) => {
            let v: Vec<Option<f64>> = a.iter().map(real_value).collect();
            if let (Some(l), Some(m), Some(r)) = (v[0], v[1], v[2]) {
                if !(l <= m && m <= r) {
                    return rt("Left border must not exceed right border.");
                }
                if let Some(x) = v[3] {
                    let e = if x > l && x < m {
                        Expr::div(Expr::sub(a[3].clone(), a[0].clone()), Expr::sub(a[1].clone(), a[0].clone()))
                    } else if x == m && l < r {
                        Expr::one()
                    } else if x > m && x < r {
                        Expr::div(Expr::sub(a[2].clone(), a[3].clone()), Expr::sub(a[2].clone(), a[1].clone()))
                    } else {
                        Expr::zero()
                    };
                    return Ok(algebra::simplify(&e));
                }
            }
            f(name, a.to_vec())
        }
        ("signIm", 1) => match a[0].eval_c(&|_| None) {
            Some(z) => Expr::from_f64(sn::sign_im(z)),
            None => f(name, a.to_vec()),
        },
        ("pochhammer", 2) => match (num_int(&a[1]), a[0].as_num()) {
            (Some(0), _) => Expr::one(),
            (Some(n), Some(_)) if n > 0 && n < 10_000 => {
                let terms: Vec<Expr> = (0..n).map(|k| Expr::add(vec![a[0].clone(), Expr::int(k)])).collect();
                algebra::expand(&Expr::mul(terms))
            }
            _ => f(name, a.to_vec()),
        },
        ("sinint" | "sinhint" | "erfi" | "dawson" | "fresnelc" | "fresnels", 1) => {
            if z0(&a[0]) {
                Expr::zero()
            } else {
                odd(name, &a[0])
            }
        }
        ("cosint" | "coshint" | "ei", 1) if z0(&a[0]) => Expr::neg(inf()),
        ("ssinint", 1) if z0(&a[0]) => Expr::neg(Expr::div(pi(), Expr::int(2))),
        ("logint", 1) if z0(&a[0]) => Expr::zero(),
        ("logint", 1) if one(&a[0]) => Expr::neg(inf()),
        ("dilog", 1) if one(&a[0]) => Expr::zero(),
        ("dilog", 1) if z0(&a[0]) => Expr::div(Expr::pow(pi(), Expr::int(2)), Expr::int(6)),
        ("polylog", 2) if z0(&a[1]) => Expr::zero(),
        ("polylog", 2) if one(&a[1]) => super::extra::zeta_sym(&a[0]),
        ("polylog", 2) if num_int(&a[0]) == Some(1) => Expr::neg(Expr::func("log", vec![Expr::sub(Expr::one(), a[1].clone())])),
        ("igamma", 2) if one(&a[0]) => Expr::func("exp", vec![Expr::neg(a[1].clone())]),
        ("igamma", 2) if z0(&a[0]) => f("expint", vec![a[1].clone()]),
        ("hurwitzZeta", 2) if one(&a[1]) => super::extra::zeta_sym(&a[0]),
        ("wrightOmega", 1) if z0(&a[0]) => f("lambertw", vec![Expr::zero(), Expr::one()]),
        ("ellipticK" | "ellipticE", 1) if z0(&a[0]) => Expr::div(pi(), Expr::int(2)),
        ("ellipticK", 1) if one(&a[0]) => inf(),
        ("ellipticE", 1) if one(&a[0]) => Expr::one(),
        ("ellipticCK" | "ellipticCE", 1) if one(&a[0]) => Expr::div(pi(), Expr::int(2)),
        ("ellipticCE", 1) if z0(&a[0]) => Expr::one(),
        ("ellipticCK", 1) if z0(&a[0]) => inf(),
        ("ellipticE" | "ellipticF", 2) if z0(&a[0]) => Expr::zero(),
        ("ellipticE", 2) if is_half_pi(&a[0]) => sym_call("ellipticE", &a[1..])?,
        ("ellipticF", 2) if is_half_pi(&a[0]) => sym_call("ellipticK", &a[1..])?,
        ("ellipticE" | "ellipticF", 2) if z0(&a[1]) => a[0].clone(),
        ("ellipticPi", 2) if z0(&a[0]) => sym_call("ellipticK", &a[1..])?,
        ("ellipticNome", 1) if z0(&a[0]) => Expr::zero(),
        ("ellipticNome", 1) if one(&a[0]) => Expr::one(),
        ("jacobiAM", 2) if z0(&a[1]) => a[0].clone(),
        ("jacobiZeta", 2) if z0(&a[1]) => Expr::zero(),
        (n, 2) if JACOBI.contains(&n) && z0(&a[1]) => {
            let (s, c) = (Expr::func("sin", vec![a[0].clone()]), Expr::func("cos", vec![a[0].clone()]));
            let part = |l: &str| match l {
                "S" => s.clone(),
                "C" => c.clone(),
                _ => Expr::one(),
            };
            algebra::simplify(&Expr::div(part(&n[6..7]), part(&n[7..8])))
        }
        (n, 2) if JACOBI.contains(&n) && z0(&a[0]) => {
            let part = |l: &str| if l == "S" { Expr::zero() } else { Expr::one() };
            let (p, q) = (part(&n[6..7]), part(&n[7..8]));
            if q.is_zero() { inf() } else { p }
        }
        _ => f(name, a.to_vec()),
    })
}

fn is_half_pi(e: &Expr) -> bool {
    *e == Expr::div(pi(), Expr::int(2))
}

fn arity_ok(name: &str, n: usize) -> bool {
    match name {
        "catalan" | "eulergamma" => n == 0,
        "dirac" => n == 1 || n == 2,
        "rectangularPulse" => n == 1 || n == 3,
        "triangularPulse" => n == 1 || n == 3 || n == 4,
        "ellipticE" => n == 1 || n == 2,
        "ellipticPi" => n == 2 || n == 3,
        "pochhammer" | "polylog" | "igamma" | "hurwitzZeta" | "ellipticF" | "ellipticCPi" | "jacobiAM" | "jacobiZeta" => n == 2,
        "kummerU" | "whittakerM" | "whittakerW" | "hypergeom" => n == 3,
        "meijerG" => n == 5,
        n2 if JACOBI.contains(&n2) => n == 2,
        _ => n == 1,
    }
}

pub fn call(it: &mut Interpreter, name: &str, args: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let out = |v: Value| Ok(Some(vec![v]));
    match name {
        "chebyshevT" | "chebyshevU" | "gegenbauerC" | "hermiteH" | "jacobiP" | "laguerreL" | "legendreP" => return out(poly_call(name, args)?),
        "bernstein" => return out(bernstein(it, args)?),
        "euler" => return out(euler_call(args)?),
        "factorIntegerPower" => return Ok(Some(factor_power(args, nargout)?)),
        "isPrimitiveRoot" => return out(primitive_root(args)?),
        "jacobiSymbol" => return out(jacobi_symbol(args)?),
        _ => {}
    }
    if args.len() > 5 || !arity_ok(name, args.len()) {
        return if args.len() < 5 && !arity_ok(name, args.len()) && args.len() < max_arity(name) { Err(not_enough()) } else { rt("Too many input arguments.") };
    }
    if name == "catalan" || name == "eulergamma" {
        return out(Value::Sym(SymArray::scalar(f(name, vec![]))));
    }
    if name == "hypergeom" || name == "meijerG" {
        return out(hyper_call(name, args)?);
    }
    if any_sym(args) {
        return out(symbolic(args, &|a| sym_call(name, a))?);
    }
    if name == "signIm" {
        let m = args[0].to_matrix()?;
        let vals = (0..m.numel()).map(|i| sn::sign_im(C::new(m.data[i], m.im_at(i)))).collect();
        return out(Value::Num(Matrix::from_dims(m.dims.clone(), vals)));
    }
    if name == "rectangularPulse" && args.len() == 3 {
        let (l, r) = (args[0].to_matrix()?, args[1].to_matrix()?);
        if l.data.iter().zip(r.data.iter().cycle()).any(|(a, b)| a > b) {
            return rt("Left border must not exceed right border.");
        }
    }
    out(numeric(args, &|x| eval_real(name, x).ok_or_else(|| LangError::new("Too many input arguments.", 0)))?)
}

fn max_arity(name: &str) -> usize {
    (0..=5).rev().find(|&n| arity_ok(name, n)).unwrap_or(1)
}

fn vec_expr(v: &Value) -> R<Expr> {
    let s = SymArray::from_value(v)?;
    Ok(if s.data.len() == 1 { s.data[0].clone() } else { f("__vec", s.data) })
}

fn hyper_call(name: &str, args: &[Value]) -> R<Value> {
    let nvec = if name == "hypergeom" { 2 } else { 4 };
    let z = args.last().unwrap();
    if any_sym(args) {
        let params = args[..nvec].iter().map(|v| {
            let s = SymArray::from_value(v)?;
            Ok(f("__vec", s.data))
        });
        let params = params.collect::<R<Vec<Expr>>>()?;
        let zs = SymArray::from_value(z)?;
        return Ok(Value::Sym(zs.map(|e| {
            let mut all = params.clone();
            all.push(e.clone());
            if name == "hypergeom" && e.is_zero() {
                return Expr::one();
            }
            f(name, all)
        })));
    }
    let params = args[..nvec].iter().map(|v| v.to_matrix().map(|m| m.data.clone())).collect::<R<Vec<_>>>()?;
    let zm = z.to_matrix()?;
    let vals = zm
        .data
        .iter()
        .map(|&x| if name == "hypergeom" { sn::hypergeom(&params[0], &params[1], x) } else { sn::meijer_g(&params[0], &params[1], &params[2], &params[3], x) })
        .collect();
    let _ = vec_expr;
    Ok(Value::Num(Matrix::from_dims(zm.dims.clone(), vals)))
}

// ---- orthogonal polynomials -------------------------------------------------------------------

trait Ring: Clone {
    fn int(n: i64) -> Self;
    fn add(&self, o: &Self) -> Self;
    fn sub(&self, o: &Self) -> Self;
    fn mul(&self, o: &Self) -> Self;
    fn div_int(&self, n: i64) -> Self;
}

impl Ring for f64 {
    fn int(n: i64) -> Self {
        n as f64
    }
    fn add(&self, o: &Self) -> Self {
        self + o
    }
    fn sub(&self, o: &Self) -> Self {
        self - o
    }
    fn mul(&self, o: &Self) -> Self {
        self * o
    }
    fn div_int(&self, n: i64) -> Self {
        self / n as f64
    }
}

impl Ring for Expr {
    fn int(n: i64) -> Self {
        Expr::int(n as i128)
    }
    fn add(&self, o: &Self) -> Self {
        algebra::expand(&Expr::add(vec![self.clone(), o.clone()]))
    }
    fn sub(&self, o: &Self) -> Self {
        algebra::expand(&Expr::sub(self.clone(), o.clone()))
    }
    fn mul(&self, o: &Self) -> Self {
        algebra::expand(&Expr::mul(vec![self.clone(), o.clone()]))
    }
    fn div_int(&self, n: i64) -> Self {
        algebra::expand(&Expr::mul(vec![Expr::rat(1, n as i128), self.clone()]))
    }
}

fn three_term<T: Ring>(n: usize, p0: T, p1: T, next: impl Fn(usize, &T, &T) -> T) -> T {
    if n == 0 {
        return p0;
    }
    let (mut a, mut b) = (p0, p1);
    for k in 1..n {
        let c = next(k, &b, &a);
        a = b;
        b = c;
    }
    b
}

fn cheb_t<T: Ring>(n: usize, x: &T) -> T {
    three_term(n, T::int(1), x.clone(), |_, b, a| T::int(2).mul(x).mul(b).sub(a))
}

fn cheb_u<T: Ring>(n: usize, x: &T) -> T {
    three_term(n, T::int(1), T::int(2).mul(x), |_, b, a| T::int(2).mul(x).mul(b).sub(a))
}

fn hermite<T: Ring>(n: usize, x: &T) -> T {
    three_term(n, T::int(1), T::int(2).mul(x), |k, b, a| T::int(2).mul(x).mul(b).sub(&T::int(2 * k as i64).mul(a)))
}

fn legendre<T: Ring>(n: usize, x: &T) -> T {
    three_term(n, T::int(1), x.clone(), |k, b, a| {
        let k = k as i64;
        T::int(2 * k + 1).mul(x).mul(b).sub(&T::int(k).mul(a)).div_int(k + 1)
    })
}

fn laguerre<T: Ring>(n: usize, al: &T, x: &T) -> T {
    let p1 = T::int(1).add(al).sub(x);
    three_term(n, T::int(1), p1, |k, b, a| {
        let kk = k as i64;
        T::int(2 * kk + 1).add(al).sub(x).mul(b).sub(&T::int(kk).add(al).mul(a)).div_int(kk + 1)
    })
}

fn gegenbauer<T: Ring>(n: usize, al: &T, x: &T) -> T {
    let p1 = T::int(2).mul(al).mul(x);
    three_term(n, T::int(1), p1, |k, b, a| {
        let kk = k as i64;
        T::int(2).mul(&T::int(kk).add(al)).mul(x).mul(b).sub(&T::int(kk - 1).add(&T::int(2).mul(al)).mul(a)).div_int(kk + 1)
    })
}

/// Generalized binomial coefficient C(r, k).
fn binom<T: Ring>(r: &T, k: usize) -> T {
    let mut p = T::int(1);
    for i in 0..k {
        p = p.mul(&r.sub(&T::int(i as i64)));
    }
    let fact: i64 = (1..=k as i64).product();
    p.div_int(fact)
}

fn jacobi_p<T: Ring>(n: usize, al: &T, be: &T, x: &T) -> T {
    let xm = x.sub(&T::int(1)).div_int(2);
    let xp = x.add(&T::int(1)).div_int(2);
    let na = T::int(n as i64).add(al);
    let nb = T::int(n as i64).add(be);
    let mut sum = T::int(0);
    for s in 0..=n {
        let mut term = binom(&na, n - s).mul(&binom(&nb, s));
        for _ in 0..s {
            term = term.mul(&xm);
        }
        for _ in 0..n - s {
            term = term.mul(&xp);
        }
        sum = sum.add(&term);
    }
    sum
}

fn poly_call(name: &str, args: &[Value]) -> R<Value> {
    let want = match name {
        "gegenbauerC" => 3,
        "jacobiP" => 4,
        "laguerreL" if args.len() == 3 => 3,
        _ => 2,
    };
    if args.len() < want {
        return Err(not_enough());
    }
    if args.len() > want {
        return rt("Too many input arguments.");
    }
    let bad_n = || LangError::new("Nonnegative integer or a symbolic variable expected.", 0);
    if any_sym(args) {
        return symbolic(args, &|a| {
            let n = match a[0].as_num() {
                Some(v) => match v.as_int() {
                    Some(k) if k >= 0 => k as usize,
                    _ => return Err(bad_n()),
                },
                None => return Ok(f(name, a.to_vec())),
            };
            Ok(match (name, a.len()) {
                ("chebyshevT", _) => cheb_t(n, &a[1]),
                ("chebyshevU", _) => cheb_u(n, &a[1]),
                ("hermiteH", _) => hermite(n, &a[1]),
                ("legendreP", _) => legendre(n, &a[1]),
                ("laguerreL", 2) => laguerre(n, &Expr::zero(), &a[1]),
                ("laguerreL", _) => laguerre(n, &a[1], &a[2]),
                ("gegenbauerC", _) => gegenbauer(n, &a[1], &a[2]),
                _ => jacobi_p(n, &a[1], &a[2], &a[3]),
            })
        });
    }
    numeric(args, &|v| {
        if v[0] < 0.0 || v[0].fract() != 0.0 {
            return Err(bad_n());
        }
        Ok(cr(poly_eval(name, v).flatten().unwrap_or(f64::NAN)))
    })
}

fn bernstein(it: &mut Interpreter, args: &[Value]) -> R<Value> {
    if args.len() < 3 {
        return Err(not_enough());
    }
    let (fv, var, nv, tv) = if args.len() >= 4 { (&args[0], Some(&args[1]), &args[2], &args[3]) } else { (&args[0], None, &args[1], &args[2]) };
    let n = nv.scalar()?;
    if n < 0.0 || n.fract() != 0.0 {
        return rt("Nonnegative integer expected.");
    }
    let n = n as i64;
    let t = SymArray::from_value(tv)?;
    let mut values = Vec::with_capacity(n as usize + 1);
    for k in 0..=n {
        let at = Expr::rat(k as i128, n.max(1) as i128);
        let v = match fv {
            Value::Func(h) => {
                let r = it.call_value(h, vec![Value::Sym(SymArray::scalar(at))], 1)?;
                SymArray::from_value(r.first().ok_or_else(not_enough)?)?.data.into_iter().next().ok_or_else(not_enough)?
            }
            other => {
                let e = SymArray::from_value(other)?.data.into_iter().next().ok_or_else(not_enough)?;
                let v = match var {
                    Some(v) => match SymArray::from_value(v)?.data.first() {
                        Some(Expr::Var(name)) => Some(name.clone()),
                        _ => return rt("The variable must be a symbolic variable."),
                    },
                    None => super::default_var(&e),
                };
                match v {
                    Some(v) => algebra::simplify(&e.subs(&v, &at)),
                    None => e,
                }
            }
        };
        values.push(v);
    }
    let result = t.map(|tt| {
        let mut terms = Vec::new();
        for (k, fk) in values.iter().enumerate() {
            if fk.is_zero() {
                continue;
            }
            let k = k as i64;
            let c = binom_i(n, k) * if (n - k) % 2 == 0 { 1 } else { -1 };
            terms.push(Expr::mul(vec![Expr::int(c), fk.clone(), Expr::pow(tt.clone(), Expr::int(k as i128)), Expr::pow(Expr::sub(tt.clone(), Expr::one()), Expr::int((n - k) as i128))]));
        }
        Expr::add(terms)
    });
    if !any_sym(args) || (!matches!(tv, Value::Sym(_)) && matches!(fv, Value::Func(_))) {
        if let Ok(m) = result.to_matrix() {
            return Ok(Value::Num(m));
        }
    }
    Ok(Value::Sym(result))
}

fn binom_i(n: i64, k: i64) -> i128 {
    let mut r: i128 = 1;
    for i in 0..k {
        r = r * (n - i) as i128 / (i + 1) as i128;
    }
    r
}

// ---- number theory ----------------------------------------------------------------------------

fn euler_numbers(n: usize) -> Vec<BigInt> {
    // E_0..E_n with E_m = -sum_{k<m, k even} C(m, k) E_k for even m.
    let mut e = vec![BigInt::zero(); n + 1];
    e[0] = BigInt::one();
    for m in (2..=n).step_by(2) {
        let mut s = BigInt::zero();
        let mut c = BigInt::one();
        for k in 0..m {
            if k % 2 == 0 {
                s += &c * &e[k];
            }
            c = c * BigInt::from(m - k) / BigInt::from(k + 1);
        }
        e[m] = -s;
    }
    e
}

fn euler_call(args: &[Value]) -> R<Value> {
    let nv = args.first().ok_or_else(not_enough)?;
    let int_n = |e: &Expr| -> R<usize> {
        match num_int(e) {
            Some(k) if k >= 0 => Ok(k as usize),
            _ => rt("Nonnegative integer or a symbolic variable expected."),
        }
    };
    if args.len() == 1 {
        let s = SymArray::from_value(nv)?;
        let vals: Vec<Expr> = s
            .data
            .iter()
            .map(|e| {
                if num_int(e).is_none() && e.as_num().is_none() {
                    return Ok(f("euler", vec![e.clone()]));
                }
                let k = int_n(e)?;
                Ok(Expr::Num(Num::from_big(BigRational::from_integer(euler_numbers(k)[k].clone()))))
            })
            .collect::<R<_>>()?;
        return Ok(if matches!(nv, Value::Sym(_)) {
            Value::Sym(SymArray::from_dims(s.dims, vals))
        } else {
            Value::Num(Matrix::from_dims(s.dims, vals.iter().map(|e| e.to_f64().unwrap_or(f64::NAN)).collect()))
        });
    }
    let poly = |k: usize, x: &Expr| -> Expr {
        let en = euler_numbers(k);
        let h = Expr::sub(x.clone(), Expr::rat(1, 2));
        let mut terms = Vec::new();
        for (j, ej) in en.iter().enumerate() {
            if ej.is_zero() {
                continue;
            }
            let c = BigRational::new(BigInt::from(binom_i(k as i64, j as i64)) * ej, BigInt::from(2).pow(j as u32));
            terms.push(Expr::mul(vec![Expr::Num(Num::from_big(c)), Expr::pow(h.clone(), Expr::int((k - j) as i128))]));
        }
        algebra::expand(&Expr::add(terms))
    };
    if any_sym(args) {
        return symbolic(args, &|a| {
            if a[0].as_num().is_none() {
                return Ok(f("euler", a.to_vec()));
            }
            Ok(poly(int_n(&a[0])?, &a[1]))
        });
    }
    numeric(args, &|v| {
        if v[0] < 0.0 || v[0].fract() != 0.0 {
            return rt("Nonnegative integer or a symbolic variable expected.");
        }
        Ok(cr(poly(v[0] as usize, &Expr::from_f64(v[1])).to_f64().unwrap_or(f64::NAN)))
    })
}

fn int_values(v: &Value) -> R<(Dims, Vec<i128>)> {
    let s = SymArray::from_value(v)?;
    let vals = s.data.iter().map(|e| num_int(e).ok_or_else(|| LangError::new("Input must be integers.", 0))).collect::<R<Vec<_>>>()?;
    Ok((s.dims, vals))
}

fn factor_power(args: &[Value], nargout: usize) -> R<Vec<Value>> {
    let v = args.first().ok_or_else(not_enough)?;
    let (dims, ns) = int_values(v)?;
    let (mut bs, mut es) = (Vec::new(), Vec::new());
    for &n in &ns {
        let (b, e) = perfect_power(n);
        bs.push(b);
        es.push(e);
    }
    let mk = |xs: &[i128]| -> Value {
        if matches!(v, Value::Sym(_)) {
            Value::Sym(SymArray::from_dims(dims.clone(), xs.iter().map(|&x| Expr::int(x)).collect()))
        } else {
            Value::Num(Matrix::from_dims(dims.clone(), xs.iter().map(|&x| x as f64).collect()))
        }
    };
    let mut out = vec![mk(&bs)];
    if nargout > 1 {
        out.push(mk(&es));
    }
    Ok(out)
}

fn perfect_power(n: i128) -> (i128, i128) {
    let m = n.unsigned_abs();
    if m < 4 {
        return (n, 1);
    }
    let maxe = 128 - m.leading_zeros() as i128;
    for e in (2..=maxe).rev() {
        if n < 0 && e % 2 == 0 {
            continue;
        }
        let guess = (m as f64).powf(1.0 / e as f64).round() as u128;
        for c in [guess.saturating_sub(1), guess, guess + 1] {
            if c < 2 {
                continue;
            }
            let mut p: u128 = 1;
            let mut ok = true;
            for _ in 0..e {
                match p.checked_mul(c) {
                    Some(v) if v <= m => p = v,
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok && p == m {
                return (if n < 0 { -(c as i128) } else { c as i128 }, e);
            }
        }
    }
    (n, 1)
}

fn pow_mod(mut b: u128, mut e: u128, m: u128) -> u128 {
    let mut r = 1 % m;
    b %= m;
    while e > 0 {
        if e & 1 == 1 {
            r = r * b % m;
        }
        b = b * b % m;
        e >>= 1;
    }
    r
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn totient_factors(n: u128) -> (u128, Vec<u128>) {
    let mut m = n;
    let mut phi = n;
    let mut p = 2;
    while p * p <= m {
        if m % p == 0 {
            while m % p == 0 {
                m /= p;
            }
            phi -= phi / p;
        }
        p += 1;
    }
    if m > 1 {
        phi -= phi / m;
    }
    let mut fs = Vec::new();
    let mut t = phi;
    let mut p = 2;
    while p * p <= t {
        if t % p == 0 {
            fs.push(p);
            while t % p == 0 {
                t /= p;
            }
        }
        p += 1;
    }
    if t > 1 {
        fs.push(t);
    }
    (phi, fs)
}

fn pairwise(args: &[Value], f: &dyn Fn(i128, i128) -> R<i128>) -> R<(Dims, Vec<i128>)> {
    if args.len() < 2 {
        return Err(not_enough());
    }
    let (da, a) = int_values(&args[0])?;
    let (db, b) = int_values(&args[1])?;
    let dims = common_dims(&[da, db])?;
    let n: usize = dims.iter().product();
    let vals = (0..n).map(|i| f(if a.len() == 1 { a[0] } else { a[i] }, if b.len() == 1 { b[0] } else { b[i] })).collect::<R<Vec<_>>>()?;
    Ok((dims, vals))
}

fn primitive_root(args: &[Value]) -> R<Value> {
    let (dims, vals) = pairwise(args, &|a, n| {
        if n <= 0 {
            return rt("Invalid argument at position 2. Value must be positive.");
        }
        if gcd(a, n) != 1 {
            return Ok(0);
        }
        let n = n as u128;
        let a = a.rem_euclid(n as i128) as u128;
        let (phi, fs) = totient_factors(n);
        Ok(fs.iter().all(|&p| pow_mod(a, phi / p, n) != 1 % n) as i128)
    })?;
    let mut m = Matrix::from_dims(dims, vals.iter().map(|&v| v as f64).collect());
    m.kind = Kind::Logical;
    Ok(Value::Num(m))
}

fn jacobi_symbol(args: &[Value]) -> R<Value> {
    let (dims, vals) = pairwise(args, &|a, n| {
        if n <= 0 {
            return rt("Invalid argument at position 2. Value must be positive.");
        }
        if n % 2 == 0 {
            return rt("Invalid argument at position 2. Value must be odd.");
        }
        let (mut a, mut n) = (a.rem_euclid(n), n);
        let mut t = 1;
        while a != 0 {
            while a % 2 == 0 {
                a /= 2;
                let r = n % 8;
                if r == 3 || r == 5 {
                    t = -t;
                }
            }
            std::mem::swap(&mut a, &mut n);
            if a % 4 == 3 && n % 4 == 3 {
                t = -t;
            }
            a %= n;
        }
        Ok(if n == 1 { t } else { 0 })
    })?;
    Ok(if any_sym(args) {
        Value::Sym(SymArray::from_dims(dims, vals.into_iter().map(Expr::int).collect()))
    } else {
        Value::Num(Matrix::from_dims(dims, vals.iter().map(|&v| v as f64).collect()))
    })
}
