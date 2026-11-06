//! Data tips: datatip on lines, scatters, surfaces and 3-D lines, snapping, Location, the
//! Children of plotted objects and axes, plus findfigs and handle, against MATLAB R2023b.

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
fn datatip_lines() {
    check(
        concat!(
            "f = figure('Visible','off');\n",
            "h = plot(1:5, [2 4 3 5 1]);\n",
            "dt = datatip(h, 3, 3)\n",
            "class(dt)\n",
            "dt.X, dt.Y, dt.Z, dt.DataIndex, dt.Parent == h, dt.Tag, dt.Type, dt.FontSize\n",
            "dt2 = datatip(h, 'DataIndex', 5)\n",
            "dt3 = datatip(h, 2.2, 3.9)\n",
            "d4 = datatip(h, 'DataIndex', 9); d4.DataIndex\n",
            "d5 = datatip(h); d5.DataIndex\n",
            "g = datatip(h, 3.4, 2.1); g.DataIndex\n",
            "g2 = datatip(h, 3.4, 4.9); g2.DataIndex\n",
            "c = datatip(h, 'DataIndex', 4, 'Location', 'southwest'); c.Location, c.LocationMode\n",
            "k = datatip(h, 2, 4, 'FontSize', 12); k.FontSize\n",
            "numel(h.Children)\n",
            "delete(c); numel(h.Children)\n",
            "ax = gca; numel(ax.Children)\n",
            "dts = findobj(gca, 'Type', 'datatip'); numel(dts)\n",
            "dt.DataIndex = 1; dt.X, dt.Y\n",
        ),
        &[
            "dt = \n  DataTip (X 3, Y 3) with properties:\n\n    DataIndex: 3\n     Location: 'northeast'\n       Parent: [1×1 Line]\n\n  Use GET to show all properties",
            "ans = 'matlab.graphics.datatip.DataTip'",
            "ans = 3",
            "ans = 3",
            "ans = 0",
            "ans = 3",
            "ans = logical\n   1",
            "ans = 'datatip'",
            "ans = 'datatip'",
            "ans = 10",
            "dt2 = \n  DataTip (X 5, Y 1) with properties:\n\n    DataIndex: 5\n     Location: 'northwest'\n       Parent: [1×1 Line]\n\n  Use GET to show all properties",
            "dt3 = \n  DataTip (X 2, Y 4) with properties:\n\n    DataIndex: 2\n     Location: 'northeast'\n       Parent: [1×1 Line]\n\n  Use GET to show all properties",
            "ans = 5",
            "ans = 1",
            "ans = 3",
            "ans = 4",
            "ans = 'southwest'",
            "ans = 'manual'",
            "ans = 12",
            "ans = 9",
            "ans = 8",
            "ans = 1",
            "ans = 8",
            "ans = 1",
            "ans = 2",
        ],
    );
}

#[test]
fn datatip_targets_and_children() {
    check(
        concat!(
            "f = figure('Visible','off');\n",
            "h = plot(1:5, [2 4 3 5 1]);\n",
            "a = datatip(h, 'DataIndex', 4); b = datatip(h, 'DataIndex', 1);\n",
            "h.Children\n",
            "hold on\n",
            "s = scatter([1 2 3], [3 1 2]);\n",
            "d = datatip(s, 2, 1)\n",
            "sf = surf(peaks(5));\n",
            "e = datatip(sf, 2, 3)\n",
            "ln = plot3(1:3, 1:3, [5 6 7]); e2 = datatip(ln, 2, 2)\n",
            "try, datatip(gca, 1, 1), catch err, disp(err.message), end\n",
            "delete(h); isvalid(a)\n",
            "close(f)\n",
            "f = figure('Visible','off');\n",
            "h = plot(0:100, 50*ones(1,101));\n",
            "x = datatip(h, 'DataIndex', 50); x.Location\n",
            "x = datatip(h, 'DataIndex', 95); x.Location\n",
            "h = plot(50*ones(1,101), 0:100);\n",
            "x = datatip(h, 'DataIndex', 50); x.Location\n",
            "x = datatip(h, 'DataIndex', 95); x.Location\n",
            "h = plot(0:100, 0:100);\n",
            "x = datatip(h, 'DataIndex', 98); x.Location\n",
            "close(f)\n",
            "f = figure('Visible','off');\n",
            "b = boxchart([1 2 3 9]); ax = gca; ax.Children\n",
            "l1 = plot(1:3, 'Tag', 'a'); hold on; l2 = plot(3:-1:1, 'DisplayName', 'bb'); l3 = plot(2*ones(1,3));\n",
            "ax.Children\n",
            "findfigs\n",
            "x = handle(f); x == f\n",
        ),
        &[
            "ans = \n  2×1 DataTip array:\n\n  DataTip    (X 1, Y 2)\n  DataTip    (X 4, Y 5)",
            "d = \n  DataTip (X 2, Y 1) with properties:\n\n    DataIndex: 2\n     Location: 'northeast'\n       Parent: [1×1 Scatter]\n\n  Use GET to show all properties",
            "e = \n  DataTip (X 2, Y 3, Z -2.77361) with properties:\n\n    DataIndex: 8\n     Location: 'northeast'\n       Parent: [1×1 Surface]\n\n  Use GET to show all properties",
            "e2 = \n  DataTip (X 2, Y 2, Z 6) with properties:\n\n    DataIndex: 2\n     Location: 'northeast'\n       Parent: [1×1 Line]\n\n  Use GET to show all properties",
            "First argument must be a chart object that supports data tips.\n",
            "ans = logical\n   0",
            "ans = 'northeast'",
            "ans = 'northwest'",
            "ans = 'northeast'",
            "ans = 'southeast'",
            "ans = 'southwest'",
            "ans = \n  BoxChart with properties:\n\n    XData: [1    1    1    1]\n    YData: [1 2 3 9]\n\n  Use GET to show all properties",
            "ans = \n  3×1 Line array:\n\n  Line\n  Line    (bb)\n  Line    (a)",
            "ans = logical\n   1",
        ],
    );
}
