//! polyshape boolean, boundary and buffer methods, legacy TriRep/DelaunayTri, triangulation
//! plots and STL files against MATLAB R2023b. Boolean results are compared as vertex sets
//! where MATLAB starts a boundary at a different vertex.

use livre_lang::{Interpreter, Value};

fn run(src: &str) -> Interpreter {
    let mut it = Interpreter::new();
    it.run_source(src).unwrap();
    it
}

fn nums(it: &Interpreter, v: &str) -> Vec<f64> {
    it.get_var(v).unwrap_or_else(|| panic!("{v} undefined")).to_matrix().unwrap().data
}

fn close(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-4, "{a:?} vs {b:?}");
    }
}

/// Rows of an n-by-2 vertex matrix as a sorted set (NaN separators dropped).
fn rows(it: &Interpreter, v: &str) -> Vec<(i64, i64)> {
    let d = nums(it, v);
    let n = d.len() / 2;
    let mut r: Vec<(i64, i64)> = (0..n).filter(|&i| !d[i].is_nan()).map(|i| ((d[i] * 1e6).round() as i64, (d[n + i] * 1e6).round() as i64)).collect();
    r.sort_unstable();
    r
}

fn set(p: &[(f64, f64)]) -> Vec<(i64, i64)> {
    let mut r: Vec<(i64, i64)> = p.iter().map(|&(x, y)| ((x * 1e6).round() as i64, (y * 1e6).round() as i64)).collect();
    r.sort_unstable();
    r
}

#[test]
fn booleans_match_matlab() {
    let it = run(concat!(
        "p1 = polyshape([0 2 2 0],[0 0 2 2]); p2 = polyshape([1 3 3 1],[1 1 3 3]);\n",
        "u = union(p1,p2); uv = u.Vertices; i = intersect(p1,p2); iv = i.Vertices; s = subtract(p1,p2); sv = s.Vertices;\n",
        "x = xor(p1,p2); xv = x.Vertices; xn = [x.NumRegions x.NumHoles];\n",
        "o = [overlaps(p1,p2) overlaps(p1, polyshape([5 6 6 5],[5 5 6 6]))];\n",
        "[in, out] = intersect(p1, [-1 1; 3 1]);\n",
    ));
    close(&nums(&it, "uv"), &[0., 0., 1., 1., 3., 3., 2., 2., 0., 2., 2., 3., 3., 1., 1., 0.]);
    assert_eq!(rows(&it, "iv"), set(&[(2., 2.), (2., 1.), (1., 1.), (1., 2.)]));
    close(&nums(&it, "sv"), &[0., 0., 1., 1., 2., 2., 0., 2., 2., 1., 1., 0.]);
    assert_eq!(rows(&it, "xv"), set(&[(0., 0.), (0., 2.), (1., 2.), (1., 1.), (2., 1.), (2., 0.), (2., 2.), (1., 2.), (1., 3.), (3., 3.), (3., 1.), (2., 1.)]));
    close(&nums(&it, "xn"), &[2., 0.]);
    close(&nums(&it, "o"), &[1., 0.]);
    close(&nums(&it, "in"), &[0., 2., 1., 1.]);
    close(&nums(&it, "out"), &[-1., 0., f64::NAN, 2., 3., 1., 1., f64::NAN, 1., 1.]);
}

