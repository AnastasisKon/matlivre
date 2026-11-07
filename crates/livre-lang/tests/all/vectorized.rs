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

/// Eigenvalues, QR, Cholesky, LU, norm, convolution, `ismember`, `unique` and `containers.Map` on
/// matrices large enough to take the fast kernels (`data/linalg_fast.m`), against MATLAB R2023b.
#[test]
fn linear_algebra_and_lookups_match_matlab() {
    let mut it = Interpreter::new();
    let _ = it.run_source(include_str!("data/linalg_fast.m"));
    let got: String = it
        .take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Text(t) => Some(t),
            OutputKind::Error(e) => Some(format!("Error: {e}
")),
            _ => None,
        })
        .collect();
    let want = include_str!("data/linalg_fast_matlab.txt");
    for (g, w) in got.trim_end().lines().zip(want.trim_end().lines()) {
        assert_eq!(g, w);
    }
    assert_eq!(got.trim_end().lines().count(), want.trim_end().lines().count());
}

/// Chains of elementwise operators on large arrays are evaluated fused; the result must equal
/// the same computation one operator at a time, as in MATLAB (`data/fusion.m`), including the
/// cases that turn complex or hold NaN, where fusion gives way to the ordinary evaluator.
#[test]
fn fused_expressions_equal_stepwise_evaluation() {
    let mut it = Interpreter::new();
    let _ = it.run_source(include_str!("data/fusion.m"));
    let got: String = it
        .take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Text(t) => Some(t),
            OutputKind::Error(e) => Some(format!("Error: {e}\n")),
            _ => None,
        })
        .collect();
    assert_eq!(got.trim_end(), include_str!("data/fusion_matlab.txt").trim_end());
}
