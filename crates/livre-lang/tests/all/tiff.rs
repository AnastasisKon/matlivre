//! The Tiff class (tags, strips, tiles, directories, compression), against MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    // One folder per call (tests run in parallel); files are named by absolute path.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("matlivre-tiff-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let full = std::path::absolute(&dir).unwrap().display().to_string();
    let mut src = src.to_string();
    for f in ["rgb.tif", "gray.tif", "f.tif"] {
        src = src.replace(&format!("'{f}'"), &format!("'{}'", std::path::Path::new(&full).join(f).display()));
    }
    let mut it = Interpreter::new();
    let _ = it.run_source(&src);
    let out: Vec<String> = it
        .take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .map(|s| s.replace(&full, "<dir>").replace("<dir>\\", "<dir>/"))
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn check(src: &str, expected: &[&str]) {
    assert_eq!(shown(src), expected, "{src}");
}

#[test]
fn write_read_strips_tiles_and_directories() {
    check(
        concat!(
            "img = uint8(reshape(0:47, 4, 4, 3)*5);\n",
            "t = Tiff('rgb.tif', 'w');\n",
            "tags.ImageLength = 4;\n",
            "tags.ImageWidth = 4;\n",
            "tags.Photometric = Tiff.Photometric.RGB;\n",
            "tags.BitsPerSample = 8;\n",
            "tags.SamplesPerPixel = 3;\n",
            "tags.PlanarConfiguration = Tiff.PlanarConfiguration.Chunky;\n",
            "tags.RowsPerStrip = 2;\n",
            "tags.Compression = Tiff.Compression.None;\n",
            "tags.Software = 'MATLAB';\n",
            "setTag(t, tags);\n",
            "write(t, img);\n",
            "close(t);\n",
            "t = Tiff('rgb.tif', 'r');\n",
            "w = getTag(t, 'ImageWidth')\n",
            "h = getTag(t, Tiff.TagID.ImageLength)\n",
            "ph = getTag(t, 'Photometric')\n",
            "ns = numberOfStrips(t)\n",
            "s1 = computeStrip(t, 3)\n",
            "st = readEncodedStrip(t, 2)\n",
            "tl = isTiled(t)\n",
            "cd = currentDirectory(t)\n",
            "ld = lastDirectory(t)\n",
            "r = read(t);\n",
            "same = isequal(r, img)\n",
            "sw = getTag(t, 'Software')\n",
            "rgba = readRGBAImage(t);\n",
            "size(rgba)\n",
            "class(rgba)\n",
            "close(t);\n",
            "g = uint16(magic(6));\n",
            "t = Tiff('gray.tif', 'w');\n",
            "setTag(t, 'ImageLength', 6); setTag(t, 'ImageWidth', 6);\n",
            "setTag(t, 'Photometric', Tiff.Photometric.MinIsBlack);\n",
            "setTag(t, 'BitsPerSample', 16); setTag(t, 'SamplesPerPixel', 1);\n",
            "setTag(t, 'PlanarConfiguration', Tiff.PlanarConfiguration.Chunky);\n",
            "setTag(t, 'Compression', Tiff.Compression.LZW);\n",
            "setTag(t, 'TileWidth', 16); setTag(t, 'TileLength', 16);\n",
            "write(t, g);\n",
            "writeDirectory(t);\n",
            "setTag(t, 'ImageLength', 2); setTag(t, 'ImageWidth', 3);\n",
            "setTag(t, 'Photometric', Tiff.Photometric.MinIsBlack);\n",
            "setTag(t, 'BitsPerSample', 8); setTag(t, 'SamplesPerPixel', 1);\n",
            "setTag(t, 'PlanarConfiguration', Tiff.PlanarConfiguration.Chunky);\n",
            "setTag(t, 'Compression', Tiff.Compression.PackBits);\n",
            "write(t, uint8([1 2 3; 4 5 6]));\n",
            "close(t);\n",
            "t = Tiff('gray.tif');\n",
            "tl2 = isTiled(t)\n",
            "nt = numberOfTiles(t)\n",
            "ct = computeTile(t, [1 1])\n",
            "g2 = read(t);\n",
            "same2 = isequal(g2, g)\n",
            "c1 = getTag(t, 'Compression')\n",
            "ld2 = lastDirectory(t)\n",
            "nextDirectory(t);\n",
            "cd2 = currentDirectory(t)\n",
            "d2 = read(t)\n",
            "c2 = getTag(t, 'Compression')\n",
            "setDirectory(t, 1);\n",
            "cd3 = currentDirectory(t)\n",
            "close(t);\n",
            "Tiff.Compression\n",
            "Tiff.Photometric\n",
            "Tiff.PlanarConfiguration\n",
            "Tiff.SampleFormat\n",
            "n = Tiff.TagID.ImageWidth\n",
            "v = Tiff.getVersion\n",
            "names = Tiff.getTagNames;\n",
            "size(names)\n",
        ),
        &[
            "w = 4",
            "h = 4",
            "ph = 2",
            "ns = 2",
            "s1 = 2",
            "st = 2×4×3 uint8 array\n\nst(:,:,1) =\n\n   10   30   50   70\n   15   35   55   75\n\nst(:,:,2) =\n\n    90   110   130   150\n    95   115   135   155\n\nst(:,:,3) =\n\n   170   190   210   230\n   175   195   215   235",
            "tl = logical\n   0",
            "cd = 1",
            "ld = logical\n   1",
            "same = logical\n   1",
            "sw = 'MATLAB'",
            "ans = 1×3\n\n     4     4     3",
            "ans = 'uint8'",
            "tl2 = logical\n   1",
            "nt = 1",
            "ct = 1",
            "same2 = logical\n   1",
            "c1 = 5",
            "ld2 = logical\n   0",
            "cd2 = 2",
            "d2 = 2×3 uint8 matrix\n\n   1   2   3\n   4   5   6",
            "c2 = 32773",
            "cd3 = 1",
            "ans = struct with fields:\n\n                  None: 1\n              CCITTRLE: 2\n             CCITTFax3: 3\n             CCITTFax4: 4\n                   LZW: 5\n                 OJPEG: 6\n                  JPEG: 7\n          AdobeDeflate: 8\n                  Next: 32766\n             CCITTRLEW: 32771\n              PackBits: 32773\n           Thunderscan: 32809\n              IT8CTPad: 32895\n                 IT8LW: 32896\n                 IT8MP: 32897\n                 IT8BL: 32898\n             PixarFilm: 32908\n              PixarLog: 32909\n               Deflate: 32946\n                   DCS: 32947\n                  JBIG: 34661\n                SGILog: 34676\n              SGILog24: 34677\n              JPEG2000: 34712\n      JPEG2000_SVS_RGB: 33005\n    JPEG2000_SVS_YCbCr: 33003",
            "ans = struct with fields:\n\n    MinIsWhite: 0\n    MinIsBlack: 1\n           RGB: 2\n       Palette: 3\n          Mask: 4\n     Separated: 5\n         YCbCr: 6\n        CIELab: 8\n        ICCLab: 9\n        ITULab: 10\n           CFA: 32803\n          LogL: 32844\n        LogLuv: 32845\n     LinearRaw: 34892",
            "ans = struct with fields:\n\n      Chunky: 1\n    Separate: 2",
            "ans = struct with fields:\n\n             UInt: 1\n              Int: 2\n           IEEEFP: 3\n             Void: 4\n       ComplexInt: 5\n    ComplexIEEEFP: 6",
            "n = 256",
            "v = \n    'LIBTIFF, Version 4.5.0\n     Copyright (c) 1988-1996 Sam Leffler\n     Copyright (c) 1991-1996 Silicon Graphics, Inc.'",
            "ans = 1×2\n\n    74     1",
        ],
    );
}

