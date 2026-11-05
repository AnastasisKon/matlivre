//! Symbolic transforms (ztrans, iztrans, fourier, ifourier, htrans, ihtrans), matrix normal
//! forms (jordan, smithForm, hermiteForm), gbasis and piecewise, against MATLAB R2023b.

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
fn z_transforms() {
    check(
        concat!(
            "syms n z k a b w t x s\n",
            "Z1 = ztrans(sym(1))\n",
            "Z2 = ztrans(n)\n",
            "Z3 = ztrans(n^2)\n",
            "Z5 = ztrans(2^n + 3)\n",
            "Z8 = ztrans(n*2^n)\n",
            "Z9 = ztrans(exp(-n))\n",
            "Z10 = ztrans(kroneckerDelta(n))\n",
            "Z12 = ztrans(n^3)\n",
            "I1 = iztrans(z/(z-1))\n",
            "I2 = iztrans(z/(z-2))\n",
            "I3 = iztrans(z/(z-1)^2)\n",
            "I5 = iztrans(2*z/(2*z-1))\n",
            "I6 = iztrans(z/((z-1)*(z-2)))\n",
            "I7 = iztrans(sym(1))\n",
            "I8 = iztrans(1/z^2)\n",
        ),
        &[
            "Z1 = z/(z - 1)",
            "Z2 = z/(z - 1)^2",
            "Z3 = (z*(z + 1))/(z - 1)^3",
            "Z5 = (3*z)/(z - 1) + z/(z - 2)",
            "Z8 = (2*z)/(z - 2)^2",
            "Z9 = z/(z - exp(-1))",
            "Z10 = 1",
            "Z12 = (z*(z^2 + 4*z + 1))/(z - 1)^4",
            "I1 = 1",
            "I2 = 2^n",
            "I3 = n",
            "I5 = (1/2)^n",
            "I6 = 2^n - 1",
            "I7 = kroneckerDelta(n, 0)",
            "I8 = kroneckerDelta(n - 2, 0)",
        ],
    );
}

#[test]
fn fourier_and_hilbert_transforms() {
    check(
        concat!(
            "syms n z k a b w t x s\n",
            "F1 = fourier(exp(-t^2))\n",
            "F2 = fourier(exp(-x^2), x, w)\n",
            "F3 = fourier(dirac(t))\n",
            "F4 = fourier(sym(1))\n",
            "F5 = fourier(exp(-abs(t)))\n",
            "F6 = fourier(heaviside(t)*exp(-t))\n",
            "F8 = fourier(cos(t))\n",
            "F9 = fourier(sin(t))\n",
            "F10 = fourier(1/(1+t^2))\n",
            "F12 = fourier(dirac(t-a))\n",
            "F13 = fourier(exp(-a*t^2))\n",
            "F14 = fourier(heaviside(t))\n",
            "G1 = ifourier(1/(1+w^2))\n",
            "G2 = ifourier(exp(-w^2))\n",
            "G3 = ifourier(sym(1))\n",
            "G4 = ifourier(dirac(w))\n",
            "G5 = ifourier(2/(w^2+1), w, t)\n",
            "H1 = htrans(1/t)\n",
            "H2 = htrans(sin(t))\n",
            "H3 = htrans(cos(t))\n",
            "H5 = htrans(1/(t^2+1))\n",
            "J1 = ihtrans(sin(x))\n",
            "J2 = ihtrans(1/(x^2+1))\n",
        ),
        &[
            "F1 = pi^(1/2)*exp(-w^2/4)",
            "F2 = pi^(1/2)*exp(-w^2/4)",
            "F3 = 1",
            "F4 = 2*pi*dirac(w)",
            "F5 = 2/(w^2 + 1)",
            "F6 = 1/(1 + w*1i)",
            "F8 = pi*(dirac(w - 1) + dirac(w + 1))",
            "F9 = -pi*(dirac(w - 1) - dirac(w + 1))*1i",
            "F10 = pi*exp(-abs(w))",
            "F12 = exp(-a*w*1i)",
            "F13 = fourier(exp(-a*t^2), t, w)",
            "F14 = pi*dirac(w) - 1i/w",
            "G1 = exp(-abs(x))/2",
            "G2 = exp(-x^2/4)/(2*pi^(1/2))",
            "G3 = dirac(x)",
            "G4 = 1/(2*pi)",
            "G5 = exp(-abs(t))",
            "H1 = -pi*dirac(x)",
            "H2 = -cos(x)",
            "H3 = sin(x)",
            "H5 = x/(x^2 + 1)",
            "J1 = cos(t)",
            "J2 = -t/(t^2 + 1)",
        ],
    );
}

