//! The `Tiff` class: tag-level reading and writing of TIFF files (strips and tiles, several
//! image directories, no compression, PackBits, LZW and Deflate), written from the TIFF 6.0
//! specification. The class itself is MATLAB source whose methods call `matlivre_tiff`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::rc::Rc;

use crate::class::{ClassDef, Object};
use crate::error::{rt, LangError};
use crate::interp::Interpreter;
use crate::value::*;

type R<T> = Result<T, LangError>;

pub const NAMES: &[&str] = &["matlivre_tiff"];
pub const CLASS: &str = "Tiff";

/// Tag names and numbers, in MATLAB's order.
const TAGS: &[(&str, u32)] = &[
    ("SubFileType", 254), ("ImageWidth", 256), ("ImageLength", 257), ("BitsPerSample", 258), ("Compression", 259), ("Photometric", 262),
    ("Thresholding", 263), ("FillOrder", 266), ("DocumentName", 269), ("ImageDescription", 270), ("Make", 271), ("Model", 272),
    ("StripOffsets", 273), ("Orientation", 274), ("SamplesPerPixel", 277), ("RowsPerStrip", 278), ("StripByteCounts", 279),
    ("MinSampleValue", 280), ("MaxSampleValue", 281), ("XResolution", 282), ("YResolution", 283), ("PlanarConfiguration", 284),
    ("PageName", 285), ("XPosition", 286), ("YPosition", 287), ("Group3Options", 292), ("Group4Options", 293), ("ResolutionUnit", 296),
    ("PageNumber", 297), ("TransferFunction", 301), ("Software", 305), ("DateTime", 306), ("Artist", 315), ("HostComputer", 316),
    ("WhitePoint", 318), ("PrimaryChromaticities", 319), ("ColorMap", 320), ("HalfToneHints", 321), ("TileWidth", 322), ("TileLength", 323),
    ("TileOffsets", 324), ("TileByteCounts", 325), ("SubIFD", 330), ("InkSet", 332), ("InkNames", 333), ("NumberOfInks", 334),
    ("DotRange", 336), ("TargetPrinter", 337), ("ExtraSamples", 338), ("SampleFormat", 339), ("SMinSampleValue", 340),
    ("SMaxSampleValue", 341), ("YCbCrCoefficients", 529), ("YCbCrSubSampling", 530), ("YCbCrPositioning", 531),
    ("ReferenceBlackWhite", 532), ("XMP", 700), ("ImageDepth", 32997), ("Copyright", 33432), ("ModelPixelScaleTag", 33550),
    ("RichTIFFIPTC", 33723), ("RPCCoefficientTag", 50844), ("ModelTiepointTag", 33922), ("ModelTransformationTag", 34264),
    ("Photoshop", 34377), ("ICCProfile", 34675), ("GeoKeyDirectoryTag", 34735), ("GeoDoubleParamsTag", 34736),
    ("GeoASCIIParamsTag", 34737), ("SToNits", 37439), ("JPEGQuality", 65537), ("JPEGColorMode", 65538), ("ZipQuality", 65557),
    ("SGILogDataFmt", 65560),
];

/// The constant structs of the class (Tiff.Compression and so on).
const CONSTS: &[(&str, &[(&str, u32)])] = &[
    ("SubFileType", &[("Default", 0), ("ReducedImage", 1), ("Page", 2), ("Mask", 4)]),
    (
        "Compression",
        &[
            ("None", 1), ("CCITTRLE", 2), ("CCITTFax3", 3), ("CCITTFax4", 4), ("LZW", 5), ("OJPEG", 6), ("JPEG", 7), ("AdobeDeflate", 8),
            ("Next", 32766), ("CCITTRLEW", 32771), ("PackBits", 32773), ("Thunderscan", 32809), ("IT8CTPad", 32895), ("IT8LW", 32896),
            ("IT8MP", 32897), ("IT8BL", 32898), ("PixarFilm", 32908), ("PixarLog", 32909), ("Deflate", 32946), ("DCS", 32947),
            ("JBIG", 34661), ("SGILog", 34676), ("SGILog24", 34677), ("JPEG2000", 34712), ("JPEG2000_SVS_RGB", 33005),
            ("JPEG2000_SVS_YCbCr", 33003),
        ],
    ),
    (
        "Photometric",
        &[
            ("MinIsWhite", 0), ("MinIsBlack", 1), ("RGB", 2), ("Palette", 3), ("Mask", 4), ("Separated", 5), ("YCbCr", 6), ("CIELab", 8),
            ("ICCLab", 9), ("ITULab", 10), ("CFA", 32803), ("LogL", 32844), ("LogLuv", 32845), ("LinearRaw", 34892),
        ],
    ),
    ("Thresholding", &[("BiLevel", 1), ("HalfTone", 2), ("ErrorDiffuse", 3)]),
    (
        "Orientation",
        &[("TopLeft", 1), ("TopRight", 2), ("BottomRight", 3), ("BottomLeft", 4), ("LeftTop", 5), ("RightTop", 6), ("RightBottom", 7), ("LeftBottom", 8)],
    ),
    ("PlanarConfiguration", &[("Chunky", 1), ("Separate", 2)]),
    ("ExtraSamples", &[("Unspecified", 0), ("AssociatedAlpha", 1), ("UnassociatedAlpha", 2)]),
    ("Group3Options", &[("Encoding2D", 1), ("Uncompressed", 2), ("FillBits", 4)]),
    ("ResolutionUnit", &[("None", 1), ("Inch", 2), ("Centimeter", 3)]),
    ("InkSet", &[("CMYK", 1), ("MultiInk", 2)]),
    ("SampleFormat", &[("UInt", 1), ("Int", 2), ("IEEEFP", 3), ("Void", 4), ("ComplexInt", 5), ("ComplexIEEEFP", 6)]),
    ("YCbCrPositioning", &[("Centered", 1), ("Cosited", 2)]),
    ("SGILogDataFmt", &[("Float", 0), ("Bits8", 3)]),
    ("JPEGColorMode", &[("RGB", 1), ("Raw", 0)]),
];

const VERSION: &str = "LIBTIFF, Version 4.5.0\nCopyright (c) 1988-1996 Sam Leffler\nCopyright (c) 1991-1996 Silicon Graphics, Inc.";

/// The methods, by the names the function reference gives them (the static ones last).
const TIFF_METHODS: &[&str] = &[
    "Tiff.close", "Tiff.getTag", "Tiff.setTag", "Tiff.read", "Tiff.write", "Tiff.writeDirectory", "Tiff.rewriteDirectory",
    "Tiff.numberOfStrips", "Tiff.numberOfTiles", "Tiff.isTiled", "Tiff.currentDirectory", "Tiff.lastDirectory", "Tiff.nextDirectory",
    "Tiff.setDirectory", "Tiff.setSubDirectory", "Tiff.computeStrip", "Tiff.computeTile", "Tiff.readEncodedStrip", "Tiff.readEncodedTile",
    "Tiff.writeEncodedStrip", "Tiff.writeEncodedTile", "Tiff.readRGBAImage", "Tiff.readRGBAStrip", "Tiff.readRGBATile", "Tiff.getVersion",
    "Tiff.getTagNames",
];

