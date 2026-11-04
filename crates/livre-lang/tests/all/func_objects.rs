//! inline, fcnchk, memoize, timeit, and the comma-list and nested-cell display fixes found with
//! them, against MATLAB R2023b.

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
fn inline_functions() {
    assert_eq!(
        shown(concat!(
            "g = inline('x^2+y')\ng(2,3)\nargnames(g)\nformula(g)\nclass(g)\n",
            "g2 = inline('sin(t)*a')\ng3 = inline('x+q', 'q', 'x'); g3(1, 10)\n",
            "g4 = inline('x^2 + P1*x', 1)\nfeval(g4, 2, 3)\nv = vectorize(g)\nnargin(g)\n",
            "g6 = inline('pi*x + exp(i*y) + Inf + eps')\ng7 = inline('a.b + c')\n",
            "try, g(1), catch e, disp(e.message), end\ntry, g(1,2,3), catch e, disp(e.message), end\n",
            "try, inline(3), catch e, disp(e.message), end\n",
            "s = struct('f', g)\ndisp(inline('x'))\n",
        )),
        [
            "g = \n     Inline function:\n     g(x,y) = x^2+y",
            "ans = 7",
            "ans = 2×1 cell\n\n    {'x'}\n    {'y'}",
            "ans = 'x^2+y'",
            "ans = 'inline'",
            "g2 = \n     Inline function:\n     g2(a,t) = sin(t)*a",
            "ans = 11",
            "g4 = \n     Inline function:\n     g4(x,P1) = x^2 + P1*x",
            "ans = 10",
            "v = \n     Inline function:\n     v(x,y) = x.^2+y",
            "ans = 2",
            "g6 = \n     Inline function:\n     g6(x,y) = pi*x + exp(i*y) + Inf + eps",
            "g7 = \n     Inline function:\n     g7(a,c) = a.b + c",
            "Not enough inputs to inline function.\n",
            "Too many inputs to inline function.\n",
            "Input must be a character vector.\n",
            "s = struct with fields:\n\n    f: [1×1 inline]",
            "     Inline function:\n     (x) = x\n",
        ]
    );
}

#[test]
fn fcnchk_makes_handles_or_inlines() {
    assert_eq!(
        shown("h = fcnchk('sin')\nh2 = fcnchk('x+1')\nh3 = fcnchk(@cos)\nh4 = fcnchk('x.^2', 'vectorized'); h4([1 2])\nb = fcnchk('x+y', 'vectorized')\nc = fcnchk('foo_nonexistent')\n"),
        [
            "h = function_handle with value:\n\n    @sin",
            "h2 = \n     Inline function:\n     h2(x) = x+1",
            "h3 = function_handle with value:\n\n    @cos",
            "ans = 1×2\n\n     1     4",
            "b = \n     Inline function:\n     b(x,y) = x+y.*ones(size(x))",
            "c = function_handle with value:\n\n    @foo_nonexistent",
        ]
    );
}

#[test]
fn memoized_functions() {
    assert_eq!(
        shown(concat!(
            "f = memoize(@(x) x^2)\nclass(f)\nf(3)\nf(3)\nst = stats(f); st.Cache.TotalHits\nst.Cache.TotalMisses\n",
            "f.CacheSize = 5; f.CacheSize\nclearCache(f)\nst2 = stats(f); st2.Cache.TotalHits\n",
            "m = memoize(@sin); m2 = memoize(@sin); isequal(m, m2)\nm.CacheSize = 3; m2.CacheSize\n",
            "m(1); m(2); m(1);\ns = stats(m)\ns.Cache.HitCount\ns.MostHitCachedInput\n",
            "e = memoize(@exp); e.CacheSize = 2; e(1); e(2); e(1); e(3); s3 = stats(e); s3.Cache.Inputs{:}\n",
            "e.Enabled = false; e(5); s3 = stats(e); s3.Cache.TotalMisses\n",
            "t = memoize(@tan); s4 = stats(t)\n",
        )),
        [
            "f = \n  MemoizedFunction with properties:\n\n     Function: @(x)x^2\n      Enabled: 1\n    CacheSize: 10",
            "ans = 'matlab.lang.MemoizedFunction'",
            "ans = 9",
            "ans = 9",
            "ans = 1",
            "ans = 1",
            "ans = 5",
            "ans = 0",
            "ans = logical\n   1",
            "ans = 3",
            "s = struct with fields:\n\n                    Cache: [1×1 struct]\n       MostHitCachedInput: [1×1 struct]\n      CacheHitRatePercent: 33.3333\n    CacheOccupancyPercent: 66.6667",
            "ans = 1×2\n\n     1     0",
            "ans = struct with fields:\n\n     Hits: 1\n    Input: {[1]}",
            "ans = 1×1 cell\n\n    {[2]}",
            "ans = 1×1 cell\n\n    {[3]}",
            "ans = 3",
            "s4 = struct with fields:\n\n                    Cache: [1×1 struct]\n       MostHitCachedInput: []\n      CacheHitRatePercent: 0\n    CacheOccupancyPercent: 0",
        ]
    );
}

#[test]
fn timeit_and_lists() {
    assert_eq!(
        shown("t = timeit(@() 1); class(t), t >= 0\nc = {{2},{3}};\nc{:}\ns.a = {{1}, {2}}\n"),
        [
            "ans = 'double'",
            "ans = logical\n   1",
            "ans = 1×1 cell\n\n    {[2]}",
            "ans = 1×1 cell\n\n    {[3]}",
            "s = struct with fields:\n\n    a: {{1×1 cell}  {1×1 cell}}",
        ]
    );
}
