//! Abstract symbolic functions (syms y(t)), dsolve, odeToVectorField and functionalDerivative,
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
fn abstract_symbolic_functions() {
    check(
        concat!(
            "syms y(t) a\n",
            "A1 = y\n",
            "A2 = diff(y, t)\n",
            "A3 = diff(y, t, 2)\n",
            "A4 = y(0)\n",
            "A5 = subs(diff(y,t), t, 0)\n",
            "A6 = diff(y,t) == y\n",
            "A7 = class(y)\n",
            "A8 = symFunType(y)\n",
            "A9 = formula(y)\n",
            "A10 = argnames(y)\n",
            "A11 = diff(y^2, t)\n",
            "A12 = diff(sin(y), t)\n",
        ),
        &[
            "A1(t) = y(t)",
            "A2(t) = diff(y(t), t)",
            "A3(t) = diff(y(t), t, t)",
            "A4 = y(0)",
            "A5(t) = subs(diff(y(t), t), t, 0)",
            "A6(t) = diff(y(t), t) == y(t)",
            "A7 = 'symfun'",
            "A8 = \"y\"",
            "A9 = y(t)",
            "A10 = t",
            "A11(t) = 2*y(t)*diff(y(t), t)",
            "A12(t) = cos(y(t))*diff(y(t), t)",
        ],
    );
}

#[test]
fn dsolve_linear_equations() {
    check(
        concat!(
            "syms y(t) a\n",
            "D1 = dsolve(diff(y,t) == y)\n",
            "D2 = dsolve(diff(y,t) == a*y)\n",
            "D3 = dsolve(diff(y,t) == y, y(0) == 2)\n",
            "D4 = dsolve(diff(y,t,2) == -y)\n",
            "D5 = dsolve(diff(y,t,2) + y == 0, [y(0) == 1, subs(diff(y,t),t,0) == 0])\n",
            "D6 = dsolve(diff(y,t,2) - 3*diff(y,t) + 2*y == 0)\n",
            "D8 = dsolve(diff(y,t) + 2*y == 4)\n",
            "D9 = dsolve(diff(y,t) == t)\n",
            "D11 = dsolve(diff(y,t,2) + 4*y == 0, [y(0) == 0, subs(diff(y,t),t,0) == 2])\n",
            "D13 = dsolve(diff(y,t,3) == 0)\n",
            "D14 = dsolve(diff(y,t) == -2*t*y)\n",
            "D18 = dsolve(diff(y,t) == 3)\n",
            "D19 = dsolve(diff(y,t) == y, y(1) == 1)\n",
        ),
        &[
            "D1 = C1*exp(t)",
            "D2 = C1*exp(a*t)",
            "D3 = 2*exp(t)",
            "D4 = C1*cos(t) - C2*sin(t)",
            "D5 = cos(t)",
            "D6 = C1*exp(t) + C2*exp(2*t)",
            "D8 = C1*exp(-2*t) + 2",
            "D9 = t^2/2 + C1",
            "D11 = sin(2*t)",
            "D13 = (C1*t^2)/2 + C2*t + C3",
            "D14 = C1*exp(-t^2)",
            "D18 = C1 + 3*t",
            "D19 = exp(-1)*exp(t)",
        ],
    );
}

#[test]
fn vector_fields_and_functional_derivatives() {
    check(
        concat!(
            "syms y(t)\n",
            "V1 = odeToVectorField(diff(y,t,2) == -y)\n",
            "[V2, S2b] = odeToVectorField(diff(y,t,2) + 3*diff(y,t) + 2*y == sin(t))\n",
            "FD1 = functionalDerivative(y^2 + diff(y,t)^2, y)\n",
            "FD2 = functionalDerivative(sin(y), y)\n",
            "FD3 = functionalDerivative(t*diff(y,t)^2, y)\n",
        ),
        &[
            "V1 = \n Y[2]\n-Y[1]",
            "V2 = \n                    Y[2]\nsin(t) - 2*Y[1] - 3*Y[2]",
            "S2b = \n y\nDy",
            "FD1(t) = 2*y(t) - 2*diff(y(t), t, t)",
            "FD2(t) = cos(y(t))",
            "FD3(t) = - 2*t*diff(y(t), t, t) - 2*diff(y(t), t)",
        ],
    );
}
