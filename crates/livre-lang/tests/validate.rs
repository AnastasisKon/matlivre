//! Argument validation (`mustBe*`, `inputParser`), error identifiers and state (`lasterr`,
//! `lastwarn`, `rethrow`), name helpers, `onCleanup` and `validatecolor`, against MATLAB R2023b.

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

/// `[identifier] message` of the error each statement raises.
fn errors(cases: &[&str]) -> Vec<String> {
    cases
        .iter()
        .map(|c| {
            let src = format!("try\n{c};\ncatch e\nfprintf('[%s] %s', e.identifier, e.message);\nend\n");
            shown(&src).concat()
        })
        .collect()
}

#[test]
fn validators_match_matlab_messages() {
    let got = errors(&[
        "mustBeA(1, \"char\")",
        "mustBeFinite([1 Inf])",
        "mustBeFloat(int8(1))",
        "mustBeGreaterThan(1, 2)",
        "mustBeInRange(5, 1, 3)",
        "mustBeInRange(3, 1, 3, \"exclusive\")",
        "mustBeInteger(1.5)",
        "mustBeLessThanOrEqual(3, 2)",
        "mustBeMember(4, [1 2 3])",
        "mustBeMember(\"x\", [\"a\" \"b\"])",
        "mustBeNonempty([])",
        "mustBeNonmissing(NaN)",
        "mustBeNonpositive(1)",
        "mustBeNonzeroLengthText(\"\")",
        "mustBeNumeric(true)",
        "mustBeReal(1i)",
        "mustBeScalarOrEmpty([1 2])",
        "mustBeText(1)",
        "mustBeTextScalar([\"a\" \"b\"])",
        "mustBeUnderlyingType(1, \"single\")",
        "mustBeValidVariableName(\"1x\")",
        "mustBeVector(ones(2))",
        "mustBeFile(\"nope.txt\")",
        "mustBeVector([], \"allow-all-empties\"); mustBePositive([1 2 3])",
    ]);
    let want = [
        "[MATLAB:validators:mustBeA] Value must be one of the following types: 'char'.",
        "[MATLAB:validators:mustBeFinite] Value must be finite.",
        "[MATLAB:validators:mustBeFloat] Value must be a floating-point array.",
        "[MATLAB:validators:mustBeGreaterThan] Value must be greater than 2.",
        "[MATLAB:validators:mustBeInRange] Value must be greater than or equal to 1, and less than or equal to 3.",
        "[MATLAB:validators:mustBeInRange] Value must be greater than 1, and less than 3.",
        "[MATLAB:validators:mustBeInteger] Value must be integer.",
        "[MATLAB:validators:mustBeLessThanOrEqual] Value must be less than or equal to 2.",
        "[MATLAB:validators:mustBeMember] Value must be a member of this set:\n    1\n    2\n    3\n",
        "[MATLAB:validators:mustBeMember] Value must be a member of this set:\n    'a'\n    'b'\n",
        "[MATLAB:validators:mustBeNonempty] Value must not be empty.",
        "[MATLAB:validators:mustBeNonmissing] Value must not have missing data.",
        "[MATLAB:validators:mustBeNonpositive] Value must not be positive.",
        "[MATLAB:validators:mustBeNonzeroLengthText] Value must be text with one or more characters.",
        "[MATLAB:validators:mustBeNumeric] Value must be numeric.",
        "[MATLAB:validators:mustBeReal] Value must be real.",
        "[MATLAB:validators:mustBeScalarOrEmpty] Value must be scalar or empty.",
        "[MATLAB:validators:mustBeText] Value must be a character vector, string array, or cell array of character vectors.",
        "[MATLAB:validators:mustBeTextScalar] Value must be a character vector or string scalar.",
        "[MATLAB:validators:mustBeUnderlyingType] Value must have underlying type 'single'.",
        "[MATLAB:validators:mustBeValidVariableName] The following are not valid variable names: '1x'.",
        "[MATLAB:validators:mustBeVector] Value must be a 1-by-n vector or an n-by-1 vector.",
        "[MATLAB:validators:mustBeFile] The following files do not exist: 'nope.txt'.",
        "",
    ];
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g, w);
    }
}

#[test]
fn input_parser_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "p = inputParser;\naddRequired(p, 'x', @isnumeric);\naddOptional(p, 'y', 5);\n",
            "addParameter(p, 'Mode', 'fast', @(s) any(strcmp(s, {'fast','slow'})));\nparse(p, 3, 'Mode', 'slow');\n",
            "r = p.Results\nu = p.UsingDefaults\np\n",
            "try, parse(p, 'a'), catch e, disp(e.message), end\n",
            "try, parse(p, 1, 2, 'Mode', 'nope'), catch e, disp(e.message), end\n",
            "try, parse(p, 1, 2, 'Bad', 1), catch e, disp(e.message), end\n",
            "p.KeepUnmatched = true; parse(p, 1, 'Bad', 1); um = p.Unmatched\n",
        )),
        [
            "r = struct with fields:\n\n    Mode: 'slow'\n       x: 3\n       y: 5",
            "u = 1×1 cell\n\n    {'y'}",
            "p = \n  inputParser with properties:\n\n       FunctionName: ''\n      CaseSensitive: 0\n      KeepUnmatched: 0\n    PartialMatching: 1\n       StructExpand: 1\n         Parameters: {'Mode'  'x'  'y'}\n            Results: [1×1 struct]\n          Unmatched: [1×1 struct]\n      UsingDefaults: {'y'}",
            "The value of 'x' is invalid. It must satisfy the function: isnumeric.\n",
            "The value of 'Mode' is invalid. It must satisfy the function: @(s)any(strcmp(s,{'fast','slow'})).\n",
            "'Bad' is not a recognized parameter. For a list of valid name-value pair arguments, see the documentation for this function.\n",
            "um = struct with fields:\n\n    Bad: 1",
        ]
    );
}

#[test]
fn error_state_names_and_cleanup() {
    assert_eq!(
        shown(concat!(
            "try, error('my:id', 'boom %d', 3), catch, end\n[m, id] = lasterr\n",
            "warning('w:id', 'careful'); [wm, wid] = lastwarn\n",
            "try, try, error('a:b', 'inner'), catch e, rethrow(e), end, catch e2, disp(e2.identifier), end\n",
            "v1 = matlab.lang.makeValidName({'1abc', 'a b', 'for', 'ok', '_x', ''})\n",
            "v2 = matlab.lang.makeUniqueStrings({'a','a','b','a'})\nv3 = matlab.lang.makeUniqueStrings('a', {'a','a_1'})\n",
            "c = namedargs2cell(struct('A', 1, 'B', 'x'))\nvc = validatecolor(\"red\")\nvc3 = validatecolor(uint8([255 0 0]))\n",
            "f()\n",
            "function f()\n  c = onCleanup(@() disp('bye'));\n  disp('in f');\nend\n",
        )),
        [
            "m = 'boom 3'",
            "id = 'my:id'",
            "Warning: careful",
            "wm = 'careful'",
            "wid = 'w:id'",
            "a:b\n",
            "v1 = 1×6 cell\n\n    {'x1abc'}    {'aB'}    {'xFor'}    {'ok'}    {'x_x'}    {'x'}",
            "v2 = 1×4 cell\n\n    {'a'}    {'a_1'}    {'b'}    {'a_2'}",
            "v3 = 'a_2'",
            "c = 1×4 cell\n\n    {'A'}    {[1]}    {'B'}    {'x'}",
            "vc = 1×3\n\n     1     0     0",
            "vc3 = 1×3\n\n     1     0     0",
            "in f\n",
            "bye\n",
        ]
    );
}
