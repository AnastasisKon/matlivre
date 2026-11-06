//! svdsketch (sizes, ranks and errors) and symbolic frac, against MATLAB R2023b.

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
fn svdsketch_ranks_and_options() {
    check(
        concat!(
            "A = [3 1 0; 1 2 1; 0 1 4; 2 0 1];\n",
            "[U,S,V] = svdsketch(A); size(U), size(S), size(V), norm(A - U*S*V') < 1e-12\n",
            "d = diag(S)'\n",
            "[U,S,V,apxErr] = svdsketch(A, 0.5); size(S), apxErr <= 0.5\n",
            "[U,S,V] = svdsketch(A, 1e-6, 'MaxSubspaceDimension', 2); size(S)\n",
            "B = ones(6,1)*[1 2 3];\n",
            "[U,S,V] = svdsketch(B); size(S), diag(S)'\n",
            "s = svdsketch(A); size(s)\n",
            "try, svdsketch(A, 2), catch err, disp(err.message), end\n",
            "try, svdsketch(A, 1e-3, 'BlockSize', 0), catch err, disp(err.message), end\n",
            "C = (1:60)'*(1:50) + 0.001*cos((1:60)'*(1:50));\n",
            "[U,S,V,e] = svdsketch(C); size(S), size(e), e < 1e-6\n",
            "[U,S,V,e] = svdsketch(C, 1e-3, 'BlockSize', 3); size(S)\n",
            "[U,S,V,e] = svdsketch(C, 1e-7, 'MaxIterations', 2, 'BlockSize', 4); size(S), size(e)\n",
        ),
        &[
            "ans = 1×2\n\n     4     3",
            "ans = 1×2\n\n     3     3",
            "ans = 1×2\n\n     3     3",
            "ans = logical\n   1",
            "d = 1×3\n\n    4.8041    3.5443    1.5360",
            "ans = 1×2\n\n     3     3",
            "ans = logical\n   1",
            "ans = 1×2\n\n     2     2",
            "ans = 1×2\n\n     1     1",
            "ans = 9.1652",
            "ans = 1×2\n\n     4     3",
            "The tolerance must be a real numeric scalar in the range sqrt(eps(class(A))) <= tolerance < 1.\n",
            "The block size must be a positive integer.\n",
            "ans = 1×2\n\n     6     6",
            "ans = 1×2\n\n     1     1",
            "ans = logical\n   1",
            "ans = 1×2\n\n     3     3",
            "ans = 1×2\n\n     8     8",
            "ans = 1×2\n\n     2     1",
        ],
    );
}

#[test]
fn symbolic_frac() {
    check(
        concat!(
            "syms x y\n",
            "a = frac(sym(7/2))\n",
            "b = frac(sym(-7/2))\n",
            "c = frac(x)\n",
            "d = frac([x, sym(5/3); sym(pi), 2*y])\n",
            "e = frac(sym(4))\n",
            "g = frac(x + 1/2)\n",
            "try, frac(2.5), catch err, disp(err.message), end\n",
        ),
        &[
            "a = 1/2",
            "b = -1/2",
            "c = x - fix(x)",
            "d = \n[x - fix(x),            2/3]\n[    pi - 3, 2*y - fix(2*y)]",
            "e = 0",
            "g = x - fix(x + 1/2) + 1/2",
            "Undefined function 'frac' for input arguments of type 'double'.\n",
        ],
    );
}
