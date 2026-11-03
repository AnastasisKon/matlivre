//! surfnorm, ruler secondary labels and ruler value conversion against MATLAB R2023b.

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

#[test]
fn surfnorm_matches_matlab() {
    assert_eq!(
        shown("[nx,ny,nz] = surfnorm([1 2 3; 2 3 5; 4 4 4])\n"),
        [
            "nx = 3×3\n\n   -0.6667   -0.5774   -0.2649\n   -0.2673   -0.7276   -0.9129\n         0         0         0",
            "ny = 3×3\n\n   -0.3333   -0.5774   -0.9272\n   -0.8018   -0.4851   -0.1826\n   -0.9285   -0.7071    0.9285",
            "nz = 3×3\n\n    0.6667    0.5774    0.2649\n    0.5345    0.4851    0.3651\n    0.3714    0.7071    0.3714",
        ]
    );
}

#[test]
fn secondary_labels_and_ruler_values() {
    assert_eq!(
        shown(concat!(
            "f = figure('Visible','off');\nplot(1:3); ax = gca;\nxsecondarylabel('units')\nt = ax.XAxis.SecondaryLabel; t.String\n",
            "ysecondarylabel(ax, 'kg')\nax.YAxis.SecondaryLabel.String\n",
            "r = ruler2num([2 3], ax.XAxis)\nr2 = num2ruler([2 3], ax.XAxis)\n",
            "plot(datetime(2020,1,1:3), 1:3); ax = gca;\nr3 = ruler2num(datetime(2020,1,2), ax.XAxis)\nr4 = num2ruler(1, ax.XAxis)\n",
        )),
        [
            "ans = 'units'",
            "ans = 'kg'",
            "r = 1×2\n\n     2     3",
            "r2 = 1×2\n\n     2     3",
            "r3 = 1",
            "r4 = datetime\n   02-Jan-2020",
        ]
    );
}
