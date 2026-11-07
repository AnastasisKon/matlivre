//! Sparse LU factorization (Markowitz pivoting with threshold, hyper-sparse solves).
//!
//! Copied from the SolverLivre project (same authors), where it factorizes simplex bases; only the
//! error type was replaced. `A\b` uses `LuFactor::factorize_csc` and `ftran` for sparse matrices.
//! The Forrest-Tomlin update and the other simplex-only parts are unused here.
#![allow(dead_code)]

//! Sparse LU factorization of the simplex basis with Forrest-Tomlin updates.
//!
//! The basis `B` (m x m, columns indexed by *basis position*, rows by constraint row) is
//! factorized as `B^-1 = U^-1 R L^-1`:
//!
//! * `L^-1` is a sequence of column etas (`a[i] -= l_i * a[pivot_row]`), produced by a
//!   right-looking sparse Gaussian elimination. Pivots follow a Markowitz-style rule: the
//!   active column with the fewest entries is taken first (column singletons cost no fill),
//!   and within it the sparsest row among those passing threshold pivoting
//!   (`|a| >= 0.1 max |col|`).
//! * `U` is stored by column in *row space*: column `p` holds the entries in the pivot rows of
//!   earlier pivots plus a diagonal, and the pivot order is kept in `order`.
//! * `R` is the list of row etas created by Forrest-Tomlin updates (`a[r] -= sum m_s a[s]`).
//!
//! A Forrest-Tomlin update replaces the column of the leaving position by the *spike*
//! `R L^-1 a_q` (saved during the entering column's ftran), moves that position to the end of
//! the pivot order and eliminates the old row of `U` with a new row eta. Only the spike's
//! nonzeros enter the factors, so fill grows with the sparsity of the spike rather than of
//! `B^-1 a_q`. The update is rejected (caller refactors) when the new diagonal disagrees with
//! the pivot element `alpha_p * u_pp` predicted by the determinant identity, which doubles as
//! a cheap stability estimate.
//!
//! Hyper-sparse solves ([`LuFactor::ftran_sparse`], [`LuFactor::btran_sparse`]) work on
//! [`SparseVec`]s (dense values plus a nonzero index list) so their cost follows the number of
//! nonzeros touched rather than `m`. When the right-hand side is sparse, the triangular solve
//! with `U` first finds the nonzero pattern of the result by a depth-first search over the
//! column graph of `U` (Gilbert-Peierls) and then visits only those positions, in topological
//! order. `U` is kept by column (`ucol`) and by row (`urow`); a replaced column is retired from
//! the row lists lazily, by bumping its version (`uver`), and purged on compaction.
//!
//! References: J. J. Forrest and J. A. Tomlin, "Updated triangular factors of the basis to
//! maintain sparsity in the product form simplex method", Math. Programming 2 (1972); I. S.
//! Duff, A. M. Erisman, J. K. Reid, *Direct Methods for Sparse Matrices* (2nd ed., 2017),
//! ch. 10; T. A. Davis, *Direct Methods for Sparse Linear Systems* (SIAM, 2006); J. R.
//! Gilbert and T. Peierls, "Sparse partial pivoting in time proportional to arithmetic
//! operations", SIAM J. Sci. Stat. Comput. 9 (1988).

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverError {
    NumericalError,
}
pub type SolverResult<T> = Result<T, SolverError>;

/// A set of growable lists stored in one contiguous buffer (no per-list heap allocation).
/// Each list owns a `(start, len, cap)` window; a list that outgrows its window moves to the
/// end of the buffer, leaving a hole that `compact` reclaims. Cloning is a few memcpys.
#[derive(Clone)]
struct Arena<T: Copy + Default> {
    start: Vec<usize>,
    len: Vec<u32>,
    cap: Vec<u32>,
    data: Vec<T>,
    /// Total number of live entries.
    live: usize,
}

impl<T: Copy + Default> Arena<T> {
    fn new(n: usize) -> Self {
        Arena { start: vec![0; n], len: vec![0; n], cap: vec![0; n], data: Vec::new(), live: 0 }
    }

    #[inline]
    fn slice(&self, i: usize) -> &[T] {
        &self.data[self.start[i]..self.start[i] + self.len[i] as usize]
    }

    #[inline]
    fn len_of(&self, i: usize) -> usize {
        self.len[i] as usize
    }

    /// Make room for at least `need` entries in list `i` (moving it to the buffer end if needed).
    fn reserve(&mut self, i: usize, need: usize) {
        if need <= self.cap[i] as usize {
            return;
        }
        let new_cap = need.max(4).max(2 * self.cap[i] as usize);
        let new_start = self.data.len();
        let (s, l) = (self.start[i], self.len[i] as usize);
        self.data.extend_from_within(s..s + l);
        self.data.resize(new_start + new_cap, T::default());
        self.start[i] = new_start;
        self.cap[i] = new_cap as u32;
    }

    fn push(&mut self, i: usize, x: T) {
        let l = self.len[i] as usize;
        self.reserve(i, l + 1);
        let at = self.start[i] + l;
        self.data[at] = x;
        self.len[i] += 1;
        self.live += 1;
    }

    fn clear(&mut self, i: usize) {
        self.live -= self.len[i] as usize;
        self.len[i] = 0;
    }

    fn swap_remove(&mut self, i: usize, ix: usize) {
        let l = self.len[i] as usize;
        let s = self.start[i];
        self.data.swap(s + ix, s + l - 1);
        self.len[i] -= 1;
        self.live -= 1;
    }

    /// Replace the contents of list `i`.
    fn set(&mut self, i: usize, items: &[T]) {
        self.live -= self.len[i] as usize;
        self.len[i] = 0;
        self.reserve(i, items.len());
        let s = self.start[i];
        self.data[s..s + items.len()].copy_from_slice(items);
        self.len[i] = items.len() as u32;
        self.live += items.len();
    }

    /// Drop the holes left by relocated lists.
    fn compact(&mut self) {
        let n = self.start.len();
        let mut data: Vec<T> = Vec::with_capacity(self.live + 4 * n);
        for i in 0..n {
            let (s, l) = (self.start[i], self.len[i] as usize);
            let ns = data.len();
            data.extend_from_slice(&self.data[s..s + l]);
            data.resize(ns + l + 2, T::default());
            self.start[i] = ns;
            self.cap[i] = (l + 2) as u32;
        }
        self.data = data;
    }

    fn needs_compaction(&self) -> bool {
        self.data.len() > 3 * self.live + 8 * self.start.len() + 256
    }
}

impl<T: Copy + Default> Arena<T> {
    /// Lists `0..n` built from `(list, item)` pairs, each list allocated once with `spare`
    /// extra room.
    fn from_pairs<I: Iterator<Item = (usize, T)> + Clone>(n: usize, spare: u32, pairs: I) -> Self {
        let mut a = Arena::new(n);
        for (i, _) in pairs.clone() {
            a.cap[i] += 1;
        }
        let mut total = 0usize;
        for i in 0..n {
            a.start[i] = total;
            a.cap[i] += spare;
            total += a.cap[i] as usize;
        }
        a.data = vec![T::default(); total];
        for (i, x) in pairs {
            let at = a.start[i] + a.len[i] as usize;
            a.data[at] = x;
            a.len[i] += 1;
            a.live += 1;
        }
        a
    }
}

