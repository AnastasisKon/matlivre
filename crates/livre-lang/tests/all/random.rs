//! The Mersenne Twister streams behind rand, randi, randperm, rng and RandStream, against
//! sequences recorded from MATLAB R2023b.

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
fn seeded_sequences_match_matlab() {
    assert_eq!(
        shown(concat!(
            "format long\nrng(0); a = rand(1,5)\nrng(1); b = rand(1,3)\nrng(0); c = randi(10,1,5)\nrng(0); c2 = randi([5 8],1,5)\n",
            "rng default; d = rand\nrng(42, 'twister'); e = rand\nrng(0); i1 = randi(1000)\nrng(0); ii2 = randi(intmax('int32'))\n",
            "format short\nrng(0); x = rand(2,2,'single')\n",
        )),
        [
            "a = 1×5\n\n   0.814723686393179   0.905791937075619   0.126986816293506   0.913375856139019   0.632359246225410",
            "b = 1×3\n\n   0.417022004702574   0.720324493442158   0.000114374817345",
            "c = 1×5\n\n     9    10     2    10     7",
            "c2 = 1×5\n\n     8     8     5     8     7",
            "d = 0.814723686393179",
            "e = 0.374540118847362",
            "i1 = 815",
            "ii2 = 1.749605794000000e+09",
            "x = 2×2 single matrix\n\n    0.8147    0.1270\n    0.9058    0.9134",
        ]
    );
}

#[test]
fn permutations_match_matlab() {
    assert_eq!(
        shown(concat!(
            "rng(0); p = randperm(6)\nrng(0); p10 = randperm(10)\nrng(0); p3 = randperm(100,3)\nrng(0); p4 = randperm(5,5)\n",
            "rng(0); p5 = randperm(40,3)\nx = rand\nrng(0); p6 = randperm(5,1)\n",
        )),
        [
            "p = 1×6\n\n     6     3     5     1     2     4",
            "p10 = 1×10\n\n     6     3     7     8     5     1     2     4     9    10",
            "p3 = 1×3\n\n    82    90    13",
            "p4 = 1×5\n\n     4     2     3     1     5",
            "p5 = 1×3\n\n    33    36     5",
            "x = 0.9134",
            "p6 = 5",
        ]
    );
}

#[test]
fn rng_settings_round_trip() {
    assert_eq!(
        shown("rng(5); s = rng\ns.State(1:3)'\nrng(s); a = rand\nrng(5); b = rand\nrng shuffle; t = rng; class(t.Seed)\n"),
        [
            "s = struct with fields:\n\n     Type: 'twister'\n     Seed: 5\n    State: [625×1 uint32]",
            "ans = 1×3 uint32 row vector\n\n            5    472231674   2253902500",
            "a = 0.2220",
            "b = 0.2220",
            "ans = 'uint32'",
        ]
    );
}

#[test]
fn rand_streams() {
    assert_eq!(
        shown(concat!(
            "format long\nrs = RandStream('mt19937ar', 'Seed', 3)\nq = rand(rs, 1, 3)\nreset(rs); q2 = rand(rs, 1, 3)\nrs.Seed\nrs.Type\n",
            "randn(rs, 1, 2); qi = randi(rs, 5, 1, 4)\n",
            "rs2 = RandStream('mt19937ar', 'Seed', 3, 'NormalTransform', 'Inversion'); qinv = randn(rs2, 1, 2)\n",
            "rs3 = RandStream('mt19937ar', 'Seed', 3, 'NormalTransform', 'Polar'); qpol = randn(rs3, 1, 3)\n",
            "format short\nrs4 = RandStream('mt19937ar','Seed',1,'NormalTransform','Polar'); randn(rs4); rs5 = rs4; randn(rs5)\n",
            "g = RandStream.getGlobalStream\n",
            "old = RandStream.setGlobalStream(RandStream('mt19937ar','Seed',7)); rand\n",
            "c3 = RandStream.create('mt19937ar', 'Seed', 9); rand(c3)\n",
            "reset(c3, 4); c3.Seed\n",
            "try, RandStream('foo'), catch e, disp(e.message), end\n",
        )),
        [
            "rs = \nmt19937ar random stream\n             Seed: 3\n  NormalTransform: Ziggurat",
            "q = 1×3\n\n   0.550797902574576   0.708147822618105   0.290904738912944",
            "q2 = 1×3\n\n   0.550797902574576   0.708147822618105   0.290904738912944",
            "ans = uint32\n   3",
            "ans = 'mt19937ar'",
            "qi = 1×4\n\n     5     1     2     1",
            "qinv = 1×2\n\n   0.127677502385859   0.547981865540973",
            "qpol = 1×3\n\n    0.436509850511989    1.788628473430319   -1.863492703364491",
            "ans = 1.6243",
            "g = \nmt19937ar random stream (current global stream)\n             Seed: 0\n  NormalTransform: Ziggurat",
            "ans = 0.0763",
            "ans = 0.0104",
            "ans = uint32\n   4",
            "Unknown RNG type: foo.\n",
        ]
    );
}
