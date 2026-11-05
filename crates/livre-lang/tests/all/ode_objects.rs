//! The ode object interface (ode, solve, odeEvent, odeJacobian, odeMassMatrix, solutionFcn),
//! against MATLAB R2023b.

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
fn ode_problems_and_solve() {
    check(
        concat!(
            "F = ode\n",
            "F.ODEFcn = @(t,y) -2*y;\n",
            "F.InitialValue = 1;\n",
            "F\n",
            "S = solve(F, 0, 1);\n",
            "n = numel(S.Time)\n",
            "y1 = S.Solution(end)\n",
            "class(S)\n",
            "S2 = solve(F, [0 0.5 1]);\n",
            "t2 = S2.Time\n",
            "y2 = S2.Solution\n",
            "F2 = ode(ODEFcn=@(t,y) [y(2); -y(1)], InitialValue=[1; 0], InitialTime=0);\n",
            "S3 = solve(F2, [0 pi/2 pi]);\n",
            "y3 = round(S3.Solution, 4)\n",
            "F2.Solver\n",
            "F2.SelectedSolver\n",
            "F2.Solver = \"ode15s\";\n",
            "F2.SelectedSolver\n",
        ),
        &[
            "F = \n  ode with properties:\n\n   Problem definition\n               ODEFcn: []\n          InitialTime: 0\n         InitialValue: [0x1 double]\n\n   Solver properties\n    AbsoluteTolerance: 1.0000e-06\n    RelativeTolerance: 1.0000e-03\n               Solver: auto\n\n  Show all properties\n",
            "F = \n  ode with properties:\n\n   Problem definition\n               ODEFcn: @(t,y)-2*y\n          InitialTime: 0\n         InitialValue: 1\n\n   Solver properties\n    AbsoluteTolerance: 1.0000e-06\n    RelativeTolerance: 1.0000e-03\n               Solver: auto\n       SelectedSolver: ode45\n\n  Show all properties\n",
            "n = 41",
            "y1 = 0.1353",
            "ans = 'matlab.ode.ODEResults'",
            "t2 = 1×3\n\n         0    0.5000    1.0000",
            "y2 = 1×3\n\n    1.0000    0.3679    0.1353",
            "y3 = 2×3\n\n     1     0    -1\n     0    -1     0",
            "ans = \n  SolverID enumeration\n\n    auto",
            "ans = \n  SolverID enumeration\n\n    ode45",
            "ans = \n  SolverID enumeration\n\n    ode15s",
        ],
    );
}

#[test]
fn events_jacobians_mass_matrices_and_solution_functions() {
    check(
        concat!(
            "F = ode;\n",
            "F.ODEFcn = @(t,y) -2*y;\n",
            "F.InitialValue = 1;\n",
            "F2 = ode(ODEFcn=@(t,y) [y(2); -y(1)], InitialValue=[1; 0], InitialTime=0);\n",
            "E = odeEvent(EventFcn=@(t,y) y(1), Direction=\"descending\", Response=\"stop\")\n",
            "F2.EventDefinition = E;\n",
            "F2.Solver = \"auto\";\n",
            "S4 = solve(F2, 0, 10);\n",
            "te = round(S4.EventTime, 4)\n",
            "J = odeJacobian(Jacobian=[0 1; -1 0])\n",
            "M = odeMassMatrix(MassMatrix=eye(2))\n",
            "fh = solutionFcn(F, 0, 1);\n",
            "v = fh(0.5)\n",
            "class(fh)\n",
            "F.Parameters = 3;\n",
            "F.ODEFcn = @(t,y,p) -p*y;\n",
            "S5 = solve(F, [0 1]);\n",
            "y5 = S5.Solution(end)\n",
        ),
        &[
            "E = \n  odeEvent with properties:\n\n       EventFcn: @(t,y)y(1)\n      Direction: descending\n       Response: stop\n    CallbackFcn: []",
            "te = 1.5707",
            "J = \n  odeJacobian with properties:\n\n           Jacobian: [2×2 double]\n    SparsityPattern: []",
            "M = \n  odeMassMatrix with properties:\n\n         MassMatrix: [2×2 double]\n           Singular: no\n    StateDependence: none\n    SparsityPattern: []",
            "v = 0.3679",
            "ans = 'function_handle'",
            "y5 = 0.0498",
        ],
    );
}