/// A vector of length `m` stored densely (`val`) together with the list of its nonzero
/// indices (`idx`). Entries outside `idx` are zero; an entry that cancels to exactly zero is
/// stored as [`SparseVec::ZERO`] so that it stays listed exactly once.
#[derive(Clone, Default, Debug)]
pub struct SparseVec {
    pub val: Vec<f64>,
    pub idx: Vec<u32>,
}

impl SparseVec {
    /// Stand-in for an exact cancellation (keeps the index list free of duplicates).
    pub const ZERO: f64 = 1e-50;

    pub fn new(m: usize) -> Self {
        SparseVec { val: vec![0.0; m], idx: Vec::new() }
    }

    /// Make the vector an all-zero vector of length `m`.
    pub fn reset(&mut self, m: usize) {
        if self.val.len() != m {
            self.val.clear();
            self.val.resize(m, 0.0);
            self.idx.clear();
        } else {
            self.clear();
        }
    }

    /// Zero the listed entries.
    pub fn clear(&mut self) {
        for &i in &self.idx {
            self.val[i as usize] = 0.0;
        }
        self.idx.clear();
    }

    /// `self[i] += v`.
    #[inline]
    pub fn add(&mut self, i: usize, v: f64) {
        if v == 0.0 {
            return;
        }
        let old = self.val[i];
        if old == 0.0 {
            self.idx.push(i as u32);
        }
        let new = old + v;
        self.val[i] = if new == 0.0 { Self::ZERO } else { new };
    }

    /// Number of listed (possibly nonzero) entries.
    pub fn count(&self) -> usize {
        self.idx.len()
    }
}

/// Reusable scratch space for the hyper-sparse solves (depth-first searches) and the
/// Forrest-Tomlin update, kept by the caller so that factorizations stay cheap to clone.
#[derive(Clone, Default)]
pub struct SolveScratch {
    mark: Vec<u32>,
    gen: u32,
    stack: Vec<(u32, u32)>,
    list: Vec<u32>,
    // Forrest-Tomlin update: dense row being eliminated, its stamps and pivot-order heap.
    w: Vec<f64>,
    ustamp: Vec<u32>,
    ugen: u32,
    heap: BinaryHeap<Reverse<(u32, u32)>>,
    eta: Vec<(u32, f64)>,
    col: Vec<(u32, f64)>,
}

impl SolveScratch {
    fn next_gen(&mut self, m: usize) -> u32 {
        if self.mark.len() != m {
            self.mark = vec![0; m];
            self.gen = 0;
        }
        self.gen = self.gen.wrapping_add(1);
        if self.gen == 0 {
            self.mark.iter_mut().for_each(|v| *v = 0);
            self.gen = 1;
        }
        self.gen
    }

    fn next_ugen(&mut self, m: usize) -> u32 {
        if self.ustamp.len() != m {
            self.ustamp = vec![0; m];
            self.w = vec![0.0; m];
            self.ugen = 0;
        }
        self.ugen = self.ugen.wrapping_add(1);
        if self.ugen == 0 {
            self.ustamp.iter_mut().for_each(|v| *v = 0);
            self.ugen = 1;
        }
        self.ugen
    }
}

/// The L etas of a factorization (fixed once computed, so shared between clones): eta `t` has
/// pivot row `row[t]` and entries `idx[ptr[t]..ptr[t+1]]` with values `val[..]`.
#[derive(Clone)]
struct LEtas {
    row: Vec<u32>,
    ptr: Vec<usize>,
    idx: Vec<u32>,
    val: Vec<f64>,
}

/// Use the depth-first (hyper-sparse) triangular solve when the right-hand side has fewer
/// than `m / HYPER_RATIO` nonzeros.
const HYPER_RATIO: usize = 10;

/// Marker for a removed entry of the pivot order.
const TOMB: u32 = u32::MAX;
/// Smallest acceptable pivot magnitude.
const PIVOT_FLOOR: f64 = 1e-11;
/// Values below this are treated as cancellation residue (dropped) in the dense solve paths.
const TINY: f64 = 1e-14;
/// Threshold pivoting parameter. 0.01 leaves the Markowitz search more candidates than 0.1
/// (10-18% less fill on the mixed LP benchmark); drift is caught by the per-pivot check in
/// the simplex loops and by the condition estimate.
const THRESHOLD: f64 = 0.01;
/// Condition-number estimate above which the factors are rebuilt.
const COND_LIMIT: f64 = 1e10;
/// Columns and rows examined per Markowitz pivot search (Duff-Reid style limited search).
const MARKOWITZ_SEARCH: usize = 8;
/// Updates allowed between refactorizations on small bases (see `update_limit`). 150 measured
/// about 2% faster than 100 on the LP benchmarks (2000x2000 sparse: 10.7 -> 9.0 ms), neutral on
/// MIPs; 60 was about 6% slower.
const MAX_UPDATES: usize = 150;

/// Active-submatrix density above which factorization switches to a dense kernel.
const DENSE_SWITCH: f64 = 0.7;
/// Smallest active submatrix handled by the dense kernel (on small bases, such as MIP node
/// LPs, the extra fill costs more in the solves than the faster factorization saves).
const DENSE_MIN: usize = 64;

/// Dense LU (partial pivoting by rows, column swaps only for numerically empty columns) of the
/// remaining active submatrix, appending to the factors in the same form as the sparse steps:
/// each pivot row's other entries become U entries, each elimination an L eta.
struct DenseKernel<'a> {
    prow: &'a mut Vec<u32>,
    udiag: &'a mut Vec<f64>,
    order: &'a mut Vec<u32>,
    u_ent: &'a mut Vec<(u32, u32, f64)>,
    l: &'a mut LEtas,
}

impl DenseKernel<'_> {
    fn run(&mut self, rows: &Arena<(u32, f64)>, row_active: &[bool], col_done: &[bool]) -> SolverResult<()> {
        let m = row_active.len();
        let rlist: Vec<u32> = (0..m as u32).filter(|&i| row_active[i as usize]).collect();
        let mut clist: Vec<u32> = (0..m as u32).filter(|&c| !col_done[c as usize]).collect();
        let k = rlist.len();
        if clist.len() != k {
            return Err(SolverError::NumericalError);
        }
        // Column-major k x k block; `cpos` maps a column to its block index.
        let mut cpos = vec![u32::MAX; m];
        for (b, &c) in clist.iter().enumerate() {
            cpos[c as usize] = b as u32;
        }
        let mut d = vec![0.0; k * k];
        for (r, &i) in rlist.iter().enumerate() {
            for &(c, v) in rows.slice(i as usize) {
                let b = cpos[c as usize];
                if b != u32::MAX {
                    d[b as usize * k + r] = v;
                }
            }
        }
        // Row r of the block is original row `rid[r]` (rows are swapped physically).
        let mut rid = rlist;
        for s in 0..k {
            // Pivot: largest entry of column s among rows s.., or of a later column if
            // column s is numerically empty.
            let mut pick: Option<(usize, usize)> = None;
            'cols: for cs in s..k {
                let col = &d[cs * k..cs * k + k];
                let (mut rbest, mut vbest) = (s, 0.0f64);
                for (r, &v) in col.iter().enumerate().skip(s) {
                    if v.abs() > vbest {
                        vbest = v.abs();
                        rbest = r;
                    }
                }
                if vbest >= PIVOT_FLOOR {
                    pick = Some((cs, rbest));
                    break 'cols;
                }
            }
            let Some((cs, rp)) = pick else {
                return Err(SolverError::NumericalError);
            };
            if cs != s {
                for r in 0..k {
                    d.swap(cs * k + r, s * k + r);
                }
                clist.swap(cs, s);
            }
            if rp != s {
                for c in 0..k {
                    d.swap(c * k + rp, c * k + s);
                }
                rid.swap(rp, s);
            }
            let piv = d[s * k + s];
            let (j, ip) = (clist[s] as usize, rid[s]);
            self.prow[j] = ip;
            self.udiag[j] = piv;
            self.order.push(j as u32);
            for c in s + 1..k {
                let u = d[c * k + s];
                if u != 0.0 {
                    self.u_ent.push((clist[c], ip, u));
                }
            }
            // Multipliers (kept in column s), then the rank-1 update column by column.
            let start = self.l.idx.len();
            for r in s + 1..k {
                let v = d[s * k + r];
                if v != 0.0 {
                    let lm = v / piv;
                    d[s * k + r] = lm;
                    self.l.idx.push(rid[r]);
                    self.l.val.push(lm);
                }
            }
            if self.l.idx.len() > start {
                self.l.row.push(ip);
                self.l.ptr.push(self.l.idx.len());
                let (left, right) = d.split_at_mut((s + 1) * k);
                let lcol = &left[s * k + s + 1..s * k + k];
                for c in 0..k - s - 1 {
                    let col = &mut right[c * k..c * k + k];
                    let u = col[s];
                    if u == 0.0 {
                        continue;
                    }
                    for (x, &lm) in col[s + 1..].iter_mut().zip(lcol) {
                        *x -= lm * u;
                    }
                }
            }
        }
        Ok(())
    }
}

