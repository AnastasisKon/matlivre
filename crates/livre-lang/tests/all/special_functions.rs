//! Symbolic Math Toolbox special functions, classical polynomials and number theory, with
//! double and sym inputs, against MATLAB R2023b.

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
fn pulses_and_constants() {
    check(
        concat!(
            "x = [-1.5 -0.5 0 0.5 1 2.5];\nh = heaviside(x)\nd = dirac(x)\nd1 = dirac(1, [0 1])\nr1 = rectangularPulse(x)\nr2 = rectangularPulse(-1, 1, x)\n",
            "t1 = triangularPulse(x)\nt2 = triangularPulse(-1, 0, 2, x)\ns1 = signIm([1+2i, 3, -1i, 0])\nc = catalan\ng = eulergamma\np1 = pochhammer(3, [0 1 2.5])\n",
            "try, rectangularPulse(1, 0, 0.5), catch e, disp(e.message), end\n",
        ),
        &[
            "h = 1×6\n\n         0         0    0.5000    1.0000    1.0000    1.0000",
            "d = 1×6\n\n     0     0   Inf     0     0     0",
            "d1 = 1×2\n\n  -Inf     0",
            "r1 = 1×6\n\n         0    0.5000    1.0000    0.5000         0         0",
            "r2 = 1×6\n\n         0    1.0000    1.0000    1.0000    0.5000         0",
            "t1 = 1×6\n\n         0    0.5000    1.0000    0.5000         0         0",
            "t2 = 1×6\n\n         0    0.5000    1.0000    0.7500    0.5000         0",
            "s1 = 1×4\n\n     1    -1    -1     0",
            "c = catalan",
            "g = eulergamma",
            "p1 = 1×3\n\n    1.0000    3.0000   26.1714",
            "Left border must not exceed right border.\n",
        ],
    );
}

#[test]
fn exponential_trigonometric_and_fresnel_integrals() {
    check(
        concat!(
            "e1 = erfi([0 0.5 1 2])\ndw = dawson([0 0.5 1 2])\ny = [0.5 1 2 5];\nsi = sinint(y)\nci = cosint(y)\nshi = sinhint(y)\nchi = coshint(y)\n",
            "ssi = ssinint(y)\nei1 = ei(y)\nei2 = ei(-1)\nli = logint([0.5 2 3 10])\ndl = dilog([0 0.5 1 2])\npl = polylog(2, [0.5 -1 0.25])\npl3 = polylog(3, 0.5)\n",
            "fc = fresnelc(y)\nfs = fresnels(y)\nformat long\nsi1 = sinint(2)\nci1 = cosint(2)\nfc1 = fresnelc(1.5)\ndl1 = dilog(0.3)\nformat short\n",
            "p1 = polylog(2, 2)\np2 = polylog(3, [2 -3 0.9 -0.7])\np3 = polylog(1, 0.5)\np4 = polylog(0, 0.5)\np5 = polylog(-2, 0.5)\np6 = polylog(4, 1)\n",
            "dl = dilog(-1)\nci = cosint(-1)\nli = logint([0 1])\n",
        ),
        &[
            "e1 = 1×4\n\n         0    0.6150    1.6504   18.5648",
            "dw = 1×4\n\n         0    0.4244    0.5381    0.3013",
            "si = 1×4\n\n    0.4931    0.9461    1.6054    1.5499",
            "ci = 1×4\n\n   -0.1778    0.3374    0.4230   -0.1900",
            "shi = 1×4\n\n    0.5070    1.0573    2.5016   20.0932",
            "chi = 1×4\n\n   -0.0528    0.8379    2.4527   20.0921",
            "ssi = 1×4\n\n   -1.0777   -0.6247    0.0346   -0.0209",
            "ei1 = 1×4\n\n    0.4542    1.8951    4.9542   40.1853",
            "ei2 = -0.2194",
            "li = 1×4\n\n   -0.3787    1.0452    2.1636    6.1656",
            "dl = 1×4\n\n    1.6449    0.5822         0   -0.8225",
            "pl = 1×3\n\n    0.5822   -0.8225    0.2677",
            "pl3 = 0.5372",
            "fc = 1×4\n\n    0.4923    0.7799    0.4883    0.5636",
            "fs = 1×4\n\n    0.0647    0.4383    0.3434    0.4992",
            "si1 = 1.605412976802695",
            "ci1 = 0.422980828774865",
            "fc1 = 0.445261176039822",
            "dl1 = 0.889377624286039",
            "p1 = 2.4674 - 2.1776i",
            "p2 = 1×4 complex\n\n    2.7621 - 0.7547i   -2.3488 + 0.0000i    1.0497 + 0.0000i   -0.6487 + 0.0000i",
            "p3 = 0.6931",
            "p4 = 1",
            "p5 = 6",
            "p6 = 1.0823",
            "dl = 2.4674 - 2.1776i",
            "ci = 0.3374 + 3.1416i",
            "li = 1×2\n\n     0  -Inf",
        ],
    );
}

