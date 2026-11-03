//! `graph` and `digraph` against MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

fn run(src: &str) -> Interpreter {
    let mut it = Interpreter::new();
    it.run_source(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    it
}

fn nums(it: &Interpreter, v: &str) -> Vec<f64> {
    it.get_var(v).unwrap_or_else(|| panic!("no {v}")).to_matrix().unwrap().data
}

fn close(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-4 || (x.is_infinite() && x == y), "{a:?} vs {b:?}");
    }
}

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

#[test]
fn construction_and_display() {
    assert_eq!(
        shown("G = graph([1 1 2 3], [2 3 3 4], [5 1 2 7])\nE = G.Edges\nH = digraph([1 2 2 3], [2 3 4 1])\nGn = graph({'a','b'}, {'b','c'})\nnd = Gn.Nodes\n"),
        [
            "G = \n  graph with properties:\n\n    Edges: [4×2 table]\n    Nodes: [4×0 table]",
            "E = 4×2 table\n\n    EndNodes    Weight\n    ________    ______\n\n     1    2       5   \n     1    3       1   \n     2    3       2   \n     3    4       7   ",
            "H = \n  digraph with properties:\n\n    Edges: [4×1 table]\n    Nodes: [4×0 table]",
            "Gn = \n  graph with properties:\n\n    Edges: [2×1 table]\n    Nodes: [3×1 table]",
            "nd = 3×1 table\n\n    Name \n    _____\n\n    {'a'}\n    {'b'}\n    {'c'}",
        ]
    );
}

#[test]
fn queries_paths_and_matrices() {
    let it = run(concat!(
        "G = graph([1 1 2 3], [2 3 3 4], [5 1 2 7]);\n",
        "nn = numnodes(G); ne = numedges(G); d = degree(G); nb = neighbors(G, 3); [p, len] = shortestpath(G, 1, 4); D = distances(G);\n",
        "A = adjacency(G); L = laplacian(G); I = incidence(G); ix = findedge(G, 1, 3); bf = bfsearch(G, 1); df = dfsearch(G, 1);\n",
        "H = digraph([1 2 2 3], [2 3 4 1]); od = outdegree(H); id = indegree(H); sc = successors(H, 2); pr = predecessors(H, 1); dag = isdag(H);\n",
        "K = digraph([1 1 2], [2 3 3]); order = toposort(K); bins = conncomp(graph([1 3], [2 4]));\n",
        "T = minspantree(G); te = T.Edges.EndNodes; G2 = addedge(G, 4, 5, 3); ne2 = numedges(G2); G3 = rmedge(G, 1, 2); ne3 = numedges(G3);\n",
        "mf = maxflow(digraph([1 1 2 3], [2 3 4 4], [3 2 1 3]), 1, 4); hc = hascycles(G); c = centrality(G, 'degree');\n",
    ));
    close(&nums(&it, "nn"), &[4.0]);
    close(&nums(&it, "ne"), &[4.0]);
    close(&nums(&it, "d"), &[2.0, 2.0, 3.0, 1.0]);
    close(&nums(&it, "nb"), &[1.0, 2.0, 4.0]);
    close(&nums(&it, "p"), &[1.0, 3.0, 4.0]);
    close(&nums(&it, "len"), &[8.0]);
    close(&nums(&it, "D"), &[0.0, 3.0, 1.0, 8.0, 3.0, 0.0, 2.0, 9.0, 1.0, 2.0, 0.0, 7.0, 8.0, 9.0, 7.0, 0.0]);
    close(&nums(&it, "A"), &[0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0]);
    close(&nums(&it, "L"), &[2.0, -1.0, -1.0, 0.0, -1.0, 2.0, -1.0, 0.0, -1.0, -1.0, 3.0, -1.0, 0.0, 0.0, -1.0, 1.0]);
    close(&nums(&it, "I"), &[-1.0, 1.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, 1.0]);
    close(&nums(&it, "ix"), &[2.0]);
    close(&nums(&it, "bf"), &[1.0, 2.0, 3.0, 4.0]);
    close(&nums(&it, "df"), &[1.0, 2.0, 3.0, 4.0]);
    close(&nums(&it, "od"), &[1.0, 2.0, 1.0, 0.0]);
    close(&nums(&it, "id"), &[1.0, 1.0, 1.0, 1.0]);
    close(&nums(&it, "sc"), &[3.0, 4.0]);
    close(&nums(&it, "pr"), &[3.0]);
    close(&nums(&it, "dag"), &[0.0]);
    close(&nums(&it, "order"), &[1.0, 2.0, 3.0]);
    close(&nums(&it, "bins"), &[1.0, 1.0, 2.0, 2.0]);
    close(&nums(&it, "te"), &[1.0, 2.0, 3.0, 3.0, 3.0, 4.0]);
    close(&nums(&it, "ne2"), &[5.0]);
    close(&nums(&it, "ne3"), &[3.0]);
    close(&nums(&it, "mf"), &[3.0]);
    close(&nums(&it, "hc"), &[1.0]);
    close(&nums(&it, "c"), &[2.0, 2.0, 3.0, 1.0]);
}