/// A sparse LU factorization with Forrest-Tomlin updates (see the module docs).
#[derive(Clone)]
pub struct LuFactor {
    m: usize,
    /// L etas (shared: a clone made for copy-on-write updates does not copy them).
    l: Arc<LEtas>,
    // R etas (Forrest-Tomlin row etas), same layout, append-only.
    r_row: Vec<u32>,
    r_ptr: Vec<usize>,
    r_idx: Vec<u32>,
    r_val: Vec<f64>,
    // U by position.
    ucol: Arena<(u32, f64)>,
    udiag: Vec<f64>,
    prow: Vec<u32>,
    /// Pivot sequence of basis positions. Removed entries are tombstones (`TOMB`) so that moving
    /// a position to the end costs O(1); the list is compacted once tombstones pile up.
    order: Vec<u32>,
    order_pos: Vec<u32>,
    tombs: usize,
    /// U by row: row `r` lists `(position, version, value)` for the entries of U in row `r`.
    /// An entry is current only while `version == uver[position]`.
    urow: Arena<(u32, u32, f64)>,
    uver: Vec<u32>,
    /// Stale entries in `urow` (purged when they pile up).
    ustale: usize,
    /// Position whose pivot row is row `r` (inverse of `prow`; fixed for a factorization).
    ppos: Arc<Vec<u32>>,
    nnz_b: usize,
    /// Stored nonzeros right after the factorization (before any update).
    nnz_f: usize,
    /// 1-norm of the basis matrix (for condition estimates).
    norm_b: f64,
    /// Forrest-Tomlin updates applied since the last factorization.
    pub updates: usize,
}

impl LuFactor {
    /// Factorization of the identity (the all-slack basis), without any elimination work.
    pub fn identity(m: usize) -> Self {
        LuFactor {
            m,
            l: Arc::new(LEtas { row: Vec::new(), ptr: vec![0], idx: Vec::new(), val: Vec::new() }),
            r_row: Vec::new(),
            r_ptr: vec![0],
            r_idx: Vec::new(),
            r_val: Vec::new(),
            ucol: Arena::new(m),
            udiag: vec![1.0; m],
            prow: (0..m as u32).collect(),
            order: (0..m as u32).collect(),
            order_pos: (0..m as u32).collect(),
            tombs: 0,
            urow: Arena::new(m),
            uver: vec![0; m],
            ustale: 0,
            ppos: Arc::new((0..m as u32).collect()),
            nnz_b: m,
            nnz_f: m,
            norm_b: if m > 0 { 1.0 } else { 0.0 },
            updates: 0,
        }
    }

    /// Factorize the matrix whose column `k` is `cols[k]` (a list of `(row, value)`).
    pub fn factorize(m: usize, cols: &[Vec<(usize, f64)>]) -> SolverResult<Self> {
        let mut ptr = Vec::with_capacity(m + 1);
        let mut idx = Vec::new();
        let mut val = Vec::new();
        ptr.push(0);
        for col in cols {
            for &(i, v) in col {
                idx.push(i as u32);
                val.push(v);
            }
            ptr.push(idx.len());
        }
        Self::factorize_csc(m, &ptr, &idx, &val)
    }

