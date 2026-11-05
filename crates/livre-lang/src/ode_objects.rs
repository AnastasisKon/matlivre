//! The `ode` object interface (R2023b): `ode`, `odeEvent`, `odeJacobian`, `odeMassMatrix`,
//! `solve` with `matlab.ode.ODEResults` and `solutionFcn`. The classes are MATLAB source run on
//! the existing solvers (`ode45`, `ode15s`, ...).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::class::ClassDef;

/// Methods of the classes (for `methods` and the coverage checklist).
pub const METHODS: &[&str] = &["solve", "solutionFcn"];

const SOLVERS: &str = "auto, ode45, ode23, ode113, ode78, ode89, ode15s, ode23s, ode23t, ode23tb, ode15i";

fn source(name: &str) -> Option<String> {
    Some(match name {
        "matlab.ode.SolverID" => format!("classdef SolverID\n  enumeration\n    {SOLVERS}\n  end\nend\n"),
        "matlab.ode.EventDirection" => "classdef EventDirection\n  enumeration\n    both, ascending, descending\n  end\nend\n".into(),
        "matlab.ode.EventResponse" => "classdef EventResponse\n  enumeration\n    proceed, stop, callback\n  end\nend\n".into(),
        "matlab.ode.MassMatrixSingular" => "classdef MassMatrixSingular\n  enumeration\n    maybe, yes, no\n  end\nend\n".into(),
        "matlab.ode.StateDependence" => "classdef StateDependence\n  enumeration\n    none, weak, strong\n  end\nend\n".into(),
        "matlab.ode.ODEResults" => "classdef ODEResults\n  properties\n    Time\n    Solution\n  end\n  properties (Hidden)\n    EventTime\n    EventSolution\n    EventIndex\n  end\n  methods\n    \
            function R = ODEResults(varargin)\n      for k = 1:2:numel(varargin)\n        R.(varargin{k}) = varargin{k+1};\n      end\n    end\n  end\nend\n"
            .into(),
        "odeEvent" => "classdef odeEvent\n  properties\n    EventFcn = []\n    Direction = matlab.ode.EventDirection.both\n    Response = matlab.ode.EventResponse.proceed\n    CallbackFcn = []\n  end\n  methods\n    \
            function E = odeEvent(varargin)\n      for k = 1:2:numel(varargin)\n        E.(varargin{k}) = varargin{k+1};\n      end\n    end\n    \
            function E = set.Direction(E, v)\n      E.Direction = matlivre_ode_enum('matlab.ode.EventDirection', v);\n    end\n    \
            function E = set.Response(E, v)\n      E.Response = matlivre_ode_enum('matlab.ode.EventResponse', v);\n    end\n  end\nend\n"
            .into(),
        "odeJacobian" => "classdef odeJacobian\n  properties\n    Jacobian = []\n    SparsityPattern = []\n  end\n  methods\n    \
            function J = odeJacobian(varargin)\n      for k = 1:2:numel(varargin)\n        J.(varargin{k}) = varargin{k+1};\n      end\n    end\n  end\nend\n"
            .into(),
        "odeMassMatrix" => "classdef odeMassMatrix\n  properties\n    MassMatrix = []\n    Singular = matlab.ode.MassMatrixSingular.maybe\n    StateDependence = matlab.ode.StateDependence.none\n    SparsityPattern = []\n  end\n  methods\n    \
            function M = odeMassMatrix(varargin)\n      for k = 1:2:numel(varargin)\n        M.(varargin{k}) = varargin{k+1};\n      end\n      \
            if isnumeric(M.MassMatrix) && ~isempty(M.MassMatrix) && ~any(strcmp(varargin(1:2:end), 'Singular'))\n        \
            if rank(full(M.MassMatrix)) < size(M.MassMatrix, 1)\n          M.Singular = matlab.ode.MassMatrixSingular.yes;\n        else\n          M.Singular = matlab.ode.MassMatrixSingular.no;\n        end\n      end\n    end\n    \
            function M = set.Singular(M, v)\n      M.Singular = matlivre_ode_enum('matlab.ode.MassMatrixSingular', v);\n    end\n    \
            function M = set.StateDependence(M, v)\n      M.StateDependence = matlivre_ode_enum('matlab.ode.StateDependence', v);\n    end\n  end\nend\n"
            .into(),
        "ode" => "classdef ode\n  properties\n    ODEFcn = []\n    InitialTime = 0\n    InitialValue = zeros(0, 1)\n    Parameters = []\n    Jacobian = []\n    MassMatrix = []\n    EventDefinition = []\n    \
            AbsoluteTolerance = 1e-6\n    RelativeTolerance = 1e-3\n    Solver = matlab.ode.SolverID.auto\n  end\n  properties (Dependent)\n    SelectedSolver\n  end\n  methods\n    \
            function F = ode(varargin)\n      for k = 1:2:numel(varargin)\n        F.(varargin{k}) = varargin{k+1};\n      end\n    end\n    \
            function F = set.Solver(F, v)\n      F.Solver = matlivre_ode_enum('matlab.ode.SolverID', v);\n    end\n    \
            function s = get.SelectedSolver(F)\n      if ~(F.Solver == matlab.ode.SolverID.auto)\n        s = F.Solver;\n      elseif isempty(F.ODEFcn)\n        s = [];\n      \
            elseif ~isempty(F.MassMatrix)\n        s = matlab.ode.SolverID.ode15s;\n      else\n        s = matlab.ode.SolverID.ode45;\n      end\n    end\n    \
            function [f, opts] = matlivreProblem(F)\n      if isempty(F.Parameters)\n        f = F.ODEFcn;\n      else\n        g = F.ODEFcn;\n        p = F.Parameters;\n        f = @(t, y) g(t, y, p);\n      end\n      \
            opts = odeset('RelTol', F.RelativeTolerance, 'AbsTol', F.AbsoluteTolerance);\n      \
            if ~isempty(F.Jacobian)\n        J = F.Jacobian;\n        if isa(J, 'odeJacobian'), J = J.Jacobian; end\n        opts = odeset(opts, 'Jacobian', J);\n      end\n      \
            if ~isempty(F.MassMatrix)\n        M = F.MassMatrix;\n        if isa(M, 'odeMassMatrix'), M = M.MassMatrix; end\n        opts = odeset(opts, 'Mass', M);\n      end\n      \
            if ~isempty(F.EventDefinition)\n        E = F.EventDefinition;\n        ef = E.EventFcn;\n        term = double(E.Response == matlab.ode.EventResponse.stop);\n        \
            dirs = [0 1 -1];\n        dir = dirs(double(E.Direction == matlab.ode.EventDirection.ascending) + 2*double(E.Direction == matlab.ode.EventDirection.descending) + 1);\n        \
            opts = odeset(opts, 'Events', @(t, y) deal(ef(t, y), term * ones(size(ef(t, y))), dir * ones(size(ef(t, y)))));\n      end\n    end\n    \
            function S = solve(F, a, b, varargin)\n      if isempty(F.ODEFcn)\n        error('The ODEFcn property must be set before solving.');\n      end\n      \
            if nargin < 3\n        tspan = a;\n        if numel(tspan) > 1 && tspan(1) ~= F.InitialTime\n          tspan = [F.InitialTime tspan(:)'];\n        end\n      else\n        tspan = [a b];\n        \
            if a ~= F.InitialTime\n          tspan = [F.InitialTime a b];\n        end\n      end\n      \
            [f, opts] = matlivreProblem(F);\n      solver = char(F.SelectedSolver);\n      \
            if isempty(F.EventDefinition)\n        [t, y] = feval(solver, f, tspan, F.InitialValue, opts);\n        S = matlab.ode.ODEResults('Time', t.', 'Solution', y.');\n      else\n        \
            [t, y, te, ye, ie] = feval(solver, f, tspan, F.InitialValue, opts);\n        S = matlab.ode.ODEResults('Time', t.', 'Solution', y.', 'EventTime', te.', 'EventSolution', ye.', 'EventIndex', ie.');\n      end\n    end\n    \
            function h = solutionFcn(F, a, b)\n      [f, opts] = matlivreProblem(F);\n      sol = feval(char(F.SelectedSolver), f, [a b], F.InitialValue, opts);\n      h = @(t) deval(sol, t);\n    end\n  end\nend\n"
            .into(),
        _ => return None,
    })
}

