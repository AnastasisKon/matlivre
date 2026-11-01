//! Text patterns (`digitsPattern`, `wildcardPattern`, boundaries, `+`, `|`) and the text
//! functions that take them, plus `strjust`, against MATLAB R2023b.

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

/// The values of string variables, element by element.
fn strings(src: &str, names: &[&str]) -> Vec<Vec<String>> {
    let mut it = Interpreter::new();
    it.run_source(src).unwrap_or_else(|e| panic!("{src}: {e}"));
    names
        .iter()
        .map(|n| match it.get_var(n).unwrap_or_else(|| panic!("no {n}")) {
            livre_lang::Value::Str(s) => s.data.iter().map(|x| x.clone().unwrap_or_default()).collect(),
            other => vec![other.text().unwrap()],
        })
        .collect()
}

#[test]
fn patterns_display_like_matlab() {
    assert_eq!(
        shown("p1 = digitsPattern\np2 = lettersPattern(2, 4)\np3 = \"ab\" + digitsPattern + optionalPattern(\"x\")\np4 = digitsPattern | lettersPattern\nnp = namedPattern(digitsPattern, \"num\")\nc = class(p1)\n"),
        [
            "p1 = pattern\n  Matching:\n\n    digitsPattern",
            "p2 = pattern\n  Matching:\n\n    lettersPattern(2,4)",
            "p3 = pattern\n  Matching:\n\n    \"ab\" + digitsPattern + optionalPattern(\"x\")",
            "p4 = pattern\n  Matching:\n\n    digitsPattern | lettersPattern",
            "np = pattern\n  Matching num:\n\n    digitsPattern",
            "c = 'pattern'",
        ]
    );
}

#[test]
fn extracting_and_replacing() {
    let src = concat!(
        "s = \"abc 123 de45 f6789 xyz\";\n",
        "e1 = extract(s, digitsPattern); e2 = extract(s, lettersPattern(2)); e3 = extract(s, digitsPattern(2,3)); e4 = extract(\"a1b22c333\", digitsPattern(2));\n",
        "r1 = replace(s, digitsPattern, \"#\"); sp = split(\"a1b22c333d\", digitsPattern); er = erase(\"a1b2c3\", digitsPattern);\n",
        "w1 = extract(\"key=value; k2=v2\", wildcardPattern + \"=\"); w2 = extractAfter(\"name: John Smith\", \":\" + whitespacePattern);\n",
        "w3 = extractBefore(\"file.tar.gz\", \".\" + wildcardPattern + textBoundary(\"end\"));\n",
        "b1 = extract(\"one two three\", lettersPattern + whitespaceBoundary(\"start\")); k1 = extract(\"ABC abc AbC\", caseInsensitivePattern(\"abc\"));\n",
        "m1 = extract(\"x=10, y=20\", lookBehindBoundary(\"=\") + digitsPattern); m2 = extract(\"10px 20em 30px\", digitsPattern + lookAheadBoundary(\"px\"));\n",
        "cl = extract(\"hello world\", characterListPattern(\"aeiou\")); rp = extract(\"abc123def\", regexpPattern(\"[a-c]+\"));\n",
        "am = extract(\"aaa bb a\", asManyOfPattern(\"a\", 2)); af = extract(\"<a><b>\", \"<\" + asFewOfPattern(wildcardPattern) + \">\");\n",
        "al = extract(\"x1 y22 z\", alphanumericsPattern); db = extract(\"a1 22b c3\", digitBoundary(\"end\") + whitespacePattern);\n",
        "lb = extract(sprintf('one\\ntwo'), lineBoundary(\"start\") + lettersPattern);\n",
    );
    let got = strings(src, &["e1", "e2", "e3", "e4", "r1", "sp", "er", "w1", "w2", "w3", "b1", "k1", "m1", "m2", "cl", "rp", "am", "af", "al", "db", "lb"]);
    let want: Vec<Vec<&str>> = vec![
        vec!["123", "45", "6789"],
        vec!["ab", "de", "xy"],
        vec!["123", "45", "678"],
        vec!["22", "33"],
        vec!["abc # de# f# xyz"],
        vec!["a", "b", "c", "d"],
        vec!["abc"],
        vec!["key=", "value; k2="],
        vec!["John Smith"],
        vec!["file"],
        vec!["one", "two"],
        vec!["ABC", "abc", "AbC"],
        vec!["10", "20"],
        vec!["10", "30"],
        vec!["e", "o", "o"],
        vec!["abc"],
        vec!["aaa"],
        vec!["<a>", "<b>"],
        vec!["x1", "y22", "z"],
        vec![" "],
        vec!["one", "two"],
    ];
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w);
    }
}

#[test]
fn predicates_and_counts() {
    assert_eq!(
        shown(concat!(
            "s = \"abc 123 de45 f6789 xyz\";\n",
            "t1 = contains(s, digitsPattern(4))\nt2 = startsWith(s, lettersPattern)\nt3 = endsWith(s, \"z\" + textBoundary(\"end\"))\nn = count(s, digitsPattern)\n",
            "k2 = matches([\"a12\" \"b\" \"c3\"], lettersPattern + digitsPattern)\nb2 = extract(\"The cat sat\", letterBoundary + \"at\")\n",
        )),
        ["t1 = logical\n   1", "t2 = logical\n   1", "t3 = logical\n   1", "n = 3", "k2 = 1×3 logical array\n\n   1   0   1", "b2 = \n  0×1 empty string array"]
    );
}

#[test]
fn strjust_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "a = ['a   ';'bb  ';' c  ';'dddd']; r = strjust(a); disp(['[' r(1,:) ']' '[' r(2,:) ']' '[' r(3,:) ']'])\n",
            "c = strjust(['a    ';'bb   ';'  c  '], 'center'); disp(['[' c(1,:) ']' '[' c(2,:) ']' '[' c(3,:) ']'])\n",
            "s = strjust(\"  x  \", 'left')\n",
        )),
        ["[   a][  bb][   c]\n", "[  a  ][ bb  ][  c  ]\n", "s = \"x    \""]
    );
}
