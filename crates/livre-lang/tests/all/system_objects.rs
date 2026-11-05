//! System objects (matlab.System: step, setup, reset, release, isLocked, isDone, clone),
//! against MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

const CLASSES: &[(&str, &str)] = &[
    ("SysCounter", "classdef SysCounter < matlab.System\n    properties\n        Step = 1\n    end\n    properties (Access = private)\n        Count\n    end\n    methods (Access = protected)\n        function setupImpl(obj)\n            obj.Count = 0;\n            disp('setup')\n        end\n        function y = stepImpl(obj, u)\n            obj.Count = obj.Count + obj.Step*u;\n            y = obj.Count;\n        end\n        function resetImpl(obj)\n            obj.Count = 0;\n            disp('reset')\n        end\n        function releaseImpl(obj)\n            disp('release')\n        end\n    end\nend\n"),
    ("SysSource", "classdef SysSource < matlab.System & matlab.system.mixin.FiniteSource\n    properties\n        N = 3\n    end\n    properties (Access = private)\n        K\n    end\n    methods (Access = protected)\n        function setupImpl(obj)\n            obj.K = 0;\n        end\n        function y = stepImpl(obj)\n            obj.K = obj.K + 1;\n            y = obj.K;\n        end\n        function tf = isDoneImpl(obj)\n            tf = obj.K >= obj.N;\n        end\n        function [a, b] = dummy(obj)\n            a = 1; b = 2;\n        end\n    end\nend\n"),
];

fn shown(src: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join(format!("matlivre-sysobj-{}-{}", std::process::id(), src.len()));
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

fn check(src: &str, expected: &[&str]) {
    assert_eq!(shown(src), expected, "{src}");
}

#[test]
fn step_setup_reset_and_release() {
    check(
        concat!(
            "c = SysCounter\n",
            "l0 = isLocked(c)\n",
            "y1 = step(c, 2)\n",
            "y2 = c(3)\n",
            "l1 = isLocked(c)\n",
            "try, d0 = isDone(c), catch e, disp(e.message), end\n",
            "reset(c)\n",
            "y3 = step(c, 1)\n",
            "y5 = step(c, 1)\n",
            "release(c)\n",
            "l2 = isLocked(c)\n",
            "c.Step = 10;\n",
            "y6 = step(c, 1)\n",
            "class(c)\n",
            "isa(c, 'matlab.System')\n",
            "isa(c, 'handle')\n",
            "release(c)\n",
            "setup(c, 1)\n",
            "l3 = isLocked(c)\n",
            "c3 = clone(c);\n",
            "l4 = isLocked(c3)\n",
            "c3.Step = 2;\n",
            "c.Step\n",
        ),
        &[
            "c = \n  SysCounter with properties:\n\n    Step: 1",
            "l0 = logical\n   0",
            "setup\n",
            "reset\n",
            "y1 = 2",
            "y2 = 5",
            "l1 = logical\n   1",
            "Undefined function 'isDone' for input arguments of type 'SysCounter'.\n",
            "reset\n",
            "y3 = 1",
            "y5 = 2",
            "release\n",
            "l2 = logical\n   0",
            "setup\n",
            "reset\n",
            "y6 = 10",
            "ans = 'SysCounter'",
            "ans = logical\n   1",
            "ans = logical\n   1",
            "release\n",
            "setup\n",
            "l3 = logical\n   1",
            "Warning: The SysCounter System object has private or protected properties, but does not implement both the saveObjectImpl and loadObjectImpl methods. The save, load, and clone methods may not copy the object exactly when it is locked.",
            "l4 = logical\n   1",
            "ans = 10",
        ],
    );
}

#[test]
fn finite_sources_and_clone() {
    check(
        concat!(
            "s = SysSource;\n",
            "v1 = s()\n",
            "v2 = step(s)\n",
            "dn1 = isDone(s)\n",
            "v3 = s()\n",
            "dn2 = isDone(s)\n",
            "c = SysCounter;\n",
            "y1 = step(c, 2);\n",
            "c2 = clone(c);\n",
            "y4 = step(c2, 1)\n",
        ),
        &[
            "v1 = 1",
            "v2 = 2",
            "dn1 = logical\n   0",
            "v3 = 3",
            "dn2 = logical\n   1",
            "setup\n",
            "reset\n",
            "Warning: The SysCounter System object has private or protected properties, but does not implement both the saveObjectImpl and loadObjectImpl methods. The save, load, and clone methods may not copy the object exactly when it is locked.",
            "y4 = []",
        ],
    );
}