fn tag_id(name: &str) -> Option<u32> {
    TAGS.iter().find(|(n, _)| *n == name).map(|(_, id)| *id)
}

fn tag_name(id: u32) -> Option<&'static str> {
    TAGS.iter().find(|(_, i)| *i == id).map(|(n, _)| *n)
}

/// The `Tiff` class.
pub fn class_def() -> Rc<ClassDef> {
    thread_local! {
        static DEF: RefCell<Option<Rc<ClassDef>>> = const { RefCell::new(None) };
    }
    DEF.with(|d| {
        d.borrow_mut()
            .get_or_insert_with(|| {
                let mut src = String::from("classdef Tiff < handle\n  properties (Hidden)\n    Id\n  end\n  properties (Constant)\n");
                let tagid: Vec<String> = TAGS.iter().map(|(n, v)| format!("'{n}', {v}")).collect();
                src += &format!("    TagID = struct({})\n", tagid.join(", "));
                for (name, fields) in CONSTS {
                    let f: Vec<String> = fields.iter().map(|(n, v)| format!("'{n}', {v}")).collect();
                    src += &format!("    {name} = struct({})\n", f.join(", "));
                }
                src += "  end\n  methods\n    function obj = Tiff(varargin)\n      obj.Id = matlivre_tiff('open', 1, [], varargin{:});\n    end\n";
                for m in TIFF_METHODS.iter().filter_map(|m| m.strip_prefix("Tiff.")).filter(|m| !matches!(*m, "getVersion" | "getTagNames")) {
                    src += &format!("    function varargout = {m}(obj, varargin)\n      varargout = matlivre_tiff('{m}', nargout, obj, varargin{{:}});\n    end\n");
                }
                src += "  end\n  methods (Static)\n    function v = getVersion()\n      v = matlivre_tiff('getVersion', 1, []);\n    end\n";
                src += "    function v = getTagNames()\n      v = matlivre_tiff('getTagNames', 1, []);\n    end\n  end\nend\n";
                let ast = crate::class::parse_classdef(&src).expect("built-in class source parses");
                let mut def = ClassDef::build(ast, &[]);
                def.name = CLASS.into();
                Rc::new(def)
            })
            .clone()
    })
}

// ---- the file model ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum TagVal {
    Ints(Vec<u64>),
    Reals(Vec<f64>),
    Ascii(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Default)]
struct Dir {
    tags: BTreeMap<u32, TagVal>,
    /// Encoded strips or tiles, in order.
    chunks: Vec<Vec<u8>>,
    /// File offset of the directory when it was read (for setSubDirectory).
    offset: u64,
}

struct TiffFile {
    path: String,
    mode: String,
    dirs: Vec<Dir>,
    cur: usize,
    dirty: bool,
}

thread_local! {
    static FILES: RefCell<HashMap<usize, TiffFile>> = RefCell::new(HashMap::new());
    static NEXT: RefCell<usize> = const { RefCell::new(1) };
}

fn with_file<T>(id: usize, f: impl FnOnce(&mut TiffFile) -> R<T>) -> R<T> {
    FILES.with(|m| match m.borrow_mut().get_mut(&id) {
        Some(t) => f(t),
        None => rt("Invalid TIFF object: the file is closed."),
    })
}

impl Dir {
    fn int(&self, id: u32) -> Option<u64> {
        match self.tags.get(&id) {
            Some(TagVal::Ints(v)) => v.first().copied(),
            Some(TagVal::Reals(v)) => v.first().map(|x| *x as u64),
            _ => None,
        }
    }

    fn get(&self, id: u32, default: u64) -> u64 {
        self.int(id).unwrap_or(default)
    }

    fn width(&self) -> usize {
        self.get(256, 0) as usize
    }
    fn height(&self) -> usize {
        self.get(257, 0) as usize
    }
    fn spp(&self) -> usize {
        self.get(277, 1).max(1) as usize
    }
    fn bps(&self) -> usize {
        self.get(258, 1) as usize
    }
    fn planar_separate(&self) -> bool {
        self.get(284, 1) == 2
    }
    fn tiled(&self) -> bool {
        self.tags.contains_key(&322)
    }
    fn rows_per_strip(&self) -> usize {
        (self.get(278, u32::MAX as u64) as usize).min(self.height().max(1)).max(1)
    }
    fn planes(&self) -> usize {
        if self.planar_separate() { self.spp() } else { 1 }
    }
    /// Samples per pixel inside one chunk.
    fn chunk_spp(&self) -> usize {
        if self.planar_separate() { 1 } else { self.spp() }
    }
    fn tile_dims(&self) -> (usize, usize) {
        (self.get(323, 0).max(1) as usize, self.get(322, 0).max(1) as usize)
    }
    fn tiles_across(&self) -> usize {
        self.width().div_ceil(self.tile_dims().1)
    }
    fn tiles_down(&self) -> usize {
        self.height().div_ceil(self.tile_dims().0)
    }
    fn chunks_per_plane(&self) -> usize {
        if self.tiled() { self.tiles_across() * self.tiles_down() } else { self.height().div_ceil(self.rows_per_strip()).max(1) }
    }
    fn n_chunks(&self) -> usize {
        self.chunks_per_plane() * self.planes()
    }
    /// (rows, cols) of chunk k (strips are as wide as the image; the last strip is shorter).
    fn chunk_shape(&self, k: usize) -> (usize, usize) {
        if self.tiled() {
            return self.tile_dims();
        }
        let r = self.rows_per_strip();
        let s = k % self.chunks_per_plane();
        let rows = r.min(self.height().saturating_sub(s * r));
        (rows, self.width())
    }
    /// Bytes in a decoded chunk of `rows` x `cols`.
    fn chunk_bytes(&self, rows: usize, cols: usize) -> usize {
        rows * (cols * self.chunk_spp() * self.bps()).div_ceil(8)
    }
}

// ---- compression ------------------------------------------------------------------------------

fn packbits_decode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let k = n as usize + 1;
            out.extend_from_slice(&src[i..(i + k).min(src.len())]);
            i += k;
        } else if n != -128 {
            if let Some(&b) = src.get(i) {
                out.extend(std::iter::repeat_n(b, (1 - n as i32) as usize));
            }
            i += 1;
        }
    }
    out
}

