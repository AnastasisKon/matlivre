//! Timetables against MATLAB R2023b (Live Editor output).

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

const TT: &str = "    Time     Var1    Var2\n    _____    ____    ____\n\n";

#[test]
fn construction_and_display_match_matlab() {
    assert_eq!(
        shown(concat!(
            "Time = seconds(1:3)';\nTT = timetable(Time, [1;2;3], [4;5;6])\n",
            "TT2 = timetable([10;20;30], 'RowTimes', datetime(2024,1,1:3)')\n",
            "TT3 = timetable([1;2;3], 'TimeStep', seconds(2))\nTT4 = timetable([1;2], 'SampleRate', 10)\n",
            "c = class(TT)\nA = array2timetable(magic(3), 'RowTimes', hours(1:3)')\n",
            "T = timetable2table(TT); TT5 = table2timetable(T); same = isequal(TT, TT5)\nct = class(T)\n",
            "i = [istimetable(TT) istimetable(T) istable(TT) istabular(TT)]\n",
        )),
        [
            format!("TT = 3×2 timetable\n\n{TT}    1 sec     1       4  \n    2 sec     2       5  \n    3 sec     3       6  "),
            "TT2 = 3×1 timetable\n\n       Time        Var1\n    ___________    ____\n\n    01-Jan-2024     10 \n    02-Jan-2024     20 \n    03-Jan-2024     30 ".into(),
            "TT3 = 3×1 timetable\n\n      Time      Var1\n    ________    ____\n\n    00:00:00     1  \n    00:00:02     2  \n    00:00:04     3  ".into(),
            "TT4 = 2×1 timetable\n\n     Time      Var1\n    _______    ____\n\n    0 sec       1  \n    0.1 sec     2  ".into(),
            "c = 'timetable'".into(),
            "A = 3×3 timetable\n\n    Time    Var1    Var2    Var3\n    ____    ____    ____    ____\n\n    1 hr     8       1       6  \n    2 hr     3       5       7  \n    3 hr     4       9       2  ".into(),
            "same = logical\n   1".into(),
            "ct = 'table'".into(),
            "i = 1×4 logical array\n\n   1   0   0   1".into(),
        ]
    );
}

#[test]
fn subscripts_match_matlab() {
    assert_eq!(
        shown(concat!(
            "TT = timetable(seconds(1:3)', [1;2;3], [4;5;6]);\n",
            "s2 = TT(seconds(2),:)\ntr = timerange(seconds(1), seconds(3))\ns3 = TT(tr,:)\n",
            "vt = vartype('numeric')\nwt = withtol(seconds(2.05), seconds(0.1))\ns5 = TT(wt, :)\n",
            "t = TT.Time'\ndn = TT.Properties.DimensionNames\n",
            "tr2 = timerange(datetime(2024,1,1), datetime(2024,1,3))\n",
        )),
        [
            format!("s2 = 1×2 timetable\n\n{TT}    2 sec     2       5  "),
            "tr = \n\ttimetable timerange subscript:\n\n\t\tSelect timetable rows with times in the half-open interval:\n\t\t[1 sec, 3 sec)".into(),
            format!("s3 = 2×2 timetable\n\n{TT}    1 sec     1       4  \n    2 sec     2       5  "),
            "vt = \n\ttable vartype subscript:\n\n\t\tSelect table variables matching the type 'numeric'".into(),
            "wt = \n\ttimetable withtol subscript:\n\n\t\tSelect timetable rows matching the following times:\n\n\t\t2.05 sec\n\t\twith tolerance of +/- 0.1 sec".into(),
            format!("s5 = 1×2 timetable\n\n{TT}    2 sec     2       5  "),
            "t = 1×3 duration array\n\n   1 sec   2 sec   3 sec".into(),
            "dn = 1×2 cell\n\n    {'Time'}    {'Variables'}".into(),
            "tr2 = \n\ttimetable timerange subscript:\n\n\t\tSelect timetable rows with times in the half-open interval:\n\t\t[01-Jan-2024 00:00:00, 03-Jan-2024 00:00:00)".into(),
        ]
    );
}

