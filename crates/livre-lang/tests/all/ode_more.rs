//! Stiff, implicit, high-order, delay, boundary value and PDE solvers against MATLAB R2023b.
//! The step sequences differ from MATLAB's, so values are compared within the tolerances
//! (the MATLAB results are quoted in the scripts).

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
fn stiff_solvers_match_matlab() {
    assert_eq!(
        shown(concat!(
            "vdp = @(t,y) [y(2); 1000*(1-y(1)^2)*y(2)-y(1)];\nok = [];\n",
            "for s = {'ode15s','ode23s','ode23t','ode23tb'}\n",
            "  [t,y] = feval(s{1}, vdp, [0 3000], [2;0]); ok(end+1) = abs(y(end,1) - (-1.5023)) < 0.02 && t(end) == 3000;\n",
            "end\nall(ok)\n",
            "rob = @(t,y) [-0.04*y(1)+1e4*y(2)*y(3); 0.04*y(1)-1e4*y(2)*y(3)-3e7*y(2)^2; 3e7*y(2)^2];\n",
            "[t,y] = ode15s(rob, [0 40], [1;0;0]); r1 = norm(y(end,:) - [0.7159311079 9.18954491e-06 0.2840597026]) < 1e-3\n",
            "M = [1 0 0; 0 1 0; 0 0 0];\n",
            "dae = @(t,y) [-0.04*y(1)+1e4*y(2)*y(3); 0.04*y(1)-1e4*y(2)*y(3)-3e7*y(2)^2; y(1)+y(2)+y(3)-1];\n",
            "o = odeset('Mass', M, 'RelTol', 1e-4, 'AbsTol', [1e-6 1e-10 1e-6]);\n",
            "[t,y] = ode15s(dae, [0 40], [1;0;0], o); r2 = abs(sum(y(end,:)) - 1) < 1e-12 && abs(y(end,1) - 0.7158533103) < 1e-3\n",
            "[t,y] = ode23t(dae, [0 1 10 40], [1;0;0], o); sz = size(y)\n",
            "sol = ode23s(@(t,y) -100*(y - cos(t)), [0 1], 0); c = class(sol.idata); v = abs(deval(sol, 1) - 0.5486) < 1e-2\n",
        )),
        [
            "ans = logical\n   1",
            "r1 = logical\n   1",
            "r2 = logical\n   1",
            "sz = 1×2\n\n     4     3",
            "v = logical\n   1",
        ]
    );
}

#[test]
fn high_order_solvers_and_odextend_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ok = [];\nfor s = {'ode113','ode78','ode89'}\n",
            "  [t,y] = feval(s{1}, @(t,y) [y(2); -y(1)], [0 10], [0;1]); ok(end+1) = abs(y(end,1) - sin(10)) < 2e-3;\n",
            "  [t,y] = feval(s{1}, @(t,y) [y(2); -y(1)], [0 1 2 3], [0;1]); ok(end+1) = isequal(size(y), [4 2]) && norm(y(:,1) - sin(t)) < 2e-3;\n",
            "  o = odeset('RelTol', 1e-10, 'AbsTol', 1e-12);\n",
            "  [t,y] = feval(s{1}, @(t,y) [y(2); -y(1)], [0 10], [0;1], o); ok(end+1) = abs(y(end,1) - sin(10)) < 1e-8;\n",
            "end\nall(ok)\n",
            "sol = ode45(@(t,y) -y, [0 1], 1); sol2 = odextend(sol, [], 2); e = abs(sol2.y(end) - exp(-2)) < 1e-6, s = sol2.solver, x = sol2.x(end)\n",
            "d = abs(deval(sol2, 1.5) - exp(-1.5)) < 1e-5\n",
            "sol = ode113(@(t,y) -y, [0 1], 1); sol = odextend(sol, [], 3); d2 = abs(deval(sol, 2.5) - exp(-2.5)) < 1e-3\n",
        )),
        [
            "ans = logical\n   1",
            "e = logical\n   1",
            "s = 'ode45'",
            "x = 2",
            "d = logical\n   1",
            "d2 = logical\n   1",
        ]
    );
}

#[test]
fn implicit_equations_match_matlab() {
    assert_eq!(
        shown(concat!(
            "f = @(t,y,yp) yp + y;\n[y0n, yp0n] = decic(f, 0, 1, 1, 0, 0)\n",
            "[t,y] = ode15i(f, [0 1], 1, -1); e = abs(y(end) - exp(-1)) < 1e-3\n",
            "g = @(t,y,yp) [yp(1) - y(2); y(1)^2 + y(2)^2 - 1];\n",
            "[y0, yp0] = decic(g, 0, [1; 0.5], [1; 0], [0; 0], [0; 0]); r = abs(y0(1)^2 + y0(2)^2 - 1) < 1e-6 && y0(1) == 1\n",
        )),
        ["y0n = 1", "yp0n = -1", "e = logical\n   1", "r = logical\n   1"]
    );
}

