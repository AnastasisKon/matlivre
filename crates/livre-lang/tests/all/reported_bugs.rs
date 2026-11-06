//! Bugs reported while documenting every function for the wiki (crashes and wrong
//! results), against MATLAB R2023b.

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
fn former_panics() {
    check(
        concat!(
            "try, d = base2dec('ab', 'ab'), catch e, disp(e.message), end\n",
            "try, d = base2dec({'ba'; 'aab'}, 'ab'), catch e, disp(e.message), end\n",
            "try, d = base2dec('10', 'xyz'), catch e, disp(e.message), end\n",
            "try, base2dec('10', 'a'), catch e, disp(e.message), end\n",
            "try, A = accumarray([1 1; 2 2; 1 1], [10 20 30]), catch e, disp(e.message), end\n",
            "try, A = accumarray([1 2; 3 1], 5), catch e, disp(e.message), end\n",
            "try, A = accumarray([1 1 1; 2 1 2], [4 6]), catch e, disp(e.message), end\n",
            "try, A = accumarray([1 1; 2 2; 1 1], [10 20 30], [3 3]), catch e, disp(e.message), end\n",
            "try, A = accumarray([1 1; 2 2; 1 1], [10 20 30], [], @max), catch e, disp(e.message), end\n",
            "TR = triangulation([1 2 3; 1 3 4], [0 0 0; 1 0 0; 1 1 1; 0 1 0]);\n",
            "try, C = circumcenter(TR), catch e, disp(e.message), end\n",
            "try, C = circumcenter(TR, 2), catch e, disp(e.message), end\n",
            "try, [C, r] = circumcenter(TR, 1), catch e, disp(e.message), end\n",
            "try, B = cartesianToBarycentric(TR, 1, [0.5 0.25 0.25]), catch e, disp(e.message), end\n",
            "try, P = barycentricToCartesian(TR, [1; 2], [0.2 0.3 0.5; 1/3 1/3 1/3]), catch e, disp(e.message), end\n",
            "try, unmesh([0 1 0 1 1]), catch e, disp(e.message), end\n",
            "try, unmesh([1 2 3]), catch e, disp(e.message), end\n",
            "try, [x, y] = unmesh([0 0 1 0; 1 0 1 1]), catch e, disp(e.message), end\n",
        ),
        &[
            "Operands to the logical AND (&&) and OR (||) operators must be convertible to logical scalar values. Use the ANY or ALL functions to reduce operands to logical scalar values.\n",
            "Operands to the logical AND (&&) and OR (||) operators must be convertible to logical scalar values. Use the ANY or ALL functions to reduce operands to logical scalar values.\n",
            "Operands to the logical AND (&&) and OR (||) operators must be convertible to logical scalar values. Use the ANY or ALL functions to reduce operands to logical scalar values.\n",
            "The base must be an integer value between 2 and 36.\n",
            "A = 2×2\n\n    40     0\n     0    20",
            "A = 3×2\n\n     0     5\n     0     0\n     5     0",
            "A = 2×1×2\n\nA(:,:,1) =\n\n     4\n     0\n\nA(:,:,2) =\n\n     0\n     6",
            "A = 3×3\n\n    40     0     0\n     0    20     0\n     0     0     0",
            "A = 2×2\n\n    30     0\n     0    20",
            "C = 2×3\n\n    0.5000    0.5000    0.5000\n    0.5000    0.5000    0.5000",
            "C = 1×3\n\n    0.5000    0.5000    0.5000",
            "C = 1×3\n\n    0.5000    0.5000    0.5000",
            "r = 0.8660",
            "B = 1×3\n\n    0.5000    0.2500    0.2500",
            "P = 2×3\n\n    0.8000    0.5000    0.5000\n    0.3333    0.6667    0.3333",
            "Mesh must have rows of the form [x1 y1 x2 y2].\n",
            "Mesh must have rows of the form [x1 y1 x2 y2].\n",
            "x = \n\n   (1,1)        1\n   (2,1)       -1\n   (1,2)       -1\n   (2,2)        2\n   (3,2)       -1\n   (2,3)       -1\n   (3,3)        1",
            "y = 3×2\n\n     0     0\n     1     0\n     1     1",
        ],
    );
}