#[test]
fn algorithms_match_matlab() {
    let it = run(concat!(
        "H = digraph([1 2 3 3 4 5], [2 3 1 4 5 4]); cs = conncomp(H); cw = conncomp(H, 'Type', 'weak');\n",
        "K = digraph([3 3 1 4], [1 2 2 2]); ts = toposort(K);\n",
        "G = graph([1 1 2 3 3 4], [2 3 3 4 5 5]); nr = nearest(G, 1, 1);\n",
        "p = allpaths(G, 1, 5); np = numel(p); p1 = p{1}; p4 = p{4}; c = allcycles(G); c1 = c{1}; c2 = c{2}; b = cyclebasis(G); b2 = b{2};\n",
        "iso = isisomorphic(G, graph([5 5 4 3 3 2], [4 3 3 2 1 1]));\n",
        "TC = transclosure(digraph([1 2], [2 3])); tc = TC.Edges.EndNodes; TR = transreduction(digraph([1 1 2], [2 3 3])); tr = TR.Edges.EndNodes;\n",
        "C = condensation(H); cn = numnodes(C); ce = C.Edges.EndNodes; bc = biconncomp(graph([1 2 3 3 4], [2 3 1 4 5]));\n",
        "cl = centrality(G, 'closeness'); bt = centrality(G, 'betweenness'); pg = centrality(G, 'pagerank'); ev = centrality(G, 'eigenvector');\n",
        "T = shortestpathtree(G, 1); st = T.Edges.EndNodes; [mf, GF] = maxflow(digraph([1 1 2 3], [2 3 4 4], [3 2 1 3]), 1, 4); gw = GF.Edges.Weight;\n",
        "R = reordernodes(G, [5 4 3 2 1]); re = R.Edges.EndNodes; Sg = subgraph(G, [3 4 5]); se = Sg.Edges.EndNodes;\n",
        "M = graph([1 1], [2 2]); mg = ismultigraph(M); ec = edgecount(M, 1, 2); GA = graph([0 2 0; 2 0 3; 0 3 0]); ga = GA.Edges.Weight;\n",
        "dfe = dfsearch(G, 1, 'edgetonew'); bfe = bfsearch(G, 1, 'edgetonew'); ie = inedges(H, 4); oe = outedges(H, 3);\n",
        "F = flipedge(digraph([1 2],[2 3])); fe = F.Edges.EndNodes; [btr, ind] = bctree(graph([1 2 3 3 4], [2 3 1 4 5])); be = btr.Edges.EndNodes;\n",
    ));
    close(&nums(&it, "cs"), &[1.0, 1.0, 1.0, 2.0, 2.0]);
    close(&nums(&it, "cw"), &[1.0; 5]);
    close(&nums(&it, "ts"), &[4.0, 3.0, 1.0, 2.0]);
    close(&nums(&it, "nr"), &[2.0, 3.0]);
    close(&nums(&it, "np"), &[4.0]);
    close(&nums(&it, "p1"), &[1.0, 2.0, 3.0, 4.0, 5.0]);
    close(&nums(&it, "p4"), &[1.0, 3.0, 5.0]);
    close(&nums(&it, "c1"), &[1.0, 2.0, 3.0]);
    close(&nums(&it, "c2"), &[3.0, 4.0, 5.0]);
    close(&nums(&it, "b2"), &[3.0, 4.0, 5.0]);
    close(&nums(&it, "iso"), &[1.0]);
    close(&nums(&it, "tc"), &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    close(&nums(&it, "tr"), &[1.0, 2.0, 2.0, 3.0]);
    close(&nums(&it, "cn"), &[2.0]);
    close(&nums(&it, "ce"), &[1.0, 2.0]);
    close(&nums(&it, "bc"), &[3.0, 3.0, 3.0, 2.0, 1.0]);
    close(&nums(&it, "cl"), &[0.1667, 0.1667, 0.25, 0.1667, 0.1667]);
    close(&nums(&it, "bt"), &[0.0, 0.0, 4.0, 0.0, 0.0]);
    close(&nums(&it, "pg"), &[0.1702, 0.1702, 0.3193, 0.1702, 0.1702]);
    close(&nums(&it, "ev"), &[0.1798, 0.1798, 0.2808, 0.1798, 0.1798]);
    close(&nums(&it, "st"), &[1.0, 1.0, 3.0, 3.0, 2.0, 3.0, 4.0, 5.0]);
    close(&nums(&it, "mf"), &[3.0]);
    close(&nums(&it, "gw"), &[1.0, 2.0, 1.0, 2.0]);
    close(&nums(&it, "re"), &[1.0, 1.0, 2.0, 3.0, 3.0, 4.0, 2.0, 3.0, 3.0, 4.0, 5.0, 5.0]);
    close(&nums(&it, "se"), &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    close(&nums(&it, "mg"), &[1.0]);
    close(&nums(&it, "ec"), &[2.0]);
    close(&nums(&it, "ga"), &[2.0, 3.0]);
    close(&nums(&it, "dfe"), &[1.0, 2.0, 3.0, 4.0, 2.0, 3.0, 4.0, 5.0]);
    close(&nums(&it, "bfe"), &[1.0, 1.0, 3.0, 3.0, 2.0, 3.0, 4.0, 5.0]);
    close(&nums(&it, "ie"), &[4.0, 6.0]);
    close(&nums(&it, "oe"), &[3.0, 4.0]);
    close(&nums(&it, "fe"), &[2.0, 3.0, 1.0, 2.0]);
    close(&nums(&it, "ind"), &[3.0, 3.0, 4.0, 5.0, 1.0]);
    close(&nums(&it, "be"), &[1.0, 2.0, 2.0, 3.0, 5.0, 4.0, 5.0, 4.0]);
}