#[test]
fn delay_equations_match_matlab() {
    assert_eq!(
        shown(concat!(
            "ddex1de = @(t,y,Z) [Z(1,1); Z(1,1)+Z(2,2); y(2)];\n",
            "sol = dde23(ddex1de, [1 0.2], ones(3,1), [0 5]);\n",
            "e = norm(sol.y(:,end) - [19.174914; 176.41204; 190.33533]) < 1e-3\nf = fieldnames(sol)'\n",
            "v = norm(deval(sol, [1 2.5]) - [2 4.6458333; 3.7141132 17.311779; 3.1816333 16.611246]) < 1e-3\n",
            "sol = ddesd(@(t,y,Z) -Z, @(t,y) t-1, 1, [0 3]); d = abs(sol.y(end) - (-0.1666448)) < 1e-3\n",
            "o = ddeset('RelTol', 1e-5); r = ddeget(o, 'RelTol')\n",
        )),
        [
            "e = logical\n   1",
            "f = 1×7 cell\n\n    {'solver'}    {'history'}    {'discont'}    {'x'}    {'y'}    {'stats'}    {'yp'}",
            "v = logical\n   1",
            "d = logical\n   1",
            "r = 1.0000e-05",
        ]
    );
}

#[test]
fn boundary_value_problems_match_matlab() {
    assert_eq!(
        shown(concat!(
            "solinit = bvpinit(linspace(0,4,5), [1 0]);\n",
            "sol = bvp4c(@(x,y)[y(2); -abs(y(1))], @(ya,yb)[ya(1); yb(1)+2], solinit);\n",
            "e = abs(sol.y(2,1) - 2.066896) < 1e-3 && sol.y(1,end) == -2\nf = fieldnames(sol)'\n",
            "v = norm(deval(sol, 2) - [1.8794659; -0.86003903]) < 1e-3\n",
            "sol5 = bvp5c(@(x,y)[y(2); -abs(y(1))], @(ya,yb)[ya(1); yb(1)+2], solinit); e5 = abs(sol5.y(2,1) - 2.0667952) < 1e-3\n",
            "solinit = bvpinit(linspace(0,pi,10), @(x)[cos(4*x); -4*sin(4*x)], 15);\n",
            "sol = bvp4c(@(x,y,lam)[y(2); -(lam-10*cos(2*x))*y(1)], @(ya,yb,lam)[ya(2); yb(2); ya(1)-1], solinit);\n",
            "lam = abs(sol.parameters - 17.09734) < 1e-3\nfieldnames(solinit)'\n",
            "o = bvpset('RelTol', 1e-5); r = bvpget(o, 'RelTol')\n",
        )),
        [
            "e = logical\n   1",
            "f = 1×5 cell\n\n    {'solver'}    {'x'}    {'y'}    {'yp'}    {'stats'}",
            "v = logical\n   1",
            "e5 = logical\n   1",
            "lam = logical\n   1",
            "ans = 1×5 cell\n\n    {'solver'}    {'x'}    {'y'}    {'parameters'}    {'yinit'}",
            "r = 1.0000e-05",
        ]
    );
}

#[test]
fn pdepe_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "x = linspace(0,1,20); t = linspace(0,2,5);\n",
            "sol = pdepe(0, @(x,t,u,dudx) deal(pi^2, dudx, 0), @(x) sin(pi*x), @(xl,ul,xr,ur,t) deal(ul, 0, pi*exp(-t), 1), x, t);\n",
            "sz = size(sol)\ne = abs(sol(3,10) - 0.36700499) < 2e-3 && abs(sol(end,end) - (-0.0011893822)) < 1e-3 && sol(1,1) == 0\n",
            "[u, dudx] = pdeval(0, x, sol(end,:), [0.5 0.75]); p = norm([u dudx] - [0.13478958 0.094859445 -0.0014721011 -0.31494208]) < 2e-3\n",
            "s2 = pdepe(1, @(x,t,u,dudx) deal(1, dudx, 0), @(x) 1 - x.^2, @(xl,ul,xr,ur,t) deal(0, 1, ur, 0), linspace(0,1,15), [0 0.1 0.2]); m1 = s2(end,1) < s2(1,1) && s2(end,end) == 0\n",
        )),
        ["sz = 1×2\n\n     5    20", "e = logical\n   1", "p = logical\n   1", "m1 = logical\n   1"]
    );
}

#[test]
fn scalars_are_padded_by_disp_like_matlab() {
    assert_eq!(
        shown("disp(0.5); disp(1e-5); disp(-1e-5); disp(12345.678); disp(NaN); disp(2)\nformat long\ndisp(pi); disp(100.5); disp(-1e-5)\n"),
        [
            "    0.5000\n",
            "   1.0000e-05\n",
            "  -1.0000e-05\n",
            "   1.2346e+04\n",
            "   NaN\n",
            "     2\n",
            "   3.141592653589793\n",
            "     1.005000000000000e+02\n",
            "    -1.000000000000000e-05\n",
        ]
    );
}