#[test]
fn display_tags_errors_and_rewrite() {
    check(
        concat!(
            "img = uint8(reshape(0:47, 4, 4, 3)*5);\n",
            "t = Tiff('rgb.tif', 'w');\n",
            "tags.ImageLength = 4;\n",
            "tags.ImageWidth = 4;\n",
            "tags.Photometric = Tiff.Photometric.RGB;\n",
            "tags.BitsPerSample = 8;\n",
            "tags.SamplesPerPixel = 3;\n",
            "tags.PlanarConfiguration = Tiff.PlanarConfiguration.Chunky;\n",
            "tags.RowsPerStrip = 2;\n",
            "tags.Compression = Tiff.Compression.None;\n",
            "tags.Software = 'MATLAB';\n",
            "setTag(t, tags);\n",
            "write(t, img);\n",
            "close(t);\n",
            "t = Tiff('rgb.tif')\n",
            "try, getTag(t, 'Artist'), catch e, disp(e.message), end\n",
            "try, getTag(t, 'Bogus'), catch e, disp(e.message), end\n",
            "x = getTag(t, 'StripOffsets')\n",
            "bc = getTag(t, 'StripByteCounts')\n",
            "rs = getTag(t, 'RowsPerStrip')\n",
            "pc = getTag(t, 'PlanarConfiguration')\n",
            "bps = getTag(t, 'BitsPerSample')\n",
            "class(bps)\n",
            "close(t)\n",
            "try, Tiff('nofile.tif'), catch e, disp(e.message), end\n",
            "t = Tiff('f.tif', 'w');\n",
            "setTag(t, 'ImageLength', 2); setTag(t, 'ImageWidth', 2);\n",
            "setTag(t, 'Photometric', Tiff.Photometric.MinIsBlack);\n",
            "setTag(t, 'BitsPerSample', 32); setTag(t, 'SamplesPerPixel', 1);\n",
            "setTag(t, 'SampleFormat', Tiff.SampleFormat.IEEEFP);\n",
            "setTag(t, 'PlanarConfiguration', Tiff.PlanarConfiguration.Chunky);\n",
            "setTag(t, 'Compression', Tiff.Compression.AdobeDeflate);\n",
            "setTag(t, 'XResolution', 72); setTag(t, 'YResolution', 72); setTag(t, 'ResolutionUnit', Tiff.ResolutionUnit.Inch);\n",
            "write(t, single([1.5 2; -3 4]));\n",
            "close(t);\n",
            "t = Tiff('f.tif');\n",
            "f = read(t)\n",
            "xr = getTag(t, 'XResolution')\n",
            "sf = getTag(t, 'SampleFormat')\n",
            "close(t)\n",
            "t = Tiff('rgb.tif', 'r+');\n",
            "setTag(t, 'Artist', 'Me');\n",
            "rewriteDirectory(t);\n",
            "close(t);\n",
            "t = Tiff('rgb.tif');\n",
            "a = getTag(t, 'Artist')\n",
            "close(t);\n",
        ),
        &[
            "t = \n\n                  TIFF File: '<dir>/rgb.tif'\n                       Mode: 'r'\n    Current Image Directory: 1\n           Number Of Strips: 2\n                SubFileType: Tiff.SubFileType.Default\n                Photometric: Tiff.Photometric.RGB\n                ImageLength: 4\n                 ImageWidth: 4\n               RowsPerStrip: 2\n              BitsPerSample: 8\n                Compression: Tiff.Compression.None\n               SampleFormat: Tiff.SampleFormat.UInt\n            SamplesPerPixel: 3\n        PlanarConfiguration: Tiff.PlanarConfiguration.Chunky\n                Orientation: Tiff.Orientation.TopLeft\n",
            "Unable to retrieve Artist.\n",
            "Unrecognized field name \"Bogus\".\n",
            "x = 1×2 uint64 row vector\n\n    8   32",
            "bc = 1×2 uint64 row vector\n\n   24   24",
            "rs = 2",
            "pc = 1",
            "bps = 8",
            "ans = 'double'",
            "Could not open file 'nofile.tif'.\n",
            "f = 2×2 single matrix\n\n    1.5000    2.0000\n   -3.0000    4.0000",
            "xr = 72",
            "sf = 3",
            "a = 'Me'",
        ],
    );
}
