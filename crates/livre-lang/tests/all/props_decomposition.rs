//! Table custom properties (addprop, rmprop) and decomposition objects, against MATLAB R2023b.

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
fn table_custom_properties() {
    check(
        concat!(
            "T = table([1;2;3], {'a';'b';'c'}, 'VariableNames', {'X', 'Y'});\n",
            "T = addprop(T, {'Source', 'Units'}, {'table', 'variable'});\n",
            "T.Properties.CustomProperties.Source = 'sensor';\n",
            "T.Properties.CustomProperties.Units = [\"m\", \"\"];\n",
            "s = T.Properties.CustomProperties.Source\n",
            "u = T.Properties.CustomProperties.Units\n",
            "T = rmprop(T, 'Source');\n",
            "try, T = addprop(T, 'Units', 'table'); catch e, disp(e.message), end\n",
            "try, T = rmprop(T, 'Nope'); catch e, disp(e.message), end\n",
            "try, T = addprop(T, 'Z', 'bogus'); catch e, disp(e.message), end\n",
            "T2 = addprop(table(1), 'Note', 'table');\n",
            "n = T2.Properties.CustomProperties.Note\n",
        ),
        &[
            "s = 'sensor'",
            "u = 1×2 string array\n\n    \"m\"    \"\"",
            "New custom property name 'Units' conflicts with existing 'variable' custom property 'Units'.\n",
            "For each custom property, specify 'variable' or 'table' to indicate whether the property applies to the variables, or to the table as a whole. Specify these settings in a string array or cell array of character vectors.\n",
            "n = []",
        ],
    );
}

#[test]
fn decomposition_objects() {
    check(
        concat!(
            "A = [4 1 0; 1 3 1; 0 1 2];\n",
            "dA = decomposition(A)\n",
            "x = dA \\ [1; 2; 3]\n",
            "dB = decomposition(A, 'lu')\n",
            "dC = decomposition(magic(3), 'qr')\n",
            "dD = decomposition([2 0; 0 3])\n",
            "dE = decomposition(triu(magic(3)))\n",
            "y = [1 2 3] / dA\n",
            "ill = isIllConditioned(dA)\n",
            "dF = decomposition([1 1; 1 1+1e-17]);\n",
            "ill2 = isIllConditioned(dF)\n",
            "r = rcond(dA)\n",
            "k = dA.MatrixSize\n",
            "t = dA.Type\n",
        ),
        &[
            "dA = \n  decomposition with properties:\n\n    MatrixSize: [3 3]\n          Type: 'lu'\n\n  Show all properties\n",
            "x = 3×1\n\n    0.2222\n    0.1111\n    1.4444",
            "dB = \n  decomposition with properties:\n\n    MatrixSize: [3 3]\n          Type: 'lu'\n\n  Show all properties\n",
            "dC = \n  decomposition with properties:\n\n    MatrixSize: [3 3]\n          Type: 'qr'\n\n  Show all properties\n",
            "dD = \n  decomposition with properties:\n\n    MatrixSize: [2 2]\n          Type: 'triangular'\n\n  Show all properties\n",
            "dE = \n  decomposition with properties:\n\n    MatrixSize: [3 3]\n          Type: 'triangular'\n\n  Show all properties\n",
            "y = 1×3\n\n    0.2222    0.1111    1.4444",
            "ill = logical\n   0",
            "ill2 = logical\n   1",
            "r = 0.2250",
            "k = 1×2\n\n     3     3",
            "t = 'lu'",
        ],
    );
}