#[test]
fn gamma_and_hypergeometric_functions() {
    check(
        concat!(
            "ig = igamma(2, [0.5 1 3])\nig2 = igamma(0.5, 2)\nig4 = igamma([0 -1 -0.5], 2)\nhz = hurwitzZeta(2, [1 0.5 3])\nhz1 = hurwitzZeta(1, 2)\n",
            "wo = wrightOmega([0 1 -2 10])\nku = kummerU(1, 2, [0.5 1 3])\nhg1 = hypergeom([1 2], 3, 0.5)\nhg2 = hypergeom([], [], 1)\nhg3 = hypergeom(1, [], 0.25)\n",
            "hg4 = hypergeom([0.5 0.5], 1, 0.3)\nwm = whittakerM(1, 0.5, [0.5 2])\nww = whittakerW(1, 0.5, [0.5 2])\nmg = meijerG([], [], 0.5, [], 1)\n",
            "format long\nku1 = kummerU(0.5, 1.5, 2)\nhg5 = hypergeom([1 1], 2, -0.5)\nwo1 = wrightOmega(3)\n",
        ),
        &[
            "ig = 1×3\n\n    0.9098    0.7358    0.1991",
            "ig2 = 0.0806",
            "ig4 = 1×3\n\n    0.0489    0.0188    0.0301",
            "hz = 1×3\n\n    1.6449    4.9348    0.3949",
            "hz1 = NaN",
            "wo = 1×4\n\n    0.5671    1.0000    0.1200    7.9294",
            "ku = 1×3\n\n    2.0000    1.0000    0.3333",
            "hg1 = 1.5452",
            "hg2 = 2.7183",
            "hg3 = 1.3333",
            "hg4 = 1.0911",
            "wm = 1×2\n\n    0.3894    0.7358",
            "ww = 1×2\n\n    0.3894    0.7358",
            "mg = 0.3679",
            "ku1 = 0.707106781186548",
            "hg5 = 0.810930216216329",
            "wo1 = 2.207940031569323",
        ],
    );
}

#[test]
fn elliptic_integrals_and_jacobi_functions() {
    check(
        concat!(
            "m = [0 0.25 0.5 0.9];\neK = ellipticK(m)\neE = ellipticE(m)\neK1 = ellipticK(1)\neE1 = ellipticE(1)\neF = ellipticF(pi/3, m)\neE2 = ellipticE(pi/3, 0.5)\n",
            "eP = ellipticPi(0.3, m)\neP2 = ellipticPi(0.3, pi/4, 0.5)\ncK = ellipticCK(m)\ncE = ellipticCE(m)\ncP = ellipticCPi(0.3, 0.5)\nq = ellipticNome([0 0.5 0.9])\n",
            "ek = ellipticK([-1 2])\n",
            "u = [0 0.5 1 2];\nsn = jacobiSN(u, 0.5)\ncn = jacobiCN(u, 0.5)\ndn = jacobiDN(u, 0.5)\nam = jacobiAM(u, 0.5)\n",
            "r = [jacobiSC(1, 0.5) jacobiSD(1, 0.5) jacobiCD(1, 0.5) jacobiCS(1, 0.5) jacobiDS(1, 0.5) jacobiDC(1, 0.5) jacobiNC(1, 0.5) jacobiND(1, 0.5) jacobiNS(1, 0.5)]\n",
            "jz = jacobiZeta(u, 0.5)\nformat long\nk2 = ellipticK(0.7)\ne2 = ellipticE(0.7)\nf2 = ellipticF(1.2, 0.7)\np3 = ellipticPi(-0.5, 0.7)\n",
            "sn2 = jacobiSN(1.3, 0.7)\nam2 = jacobiAM(1.3, 0.7)\nq2 = ellipticNome(0.7)\n",
        ),
        &[
            "eK = 1×4\n\n    1.5708    1.6858    1.8541    2.5781",
            "eE = 1×4\n\n    1.5708    1.4675    1.3506    1.1048",
            "eK1 = Inf",
            "eE1 = 1",
            "eF = 1×4\n\n    1.0472    1.0896    1.1424    1.2690",
            "eE2 = 0.9650",
            "eP = 1×4\n\n    1.8775    2.0278    2.2504    3.2348",
            "eP2 = 0.8777",
            "cK = 1×4\n\n       Inf    2.1565    1.8541    1.6124",
            "cE = 1×4\n\n    1.0000    1.2111    1.3506    1.5308",
            "cP = 2.2504",
            "q = 1×3\n\n         0    0.0432    0.1402",
            "ek = 1×2 complex\n\n   1.3110 + 0.0000i   1.3110 - 1.3110i",
            "sn = 1×4\n\n         0    0.4708    0.8030    0.9947",
            "cn = 1×4\n\n    1.0000    0.8823    0.5960   -0.1032",
            "dn = 1×4\n\n    1.0000    0.9430    0.8232    0.7109",
            "am = 1×4\n\n         0    0.4901    0.9323    1.6742",
            "r = 1×9\n\n    1.3474    0.9755    0.7240    0.7422    1.0251    1.3812    1.6779    1.2148    1.2453",
            "jz = 1×4\n\n         0    0.1164    0.1438   -0.0331",
            "k2 = 2.075363135292469",
            "e2 = 1.241670567945823",
            "f2 = 1.429448433022763",
            "p3 = 1.645042868400853",
            "sn2 = 0.898731076260419",
            "am2 = 1.116867096931372",
            "q2 = 0.074689943537179",
        ],
    );
}

