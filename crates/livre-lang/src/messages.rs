//! `message` objects: an identifier from MATLAB's message catalog and the values for its
//! holes. `getString` (and `string`) fill the template; `error`, `warning` and `MException`
//! accept a message in place of an identifier and text.

use std::cell::RefCell;
use std::rc::Rc;

use crate::builtins::Ret;
use crate::class::{ClassDef, Object};
use crate::error::{rt, rt_id, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["matlivre_message_new", "matlivre_message_text"];
pub const CLASS: &str = "message";
/// Methods of the class (for `methods` and the coverage checklist).
pub const METHODS: &[&str] = &["getString", "string"];

/// Templates of the catalog entries MatLivre knows; `{k}` is hole k.
const CATALOG: &[(&str, &str)] = &[
    ("MATLAB:InputParser:ArgumentFailedValidation", "The value of '{0}' is invalid. It must satisfy the function: {1}."),
    ("MATLAB:InputParser:ParamMissingValue", "No value was given for '{0}'. Name-value pair arguments require a name followed by a value."),
    ("MATLAB:InputParser:ParamMustBeChar", "Expected a string scalar or character vector for the parameter name."),
    ("MATLAB:InputParser:UnmatchedParameter", "'{0}' is not a recognized parameter. {1}"),
    ("MATLAB:MKDIR:DirectoryExists", "Directory already exists."),
    ("MATLAB:TooManyInputs", "Too many input arguments."),
    ("MATLAB:TooManyOutputs", "Too many output arguments."),
    ("MATLAB:UndefinedFunction", "{0}"),
    ("MATLAB:badsubscript", "{0}"),
    ("MATLAB:graphics:validatecolor:InvalidColorString", "'{0}' is not a valid color specification. Valid names include: 'red', 'green', 'blue', 'cyan', 'magenta', 'yellow', 'black', and 'white'. Valid hexadecimal color codes consist of '#' followed by three or six hexadecimal digits."),
    ("MATLAB:graphics:validatecolor:MultipleColors", "Specify one color specification or use the 'multiple' option."),
    ("MATLAB:graphics:validatecolor:OutOfRange", "Specify double or single values in the range [0, 1]."),
    ("MATLAB:innerdim", "Incorrect dimensions for matrix multiplication. Check that the number of columns in the first matrix matches the number of rows in the second matrix. To operate on each element of the matrix individually, use TIMES (.*) for elementwise multiplication."),
    ("MATLAB:javachk:featureNotAvailable", "{0} is not supported because:\\n{1}"),
    ("MATLAB:javachk:thisFeatureNotAvailable", "This feature is not supported because:\\n{0}"),
    ("MATLAB:matrix:singleSubscriptNumelMismatch", "Unable to perform assignment because the left and right sides have a different number of elements."),
    ("MATLAB:minrhs", "Not enough input arguments."),
    ("MATLAB:nonExistentField", "Unrecognized field name \"{0}\"."),
    ("MATLAB:sizeDimensionsMustMatch", "Arrays have incompatible sizes for this operation."),
    ("MATLAB:nomem", "Out of memory."),
    ("MATLAB:dimagree", "Matrix dimensions must agree."),
    ("MATLAB:maxrhs", "Too many input arguments."),
    ("MATLAB:nologicalnan", "NaN values cannot be converted to logicals."),
    ("MATLAB:undefinedVarOrClass", "Unable to resolve the name '{0}'."),
    ("MATLAB:subsassigndimmismatch", "Unable to perform assignment because the indices on the left side are not compatible with the size of the right side."),
    ("MATLAB:validators:mustBePositive", "Value must be positive."),
    ("MATLAB:validators:mustBeInteger", "Value must be integer."),
    ("MATLAB:validators:mustBeNumeric", "Value must be numeric."),
    ("MATLAB:validators:mustBeNonnegative", "Value must be nonnegative."),
    ("MATLAB:validators:mustBeFinite", "Value must be finite."),
    ("MATLAB:validators:mustBeMember", "Value must be a member of this set:{0}\\n"),
    ("MATLAB:validators:mustBeNonempty", "Value must not be empty."),
    ("MATLAB:validators:mustBeText", "Value must be a character vector, string array, or cell array of character vectors."),
    ("MATLAB:validators:mustBeFile", "The following files do not exist: {0}"),
    ("MATLAB:validators:mustBeFolder", "The following folders do not exist: {0}"),
    ("MATLAB:FileIO:InvalidFid", "Invalid file identifier. Use fopen to generate a valid file identifier."),
    ("MATLAB:load:couldNotReadFile", "Unable to find file or directory '{0}'."),
    ("MATLAB:mustBeFieldName", "Argument to dynamic structure reference must evaluate to a valid field name."),
    ("MATLAB:mpower:notScalarAndSquareMatrix", "Incorrect dimensions for raising a matrix to a power. Check that the matrix is square and the power is a scalar. To operate on each element of the matrix individually, use POWER (.^) for elementwise power."),
    ("MATLAB:square", "Matrix must be square."),
    ("MATLAB:cellRefFromNonCell", "Brace indexing is not supported for variables of this type."),
    ("MATLAB:structRefFromNonStruct", "Dot indexing is not supported for variables of this type."),
    ("MATLAB:needMoreRhsOutputs", "Insufficient number of outputs from right hand side of equal sign to satisfy assignment."),
    ("MATLAB:unassignedOutputs", "{0}"),
    ("MATLAB:class:InvalidHandle", "Invalid or deleted object."),
    ("MATLAB:class:undefinedMethod", "Method '{0}' is not defined for class '{1}' or is removed from MATLAB's search path."),
    ("MATLAB:noSuchMethodOrField", "Unrecognized method, property, or field '{0}' for class '{1}'."),
    ("MATLAB:str2func:invalidFunctionName", "The input to STR2FUNC \"{0}\" is not a valid function name. This will generate an error in a future release."),
    ("MATLAB:inputArgUndefined", "Input argument \"{0}\" is undefined."),
    ("MATLAB:interp1:NaNinX", "NaN is not an appropriate value for X."),
    ("MATLAB:colon:nonFiniteEndpoint", "Nonfinite endpoints or increment for colon operator in index."),
];

/// Catalogs that exist but whose entries MatLivre does not carry.
const OTHER_CATALOGS: &[&str] = &["MATLAB:catenate", "MATLAB:graphics:validatecolor", "MATLAB:index", "MATLAB:MKDIR"];

fn catalog_of(id: &str) -> Option<&str> {
    id.rfind(':').map(|k| &id[..k])
}

fn catalog_exists(cat: &str) -> bool {
    OTHER_CATALOGS.contains(&cat) || CATALOG.iter().any(|(id, _)| catalog_of(id) == Some(cat))
}

/// The `message` class: `message(id, holes...)` with `getString` and `string`.
pub fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| {
                let src = "classdef message\n  properties\n    Identifier\n    Arguments\n  end\n  methods\n    \
                    function m = message(varargin)\n      [id, args] = matlivre_message_new(varargin{:});\n      m.Identifier = id;\n      m.Arguments = args;\n    end\n    \
                    function s = getString(m)\n      s = matlivre_message_text(m.Identifier, m.Arguments{:});\n    end\n    \
                    function s = string(m)\n      s = string(matlivre_message_text(m.Identifier, m.Arguments{:}));\n    end\n  end\nend\n";
                debug_assert!(METHODS.iter().all(|m| src.contains(&format!("= {m}("))));
                let ast = crate::class::parse_classdef(src).expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = CLASS.into();
                Rc::new(def)
            })
            .clone()
    })
}

