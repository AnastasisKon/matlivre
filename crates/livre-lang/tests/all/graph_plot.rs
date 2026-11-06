//! Graph plots: the GraphPlot object, highlight, labelnode, labeledge and layout, against
//! MATLAB R2023b.

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
fn graph_plot_highlight_and_labels() {
    check(
        concat!(
            "s = [1 1 2 3]; t = [2 3 4 4];\n",
            "G = graph(s,t);\n",
            "h = plot(G, \"Layout\", \"circle\")\n",
            "c = class(h)\n",
            "nc0 = h.NodeColor\n",
            "ec0 = h.EdgeColor\n",
            "ms0 = h.MarkerSize\n",
            "nl0 = h.NodeLabel\n",
            "el0 = h.EdgeLabel\n",
            "lw0 = h.LineWidth\n",
            "mk0 = h.Marker\n",
            "layout(h,'circle'); x1 = h.XData, y1 = h.YData\n",
            "highlight(h,[1 3]); nc1 = h.NodeColor, ms1 = h.MarkerSize\n",
            "highlight(h,[1 2],'NodeColor','g','EdgeColor','r','LineWidth',2);\n",
            "nc2 = h.NodeColor, ec2 = h.EdgeColor, lw2 = h.LineWidth\n",
            "highlight(h,[2 3],[4 4],'EdgeColor','k','LineStyle','--');\n",
            "ec3 = h.EdgeColor, ls3 = h.LineStyle\n",
            "labelnode(h,[1 2],{'a','b'}); nl1 = h.NodeLabel\n",
            "labeledge(h,1,'e1'); el1 = h.EdgeLabel\n",
            "labeledge(h,[1 2],[2 4],[5 6]); el2 = h.EdgeLabel\n",
            "highlight(h,4,'Marker','s','MarkerSize',10); mk1 = h.Marker, ms2 = h.MarkerSize\n",
        ),
        &[
            "h = \n  GraphPlot with properties:\n\n     NodeColor: [0 0.4470 0.7410]\n    MarkerSize: 4\n        Marker: 'o'\n     EdgeColor: [0 0.4470 0.7410]\n     LineWidth: 0.5000\n     LineStyle: '-'\n     NodeLabel: {'1'  '2'  '3'  '4'}\n     EdgeLabel: {}\n         XData: [1 0 -1 0]\n         YData: [0 1 0 -1]\n         ZData: [0 0 0 0]\n\n  Use GET to show all properties",
            "c = 'matlab.graphics.chart.primitive.GraphPlot'",
            "nc0 = 1×3\n\n         0    0.4470    0.7410",
            "ec0 = 1×3\n\n         0    0.4470    0.7410",
            "ms0 = 4",
            "nl0 = 1×4 cell\n\n    {'1'}    {'2'}    {'3'}    {'4'}",
            "el0 =\n\n  0×0 empty cell array",
            "lw0 = 0.5000",
            "mk0 = 'o'",
            "x1 = 1×4\n\n     1     0    -1     0",
            "y1 = 1×4\n\n     0     1     0    -1",
            "nc1 = 1×3\n\n         0    0.4470    0.7410",
            "ms1 = 1×4\n\n     8     4     8     4",
            "nc2 = 4×3\n\n         0    1.0000         0\n         0    1.0000         0\n         0    0.4470    0.7410\n         0    0.4470    0.7410",
            "ec2 = 4×3\n\n    1.0000         0         0\n         0    0.4470    0.7410\n         0    0.4470    0.7410\n         0    0.4470    0.7410",
            "lw2 = 1×4\n\n    2.0000    0.5000    0.5000    0.5000",
            "ec3 = 4×3\n\n    1.0000         0         0\n         0    0.4470    0.7410\n         0         0         0\n         0         0         0",
            "ls3 = 1×4 cell\n\n    {'-'}    {'-'}    {'--'}    {'--'}",
            "nl1 = 1×4 cell\n\n    {'a'}    {'b'}    {'3'}    {'4'}",
            "el1 = 1×4 cell\n\n    {'e1'}    {0×0 char}    {0×0 char}    {0×0 char}",
            "el2 = 1×4 cell\n\n    {'5'}    {0×0 char}    {'6'}    {0×0 char}",
            "mk1 = 1×4 cell\n\n    {'o'}    {'o'}    {'o'}    {'square'}",
            "ms2 = 1×4\n\n     8     4     8    10",
        ],
    );
}

#[test]
fn graph_plot_layouts_and_errors() {
    check(
        concat!(
            "D = digraph([1 1 2 3],[2 3 4 4]);\n",
            "h2 = plot(D);\n",
            "x2 = h2.XData, y2 = h2.YData\n",
            "as = h2.ArrowSize\n",
            "layout(h2,'circle'); x3 = h2.XData, y3 = h2.YData\n",
            "layout(h2,'layered','Direction','right'); x4 = h2.XData, y4 = h2.YData\n",
            "T = digraph([1 1 2 2 3],[2 3 4 5 6]);\n",
            "h3 = plot(T); x5 = h3.XData, y5 = h3.YData\n",
            "G2 = graph({'x','y','z'},{'y','z','x'});\n",
            "h4 = plot(G2,'NodeColor','r','Layout','circle');\n",
            "nl4 = h4.NodeLabel, nc4 = h4.NodeColor, x6 = h4.XData, y6 = h4.YData\n",
            "highlight(h4,{'x','z'},'NodeColor','b'); nc5 = h4.NodeColor\n",
            "h5 = plot(graph([1 2],[2 3],[7 8]),'EdgeLabel',[7 8]); el5 = h5.EdgeLabel\n",
            "h6 = plot(graph([1 2],[2 3]));\n",
            "try, highlight(h6,9), catch e, disp(e.message), end\n",
            "labelnode(h6,[1 2],{'q'}); nl6 = h6.NodeLabel\n",
            "try, layout(h6,'bogus'), catch e, disp(e.message), end\n",
            "try, highlight(h6,1,3), catch e, disp(e.message), end\n",
        ),
        &[
            "x2 = 1×4\n\n    1.5000    1.0000    2.0000    1.5000",
            "y2 = 1×4\n\n     3     2     2     1",
            "as = 7",
            "x3 = 1×4\n\n     1     0    -1     0",
            "y3 = 1×4\n\n     0     1     0    -1",
            "x4 = 1×4\n\n     1     2     2     3",
            "y4 = 1×4\n\n    1.5000    1.0000    2.0000    1.5000",
            "x5 = 1×6\n\n    2.0000    1.5000    3.0000    1.0000    2.0000    3.0000",
            "y5 = 1×6\n\n     3     2     2     1     1     1",
            "nl4 = 1×3 cell\n\n    {'x'}    {'y'}    {'z'}",
            "nc4 = 1×3\n\n     1     0     0",
            "x6 = 1×3\n\n    1.0000   -0.5000   -0.5000",
            "y6 = 1×3\n\n         0    0.8660   -0.8660",
            "nc5 = 3×3\n\n     0     0     1\n     1     0     0\n     0     0     1",
            "el5 = 1×2 cell\n\n    {'7'}    {'8'}",
            "Nodes must be a numeric vector, a vector of node names, or a logical vector of length equal to the number of nodes.\n",
            "nl6 = 1×3 cell\n\n    {'q'}    {'q'}    {'3'}",
            "Expected input to match one of these values:\n\n'auto', 'circle', 'force', 'layered', 'subspace', 'force3', 'subspace3'\n\nThe input, 'bogus', did not match any of the valid values.\n",
            "s and t must specify edges in the graph.\n",
        ],
    );
}
