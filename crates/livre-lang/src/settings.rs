//! The settings API: `settings`, `matlab.settings.SettingsGroup` (`addGroup`, `addSetting`,
//! `hasGroup`, `hasSetting`, `removeGroup`, `removeSetting`), `matlab.settings.Setting`
//! (`ActiveValue`, `TemporaryValue`, `PersonalValue`, `InstallationValue`, `FactoryValue` and the
//! `has*Value` / `clear*Value` methods) and the `matlab.settings.mustBe*Scalar` validators.
//!
//! The tree starts with a few of MATLAB's product groups (`matlab.fonts.codefont`, …). With
//! persistence on (the app and matlivre-cli turn it on), personal values and the groups and
//! settings added are saved next to the preferences and come back in the next session.

use crate::builtins::{arg, none, one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "settings", "matlivre_settings_method", "matlab.settings.mustBeLogicalScalar", "matlab.settings.mustBeNumericScalar",
    "matlab.settings.mustBeIntegerScalar", "matlab.settings.mustBeStringScalar",
];

/// Methods of SettingsGroup and Setting (for the coverage checklist).
#[allow(dead_code)]
const SETTINGS_METHODS: &[&str] = &["SettingsGroup", "Setting"];

const GROUP: &str = "matlab.settings.SettingsGroup";
const SETTING: &str = "matlab.settings.Setting";

const GROUP_METHODS: &[&str] = &["addGroup", "addSetting", "hasGroup", "hasSetting", "removeGroup", "removeSetting", "properties", "fieldnames"];
const SETTING_METHODS: &[&str] = &[
    "hasTemporaryValue", "hasPersonalValue", "hasInstallationValue", "hasFactoryValue", "clearTemporaryValue", "clearPersonalValue", "clearInstallationValue",
];

/// Value levels, highest priority first.
const LEVELS: &[&str] = &["Temporary", "Personal", "Installation", "Factory"];

enum Kind2 {
    Group(Vec<usize>),
    Setting { values: [Option<Value>; 4], validation: Option<Value> },
}

struct Node {
    name: String,
    parent: Option<usize>,
    kind: Kind2,
    hidden: bool,
}

pub struct Tree {
    nodes: Vec<Node>,
    objects: HashMap<usize, Value>,
    /// Nodes below this index were added by the user (the rest are the built-in tree).
    builtin: usize,
    /// Where personal values are saved, when persistence is on.
    file: Option<std::path::PathBuf>,
}

impl Default for Tree {
    fn default() -> Self {
        let mut t = Tree { nodes: vec![Node { name: String::new(), parent: None, kind: Kind2::Group(Vec::new()), hidden: false }], objects: HashMap::new(), builtin: 0, file: None };
        let m = t.add(0, "matlab", None);
        for g in ["codeanalyzer", "appdesigner", "commandwindow", "keyboard", "editor", "colors", "toolboxpathcache"] {
            t.add(m, g, None);
        }
        let fonts = t.add(m, "fonts", None);
        let ed = t.add(fonts, "editor", None);
        for g in ["code", "title", "normal", "heading3", "heading1", "heading2"] {
            t.add(ed, g, None);
        }
        let code = t.add(fonts, "codefont", None);
        let factory = |v: Value| Some([None, None, None, Some(v)]);
        t.add(code, "Name", factory(Value::chars("Monospaced")));
        t.add(code, "Style", factory(Value::Num(Matrix::scalar(0.0).with_kind(Kind::Int(IntClass::I32)))));
        t.add(code, "Size", factory(Value::num(10.0)));
        let general = t.add(m, "general", None);
        for g in ["windows", "matfile", "randomnumbers"] {
            t.add(general, g, None);
        }
        t.builtin = t.nodes.len();
        t
    }
}

impl Tree {
    /// Turns persistence on with the settings file `file`, loading what it holds.
    pub fn persist_to(&mut self, file: std::path::PathBuf) {
        if file.exists() {
            let _ = self.load(&file);
        }
        self.file = Some(file);
    }

