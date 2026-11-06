//! copyobj (lines, text, axes and figures into new parents) and graphics handle concatenation against MATLAB R2023b.

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
fn copy_line_and_axes() {
    check(
        concat!(
            "f1 = figure; ax1 = axes(f1);\n",
            "l = plot(ax1, 1:3, [2 4 1], 'r--', 'LineWidth', 2);\n",
            "title(ax1, 'Src'); xlabel(ax1,'xx');\n",
            "f2 = figure; ax2 = axes(f2);\n",
            "l2 = copyobj(l, ax2)\n",
            "isequal(l2, l)\n",
            "l2.Parent == ax2\n",
            "l2.LineWidth\n",
            "l2.XData\n",
            "numel(ax2.Children)\n",
            "numel(ax1.Children)\n",
            "ax3 = copyobj(ax1, f2);\n",
            "class(ax3)\n",
            "ax3.Title.String\n",
            "ax3.XLabel.String\n",
            "numel(f2.Children)\n",
            "numel(ax3.Children)\n",
            "ax3.Children.Color\n",
        ),
        &[
            "l2 = \n  Line with properties:\n\n              Color: [1 0 0]\n          LineStyle: '--'\n          LineWidth: 2\n             Marker: 'none'\n         MarkerSize: 6\n    MarkerFaceColor: 'none'\n              XData: [1 2 3]\n              YData: [2 4 1]\n\n  Use GET to show all properties",
            "ans = logical\n   0",
            "ans = logical\n   1",
            "ans = 2",
            "ans = 1×3\n\n     1     2     3",
            "ans = 1",
            "ans = 1",
            "ans = 'matlab.graphics.axis.Axes'",
            "ans = 'Src'",
            "ans = 'xx'",
            "ans = 2",
            "ans = 1",
            "ans = 1×3\n\n     1     0     0",
        ],
    );
}

#[test]
fn copy_pairs_order_and_figures() {
    check(
        concat!(
            "f1 = figure; ax1 = axes(f1);\n",
            "l = plot(ax1, 1:3, [2 4 1]); hold(ax1,'on'); m = plot(ax1, 1:3, [5 5 5]); title(ax1, 'Src');\n",
            "f2 = figure; ax2 = axes(f2);\n",
            "hs = copyobj([l m], ax2);\n",
            "size(hs)\n",
            "[ax2.Children(1) == hs(1), ax2.Children(2) == hs(2)]\n",
            "hs(1).Color, hs(2).Color\n",
            "h = copyobj(l, [ax1; ax2]);\n",
            "size(h)\n",
            "numel(ax1.Children)\n",
            "h2 = copyobj([l m], [ax1 ax2]);\n",
            "h2(1).YData\n",
            "h2(2).YData\n",
            "h2(2).Parent == ax2\n",
            "f3 = copyobj(f1, groot);\n",
            "class(f3)\n",
            "f3.Number\n",
            "numel(f3.Children)\n",
            "f3.Children.Title.String\n",
            "f3 == f1\n",
            "gcf == f3\n",
            "t = text(ax1, 1, 2, 'hi');\n",
            "t2 = copyobj(t, ax2);\n",
            "t2.String\n",
        ),
        &[
            "ans = 1×2\n\n     2     1",
            "ans = 1×2 logical array\n\n   1   1",
            "ans = 1×3\n\n         0    0.4470    0.7410",
            "ans = 1×3\n\n    0.8500    0.3250    0.0980",
            "ans = 1×2\n\n     2     1",
            "ans = 3",
            "ans = 1×3\n\n     2     4     1",
            "ans = 1×3\n\n     5     5     5",
            "ans = logical\n   1",
            "ans = 'matlab.ui.Figure'",
            "ans = 3",
            "ans = 1",
            "ans = 'Src'",
            "ans = logical\n   0",
            "ans = logical\n   1",
            "ans = 'hi'",
        ],
    );
}

#[test]
fn copyobj_errors() {
    check(
        concat!(
            "f1 = figure; ax1 = axes(f1);\n",
            "l = plot(ax1, 1:3, [2 4 1]); hold(ax1,'on'); m = plot(ax1, 1:3, [5 5 5]);\n",
            "f2 = figure; ax2 = axes(f2);\n",
            "try, copyobj([l l], ax2), catch e, disp(e.message), end\n",
            "try, copyobj(l, l), catch e, disp(e.message), end\n",
            "try, copyobj(ax1, ax2), catch e, disp(e.message), end\n",
            "try, copyobj(1, ax2), catch e, disp(e.message), end\n",
            "try, copyobj(l), catch e, disp(e.message), end\n",
            "try, copyobj([l m], [ax1 ax2 ax2]), catch e, disp(e.message), end\n",
            "try, copyobj(l, f2), catch e, disp(e.message), end\n",
            "try, copyobj(ax1, groot), catch e, disp(e.message), end\n",
        ),
        &[
            "Cannot copy the same object more than once to the same parent. Try calling COPYOBJ twice instead.\n",
            "Line cannot be a child of Line.\n",
            "Axes cannot be a child of Axes.\n",
            "Axes cannot be a parent.\n",
            "Not enough input arguments.\n",
            "Parent and child input vectors must be equal length.\n",
            "Line cannot be a child of Figure.\n",
            "Axes cannot be a child of Root.\n",
        ],
    );
}

#[test]
fn concatenate_handles() {
    check(
        concat!(
            "f = figure; ax = axes(f);\n",
            "l = plot(ax, 1:3); hold(ax,'on'); m = plot(ax, 3:-1:1); t = text(ax, 1, 1, 'a');\n",
            "h = [l m]; class(h), size(h)\n",
            "h = [l; m; l]; size(h)\n",
            "h = [l t]; class(h)\n",
            "h = [ax l]; class(h)\n",
            "h = [l gobjects(0)]; class(h), size(h)\n",
            "h = [l m]\n",
        ),
        &[
            "ans = 'matlab.graphics.chart.primitive.Line'",
            "ans = 1×2\n\n     1     2",
            "ans = 1×2\n\n     3     1",
            "ans = 'matlab.graphics.primitive.Data'",
            "ans = 'matlab.graphics.primitive.world.Group'",
            "ans = 'matlab.graphics.chart.primitive.Line'",
            "ans = 1×2\n\n     1     1",
            "h = \n  1×2 Line array:\n\n    Line    Line",
        ],
    );
}
