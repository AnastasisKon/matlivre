//! Volume and stream visualization against MATLAB R2023b. Isosurfaces use marching tetrahedra,
//! so their triangle counts differ from MATLAB's; the tests check that the vertices lie on the
//! surface instead.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
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

const GRID: &str = "[x,y,z] = meshgrid(-2:2, -2:2, -2:2); V = x.^2 + y.^2 + z.^2;\n";

#[test]
fn isosurfaces_match_matlab() {
    let out = shown(&format!(
        concat!(
            "{}[f, v] = isosurface(x, y, z, V, 2); on = max(abs(sqrt(sum(v.^2, 2)) - sqrt(2))) < 0.5\nw = size(f, 2)\n",
            "s = isosurface(V, 3); fn = fieldnames(s)'\nN = isonormals(x, y, z, V, [1 0 0; 0 1.5 0])\n",
            "c = isocolors(x, y, z, V, [1 0 0; 0.5 0.5 0.5])\n",
            "cp = isocaps(x, y, z, V, 7); fc = fieldnames(cp)'\nabove = all(cp.facevertexcdata >= 7 - 1e-9)\n",
        ),
        GRID
    ));
    assert_eq!(
        out,
        [
            "on = logical\n   1",
            "w = 3",
            "fn = 1×2 cell\n\n    {'vertices'}    {'faces'}",
            "N = 2×3\n\n   -2.0000         0         0\n         0   -2.5000         0",
            "c = 2×1\n\n    1.0000\n    1.5000",
            "fc = 1×3 cell\n\n    {'vertices'}    {'faces'}    {'facevertexcdata'}",
            "above = logical\n   1",
        ]
    );
}

#[test]
fn volume_utilities_match_matlab() {
    assert_eq!(
        shown(&format!(
            concat!(
                "{}[xs, ys, zs, vs] = subvolume(x, y, z, V, [-1 1 nan nan 0 2]); s = size(vs)\nxr = squeeze(xs(1,:,1))\n",
                "[nx, ny, nz, nv] = reducevolume(x, y, z, V, 2); sr = size(nv)\nb = volumebounds(x, y, z, V)\n",
                "W = smooth3(V); w1 = W(3,3,3)\nW2 = smooth3(V, 'gaussian', 3); w2 = W2(3,3,3)\nW3 = smooth3(V, 'box', [3 3 1]); w3 = W3(2,2,2)\n",
            ),
            GRID
        )),
        [
            "s = 1×3\n\n     5     3     3",
            "xr = 1×3\n\n    -1     0     1",
            "sr = 1×3\n\n     3     3     3",
            "b = 1×8\n\n    -2     2    -2     2    -2     2     0    12",
            "w1 = 2.0000",
            "w2 = 1.1395",
            "w3 = 4.3333",
        ]
    );
}

#[test]
fn patch_utilities_match_matlab() {
    assert_eq!(
        shown(concat!(
            "p = surf2patch(peaks(3)); fn = fieldnames(p)'\nsf = size(p.faces)\nsv = size(p.vertices)\n",
            "[F, Vt] = surf2patch([1 2; 3 4], 'triangles')\n",
            "fv = struct('faces', [1 2 3; 1 3 4], 'vertices', [0 0 0; 1 0 0; 1 1 0; 0 1 0]);\ns2 = shrinkfaces(fv, 0.5); v2 = s2.vertices\n",
            "[x,y,z] = meshgrid(-2:2, -2:2, -2:2); V = x.^2 + y.^2 + z.^2; iso = isosurface(x, y, z, V, 2);\n",
            "r = reducepatch(iso, 0.5); fewer = size(r.faces, 1) <= size(iso.faces, 1) * 0.5\n",
            "h = patch(iso); c = class(h)\nh2 = patch('Faces', [1 2 3], 'Vertices', [0 0 0; 1 0 1; 0 1 0], 'FaceColor', 'r'); f2 = get(h2, 'Faces')\n",
        )),
        [
            "fn = 1×2 cell\n\n    {'faces'}    {'vertices'}",
            "sf = 1×2\n\n     4     4",
            "sv = 1×2\n\n     9     3",
            "F = 2×3\n\n     1     3     4\n     1     4     2",
            "Vt = 4×3\n\n     1     1     1\n     1     2     3\n     2     1     2\n     2     2     4",
            "v2 = 6×3\n\n    0.1953    0.0976         0\n    0.9024    0.0976         0\n    0.9024    0.8047         0\n    0.0976    0.1953         0\n    0.8047    0.9024         0\n    0.0976    0.9024         0",
            "fewer = logical\n   1",
            "c = 'matlab.graphics.primitive.Patch'",
            "f2 = 1×3\n\n     1     2     3",
        ]
    );
}

