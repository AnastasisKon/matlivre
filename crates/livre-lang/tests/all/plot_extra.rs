//! ez* plotters, fimplicit, animated lines, histogram2, pie3, plotmatrix, surfl, curl,
//! divergence and movie frames against MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Warning(w) => Some(format!("Warning: {w}")),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

#[test]
fn curl_and_divergence_match_matlab() {
    assert_eq!(
        shown(concat!(
            "[x, y] = meshgrid(0:2, 0:3); u = x.^2 .* y; v = x + y.^2;\n[cz, cav] = curl(x, y, u, v); disp(cz); disp(cav)\nd = divergence(x, y, u, v); disp(d)\n",
            "[x3, y3, z3] = meshgrid(0:2, 0:2, 0:2); U = x3.*y3; V = y3.*z3; W = z3.*x3;\n",
            "[cx, cy, czz, cav3] = curl(x3, y3, z3, U, V, W); disp(squeeze(cx(:,:,2))); disp(squeeze(cav3(:,:,2)))\n",
            "d3 = divergence(x3, y3, z3, U, V, W); disp(squeeze(d3(:,:,2)))\n",
        )),
        [
            "     1     0    -3\n     1     0    -3\n     1     0    -3\n     1     0    -3\n",
            "    0.5000         0   -1.5000\n    0.5000         0   -1.5000\n    0.5000         0   -1.5000\n    0.5000         0   -1.5000\n",
            "     1     1     1\n     3     4     5\n     6     8    10\n     8    11    14\n",
            "     0     0     0\n    -1    -1    -1\n    -2    -2    -2\n",
            "       NaN   -0.5000   -1.0000\n   -0.5000   -0.8660   -1.1667\n   -0.5000   -1.1667   -1.4289\n",
            "     1     2     3\n     2     3     4\n     3     4     5\n",
        ]
    );
}

#[test]
fn implicit_and_animated_lines_match_matlab() {
    assert_eq!(
        shown(concat!(
            "h = fimplicit(@(x, y) x.^2 + y.^2 - 1); c = class(h)\nxr = h.XRange\nyr = h.YRange\nls = h.LineStyle\n",
            "h2 = fimplicit(@(x, y) x - y, [-1 2]); xr2 = h2.XRange\n",
            "a = animatedline; ca = class(a)\naddpoints(a, [1 2 3], [4 5 6]); addpoints(a, 4, 7); [xa, ya] = getpoints(a); p = [xa; ya]\n",
            "clearpoints(a); [xa, ya] = getpoints(a); s = size(xa)\n",
            "a2 = animatedline('MaximumNumPoints', 2); addpoints(a2, 1:5, 1:5); [xb, yb] = getpoints(a2); xb\nm = a2.MaximumNumPoints\n",
            "a3 = animatedline([1 2], [3 4], [5 6]); [p1, q1, r1] = getpoints(a3); pqr = [p1; q1; r1]\n",
        )),
        [
            "c = 'matlab.graphics.function.ImplicitFunctionLine'",
            "xr = 1×2\n\n    -1     1",
            "yr = 1×2\n\n    -1     1",
            "ls = '-'",
            "xr2 = 1×2\n\n    -1     2",
            "ca = 'matlab.graphics.animation.AnimatedLine'",
            "p = 2×4\n\n     1     2     3     4\n     4     5     6     7",
            "s = 1×2\n\n     1     0",
            "xb = 1×2\n\n     4     5",
            "m = 2",
            "pqr = 3×2\n\n     1     2\n     3     4\n     5     6",
        ]
    );
}

