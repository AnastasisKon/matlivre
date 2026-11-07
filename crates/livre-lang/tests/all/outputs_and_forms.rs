//! Missing outputs, options and call forms reported while documenting the wiki, against
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
fn extra_outputs() {
    check(
        concat!(
            "[c, m] = strsplit('a,b;c', {',', ';'})\n",
            "[c, m] = strsplit('a,,b', ',')\n",
            "c = strsplit('a,,b', ',', 'CollapseDelimiters', false)\n",
            "c = strsplit(\"x  y\")\n",
            "[n, s] = weekday(738000)\n",
            "[n, s] = weekday([738000 738001], 'long')\n",
            "[g, c, d] = gcd([12 7 0], [18 -3 5])\n",
            "[f, e] = log2([8 10 0.3 -6 0])\n",
            "[p, S, mu] = polyfit(1:5, [2 4 7 9 12], 1);\n",
            "p, S.df, mu\n",
            "[u, k] = matlab.lang.makeUniqueStrings({'a', 'a', 'b'})\n",
            "v = matlab.lang.makeValidName('1a', 'Prefix', 'p_')\n",
            "v = matlab.lang.makeValidName('a-b c', 'ReplacementStyle', 'delete')\n",
            "v = matlab.lang.makeValidName('a-b', 'ReplacementStyle', 'hex')\n",
            "v = matlab.lang.makeValidName('for')\n",
            "r = cellfun(@(x) error('my:id', 'bad'), {1, 2}, 'ErrorHandler', @(s, x) s.index * 10)\n",
            "[xb, yb] = stairs([1 2 4], [5 3 6])\n",
        ),
        &[
            "c = 1×3 cell\n\n    {'a'}    {'b'}    {'c'}",
            "m = 1×2 cell\n\n    {','}    {';'}",
            "c = 1×2 cell\n\n    {'a'}    {'b'}",
            "m = 1×1 cell\n\n    {',,'}",
            "c = 1×3 cell\n\n    {'a'}    {0×0 char}    {'b'}",
            "c = 1×2 string array\n\n    \"x\"    \"y\"",
            "n = 3",
            "s = 'Tue'",
            "n = 1×2\n\n     3     4",
            "s = 2×9 char array\n\n    'Tuesday  '\n    'Wednesday'",
            "g = 1×3\n\n     6     1     5",
            "c = 1×3\n\n    -1     1     0",
            "d = 1×3\n\n     1     2     1",
            "f = 1×5\n\n    0.5000    0.6250    0.6000   -0.7500         0",
            "e = 1×5\n\n     4     4    -1     3     0",
            "p = 1×2\n\n    3.9528    6.8000",
            "ans = 3",
            "mu = 2×1\n\n    3.0000\n    1.5811",
            "u = 1×3 cell\n\n    {'a'}    {'a_1'}    {'b'}",
            "k = 1×3 logical array\n\n   0   1   0",
            "v = 'p_1a'",
            "v = 'abC'",
            "v = 'a0x2Db'",
            "v = 'xFor'",
            "r = 1×2\n\n    10    20",
            "xb = 5×1\n\n     1\n     2\n     2\n     4\n     4",
            "yb = 5×1\n\n     5\n     5\n     3\n     3\n     6",
        ],
    );
}

