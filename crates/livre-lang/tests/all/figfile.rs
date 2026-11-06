//! savefig, hgsave, openfig and hgload (figure files), and arrayfun over the graphics objects and strings they use, against MATLAB R2023b.

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
fn save_and_open_lines() {
    check(
        concat!(
            "fn = fullfile(tempdir, 'livre_fig_a.fig');\n",
            "f = figure;\n",
            "l = plot(1:3, [2 4 1], 'r--', 'LineWidth', 2, 'DisplayName', 'data');\n",
            "hold on; plot(1:3, [1 1 3], 'o');\n",
            "title('Tt'); xlabel('xx'); ylabel('yy'); grid on; xlim([0 4]);\n",
            "legend('show');\n",
            "savefig(f, fn);\n",
            "close(f);\n",
            "g = openfig(fn);\n",
            "class(g)\n",
            "ax = g.Children(end);\n",
            "class(ax)\n",
            "numel(ax.Children)\n",
            "ax.Children(end).XData\n",
            "ax.Children(end).YData\n",
            "ax.Children(end).Color\n",
            "ax.Children(end).LineStyle\n",
            "ax.Children(end).LineWidth\n",
            "ax.Children(end).DisplayName\n",
            "ax.Children(1).Marker\n",
            "ax.Children(1).LineStyle\n",
            "ax.Title.String\n",
            "ax.XLabel.String\n",
            "ax.YLabel.String\n",
            "ax.XLim\n",
            "ax.XLimMode\n",
            "ax.YLimMode\n",
            "char(ax.XGrid)\n",
            "isempty(ax.Legend)\n",
            "ax.Legend.String\n",
        ),
        &[
            "ans = 'matlab.ui.Figure'",
            "ans = 'matlab.graphics.axis.Axes'",
            "ans = 2",
            "ans = 1×3\n\n     1     2     3",
            "ans = 1×3\n\n     2     4     1",
            "ans = 1×3\n\n     1     0     0",
            "ans = '--'",
            "ans = 2",
            "ans = 'data'",
            "ans = 'o'",
            "ans = 'none'",
            "ans = 'Tt'",
            "ans = 'xx'",
            "ans = 'yy'",
            "ans = 1×2\n\n     0     4",
            "ans = 'manual'",
            "ans = 'auto'",
            "ans = 'on'",
            "ans = logical\n   0",
            "ans = 1×2 cell\n\n    {'data'}    {'data1'}",
        ],
    );
}

#[test]
fn subplots_bars_surfaces() {
    check(
        concat!(
            "fn = fullfile(tempdir, 'livre_fig_b');\n",
            "f = figure;\n",
            "subplot(2,1,1); bar([3 1 2]); title('top');\n",
            "subplot(2,1,2); surf(peaks(4)); zlabel('zz');\n",
            "savefig(f, fn);\n",
            "exist([fn '.fig'], 'file')\n",
            "g = hgload([fn '.fig']);\n",
            "numel(findobj(g, 'Type', 'axes'))\n",
            "b = findobj(g, 'Type', 'bar');\n",
            "b.YData\n",
            "s = findobj(g, 'Type', 'surface');\n",
            "size(s.ZData)\n",
            "s.ZData(2,3)\n",
            "t = findobj(g, 'Type', 'axes');\n",
            "sort(arrayfun(@(a) string(a.Title.String), t))'\n",
        ),
        &[
            "ans = 2",
            "ans = 2",
            "ans = 1×3\n\n     3     1     2",
            "ans = 1×2\n\n     4     4",
            "ans = -0.2729",
            "ans = 1×2 string array\n\n    \"\"    \"top\"",
        ],
    );
}

#[test]
fn images_text_colormap() {
    check(
        concat!(
            "fn = fullfile(tempdir, 'livre_fig_c.fig');\n",
            "f = figure; imagesc(magic(3)); colormap(f, gray(4)); text(1, 2, 'here', 'Color', 'r');\n",
            "hgsave(f, fn);\n",
            "g = openfig(fn, 'new', 'invisible');\n",
            "size(g.Colormap)\n",
            "im = findobj(g, 'Type', 'image');\n",
            "im.CData\n",
            "im.CDataMapping\n",
            "tx = findobj(g, 'Type', 'text');\n",
            "tx.String\n",
            "tx.Color\n",
            "tx.Position\n",
            "ax = findobj(g, 'Type', 'axes');\n",
            "ax.YDir\n",
            "ax.XLim\n",
            "try, openfig(fullfile(tempdir, 'livre_missing.fig')), catch e, disp(startsWith(e.message, 'Unable to find file or directory')), end\n",
        ),
        &[
            "ans = 1×2\n\n     4     3",
            "ans = 3×3\n\n     8     1     6\n     3     5     7\n     4     9     2",
            "ans = 'scaled'",
            "ans = 'here'",
            "ans = 1×3\n\n     1     0     0",
            "ans = 1×3\n\n     1     2     0",
            "ans = 'reverse'",
            "ans = 1×2\n\n    0.5000    3.5000",
            "   1\n",
        ],
    );
}

#[test]
fn arrayfun_over_handles_and_strings() {
    check(
        concat!(
            "s = arrayfun(@(k) string(k), 1:3)\n",
            "class(s)\n",
            "c = arrayfun(@(k) \"x\" + k, [1; 2])\n",
            "f = figure; subplot(2,1,1); title('top'); subplot(2,1,2); title('bottom');\n",
            "t = findobj(f, 'Type', 'axes');\n",
            "n = sort(arrayfun(@(a) numel(a.Title.String), t))'\n",
        ),
        &[
            "s = 1×3 string array\n\n    \"1\"    \"2\"    \"3\"",
            "ans = 'string'",
            "c = 2×1 string array\n\n    \"x1\"\n    \"x2\"",
            "n = 1×2\n\n     3     6",
        ],
    );
}
