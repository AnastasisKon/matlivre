//! `polyshape` and `nsidedpoly`: 2-D polygons with the `Vertices`, `NumRegions` and `NumHoles`
//! properties, their measuring, transforming and boundary methods, boolean operations
//! (`union`, `intersect`, `subtract`, `xor`), `polybuffer`, `simplify`, `rmslivers`,
//! `turningdist` and `boundaryshape`.
//!
//! Like the other built-in classes, `polyshape` is an ordinary value class whose definition is
//! MATLAB source generated here; each method forwards to the hidden builtin
//! `matlivre_polyshape_method`. Boundaries are separated by NaN rows in `Vertices`: solid
//! boundaries run clockwise and holes counterclockwise, as MATLAB stores them. Input that is
//! already simple keeps its vertex order; boolean operations and simplification (by the
//! even-odd rule, through the `geo` crate) start each boundary at its lowest-left vertex.
//! Methods returning several polyshapes (`regions`, `holes`) return a cell column, since
//! object arrays are not supported.

use std::cell::RefCell;
use std::rc::Rc;

use geo::algorithm::bool_ops::{FillRule, OpType};
use geo::algorithm::buffer::{BufferStyle, LineCap, LineJoin};
use geo::{BooleanOps, Buffer, Coord, LineString, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon};

use crate::builtins::{one, Ret};
use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::{Interpreter, OutputKind};
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["polyshape", "nsidedpoly", "polybuffer", "boundaryshape", "matlivre_polyshape_method"];

const METHODS: &[&str] = &[
    "area", "perimeter", "centroid", "translate", "rotate", "scale", "numsides", "isinterior", "boundingbox", "plot", "regions", "holes", "ishole",
    "numboundaries", "rmholes", "addboundary", "rmboundary", "issimplified", "simplify", "union", "intersect", "subtract", "xor", "convhull",
    "nearestvertex", "overlaps", "polybuffer", "sortboundaries", "sortregions", "boundary", "rmslivers", "turningdist",
];

const MODIFIED: &str =
    "Polyshape has duplicate vertices, intersections, or other inconsistencies that may produce inaccurate or unexpected results. Input data has been modified to create a well-defined polyshape.";

fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        if let Some(def) = d.borrow().as_ref() {
            return def.clone();
        }
        let mut src = String::from("classdef polyshape\n  properties\n    Vertices\n    NumRegions\n    NumHoles\n  end\n  methods\n");
        for m in METHODS {
            src.push_str(&format!(
                "    function varargout = {m}(obj, varargin)\n      varargout = matlivre_polyshape_method('{m}', max(nargout, 1), obj, varargin{{:}});\n    end\n"
            ));
        }
        src.push_str("  end\nend\n");
        let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
        let def = Rc::new(ClassDef::build(ast, &[]));
        *d.borrow_mut() = Some(def.clone());
        def
    })
}

/// One boundary as (x, y) points, not closed (the last point is not the first repeated).
type Ring = Vec<(f64, f64)>;

