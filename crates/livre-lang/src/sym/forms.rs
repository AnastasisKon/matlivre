//! Matrix normal forms (`jordan`, `smithForm`, `hermiteForm`), Groebner bases (`gbasis`) and
//! `piecewise`.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use super::expr::{Const, Num, Rel};
use super::{algebra, Expr, SymArray};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;
type Q = BigRational;

pub const NAMES: &[&str] = &["jordan", "smithForm", "hermiteForm", "gbasis", "piecewise"];

fn not_enough() -> LangError {
    LangError::new("Not enough input arguments.", 0)
}

fn is_zero(e: &Expr) -> bool {
    e.is_zero() || algebra::simplify(e).is_zero()
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let numeric = !a.iter().any(|v| matches!(v, Value::Sym(_)));
    let out = |s: SymArray| -> R<Value> { if numeric { Ok(Value::Num(s.to_matrix()?)) } else { Ok(Value::Sym(s)) } };
    match name {
        "jordan" => {
            let s = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            let (v, j) = jordan(&s)?;
            Ok(Some(if nargout > 1 { vec![out(v)?, out(j)?] } else { vec![out(j)?] }))
        }
        "smithForm" => {
            let s = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            let (u, v, sm) = smith(&s)?;
            Ok(Some(if nargout > 1 { vec![out(u)?, out(v)?, out(sm)?] } else { vec![out(sm)?] }))
        }
        "hermiteForm" => {
            let s = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            let (u, h) = hermite(&s)?;
            Ok(Some(if nargout > 1 { vec![out(u)?, out(h)?] } else { vec![out(h)?] }))
        }
        "gbasis" => {
            let polys = SymArray::from_value(a.first().ok_or_else(not_enough)?)?;
            let mut vars: Option<Vec<String>> = None;
            let mut order = MonoOrder::GrevLex;
            let mut i = 1;
            while i < a.len() {
                if a[i].is_text() && !matches!(a[i], Value::Sym(_)) {
                    let key = a[i].text()?;
                    if key.eq_ignore_ascii_case("MonomialOrder") {
                        let v = a.get(i + 1).ok_or_else(not_enough)?.text()?;
                        order = match v.as_str() {
                            "lexicographic" => MonoOrder::Lex,
                            "degreeLexicographic" => MonoOrder::GrLex,
                            "degreeInverseLexicographic" => MonoOrder::GrevLex,
                            _ => return rt("MonomialOrder must be 'degreeInverseLexicographic', 'degreeLexicographic', or 'lexicographic'."),
                        };
                    }
                    i += 2;
                } else {
                    let s = SymArray::from_value(&a[i])?;
                    vars = Some(s.data.iter().map(|e| match e { Expr::Var(n) => Ok(n.clone()), _ => rt("Variables must be symbolic variables.") }).collect::<R<_>>()?);
                    i += 1;
                }
            }
            let vars = match vars {
                Some(v) => v,
                None => {
                    let mut vs = Vec::new();
                    for e in &polys.data {
                        e.free_vars(&mut vs);
                    }
                    vs.sort();
                    vs
                }
            };
            let g = gbasis(&polys.data, &vars, order)?;
            let n = g.len();
            Ok(Some(vec![Value::Sym(SymArray::from_dims(vec![1, n], g))]))
        }
        "piecewise" => {
            let mut args = Vec::new();
            for (k, v) in a.iter().enumerate() {
                let s = SymArray::from_value(v)?;
                let e = s.data.into_iter().next().ok_or_else(not_enough)?;
                args.push(if k % 2 == 0 && k + 1 < a.len() { normalize_cond(e) } else { e });
            }
            if args.len() % 2 == 1 {
                let last = args.pop().unwrap();
                args.push(Expr::Fn("symtrue".into(), Vec::new()));
                args.push(last);
            }
            Ok(Some(vec![Value::Sym(SymArray::scalar(Expr::func("piecewise", args)))]))
        }
        _ => Ok(None),
    }
}

/// x >= 0 is shown as 0 <= x, as MATLAB does.
fn normalize_cond(e: Expr) -> Expr {
    match e {
        Expr::Rel(Rel::Ge, l, r) if !matches!(*l, Expr::Rel(..)) => Expr::Rel(Rel::Le, r, l),
        Expr::Rel(Rel::Gt, l, r) if !matches!(*l, Expr::Rel(..)) => Expr::Rel(Rel::Lt, r, l),
        other => other,
    }
}