fn packbits_encode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        // A run of at least 2 equal bytes.
        let mut run = 1;
        while i + run < src.len() && src[i + run] == src[i] && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((1 - run as i32) as i8 as u8);
            out.push(src[i]);
            i += run;
            continue;
        }
        let start = i;
        while i < src.len() && i - start < 128 && !(i + 1 < src.len() && src[i] == src[i + 1]) {
            i += 1;
        }
        if i == start {
            i += 1;
        }
        out.push((i - start - 1) as u8);
        out.extend_from_slice(&src[start..i]);
    }
    out
}

fn lzw_decode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut table: Vec<Vec<u8>> = (0..256).map(|b| vec![b as u8]).collect();
    table.push(Vec::new());
    table.push(Vec::new());
    let (mut bitpos, mut width) = (0usize, 9usize);
    let mut prev: Option<Vec<u8>> = None;
    let read = |pos: usize, w: usize| -> Option<usize> {
        if pos + w > src.len() * 8 {
            return None;
        }
        let mut v = 0usize;
        for k in 0..w {
            let bit = (src[(pos + k) / 8] >> (7 - (pos + k) % 8)) & 1;
            v = (v << 1) | bit as usize;
        }
        Some(v)
    };
    while let Some(code) = read(bitpos, width) {
        bitpos += width;
        if code == 256 {
            table.truncate(258);
            width = 9;
            prev = None;
            continue;
        }
        if code == 257 {
            break;
        }
        let entry = if code < table.len() {
            table[code].clone()
        } else if let Some(p) = &prev {
            let mut e = p.clone();
            e.push(p[0]);
            e
        } else {
            break;
        };
        out.extend_from_slice(&entry);
        if let Some(p) = prev {
            let mut e = p;
            e.push(entry[0]);
            table.push(e);
        }
        prev = Some(entry);
        // Early change: the width grows one code before the table fills.
        width = match table.len() + 1 {
            n if n >= 2048 => 12,
            n if n >= 1024 => 11,
            n if n >= 512 => 10,
            _ => 9,
        };
    }
    out
}

fn lzw_encode(src: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let (mut acc, mut nbits) = (0u64, 0usize);
    let mut emit = |code: usize, width: usize, out: &mut Vec<u8>| {
        acc = (acc << width) | code as u64;
        nbits += width;
        while nbits >= 8 {
            out.push((acc >> (nbits - 8)) as u8);
            nbits -= 8;
        }
    };
    let mut dict: HashMap<Vec<u8>, usize> = HashMap::new();
    let mut next = 258usize;
    let mut width = 9usize;
    emit(256, width, &mut out);
    let mut cur: Vec<u8> = Vec::new();
    for &b in src {
        let mut ext = cur.clone();
        ext.push(b);
        if ext.len() == 1 || dict.contains_key(&ext) {
            cur = ext;
            continue;
        }
        let code = if cur.len() == 1 { cur[0] as usize } else { dict[&cur] };
        emit(code, width, &mut out);
        dict.insert(ext, next);
        next += 1;
        if next + 1 >= 4094 {
            emit(256, width, &mut out);
            dict.clear();
            next = 258;
            width = 9;
        } else {
            width = match next + 1 {
                n if n >= 2048 => 12,
                n if n >= 1024 => 11,
                n if n >= 512 => 10,
                _ => 9,
            };
        }
        cur = vec![b];
    }
    if !cur.is_empty() {
        let code = if cur.len() == 1 { cur[0] as usize } else { dict[&cur] };
        emit(code, width, &mut out);
        next += 1;
        width = match next + 1 {
            n if n >= 2048 => 12,
            n if n >= 1024 => 11,
            n if n >= 512 => 10,
            _ => 9,
        };
    }
    emit(257, width, &mut out);
    if nbits > 0 {
        out.push((acc << (8 - nbits)) as u8);
    }
    out
}

fn decode(d: &Dir, data: &[u8]) -> R<Vec<u8>> {
    Ok(match d.get(259, 1) {
        1 => data.to_vec(),
        32773 => packbits_decode(data),
        5 => lzw_decode(data),
        8 | 32946 => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(data).read_to_end(&mut out).map_err(|e| LangError::new(format!("Deflate data is corrupt: {e}"), 0))?;
            out
        }
        c => return rt(format!("Compression scheme {c} is not supported in this version.")),
    })
}

fn encode(d: &Dir, data: &[u8]) -> R<Vec<u8>> {
    Ok(match d.get(259, 1) {
        1 => data.to_vec(),
        32773 => packbits_encode(data),
        5 => lzw_encode(data),
        8 | 32946 => {
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).map_err(|e| LangError::new(e.to_string(), 0))?;
            e.finish().map_err(|e| LangError::new(e.to_string(), 0))?
        }
        c => return rt(format!("Compression scheme {c} is not supported in this version.")),
    })
}

// ---- parsing and serialization ----------------------------------------------------------------

struct Reader<'a> {
    b: &'a [u8],
    le: bool,
}

impl Reader<'_> {
    fn u16(&self, o: usize) -> Option<u16> {
        let s = self.b.get(o..o + 2)?;
        Some(if self.le { u16::from_le_bytes([s[0], s[1]]) } else { u16::from_be_bytes([s[0], s[1]]) })
    }
    fn u32(&self, o: usize) -> Option<u32> {
        let s: [u8; 4] = self.b.get(o..o + 4)?.try_into().ok()?;
        Some(if self.le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    }
    fn u64(&self, o: usize) -> Option<u64> {
        let s: [u8; 8] = self.b.get(o..o + 8)?.try_into().ok()?;
        Some(if self.le { u64::from_le_bytes(s) } else { u64::from_be_bytes(s) })
    }
}

fn type_size(t: u16) -> usize {
    match t {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 => 4,
        5 | 10 | 12 | 16 | 17 => 8,
        _ => 1,
    }
}

