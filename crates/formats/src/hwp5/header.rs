//! FileHeader and DocInfo stream parsing for HWP 5.x files.

use std::sync::Arc;
use wordcraft_doc::{Align, Rgb};

use super::record::{RecordReader, i32_at, tags, u16_at, u32_at, utf16_units};
use crate::model::{Fmt, Spacing, kept_font};

pub const HWP5_SIGNATURE: &[u8] = b"HWP Document File";

/// The 256-byte fixed FileHeader stream of an HWP 5.x file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileHeader {
    pub version: u32,
    pub is_compressed: bool,
    pub is_encrypted: bool,
    /// A distribution document: its body is encrypted in `ViewText`, not `BodyText`.
    pub is_distribution: bool,
}

impl FileHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 256 {
            return Err("HWP 5.x FileHeader stream is shorter than 256 bytes".to_string());
        }
        if !bytes.starts_with(HWP5_SIGNATURE) {
            return Err("Invalid HWP 5.x signature (expected 'HWP Document File')".to_string());
        }
        let version = u32_at(bytes, 32).unwrap_or_default();
        let flags = u32_at(bytes, 36).unwrap_or_default();
        Ok(FileHeader { version, is_compressed: flags & 0x01 != 0, is_encrypted: flags & 0x02 != 0, is_distribution: flags & 0x04 != 0 })
    }
}

/// A paragraph shape: alignment and indents, points.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParaShape {
    pub align: Option<Align>,
    pub spacing: Spacing,
}

/// A binary item embedded in the file: the `BinData/BIN{id:04X}.{ext}` stream.
#[derive(Clone, Debug, PartialEq)]
pub struct BinItem {
    pub id: u16,
    pub ext: String,
    /// Whether its stream is compressed: `None` follows the document.
    pub compressed: Option<bool>,
}

/// What the body needs from the `DocInfo` stream, indexed by the ids the body refers to.
#[derive(Clone, Debug, Default)]
pub struct DocInfo {
    /// Hangul font faces (the first language group); `None` for a name too long to keep.
    pub fonts: Vec<Option<Arc<str>>>,
    pub char_shapes: Vec<Fmt>,
    pub para_shapes: Vec<ParaShape>,
    /// Binary items in order (pictures refer to them from 1); `None` for a linked file.
    pub bin_items: Vec<Option<BinItem>>,
}

impl DocInfo {
    pub fn parse(stream_bytes: &[u8]) -> Self {
        let mut info = DocInfo::default();
        // Face names come in seven language groups (Hangul first); ID_MAPPINGS gives their sizes.
        let mut hangul_fonts = None;
        let mut faces = 0usize;
        for rec in RecordReader::new(stream_bytes) {
            let d = rec.data;
            match rec.tag {
                tags::ID_MAPPINGS => hangul_fonts = u32_at(d, 4).map(|n| n as usize),
                tags::BIN_DATA => info.bin_items.push(bin_item(d)),
                tags::FACE_NAME => {
                    faces += 1;
                    if hangul_fonts.is_none_or(|n| faces <= n) {
                        info.fonts.push(face_name(d));
                    }
                }
                // Shapes keep their place even when unreadable: the body refers to them by index.
                tags::CHAR_SHAPE => info.char_shapes.push(char_shape(d, &info.fonts)),
                tags::PARA_SHAPE => info.para_shapes.push(para_shape(d)),
                _ => {}
            }
        }
        info
    }
}

/// BIN_DATA: attributes (bits 0-3: 0 link, 1 embedded, 2 storage; bits 4-5: 0 as the document,
/// 1 compressed, 2 stored), then for an embedded item its id and its extension (length-prefixed
/// UTF-16).
fn bin_item(d: &[u8]) -> Option<BinItem> {
    let attr = u16_at(d, 0)?;
    if attr & 0x0F != 1 {
        return None;
    }
    let id = u16_at(d, 2)?;
    let len = usize::from(u16_at(d, 4)?).min(16);
    let units = utf16_units(d.get(6..6 + len * 2)?);
    let ext: String = String::from_utf16_lossy(&units).chars().filter(char::is_ascii_alphanumeric).collect();
    let compressed = match (attr >> 4) & 0x03 {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    };
    Some(BinItem { id, ext, compressed })
}

/// FACE_NAME: a property byte, the name's length in UTF-16 units, then the name.
fn face_name(d: &[u8]) -> Option<Arc<str>> {
    let len = usize::from(u16_at(d, 1)?);
    let bytes = d.get(3..3 + len * 2)?;
    let units = utf16_units(bytes);
    let name = String::from_utf16_lossy(&units);
    let name = name.trim();
    if name.is_empty() { None } else { kept_font(name) }
}

/// CHAR_SHAPE: seven font ids (Hangul first), per-language ratios, spacings, relative sizes and
/// offsets (42 bytes), then the base size (HWPUNIT, 1/100 pt), the attributes and the text colour.
fn char_shape(d: &[u8], fonts: &[Option<Arc<str>>]) -> Fmt {
    let mut fmt = Fmt::default();
    if let Some(id) = u16_at(d, 0) {
        fmt.font = fonts.get(usize::from(id)).cloned().flatten();
    }
    if let Some(size) = i32_at(d, 42).filter(|s| (100..=409_600).contains(s)) {
        fmt.size = Some(size as f32 / 100.0);
    }
    if let Some(attr) = u32_at(d, 46) {
        fmt.italic = attr & 0x01 != 0;
        fmt.bold = attr & 0x02 != 0;
        // Underline kind (bits 2-3): 1 below the text, 3 above it.
        fmt.underline = (attr >> 2) & 0x03 == 1;
        fmt.sup = attr & (1 << 15) != 0;
        fmt.sub = attr & (1 << 16) != 0;
        fmt.strike = (attr >> 18) & 0x07 != 0;
    }
    // COLORREF 0x00BBGGRR; black is the default colour.
    if let Some(c) = d.get(52..55)
        && c != [0, 0, 0]
    {
        fmt.color = Some(Rgb(c[0], c[1], c[2]));
    }
    fmt
}

/// PARA_SHAPE: attributes (alignment in bits 2-4), then the left and right margins, the first
/// line indent and the space above and below, in HWPUNIT. The margins and spaces are stored
/// doubled (HWPX writes the same doubled values in its `hp:default` branch).
fn para_shape(d: &[u8]) -> ParaShape {
    let Some(attr) = u32_at(d, 0) else { return ParaShape::default() };
    let align = match (attr >> 2) & 0x07 {
        0 => Some(Align::Justify),
        1 => Some(Align::Left),
        2 => Some(Align::Right),
        3 => Some(Align::Center),
        4 | 5 => Some(Align::Distribute),
        _ => None,
    };
    let pt = |at: usize| i32_at(d, at).filter(|v| *v != 0).map(|v| v as f32 / 200.0);
    let spacing = Spacing { left: pt(4), right: pt(8), first: pt(12), before: pt(16), after: pt(20), ..Default::default() };
    ParaShape { align, spacing }
}
