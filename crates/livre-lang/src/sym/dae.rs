//! Differential-algebraic equation tools on abstract functions: `incidenceMatrix`,
//! `massMatrixForm`, `isLowIndexDAE`, `findDecoupledBlocks`, `reduceDifferentialOrder`,
//! `odeFunction` and `daeFunction`.

use super::expr::Rel;
use super::{algebra, calculus, print, Expr, SymArray};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["incidenceMatrix", "massMatrixForm", "isLowIndexDAE", "findDecoupledBlocks", "reduceDifferentialOrder", "odeFunction", "daeFunction"];

fn not_enough() -> LangError {
    LangError::new("Not enough input arguments.", 0)
}

/// The state variables: (function name, independent variable).
fn state_vars(v: &Value) -> R<Vec<(String, String)>> {
    SymArray::from_value(v)?
        .data
        .iter()
        .map(|e| match e {
            Expr::Fn(n, a) if a.len() == 1 && matches!(&a[0], Expr::Var(_)) => {
                let Expr::Var(t) = &a[0] else { unreachable!() };
                Ok((n.clone(), t.clone()))
            }
            _ => rt("Variables must be symbolic functions such as x(t)."),
        })
        .collect()
}

fn as_zero(e: &Expr) -> Expr {
    match e {
        Expr::Rel(Rel::Eq, l, r) => Expr::sub((**l).clone(), (**r).clone()),
        other => other.clone(),
    }
}

fn is_fn(e: &Expr, y: &str, t: &str) -> bool {
    matches!(e, Expr::Fn(n, a) if n == y && a.len() == 1 && matches!(&a[0], Expr::Var(v) if v == t))
}

fn deriv(y: &str, t: &str, k: usize) -> Expr {
    let f = Expr::Fn(y.into(), vec![Expr::var(t)]);
    if k == 0 {
        return f;
    }
    let mut a = vec![f];
    a.extend((0..k).map(|_| Expr::var(t)));
    Expr::Fn("diff".into(), a)
}

/// The highest derivative order of y in e (None when y does not occur).
fn order_in(e: &Expr, y: &str, t: &str) -> Option<usize> {
    match e {
        Expr::Fn(n, a) if n == "diff" && a.first().is_some_and(|f| is_fn(f, y, t)) => Some(a.len() - 1),
        other if is_fn(other, y, t) => Some(0),
        Expr::Add(xs) | Expr::Mul(xs) | Expr::Fn(_, xs) => xs.iter().filter_map(|x| order_in(x, y, t)).max(),
        Expr::Pow(a, b) | Expr::Rel(_, a, b) => [order_in(a, y, t), order_in(b, y, t)].into_iter().flatten().max(),
        _ => None,
    }
}

fn slot(k: usize, j: usize) -> String {
    format!("\u{0}s{j}_{k}")
}

/// y_j^(k) replaced by placeholder variables.
fn to_slots(e: &Expr, vars: &[(String, String)]) -> Expr {
    e.map(&|x| {
        for (j, (y, t)) in vars.iter().enumerate() {
            match x {
                Expr::Fn(n, a) if n == "diff" && a.first().is_some_and(|f| is_fn(f, y, t)) => return Some(Expr::var(&slot(a.len() - 1, j))),
                other if is_fn(other, y, t) => return Some(Expr::var(&slot(0, j))),
                _ => {}
            }
        }
        None
    })
}

fn logical_matrix(r: usize, c: usize, vals: Vec<f64>) -> Value {
    Value::Num(Matrix::from_dims(vec![r, c], vals))
}

fn incidence(eqs: &[Expr], vars: &[(String, String)]) -> Vec<Vec<bool>> {
    eqs.iter().map(|e| vars.iter().map(|(y, t)| order_in(e, y, t).is_some()).collect()).collect()
}

/// A maximum matching of equations to variables (augmenting paths).
fn matching(inc: &[Vec<bool>], n: usize) -> Vec<Option<usize>> {
    let mut var_of_eq: Vec<Option<usize>> = vec![None; inc.len()];
    let mut eq_of_var: Vec<Option<usize>> = vec![None; n];
    fn augment(i: usize, inc: &[Vec<bool>], seen: &mut [bool], var_of_eq: &mut [Option<usize>], eq_of_var: &mut [Option<usize>]) -> bool {
        for j in 0..inc[i].len() {
            if inc[i][j] && !seen[j] {
                seen[j] = true;
                if eq_of_var[j].is_none() || augment(eq_of_var[j].unwrap(), inc, seen, var_of_eq, eq_of_var) {
                    var_of_eq[i] = Some(j);
                    eq_of_var[j] = Some(i);
                    return true;
                }
            }
        }
        false
    }
    for i in 0..inc.len() {
        let mut seen = vec![false; n];
        augment(i, inc, &mut seen, &mut var_of_eq, &mut eq_of_var);
    }
    var_of_eq
}

