//! Units of measurement: `symunit` and the unit functions (`unitConvert`, `separateUnits`,
//! `checkUnits`, `newUnit`, ...). A unit is the symbolic atom `[name]`; its definition is a
//! scale to SI and a dimension over the SI base units (plus plane and solid angle).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use num_bigint::BigInt;
use num_rational::BigRational;

use super::expr::{Const, Num};
use super::{algebra, Expr, SymArray};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &[
    "symunit", "unitConvert", "unitConversionFactor", "separateUnits", "isUnit", "symunit2str", "str2symunit", "checkUnits", "baseUnits", "derivedUnits",
    "unitSystems", "findUnits", "rewrite", "unitInfo", "mixedUnits", "newUnit", "removeUnit", "newUnitSystem", "removeUnitSystem",
];

/// Dimension exponents over kg, s, m, A, cd, mol, K, rad, sr.
type Dims = [i32; 9];

const BASE: [&str; 9] = ["kg", "s", "m", "A", "cd", "mol", "K", "rad", "sr"];

#[derive(Clone)]
struct UnitDef {
    /// The value of one unit in SI base units.
    scale: Expr,
    dims: Dims,
    /// Temperature units: SI = scale * (value + offset) for absolute temperatures.
    offset: Option<Expr>,
}

/// (name, factor, definition in earlier units). The factor is exact unless it starts with
/// `f:` (MATLAB keeps those as floating-point numbers).
const DEFS: &[(&str, &str, &str)] = &[
    ("min", "60", "s"), ("h", "3600", "s"), ("d", "86400", "s"), ("week", "7", "d"), ("fortnight", "14", "d"),
    ("year_Julian", "36525/100", "d"), ("year_Gregorian", "365.2425", "d"), ("year_360", "360", "d"), ("month_30", "30", "d"),
    ("g", "1/1000", "kg"), ("t", "1000", "kg"),
    ("in", "0.0254", "m"), ("ft", "0.3048", "m"), ("yd", "0.9144", "m"), ("mi", "1609.344", "m"),
    ("nmile", "1852", "m"), ("Ao", "1e-10", "m"), ("au", "149597870700", "m"), ("ly", "9460730472580800", "m"), ("mil", "0.0000254", "m"),
    ("ft_US", "1200/3937", "m"), ("mi_US", "6336000/3937", "m"), ("fur", "201.168", "m"), ("ch", "20.1168", "m"), ("rod", "5.0292", "m"),
    ("hand", "0.1016", "m"), ("li", "0.201168", "m"), ("span", "0.2286", "m"), ("pt", "1/72", "in"), ("pica", "12", "pt"),
    ("lbm", "0.45359237", "kg"), ("oz", "1/16", "lbm"), ("gr", "1/7000", "lbm"), ("dr", "1/256", "lbm"), ("stone", "14", "lbm"), ("quarter", "28", "lbm"),
    ("cwt", "100", "lbm"), ("cwt_UK", "112", "lbm"), ("tn", "2000", "lbm"), ("ton_UK", "2240", "lbm"), ("ct", "0.0002", "kg"), ("dalton", "1.66053906660e-27", "kg"),
    ("N", "1", "kg*m/s^2"), ("J", "1", "N*m"), ("W", "1", "J/s"), ("Pa", "1", "N/m^2"), ("Hz", "1", "1/s"), ("C", "1", "A*s"), ("V", "1", "W/A"),
    ("Ohm", "1", "V/A"), ("S", "1", "A/V"), ("F", "1", "C/V"), ("Wb", "1", "V*s"), ("H", "1", "Wb/A"), ("T", "1", "Wb/m^2"), ("lm", "1", "cd*sr"),
    ("lx", "1", "lm/m^2"), ("Bq", "1", "1/s"), ("Gy", "1", "J/kg"), ("Sv", "1", "J/kg"), ("kat", "1", "mol/s"),
    ("deg", "pi/180", "rad"), ("arcmin", "1/60", "deg"), ("arcsec", "1/60", "arcmin"), ("rev", "2*pi", "rad"),
    ("l", "1/1000", "m^3"), ("gal", "3.785411784", "l"), ("eV", "1.602176634e-19", "J"), ("cal", "4.1868", "J"), ("Wh", "3600", "J"),
    ("erg", "1e-7", "J"), ("dyn", "1e-5", "N"), ("bar", "100000", "Pa"), ("atm", "101325", "Pa"), ("Torr", "1/760", "atm"), ("mmHg", "f:133.3224", "Pa"),
    ("g_n", "9.80665", "m/s^2"), ("lbf", "1", "lbm*g_n"), ("psi", "1", "lbf/in^2"), ("slug", "1", "lbf*s^2/ft"), ("HP_DIN", "f:735.49875", "W"),
    ("mph", "1", "mi/h"), ("kn", "1", "nmile/h"),
];