#[test]
fn stream_lines_match_matlab() {
    assert_eq!(
        shown(concat!(
            "format long\n[X, Y] = meshgrid(0:4, 0:4); U = ones(5); Vv = zeros(5);\n",
            "XY = stream2(X, Y, U, Vv, 0, 2); c = class(XY)\nn = size(XY{1})\nlast = XY{1}(end, 1)\n",
            "XY2 = stream2(X, Y, U, Vv, 0, 2, [0.5 10]); n2 = size(XY2{1})\nn3 = size(stream2(X, Y, 2*U, Vv, 0, 2){1})\n",
            "[X, Y] = meshgrid(0:4); XY6 = stream2(X, Y, Y, -X, 1, 2, [0.5 5]); p6 = XY6{1}'\nformat short\n",
            "[X3, Y3, Z3] = meshgrid(0:3); U3 = ones(4,4,4); V3 = zeros(4,4,4);\nXYZ = stream3(X3, Y3, Z3, U3, V3, V3, 0, 1, 1); n4 = size(XYZ{1})\ne4 = XYZ{1}(end,:)\n",
        )),
        [
            "c = 'cell'",
            "n = 1×2\n\n    40     2",
            "last = 3.900000000000002",
            "n2 = 1×2\n\n     9     2",
            "n3 = 1×2\n\n    40     2",
            "p6 = 2×5\n\n   1.000000000000000   1.500000000000000   2.000000000000000   2.330357142857143   2.506602353585112\n   2.000000000000000   1.750000000000000   1.321428571428571   0.821428571428571   0.321428571428571",
            "n4 = 1×2\n\n    30     3",
            "e4 = 1×3\n\n    2.9000    1.0000    1.0000",
        ]
    );
}

#[test]
fn volume_plots_create_matlab_objects() {
    assert_eq!(
        shown(&format!(
            concat!(
                "{}[X3, Y3, Z3] = meshgrid(0:3); U3 = ones(4,4,4); V3 = zeros(4,4,4); XYZ = stream3(X3, Y3, Z3, U3, V3, V3, 0, 1, 1);\n",
                "[X, Y] = meshgrid(0:4); XY = stream2(X, Y, ones(5), zeros(5), 0, 2);\n",
                "h = streamline(XY); c1 = class(h)\nfigure; hs = slice(x, y, z, V, 0, 0, 0); n = numel(hs)\nc2 = class(hs(1))\nsz = size(get(hs(1), 'XData'))\n",
                "figure; cs = contourslice(x, y, z, V, 0, [], [], [2 4]); c3 = class(cs)\nfigure; hc = coneplot(X3, Y3, Z3, U3, V3, V3, 1, 1, 1); c4 = class(hc)\n",
                "figure; ht = streamtube(XYZ); c5 = class(ht)\nfigure; hr = streamribbon(XYZ, {{zeros(30,1)}}); c6 = class(hr)\n",
            ),
            GRID
        )),
        [
            "c1 = 'matlab.graphics.primitive.Line'",
            "n = 3",
            "c2 = 'matlab.graphics.primitive.Surface'",
            "sz = 1×2\n\n     5     5",
            "c3 = 'matlab.graphics.primitive.Patch'",
            "c4 = 'matlab.graphics.primitive.Patch'",
            "c5 = 'matlab.graphics.primitive.Surface'",
            "c6 = 'matlab.graphics.primitive.Surface'",
        ]
    );
}