#[test]
fn holes_and_boundaries_match_matlab() {
    let it = run(concat!(
        "h = polyshape({[0 4 4 0],[1 3 3 1]},{[0 0 4 4],[1 1 3 3]}); hv = h.Vertices; hm = [h.NumRegions h.NumHoles area(h) perimeter(h)];\n",
        "H = holes(h); Hv = H.Vertices; ih = ishole(h)'; nb = numboundaries(h); r = rmholes(h); rv = r.Vertices;\n",
        "p1 = polyshape([0 2 2 0],[0 0 2 2]); a = addboundary(p1,[3 4 4 3],[3 3 4 4]); av = a.Vertices; an = a.NumRegions;\n",
        "b = rmboundary(a, 1); bv = b.Vertices;\n",
        "q2 = polyshape([0 1 0 1],[0 0 1 1],'Simplify',false); si = [issimplified(q2) issimplified(p1)];\n",
        "[vid, bid, ind] = nearestvertex(p1, 1.9, 0.1); nv = [vid bid ind];\n",
        "S = sortboundaries(h, 'area', 'ascend'); Sv = S.Vertices;\n",
        "[bx, by] = boundary(h, 2); bxy = [bx by];\n",
        "c = convhull(polyshape([0 2 1 2 0],[0 0 1 2 2])); cv = c.Vertices;\n",
        "z = rmslivers(polyshape([0 1 1 1.000001 1.000001 2 2 0],[0 0 1 1 0 0 2 2]), 0.01); zv = z.Vertices;\n",
        "sr = sortregions(union(p1, polyshape([5 6 6 5],[5 5 6 6])), 'area', 'descend'); srv = sr.Vertices;\n",
        "td = turningdist(p1, polyshape([0 3 3 0],[0 0 1 1]));\n",
        "[cx, cy] = centroid(h); cxy = [cx cy];\n",
    ));
    close(&nums(&it, "hv"), &[0., 0., 4., 4., f64::NAN, 1., 3., 3., 1., 0., 4., 4., 0., f64::NAN, 1., 1., 3., 3.]);
    close(&nums(&it, "hm"), &[1., 1., 12., 24.]);
    close(&nums(&it, "Hv"), &[1., 3., 3., 1., 1., 1., 3., 3.]);
    close(&nums(&it, "ih"), &[0., 1.]);
    close(&nums(&it, "nb"), &[2.]);
    close(&nums(&it, "rv"), &[0., 0., 4., 4., 0., 4., 4., 0.]);
    close(&nums(&it, "av"), &[0., 0., 2., 2., f64::NAN, 3., 3., 4., 4., 0., 2., 2., 0., f64::NAN, 3., 4., 4., 3.]);
    close(&nums(&it, "an"), &[2.]);
    close(&nums(&it, "bv"), &[3., 3., 4., 4., 3., 4., 4., 3.]);
    close(&nums(&it, "si"), &[0., 1.]);
    close(&nums(&it, "nv"), &[4., 1., 4.]);
    close(&nums(&it, "Sv"), &[1., 3., 3., 1., f64::NAN, 0., 0., 4., 4., 1., 1., 3., 3., f64::NAN, 0., 4., 4., 0.]);
    close(&nums(&it, "bxy"), &[1., 3., 3., 1., 1., 1., 1., 3., 3., 1.]);
    close(&nums(&it, "cv"), &[0., 0., 2., 2., 0., 2., 2., 0.]);
    close(&nums(&it, "zv"), &[0., 0., 2., 2., 0., 2., 2., 0.]);
    close(&nums(&it, "srv"), &[0., 0., 2., 2., f64::NAN, 5., 5., 6., 6., 0., 2., 2., 0., f64::NAN, 5., 6., 6., 5.]);
    close(&nums(&it, "td"), &[0.6802]);
    close(&nums(&it, "cxy"), &[2., 2.]);
}

#[test]
fn simplification_and_buffers_match_matlab() {
    let mut it = Interpreter::new();
    it.run_source("q = polyshape([0 1 0 1],[0 0 1 1]); qv = q.Vertices; qn = q.NumRegions;\n").unwrap();
    let warned = it.take_outputs().into_iter().any(|o| matches!(o.kind, livre_lang::OutputKind::Warning(w) if w.starts_with("Polyshape has duplicate vertices")));
    assert!(warned);
    close(&nums(&it, "qv"), &[0., 0.5, 1., f64::NAN, 0., 1., 0.5, 0., 0.5, 0., f64::NAN, 1., 1., 0.5]);
    close(&nums(&it, "qn"), &[2.]);
    let it = run(concat!(
        "p1 = polyshape([0 2 2 0],[0 0 2 2]); B = polybuffer(p1, 0.5); b = [area(B) numsides(B)];\n",
        "L = polybuffer([0 0; 1 0], 'lines', 0.5); l = [area(L) numsides(L)];\n",
        "M = polybuffer(p1, 1, 'JointType', 'miter'); m = area(M);\n",
    ));
    close(&nums(&it, "b"), &[8.7852, 184.0]);
    close(&nums(&it, "l"), &[1.7852, 182.0]);
    close(&nums(&it, "m"), &[16.0]);
}

