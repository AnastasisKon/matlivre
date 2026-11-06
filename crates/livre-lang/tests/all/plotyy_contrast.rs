//! plotyy (aligned y ticks, colors, overlaid right axes) and contrast against MATLAB R2023b.

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

fn check(src: &str, expected: &[&str]) {
    assert_eq!(shown(src), expected, "{src}");
}

#[test]
fn contrast_colormaps() {
    check(
        concat!(
            "m2 = contrast(magic(4), 5)\n",
            "m = contrast(magic(4)); s = size(m)\n",
            "r = m([1 2 16 17 18 255 256], 1)'\n",
            "c = contrast(peaks(10), 8)\n",
            "c1 = contrast(1:3, 4)\n",
            "c2 = contrast([0 0.5 1], 2)\n",
            "c3 = contrast([1 1 1], 4)\n",
            "c4 = contrast([0 0.5 1 NaN], 3)\n",
            "c5 = contrast(int8([1 5 9]), 3)\n",
            "c6 = contrast('ab', 3)\n",
            "c7 = contrast([1 2], 0)\n",
        ),
        &[
            "m2 = 5×3\n\n    0.1429    0.1429    0.1429\n    0.3810    0.3810    0.3810\n    0.6190    0.6190    0.6190\n    0.8571    0.8571    0.8571\n    1.0000    1.0000    1.0000",
            "s = 1×2\n\n   256     3",
            "r = 1×7\n\n    0.0074    0.0110    0.0625    0.0662    0.0735    0.9926    1.0000",
            "c = 8×3\n\n    0.0185    0.0185    0.0185\n    0.0370    0.0370    0.0370\n    0.1111    0.1111    0.1111\n    0.7315    0.7315    0.7315\n    0.8426    0.8426    0.8426\n    0.9630    0.9630    0.9630\n    0.9722    0.9722    0.9722\n    1.0000    1.0000    1.0000",
            "c1 = 4×3\n\n    0.2857    0.2857    0.2857\n    0.4286    0.4286    0.4286\n    0.7143    0.7143    0.7143\n    1.0000    1.0000    1.0000",
            "c2 = 2×3\n\n    0.4000    0.4000    0.4000\n    1.0000    1.0000    1.0000",
            "c3 = 7×3\n\n    0.1429    0.1429    0.1429\n    0.2857    0.2857    0.2857\n    0.4286    0.4286    0.4286\n    0.5714    0.5714    0.5714\n    0.7143    0.7143    0.7143\n    0.8571    0.8571    0.8571\n    1.0000    1.0000    1.0000",
            "c4 = 4×3\n\n    0.2857    0.2857    0.2857\n    0.5714    0.5714    0.5714\n    0.8571    0.8571    0.8571\n    1.0000    1.0000    1.0000",
            "c5 = 3×3\n\n    0.3333    0.3333    0.3333\n    0.6667    0.6667    0.6667\n    1.0000    1.0000    1.0000",
            "c6 = 3×3\n\n    0.4000    0.4000    0.4000\n    0.6000    0.6000    0.6000\n    1.0000    1.0000    1.0000",
            "c7 = 1×3\n\n     1     1     1",
        ],
    );
}

