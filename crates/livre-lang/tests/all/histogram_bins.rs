//! histcounts options (BinWidth, BinLimits, BinMethod, Normalization) and histogram rebinning
//! (NumBins, BinWidth, BinEdges, morebins, fewerbins), against MATLAB R2023b.

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
fn histcounts_options() {
    check(
        concat!(
            "x = [1 2 2 3 3 3 4 4 5];\n",
            "[n1, e1] = histcounts(x, 3)\n",
            "[n2, e2] = histcounts(x, 'BinWidth', 2)\n",
            "[n3, e3] = histcounts(x, 3, 'BinLimits', [0 5])\n",
            "[n5, e5] = histcounts(x, 'BinLimits', [2 4])\n",
            "[n6, e6] = histcounts(x, 4)\n",
            "n7 = histcounts([0.3 1.7 2.2 9.1], 5)\n",
            "[n8, e8] = histcounts(x, 'BinMethod', 'integers')\n",
            "[n9, e9] = histcounts(x, 'Normalization', 'probability')\n",
            "[n12, e12] = histcounts(x, 'BinWidth', 1.5)\n",
        ),
        &[
            "n1 = 1×3\n\n     3     3     3",
            "e1 = 1×4\n\n    1.0000    2.4000    3.8000    5.2000",
            "n2 = 1×3\n\n     1     5     3",
            "e2 = 1×4\n\n     0     2     4     6",
            "n3 = 1×3\n\n     1     5     3",
            "e3 = 1×4\n\n         0    1.6667    3.3333    5.0000",
            "n5 = 1×3\n\n     2     3     2",
            "e5 = 1×4\n\n    2.0000    2.5000    3.5000    4.0000",
            "n6 = 1×4\n\n     1     2     3     3",
            "e6 = 1×5\n\n     1     2     3     4     5",
            "n7 = 1×5\n\n     2     1     0     0     1",
            "n8 = 1×5\n\n     1     2     3     2     1",
            "e8 = 1×6\n\n    0.5000    1.5000    2.5000    3.5000    4.5000    5.5000",
            "n9 = 1×5\n\n    0.1111    0.2222    0.3333    0.2222    0.1111",
            "e9 = 1×6\n\n    0.5000    1.5000    2.5000    3.5000    4.5000    5.5000",
            "n12 = 1×4\n\n     1     2     5     1",
            "e12 = 1×5\n\n         0    1.5000    3.0000    4.5000    6.0000",
        ],
    );
}

#[test]
fn histogram_rebinning_morebins_and_fewerbins() {
    check(
        concat!(
            "figure('Visible','off'); h = histogram([1 2 2 3 3 3 4 4 5]);\n",
            "nb = h.NumBins\n",
            "be = h.BinEdges\n",
            "v = h.Values\n",
            "bw = h.BinWidth\n",
            "h.NumBins = 3;\n",
            "be2 = h.BinEdges\n",
            "v2 = h.Values\n",
            "bw2 = h.BinWidth\n",
            "h.BinWidth = 2;\n",
            "be3 = h.BinEdges\n",
            "v3 = h.Values\n",
            "nb3 = h.NumBins\n",
            "h.BinEdges = [0 2.5 5];\n",
            "v4 = h.Values\n",
            "morebins(h); nb5 = h.NumBins\n",
            "be5 = h.BinEdges\n",
            "fewerbins(h); nb6 = h.NumBins\n",
            "be6 = h.BinEdges\n",
            "h2 = histogram((1:1000)'/100); n0 = h2.NumBins\n",
            "morebins(h2); n1 = h2.NumBins\n",
        ),
        &[
            "nb = 5",
            "be = 1×6\n\n    0.5000    1.5000    2.5000    3.5000    4.5000    5.5000",
            "v = 1×5\n\n     1     2     3     2     1",
            "bw = 1",
            "be2 = 1×4\n\n    1.0000    2.4000    3.8000    5.2000",
            "v2 = 1×3\n\n     3     3     3",
            "bw2 = 1.4000",
            "be3 = 1×4\n\n     0     2     4     6",
            "v3 = 1×3\n\n     1     5     3",
            "nb3 = 3",
            "v4 = 1×2\n\n     3     6",
            "nb5 = 3",
            "be5 = 1×4\n\n         0    1.6667    3.3333    5.0000",
            "nb6 = 2",
            "be6 = 1×3\n\n         0    2.5000    5.0000",
            "n0 = 10",
            "n1 = 11",
        ],
    );
}