/// Truth of a condition when it can be decided (no free variables).
pub fn decide(c: &Expr) -> Option<bool> {
    match c {
        Expr::Fn(n, a) if a.is_empty() && n == "symtrue" => Some(true),
        Expr::Fn(n, a) if a.is_empty() && n == "symfalse" => Some(false),
        Expr::Rel(op, l, r) => {
            // a <= x <= b: both comparisons.
            if let Expr::Rel(_, _, mid) = &**l {
                let first = decide(l)?;
                let second = decide(&Expr::Rel(*op, mid.clone(), r.clone()))?;
                return Some(first && second);
            }
            let d = algebra::simplify(&Expr::sub((**l).clone(), (**r).clone()));
            if matches!(op, Rel::Eq | Rel::Ne) && d.is_zero() {
                return Some(*op == Rel::Eq);
            }
            let mut vs = Vec::new();
            d.free_vars(&mut vs);
            if !vs.is_empty() {
                return None;
            }
            let z = d.eval_c(&|_| None)?;
            if z.im != 0.0 {
                return Some(*op == Rel::Ne);
            }
            let x = z.re;
            Some(match op {
                Rel::Eq => x == 0.0,
                Rel::Ne => x != 0.0,
                Rel::Lt => x < 0.0,
                Rel::Le => x <= 0.0,
                Rel::Gt => x > 0.0,
                Rel::Ge => x >= 0.0,
            })
        }
        _ => None,
    }
}

/// piecewise(c1, v1, c2, v2, ...) with decided conditions folded.
pub fn fold_piecewise(args: Vec<Expr>) -> Expr {
    let mut kept = Vec::new();
    for pair in args.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        match decide(&pair[0]) {
            Some(false) => continue,
            Some(true) if kept.is_empty() => return pair[1].clone(),
            Some(true) => {
                kept.push(Expr::Fn("symtrue".into(), Vec::new()));
                kept.push(pair[1].clone());
                break;
            }
            None => {
                kept.push(pair[0].clone());
                kept.push(pair[1].clone());
            }
        }
    }
    if kept.is_empty() {
        return Expr::Const(Const::NaN);
    }
    Expr::Fn("piecewise".into(), kept)
}

// ---- Jordan form ------------------------------------------------------------------------------

fn mat_rows(s: &SymArray) -> Vec<Vec<Expr>> {
    let (r, c) = (s.dims[0], s.dims[1]);
    (0..r).map(|i| (0..c).map(|j| s.data[j * r + i].clone()).collect()).collect()
}

fn from_rows(rows: &[Vec<Expr>]) -> SymArray {
    let r = rows.len();
    let c = rows.first().map(|x| x.len()).unwrap_or(0);
    let mut data = Vec::with_capacity(r * c);
    for j in 0..c {
        for row in rows {
            data.push(row[j].clone());
        }
    }
    SymArray::from_dims(vec![r, c], data)
}

fn mmul(a: &[Vec<Expr>], b: &[Vec<Expr>]) -> Vec<Vec<Expr>> {
    let (n, m, p) = (a.len(), b.len(), b.first().map(|r| r.len()).unwrap_or(0));
    (0..n).map(|i| (0..p).map(|j| algebra::simplify(&Expr::add((0..m).map(|k| Expr::mul(vec![a[i][k].clone(), b[k][j].clone()])).collect()))).collect()).collect()
}

fn mvec(a: &[Vec<Expr>], v: &[Expr]) -> Vec<Expr> {
    a.iter().map(|row| algebra::simplify(&Expr::add(row.iter().zip(v).map(|(x, y)| Expr::mul(vec![x.clone(), y.clone()])).collect()))).collect()
}

fn rank(rows: &[Vec<Expr>]) -> usize {
    let mut m = rows.to_vec();
    super::tools::rref(&mut m).len()
}

