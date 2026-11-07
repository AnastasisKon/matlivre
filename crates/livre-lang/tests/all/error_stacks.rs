//! Error stacks and getReport, against MATLAB R2023b: "Error using f", "Error in f (line n)"
//! with the source line, MException.stack, throwAsCaller and rethrow.

use livre_lang::{Interpreter, OutputKind};

/// Runs `src` as the script `name`, returning the displayed values and text.
fn shown_as(name: &str, src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    it.set_script(name, "");
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

/// The text of the value displayed as `name = ...`.
fn value<'a>(out: &'a [String], name: &str) -> &'a str {
    out.iter().find(|t| t.starts_with(&format!("{name} ="))).map(|s| s.as_str()).unwrap_or("")
}

#[test]
fn reports_and_stacks() {
    let out = shown_as("stk", STK);
    assert_eq!(
        value(&out, "r"),
        "r = \n    'Error using stk>inner\n     Bad value 3\n     \n     Error in stk>outer (line 29)\n     inner(k + x);\n     \n     Error in stk (line 2)\n         outer(2)'"
    );
    assert!(out.iter().any(|t| t == "inner 32\n") && out.iter().any(|t| t == "outer 29\n") && out.iter().any(|t| t == "stk 2\n"), "{out:?}");
    assert_eq!(
        value(&out, "r2"),
        "r2 = \n    'Index exceeds the number of array elements. Index must not exceed 2.\n     \n     Error in stk (line 10)\n         q = [1 2]; q(3)'"
    );
    assert_eq!(
        value(&out, "r3"),
        "r3 = \n    'Unable to perform assignment because the size of the left side is 1-by-1 and the size of the right side is 2-by-2.\n     \n     Error in stk>viaHelper (line 36)\n     y(5, 5, 5) = ones(2);\n     \n     Error in stk (line 16)\n         viaHelper()'"
    );
    assert_eq!(value(&out, "r4"), "r4 = \n    'Error using stk>useCaller\n     Negative input\n     \n     Error in stk (line 22)\n         useCaller(-1)'");
    assert_eq!(value(&out, "r5"), "r5 = 'Plain 3'");
}

#[test]
fn rethrow_keeps_the_stack() {
    let out = shown_as("rt2", RT2);
    assert_eq!(out[0], "     3\n");
    assert_eq!(
        value(&out, "r"),
        "r = \n    'Error using rt2>deep\n     Deep 1\n     \n     Error in rt2>wrap (line 9)\n         deep(1)\n     \n     Error in rt2 (line 2)\n         wrap()'"
    );
    assert_eq!(value(&out, "ans"), "ans = 1\u{d7}3 cell\n\n    {'deep'}    {'wrap'}    {'rt2'}");
}

const STK: &str = "try\n    outer(2)\ncatch e\n    r = getReport(e)\n    s = e.stack;\n    size(s)\n    for k = 1:numel(s), disp([s(k).name ' ' num2str(s(k).line)]), end\nend\ntry\n    q = [1 2]; q(3)\ncatch e\n    r2 = getReport(e)\n    size(e.stack)\nend\ntry\n    viaHelper()\ncatch e\n    r3 = getReport(e)\n    for k = 1:numel(e.stack), disp([e.stack(k).name ' ' num2str(e.stack(k).line)]), end\nend\ntry\n    useCaller(-1)\ncatch e\n    r4 = getReport(e)\nend\nr5 = getReport(MException('a:b', 'Plain %d', 3))\nfunction outer(k)\nx = 1;\ninner(k + x);\nend\nfunction inner(k)\nerror('my:id', 'Bad value %d', k);\nend\nfunction viaHelper()\ny = zeros(2);\ny(5, 5, 5) = ones(2);\nend\nfunction useCaller(v)\ncheckPos(v);\nend\nfunction checkPos(v)\nif v < 0\n    ME = MException('c:neg', 'Negative input');\n    throwAsCaller(ME);\nend\nend\n";

const RT2: &str = "try\n    wrap()\ncatch e\n    r = getReport(e)\n    {e.stack.name}\nend\nfunction wrap()\ntry\n    deep(1)\ncatch err\n    s = err.stack; disp(numel(s))\n    rethrow(err)\nend\nend\nfunction deep(n)\nerror('d:e', 'Deep %d', n)\nend\n";