const PREFIXES: &[(&str, i32)] = &[
    ("Y", 24), ("Z", 21), ("E", 18), ("P", 15), ("T", 12), ("G", 9), ("M", 6), ("k", 3), ("h", 2), ("da", 1), ("d", -1), ("c", -2), ("m", -3), ("u", -6),
    ("n", -9), ("p", -12), ("f", -15), ("a", -18),
];

const PREFIXABLE: &[&str] = &[
    "m", "g", "s", "A", "K", "mol", "cd", "N", "J", "W", "Pa", "Hz", "V", "C", "Ohm", "F", "H", "T", "Wb", "S", "l", "eV", "bar", "lm", "lx", "Bq", "Gy",
    "Sv", "kat", "Wh", "cal", "t",
];

const DERIVED_SI: &[&str] = &[
    "F", "C", "S", "H", "V", "J", "N", "lx", "lm", "Wb", "W", "Pa", "Ohm", "T", "Gy", "Bq", "Sv", "Hz", "kat", "rad", "sr", "Celsius",
];

/// Physical dimension names, by dimension.
const DIM_NAMES: &[(&str, Dims)] = &[
    ("Mass", [1, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("Time", [0, 1, 0, 0, 0, 0, 0, 0, 0]),
    ("Length", [0, 0, 1, 0, 0, 0, 0, 0, 0]),
    ("ElectricCurrent", [0, 0, 0, 1, 0, 0, 0, 0, 0]),
    ("LuminousIntensity", [0, 0, 0, 0, 1, 0, 0, 0, 0]),
    ("AmountOfSubstance", [0, 0, 0, 0, 0, 1, 0, 0, 0]),
    ("Temperature", [0, 0, 0, 0, 0, 0, 1, 0, 0]),
    ("PlaneAngle", [0, 0, 0, 0, 0, 0, 0, 1, 0]),
    ("SolidAngle", [0, 0, 0, 0, 0, 0, 0, 0, 1]),
    ("Area", [0, 0, 2, 0, 0, 0, 0, 0, 0]),
    ("Volume", [0, 0, 3, 0, 0, 0, 0, 0, 0]),
    ("Velocity", [0, -1, 1, 0, 0, 0, 0, 0, 0]),
    ("Acceleration", [0, -2, 1, 0, 0, 0, 0, 0, 0]),
    ("Frequency", [0, -1, 0, 0, 0, 0, 0, 0, 0]),
    ("Force", [1, -2, 1, 0, 0, 0, 0, 0, 0]),
    ("Energy", [1, -2, 2, 0, 0, 0, 0, 0, 0]),
    ("PowerOrHeatFlowRate", [1, -3, 2, 0, 0, 0, 0, 0, 0]),
    ("Pressure", [1, -2, -1, 0, 0, 0, 0, 0, 0]),
    ("ElectricCharge", [0, 1, 0, 1, 0, 0, 0, 0, 0]),
    ("ElectricPotential", [1, -3, 2, -1, 0, 0, 0, 0, 0]),
    ("Resistance", [1, -3, 2, -2, 0, 0, 0, 0, 0]),
    ("Conductance", [-1, 3, -2, 2, 0, 0, 0, 0, 0]),
    ("Capacitance", [-1, 4, -2, 2, 0, 0, 0, 0, 0]),
    ("MagneticFlux", [1, -2, 2, -1, 0, 0, 0, 0, 0]),
    ("MagneticFluxDensity", [1, -2, 0, -1, 0, 0, 0, 0, 0]),
    ("Inductance", [1, -2, 2, -2, 0, 0, 0, 0, 0]),
    ("Density", [1, 0, -3, 0, 0, 0, 0, 0, 0]),
    ("Momentum", [1, -1, 1, 0, 0, 0, 0, 0, 0]),
    ("CatalyticActivity", [0, -1, 0, 0, 0, 1, 0, 0, 0]),
    ("LuminousFlux", [0, 0, 0, 0, 1, 0, 0, 0, 1]),
    ("Illuminance", [0, 0, -2, 0, 1, 0, 0, 0, 1]),
    ("AbsorbedDoseOrDoseEquivalent", [0, -2, 2, 0, 0, 0, 0, 0, 0]),
    ("Dimensionless", [0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

const SYSTEMS: &[&str] = &["CGS", "EMU", "ESU", "GU", "SI", "US"];

thread_local! {
    static TABLE: RefCell<Option<HashMap<String, UnitDef>>> = const { RefCell::new(None) };
    static CUSTOM: RefCell<Vec<(String, UnitDef)>> = const { RefCell::new(Vec::new()) };
    static CUSTOM_SYSTEMS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// An exact decimal ("1.602176634e-19") or fraction ("381/1250") as a rational.
fn parse_rational(s: &str) -> BigRational {
    if let Some((p, q)) = s.split_once('/') {
        return parse_rational(p) / parse_rational(q);
    }
    let (mant, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (s, 0),
    };
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let digits: BigInt = format!("{int}{frac}").parse().unwrap_or_default();
    let exp = exp - frac.len() as i32;
    let ten = BigInt::from(10);
    if exp >= 0 {
        BigRational::from_integer(digits * num_traits::pow(ten, exp as usize))
    } else {
        BigRational::new(digits, num_traits::pow(ten, (-exp) as usize))
    }
}

fn factor_expr(s: &str) -> Expr {
    if let Some(f) = s.strip_prefix("f:") {
        return Expr::Num(Num::F(f.parse().unwrap_or(f64::NAN)));
    }
    match s {
        "pi/180" => Expr::div(Expr::Const(Const::Pi), Expr::int(180)),
        "2*pi" => Expr::mul(vec![Expr::int(2), Expr::Const(Const::Pi)]),
        _ => Expr::Num(Num::from_big(parse_rational(s))),
    }
}

fn build_table() -> HashMap<String, UnitDef> {
    let mut t: HashMap<String, UnitDef> = HashMap::new();
    for (i, b) in BASE.iter().enumerate() {
        let mut d = [0; 9];
        d[i] = 1;
        t.insert(b.to_string(), UnitDef { scale: Expr::one(), dims: d, offset: None });
    }
    let kelvin = [0, 0, 0, 0, 0, 0, 1, 0, 0];
    t.insert("Celsius".into(), UnitDef { scale: Expr::one(), dims: kelvin, offset: Some(Expr::rat(5463, 20)) });
    t.insert("Fahrenheit".into(), UnitDef { scale: Expr::rat(5, 9), dims: kelvin, offset: Some(Expr::rat(45967, 100)) });
    t.insert("Rankine".into(), UnitDef { scale: Expr::rat(5, 9), dims: kelvin, offset: None });
    for (name, factor, def) in DEFS {
        let (s, d) = parse_def(def, &t).unwrap_or_else(|| panic!("unit definition {name}"));
        t.insert(name.to_string(), UnitDef { scale: simp(Expr::mul(vec![factor_expr(factor), s])), dims: d, offset: None });
    }
    t
}

/// A definition such as "kg*m/s^2" in units already in the table.
fn parse_def(def: &str, t: &HashMap<String, UnitDef>) -> Option<(Expr, Dims)> {
    let mut scale = Expr::one();
    let mut dims = [0; 9];
    let mut sign = 1;
    let mut token = String::new();
    let apply = |tok: &str, sign: i32, scale: &mut Expr, dims: &mut Dims| -> Option<()> {
        if tok == "1" || tok.is_empty() {
            return Some(());
        }
        let (name, p) = match tok.split_once('^') {
            Some((n, p)) => (n, p.parse::<i32>().ok()?),
            None => (tok, 1),
        };
        let u = t.get(name)?;
        let p = p * sign;
        *scale = Expr::mul(vec![scale.clone(), Expr::pow(u.scale.clone(), Expr::int(p as i128))]);
        for k in 0..9 {
            dims[k] += u.dims[k] * p;
        }
        Some(())
    };
    for c in def.chars() {
        if c == '*' || c == '/' {
            apply(&token, sign, &mut scale, &mut dims)?;
            token.clear();
            sign = if c == '/' { -1 } else { 1 };
        } else {
            token.push(c);
        }
    }
    apply(&token, sign, &mut scale, &mut dims)?;
    Some((scale, dims))
}

fn simp(e: Expr) -> Expr {
    algebra::simplify(&e)
}

fn lookup(name: &str) -> Option<UnitDef> {
    if let Some(u) = CUSTOM.with(|c| c.borrow().iter().find(|(n, _)| n == name).map(|(_, d)| d.clone())) {
        return Some(u);
    }
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        let t = t.get_or_insert_with(build_table);
        if let Some(u) = t.get(name) {
            return Some(u.clone());
        }
        for (p, k) in PREFIXES {
            if let Some(rest) = name.strip_prefix(p) {
                if PREFIXABLE.contains(&rest) {
                    let u = t.get(rest)?;
                    let f = Expr::pow(Expr::int(10), Expr::int(*k as i128));
                    return Some(UnitDef { scale: simp(Expr::mul(vec![f, u.scale.clone()])), dims: u.dims, offset: None });
                }
            }
        }
        None
    })
}

fn undefined(name: &str) -> LangError {
    LangError::new(format!("Undefined unit '{name}'. For details, see Units List."), 0)
}

/// Other names of units, and the name MATLAB shows for them.
const ALIASES: &[(&str, &str)] = &[("L", "l"), ("inch", "in"), ("foot", "ft"), ("yr", "year_Julian"), ("gn", "g_n"), ("hp", "HP_DIN")];

pub fn unit(name: &str) -> R<Expr> {
    let name = ALIASES.iter().find(|(a, _)| *a == name).map(|(_, n)| *n).unwrap_or(name);
    lookup(name).ok_or_else(|| undefined(name))?;
    Ok(Expr::Var(format!("[{name}]")))
}

/// The unit name of the atom `[name]`.
pub fn unit_name(e: &Expr) -> Option<&str> {
    match e {
        Expr::Var(v) => v.strip_prefix('[').and_then(|s| s.strip_suffix(']')),
        _ => None,
    }
}

pub fn is_unit_factor(e: &Expr) -> bool {
    match e {
        Expr::Pow(b, x) => unit_name(b).is_some() && x.as_num().is_some(),
        other => unit_name(other).is_some(),
    }
}

// ---- the symunit object -----------------------------------------------------------------------

thread_local! {
    static CLASS: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
}

fn class_def() -> Rc<ClassDef> {
    CLASS.with(|c| {
        c.borrow_mut()
            .get_or_insert_with(|| {
                let ast = crate::class::parse_classdef("classdef symunit\nend\n").expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = "symunit".into();
                Rc::new(def)
            })
            .clone()
    })
}

pub fn is_symunit(class: &str) -> bool {
    class == "symunit"
}

/// `u.name` on the object `symunit` returns.
pub fn member(name: &str) -> R<Value> {
    Ok(Value::Sym(SymArray::scalar(unit(name)?)))
}

// ---- analysis ---------------------------------------------------------------------------------

/// A term split into its coefficient and its unit factors (name, power).
fn split_term(t: &Expr) -> (Expr, Vec<(String, i32)>) {
    let factors: Vec<Expr> = match t {
        Expr::Mul(fs) => fs.clone(),
        other => vec![other.clone()],
    };
    let mut coeff = Vec::new();
    let mut units = Vec::new();
    for f in factors {
        match &f {
            Expr::Pow(b, x) if unit_name(b).is_some() => {
                let p = x.as_num().and_then(|n| n.as_int()).unwrap_or(1) as i32;
                units.push((unit_name(b).unwrap().to_string(), p));
            }
            other if unit_name(other).is_some() => units.push((unit_name(other).unwrap().to_string(), 1)),
            _ => coeff.push(f),
        }
    }
    (Expr::mul(coeff), units)
}

fn units_expr(units: &[(String, i32)]) -> Expr {
    Expr::mul(units.iter().map(|(n, p)| Expr::pow(Expr::Var(format!("[{n}]")), Expr::int(*p as i128))).collect())
}

fn analyze(units: &[(String, i32)]) -> R<(Expr, Dims)> {
    let mut scale = Expr::one();
    let mut dims = [0; 9];
    for (n, p) in units {
        let u = lookup(n).ok_or_else(|| undefined(n))?;
        scale = Expr::mul(vec![scale, Expr::pow(u.scale.clone(), Expr::int(*p as i128))]);
        for k in 0..9 {
            dims[k] += u.dims[k] * p;
        }
    }
    Ok((simp(scale), dims))
}

fn si_units(dims: &Dims) -> Expr {
    Expr::mul(BASE.iter().zip(dims).filter(|(_, &p)| p != 0).map(|(b, &p)| Expr::pow(Expr::Var(format!("[{b}]")), Expr::int(p as i128))).collect())
}

fn terms(e: &Expr) -> Vec<Expr> {
    match e {
        Expr::Add(ts) => ts.clone(),
        other => vec![other.clone()],
    }
}

enum Target {
    Si,
    Unit(Expr, Vec<(String, i32)>),
}

fn convert(e: &Expr, target: &Target, absolute: bool) -> R<Expr> {
    if let Expr::Rel(op, l, r) = e {
        return Ok(Expr::rel(*op, convert(l, target, absolute)?, convert(r, target, absolute)?));
    }
    let mut out = Vec::new();
    for t in terms(e) {
        let (c, units) = split_term(&t);
        if units.is_empty() {
            out.push(t);
            continue;
        }
        let (s, d) = analyze(&units)?;
        let new = match target {
            Target::Si => Expr::mul(vec![simp(Expr::mul(vec![c, s])), si_units(&d)]),
            Target::Unit(texpr, tunits) => {
                let (ts, td) = analyze(tunits)?;
                // Absolute temperatures shift by the units' offsets.
                if absolute && units.len() == 1 && units[0].1 == 1 && tunits.len() == 1 && tunits[0].1 == 1 && d == td {
                    let from = lookup(&units[0].0).unwrap();
                    let to = lookup(&tunits[0].0).unwrap();
                    if from.offset.is_some() || to.offset.is_some() {
                        let si = Expr::mul(vec![from.scale.clone(), Expr::add(vec![c, from.offset.clone().unwrap_or_else(Expr::zero)])]);
                        let v = simp(Expr::sub(Expr::div(si, to.scale.clone()), to.offset.clone().unwrap_or_else(Expr::zero)));
                        out.push(Expr::mul(vec![v, texpr.clone()]));
                        continue;
                    }
                }
                let rest: Dims = std::array::from_fn(|k| d[k] - td[k]);
                let k = simp(Expr::div(Expr::mul(vec![c, s]), ts));
                Expr::mul(vec![k, texpr.clone(), si_units(&rest)])
            }
        };
        out.push(new);
    }
    Ok(Expr::add(out))
}

fn sym_arg(a: &[Value], i: usize) -> R<SymArray> {
    SymArray::from_value(a.get(i).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?)
}

fn scalar(a: &[Value], i: usize) -> R<Expr> {
    let s = sym_arg(a, i)?;
    if s.data.len() != 1 {
        return rt("A scalar symbolic expression is required.");
    }
    Ok(s.data.into_iter().next().unwrap())
}

fn sym_row(data: Vec<Expr>) -> Value {
    let n = data.len();
    Value::Sym(SymArray::from_dims(vec![1, n], data))
}

fn logical(dims: Vec<usize>, vals: Vec<bool>) -> Value {
    let mut m = Matrix::from_dims(dims, vals.into_iter().map(|b| b as i32 as f64).collect());
    m.kind = Kind::Logical;
    Value::Num(m)
}

fn collect_units(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Var(_) => {
            if let Some(n) = unit_name(e) {
                if !out.iter().any(|x| x == n) {
                    out.push(n.to_string());
                }
            }
        }
        Expr::Add(xs) | Expr::Mul(xs) | Expr::Fn(_, xs) => xs.iter().for_each(|x| collect_units(x, out)),
        Expr::Pow(a, b) | Expr::Rel(_, a, b) => {
            collect_units(a, out);
            collect_units(b, out);
        }
        _ => {}
    }
}

/// (consistent, compatible) over every sum and relation in `e`.
fn check(e: &Expr) -> R<(bool, bool)> {
    let (mut cons, mut comp) = (true, true);
    let mut groups: Vec<Vec<Expr>> = Vec::new();
    fn walk(e: &Expr, g: &mut Vec<Vec<Expr>>) {
        match e {
            Expr::Add(ts) => {
                g.push(ts.clone());
                ts.iter().for_each(|t| walk(t, g));
            }
            Expr::Rel(_, l, r) => {
                g.push(vec![(**l).clone(), (**r).clone()]);
                walk(l, g);
                walk(r, g);
            }
            Expr::Mul(xs) | Expr::Fn(_, xs) => xs.iter().for_each(|x| walk(x, g)),
            Expr::Pow(a, b) => {
                walk(a, g);
                walk(b, g);
            }
            _ => {}
        }
    }
    walk(e, &mut groups);
    for g in groups {
        let mut first: Option<(Expr, Dims)> = None;
        for t in &g {
            let (_, units) = split_term(t);
            let (_, d) = analyze(&units)?;
            let ue = units_expr(&units);
            match &first {
                None => first = Some((ue, d)),
                Some((fu, fd)) => {
                    if *fu != ue {
                        cons = false;
                    }
                    if *fd != d {
                        comp = false;
                    }
                }
            }
        }
    }
    Ok((cons, comp))
}

fn dim_name(d: &Dims) -> Option<&'static str> {
    DIM_NAMES.iter().find(|(_, x)| x == d).map(|(n, _)| *n)
}

fn strip_brackets(s: &str) -> String {
    s.replace(['[', ']'], "")
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], nargout: usize) -> R<Option<Vec<Value>>> {
    let one = |v: Value| Ok(Some(vec![v]));
    match name {
        "symunit" => one(Value::Object(Object { class: class_def(), props: Rc::new(RefCell::new(Vec::new())) })),
        "unitConvert" => {
            let s = sym_arg(a, 0)?;
            let tv = a.get(1).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
            let mut absolute = false;
            let mut i = 2;
            while i + 1 < a.len() {
                if a[i].text().map(|t| t.eq_ignore_ascii_case("Temperature")).unwrap_or(false) {
                    absolute = a[i + 1].text().map(|t| t.eq_ignore_ascii_case("absolute")).unwrap_or(false);
                }
                i += 2;
            }
            let target = if tv.is_text() && !matches!(tv, Value::Sym(_)) {
                let sys = tv.text()?;
                if !sys.eq_ignore_ascii_case("SI") && !SYSTEMS.contains(&sys.as_str()) && !CUSTOM_SYSTEMS.with(|c| c.borrow().contains(&sys)) {
                    return rt(format!("Undefined unit system '{sys}'."));
                }
                Target::Si
            } else {
                let t = scalar(a, 1)?;
                let (c, units) = split_term(&t);
                if !c.is_one() || units.is_empty() {
                    return rt("Second argument must be a unit or a unit system.");
                }
                Target::Unit(t, units)
            };
            let mut out = Vec::with_capacity(s.data.len());
            for e in &s.data {
                out.push(convert(e, &target, absolute)?);
            }
            one(Value::Sym(SymArray::from_dims(s.dims.clone(), out)))
        }
        "unitConversionFactor" => {
            let (_, fu) = split_term(&scalar(a, 0)?);
            let (_, tu) = split_term(&scalar(a, 1)?);
            let (fs, fd) = analyze(&fu)?;
            let (ts, td) = analyze(&tu)?;
            if fd != td {
                return rt("Incompatible units. Use 'checkUnits' to check dimensions and 'unitInfo' to get details on specific units.");
            }
            one(Value::Sym(SymArray::scalar(simp(Expr::div(fs, ts)))))
        }
        "separateUnits" => {
            let s = sym_arg(a, 0)?;
            let mut vals = Vec::new();
            let mut units = Vec::new();
            for e in &s.data {
                let mut uexpr: Option<Expr> = None;
                let mut cs = Vec::new();
                for t in terms(e) {
                    let (c, us) = split_term(&t);
                    let ue = units_expr(&us);
                    match &uexpr {
                        None => uexpr = Some(ue),
                        Some(u) if *u == ue => {}
                        Some(_) => return rt("Argument has incompatible units. Use 'checkUnits' to check dimensions and 'unitInfo' to get details on specific units."),
                    }
                    cs.push(c);
                }
                vals.push(Expr::add(cs));
                units.push(uexpr.unwrap_or_else(Expr::one));
            }
            let v = Value::Sym(SymArray::from_dims(s.dims.clone(), vals));
            Ok(Some(if nargout > 1 { vec![v, Value::Sym(SymArray::from_dims(s.dims.clone(), units))] } else { vec![v] }))
        }
        "isUnit" => {
            let s = sym_arg(a, 0)?;
            let vals = s.data.iter().map(|e| { let (c, us) = split_term(e); c.is_one() && !us.is_empty() }).collect();
            one(logical(s.dims.clone(), vals))
        }
        "symunit2str" => {
            let s = sym_arg(a, 0)?;
            if s.data.len() == 1 {
                return one(Value::chars(&strip_brackets(&s.data[0].to_string())));
            }
            one(Value::Cell(CellArray::from_dims(s.dims.clone(), s.data.iter().map(|e| Value::chars(&strip_brackets(&e.to_string()))).collect())))
        }
        "str2symunit" => {
            let text = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
            let e = super::parse_sym_text(&text)?;
            let mut vars = Vec::new();
            e.free_vars(&mut vars);
            let mut out = e;
            for v in vars {
                out = out.subs(&v, &unit(&v)?);
            }
            one(Value::Sym(SymArray::scalar(out)))
        }
        "checkUnits" => {
            let s = sym_arg(a, 0)?;
            let (mut cons, mut comp) = (true, true);
            for e in &s.data {
                let (c1, c2) = check(e)?;
                cons &= c1;
                comp &= c2;
            }
            match a.get(1).map(|v| v.text()).transpose()?.as_deref() {
                Some(t) if t.eq_ignore_ascii_case("Consistent") => one(Value::bool(cons)),
                Some(t) if t.eq_ignore_ascii_case("Compatible") => one(Value::bool(comp)),
                _ => {
                    let mut st = StructArray::scalar();
                    st.set("Consistent", Value::bool(cons));
                    st.set("Compatible", Value::bool(comp));
                    one(Value::Struct(st))
                }
            }
        }
        "baseUnits" | "derivedUnits" => {
            let sys = match a.first() {
                Some(v) => v.text()?,
                None => "SI".into(),
            };
            if sys != "SI" {
                return rt(format!("Unit system '{sys}' is not supported in this version."));
            }
            let names: &[&str] = if name == "baseUnits" { &BASE[..7] } else { DERIVED_SI };
            one(sym_row(names.iter().map(|n| Expr::Var(format!("[{n}]"))).collect()))
        }
        "unitSystems" => {
            let mut all: Vec<String> = SYSTEMS.iter().map(|s| s.to_string()).collect();
            all.extend(CUSTOM_SYSTEMS.with(|c| c.borrow().clone()));
            all.sort();
            let n = all.len();
            one(Value::Str(StrArray::from_dims(vec![1, n], all.into_iter().map(Some).collect())))
        }
        "findUnits" => {
            let s = sym_arg(a, 0)?;
            let mut names = Vec::new();
            for e in &s.data {
                collect_units(e, &mut names);
            }
            names.sort();
            one(sym_row(names.iter().map(|n| Expr::Var(format!("[{n}]"))).collect()))
        }
        "rewrite" => {
            let s = sym_arg(a, 0)?;
            let target = a.get(1).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
            if !matches!(target, Value::Sym(_)) {
                return rt("Only rewriting units in terms of other units is supported in this version.");
            }
            let mut out = Vec::new();
            for e in &s.data {
                out.push(convert(e, &Target::Si, false)?);
            }
            one(Value::Sym(SymArray::from_dims(s.dims.clone(), out)))
        }
        "unitInfo" => {
            match a.first() {
                None => {
                    let mut names: Vec<&str> = DIM_NAMES.iter().map(|(n, _)| *n).collect();
                    names.sort();
                    let n = names.len();
                    one(Value::Str(StrArray::from_dims(vec![n, 1], names.into_iter().map(|s| Some(s.to_string())).collect())))
                }
                Some(Value::Sym(_)) => {
                    let (_, us) = split_term(&scalar(a, 0)?);
                    let (_, d) = analyze(&us)?;
                    let n = dim_name(&d).unwrap_or("Dimensionless");
                    one(Value::Str(StrArray::from_dims(vec![1, 1], vec![Some(n.to_string())])))
                }
                Some(v) => {
                    let wanted = v.text()?;
                    let Some((_, d)) = DIM_NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(&wanted)) else {
                        return rt(format!("Undefined physical dimension '{wanted}'."));
                    };
                    let table_names: Vec<String> = TABLE.with(|t| {
                        let mut t = t.borrow_mut();
                        let t = t.get_or_insert_with(build_table);
                        t.iter().filter(|(_, u)| u.dims == *d).map(|(n, _)| n.clone()).collect()
                    });
                    let mut names = table_names;
                    names.sort_by_key(|n| n.to_lowercase());
                    let n = names.len();
                    one(Value::Sym(SymArray::from_dims(vec![n, if n == 0 { 0 } else { 1 }], names.iter().map(|n| Expr::Var(format!("[{n}]"))).collect())))
                }
            }
        }
        "mixedUnits" => {
            let (c, us) = split_term(&scalar(a, 0)?);
            let (s, d) = analyze(&us)?;
            let mut rest = simp(Expr::mul(vec![c, s]));
            let targets = sym_arg(a, 1)?;
            let n = targets.data.len();
            let mut out = Vec::with_capacity(n);
            for (k, t) in targets.data.iter().enumerate() {
                let (_, tu) = split_term(t);
                let (ts, td) = analyze(&tu)?;
                if td != d {
                    return rt("Incompatible units. Use 'checkUnits' to check dimensions and 'unitInfo' to get details on specific units.");
                }
                let q = simp(Expr::div(rest.clone(), ts.clone()));
                let whole = if k + 1 == n {
                    q
                } else {
                    let f = q.to_f64().ok_or_else(|| LangError::new("Value must be numeric.", 0))?.floor();
                    let f = Expr::from_f64(f);
                    rest = simp(Expr::sub(rest.clone(), Expr::mul(vec![f.clone(), ts])));
                    f
                };
                out.push(Expr::mul(vec![whole, t.clone()]));
            }
            one(sym_row(out))
        }
        "newUnit" => {
            let n = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
            if lookup(&n).is_some() {
                return rt(format!("Unit '{n}' already exists."));
            }
            let (c, us) = split_term(&scalar(a, 1)?);
            let (s, d) = analyze(&us)?;
            CUSTOM.with(|cu| cu.borrow_mut().push((n.clone(), UnitDef { scale: simp(Expr::mul(vec![c, s])), dims: d, offset: None })));
            if nargout > 0 {
                return one(Value::Sym(SymArray::scalar(Expr::Var(format!("[{n}]")))));
            }
            Ok(Some(Vec::new()))
        }
        "removeUnit" => {
            let names: Vec<String> = match a.first() {
                Some(Value::Sym(s)) => s.data.iter().filter_map(|e| unit_name(e).map(String::from)).collect(),
                Some(v) => vec![v.text()?],
                None => return rt("Not enough input arguments."),
            };
            for n in names {
                let found = CUSTOM.with(|c| {
                    let mut c = c.borrow_mut();
                    let before = c.len();
                    c.retain(|(x, _)| *x != n);
                    c.len() != before
                });
                if !found {
                    return rt(format!("Unable to remove unit '{n}'. Only units defined with 'newUnit' can be removed."));
                }
            }
            Ok(Some(Vec::new()))
        }
        "newUnitSystem" => {
            let n = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
            CUSTOM_SYSTEMS.with(|c| {
                if !c.borrow().contains(&n) {
                    c.borrow_mut().push(n.clone());
                }
            });
            if nargout > 0 {
                return one(Value::Str(StrArray::from_dims(vec![1, 1], vec![Some(n)])));
            }
            Ok(Some(Vec::new()))
        }
        "removeUnitSystem" => {
            let n = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
            let found = CUSTOM_SYSTEMS.with(|c| {
                let mut c = c.borrow_mut();
                let before = c.len();
                c.retain(|x| *x != n);
                c.len() != before
            });
            if !found {
                return rt(format!("Unable to remove unit system '{n}'. Only unit systems defined with 'newUnitSystem' can be removed."));
            }
            Ok(Some(Vec::new()))
        }
        _ => {
            let _ = it;
            Ok(None)
        }
    }
}