#[test]
fn matrix_operators() {
    check(
        concat!(
            "[V, D] = eig([1 2; 3 4])\n",
            "[V, D] = eig([0 1; -1 0])\n",
            "A = [4 1; 2 3]^0.5\n",
            "A = [1 2; 3 4]^0.5\n",
            "B = 2^[1 2; 3 4]\n",
            "x = [1 2; 2 4] \\ [1; 2]\n",
            "y = [1 2; 2 4] \\ [1; 3]\n",
            "z = [1 3] / [1 2; 2 4]\n",
        ),
        &[
            "V = 2×2\n\n   -0.8246   -0.4160\n    0.5658   -0.9094",
            "D = 2×2\n\n   -0.3723         0\n         0    5.3723",
            "V = 2×2 complex\n\n   0.7071 + 0.0000i   0.7071 + 0.0000i\n   0.0000 + 0.7071i   0.0000 - 0.7071i",
            "D = 2×2 complex\n\n   0.0000 + 1.0000i   0.0000 + 0.0000i\n   0.0000 + 0.0000i   0.0000 - 1.0000i",
            "A = 2×2\n\n    1.9621    0.2740\n    0.5479    1.6882",
            "A = 2×2 complex\n\n   0.5537 + 0.4644i   0.8070 - 0.2124i\n   1.2104 - 0.3186i   1.7641 + 0.1458i",
            "B = 2×2\n\n   10.4827   14.1519\n   21.2278   31.7106",
            "Warning: Matrix is singular to working precision.",
            "x = 2×1\n\n   NaN\n   NaN",
            "Warning: Matrix is singular to working precision.",
            "y = 2×1\n\n   Inf\n  -Inf",
            "Warning: Matrix is singular to working precision.",
            "z = 1×2\n\n   Inf  -Inf",
        ],
    );
}

#[test]
fn sort_options() {
    check(
        concat!(
            "s = sort([3 NaN 1], 'MissingPlacement', 'first')\n",
            "s = sort([3 NaN 1], 'descend', 'MissingPlacement', 'last')\n",
            "[s, i] = sort([-3 2 1 -2], 'ComparisonMethod', 'abs')\n",
            "t = [issorted([3 2 1], 'descend'), issorted([1 2 NaN]), issorted([NaN 1 2]), issorted([1 1 2], 'strictascend')]\n",
            "t = [issorted({'a', 'b', 'c'}), issorted([\"b\" \"a\"]), issorted([3 1; 4 2]), issorted([3 1; 4 2], 2)]\n",
            "t = issorted([1 3 2 0], 'monotonic')\n",
            "[r, i] = sortrows({'b'; 'a'; 'c'})\n",
            "r = sortrows([\"b\" \"x\"; \"a\" \"z\"; \"b\" \"a\"])\n",
            "r = sortrows({'b'; 'a'; 'c'}, 'descend')\n",
        ),
        &[
            "s = 1×3\n\n   NaN     1     3",
            "s = 1×3\n\n     3     1   NaN",
            "s = 1×4\n\n     1     2    -2    -3",
            "i = 1×4\n\n     3     2     4     1",
            "t = 1×4 logical array\n\n   1   1   0   0",
            "t = 1×4 logical array\n\n   1   0   1   0",
            "t = logical\n   0",
            "r = 3×1 cell\n\n    {'a'}\n    {'b'}\n    {'c'}",
            "i = 3×1\n\n     2\n     1\n     3",
            "r = 3×2 string array\n\n    \"a\"    \"z\"\n    \"b\"    \"a\"\n    \"b\"    \"x\"",
            "r = 3×1 cell\n\n    {'c'}\n    {'b'}\n    {'a'}",
        ],
    );
}

#[test]
fn text_forms() {
    check(
        concat!(
            "k = strfind({'abcab', 'xab'}, 'ab')\n",
            "k = strfind([\"aa\" \"aba\"], 'a')\n",
            "k = strfind('abc', 'b', 'ForceCellOutput', true)\n",
            "t = isspace(\"a b\")\n",
            "t = isstrprop(\"a1\", 'digit')\n",
            "t = isstrprop({'a1', 'b'}, 'alpha')\n",
            "t = iskeyword({'for', 'x'})\n",
            "h = height(ones(3, 4)), w = width(ones(3, 4))\n",
            "c = cast(int8(5), 'like', single(1)); class(c)\n",
            "try, anymissing([1 NaN], 1), catch e, disp(e.message), end\n",
        ),
        &[
            "k = 1×2 cell\n\n    {[1 4]}    {[2]}",
            "k = 1×2 cell\n\n    {[1 2]}    {[1 3]}",
            "k = 1×1 cell\n\n    {[2]}",
            "t = 1×3 logical array\n\n   0   1   0",
            "t = 1×2 logical array\n\n   0   1",
            "t = 1×2 cell\n\n    {[1 0]}    {[1]}",
            "t = logical\n   0",
            "h = 3",
            "w = 4",
            "ans = 'single'",
            "Too many input arguments.\n",
        ],
    );
}

