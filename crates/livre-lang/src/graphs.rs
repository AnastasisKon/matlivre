//! `graph` and `digraph`: construction, editing, queries and algorithms (paths, search,
//! components, spanning trees, flows, centrality, cycles, isomorphism) and `plot(G)`.
//!
//! A graph is a value object whose `Edges` and `Nodes` tables are the truth, as in MATLAB
//! (`G.Edges.Weight = ...` works); every operation reads them into [`Gr`] and writes a new
//! object. Edges are kept in MATLAB's canonical order: sorted by end nodes, and for undirected
//! graphs with the smaller node first.

use std::cell::RefCell;
use std::collections::{BTreeSet, BinaryHeap, VecDeque};
use std::rc::Rc;

use crate::builtins::{arg, num2str_scalar, one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::table::Table;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["graph", "digraph", "matlivre_graph_method"];

/// The graph functions, all methods of `graph` and `digraph`.
const METHODS: &[&str] = &[
    "numnodes", "numedges", "degree", "indegree", "outdegree", "neighbors", "successors", "predecessors", "shortestpath", "distances", "adjacency",
    "incidence", "laplacian", "addedge", "rmedge", "addnode", "rmnode", "findedge", "findnode", "bfsearch", "dfsearch", "conncomp", "minspantree",
    "toposort", "isdag", "hascycles", "maxflow", "centrality", "subgraph", "reordernodes", "flipedge", "edgecount", "inedges", "outedges", "ismultigraph",
    "transclosure", "transreduction", "condensation", "nearest", "shortestpathtree", "allpaths", "allcycles", "cyclebasis", "isisomorphic", "isomorphism",
    "biconncomp", "bctree", "plot",
];

/// Names counted as implemented (methods are dispatched through the class).
pub const FUNCS: &[&str] = &[
    "numnodes", "numedges", "degree", "indegree", "outdegree", "neighbors", "successors", "predecessors", "shortestpath", "distances", "adjacency",
    "incidence", "laplacian", "addedge", "rmedge", "addnode", "rmnode", "findedge", "findnode", "bfsearch", "dfsearch", "conncomp", "minspantree",
    "toposort", "isdag", "hascycles", "maxflow", "centrality", "subgraph", "reordernodes", "flipedge", "edgecount", "inedges", "outedges", "ismultigraph",
    "transclosure", "transreduction", "condensation", "nearest", "shortestpathtree", "allpaths", "allcycles", "cyclebasis", "isisomorphic", "isomorphism",
    "biconncomp", "bctree",
];

fn class_def(name: &str) -> Rc<ClassDef> {
    thread_local! {
        static DEFS: RefCell<Vec<(String, Rc<ClassDef>)>> = const { RefCell::new(Vec::new()) };
    }
    DEFS.with(|d| {
        if let Some((_, def)) = d.borrow().iter().find(|(n, _)| n == name) {
            return def.clone();
        }
        let mut src = format!("classdef {name}\n  properties\n    Edges\n    Nodes\n  end\n  methods\n");
        for m in METHODS {
            src.push_str(&format!("    function varargout = {m}(varargin)\n      varargout = matlivre_graph_method('{m}', nargout, varargin{{:}});\n    end\n"));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        d.borrow_mut().push((name.to_string(), def.clone()));
        def
    })
}

pub fn is_graph(v: &Value) -> bool {
    matches!(v, Value::Object(o) if o.class.name == "graph" || o.class.name == "digraph")
}

/// A graph read out of its tables.
#[derive(Clone)]
struct Gr {
    directed: bool,
    n: usize,
    names: Option<Vec<String>>,
    edges: Vec<(usize, usize)>,
    weights: Option<Vec<f64>>,
}

impl Gr {
    fn of(o: &Object) -> R<Gr> {
        let directed = o.class.name == "digraph";
        let Some(Value::Table(nodes)) = o.get("Nodes") else { return rt("Invalid graph.") };
        let Some(Value::Table(edges)) = o.get("Edges") else { return rt("Invalid graph.") };
        let names = match nodes.var("Name") {
            Some(v) => Some(texts(v)?),
            None => None,
        };
        let n = nodes.nrows;
        let mut list = Vec::new();
        if let Some(e) = edges.var("EndNodes") {
            match e {
                Value::Num(m) => {
                    let r = m.rows();
                    for i in 0..r {
                        list.push((m.data[i] as usize - 1, m.data[r + i] as usize - 1));
                    }
                }
                Value::Cell(c) => {
                    let nm = names.clone().unwrap_or_default();
                    let r = c.rows();
                    let idx = |v: &Value| -> R<usize> {
                        let t = v.text()?;
                        nm.iter().position(|x| *x == t).ok_or_else(|| LangError::new(format!("Unknown node '{t}'."), 0))
                    };
                    for i in 0..r {
                        list.push((idx(&c.data[i])?, idx(&c.data[r + i])?));
                    }
                }
                _ => return rt("Invalid graph edges."),
            }
        }
        let weights = edges.var("Weight").map(|w| w.to_matrix().map(|m| m.data)).transpose()?;
        Ok(Gr { directed, n, names, edges: list, weights })
    }

    fn weighted(&self) -> bool {
        self.weights.is_some()
    }

    fn w(&self, k: usize) -> f64 {
        self.weights.as_ref().map(|w| w[k]).unwrap_or(1.0)
    }

    /// Puts edges in MATLAB's order (undirected edges with the smaller node first).
    fn canonical(mut self) -> Gr {
        if !self.directed {
            for e in self.edges.iter_mut() {
                if e.0 > e.1 {
                    *e = (e.1, e.0);
                }
            }
        }
        let mut idx: Vec<usize> = (0..self.edges.len()).collect();
        idx.sort_by_key(|&k| self.edges[k]);
        self.edges = idx.iter().map(|&k| self.edges[k]).collect();
        if let Some(w) = &self.weights {
            self.weights = Some(idx.iter().map(|&k| w[k]).collect());
        }
        self
    }

    fn value(self) -> Value {
        let g = self.canonical();
        let m = g.edges.len();
        let mut names = vec!["EndNodes".to_string()];
        let end = match &g.names {
            Some(nm) => {
                let mut data: Vec<Value> = g.edges.iter().map(|e| Value::chars(&nm[e.0])).collect();
                data.extend(g.edges.iter().map(|e| Value::chars(&nm[e.1])));
                Value::Cell(CellArray::new(m, 2, data))
            }
            None => {
                let mut data: Vec<f64> = g.edges.iter().map(|e| (e.0 + 1) as f64).collect();
                data.extend(g.edges.iter().map(|e| (e.1 + 1) as f64));
                Value::Num(Matrix::new(m, 2, data))
            }
        };
        let mut vars = vec![end];
        if let Some(w) = &g.weights {
            names.push("Weight".into());
            vars.push(Value::Num(Matrix::new(m, 1, w.clone())));
        }
        let mut edges = Table::new(names, vars).unwrap_or_else(|_| Table::new(Vec::new(), Vec::new()).unwrap());
        edges.nrows = m;
        let mut nodes = match &g.names {
            Some(nm) => Table::new(vec!["Name".into()], vec![Value::Cell(CellArray::new(g.n, 1, nm.iter().map(|s| Value::chars(s)).collect()))]).unwrap(),
            None => Table::new(Vec::new(), Vec::new()).unwrap(),
        };
        nodes.nrows = g.n;
        let class = if g.directed { "digraph" } else { "graph" };
        Value::Object(Object { class: class_def(class), props: Rc::new(RefCell::new(vec![Value::Table(Box::new(edges)), Value::Table(Box::new(nodes))])) })
    }

    /// Neighbours along edges (for digraphs, successors), with the edge index, ascending.
    fn out_adj(&self) -> Vec<Vec<(usize, usize)>> {
        let mut adj = vec![Vec::new(); self.n];
        for (k, &(s, t)) in self.edges.iter().enumerate() {
            adj[s].push((t, k));
            if !self.directed && s != t {
                adj[t].push((s, k));
            }
        }
        for a in adj.iter_mut() {
            a.sort();
        }
        adj
    }

    fn in_adj(&self) -> Vec<Vec<(usize, usize)>> {
        let mut adj = vec![Vec::new(); self.n];
        for (k, &(s, t)) in self.edges.iter().enumerate() {
            adj[t].push((s, k));
            if !self.directed && s != t {
                adj[s].push((t, k));
            }
        }
        for a in adj.iter_mut() {
            a.sort();
        }
        adj
    }

    /// A node given by index or name, 0-based.
    fn node(&self, v: &Value) -> R<usize> {
        if v.is_text() {
            let t = v.text()?;
            return self.names.as_ref().and_then(|nm| nm.iter().position(|x| *x == t)).ok_or_else(|| LangError::new(format!("'{t}' is not a node name."), 0));
        }
        let k = v.scalar()?;
        if k < 1.0 || k.fract() != 0.0 || k as usize > self.n {
            return rt("Node index exceeds number of nodes.");
        }
        Ok(k as usize - 1)
    }

    fn nodes(&self, v: &Value) -> R<Vec<usize>> {
        match v {
            Value::Cell(c) => c.data.iter().map(|x| self.node(x)).collect(),
            Value::Str(s) if s.data.len() != 1 => s.data.iter().map(|x| self.node(&Value::string(x.clone().unwrap_or_default()))).collect(),
            Value::Num(m) if m.kind != Kind::Char && m.numel() != 1 => m.data.iter().map(|&x| self.node(&Value::num(x))).collect(),
            other => Ok(vec![self.node(other)?]),
        }
    }

    /// A list of nodes as MATLAB returns it: indices, or names when the nodes have them.
    fn node_list(&self, v: &[usize], column: bool) -> Value {
        let n = v.len();
        let dims = if column { vec![n, 1] } else { vec![1, n] };
        match &self.names {
            Some(nm) => Value::Cell(CellArray::from_dims(dims, v.iter().map(|&k| Value::chars(&nm[k])).collect())),
            None => Value::Num(Matrix::from_dims(dims, v.iter().map(|&k| (k + 1) as f64).collect())),
        }
    }

    /// Shortest distances from `s` (Dijkstra; Bellman-Ford with negative weights), with the
    /// predecessor of each node.
    fn sssp(&self, s: usize, use_weights: bool) -> (Vec<f64>, Vec<Option<usize>>) {
        let adj = self.out_adj();
        let mut dist = vec![f64::INFINITY; self.n];
        let mut pred = vec![None; self.n];
        dist[s] = 0.0;
        let wt = |k: usize| if use_weights { self.w(k) } else { 1.0 };
        if use_weights && self.weights.as_ref().is_some_and(|w| w.iter().any(|&x| x < 0.0)) {
            for _ in 0..self.n {
                let mut changed = false;
                for u in 0..self.n {
                    if !dist[u].is_finite() {
                        continue;
                    }
                    for &(v, k) in &adj[u] {
                        if dist[u] + wt(k) < dist[v] {
                            dist[v] = dist[u] + wt(k);
                            pred[v] = Some(u);
                            changed = true;
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
            return (dist, pred);
        }
        #[derive(PartialEq)]
        struct Item(f64, usize);
        impl Eq for Item {}
        impl PartialOrd for Item {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Item {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.partial_cmp(&self.0).unwrap_or(std::cmp::Ordering::Equal).then(o.1.cmp(&self.1))
            }
        }
        let mut heap = BinaryHeap::new();
        heap.push(Item(0.0, s));
        let mut done = vec![false; self.n];
        while let Some(Item(d, u)) = heap.pop() {
            if done[u] {
                continue;
            }
            done[u] = true;
            for &(v, k) in &adj[u] {
                let nd = d + wt(k);
                if nd < dist[v] {
                    dist[v] = nd;
                    pred[v] = Some(u);
                    heap.push(Item(nd, v));
                }
            }
        }
        (dist, pred)
    }

    /// Breadth-first order from `s`, with the tree edges discovering new nodes.
    fn bfs(&self, s: usize) -> (Vec<usize>, Vec<(usize, usize)>) {
        let adj = self.out_adj();
        let mut seen = vec![false; self.n];
        let mut order = vec![s];
        let mut tree = Vec::new();
        seen[s] = true;
        let mut q = VecDeque::from([s]);
        while let Some(u) = q.pop_front() {
            for &(v, _) in &adj[u] {
                if !seen[v] {
                    seen[v] = true;
                    order.push(v);
                    tree.push((u, v));
                    q.push_back(v);
                }
            }
        }
        (order, tree)
    }

    /// Depth-first discovery order from `s`, the tree edges, and the finishing order.
    fn dfs(&self, s: usize) -> (Vec<usize>, Vec<(usize, usize)>, Vec<usize>) {
        let adj = self.out_adj();
        let mut seen = vec![false; self.n];
        let (mut order, mut tree, mut finish) = (Vec::new(), Vec::new(), Vec::new());
        fn go(u: usize, adj: &[Vec<(usize, usize)>], seen: &mut [bool], order: &mut Vec<usize>, tree: &mut Vec<(usize, usize)>, finish: &mut Vec<usize>) {
            seen[u] = true;
            order.push(u);
            for &(v, _) in &adj[u] {
                if !seen[v] {
                    tree.push((u, v));
                    go(v, adj, seen, order, tree, finish);
                }
            }
            finish.push(u);
        }
        go(s, &adj, &mut seen, &mut order, &mut tree, &mut finish);
        (order, tree, finish)
    }

    /// Strongly connected components numbered in topological order of the condensation
    /// (weak components for undirected graphs, numbered by first node).
    fn components(&self, strong: bool) -> Vec<usize> {
        if !self.directed || !strong {
            let und = Gr { directed: false, ..self.clone() };
            let adj = und.out_adj();
            let mut comp = vec![usize::MAX; self.n];
            let mut c = 0;
            for s in 0..self.n {
                if comp[s] != usize::MAX {
                    continue;
                }
                let mut stack = vec![s];
                comp[s] = c;
                while let Some(u) = stack.pop() {
                    for &(v, _) in &adj[u] {
                        if comp[v] == usize::MAX {
                            comp[v] = c;
                            stack.push(v);
                        }
                    }
                }
                c += 1;
            }
            return comp;
        }
        // Tarjan: components are found in reverse topological order.
        let adj = self.out_adj();
        let n = self.n;
        let (mut index, mut low, mut on) = (vec![usize::MAX; n], vec![0; n], vec![false; n]);
        let mut stack = Vec::new();
        let mut found: Vec<Vec<usize>> = Vec::new();
        let mut counter = 0;
        fn tarjan(
            u: usize, adj: &[Vec<(usize, usize)>], index: &mut [usize], low: &mut [usize], on: &mut [bool], stack: &mut Vec<usize>, found: &mut Vec<Vec<usize>>,
            counter: &mut usize,
        ) {
            index[u] = *counter;
            low[u] = *counter;
            *counter += 1;
            stack.push(u);
            on[u] = true;
            for &(v, _) in &adj[u] {
                if index[v] == usize::MAX {
                    tarjan(v, adj, index, low, on, stack, found, counter);
                    low[u] = low[u].min(low[v]);
                } else if on[v] {
                    low[u] = low[u].min(index[v]);
                }
            }
            if low[u] == index[u] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp.push(w);
                    if w == u {
                        break;
                    }
                }
                found.push(comp);
            }
        }
        for s in 0..n {
            if index[s] == usize::MAX {
                tarjan(s, &adj, &mut index, &mut low, &mut on, &mut stack, &mut found, &mut counter);
            }
        }
        let mut comp = vec![0; n];
        let k = found.len();
        for (i, c) in found.iter().enumerate() {
            for &v in c {
                comp[v] = k - 1 - i;
            }
        }
        comp
    }

    fn has_cycle(&self) -> bool {
        if self.edges.iter().any(|e| e.0 == e.1) {
            return true;
        }
        if !self.directed {
            let comps = self.components(false).into_iter().collect::<BTreeSet<_>>().len();
            return self.edges.len() + comps > self.n;
        }
        let comp = self.components(true);
        let k = comp.iter().collect::<BTreeSet<_>>().len();
        k < self.n
    }

    fn adjacency(&self, weighted: bool) -> Matrix {
        let n = self.n;
        let mut a = vec![0.0; n * n];
        for (k, &(s, t)) in self.edges.iter().enumerate() {
            let w = if weighted { self.w(k) } else { 1.0 };
            a[t * n + s] += w;
            if !self.directed && s != t {
                a[s * n + t] += w;
            }
        }
        Matrix::new(n, n, a)
    }
}

fn texts(v: &Value) -> R<Vec<String>> {
    match v {
        Value::Cell(c) => c.data.iter().map(|x| x.text()).collect(),
        Value::Str(s) => Ok(s.data.iter().map(|x| x.clone().unwrap_or_default()).collect()),
        other => Ok(vec![other.text()?]),
    }
}

fn col(v: Vec<f64>) -> Value {
    Value::Num(Matrix::col(v))
}

fn row(v: Vec<f64>) -> Value {
    Value::Num(Matrix::row(v))
}

fn lg(b: bool) -> Value {
    Value::bool(b)
}

/// End nodes given as indices or names, numbering new names after the existing ones.
fn end_nodes(v: &Value, names: &mut Option<Vec<String>>, n: &mut usize) -> R<Vec<usize>> {
    if v.is_text() || matches!(v, Value::Cell(_) | Value::Str(_)) {
        let list = texts(v)?;
        let nm = names.get_or_insert_with(Vec::new);
        let mut out = Vec::with_capacity(list.len());
        for t in list {
            let k = match nm.iter().position(|x| *x == t) {
                Some(k) => k,
                None => {
                    nm.push(t);
                    nm.len() - 1
                }
            };
            out.push(k);
        }
        *n = (*n).max(nm.len());
        return Ok(out);
    }
    let m = v.to_matrix()?;
    let mut out = Vec::with_capacity(m.numel());
    for &x in &m.data {
        if x < 1.0 || x.fract() != 0.0 {
            return rt("Node indices must be positive integers.");
        }
        let k = x as usize - 1;
        *n = (*n).max(k + 1);
        out.push(k);
    }
    Ok(out)
}

fn construct(directed: bool, a: &[Value]) -> Ret {
    let a: Vec<&Value> = a.iter().filter(|v| !v.text().map(|t| t.eq_ignore_ascii_case("omitselfloops")).unwrap_or(false)).collect();
    let mut g = Gr { directed, n: 0, names: None, edges: Vec::new(), weights: None };
    match a.as_slice() {
        [] => {}
        // graph(A) or graph(A, names): an adjacency matrix.
        [m] | [m, _] if matches!(m, Value::Num(x) if x.kind != Kind::Char && x.rows() == x.cols() && x.rows() > 0 || x.numel() == 0) && (a.len() == 1 || !matches!(a[1], Value::Num(_))) => {
            let m = m.to_matrix()?;
            let n = m.rows();
            if !directed && (0..n).any(|i| (0..n).any(|j| m.data[j * n + i] != m.data[i * n + j])) {
                return rt("Adjacency matrix must be symmetric.");
            }
            g.n = n;
            let weighted = m.kind != Kind::Logical;
            let mut w = Vec::new();
            for j in 0..n {
                for i in 0..n {
                    let x = m.data[j * n + i];
                    if x != 0.0 && (directed || i <= j) {
                        g.edges.push((i, j));
                        w.push(x);
                    }
                }
            }
            if weighted {
                g.weights = Some(w);
            }
            if let Some(nm) = a.get(1) {
                g.names = Some(texts(nm)?);
            }
        }
        [s, t, rest @ ..] => {
            let mut names = None;
            let mut n = 0;
            // graph(s, t, w, nodes): the node count or names come last.
            let nodes_arg = rest.iter().find(|v| match v {
                Value::Cell(_) | Value::Str(_) => true,
                Value::Num(m) => m.kind == Kind::Char,
                _ => false,
            });
            if let Some(nm) = nodes_arg {
                names = Some(texts(nm)?);
                n = names.as_ref().unwrap().len();
            }
            let ss = end_nodes(s, &mut names, &mut n)?;
            let tt = end_nodes(t, &mut names, &mut n)?;
            let (ss, tt) = match (ss.len(), tt.len()) {
                (a, b) if a == b => (ss, tt),
                (1, b) => (vec![ss[0]; b], tt),
                (a, 1) => (ss, vec![tt[0]; a]),
                _ => return rt("Source and target node IDs must have the same number of elements."),
            };
            let numeric: Vec<&Value> = rest.iter().copied().filter(|v| matches!(v, Value::Num(m) if m.kind != Kind::Char)).collect();
            if let Some(w) = numeric.first() {
                let w = w.to_matrix()?;
                if w.numel() == ss.len() {
                    g.weights = Some(w.data.clone());
                } else if w.numel() == 1 && ss.len() != 1 {
                    g.weights = Some(vec![w.data[0]; ss.len()]);
                } else if w.numel() == 1 && numeric.len() == 1 && ss.len() == 1 {
                    g.weights = Some(w.data.clone());
                }
            }
            if let Some(cnt) = numeric.get(1) {
                n = n.max(cnt.as_usize()?);
            }
            g.n = n;
            g.names = names;
            g.edges = ss.into_iter().zip(tt).collect();
        }
        _ => return rt("Invalid graph construction."),
    }
    one(g.value())
}

/// Brute-force-with-pruning isomorphism: the lexicographically first mapping P (node i of
/// `a` to node P(i) of `b`).
fn isomorphism(a: &Gr, b: &Gr) -> Option<Vec<usize>> {
    if a.n != b.n || a.edges.len() != b.edges.len() || a.directed != b.directed {
        return None;
    }
    let (ma, mb) = (a.adjacency(false), b.adjacency(false));
    let n = a.n;
    let deg = |m: &Matrix, i: usize| -> (f64, f64) { ((0..n).map(|j| m.data[j * n + i]).sum(), (0..n).map(|j| m.data[i * n + j]).sum()) };
    let (da, db): (Vec<_>, Vec<_>) = ((0..n).map(|i| deg(&ma, i)).collect(), (0..n).map(|i| deg(&mb, i)).collect());
    let mut map = vec![usize::MAX; n];
    let mut used = vec![false; n];
    fn go(i: usize, n: usize, ma: &Matrix, mb: &Matrix, da: &[(f64, f64)], db: &[(f64, f64)], map: &mut [usize], used: &mut [bool]) -> bool {
        if i == n {
            return true;
        }
        for c in 0..n {
            if used[c] || da[i] != db[c] {
                continue;
            }
            let ok = (0..i).all(|j| ma.data[j * n + i] == mb.data[map[j] * n + c] && ma.data[i * n + j] == mb.data[c * n + map[j]]) && ma.data[i * n + i] == mb.data[c * n + c];
            if ok {
                map[i] = c;
                used[c] = true;
                if go(i + 1, n, ma, mb, da, db, map, used) {
                    return true;
                }
                used[c] = false;
            }
        }
        false
    }
    go(0, n, &ma, &mb, &da, &db, &mut map, &mut used).then_some(map)
}

/// Simple cycles as node lists starting at their smallest node (undirected: the direction
/// whose second node is smaller), sorted.
fn all_cycles(g: &Gr) -> Vec<Vec<usize>> {
    let adj = g.out_adj();
    let mut found: BTreeSet<Vec<usize>> = BTreeSet::new();
    for s in 0..g.n {
        // Cycles whose smallest node is s.
        let mut path = vec![s];
        let mut on = vec![false; g.n];
        on[s] = true;
        fn go(u: usize, s: usize, adj: &[Vec<(usize, usize)>], path: &mut Vec<usize>, on: &mut [bool], found: &mut BTreeSet<Vec<usize>>, directed: bool) {
            for &(v, _) in &adj[u] {
                if v == s && (directed || path.len() >= 3) {
                    let mut c = path.clone();
                    if !directed && c.len() > 2 && c[1] > c[c.len() - 1] {
                        c[1..].reverse();
                    }
                    found.insert(c);
                } else if v > s && !on[v] {
                    on[v] = true;
                    path.push(v);
                    go(v, s, adj, path, on, found, directed);
                    path.pop();
                    on[v] = false;
                }
            }
        }
        go(s, s, &adj, &mut path, &mut on, &mut found, g.directed);
    }
    found.into_iter().collect()
}

fn cells_of_rows(rows: Vec<Vec<usize>>, g: &Gr) -> Value {
    let n = rows.len();
    Value::Cell(CellArray::from_dims(vec![n, 1], rows.iter().map(|r| g.node_list(r, false)).collect()))
}

/// Index of the edge from `u` to `v` (either way when undirected), the lightest of parallel ones.
fn edge_between(g: &Gr, u: usize, v: usize) -> Option<usize> {
    (0..g.edges.len())
        .filter(|&k| g.edges[k] == (u, v) || (!g.directed && g.edges[k] == (v, u)))
        .min_by(|&a, &b| g.w(a).partial_cmp(&g.w(b)).unwrap_or(std::cmp::Ordering::Equal))
}

/// The edge indices (1-based row) along a node path; `closed` adds the edge back to the start.
fn edge_path(g: &Gr, path: &[usize], closed: bool) -> Value {
    let mut ks: Vec<f64> = path.windows(2).filter_map(|w| edge_between(g, w[0], w[1])).map(|k| (k + 1) as f64).collect();
    if closed && path.len() > 1 {
        if let Some(k) = edge_between(g, path[path.len() - 1], path[0]) {
            ks.push((k + 1) as f64);
        }
    }
    let n = ks.len();
    Value::Num(Matrix::new(1, n, ks))
}

fn edge_cells(g: &Gr, rows: &[Vec<usize>], closed: bool) -> Value {
    Value::Cell(CellArray::from_dims(vec![rows.len(), 1], rows.iter().map(|r| edge_path(g, r, closed)).collect()))
}

/// The edge order of `h.value()`: index into `h.edges` of each edge after `canonical`.
fn canonical_order(h: &Gr) -> Vec<usize> {
    let ends: Vec<(usize, usize)> = h.edges.iter().map(|&(s, t)| if !h.directed && s > t { (t, s) } else { (s, t) }).collect();
    let mut idx: Vec<usize> = (0..ends.len()).collect();
    idx.sort_by_key(|&k| ends[k]);
    idx
}

fn edges_matrix(e: &[(usize, usize)]) -> Value {
    let m = e.len();
    let mut data: Vec<f64> = e.iter().map(|x| (x.0 + 1) as f64).collect();
    data.extend(e.iter().map(|x| (x.1 + 1) as f64));
    Value::Num(Matrix::new(m, 2, data))
}

fn method(it: &mut Interpreter, m: &str, nargout: usize, a: &[Value]) -> Ret {
    let Some(Value::Object(o)) = a.first() else { return rt(format!("Undefined function '{m}'.")) };
    let g = Gr::of(o)?;
    let rest = &a[1..];
    let n = g.n;
    let need = |i: usize| arg(rest, i, m);
    Ok(Some(match m {
        "numnodes" => vec![Value::num(n as f64)],
        "numedges" => vec![Value::num(g.edges.len() as f64)],
        "degree" | "indegree" | "outdegree" => {
            let mut d = vec![0.0; n];
            for &(s, t) in &g.edges {
                if m != "indegree" || !g.directed {
                    d[s] += 1.0;
                }
                if m != "outdegree" || !g.directed {
                    d[t] += 1.0;
                }
            }
            let nodes = match rest.first() {
                Some(v) => g.nodes(v)?,
                None => (0..n).collect(),
            };
            vec![col(nodes.iter().map(|&k| d[k]).collect())]
        }
        "neighbors" | "successors" | "predecessors" => {
            let u = g.node(need(0)?)?;
            let adj = if m == "predecessors" { g.in_adj() } else if m == "successors" || !g.directed { g.out_adj() } else {
                // neighbors of a digraph: both directions.
                let (o, i) = (g.out_adj(), g.in_adj());
                let mut both: Vec<Vec<(usize, usize)>> = o;
                for (k, list) in i.into_iter().enumerate() {
                    both[k].extend(list);
                }
                both
            };
            let set: BTreeSet<usize> = adj[u].iter().map(|x| x.0).collect();
            vec![g.node_list(&set.into_iter().collect::<Vec<_>>(), true)]
        }
        "shortestpath" => {
            let (s, t) = (g.node(need(0)?)?, g.node(need(1)?)?);
            let unweighted = rest.iter().any(|v| v.text().map(|x| x.eq_ignore_ascii_case("unweighted")).unwrap_or(false));
            let (dist, pred) = g.sssp(s, g.weighted() && !unweighted);
            if !dist[t].is_finite() {
                vec![Value::Num(Matrix::new(1, 0, Vec::new())), Value::num(f64::INFINITY), Value::Num(Matrix::new(1, 0, Vec::new()))]
            } else {
                let mut path = vec![t];
                while let Some(p) = pred[*path.last().unwrap()] {
                    path.push(p);
                }
                path.reverse();
                let ep = edge_path(&g, &path, false);
                vec![g.node_list(&path, false), Value::num(dist[t]), ep]
            }
        }
        "distances" => {
            // A trailing 'Method', 'unweighted' ignores the weights.
            let mpos = rest.iter().position(|v| v.text().is_ok_and(|t| t.eq_ignore_ascii_case("Method")));
            let unweighted = mpos.and_then(|k| rest.get(k + 1)).is_some_and(|v| v.text().is_ok_and(|t| t.eq_ignore_ascii_case("unweighted")));
            let rest = &rest[..mpos.unwrap_or(rest.len())];
            let use_w = g.weighted() && !unweighted;
            let src = match rest.first().filter(|v| !v.is_text() || g.names.is_some()) {
                Some(v) if !v.text().map(|x| x.eq_ignore_ascii_case("all")).unwrap_or(false) => g.nodes(v)?,
                _ => (0..n).collect(),
            };
            let dst = match rest.get(1) {
                Some(v) => g.nodes(v)?,
                None => (0..n).collect(),
            };
            let (r, c) = (src.len(), dst.len());
            let mut data = vec![0.0; r * c];
            for (i, &s) in src.iter().enumerate() {
                let (d, _) = g.sssp(s, use_w);
                for (j, &t) in dst.iter().enumerate() {
                    data[j * r + i] = d[t];
                }
            }
            vec![Value::Num(Matrix::new(r, c, data))]
        }
        "adjacency" => {
            let weighted = rest.first().map(|v| v.text().map(|t| t.eq_ignore_ascii_case("weighted")).unwrap_or(false)).unwrap_or(false);
            vec![Value::Num(g.adjacency(weighted))]
        }
        "incidence" => {
            let ne = g.edges.len();
            let mut data = vec![0.0; n * ne];
            for (k, &(s, t)) in g.edges.iter().enumerate() {
                if s != t {
                    data[k * n + s] = -1.0;
                    data[k * n + t] = 1.0;
                }
            }
            vec![Value::Num(Matrix::new(n, ne, data))]
        }
        "laplacian" => {
            let a = g.adjacency(false);
            let mut l = vec![0.0; n * n];
            for i in 0..n {
                let mut d = 0.0;
                for j in 0..n {
                    if i != j {
                        l[j * n + i] = -a.data[j * n + i];
                        d += a.data[j * n + i];
                    }
                }
                l[i * n + i] = d;
            }
            vec![Value::Num(Matrix::new(n, n, l))]
        }
        "addedge" => {
            let mut h = g.clone();
            let mut names = h.names.take();
            let mut nn = h.n;
            let ss = end_nodes(need(0)?, &mut names, &mut nn)?;
            let tt = end_nodes(need(1)?, &mut names, &mut nn)?;
            let w = rest.get(2).map(|v| v.to_matrix()).transpose()?;
            for (k, (&s, &t)) in ss.iter().zip(tt.iter().cycle()).enumerate() {
                h.edges.push((s, t));
                if let Some(w) = &w {
                    let ws = h.weights.get_or_insert_with(|| vec![1.0; h.edges.len() - 1]);
                    ws.push(w.data[k.min(w.numel() - 1)]);
                } else if let Some(ws) = &mut h.weights {
                    ws.push(1.0);
                }
            }
            h.names = names;
            h.n = nn;
            vec![h.value()]
        }
        "rmedge" => {
            let mut h = g.clone();
            let remove: Vec<usize> = if rest.len() >= 2 {
                let (ss, tt) = (g.nodes(&rest[0])?, g.nodes(&rest[1])?);
                h.edges.iter().enumerate().filter(|(_, e)| ss.iter().zip(&tt).any(|(&s, &t)| (e.0 == s && e.1 == t) || (!g.directed && e.0 == t && e.1 == s))).map(|(k, _)| k).collect()
            } else {
                need(0)?.to_matrix()?.data.iter().map(|&x| x as usize - 1).collect()
            };
            let keep: Vec<usize> = (0..h.edges.len()).filter(|k| !remove.contains(k)).collect();
            h.edges = keep.iter().map(|&k| g.edges[k]).collect();
            h.weights = g.weights.as_ref().map(|w| keep.iter().map(|&k| w[k]).collect());
            vec![h.value()]
        }
        "addnode" => {
            let mut h = g.clone();
            let v = need(0)?;
            if v.is_text() || matches!(v, Value::Cell(_) | Value::Str(_)) {
                let new = texts(v)?;
                let nm = h.names.get_or_insert_with(|| (1..=n).map(|k| format!("Node{k}")).collect());
                nm.extend(new);
                h.n = nm.len();
            } else {
                h.n += v.as_usize()?;
            }
            vec![h.value()]
        }
        "rmnode" => {
            let gone = g.nodes(need(0)?)?;
            let map: Vec<Option<usize>> = {
                let mut next = 0;
                (0..n).map(|k| if gone.contains(&k) { None } else { next += 1; Some(next - 1) }).collect()
            };
            let mut h = g.clone();
            let keep: Vec<usize> = (0..g.edges.len()).filter(|&k| map[g.edges[k].0].is_some() && map[g.edges[k].1].is_some()).collect();
            h.edges = keep.iter().map(|&k| (map[g.edges[k].0].unwrap(), map[g.edges[k].1].unwrap())).collect();
            h.weights = g.weights.as_ref().map(|w| keep.iter().map(|&k| w[k]).collect());
            h.names = g.names.as_ref().map(|nm| nm.iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, s)| s.clone()).collect());
            h.n = n - gone.iter().collect::<BTreeSet<_>>().len();
            vec![h.value()]
        }
        "findedge" => {
            if rest.is_empty() || rest.len() == 1 {
                let idx: Vec<usize> = match rest.first() {
                    Some(v) => v.to_matrix()?.data.iter().map(|&x| x as usize - 1).collect(),
                    None => (0..g.edges.len()).collect(),
                };
                let s: Vec<f64> = idx.iter().map(|&k| (g.edges[k].0 + 1) as f64).collect();
                let t: Vec<f64> = idx.iter().map(|&k| (g.edges[k].1 + 1) as f64).collect();
                vec![col(s), col(t)]
            } else {
                let (ss, tt) = (g.nodes(&rest[0])?, g.nodes(&rest[1])?);
                let idx: Vec<f64> = ss
                    .iter()
                    .zip(&tt)
                    .map(|(&s, &t)| g.edges.iter().position(|e| (e.0 == s && e.1 == t) || (!g.directed && e.0 == t && e.1 == s)).map(|k| (k + 1) as f64).unwrap_or(0.0))
                    .collect();
                vec![if idx.len() == 1 { Value::num(idx[0]) } else { col(idx) }]
            }
        }
        "findnode" => {
            let v = need(0)?;
            let list: Vec<f64> = match v {
                Value::Num(x) if x.kind != Kind::Char => x.data.iter().map(|&k| if k >= 1.0 && k as usize <= n { k } else { 0.0 }).collect(),
                other => texts(other)?.iter().map(|t| g.names.as_ref().and_then(|nm| nm.iter().position(|x| x == t)).map(|k| (k + 1) as f64).unwrap_or(0.0)).collect(),
            };
            vec![col(list)]
        }
        "bfsearch" | "dfsearch" => {
            let s = g.node(need(0)?)?;
            let event = rest.get(1).map(|v| v.text()).transpose()?.unwrap_or_else(|| "discovernode".into()).to_lowercase();
            let (order, tree, finish) = if m == "bfsearch" {
                let (o, t) = g.bfs(s);
                (o, t, Vec::new())
            } else {
                g.dfs(s)
            };
            vec![match event.as_str() {
                "edgetonew" => edges_matrix(&tree),
                "finishnode" if m == "dfsearch" => g.node_list(&finish, true),
                _ => g.node_list(&order, true),
            }]
        }
        "conncomp" => {
            let weak = rest.windows(2).any(|p| p[0].text().map(|t| t.eq_ignore_ascii_case("Type")).unwrap_or(false) && p[1].text().map(|t| t.eq_ignore_ascii_case("weak")).unwrap_or(false));
            let comp = g.components(!weak);
            let k = comp.iter().max().map(|m| m + 1).unwrap_or(0);
            let mut sizes = vec![0.0; k];
            for &c in &comp {
                sizes[c] += 1.0;
            }
            vec![row(comp.iter().map(|&c| (c + 1) as f64).collect()), row(sizes)]
        }
        "minspantree" => {
            // Prim from node 1 (each component in turn for a forest).
            let adj = g.out_adj();
            let mut inside = vec![false; n];
            let mut chosen: Vec<usize> = Vec::new();
            let forest = rest.windows(2).any(|p| p[1].text().map(|t| t.eq_ignore_ascii_case("forest")).unwrap_or(false));
            let roots: Vec<usize> = match rest.windows(2).find(|p| p[0].text().map(|t| t.eq_ignore_ascii_case("Root")).unwrap_or(false)) {
                Some(p) => vec![g.node(&p[1])?],
                None => vec![0],
            };
            let starts: Vec<usize> = if forest { roots.iter().copied().chain(0..n).collect() } else { roots };
            for s in starts.clone() {
                if s >= n || inside[s] {
                    continue;
                }
                inside[s] = true;
                loop {
                    let mut best: Option<(f64, usize, usize)> = None;
                    for u in (0..n).filter(|&u| inside[u]) {
                        for &(v, k) in &adj[u] {
                            if !inside[v] && best.map(|b| g.w(k) < b.0).unwrap_or(true) {
                                best = Some((g.w(k), v, k));
                            }
                        }
                    }
                    let Some((_, v, k)) = best else { break };
                    inside[v] = true;
                    chosen.push(k);
                }
            }
            chosen.sort();
            let mut h = g.clone();
            h.edges = chosen.iter().map(|&k| g.edges[k]).collect();
            h.weights = g.weights.as_ref().map(|w| chosen.iter().map(|&k| w[k]).collect());
            // Predecessor of each node in the tree (0 at roots, NaN off the tree).
            let mut pred = vec![f64::NAN; n];
            let tadj = h.out_adj();
            let mut seen = vec![false; n];
            for &s in &starts {
                if s >= n || seen[s] || !inside[s] {
                    continue;
                }
                seen[s] = true;
                pred[s] = 0.0;
                let mut q = VecDeque::from([s]);
                while let Some(u) = q.pop_front() {
                    for &(v, _) in &tadj[u] {
                        if !seen[v] {
                            seen[v] = true;
                            pred[v] = (u + 1) as f64;
                            q.push_back(v);
                        }
                    }
                }
            }
            vec![h.value(), Value::Num(Matrix::row(pred))]
        }
        "toposort" => {
            if !g.directed {
                return rt("Graph must be a directed acyclic graph.");
            }
            if g.has_cycle() {
                return rt("Unable to perform a topological sort because the graph is not acyclic.");
            }
            let adj = g.out_adj();
            let mut seen = vec![false; n];
            let mut post = Vec::new();
            fn go(u: usize, adj: &[Vec<(usize, usize)>], seen: &mut [bool], post: &mut Vec<usize>) {
                seen[u] = true;
                for &(v, _) in &adj[u] {
                    if !seen[v] {
                        go(v, adj, seen, post);
                    }
                }
                post.push(u);
            }
            for s in 0..n {
                if !seen[s] {
                    go(s, &adj, &mut seen, &mut post);
                }
            }
            post.reverse();
            let mut out = vec![g.node_list(&post, false)];
            if nargout > 1 {
                // The graph with its nodes in that order.
                let mut inv = vec![0; n];
                for (newk, &old) in post.iter().enumerate() {
                    inv[old] = newk;
                }
                let h = Gr {
                    edges: g.edges.iter().map(|&(s, t)| (inv[s], inv[t])).collect(),
                    names: g.names.as_ref().map(|nm| post.iter().map(|&k| nm[k].clone()).collect()),
                    ..g.clone()
                };
                out.push(h.value());
            }
            out
        }
        "isdag" => vec![lg(g.directed && !g.has_cycle())],
        "hascycles" => vec![lg(g.has_cycle())],
        "maxflow" => {
            let (s, t) = (g.node(need(0)?)?, g.node(need(1)?)?);
            // Edmonds-Karp on the edge list (undirected edges usable both ways).
            let ne = g.edges.len();
            let cap: Vec<f64> = (0..ne).map(|k| g.w(k)).collect();
            let mut flow = vec![0.0; ne];
            let mut total = 0.0;
            loop {
                let mut prev: Vec<Option<(usize, usize, bool)>> = vec![None; n];
                let mut q = VecDeque::from([s]);
                let mut seen = vec![false; n];
                seen[s] = true;
                while let Some(u) = q.pop_front() {
                    for (k, &(a, b)) in g.edges.iter().enumerate() {
                        let steps: [(usize, usize, bool, f64); 2] = [(a, b, true, cap[k] - flow[k]), (b, a, false, if g.directed { flow[k] } else { cap[k] + flow[k] })];
                        for (from, to, fwd, room) in steps {
                            if from == u && !seen[to] && room > 1e-12 {
                                seen[to] = true;
                                prev[to] = Some((u, k, fwd));
                                q.push_back(to);
                            }
                        }
                    }
                }
                if !seen[t] {
                    break;
                }
                let mut push = f64::INFINITY;
                let mut v = t;
                while let Some((u, k, fwd)) = prev[v] {
                    let room = if fwd { cap[k] - flow[k] } else if g.directed { flow[k] } else { cap[k] + flow[k] };
                    push = push.min(room);
                    v = u;
                }
                let mut v = t;
                while let Some((u, k, fwd)) = prev[v] {
                    if fwd { flow[k] += push } else { flow[k] -= push }
                    v = u;
                }
                total += push;
            }
            let mut out = vec![Value::num(total)];
            if nargout > 1 {
                let used: Vec<usize> = (0..ne).filter(|&k| flow[k].abs() > 1e-12).collect();
                let gf = Gr {
                    directed: true,
                    n,
                    names: g.names.clone(),
                    edges: used.iter().map(|&k| if flow[k] >= 0.0 { g.edges[k] } else { (g.edges[k].1, g.edges[k].0) }).collect(),
                    weights: Some(used.iter().map(|&k| flow[k].abs()).collect()),
                };
                out.push(gf.value());
            }
            if nargout > 2 {
                // The minimum cut: nodes reachable from s in the residual graph, and the rest.
                let mut seen = vec![false; n];
                seen[s] = true;
                let mut q = VecDeque::from([s]);
                while let Some(u) = q.pop_front() {
                    for (k, &(a, b)) in g.edges.iter().enumerate() {
                        let steps = [(a, b, cap[k] - flow[k]), (b, a, if g.directed { flow[k] } else { cap[k] + flow[k] })];
                        for (from, to, room) in steps {
                            if from == u && !seen[to] && room > 1e-12 {
                                seen[to] = true;
                                q.push_back(to);
                            }
                        }
                    }
                }
                let cs: Vec<usize> = (0..n).filter(|&k| seen[k]).collect();
                let ct: Vec<usize> = (0..n).filter(|&k| !seen[k]).collect();
                out.push(g.node_list(&cs, true));
                out.push(g.node_list(&ct, true));
            }
            out
        }
        "centrality" => {
            let kind = need(0)?.text()?.to_lowercase();
            let c: Vec<f64> = match kind.as_str() {
                "degree" | "indegree" | "outdegree" => {
                    let which = if kind == "degree" { "degree" } else { kind.as_str() };
                    return method(it, which, 1, &[a[0].clone()]);
                }
                "closeness" => (0..n)
                    .map(|s| {
                        let (d, _) = g.sssp(s, g.weighted());
                        let reach: Vec<f64> = d.iter().enumerate().filter(|(k, x)| *k != s && x.is_finite()).map(|(_, &x)| x).collect();
                        let total: f64 = reach.iter().sum();
                        // (reachable share)^2 / total distance: 1/total when connected.
                        if total > 0.0 { (reach.len() as f64 / (n - 1).max(1) as f64).powi(2) / total } else { 0.0 }
                    })
                    .collect(),
                "betweenness" => {
                    // Brandes on unit or weighted edges.
                    let adj = g.out_adj();
                    let mut cb = vec![0.0; n];
                    for s in 0..n {
                        let (dist, _) = g.sssp(s, g.weighted());
                        let mut sigma = vec![0.0; n];
                        sigma[s] = 1.0;
                        let mut order: Vec<usize> = (0..n).filter(|&v| dist[v].is_finite()).collect();
                        order.sort_by(|&p, &q| dist[p].partial_cmp(&dist[q]).unwrap());
                        let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
                        for &u in &order {
                            for &(v, k) in &adj[u] {
                                let w = if g.weighted() { g.w(k) } else { 1.0 };
                                if (dist[u] + w - dist[v]).abs() < 1e-9 * (1.0 + dist[v].abs()) && v != s {
                                    sigma[v] += sigma[u];
                                    preds[v].push(u);
                                }
                            }
                        }
                        let mut delta = vec![0.0; n];
                        for &w in order.iter().rev() {
                            for &v in &preds[w] {
                                delta[v] += sigma[v] / sigma[w] * (1.0 + delta[w]);
                            }
                            if w != s {
                                cb[w] += delta[w];
                            }
                        }
                    }
                    if !g.directed {
                        for x in cb.iter_mut() {
                            *x /= 2.0;
                        }
                    }
                    cb
                }
                "pagerank" => {
                    let d = 0.85;
                    let adj = g.out_adj();
                    let outdeg: Vec<f64> = adj.iter().map(|l| l.len() as f64).collect();
                    let mut p = vec![1.0 / n as f64; n];
                    for _ in 0..1000 {
                        let dangling: f64 = (0..n).filter(|&u| outdeg[u] == 0.0).map(|u| p[u]).sum();
                        let mut next = vec![(1.0 - d) / n as f64 + d * dangling / n as f64; n];
                        for u in 0..n {
                            for &(v, _) in &adj[u] {
                                next[v] += d * p[u] / outdeg[u];
                            }
                        }
                        let change: f64 = next.iter().zip(&p).map(|(a, b)| (a - b).abs()).sum();
                        p = next;
                        if change < 1e-12 {
                            break;
                        }
                    }
                    p
                }
                "eigenvector" => {
                    let a = g.adjacency(false);
                    let mut x = vec![1.0; n];
                    for _ in 0..10000 {
                        let mut y = vec![0.0; n];
                        for i in 0..n {
                            for j in 0..n {
                                y[i] += a.data[j * n + i] * x[j];
                            }
                            y[i] += x[i] * 1e-12;
                        }
                        let s: f64 = y.iter().sum();
                        if s == 0.0 {
                            break;
                        }
                        let y: Vec<f64> = y.iter().map(|v| v / s).collect();
                        let change: f64 = y.iter().zip(&x).map(|(a, b)| (a - b).abs()).sum();
                        x = y;
                        if change < 1e-14 {
                            break;
                        }
                    }
                    let s: f64 = x.iter().sum();
                    x.iter().map(|v| v / s).collect()
                }
                other => return rt(format!("Centrality type '{other}' is not supported in this version.")),
            };
            vec![col(c)]
        }
        "subgraph" => {
            let mut keep = g.nodes(need(0)?)?;
            keep.sort();
            keep.dedup();
            let map: Vec<Option<usize>> = (0..n).map(|k| keep.iter().position(|&x| x == k)).collect();
            let sel: Vec<usize> = (0..g.edges.len()).filter(|&k| map[g.edges[k].0].is_some() && map[g.edges[k].1].is_some()).collect();
            let h = Gr {
                directed: g.directed,
                n: keep.len(),
                names: g.names.as_ref().map(|nm| keep.iter().map(|&k| nm[k].clone()).collect()),
                edges: sel.iter().map(|&k| (map[g.edges[k].0].unwrap(), map[g.edges[k].1].unwrap())).collect(),
                weights: g.weights.as_ref().map(|w| sel.iter().map(|&k| w[k]).collect()),
            };
            vec![h.value()]
        }
        "reordernodes" => {
            let order = g.nodes(need(0)?)?;
            if order.len() != n {
                return rt("Node order must be a permutation of the nodes.");
            }
            let mut inv = vec![0; n];
            for (newk, &old) in order.iter().enumerate() {
                inv[old] = newk;
            }
            let h = Gr {
                edges: g.edges.iter().map(|&(s, t)| (inv[s], inv[t])).collect(),
                names: g.names.as_ref().map(|nm| order.iter().map(|&k| nm[k].clone()).collect()),
                ..g.clone()
            };
            // idx: the original edge of each edge of the reordered graph.
            let idx: Vec<f64> = canonical_order(&h).into_iter().map(|k| (k + 1) as f64).collect();
            vec![h.value(), Value::Num(Matrix::col(idx))]
        }
        "flipedge" => {
            let which: Vec<usize> = match rest.len() {
                0 => (0..g.edges.len()).collect(),
                1 => need(0)?.to_matrix()?.data.iter().map(|&x| x as usize - 1).collect(),
                _ => {
                    let (ss, tt) = (g.nodes(&rest[0])?, g.nodes(&rest[1])?);
                    g.edges.iter().enumerate().filter(|(_, e)| ss.iter().zip(&tt).any(|(&s, &t)| e.0 == s && e.1 == t)).map(|(k, _)| k).collect()
                }
            };
            let mut h = g.clone();
            for k in which {
                h.edges[k] = (g.edges[k].1, g.edges[k].0);
            }
            vec![h.value()]
        }
        "edgecount" => {
            let (s, t) = (g.node(need(0)?)?, g.node(need(1)?)?);
            vec![Value::num(g.edges.iter().filter(|e| (e.0 == s && e.1 == t) || (!g.directed && e.0 == t && e.1 == s)).count() as f64)]
        }
        "inedges" | "outedges" => {
            let u = g.node(need(0)?)?;
            let list: Vec<(usize, usize)> = if m == "inedges" { g.in_adj()[u].clone() } else { g.out_adj()[u].clone() };
            let mut ks: Vec<(usize, usize)> = list.iter().map(|&(v, k)| (k, v)).collect();
            ks.sort();
            vec![col(ks.iter().map(|x| (x.0 + 1) as f64).collect()), g.node_list(&ks.iter().map(|x| x.1).collect::<Vec<_>>(), true)]
        }
        "ismultigraph" => {
            let mut seen = BTreeSet::new();
            vec![lg(!g.clone().canonical().edges.iter().all(|e| seen.insert(*e)))]
        }
        "transclosure" => {
            let mut edges = Vec::new();
            for s in 0..n {
                let (order, _) = g.bfs(s);
                for &t in order.iter().skip(1) {
                    edges.push((s, t));
                }
            }
            vec![Gr { edges, weights: None, ..g.clone() }.value()]
        }
        "transreduction" => {
            let reach = |s: usize, skip: usize| -> Vec<bool> {
                let adj = g.out_adj();
                let mut seen = vec![false; n];
                let mut stack: Vec<usize> = adj[s].iter().filter(|&&(_, k)| k != skip).map(|x| x.0).collect();
                while let Some(u) = stack.pop() {
                    if !seen[u] {
                        seen[u] = true;
                        stack.extend(adj[u].iter().map(|x| x.0));
                    }
                }
                seen
            };
            let keep: Vec<(usize, usize)> = g.edges.iter().enumerate().filter(|&(k, &(s, t))| s != t && !reach(s, k)[t]).map(|(_, &e)| e).collect();
            let mut set = BTreeSet::new();
            let edges = keep.into_iter().filter(|e| set.insert(*e)).collect();
            vec![Gr { edges, weights: None, ..g.clone() }.value()]
        }
        "condensation" => {
            let comp = g.components(true);
            let k = comp.iter().max().map(|m| m + 1).unwrap_or(0);
            let set: BTreeSet<(usize, usize)> = g.edges.iter().filter(|e| comp[e.0] != comp[e.1]).map(|e| (comp[e.0], comp[e.1])).collect();
            vec![Gr { directed: true, n: k, names: None, edges: set.into_iter().collect(), weights: None }.value()]
        }
        "nearest" => {
            let s = g.node(need(0)?)?;
            let d = need(1)?.scalar()?;
            let (dist, _) = g.sssp(s, g.weighted());
            let mut list: Vec<usize> = (0..n).filter(|&v| v != s && dist[v] <= d).collect();
            list.sort_by(|&p, &q| dist[p].partial_cmp(&dist[q]).unwrap().then(p.cmp(&q)));
            vec![g.node_list(&list, true), col(list.iter().map(|&v| dist[v]).collect())]
        }
        "shortestpathtree" => {
            let s = g.node(need(0)?)?;
            let (dist, pred) = g.sssp(s, g.weighted());
            let mut edges = Vec::new();
            let mut w = Vec::new();
            for v in 0..n {
                if let Some(p) = pred[v] {
                    edges.push((p, v));
                    w.push(dist[v] - dist[p]);
                }
            }
            vec![Gr { directed: true, n, names: g.names.clone(), edges, weights: g.weights.as_ref().map(|_| w) }.value(), Value::Num(Matrix::row(dist))]
        }
        "allpaths" => {
            let (s, t) = (g.node(need(0)?)?, g.node(need(1)?)?);
            let adj = g.out_adj();
            let mut paths = Vec::new();
            let mut path = vec![s];
            let mut on = vec![false; n];
            on[s] = true;
            fn go(u: usize, t: usize, adj: &[Vec<(usize, usize)>], path: &mut Vec<usize>, on: &mut [bool], out: &mut Vec<Vec<usize>>) {
                if u == t {
                    out.push(path.clone());
                    return;
                }
                let mut next: Vec<usize> = adj[u].iter().map(|x| x.0).collect();
                next.dedup();
                for v in next {
                    if !on[v] {
                        on[v] = true;
                        path.push(v);
                        go(v, t, adj, path, on, out);
                        path.pop();
                        on[v] = false;
                    }
                }
            }
            go(s, t, &adj, &mut path, &mut on, &mut paths);
            let e = edge_cells(&g, &paths, false);
            vec![cells_of_rows(paths, &g), e]
        }
        "allcycles" => {
            let c = all_cycles(&g);
            let e = edge_cells(&g, &c, true);
            vec![cells_of_rows(c, &g), e]
        }
        "cyclebasis" => {
            if g.directed {
                return rt("cyclebasis requires an undirected graph.");
            }
            // Fundamental cycles of a depth-first spanning forest.
            let mut forest = Gr { edges: Vec::new(), weights: None, ..g.clone() };
            let mut tree_edges = BTreeSet::new();
            let mut seen = vec![false; n];
            for s in 0..n {
                if seen[s] {
                    continue;
                }
                let (order, tree, _) = g.dfs(s);
                for v in order {
                    seen[v] = true;
                }
                for (a, b) in tree {
                    tree_edges.insert((a.min(b), a.max(b)));
                    forest.edges.push((a, b));
                }
            }
            let mut cycles = Vec::new();
            for &(a, b) in g.clone().canonical().edges.iter() {
                if tree_edges.remove(&(a, b)) {
                    continue;
                }
                let (_, pred) = forest.sssp(a, false);
                let mut path = vec![b];
                while let Some(p) = pred[*path.last().unwrap()] {
                    path.push(p);
                }
                let min = *path.iter().min().unwrap();
                let at = path.iter().position(|&x| x == min).unwrap();
                path.rotate_left(at);
                if path.len() > 2 && path[1] > path[path.len() - 1] {
                    path[1..].reverse();
                }
                cycles.push(path);
            }
            cycles.sort();
            let e = edge_cells(&g, &cycles, true);
            vec![cells_of_rows(cycles, &g), e]
        }
        "isisomorphic" | "isomorphism" => {
            let Some(Value::Object(o2)) = rest.first() else { return rt("Second argument must be a graph.") };
            let h = Gr::of(o2)?;
            let p = isomorphism(&g, &h);
            if m == "isisomorphic" {
                vec![lg(p.is_some())]
            } else {
                vec![match p {
                    Some(p) => col(p.iter().map(|&x| (x + 1) as f64).collect()),
                    None => Value::Num(Matrix::new(0, 1, Vec::new())),
                }]
            }
        }
        "biconncomp" | "bctree" => {
            if g.directed {
                return rt(format!("{m} requires an undirected graph."));
            }
            let (bins, cuts) = biconnected(&g);
            if m == "biconncomp" {
                vec![row(bins.iter().map(|&b| (b + 1) as f64).collect()), row(cuts.iter().map(|&c| (c + 1) as f64).collect())]
            } else {
                let nb = bins.iter().max().map(|m| m + 1).unwrap_or(0);
                let mut node_of = vec![0.0; n];
                let mut tree = BTreeSet::new();
                for (k, &(s, t)) in g.edges.iter().enumerate() {
                    for v in [s, t] {
                        match cuts.iter().position(|&c| c == v) {
                            Some(ci) => {
                                node_of[v] = (nb + ci + 1) as f64;
                                tree.insert((bins[k], nb + ci));
                            }
                            None => node_of[v] = (bins[k] + 1) as f64,
                        }
                    }
                }
                let tr = Gr { directed: false, n: nb + cuts.len(), names: None, edges: tree.into_iter().collect(), weights: None };
                vec![tr.value(), row(node_of)]
            }
        }
        "plot" => return plot(it, &g, rest, nargout),
        _ => return rt(format!("Undefined function '{m}' for input arguments of type '{}'.", o.class.name)),
    }))
}

/// Biconnected components of an undirected graph: the component of each edge, numbered as
/// the edge-stack algorithm completes them, and the cut vertices in increasing order.
fn biconnected(g: &Gr) -> (Vec<usize>, Vec<usize>) {
    let n = g.n;
    let adj = g.out_adj();
    let mut disc = vec![usize::MAX; n];
    let mut low = vec![0; n];
    let mut bins = vec![usize::MAX; g.edges.len()];
    let mut cuts = BTreeSet::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut time = 0;
    let mut comp = 0;
    #[allow(clippy::too_many_arguments)]
    fn go(
        u: usize, parent_edge: Option<usize>, adj: &[Vec<(usize, usize)>], disc: &mut [usize], low: &mut [usize], bins: &mut [usize], cuts: &mut BTreeSet<usize>,
        stack: &mut Vec<usize>, time: &mut usize, comp: &mut usize, root: bool,
    ) {
        disc[u] = *time;
        low[u] = *time;
        *time += 1;
        let mut children = 0;
        for &(v, k) in &adj[u] {
            if Some(k) == parent_edge {
                continue;
            }
            if disc[v] == usize::MAX {
                children += 1;
                stack.push(k);
                go(v, Some(k), adj, disc, low, bins, cuts, stack, time, comp, false);
                low[u] = low[u].min(low[v]);
                if low[v] >= disc[u] {
                    if !root || children > 1 {
                        cuts.insert(u);
                    }
                    while let Some(e) = stack.pop() {
                        bins[e] = *comp;
                        if e == k {
                            break;
                        }
                    }
                    *comp += 1;
                }
            } else if disc[v] < disc[u] {
                stack.push(k);
                low[u] = low[u].min(disc[v]);
            }
        }
        if root && children > 1 {
            cuts.insert(u);
        }
    }
    for s in 0..n {
        if disc[s] == usize::MAX {
            go(s, None, &adj, &mut disc, &mut low, &mut bins, &mut cuts, &mut stack, &mut time, &mut comp, true);
        }
    }
    (bins.into_iter().map(|b| if b == usize::MAX { 0 } else { b }).collect(), cuts.into_iter().collect())
}

/// `plot(G)`: a GraphPlot (see `charts`) with the nodes on a layered layout (DAGs), a circle or
/// a force-directed one.
fn plot(it: &mut Interpreter, g: &Gr, a: &[Value], nargout: usize) -> Ret {
    let mut layout = "auto".to_string();
    let mut direction = "down".to_string();
    let mut props: Vec<(String, Value)> = Vec::new();
    let mut i = 0;
    // A line spec ('r-o') may come first.
    if let Some(Value::Num(m)) = a.first() {
        if m.kind == Kind::Char && a.len() % 2 == 1 {
            props.extend(crate::charts::graph_linespec(&a[0].text()?));
            i = 1;
        }
    }
    while i + 1 < a.len() {
        let k = a[i].text()?;
        match k.to_ascii_lowercase().as_str() {
            "layout" => layout = a[i + 1].text()?.to_lowercase(),
            "direction" => direction = a[i + 1].text()?.to_lowercase(),
            _ => props.push((k, a[i + 1].clone())),
        }
        i += 2;
    }
    check_layout(&layout)?;
    let pos = layout_nodes(g, &layout, &direction);
    let labels: Vec<String> = match &g.names {
        Some(nm) => nm.clone(),
        None => (1..=g.n).map(|k| num2str_scalar(k as f64)).collect(),
    };
    let graph = crate::charts::GraphData { directed: g.directed, edges: g.edges.clone(), names: g.names.clone() };
    crate::charts::graphplot(it, graph, pos, labels, &props, nargout)
}

fn check_layout(layout: &str) -> R<()> {
    const OK: [&str; 7] = ["auto", "circle", "force", "layered", "subspace", "force3", "subspace3"];
    if OK.contains(&layout) {
        return Ok(());
    }
    rt(format!("Expected input to match one of these values:

'auto', 'circle', 'force', 'layered', 'subspace', 'force3', 'subspace3'

The input, '{layout}', did not match any of the valid values."))
}

/// `layout(h, method, ...)` for a GraphPlot: the node positions of a graph given by its edges.
pub(crate) fn relayout(n: usize, edges: &[(usize, usize)], directed: bool, layout: &str, direction: &str) -> R<Vec<(f64, f64)>> {
    let layout = layout.to_lowercase();
    check_layout(&layout)?;
    let g = Gr { directed, n, names: None, edges: edges.to_vec(), weights: None };
    Ok(layout_nodes(&g, &layout, &direction.to_lowercase()))
}

fn layout_nodes(g: &Gr, layout: &str, direction: &str) -> Vec<(f64, f64)> {
    let n = g.n;
    if n == 0 {
        return Vec::new();
    }
    let layered = layout == "layered" || (layout == "auto" && g.directed && !g.has_cycle());
    if layout == "circle" {
        // Counterclockwise from (1, 0).
        return (0..n).map(|k| {
            let a = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            let (s, c) = a.sin_cos();
            (if c.abs() < 1e-12 { 0.0 } else { c }, if s.abs() < 1e-12 { 0.0 } else { s })
        }).collect();
    }
    if layered {
        return layered_layout(g, direction);
    }
    // Force-directed (Fruchterman-Reingold) from a circle, deterministic.
    let mut p: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let a = 2.0 * std::f64::consts::PI * k as f64 / n as f64;
            (a.cos(), a.sin())
        })
        .collect();
    let k = (4.0 / n as f64).sqrt();
    let mut temp = 0.3;
    for _ in 0..300 {
        let mut d = vec![(0.0, 0.0); n];
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    let (dx, dy) = (p[i].0 - p[j].0, p[i].1 - p[j].1);
                    let dist = dx.hypot(dy).max(1e-6);
                    let f = k * k / dist;
                    d[i].0 += dx / dist * f;
                    d[i].1 += dy / dist * f;
                }
            }
        }
        for &(s, t) in &g.edges {
            if s == t {
                continue;
            }
            let (dx, dy) = (p[s].0 - p[t].0, p[s].1 - p[t].1);
            let dist = dx.hypot(dy).max(1e-6);
            let f = dist * dist / k;
            d[s].0 -= dx / dist * f;
            d[s].1 -= dy / dist * f;
            d[t].0 += dx / dist * f;
            d[t].1 += dy / dist * f;
        }
        for i in 0..n {
            let len = d[i].0.hypot(d[i].1).max(1e-9);
            let step = len.min(temp);
            p[i].0 += d[i].0 / len * step;
            p[i].1 += d[i].1 / len * step;
        }
        temp *= 0.98;
    }
    p
}

