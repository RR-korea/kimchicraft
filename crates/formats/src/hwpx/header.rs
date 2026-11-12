//! Header metadata for HWPX documents (`Contents/header.xml`): fonts, character and paragraph
//! properties, border fills and styles, by id.

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::{Align, Rgb};

use super::styles::{hwpu_to_pt, parse_align, parse_color};
use super::xml::{self, Elem};
use crate::model::{Fmt, Spacing, kept_font};

/// Paragraph properties from `<hh:paraPr>`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParaStyleDef {
    pub align: Option<Align>,
    pub spacing: Spacing,
    pub page_break: bool,
    pub keep_next: bool,
}

/// What the sections need from `Contents/header.xml`.
#[derive(Clone, Debug, Default)]
pub struct HeaderData {
    pub char_props: HashMap<u32, Fmt>,
    pub para_props: HashMap<u32, ParaStyleDef>,
    /// Border fills with a solid background: id → colour.
    pub border_fills: HashMap<u32, Rgb>,
    /// Styles: id → (paraPr id, charPr id).
    pub styles: HashMap<u32, (Option<u32>, Option<u32>)>,
}

fn id(e: &Elem, key: &str) -> Option<u32> {
    e.int(key).and_then(|v| u32::try_from(v).ok())
}

impl HeaderData {
    pub fn parse(xml: &[u8]) -> Self {
        let mut data = HeaderData::default();
        let Some(root) = xml::parse(xml) else { return data };

        // Font ids count from 0 in each language group; characters name the Hangul one.
        let mut fonts: HashMap<u32, Arc<str>> = HashMap::new();
        let mut faces = Vec::new();
        root.find_all("fontface", &mut faces);
        for face in faces.iter().filter(|f| f.attr("lang").is_some_and(|l| l.eq_ignore_ascii_case("HANGUL"))) {
            for font in face.elems().filter(|e| e.name == "font") {
                if let (Some(i), Some(name)) = (id(font, "id"), font.attr("face").and_then(kept_font)) {
                    fonts.insert(i, name);
                }
            }
        }

        let mut list = Vec::new();
        root.find_all("charPr", &mut list);
        for e in &list {
            if let Some(i) = id(e, "id") {
                data.char_props.insert(i, char_pr(e, &fonts));
            }
        }

        list.clear();
        root.find_all("paraPr", &mut list);
        for e in &list {
            if let Some(i) = id(e, "id") {
                data.para_props.insert(i, para_pr(e));
            }
        }

        list.clear();
        root.find_all("borderFill", &mut list);
        for e in &list {
            let color = e.find("winBrush").and_then(|b| b.attr("faceColor")).and_then(parse_color);
            if let (Some(i), Some(c)) = (id(e, "id"), color) {
                data.border_fills.insert(i, c);
            }
        }

        list.clear();
        root.find_all("style", &mut list);
        for e in &list {
            if let Some(i) = id(e, "id") {
                data.styles.insert(i, (id(e, "paraPrIDRef"), id(e, "charPrIDRef")));
            }
        }
        data
    }
}

/// `<hh:charPr height textColor shadeColor>` with `fontRef`, `bold`, `italic`, `underline`,
/// `strikeout`, `supscript` and `subscript` children.
fn char_pr(e: &Elem, fonts: &HashMap<u32, Arc<str>>) -> Fmt {
    let mut fmt = Fmt::default();
    if let Some(h) = e.int("height").filter(|h| (100..=409_600).contains(h)) {
        fmt.size = Some(hwpu_to_pt(h));
    }
    fmt.color = e.attr("textColor").and_then(parse_color).filter(|c| *c != Rgb(0, 0, 0));
    fmt.background = e.attr("shadeColor").and_then(parse_color).filter(|c| *c != Rgb(0xFF, 0xFF, 0xFF));
    fmt.font = e.child("fontRef").and_then(|f| id(f, "hangul")).and_then(|i| fonts.get(&i)).cloned();
    fmt.bold = e.child("bold").is_some();
    fmt.italic = e.child("italic").is_some();
    fmt.underline = e.child("underline").and_then(|u| u.attr("type")).is_some_and(|t| t.eq_ignore_ascii_case("BOTTOM"));
    fmt.strike = e.child("strikeout").and_then(|s| s.attr("shape")).is_some_and(|s| !s.eq_ignore_ascii_case("NONE"));
    fmt.sup = e.child("supscript").is_some();
    fmt.sub = e.child("subscript").is_some();
    fmt
}

/// `<hh:paraPr>` with `align`, `breakSetting` and `margin` children. The margin sits in an
/// `hp:switch`: its `hp:case` (the HwpUnitChar namespace) holds the true values, its `hp:default`
/// the same doubled for older readers.
fn para_pr(e: &Elem) -> ParaStyleDef {
    let mut def = ParaStyleDef { align: e.child("align").and_then(|a| a.attr("horizontal")).and_then(parse_align), ..Default::default() };
    if let Some(b) = e.child("breakSetting") {
        def.page_break = b.flag("pageBreakBefore");
        def.keep_next = b.flag("keepWithNext");
    }
    let switch = e.child("switch");
    let margin = switch
        .and_then(|s| s.child("case"))
        .and_then(|c| c.child("margin"))
        .map(|m| (m, 1.0))
        .or_else(|| e.child("margin").map(|m| (m, 1.0)))
        .or_else(|| switch.and_then(|s| s.child("default")).and_then(|d| d.child("margin")).map(|m| (m, 0.5)));
    if let Some((m, scale)) = margin {
        // Each value is a child (`<hc:left value unit>`); older writers put them on attributes.
        let value = |name: &str| -> Option<f32> {
            let v = match m.child(name) {
                Some(c) if c.attr("unit").is_none_or(|u| u.eq_ignore_ascii_case("HWPUNIT")) => c.int("value"),
                Some(_) => None,
                None => m.int(name),
            };
            v.filter(|v| *v != 0 && v.abs() <= 1_440_000).map(|v| hwpu_to_pt(v) * scale)
        };
        def.spacing = Spacing {
            left: value("left"),
            right: value("right"),
            first: value("intent").or_else(|| value("indent")),
            before: value("prev"),
            after: value("next"),
            ..Default::default()
        };
    }
    def
}