/// Basis of the null space, each vector with its free component 1.
fn nullspace(rows: &[Vec<Expr>]) -> Vec<Vec<Expr>> {
    let n = rows.first().map(|r| r.len()).unwrap_or(0);
    let mut m = rows.to_vec();
    let piv = super::tools::rref(&mut m);
    (0..n)
        .filter(|c| !piv.contains(c))
        .map(|f| {
            let mut v = vec![Expr::zero(); n];
            v[f] = Expr::one();
            for (r, &pc) in piv.iter().enumerate() {
                v[pc] = algebra::simplify(&Expr::neg(m[r][f].clone()));
            }
            v
        })
        .collect()
}

fn jordan(s: &SymArray) -> R<(SymArray, SymArray)> {
    if s.dims.len() != 2 || s.dims[0] != s.dims[1] {
        return rt("Matrix must be square.");
    }
    let n = s.dims[0];
    let a = mat_rows(s);
    let lam = "__jordan_lambda";
    let mut cm = a.clone();
    for (i, row) in cm.iter_mut().enumerate() {
        row[i] = Expr::sub(row[i].clone(), Expr::var(lam));
    }
    let p = super::sym_det(&from_rows(&cm))?;
    let roots = algebra::solve_one(&algebra::expand(&p), lam).ok_or_else(|| LangError::new("Unable to compute the eigenvalues.", 0))?;
    let mut eig: Vec<Expr> = Vec::new();
    for r in roots {
        let r = algebra::simplify(&r);
        if !eig.iter().any(|e| is_zero(&Expr::sub(e.clone(), r.clone()))) {
            eig.push(r);
        }
    }
    // (eigenvalue, chain of vectors)
    let mut blocks: Vec<(Expr, Vec<Vec<Expr>>)> = Vec::new();
    for l in &eig {
        let mut nm = a.clone();
        for (i, row) in nm.iter_mut().enumerate() {
            row[i] = algebra::simplify(&Expr::sub(row[i].clone(), l.clone()));
        }
        let mut powers = vec![nm.clone()];
        let mut ranks = vec![n, rank(&nm)];
        while ranks.len() <= n && ranks[ranks.len() - 1] != ranks[ranks.len() - 2] {
            let next = mmul(powers.last().unwrap(), &nm);
            ranks.push(rank(&next));
            powers.push(next);
        }
        // The largest block: the last power at which the rank still drops.
        let top = (1..ranks.len()).filter(|&j| ranks[j] < ranks[j - 1]).max().unwrap_or(0);
        // Number of blocks of size >= j is ranks[j-1] - ranks[j].
        let mut chosen_eigvecs: Vec<Vec<Expr>> = Vec::new();
        for k in (1..=top).rev() {
            let at_least = |j: usize| if j <= top { ranks[j - 1] - ranks[j] } else { 0 };
            let count = at_least(k) - at_least(k + 1);
            if count == 0 {
                continue;
            }
            let kernel = nullspace(&powers[k - 1]);
            let mut made = 0;
            for v in kernel {
                if made == count {
                    break;
                }
                let mut chain = vec![v.clone()];
                for _ in 1..k {
                    let prev = mvec(&nm, chain.last().unwrap());
                    chain.push(prev);
                }
                let ev = chain.last().unwrap().clone();
                if ev.iter().all(is_zero) {
                    continue;
                }
                let mut test = chosen_eigvecs.clone();
                test.push(ev.clone());
                if rank(&test) < test.len() {
                    continue;
                }
                chosen_eigvecs.push(ev);
                chain.reverse();
                blocks.push((l.clone(), chain));
                made += 1;
            }
        }
    }
    let key = |e: &Expr| e.eval_c(&|_| None).map(|z| (z.re, z.im)).unwrap_or((f64::INFINITY, 0.0));
    blocks.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then_with(|| key(&x.0).partial_cmp(&key(&y.0)).unwrap_or(Ordering::Equal)));
    let mut j = vec![vec![Expr::zero(); n]; n];
    let mut v = vec![vec![Expr::zero(); n]; n];
    let mut col = 0;
    for (l, chain) in &blocks {
        for (k, vec) in chain.iter().enumerate() {
            if col >= n {
                break;
            }
            j[col][col] = l.clone();
            if k > 0 {
                j[col - 1][col] = Expr::one();
            }
            for (i, x) in vec.iter().enumerate() {
                v[i][col] = x.clone();
            }
            col += 1;
        }
    }
    if col != n {
        return rt("Unable to compute the Jordan form.");
    }
    Ok((from_rows(&v), from_rows(&j)))
}

