//! Preferences, loadenv, formattedDisplayText, matfile, saveVariablesToScript and text import
//! options against MATLAB R2023b (Live Editor output).

use livre_lang::{Interpreter, OutputKind};

fn shown(dir: &std::path::Path, src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    // Files are named relative to `dir` through the `D:` prefix (tests run in parallel, so
    // the process's current folder is left alone).
    let d = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR).replace('\'', "''");
    let _ = it.run_source(&src.replace("D:", &d));
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

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("matlivre-io-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn preferences_match_matlab() {
    let dir = scratch("prefs");
    std::env::set_var("MATLIVRE_PREFDIR", &dir);
    let out = shown(
        &dir,
        concat!(
            "if ispref('mlgrp'), rmpref('mlgrp'); end\naddpref('mlgrp', 'alpha', 5)\na = getpref('mlgrp', 'alpha')\n",
            "setpref('mlgrp', {'beta','gamma'}, {'b', [1 2]})\ns = getpref('mlgrp')\nt = ispref('mlgrp', {'alpha','zzz'})\n",
            "v = getpref('mlgrp', 'zzz', 99)\nrmpref('mlgrp', 'beta'); f = fieldnames(getpref('mlgrp'))'\n",
            "try, addpref('mlgrp', 'alpha', 2), catch e, disp(e.message), end\ntry, rmpref('mlgrp', 'nope'), catch e, disp(e.message), end\n",
            "try, getpref('mlgrp', 'nope'), catch e, disp(e.message), end\ntry, getpref('nogroup_zz', 'x'), catch e, disp(e.message), end\n",
            "rmpref('mlgrp'); e = ispref('mlgrp')\n",
        ),
    );
    assert_eq!(
        out,
        [
            "a = 5",
            "s = struct with fields:\n\n    alpha: 5\n     beta: 'b'\n    gamma: [1 2]",
            "t = 1×2 logical array\n\n   1   0",
            "v = 99",
            "f = 1×3 cell\n\n    {'alpha'}    {'gamma'}    {'zzz'}",
            "A preference with that GROUP and NAME already exists.\n",
            "GROUP and PREF must exist\n",
            "preference nope does not exist in group mlgrp\n",
            "group nogroup_zz does not exist\n",
            "e = logical\n   0",
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn environment_display_text_and_matfiles_match_matlab() {
    let dir = scratch("env");
    std::fs::write(dir.join("tt.env"), "KEY1=hello\n# comment\nKEY2 = \"quoted value\"\nNUM=42\n").unwrap();
    let out = shown(
        &dir,
        concat!(
            "loadenv('D:tt.env'); k = getenv('KEY2')\nD = loadenv('D:tt.env')\n",
            "str = formattedDisplayText(magic(3))\nstr2 = formattedDisplayText(pi)\n",
            "s3 = formattedDisplayText('abc'); n3 = strlength(s3)\ns4 = formattedDisplayText(true, 'UseTrueFalseForLogical', true)\n",
            "x = 1; y = 'abc'; save('D:mf.mat', 'x', 'y'); m = matfile('D:mf.mat'); mx = m.x\nc = class(m)\n",
        ),
    );
    assert_eq!(
        out,
        [
            "k = 'quoted value'",
            "D = \n  dictionary (string --> string) with 3 entries:\n\n    \"KEY1\" --> \"hello\"\n    \"KEY2\" --> \"quoted value\"\n    \"NUM\"  --> \"42\"",
            "str = \"     8     1     6\n     3     5     7\n     4     9     2\n\"",
            "str2 = \"    3.1416\n\"",
            "n3 = 3",
            "s4 = \"    true\n\"",
            "mx = 1",
            "c = 'matlab.io.MatFile'",
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn variables_are_saved_to_scripts_like_matlab() {
    let dir = scratch("script");
    let out = shown(
        &dir,
        concat!(
            "a = 5; e = false; f = {'alpha', 'gamma'}; s = struct('alpha', 5, 'beta', 'b'); t = logical([1 0]); m = [1 2; 3 4];\n",
            "str = string(['ab', newline, 'c']);\nmatlab.io.saveVariablesToScript('D:vs.m');\nclear\nrun('D:vs.m')\nok = isequal(m, [1 2; 3 4]) && isequal(f, {'alpha', 'gamma'}) && s.alpha == 5 && isequal(t, logical([1 0])) && str == \"ab\" + newline + \"c\"\n",
        ),
    );
    assert_eq!(out, ["ok = logical\n   1"]);
    let script = std::fs::read_to_string(dir.join("vs.m")).unwrap();
    assert!(script.contains("f = cell(1, 2);\nf{1} = 'alpha';\nf{2} = 'gamma';\n\nm = [1 2;3 4];\n\ns = struct;\ns.alpha = 5;\ns.beta = 'b';\n"), "{script}");
    assert!(script.contains("str = ['ab', newline, 'c'];\nstr = convertCharsToStrings(str);\n\nt = logical([1 0]);\n"), "{script}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_options_match_matlab() {
    let dir = scratch("opts");
    std::fs::write(dir.join("tt.csv"), "a,b,name\n1,2.5,x\n3,4.5,y\n").unwrap();
    std::fs::write(dir.join("t2.csv"), "a;b;name;flag\n1;;x;true\n3;4.5;y;false\n5;6;;true\n").unwrap();
    let out = shown(
        &dir,
        concat!(
            "opts = detectImportOptions('D:tt.csv')\nopts = setvartype(opts, 'a', 'int32');\nT = readtable('D:tt.csv', opts)\nca = class(T.a)\nvo = getvaropts(opts, 'name')\n",
            "opts = detectImportOptions('D:t2.csv');\nd = opts.Delimiter\nvt = opts.VariableTypes\nvo = getvaropts(opts, 'b')\n",
            "opts = setvaropts(opts, 'b', 'FillValue', -1);\nopts.SelectedVariableNames = {'a', 'b'};\nT = readtable('D:t2.csv', opts)\nM = readmatrix('D:t2.csv', opts)\n",
            "opts2 = setvartype(detectImportOptions('D:t2.csv'), {'name'}, 'string');\nT2 = readtable('D:t2.csv', opts2)\nc = class(opts)\n",
            "opts3 = detectImportOptions('D:t2.csv'); opts3.DataLines = [3 Inf];\nT3 = readtable('D:t2.csv', opts3)\n",
            "p = preview('D:tt.csv', detectImportOptions('D:tt.csv'))\n",
        ),
    );
    let fmt_props = concat!(
        "   Format Properties:\n                    Delimiter: {','}\n                   Whitespace: '\\b\\t '\n                   LineEnding: {'\\n'  '\\r'  '\\r\\n'}\n",
        "                 CommentStyle: {}\n    ConsecutiveDelimitersRule: 'split'\n        LeadingDelimitersRule: 'keep'\n       TrailingDelimitersRule: 'ignore'\n",
        "                EmptyLineRule: 'skip'\n                     Encoding: 'UTF-8'\n\n   Replacement Properties:\n                  MissingRule: 'fill'\n",
        "              ImportErrorRule: 'fill'\n             ExtraColumnsRule: 'addvars'\n\n   Variable Import Properties: Set types by name using setvartype\n",
        "                VariableNames: {'a', 'b', 'name'}\n                VariableTypes: {'double', 'double', 'char'}\n        SelectedVariableNames: {'a', 'b', 'name'}\n",
        "              VariableOptions: [1-by-3 matlab.io.VariableImportOptions] \n\tAccess VariableOptions sub-properties using setvaropts/getvaropts\n",
        "           VariableNamingRule: 'modify'\n\n   Location Properties:\n                    DataLines: [2 Inf]\n            VariableNamesLine: 1\n",
        "               RowNamesColumn: 0\n            VariableUnitsLine: 0\n     VariableDescriptionsLine: 0 \n\tTo display a preview of the table, use preview",
    );
    let varopts_tail = "         QuoteRule: 'remove'\n          Prefixes: {}\n          Suffixes: {}\n    EmptyFieldRule: 'missing'\n";
    assert_eq!(
        out,
        [
            format!("opts = \n  DelimitedTextImportOptions with properties:\n\n{fmt_props}"),
            "T = 2×3 table\n\n    a     b     name \n    _    ___    _____\n\n    1    2.5    {'x'}\n    3    4.5    {'y'}".into(),
            "ca = 'int32'".into(),
            format!("vo = \n  TextVariableImportOptions with properties:\n\n   Variable Properties:\n              Name: 'name'\n              Type: 'char'\n         FillValue: ''\n    TreatAsMissing: {{}}\n{varopts_tail}\n   String Options:\n    WhitespaceRule: 'trim'"),
            "d = 1×1 cell\n\n    {';'}".into(),
            "vt = 1×4 cell\n\n    {'double'}    {'double'}    {'char'}    {'char'}".into(),
            concat!(
                "vo = \n  NumericVariableImportOptions with properties:\n\n   Variable Properties:\n                  Name: 'b'\n                  Type: 'double'\n",
                "             FillValue: NaN\n        TreatAsMissing: {}\n             QuoteRule: 'remove'\n              Prefixes: {}\n              Suffixes: {}\n",
                "        EmptyFieldRule: 'missing'\n\n   Numeric Options:\n     ExponentCharacter: 'eEdD'\n      DecimalSeparator: '.'\n    ThousandsSeparator: ''\n",
                "        TrimNonNumeric: 0\n          NumberSystem: 'decimal'"
            )
            .into(),
            "T = 3×2 table\n\n    a     b \n    _    ___\n\n    1     -1\n    3    4.5\n    5      6".into(),
            "Warning: Different 'VariableOptions' detected for the selected variables. Importing data using options defined in the first selected variable. \n To import the selected variables using different variable options, use readtable".into(),
            "M = 3×2\n\n    1.0000       NaN\n    3.0000    4.5000\n    5.0000    6.0000".into(),
            "T2 = 3×4 table\n\n    a     b       name         flag   \n    _    ___    _________    _________\n\n    1    NaN    \"x\"          {'true' }\n    3    4.5    \"y\"          {'false'}\n    5      6    <missing>    {'true' }".into(),
            "c = 'matlab.io.text.DelimitedTextImportOptions'".into(),
            "T3 = 2×4 table\n\n    a     b        name         flag   \n    _    ___    __________    _________\n\n    3    4.5    {'y'     }    {'false'}\n    5      6    {0×0 char}    {'true' }".into(),
            "p = 2×3 table\n\n    a     b     name \n    _    ___    _____\n\n    1    2.5    {'x'}\n    3    4.5    {'y'}".into(),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
