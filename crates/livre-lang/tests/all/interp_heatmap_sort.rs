//! TriScatteredInterp and heatmap sorting (sortx, sorty), against MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

fn check(src: &str, expected: &[&str]) {
    assert_eq!(shown(src), expected, "{src}");
}

#[test]
fn tri_scattered_interp() {
    check(
        concat!(
            "P = [0 0; 1 0; 0 1; 1 1];\n",
            "F = TriScatteredInterp(P, [1; 2; 3; 4])\n",
            "q = F(0.5, 0.25)\n",
            "m = F.Method\n",
            "out = F(2, 2)\n",
            "G = TriScatteredInterp([0;1;0], [0;0;1], [1;2;3], 'nearest');\n",
            "g = G(0.9, 0.05)\n",
        ),
        &[
            "F = \n  TriScatteredInterp with properties:\n\n         X: [4×2 double]\n         V: [4×1 double]\n    Method: 'linear'",
            "q = 2",
            "m = 'linear'",
            "out = NaN",
            "g = 2",
        ],
    );
}

#[test]
fn heatmap_sortx_and_sorty() {
    check(
        concat!(
            "fig = figure('Visible','off');\n",
            "hm = heatmap([3 1 2; 6 4 5]);\n",
            "sortx(hm, '1');\n",
            "cd1 = hm.XDisplayData'\n",
            "c1 = hm.ColorDisplayData\n",
            "sortx(hm, '2', 'descend');\n",
            "cd2 = hm.XDisplayData'\n",
            "sorty(hm, '1');\n",
            "rd = hm.YDisplayData'\n",
            "sorty(hm, '1', 'descend');\n",
            "rd2 = hm.YDisplayData'\n",
            "c2 = hm.ColorDisplayData\n",
            "sortx(hm);\n",
            "cd3 = hm.XDisplayData'\n",
            "close(fig)\n",
        ),
        &[
            "cd1 = 1×3 cell\n\n    {'2'}    {'3'}    {'1'}",
            "c1 = 2×3\n\n     1     2     3\n     4     5     6",
            "cd2 = 1×3 cell\n\n    {'1'}    {'3'}    {'2'}",
            "rd = 1×2 cell\n\n    {'1'}    {'2'}",
            "rd2 = 1×2 cell\n\n    {'2'}    {'1'}",
            "c2 = 2×3\n\n     6     5     4\n     3     2     1",
            "cd3 = 1×3 cell\n\n    {'2'}    {'3'}    {'1'}",
        ],
    );
}