// ---- Smith and Hermite forms over the integers ------------------------------------------------

fn int_rows(s: &SymArray) -> R<Vec<Vec<BigInt>>> {
    mat_rows(s)
        .into_iter()
        .map(|row| {
            row.iter()
                .map(|e| match e.as_num().and_then(|n| n.exact()) {
                    Some(q) if q.is_integer() => Ok(q.to_integer()),
                    _ => rt("Matrix entries must be integers (polynomial matrices are not supported in this version)."),
                })
                .collect()
        })
        .collect()
}

fn to_sym(rows: &[Vec<BigInt>]) -> SymArray {
    from_rows(&rows.iter().map(|r| r.iter().map(|x| Expr::Num(Num::from_big(Q::from_integer(x.clone())))).collect()).collect::<Vec<_>>())
}

fn identity(n: usize) -> Vec<Vec<BigInt>> {
    (0..n).map(|i| (0..n).map(|j| if i == j { BigInt::one() } else { BigInt::zero() }).collect()).collect()
}

fn floor_div(a: &BigInt, b: &BigInt) -> BigInt {
    num_integer::Integer::div_floor(a, b)
}

fn smith(s: &SymArray) -> R<(SymArray, SymArray, SymArray)> {
    let mut a = int_rows(s)?;
    let (m, n) = (a.len(), a.first().map(|r| r.len()).unwrap_or(0));
    let mut u = identity(m);
    let mut v = identity(n);
    for t in 0..m.min(n) {
        loop {
            // The smallest nonzero entry of the remaining block goes to (t, t).
            let mut best: Option<(usize, usize)> = None;
            for i in t..m {
                for j in t..n {
                    if !a[i][j].is_zero() && best.is_none_or(|(bi, bj)| a[i][j].abs() < a[bi][bj].abs()) {
                        best = Some((i, j));
                    }
                }
            }
            let Some((bi, bj)) = best else { break };
            a.swap(t, bi);
            u.swap(t, bi);
            for row in a.iter_mut() {
                row.swap(t, bj);
            }
            for row in v.iter_mut() {
                row.swap(t, bj);
            }
            let p = a[t][t].clone();
            let mut clean = true;
            for i in t + 1..m {
                let q = floor_div(&a[i][t], &p);
                if !q.is_zero() {
                    for j in 0..n {
                        let x = &a[t][j] * &q;
                        a[i][j] -= x;
                    }
                    for j in 0..m {
                        let x = &u[t][j] * &q;
                        u[i][j] -= x;
                    }
                }
                clean &= a[i][t].is_zero();
            }
            for j in t + 1..n {
                let q = floor_div(&a[t][j], &p);
                if !q.is_zero() {
                    for i in 0..m {
                        let x = &a[i][t] * &q;
                        a[i][j] -= x;
                    }
                    for i in 0..n {
                        let x = &v[i][t] * &q;
                        v[i][j] -= x;
                    }
                }
                clean &= a[t][j].is_zero();
            }
            if !clean {
                continue;
            }
            // Divisibility: a row with an entry the pivot does not divide is added to row t.
            let bad = (t + 1..m).find(|&i| (t + 1..n).any(|j| !(&a[i][j] % &p).is_zero()));
            match bad {
                Some(i) => {
                    for j in 0..n {
                        let x = a[i][j].clone();
                        a[t][j] += x;
                    }
                    for j in 0..m {
                        let x = u[i][j].clone();
                        u[t][j] += x;
                    }
                }
                None => break,
            }
        }
        if a[t][t].is_negative() {
            for j in 0..n {
                a[t][j] = -a[t][j].clone();
            }
            for j in 0..m {
                u[t][j] = -u[t][j].clone();
            }
        }
    }
    Ok((to_sym(&u), to_sym(&v), to_sym(&a)))
}

