//! message objects: getString, string, catalog errors, and messages in error, warning and
//! MException, against MATLAB R2023b.

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
fn message_objects_and_errors() {
    check(
        concat!(
            "m = message('MATLAB:UndefinedFunction', 'foo')\n",
            "s = getString(m)\n",
            "m2 = message('MATLAB:nomem')\n",
            "getString(m2)\n",
            "string(message('MATLAB:minrhs'))\n",
            "getString(message('MATLAB:noSuchMethodOrField', 'Q', 'double'))\n",
            "getString(message('MATLAB:nonExistentField', \"abc\"))\n",
            "getString(message('MATLAB:UndefinedFunction', 7))\n",
            "getString(message('MATLAB:UndefinedFunction', int8(-4)))\n",
            "m3 = message('MATLAB:UndefinedFunction', 7, \"x\")\n",
            "m3.Identifier, class(m3.Arguments), class(m3.Arguments{2})\n",
            "class(m3)\n",
            "try, error(message('MATLAB:minrhs')), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, error(message('MATLAB:nonExistentField', 'abc')), catch e, disp(e.identifier), disp(e.message), end\n",
            "e = MException(message('MATLAB:UndefinedFunction', 'q')); e.identifier, e.message\n",
            "warning(message('MATLAB:nonExistentField', 'zz'))\n",
            "[msg, id] = lastwarn\n",
        ),
        &[
            "m = \n  message with properties:\n\n    Identifier: 'MATLAB:UndefinedFunction'\n     Arguments: {'foo'}",
            "s = 'foo'",
            "m2 = \n  message with properties:\n\n    Identifier: 'MATLAB:nomem'\n     Arguments: {}",
            "ans = 'Out of memory.'",
            "ans = \"Not enough input arguments.\"",
            "ans = 'Unrecognized method, property, or field 'Q' for class 'double'.'",
            "ans = 'Unrecognized field name \"abc\".'",
            "ans = '7'",
            "ans = '-4'",
            "m3 = \n  message with properties:\n\n    Identifier: 'MATLAB:UndefinedFunction'\n     Arguments: {[7]  'x'}",
            "ans = 'MATLAB:UndefinedFunction'",
            "ans = 'cell'",
            "ans = 'char'",
            "ans = 'message'",
            "MATLAB:minrhs\n",
            "Not enough input arguments.\n",
            "MATLAB:nonExistentField\n",
            "Unrecognized field name \"abc\".\n",
            "ans = 'MATLAB:UndefinedFunction'",
            "ans = 'q'",
            "Warning: Unrecognized field name \"zz\".",
            "msg = 'Unrecognized field name \"zz\".'",
            "id = 'MATLAB:nonExistentField'",
        ],
    );
}

#[test]
fn message_catalog_errors() {
    check(
        concat!(
            "try, message('Foo:bar'), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, message('nocolon'), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, message(), catch e, disp(e.message), end\n",
            "try, message(3), catch e, disp(e.message), end\n",
            "m = message('MATLAB:nonExistentFieldX')\n",
            "try, getString(m), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, getString(message('MATLAB:minrhs', 'extra')), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, getString(message('MATLAB:validators:mustBeMember')), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, getString(message('MATLAB:UndefinedFunction', 2.5)), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, getString(message('MATLAB:UndefinedFunction', true)), catch e, disp(e.identifier), disp(e.message), end\n",
            "try, getString(message('MATLAB:UndefinedFunction', [1 2])), catch e, disp(e.identifier), disp(e.message), end\n",
        ),
        &[
            "MATLAB:builtins:InvalidMessageID\n",
            "Invalid Message ID format: 'Foo:bar'.\n",
            "MATLAB:builtins:InvalidMessageID\n",
            "Invalid Message ID format: 'nocolon'.\n",
            "Invalid constructor signature.\n",
            "Invalid input for argument 1 (rhs1):\nValue must be a character vector or a string scalar.\n",
            "m = \n  message with properties:\n\n    Identifier: 'MATLAB:nonExistentFieldX'\n     Arguments: {}",
            "MATLAB:builtins:MessageNotFound\n",
            "Unable to find message key 'nonExistentFieldX' in catalog 'MATLAB'.\n",
            "MATLAB:builtins:IncorrectHoleCount\n",
            "Incorrect number of parameters supplied for 'MATLAB:minrhs'. Expected 0 but found 1.\n",
            "MATLAB:builtins:IncorrectHoleCount\n",
            "Incorrect number of parameters supplied for 'MATLAB:validators:mustBeMember'. Expected 1 but found 0.\n",
            "MATLAB:builtins:NoFloatingPointNumbers\n",
            "Error filling holes for MATLAB:UndefinedFunction. Floating point numbers are not allowed as holes. They should be converted to character vectors.\n",
            "MATLAB:builtins:BadDataType\n",
            "In 'MATLAB:UndefinedFunction', data type supplied is incorrect for parameter {0}.\n",
            "MATLAB:builtins:NonScalarInput\n",
            "In 'MATLAB:UndefinedFunction', parameter {0} must be a real scalar.\n",
        ],
    );
}