    /// Factorize the matrix given in compressed sparse column form: column `k` has the
    /// entries `idx[ptr[k]..ptr[k+1]]` (rows) with values `val[..]`.
    ///
    /// Right-looking elimination on the active submatrix, stored row-wise with values (`rows`)
    /// and column-wise as a pattern (`colpat`, a superset verified against `rows`). Columns sit
    /// and rows in doubly linked buckets by their exact active count. Each step takes a column
    /// singleton, else a row singleton (absolute pivot test only: no fill), else runs a
    /// Markowitz search: up to `MARKOWITZ_SEARCH` columns and rows of count 2, 3, ... are
    /// examined alternately and, among their entries passing threshold pivoting, the one with
    /// the smallest `(r - 1)(c - 1)` is taken.
    pub fn factorize_csc(m: usize, ptr: &[usize], idx: &[u32], val: &[f64]) -> SolverResult<Self> {
        const NIL: u32 = u32::MAX;

        // Active submatrix, row-wise with values (built in two passes, a little spare room).
        let mut rowcnt = vec![0u32; m];
        let mut colcnt = vec![0u32; m];
        let mut nnz_b = 0usize;
        let mut norm_b = 0.0f64;
        for k in 0..m {
            let mut s = 0.0;
            for e in ptr[k]..ptr[k + 1] {
                if val[e] != 0.0 {
                    rowcnt[idx[e] as usize] += 1;
                    colcnt[k] += 1;
                    nnz_b += 1;
                    s += val[e].abs();
                }
            }
            norm_b = norm_b.max(s);
        }
        let mut rows: Arena<(u32, f64)> = Arena::new(m);
        let mut colpat: Arena<u32> = Arena::new(m);
        {
            let mut total = 0usize;
            for i in 0..m {
                rows.start[i] = total;
                rows.cap[i] = rowcnt[i] + 4;
                total += rows.cap[i] as usize;
            }
            rows.data = vec![(0, 0.0); total];
            let mut ctotal = 0usize;
            for k in 0..m {
                colpat.start[k] = ctotal;
                colpat.cap[k] = colcnt[k] + 4;
                ctotal += colpat.cap[k] as usize;
            }
            colpat.data = vec![0; ctotal];
            for k in 0..m {
                for e in ptr[k]..ptr[k + 1] {
                    if val[e] != 0.0 {
                        let i = idx[e] as usize;
                        let at = rows.start[i] + rows.len[i] as usize;
                        rows.data[at] = (k as u32, val[e]);
                        rows.len[i] += 1;
                        let at = colpat.start[k] + colpat.len[k] as usize;
                        colpat.data[at] = i as u32;
                        colpat.len[k] += 1;
                    }
                }
            }
            rows.live = nnz_b;
            colpat.live = nnz_b;
        }

        // Count buckets: doubly linked lists of active columns by active count.
        let mut bhead = vec![NIL; m + 1];
        let mut bnext = vec![NIL; m];
        let mut bprev = vec![NIL; m];
        let mut bkey = vec![0u32; m];
        let link = |c: usize, k: u32, bhead: &mut [u32], bnext: &mut [u32], bprev: &mut [u32], bkey: &mut [u32]| {
            bkey[c] = k;
            bprev[c] = NIL;
            bnext[c] = bhead[k as usize];
            if bhead[k as usize] != NIL {
                bprev[bhead[k as usize] as usize] = c as u32;
            }
            bhead[k as usize] = c as u32;
        };
        let unlink = |c: usize, bhead: &mut [u32], bnext: &mut [u32], bprev: &mut [u32], bkey: &[u32]| {
            let (p, n) = (bprev[c], bnext[c]);
            if p != NIL {
                bnext[p as usize] = n;
            } else {
                bhead[bkey[c] as usize] = n;
            }
            if n != NIL {
                bprev[n as usize] = p;
            }
        };
        for c in (0..m).rev() {
            link(c, colcnt[c].min(m as u32), &mut bhead, &mut bnext, &mut bprev, &mut bkey);
        }
        // The same buckets for the active rows, by their active count.
        let mut rhead = vec![NIL; m + 1];
        let mut rnext = vec![NIL; m];
        let mut rprev = vec![NIL; m];
        let mut rkey = vec![0u32; m];
        for i in (0..m).rev() {
            link(i, rowcnt[i].min(m as u32), &mut rhead, &mut rnext, &mut rprev, &mut rkey);
        }

        let mut l_row: Vec<u32> = Vec::new();
        let mut l_ptr: Vec<usize> = vec![0];
        let mut l_idx: Vec<u32> = Vec::new();
        let mut l_val: Vec<f64> = Vec::new();
        // U entries as (column, row, value), turned into columns at the end.
        let mut u_ent: Vec<(u32, u32, f64)> = Vec::with_capacity(nnz_b);
        let mut udiag = vec![0.0; m];
        let mut prow = vec![0u32; m];
        let mut order: Vec<u32> = Vec::with_capacity(m);

        let mut row_active = vec![true; m];
        let mut loc = vec![-1i32; m];
        let mut in_prow = vec![u32::MAX; m];
        let mut entries: Vec<(u32, f64)> = Vec::new();
        let mut best_entries: Vec<(u32, f64)> = Vec::new();
        let mut prow_buf: Vec<(u32, f64)> = Vec::new();
        let mut kmin = 0usize;

        // Entries of active column `c` in active rows. A column that no elimination has touched
        // yet (`cmod[c]` false) still has its input values, read straight from the input;
        // otherwise the pattern is verified against the rows, with `seen` marking rows already
        // taken (the pattern may list a row twice).
        let mut seen = vec![0u32; m];
        let mut seen_gen = 0u32;
        let mut cmod = vec![false; m];
        let gather = |c: usize, rows: &Arena<(u32, f64)>, colpat: &Arena<u32>, row_active: &[bool], cmod: &[bool], seen: &mut [u32], gen: u32, out: &mut Vec<(u32, f64)>| {
            out.clear();
            if !cmod[c] {
                for e in ptr[c]..ptr[c + 1] {
                    if val[e] != 0.0 && row_active[idx[e] as usize] {
                        out.push((idx[e], val[e]));
                    }
                }
                return;
            }
            for &ri in colpat.slice(c) {
                let i = ri as usize;
                if !row_active[i] || seen[i] == gen {
                    continue;
                }
                if let Some(&(_, v)) = rows.slice(i).iter().find(|&&(cc, _)| cc as usize == c) {
                    seen[i] = gen;
                    out.push((ri, v));
                }
            }
        };

        let mut col_done = vec![false; m];
        let mut l = LEtas { row: Vec::new(), ptr: vec![0], idx: Vec::new(), val: Vec::new() };
        for step in 0..m {
            // Once the active submatrix is dense, finish with a dense LU.
            let k_act = m - step;
            if k_act >= DENSE_MIN && rows.live as f64 >= DENSE_SWITCH * (k_act * k_act) as f64 {
                std::mem::swap(&mut l.row, &mut l_row);
                std::mem::swap(&mut l.ptr, &mut l_ptr);
                std::mem::swap(&mut l.idx, &mut l_idx);
                std::mem::swap(&mut l.val, &mut l_val);
                let mut dk = DenseKernel { prow: &mut prow, udiag: &mut udiag, order: &mut order, u_ent: &mut u_ent, l: &mut l };
                dk.run(&rows, &row_active, &col_done)?;
                std::mem::swap(&mut l.row, &mut l_row);
                std::mem::swap(&mut l.ptr, &mut l_ptr);
                std::mem::swap(&mut l.idx, &mut l_idx);
                std::mem::swap(&mut l.val, &mut l_val);
                break;
            }
            // Markowitz search over the lowest buckets.
            if bhead[0] != NIL {
                return Err(SolverError::NumericalError);
            }
            while kmin <= m && bhead[kmin] == NIL {
                kmin += 1;
            }
            let mut best: Option<(usize, usize, f64, u64)> = None; // (col, row, value, cost)
            // A row singleton creates no fill: take it (absolute pivot test only) unless a
            // column singleton is available, which the column search below takes first.
            if bhead[1] == NIL && rhead[1] != NIL {
                let i = rhead[1] as usize;
                let (c, _) = rows.slice(i)[0];
                seen_gen += 1;
                gather(c as usize, &rows, &colpat, &row_active, &cmod, &mut seen, seen_gen, &mut entries);
                if let Some(&(_, v)) = entries.iter().find(|&&(r, _)| r as usize == i) {
                    if v.abs() >= PIVOT_FLOOR {
                        best = Some((c as usize, i, v, 0));
                        std::mem::swap(&mut best_entries, &mut entries);
                    }
                }
            }
            let mut searched = 0usize;
            let mut k = kmin.max(1);
            'search: while best.map_or(true, |b| b.3 > 0) && k <= m {
                let mut c = bhead[k];
                while c != NIL {
                    let cu = c as usize;
                    seen_gen += 1;
                    gather(cu, &rows, &colpat, &row_active, &cmod, &mut seen, seen_gen, &mut entries);
                    let max_abs = entries.iter().fold(0.0f64, |a, &(_, v)| a.max(v.abs()));
                    if max_abs >= PIVOT_FLOOR {
                        let cc = entries.len() as u64 - 1;
                        let mut improved = false;
                        for &(i, v) in &entries {
                            if v.abs() < THRESHOLD * max_abs {
                                continue;
                            }
                            let cost = (rows.len_of(i as usize) as u64 - 1) * cc;
                            let better = match best {
                                None => true,
                                Some((_, _, bv, bc)) => cost < bc || (cost == bc && v.abs() > bv.abs()),
                            };
                            if better {
                                best = Some((cu, i as usize, v, cost));
                                improved = true;
                            }
                        }
                        if improved {
                            std::mem::swap(&mut best_entries, &mut entries);
                        }
                        searched += 1;
                        let bc = best.map_or(u64::MAX, |b| b.3);
                        // No later candidate can beat cost (k-1)^2 by much; stop early.
                        if bc == 0 || searched >= MARKOWITZ_SEARCH || bc <= (k as u64 - 1) * (k as u64 - 1) {
                            break 'search;
                        }
                    }
                    c = bnext[cu];
                }
                // Rows with k entries: a pivot in a short row can sit in a long column that the
                // column search never reaches. The threshold test needs the column's largest
                // entry, so a column is gathered only when its entry would improve the cost.
                if k >= 2 {
                    let mut r = rhead[k];
                    while r != NIL {
                        let i = r as usize;
                        for &(c, _) in rows.slice(i) {
                            let cu = c as usize;
                            let cost = (k as u64 - 1) * (colcnt[cu] as u64).saturating_sub(1);
                            if best.is_some_and(|b| cost >= b.3) {
                                continue;
                            }
                            seen_gen += 1;
                            gather(cu, &rows, &colpat, &row_active, &cmod, &mut seen, seen_gen, &mut entries);
                            let max_abs = entries.iter().fold(0.0f64, |a, &(_, v)| a.max(v.abs()));
                            let Some(&(_, v)) = entries.iter().find(|&&(ri, _)| ri as usize == i) else { continue };
                            if max_abs >= PIVOT_FLOOR && v.abs() >= THRESHOLD * max_abs {
                                best = Some((cu, i, v, cost));
                                std::mem::swap(&mut best_entries, &mut entries);
                            }
                        }
                        searched += 1;
                        let bc = best.map_or(u64::MAX, |b| b.3);
                        if bc == 0 || searched >= MARKOWITZ_SEARCH || bc <= (k as u64 - 1) * (k as u64 - 1) {
                            break 'search;
                        }
                        r = rnext[i];
                    }
                }
                k += 1;
            }
            let Some((j, ip, pval, _)) = best else {
                return Err(SolverError::NumericalError);
            };
            let col_entries = std::mem::take(&mut best_entries);

            // The pivot row becomes row `ip` of U.
            unlink(j, &mut bhead, &mut bnext, &mut bprev, &bkey);
            col_done[j] = true;
            row_active[ip] = false;
            unlink(ip, &mut rhead, &mut rnext, &mut rprev, &rkey);
            prow_buf.clear();
            prow_buf.extend_from_slice(rows.slice(ip));
            rows.clear(ip);
            for &(c, v) in &prow_buf {
                let cu = c as usize;
                if cu != j {
                    u_ent.push((c, ip as u32, v));
                    colcnt[cu] -= 1;
                    in_prow[cu] = step as u32;
                }
            }
            prow[j] = ip as u32;
            udiag[j] = pval;
            order.push(j as u32);

            // Eliminate column j from the other active rows.
            let l_start = l_idx.len();
            for &(ri, v) in &col_entries {
                let i = ri as usize;
                if i == ip {
                    continue;
                }
                let l = v / pval;
                l_idx.push(ri);
                l_val.push(l);
                if let Some(pos) = rows.slice(i).iter().position(|&(c, _)| c as usize == j) {
                    rows.swap_remove(i, pos);
                }
                for (e, &(c, _)) in rows.slice(i).iter().enumerate() {
                    loc[c as usize] = e as i32;
                }
                for &(c, u) in &prow_buf {
                    let cu = c as usize;
                    if cu == j {
                        continue;
                    }
                    let delta = -l * u;
                    if loc[cu] >= 0 {
                        let at = rows.start[i] + loc[cu] as usize;
                        rows.data[at].1 += delta;
                    } else {
                        rows.push(i, (c, delta));
                        loc[cu] = rows.len_of(i) as i32 - 1;
                        colpat.push(cu, ri);
                        colcnt[cu] += 1;
                    }
                }
                // Reset the markers and drop numerically cancelled entries (only entries in
                // pivot-row columns changed).
                let mut e = 0;
                while e < rows.len_of(i) {
                    let (c, v) = rows.slice(i)[e];
                    loc[c as usize] = -1;
                    if in_prow[c as usize] == step as u32 && v.abs() < 1e-14 {
                        rows.swap_remove(i, e);
                        colcnt[c as usize] -= 1;
                    } else {
                        e += 1;
                    }
                }
                let key = (rows.len_of(i) as u32).min(m as u32);
                if key != rkey[i] {
                    unlink(i, &mut rhead, &mut rnext, &mut rprev, &rkey);
                    link(i, key, &mut rhead, &mut rnext, &mut rprev, &mut rkey);
                }
            }
            if l_idx.len() > l_start {
                l_row.push(ip as u32);
                l_ptr.push(l_idx.len());
                // The pivot row's columns now hold eliminated values.
                for &(c, _) in &prow_buf {
                    cmod[c as usize] = true;
                }
            }
            best_entries = col_entries;

            // Re-bucket the columns of the pivot row (their counts changed).
            for &(c, _) in &prow_buf {
                let cu = c as usize;
                if cu == j {
                    continue;
                }
                let key = colcnt[cu].min(m as u32);
                if key != bkey[cu] {
                    unlink(cu, &mut bhead, &mut bnext, &mut bprev, &bkey);
                    link(cu, key, &mut bhead, &mut bnext, &mut bprev, &mut bkey);
                }
                kmin = kmin.min(key as usize);
            }
        }