fn hermite(s: &SymArray) -> R<(SymArray, SymArray)> {
    let mut a = int_rows(s)?;
    let (m, n) = (a.len(), a.first().map(|r| r.len()).unwrap_or(0));
    let mut u = identity(m);
    let mut r = 0;
    for c in 0..n {
        if r >= m {
            break;
        }
        loop {
            let best = (r..m).filter(|&i| !a[i][c].is_zero()).min_by(|&x, &y| a[x][c].abs().cmp(&a[y][c].abs()));
            let Some(b) = best else { break };
            a.swap(r, b);
            u.swap(r, b);
            let p = a[r][c].clone();
            let mut done = true;
            for i in r + 1..m {
                let q = floor_div(&a[i][c], &p);
                if !q.is_zero() {
                    for j in 0..n {
                        let x = &a[r][j] * &q;
                        a[i][j] -= x;
                    }
                    for j in 0..m {
                        let x = &u[r][j] * &q;
                        u[i][j] -= x;
                    }
                }
                done &= a[i][c].is_zero();
            }
            if done {
                break;
            }
        }
        if a[r][c].is_zero() {
            continue;
        }
        if a[r][c].is_negative() {
            for j in 0..n {
                a[r][j] = -a[r][j].clone();
            }
            for j in 0..m {
                u[r][j] = -u[r][j].clone();
            }
        }
        // Entries above the pivot reduced into (-p/2, p/2].
        let p = a[r][c].clone();
        for i in 0..r {
            let mut rem = num_integer::Integer::mod_floor(&a[i][c], &p);
            if &rem * 2 > p {
                rem -= &p;
            }
            let q = (&a[i][c] - &rem) / &p;
            if !q.is_zero() {
                for j in 0..n {
                    let x = &a[r][j] * &q;
                    a[i][j] -= x;
                }
                for j in 0..m {
                    let x = &u[r][j] * &q;
                    u[i][j] -= x;
                }
            }
        }
        r += 1;
    }
    Ok((to_sym(&u), to_sym(&a)))
}

// ---- Groebner bases ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum MonoOrder {
    Lex,
    GrLex,
    GrevLex,
}

type Mono = Vec<u32>;
type Poly = BTreeMap<Mono, Q>;

fn mono_cmp(o: MonoOrder, a: &Mono, b: &Mono) -> Ordering {
    let (da, db): (u32, u32) = (a.iter().sum(), b.iter().sum());
    match o {
        MonoOrder::Lex => a.cmp(b),
        MonoOrder::GrLex => da.cmp(&db).then_with(|| a.cmp(b)),
        MonoOrder::GrevLex => da.cmp(&db).then_with(|| {
            for (x, y) in a.iter().zip(b).rev() {
                if x != y {
                    return y.cmp(x);
                }
            }
            Ordering::Equal
        }),
    }
}

fn lead(o: MonoOrder, p: &Poly) -> Option<(Mono, Q)> {
    p.iter().max_by(|a, b| mono_cmp(o, a.0, b.0)).map(|(m, c)| (m.clone(), c.clone()))
}

fn to_qpoly(e: &Expr, vars: &[String]) -> R<Poly> {
    let p = super::tools::to_poly(e, vars).ok_or_else(|| LangError::new("Arguments must be polynomials.", 0))?;
    let mut out = Poly::new();
    for (m, c) in p {
        let q = c.as_num().and_then(|n| n.exact()).ok_or_else(|| LangError::new("Polynomial coefficients must be rational numbers.", 0))?;
        out.insert(m, q);
    }
    Ok(out)
}

fn sub_scaled(p: &mut Poly, d: &Poly, shift: &Mono, c: &Q) {
    for (m, dc) in d {
        let mm: Mono = m.iter().zip(shift).map(|(a, b)| a + b).collect();
        let v = p.get(&mm).cloned().unwrap_or_else(Q::zero) - c * dc;
        if v.is_zero() {
            p.remove(&mm);
        } else {
            p.insert(mm, v);
        }
    }
}

fn divides(a: &Mono, b: &Mono) -> bool {
    a.iter().zip(b).all(|(x, y)| x <= y)
}