    /// The node at `path` (dot-separated) under the root.
    fn find(&self, path: &str) -> Option<usize> {
        path.split('.').try_fold(0, |g, part| self.child(g, part))
    }

    fn load(&mut self, file: &std::path::Path) -> R<()> {
        let vars = crate::builtins_data::read_mat(&file.display().to_string())?;
        let get = |n: &str| vars.iter().find(|(k, _)| k == n).and_then(|(_, v)| v.clone());
        let (Some(Value::Cell(kinds)), Some(Value::Cell(paths)), Some(hidden), Some(Value::Cell(values)), Some(has)) = (get("kinds"), get("paths"), get("hidden"), get("values"), get("hasvalue")) else {
            return Ok(());
        };
        let (hidden, has) = (hidden.to_matrix()?.data, has.to_matrix()?.data);
        for k in 0..kinds.data.len() {
            let kind = kinds.data[k].text()?;
            let path = paths.data[k].text()?;
            let value = if has.get(k).copied().unwrap_or(0.0) != 0.0 { values.data.get(k).cloned() } else { None };
            let (parent, name) = match path.rsplit_once('.') {
                Some((p, n)) => (self.find(p), n.to_string()),
                None => (Some(0), path.clone()),
            };
            let Some(parent) = parent else { continue };
            let id = match self.child(parent, &name) {
                Some(id) => id,
                None if kind == "group" => self.add(parent, &name, None),
                None if kind == "setting" => self.add(parent, &name, Some([None, None, None, None])),
                None => continue,
            };
            if kind != "value" {
                self.nodes[id].hidden = hidden.get(k).copied().unwrap_or(0.0) != 0.0;
            }
            if let (Some(v), false) = (value, self.is_group(id)) {
                self.values_mut(id)[1] = Some(v);
            }
        }
        Ok(())
    }

    /// Saves the groups and settings added and every personal value (when persistence is on).
    fn save(&self) {
        let Some(file) = &self.file else { return };
        let (mut kinds, mut paths, mut hidden, mut values, mut has) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut todo = vec![0usize];
        while let Some(g) = todo.pop() {
            for &c in self.children(g) {
                let personal = if self.is_group(c) { None } else { self.values(c)[1].clone() };
                let kind = match (c >= self.builtin, self.is_group(c)) {
                    (true, true) => "group",
                    (true, false) => "setting",
                    (false, false) if personal.is_some() => "value",
                    _ => "",
                };
                if !kind.is_empty() {
                    kinds.push(Value::chars(kind));
                    paths.push(Value::chars(&self.path(c)));
                    hidden.push(if self.nodes[c].hidden { 1.0 } else { 0.0 });
                    has.push(if personal.is_some() { 1.0 } else { 0.0 });
                    values.push(personal.unwrap_or_else(Value::empty));
                }
                if self.is_group(c) {
                    todo.push(c);
                }
            }
        }
        // Parents come before their children (the loader creates them in order).
        let n = kinds.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&k| paths[k].text().map(|p| p.matches('.').count()).unwrap_or(0));
        let pick = |v: &Vec<Value>| Value::Cell(CellArray::from_dims(vec![n, 1], order.iter().map(|&k| v[k].clone()).collect()));
        let col = |v: &Vec<f64>| Value::Num(Matrix::col(order.iter().map(|&k| v[k]).collect()));
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let vars = vec![("kinds".to_string(), pick(&kinds)), ("paths".to_string(), pick(&paths)), ("hidden".to_string(), col(&hidden)), ("values".to_string(), pick(&values)), ("hasvalue".to_string(), col(&has))];
        let _ = crate::builtins_data::write_mat(&file.display().to_string(), &vars, false);
    }
}