/// Strongly connected components (Tarjan), each sorted; dependencies come first.
fn blocks(inc: &[Vec<bool>], n: usize) -> R<Vec<(Vec<usize>, Vec<usize>)>> {
    let m = matching(inc, n);
    if m.iter().any(|v| v.is_none()) || inc.len() != n {
        return rt("The system must have as many equations as variables, and each equation must be solvable for a distinct variable.");
    }
    let eq_of_var: Vec<usize> = {
        let mut e = vec![0; n];
        for (i, v) in m.iter().enumerate() {
            e[v.unwrap()] = i;
        }
        e
    };
    // Edge j -> k: the equation solved for j uses k.
    let adj: Vec<Vec<usize>> = (0..n).map(|j| (0..n).filter(|&k| k != j && inc[eq_of_var[j]][k]).collect()).collect();
    let (mut index, mut low, mut on, mut stack, mut comps) = (vec![usize::MAX; n], vec![0; n], vec![false; n], Vec::new(), Vec::new());
    let mut counter = 0;
    fn strong(v: usize, adj: &[Vec<usize>], counter: &mut usize, index: &mut [usize], low: &mut [usize], on: &mut [bool], stack: &mut Vec<usize>, comps: &mut Vec<Vec<usize>>) {
        index[v] = *counter;
        low[v] = *counter;
        *counter += 1;
        stack.push(v);
        on[v] = true;
        for &w in &adj[v] {
            if index[w] == usize::MAX {
                strong(w, adj, counter, index, low, on, stack, comps);
                low[v] = low[v].min(low[w]);
            } else if on[w] {
                low[v] = low[v].min(index[w]);
            }
        }
        if low[v] == index[v] {
            let mut c = Vec::new();
            while let Some(w) = stack.pop() {
                on[w] = false;
                c.push(w);
                if w == v {
                    break;
                }
            }
            comps.push(c);
        }
    }
    for v in 0..n {
        if index[v] == usize::MAX {
            strong(v, &adj, &mut counter, &mut index, &mut low, &mut on, &mut stack, &mut comps);
        }
    }
    Ok(comps
        .into_iter()
        .map(|mut vs| {
            vs.sort();
            let mut es: Vec<usize> = vs.iter().map(|&j| eq_of_var[j]).collect();
            es.sort();
            (es, vs)
        })
        .collect())
}