#[test]
fn retime_synchronize_and_lag_match_matlab() {
    assert_eq!(
        shown(concat!(
            "TT = timetable(seconds(1:3)', [1;2;3], [4;5;6]);\n",
            "R = retime(TT, seconds(0:1:4)', 'linear')\nR2 = retime(TT, seconds(0:1:4)', 'previous')\n",
            "TT2 = timetable([10;20;30], 'RowTimes', datetime(2024,1,1:3)');\nR3 = retime(TT2, 'weekly', 'sum')\n",
            "S = synchronize(TT, timetable(seconds([1.5;2.5]), [7;8], 'VariableNames', {'B'}))\nL = lag(TT, 1)\n",
        )),
        [
            format!("R = 5×2 timetable\n\n{TT}    0 sec     0       3  \n    1 sec     1       4  \n    2 sec     2       5  \n    3 sec     3       6  \n    4 sec     4       7  "),
            format!("R2 = 5×2 timetable\n\n{TT}    0 sec    NaN     NaN \n    1 sec      1       4 \n    2 sec      2       5 \n    3 sec      3       6 \n    4 sec      3       6 "),
            "R3 = 1×1 timetable\n\n       Time        Var1\n    ___________    ____\n\n    31-Dec-2023     60 ".into(),
            "S = 5×3 timetable\n\n     Time      Var1    Var2     B \n    _______    ____    ____    ___\n\n    1 sec        1       4     NaN\n    1.5 sec    NaN     NaN       7\n    2 sec        2       5     NaN\n    2.5 sec    NaN     NaN       8\n    3 sec        3       6     NaN".into(),
            format!("L = 3×2 timetable\n\n{TT}    1 sec    NaN     NaN \n    2 sec      1       4 \n    3 sec      2       5 "),
        ]
    );
}

#[test]
fn editing_sorting_and_concatenation_match_matlab() {
    assert_eq!(
        shown(concat!(
            "TT = timetable(seconds(1:3)', [1;2;3], [4;5;6]);\n",
            "so = sortrows(TT, 'Var1', 'descend')\nTT.Var3 = [7;8;9];\nTT.Time(2) = seconds(5);\nTT\n",
            "TT6 = timetable(datetime(2024,1,[3 1 2])', [3;1;2], 'VariableNames', {'X'});\nTT6 = sortrows(TT6)\n",
            "x = TT6(timerange(datetime(2024,1,1), datetime(2024,1,3)), :)\nU = [TT6; TT6(1,:)]\n",
        )),
        [
            format!("so = 3×2 timetable\n\n{TT}    3 sec     3       6  \n    2 sec     2       5  \n    1 sec     1       4  "),
            "TT = 3×3 timetable\n\n    Time     Var1    Var2    Var3\n    _____    ____    ____    ____\n\n    1 sec     1       4       7  \n    5 sec     2       5       8  \n    3 sec     3       6       9  ".into(),
            "TT6 = 3×1 timetable\n\n       Time        X\n    ___________    _\n\n    01-Jan-2024    1\n    02-Jan-2024    2\n    03-Jan-2024    3".into(),
            "x = 2×1 timetable\n\n       Time        X\n    ___________    _\n\n    01-Jan-2024    1\n    02-Jan-2024    2".into(),
            "U = 4×1 timetable\n\n       Time        X\n    ___________    _\n\n    01-Jan-2024    1\n    02-Jan-2024    2\n    03-Jan-2024    3\n    01-Jan-2024    1".into(),
        ]
    );
}

#[test]
fn ranges_and_regularity_match_matlab() {
    assert_eq!(
        shown(concat!(
            "TT = timetable(seconds([1;2;4]), [1;2;3]);\n[tf, dt] = isregular(TT); disp(tf); disp(dt)\n",
            "disp([containsrange(TT, timerange(seconds(1), seconds(3))) containsrange(TT, timerange(seconds(0), seconds(3)))])\n",
            "disp([withinrange(TT, timerange(seconds(0), seconds(5))) withinrange(TT, timerange(seconds(0), seconds(4)))])\n",
            "disp([overlapsrange(TT, timerange(seconds(3), seconds(10))) overlapsrange(TT, timerange(seconds(5), seconds(10)))])\n",
            "f = [tempname '.csv']; writetimetable(TT, f); R = readtimetable(f);\n",
        )),
        [
            "   0\n",
            "   NaN\n",
            "   1   0\n",
            "   1   0\n",
            "   1   0\n",
            "Error: Expected a datetime or duration variable to use as RowTimes. Check that the data can be converted to datetime or duration, or supply a \"TimeStep\" or \"SampleRate\".",
        ]
    );
}
