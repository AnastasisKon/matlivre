//! Fused evaluation of elementwise expressions on large arrays.
//!
//! `exp(-x.^2) .* sin(20*x) + sqrt(x+1)` evaluated operator by operator reads and writes a full
//! temporary array for every operator, and on arrays of millions of elements the time goes to
//! memory traffic. Here the whole expression is evaluated in blocks small enough to stay in the
//! cache, on several threads, and only the final result is written to memory.
//!
//! Only expressions whose meaning is plain double arithmetic are fused: variables holding real
//! double arrays (all of one size, or scalars), numeric literals, `+ - .* ./ .^`, unary minus,
//! `*` and `/` with a scalar operand, and a short list of elementwise functions. The arithmetic
//! of every operator is the same as in `ops`, so results are identical. Whenever something does
//! not fit (another class, complex results such as `sqrt` of a negative number or a negative base
//! with a fractional exponent, differing sizes) the function gives up and the ordinary evaluator
//! runs, so behaviour and error messages are unchanged. `MATLIVRE_NOFUSE=1` turns fusion off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use rayon::prelude::*;

use crate::ast::{BinOp, Expr, UnOp};
use crate::interp::Interpreter;
use crate::ops::PAR_MIN;
use crate::value::{Kind, Matrix, Value};

/// Elements per block: 4096 doubles per temporary stay in the first-level caches.
const BLOCK: usize = 4096;
/// Fewest operators (calls count) for which fusion pays.
const MIN_OPS: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Func {
    Sin,
    Cos,
    Tan,
    Sinh,
    Cosh,
    Tanh,
    Exp,
    Log,
    Log10,
    Log2,
    Sqrt,
    Atan,
    Abs,
    Round,
    Floor,
    Ceil,
}

impl Func {
    fn parse(name: &str) -> Option<Func> {
        Some(match name {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "sinh" => Func::Sinh,
            "cosh" => Func::Cosh,
            "tanh" => Func::Tanh,
            "exp" => Func::Exp,
            "log" => Func::Log,
            "log10" => Func::Log10,
            "log2" => Func::Log2,
            "sqrt" => Func::Sqrt,
            "atan" => Func::Atan,
            "abs" => Func::Abs,
            "round" => Func::Round,
            "floor" => Func::Floor,
            "ceil" => Func::Ceil,
            _ => return None,
        })
    }

    /// True for the functions that leave the real line for some real inputs.
    fn has_domain(self) -> bool {
        matches!(self, Func::Log | Func::Log10 | Func::Log2 | Func::Sqrt)
    }