impl Tree {
    /// Adds a group (`values` None) or a setting under group `parent`.
    fn add(&mut self, parent: usize, name: &str, values: Option<[Option<Value>; 4]>) -> usize {
        let kind = match values {
            None => Kind2::Group(Vec::new()),
            Some(values) => Kind2::Setting { values, validation: None },
        };
        self.nodes.push(Node { name: name.to_string(), parent: Some(parent), kind, hidden: false });
        let id = self.nodes.len() - 1;
        if let Kind2::Group(c) = &mut self.nodes[parent].kind {
            c.push(id);
        }
        id
    }

    fn children(&self, g: usize) -> &[usize] {
        match &self.nodes[g].kind {
            Kind2::Group(c) => c,
            _ => &[],
        }
    }

    fn child(&self, g: usize, name: &str) -> Option<usize> {
        self.children(g).iter().copied().find(|&c| self.nodes[c].name == name)
    }

    fn is_group(&self, n: usize) -> bool {
        matches!(self.nodes[n].kind, Kind2::Group(_))
    }

    /// `matlab.fonts.codefont` (the root has no name).
    fn path(&self, n: usize) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(n);
        while let Some(c) = cur {
            if self.nodes[c].parent.is_some() {
                parts.push(self.nodes[c].name.clone());
            }
            cur = self.nodes[c].parent;
        }
        parts.reverse();
        parts.join(".")
    }

    fn values(&self, n: usize) -> &[Option<Value>; 4] {
        match &self.nodes[n].kind {
            Kind2::Setting { values, .. } => values,
            _ => unreachable!("a setting"),
        }
    }

    fn values_mut(&mut self, n: usize) -> &mut [Option<Value>; 4] {
        match &mut self.nodes[n].kind {
            Kind2::Setting { values, .. } => values,
            _ => unreachable!("a setting"),
        }
    }
}

fn class_def(group: bool) -> Rc<ClassDef> {
    thread_local! {
        static DEFS: RefCell<Vec<Rc<ClassDef>>> = const { RefCell::new(Vec::new()) };
    }
    DEFS.with(|d| {
        if d.borrow().is_empty() {
            let mut defs = Vec::new();
            for (short, full, methods) in [("SettingsGroup", GROUP, GROUP_METHODS), ("Setting", SETTING, SETTING_METHODS)] {
                let mut src = format!("classdef {short} < handle\n  properties (Hidden)\n    Id\n  end\n  methods\n");
                for m in methods {
                    let nout = if *m == "properties" { "nargout" } else { "max(nargout, 1)" };
                    src += &format!("    function varargout = {m}(obj, varargin)\n      varargout = matlivre_settings_method('{m}', {nout}, obj, varargin{{:}});\n    end\n");
                }
                src += "  end\nend\n";
                let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = full.to_string();
                defs.push(Rc::new(def));
            }
            *d.borrow_mut() = defs;
        }
        d.borrow()[if group { 0 } else { 1 }].clone()
    })
}

pub fn is_node(class: &str) -> bool {
    class == GROUP || class == SETTING
}

fn object(it: &mut Interpreter, n: usize) -> Value {
    if let Some(v) = it.settings.objects.get(&n) {
        return v.clone();
    }
    let group = it.settings.is_group(n);
    let v = Value::Object(Object { class: class_def(group), props: Rc::new(RefCell::new(vec![Value::num(n as f64)])) });
    it.settings.objects.insert(n, v.clone());
    v
}

fn node_of(it: &Interpreter, v: &Value) -> R<usize> {
    if let Value::Object(o) = v {
        if is_node(&o.class.name) {
            let n = o.props.borrow()[0].scalar()? as usize;
            if n < it.settings.nodes.len() && attached(it, n) {
                return Ok(n);
            }
            return rt("Invalid or deleted object.");
        }
    }
    rt("Expected a settings group or setting.")
}

/// Whether node `n` is still in the tree (not under a removed group).
fn attached(it: &Interpreter, n: usize) -> bool {
    let t = &it.settings;
    let mut cur = n;
    while let Some(p) = t.nodes[cur].parent {
        if !t.children(p).contains(&cur) {
            return false;
        }
        cur = p;
    }
    cur == 0
}