fn parse(bytes: &[u8]) -> Option<Vec<Dir>> {
    let le = match bytes.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let r = Reader { b: bytes, le };
    if r.u16(2)? != 42 {
        return None;
    }
    let mut off = r.u32(4)? as usize;
    let mut dirs = Vec::new();
    while off != 0 && dirs.len() < 10000 {
        let n = r.u16(off)? as usize;
        let mut d = Dir { offset: off as u64, ..Default::default() };
        for k in 0..n {
            let e = off + 2 + 12 * k;
            let (tag, typ, count) = (r.u16(e)? as u32, r.u16(e + 2)?, r.u32(e + 4)? as usize);
            let size = type_size(typ) * count;
            let at = if size <= 4 { e + 8 } else { r.u32(e + 8)? as usize };
            let val = match typ {
                2 => {
                    let raw = bytes.get(at..at + count)?;
                    TagVal::Ascii(String::from_utf8_lossy(raw).trim_end_matches('\0').to_string())
                }
                1 | 3 | 4 | 16 => TagVal::Ints((0..count).map(|i| match typ {
                    1 => bytes.get(at + i).copied().unwrap_or(0) as u64,
                    3 => r.u16(at + 2 * i).unwrap_or(0) as u64,
                    4 => r.u32(at + 4 * i).unwrap_or(0) as u64,
                    _ => r.u64(at + 8 * i).unwrap_or(0),
                }).collect()),
                6 | 8 | 9 => TagVal::Reals((0..count).map(|i| match typ {
                    6 => bytes.get(at + i).copied().unwrap_or(0) as i8 as f64,
                    8 => r.u16(at + 2 * i).unwrap_or(0) as i16 as f64,
                    _ => r.u32(at + 4 * i).unwrap_or(0) as i32 as f64,
                }).collect()),
                5 | 10 => TagVal::Reals((0..count).map(|i| {
                    let (p, q) = (r.u32(at + 8 * i).unwrap_or(0), r.u32(at + 8 * i + 4).unwrap_or(1));
                    if typ == 5 { p as f64 / q as f64 } else { p as i32 as f64 / q as i32 as f64 }
                }).collect()),
                11 => TagVal::Reals((0..count).map(|i| f32::from_bits(r.u32(at + 4 * i).unwrap_or(0)) as f64).collect()),
                12 => TagVal::Reals((0..count).map(|i| f64::from_bits(r.u64(at + 8 * i).unwrap_or(0))).collect()),
                _ => TagVal::Bytes(bytes.get(at..at + size)?.to_vec()),
            };
            d.tags.insert(tag, val);
        }
        let (offs, counts) = if d.tiled() { (324, 325) } else { (273, 279) };
        if let (Some(TagVal::Ints(o)), Some(TagVal::Ints(c))) = (d.tags.get(&offs).cloned(), d.tags.get(&counts).cloned()) {
            for (a, n) in o.iter().zip(&c) {
                d.chunks.push(bytes.get(*a as usize..(*a + *n) as usize).map(|s| s.to_vec()).unwrap_or_default());
            }
        }
        dirs.push(d);
        off = r.u32(off + 2 + 12 * n)? as usize;
    }
    Some(dirs)
}

/// (TIFF type, values as bytes) for a tag value.
fn entry_bytes(tag: u32, v: &TagVal) -> (u16, u32, Vec<u8>) {
    match v {
        TagVal::Ascii(s) => {
            let mut b = s.as_bytes().to_vec();
            b.push(0);
            (2, b.len() as u32, b)
        }
        TagVal::Bytes(b) => (7, b.len() as u32, b.clone()),
        TagVal::Ints(xs) => {
            let short = matches!(tag, 258 | 259 | 262 | 263 | 266 | 274 | 277 | 280 | 281 | 284 | 296 | 297 | 320 | 321 | 332 | 334 | 338 | 339 | 530 | 531);
            if short {
                (3, xs.len() as u32, xs.iter().flat_map(|&x| (x as u16).to_le_bytes()).collect())
            } else {
                (4, xs.len() as u32, xs.iter().flat_map(|&x| (x as u32).to_le_bytes()).collect())
            }
        }
        TagVal::Reals(xs) => {
            if matches!(tag, 282 | 283 | 286 | 287 | 318 | 319 | 529 | 532) {
                // RATIONAL with a denominator that keeps the value exact for simple numbers.
                let b = xs
                    .iter()
                    .flat_map(|&x| {
                        let q: u32 = if x.fract() == 0.0 { 1 } else { 10000 };
                        let p = (x * q as f64).round().max(0.0) as u32;
                        [p.to_le_bytes(), q.to_le_bytes()].concat()
                    })
                    .collect();
                (5, xs.len() as u32, b)
            } else {
                (12, xs.len() as u32, xs.iter().flat_map(|x| x.to_le_bytes()).collect())
            }
        }
    }
}

fn serialize(dirs: &[Dir]) -> Vec<u8> {
    let mut out: Vec<u8> = b"II*\0\0\0\0\0".to_vec();
    let mut link_at = 4usize;
    for d in dirs {
        let mut d = d.clone();
        // Chunk data first, then the directory (as MATLAB writes files).
        let mut offsets = Vec::new();
        let mut counts = Vec::new();
        for c in &d.chunks {
            offsets.push(out.len() as u64);
            counts.push(c.len() as u64);
            out.extend_from_slice(c);
            if out.len() % 2 == 1 {
                out.push(0);
            }
        }
        let (o, c) = if d.tiled() { (324, 325) } else { (273, 279) };
        d.tags.insert(o, TagVal::Ints(offsets));
        d.tags.insert(c, TagVal::Ints(counts));
        if !d.tiled() && !d.tags.contains_key(&278) {
            d.tags.insert(278, TagVal::Ints(vec![d.rows_per_strip() as u64]));
        }
        let entries: Vec<(u32, u16, u32, Vec<u8>)> = d.tags.iter().map(|(t, v)| {
            let (ty, n, b) = entry_bytes(*t, v);
            (*t, ty, n, b)
        }).collect();
        let ifd = out.len();
        let data_start = ifd + 2 + 12 * entries.len() + 4;
        let mut extra: Vec<u8> = Vec::new();
        let mut ifd_bytes: Vec<u8> = (entries.len() as u16).to_le_bytes().to_vec();
        for (t, ty, n, b) in &entries {
            ifd_bytes.extend_from_slice(&(*t as u16).to_le_bytes());
            ifd_bytes.extend_from_slice(&ty.to_le_bytes());
            ifd_bytes.extend_from_slice(&n.to_le_bytes());
            if b.len() <= 4 {
                let mut v = b.clone();
                v.resize(4, 0);
                ifd_bytes.extend_from_slice(&v);
            } else {
                let at = (data_start + extra.len()) as u32;
                ifd_bytes.extend_from_slice(&at.to_le_bytes());
                extra.extend_from_slice(b);
                if extra.len() % 2 == 1 {
                    extra.push(0);
                }
            }
        }
        let next_at = ifd + ifd_bytes.len();
        ifd_bytes.extend_from_slice(&[0, 0, 0, 0]);
        out[link_at..link_at + 4].copy_from_slice(&(ifd as u32).to_le_bytes());
        out.extend_from_slice(&ifd_bytes);
        out.extend_from_slice(&extra);
        link_at = next_at;
    }
    out
}

// ---- pixels -----------------------------------------------------------------------------------