    #[inline]
    fn apply(self, x: f64) -> f64 {
        match self {
            Func::Sin => x.sin(),
            Func::Cos => x.cos(),
            Func::Tan => x.tan(),
            Func::Sinh => x.sinh(),
            Func::Cosh => x.cosh(),
            Func::Tanh => x.tanh(),
            Func::Exp => x.exp(),
            Func::Log => x.ln(),
            Func::Log10 => x.log10(),
            Func::Log2 => x.log2(),
            Func::Sqrt => x.sqrt(),
            Func::Atan => x.atan(),
            Func::Abs => x.abs(),
            Func::Round => x.round(),
            Func::Floor => x.floor(),
            Func::Ceil => x.ceil(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Bop {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

enum Node {
    Const(f64),
    /// Index into the list of variables.
    Leaf(usize),
    Neg(usize),
    Bin(Bop, usize, usize),
    Call(Func, usize),
}

struct Plan {
    nodes: Vec<Node>,
    vars: Vec<String>,
    ops: usize,
    /// Matrix products and divisions (node, scalar operand may be on either side): elementwise
    /// only when the sizes turn out to make them so.
    markers: Vec<(usize, bool)>,
}

fn disabled() -> bool {
    static OFF: OnceLock<bool> = OnceLock::new();
    *OFF.get_or_init(|| std::env::var_os("MATLIVRE_NOFUSE").is_some())
}

/// Appends the nodes of `e`; `None` when `e` is not a fusable expression.
fn build(e: &Expr, it: &Interpreter, plan: &mut Plan) -> Option<usize> {
    let node = match e {
        Expr::Num(n) => Node::Const(*n),
        Expr::Ident(name) if it.get_var(name).is_none() => {
            // A few constants that are written as function calls without arguments.
            if it.user_function_defined(name) {
                return None;
            }
            Node::Const(match name.as_str() {
                "pi" => std::f64::consts::PI,
                "Inf" | "inf" => f64::INFINITY,
                "NaN" | "nan" => f64::NAN,
                "eps" => f64::EPSILON,
                _ => return None,
            })
        }
        Expr::Ident(name) => {
            let id = match plan.vars.iter().position(|v| v == name) {
                Some(i) => i,
                None => {
                    plan.vars.push(name.clone());
                    plan.vars.len() - 1
                }
            };
            Node::Leaf(id)
        }
        Expr::Unary(UnOp::Plus, x) => return build(x, it, plan),
        Expr::Unary(UnOp::Neg, x) => {
            let a = build(x, it, plan)?;
            plan.ops += 1;
            Node::Neg(a)
        }
        Expr::Binary(op, l, r) => {
            // `*` and `/` are elementwise only with a scalar operand, which is checked once the
            // sizes are known; the matrix products are left to the ordinary evaluator.
            let bop = match op {
                BinOp::Add => Bop::Add,
                BinOp::Sub => Bop::Sub,
                BinOp::Mul | BinOp::MatMul => Bop::Mul,
                BinOp::Div | BinOp::MatDiv => Bop::Div,
                BinOp::Pow => Bop::Pow,
                _ => return None,
            };
            let a = build(l, it, plan)?;
            let b = build(r, it, plan)?;
            plan.ops += 1;
            if matches!(op, BinOp::MatMul | BinOp::MatDiv) {
                plan.markers.push((plan.nodes.len(), *op == BinOp::MatMul));
            }
            Node::Bin(bop, a, b)
        }
        Expr::Index(base, args) if args.len() == 1 => {
            let Expr::Ident(name) = base.as_ref() else { return None };
            let f = Func::parse(name)?;
            if it.get_var(name).is_some() || it.user_function_defined(name) {
                return None;
            }
            let a = build(&args[0], it, plan)?;
            plan.ops += 1;
            Node::Call(f, a)
        }
        _ => return None,
    };
    plan.nodes.push(node);
    Some(plan.nodes.len() - 1)
}

/// Cheap structural test, before any variable is looked up: enough operators, and only the
/// node kinds that `build` accepts.
fn shape_ok(e: &Expr, ops: &mut usize) -> bool {
    match e {
        Expr::Num(_) | Expr::Ident(_) => true,
        Expr::Unary(UnOp::Plus, x) => shape_ok(x, ops),
        Expr::Unary(UnOp::Neg, x) => {
            *ops += 1;
            shape_ok(x, ops)
        }
        Expr::Binary(op, l, r) => {
            if !matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::MatMul | BinOp::Div | BinOp::MatDiv | BinOp::Pow) {
                return false;
            }
            *ops += 1;
            shape_ok(l, ops) && shape_ok(r, ops)
        }
        Expr::Index(base, args) if args.len() == 1 => match base.as_ref() {
            Expr::Ident(name) if Func::parse(name).is_some() => {
                *ops += 1;
                shape_ok(&args[0], ops)
            }
            _ => false,
        },
        _ => false,
    }
}

/// Evaluates `e` fused, or returns `None` to let the ordinary evaluator handle it.
pub fn try_eval(it: &Interpreter, e: &Expr) -> Option<Value> {
    let mut ops = 0;
    if !shape_ok(e, &mut ops) || ops < MIN_OPS || disabled() {
        return None;
    }
    let mut plan = Plan { nodes: Vec::new(), vars: Vec::new(), ops: 0, markers: Vec::new() };
    let root = build(e, it, &mut plan)?;
    if plan.ops < MIN_OPS {
        return None;
    }
    run(it, &plan.nodes, root, &plan.vars, &plan.markers)
}

fn run(it: &Interpreter, nodes: &[Node], root: usize, vars: &[String], markers: &[(usize, bool)]) -> Option<Value> {
    // The variables: real double arrays or scalars, all of one size.
    let mut mats: Vec<&Matrix> = Vec::with_capacity(vars.len());
    for name in vars {
        match it.get_var(name)? {
            Value::Num(m) if m.kind == Kind::Double && m.im.is_none() && !m.sparse => mats.push(m),
            _ => return None,
        }
    }
    let mut dims: Option<&Vec<usize>> = None;
    for m in &mats {
        if m.numel() != 1 {
            match dims {
                None => dims = Some(&m.dims),
                Some(d) if *d == m.dims => {}
                _ => return None,
            }
        }
    }
    let dims = dims?;
    let n: usize = dims.iter().product();
    if n < PAR_MIN {
        return None;
    }
    // Which nodes hold a single value (the same for every element).
    let mut scalar = vec![false; nodes.len()];
    for (k, node) in nodes.iter().enumerate() {
        scalar[k] = match node {
            Node::Const(_) => true,
            Node::Leaf(id) => mats[*id].numel() == 1,
            Node::Neg(a) | Node::Call(_, a) => scalar[*a],
            Node::Bin(_, a, b) => scalar[*a] && scalar[*b],
        };
    }
    for &(at, either) in markers {
        let Node::Bin(_, a, b) = &nodes[at] else { return None };
        // `A*B` is elementwise with a scalar on either side; `A/B` with a scalar divisor.
        if !(scalar[*b] || (either && scalar[*a])) {
            return None;
        }
    }
    // A power must have a scalar exponent: whether the result is real then depends on one value.
    for node in nodes {
        if let Node::Bin(Bop::Pow, _, b) = node {
            if !scalar[*b] {
                return None;
            }
        }
    }
    // Constant subtrees are folded once.
    let mut cval = vec![0.0f64; nodes.len()];
    for (k, node) in nodes.iter().enumerate() {
        if !scalar[k] {
            continue;
        }
        cval[k] = match node {
            Node::Const(c) => *c,
            Node::Leaf(id) => mats[*id].data[0],
            Node::Neg(a) => -cval[*a],
            Node::Call(f, a) => {
                if f.has_domain() && cval[*a] < 0.0 {
                    return None;
                }
                f.apply(cval[*a])
            }
            Node::Bin(op, a, b) => {
                let (x, y) = (cval[*a], cval[*b]);
                if *op == Bop::Pow && x < 0.0 && y.fract() != 0.0 && y.is_finite() {
                    return None;
                }
                scalar_bin(*op, x, y)
            }
        };
    }
    if scalar[root] {
        return None;
    }
    let abort = AtomicBool::new(false);
    let mut out = vec![0.0f64; n];
    out.par_chunks_mut(BLOCK).enumerate().for_each_init(
        || vec![vec![0.0f64; BLOCK]; nodes.len()],
        |scratch, (ci, chunk)| {
            if abort.load(Ordering::Relaxed) {
                return;
            }
            if !eval_block(nodes, root, &mats, &scalar, &cval, ci * BLOCK, scratch, chunk) {
                abort.store(true, Ordering::Relaxed);
            }
        },
    );
    if abort.load(Ordering::Relaxed) {
        return None;
    }
    Some(Value::Num(Matrix::from_dims(dims.clone(), out)))
}

#[inline]
fn scalar_bin(op: Bop, x: f64, y: f64) -> f64 {
    match op {
        Bop::Add => x + y,
        Bop::Sub => x - y,
        Bop::Mul => x * y,
        Bop::Div => x / y,
        // The ordinary evaluator squares with a multiplication.
        Bop::Pow => {
            if y == 2.0 {
                x * x
            } else {
                x.powf(y)
            }
        }
    }
}

/// An operand of a block: one value for all elements, or a slice.
#[derive(Clone, Copy)]
enum Opd<'a> {
    C(f64),
    S(&'a [f64]),
}

#[inline]
fn zip2(out: &mut [f64], a: Opd, b: Opd, f: impl Fn(f64, f64) -> f64) {
    match (a, b) {
        (Opd::S(x), Opd::S(y)) => {
            for ((o, &p), &q) in out.iter_mut().zip(x).zip(y) {
                *o = f(p, q);
            }
        }
        (Opd::S(x), Opd::C(c)) => {
            for (o, &p) in out.iter_mut().zip(x) {
                *o = f(p, c);
            }
        }
        (Opd::C(c), Opd::S(y)) => {
            for (o, &q) in out.iter_mut().zip(y) {
                *o = f(c, q);
            }
        }
        (Opd::C(c), Opd::C(d)) => out.fill(f(c, d)),
    }
}

/// Evaluates all nodes for the elements `start..start + chunk.len()`, writing the root into
/// `chunk`. Returns false when the block needs a complex result.
#[allow(clippy::too_many_arguments)]
fn eval_block(nodes: &[Node], root: usize, mats: &[&Matrix], scalar: &[bool], cval: &[f64], start: usize, scratch: &mut [Vec<f64>], chunk: &mut [f64]) -> bool {
    let len = chunk.len();
    for k in 0..=root {
        if scalar[k] {
            continue;
        }
        if let Node::Leaf(_) = nodes[k] {
            continue;
        }
        let (before, rest) = scratch.split_at_mut(k);
        let before: &[Vec<f64>] = before;
        let operand = |j: usize| -> Opd {
            if scalar[j] {
                Opd::C(cval[j])
            } else if let Node::Leaf(id) = nodes[j] {
                Opd::S(&mats[id].data[start..start + len])
            } else {
                Opd::S(&before[j][..len])
            }
        };
        let out: &mut [f64] = if k == root { &mut *chunk } else { &mut rest[0][..len] };
        match &nodes[k] {
            Node::Neg(a) => match operand(*a) {
                Opd::S(x) => {
                    for (o, &p) in out.iter_mut().zip(x) {
                        *o = -p;
                    }
                }
                Opd::C(c) => out.fill(-c),
            },
            Node::Call(f, a) => {
                let Opd::S(x) = operand(*a) else { continue };
                if f.has_domain() && x.iter().any(|&p| p < 0.0) {
                    return false;
                }
                match f {
                    Func::Sin => map1(out, x, f64::sin),
                    Func::Cos => map1(out, x, f64::cos),
                    Func::Tan => map1(out, x, f64::tan),
                    Func::Sinh => map1(out, x, f64::sinh),
                    Func::Cosh => map1(out, x, f64::cosh),
                    Func::Tanh => map1(out, x, f64::tanh),
                    Func::Exp => map1(out, x, f64::exp),
                    Func::Log => map1(out, x, f64::ln),
                    Func::Log10 => map1(out, x, f64::log10),
                    Func::Log2 => map1(out, x, f64::log2),
                    Func::Sqrt => map1(out, x, f64::sqrt),
                    Func::Atan => map1(out, x, f64::atan),
                    Func::Abs => map1(out, x, f64::abs),
                    Func::Round => map1(out, x, f64::round),
                    Func::Floor => map1(out, x, f64::floor),
                    Func::Ceil => map1(out, x, f64::ceil),
                }
            }
            Node::Bin(op, a, b) => {
                let (x, y) = (operand(*a), operand(*b));
                match op {
                    Bop::Add => zip2(out, x, y, |p, q| p + q),
                    Bop::Sub => zip2(out, x, y, |p, q| p - q),
                    Bop::Mul => zip2(out, x, y, |p, q| p * q),
                    Bop::Div => zip2(out, x, y, |p, q| p / q),
                    Bop::Pow => {
                        // The exponent is a scalar (checked when planning).
                        let Opd::C(e) = y else { return false };
                        if e == 2.0 {
                            zip2(out, x, y, |p, _| p * p);
                        } else {
                            if e.fract() != 0.0 && e.is_finite() {
                                if let Opd::S(s) = x {
                                    if s.iter().any(|&p| p < 0.0) {
                                        return false;
                                    }
                                }
                            }
                            zip2(out, x, y, |p, q| p.powf(q));
                        }
                    }
                }
            }
            Node::Const(_) | Node::Leaf(_) => {}
        }
    }
    true
}

#[inline]
fn map1(out: &mut [f64], x: &[f64], f: impl Fn(f64) -> f64) {
    for (o, &p) in out.iter_mut().zip(x) {
        *o = f(p);
    }
}