fn level_index(name: &str) -> Option<usize> {
    LEVELS.iter().position(|l| format!("{l}Value") == name)
}

fn no_value(name: &str, level: usize) -> LangError {
    LangError::new(format!("No value is set for setting '{name}' at {} level.", LEVELS[level].to_lowercase()), 0)
}

fn active(it: &Interpreter, n: usize) -> Option<Value> {
    it.settings.values(n).iter().flatten().next().cloned()
}

/// `node.name`: a child of a group, or a value of a setting.
pub fn member(it: &mut Interpreter, v: &Value, name: &str) -> R<Value> {
    let n = node_of(it, v)?;
    let t = &it.settings;
    if t.is_group(n) {
        return match t.child(n, name) {
            Some(c) => Ok(object(it, c)),
            None => rt(format!("Unrecognized method, property, or field '{name}' for class '{GROUP}'.")),
        };
    }
    let short = t.nodes[n].name.clone();
    if name == "ActiveValue" {
        return active(it, n).ok_or_else(|| LangError::new(format!("Setting '{short}' has no value."), 0));
    }
    match level_index(name) {
        Some(l) => t.values(n)[l].clone().ok_or_else(|| no_value(&short, l)),
        None => rt(format!("Unrecognized method, property, or field '{name}' for class '{SETTING}'.")),
    }
}

/// `node.name = value`.
pub fn set_member(it: &mut Interpreter, v: &Value, name: &str, value: Value) -> R<()> {
    let n = node_of(it, v)?;
    if it.settings.is_group(n) {
        // Assigning a child back after setting one of its values (`g.x.PersonalValue = 1`).
        return match it.settings.child(n, name) {
            Some(c) if node_of(it, &value).ok() == Some(c) => Ok(()),
            Some(_) => rt(format!("Unable to set the '{name}' property of class ''SettingsGroup'' because it is read-only.")),
            None => rt(format!("Unrecognized property '{name}' for class '{GROUP}'.")),
        };
    }
    let l = match level_index(name) {
        Some(l @ (0 | 1)) => l,
        Some(2) => return rt("No write permission for 'installation' level settings."),
        Some(_) => return rt(format!("Unable to set the '{name}' property of class ''Setting'' because it is read-only.")),
        None if name == "ActiveValue" => return rt("Unable to set the 'ActiveValue' property of class ''Setting'' because it is read-only."),
        None => return rt(format!("Unrecognized property '{name}' for class '{SETTING}'.")),
    };
    validate(it, n, &value)?;
    it.settings.values_mut(n)[l] = Some(value);
    if l == 1 {
        it.settings.save();
    }
    Ok(())
}

fn validate(it: &mut Interpreter, n: usize, value: &Value) -> R<()> {
    let Kind2::Setting { validation: Some(Value::Func(f)), .. } = &it.settings.nodes[n].kind else { return Ok(()) };
    let f = f.clone();
    if let Err(e) = it.call_value(&f, vec![value.clone()], 0) {
        let t = &it.settings;
        let group = t.nodes[n].parent.map(|p| t.path(p)).unwrap_or_default();
        return rt(format!("Error setting '{}' in group '{group}': {}", t.nodes[n].name, e.message));
    }
    Ok(())
}

fn unknown_param(p: &str) -> LangError {
    LangError::new(format!("'{p}' is not a recognized parameter. For a list of valid name-value pair arguments, see the documentation for this function."), 0)
}