/// The MATLAB class and kind of the samples.
fn sample_kind(d: &Dir) -> R<Kind> {
    let (bps, fmt) = (d.bps(), d.get(339, 1));
    Ok(match (bps, fmt) {
        (1, _) => Kind::Logical,
        (8, 2) => Kind::Int(IntClass::I8),
        (8, _) => Kind::Int(IntClass::U8),
        (16, 2) => Kind::Int(IntClass::I16),
        (16, _) => Kind::Int(IntClass::U16),
        (32, 2) => Kind::Int(IntClass::I32),
        (32, 3) => Kind::Single,
        (32, _) => Kind::Int(IntClass::U32),
        (64, 2) => Kind::Int(IntClass::I64),
        (64, 3) => Kind::Double,
        (64, _) => Kind::Int(IntClass::U64),
        _ => return rt(format!("{bps}-bit samples are not supported in this version.")),
    })
}

fn sample_at(bytes: &[u8], i: usize, bps: usize, fmt: u64) -> f64 {
    let b = |k: usize| bytes.get(k).copied().unwrap_or(0);
    match (bps, fmt) {
        (1, _) => ((b(i / 8) >> (7 - i % 8)) & 1) as f64,
        (8, 2) => b(i) as i8 as f64,
        (8, _) => b(i) as f64,
        (16, 2) => i16::from_le_bytes([b(2 * i), b(2 * i + 1)]) as f64,
        (16, _) => u16::from_le_bytes([b(2 * i), b(2 * i + 1)]) as f64,
        (32, 2) => i32::from_le_bytes([b(4 * i), b(4 * i + 1), b(4 * i + 2), b(4 * i + 3)]) as f64,
        (32, 3) => f32::from_le_bytes([b(4 * i), b(4 * i + 1), b(4 * i + 2), b(4 * i + 3)]) as f64,
        (32, _) => u32::from_le_bytes([b(4 * i), b(4 * i + 1), b(4 * i + 2), b(4 * i + 3)]) as f64,
        (64, f) => {
            let a: [u8; 8] = std::array::from_fn(|k| b(8 * i + k));
            match f {
                2 => i64::from_le_bytes(a) as f64,
                3 => f64::from_le_bytes(a),
                _ => u64::from_le_bytes(a) as f64,
            }
        }
        _ => 0.0,
    }
}

fn put_sample(out: &mut Vec<u8>, x: f64, bps: usize, fmt: u64) {
    match (bps, fmt) {
        (8, 2) => out.push(x as i8 as u8),
        (8, _) => out.push(x as u8),
        (16, 2) => out.extend((x as i16).to_le_bytes()),
        (16, _) => out.extend((x as u16).to_le_bytes()),
        (32, 2) => out.extend((x as i32).to_le_bytes()),
        (32, 3) => out.extend((x as f32).to_le_bytes()),
        (32, _) => out.extend((x as u32).to_le_bytes()),
        (64, 2) => out.extend((x as i64).to_le_bytes()),
        (64, 3) => out.extend(x.to_le_bytes()),
        (64, _) => out.extend((x as u64).to_le_bytes()),
        _ => out.push(x as u8),
    }
}

/// Decoded chunk k as rows x cols x samples values.
fn chunk_values(d: &Dir, k: usize) -> R<(usize, usize, Vec<f64>)> {
    let raw = d.chunks.get(k).ok_or_else(|| LangError::new("Strip or tile number is out of range.", 0))?;
    let bytes = decode(d, raw)?;
    let (rows, cols) = d.chunk_shape(k);
    let (spp, bps, fmt) = (d.chunk_spp(), d.bps(), d.get(339, 1));
    let row_bytes = (cols * spp * bps).div_ceil(8);
    let mut vals = vec![0.0; rows * cols * spp];
    for r in 0..rows {
        let line = bytes.get(r * row_bytes..((r + 1) * row_bytes).min(bytes.len())).unwrap_or(&[]);
        for c in 0..cols {
            for s in 0..spp {
                vals[(s * cols + c) * rows + r] = sample_at(line, c * spp + s, bps, fmt);
            }
        }
    }
    Ok((rows, cols, vals))
}

fn make_array(dims: Vec<usize>, vals: Vec<f64>, kind: Kind) -> Value {
    let mut m = Matrix::from_dims(dims, vals);
    m.kind = kind;
    Value::Num(m)
}

fn read_image(d: &Dir) -> R<Value> {
    let (h, w, spp) = (d.height(), d.width(), d.spp());
    let kind = sample_kind(d)?;
    let mut img = vec![0.0; h * w * spp];
    let per = d.chunks_per_plane();
    for k in 0..d.n_chunks() {
        let (rows, cols, vals) = chunk_values(d, k)?;
        let plane = k / per;
        let local = k % per;
        let (r0, c0) = if d.tiled() {
            let (tl, tw) = d.tile_dims();
            ((local / d.tiles_across()) * tl, (local % d.tiles_across()) * tw)
        } else {
            (local * d.rows_per_strip(), 0)
        };
        let cspp = d.chunk_spp();
        for s in 0..cspp {
            let ps = if d.planar_separate() { plane } else { s };
            for c in 0..cols {
                for r in 0..rows {
                    let (y, x) = (r0 + r, c0 + c);
                    if y < h && x < w {
                        img[(ps * w + x) * h + y] = vals[(s * cols + c) * rows + r];
                    }
                }
            }
        }
    }
    let dims = if spp == 1 { vec![h, w] } else { vec![h, w, spp] };
    Ok(make_array(dims, img, kind))
}

/// The image (or one chunk) as 8-bit RGB plus alpha.
fn to_rgba(d: &Dir, rows: usize, cols: usize, vals: &[f64], spp: usize) -> (Value, Value) {
    let bps = d.bps();
    let photo = d.get(262, 1);
    let scale = |x: f64| -> f64 {
        match bps {
            1 => x * 255.0,
            8 => x,
            16 => (x / 257.0).round(),
            _ => (x.clamp(0.0, 1.0) * 255.0).round(),
        }
    };
    let n = rows * cols;
    let mut rgb = vec![0.0; n * 3];
    let mut alpha = vec![255.0; n];
    let cmap: Option<Vec<u64>> = match d.tags.get(&320) {
        Some(TagVal::Ints(v)) => Some(v.clone()),
        _ => None,
    };
    for i in 0..n {
        let s = |k: usize| vals.get(k * n + i).copied().unwrap_or(0.0);
        let (r, g, b) = match photo {
            0 => {
                let v = 255.0 - scale(s(0));
                (v, v, v)
            }
            3 => match &cmap {
                Some(m) => {
                    let len = m.len() / 3;
                    let k = s(0) as usize;
                    let at = |c: usize| (m.get(c * len + k).copied().unwrap_or(0) as f64 / 257.0).round();
                    (at(0), at(1), at(2))
                }
                None => (s(0), s(0), s(0)),
            },
            2 => (scale(s(0)), scale(s(1)), scale(s(2))),
            _ => {
                let v = scale(s(0));
                (v, v, v)
            }
        };
        rgb[i] = r;
        rgb[n + i] = g;
        rgb[2 * n + i] = b;
        let alpha_at = if photo == 2 { 3 } else { 1 };
        if spp > alpha_at {
            alpha[i] = scale(s(alpha_at));
        }
    }
    (make_array(vec![rows, cols, 3], rgb, Kind::Int(IntClass::U8)), make_array(vec![rows, cols], alpha, Kind::Int(IntClass::U8)))
}

