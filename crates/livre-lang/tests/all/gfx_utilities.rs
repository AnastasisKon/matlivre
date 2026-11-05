//! Graphics utilities (imtile, imformats, orient, objbounds, fontname), against MATLAB R2023b.

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
fn imtile_and_imformats() {
    check(
        concat!(
            "a = uint8(ones(2,3)*10); b = uint8(ones(2,3)*20); c = uint8(ones(2,3)*30);\n",
            "T1 = imtile({a, b, c})\n",
            "T2 = imtile({a, b, c}, 'GridSize', [1 3])\n",
            "T3 = imtile({a, b, c}, 'GridSize', [3 1], 'BorderSize', 1, 'BackgroundColor', 'w');\n",
            "size(T3)\n",
            "T3(:,:,1)\n",
            "f = imformats;\n",
            "size(f)\n",
            "{f.ext}\n",
            "f(1)\n",
            "p = imformats('png')\n",
            "q = imformats('xyz')\n",
        ),
        &[
            "T1 = 4×6 uint8 matrix\n\n   10   10   10   20   20   20\n   10   10   10   20   20   20\n   30   30   30    0    0    0\n   30   30   30    0    0    0",
            "T2 = 2×9 uint8 matrix\n\n   10   10   10   20   20   20   30   30   30\n   10   10   10   20   20   20   30   30   30",
            "ans = 1×3\n\n    12     5     3",
            "ans = 12×5 uint8 matrix\n\n   255   255   255   255   255\n   255    10    10    10   255\n   255    10    10    10   255\n   255   255   255   255   255\n   255   255   255   255   255\n   255    20    20    20   255\n   255    20    20    20   255\n   255   255   255   255   255\n   255   255   255   255   255\n   255    30    30    30   255\n   255    30    30    30   255\n   255   255   255   255   255",
            "ans = 1×2\n\n     1    20",
            "ans = 1×20 cell\n\n    {1×1 cell}    {1×1 cell}    {1×2 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×2 cell}    {1×1 cell}    {1×2 cell}    {1×2 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×1 cell}    {1×2 cell}    {1×1 cell}",
            "ans = struct with fields:\n\n            ext: {'bmp'}\n            isa: @isbmp\n           info: @imbmpinfo\n           read: @readbmp\n          write: @writebmp\n          alpha: 0\n    description: 'Windows Bitmap'",
            "p = struct with fields:\n\n            ext: {'png'}\n            isa: @ispng\n           info: @impnginfo\n           read: @readpng\n          write: @writepng\n          alpha: 1\n    description: 'Portable Network Graphics'",
            "q = 0×0 empty struct array with no fields.",
        ],
    );
}

#[test]
fn orient_objbounds_and_fontname() {
    check(
        concat!(
            "fig = figure('Visible', 'off');\n",
            "o1 = orient(fig)\n",
            "orient(fig, 'landscape');\n",
            "o2 = orient(fig)\n",
            "fig.PaperOrientation\n",
            "ax = axes(fig);\n",
            "plot(ax, [1 2 3], [4 5 1]);\n",
            "ob = objbounds(ax)\n",
            "fontname(fig, 'Arial');\n",
            "ax.FontName\n",
            "fontname(ax, 'Courier');\n",
            "ax.FontName\n",
            "close(fig)\n",
            "fig = figure('Visible','off'); orient(fig, 'tall'); o = orient(fig)\n",
            "pp = fig.PaperPosition\n",
            "close(fig)\n",
        ),
        &[
            "o1 = 'portrait'",
            "o2 = 'landscape'",
            "ans = 'landscape'",
            "ob = 1×6\n\n     1     3     1     5     0     0",
            "ans = 'Helvetica'",
            "ans = 'Courier'",
            "o = 'tall'",
            "pp = 1×4\n\n    0.2500    0.2500    8.0000   10.5000",
        ],
    );
}
