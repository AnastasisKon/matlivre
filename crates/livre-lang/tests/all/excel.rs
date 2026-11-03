//! Spreadsheets and MException against MATLAB R2023b. `data/matlab_written.xlsx` was written by
//! MATLAB (`writetable`, `writematrix` and `xlswrite` into sheets Sheet1, M and W).

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

fn fixture() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/matlab_written.xlsx").display().to_string().replace('\'', "''")
}

#[test]
fn reads_a_workbook_written_by_matlab() {
    let f = fixture();
    assert_eq!(
        shown(&format!(
            "f = '{f}';\ns = sheetnames(f)\nT2 = readtable(f)\nM = readmatrix(f, 'Sheet', 'M')\n[num, txt, raw] = xlsread(f);\nnum\ntxt\nnum2 = xlsread(f, 'M', 'A1:B2')\nR = readcell(f, 'Sheet', 'W')\nM2 = readmatrix(f, 'Sheet', 2, 'Range', 'B2:C3')\n"
        )),
        [
            "s = 3×1 string array\n\n    \"Sheet1\"\n    \"M\"\n    \"W\"",
            "T2 = 3×3 table\n\n    Id    Name     Val\n    __    _____    ___\n\n    1     {'a'}    0.5\n    2     {'b'}    1.5\n    3     {'c'}    2.5",
            "M = 3×3\n\n     8     1     6\n     3     5     7\n     4     9     2",
            "num = 3×3\n\n    1.0000       NaN    0.5000\n    2.0000       NaN    1.5000\n    3.0000       NaN    2.5000",
            "txt = 4×3 cell\n\n    {'Id'    }    {'Name'}    {'Val'   }\n    {0×0 char}    {'a'   }    {0×0 char}\n    {0×0 char}    {'b'   }    {0×0 char}\n    {0×0 char}    {'c'   }    {0×0 char}",
            "num2 = 2×2\n\n     8     1\n     3     5",
            "R = 2×2 cell\n\n    {'x'}    {[1]}\n    {'y'}    {[2]}",
            "M2 = 2×2\n\n     5     7\n     9     2",
        ]
    );
}

#[test]
fn writes_workbooks_that_read_back() {
    let dir = std::env::temp_dir().join(format!("matlivre-xlsx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("out.xlsx").display().to_string().replace('\'', "''");
    let out = shown(&format!(
        concat!(
            "f = '{f}';\nT = table([1;2;3], {{'a';'b';'c'}}, [0.5;1.5;2.5], 'VariableNames', {{'Id','Name','Val'}});\n",
            "writetable(T, f); writematrix(magic(3), f, 'Sheet', 'M'); writecell({{'x', 1; 'y', 2}}, f, 'Sheet', 'W', 'Range', 'B2');\n",
            "xlswrite(f, [10 20; 30 40], 'X');\ns = sheetnames(f)'\nT2 = readtable(f)\nM = readmatrix(f, 'Sheet', 'M')\nR = readcell(f, 'Sheet', 'W')\n",
        ),
        f = f
    ));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        out,
        [
            "Warning: Added specified worksheet.",
            "s = 1×4 string array\n\n    \"Sheet1\"    \"M\"    \"W\"    \"X\"",
            "T2 = 3×3 table\n\n    Id    Name     Val\n    __    _____    ___\n\n    1     {'a'}    0.5\n    2     {'b'}    1.5\n    3     {'c'}    2.5",
            "M = 3×3\n\n     8     1     6\n     3     5     7\n     4     9     2",
            "R = 2×2 cell\n\n    {'x'}    {[1]}\n    {'y'}    {[2]}",
        ]
    );
}

#[test]
fn mexception_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "ME = MException('my:id', 'Value %d is bad', 3)\nc = class(ME)\n",
            "try, throw(ME), catch e, disp(e.message), disp(class(e)), end\n",
            "ME2 = addCause(ME, MException('inner:id', 'inner problem')); nc = numel(ME2.cause)\nr = getReport(MException('a:b', 'boom'))\n",
            "v = [1 2]; try, x = v(3); catch e3, i3 = e3.identifier, end\n",
            "try, undefinedThing(1), catch e4, i4 = e4.identifier, end\n",
            "try, [1 2] + [1 2 3], catch e5, i5 = e5.identifier, end\n",
        )),
        [
            "ME = \n  MException with properties:\n\n    identifier: 'my:id'\n       message: 'Value 3 is bad'\n         cause: {}\n         stack: [0×1 struct]\n    Correction: []",
            "c = 'MException'",
            "Value 3 is bad\n",
            "MException\n",
            "nc = 1",
            "r = 'boom'",
            "i3 = 'MATLAB:badsubscript'",
            "i4 = 'MATLAB:UndefinedFunction'",
            "i5 = 'MATLAB:sizeDimensionsMustMatch'",
        ]
    );
}
