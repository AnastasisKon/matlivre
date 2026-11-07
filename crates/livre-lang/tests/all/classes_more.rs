//! Events and listeners, enumerations, class queries and tolerant set functions against
//! MATLAB R2023b (Live Editor output).

use livre_lang::{Interpreter, OutputKind};

const CLASSES: &[(&str, &str)] = &[
    (
        "Color",
        r#"classdef Color
  properties
    R
    G
    B
  end
  methods
    function c = Color(r, g, b)
      c.R = r; c.G = g; c.B = b;
    end
  end
  enumeration
    Red (1, 0, 0)
    Green (0, 1, 0)
    Blue (0, 0, 1)
  end
end
"#,
    ),
    ("Day", "classdef Day\n  enumeration\n    Mon, Tue, Wed\n  end\nend\n"),
    ("Dyn", "classdef Dyn < dynamicprops\n  properties\n    A = 1\n  end\nend\n"),
    (
        "Bank",
        r#"classdef Bank < handle
  properties
    Balance = 0
  end
  events
    Low
    Deposit
  end
  methods
    function deposit(b, x)
      b.Balance = b.Balance + x;
      notify(b, 'Deposit');
      if b.Balance < 10, notify(b, 'Low', BankData(b.Balance)); end
    end
  end
end
"#,
    ),
    (
        "BankData",
        r#"classdef BankData < event.EventData
  properties
    Amount
  end
  methods
    function d = BankData(a)
      d.Amount = a;
    end
  end
end
"#,
    ),
];

fn shown(src: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("matlivre-classes-more-{}-{}", std::process::id(), src.len()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in CLASSES {
        std::fs::write(dir.join(format!("{name}.m")), text).unwrap();
    }
    let mut it = Interpreter::new();
    it.set_search_path(vec![dir.clone()]);
    let _ = it.run_source(src);
    let _ = std::fs::remove_dir_all(&dir);
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
fn enumerations_match_matlab() {
    assert_eq!(
        shown(concat!(
            "c = Color.Red\ng = Color.Green.G\nt = c == Color.Red\nf = c == Color.Blue\nn = char(c)\ns = string(Day.Tue)\nd = Day.Wed\n",
            "enumeration('Day')\n[m, names] = enumeration('Color');\nnames\nsz = size(m)\n",
            "switch d\n  case Day.Mon\n    disp('monday')\n  case Day.Wed\n    disp('wednesday')\nend\nw = d == 'Wed'\n",
            "mc = meta.class.fromName('Color'); en = {mc.EnumerationMemberList.Name}\n",
        )),
        [
            "c = \n  Color enumeration\n\n    Red",
            "g = 1",
            "t = logical\n   1",
            "f = logical\n   0",
            "n = 'Red'",
            "s = \"Tue\"",
            "d = \n  Day enumeration\n\n    Wed",
            "Enumeration members for class 'Day':\n\n    Mon\n    Tue\n    Wed\n\n",
            "names = 3×1 cell\n\n    {'Red'  }\n    {'Green'}\n    {'Blue' }",
            "sz = 1×2\n\n     3     1",
            "wednesday\n",
            "w = logical\n   1",
            "en = 1×3 cell\n\n    {'Red'}    {'Green'}    {'Blue'}",
        ]
    );
}

#[test]
fn events_and_listeners_match_matlab() {
    assert_eq!(
        shown(concat!(
            "b = Bank;\nlh = addlistener(b, 'Deposit', @(src, evt) disp(['deposit: ' evt.EventName ' ' class(src)]));\n",
            "l2 = addlistener(b, 'Low', @(src, evt) fprintf('low %g %s\\n', evt.Amount, class(evt)));\n",
            "b.deposit(5)\nh = event.hasListener(b, 'Low')\nev = events(b)\nevents('Bank')\ndelete(lh)\nb.deposit(1)\ncl = class(l2)\n",
            "l2.Enabled = false; b.deposit(1)\n",
            "try, notify(b, 'Nope'), catch e, disp(e.message), end\n",
            "sc = superclasses('BankData')\nsuperclasses('Bank')\n",
            "mc = meta.class.fromName('Bank'); mn = mc.Name\npn = {mc.PropertyList.Name}\nen = {mc.EventList.Name}\n",
        )),
        [
            "deposit: Deposit Bank\n",
            "low 5 BankData\n",
            "h = logical\n   1",
            "ev = 3×1 cell\n\n    {'Low'                 }\n    {'Deposit'             }\n    {'ObjectBeingDestroyed'}",
            "Events for class Bank:\n\n    Low\n    Deposit\n    ObjectBeingDestroyed\n\n",
            "low 6 BankData\n",
            "cl = 'event.listener'",
            "Event 'Nope' is not defined for class 'Bank'.\n",
            "sc = 2×1 cell\n\n    {'event.EventData'}\n    {'handle'         }",
            "Superclasses for class Bank:\n\n    handle\n\n",
            "mn = 'Bank'",
            "pn = 1×1 cell\n\n    {'Balance'}",
            "en = 1×3 cell\n\n    {'Low'}    {'Deposit'}    {'ObjectBeingDestroyed'}",
        ]
    );
}

#[test]
fn tolerant_sets_and_powermod_match_matlab() {
    assert_eq!(
        shown(concat!(
            "tf = ismembertol([1 2.0000001 3], [2 3 4])\n[tf2, loc] = ismembertol([1 2.0000001 3], [2 3 4], 1e-3)\n",
            "C = uniquetol([1 1.0000001 2 3 3.00000001])\n[C2, ia, ic] = uniquetol([3 1 1.05 2], 0.1)\n",
            "R = uniquetol([1 2; 1.01 2.01; 5 5], 0.01, 'ByRows', true)\np = powermod(3, 200, 7)\n",
        )),
        [
            "tf = 1×3 logical array\n\n   0   0   1",
            "tf2 = 1×3 logical array\n\n   0   1   1",
            "loc = 1×3\n\n     0     1     2",
            "C = 1×5\n\n    1.0000    1.0000    2.0000    3.0000    3.0000",
            "C2 = 1×3\n\n     1     2     3",
            "ia = 3×1\n\n     2\n     4\n     1",
            "ic = 4×1\n\n     3\n     1\n     1\n     2",
            "R = 2×2\n\n     1     2\n     5     5",
            "p = 2",
        ]
    );
}

#[test]
fn dynamic_properties() {
    let out = shown(concat!(
        "d = Dyn;\n",
        "p = addprop(d, 'Extra'); class(p), p.Name\n",
        "d.Extra = 5; d.Extra\n",
        "isprop(d, 'Extra')\n",
        "properties(d)\n",
        "d\n",
        "delete(p); isprop(d, 'Extra')\n",
        "try, addprop(d, 'A'), catch e, disp(e.message), end\n",
        "c = matlab.lang.correction.ConvertToFunctionNotationCorrection('foo')\n",
    ));
    assert_eq!(
        out,
        [
            "ans = 'meta.DynamicProperty'",
            "ans = 'Extra'",
            "ans = 5",
            "ans = logical\n   1",
            "\nProperties for class Dyn:\n\n    A\n    Extra\n\n",
            "d = \n  Dyn with properties:\n\n        A: 1\n    Extra: 5",
            "ans = logical\n   0",
            "'A' is already defined as a property.\n",
            "c = \n  ConvertToFunctionNotationCorrection with properties:\n\n    MethodToConvert: \"foo\"",
        ]
    );
}