/// Image data (rows x cols x samples) as chunk bytes.
fn pack(d: &Dir, vals: &[f64], dims: &[usize], r0: usize, c0: usize, rows: usize, cols: usize, plane: Option<usize>) -> Vec<u8> {
    let (h, w) = (dims[0], dims.get(1).copied().unwrap_or(1));
    let spp = d.chunk_spp();
    let (bps, fmt) = (d.bps(), d.get(339, 1));
    let mut out = Vec::with_capacity(d.chunk_bytes(rows, cols));
    for r in 0..rows {
        if bps == 1 {
            let mut bits = vec![0u8; (cols * spp).div_ceil(8)];
            for c in 0..cols {
                let (y, x) = (r0 + r, c0 + c);
                let v = if y < h && x < w { vals[x * h + y] } else { 0.0 };
                if v != 0.0 {
                    bits[c / 8] |= 0x80 >> (c % 8);
                }
            }
            out.extend(bits);
            continue;
        }
        for c in 0..cols {
            for s in 0..spp {
                let ps = plane.unwrap_or(s);
                let (y, x) = (r0 + r, c0 + c);
                let v = if y < h && x < w { vals.get((ps * w + x) * h + y).copied().unwrap_or(0.0) } else { 0.0 };
                put_sample(&mut out, v, bps, fmt);
            }
        }
    }
    out
}

// ---- the methods ------------------------------------------------------------------------------

fn id_of(it: &Interpreter, v: &Value) -> R<usize> {
    let _ = it;
    match v {
        Value::Object(o) if o.class.name == CLASS => {
            let i = o.class.prop_index("Id").ok_or_else(|| LangError::new("Invalid TIFF object.", 0))?;
            Ok(o.props.borrow()[i].scalar()? as usize)
        }
        _ => rt("Expected a Tiff object."),
    }
}

fn num(x: f64) -> Value {
    Value::num(x)
}

fn tag_value(id: u32, v: &TagVal) -> Value {
    match v {
        TagVal::Ascii(s) => Value::chars(s),
        TagVal::Bytes(b) => make_array(vec![1, b.len()], b.iter().map(|&x| x as f64).collect(), Kind::Int(IntClass::U8)),
        TagVal::Ints(xs) => {
            if matches!(id, 273 | 279 | 324 | 325) {
                return make_array(vec![1, xs.len()], xs.iter().map(|&x| x as f64).collect(), Kind::Int(IntClass::U64));
            }
            if id == 320 {
                // ColorMap: n x 3 in [0, 1].
                let n = xs.len() / 3;
                return Value::Num(Matrix::from_dims(vec![n, 3], xs.iter().map(|&x| x as f64 / 65535.0).collect()));
            }
            if matches!(id, 258 | 339) || xs.len() == 1 {
                return num(xs.first().copied().unwrap_or(0) as f64);
            }
            Value::Num(Matrix::row(xs.iter().map(|&x| x as f64).collect()))
        }
        TagVal::Reals(xs) => {
            if xs.len() == 1 {
                num(xs[0])
            } else {
                Value::Num(Matrix::row(xs.clone()))
            }
        }
    }
}

/// Defaults the TIFF specification gives tags that are not written.
fn default_tag(id: u32, d: &Dir) -> Option<TagVal> {
    let i = |x: u64| Some(TagVal::Ints(vec![x]));
    match id {
        254 => i(0),
        258 => i(1),
        259 => i(1),
        263 => i(1),
        266 => i(1),
        274 => i(1),
        277 => i(1),
        278 => i(u32::MAX as u64),
        280 => i(0),
        281 => i((1u64 << d.bps().min(32)) - 1),
        284 => i(1),
        296 => i(2),
        339 => i(1),
        _ => None,
    }
}

fn to_tag(id: u32, v: &Value) -> R<TagVal> {
    if v.is_text() {
        return Ok(TagVal::Ascii(v.text()?));
    }
    let m = v.to_matrix()?;
    if id == 320 {
        // ColorMap given as n x 3 in [0, 1].
        let n = m.rows();
        let mut out = Vec::with_capacity(3 * n);
        for c in 0..3 {
            for r in 0..n {
                out.push((m.data[c * n + r] * 65535.0).round() as u64);
            }
        }
        return Ok(TagVal::Ints(out));
    }
    if matches!(id, 282 | 283 | 286 | 287 | 318 | 319 | 529 | 532 | 33550 | 33922 | 34264 | 34736 | 37439) {
        return Ok(TagVal::Reals(m.data.clone()));
    }
    if m.data.iter().all(|x| x.fract() == 0.0 && *x >= 0.0) {
        return Ok(TagVal::Ints(m.data.iter().map(|&x| x as u64).collect()));
    }
    Ok(TagVal::Reals(m.data.clone()))
}

fn tag_arg(v: &Value) -> R<u32> {
    if v.is_text() {
        let n = v.text()?;
        return tag_id(&n).ok_or_else(|| LangError::new(format!("Unrecognized field name \"{n}\"."), 0));
    }
    Ok(v.scalar()? as u32)
}

fn const_name(group: &str, v: u64) -> String {
    CONSTS
        .iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, fs)| fs.iter().find(|(_, x)| *x as u64 == v))
        .map(|(n, _)| format!("Tiff.{group}.{n}"))
        .unwrap_or_else(|| v.to_string())
}