/// The built-in classes of the ode interface, by name.
pub fn builtin_class(name: &str) -> Option<Rc<ClassDef>> {
    thread_local! {
        static DEFS: RefCell<HashMap<String, Rc<ClassDef>>> = RefCell::new(HashMap::new());
    }
    if let Some(c) = DEFS.with(|d| d.borrow().get(name).cloned()) {
        return Some(c);
    }
    let src = source(name)?;
    let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
    let mut def = ClassDef::build(ast, &[]);
    def.name = name.to_string();
    let def = Rc::new(def);
    DEFS.with(|d| d.borrow_mut().insert(name.to_string(), def.clone()));
    Some(def)
}

pub const NAMES: &[&str] = &["matlivre_ode_enum"];

/// `matlivre_ode_enum(class, v)`: v as a member of the enumeration class (from its name).
pub fn call(it: &mut crate::interp::Interpreter, name: &str, a: &[crate::value::Value], _nargout: usize) -> Result<Option<Vec<crate::value::Value>>, crate::error::LangError> {
    use crate::value::*;
    if name != "matlivre_ode_enum" {
        return Ok(None);
    }
    let cls = a.first().map(|v| v.text()).transpose()?.unwrap_or_default();
    let v = a.get(1).cloned().unwrap_or_else(Value::empty);
    if let Value::Object(o) = &v {
        if o.class.name == cls {
            return Ok(Some(vec![v]));
        }
    }
    let wanted = v.text()?;
    let out = it.call_function("enumeration", vec![Value::chars(&cls)], 2)?;
    let names: Vec<String> = match out.get(1) {
        Some(Value::Cell(c)) => c.data.iter().map(|x| x.text().unwrap_or_default()).collect(),
        _ => Vec::new(),
    };
    let Some(k) = names.iter().position(|n| n.eq_ignore_ascii_case(&wanted)) else {
        return crate::error::rt(format!("'{wanted}' is not a valid value. Valid values are {}.", names.iter().map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ")));
    };
    let member = match &out[0] {
        Value::Cell(c) => c.data[k].clone(),
        other => crate::indexing::index(other, &[crate::nd::IndexArg::scalar(k)])?,
    };
    Ok(Some(vec![member]))
}

/// The display of an `ode` object: the problem definition, then the solver properties.
pub fn object_text(o: &crate::class::Object) -> Option<String> {
    use crate::value::Value;
    if o.class.name != "ode" {
        return None;
    }
    let get = |n: &str| o.class.prop_index(n).map(|i| o.props.borrow()[i].clone()).unwrap_or_else(Value::empty);
    let text = |v: &Value| -> String {
        match v {
            Value::Object(e) => crate::class::enum_name(e).unwrap_or_else(|| crate::display::summary(v, Default::default())),
            Value::Num(m) if m.is_empty() && m.dims != [0, 0] => format!("[{} double]", m.dims.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x")),
            other => crate::display::summary(other, Default::default()),
        }
    };
    let empty = |v: &Value| matches!(v, Value::Num(m) if m.is_empty() && m.dims == [0, 0]);
    let mut problem = vec![("ODEFcn", get("ODEFcn"))];
    if !empty(&get("Parameters")) {
        problem.push(("Parameters", get("Parameters")));
    }
    problem.push(("InitialTime", get("InitialTime")));
    problem.push(("InitialValue", get("InitialValue")));
    for p in ["Jacobian", "MassMatrix", "EventDefinition"] {
        if !empty(&get(p)) {
            problem.push((p, get(p)));
        }
    }
    let solver = get("Solver");
    let mut solver_rows = vec![("AbsoluteTolerance", text(&get("AbsoluteTolerance"))), ("RelativeTolerance", text(&get("RelativeTolerance"))), ("Solver", text(&solver))];
    if !empty(&get("ODEFcn")) {
        let s = text(&solver);
        let selected = if s != "auto" { s } else if !empty(&get("MassMatrix")) { "ode15s".into() } else { "ode45".into() };
        solver_rows.push(("SelectedSolver", selected));
    }
    let rows: Vec<(&str, String)> = problem.iter().map(|(n, v)| (*n, text(v))).collect();
    let w = rows.iter().chain(&solver_rows).map(|(n, _)| n.len()).max().unwrap_or(0);
    let mut out = String::from("  ode with properties:\n\n   Problem definition\n");
    for (n, v) in &rows {
        out.push_str(&format!("    {n:>w$}: {v}\n"));
    }
    out.push_str("\n   Solver properties\n");
    for (n, v) in &solver_rows {
        out.push_str(&format!("    {n:>w$}: {v}\n"));
    }
    out.push_str("\n  Show all properties\n");
    Some(out)
}