fn new_child(it: &mut Interpreter, g: usize, a: &[Value], setting: bool) -> R<Value> {
    let name = arg(a, 0, "addGroup")?.text()?;
    let valid = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return rt(format!("'{name}' is not a valid setting or settings group name."));
    }
    let (mut hidden, mut personal, mut validation) = (false, None, None);
    for pair in a[1..].chunks(2) {
        let key = pair[0].text()?;
        let Some(v) = pair.get(1) else { return rt("Name-value arguments must come in pairs.") };
        match (key.to_lowercase().as_str(), setting) {
            ("hidden", _) => hidden = v.is_true()?,
            ("personalvalue", true) => personal = Some(v.clone()),
            ("validationfcn", true) => validation = Some(v.clone()),
            ("readonly", true) => {}
            _ => return Err(unknown_param(&key)),
        }
    }
    let t = &it.settings;
    if setting && g == 0 {
        return rt(format!("Unable to add setting '{name}'. The settings root contains only product settings groups."));
    }
    if t.child(g, &name).is_some() {
        return rt(format!("Settings group '{}' already contains a setting or settings group '{name}'.", t.path(g)));
    }
    let id = if setting {
        let id = it.settings.add(g, &name, Some([None, None, None, None]));
        if let Kind2::Setting { validation: slot, .. } = &mut it.settings.nodes[id].kind {
            *slot = validation;
        }
        if let Some(p) = personal {
            if let Err(e) = validate(it, id, &p) {
                it.settings.nodes.pop();
                let g_children = match &mut it.settings.nodes[g].kind {
                    Kind2::Group(c) => c,
                    _ => unreachable!(),
                };
                g_children.retain(|&c| c != id);
                return Err(e);
            }
            it.settings.values_mut(id)[1] = Some(p);
        }
        id
    } else {
        it.settings.add(g, &name, None)
    };
    it.settings.nodes[id].hidden = hidden;
    it.settings.save();
    Ok(object(it, id))
}

fn remove_child(it: &mut Interpreter, g: usize, name: &str, setting: bool) -> R<()> {
    let t = &it.settings;
    match t.child(g, name) {
        Some(c) if t.is_group(c) != setting => {
            if let Kind2::Group(ch) = &mut it.settings.nodes[g].kind {
                ch.retain(|&x| x != c);
            }
            it.settings.save();
            Ok(())
        }
        _ => rt(format!("Settings group '{}' does not contain {} '{name}'.", t.path(g), if setting { "setting" } else { "settings group" })),
    }
}

/// The children a group shows: settings, then groups, without hidden ones.
fn visible(it: &Interpreter, g: usize) -> Vec<usize> {
    let t = &it.settings;
    let ch: Vec<usize> = t.children(g).iter().copied().filter(|&c| !t.nodes[c].hidden).collect();
    let mut out: Vec<usize> = ch.iter().copied().filter(|&c| !t.is_group(c)).collect();
    out.extend(ch.iter().copied().filter(|&c| t.is_group(c)));
    out
}

fn method(it: &mut Interpreter, a: &[Value]) -> Ret {
    let m = arg(a, 0, "settings")?.text()?;
    let nargout = arg(a, 1, "settings")?.scalar()? as usize;
    let obj = arg(a, 2, "settings")?.clone();
    let n = node_of(it, &obj)?;
    let rest = &a[3..];
    let out = |v: Vec<Value>| one(Value::Cell(CellArray::row(v)));
    let name_arg = |i: usize| -> R<String> { rest.get(i).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text() };
    match m.as_str() {
        "addGroup" => out(vec![new_child(it, n, rest, false)?]),
        "addSetting" => out(vec![new_child(it, n, rest, true)?]),
        "hasGroup" | "hasSetting" => {
            let name = name_arg(0)?;
            let t = &it.settings;
            let has = t.child(n, &name).is_some_and(|c| t.is_group(c) == (m == "hasGroup"));
            out(vec![Value::bool(has)])
        }
        "removeGroup" | "removeSetting" => {
            remove_child(it, n, &name_arg(0)?, m == "removeSetting")?;
            out(Vec::new())
        }
        "properties" | "fieldnames" => {
            let names: Vec<String> = visible(it, n).iter().map(|&c| it.settings.nodes[c].name.clone()).collect();
            if m == "properties" && nargout == 0 {
                let mut s = format!("\nProperties for class {GROUP}:\n\n");
                for x in &names {
                    s += &format!("    {x}\n");
                }
                it.emit(OutputKind::Text(s + "\n"));
                return out(Vec::new());
            }
            let k = names.len();
            out(vec![Value::Cell(CellArray::from_dims(vec![k, 1], names.iter().map(|s| Value::chars(s)).collect()))])
        }
        _ if m.starts_with("has") => {
            let l = level_index(&m[3..]).expect("level");
            out(vec![Value::bool(it.settings.values(n)[l].is_some())])
        }
        _ if m.starts_with("clear") => {
            let l = level_index(&m[5..]).expect("level");
            it.settings.values_mut(n)[l] = None;
            if l == 1 {
                it.settings.save();
            }
            out(Vec::new())
        }
        _ => rt(format!("Unknown settings method '{m}'.")),
    }
}