#[test]
fn numeric_results() {
    check(
        concat!(
            "try, s = sign(3+4i), catch e, disp(e.message), end\n",
            "try, s = sign([-2 0 1i]), catch e, disp(e.message), end\n",
            "try, f = factorial(170), catch e, disp(e.message), end\n",
            "try, g = gamma(171), catch e, disp(e.message), end\n",
            "try, f = factorial([20 171]), catch e, disp(e.message), end\n",
            "try, t = chebyshevT(3, 1+1i), catch e, disp(e.message), end\n",
            "try, t = chebyshevT(2, [0.5 2]), catch e, disp(e.message), end\n",
            "try, z = zeta(0), catch e, disp(e.message), end\n",
            "try, z = zeta([2 -1]), catch e, disp(e.message), end\n",
            "try, z = zeta(1, 2), catch e, disp(e.message), end\n",
            "try, d = dot([1 2; 3 4], [1 0; 0 1]), catch e, disp(e.message), end\n",
            "try, c = cross([1 0 0; 0 1 0], [0 1 0; 0 0 1]), catch e, disp(e.message), end\n",
            "try, D = diag([1+2i 3-1i]), catch e, disp(e.message), end\n",
            "try, C = nchoosek([2 4 6 8], 3), catch e, disp(e.message), end\n",
            "try, [X, Y] = ndgrid(1:2, 1:3), catch e, disp(e.message), end\n",
            "try, c = circshift([1 2 3; 4 5 6], [1 -1]), catch e, disp(e.message), end\n",
            "try, l = linspace(1+1i, 3+3i, 3), catch e, disp(e.message), end\n",
            "try, m = max('ab'), catch e, disp(e.message), end\n",
            "try, m = median(int8([1 2])), catch e, disp(e.message), end\n",
            "try, c = cumsum([1 2 3], 'reverse'), catch e, disp(e.message), end\n",
            "try, c = cumsum([1 NaN 3], 'omitnan'), catch e, disp(e.message), end\n",
            "try, m = magic(2), catch e, disp(e.message), end\n",
        ),
        &[
            "s = 0.6000 + 0.8000i",
            "s = 1×3 complex\n\n   -1.0000 + 0.0000i    0.0000 + 0.0000i    0.0000 + 1.0000i",
            "f = 7.2574e+306",
            "g = 7.2574e+306",
            "f = 1×2\n\n   1.0e+18 *\n\n    2.4329       Inf",
            "t = -11.0000 + 5.0000i",
            "t = 1×2\n\n   -0.5000    7.0000",
            "z = -0.5000",
            "z = 1×2\n\n    1.6449   -0.0833",
            "z = -0.9375",
            "d = 1×2\n\n     1     4",
            "c = 2×3\n\n     0     0     1\n     1     0     0",
            "D = 2×2 complex\n\n   1.0000 + 2.0000i   0.0000 + 0.0000i\n   0.0000 + 0.0000i   3.0000 - 1.0000i",
            "C = 4×3\n\n     2     4     6\n     2     4     8\n     2     6     8\n     4     6     8",
            "X = 2×3\n\n     1     1     1\n     2     2     2",
            "Y = 2×3\n\n     1     2     3\n     1     2     3",
            "c = 2×3\n\n     5     6     4\n     2     3     1",
            "l = 1×3 complex\n\n   1.0000 + 1.0000i   2.0000 + 2.0000i   3.0000 + 3.0000i",
            "m = 98",
            "m = int8\n   2",
            "c = 1×3\n\n     6     5     3",
            "c = 1×3\n\n     1     1     4",
            "m = 2×2\n\n     1     3\n     4     2",
        ],
    );
}

#[test]
fn sets_and_text() {
    check(
        concat!(
            "try, [C, ia, ic] = unique([1 2; 3 4; 1 2], 'rows'), catch e, disp(e.message), end\n",
            "try, [C, ia] = unique([3 1 3 2 1], 'last'), catch e, disp(e.message), end\n",
            "try, [tf, loc] = ismember([1 2; 5 6], [5 6; 1 2; 3 4], 'rows'), catch e, disp(e.message), end\n",
            "try, v = sscanf('0x1F', '%x'), catch e, disp(e.message), end\n",
            "try, v = sscanf('ff 10', '%x'), catch e, disp(e.message), end\n",
            "try, v = str2double('1+2i'), catch e, disp(e.message), end\n",
            "try, d = duration('01:30:00'), catch e, disp(e.message), end\n",
            "try, c = char('a', 'bcd'), catch e, disp(e.message), end\n",
            "try, c = char(72, 105), catch e, disp(e.message), end\n",
            "try, f = fieldnames(struct()); size(f), catch e, disp(e.message), end\n",
            "try, e = isempty(struct([])), catch e, disp(e.message), end\n",
            "try, c = validatecolor('#0F0'), catch e, disp(e.message), end\n",
            "try, b = dec2bin(-5), catch e, disp(e.message), end\n",
            "try, h = dec2hex(-1), catch e, disp(e.message), end\n",
        ),
        &[
            "C = 2×2\n\n     1     2\n     3     4",
            "ia = 2×1\n\n     1\n     2",
            "ic = 3×1\n\n     1\n     2\n     1",
            "C = 1×3\n\n     1     2     3",
            "ia = 3×1\n\n     5\n     4\n     3",
            "tf = 2×1 logical array\n\n   1\n   1",
            "loc = 2×1\n\n     2\n     1",
            "v = 31",
            "v = 2×1\n\n   255\n    16",
            "v = 1.0000 + 2.0000i",
            "d = duration\n   01:30:00",
            "c = 2×3 char array\n\n    'a  '\n    'bcd'",
            "c = 2×1 char array\n\n    'H'\n    'i'",
            "ans = 1×2\n\n     0     1",
            "e = logical\n   1",
            "c = 1×3\n\n     0     1     0",
            "b = '11111011'",
            "h = 'FF'",
        ],
    );
}

#[test]
fn misc_functions() {
    check(
        concat!(
            "try, x = and(1, 2, 0), catch e, disp(e.message), end\n",
            "try, x = NaN & 1, catch e, disp(e.message), end\n",
            "try, t = tic; class(t), e = toc(t); isa(e, 'double') && e >= 0, catch e, disp(e.message), end\n",
            "try, [M, F, C] = mode([1 2 2 3 3]), catch e, disp(e.message), end\n",
            "try, p = pi('single'); class(p), catch e, disp(e.message), end\n",
            "try, [x, y] = unmesh([1 1 0 0; 0 0 1 0]), catch e, disp(e.message), end\n",
        ),
        &[
            "Too many input arguments.\n",
            "NaN values cannot be converted to logicals.\n",
            "ans = 'uint64'",
            "ans = logical\n   1",
            "M = 2",
            "F = 2",
            "C = 1×1 cell\n\n    {2×1 double}",
            "Too many input arguments.\n",
            "x = \n\n   (1,1)        2\n   (2,1)       -1\n   (3,1)       -1\n   (1,2)       -1\n   (2,2)        1\n   (1,3)       -1\n   (3,3)        1",
            "y = 3×2\n\n     0     0\n     1     0\n     1     1",
        ],
    );
}
