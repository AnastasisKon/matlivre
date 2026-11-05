//! numjac, ODE output functions (OutputFcn, odeprint, OutputSel) and ts2timetable, against
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
fn numjac_odeprint_and_ts2timetable() {
    check(
        concat!(
            "f = @(t,y) [y(2); -sin(y(1))];\n",
            "[J, fac] = numjac(f, 0, [1; 0.5], f(0,[1;0.5]), 1e-6, [], 0)\n",
            "[J2, fac2, nf] = numjac(@(t,y) [y(1)^2; y(1)*y(2)], 0, [2; 3], [4; 6], [1e-6; 1e-6], [], 0)\n",
            "opts = odeset('OutputFcn', @odeprint);\n",
            "[t, y] = ode45(@(t,y) -y, [0 0.5 1], 1, opts);\n",
            "ts = timeseries([1;2;3], [0 1 2], 'Name', 'Speed');\n",
            "TT = ts2timetable(ts)\n",
        ),
        &[
            "J = 2×2\n\n         0    1.0000\n   -0.5403         0",
            "fac = 2×1\n\n   1.0e-07 *\n\n    0.1490\n    0.1490",
            "J2 = 2×2\n\n    4.0000         0\n    3.0000    2.0000",
            "fac2 = 2×1\n\n   1.0e-07 *\n\n    0.1490\n    0.1490",
            "nf = 2×1\n\n     1\n     2",
            "t = 0",
            "y = 1",
            "t = 0.5000",
            "y = 0.6065",
            "t = 1",
            "y = 0.3679",
            "TT = 3×1 timetable\n\n    Time     Speed\n    _____    _____\n\n    0 sec      1  \n    1 sec      2  \n    2 sec      3  ",
        ],
    );
}

#[test]
fn output_functions_stop_and_select() {
    check(
        concat!(
            "stopper = @(t,y,flag) ~isempty(t) && strcmp(flag, '') && t(end) > 0.5;\n",
            "[t, y] = ode45(@(t,y) -y, [0 1], 1, odeset('OutputFcn', stopper));\n",
            "n = numel(t)\n",
            "tl = t(end)\n",
            "[t2, y2] = ode45(@(t,y) -y, [0 0.25 0.5 0.75 1], 1, odeset('OutputFcn', stopper));\n",
            "t2.'\n",
            "[t3, y3] = ode23(@(t,y) [y(2); -y(1)], [0 0.5 1], [1; 0], odeset('OutputFcn', @odeprint, 'OutputSel', 2));\n",
        ),
        &[
            "n = 25",
            "tl = 0.6000",
            "ans = 1×4\n\n         0    0.2500    0.5000    0.7500",
            "t = 0",
            "y = 0",
            "t = 0.5000",
            "y = -0.4794",
            "t = 1",
            "y = -0.8414",
        ],
    );
}