/// Full reduction of p by the set g.
fn reduce(o: MonoOrder, p: &Poly, g: &[Poly]) -> Poly {
    let mut p = p.clone();
    let mut rem = Poly::new();
    while let Some((lm, lc)) = lead(o, &p) {
        let mut hit = false;
        for d in g {
            if let Some((dm, dc)) = lead(o, d) {
                if divides(&dm, &lm) {
                    let shift: Mono = lm.iter().zip(&dm).map(|(a, b)| a - b).collect();
                    sub_scaled(&mut p, d, &shift, &(&lc / &dc));
                    hit = true;
                    break;
                }
            }
        }
        if !hit {
            p.remove(&lm);
            rem.insert(lm, lc);
        }
    }
    rem
}

fn spoly(o: MonoOrder, f: &Poly, g: &Poly) -> Poly {
    let (fm, fc) = lead(o, f).unwrap();
    let (gm, gc) = lead(o, g).unwrap();
    let l: Mono = fm.iter().zip(&gm).map(|(a, b)| *a.max(b)).collect();
    let mut out = Poly::new();
    let sf: Mono = l.iter().zip(&fm).map(|(a, b)| a - b).collect();
    let sg: Mono = l.iter().zip(&gm).map(|(a, b)| a - b).collect();
    sub_scaled(&mut out, f, &sf, &(-Q::one() / &fc));
    sub_scaled(&mut out, g, &sg, &(Q::one() / &gc));
    out
}

fn gbasis(polys: &[Expr], vars: &[String], o: MonoOrder) -> R<Vec<Expr>> {
    let mut g: Vec<Poly> = polys.iter().map(|e| to_qpoly(e, vars)).collect::<R<Vec<_>>>()?.into_iter().filter(|p| !p.is_empty()).collect();
    let mut pairs: Vec<(usize, usize)> = (0..g.len()).flat_map(|i| (i + 1..g.len()).map(move |j| (i, j))).collect();
    let mut guard = 0;
    while let Some((i, j)) = pairs.pop() {
        guard += 1;
        if guard > 5000 {
            return rt("Groebner basis computation did not terminate.");
        }
        let s = spoly(o, &g[i], &g[j]);
        let r = reduce(o, &s, &g);
        if !r.is_empty() {
            let k = g.len();
            g.push(r);
            pairs.extend((0..k).map(|i| (i, k)));
        }
    }
    // Minimal basis: drop elements whose leading monomial another one divides.
    let mut keep: Vec<Poly> = Vec::new();
    for (k, p) in g.iter().enumerate() {
        let lm = lead(o, p).unwrap().0;
        let redundant = g.iter().enumerate().any(|(j, q)| {
            let qm = lead(o, q).unwrap().0;
            j != k && divides(&qm, &lm) && (qm != lm || j < k)
        });
        if !redundant {
            keep.push(p.clone());
        }
    }
    // Reduced basis.
    let mut reduced = Vec::with_capacity(keep.len());
    for k in 0..keep.len() {
        let others: Vec<Poly> = keep.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, p)| p.clone()).collect();
        let r = reduce(o, &keep[k], &others);
        reduced.push(r);
    }
    reduced.sort_by(|a, b| mono_cmp(o, &lead(o, b).unwrap().0, &lead(o, a).unwrap().0));
    Ok(reduced.iter().map(|p| primitive_expr(o, p, vars)).collect())
}

/// The polynomial scaled to integer coefficients without common factor, leading one positive.
fn primitive_expr(o: MonoOrder, p: &Poly, vars: &[String]) -> Expr {
    let l = p.values().fold(BigInt::one(), |acc, c| num_integer::Integer::lcm(&acc, c.denom()));
    let ints: Vec<BigInt> = p.values().map(|c| (c * Q::from_integer(l.clone())).to_integer()).collect();
    let mut g = ints.iter().fold(BigInt::zero(), |acc, x| num_integer::Integer::gcd(&acc, x));
    if lead(o, p).is_some_and(|(_, c)| c.is_negative()) {
        g = -g;
    }
    Expr::add(
        p.keys()
            .zip(&ints)
            .map(|(m, c)| {
                let mut fs = vec![Expr::Num(Num::from_big(Q::from_integer(c / &g)))];
                for (v, &k) in vars.iter().zip(m) {
                    if k > 0 {
                        fs.push(Expr::pow(Expr::var(v), Expr::int(k as i128)));
                    }
                }
                Expr::mul(fs)
            })
            .collect(),
    )
}