/// The display of a Tiff object.
pub fn object_text(o: &Object) -> Option<String> {
    if o.class.name != CLASS {
        return None;
    }
    let i = o.class.prop_index("Id")?;
    let id = o.props.borrow()[i].scalar().ok()? as usize;
    FILES.with(|m| {
        let m = m.borrow();
        let t = m.get(&id)?;
        let d = t.dirs.get(t.cur).cloned().unwrap_or_default();
        let mut rows: Vec<(String, String)> = vec![("TIFF File".into(), format!("'{}'", t.path)), ("Mode".into(), format!("'{}'", t.mode)), ("Current Image Directory".into(), (t.cur + 1).to_string())];
        if d.tiled() {
            rows.push(("Number Of Tiles".into(), d.n_chunks().to_string()));
        } else {
            rows.push(("Number Of Strips".into(), d.n_chunks().to_string()));
        }
        let g = |id: u32| d.int(id).or_else(|| match default_tag(id, &d) { Some(TagVal::Ints(v)) => v.first().copied(), _ => None }).unwrap_or(0);
        rows.push(("SubFileType".into(), const_name("SubFileType", g(254))));
        rows.push(("Photometric".into(), const_name("Photometric", g(262))));
        rows.push(("ImageLength".into(), g(257).to_string()));
        rows.push(("ImageWidth".into(), g(256).to_string()));
        if d.tiled() {
            rows.push(("TileLength".into(), g(323).to_string()));
            rows.push(("TileWidth".into(), g(322).to_string()));
        } else {
            rows.push(("RowsPerStrip".into(), g(278).to_string()));
        }
        rows.push(("BitsPerSample".into(), g(258).to_string()));
        rows.push(("Compression".into(), const_name("Compression", g(259))));
        rows.push(("SampleFormat".into(), const_name("SampleFormat", g(339))));
        rows.push(("SamplesPerPixel".into(), g(277).to_string()));
        rows.push(("PlanarConfiguration".into(), const_name("PlanarConfiguration", g(284))));
        rows.push(("Orientation".into(), const_name("Orientation", g(274))));
        let w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        Some(rows.iter().map(|(k, v)| format!("    {k:>w$}: {v}")).collect::<Vec<_>>().join("\n") + "\n")
    })
}

fn open(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let name = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
    let mode = match a.get(1) {
        Some(m) => m.text()?,
        None => "r".into(),
    };
    if !matches!(mode.as_str(), "r" | "w" | "w8" | "a" | "r+") {
        return rt(format!("Invalid mode '{mode}'."));
    }
    let _ = it;
    let path = std::path::PathBuf::from(&name);
    let full = std::path::absolute(&path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.display().to_string());
    let dirs = if mode.starts_with('w') {
        vec![Dir::default()]
    } else {
        let bytes = std::fs::read(&path).map_err(|_| LangError::new(format!("Could not open file '{name}'."), 0))?;
        let mut d = parse(&bytes).ok_or_else(|| LangError::new(format!("Could not open file '{name}'. It is not a TIFF file."), 0))?;
        if mode == "a" {
            d.push(Dir::default());
        }
        d
    };
    let cur = if mode == "a" { dirs.len() - 1 } else { 0 };
    let id = NEXT.with(|n| {
        let mut n = n.borrow_mut();
        *n += 1;
        *n
    });
    FILES.with(|m| m.borrow_mut().insert(id, TiffFile { path: full, mode, dirs, cur, dirty: false }));
    Ok(num(id as f64))
}

fn flush(t: &mut TiffFile) -> R<()> {
    if t.dirty && t.mode != "r" {
        let dirs: Vec<Dir> = t.dirs.iter().filter(|d| !d.tags.is_empty()).cloned().collect();
        std::fs::write(&t.path, serialize(&dirs)).map_err(|e| LangError::new(format!("Unable to write file '{}': {e}", t.path), 0))?;
        t.dirty = false;
    }
    Ok(())
}