/// The display of a settings group or setting.
pub fn node_text(it: &Interpreter, o: &Object) -> Option<String> {
    if !is_node(&o.class.name) {
        return None;
    }
    let n = node_of(it, &Value::Object(o.clone())).ok()?;
    let t = &it.settings;
    let path = t.path(n);
    let quoted = if path.is_empty() { String::new() } else { format!(" '{path}'") };
    if t.is_group(n) {
        let ch = visible(it, n);
        if ch.is_empty() {
            return Some(format!("  SettingsGroup{quoted} with no properties."));
        }
        let w = ch.iter().map(|&c| t.nodes[c].name.len()).max().unwrap_or(0);
        let mut s = format!("  SettingsGroup{quoted} with properties:\n\n");
        for &c in &ch {
            let kind = if t.is_group(c) { "SettingsGroup" } else { "Setting" };
            s += &format!("    {:>w$}: [1×1 {kind}]\n", t.nodes[c].name);
        }
        return Some(s.trim_end().to_string());
    }
    // A struct shows the values as MATLAB does; `<no value>` replaces a placeholder.
    const NONE: &str = "\u{1}no value\u{1}";
    let mut st = StructArray::scalar();
    let vals = t.values(n);
    st.set("ActiveValue", active(it, n).unwrap_or_else(|| Value::chars(NONE)));
    for (l, name) in LEVELS.iter().enumerate() {
        st.set(&format!("{name}Value"), vals[l].clone().unwrap_or_else(|| Value::chars(NONE)));
    }
    let body = crate::display::struct_fields_body(&st, it.format).replace(&format!("'{NONE}'"), "<no value>");
    Some(format!("  Setting{quoted} with properties:\n\n{}", body.trim_end()))
}

fn must_be(name: &str, v: &Value) -> R<()> {
    let numeric = matches!(v, Value::Num(m) if m.kind != Kind::Char);
    let scalar = v.numel() == 1;
    let ok_msg = match name {
        "matlab.settings.mustBeLogicalScalar" => (matches!(v, Value::Num(m) if m.kind == Kind::Logical) && scalar, "Value must be a logical scalar."),
        "matlab.settings.mustBeStringScalar" => (matches!(v, Value::Str(s) if s.data.len() == 1) || matches!(v, Value::Num(m) if m.kind == Kind::Char && m.rows() <= 1), "Value must be a string scalar."),
        "matlab.settings.mustBeIntegerScalar" => (
            numeric && scalar && v.scalar().is_ok_and(|x| x.fract() == 0.0),
            if numeric && scalar { "Value must be an integer." } else if numeric { "Value must be scalar." } else { "Value must be numeric." },
        ),
        _ => (numeric && scalar, if numeric { "Value must be scalar." } else { "Value must be numeric." }),
    };
    if ok_msg.0 {
        Ok(())
    } else {
        rt(ok_msg.1)
    }
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "settings" => one(object(it, 0)),
        "matlivre_settings_method" => method(it, a),
        _ if name.starts_with("matlab.settings.mustBe") => {
            must_be(name, arg(a, 0, name)?)?;
            none()
        }
        _ => Ok(None),
    }
}
