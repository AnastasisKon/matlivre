//! Symbolic DAE tools (incidenceMatrix, massMatrixForm, isLowIndexDAE, findDecoupledBlocks,
//! reduceDifferentialOrder, odeFunction), against MATLAB R2023b.

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
fn incidence_mass_matrix_index_and_blocks() {
    check(
        concat!(
            "syms x(t) y(t) z(t) a\n",
            "eqs = [diff(x,t) == y; diff(y,t) == -x + z; x + y + z == 1];\n",
            "vars = [x; y; z];\n",
            "I = incidenceMatrix(eqs, vars)\n",
            "[M, F] = massMatrixForm(eqs, vars);\n",
            "M\n",
            "F12 = F(1:2)\n",
            "low = isLowIndexDAE(eqs, vars)\n",
            "eqs2 = [diff(x,t,2) == -x; diff(y,t) == x];\n",
            "[n2, v2, R2] = reduceDifferentialOrder(eqs2, [x; y]);\n",
            "n2b = n2(2:3)\n",
            "v2\n",
            "R2\n",
            "[eqsBlocks, varsBlocks] = findDecoupledBlocks(eqs, vars)\n",
        ),
        &[
            "I = 3×3\n\n     1     1     0\n     1     1     1\n     1     1     1",
            "M = \n[1, 0, 0]\n[0, 1, 0]\n[0, 0, 0]",
            "F12 = \n       y(t)\nz(t) - x(t)",
            "low = logical\n   1",
            "n2b = \n  diff(y(t), t) - x(t)\nDxt(t) - diff(x(t), t)",
            "v2 = \n  x(t)\n  y(t)\nDxt(t)",
            "R2 = \n[Dxt(t), diff(x(t), t)]",
            "eqsBlocks = 1×1 cell\n\n    {[1 2 3]}",
            "varsBlocks = 1×1 cell\n\n    {[1 2 3]}",
        ],
    );
}

#[test]
fn ode_functions() {
    check(
        concat!(
            "syms x(t) y(t) z(t) a\n",
            "f = odeFunction([y; -x], [x; y]);\n",
            "class(f)\n",
            "v = f(0, [1; 2])\n",
            "f2 = odeFunction([a*y; -x], [x; y], a);\n",
            "v2b = f2(0, [1; 2], 3)\n",
            "eqs3 = [diff(x,t) == y; x^2 + y^2 == 1];\n",
            "low3 = isLowIndexDAE(eqs3, [x; y])\n",
        ),
        &[
            "ans = 'function_handle'",
            "v = 2×1\n\n     2\n    -1",
            "v2b = 2×1\n\n     6\n    -1",
            "low3 = logical\n   1",
        ],
    );
}
