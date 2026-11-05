//! Table custom properties (`addprop`, `rmprop`) and `decomposition` objects with
//! `isIllConditioned`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["addprop", "rmprop", "matlivre_decomposition_type"];
pub const CLASS: &str = "decomposition";

const TYPES: &[&str] = &["auto", "lu", "qr", "chol", "ldl", "triangular", "permutedTriangular", "banded", "hessenberg", "diagonal", "cod"];

/// The `decomposition` class: solves with the stored matrix through `\` and `/`.
pub fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| {
                let src = "classdef decomposition\n  properties\n    MatrixSize\n    Type\n  end\n  properties (Hidden)\n    Matrix\n  end\n  methods\n    \
                    function d = decomposition(A, varargin)\n      d.Type = matlivre_decomposition_type(A, varargin{:});\n      d.Matrix = A;\n      d.MatrixSize = size(A);\n    end\n    \
                    function x = mldivide(d, b)\n      x = d.Matrix \\ b;\n    end\n    \
                    function x = mrdivide(b, d)\n      x = b / d.Matrix;\n    end\n    \
                    function r = rcond(d)\n      r = rcond(d.Matrix);\n    end\n    \
                    function tf = isIllConditioned(d)\n      tf = rcond(d.Matrix) < eps(class(d.Matrix));\n    end\n  end\nend\n";
                let ast = crate::class::parse_classdef(src).expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = CLASS.into();
                Rc::new(def)
            })
            .clone()
    })
}

/// The display of a decomposition object.
pub fn object_text(o: &Object) -> Option<String> {
    if o.class.name != CLASS {
        return None;
    }
    let get = |n: &str| o.class.prop_index(n).map(|i| o.props.borrow()[i].clone());
    let size = get("MatrixSize")?.to_matrix().ok()?;
    let ty = get("Type")?.text().ok()?;
    let dims: Vec<String> = size.data.iter().map(|x| format!("{x}")).collect();
    Some(format!("  decomposition with properties:\n\n    MatrixSize: [{}]\n          Type: '{ty}'\n\n  Show all properties\n", dims.join(" ")))
}

fn is_triangular(m: &Matrix) -> bool {
    let (r, c) = (m.rows(), m.cols());
    if r != c {
        return false;
    }
    let at = |i: usize, j: usize| m.data[j * r + i];
    let upper = (0..r).all(|i| (0..i.min(c)).all(|j| at(i, j) == 0.0));
    let lower = (0..r).all(|i| (i + 1..c).all(|j| at(i, j) == 0.0));
    upper || lower
}

fn decomposition_type(a: &[Value]) -> R<Value> {
    let m = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.to_matrix()?;
    let mut ty = "auto".to_string();
    let mut i = 1;
    while i < a.len() {
        let t = a[i].text()?;
        if TYPES.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            ty = TYPES.iter().find(|x| x.eq_ignore_ascii_case(&t)).unwrap().to_string();
            i += 1;
        } else if matches!(t.to_lowercase().as_str(), "checkcondition" | "rankcheck" | "upper" | "lower" | "ranktolerance") {
            i += 2;
        } else {
            return rt(format!("Invalid decomposition type '{t}'."));
        }
    }
    let square = m.rows() == m.cols();
    let ty = match ty.as_str() {
        "auto" if square && is_triangular(&m) => "triangular".to_string(),
        "auto" if square => "lu".to_string(),
        "auto" => "qr".to_string(),
        "diagonal" => "triangular".to_string(),
        "lu" | "chol" | "ldl" | "triangular" | "hessenberg" | "permutedTriangular" | "banded" if !square => {
            return rt(format!("Decomposition type '{ty}' requires a square matrix."));
        }
        "chol" => {
            // A Cholesky factor exists only for a Hermitian positive definite matrix.
            let n = m.rows();
            let at = |i: usize, j: usize| m.data[j * n + i];
            let symmetric = (0..n).all(|i| (0..n).all(|j| at(i, j) == at(j, i)));
            let mut l = vec![0.0; n * n];
            let mut ok = symmetric;
            for j in 0..n {
                if !ok {
                    break;
                }
                let d = at(j, j) - (0..j).map(|k| l[k * n + j] * l[k * n + j]).sum::<f64>();
                if d <= 0.0 {
                    ok = false;
                    break;
                }
                l[j * n + j] = d.sqrt();
                for i in j + 1..n {
                    l[j * n + i] = (at(i, j) - (0..j).map(|k| l[k * n + i] * l[k * n + j]).sum::<f64>()) / l[j * n + j];
                }
            }
            if !ok {
                return rt("Matrix must be symmetric positive definite for a Cholesky decomposition.");
            }
            ty
        }
        _ => ty,
    };
    Ok(Value::chars(&ty))
}

fn names_of(v: &Value) -> R<Vec<String>> {
    Ok(match v {
        Value::Cell(c) => c.data.iter().map(|x| x.text()).collect::<R<_>>()?,
        Value::Str(s) => s.data.iter().map(|x| x.clone().unwrap_or_default()).collect(),
        other => vec![other.text()?],
    })
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> R<Option<Vec<Value>>> {
    match name {
        "matlivre_decomposition_type" => Ok(Some(vec![decomposition_type(a)?])),
        "addprop" | "rmprop" => {
            let Some(Value::Table(t)) = a.first() else { return Ok(None) };
            let mut t = t.clone();
            let names = names_of(a.get(1).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?)?;
            if name == "rmprop" {
                t.custom.retain(|(n, _, _)| !names.contains(n));
                return Ok(Some(vec![Value::Table(t)]));
            }
            let kinds = names_of(a.get(2).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?)?;
            let bad = || LangError::new("For each custom property, specify 'variable' or 'table' to indicate whether the property applies to the variables, or to the table as a whole. Specify these settings in a string array or cell array of character vectors.", 0);
            if kinds.len() != names.len() {
                return Err(bad());
            }
            for (n, k) in names.iter().zip(&kinds) {
                let per_var = match k.as_str() {
                    "variable" => true,
                    "table" => false,
                    _ => return Err(bad()),
                };
                if let Some((_, pv, _)) = t.custom.iter().find(|(x, _, _)| x == n) {
                    return rt(format!("New custom property name '{n}' conflicts with existing '{}' custom property '{n}'.", if *pv { "variable" } else { "table" }));
                }
                if t.names.iter().any(|x| x == n) || matches!(n.as_str(), "Description" | "UserData" | "DimensionNames" | "VariableNames" | "RowNames") {
                    return rt(format!("Custom property name '{n}' conflicts with a table property or variable name."));
                }
                t.custom.push((n.clone(), per_var, Value::empty()));
            }
            Ok(Some(vec![Value::Table(t)]))
        }
        _ => Ok(None),
    }
}
