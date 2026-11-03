//! colorcube, imresize, rgb2ind, cmunique, cmpermute, cmap2gray, getrangefromclass, diffuse and
//! specular against MATLAB R2023b.

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

#[test]
fn colorcube_matches_matlab() {
    assert_eq!(
        shown("c = colorcube(10)\nc2 = size(colorcube)\nc4 = colorcube(4)\nc27 = colorcube(27); top = c27(1:5,:)\nc256 = colorcube(256); rows = c256([1 100 196 241 256],:)\n"),
        [
            "c = 10×3\n\n    1.0000    1.0000         0\n         0    1.0000    1.0000\n    1.0000         0    1.0000\n    1.0000         0         0\n         0    1.0000         0\n         0         0    1.0000\n         0         0         0\n    0.3333    0.3333    0.3333\n    0.6667    0.6667    0.6667\n    1.0000    1.0000    1.0000",
            "c2 = 1×2\n\n   256     3",
            "c4 = 4×3\n\n         0         0         0\n    0.3333    0.3333    0.3333\n    0.6667    0.6667    0.6667\n    1.0000    1.0000    1.0000",
            "top = 5×3\n\n    0.5000    0.5000         0\n    0.5000    1.0000         0\n    1.0000    0.5000         0\n    1.0000    1.0000         0\n         0    0.5000    1.0000",
            "rows = 5×3\n\n    0.2000    0.2000         0\n    0.2000    0.2000    0.6000\n    0.0667         0         0\n         0         0         0\n    1.0000    1.0000    1.0000",
        ]
    );
}

#[test]
fn imresize_matches_matlab() {
    assert_eq!(
        shown("I = imresize([1 2; 3 4], 2, 'nearest')\nI2 = imresize([1 2; 3 4], [3 3], 'bilinear')\nI3 = imresize(magic(4), 0.5)\nI4 = imresize([1 2; 3 4], 2)\n"),
        [
            "I = 4×4\n\n     1     1     2     2\n     1     1     2     2\n     3     3     4     4\n     3     3     4     4",
            "I2 = 3×3\n\n    1.0000    1.5000    2.0000\n    2.0000    2.5000    3.0000\n    3.0000    3.5000    4.0000",
            "I3 = 2×2\n\n    8.5000    8.5000\n    8.5000    8.5000",
            "I4 = 4×4\n\n    0.7188    1.0156    1.6094    1.9062\n    1.3125    1.6094    2.2031    2.5000\n    2.5000    2.7969    3.3906    3.6875\n    3.0938    3.3906    3.9844    4.2812",
        ]
    );
}

#[test]
fn indexed_color_helpers_match_matlab() {
    assert_eq!(
        shown(concat!(
            "[X, map] = cmunique([0.2 0.5; 0.2 0.9])\n[X2, map2] = cmunique([1 3; 3 4], [1 0 0; 0 1 0; 1 0 0; 0 0 1])\n",
            "[X3, m3] = cmunique(cat(3, [1 0; 1 0.5], [0 0; 0 0.5], [0 1; 0 0.5]))\n",
            "[Y, mapY] = cmpermute([1 2; 2 3], [1 0 0; 0 1 0; 0 0 1], [3 1 2])\n",
            "rgb = cat(3, [1 0; 0 1], [0 1; 0 0], [0 0; 1 1]);\nZ = rgb2ind(rgb, [1 0 0; 0 1 0; 0 0 1; 1 1 1])\n",
            "r = repmat(linspace(0,1,6), 3, 1);\nrgb = cat(3, r, repmat(linspace(0,1,3)', 1, 6), 0.5*ones(3,6));\n",
            "Z2 = rgb2ind(rgb, [0 0 0; 1 0 0; 0 1 0; 1 1 0; 0.5 0.5 0.5; 1 1 1], 'nodither')\n",
            "g = cmap2gray([1 0 0; 0 1 0; 0 0 1])\nr = getrangefromclass(uint8(1))\nr3 = getrangefromclass(int16(1))\nr4 = getrangefromclass(true)\n",
        )),
        [
            "X = 2×2 uint8 matrix\n\n   0   1\n   0   2",
            "map = 3×3\n\n    0.2002    0.2002    0.2002\n    0.5000    0.5000    0.5000\n    0.9004    0.9004    0.9004",
            "X2 = 2×2 uint8 matrix\n\n   0   0\n   0   1",
            "map2 = 2×3\n\n     1     0     0\n     0     0     1",
            "X3 = 2×2 uint8 matrix\n\n   0   2\n   0   1",
            "m3 = 3×3\n\n    1.0000         0         0\n    0.5000    0.5000    0.5000\n         0         0    1.0000",
            "Y = 2×2\n\n     2     3\n     3     1",
            "mapY = 3×3\n\n     0     0     1\n     1     0     0\n     0     1     0",
            "Z = 2×2 uint8 matrix\n\n   0   1\n   2   3",
            "Z2 = 3×6 uint8 matrix\n\n   0   0   4   4   1   1\n   4   4   4   4   4   4\n   2   2   4   4   5   5",
            "g = 3×3\n\n    0.2989    0.2989    0.2989\n    0.5870    0.5870    0.5870\n    0.1140    0.1140    0.1140",
            "r = 1×2\n\n     0   255",
            "r3 = 1×2\n\n      -32768       32767",
            "r4 = 1×2\n\n     0     1",
        ]
    );
}

#[test]
fn lighting_reflectance_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "d = diffuse([0 1 -1], [0 0 0], [1 1 0], [60 10])\nd2 = diffuse(1, 0, 0, [45 30])\n",
            "s = specular([0 1], [0 0], [1 1], [1 0 1], [0 0 1])\na = [0 15 30]*pi/180;\ns2 = specular(sin(a), 0*a, cos(a), [0 0 1], [0 0 1], 20)\n",
        )),
        [
            "d = 1×3\n\n    0.1736    0.4710         0",
            "d2 = 0.6124",
            "s = 1×2\n\n    0.0312    0.0312",
            "s2 = 1×3\n\n    1.0000    0.0563    0.0000",
        ]
    );
}