#[test]
fn plotyy_axes() {
    check(
        concat!(
            "x = 0:0.5:3;\n",
            "figure;\n",
            "[ax, h1, h2] = plotyy(x, sin(x), x, 10*cos(x))\n",
            "cls = class(ax), sz = size(ax)\n",
            "yl1 = ax(1).YLim, yl2 = ax(2).YLim, xl = ax(2).XLim\n",
            "yt1 = ax(1).YTick, yt2 = ax(2).YTick\n",
            "loc = ax(2).YAxisLocation, bg = ax(2).Color, yc = ax(2).YColor\n",
            "c2 = h2.Color\n",
        ),
        &[
            "ax = \n  1×2 Axes array:\n\n    Axes    Axes",
            "h1 = \n  Line with properties:\n\n              Color: [0 0.4470 0.7410]\n          LineStyle: '-'\n          LineWidth: 0.5000\n             Marker: 'none'\n         MarkerSize: 6\n    MarkerFaceColor: 'none'\n              XData: [0 0.5000 1 1.5000 2 2.5000 3]\n              YData: [0 0.4794 0.8415 0.9975 0.9093 0.5985 0.1411]\n\n  Use GET to show all properties",
            "h2 = \n  Line with properties:\n\n              Color: [0.8500 0.3250 0.0980]\n          LineStyle: '-'\n          LineWidth: 0.5000\n             Marker: 'none'\n         MarkerSize: 6\n    MarkerFaceColor: 'none'\n              XData: [0 0.5000 1 1.5000 2 2.5000 3]\n              YData: [10 8.7758 5.4030 0.7074 -4.1615 -8.0114 -9.8999]\n\n  Use GET to show all properties",
            "cls = 'matlab.graphics.axis.Axes'",
            "sz = 1×2\n\n     1     2",
            "yl1 = 1×2\n\n     0     1",
            "yl2 = 1×2\n\n   -10    10",
            "xl = 1×2\n\n     0     3",
            "yt1 = 1×3\n\n         0    0.5000    1.0000",
            "yt2 = 1×3\n\n   -10     0    10",
            "loc = 'right'",
            "bg = 'none'",
            "yc = 1×3\n\n    0.8500    0.3250    0.0980",
            "c2 = 1×3\n\n    0.8500    0.3250    0.0980",
        ],
    );
}

#[test]
fn plotyy_tick_alignment() {
    check(
        concat!(
            "cases = {{1:10, @(x) x.^2, @(x) log(x)}, {0:0.1:2, @(x) exp(x), @(x) 3-x}, {0:20, @(x) 1000+x, @(x) -x/7}, ...\n",
            "  {[1 2 3], @(x) [0.2 0.9 0.3], @(x) [-7 3 50]}, {[1 2 3], @(x) [3 3 3], @(x) x}, {1:4, @(x) [5 1 2 9], @(x) [0.001 0.003 0.002 0.0015]}, ...\n",
            "  {[0 1], @(x) [-1 1], @(x) [2 1e4]}, {[1 2], @(x) [-3 0], @(x) [-30 0]}, {[1 2], @(x) [0 0.8], @(x) [0 8]}, {[1 2], @(x) [0 2.5], @(x) [0 25]}};\n",
            "for k = 1:numel(cases)\n",
            "  c = cases{k}; x = c{1};\n",
            "  figure; ax = plotyy(x, c{2}(x), x, c{3}(x));\n",
            "  fprintf('%d: [%s] [%s]\\n', k, mat2str(ax(1).YTick), mat2str(ax(2).YTick));\n",
            "end\n",
            "figure; [ax, h1, h2] = plotyy(0:0.5:3, sin(0:0.5:3), 0:0.5:3, 10*cos(0:0.5:3), @stem, 'bar');\n",
            "k1 = class(h1), k2 = class(h2)\n",
            "figure; ax = plotyy(1:3, [1 2 3; 4 5 6]', 1:3, [3 2 1]);\n",
            "cc = ax(2).Children.Color\n",
            "yc = ax(1).YColor\n",
        ),
        &[
            "1: [[0 20 40 60 80 100]] [[0 0.5 1 1.5 2 2.5]]\n",
            "2: [[0 2 4 6 8]] [[1 1.5 2 2.5 3]]\n",
            "3: [[1000 1010 1020]] [[-4 -2 0]]\n",
            "4: [[0.2 0.4 0.6 0.8 1]] [[-20 0 20 40 60]]\n",
            "5: [[2 2.5 3 3.5 4]] [[1 1.5 2 2.5 3]]\n",
            "6: [[0 5 10]] [[0.001 0.002 0.003]]\n",
            "7: [[-1 0 1]] [[0 5000 10000]]\n",
            "8: [[-3 -2.5 -2 -1.5 -1 -0.5 0]] [[-30 -25 -20 -15 -10 -5 0]]\n",
            "9: [[0 0.2 0.4 0.6 0.8]] [[0 2 4 6 8]]\n",
            "10: [[0 0.5 1 1.5 2 2.5]] [[0 5 10 15 20 25]]\n",
            "k1 = 'matlab.graphics.chart.primitive.Stem'",
            "k2 = 'matlab.graphics.chart.primitive.Bar'",
            "cc = 1×3\n\n    0.9290    0.6940    0.1250",
            "yc = 1×3\n\n    0.1500    0.1500    0.1500",
        ],
    );
}