#[test]
fn arguments_blocks() {
    check(
        concat!(
            "r = f1(3)\n",
            "r = f1(3, 'scale', 2)\n",
            "try, f1(-1), catch e, disp(e.message), end\n",
            "try, f1([1 2]), catch e, disp(e.message), end\n",
            "try, f1(3, 'Bogus', 1), catch e, disp(e.message), end\n",
            "try, f1(3, 'Scale'), catch e, disp(e.message), end\n",
            "try, f1(), catch e, disp(e.message), end\n",
            "try, f1(3, 'Scale', -2), catch e, disp(e.message), end\n",
            "v = f2(5)\n",
            "try, f2([1 2]), catch e, disp(e.message), end\n",
            "s = f3(1, 'B', 5)\n",
            "m = f4()\n",
            "try, f4('quad'), catch e, disp(e.message), end\n",
            "try, f5(1, 2), catch e, disp(e.message), end\n",
            "try, f6({1}), catch e, disp(e.message), end\n",
            "w = f6(uint8(7)); class(w)\n",
            "try, f7(ones(2, 3)), catch e, disp(e.message), end\n",
            "try, f8([1; 2]), catch e, disp(e.message), end\n",
            "try, f9([1 2]), catch e, disp(e.message), end\n",
            "try, f10([1 2]), catch e, disp(e.message), end\n",
            "function r = f1(x, opts)\n",
            "arguments\n",
            "    x (1,1) double {mustBePositive}\n",
            "    opts.Scale (1,1) double {mustBePositive} = 1\n",
            "end\n",
            "r = x * opts.Scale;\n",
            "end\n",
            "function v = f2(v)\n",
            "arguments\n",
            "    v (1,3) double\n",
            "end\n",
            "end\n",
            "function s = f3(a, opts)\n",
            "arguments\n",
            "    a\n",
            "    opts.A = 10\n",
            "    opts.B string\n",
            "end\n",
            "s = opts;\n",
            "end\n",
            "function m = f4(method)\n",
            "arguments\n",
            "    method (1,:) char {mustBeMember(method, {'linear', 'cubic'})} = 'linear'\n",
            "end\n",
            "m = method;\n",
            "end\n",
            "function z = f5(x, y)\n",
            "arguments\n",
            "    x double\n",
            "    y double {mustBeLessThan(y, x)}\n",
            "end\n",
            "z = x + y;\n",
            "end\n",
            "function w = f6(w)\n",
            "arguments\n",
            "    w double\n",
            "end\n",
            "end\n",
            "function q = f7(q)\n",
            "arguments\n",
            "    q (:,2)\n",
            "end\n",
            "end\n",
            "function q = f8(q)\n",
            "arguments\n",
            "    q (1,:)\n",
            "end\n",
            "end\n",
            "function q = f9(q)\n",
            "arguments\n",
            "    q (2,:)\n",
            "end\n",
            "end\n",
            "function q = f10(q)\n",
            "arguments\n",
            "    q (2,3)\n",
            "end\n",
            "end\n",
        ),
        &[
            "r = 3",
            "r = 6",
            "Invalid argument at position 1. Value must be positive.\n",
            "Invalid argument at position 1. Value must be a scalar.\n",
            "Invalid argument name 'Bogus'. Use 'Scale'.\n",
            "Invalid argument at position 2. Make sure name-value arguments include both the name and the value.\n",
            "Invalid argument list. Function requires 1 more input(s).\n",
            "Invalid value for 'Scale' argument. Value must be positive.\n",
            "v = 1×3\n\n     5     5     5",
            "Invalid argument at position 1. Value must be a vector with 3 elements.\n",
            "s = struct with fields:\n\n    A: 10\n    B: \"5\"",
            "m = 'linear'",
            "Invalid argument at position 1. Value must be a member of this set:\n    'linear'\n    'cubic'\n\n",
            "Invalid argument at position 2. Value must be less than 1.\n",
            "Invalid argument at position 1. Value must be of type double or be convertible to double.\n",
            "ans = 'double'",
            "Invalid argument at position 1. Value must be a matrix with 2 columns.\n",
            "ans = 1×2\n\n     1     2",
            "Invalid argument at position 1. Value must be a matrix with 2 rows.\n",
            "Invalid argument at position 1. Value must be a matrix of size 2-by-3.\n",
        ],
    );
}