        // U by column and by row from the recorded entries.
        let ucol = Arena::from_pairs(m, 2, u_ent.iter().map(|&(c, r, v)| (c as usize, (r, v))));
        let urow = Arena::from_pairs(m, 2, u_ent.iter().map(|&(c, r, v)| (r as usize, (c, 0u32, v))));
        let mut ppos = vec![0u32; m];
        for (p, &r) in prow.iter().enumerate() {
            ppos[r as usize] = p as u32;
        }

        let mut order_pos = vec![0u32; m];
        for (i, &p) in order.iter().enumerate() {
            order_pos[p as usize] = i as u32;
        }
        let (l_nnz, u_nnz) = (l_idx.len(), ucol.live);
        Ok(LuFactor {
            m,
            l: Arc::new(LEtas { row: l_row, ptr: l_ptr, idx: l_idx, val: l_val }),
            r_row: Vec::new(),
            r_ptr: vec![0],
            r_idx: Vec::new(),
            r_val: Vec::new(),
            ucol,
            udiag,
            prow,
            order,
            order_pos,
            tombs: 0,
            urow,
            uver: vec![0; m],
            ustale: 0,
            ppos: Arc::new(ppos),
            nnz_b,
            nnz_f: l_nnz + u_nnz + m,
            norm_b,
            updates: 0,
        })
    }

    /// Hager/Higham estimate of `||B^-1||_1`, using only solves with `B` and `B^T`.
    pub fn inverse_norm1_estimate(&self) -> f64 {
        let m = self.m;
        if m == 0 {
            return 0.0;
        }
        let mut x = vec![1.0 / m as f64; m];
        let mut est = 0.0;
        for _ in 0..5 {
            let y = self.ftran(&x, None);
            let gamma: f64 = y.iter().map(|v| v.abs()).sum();
            if gamma <= est {
                break;
            }
            est = gamma;
            let xi: Vec<f64> = y.iter().map(|v| if *v >= 0.0 { 1.0 } else { -1.0 }).collect();
            let z = self.btran(&xi);
            let (jmax, zmax) = z.iter().enumerate().fold((0, 0.0f64), |(bj, bz), (j, v)| if v.abs() > bz { (j, v.abs()) } else { (bj, bz) });
            let zx: f64 = z.iter().zip(&x).map(|(a, b)| a * b).sum();
            if zmax <= zx {
                break;
            }
            x = vec![0.0; m];
            x[jmax] = 1.0;
        }
        est
    }

    /// 1-norm condition estimate `||B||_1 * est(||B^-1||_1)` of the current factors.
    pub fn condition_estimate(&self) -> f64 {
        self.norm_b * self.inverse_norm1_estimate()
    }

    /// Approximate heap memory of the factors, in bytes.
    pub fn memory_bytes(&self) -> usize {
        12 * (self.l.idx.len() + self.r_idx.len()) + 16 * self.ucol.data.len() + 16 * self.urow.data.len() + 60 * self.m
    }

    /// Number of stored nonzeros in the factors (L + U + R + diagonal).
    pub fn nnz(&self) -> usize {
        self.l.idx.len() + self.r_idx.len() + self.ucol.live + self.m
    }

    /// Nonzeros of the basis matrix this factorization started from.
    pub fn basis_nnz(&self) -> usize {
        self.nnz_b
    }

    /// Updates allowed before a refactorization. Grows with the basis size: on large sparse
    /// bases the Forrest-Tomlin factors stay sparse for many updates, while a factorization
    /// costs O(m) or more (the fill check in `needs_refactor` still guards against growth).
    pub fn update_limit(&self) -> usize {
        (self.m / 25).clamp(MAX_UPDATES, 1000)
    }

    /// Should the caller refactorize (update count, fill growth or conditioning)?
    pub fn needs_refactor(&self) -> bool {
        let lim = self.update_limit();
        // Fill is measured from the fresh factors, not the basis: a basis whose factors fill
        // in heavily would otherwise ask for a refactorization after every update.
        if self.updates >= lim || self.nnz() > self.nnz_f + 3 * (self.nnz_b + self.m) + 200 {
            return true;
        }
        // Stability: the updated factors are badly conditioned. The estimate costs a few dense
        // solves, so it only runs at mid-cycle as a backstop; the simplex loops catch drift on
        // every pivot by comparing the pivot element from the column and from the row.
        self.updates > 0 && self.updates % (lim / 2).max(20) == 0 && self.condition_estimate() > COND_LIMIT
    }

    /// Solve `B x = rhs`. `rhs` is indexed by row, the result by basis position. If `spike`
    /// is given it receives the partially transformed vector `R L^-1 rhs` needed by [`update`].
    pub fn ftran(&self, rhs: &[f64], spike: Option<&mut Vec<f64>>) -> Vec<f64> {
        let (mut a, mut x) = (Vec::new(), Vec::new());
        self.ftran_into(rhs, &mut a, &mut x, spike);
        x
    }

    /// Allocation-free [`ftran`](Self::ftran): `a` is scratch space, the result is written to `x`
    /// (both are resized as needed and may hold arbitrary data on entry).
    pub fn ftran_into(&self, rhs: &[f64], a: &mut Vec<f64>, x: &mut Vec<f64>, spike: Option<&mut Vec<f64>>) {
        a.clear();
        a.extend_from_slice(rhs);
        for t in 0..self.l.row.len() {
            let v = a[self.l.row[t] as usize];
            if v != 0.0 {
                for k in self.l.ptr[t]..self.l.ptr[t + 1] {
                    a[self.l.idx[k] as usize] -= self.l.val[k] * v;
                }
            }
        }
        for t in 0..self.r_row.len() {
            let mut s = 0.0;
            for k in self.r_ptr[t]..self.r_ptr[t + 1] {
                s += self.r_val[k] * a[self.r_idx[k] as usize];
            }
            a[self.r_row[t] as usize] -= s;
        }
        if let Some(sp) = spike {
            sp.clear();
            sp.extend_from_slice(a);
        }
        x.clear();
        x.resize(self.m, 0.0);
        for &p in self.order.iter().rev() {
            if p == TOMB {
                continue;
            }
            let p = p as usize;
            let v = a[self.prow[p] as usize];
            if v != 0.0 {
                let xp = v / self.udiag[p];
                x[p] = xp;
                for &(row, u) in self.ucol.slice(p) {
                    a[row as usize] -= u * xp;
                }
            }
        }
    }

    /// Solve `B^T y = c`. `c` is indexed by basis position, the result by row.
    pub fn btran(&self, c: &[f64]) -> Vec<f64> {
        let mut z = Vec::new();
        self.btran_into(c, &mut z);
        z
    }

    /// Allocation-free [`btran`](Self::btran): the result is written to `z`.
    pub fn btran_into(&self, c: &[f64], z: &mut Vec<f64>) {
        z.clear();
        z.resize(self.m, 0.0);
        for &p in &self.order {
            if p == TOMB {
                continue;
            }
            let p = p as usize;
            let mut s = c[p];
            for &(row, u) in self.ucol.slice(p) {
                s -= u * z[row as usize];
            }
            z[self.prow[p] as usize] = s / self.udiag[p];
        }
        for t in (0..self.r_row.len()).rev() {
            let v = z[self.r_row[t] as usize];
            if v != 0.0 {
                for k in self.r_ptr[t]..self.r_ptr[t + 1] {
                    z[self.r_idx[k] as usize] -= self.r_val[k] * v;
                }
            }
        }
        for t in (0..self.l.row.len()).rev() {
            let mut s = 0.0;
            for k in self.l.ptr[t]..self.l.ptr[t + 1] {
                s += self.l.val[k] * z[self.l.idx[k] as usize];
            }
            z[self.l.row[t] as usize] -= s;
        }
    }

    /// Positions reachable from the nonzeros of `a` (indexed by row) in the column graph of
    /// U (`p -> ppos[r]` for each entry `(r, _)` of column `p`), in DFS postorder in
    /// `sc.list`. Reversed, the list is a valid order for the backward solve with U.
    fn reach_ftran(&self, a: &SparseVec, sc: &mut SolveScratch) {
        let gen = sc.next_gen(self.m);
        sc.list.clear();
        for &ri in &a.idx {
            let s0 = self.ppos[ri as usize] as usize;
            if sc.mark[s0] == gen {
                continue;
            }
            sc.mark[s0] = gen;
            sc.stack.push((s0 as u32, 0));
            while let Some(&(p, k)) = sc.stack.last() {
                let col = self.ucol.slice(p as usize);
                if (k as usize) < col.len() {
                    sc.stack.last_mut().unwrap().1 = k + 1;
                    let q = self.ppos[col[k as usize].0 as usize] as usize;
                    if sc.mark[q] != gen {
                        sc.mark[q] = gen;
                        sc.stack.push((q as u32, 0));
                    }
                } else {
                    sc.stack.pop();
                    sc.list.push(p);
                }
            }
        }
    }

    /// Positions reachable from the nonzeros of `c` (indexed by position) in the row graph of
    /// U (`p -> q` for each current entry `(q, _, _)` of row `prow[p]`), in DFS postorder in
    /// `sc.list`. Reversed, the list is a valid order for the forward solve with U^T.
    fn reach_btran(&self, c: &SparseVec, sc: &mut SolveScratch) {
        let gen = sc.next_gen(self.m);
        sc.list.clear();
        for &pi in &c.idx {
            let s0 = pi as usize;
            if sc.mark[s0] == gen {
                continue;
            }
            sc.mark[s0] = gen;
            sc.stack.push((s0 as u32, 0));
            while let Some(&(p, k)) = sc.stack.last() {
                let row = self.urow.slice(self.prow[p as usize] as usize);
                if (k as usize) < row.len() {
                    sc.stack.last_mut().unwrap().1 = k + 1;
                    let (q, ver, _) = row[k as usize];
                    let q = q as usize;
                    if ver == self.uver[q] && sc.mark[q] != gen {
                        sc.mark[q] = gen;
                        sc.stack.push((q as u32, 0));
                    }
                } else {
                    sc.stack.pop();
                    sc.list.push(p);
                }
            }
        }
    }

    /// Hyper-sparse `B x = rhs`: `rhs` is indexed by row and is consumed (left all zero), the
    /// result `x` is indexed by basis position. If `spike` is given it receives `R L^-1 rhs`,
    /// the input to [`update_sparse`](Self::update_sparse). `hint` is the expected density of
    /// the result (fraction of nonzeros, e.g. a running average): the depth-first solve with
    /// U is used only when both the transformed right-hand side and the hint are sparse,
    /// otherwise a plain loop over the pivot order.
    pub fn ftran_sparse(&self, rhs: &mut SparseVec, x: &mut SparseVec, spike: Option<&mut SparseVec>, sc: &mut SolveScratch, hint: f64) {
        let a = rhs;
        x.reset(self.m);
        for t in 0..self.l.row.len() {
            let v = a.val[self.l.row[t] as usize];
            if v != 0.0 {
                for k in self.l.ptr[t]..self.l.ptr[t + 1] {
                    a.add(self.l.idx[k] as usize, -self.l.val[k] * v);
                }
            }
        }
        for t in 0..self.r_row.len() {
            let mut s = 0.0;
            for k in self.r_ptr[t]..self.r_ptr[t + 1] {
                s += self.r_val[k] * a.val[self.r_idx[k] as usize];
            }
            a.add(self.r_row[t] as usize, -s);
        }
        if let Some(sp) = spike {
            sp.reset(self.m);
            for &i in &a.idx {
                sp.val[i as usize] = a.val[i as usize];
                sp.idx.push(i);
            }
        }
        if a.count() * HYPER_RATIO < self.m && hint * (HYPER_RATIO as f64) < 1.0 {
            self.reach_ftran(a, sc);
            for t in (0..sc.list.len()).rev() {
                let p = sc.list[t] as usize;
                let v = a.val[self.prow[p] as usize];
                if v != 0.0 {
                    let xp = v / self.udiag[p];
                    x.add(p, xp);
                    for &(row, u) in self.ucol.slice(p) {
                        a.add(row as usize, -u * xp);
                    }
                }
            }
        } else {
            // Dense: no index bookkeeping for the work vector, which is wiped at the end.
            for &p in self.order.iter().rev() {
                if p == TOMB {
                    continue;
                }
                let p = p as usize;
                let v = a.val[self.prow[p] as usize];
                if v.abs() > TINY {
                    let xp = v / self.udiag[p];
                    x.val[p] = xp;
                    x.idx.push(p as u32);
                    for &(row, u) in self.ucol.slice(p) {
                        a.val[row as usize] -= u * xp;
                    }
                }
            }
            a.val.iter_mut().for_each(|v| *v = 0.0);
            a.idx.clear();
            return;
        }
        a.clear();
    }

    /// `K` dense solves `B x_k = rhs_k` in one pass over the factors (each right-hand side is
    /// consumed, as in [`ftran_sparse`](Self::ftran_sparse)). On bases whose solves fill in,
    /// reading the factors dominates the cost, so sharing that pass between the simplex's
    /// solves of one iteration saves most of the extra solves. `spike` receives `R L^-1 rhs_0`.
    /// `buf` is scratch space.
    pub fn ftran_multi<const K: usize>(
        &self,
        rhs: [&mut SparseVec; K],
        x: [&mut SparseVec; K],
        spike: Option<&mut SparseVec>,
        buf: &mut Vec<[f64; K]>,
    ) {
        let m = self.m;
        buf.clear();
        buf.resize(m, [0.0; K]);
        let a = buf.as_mut_slice();
        for (k, r) in rhs.into_iter().enumerate() {
            for &i in &r.idx {
                a[i as usize][k] = r.val[i as usize];
            }
            r.clear();
        }
        for t in 0..self.l.row.len() {
            let v = a[self.l.row[t] as usize];
            if v.iter().any(|&e| e != 0.0) {
                for e in self.l.ptr[t]..self.l.ptr[t + 1] {
                    let (row, lv) = (&mut a[self.l.idx[e] as usize], self.l.val[e]);
                    for k in 0..K {
                        row[k] -= lv * v[k];
                    }
                }
            }
        }
        for t in 0..self.r_row.len() {
            let mut sum = [0.0; K];
            for e in self.r_ptr[t]..self.r_ptr[t + 1] {
                let (row, rv) = (&a[self.r_idx[e] as usize], self.r_val[e]);
                for k in 0..K {
                    sum[k] += rv * row[k];
                }
            }
            let row = &mut a[self.r_row[t] as usize];
            for k in 0..K {
                row[k] -= sum[k];
            }
        }
        if let Some(sp) = spike {
            sp.reset(m);
            for (i, v) in a.iter().enumerate() {
                if v[0] != 0.0 {
                    sp.val[i] = v[0];
                    sp.idx.push(i as u32);
                }
            }
        }
        let mut x = x;
        for xk in x.iter_mut() {
            xk.reset(m);
        }
        for &p in self.order.iter().rev() {
            if p == TOMB {
                continue;
            }
            let p = p as usize;
            let v = a[self.prow[p] as usize];
            let mut xp = [0.0; K];
            let mut any = false;
            for k in 0..K {
                if v[k].abs() > TINY {
                    xp[k] = v[k] / self.udiag[p];
                    x[k].val[p] = xp[k];
                    x[k].idx.push(p as u32);
                    any = true;
                }
            }
            if any {
                for &(row, u) in self.ucol.slice(p) {
                    let row = &mut a[row as usize];
                    for k in 0..K {
                        row[k] -= u * xp[k];
                    }
                }
            }
        }
    }

    /// Hyper-sparse `B^T z = c`: `c` is indexed by basis position and is consumed (left all
    /// zero), the result `z` is indexed by row. `hint` is the expected result density, as for
    /// [`ftran_sparse`](Self::ftran_sparse); the dense path is the pull-form solve over the
    /// columns of U.
    pub fn btran_sparse(&self, c: &mut SparseVec, z: &mut SparseVec, sc: &mut SolveScratch, hint: f64) {
        z.reset(self.m);
        if c.count() * HYPER_RATIO < self.m && hint * (HYPER_RATIO as f64) < 1.0 {
            self.reach_btran(c, sc);
            for t in (0..sc.list.len()).rev() {
                let p = sc.list[t] as usize;
                self.btran_u_step(p, c, z);
            }
        } else {
            for &p in &self.order {
                if p == TOMB {
                    continue;
                }
                let p = p as usize;
                let mut s = c.val[p];
                for &(row, u) in self.ucol.slice(p) {
                    s -= u * z.val[row as usize];
                }
                if s.abs() > TINY {
                    let r = self.prow[p] as usize;
                    z.val[r] = s / self.udiag[p];
                    z.idx.push(r as u32);
                }
            }
        }
        c.clear();
        for t in (0..self.r_row.len()).rev() {
            let v = z.val[self.r_row[t] as usize];
            if v != 0.0 {
                for k in self.r_ptr[t]..self.r_ptr[t + 1] {
                    z.add(self.r_idx[k] as usize, -self.r_val[k] * v);
                }
            }
        }
        for t in (0..self.l.row.len()).rev() {
            let mut s = 0.0;
            for k in self.l.ptr[t]..self.l.ptr[t + 1] {
                s += self.l.val[k] * z.val[self.l.idx[k] as usize];
            }
            z.add(self.l.row[t] as usize, -s);
        }
    }

    /// One step of the push-form solve with U^T: position `p` is final, so its value goes to
    /// `z` and is pushed along row `prow[p]` of U.
    #[inline]
    fn btran_u_step(&self, p: usize, c: &mut SparseVec, z: &mut SparseVec) {
        let v = c.val[p];
        if v == 0.0 {
            return;
        }
        let r = self.prow[p] as usize;
        let zr = v / self.udiag[p];
        z.add(r, zr);
        for &(q, ver, u) in self.urow.slice(r) {
            if ver == self.uver[q as usize] {
                c.add(q as usize, -u * zr);
            }
        }
    }

    /// One elimination step of the Forrest-Tomlin row update: the entry of the pulled-out row
    /// at position `q` (in `w`, cleared here) is eliminated with pivot row `prow[q]` of U,
    /// recording the multiplier in `eta`.
    #[inline]
    fn eliminate_into(&self, q: usize, w: &mut [f64], eta: &mut Vec<(u32, f64)>) {
        let val = w[q];
        if val == 0.0 {
            return;
        }
        w[q] = 0.0;
        let mlt = val / self.udiag[q];
        let prq = self.prow[q] as usize;
        eta.push((prq as u32, mlt));
        for &(c, ver, u) in self.urow.slice(prq) {
            if ver == self.uver[c as usize] {
                w[c as usize] -= mlt * u;
            }
        }
    }

    /// Forrest-Tomlin update from a dense spike (see [`update_sparse`](Self::update_sparse)).
    pub fn update(&mut self, p: usize, spike: &[f64], alpha_p: f64) -> bool {
        let mut sv = SparseVec::new(self.m);
        for (i, &v) in spike.iter().enumerate() {
            sv.add(i, v);
        }
        self.update_sparse(p, &sv, alpha_p, &mut SolveScratch::default())
    }

    /// Forrest-Tomlin update: basis position `p` is replaced by the column whose spike (from
    /// [`ftran_sparse`](Self::ftran_sparse)) is `spike`; `alpha_p` is the `p`-th entry of that
    /// column's ftran result. Returns `false` if the update is numerically unsafe, in which case
    /// the factors are left inconsistent and the caller must refactorize before the next solve.
    pub fn update_sparse(&mut self, p: usize, spike: &SparseVec, alpha_p: f64, sc: &mut SolveScratch) -> bool {
        let rho = self.prow[p] as usize;

        // The old column p leaves the row lists (lazily: its entries become stale).
        self.ustale += self.ucol.len_of(p);
        self.uver[p] = self.uver[p].wrapping_add(1);

        // 1. Pull row rho out of U (it only has entries in columns pivoted after p).
        let gen = sc.next_ugen(self.m);
        let mut w = std::mem::take(&mut sc.w);
        let mut stamp = std::mem::take(&mut sc.ustamp);
        let mut heap = std::mem::take(&mut sc.heap);
        heap.clear();
        for k in 0..self.urow.len_of(rho) {
            let (q, ver, val) = self.urow.slice(rho)[k];
            let q = q as usize;
            if ver != self.uver[q] {
                continue;
            }
            if let Some(ix) = self.ucol.slice(q).iter().position(|&(r, _)| r as usize == rho) {
                self.ucol.swap_remove(q, ix);
            }
            w[q] += val;
            if stamp[q] != gen {
                stamp[q] = gen;
                heap.push(Reverse((self.order_pos[q], q as u32)));
            }
        }
        self.urow.clear(rho);

        // 2./3. Eliminate row rho with the later pivot rows, in pivot order: a heap keyed by
        // pivot position while few positions are touched, a plain scan of the rest of the
        // pivot order once the heap holds a sizable share of it (dense rows).
        let mut eta = std::mem::take(&mut sc.eta);
        eta.clear();
        let end = self.order.len();
        while let Some(Reverse((pos, q))) = heap.pop() {
            if heap.len() * 2 > end - pos as usize {
                for t in pos as usize..end {
                    let q = self.order[t];
                    if q != TOMB {
                        self.eliminate_into(q as usize, &mut w, &mut eta);
                    }
                }
                heap.clear();
                break;
            }
            let q = q as usize;
            if w[q] == 0.0 {
                continue;
            }
            let prq = self.prow[q] as usize;
            self.eliminate_into(q, &mut w, &mut eta);
            for &(c, ver, _) in self.urow.slice(prq) {
                let c = c as usize;
                if ver == self.uver[c] && stamp[c] != gen {
                    stamp[c] = gen;
                    heap.push(Reverse((self.order_pos[c], c as u32)));
                }
            }
        }
        sc.w = w;
        sc.ustamp = stamp;
        sc.heap = heap;

        // 4. New diagonal and stability check.
        let mut d = spike.val[rho];
        for &(r, mlt) in &eta {
            d -= mlt * spike.val[r as usize];
        }
        let expected = alpha_p * self.udiag[p];
        if d.abs() < PIVOT_FLOOR || (d - expected).abs() > 1e-7 * (d.abs() + expected.abs()) + 1e-12 {
            sc.eta = eta;
            return false;
        }

        // 5. New column p: spike entries (all other rows precede p in the new order).
        let ver = self.uver[p];
        let mut col = std::mem::take(&mut sc.col);
        col.clear();
        for &i in &spike.idx {
            let v = spike.val[i as usize];
            if i as usize != rho && v.abs() > 1e-14 {
                col.push((i, v));
                self.urow.push(i as usize, (p as u32, ver, v));
            }
        }
        self.ucol.set(p, &col);
        sc.col = col;
        self.udiag[p] = d;

        // 6. Move p to the end of the pivot order; record the row eta.
        self.order[self.order_pos[p] as usize] = TOMB;
        self.tombs += 1;
        self.order_pos[p] = self.order.len() as u32;
        self.order.push(p as u32);
        if self.tombs > self.m / 2 + 8 {
            self.order.retain(|&q| q != TOMB);
            self.tombs = 0;
            for (i, &q) in self.order.iter().enumerate() {
                self.order_pos[q as usize] = i as u32;
            }
        }
        if !eta.is_empty() {
            self.r_row.push(rho as u32);
            for &(r, mlt) in &eta {
                self.r_idx.push(r);
                self.r_val.push(mlt);
            }
            self.r_ptr.push(self.r_idx.len());
        }
        sc.eta = eta;
        if self.ucol.needs_compaction() {
            self.ucol.compact();
        }
        if self.ustale > self.urow.live / 2 + self.m || self.urow.needs_compaction() {
            self.purge_urow();
        }
        self.updates += 1;
        true
    }

    /// Drop the stale entries of the row lists and compact them.
    fn purge_urow(&mut self) {
        for r in 0..self.m {
            let s0 = self.urow.start[r];
            let mut l = self.urow.len[r] as usize;
            let mut k = 0;
            while k < l {
                let (q, ver, _) = self.urow.data[s0 + k];
                if ver != self.uver[q as usize] {
                    self.urow.data[s0 + k] = self.urow.data[s0 + l - 1];
                    l -= 1;
                    self.urow.live -= 1;
                } else {
                    k += 1;
                }
            }
            self.urow.len[r] = l as u32;
        }
        self.ustale = 0;
        self.urow.compact();
    }
}