#[test]
fn polynomials_and_number_theory() {
    check(
        concat!(
            "syms x t\nT4 = chebyshevT(4, x)\nU3 = chebyshevU(3, x)\nG2 = gegenbauerC(2, 1, x)\nH4 = hermiteH(4, x)\nJ1 = jacobiP(2, 1, 2, x)\nL3 = laguerreL(3, x)\n",
            "P3 = legendreP(3, x)\nP0 = legendreP(0, x)\nnT = chebyshevT([0 1 2 3], 0.5)\nnU = chebyshevU(2, [0.1 0.7])\nnH = hermiteH(3, 2)\nnL = laguerreL(3, 1.5)\n",
            "nP = legendreP(2, [0 0.5 1])\nnG = gegenbauerC(2, 0.5, 0.3)\nnJ = jacobiP(2, 1, 2, 0.4)\neu = euler(0:6)\neup = euler(3, x)\nfp = factorIntegerPower(64)\n",
            "[bb, ee] = factorIntegerPower(1000)\n[bb2, ee2] = factorIntegerPower(12)\npr = isPrimitiveRoot([2 3 4 5], 7)\njs = jacobiSymbol([1 2 3 4 5], 15)\njs2 = jacobiSymbol(-1, 7)\n",
            "try, chebyshevT(-1, 0.5), catch e, disp(e.message), end\ntry, jacobiSymbol(3, 4), catch e, disp(e.message), end\n",
            "try, isPrimitiveRoot(2, 0), catch e, disp(e.message), end\n",
            "syms n\ns1 = chebyshevT(n, x)\ns2 = legendreP(2, sym(1)/3)\ns3 = hermiteH(3, sym(2))\nv = isPrimitiveRoot(sym(3), sym(7))\nw = jacobiSymbol(sym(2), sym(15))\n",
            "z30 = euler(sym(4))\n",
        ),
        &[
            "T4 = 8*x^4 - 8*x^2 + 1",
            "U3 = 8*x^3 - 4*x",
            "G2 = 4*x^2 - 1",
            "H4 = 16*x^4 - 48*x^2 + 12",
            "J1 = (21*x^2)/4 - (3*x)/2 - 3/4",
            "L3 = - x^3/6 + (3*x^2)/2 - 3*x + 1",
            "P3 = (5*x^3)/2 - (3*x)/2",
            "P0 = 1",
            "nT = 1×4\n\n    1.0000    0.5000   -0.5000   -1.0000",
            "nU = 1×2\n\n   -0.9600    0.9600",
            "nH = 40",
            "nL = -0.6875",
            "nP = 1×3\n\n   -0.5000   -0.1250    1.0000",
            "nG = -0.3650",
            "nJ = -0.5100",
            "eu = 1×7\n\n     1     0    -1     0     5     0   -61",
            "eup = x^3 - (3*x^2)/2 + 1/4",
            "fp = 2",
            "bb = 10",
            "ee = 3",
            "bb2 = 12",
            "ee2 = 1",
            "pr = 1×4 logical array\n\n   0   1   0   1",
            "js = 1×5\n\n     1     1     0     1     0",
            "js2 = -1",
            "Nonnegative integer or a symbolic variable expected.\n",
            "Invalid argument at position 2. Value must be odd.\n",
            "Invalid argument at position 2. Value must be positive.\n",
            "s1 = chebyshevT(n, x)",
            "s2 = -1/3",
            "s3 = 40",
            "v = logical\n   1",
            "w = 1",
            "z30 = 5",
        ],
    );
}