#[test]
fn graph_outputs() {
    check(
        concat!(
            "G = graph([1 1 2 3 4], [2 3 4 4 5], [1 2 3 1 5]);\n",
            "D = digraph([1 1 2 3 4], [2 3 4 4 5], [1 2 3 1 5]);\n",
            "[p, d, e] = shortestpath(G, 1, 5)\n",
            "[TR, dist] = shortestpathtree(G, 1); dist\n",
            "[mf, GF, cs, ct] = maxflow(D, 1, 5); mf, GF.Edges.EndNodes', cs', ct'\n",
            "[P, E] = allpaths(D, 1, 5)\n",
            "[c, ec] = allcycles(digraph([1 2 3], [2 3 1]))\n",
            "[cy, ecy] = cyclebasis(G)\n",
            "[T, pred] = minspantree(G); pred\n",
            "[T, pred] = minspantree(G, 'Root', 3); pred\n",
            "[H, idx] = reordernodes(G, [5 4 3 2 1]); idx'\n",
            "[H, idx] = reordernodes(G, [2 1 3 5 4]); idx', H.Edges.EndNodes'\n",
            "[n, H] = toposort(digraph([3 1 2], [1 2 4])); n, H.Edges.EndNodes'\n",
        ),
        &[
            "p = 1×4\n\n     1     3     4     5",
            "d = 8",
            "e = 1×3\n\n     2     4     5",
            "dist = 1×5\n\n     0     1     2     3     8",
            "mf = 2",
            "ans = 2×5\n\n     1     1     2     3     4\n     2     3     4     4     5",
            "ans = 1×2\n\n     1     3",
            "ans = 1×3\n\n     2     4     5",
            "P = 2×1 cell\n\n    {[1 2 4 5]}\n    {[1 3 4 5]}",
            "E = 2×1 cell\n\n    {[1 3 5]}\n    {[2 4 5]}",
            "c = 1×1 cell\n\n    {[1 2 3]}",
            "ec = 1×1 cell\n\n    {[1 2 3]}",
            "cy = 1×1 cell\n\n    {[1 2 4 3]}",
            "ecy = 1×1 cell\n\n    {[1 3 4 2]}",
            "pred = 1×5\n\n     0     1     1     3     4",
            "pred = 1×5\n\n     3     1     0     3     4",
            "ans = 1×5\n\n     5     4     3     2     1",
            "ans = 1×5\n\n     1     3     2     4     5",
            "ans = 2×5\n\n     1     1     2     3     4\n     2     5     3     5     5",
            "n = 1×4\n\n     3     1     2     4",
            "ans = 2×3\n\n     1     2     3\n     2     3     4",
        ],
    );
}

