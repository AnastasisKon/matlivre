//! More differential equation solvers: stiff solvers (`ode15s`, `ode23s`, `ode23t`, `ode23tb`)
//! with mass matrices and DAEs, high-order nonstiff solvers (`ode113`, `ode78`, `ode89`), fully
//! implicit equations (`ode15i`, `decic`), `odextend`, delay equations (`dde23`, `ddesd`,
//! `ddensd`), boundary value problems (`bvp4c`, `bvp5c`, `bvpinit`, `bvpxtend`) and parabolic
//! PDEs in one space dimension (`pdepe`, `pdeval`).
//!
//! `ode23s` is the Rosenbrock pair of Shampine and Reichelt. `ode15s`, `ode23t` and `ode23tb`
//! share a TR-BDF2 integrator (a trapezoidal stage followed by BDF2) with Newton iterations;
//! `ode113`, `ode78` and `ode89` use Gragg-Bulirsch-Stoer extrapolation of orders 6, 8 and 10.
//! Step sequences differ from MATLAB's, so results agree to the requested tolerances rather
//! than digit for digit.

use crate::builtins::{arg, one, Ret};
use crate::builtins_funfun::{
    blank_options, call1, col, cols_to_matrix, eps_of, fmt, func_text, is_func, make_options, opt, opt_num, opt_text, optget, options_arg, row,
    rows_to_matrix, warn, ODE_FIELDS,
};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "ode15s", "ode23s", "ode23t", "ode23tb", "ode113", "ode78", "ode89", "ode15i", "decic", "odextend", "dde23", "ddesd", "ddensd", "ddeget",
    "ddeset", "bvp4c", "bvp5c", "bvpinit", "bvpget", "bvpset", "bvpxtend", "pdepe", "pdeval",
];

const DDE_FIELDS: &[&str] = &["AbsTol", "Events", "InitialStep", "InitialY", "Jumps", "MaxStep", "NormControl", "OutputFcn", "OutputSel", "Refine", "RelTol", "Stats"];
const BVP_FIELDS: &[&str] = &["AbsTol", "RelTol", "SingularTerm", "FJacobian", "BCJacobian", "Stats", "Nmax", "Vectorized"];

// ---- dense linear algebra ------------------------------------------------------------------

/// LU factorization with partial pivoting of a column-major n-by-n matrix.
struct Lu {
    a: Vec<f64>,
    piv: Vec<usize>,
    n: usize,
}

impl Lu {
    fn new(mut a: Vec<f64>, n: usize) -> Option<Lu> {
        let mut piv: Vec<usize> = (0..n).collect();
        let scale = a.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(f64::MIN_POSITIVE);
        for k in 0..n {
            let p = (k..n).max_by(|&i, &j| a[k * n + i].abs().total_cmp(&a[k * n + j].abs()))?;
            if a[k * n + p].abs() <= scale * 1e-14 {
                return None;
            }
            if p != k {
                piv.swap(k, p);
                for j in 0..n {
                    a.swap(j * n + k, j * n + p);
                }
            }
            let d = a[k * n + k];
            for i in k + 1..n {
                a[k * n + i] /= d;
            }
            for j in k + 1..n {
                let akj = a[j * n + k];
                if akj != 0.0 {
                    for i in k + 1..n {
                        a[j * n + i] -= a[k * n + i] * akj;
                    }
                }
            }
        }
        Some(Lu { a, piv, n })
    }

    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = self.n;
        let mut x: Vec<f64> = self.piv.iter().map(|&p| b[p]).collect();
        for j in 0..n {
            let xj = x[j];
            for i in j + 1..n {
                x[i] -= self.a[j * n + i] * xj;
            }
        }
        for j in (0..n).rev() {
            x[j] /= self.a[j * n + j];
            let xj = x[j];
            for i in 0..j {
                x[i] -= self.a[j * n + i] * xj;
            }
        }
        x
    }
}

fn matvec(m: &[f64], n: usize, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; n];
    for j in 0..n {
        if x[j] != 0.0 {
            for i in 0..n {
                y[i] += m[j * n + i] * x[j];
            }
        }
    }
    y
}

fn identity(n: usize) -> Vec<f64> {
    let mut m = vec![0.0; n * n];
    for i in 0..n {
        m[i * n + i] = 1.0;
    }
    m
}

/// Cubic Hermite interpolation on [t0, t1] and its derivative.
fn hermite(t0: f64, y0: &[f64], p0: &[f64], t1: f64, y1: &[f64], p1: &[f64], tq: f64) -> (Vec<f64>, Vec<f64>) {
    let h = t1 - t0;
    if h == 0.0 {
        return (y0.to_vec(), p0.to_vec());
    }
    let s = (tq - t0) / h;
    let (s2, s3) = (s * s, s * s * s);
    let (h00, h10, h01, h11) = (2.0 * s3 - 3.0 * s2 + 1.0, s3 - 2.0 * s2 + s, -2.0 * s3 + 3.0 * s2, s3 - s2);
    let (d00, d10, d01, d11) = ((6.0 * s2 - 6.0 * s) / h, 3.0 * s2 - 4.0 * s + 1.0, (-6.0 * s2 + 6.0 * s) / h, 3.0 * s2 - 2.0 * s);
    let y = (0..y0.len()).map(|i| h00 * y0[i] + h10 * h * p0[i] + h01 * y1[i] + h11 * h * p1[i]).collect();
    let p = (0..y0.len()).map(|i| d00 * y0[i] + d10 * p0[i] + d01 * y1[i] + d11 * p1[i]).collect();
    (y, p)
}

/// A computed solution: points, values and derivatives, for interpolation.
#[derive(Default, Clone)]
struct Traj {
    t: Vec<f64>,
    y: Vec<Vec<f64>>,
    yp: Vec<Vec<f64>>,
}

impl Traj {
    fn push(&mut self, t: f64, y: Vec<f64>, yp: Vec<f64>) {
        self.t.push(t);
        self.y.push(y);
        self.yp.push(yp);
    }

    /// The interval containing tq (for increasing or decreasing t).
    fn interval(&self, tq: f64) -> usize {
        let n = self.t.len();
        if n < 2 {
            return 0;
        }
        let dir = if self.t[n - 1] >= self.t[0] { 1.0 } else { -1.0 };
        let (mut lo, mut hi) = (0usize, n - 1);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if dir * (tq - self.t[mid]) >= 0.0 { lo = mid } else { hi = mid }
        }
        lo
    }

    fn eval(&self, tq: f64) -> (Vec<f64>, Vec<f64>) {
        let n = self.t.len();
        if n == 1 {
            return (self.y[0].clone(), self.yp[0].clone());
        }
        let k = self.interval(tq).min(n - 2);
        hermite(self.t[k], &self.y[k], &self.yp[k], self.t[k + 1], &self.y[k + 1], &self.yp[k + 1], tq)
    }
}

// ---- options and the user's system -----------------------------------------------------------

struct Tol {
    rtol: f64,
    thr: Vec<f64>,
}

impl Tol {
    fn new(it: &mut Interpreter, o: Option<&StructArray>, n: usize, f: &str) -> R<Tol> {
        let mut rtol = opt_num(o, "RelTol", 1e-3)?;
        if rtol < 100.0 * f64::EPSILON {
            rtol = 100.0 * f64::EPSILON;
            warn(it, fmt("RelTol has been increased to %g.", &[rtol]));
        }
        let atol: Vec<f64> = match opt(o, "AbsTol") {
            Some(v) => v.to_matrix()?.data,
            None => vec![1e-6],
        };
        if atol.len() != 1 && atol.len() != n {
            return rt(format!("Solving {f} requires a scalar AbsTol, or a vector AbsTol of length {n}"));
        }
        let thr = (0..n).map(|i| atol[if atol.len() == 1 { 0 } else { i }] / rtol).collect();
        Ok(Tol { rtol, thr })
    }

    /// The weighted error of `e` relative to the larger of |y| and |ynew|.
    fn err(&self, e: &[f64], y: &[f64], ynew: &[f64]) -> f64 {
        (0..e.len()).map(|i| (e[i] / y[i].abs().max(ynew[i].abs()).max(self.thr[i])).abs()).fold(0.0, |a: f64, b| if b.is_nan() { f64::NAN } else { a.max(b) })
    }
}

#[derive(Default)]
struct Counts {
    nsteps: usize,
    nfailed: usize,
    nfevals: usize,
    npds: usize,
    ndecomps: usize,
    nsolves: usize,
}

