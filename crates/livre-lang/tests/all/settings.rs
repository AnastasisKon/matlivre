//! The settings API (settings, SettingsGroup, Setting and their methods), against MATLAB R2023b.

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

fn setting(path: &str, rows: [&str; 5]) -> String {
    format!(
        "  Setting '{path}' with properties:\n\n          ActiveValue: {}\n       TemporaryValue: {}\n        PersonalValue: {}\n    InstallationValue: {}\n         FactoryValue: {}",
        rows[0], rows[1], rows[2], rows[3], rows[4]
    )
}

#[test]
fn groups_and_values() {
    let none = "<no value>";
    assert_eq!(
        shown(concat!(
            "s = settings\nclass(s)\ng = addGroup(s, 'livretest')\ns.livretest.addSetting('x', 'PersonalValue', 5);\nx = s.livretest.x\n",
            "hasPersonalValue(x)\nhasTemporaryValue(x)\nhasFactoryValue(x)\nhasInstallationValue(x)\n",
            "x.TemporaryValue = 7;\nx\nclearTemporaryValue(x);\nx.ActiveValue\n",
            "try, x.TemporaryValue, catch e, disp(e.message), end\n",
            "addSetting(g, 'y', 'ValidationFcn', @matlab.settings.mustBeNumericScalar);\n",
            "try, g.y.PersonalValue = 'abc'; catch e, disp(e.message), end\n",
            "g.y.PersonalValue = 3;\ng.y.ActiveValue\ntry, g.y.FactoryValue, catch e, disp(e.message), end\n",
            "hasGroup(s, 'livretest')\nhasSetting(g, 'x')\nhasSetting(g, 'nope')\nh = addGroup(g, 'sub')\n",
            "removeSetting(g, 'y');\nhasSetting(g, 'y')\ng\n",
            "clearPersonalValue(x);\ntry, x.ActiveValue, catch e, disp(e.message), end\n",
            "addSetting(g, 'h', 'Hidden', true, 'PersonalValue', 1);\ng\n",
            "g.addSetting('k', 'PersonalValue', {1, 'a'}); k = g.k\ng.k.PersonalValue = \"str\"; k.ActiveValue\n",
            "removeGroup(s, 'livretest');\nhasGroup(s, 'livretest')\n",
        )),
        [
            "s = \n  SettingsGroup with properties:\n\n    matlab: [1×1 SettingsGroup]".to_string(),
            "ans = 'matlab.settings.SettingsGroup'".into(),
            "g = \n  SettingsGroup 'livretest' with no properties.".into(),
            format!("x = \n{}", setting("livretest.x", ["5", none, "5", none, none])),
            "ans = logical\n   1".into(),
            "ans = logical\n   0".into(),
            "ans = logical\n   0".into(),
            "ans = logical\n   0".into(),
            format!("x = \n{}", setting("livretest.x", ["7", "7", "5", none, none])),
            "ans = 5".into(),
            "No value is set for setting 'x' at temporary level.\n".into(),
            "Error setting 'y' in group 'livretest': Value must be numeric.\n".into(),
            "ans = 3".into(),
            "No value is set for setting 'y' at factory level.\n".into(),
            "ans = logical\n   1".into(),
            "ans = logical\n   1".into(),
            "ans = logical\n   0".into(),
            "h = \n  SettingsGroup 'livretest.sub' with no properties.".into(),
            "ans = logical\n   0".into(),
            "g = \n  SettingsGroup 'livretest' with properties:\n\n      x: [1×1 Setting]\n    sub: [1×1 SettingsGroup]".into(),
            "Setting 'x' has no value.\n".into(),
            "g = \n  SettingsGroup 'livretest' with properties:\n\n      x: [1×1 Setting]\n    sub: [1×1 SettingsGroup]".into(),
            format!("k = \n{}", setting("livretest.k", ["{[1]  'a'}", none, "{[1]  'a'}", none, none])),
            "ans = \"str\"".into(),
            "ans = logical\n   0".into(),
        ]
    );
}

#[test]
fn errors_and_product_settings() {
    let none = "<no value>";
    assert_eq!(
        shown(concat!(
            "s = settings; g = addGroup(s, 'livretest'); addSetting(g, 'x'); addGroup(g, 'sub');\n",
            "try, removeSetting(g, 'zz'), catch e, disp(e.message), end\n",
            "try, s.livretest.nope, catch e, disp(e.message), end\n",
            "try, addGroup(g, 'sub'), catch e, disp(e.message), end\n",
            "try, addSetting(g, 'x'), catch e, disp(e.message), end\n",
            "try, addGroup(g, 'q', 'Description', 'd'), catch e, disp(e.message), end\n",
            "try, s.addSetting('q'), catch e, disp(e.message), end\n",
            "c = g.x\n",
            "f = s.matlab.fonts\nk = s.matlab.fonts.codefont.Size\n",
            "s.matlab.fonts.codefont.Size.TemporaryValue = 12;\ns.matlab.fonts.codefont.Size.ActiveValue\n",
            "try, s.matlab.fonts.codefont.Size.FactoryValue = 3; catch e, disp(e.message), end\n",
            "try, g.x.ActiveValue = 3; catch e, disp(e.message), end\n",
            "s.matlab.fonts.codefont.Style.FactoryValue\n",
        )),
        [
            "Settings group 'livretest' does not contain setting 'zz'.\n".to_string(),
            "Unrecognized method, property, or field 'nope' for class 'matlab.settings.SettingsGroup'.\n".into(),
            "Settings group 'livretest' already contains a setting or settings group 'sub'.\n".into(),
            "Settings group 'livretest' already contains a setting or settings group 'x'.\n".into(),
            "'Description' is not a recognized parameter. For a list of valid name-value pair arguments, see the documentation for this function.\n".into(),
            "Unable to add setting 'q'. The settings root contains only product settings groups.\n".into(),
            format!("c = \n{}", setting("livretest.x", [none, none, none, none, none])),
            "f = \n  SettingsGroup 'matlab.fonts' with properties:\n\n      editor: [1×1 SettingsGroup]\n    codefont: [1×1 SettingsGroup]".into(),
            format!("k = \n{}", setting("matlab.fonts.codefont.Size", ["10", none, none, none, "10"])),
            "ans = 12".into(),
            "Unable to set the 'FactoryValue' property of class ''Setting'' because it is read-only.\n".into(),
            "Unable to set the 'ActiveValue' property of class ''Setting'' because it is read-only.\n".into(),
            "ans = int32\n   0".into(),
        ]
    );
}

#[test]
fn validators() {
    assert_eq!(
        shown(concat!(
            "matlab.settings.mustBeNumericScalar(3); matlab.settings.mustBeLogicalScalar(true); matlab.settings.mustBeStringScalar(\"a\");\n",
            "matlab.settings.mustBeStringScalar('a'); matlab.settings.mustBeIntegerScalar(4); disp('ok')\n",
            "try, matlab.settings.mustBeNumericScalar('a'), catch e, disp(e.message), end\n",
        )),
        ["ok\n", "Value must be numeric.\n"]
    );
}
