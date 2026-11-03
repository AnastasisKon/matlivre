//! timeseries, tscollection and tsdata.event against MATLAB R2023b (Live Editor output and
//! `disp` of the results).

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

#[test]
fn construction_and_display_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ts = timeseries([1;2;3;4], [0 1 2 3])\nc = class(ts)\nn = ts.Length\nts.Name = 'speed';\nu = ts.TimeInfo.Units\n",
            "r = ts.TimeInfo\nd = ts.DataInfo\ne = tsdata.event('Peak', 2)\n",
            "tr = timeseries([1 2 3 4], [0 1 2 3]); s = size(tr.Data)\nf = tr.IsTimeFirst\n",
        )),
        [
            "  timeseries\n\n  Common Properties:\n            Name: 'unnamed'\n            Time: [4x1 double]\n        TimeInfo: tsdata.timemetadata\n            Data: [4x1 double]\n        DataInfo: tsdata.datametadata\n",
            "c = 'timeseries'",
            "n = 4",
            "u = 'seconds'",
            concat!(
                "  tsdata.timemetadata\n  Package: tsdata\n\n  Uniform Time:\n    Length       4\n    Increment    1 seconds\n\n  Time Range:\n    Start        0 seconds\n",
                "    End          3 seconds\n\n  Common Properties:\n          Units: 'seconds'\n         Format: ''\n      StartDate: ''\n"
            ),
            "  tsdata.datametadata\n  Package: tsdata\n\n  Common Properties:\n               Units: ''\n       Interpolation: linear (tsdata.interpolation)\n",
            "    EventData: []\n         Name: 'Peak'\n         Time: 2\n        Units: 'seconds'\n    StartDate: ''\n",
            "s = 1×3\n\n     1     1     4",
            "f = logical\n   0",
        ]
    );
}

#[test]
fn samples_and_resampling_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ts = timeseries([1;2;3;4], [0 1 2 3]);\nr = resample(ts, [0 0.5 1 1.5]); rd = r.Data'\ng = getdatasamples(ts, [2 3])'\n",
            "s = getsamples(ts, 2:3); sd = s.Data'\ngs = getsampleusingtime(ts, 1, 2); gsd = gs.Data'\n",
            "ts3 = addsample(ts, 'Data', 9, 'Time', 5); a3 = ts3.Data'\nts4 = delsample(ts, 'Index', 1); a4 = ts4.Data'\n",
            "im = getinterpmethod(ts)\nts5 = setinterpmethod(ts, 'zoh'); r5 = resample(ts5, [0.5 1.5]); r5d = r5.Data'\n",
            "q = getdatasamplesize(ts)\nm = mean(ts)\nmx = max(ts)\nsd2 = std(timeseries([1;2;3;4]))\nm9 = mean(timeseries([1 10; 2 20], [0 1]))\n",
            "p = ts + ts; pd = p.Data'\nts7 = setuniformtime(ts, 'StartTime', 10, 'Interval', 2); t7 = ts7.Time'\n",
        )),
        [
            "rd = 1×4\n\n    1.0000    1.5000    2.0000    2.5000",
            "g = 1×2\n\n     2     3",
            "sd = 1×2\n\n     2     3",
            "gsd = 1×2\n\n     2     3",
            "a3 = 1×5\n\n     1     2     3     4     9",
            "a4 = 1×3\n\n     2     3     4",
            "im = 'linear'",
            "r5d = 1×2\n\n     1     2",
            "q = 1×2\n\n     1     1",
            "m = 2.5000",
            "mx = 4",
            "sd2 = 1.2910",
            "m9 = 1×2\n\n    1.5000   15.0000",
            "pd = 1×4\n\n     2     4     6     8",
            "t7 = 1×4\n\n    10    12    14    16",
        ]
    );
}

#[test]
fn events_times_and_filters_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ts = timeseries([1;2;3;4;3;2;1;0], 0:7);\nf = idealfilter(ts, [0 0.2], 'pass'); fd = f.Data'\n",
            "ts.TimeInfo.StartDate = '01-Jan-2020'; a = getabstime(ts); a2 = a(1:2)'\n",
            "ts2 = setabstime(timeseries([1;2]), {'01-Jan-2020 00:00:00', '01-Jan-2020 00:00:10'}); t2 = ts2.Time'\nsd = ts2.TimeInfo.StartDate\n",
            "ts = timeseries([1;2;3;4], 0:3); ts = addevent(ts, 'E', 1.5); ts = addevent(ts, 'E', 2.5); ne = numel(ts.Events)\n",
            "g = gettsafteratevent(ts, 'E'); gd = g.Data'\ng2 = gettsafterevent(ts, 'E', 2); g2d = g2.Data'\ng3 = gettsbeforeatevent(ts, 'E'); g3d = g3.Data'\n",
            "g4 = gettsbetweenevents(ts, 'E', 'E'); s4 = size(g4.Data)\nd8 = delevent(ts, 'E'); n8 = numel(d8.Events)\n",
            "TT = timeseries2timetable(timeseries([1;2], [0 1])); vn = TT.Properties.VariableNames\n",
        )),
        [
            "fd = 1×8\n\n   -1.2071         0    1.2071    1.7071    1.2071         0   -1.2071   -1.7071",
            "a2 = 1×2 cell\n\n    {'01-Jan-2020 00:00:00'}    {'01-Jan-2020 00:00:01'}",
            "t2 = 1×2\n\n     0    10",
            "sd = '01-Jan-2020 00:00:00'",
            "ne = 2",
            "gd = 1×2\n\n     3     4",
            "g2d = 4",
            "g3d = 1×2\n\n     1     2",
            "s4 = 1×2\n\n     0     0",
            "n8 = 0",
            "vn = 1×1 cell\n\n    {'Data'}",
        ]
    );
}

#[test]
fn collections_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ts = timeseries([1;2;3;4], [0 1 2 3], 'Name', 'speed');\ntc = tscollection({ts, timeseries([5;6;7;8], [0 1 2 3], 'Name', 'b')})\n",
            "names = gettimeseriesnames(tc)\ntc2 = removets(tc, 'b'); n2 = gettimeseriesnames(tc2)\n",
            "tc3 = addts(tc2, timeseries([1;1;1;1], [0 1 2 3], 'Name', 'z')); n3 = gettimeseriesnames(tc3)\nzd = tc3.z.Data'\n",
            "tc4 = addsampletocollection(tc2, 'Time', 4, 'speed', 9); d4 = tc4.speed.Data'\ntc5 = settimeseriesnames(tc2, 'speed', 'w'); n5 = gettimeseriesnames(tc5)\n",
        )),
        [
            "Time Series Collection Object: unnamed\n\nTime vector characteristics\n\n      Start time            0 seconds\n      End time              3 seconds\n\nMember Time Series Objects:\n\n      speed\n      b\n\n",
            "names = 1×2 cell\n\n    {'speed'}    {'b'}",
            "n2 = 1×1 cell\n\n    {'speed'}",
            "n3 = 1×2 cell\n\n    {'speed'}    {'z'}",
            "zd = 1×4\n\n     1     1     1     1",
            "d4 = 1×5\n\n     1     2     3     4     9",
            "n5 = 1×1 cell\n\n    {'w'}",
        ]
    );
}