/// A first-order system M(t,y) y' = f(t,y).
trait System {
    fn n(&self) -> usize;
    fn f(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Vec<f64>>;
    /// The mass matrix (None for the identity).
    fn mass(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Option<Vec<f64>>>;
    fn mass_depends_on_y(&self) -> bool;
    /// An analytic Jacobian of f, if one was given.
    fn jacobian(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Option<Vec<f64>>>;
    /// Half bandwidth of the Jacobian, for grouped finite differences.
    fn band(&self) -> Option<usize> {
        None
    }
    fn evals(&self) -> usize;
}

enum Mass {
    Identity,
    Const(Vec<f64>),
    Fun { f: Value, with_y: bool },
}

struct UserSys {
    f: Value,
    extra: Vec<Value>,
    n: usize,
    text: String,
    nfevals: usize,
    mass: Mass,
    jac: Option<Value>,
}

impl UserSys {
    fn new(it: &mut Interpreter, f: &Value, extra: Vec<Value>, n: usize, o: Option<&StructArray>) -> R<UserSys> {
        let mass = match opt(o, "Mass") {
            None => Mass::Identity,
            Some(v) if is_func(v) => Mass::Fun { f: v.clone(), with_y: opt_text(o, "MStateDependence", "weak")? != "none" },
            Some(v) => {
                let m = v.to_matrix()?;
                if m.rows() != n || m.cols() != n {
                    return rt(format!("The mass matrix must be {n}-by-{n}."));
                }
                Mass::Const(m.data)
            }
        };
        Ok(UserSys { f: f.clone(), extra, n, text: func_text(it, f), nfevals: 0, mass, jac: opt(o, "Jacobian").cloned() })
    }
}

impl System for UserSys {
    fn n(&self) -> usize {
        self.n
    }

    fn f(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Vec<f64>> {
        let mut args = vec![Value::num(t), col(y.to_vec())];
        args.extend_from_slice(&self.extra);
        let m = call1(it, &self.f, args)?.to_matrix()?;
        self.nfevals += 1;
        if m.numel() != self.n {
            return rt(format!(
                "{} returns a vector of length {}, but the length of initial conditions vector is {}. The vector returned by {} and the initial conditions vector must have the same number of elements.",
                self.text,
                m.numel(),
                self.n,
                self.text
            ));
        }
        Ok(m.data)
    }

    fn mass(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Option<Vec<f64>>> {
        Ok(match &self.mass {
            Mass::Identity => None,
            Mass::Const(m) => Some(m.clone()),
            Mass::Fun { f, with_y } => {
                let mut args = vec![Value::num(t)];
                if *with_y {
                    args.push(col(y.to_vec()));
                }
                args.extend_from_slice(&self.extra);
                let m = call1(it, f, args)?.to_matrix()?;
                if m.numel() != self.n * self.n {
                    return rt(format!("The mass matrix must be {0}-by-{0}.", self.n));
                }
                Some(m.data)
            }
        })
    }

    fn mass_depends_on_y(&self) -> bool {
        matches!(self.mass, Mass::Fun { with_y: true, .. })
    }

    fn jacobian(&mut self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<Option<Vec<f64>>> {
        let Some(j) = &self.jac else { return Ok(None) };
        let m = if is_func(j) {
            let mut args = vec![Value::num(t), col(y.to_vec())];
            args.extend_from_slice(&self.extra);
            call1(it, j, args)?.to_matrix()?
        } else {
            j.to_matrix()?
        };
        if m.numel() != self.n * self.n {
            return rt(format!("The Jacobian must be {0}-by-{0}.", self.n));
        }
        Ok(Some(m.data))
    }

    fn evals(&self) -> usize {
        self.nfevals
    }
}

/// Finite-difference Jacobian of f at (t, y), grouping columns for banded systems.
fn num_jacobian(sys: &mut dyn System, it: &mut Interpreter, t: f64, y: &[f64], f0: &[f64], thr: &[f64]) -> R<Vec<f64>> {
    if let Some(j) = sys.jacobian(it, t, y)? {
        return Ok(j);
    }
    let n = y.len();
    let mut jac = vec![0.0; n * n];
    let step = |j: usize| {
        let d = f64::EPSILON.sqrt() * y[j].abs().max(thr[j]).max(1e-8);
        // A representable step.
        (y[j] + d) - y[j]
    };
    match sys.band() {
        Some(b) if 2 * b + 1 < n => {
            let groups = 2 * b + 1;
            for g in 0..groups {
                let mut yd = y.to_vec();
                let cols: Vec<usize> = (g..n).step_by(groups).collect();
                for &j in &cols {
                    yd[j] += step(j);
                }
                let fd = sys.f(it, t, &yd)?;
                for &j in &cols {
                    let d = yd[j] - y[j];
                    for i in j.saturating_sub(b)..(j + b + 1).min(n) {
                        jac[j * n + i] = (fd[i] - f0[i]) / d;
                    }
                }
            }
        }
        _ => {
            for j in 0..n {
                let mut yd = y.to_vec();
                let d = step(j);
                yd[j] += d;
                let fd = sys.f(it, t, &yd)?;
                for i in 0..n {
                    jac[j * n + i] = (fd[i] - f0[i]) / d;
                }
            }
        }
    }
    Ok(jac)
}

// ---- events --------------------------------------------------------------------------------

struct Events {
    f: Value,
    extra: Vec<Value>,
}

impl Events {
    fn eval(&self, it: &mut Interpreter, t: f64, y: &[f64]) -> R<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        let mut args = vec![Value::num(t), col(y.to_vec())];
        args.extend_from_slice(&self.extra);
        let r = it.call_func_value(&self.f, args, 3)?;
        let get = |i: usize| -> R<Vec<f64>> { Ok(r.get(i).map(|v| v.to_matrix()).transpose()?.map(|m| m.data).unwrap_or_default()) };
        let v = get(0)?;
        let mut term = get(1)?;
        let mut dir = get(2)?;
        term.resize(v.len(), 0.0);
        dir.resize(v.len(), 0.0);
        Ok((v, term, dir))
    }
}

#[derive(Default)]
struct EventLog {
    te: Vec<f64>,
    ye: Vec<Vec<f64>>,
    ie: Vec<f64>,
}

/// Events between the last two points of `traj`; returns the terminal event time if any.
fn locate_events(it: &mut Interpreter, ev: &Events, left: &mut Vec<f64>, traj: &Traj, log: &mut EventLog) -> R<Option<(f64, Vec<f64>)>> {
    let k = traj.t.len() - 1;
    let (t0, t1) = (traj.t[k - 1], traj.t[k]);
    let seg = |tq: f64| hermite(t0, &traj.y[k - 1], &traj.yp[k - 1], t1, &traj.y[k], &traj.yp[k], tq).0;
    let (right, term, dir) = ev.eval(it, t1, &traj.y[k])?;
    let tol = (128.0 * eps_of(t0).max(eps_of(t1))).min((t1 - t0).abs());
    let mut found: Vec<(f64, Vec<f64>, usize, bool)> = Vec::new();
    for i in 0..right.len().min(left.len()) {
        let (a, b) = (left[i], right[i]);
        if a == 0.0 || (a.signum() == b.signum() && b != 0.0) || dir[i] * (b - a) < 0.0 {
            continue;
        }
        let (mut tl, mut fl, mut tr, mut fr) = (t0, a, t1, b);
        let mut yr = traj.y[k].clone();
        let mut side = 0i32;
        while (tr - tl).abs() > tol && fr != 0.0 {
            let mut tm = tr - fr * (tr - tl) / (fr - fl);
            if !(tm > tl.min(tr) && tm < tl.max(tr)) {
                tm = 0.5 * (tl + tr);
            }
            let ym = seg(tm);
            let fm = ev.eval(it, tm, &ym)?.0[i];
            if fm.signum() == fl.signum() && fm != 0.0 {
                tl = tm;
                fl = fm;
                if side == -1 {
                    fr /= 2.0;
                }
                side = -1;
            } else {
                tr = tm;
                fr = fm;
                yr = ym;
                if side == 1 {
                    fl /= 2.0;
                }
                side = 1;
            }
        }
        found.push((tr, yr, i, term[i] != 0.0));
    }
    *left = right;
    let sign = if t1 > t0 { 1.0 } else { -1.0 };
    found.sort_by(|x, y| (sign * x.0).total_cmp(&(sign * y.0)));
    for (te, ye, i, terminal) in found {
        log.te.push(te);
        log.ye.push(ye.clone());
        log.ie.push(i as f64 + 1.0);
        if terminal {
            return Ok(Some((te, ye)));
        }
    }
    Ok(None)
}

// ---- integrators -----------------------------------------------------------------------------

struct Setup {
    t0: f64,
    tfinal: f64,
    tdir: f64,
    hmax: f64,
    h0: Option<f64>,
    /// Points the integrator must step to exactly (output times for the high-order solvers).
    stops: Vec<f64>,
}

fn initial_step(s: &Setup, tol: &Tol, y: &[f64], yp: &[f64], pow: f64) -> f64 {
    let hmin = 16.0 * eps_of(s.t0);
    if let Some(h) = s.h0 {
        return s.hmax.min(hmin.max(h));
    }
    let mut absh = s.hmax.min((s.tfinal - s.t0).abs());
    let rh = (0..y.len()).map(|i| (yp[i] / y[i].abs().max(tol.thr[i])).abs()).fold(0.0, f64::max) / (0.8 * tol.rtol.powf(pow));
    if absh * rh > 1.0 {
        absh = 1.0 / rh;
    }
    absh.max(hmin)
}

fn too_small(it: &mut Interpreter, t: f64, hmin: f64) {
    warn(
        it,
        format!(
            "Failure at t={}.  Unable to meet integration tolerances without reducing the step size below the smallest value allowed ({}) at time t.",
            fmt("%e", &[t]),
            fmt("%e", &[hmin])
        ),
    );
}

/// Clips a step to the final time and the next required stop; returns (h, last step).
fn clip(s: &Setup, t: f64, absh: f64) -> (f64, bool) {
    let mut h = s.tdir * absh;
    let mut target = s.tfinal;
    if let Some(&st) = s.stops.iter().find(|&&st| s.tdir * (st - t) > 0.0) {
        target = st;
    }
    if 1.1 * absh >= (target - t).abs() {
        h = target - t;
    }
    (h, t + h == s.tfinal || h == s.tfinal - t)
}

/// Gragg-Bulirsch-Stoer step of size `h` with `k` extrapolation columns: (value, error).
fn gbs_step(sys: &mut dyn System, it: &mut Interpreter, t: f64, y: &[f64], f0: &[f64], h: f64, k: usize) -> R<(Vec<f64>, Vec<f64>)> {
    let n = y.len();
    let mut table: Vec<Vec<Vec<f64>>> = Vec::with_capacity(k);
    for j in 0..k {
        let steps = 2 * (j + 1);
        let hs = h / steps as f64;
        let mut zprev = y.to_vec();
        let mut z: Vec<f64> = (0..n).map(|i| y[i] + hs * f0[i]).collect();
        for m in 1..steps {
            let fz = sys.f(it, t + m as f64 * hs, &z)?;
            let znext: Vec<f64> = (0..n).map(|i| zprev[i] + 2.0 * hs * fz[i]).collect();
            zprev = z;
            z = znext;
        }
        let mut rowj = vec![z];
        for l in 1..=j {
            let ratio = (steps as f64 / (2 * (j + 1 - l)) as f64).powi(2);
            let prev = &table[j - 1][l - 1];
            let cur = &rowj[l - 1];
            let next: Vec<f64> = (0..n).map(|i| cur[i] + (cur[i] - prev[i]) / (ratio - 1.0)).collect();
            rowj.push(next);
        }
        table.push(rowj);
    }
    let last = &table[k - 1];
    let best = last[k - 1].clone();
    let err = (0..n).map(|i| best[i] - last[k - 2][i]).collect();
    Ok((best, err))
}

fn explicit_gbs(it: &mut Interpreter, sys: &mut dyn System, s: &Setup, tol: &Tol, y0: Vec<f64>, k: usize, ev: Option<&Events>, log: &mut EventLog, c: &mut Counts) -> R<Traj> {
    let pow = 1.0 / (2 * k - 1) as f64;
    let mut traj = Traj::default();
    let mut t = s.t0;
    let mut y = y0;
    let mut f0 = sys.f(it, t, &y)?;
    traj.push(t, y.clone(), f0.clone());
    let mut absh = initial_step(s, tol, &y, &f0, pow);
    let mut left = match ev {
        Some(e) => e.eval(it, t, &y)?.0,
        None => Vec::new(),
    };
    loop {
        let hmin = 16.0 * eps_of(t);
        absh = s.hmax.min(hmin.max(absh));
        let (mut h, mut last) = clip(s, t, absh);
        let (ynew, err) = loop {
            let (yn, e) = gbs_step(sys, it, t, &y, &f0, h, k)?;
            let err = tol.err(&e, &y, &yn);
            if err <= tol.rtol {
                break (yn, err);
            }
            c.nfailed += 1;
            if h.abs() <= hmin {
                too_small(it, t, hmin);
                return Ok(traj);
            }
            let fac = if err.is_nan() { 0.25 } else { (0.9 * (tol.rtol / err).powf(pow)).clamp(0.2, 0.9) };
            h *= fac;
            last = false;
        };
        c.nsteps += 1;
        let tnew = t + h;
        let fnew = sys.f(it, tnew, &ynew)?;
        traj.push(tnew, ynew.clone(), fnew.clone());
        if let Some(e) = ev {
            if let Some((te, ye)) = locate_events(it, e, &mut left, &traj, log)? {
                *traj.t.last_mut().unwrap() = te;
                *traj.y.last_mut().unwrap() = ye.clone();
                *traj.yp.last_mut().unwrap() = sys.f(it, te, &ye)?;
                break;
            }
        }
        if last || s.tdir * (tnew - s.tfinal) >= 0.0 {
            break;
        }
        absh = h.abs() * if err == 0.0 { 4.0 } else { (0.9 * (tol.rtol / err).powf(pow)).clamp(0.2, 4.0) };
        t = tnew;
        y = ynew;
        f0 = fnew;
    }
    c.nfevals = sys.evals();
    Ok(traj)
}

/// `ode23s`: the Rosenbrock (2,3) pair with a constant mass matrix.
fn rosenbrock(it: &mut Interpreter, sys: &mut dyn System, s: &Setup, tol: &Tol, y0: Vec<f64>, ev: Option<&Events>, log: &mut EventLog, c: &mut Counts) -> R<Traj> {
    let n = sys.n();
    let d = 1.0 / (2.0 + 2f64.sqrt());
    let e32 = 6.0 + 2f64.sqrt();
    let pow = 1.0 / 3.0;
    let mass = sys.mass(it, s.t0, &y0)?;
    let mlu = mass.as_ref().and_then(|m| Lu::new(m.clone(), n));
    if mass.is_some() && mlu.is_none() {
        return rt("ode23s cannot solve problems with a singular mass matrix.");
    }
    let mv = |x: &[f64]| -> Vec<f64> { mass.as_ref().map(|m| matvec(m, n, x)).unwrap_or_else(|| x.to_vec()) };
    let mut traj = Traj::default();
    let mut t = s.t0;
    let mut y = y0;
    let mut f0 = sys.f(it, t, &y)?;
    let to_yp = |f: &[f64]| -> Vec<f64> { mlu.as_ref().map(|l| l.solve(f)).unwrap_or_else(|| f.to_vec()) };
    traj.push(t, y.clone(), to_yp(&f0));
    let mut absh = initial_step(s, tol, &y, &to_yp(&f0), pow);
    let mut left = match ev {
        Some(e) => e.eval(it, t, &y)?.0,
        None => Vec::new(),
    };
    loop {
        let hmin = 16.0 * eps_of(t);
        absh = s.hmax.min(hmin.max(absh));
        let (mut h, mut last) = clip(s, t, absh);
        let jac = num_jacobian(sys, it, t, &y, &f0, &tol.thr)?;
        c.npds += 1;
        let dt = f64::EPSILON.sqrt() * t.abs().max(h.abs()).max(1e-8);
        let fdt = sys.f(it, t + dt, &y)?;
        let tder: Vec<f64> = (0..n).map(|i| (fdt[i] - f0[i]) / dt).collect();
        let (ynew, fnew, err) = loop {
            let mut w = mass.clone().unwrap_or_else(|| identity(n));
            for k in 0..n * n {
                w[k] -= h * d * jac[k];
            }
            c.ndecomps += 1;
            let Some(lu) = Lu::new(w, n) else {
                h *= 0.5;
                last = false;
                if h.abs() <= hmin {
                    too_small(it, t, hmin);
                    return Ok(traj);
                }
                continue;
            };
            let k1 = lu.solve(&(0..n).map(|i| f0[i] + h * d * tder[i]).collect::<Vec<_>>());
            let f1 = sys.f(it, t + 0.5 * h, &(0..n).map(|i| y[i] + 0.5 * h * k1[i]).collect::<Vec<_>>())?;
            let mk1 = mv(&k1);
            let k2: Vec<f64> = lu.solve(&(0..n).map(|i| f1[i] - mk1[i]).collect::<Vec<_>>()).iter().zip(&k1).map(|(a, b)| a + b).collect();
            let tn = t + h;
            let yn: Vec<f64> = (0..n).map(|i| y[i] + h * k2[i]).collect();
            let f2 = sys.f(it, tn, &yn)?;
            let mk2 = mv(&k2);
            let k3 = lu.solve(&(0..n).map(|i| f2[i] - e32 * (mk2[i] - f1[i]) - 2.0 * (mk1[i] - f0[i]) + h * d * tder[i]).collect::<Vec<_>>());
            c.nsolves += 3;
            let e: Vec<f64> = (0..n).map(|i| h / 6.0 * (k1[i] - 2.0 * k2[i] + k3[i])).collect();
            let err = tol.err(&e, &y, &yn);
            if err <= tol.rtol {
                break (yn, f2, err);
            }
            c.nfailed += 1;
            if h.abs() <= hmin {
                too_small(it, t, hmin);
                return Ok(traj);
            }
            h *= if err.is_nan() { 0.5 } else { (0.8 * (tol.rtol / err).powf(pow)).clamp(0.5, 0.9) };
            last = false;
        };
        c.nsteps += 1;
        let tnew = t + h;
        traj.push(tnew, ynew.clone(), to_yp(&fnew));
        if let Some(e) = ev {
            if let Some((te, ye)) = locate_events(it, e, &mut left, &traj, log)? {
                *traj.t.last_mut().unwrap() = te;
                *traj.y.last_mut().unwrap() = ye;
                break;
            }
        }
        if last || s.tdir * (tnew - s.tfinal) >= 0.0 {
            break;
        }
        absh = h.abs() * if err == 0.0 { 5.0 } else { (0.8 * (tol.rtol / err).powf(pow)).clamp(0.2, 5.0) };
        t = tnew;
        y = ynew;
        f0 = fnew;
    }
    c.nfevals = sys.evals();
    Ok(traj)
}

/// TR-BDF2 for M(t,y) y' = f(t,y), including singular M (index-1 DAEs).
fn trbdf2(it: &mut Interpreter, sys: &mut dyn System, s: &Setup, tol: &Tol, y0: Vec<f64>, ev: Option<&Events>, log: &mut EventLog, c: &mut Counts) -> R<Traj> {
    let n = sys.n();
    let g = 2.0 - 2f64.sqrt();
    let d = g / 2.0;
    let ca = 1.0 / (g * (2.0 - g));
    let cb = (1.0 - g).powi(2) / (g * (2.0 - g));
    // Quadrature weights over nodes 0, g, 1 (exact for quadratics), for the error estimate.
    let w2 = (1.0 / 6.0) / (g * (1.0 - g));
    let w3 = 0.5 - w2 * g;
    let w1 = 1.0 - w2 - w3;
    let pow = 1.0 / 3.0;
    let varies = sys.mass_depends_on_y();
    let mut traj = Traj::default();
    let mut t = s.t0;
    let mut y = y0;
    let mut f0 = sys.f(it, t, &y)?;
    let mut m0 = sys.mass(it, t, &y)?;
    let mut yp = match &m0 {
        None => f0.clone(),
        Some(m) => Lu::new(m.clone(), n).map(|l| l.solve(&f0)).unwrap_or_else(|| vec![0.0; n]),
    };
    traj.push(t, y.clone(), yp.clone());
    let mut absh = initial_step(s, tol, &y, &yp, pow);
    let mut left = match ev {
        Some(e) => e.eval(it, t, &y)?.0,
        None => Vec::new(),
    };
    let mut jac: Option<Vec<f64>> = None;
    let newton_tol = (0.01 * tol.rtol).max(100.0 * f64::EPSILON);
    loop {
        let hmin = 16.0 * eps_of(t);
        absh = s.hmax.min(hmin.max(absh));
        let (mut h, mut last) = clip(s, t, absh);
        let (z, fz, y1, f1, m1, err) = 'attempt: loop {
            let fresh = jac.is_none();
            if fresh {
                jac = Some(num_jacobian(sys, it, t, &y, &f0, &tol.thr)?);
                c.npds += 1;
            }
            let jm = jac.as_ref().unwrap();
            let mut w = m0.clone().unwrap_or_else(|| identity(n));
            for k in 0..n * n {
                w[k] -= d * h * jm[k];
            }
            c.ndecomps += 1;
            let lu = match Lu::new(w, n) {
                Some(l) => l,
                None => {
                    if !fresh {
                        jac = None;
                    } else {
                        h *= 0.5;
                        last = false;
                    }
                    continue;
                }
            };
            // Newton iterations for the residual r(x) = 0 of a stage.
            let newton = |it: &mut Interpreter, sys: &mut dyn System, c: &mut Counts, x0: Vec<f64>, res: &dyn Fn(&mut Interpreter, &mut dyn System, &[f64]) -> R<(Vec<f64>, Vec<f64>, Option<Vec<f64>>)>| -> R<Option<(Vec<f64>, Vec<f64>, Option<Vec<f64>>)>> {
                let mut x = x0;
                let mut prev = f64::INFINITY;
                for iter in 0..8 {
                    let (r, fx, mx) = res(it, sys, &x)?;
                    let dx = lu.solve(&r.iter().map(|v| -v).collect::<Vec<_>>());
                    c.nsolves += 1;
                    let nd = (0..n).map(|i| (dx[i] / x[i].abs().max(tol.thr[i])).abs()).fold(0.0, f64::max);
                    for i in 0..n {
                        x[i] += dx[i];
                    }
                    if !nd.is_finite() || (iter > 0 && nd > 0.9 * prev) {
                        return Ok(None);
                    }
                    if nd <= newton_tol {
                        let (_, fx2, mx2) = if iter == 0 { (r, fx, mx) } else { res(it, sys, &x)? };
                        return Ok(Some((x, fx2, mx2)));
                    }
                    prev = nd;
                }
                Ok(None)
            };
            let (tz, t1) = (t + g * h, t + h);
            // Stage 1: trapezoidal rule to t + g*h.
            let yv = y.clone();
            let f0v = f0.clone();
            let m0v = m0.clone();
            let stage1 = move |it: &mut Interpreter, sys: &mut dyn System, x: &[f64]| -> R<(Vec<f64>, Vec<f64>, Option<Vec<f64>>)> {
                let fx = sys.f(it, tz, x)?;
                let mm = if varies { sys.mass(it, t + 0.5 * g * h, &(0..n).map(|i| 0.5 * (yv[i] + x[i])).collect::<Vec<_>>())? } else { m0v.clone() };
                let dz: Vec<f64> = (0..n).map(|i| x[i] - yv[i]).collect();
                let md = mm.as_ref().map(|m| matvec(m, n, &dz)).unwrap_or(dz);
                Ok(((0..n).map(|i| md[i] - d * h * (f0v[i] + fx[i])).collect(), fx, mm))
            };
            let guess: Vec<f64> = (0..n).map(|i| y[i] + g * h * yp[i]).collect();
            let Some((z, fz, _)) = newton(it, sys, c, guess, &stage1)? else {
                c.nfailed += 1;
                if fresh {
                    h *= 0.25;
                    last = false;
                    if h.abs() <= hmin {
                        too_small(it, t, hmin);
                        return Ok(traj);
                    }
                }
                jac = None;
                continue 'attempt;
            };
            // Stage 2: BDF2 through y, z to t + h.
            let yv = y.clone();
            let zv = z.clone();
            let m0v = m0.clone();
            let stage2 = move |it: &mut Interpreter, sys: &mut dyn System, x: &[f64]| -> R<(Vec<f64>, Vec<f64>, Option<Vec<f64>>)> {
                let fx = sys.f(it, t1, x)?;
                let mm = if varies { sys.mass(it, t1, x)? } else { m0v.clone() };
                let dz: Vec<f64> = (0..n).map(|i| x[i] - ca * zv[i] + cb * yv[i]).collect();
                let md = mm.as_ref().map(|m| matvec(m, n, &dz)).unwrap_or(dz);
                Ok(((0..n).map(|i| md[i] - d * h * fx[i]).collect(), fx, mm))
            };
            let guess: Vec<f64> = (0..n).map(|i| y[i] + (z[i] - y[i]) / g).collect();
            let Some((y1, f1, m1)) = newton(it, sys, c, guess, &stage2)? else {
                c.nfailed += 1;
                if fresh {
                    h *= 0.25;
                    last = false;
                    if h.abs() <= hmin {
                        too_small(it, t, hmin);
                        return Ok(traj);
                    }
                }
                jac = None;
                continue 'attempt;
            };
            // Error: the step against a third-order quadrature of the stage derivatives.
            let dy: Vec<f64> = (0..n).map(|i| y1[i] - y[i]).collect();
            let mdy = m0.as_ref().map(|m| matvec(m, n, &dy)).unwrap_or(dy);
            let est: Vec<f64> = (0..n).map(|i| h * (w1 * f0[i] + w2 * fz[i] + w3 * f1[i]) - mdy[i]).collect();
            let e = lu.solve(&est);
            c.nsolves += 1;
            let err = tol.err(&e, &y, &y1);
            if err <= tol.rtol {
                break (z, fz, y1, f1, m1, err);
            }
            c.nfailed += 1;
            if h.abs() <= hmin {
                too_small(it, t, hmin);
                return Ok(traj);
            }
            h *= if err.is_nan() { 0.25 } else { (0.9 * (tol.rtol / err).powf(pow)).clamp(0.1, 0.9) };
            last = false;
        };
        let _ = fz;
        c.nsteps += 1;
        let tnew = t + h;
        let yp1: Vec<f64> = (0..n).map(|i| (y1[i] - ca * z[i] + cb * y[i]) / (d * h)).collect();
        traj.push(tnew, y1.clone(), yp1.clone());
        if let Some(e) = ev {
            if let Some((te, ye)) = locate_events(it, e, &mut left, &traj, log)? {
                *traj.t.last_mut().unwrap() = te;
                *traj.y.last_mut().unwrap() = ye;
                break;
            }
        }
        if last || s.tdir * (tnew - s.tfinal) >= 0.0 {
            break;
        }
        absh = h.abs() * if err == 0.0 { 5.0 } else { (0.9 * (tol.rtol / err).powf(pow)).clamp(0.2, 5.0) };
        t = tnew;
        y = y1;
        f0 = f1;
        yp = yp1;
        m0 = if varies { m1 } else { m0 };
    }
    c.nfevals = sys.evals();
    Ok(traj)
}

// ---- the ode* front end ----------------------------------------------------------------------

fn check_tspan(tspan: &[f64]) -> R<(f64, f64, f64)> {
    if tspan.len() < 2 {
        return rt("The input argument tspan must have two or more elements.");
    }
    let (t0, tfinal) = (tspan[0], tspan[tspan.len() - 1]);
    if t0 == tfinal {
        return rt("The last entry in tspan must be different from the first entry.");
    }
    let tdir = (tfinal - t0).signum();
    if tspan.windows(2).any(|w| tdir * (w[1] - w[0]) <= 0.0) {
        return rt("The entries in tspan must strictly increase or decrease.");
    }
    Ok((t0, tfinal, tdir))
}

fn stats_text(name: &str, c: &Counts) -> String {
    let mut s = format!("{} successful steps\n{} failed attempts\n{} function evaluations\n", c.nsteps, c.nfailed, c.nfevals);
    if matches!(name, "ode15s" | "ode23s" | "ode23t" | "ode23tb" | "ode15i") {
        s.push_str(&format!("{} partial derivatives\n{} LU decompositions\n{} solutions of linear systems\n", c.npds, c.ndecomps, c.nsolves));
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn finish(
    it: &mut Interpreter,
    name: &str,
    a: &[Value],
    nargout: usize,
    traj: &Traj,
    tspan: &[f64],
    refine: usize,
    log: &EventLog,
    c: &Counts,
    has_events: bool,
    extra: &[Value],
    exact: &mut dyn FnMut(&mut Interpreter, usize, f64) -> R<Vec<f64>>,
) -> Ret {
    let n = traj.y.first().map(|v| v.len()).unwrap_or(0);
    if nargout == 1 {
        let mut s = StructArray::scalar();
        s.set("solver", Value::chars(name));
        let mut ext = StructArray::scalar();
        ext.set("odefun", a[0].clone());
        ext.set("options", a.get(3).cloned().unwrap_or_else(|| Value::Struct(blank_options(ODE_FIELDS))));
        ext.set("varargin", Value::Cell(CellArray::row(extra.to_vec())));
        s.set("extdata", Value::Struct(ext));
        s.set("x", row(traj.t.clone()));
        s.set("y", cols_to_matrix(&traj.y, n));
        if has_events {
            s.set("xe", row(log.te.clone()));
            s.set("ye", cols_to_matrix(&log.ye, n));
            s.set("ie", row(log.ie.clone()));
        }
        let mut st = StructArray::scalar();
        st.set("nsteps", Value::num(c.nsteps as f64));
        st.set("nfailed", Value::num(c.nfailed as f64));
        st.set("nfevals", Value::num(c.nfevals as f64));
        s.set("stats", Value::Struct(st));
        let mut id = StructArray::scalar();
        id.set("yp", cols_to_matrix(&traj.yp, n));
        s.set("idata", Value::Struct(id));
        return one(Value::Struct(s));
    }
    let (mut tout, mut yout) = (Vec::new(), Vec::new());
    let tend = *traj.t.last().unwrap();
    let tdir = if tspan[tspan.len() - 1] >= tspan[0] { 1.0 } else { -1.0 };
    if tspan.len() > 2 {
        for &tq in tspan {
            if tdir * (tq - tend) > 0.0 {
                break;
            }
            let k = traj.interval(tq);
            let v = if traj.t[k] == tq {
                traj.y[k].clone()
            } else if k + 1 < traj.t.len() && traj.t[k + 1] == tq {
                traj.y[k + 1].clone()
            } else {
                exact(it, k, tq)?
            };
            tout.push(tq);
            yout.push(v);
        }
        if tout.last() != Some(&tend) && tdir * (tend - tspan[tspan.len() - 1]) < 0.0 {
            tout.push(tend);
            yout.push(traj.y.last().unwrap().clone());
        }
    } else {
        tout.push(traj.t[0]);
        yout.push(traj.y[0].clone());
        for k in 0..traj.t.len() - 1 {
            for r in 1..refine {
                let tq = traj.t[k] + (traj.t[k + 1] - traj.t[k]) * r as f64 / refine as f64;
                tout.push(tq);
                yout.push(exact(it, k, tq)?);
            }
            tout.push(traj.t[k + 1]);
            yout.push(traj.y[k + 1].clone());
        }
    }
    crate::builtins_funfun::run_output_fcn(it, a.get(3), tspan, &mut tout, &mut yout, if tspan.len() == 2 { refine } else { 1 })?;
    if nargout == 0 {
        it.call_function("plot", vec![col(tout), rows_to_matrix(&yout, n)], 0)?;
        return Ok(Some(Vec::new()));
    }
    let outs = vec![col(tout), rows_to_matrix(&yout, n), col(log.te.clone()), rows_to_matrix(&log.ye, n), col(log.ie.clone())];
    Ok(Some(outs.into_iter().take(nargout).collect()))
}

fn ode_solve(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    let f = arg(a, 0, name)?.clone();
    let tspan = arg(a, 1, name)?.to_matrix()?.data;
    let y0 = arg(a, 2, name)?.to_matrix()?.data;
    let o = options_arg(a.get(3))?;
    let extra: Vec<Value> = a.get(4..).unwrap_or(&[]).to_vec();
    let (t0, tfinal, tdir) = check_tspan(&tspan)?;
    let n = y0.len();
    let mut sys = UserSys::new(it, &f, extra.clone(), n, o)?;
    let tol = Tol::new(it, o, n, &sys.text)?;
    let hmax = (tfinal - t0).abs().min(opt_num(o, "MaxStep", 0.1 * (tfinal - t0))?.abs());
    let h0 = opt(o, "InitialStep").map(|v| v.scalar()).transpose()?.map(f64::abs);
    let high_order = matches!(name, "ode113" | "ode78" | "ode89");
    let refine = opt_num(o, "Refine", if matches!(name, "ode78" | "ode89") { 4.0 } else { 1.0 })?.max(1.0) as usize;
    let stops = if high_order && tspan.len() > 2 { tspan[1..].to_vec() } else { Vec::new() };
    let setup = Setup { t0, tfinal, tdir, hmax, h0, stops };
    let events = opt(o, "Events").filter(|v| is_func(v)).map(|v| Events { f: v.clone(), extra: extra.clone() });
    let mut log = EventLog::default();
    let mut c = Counts::default();
    let traj = match name {
        "ode113" => explicit_gbs(it, &mut sys, &setup, &tol, y0, 3, events.as_ref(), &mut log, &mut c)?,
        "ode78" => explicit_gbs(it, &mut sys, &setup, &tol, y0, 4, events.as_ref(), &mut log, &mut c)?,
        "ode89" => explicit_gbs(it, &mut sys, &setup, &tol, y0, 5, events.as_ref(), &mut log, &mut c)?,
        "ode23s" => rosenbrock(it, &mut sys, &setup, &tol, y0, events.as_ref(), &mut log, &mut c)?,
        _ => trbdf2(it, &mut sys, &setup, &tol, y0, events.as_ref(), &mut log, &mut c)?,
    };
    c.nfevals = sys.evals();
    if opt_text(o, "Stats", "off")? == "on" {
        it.emit(OutputKind::Text(stats_text(name, &c)));
    }
    let kcols = match name {
        "ode113" => 3,
        "ode78" => 4,
        _ => 5,
    };
    let tr = traj.clone();
    let mut exact = |it: &mut Interpreter, k: usize, tq: f64| -> R<Vec<f64>> {
        if high_order {
            Ok(gbs_step(&mut sys, it, tr.t[k], &tr.y[k], &tr.yp[k], tq - tr.t[k], kcols)?.0)
        } else {
            Ok(tr.eval(tq).0)
        }
    };
    finish(it, name, a, nargout, &traj, &tspan, refine, &log, &c, events.is_some(), &extra, &mut exact)
}

/// `deval` for solutions with Hermite data (`yp` or `idata.yp`).
pub fn deval_hermite(sol: &StructArray, xint: &[f64], idx: Option<&Value>, nargout: usize) -> Ret {
    let get = |name: &str| sol.get(name).ok_or_else(|| LangError::new("Invalid solution structure.", 0));
    let x = get("x")?.to_matrix()?.data;
    let ym = get("y")?.to_matrix()?;
    let yp = match sol.get("yp") {
        Some(v) => v.to_matrix()?,
        None => match sol.get("idata") {
            Some(Value::Struct(id)) => id.get("yp").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?,
            _ => return rt("Invalid solution structure."),
        },
    };
    let neq = ym.rows();
    let column = |m: &Matrix, k: usize| -> Vec<f64> { (0..neq).map(|i| m.get(i, k)).collect() };
    let traj = Traj { t: x.clone(), y: (0..x.len()).map(|k| column(&ym, k)).collect(), yp: (0..x.len()).map(|k| column(&yp, k)).collect() };
    let idx: Vec<usize> = match idx {
        Some(v) => v.to_matrix()?.data.iter().map(|&k| k as usize - 1).collect(),
        None => (0..neq).collect(),
    };
    if idx.iter().any(|&k| k >= neq) {
        return rt("Index exceeds the number of solution components.");
    }
    let n = x.len();
    let (lo, hi) = (x[0].min(x[n - 1]), x[0].max(x[n - 1]));
    let (mut out, mut dout) = (Vec::new(), Vec::new());
    for &xi in xint {
        if xi < lo || xi > hi {
            return rt(format!("Attempting to evaluate the solution outside the interval {} where it is defined.", fmt("[%e, %e]", &[x[0], x[n - 1]])));
        }
        let (y, p) = traj.eval(xi);
        out.extend(idx.iter().map(|&i| y[i]));
        dout.extend(idx.iter().map(|&i| p[i]));
    }
    let r = idx.len();
    Ok(Some(vec![Value::Num(Matrix::new(r, xint.len(), out)), Value::Num(Matrix::new(r, xint.len(), dout))].into_iter().take(nargout.max(1)).collect()))
}

/// `odextend(sol, odefun, tfinal, y0, options)`.
fn odextend(it: &mut Interpreter, a: &[Value]) -> Ret {
    let Value::Struct(sol) = arg(a, 0, "odextend")? else { return rt("SOL must be a solution structure returned by an ODE solver.") };
    let solver = sol.get("solver").map(|v| v.text()).transpose()?.unwrap_or_default();
    let Some(Value::Struct(ext)) = sol.get("extdata") else { return rt("SOL must be a solution structure returned by an ODE solver.") };
    let f = match a.get(1) {
        Some(v) if is_func(v) => v.clone(),
        _ => ext.get("odefun").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.clone(),
    };
    let tfinal = arg(a, 2, "odextend")?.scalar()?;
    let x = sol.get("x").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?;
    let y = sol.get("y").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?;
    let n = y.rows();
    let last = x.data[x.numel() - 1];
    let y0 = match a.get(3) {
        Some(v) if v.numel() > 0 => v.clone(),
        _ => col((0..n).map(|i| y.get(i, y.cols() - 1)).collect()),
    };
    let options = match a.get(4) {
        Some(v) => v.clone(),
        None => ext.get("options").cloned().unwrap_or_else(Value::empty),
    };
    let mut args = vec![f.clone(), row(vec![last, tfinal]), y0, options.clone()];
    if let Some(Value::Cell(c)) = ext.get("varargin") {
        args.extend(c.data.iter().cloned());
    }
    let Value::Struct(new) = it.call_function(&solver, args, 1)?.remove(0) else { return rt("The solver did not return a solution structure.") };
    let nx = new.get("x").unwrap().to_matrix()?;
    let ny = new.get("y").unwrap().to_matrix()?;
    let mut out = sol.clone();
    let mut xs = x.data.clone();
    xs.extend_from_slice(&nx.data[1..]);
    let mut ys = y.data.clone();
    ys.extend_from_slice(&ny.data[n..]);
    out.set("x", row(xs.clone()));
    out.set("y", Value::Num(Matrix::new(n, xs.len(), ys)));
    let mut e2 = ext.clone();
    e2.set("odefun", f);
    e2.set("options", options);
    out.set("extdata", Value::Struct(e2));
    // Interpolation data: per-step stages (f3d) or per-point derivatives (yp).
    if let (Some(Value::Struct(old)), Some(Value::Struct(nid))) = (sol.get("idata"), new.get("idata")) {
        let mut id = old.clone();
        if let (Some(a3), Some(b3)) = (old.get("f3d"), nid.get("f3d")) {
            let (a3, b3) = (a3.to_matrix()?, b3.to_matrix()?);
            let mut data = a3.data.clone();
            data.extend_from_slice(&b3.data);
            let steps = xs.len() - 1;
            let stages = a3.dims.get(1).copied().unwrap_or(1);
            id.set("f3d", Value::Num(Matrix::from_dims(vec![n, stages, steps], data)));
        }
        if let (Some(ap), Some(bp)) = (old.get("yp"), nid.get("yp")) {
            let (ap, bp) = (ap.to_matrix()?, bp.to_matrix()?);
            let mut data = ap.data.clone();
            data.extend_from_slice(&bp.data[n..]);
            id.set("yp", Value::Num(Matrix::new(n, xs.len(), data)));
        }
        out.set("idata", Value::Struct(id));
    }
    if let (Some(Value::Struct(s1)), Some(Value::Struct(s2))) = (sol.get("stats"), new.get("stats")) {
        let mut st = s1.clone();
        for f in ["nsteps", "nfailed", "nfevals"] {
            let v = s1.get(f).map(|v| v.scalar()).transpose()?.unwrap_or(0.0) + s2.get(f).map(|v| v.scalar()).transpose()?.unwrap_or(0.0);
            st.set(f, Value::num(v));
        }
        out.set("stats", Value::Struct(st));
    }
    one(Value::Struct(out))
}

// ---- fully implicit equations ------------------------------------------------------------------

struct Implicit {
    f: Value,
    extra: Vec<Value>,
    n: usize,
    nfevals: usize,
}

impl Implicit {
    fn eval(&mut self, it: &mut Interpreter, t: f64, y: &[f64], yp: &[f64]) -> R<Vec<f64>> {
        let mut args = vec![Value::num(t), col(y.to_vec()), col(yp.to_vec())];
        args.extend_from_slice(&self.extra);
        let m = call1(it, &self.f, args)?.to_matrix()?;
        self.nfevals += 1;
        if m.numel() != self.n {
            return rt(format!("The function must return a column vector of length {}.", self.n));
        }
        Ok(m.data)
    }

    /// dF/dy and dF/dyp by finite differences.
    fn jacobians(&mut self, it: &mut Interpreter, t: f64, y: &[f64], yp: &[f64], f0: &[f64]) -> R<(Vec<f64>, Vec<f64>)> {
        let n = self.n;
        let (mut jy, mut jp) = (vec![0.0; n * n], vec![0.0; n * n]);
        for j in 0..n {
            let dy = f64::EPSILON.sqrt() * y[j].abs().max(1e-6);
            let mut yd = y.to_vec();
            yd[j] += dy;
            let fd = self.eval(it, t, &yd, yp)?;
            let dp = f64::EPSILON.sqrt() * yp[j].abs().max(1e-6);
            let mut pd = yp.to_vec();
            pd[j] += dp;
            let fp = self.eval(it, t, y, &pd)?;
            for i in 0..n {
                jy[j * n + i] = (fd[i] - f0[i]) / dy;
                jp[j * n + i] = (fp[i] - f0[i]) / dp;
            }
        }
        Ok((jy, jp))
    }
}

/// Adds a small multiple of the identity to a Gram matrix, so rank-deficient steps stay defined.
fn ridge(g: &mut [f64], n: usize) {
    let top = (0..n).map(|i| g[i * n + i]).fold(0.0f64, f64::max);
    for i in 0..n {
        g[i * n + i] += 1e-12 * top + f64::MIN_POSITIVE;
    }
}

/// `decic`: consistent y0, yp0 changing the free components as little as possible.
fn decic(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let f = arg(a, 0, "decic")?.clone();
    let t0 = arg(a, 1, "decic")?.scalar()?;
    let mut y = arg(a, 2, "decic")?.to_matrix()?.data;
    let fy: Vec<bool> = arg(a, 3, "decic")?.to_matrix()?.data.iter().map(|&v| v != 0.0).collect();
    let mut yp = arg(a, 4, "decic")?.to_matrix()?.data;
    let fp: Vec<bool> = arg(a, 5, "decic")?.to_matrix()?.data.iter().map(|&v| v != 0.0).collect();
    let n = y.len();
    let extra = a.get(7..).unwrap_or(&[]).to_vec();
    let fixed = |v: &[bool], i: usize| v.get(i).copied().unwrap_or(v.first().copied().unwrap_or(false));
    if (0..n).filter(|&i| fixed(&fy, i)).count() + (0..n).filter(|&i| fixed(&fp, i)).count() > n {
        return rt("You cannot fix more than length(Y0) components.");
    }
    let mut sys = Implicit { f, extra, n, nfevals: 0 };
    let free: Vec<usize> = (0..2 * n).filter(|&k| if k < n { !fixed(&fy, k) } else { !fixed(&fp, k - n) }).collect();
    let mut r = sys.eval(it, t0, &y, &yp)?;
    let mut best = f64::INFINITY;
    for _ in 0..20 {
        let norm = r.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        // Iterate to rounding level (exact for linear problems) while the residual shrinks.
        let scale = y.iter().chain(&yp).fold(1.0f64, |m, v| m.max(v.abs()));
        if norm <= 4.0 * f64::EPSILON * scale || norm >= best || free.is_empty() {
            break;
        }
        best = norm;
        let (jy, jp) = sys.jacobians(it, t0, &y, &yp, &r)?;
        let m = free.len();
        // J is n-by-m over the free unknowns; minimum-norm (or least-squares) Newton step.
        let jcol = |k: usize, i: usize| if free[k] < n { jy[free[k] * n + i] } else { jp[(free[k] - n) * n + i] };
        let dx: Vec<f64> = if m >= n {
            let mut jjt = vec![0.0; n * n];
            for i in 0..n {
                for l in 0..n {
                    jjt[l * n + i] = (0..m).map(|k| jcol(k, i) * jcol(k, l)).sum();
                }
            }
            ridge(&mut jjt, n);
            let Some(lu) = Lu::new(jjt, n) else { return rt("Convergence failure in DECIC.") };
            let w = lu.solve(&r.iter().map(|v| -v).collect::<Vec<_>>());
            (0..m).map(|k| (0..n).map(|i| jcol(k, i) * w[i]).sum()).collect()
        } else {
            let mut jtj = vec![0.0; m * m];
            for k in 0..m {
                for l in 0..m {
                    jtj[l * m + k] = (0..n).map(|i| jcol(k, i) * jcol(l, i)).sum();
                }
            }
            ridge(&mut jtj, m);
            let Some(lu) = Lu::new(jtj, m) else { return rt("Convergence failure in DECIC.") };
            lu.solve(&(0..m).map(|k| -(0..n).map(|i| jcol(k, i) * r[i]).sum::<f64>()).collect::<Vec<_>>())
        };
        for (k, &u) in free.iter().enumerate() {
            if u < n { y[u] += dx[k] } else { yp[u - n] += dx[k] }
        }
        r = sys.eval(it, t0, &y, &yp)?;
    }
    let resnrm = r.iter().map(|v| v * v).sum::<f64>().sqrt();
    Ok(Some(vec![col(y), col(yp), Value::num(resnrm)].into_iter().take(nargout.max(1)).collect()))
}

/// `ode15i(f, tspan, y0, yp0, options)`: TR-BDF2 on F(t, y, y') = 0.
fn ode15i(it: &mut Interpreter, a: &[Value], nargout: usize) -> Ret {
    let name = "ode15i";
    let f = arg(a, 0, name)?.clone();
    let tspan = arg(a, 1, name)?.to_matrix()?.data;
    let y0 = arg(a, 2, name)?.to_matrix()?.data;
    let yp0 = arg(a, 3, name)?.to_matrix()?.data;
    let o = options_arg(a.get(4))?;
    let extra: Vec<Value> = a.get(5..).unwrap_or(&[]).to_vec();
    let (t0, tfinal, tdir) = check_tspan(&tspan)?;
    let n = y0.len();
    if yp0.len() != n {
        return rt("Y0 and YP0 must have the same length.");
    }
    let text = func_text(it, &f);
    let tol = Tol::new(it, o, n, &text)?;
    let hmax = (tfinal - t0).abs().min(opt_num(o, "MaxStep", 0.1 * (tfinal - t0))?.abs());
    let h0 = opt(o, "InitialStep").map(|v| v.scalar()).transpose()?.map(f64::abs);
    let s = Setup { t0, tfinal, tdir, hmax, h0, stops: Vec::new() };
    let mut sys = Implicit { f, extra: extra.clone(), n, nfevals: 0 };
    let g = 2.0 - 2f64.sqrt();
    let d = g / 2.0;
    let ca = 1.0 / (g * (2.0 - g));
    let cb = (1.0 - g).powi(2) / (g * (2.0 - g));
    let w2 = (1.0 / 6.0) / (g * (1.0 - g));
    let w3 = 0.5 - w2 * g;
    let w1 = 1.0 - w2 - w3;
    let pow = 1.0 / 3.0;
    let mut c = Counts::default();
    let mut traj = Traj::default();
    let (mut t, mut y, mut yp) = (t0, y0, yp0);
    traj.push(t, y.clone(), yp.clone());
    let mut absh = initial_step(&s, &tol, &y, &yp, pow);
    let newton_tol = (0.01 * tol.rtol).max(100.0 * f64::EPSILON);
    loop {
        let hmin = 16.0 * eps_of(t);
        absh = s.hmax.min(hmin.max(absh));
        let (mut h, mut last) = clip(&s, t, absh);
        let accepted = loop {
            let f0 = sys.eval(it, t, &y, &yp)?;
            let (jy, jp) = sys.jacobians(it, t, &y, &yp, &f0)?;
            c.npds += 1;
            let mut w = vec![0.0; n * n];
            for k in 0..n * n {
                w[k] = jy[k] + jp[k] / (d * h);
            }
            c.ndecomps += 1;
            let lu = Lu::new(w, n);
            // Solves F(tn, x, deriv(x)) = 0 where deriv is affine in x with slope 1/(d h).
            let mut solve = |it: &mut Interpreter, tn: f64, x0: Vec<f64>, deriv: &dyn Fn(&[f64]) -> Vec<f64>| -> R<Option<Vec<f64>>> {
                let Some(lu) = &lu else { return Ok(None) };
                let mut x = x0;
                let mut prev = f64::INFINITY;
                for iter in 0..8 {
                    let r = sys.eval(it, tn, &x, &deriv(&x))?;
                    let dx = lu.solve(&r.iter().map(|v| -v).collect::<Vec<_>>());
                    c.nsolves += 1;
                    let nd = (0..n).map(|i| (dx[i] / x[i].abs().max(tol.thr[i])).abs()).fold(0.0, f64::max);
                    for i in 0..n {
                        x[i] += dx[i];
                    }
                    if !nd.is_finite() || (iter > 0 && nd > 0.9 * prev) {
                        return Ok(None);
                    }
                    if nd <= newton_tol {
                        return Ok(Some(x));
                    }
                    prev = nd;
                }
                Ok(None)
            };
            let (yv, ypv) = (y.clone(), yp.clone());
            let d1 = move |x: &[f64]| -> Vec<f64> { (0..n).map(|i| (x[i] - yv[i]) / (d * h) - ypv[i]).collect() };
            let z = solve(it, t + g * h, (0..n).map(|i| y[i] + g * h * yp[i]).collect(), &d1)?;
            let result = match z {
                None => None,
                Some(z) => {
                    let ypz = d1(&z);
                    let (yv, zv) = (y.clone(), z.clone());
                    let d2 = move |x: &[f64]| -> Vec<f64> { (0..n).map(|i| (x[i] - ca * zv[i] + cb * yv[i]) / (d * h)).collect() };
                    solve(it, t + h, (0..n).map(|i| y[i] + (z[i] - y[i]) / g).collect(), &d2)?.map(|y1| {
                        let yp1 = d2(&y1);
                        (ypz, y1, yp1)
                    })
                }
            };
            match result {
                None => {
                    c.nfailed += 1;
                    h *= 0.25;
                    last = false;
                    if h.abs() <= hmin {
                        too_small(it, t, hmin);
                        break None;
                    }
                }
                Some((ypz, y1, yp1)) => {
                    let est: Vec<f64> = (0..n).map(|i| h * (w1 * yp[i] + w2 * ypz[i] + w3 * yp1[i]) - (y1[i] - y[i])).collect();
                    let scaled: Vec<f64> = matvec(&jp, n, &est).iter().map(|v| v / (d * h)).collect();
                    let e = match Lu::new((0..n * n).map(|k| jy[k] + jp[k] / (d * h)).collect(), n) {
                        Some(l) => l.solve(&scaled),
                        None => est,
                    };
                    let err = tol.err(&e, &y, &y1);
                    if err <= tol.rtol {
                        break Some((y1, yp1, err));
                    }
                    c.nfailed += 1;
                    if h.abs() <= hmin {
                        too_small(it, t, hmin);
                        break None;
                    }
                    h *= (0.9 * (tol.rtol / err).powf(pow)).clamp(0.1, 0.9);
                    last = false;
                }
            }
        };
        let Some((y1, yp1, err)) = accepted else { break };
        c.nsteps += 1;
        t += h;
        traj.push(t, y1.clone(), yp1.clone());
        if last || s.tdir * (t - s.tfinal) >= 0.0 {
            break;
        }
        absh = h.abs() * if err == 0.0 { 5.0 } else { (0.9 * (tol.rtol / err).powf(pow)).clamp(0.2, 5.0) };
        y = y1;
        yp = yp1;
    }
    c.nfevals = sys.nfevals;
    if opt_text(o, "Stats", "off")? == "on" {
        it.emit(OutputKind::Text(stats_text(name, &c)));
    }
    let tr = traj.clone();
    let mut exact = |_: &mut Interpreter, _: usize, tq: f64| -> R<Vec<f64>> { Ok(tr.eval(tq).0) };
    let mut a2 = a.to_vec();
    a2.remove(3);
    finish(it, name, &a2, nargout, &traj, &tspan, 1, &EventLog::default(), &c, false, &extra, &mut exact)
}

// ---- delay differential equations ----------------------------------------------------------

enum History {
    Const(Vec<f64>),
    Fun(Value),
    Sol(Traj),
}

impl History {
    fn from(v: &Value) -> R<History> {
        Ok(match v {
            Value::Struct(s) => {
                let x = s.get("x").ok_or_else(|| LangError::new("Invalid history structure.", 0))?.to_matrix()?.data;
                let y = s.get("y").ok_or_else(|| LangError::new("Invalid history structure.", 0))?.to_matrix()?;
                let yp = s.get("yp").ok_or_else(|| LangError::new("Invalid history structure.", 0))?.to_matrix()?;
                let n = y.rows();
                let colm = |m: &Matrix, k: usize| -> Vec<f64> { (0..n).map(|i| m.get(i, k)).collect() };
                History::Sol(Traj { t: x.clone(), y: (0..x.len()).map(|k| colm(&y, k)).collect(), yp: (0..x.len()).map(|k| colm(&yp, k)).collect() })
            }
            v if is_func(v) => History::Fun(v.clone()),
            v => History::Const(v.to_matrix()?.data),
        })
    }

    fn at(&self, it: &mut Interpreter, t: f64, extra: &[Value]) -> R<(Vec<f64>, Vec<f64>)> {
        Ok(match self {
            History::Const(v) => (v.clone(), vec![0.0; v.len()]),
            History::Sol(tr) => tr.eval(t),
            History::Fun(f) => {
                let call = |it: &mut Interpreter, t: f64| -> R<Vec<f64>> {
                    let mut args = vec![Value::num(t)];
                    args.extend_from_slice(extra);
                    Ok(call1(it, f, args)?.to_matrix()?.data)
                };
                let y = call(it, t)?;
                let dt = f64::EPSILON.cbrt() * t.abs().max(1.0);
                let (a, b) = (call(it, t + dt)?, call(it, t - dt)?);
                let p = (0..y.len()).map(|i| (a[i] - b[i]) / (2.0 * dt)).collect();
                (y, p)
            }
        })
    }
}

/// What a DDE solver delays: constant lags, or delay functions giving delayed times.
enum Delays {
    Lags(Vec<f64>),
    Fun(Value),
}

impl Delays {
    fn times(&self, it: &mut Interpreter, t: f64, y: &[f64], extra: &[Value]) -> R<Vec<f64>> {
        match self {
            Delays::Lags(l) => Ok(l.iter().map(|lag| t - lag).collect()),
            Delays::Fun(f) => {
                let mut args = vec![Value::num(t), col(y.to_vec())];
                args.extend_from_slice(extra);
                Ok(call1(it, f, args)?.to_matrix()?.data)
            }
        }
    }
}

struct Dde {
    f: Value,
    extra: Vec<Value>,
    delays: Delays,
    /// Delays of the derivative (ddensd).
    delays_p: Option<Delays>,
    history: History,
    t0: f64,
    nfevals: usize,
}

impl Dde {
    /// y and y' at time tq: history before t0, the computed solution after.
    fn lookup(&self, it: &mut Interpreter, traj: &Traj, tq: f64) -> R<(Vec<f64>, Vec<f64>)> {
        if tq <= self.t0 || traj.t.is_empty() {
            return self.history.at(it, tq, &self.extra);
        }
        let last = *traj.t.last().unwrap();
        if tq > last {
            // Ahead of the solution (short state-dependent delays): extrapolate linearly.
            let (y, p) = (traj.y.last().unwrap(), traj.yp.last().unwrap());
            return Ok(((0..y.len()).map(|i| y[i] + (tq - last) * p[i]).collect(), p.clone()));
        }
        Ok(traj.eval(tq))
    }

    fn rhs(&mut self, it: &mut Interpreter, traj: &Traj, t: f64, y: &[f64]) -> R<Vec<f64>> {
        let times = self.delays.times(it, t, y, &self.extra)?;
        let n = y.len();
        let mut z = Vec::with_capacity(n * times.len());
        for &tq in &times {
            z.extend(self.lookup(it, traj, tq)?.0);
        }
        let mut args = vec![Value::num(t), col(y.to_vec()), Value::Num(Matrix::new(n, times.len(), z))];
        if let Some(dp) = &self.delays_p {
            let tp = dp.times(it, t, y, &self.extra)?;
            let mut zp = Vec::with_capacity(n * tp.len());
            for &tq in &tp {
                zp.extend(self.lookup(it, traj, tq)?.1);
            }
            args.push(Value::Num(Matrix::new(n, tp.len(), zp)));
        }
        args.extend_from_slice(&self.extra);
        self.nfevals += 1;
        let m = call1(it, &self.f, args)?.to_matrix()?;
        if m.numel() != n {
            return rt(format!("The DDE function must return a column vector of length {n}."));
        }
        Ok(m.data)
    }
}

fn dde_solve(it: &mut Interpreter, name: &str, a: &[Value]) -> Ret {
    let f = arg(a, 0, name)?.clone();
    let shift = usize::from(name == "ddensd");
    let to_delays = |v: &Value| -> R<Delays> { Ok(if is_func(v) { Delays::Fun(v.clone()) } else { Delays::Lags(v.to_matrix()?.data) }) };
    let delays = to_delays(arg(a, 1, name)?)?;
    let delays_p = if shift == 1 { Some(to_delays(arg(a, 2, name)?)?) } else { None };
    let hist_v = arg(a, 2 + shift, name)?.clone();
    let tspan = arg(a, 3 + shift, name)?.to_matrix()?.data;
    let o = options_arg(a.get(4 + shift))?;
    let extra: Vec<Value> = a.get(5 + shift..).unwrap_or(&[]).to_vec();
    let (t0, tfinal, tdir) = check_tspan(&tspan)?;
    if tdir < 0.0 {
        return rt("The final time must be greater than the initial time.");
    }
    let history = History::from(&hist_v)?;
    let y0 = match opt(o, "InitialY") {
        Some(v) => v.to_matrix()?.data,
        None => history.at(it, t0, &extra)?.0,
    };
    let n = y0.len();
    let lags: Vec<f64> = match &delays {
        Delays::Lags(l) => l.clone(),
        Delays::Fun(_) => Vec::new(),
    };
    if lags.iter().any(|&l| l < 0.0) {
        return rt("The lags must be non-negative.");
    }
    let text = func_text(it, &f);
    let tol = Tol::new(it, o, n, &text)?;
    let mut hmax = (tfinal - t0).abs().min(opt_num(o, "MaxStep", 0.1 * (tfinal - t0))?.abs());
    let minlag = lags.iter().copied().filter(|&l| l > 0.0).fold(f64::INFINITY, f64::min);
    if minlag.is_finite() {
        hmax = hmax.min(minlag);
    }
    // Discontinuities propagated from t0 by up to three lags, and the user's jumps.
    let mut discont = vec![t0];
    if name == "dde23" {
        let mut level = vec![t0];
        for _ in 0..3 {
            let mut next = Vec::new();
            for &p in &level {
                for &l in &lags {
                    let q = p + l;
                    if l > 0.0 && q <= tfinal && !next.iter().any(|&x: &f64| (x - q).abs() <= 1e-12 * q.abs().max(1.0)) {
                        next.push(q);
                    }
                }
            }
            discont.extend(next.iter().copied());
            level = next;
        }
    }
    if let Some(j) = opt(o, "Jumps") {
        discont.extend(j.to_matrix()?.data.iter().copied().filter(|&x| x > t0 && x <= tfinal));
    }
    discont.sort_by(f64::total_cmp);
    discont.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * a.abs().max(1.0));
    let mut stops: Vec<f64> = discont.iter().copied().filter(|&x| x > t0 && x < tfinal).collect();
    stops.push(tfinal);
    let s = Setup { t0, tfinal, tdir, hmax, h0: opt(o, "InitialStep").map(|v| v.scalar()).transpose()?.map(f64::abs), stops };
    let mut dde = Dde { f, extra: extra.clone(), delays, delays_p, history, t0, nfevals: 0 };
    let mut c = Counts::default();
    let mut traj = Traj::default();
    let mut t = t0;
    let mut y = y0;
    let mut f0 = dde.rhs(it, &traj, t, &y)?;
    traj.push(t, y.clone(), f0.clone());
    let pow = 1.0 / 3.0;
    let mut absh = initial_step(&s, &tol, &y, &f0, pow);
    let events = opt(o, "Events").filter(|v| is_func(v)).map(|v| Events { f: v.clone(), extra: extra.clone() });
    let mut log = EventLog::default();
    let mut left = match &events {
        Some(e) => e.eval(it, t, &y)?.0,
        None => Vec::new(),
    };
    loop {
        let hmin = 16.0 * eps_of(t);
        absh = s.hmax.min(hmin.max(absh));
        let (mut h, mut last) = clip(&s, t, absh);
        // Bogacki-Shampine (2,3), as dde23.
        let (ynew, fnew, err) = loop {
            let at = |k: &[f64], c: f64| -> Vec<f64> { (0..n).map(|i| y[i] + c * h * k[i]).collect() };
            let k1 = f0.clone();
            let k2 = dde.rhs(it, &traj, t + 0.5 * h, &at(&k1, 0.5))?;
            let k3 = dde.rhs(it, &traj, t + 0.75 * h, &at(&k2, 0.75))?;
            let yn: Vec<f64> = (0..n).map(|i| y[i] + h * (2.0 / 9.0 * k1[i] + 1.0 / 3.0 * k2[i] + 4.0 / 9.0 * k3[i])).collect();
            let k4 = dde.rhs(it, &traj, t + h, &yn)?;
            let e: Vec<f64> = (0..n).map(|i| h * (-5.0 / 72.0 * k1[i] + 1.0 / 12.0 * k2[i] + 1.0 / 9.0 * k3[i] - 1.0 / 8.0 * k4[i])).collect();
            let err = tol.err(&e, &y, &yn);
            if err <= tol.rtol {
                break (yn, k4, err);
            }
            c.nfailed += 1;
            if h.abs() <= hmin {
                too_small(it, t, hmin);
                return dde_sol(name, &hist_v, &discont, &traj, &c, dde.nfevals, &log, events.is_some());
            }
            h *= if err.is_nan() { 0.5 } else { (0.8 * (tol.rtol / err).powf(pow)).clamp(0.5, 0.9) };
            last = false;
        };
        c.nsteps += 1;
        t += h;
        traj.push(t, ynew.clone(), fnew.clone());
        if let Some(e) = &events {
            if let Some((te, ye)) = locate_events(it, e, &mut left, &traj, &mut log)? {
                *traj.t.last_mut().unwrap() = te;
                *traj.y.last_mut().unwrap() = ye;
                break;
            }
        }
        if last || t >= tfinal {
            break;
        }
        absh = h.abs() * if err == 0.0 { 5.0 } else { (0.8 * (tol.rtol / err).powf(pow)).clamp(0.2, 5.0) };
        y = ynew;
        f0 = fnew;
    }
    if opt_text(o, "Stats", "off")? == "on" {
        it.emit(OutputKind::Text(format!("{} successful steps\n{} failed attempts\n{} function evaluations\n", c.nsteps, c.nfailed, dde.nfevals)));
    }
    dde_sol(name, &hist_v, &discont, &traj, &c, dde.nfevals, &log, events.is_some())
}

#[allow(clippy::too_many_arguments)]
fn dde_sol(name: &str, history: &Value, discont: &[f64], traj: &Traj, c: &Counts, nfevals: usize, log: &EventLog, events: bool) -> Ret {
    let n = traj.y.first().map(|v| v.len()).unwrap_or(0);
    let mut s = StructArray::scalar();
    s.set("solver", Value::chars(name));
    s.set("history", history.clone());
    if name == "dde23" {
        s.set("discont", row(discont.to_vec()));
    }
    s.set("x", row(traj.t.clone()));
    s.set("y", cols_to_matrix(&traj.y, n));
    if events {
        s.set("xe", row(log.te.clone()));
        s.set("ye", cols_to_matrix(&log.ye, n));
        s.set("ie", row(log.ie.clone()));
    }
    let mut st = StructArray::scalar();
    st.set("nsteps", Value::num(c.nsteps as f64));
    st.set("nfailed", Value::num(c.nfailed as f64));
    st.set("nfevals", Value::num(nfevals as f64));
    s.set("stats", Value::Struct(st));
    s.set("yp", cols_to_matrix(&traj.yp, n));
    one(Value::Struct(s))
}

// ---- boundary value problems -------------------------------------------------------------------

/// `bvpinit(x, yinit, parameters, ...)` and `bvpinit(sol, [anew bnew], ...)`.
fn bvpinit(it: &mut Interpreter, a: &[Value]) -> Ret {
    let x0 = arg(a, 0, "bvpinit")?;
    if let Value::Struct(sol) = x0 {
        // Extend a previous solution to a larger interval by constant extrapolation.
        let ab = arg(a, 1, "bvpinit")?.to_matrix()?.data;
        let x = sol.get("x").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?.data;
        let y = sol.get("y").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?;
        let n = y.rows();
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        if ab[0] < x[0] {
            xs.push(ab[0]);
            ys.extend((0..n).map(|i| y.get(i, 0)));
        }
        xs.extend_from_slice(&x);
        ys.extend_from_slice(&y.data);
        if let Some(&b) = ab.get(1).filter(|&&b| b > x[x.len() - 1]) {
            xs.push(b);
            ys.extend((0..n).map(|i| y.get(i, x.len() - 1)));
        }
        let mut s = StructArray::scalar();
        s.set("solver", Value::chars("bvpinit"));
        s.set("x", row(xs.clone()));
        s.set("y", Value::Num(Matrix::new(n, xs.len(), ys)));
        if let Some(p) = a.get(2).cloned().or_else(|| sol.get("parameters").cloned()) {
            s.set("parameters", p);
        }
        return one(Value::Struct(s));
    }
    let x = x0.to_matrix()?.data;
    if x.len() < 2 {
        return rt("The initial mesh must have at least two points.");
    }
    if x.windows(2).any(|w| w[1] < w[0]) && x.windows(2).any(|w| w[1] > w[0]) {
        return rt("The entries of x must increase or decrease.");
    }
    let yinit = arg(a, 1, "bvpinit")?.clone();
    let extra: Vec<Value> = a.get(3..).unwrap_or(&[]).to_vec();
    let mut cols = Vec::new();
    for &xi in &x {
        let v = if is_func(&yinit) {
            let mut args = vec![Value::num(xi)];
            args.extend_from_slice(&extra);
            call1(it, &yinit, args)?.to_matrix()?.data
        } else {
            yinit.to_matrix()?.data
        };
        cols.push(v);
    }
    let n = cols[0].len();
    let mut s = StructArray::scalar();
    s.set("solver", Value::chars("bvpinit"));
    s.set("x", row(x));
    s.set("y", cols_to_matrix(&cols, n));
    if let Some(p) = a.get(2).filter(|v| v.numel() > 0) {
        s.set("parameters", p.clone());
    }
    s.set("yinit", yinit);
    one(Value::Struct(s))
}

/// `bvpxtend(sol, xnew, ynew)`: a guess on a larger interval.
fn bvpxtend(a: &[Value]) -> Ret {
    let Value::Struct(sol) = arg(a, 0, "bvpxtend")? else { return rt("SOL must be a solution structure.") };
    let xnew = arg(a, 1, "bvpxtend")?.scalar()?;
    let x = sol.get("x").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?.data;
    let y = sol.get("y").ok_or_else(|| LangError::new("Invalid solution structure.", 0))?.to_matrix()?;
    let n = y.rows();
    let end = |k: usize| -> Vec<f64> { (0..n).map(|i| y.get(i, k)).collect() };
    let ynew = match a.get(2) {
        Some(v) if !v.is_text() && v.numel() > 0 => v.to_matrix()?.data,
        _ => {
            if xnew < x[0] { end(0) } else { end(x.len() - 1) }
        }
    };
    let (mut xs, mut ys) = (x.clone(), y.data.clone());
    if xnew < x[0] {
        xs.insert(0, xnew);
        let mut d = ynew;
        d.extend_from_slice(&ys);
        ys = d;
    } else if xnew > x[x.len() - 1] {
        xs.push(xnew);
        ys.extend_from_slice(&ynew);
    } else {
        return rt("XNEW must be outside the interval of SOL.");
    }
    let mut s = StructArray::scalar();
    s.set("solver", Value::chars("bvpinit"));
    s.set("x", row(xs.clone()));
    s.set("y", Value::Num(Matrix::new(n, xs.len(), ys)));
    if let Some(p) = sol.get("parameters") {
        s.set("parameters", p.clone());
    }
    one(Value::Struct(s))
}

struct Bvp {
    f: Value,
    bc: Value,
    extra: Vec<Value>,
    n: usize,
    np: usize,
    nf: usize,
    nbc: usize,
}

impl Bvp {
    fn f(&mut self, it: &mut Interpreter, x: f64, y: &[f64], p: &[f64]) -> R<Vec<f64>> {
        let mut args = vec![Value::num(x), col(y.to_vec())];
        if self.np > 0 {
            args.push(col(p.to_vec()));
        }
        args.extend_from_slice(&self.extra);
        self.nf += 1;
        let m = call1(it, &self.f, args)?.to_matrix()?;
        if m.numel() != self.n {
            return rt(format!("The derivative function ODEFUN should return a column vector of length {}.", self.n));
        }
        Ok(m.data)
    }

    fn bc(&mut self, it: &mut Interpreter, ya: &[f64], yb: &[f64], p: &[f64]) -> R<Vec<f64>> {
        let mut args = vec![col(ya.to_vec()), col(yb.to_vec())];
        if self.np > 0 {
            args.push(col(p.to_vec()));
        }
        args.extend_from_slice(&self.extra);
        self.nbc += 1;
        let m = call1(it, &self.bc, args)?.to_matrix()?;
        if m.numel() != self.n + self.np {
            return rt(format!("The boundary condition function BCFUN should return a column vector of length {}.", self.n + self.np));
        }
        Ok(m.data)
    }

    /// Collocation residual and the values at the mesh points: (residual, f at nodes).
    fn residual(&mut self, it: &mut Interpreter, x: &[f64], u: &[f64]) -> R<(Vec<f64>, Vec<Vec<f64>>)> {
        let (n, m) = (self.n, x.len());
        let p = &u[n * m..];
        let ys: Vec<&[f64]> = (0..m).map(|k| &u[k * n..(k + 1) * n]).collect();
        let fs: Vec<Vec<f64>> = (0..m).map(|k| self.f(it, x[k], ys[k], p)).collect::<R<_>>()?;
        let mut r = self.bc(it, ys[0], ys[m - 1], p)?;
        for k in 0..m - 1 {
            let h = x[k + 1] - x[k];
            let ym: Vec<f64> = (0..n).map(|i| 0.5 * (ys[k][i] + ys[k + 1][i]) - h / 8.0 * (fs[k + 1][i] - fs[k][i])).collect();
            let fm = self.f(it, x[k] + 0.5 * h, &ym, p)?;
            r.extend((0..n).map(|i| ys[k + 1][i] - ys[k][i] - h / 6.0 * (fs[k][i] + 4.0 * fm[i] + fs[k + 1][i])));
        }
        Ok((r, fs))
    }
}

/// Solves the collocation equations by damped Newton with a finite-difference Jacobian.
fn bvp_newton(it: &mut Interpreter, bvp: &mut Bvp, x: &[f64], mut u: Vec<f64>, rtol: f64) -> R<Option<Vec<f64>>> {
    let nu = u.len();
    let size = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
    let (mut r, _) = bvp.residual(it, x, &u)?;
    for _ in 0..40 {
        // Jacobian column by column (the residual is cheap next to the interpreter calls).
        let mut jac = vec![0.0; nu * nu];
        for j in 0..nu {
            let d = f64::EPSILON.sqrt() * u[j].abs().max(1e-3);
            let mut ud = u.clone();
            ud[j] += d;
            let (rd, _) = bvp.residual(it, x, &ud)?;
            for i in 0..nu {
                jac[j * nu + i] = (rd[i] - r[i]) / d;
            }
        }
        let Some(lu) = Lu::new(jac, nu) else { return Ok(None) };
        let du = lu.solve(&r.iter().map(|v| -v).collect::<Vec<_>>());
        let r0 = size(&r);
        let mut lambda = 1.0;
        let mut accepted = None;
        for _ in 0..8 {
            let ut: Vec<f64> = (0..nu).map(|i| u[i] + lambda * du[i]).collect();
            let (rt2, _) = bvp.residual(it, x, &ut)?;
            if size(&rt2) < r0 || lambda < 0.02 {
                accepted = Some((ut, rt2));
                break;
            }
            lambda *= 0.5;
        }
        let Some((un, rn)) = accepted else { return Ok(None) };
        let step = (0..nu).map(|i| (lambda * du[i]).abs() / u[i].abs().max(1.0)).fold(0.0, f64::max);
        u = un;
        r = rn;
        if step <= 1e-3 * rtol || size(&r) <= 1e-12 {
            return Ok(Some(u));
        }
    }
    Ok(if size(&r) <= 1e-6 { Some(u) } else { None })
}

fn bvp_solve(it: &mut Interpreter, name: &str, a: &[Value]) -> Ret {
    let f = arg(a, 0, name)?.clone();
    let bc = arg(a, 1, name)?.clone();
    let Value::Struct(init) = arg(a, 2, name)? else { return rt("The initial guess SOLINIT must be a structure created with BVPINIT.") };
    let o = options_arg(a.get(3))?;
    let extra: Vec<Value> = a.get(4..).unwrap_or(&[]).to_vec();
    if opt(o, "SingularTerm").is_some() {
        return rt("The SingularTerm option is not supported in this version.");
    }
    let mut x = init.get("x").ok_or_else(|| LangError::new("SOLINIT must have a field x.", 0))?.to_matrix()?.data;
    let y = init.get("y").ok_or_else(|| LangError::new("SOLINIT must have a field y.", 0))?.to_matrix()?;
    let p0: Vec<f64> = init.get("parameters").map(|v| v.to_matrix()).transpose()?.map(|m| m.data).unwrap_or_default();
    let n = y.rows();
    let rtol = opt_num(o, "RelTol", 1e-3)?;
    let atol: Vec<f64> = match opt(o, "AbsTol") {
        Some(v) => v.to_matrix()?.data,
        None => vec![1e-6],
    };
    let thr: Vec<f64> = (0..n).map(|i| atol[if atol.len() == 1 { 0 } else { i.min(atol.len() - 1) }] / rtol).collect();
    let nmax = opt_num(o, "Nmax", (10000 / n.max(1)) as f64)? as usize;
    let mut bvp = Bvp { f, bc, extra, n, np: p0.len(), nf: 0, nbc: 0 };
    let mut u: Vec<f64> = y.data.clone();
    u.extend_from_slice(&p0);
    let mut maxres;
    loop {
        let Some(sol) = bvp_newton(it, &mut bvp, &x, u.clone(), rtol)? else {
            return rt("Unable to solve the collocation equations -- a singular Jacobian encountered.");
        };
        u = sol;
        let m = x.len();
        let p = u[n * m..].to_vec();
        let (_, fs) = bvp.residual(it, &x, &u)?;
        // Residual of the cubic interpolant at interior points of each interval.
        let mut res = vec![0.0; m - 1];
        for k in 0..m - 1 {
            let (y0, y1) = (&u[k * n..(k + 1) * n], &u[(k + 1) * n..(k + 2) * n]);
            for tau in [0.5 - 21f64.sqrt() / 14.0, 0.5, 0.5 + 21f64.sqrt() / 14.0] {
                let xq = x[k] + tau * (x[k + 1] - x[k]);
                let (s, sp) = hermite(x[k], y0, &fs[k], x[k + 1], y1, &fs[k + 1], xq);
                let fq = bvp.f(it, xq, &s, &p)?;
                for i in 0..n {
                    let r = (sp[i] - fq[i]).abs() / (fq[i].abs() + thr[i]);
                    res[k] = f64::max(res[k], r);
                }
            }
        }
        maxres = res.iter().fold(0.0f64, |a, &b| a.max(b));
        if maxres <= rtol {
            break;
        }
        // Refine: one new point where the residual is moderate, two where it is large.
        let (mut nx, mut nu) = (Vec::new(), Vec::new());
        for k in 0..m - 1 {
            let (y0, y1) = (&u[k * n..(k + 1) * n], &u[(k + 1) * n..(k + 2) * n]);
            nx.push(x[k]);
            nu.extend_from_slice(y0);
            let parts = if res[k] > 100.0 * rtol { 3 } else if res[k] > rtol { 2 } else { 1 };
            for q in 1..parts {
                let xq = x[k] + (x[k + 1] - x[k]) * q as f64 / parts as f64;
                nx.push(xq);
                nu.extend(hermite(x[k], y0, &fs[k], x[k + 1], y1, &fs[k + 1], xq).0);
            }
        }
        nx.push(x[m - 1]);
        nu.extend_from_slice(&u[(m - 1) * n..m * n]);
        if nx.len() > nmax {
            warn(it, format!("Unable to meet the tolerance without using more than {nmax} mesh points. \n The last mesh of {} points and the solution are available in the output argument. \n The maximum residual is {}, while requested accuracy is {}.", m, fmt("%g", &[maxres]), fmt("%g", &[rtol])));
            break;
        }
        nu.extend_from_slice(&p);
        x = nx;
        u = nu;
    }
    let m = x.len();
    let p = u[n * m..].to_vec();
    let (_, fs) = bvp.residual(it, &x, &u)?;
    if opt_text(o, "Stats", "off")? == "on" {
        it.emit(OutputKind::Text(format!(
            "The solution was obtained on a mesh of {m} points.\nThe maximum residual is {}. \nThere were {} calls to the ODE function. \nThere were {} calls to the BC function. \n",
            fmt("%10.3e", &[maxres]),
            bvp.nf,
            bvp.nbc
        )));
    }
    let mut s = StructArray::scalar();
    s.set("solver", Value::chars(name));
    s.set("x", row(x.clone()));
    s.set("y", Value::Num(Matrix::new(n, m, u[..n * m].to_vec())));
    s.set("yp", cols_to_matrix(&fs, n));
    if !p.is_empty() {
        s.set("parameters", col(p));
    }
    let mut st = StructArray::scalar();
    st.set("nmeshpoints", Value::num(m as f64));
    st.set("maxres", Value::num(maxres));
    st.set("nODEevals", Value::num(bvp.nf as f64));
    st.set("nBCevals", Value::num(bvp.nbc as f64));
    s.set("stats", Value::Struct(st));
    one(Value::Struct(s))
}

// ---- pdepe ---------------------------------------------------------------------------------

/// The method-of-lines system of `pdepe`: a finite-volume discretization of
/// c u_t = x^-m (x^m f)_x + s on the mesh, with the boundary conditions p + q f = 0.
struct Pde {
    m: f64,
    pde: Value,
    bc: Value,
    extra: Vec<Value>,
    x: Vec<f64>,
    npde: usize,
    nf: usize,
}

impl Pde {
    fn call3(&mut self, it: &mut Interpreter, f: &Value, args: Vec<Value>, n: usize) -> R<Vec<Vec<f64>>> {
        let mut args = args;
        args.extend_from_slice(&self.extra);
        let r = it.call_func_value(f, args, n)?;
        if r.len() < n {
            return rt("The PDE and boundary functions must return all of their outputs.");
        }
        r.iter().map(|v| Ok(v.to_matrix()?.data)).collect()
    }

    fn coeffs(&mut self, it: &mut Interpreter, x: f64, t: f64, u: &[f64], ux: &[f64]) -> R<(Vec<f64>, Vec<f64>, Vec<f64>)> {
        let pde = self.pde.clone();
        let r = self.call3(it, &pde, vec![Value::num(x), Value::num(t), col(u.to_vec()), col(ux.to_vec())], 3)?;
        let fix = |v: &Vec<f64>, n: usize| if v.len() == 1 && n > 1 { vec![v[0]; n] } else { v.clone() };
        Ok((fix(&r[0], self.npde), fix(&r[1], self.npde), fix(&r[2], self.npde)))
    }

    fn node(&self, u: &[f64], k: usize) -> Vec<f64> {
        u[k * self.npde..(k + 1) * self.npde].to_vec()
    }

    /// Slope of u at node k (one-sided at the ends).
    fn slope(&self, u: &[f64], k: usize) -> Vec<f64> {
        let nx = self.x.len();
        let (a, b) = if k == 0 { (0, 1) } else if k == nx - 1 { (nx - 2, nx - 1) } else { (k - 1, k + 1) };
        let (ua, ub) = (self.node(u, a), self.node(u, b));
        (0..self.npde).map(|c| (ub[c] - ua[c]) / (self.x[b] - self.x[a])).collect()
    }

    /// ∫ x^m dx over [a, b].
    fn vol(&self, a: f64, b: f64) -> f64 {
        let m = self.m;
        (b.powf(m + 1.0) - a.powf(m + 1.0)) / (m + 1.0)
    }

    fn cells(&self) -> Vec<(f64, f64)> {
        let x = &self.x;
        let nx = x.len();
        (0..nx).map(|k| (if k == 0 { x[0] } else { 0.5 * (x[k - 1] + x[k]) }, if k == nx - 1 { x[nx - 1] } else { 0.5 * (x[k] + x[k + 1]) })).collect()
    }
}

impl System for Pde {
    fn n(&self) -> usize {
        self.x.len() * self.npde
    }

    fn f(&mut self, it: &mut Interpreter, t: f64, u: &[f64]) -> R<Vec<f64>> {
        self.nf += 1;
        let (nx, np) = (self.x.len(), self.npde);
        let x = self.x.clone();
        // Fluxes at the midpoints between nodes.
        let mut flux = Vec::with_capacity(nx - 1);
        for k in 0..nx - 1 {
            let (ua, ub) = (self.node(u, k), self.node(u, k + 1));
            let um: Vec<f64> = (0..np).map(|c| 0.5 * (ua[c] + ub[c])).collect();
            let ux: Vec<f64> = (0..np).map(|c| (ub[c] - ua[c]) / (x[k + 1] - x[k])).collect();
            flux.push(self.coeffs(it, 0.5 * (x[k] + x[k + 1]), t, &um, &ux)?.1);
        }
        let bc = self.bc.clone();
        let (ul, ur) = (self.node(u, 0), self.node(u, nx - 1));
        let b = self.call3(it, &bc, vec![Value::num(x[0]), col(ul.clone()), Value::num(x[nx - 1]), col(ur.clone()), Value::num(t)], 4)?;
        let fix = |v: &Vec<f64>| if v.len() == 1 && np > 1 { vec![v[0]; np] } else { v.clone() };
        let (pl, ql, pr, qr) = (fix(&b[0]), fix(&b[1]), fix(&b[2]), fix(&b[3]));
        let cells = self.cells();
        let mut out = vec![0.0; nx * np];
        for k in 0..nx {
            let (a, bb) = cells[k];
            let (_, _, s) = self.coeffs(it, x[k], t, &self.node(u, k), &self.slope(u, k))?;
            let v = self.vol(a, bb);
            for c in 0..np {
                let i = k * np + c;
                let fa = if k == 0 {
                    if ql[c] == 0.0 {
                        out[i] = pl[c];
                        continue;
                    }
                    -pl[c] / ql[c]
                } else {
                    flux[k - 1][c]
                };
                let fb = if k == nx - 1 {
                    if qr[c] == 0.0 {
                        out[i] = pr[c];
                        continue;
                    }
                    -pr[c] / qr[c]
                } else {
                    flux[k][c]
                };
                out[i] = bb.powf(self.m) * fb - a.powf(self.m) * fa + s[c] * v;
            }
        }
        Ok(out)
    }

    fn mass(&mut self, it: &mut Interpreter, t: f64, u: &[f64]) -> R<Option<Vec<f64>>> {
        let (nx, np) = (self.x.len(), self.npde);
        let n = nx * np;
        let bc = self.bc.clone();
        let x = self.x.clone();
        let b = self.call3(it, &bc, vec![Value::num(x[0]), col(self.node(u, 0)), Value::num(x[nx - 1]), col(self.node(u, nx - 1)), Value::num(t)], 4)?;
        let fix = |v: &Vec<f64>| if v.len() == 1 && np > 1 { vec![v[0]; np] } else { v.clone() };
        let (ql, qr) = (fix(&b[1]), fix(&b[3]));
        let cells = self.cells();
        let mut m = vec![0.0; n * n];
        for k in 0..nx {
            let (c, _, _) = self.coeffs(it, x[k], t, &self.node(u, k), &self.slope(u, k))?;
            let v = self.vol(cells[k].0, cells[k].1);
            for p in 0..np {
                // Dirichlet rows are algebraic.
                if (k == 0 && ql[p] == 0.0) || (k == nx - 1 && qr[p] == 0.0) {
                    continue;
                }
                let i = k * np + p;
                m[i * n + i] = c[p] * v;
            }
        }
        Ok(Some(m))
    }

    fn mass_depends_on_y(&self) -> bool {
        true
    }

    fn jacobian(&mut self, _: &mut Interpreter, _: f64, _: &[f64]) -> R<Option<Vec<f64>>> {
        Ok(None)
    }

    fn band(&self) -> Option<usize> {
        Some(2 * self.npde - 1)
    }

    fn evals(&self) -> usize {
        self.nf
    }
}

fn pdepe(it: &mut Interpreter, a: &[Value]) -> Ret {
    let m = arg(a, 0, "pdepe")?.scalar()?;
    if ![0.0, 1.0, 2.0].contains(&m) {
        return rt("M must be 0, 1, or 2.");
    }
    let pde = arg(a, 1, "pdepe")?.clone();
    let ic = arg(a, 2, "pdepe")?.clone();
    let bc = arg(a, 3, "pdepe")?.clone();
    let x = arg(a, 4, "pdepe")?.to_matrix()?.data;
    let tspan = arg(a, 5, "pdepe")?.to_matrix()?.data;
    let o = options_arg(a.get(6))?;
    let extra: Vec<Value> = a.get(7..).unwrap_or(&[]).to_vec();
    if x.len() < 3 {
        return rt("XMESH must have at least 3 entries.");
    }
    if tspan.len() < 3 {
        return rt("TSPAN must have at least 3 entries.");
    }
    if x.windows(2).any(|w| w[1] <= w[0]) {
        return rt("The entries of XMESH must be strictly increasing.");
    }
    if m > 0.0 && x[0] < 0.0 {
        return rt("When M > 0, XMESH must not contain negative values.");
    }
    let mut u0 = Vec::new();
    for &xi in &x {
        let mut args = vec![Value::num(xi)];
        args.extend_from_slice(&extra);
        u0.extend(call1(it, &ic, args)?.to_matrix()?.data);
    }
    let npde = u0.len() / x.len();
    let (t0, tfinal, tdir) = check_tspan(&tspan)?;
    let mut sys = Pde { m, pde, bc, extra, x: x.clone(), npde, nf: 0 };
    let n = u0.len();
    let tol = Tol::new(it, o, n, "pdepe")?;
    let s = Setup { t0, tfinal, tdir, hmax: (tfinal - t0).abs() / 10.0, h0: None, stops: Vec::new() };
    let mut c = Counts::default();
    let traj = trbdf2(it, &mut sys, &s, &tol, u0, None, &mut EventLog::default(), &mut c)?;
    let (nt, nx) = (tspan.len(), x.len());
    let mut out = vec![0.0; nt * nx * npde];
    for (k, &tq) in tspan.iter().enumerate() {
        let u = if tq == traj.t[0] { traj.y[0].clone() } else { traj.eval(tq).0 };
        for j in 0..nx {
            for p in 0..npde {
                out[p * nt * nx + j * nt + k] = u[j * npde + p];
            }
        }
    }
    let dims = if npde == 1 { vec![nt, nx] } else { vec![nt, nx, npde] };
    one(Value::Num(Matrix::from_dims(dims, out)))
}

/// `pdeval(m, xmesh, ui, xout)`: the solution and its slope between mesh points.
fn pdeval(a: &[Value], nargout: usize) -> Ret {
    let x = arg(a, 1, "pdeval")?.to_matrix()?.data;
    let ui = arg(a, 2, "pdeval")?.to_matrix()?.data;
    let xout = arg(a, 3, "pdeval")?.to_matrix()?.data;
    if ui.len() != x.len() {
        return rt("UI must have the same length as XMESH.");
    }
    let (mut u, mut du) = (Vec::new(), Vec::new());
    for &xq in &xout {
        if xq < x[0] || xq > x[x.len() - 1] {
            return rt("XOUT must lie within the interval of XMESH.");
        }
        let k = (0..x.len() - 1).find(|&k| xq <= x[k + 1]).unwrap_or(x.len() - 2);
        let slope = (ui[k + 1] - ui[k]) / (x[k + 1] - x[k]);
        u.push(ui[k] + slope * (xq - x[k]));
        du.push(slope);
    }
    Ok(Some(vec![row(u), row(du)].into_iter().take(nargout.max(1)).collect()))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> Ret {
    match name {
        "ode15s" | "ode23s" | "ode23t" | "ode23tb" | "ode113" | "ode78" | "ode89" => ode_solve(it, name, a, nargout),
        "ode15i" => ode15i(it, a, nargout),
        "decic" => decic(it, a, nargout),
        "odextend" => odextend(it, a),
        "dde23" | "ddesd" | "ddensd" => dde_solve(it, name, a),
        "ddeset" => one(Value::Struct(make_options(a, DDE_FIELDS, "property")?)),
        "ddeget" => optget(a, DDE_FIELDS, "property"),
        "bvpset" => one(Value::Struct(make_options(a, BVP_FIELDS, "property")?)),
        "bvpget" => optget(a, BVP_FIELDS, "property"),
        "bvp4c" | "bvp5c" => bvp_solve(it, name, a),
        "bvpinit" => bvpinit(it, a),
        "bvpxtend" => bvpxtend(a),
        "pdepe" => pdepe(it, a),
        "pdeval" => pdeval(a, nargout),
        _ => Ok(None),
    }
}