#[test]
fn histograms_pies_and_matrices_match_matlab() {
    assert_eq!(
        shown(concat!(
            "H = histogram2([1 2 2 3 3 3], [1 1 2 2 3 3]); c = class(H)\nv = H.Values\ne = H.XBinEdges\nn = H.NumBins\nds = H.DisplayStyle\n",
            "H2 = histogram2(randn(100,1), randn(100,1), 4); s2 = size(H2.Values)\n",
            "H3 = histogram2([0.5 1.5 2.5], [0.5 0.5 2.5], [0 1 2 3], [0 1 2 3]); v3 = H3.Values\nw3 = H3.BinWidth\n",
            "figure; hp = pie3([1 2 3]); np = numel(hp)\nk = {class(hp(1)), class(hp(2)), class(hp(3)), class(hp(4))}\n",
            "figure; [S, AX, BigAx, Hh, HAx] = plotmatrix(magic(3)); ss = size(S)\ncs = class(S(1))\nsa = size(AX)\nsh = size(Hh)\nch = class(Hh(1))\n",
            "figure; [S2, AX2] = plotmatrix([1 2 3]', [4 5 6; 7 8 10]'); s3 = size(S2)\n",
            "figure; hs = surfl(peaks(10)); cl = class(hs)\n",
        )),
        [
            "c = 'matlab.graphics.chart.primitive.Histogram2'",
            "v = 3×3\n\n     1     0     0\n     1     1     0\n     0     1     2",
            "e = 1×4\n\n    0.5000    1.5000    2.5000    3.5000",
            "n = 1×2\n\n     3     3",
            "ds = 'bar3'",
            "s2 = 1×2\n\n     4     4",
            "v3 = 3×3\n\n     1     0     0\n     1     0     0\n     0     0     1",
            "w3 = 1×2\n\n     1     1",
            "np = 12",
            "k = 1×4 cell\n\n    {'matlab.graphics.primitive.Patch'}    {'matlab.graphics.primitive.Surface'}    {'matlab.graphics.primitive.Patch'}    {'matlab.graphics.primitive.Text'}",
            "ss = 1×2\n\n     3     3",
            "cs = 'matlab.graphics.chart.primitive.Line'",
            "sa = 1×2\n\n     3     3",
            "sh = 1×2\n\n     1     3",
            "ch = 'matlab.graphics.chart.primitive.Histogram'",
            "s3 = 1×2\n\n     2     1",
            "cl = 'matlab.graphics.chart.primitive.Surface'",
        ]
    );
}

#[test]
fn ez_plotters_and_frames_match_matlab() {
    assert_eq!(
        shown(concat!(
            "he = ezplot('sin(x)'); c1 = class(he)\nxl = get(gca, 'XLim')\nt = get(gca, 'Title'); t1 = t.String\n",
            "figure; he2 = ezplot('x^2 + y^2 - 1'); c2 = class(he2)\nt = get(gca, 'Title'); t2 = t.String\n",
            "figure; he3 = ezsurf('x*y'); c3 = class(he3)\nt = get(gca, 'Title'); t3 = t.String\n",
            "figure; he4 = ezplot3('cos(t)', 'sin(t)', 't'); c4 = class(he4)\nfigure; he5 = ezcontour('x*y'); c5 = class(he5)\n",
            "F2 = im2frame(uint8(ones(4, 5, 3))); s = size(F2.cdata)\nF3 = im2frame(magic(3), gray(16)); sm = size(F3.colormap)\nc6 = class(F3.cdata)\n",
            "[im, map] = frame2im(F3); same = isequal(im, F3.cdata)\nf = fieldnames(F2)'\n",
        )),
        [
            "c1 = 'matlab.graphics.chart.primitive.Line'",
            "xl = 1×2\n\n   -6.2832    6.2832",
            "t1 = '{sin}({x})'",
            "c2 = 'matlab.graphics.chart.primitive.Contour'",
            "t2 = '{x}^{2} + {y}^{2} - {1} = {0}'",
            "c3 = 'matlab.graphics.chart.primitive.Surface'",
            "t3 = '{x} {y}'",
            "c4 = 'matlab.graphics.chart.primitive.Line'",
            "c5 = 'matlab.graphics.chart.primitive.Contour'",
            "s = 1×3\n\n     4     5     3",
            "sm = 1×2\n\n    16     3",
            "c6 = 'uint8'",
            "same = logical\n   1",
            "f = 1×2 cell\n\n    {'cdata'}    {'colormap'}",
        ]
    );
}