#[test]
fn sparse_and_shapes() {
    check(
        concat!(
            "A = sparse([4 1 0; 1 4 1; 0 1 4]);\n",
            "[pe, q] = etree(A)\n",
            "[pe, q] = etree(sparse([4 0 1 0; 0 4 0 1; 1 0 4 0; 0 1 0 4]))\n",
            "[cnt, h, parent, post, R] = symbfact(A); cnt', h, parent, post, full(R)\n",
            "[L, U, P] = ilu(A); full(P)\n",
            "[x, y] = gplot([0 1; 1 0], [0 0; 1 1])\n",
            "ps = polyshape([0 2 2 0], [0 0 2 2]);\n",
            "[in, on] = isinterior(ps, [1 2 3 0], [1 1 1 2])\n",
        ),
        &[
            "pe = 1×3\n\n     2     3     0",
            "q = 1×3\n\n     1     2     3",
            "pe = 1×4\n\n     3     4     0     0",
            "q = 1×4\n\n     1     3     2     4",
            "ans = 3×1\n\n     2\n     2\n     1",
            "h = 3",
            "parent = 1×3\n\n     2     3     0",
            "post = 1×3\n\n     1     2     3",
            "ans = 3×3\n\n     1     1     0\n     0     1     1\n     0     0     1",
            "ans = 3×3\n\n     1     0     0\n     0     1     0\n     0     0     1",
            "x = 6×1\n\n     1\n     0\n   NaN\n     0\n     1\n   NaN",
            "y = 6×1\n\n     1\n     0\n   NaN\n     0\n     1\n   NaN",
            "in = 4×1 logical array\n\n   1\n   1\n   0\n   1",
            "on = 4×1 logical array\n\n   0\n   1\n   0\n   1",
        ],
    );
}

#[test]
fn plot_outputs() {
    check(
        concat!(
            "set(0, 'DefaultFigureVisible', 'off');\n",
            "[h, ax] = pareto([3 1 2]); numel(h), class(ax)\n",
            "h = fplot(@sin); isgraphics(h)\n",
            "h = fplot(@cos, @sin, [0 2*pi]); x = h.XData; [x(1) x(end)]\n",
            "y = h.YData; round([y(1) y(end)], 4)\n",
        ),
        &[
            "ans = 2",
            "ans = 'matlab.graphics.axis.Axes'",
            "ans = logical\n   1",
            "ans = 1×2\n\n     1     1",
            "ans = 1×2\n\n     0     0",
        ],
    );
}

#[test]
fn ignored_options() {
    check(
        concat!(
            "c = addcats(categorical({'b', 'a'}), {'y', 'z'}, 'Before', 'b'); categories(c)', double(c)\n",
            "c = addcats(categorical({'b', 'a'}), 'm', 'After', 'a'); categories(c)'\n",
            "try, polybuffer([0 0; 1 0], 'lines', 0.5, 'NumPoints', 8), catch e, disp(e.message), end\n",
            "d = distances(graph([1 2], [2 3], [5 7]), 'Method', 'unweighted')\n",
            "T = table([1; 1; 2], [3; 4; 5], 'VariableNames', {'g', 'x'});\n",
            "varfun(@mean, T, 'GroupingVariables', 'g')\n",
            "rowfun(@(x) x * 2, T, 'GroupingVariables', 'g', 'InputVariables', 'x')\n",
            "s = table2struct(T, 'ToScalar', true)\n",
            "F = fillmissing2([1 NaN 3; 4 5 NaN; 7 8 9], 'linear')\n",
            "c = textscan('1,,3', '%f %f %f', 'Delimiter', ',', 'EmptyValue', -1); [c{:}]\n",
            "abc1 = 1; abd2 = 2; xyz = 3; clear -regexp ^ab\n",
            "exist('abc1'), exist('xyz')\n",
            "q1 = 1; q22 = 2; clear q*2; exist('q1'), exist('q22')\n",
            "T2 = table([1; 2], [7; 8], 'VariableNames', {'g', 'y'});\n",
            "J = join(T, T2)\n",
            "s = extractBetween('abcdef', 2, 4)\n",
            "s = extractBetween(\"abcdef\", 2, 4)\n",
            "s = insertAfter('abcdef', 2, 'XX')\n",
            "s = insertBefore(\"abcdef\", 3, \"--\")\n",
            "[hh, mm, ss] = hms(duration(1, 2, 3.5))\n",
            "t = issortedrows(table([1; 2; 2], [3; 1; 2]))\n",
            "f = [tempname '.csv']; writematrix([\"a\" \"b\"; \"c\" \"d\"], f); disp(fileread(f)); delete(f)\n",
            "T3 = convertvars(T, @(v) all(v > 2), 'string'); class(T3.x), class(T3.g)\n",
        ),
        &[
            "ans = 1×4 cell\n\n    {'a'}    {'y'}    {'z'}    {'b'}",
            "ans = 1×2\n\n     4     1",
            "ans = 1×3 cell\n\n    {'a'}    {'m'}    {'b'}",
            "Parameter name must be 'JointType' or 'MiterLimit'.\n",
            "d = 3×3\n\n     0     1     2\n     1     0     1\n     2     1     0",
            "ans = 2×3 table\n\n    g    GroupCount    mean_x\n    _    __________    ______\n\n    1        2          3.5  \n    2        1            5  ",
            "ans = 3×3 table\n\n    g    GroupCount    Var3\n    _    __________    ____\n\n    1        2           6 \n    1        2           8 \n    2        1          10 ",
            "s = struct with fields:\n\n    g: [3×1 double]\n    x: [3×1 double]",
            "F = 3×3\n\n     1     2     3\n     4     5     6\n     7     8     9",
            "ans = 1×3\n\n     1    -1     3",
            "ans = 0",
            "ans = 1",
            "ans = 1",
            "ans = 0",
            "J = 3×3 table\n\n    g    x    y\n    _    _    _\n\n    1    3    7\n    1    4    7\n    2    5    8",
            "s = 1×1 cell\n\n    {'bcd'}",
            "s = \"bcd\"",
            "s = 'abXXcdef'",
            "s = \"ab--cdef\"",
            "hh = 1",
            "mm = 2",
            "ss = 3.5000",
            "t = logical\n   1",
            "a,b\nc,d\n\n",
            "ans = 'string'",
            "ans = 'double'",
        ],
    );
}