/// The text of a message: its template with the holes filled.
pub fn text(id: &str, holes: &[Value]) -> R<String> {
    let Some(&(_, template)) = CATALOG.iter().find(|(k, _)| *k == id) else {
        let cat = catalog_of(id).unwrap_or("");
        let key = id.rsplit(':').next().unwrap_or(id);
        return rt_id("MATLAB:builtins:MessageNotFound", format!("Unable to find message key '{key}' in catalog '{cat}'."));
    };
    let expected = (0..10).filter(|k| template.contains(&format!("{{{k}}}"))).map(|k| k + 1).max().unwrap_or(0);
    if expected != holes.len() {
        return rt_id(
            "MATLAB:builtins:IncorrectHoleCount",
            format!("Incorrect number of parameters supplied for '{id}'. Expected {expected} but found {}.", holes.len()),
        );
    }
    let mut out = template.to_string();
    for (k, v) in holes.iter().enumerate() {
        let filled = match v {
            _ if v.is_text() => v.text()?,
            Value::Num(m) if m.kind == Kind::Logical => {
                return rt_id("MATLAB:builtins:BadDataType", format!("In '{id}', data type supplied is incorrect for parameter {{{k}}}."));
            }
            Value::Num(m) if m.numel() != 1 || m.is_complex() => {
                return rt_id("MATLAB:builtins:NonScalarInput", format!("In '{id}', parameter {{{k}}} must be a real scalar."));
            }
            Value::Num(m) => {
                let x = m.data[0];
                if x.fract() != 0.0 || !x.is_finite() {
                    return rt_id(
                        "MATLAB:builtins:NoFloatingPointNumbers",
                        format!("Error filling holes for {id}. Floating point numbers are not allowed as holes. They should be converted to character vectors."),
                    );
                }
                format!("{x}")
            }
            _ => return rt_id("MATLAB:builtins:BadDataType", format!("In '{id}', data type supplied is incorrect for parameter {{{k}}}.")),
        };
        out = out.replace(&format!("{{{k}}}"), &filled);
    }
    Ok(out)
}

/// The text and identifier of a message object (for `error`, `warning` and `MException`).
pub fn parts(o: &Object) -> Option<R<(String, String)>> {
    if o.class.name != CLASS {
        return None;
    }
    let get = |n: &str| o.class.prop_index(n).map(|i| o.props.borrow()[i].clone());
    let id = get("Identifier")?.text().ok()?;
    let holes = match get("Arguments")? {
        Value::Cell(c) => c.data.clone(),
        _ => Vec::new(),
    };
    Some(text(&id, &holes).map(|t| (t, id)))
}

pub fn call(_it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "matlivre_message_new" => {
            let Some(idv) = a.first() else { return rt("Invalid constructor signature.") };
            if !(idv.is_text() && idv.numel() >= 1 && (matches!(idv, Value::Str(_)) && idv.numel() == 1 || matches!(idv, Value::Num(m) if m.kind == Kind::Char && m.rows() <= 1))) {
                return rt("Invalid input for argument 1 (rhs1):\nValue must be a character vector or a string scalar.");
            }
            let id = idv.text()?;
            if !catalog_of(&id).is_some_and(catalog_exists) {
                return rt_id("MATLAB:builtins:InvalidMessageID", format!("Invalid Message ID format: '{id}'."));
            }
            // String holes are kept as character vectors.
            let holes: Vec<Value> = a[1..].iter().map(|v| if matches!(v, Value::Str(_)) { v.text().map(|t| Value::chars(&t)) } else { Ok(v.clone()) }).collect::<R<_>>()?;
            Ok(Some(vec![Value::chars(&id), Value::Cell(CellArray::row(holes))]))
        }
        "matlivre_message_text" => {
            let id = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
            Ok(Some(vec![Value::chars(&text(&id, &a[1..])?)]))
        }
        _ => Ok(None),
    }
}