#[test]
fn symbolic_forms_and_special_values() {
    check(
        concat!(
            "syms x\nz1 = heaviside(x)\nz2 = heaviside(sym(0))\nz3 = dirac(sym(0))\nz4 = sinint(x)\nz5 = sinint(sym(0))\nz6 = ellipticK(sym(1/2))\nz7 = ellipticK(sym(0))\n",
            "z8 = pochhammer(x, 3)\nz9 = pochhammer(sym(3), 2)\nz10 = polylog(2, sym(1))\nz11 = dilog(sym(1))\nz12 = igamma(sym(1), x)\nz14 = erfi(sym(0))\n",
            "z15 = vpa(catalan)\nz16 = vpa(eulergamma)\nz17 = double(catalan)\nz18 = jacobiSN(x, 0)\nz19 = ellipticE(sym(pi)/2, x)\nz20 = cosint(sym(2))\n",
            "z22 = rectangularPulse(x)\nz23 = triangularPulse(x)\nz24 = diff(heaviside(x), x)\nz25 = diff(sinint(x), x)\nz26 = signIm(sym(1i))\nz27 = fresnelc(x)\n",
            "z28 = wrightOmega(sym(0))\nz29 = hurwitzZeta(2, sym(1))\ns4 = pochhammer(x, sym(1)/2)\ns5 = sinint(-x)\ns6 = dirac(2, x)\ns7 = ei(sym(0))\ns8 = dawson(sym(0))\n",
            "s9 = logint(sym(1))\ns10 = igamma(0, x)\ns11 = heaviside(sym(-3))\ns12 = rectangularPulse(sym(0))\ns13 = class(catalan)\ns14 = catalan + 1\ns15 = 2*eulergamma\n",
            "s16 = jacobiCN(x, 0)\ns17 = jacobiDN(x, 0)\ns18 = ellipticE(sym(0))\ns19 = hurwitzZeta(x, 1)\ns20 = kummerU(x, 1, 0)\nd = double(sinint(sym(2)))\n",
        ),
        &[
            "z1 = heaviside(x)",
            "z2 = 1/2",
            "z3 = Inf",
            "z4 = sinint(x)",
            "z5 = 0",
            "z6 = ellipticK(1/2)",
            "z7 = pi/2",
            "z8 = pochhammer(x, 3)",
            "z9 = 12",
            "z10 = pi^2/6",
            "z11 = 0",
            "z12 = exp(-x)",
            "z14 = 0",
            "z15 = 0.91596559417721901505460351493238",
            "z16 = 0.5772156649015328606065120900824",
            "z17 = 0.9160",
            "z18 = sin(x)",
            "z19 = ellipticE(x)",
            "z20 = cosint(2)",
            "z22 = rectangularPulse(-1/2, 1/2, x)",
            "z23 = triangularPulse(-1, 0, 1, x)",
            "z24 = dirac(x)",
            "z25 = sin(x)/x",
            "z26 = 1",
            "z27 = fresnelc(x)",
            "z28 = lambertw(0, 1)",
            "z29 = pi^2/6",
            "s4 = pochhammer(x, 1/2)",
            "s5 = -sinint(x)",
            "s6 = dirac(2, x)",
            "s7 = -Inf",
            "s8 = 0",
            "s9 = -Inf",
            "s10 = expint(x)",
            "s11 = 0",
            "s12 = 1",
            "s13 = 'sym'",
            "s14 = catalan + 1",
            "s15 = 2*eulergamma",
            "s16 = cos(x)",
            "s17 = 1",
            "s18 = pi/2",
            "s19 = zeta(x)",
            "s20 = kummerU(x, 1, 0)",
            "d = 1.6054",
        ],
    );
}