/// Layers by longest path from the sources (top layer first). The widest layer spreads its
/// nodes at 1, 2, ...; every other node sits at the median of its descendants (above) or
/// ancestors (below) in that layer, then nodes in a layer are pushed apart to unit spacing.
fn layered_layout(g: &Gr, direction: &str) -> Vec<(f64, f64)> {
    let n = g.n;
    let adj = g.out_adj();
    let mut level = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for u in 0..n {
            for &(v, _) in &adj[u] {
                if v != u && level[v] < level[u] + 1 && level[u] + 1 < n {
                    level[v] = level[u] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let top = *level.iter().max().unwrap_or(&0);
    let layers: Vec<Vec<usize>> = (0..=top).map(|l| (0..n).filter(|&v| level[v] == l).collect()).collect();
    let wide = (0..=top).max_by_key(|&l| (layers[l].len(), std::cmp::Reverse(l))).unwrap_or(0);
    let mut x = vec![f64::NAN; n];
    for (i, &v) in layers[wide].iter().enumerate() {
        x[v] = (i + 1) as f64;
    }
    // Reachability to the widest layer, downward and upward.
    let mut radj = vec![Vec::new(); n];
    for &(s, t) in &g.edges {
        radj[t].push(s);
    }
    let reach = |start: usize, down: bool| -> Vec<usize> {
        let mut seen = vec![false; n];
        let mut stack = vec![start];
        let mut out = Vec::new();
        while let Some(u) = stack.pop() {
            let next: Vec<usize> = if down { adj[u].iter().map(|e| e.0).collect() } else { radj[u].clone() };
            for v in next {
                if !seen[v] {
                    seen[v] = true;
                    if level[v] == wide {
                        out.push(v);
                    }
                    stack.push(v);
                }
            }
        }
        out
    };
    for l in 0..=top {
        if l == wide {
            continue;
        }
        let mut prev = f64::NEG_INFINITY;
        for (i, &v) in layers[l].iter().enumerate() {
            let mut xs: Vec<f64> = reach(v, l < wide).iter().map(|&w| x[w]).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let m = xs.len();
            let mut p = if m == 0 { (i + 1) as f64 } else if m % 2 == 1 { xs[m / 2] } else { (xs[m / 2 - 1] + xs[m / 2]) / 2.0 };
            if p < prev + 1.0 {
                p = prev + 1.0;
            }
            x[v] = p;
            prev = p;
        }
    }
    let layers_n = (top + 1) as f64;
    (0..n)
        .map(|v| {
            let depth = level[v] as f64;
            match direction {
                "up" => (x[v], depth + 1.0),
                "right" => (depth + 1.0, x[v]),
                "left" => (layers_n - depth, x[v]),
                _ => (x[v], layers_n - depth),
            }
        })
        .collect()
}

/// `G = ...` display: the class and the sizes of its tables.
pub fn display(name: &str, v: &Value) -> Option<String> {
    let Value::Object(o) = v else { return None };
    if o.class.name != "graph" && o.class.name != "digraph" {
        return None;
    }
    let size = |p: &str| match o.get(p) {
        Some(Value::Table(t)) => format!("[{}×{} table]", t.nrows, t.vars.len()),
        _ => "[]".into(),
    };
    Some(format!("{name} = \n  {} with properties:\n\n    Edges: {}\n    Nodes: {}", o.class.name, size("Edges"), size("Nodes")))
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "graph" => construct(false, a),
        "digraph" => construct(true, a),
        "matlivre_graph_method" => {
            let m = arg(a, 0, name)?.text()?;
            let nargout = arg(a, 1, name)?.scalar()? as usize;
            let out = method(it, &m, nargout, &a[2..])?.unwrap_or_default();
            let keep = nargout.max(1).min(out.len());
            one(Value::Cell(CellArray::row(out.into_iter().take(keep).collect())))
        }
        _ => Ok(None),
    }
}
