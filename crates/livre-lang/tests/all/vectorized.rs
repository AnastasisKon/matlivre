//! Reductions, cumulative sums, max/min, `find` and implicit expansion on N-D and large arrays,
//! against MATLAB R2023b (`data/vectorized.m` printed with `fprintf`; the reference output is in
//! `data/vectorized_matlab.txt`). The large cases cross the thresholds of the parallel paths.

use livre_lang::{Interpreter, OutputKind};

#[test]
fn reductions_scans_and_expansion_match_matlab() {
    let mut it = Interpreter::new();
    let _ = it.run_source(include_str!("data/vectorized.m"));
    let got: Vec<String> = it
        .take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Text(t) => Some(t.trim_end().to_string()),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect();
    let want: Vec<&str> = include_str!("data/vectorized_matlab.txt").lines().collect();
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w);
    }
}