#[test]
fn legacy_triangulations_plots_and_stl_match_matlab() {
    let it = run(concat!(
        "TR = TriRep([1 2 3;1 3 4], [0 1 1 0]', [0 0 1 1]'); c = class(TR); X = TR.X; cc = circumcenters(TR);\n",
        "e = [isEdge(TR, 1, 3) isEdge(TR, 2, 4)]; b = baryToCart(TR, 1, [1 0 0]); fb = freeBoundary(TR);\n",
        "DT = DelaunayTri([0 1 0 1]', [0 0 1 1]'); c2 = class(DT); n = size(DT.Triangulation); ch = convexHull(DT);\n",
        "h = triplot([1 2 3;1 3 4], [0 1 1 0], [0 0 1 1]); c3 = class(h); x = get(h, 'XData');\n",
        "h2 = trisurf([1 2 3;1 3 4], [0 1 1 0], [0 0 1 1], [0 1 2 1]); c4 = class(h2);\n",
        "f = [tempname '.stl']; stlwrite(triangulation([1 2 3;1 3 4], [0 0 0; 1 0 0; 1 1 0; 0 1 1]), f);\n",
        "R = stlread(f); p = R.Points; cl = R.ConnectivityList;\n",
        "f2 = [tempname '.stl']; stlwrite(R, f2, 'text'); R2 = stlread(f2); p2 = R2.Points; delete(f); delete(f2);\n",
        "bs = boundaryshape(triangulation([1 2 3;1 3 4], [0 0; 1 0; 1 1; 0 1])); bv = bs.Vertices;\n",
    ));
    let text = |v: &str| match it.get_var(v) {
        Some(Value::Num(m)) => m.to_string_lossy(),
        _ => panic!("{v}"),
    };
    assert_eq!(text("c"), "TriRep");
    assert_eq!(text("c2"), "DelaunayTri");
    assert_eq!(text("c3"), "matlab.graphics.chart.primitive.Line");
    assert_eq!(text("c4"), "matlab.graphics.primitive.Patch");
    close(&nums(&it, "X"), &[0., 1., 1., 0., 0., 0., 1., 1.]);
    close(&nums(&it, "cc"), &[0.5, 0.5, 0.5, 0.5]);
    close(&nums(&it, "e"), &[1., 0.]);
    close(&nums(&it, "b"), &[0., 0.]);
    close(&nums(&it, "fb"), &[1., 2., 3., 4., 2., 3., 4., 1.]);
    close(&nums(&it, "n"), &[2., 3.]);
    close(&nums(&it, "ch"), &[1., 2., 4., 3., 1.]);
    let nan = f64::NAN;
    close(&nums(&it, "x"), &[0., 1., nan, 0., 1., nan, 0., 0., nan, 1., 1., nan, 1., 0., nan]);
    close(&nums(&it, "p"), &[0., 1., 1., 0., 0., 0., 1., 1., 0., 0., 0., 1.]);
    close(&nums(&it, "cl"), &[1., 1., 2., 3., 3., 4.]);
    close(&nums(&it, "p2"), &[0., 1., 1., 0., 0., 0., 1., 1., 0., 0., 0., 1.]);
    close(&nums(&it, "bv"), &[0., 1., 1., 0., 0., 0., 1., 1.]);
}