/// `matlivre_tiff(op, nargout, obj, args...)`: the methods of the Tiff class.
fn method(it: &mut Interpreter, a: &[Value]) -> R<Value> {
    let op = a.first().ok_or_else(|| LangError::new("Not enough input arguments.", 0))?.text()?;
    let nargout = a.get(1).map(|v| v.scalar().unwrap_or(0.0) as usize).unwrap_or(0);
    let rest = if a.len() > 3 { &a[3..] } else { &[] };
    let cell = |vs: Vec<Value>| Value::Cell(CellArray::row(vs));
    match op.as_str() {
        "open" => return open(it, rest),
        "getVersion" => return Ok(Value::chars(VERSION)),
        "getTagNames" => return Ok(Value::Cell(CellArray::col(TAGS.iter().map(|(n, _)| Value::chars(n)).collect()))),
        _ => {}
    }
    let id = id_of(it, a.get(2).ok_or_else(|| LangError::new("Not enough input arguments.", 0))?)?;
    let arg = |i: usize| rest.get(i).ok_or_else(|| LangError::new("Not enough input arguments.", 0));
    with_file(id, |t| {
        let writable = t.mode != "r";
        let cur = t.cur;
        Ok(match op.as_str() {
            "close" => {
                flush(t)?;
                cell(Vec::new())
            }
            "getTag" => {
                let tid = tag_arg(arg(0)?)?;
                let d = &t.dirs[cur];
                let name = tag_name(tid).map(String::from).unwrap_or_else(|| tid.to_string());
                let v = d.tags.get(&tid).cloned().or_else(|| default_tag(tid, d)).ok_or_else(|| LangError::new(format!("Unable to retrieve {name}."), 0))?;
                cell(vec![tag_value(tid, &v)])
            }
            "setTag" => {
                if !writable {
                    return rt("Unable to set tags: the file is open for reading only.");
                }
                let pairs: Vec<(u32, Value)> = match arg(0)? {
                    Value::Struct(s) => s.fields.iter().map(|f| Ok((tag_id(f).ok_or_else(|| LangError::new(format!("Unrecognized field name \"{f}\"."), 0))?, s.get(f).cloned().unwrap_or_else(Value::empty)))).collect::<R<_>>()?,
                    other => vec![(tag_arg(other)?, arg(1)?.clone())],
                };
                for (tid, v) in pairs {
                    let tv = to_tag(tid, &v)?;
                    t.dirs[cur].tags.insert(tid, tv);
                }
                t.dirty = true;
                cell(Vec::new())
            }
            "write" => {
                if !writable {
                    return rt("Unable to write: the file is open for reading only.");
                }
                let img = arg(0)?.to_matrix()?;
                let d = &mut t.dirs[cur];
                if d.width() == 0 || d.height() == 0 {
                    return rt("ImageLength and ImageWidth must be set before writing the image.");
                }
                if !d.tiled() && !d.tags.contains_key(&278) {
                    // Strips of about 8 KB, as libtiff chooses.
                    let line = (d.width() * d.chunk_spp() * d.bps()).div_ceil(8).max(1);
                    d.tags.insert(278, TagVal::Ints(vec![(8192 / line).max(1) as u64]));
                }
                let dims = img.dims.clone();
                let mut chunks = Vec::new();
                let per = d.chunks_per_plane();
                for k in 0..d.n_chunks() {
                    let plane = if d.planar_separate() { Some(k / per) } else { None };
                    let local = k % per;
                    let (rows, cols) = d.chunk_shape(k);
                    let (r0, c0) = if d.tiled() {
                        let (tl, tw) = d.tile_dims();
                        ((local / d.tiles_across()) * tl, (local % d.tiles_across()) * tw)
                    } else {
                        (local * d.rows_per_strip(), 0)
                    };
                    let raw = pack(d, &img.data, &dims, r0, c0, rows, cols, plane);
                    chunks.push(encode(d, &raw)?);
                }
                d.chunks = chunks;
                t.dirty = true;
                cell(Vec::new())
            }
            "writeDirectory" => {
                t.dirs.push(Dir::default());
                t.cur = t.dirs.len() - 1;
                t.dirty = true;
                flush(t)?;
                t.dirty = true;
                cell(Vec::new())
            }
            "rewriteDirectory" => {
                t.dirty = true;
                flush(t)?;
                cell(Vec::new())
            }
            "numberOfStrips" => {
                let d = &t.dirs[cur];
                if d.tiled() {
                    return rt("The image is tiled; use numberOfTiles.");
                }
                cell(vec![num(d.n_chunks() as f64)])
            }
            "numberOfTiles" => {
                let d = &t.dirs[cur];
                if !d.tiled() {
                    return rt("The image is stripped; use numberOfStrips.");
                }
                cell(vec![num(d.n_chunks() as f64)])
            }
            "isTiled" => cell(vec![Value::bool(t.dirs[cur].tiled())]),
            "currentDirectory" => cell(vec![num((cur + 1) as f64)]),
            "lastDirectory" => cell(vec![Value::bool(cur + 1 >= t.dirs.iter().filter(|d| !d.tags.is_empty()).count())]),
            "nextDirectory" => {
                if cur + 1 >= t.dirs.len() {
                    return rt("Unable to move to the next directory: this is the last one.");
                }
                t.cur += 1;
                cell(Vec::new())
            }
            "setDirectory" => {
                let n = arg(0)?.scalar()? as usize;
                if n < 1 || n > t.dirs.len() {
                    return rt(format!("Directory {n} does not exist."));
                }
                t.cur = n - 1;
                cell(Vec::new())
            }
            "setSubDirectory" => {
                let off = arg(0)?.scalar()? as u64;
                let k = t.dirs.iter().position(|d| d.offset == off).ok_or_else(|| LangError::new("No image directory at that offset.", 0))?;
                t.cur = k;
                cell(Vec::new())
            }
            "computeStrip" => {
                let d = &t.dirs[cur];
                let row = arg(0)?.scalar()?.max(1.0) as usize;
                let plane = rest.get(1).map(|v| v.scalar().unwrap_or(1.0) as usize).unwrap_or(1).max(1);
                let s = ((row - 1) / d.rows_per_strip()).min(d.chunks_per_plane() - 1);
                cell(vec![num((s + (plane - 1) * d.chunks_per_plane() + 1) as f64)])
            }
            "computeTile" => {
                let d = &t.dirs[cur];
                let rc = arg(0)?.to_matrix()?;
                let (row, col) = (rc.data.first().copied().unwrap_or(1.0).max(1.0) as usize, rc.data.get(1).copied().unwrap_or(1.0).max(1.0) as usize);
                let plane = rest.get(1).map(|v| v.scalar().unwrap_or(1.0) as usize).unwrap_or(1).max(1);
                let (tl, tw) = d.tile_dims();
                let k = ((row - 1) / tl).min(d.tiles_down() - 1) * d.tiles_across() + ((col - 1) / tw).min(d.tiles_across() - 1);
                cell(vec![num((k + (plane - 1) * d.chunks_per_plane() + 1) as f64)])
            }
            "readEncodedStrip" | "readEncodedTile" => {
                let d = &t.dirs[cur];
                let k = arg(0)?.scalar()? as usize;
                if k < 1 || k > d.n_chunks() {
                    return rt("Strip or tile number is out of range.");
                }
                let (rows, cols, vals) = chunk_values(d, k - 1)?;
                let spp = d.chunk_spp();
                let dims = if spp == 1 { vec![rows, cols] } else { vec![rows, cols, spp] };
                cell(vec![make_array(dims, vals, sample_kind(d)?)])
            }
            "writeEncodedStrip" | "writeEncodedTile" => {
                let d = &mut t.dirs[cur];
                let k = arg(0)?.scalar()? as usize;
                let n = d.n_chunks();
                if k < 1 || k > n {
                    return rt("Strip or tile number is out of range.");
                }
                let m = arg(1)?.to_matrix()?;
                let (rows, cols) = d.chunk_shape(k - 1);
                let dims = m.dims.clone();
                let raw = pack(d, &m.data, &dims, 0, 0, rows, cols, if d.chunk_spp() == 1 { Some(0) } else { None });
                let enc = encode(d, &raw)?;
                d.chunks.resize(n, Vec::new());
                d.chunks[k - 1] = enc;
                t.dirty = true;
                cell(Vec::new())
            }
            "read" => cell(vec![read_image(&t.dirs[cur])?]),
            "readRGBAImage" => {
                let d = &t.dirs[cur];
                let img = read_image(d)?.to_matrix()?;
                let (h, w) = (d.height(), d.width());
                let (rgb, alpha) = to_rgba(d, h, w, &img.data, d.spp());
                cell(if nargout > 1 { vec![rgb, alpha] } else { vec![rgb] })
            }
            "readRGBAStrip" | "readRGBATile" => {
                let d = &t.dirs[cur];
                let img = read_image(d)?.to_matrix()?;
                let (h, w, spp) = (d.height(), d.width(), d.spp());
                let row = arg(0)?.scalar()?.max(1.0) as usize - 1;
                let (r0, c0, rows, cols) = if op == "readRGBAStrip" {
                    let rps = d.rows_per_strip();
                    let r0 = (row / rps) * rps;
                    (r0, 0, rps.min(h - r0), w)
                } else {
                    let col = arg(1)?.scalar()?.max(1.0) as usize - 1;
                    let (tl, tw) = d.tile_dims();
                    let (r0, c0) = ((row / tl) * tl, (col / tw) * tw);
                    (r0, c0, tl.min(h - r0), tw.min(w - c0))
                };
                let mut sub = vec![0.0; rows * cols * spp];
                for s in 0..spp {
                    for c in 0..cols {
                        for r in 0..rows {
                            sub[(s * cols + c) * rows + r] = img.data[(s * w + c0 + c) * h + r0 + r];
                        }
                    }
                }
                let (rgb, alpha) = to_rgba(d, rows, cols, &sub, spp);
                cell(if nargout > 1 { vec![rgb, alpha] } else { vec![rgb] })
            }
            other => return rt(format!("Unknown Tiff method '{other}'.")),
        })
    })
}

pub fn call(it: &mut Interpreter, name: &str, a: &[Value], _nargout: usize) -> R<Option<Vec<Value>>> {
    if name != "matlivre_tiff" {
        return Ok(None);
    }
    let op = a.first().map(|v| v.text().unwrap_or_default()).unwrap_or_default();
    let v = method(it, a)?;
    // The constructor and the static methods return a value; the methods a varargout cell.
    if matches!(op.as_str(), "open" | "getVersion" | "getTagNames") {
        return Ok(Some(vec![v]));
    }
    Ok(Some(vec![v]))
}
