//! Formula tools (combine, root, sympref, mathml, eliminate, displayFormula,
//! changeIntegrationVariable, integrateByParts), against MATLAB R2023b.

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
fn combine_and_roots() {
    check(
        concat!(
            "syms x y a b z\n",
            "C1 = combine(exp(x)*exp(y))\n",
            "C2 = combine(x^a*x^b)\n",
            "C3 = combine(log(x) + log(y))\n",
            "C4 = combine(log(x) + log(y), 'log', 'IgnoreAnalyticConstraints', true)\n",
            "C5 = combine(2*sin(x)*cos(x), 'sincos')\n",
            "C6 = combine(sqrt(x)*sqrt(y))\n",
            "C7 = combine(sin(x)^2, 'sincos')\n",
            "C8 = combine(exp(x)^2)\n",
            "C9 = combine(a*log(x), 'log')\n",
            "R1 = root(x^3 + x + 1, x)\n",
            "R2 = root(x^2 - 2, x)\n",
            "R3 = root(x^3 + x + 1, x, 2)\n",
            "R4 = vpa(root(x^3 + x + 1, x, 1))\n",
            "R5 = root(x^5 + a*x + 1, x)\n",
        ),
        &[
            "C1 = exp(x)*exp(y)",
            "C2 = x^(a + b)",
            "C3 = log(x) + log(y)",
            "C4 = log(x*y)",
            "C5 = sin(2*x)",
            "C6 = x^(1/2)*y^(1/2)",
            "C7 = 1/2 - cos(2*x)/2",
            "C8 = exp(2*x)",
            "C9 = a*log(x)",
            "R1 = \nroot(x^3 + x + 1, x, 1)\nroot(x^3 + x + 1, x, 2)\nroot(x^3 + x + 1, x, 3)",
            "R2 = \nroot(x^2 - 2, x, 1)\nroot(x^2 - 2, x, 2)",
            "R3 = root(x^3 + x + 1, x, 2)",
            "R4 = -0.68232780382801932736948373971105",
            "R5 = \nroot(x^5 + a*x + 1, x, 1)\nroot(x^5 + a*x + 1, x, 2)\nroot(x^5 + a*x + 1, x, 3)\nroot(x^5 + a*x + 1, x, 4)\nroot(x^5 + a*x + 1, x, 5)",
        ],
    );
}