/// Code text of an expression with the states as in2(k) (and their derivatives as in3(k)).
fn handle_source(exprs: &[Expr], vars: &[(String, String)], params: &[String], dae: bool) -> String {
    let mut out = Vec::new();
    for e in exprs {
        let mut e = e.clone();
        for (j, (y, t)) in vars.iter().enumerate() {
            e = e.map(&|x| match x {
                Expr::Fn(n, a) if dae && n == "diff" && a.len() == 2 && a.first().is_some_and(|f| is_fn(f, y, t)) => Some(Expr::var(&format!("in3({})", j + 1))),
                other if is_fn(other, y, t) => Some(Expr::var(&format!("in2({})", j + 1))),
                _ => None,
            });
        }
        out.push(print::print(&e, print::Style::Code));
    }
    let t = vars.first().map(|(_, t)| t.clone()).unwrap_or_else(|| "t".into());
    let mut args = vec![t, "in2".to_string()];
    if dae {
        args.push("in3".into());
    }
    args.extend(params.iter().cloned());
    format!("@({})[{}]", args.join(","), out.join(";"))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let eqs: Vec<Expr> = SymArray::from_value(a.first().ok_or_else(not_enough)?)?.data.iter().map(as_zero).collect();
    let vars = state_vars(a.get(1).ok_or_else(not_enough)?)?;
    let n = vars.len();
    match name {
        "incidenceMatrix" => {
            let inc = incidence(&eqs, &vars);
            let mut data = vec![0.0; eqs.len() * n];
            for (i, row) in inc.iter().enumerate() {
                for (j, &b) in row.iter().enumerate() {
                    data[j * eqs.len() + i] = if b { 1.0 } else { 0.0 };
                }
            }
            Ok(Some(vec![logical_matrix(eqs.len(), n, data)]))
        }
        "massMatrixForm" => {
            // M*diff(X) = F for equations linear in the first derivatives.
            let m = eqs.len();
            let mut mm = vec![Expr::zero(); m * n];
            let mut f = Vec::with_capacity(m);
            for (i, e) in eqs.iter().enumerate() {
                let s = algebra::expand(&to_slots(e, &vars));
                let mut rest = s.clone();
                for j in 0..n {
                    let c = algebra::simplify(&calculus::diff(&s, &slot(1, j)));
                    if (0..n).any(|k| c.contains_var(&slot(1, k))) {
                        return rt("The equations must be linear in the first derivatives of the variables.");
                    }
                    mm[j * m + i] = c;
                    rest = rest.subs(&slot(1, j), &Expr::zero());
                }
                let mut fi = Expr::neg(rest);
                for (j, (y, t)) in vars.iter().enumerate() {
                    fi = fi.subs(&slot(0, j), &deriv(y, t, 0));
                }
                f.push(algebra::simplify(&fi));
            }
            let mut sm = SymArray::from_dims(vec![m, n], mm);
            sm.args.clear();
            Ok(Some(vec![Value::Sym(sm), Value::Sym(SymArray::from_dims(vec![m, 1], f))]))
        }
        "isLowIndexDAE" => {
            // The Jacobian with respect to the highest derivatives (the variables themselves
            // when they occur only algebraically) must be nonsingular.
            let m = eqs.len();
            if m != n {
                return Ok(Some(vec![Value::bool(false)]));
            }
            let mut rows: Vec<Vec<Expr>> = Vec::with_capacity(m);
            let orders: Vec<usize> = vars.iter().map(|(y, t)| eqs.iter().filter_map(|e| order_in(e, y, t)).max().unwrap_or(0)).collect();
            for e in &eqs {
                let s = to_slots(e, &vars);
                rows.push((0..n).map(|j| algebra::simplify(&calculus::diff(&s, &slot(orders[j], j)))).collect());
            }
            let rank = super::tools::rref(&mut rows).len();
            Ok(Some(vec![Value::bool(rank == n)]))
        }
        "findDecoupledBlocks" => {
            let inc = incidence(&eqs, &vars);
            let bl = blocks(&inc, n)?;
            let cell = |sel: &dyn Fn(&(Vec<usize>, Vec<usize>)) -> &Vec<usize>| {
                Value::Cell(CellArray::row(bl.iter().map(|b| Value::Num(Matrix::row(sel(b).iter().map(|&k| (k + 1) as f64).collect()))).collect()))
            };
            Ok(Some(vec![cell(&|b| &b.0), cell(&|b| &b.1)]))
        }
        "reduceDifferentialOrder" => {
            // Each derivative of order >= 2 gets new variables Dxt, D2xt, ... for the lower ones.
            let mut new_eqs: Vec<Expr> = eqs.clone();
            let mut all_vars: Vec<Expr> = vars.iter().map(|(y, t)| deriv(y, t, 0)).collect();
            let mut extra_eqs = Vec::new();
            let mut r_rows: Vec<(Expr, Expr)> = Vec::new();
            for (y, t) in &vars {
                let ord = eqs.iter().filter_map(|e| order_in(e, y, t)).max().unwrap_or(0);
                if ord < 2 {
                    continue;
                }
                let names: Vec<String> = (1..ord).map(|k| if k == 1 { format!("D{y}{t}") } else { format!("D{k}{y}{t}") }).collect();
                let named = |k: usize| Expr::Fn(names[k - 1].clone(), vec![Expr::var(t)]);
                // y^(k) for 2 <= k < ord is the new variable of order k; y^(ord) its last one's derivative.
                for e in new_eqs.iter_mut() {
                    *e = e.map(&|x| match x {
                        Expr::Fn(n, a) if n == "diff" && a.first().is_some_and(|f| is_fn(f, y, t)) && a.len() - 1 >= 2 => {
                            let k = a.len() - 1;
                            Some(if k < ord { named(k) } else { Expr::Fn("diff".into(), vec![named(ord - 1), Expr::var(t)]) })
                        }
                        _ => None,
                    });
                }
                for k in 1..ord {
                    let prev = if k == 1 { deriv(y, t, 1) } else { Expr::Fn("diff".into(), vec![named(k - 1), Expr::var(t)]) };
                    extra_eqs.push(Expr::sub(named(k), prev));
                    r_rows.push((named(k), deriv(y, t, k)));
                    all_vars.push(named(k));
                }
            }
            new_eqs.extend(extra_eqs);
            let ne = new_eqs.len();
            let nv = all_vars.len();
            let mut out = vec![Value::Sym(SymArray::from_dims(vec![ne, 1], new_eqs)), Value::Sym(SymArray::from_dims(vec![nv, 1], all_vars))];
            if nargout > 2 {
                let k = r_rows.len();
                let mut data = Vec::with_capacity(2 * k);
                data.extend(r_rows.iter().map(|r| r.0.clone()));
                data.extend(r_rows.iter().map(|r| r.1.clone()));
                out.push(Value::Sym(SymArray::from_dims(vec![k, if k == 0 { 0 } else { 2 }], data)));
            }
            Ok(Some(out))
        }
        "odeFunction" | "daeFunction" => {
            let mut params = Vec::new();
            for v in &a[2..] {
                if v.is_text() && !matches!(v, Value::Sym(_)) {
                    break;
                }
                for e in SymArray::from_value(v)?.data {
                    match e {
                        Expr::Var(p) => params.push(p),
                        _ => return rt("Parameters must be symbolic variables."),
                    }
                }
            }
            let exprs: Vec<Expr> = SymArray::from_value(&a[0])?.data.iter().map(as_zero).collect();
            let src = handle_source(&exprs, &vars, &params, name == "daeFunction");
            let ast = crate::parser::parse_expr(&src)?;
            Ok(Some(vec![it.eval(&ast)?]))
        }
        _ => Ok(None),
    }
}