#[test]
fn format_modes() {
    check(
        concat!(
            "format short e\n",
            "a = pi, f = [pi -2.5 1e5], e = [1 2 3], disp([1.5 -2.25])\n",
            "format long e\n",
            "a = pi, f = [pi -2.5 1e5]\n",
            "format short g\n",
            "a = pi, c = -0.000123456, g = [0.001 0.5; -7 1234.5678]\n",
            "format long g\n",
            "b = 314159.26535, f = [pi -2.5 1e5]\n",
            "format bank\n",
            "c = -0.000123456, n = [NaN Inf -Inf 0], f = [pi -2.5 1e5]\n",
            "format rat\n",
            "a = pi, f = [pi -2.5 1e5], g = [0.001 0.5; -7 1234.5678]\n",
            "format hex\n",
            "a = pi, n = [NaN Inf -Inf 0], s = single(pi)\n",
            "format +\n",
            "e = [1 -2 0], disp([NaN -Inf 3])\n",
            "format compact\n",
            "x = 0.5\n",
            "format\n",
            "x = 0.5\n",
            "format longG\n",
            "x = 1/3\n",
            "format\n",
        ),
        &[
            "a = 3.1416e+00",
            "f = 1×3\n\n   3.1416e+00  -2.5000e+00   1.0000e+05",
            "e = 1×3\n\n     1     2     3",
            "   1.5000e+00  -2.2500e+00\n",
            "a = 3.141592653589793e+00",
            "f = 1×3\n\n     3.141592653589793e+00    -2.500000000000000e+00     1.000000000000000e+05",
            "a = 3.1416",
            "c = -0.00012346",
            "g = 2×2\n\n        0.001          0.5\n           -7       1234.6",
            "b = 314159.26535",
            "f = 1×3\n\n          3.14159265358979                      -2.5                    100000",
            "c = -0.00",
            "n = 1×4\n\n           NaN           Inf          -Inf             0",
            "f = 1×3\n\n          3.14         -2.50     100000.00",
            "a = 355/113",
            "f = 1×3\n\n     355/113         -5/2       100000       ",
            "g = 2×2\n\n       1/1000         1/2     \n      -7          45679/37    ",
            "a = 400921fb54442d18",
            "n = 1×4\n\n   fff8000000000000   7ff0000000000000   fff0000000000000   0000000000000000",
            "s = single\n   40490fdb",
            "e = 1×3\n\n+- ",
            "+-+\n",
            "x = +",
            "x = 0.5000",
            "x = 0.333333333333333",
        ],
    );
}