#[test]
fn sympref_mathml_and_elimination() {
    check(
        concat!(
            "syms x y a b z\n",
            "sp = sympref\n",
            "sp2 = sympref('FloatingPointOutput')\n",
            "M1 = mathml(x^2 + 1)\n",
            "M2 = mathml(sin(x)/y)\n",
            "E1 = eliminate([x + y == 3, x - y == 1], x)\n",
            "E2 = eliminate([x^2 + y == 1, x == y], y)\n",
            "old = sympref('HeavisideAtOrigin', 1)\n",
            "h = heaviside(sym(0))\n",
            "sympref('default');\n",
            "h2 = heaviside(sym(0))\n",
            "disp(mathml(x*y))\n",
            "disp(mathml(2*x))\n",
            "disp(mathml(-x))\n",
            "disp(mathml(x - y))\n",
            "disp(mathml(sym(pi)))\n",
            "disp(mathml(x^(1/3)))\n",
            "disp(mathml(exp(x)))\n",
            "disp(mathml(sym(1/2)))\n",
            "disp(mathml(x == 1))\n",
            "disp(mathml(2*x^2 - 3*x + 1))\n",
            "disp(mathml(1/x))\n",
            "disp(mathml(sin(x)^2))\n",
            "disp(mathml(sym(-3)))\n",
            "disp(mathml(vpa(2.5)))\n",
            "disp(mathml(x/2))\n",
            "disp(mathml((x+1)^2))\n",
            "disp(mathml(sym(1i)))\n",
            "disp(mathml(x*(y+1)))\n",
            "displayFormula(\"x^2 + 1\")\n",
            "displayFormula([\"x == 1\"; \"y^2\"])\n",
            "syms f(x)\n",
            "F = int(f(x), x, a, b)\n",
            "S1 = changeIntegrationVariable(F, x, sqrt(z))\n",
            "P2 = integrateByParts(int(x*sin(x), x, 0, pi), sin(x))\n",
        ),
        &[
            "sp = struct with fields:\n\n           FourierParameters: [1 -1]\n           HeavisideAtOrigin: 1/2\n            AbbreviateOutput: 1\n               TypesetOutput: 1\n         FloatingPointOutput: 0\n      PolynomialDisplayStyle: 'default'\n    MatrixWithSquareBrackets: 0",
            "sp2 = logical\n   0",
            "M1 = \n    '<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n       <mrow>\n         <msup>\n           <mi>x</mi>\n           <mn>2</mn>\n         </msup>\n         <mo>+</mo>\n         <mn>1</mn>\n       </mrow>\n     </math>\n     '",
            "M2 = \n    '<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n       <mfrac>\n         <mrow>\n           <mi>sin</mi>\n           <mrow>\n             <mo form='prefix'>(</mo>\n             <mrow>\n               <mi>x</mi>\n             </mrow>\n             <mo form='postfix'>)</mo>\n           </mrow>\n         </mrow>\n         <mi>y</mi>\n       </mfrac>\n     </math>\n     '",
            "E1 = y - 1",
            "E2 = x^2 + x - 1",
            "old = 1/2",
            "h = 1",
            "h2 = 1/2",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mi>x</mi>\n    <mo form='infix'>&InvisibleTimes;</mo>\n    <mi>y</mi>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mn>2</mn>\n    <mo form='infix'>&InvisibleTimes;</mo>\n    <mi>x</mi>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mo>-</mo>\n    <mi>x</mi>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mi>x</mi>\n    <mo>-</mo>\n    <mi>y</mi>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mi>&pi;</mi>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <msup>\n    <mi>x</mi>\n    <mrow>\n      <mn>1</mn>\n      <mo>/</mo>\n      <mn>3</mn>\n    </mrow>\n  </msup>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <msup>\n    <mo>&ee;</mo>\n    <mi>x</mi>\n  </msup>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mfrac>\n    <mn>1</mn>\n    <mn>2</mn>\n  </mfrac>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mi>x</mi>\n    <mo form='infix'>&equals;</mo>\n    <mn>1</mn>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mrow>\n      <mn>2</mn>\n      <mo form='infix'>&InvisibleTimes;</mo>\n      <msup>\n        <mi>x</mi>\n        <mn>2</mn>\n      </msup>\n    </mrow>\n    <mo>-</mo>\n    <mrow>\n      <mn>3</mn>\n      <mo form='infix'>&InvisibleTimes;</mo>\n      <mi>x</mi>\n    </mrow>\n    <mo>+</mo>\n    <mn>1</mn>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mfrac>\n    <mn>1</mn>\n    <mi>x</mi>\n  </mfrac>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <msup>\n    <mrow>\n      <mi>sin</mi>\n      <mrow>\n        <mo form='prefix'>(</mo>\n        <mrow>\n          <mi>x</mi>\n        </mrow>\n        <mo form='postfix'>)</mo>\n      </mrow>\n    </mrow>\n    <mn>2</mn>\n  </msup>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mo>-</mo>\n    <mn>3</mn>\n  </mrow>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mn>2.5</mn>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mfrac>\n    <mi>x</mi>\n    <mn>2</mn>\n  </mfrac>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <msup>\n    <mfenced>\n      <mrow>\n        <mi>x</mi>\n        <mo>+</mo>\n        <mn>1</mn>\n      </mrow>\n    </mfenced>\n    <mn>2</mn>\n  </msup>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mo>&ii;</mo>\n</math>\n\n",
            "<math xmlns='http://www.w3.org/1998/Math/MathML' display='block'>\n  <mrow>\n    <mi>x</mi>\n    <mo form='infix'>&InvisibleTimes;</mo>\n    <mfenced>\n      <mrow>\n        <mi>y</mi>\n        <mo>+</mo>\n        <mn>1</mn>\n      </mrow>\n    </mfenced>\n  </mrow>\n</math>\n\n",
            "x^2 + 1\n",
            "x == 1\n",
            "y^2\n",
            "F = int(f(x), x, a, b)",
            "S1 = int(f(z^(1/2))/(2*z^(1/2)), z, a^2, b^2)",
            "P2 = pi",
        ],
    );
}