#[test]
fn jordan_smith_and_hermite_forms() {
    check(
        concat!(
            "syms x y z a\n",
            "J1 = jordan(sym([1 1; 0 1]))\n",
            "[V2, J2] = jordan(sym([2 0; 0 3]))\n",
            "J3 = jordan(sym([5 4 2 1; 0 1 -1 -1; -1 -1 3 0; 1 1 -1 2]))\n",
            "[V4, J4] = jordan(sym([1 2; 3 2]))\n",
            "J5 = jordan([4 1; -1 2])\n",
            "S1 = smithForm(sym([2 4 4; -6 6 12; 10 -4 -16]))\n",
            "H1 = hermiteForm(sym([2 4 4; -6 6 12; 10 -4 -16]))\n",
            "[U3, H3] = hermiteForm(sym([2 4; 6 8]))\n",
            "H4 = hermiteForm(sym([9 -36 30; -36 192 -180; 30 -180 180]))\n",
            "S2 = smithForm(sym([2 4; 6 8]))\n",
        ),
        &[
            "J1 = \n[1, 1]\n[0, 1]",
            "V2 = \n[1, 0]\n[0, 1]",
            "J2 = \n[2, 0]\n[0, 3]",
            "J3 = \n[4, 1, 0, 0]\n[0, 4, 0, 0]\n[0, 0, 1, 0]\n[0, 0, 0, 2]",
            "V4 = \n[-1, 2/3]\n[ 1,   1]",
            "J4 = \n[-1, 0]\n[ 0, 4]",
            "J5 = 2×2\n\n     3     1\n     0     3",
            "S1 = \n[2, 0,  0]\n[0, 6,  0]\n[0, 0, 12]",
            "H1 = \n[2, -2,  4]\n[0,  6,  0]\n[0,  0, 12]",
            "U3 = \n[-2,  1]\n[ 3, -1]",
            "H3 = \n[2, 0]\n[0, 4]",
            "H4 = \n[3,  0, 30]\n[0, 12,  0]\n[0,  0, 60]",
            "S2 = \n[2, 0]\n[0, 4]",
        ],
    );
}

#[test]
fn groebner_bases_and_piecewise() {
    check(
        concat!(
            "syms x y z a\n",
            "G1 = gbasis([x^2 - y, x*y - 1])\n",
            "G2 = gbasis([x^2 + y^2 - 1, x - y], [x y])\n",
            "G3 = gbasis([x*y - 1, y^2 - x])\n",
            "G4 = gbasis([x^3 - 2*x*y, x^2*y - 2*y^2 + x], [x y], 'MonomialOrder', 'lexicographic')\n",
            "P1 = piecewise(x < 0, -x, x >= 0, x)\n",
            "P2 = subs(P1, x, -3)\n",
            "P3 = piecewise(x < 0, 0, 1)\n",
            "P4 = subs(P3, x, 2)\n",
            "P5 = piecewise(x < -1, -1, -1 <= x <= 1, x, 1);\n",
            "P6 = diff(piecewise(x < 0, x^2, x^3))\n",
            "P7 = subs(P5, x, [-2 0 3])\n",
            "T = symtrue\n",
            "F = symfalse\n",
        ),
        &[
            "G1 = \n[x^2 - y, x*y - 1, y^2 - x]",
            "G2 = \n[2*y^2 - 1, x - y]",
            "G3 = \n[x^2 - y, x*y - 1, y^2 - x]",
            "G4 = \n[- 2*y^2 + x, y^3]",
            "P1 = piecewise(x < 0, -x, 0 <= x, x)",
            "P2 = 3",
            "P3 = piecewise(x < 0, 0, symtrue, 1)",
            "P4 = 1",
            "P6 = piecewise(x < 0, 2*x, symtrue, 3*x^2)",
            "P7 = \n[-1, 0, 1]",
            "T = symtrue",
            "F = symfalse",
        ],
    );
}