fn signed_area(r: &Ring) -> f64 {
    let n = r.len();
    (0..n)
        .map(|i| {
            let (a, b) = (r[i], r[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

/// Which boundaries are holes: those nested at odd depth (MATLAB keeps solids clockwise and
/// holes counterclockwise, but some results, such as `holes` and `boundaryshape`, keep the
/// order they were built in).
fn hole_flags(rings: &[Ring]) -> Vec<bool> {
    classify(rings)
}

/// Orients a boundary (clockwise for a solid, counterclockwise for a hole), keeping its
/// first vertex.
fn orient(r: &mut Ring, hole: bool) {
    if r.len() > 2 && (signed_area(r) > 0.0) != hole {
        r[1..].reverse();
    }
}

/// The polyshape object for these boundaries, oriented as they are.
fn make(mut rings: Vec<Ring>) -> Value {
    rings.retain(|r| r.len() >= 3);
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for (k, r) in rings.iter().enumerate() {
        if k > 0 {
            xs.push(f64::NAN);
            ys.push(f64::NAN);
        }
        for &(x, y) in r {
            xs.push(x);
            ys.push(y);
        }
    }
    let n = xs.len();
    let mut data = xs;
    data.extend(ys);
    let verts = if n == 0 { Matrix::new(0, 2, Vec::new()) } else { Matrix::new(n, 2, data) };
    let holes = hole_flags(&rings).into_iter().filter(|&h| h).count();
    let props = vec![Value::Num(verts), Value::num((rings.len() - holes) as f64), Value::num(holes as f64)];
    Value::Object(Object { class: class_def(), props: Rc::new(RefCell::new(props)) })
}

/// Boundaries from x and y vectors, NaN-separated, with consecutive duplicates and a closing
/// repeat of the first point removed.
fn rings_from(x: &[f64], y: &[f64]) -> Vec<Ring> {
    let mut rings = vec![Vec::new()];
    for (&a, &b) in x.iter().zip(y) {
        if a.is_nan() || b.is_nan() {
            rings.push(Vec::new());
            continue;
        }
        let r = rings.last_mut().unwrap();
        if r.last() != Some(&(a, b)) {
            r.push((a, b));
        }
    }
    for r in &mut rings {
        if r.len() > 1 && r.first() == r.last() {
            r.pop();
        }
    }
    rings.into_iter().filter(|r| !r.is_empty()).collect()
}

fn rings_of(o: &Object) -> R<Vec<Ring>> {
    let v = o.get("Vertices").unwrap_or_else(Value::empty).to_matrix()?;
    if v.is_empty() {
        return Ok(Vec::new());
    }
    let n = v.rows();
    Ok(rings_from(&v.data[..n], &v.data[n..]))
}

fn poly_of(v: &Value) -> R<Vec<Ring>> {
    match v {
        Value::Object(o) if o.class.name == "polyshape" => rings_of(o),
        _ => rt("Expected a polyshape."),
    }
}

// ---- geometry helpers --------------------------------------------------------------------------

fn cross(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

fn scale_of(rings: &[Ring]) -> f64 {
    rings.iter().flatten().fold(1.0f64, |m, p| m.max(p.0.abs()).max(p.1.abs()))
}

/// Removes collinear vertices (and back-and-forth spikes).
fn drop_collinear(r: &Ring, tol: f64) -> Ring {
    let mut r = r.clone();
    loop {
        let n = r.len();
        if n < 3 {
            return r;
        }
        let k = (0..n).find(|&i| cross(r[(i + n - 1) % n], r[i], r[(i + 1) % n]).abs() <= tol);
        match k {
            Some(i) => {
                r.remove(i);
            }
            None => return r,
        }
    }
}

/// Whether segments ab and cd touch or cross.
fn segments_meet(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    let on = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| r.0 >= p.0.min(q.0) && r.0 <= p.0.max(q.0) && r.1 >= p.1.min(q.1) && r.1 <= p.1.max(q.1);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    (d1 == 0.0 && on(c, d, a)) || (d2 == 0.0 && on(c, d, b)) || (d3 == 0.0 && on(a, b, c)) || (d4 == 0.0 && on(a, b, d))
}

/// No boundary crosses or touches itself or another, and no vertex repeats.
fn is_simple(rings: &[Ring]) -> bool {
    let edges: Vec<(usize, usize, (f64, f64), (f64, f64))> =
        rings.iter().enumerate().flat_map(|(k, r)| (0..r.len()).map(move |i| (k, i, r[i], r[(i + 1) % r.len()]))).collect();
    if rings.iter().any(|r| r.len() < 3) {
        return false;
    }
    for (x, e) in edges.iter().enumerate() {
        for f in &edges[x + 1..] {
            let n = rings[e.0].len();
            let adjacent = e.0 == f.0 && (f.1 == (e.1 + 1) % n || e.1 == (f.1 + 1) % n);
            if adjacent {
                // Adjacent edges may only share their common vertex.
                if cross(e.2, e.3, f.3).abs() == 0.0 && cross(e.2, e.3, f.2).abs() == 0.0 && (f.1 == (e.1 + 1) % n) && {
                    let (d1, d2) = ((e.3 .0 - e.2 .0, e.3 .1 - e.2 .1), (f.3 .0 - f.2 .0, f.3 .1 - f.2 .1));
                    d1.0 * d2.0 + d1.1 * d2.1 < 0.0
                } {
                    return false;
                }
                continue;
            }
            if segments_meet(e.2, e.3, f.2, f.3) {
                return false;
            }
        }
    }
    true
}

/// Point in polygon by the even-odd rule over all boundaries (boundary points count as inside).
fn inside(rings: &[Ring], (x, y): (f64, f64)) -> bool {
    let mut c = false;
    for r in rings {
        let n = r.len();
        for i in 0..n {
            let (p, q) = (r[i], r[(i + 1) % n]);
            if cross(p, q, (x, y)) == 0.0 && x >= p.0.min(q.0) && x <= p.0.max(q.0) && y >= p.1.min(q.1) && y <= p.1.max(q.1) {
                return true;
            }
            if (p.1 > y) != (q.1 > y) && x < (q.0 - p.0) * (y - p.1) / (q.1 - p.1) + p.0 {
                c = !c;
            }
        }
    }
    c
}

fn on_boundary(rings: &[Ring], (x, y): (f64, f64)) -> bool {
    rings.iter().any(|r| {
        let n = r.len();
        (0..n).any(|i| {
            let (p, q) = (r[i], r[(i + 1) % n]);
            let len = (q.0 - p.0).hypot(q.1 - p.1);
            let tol = 1e-12 * (1.0 + len);
            cross(p, q, (x, y)).abs() <= tol * len.max(1.0) && x >= p.0.min(q.0) - tol && x <= p.0.max(q.0) + tol && y >= p.1.min(q.1) - tol && y <= p.1.max(q.1) + tol
        })
    })
}

fn strictly_inside(r: &Ring, (x, y): (f64, f64)) -> bool {
    let n = r.len();
    let mut c = false;
    for i in 0..n {
        let (p, q) = (r[i], r[(i + 1) % n]);
        if (p.1 > y) != (q.1 > y) && x < (q.0 - p.0) * (y - p.1) / (q.1 - p.1) + p.0 {
            c = !c;
        }
    }
    c
}

/// Holes of simple input by nesting depth: odd depth makes a boundary a hole.
fn classify(rings: &[Ring]) -> Vec<bool> {
    (0..rings.len())
        .map(|k| {
            let p = rings[k][0];
            rings.iter().enumerate().filter(|&(j, r)| j != k && strictly_inside(r, p)).count() % 2 == 1
        })
        .collect()
}

/// Rotates a boundary to start at its lowest-left vertex (smallest x, then smallest y).
fn start_low_left(r: &mut Ring) {
    if let Some(k) = (0..r.len()).min_by(|&a, &b| r[a].0.total_cmp(&r[b].0).then(r[a].1.total_cmp(&r[b].1))) {
        r.rotate_left(k);
    }
}

fn low_left(r: &Ring) -> (f64, f64) {
    r.iter().copied().fold((f64::INFINITY, f64::INFINITY), |m, p| if (p.0, p.1) < m { p } else { m })
}

fn ring_to_ls(r: &Ring) -> LineString<f64> {
    let mut c: Vec<Coord<f64>> = r.iter().map(|&(x, y)| Coord { x, y }).collect();
    if let Some(&f) = c.first() {
        c.push(f);
    }
    LineString::new(c)
}

fn ls_to_ring(l: &LineString<f64>) -> Ring {
    let mut r: Ring = l.coords().map(|c| (c.x, c.y)).collect();
    if r.len() > 1 && r.first() == r.last() {
        r.pop();
    }
    r
}

/// Solid boundaries with their holes (each hole goes to the smallest solid containing it).
fn regions_of(rings: &[Ring]) -> Vec<(Ring, Vec<Ring>)> {
    let flags = hole_flags(rings);
    let mut regions: Vec<(Ring, Vec<Ring>)> = rings.iter().zip(&flags).filter(|(_, &h)| !h).map(|(r, _)| (r.clone(), Vec::new())).collect();
    for h in rings.iter().zip(&flags).filter(|(_, &h)| h).map(|(r, _)| r) {
        let best = (0..regions.len()).filter(|&k| strictly_inside(&regions[k].0, h[0]) || h.iter().all(|&p| inside(&[regions[k].0.clone()], p))).min_by(|&a, &b| {
            signed_area(&regions[a].0).abs().total_cmp(&signed_area(&regions[b].0).abs())
        });
        if let Some(k) = best {
            regions[k].1.push(h.clone());
        }
    }
    regions
}

fn to_geo(rings: &[Ring]) -> MultiPolygon<f64> {
    MultiPolygon::new(regions_of(rings).into_iter().map(|(o, hs)| Polygon::new(ring_to_ls(&o), hs.iter().map(ring_to_ls).collect())).collect())
}

/// MATLAB's layout of a geo result: regions by their lowest-left vertex, each followed by
/// its holes; solids clockwise, holes counterclockwise, collinear vertices removed.
fn from_geo(mp: &MultiPolygon<f64>) -> Vec<Ring> {
    let tol = 1e-12 * mp.0.iter().flat_map(|p| p.exterior().coords()).fold(1.0f64, |m, c| m.max(c.x.abs()).max(c.y.abs()));
    let mut regions: Vec<(Ring, Vec<Ring>)> = mp
        .0
        .iter()
        .map(|p| {
            let mut o = drop_collinear(&ls_to_ring(p.exterior()), tol * tol.max(1.0));
            orient(&mut o, false);
            start_low_left(&mut o);
            let mut hs: Vec<Ring> = p
                .interiors()
                .iter()
                .map(|h| {
                    let mut h = drop_collinear(&ls_to_ring(h), tol);
                    orient(&mut h, true);
                    start_low_left(&mut h);
                    h
                })
                .filter(|h| h.len() >= 3)
                .collect();
            hs.sort_by(|a, b| low_left(a).partial_cmp(&low_left(b)).unwrap_or(std::cmp::Ordering::Equal));
            (o, hs)
        })
        .filter(|(o, _)| o.len() >= 3)
        .collect();
    regions.sort_by(|a, b| low_left(&a.0).partial_cmp(&low_left(&b.0)).unwrap_or(std::cmp::Ordering::Equal));
    regions.into_iter().flat_map(|(o, hs)| std::iter::once(o).chain(hs)).collect()
}

/// The well-defined shape of arbitrary boundaries, by the even-odd rule.
fn resolve(rings: &[Ring]) -> Vec<Ring> {
    let subject = MultiPolygon::new(rings.iter().filter(|r| r.len() >= 3).map(|r| Polygon::new(ring_to_ls(r), Vec::new())).collect());
    from_geo(&subject.boolean_op_with_fill_rule(&MultiPolygon::<f64>::new(Vec::new()), OpType::Union, FillRule::EvenOdd))
}

/// Simplification as the constructor does it: duplicates and collinear points removed, holes
/// found by nesting; inconsistent input is resolved by the even-odd rule (with a warning).
fn simplify_rings(it: &mut Interpreter, rings: Vec<Ring>, keep_collinear: bool, warn: bool) -> Vec<Ring> {
    let tol = 1e-12 * scale_of(&rings).powi(2);
    let mut rings: Vec<Ring> = rings.into_iter().map(|r| if keep_collinear { r } else { drop_collinear(&r, tol) }).filter(|r| r.len() >= 3).collect();
    if is_simple(&rings) {
        let holes = classify(&rings);
        for (r, h) in rings.iter_mut().zip(holes) {
            orient(r, h);
        }
        return rings;
    }
    if warn {
        it.emit(OutputKind::Warning(MODIFIED.into()));
    }
    resolve(&rings)
}

fn ring_perimeter(r: &Ring) -> f64 {
    (0..r.len()).map(|i| {
        let (p, q) = (r[i], r[(i + 1) % r.len()]);
        (q.0 - p.0).hypot(q.1 - p.1)
    }).sum()
}

/// Area-weighted centroid: solids count positively, holes negatively.
fn centroid(rings: &[Ring]) -> (f64, f64) {
    let flags = hole_flags(rings);
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (r, h) in rings.iter().zip(flags) {
        let n = r.len();
        // Each boundary's own moments, made positive, then signed by its role.
        let (mut ar, mut xr, mut yr) = (0.0, 0.0, 0.0);
        for i in 0..n {
            let (p, q) = (r[i], r[(i + 1) % n]);
            let c = p.0 * q.1 - q.0 * p.1;
            ar += c;
            xr += (p.0 + q.0) * c;
            yr += (p.1 + q.1) * c;
        }
        let s = if h { -1.0 } else { 1.0 } * ar.signum();
        a += s * ar;
        cx += s * xr;
        cy += s * yr;
    }
    if a == 0.0 { (f64::NAN, f64::NAN) } else { (cx / (3.0 * a), cy / (3.0 * a)) }
}

fn area(rings: &[Ring]) -> f64 {
    rings.iter().zip(hole_flags(rings)).map(|(r, h)| if h { -signed_area(r).abs() } else { signed_area(r).abs() }).sum()
}

fn sind(d: f64) -> f64 {
    if d % 180.0 == 0.0 { 0.0 } else { crate::builtins::sind_cosd(d).0 }
}

fn cosd(d: f64) -> f64 {
    if (d - 90.0) % 180.0 == 0.0 { 0.0 } else { crate::builtins::sind_cosd(d).1 }
}

// ---- constructors --------------------------------------------------------------------------

/// Name-value options after the positional arguments.
fn options(a: &[Value]) -> R<Vec<(String, Value)>> {
    if a.len() % 2 != 0 {
        return rt("Name-value arguments must come in pairs.");
    }
    a.chunks(2).map(|c| Ok((c[0].text()?.to_lowercase(), c[1].clone()))).collect()
}

fn nsidedpoly(a: &[Value]) -> Ret {
    let n = a.first().map(|v| v.scalar()).transpose()?.unwrap_or(f64::NAN);
    if !(n >= 3.0) || n.fract() != 0.0 {
        return rt("Number of sides must be a positive integer larger than or equal to 3.");
    }
    let mut center = (0.0, 0.0);
    let mut radius = None;
    let mut side = None;
    for (k, v) in options(&a[1..])? {
        match k.as_str() {
            "center" => {
                let m = v.to_matrix()?;
                if m.numel() != 2 {
                    return rt("Center must be a two-element vector.");
                }
                center = (m.data[0], m.data[1]);
            }
            "radius" => radius = Some(v.scalar()?),
            "sidelength" => side = Some(v.scalar()?),
            other => return rt(format!("'{other}' is not a recognized parameter.")),
        }
    }
    let step = 360.0 / n;
    // Circumradius: given, from the side length, or 1.
    let r = radius.unwrap_or_else(|| side.map_or(1.0, |s| s / (2.0 * sind(180.0 / n))));
    // Clockwise from the vertex just left of straight down, so the bottom side is horizontal.
    let start = 270.0 - 180.0 / n;
    let ring: Ring = (0..n as usize)
        .map(|k| {
            let t = start - step * k as f64;
            (center.0 + r * cosd(t), center.1 + r * sind(t))
        })
        .collect();
    one(make(vec![ring]))
}

/// Boundaries from (x, y) vectors, cell arrays of vectors, or a two-column matrix, starting at
/// argument `i`; returns the boundaries and the index after them.
fn boundaries_arg(a: &[Value], i: usize) -> R<(Vec<Ring>, usize)> {
    let Some(first) = a.get(i) else { return Ok((Vec::new(), i)) };
    if let (Value::Cell(cx), Some(Value::Cell(cy))) = (first, a.get(i + 1)) {
        if cx.data.len() != cy.data.len() {
            return rt("Input x and y cell arrays must have the same number of elements.");
        }
        let mut rings = Vec::new();
        for (x, y) in cx.data.iter().zip(&cy.data) {
            rings.extend(rings_from(&x.to_matrix()?.data, &y.to_matrix()?.data));
        }
        return Ok((rings, i + 2));
    }
    match a.get(i + 1) {
        Some(v) if !v.is_text() => {
            let (x, y) = (first.to_matrix()?.data, v.to_matrix()?.data);
            if x.len() != y.len() {
                return rt("Input x and y vectors must have the same number of elements.");
            }
            Ok((rings_from(&x, &y), i + 2))
        }
        _ => {
            let p = first.to_matrix()?;
            if p.cols() != 2 && !p.is_empty() {
                return rt("Input vertices must be a two-column matrix.");
            }
            let n = p.rows();
            Ok((rings_from(&p.data[..n], &p.data[n..]), i + 1))
        }
    }
}

fn constructor(it: &mut Interpreter, a: &[Value]) -> Ret {
    if a.is_empty() {
        return one(make(Vec::new()));
    }
    let (rings, next) = boundaries_arg(a, 0)?;
    let (mut simplify, mut keep) = (true, false);
    for (k, v) in options(&a[next..])? {
        match k.as_str() {
            "simplify" => simplify = v.scalar()? != 0.0,
            "keepcollinearpoints" => keep = v.scalar()? != 0.0,
            "solidboundaryorientation" => {}
            other => return rt(format!("'{other}' is not a recognized parameter.")),
        }
    }
    if !simplify {
        return one(make(rings));
    }
    one(make(simplify_rings(it, rings, keep, true)))
}

/// `boundaryshape(TR)`: the polyshape bounded by the free boundary of a 2-D triangulation.
fn boundaryshape(it: &mut Interpreter, a: &[Value]) -> Ret {
    let Some(Value::Object(o)) = a.first() else { return rt("Input must be a 2-D triangulation.") };
    let pts = o.get("Points").or_else(|| o.get("X")).unwrap_or_else(Value::empty).to_matrix()?;
    let t = o.get("ConnectivityList").or_else(|| o.get("Triangulation")).unwrap_or_else(Value::empty).to_matrix()?;
    if pts.cols() != 2 || t.cols() != 3 {
        return rt("Input must be a 2-D triangulation.");
    }
    let (np, nt) = (pts.rows(), t.rows());
    let mut count: std::collections::HashMap<(usize, usize), (usize, usize, usize)> = std::collections::HashMap::new();
    for k in 0..nt {
        let v: Vec<usize> = (0..3).map(|c| t.data[c * nt + k] as usize - 1).collect();
        for i in 0..3 {
            let (a, b) = (v[i], v[(i + 1) % 3]);
            count.entry((a.min(b), a.max(b))).or_insert((a, b, 0)).2 += 1;
        }
    }
    // Directed boundary edges, chained into rings.
    let mut next: std::collections::HashMap<usize, usize> = count.values().filter(|e| e.2 == 1).map(|e| (e.0, e.1)).collect();
    let mut rings = Vec::new();
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_unstable();
    for s in starts {
        if !next.contains_key(&s) {
            continue;
        }
        let mut r = Vec::new();
        let mut v = s;
        while let Some(w) = next.remove(&v) {
            r.push((pts.data[v], pts.data[np + v]));
            v = w;
        }
        rings.push(r);
    }
    let _ = it;
    one(make(rings))
}

// ---- buffers -------------------------------------------------------------------------------

fn buffer_style(distance: f64, opts: &[(String, Value)]) -> R<BufferStyle<f64>> {
    let mut join = "round".to_string();
    let mut miter = 3.0;
    for (k, v) in opts {
        match k.as_str() {
            "jointtype" => join = v.text()?.to_lowercase(),
            "miterlimit" => miter = v.scalar()?,
            other => return rt(format!("'{other}' is not a recognized parameter.")),
        }
    }
    let arc = std::f64::consts::PI / 90.0;
    let style = BufferStyle::new(distance).line_cap(LineCap::Round(arc));
    Ok(match join.as_str() {
        "round" => style.line_join(LineJoin::Round(arc)),
        "square" => style.line_join(LineJoin::Bevel).line_cap(LineCap::Square),
        "miter" => style.line_join(LineJoin::Miter((2.0 / miter.max(1.0)).asin() * 2.0)),
        other => return rt(format!("'{other}' is not a valid JointType.")),
    })
}

/// `polybuffer(P, d)`, `polybuffer(xy, 'lines', d)` and `polybuffer(xy, 'points', d)`.
fn polybuffer(a: &[Value]) -> Ret {
    let first = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?;
    if let Value::Object(o) = first {
        let rings = rings_of(o)?;
        let d = crate::builtins::arg(a, 1, "polybuffer")?.scalar()?;
        let style = buffer_style(d, &options(&a[2..])?)?;
        return one(make(from_geo(&to_geo(&rings).buffer_with_style(style))));
    }
    let p = first.to_matrix()?;
    let kind = crate::builtins::arg(a, 1, "polybuffer")?.text()?.to_lowercase();
    let d = crate::builtins::arg(a, 2, "polybuffer")?.scalar()?;
    let style = buffer_style(d, &options(&a[3..])?)?;
    let n = p.rows();
    let pts: Vec<(f64, f64)> = (0..n).map(|i| (p.data[i], p.data[n + i])).collect();
    let out = match kind.as_str() {
        "lines" => {
            // NaN rows separate polylines.
            let mut lines = vec![Vec::new()];
            for &(x, y) in &pts {
                if x.is_nan() || y.is_nan() {
                    lines.push(Vec::new());
                } else {
                    lines.last_mut().unwrap().push(Coord { x, y });
                }
            }
            MultiLineString::new(lines.into_iter().filter(|l| !l.is_empty()).map(LineString::new).collect()).buffer_with_style(style)
        }
        "points" => MultiPoint::new(pts.iter().filter(|p| !p.0.is_nan()).map(|&(x, y)| Point::new(x, y)).collect()).buffer_with_style(style),
        other => return rt(format!("'{other}' must be 'lines' or 'points'.")),
    };
    one(make(from_geo(&out)))
}

// ---- turning function distance ------------------------------------------------------------

/// The turning function of a closed boundary (counterclockwise, perimeter normalized to 1):
/// breakpoints and the edge angle on each piece.
fn turning(r: &Ring) -> (Vec<f64>, Vec<f64>) {
    let mut r = r.clone();
    if signed_area(&r) < 0.0 {
        r[1..].reverse();
    }
    let n = r.len();
    let total = ring_perimeter(&r);
    let (mut s, mut breaks, mut angles) = (0.0, Vec::new(), Vec::new());
    for i in 0..n {
        let (p, q) = (r[i], r[(i + 1) % n]);
        let dir = (q.1 - p.1).atan2(q.0 - p.0);
        let angle = if i == 0 {
            dir
        } else {
            let prev = angles[i - 1];
            let mut turn = dir - prev;
            while turn > std::f64::consts::PI {
                turn -= 2.0 * std::f64::consts::PI;
            }
            while turn <= -std::f64::consts::PI {
                turn += 2.0 * std::f64::consts::PI;
            }
            prev + turn
        };
        breaks.push(s / total);
        angles.push(angle);
        s += (q.0 - p.0).hypot(q.1 - p.1);
    }
    (breaks, angles)
}

fn turning_at(f: &(Vec<f64>, Vec<f64>), x: f64) -> f64 {
    let (x, wrap) = if x >= 1.0 { (x - 1.0, 2.0 * std::f64::consts::PI) } else { (x, 0.0) };
    let k = f.0.iter().rposition(|&b| b <= x).unwrap_or(0);
    f.1[k] + wrap
}

/// Arkin's distance between turning functions, minimized over rotation and starting point.
fn turningdist(p: &Ring, q: &Ring) -> f64 {
    let (f, g) = (turning(p), turning(q));
    let mut best = f64::INFINITY;
    for &a in &f.0 {
        for &b in &g.0 {
            let t = (a - b).rem_euclid(1.0);
            let mut cuts: Vec<f64> = g.0.iter().copied().chain(f.0.iter().map(|&x| (x - t).rem_euclid(1.0))).chain([0.0, 1.0]).collect();
            cuts.sort_by(f64::total_cmp);
            cuts.dedup();
            let (mut s1, mut s2) = (0.0, 0.0);
            for w in cuts.windows(2) {
                let (lo, hi) = (w[0], w[1]);
                if hi <= lo {
                    continue;
                }
                let m = 0.5 * (lo + hi);
                let d = turning_at(&f, m + t) - turning_at(&g, m);
                s1 += d * (hi - lo);
                s2 += d * d * (hi - lo);
            }
            best = best.min((s2 - s1 * s1).max(0.0).sqrt());
        }
    }
    best
}

// ---- methods -----------------------------------------------------------------------------------

fn map_points(rings: &[Ring], f: impl Fn(f64, f64) -> (f64, f64)) -> Value {
    let flags = hole_flags(rings);
    make(
        rings
            .iter()
            .zip(flags)
            .map(|(r, hole)| {
                let mut m: Ring = r.iter().map(|&(x, y)| f(x, y)).collect();
                // A reflection keeps solids clockwise and holes counterclockwise.
                if (signed_area(&m) > 0.0) != (signed_area(r) > 0.0) {
                    orient(&mut m, hole);
                }
                m
            })
            .collect(),
    )
}

/// Two numbers given as (a, b) or ([a b]).
fn pair(a: &[Value], i: usize, what: &str) -> R<(f64, f64)> {
    let m = a.get(i).ok_or_else(|| LangError::new(format!("Not enough input arguments for {what}."), 0))?.to_matrix()?;
    match (m.numel(), a.get(i + 1)) {
        (2, _) => Ok((m.data[0], m.data[1])),
        (1, Some(b)) => Ok((m.data[0], b.scalar()?)),
        (1, None) => Ok((m.data[0], m.data[0])),
        _ => rt(format!("Invalid {what}.")),
    }
}

/// Query points as (x, y) vectors or a two-column matrix.
fn points_arg(a: &[Value]) -> R<Vec<(f64, f64)>> {
    let (xs, ys) = match a.get(1) {
        Some(y) if !y.is_text() => (a[0].to_matrix()?.data, y.to_matrix()?.data),
        _ => {
            let p = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.to_matrix()?;
            let n = p.rows();
            (p.data[..n].to_vec(), p.data[n..].to_vec())
        }
    };
    Ok(xs.into_iter().zip(ys).collect())
}

fn logical_col(v: Vec<bool>) -> Value {
    Value::Num(Matrix::col(v.into_iter().map(|b| if b { 1.0 } else { 0.0 }).collect()).cast_to(Kind::Logical))
}

/// A boundary measure for sorting: area, perimeter, number of sides or centroid distance.
fn measure(rings: &[Ring], criterion: &str, refp: (f64, f64)) -> R<f64> {
    Ok(match criterion {
        "area" => area(rings).abs(),
        "perimeter" => rings.iter().map(ring_perimeter).sum(),
        "numsides" => rings.iter().map(|r| r.len()).sum::<usize>() as f64,
        "centroid" => {
            let (cx, cy) = centroid(rings);
            (cx - refp.0).hypot(cy - refp.1)
        }
        other => return rt(format!("'{other}' is not a valid sorting criterion.")),
    })
}

fn sort_args(a: &[Value]) -> R<(String, bool, (f64, f64))> {
    let criterion = a.first().map(|v| v.text()).transpose()?.unwrap_or_else(|| "area".into()).to_lowercase();
    let mut descend = false;
    let mut refp = (0.0, 0.0);
    let mut i = 1;
    while i < a.len() {
        let t = a[i].text()?.to_lowercase();
        match t.as_str() {
            "ascend" => descend = false,
            "descend" => descend = true,
            "referencepoint" => {
                refp = pair(a, i + 1, "reference point")?;
                i += 1;
            }
            other => return rt(format!("'{other}' is not a valid option.")),
        }
        i += 1;
    }
    Ok((criterion, descend, refp))
}

/// Clips a polyline to the polygon: (inside pieces, outside pieces), NaN-separated.
fn clip_line(rings: &[Ring], line: &[(f64, f64)]) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let mut pieces: Vec<((f64, f64), (f64, f64), bool)> = Vec::new();
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut ts = vec![0.0, 1.0];
        for r in rings {
            let n = r.len();
            for i in 0..n {
                let (c, d) = (r[i], r[(i + 1) % n]);
                let den = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
                if den == 0.0 {
                    continue;
                }
                let t = ((c.0 - a.0) * (d.1 - c.1) - (c.1 - a.1) * (d.0 - c.0)) / den;
                let u = ((c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)) / den;
                if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        ts.dedup();
        let at = |t: f64| (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        for w in ts.windows(2) {
            if w[1] > w[0] {
                pieces.push((at(w[0]), at(w[1]), inside(rings, at(0.5 * (w[0] + w[1])))));
            }
        }
    }
    let (mut inn, mut out): (Vec<(f64, f64)>, Vec<(f64, f64)>) = (Vec::new(), Vec::new());
    let mut prev: Option<bool> = None;
    for (p, q, ins) in pieces {
        let target = if ins { &mut inn } else { &mut out };
        if prev == Some(ins) && target.last() == Some(&p) {
            target.push(q);
        } else {
            if !target.is_empty() {
                target.push((f64::NAN, f64::NAN));
            }
            target.push(p);
            target.push(q);
        }
        prev = Some(ins);
    }
    (inn, out)
}

fn xy_matrix(pts: &[(f64, f64)]) -> Value {
    let n = pts.len();
    let mut data: Vec<f64> = pts.iter().map(|p| p.0).collect();
    data.extend(pts.iter().map(|p| p.1));
    Value::Num(Matrix::new(n, 2, data))
}

/// Removes slivers narrower than `tol`: nearby vertices merge, then spikes and collinear
/// vertices go.
fn rmslivers(rings: &[Ring], tol: f64) -> Vec<Ring> {
    let flags = hole_flags(rings);
    rings
        .iter()
        .zip(flags)
        .map(|(r, hole)| {
            let mut out: Ring = Vec::new();
            for &p in r {
                let snapped = out.iter().copied().find(|q| (q.0 - p.0).hypot(q.1 - p.1) <= tol).unwrap_or(p);
                if out.last() != Some(&snapped) {
                    out.push(snapped);
                }
            }
            if out.len() > 1 && out.first() == out.last() {
                out.pop();
            }
            // Spikes (a, b, a) and collinear vertices.
            loop {
                let n = out.len();
                if n < 3 {
                    break;
                }
                let k = (0..n).find(|&i| {
                    let (p, q, s) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
                    p == s || cross(p, q, s).abs() <= tol * (s.0 - p.0).hypot(s.1 - p.1).max(f64::MIN_POSITIVE)
                });
                match k {
                    Some(i) => {
                        out.remove(i);
                        let n = out.len();
                        if n > 1 {
                            let j = i.min(n - 1);
                            let prev = (j + n - 1) % n;
                            if out[prev] == out[j] {
                                out.remove(j);
                            }
                        }
                    }
                    None => break,
                }
            }
            orient(&mut out, hole);
            start_low_left(&mut out);
            out
        })
        .filter(|r| r.len() >= 3)
        .collect()
}

fn method(it: &mut Interpreter, name: &str, o: &Object, a: &[Value], nargout: usize) -> R<Vec<Value>> {
    let rings = rings_of(o)?;
    let index_arg = |v: Option<&Value>| -> R<Vec<usize>> {
        match v {
            Some(v) => v.to_matrix()?.data.iter().map(|&k| {
                if k < 1.0 || k as usize > rings.len() || k.fract() != 0.0 {
                    return rt("Boundary index exceeds the number of boundaries.");
                }
                Ok(k as usize - 1)
            }).collect(),
            None => Ok((0..rings.len()).collect()),
        }
    };
    Ok(match name {
        "area" => vec![Value::num(area(&rings))],
        "perimeter" => vec![Value::num(rings.iter().map(ring_perimeter).sum())],
        "centroid" => {
            let (cx, cy) = centroid(&rings);
            if nargout <= 1 { vec![Value::Num(Matrix::row(vec![cx, cy]))] } else { vec![Value::num(cx), Value::num(cy)] }
        }
        "numsides" => vec![Value::num(rings.iter().map(|r| r.len()).sum::<usize>() as f64)],
        "numboundaries" => vec![Value::num(rings.len() as f64)],
        "ishole" => vec![logical_col(hole_flags(&rings))],
        "translate" => {
            let (dx, dy) = pair(a, 0, "translation")?;
            vec![map_points(&rings, |x, y| (x + dx, y + dy))]
        }
        "scale" => {
            let (sx, sy) = pair(a, 0, "scale factor")?;
            let (cx, cy) = if a.len() > 1 && a[0].numel() == 2 || a.len() > 2 { pair(a, if a[0].numel() == 2 { 1 } else { 2 }, "reference point")? } else { (0.0, 0.0) };
            vec![map_points(&rings, |x, y| (cx + (x - cx) * sx, cy + (y - cy) * sy))]
        }
        "rotate" => {
            let theta = a.first().map(|v| v.scalar()).transpose()?.unwrap_or(0.0);
            let (cx, cy) = if a.len() > 1 { pair(a, 1, "reference point")? } else { (0.0, 0.0) };
            let (c, s) = (cosd(theta), sind(theta));
            vec![map_points(&rings, |x, y| {
                let (u, v) = (x - cx, y - cy);
                (cx + c * u - s * v, cy + s * u + c * v)
            })]
        }
        "isinterior" => {
            let pts = points_arg(a)?;
            // [in, on]: inside or on the boundary, and on the boundary.
            let on: Vec<bool> = pts.iter().map(|&p| on_boundary(&rings, p)).collect();
            vec![logical_col(pts.iter().map(|&p| inside(&rings, p)).collect()), logical_col(on)]
        }
        "boundingbox" => {
            let pts: Vec<&(f64, f64)> = rings.iter().flatten().collect();
            let lim = |f: fn(&(f64, f64)) -> f64| {
                let v: Vec<f64> = pts.iter().map(|p| f(p)).collect();
                if v.is_empty() { Matrix::new(0, 0, Vec::new()) } else { Matrix::row(vec![v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)]) }
            };
            vec![Value::Num(lim(|p| p.0)), Value::Num(lim(|p| p.1))]
        }
        "regions" => {
            let parts: Vec<Value> = regions_of(&rings).into_iter().map(|(o, hs)| make(std::iter::once(o).chain(hs).collect())).collect();
            vec![if parts.len() == 1 { parts.into_iter().next().unwrap() } else { Value::Cell(CellArray::col(parts)) }]
        }
        "holes" => {
            let parts: Vec<Value> = rings.iter().zip(hole_flags(&rings)).filter(|(_, h)| *h).map(|(r, _)| make(vec![r.clone()])).collect();
            vec![match parts.len() {
                0 => make(Vec::new()),
                1 => parts.into_iter().next().unwrap(),
                _ => Value::Cell(CellArray::col(parts)),
            }]
        }
        "rmholes" => vec![make(rings.iter().zip(hole_flags(&rings)).filter(|(_, h)| !*h).map(|(r, _)| r.clone()).collect())],
        "addboundary" => {
            let (new, next) = boundaries_arg(a, 0)?;
            let opts = options(&a[next..])?;
            let simplify = opts.iter().find(|(k, _)| k == "simplify").map(|(_, v)| v.scalar()).transpose()?.unwrap_or(1.0) != 0.0;
            let all: Vec<Ring> = rings.iter().cloned().chain(new).collect();
            vec![if simplify { make(simplify_rings(it, all, false, true)) } else { make(all) }]
        }
        "rmboundary" => {
            let drop = index_arg(a.first())?;
            vec![make(rings.iter().enumerate().filter(|(k, _)| !drop.contains(k)).map(|(_, r)| r.clone()).collect())]
        }
        "issimplified" => {
            let holes = classify(&rings);
            let ok = is_simple(&rings) && rings.iter().zip(&holes).all(|(r, &h)| (signed_area(r) > 0.0) == h);
            vec![Value::bool(ok)]
        }
        "simplify" => vec![make(simplify_rings(it, rings, false, false))],
        "union" | "intersect" | "subtract" | "xor" => {
            let other = crate::builtins::arg(a, 0, name)?;
            if name == "intersect" && !matches!(other, Value::Object(_)) {
                let p = other.to_matrix()?;
                let n = p.rows();
                let line: Vec<(f64, f64)> = (0..n).map(|i| (p.data[i], p.data[n + i])).collect();
                let (inn, out) = clip_line(&rings, &line);
                return Ok(vec![xy_matrix(&inn), xy_matrix(&out)]);
            }
            let q = poly_of(other)?;
            let op = match name {
                "union" => OpType::Union,
                "intersect" => OpType::Intersection,
                "subtract" => OpType::Difference,
                _ => OpType::Xor,
            };
            vec![make(from_geo(&to_geo(&rings).boolean_op(&to_geo(&q), op)))]
        }
        "overlaps" => {
            let q = poly_of(crate::builtins::arg(a, 0, name)?)?;
            let shared = area(&from_geo(&to_geo(&rings).intersection(&to_geo(&q)))) > 0.0;
            let touch = rings.iter().flatten().any(|&p| inside(&q, p)) || q.iter().flatten().any(|&p| inside(&rings, p));
            vec![Value::bool(shared || touch)]
        }
        "convhull" => {
            let mut pts: Vec<(f64, f64)> = rings.iter().flatten().copied().collect();
            pts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            pts.dedup();
            // Andrew's monotone chain, counterclockwise.
            let mut hull: Vec<(f64, f64)> = Vec::new();
            for pass in 0..2 {
                let start = hull.len();
                let iter: Box<dyn Iterator<Item = &(f64, f64)>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
                for &p in iter {
                    while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                        hull.pop();
                    }
                    hull.push(p);
                }
                hull.pop();
            }
            orient(&mut hull, false);
            start_low_left(&mut hull);
            vec![make(vec![hull])]
        }
        "nearestvertex" => {
            let q = points_arg(a)?;
            let (mut vid, mut bid, mut idx) = (Vec::new(), Vec::new(), Vec::new());
            for p in q {
                let mut best = (f64::INFINITY, 0usize, 0usize, 0usize);
                let mut row = 0;
                for (k, r) in rings.iter().enumerate() {
                    for (i, v) in r.iter().enumerate() {
                        let d = (v.0 - p.0).hypot(v.1 - p.1);
                        if d < best.0 {
                            best = (d, row + i + 1, k + 1, i + 1);
                        }
                    }
                    row += r.len() + 1;
                }
                vid.push(best.1 as f64);
                bid.push(best.2 as f64);
                idx.push(best.3 as f64);
            }
            vec![Value::Num(Matrix::col(vid)), Value::Num(Matrix::col(bid)), Value::Num(Matrix::col(idx))]
        }
        "polybuffer" => {
            let mut args = vec![Value::Object(o.clone())];
            args.extend(a.iter().cloned());
            polybuffer(&args)?.unwrap_or_default()
        }
        "sortboundaries" => {
            let (criterion, descend, refp) = sort_args(a)?;
            let mut keyed: Vec<(f64, Ring)> = rings.iter().map(|r| Ok((measure(std::slice::from_ref(r), &criterion, refp)?, r.clone()))).collect::<R<_>>()?;
            keyed.sort_by(|x, y| if descend { y.0.total_cmp(&x.0) } else { x.0.total_cmp(&y.0) });
            vec![make(keyed.into_iter().map(|(_, r)| r).collect())]
        }
        "sortregions" => {
            let (criterion, descend, refp) = sort_args(a)?;
            let mut keyed: Vec<(f64, Vec<Ring>)> = regions_of(&rings)
                .into_iter()
                .map(|(o, hs)| {
                    let all: Vec<Ring> = std::iter::once(o).chain(hs).collect();
                    Ok((measure(&all, &criterion, refp)?, all))
                })
                .collect::<R<_>>()?;
            keyed.sort_by(|x, y| if descend { y.0.total_cmp(&x.0) } else { x.0.total_cmp(&y.0) });
            vec![make(keyed.into_iter().flat_map(|(_, r)| r).collect())]
        }
        "boundary" => {
            let ids = index_arg(a.first())?;
            let mut pts = Vec::new();
            for (n, &k) in ids.iter().enumerate() {
                if n > 0 {
                    pts.push((f64::NAN, f64::NAN));
                }
                pts.extend(rings[k].iter().chain(rings[k].first()).copied());
            }
            let x = Matrix::col(pts.iter().map(|p| p.0).collect());
            let y = Matrix::col(pts.iter().map(|p| p.1).collect());
            vec![Value::Num(x), Value::Num(y)]
        }
        "rmslivers" => {
            let tol = crate::builtins::arg(a, 0, name)?.scalar()?;
            vec![make(rmslivers(&rings, tol))]
        }
        "turningdist" => {
            let q = poly_of(crate::builtins::arg(a, 0, name)?)?;
            if rings.len() != 1 || q.len() != 1 {
                return rt("turningdist requires polyshapes with one boundary each.");
            }
            vec![Value::num(turningdist(&rings[0], &q[0]))]
        }
        "plot" => {
            // Each boundary as a closed outline.
            let (mut x, mut y) = (Vec::new(), Vec::new());
            for (k, r) in rings.iter().enumerate() {
                if k > 0 {
                    x.push(f64::NAN);
                    y.push(f64::NAN);
                }
                for &(px, py) in r.iter().chain(r.first()) {
                    x.push(px);
                    y.push(py);
                }
            }
            let mut args = vec![Value::Num(Matrix::col(x)), Value::Num(Matrix::col(y))];
            args.extend(a.iter().cloned());
            it.call_function("plot", args, 0)?;
            Vec::new()
        }
        _ => return rt(format!("Undefined function '{name}' for input arguments of type 'polyshape'.")),
    })
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> Ret {
    match name {
        "nsidedpoly" => nsidedpoly(a),
        "polyshape" => constructor(it, a),
        "polybuffer" => polybuffer(a),
        "boundaryshape" => boundaryshape(it, a),
        "matlivre_polyshape_method" => {
            let which = crate::builtins::arg(a, 0, name)?.text()?;
            let nargout = crate::builtins::arg(a, 1, name)?.scalar()? as usize;
            let Some(Value::Object(o)) = a.get(2) else { return rt("Invalid polyshape.") };
            let out = method(it, &which, o, &a[3..], nargout)?;
            one(Value::Cell(CellArray::row(out)))
        }
        _ => Ok(None),
    }
}
