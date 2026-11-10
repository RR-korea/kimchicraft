//! Header metadata parser for HWPX documents (`Contents/header.xml`).

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use quick_xml::Reader;
use quick_xml::events::Event;
use wordcraft_doc::{Align, Rgb};

use crate::model::{Fmt, Kind, Spacing, kept_font};
use super::styles::{hwpu_to_pt, parse_align, parse_color};

/// Paragraph style definition extracted from `<hh:paraPr>`.
#[derive(Clone, Debug, Default)]
pub struct ParaStyleDef {
    pub align: Option<Align>,
    pub kind: Kind,
    pub spacing: Spacing,
    pub border_fill_id: Option<u32>,
}

/// Metadata collection from `Contents/header.xml`.
#[derive(Clone, Debug, Default)]
pub struct HeaderData {
    /// Font faces by id: id -> font name
    pub fonts: HashMap<u32, Arc<str>>,
    /// Character properties by id: id -> Fmt
    pub char_props: HashMap<u32, Fmt>,
    /// Paragraph properties by id: id -> ParaStyleDef
    pub para_props: HashMap<u32, ParaStyleDef>,
    /// Border/Fill properties: id -> (shading, borders)
    pub border_fills: HashMap<u32, Option<Rgb>>,
    /// Style definitions: id -> (para_pr_id, char_pr_id)
    pub styles: HashMap<u32, (Option<u32>, Option<u32>)>,
}

impl HeaderData {
    pub fn parse(xml: &[u8]) -> Self {
        let mut data = HeaderData::default();
        let mut reader = Reader::from_reader(Cursor::new(xml));
        reader.config_mut().trim_text(true);
        let mut buf = Vec::with_capacity(1024);

        let mut current_para_pr_id: Option<u32> = None;
        let mut current_border_fill_id: Option<u32> = None;

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"font" => {
                            let mut id = None;
                            let mut face = None;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    b"face" => face = attr.unescape_value().ok().map(|v| v.to_string()),
                                    _ => {}
                                }
                            }
                            if let (Some(id), Some(face)) = (id, face) {
                                if let Some(font_arc) = kept_font(&face) {
                                    data.fonts.insert(id, font_arc);
                                }
                            }
                        }
                        b"charPr" => {
                            let mut id = None;
                            let mut fmt = Fmt::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    b"height" => {
                                        if let Ok(h) = attr.unescape_value() {
                                            if let Ok(hwpu) = h.parse::<i64>() {
                                                fmt.size = Some(hwpu_to_pt(hwpu));
                                            }
                                        }
                                    }
                                    b"textColor" => {
                                        if let Ok(c) = attr.unescape_value() {
                                            fmt.color = parse_color(&c);
                                        }
                                    }
                                    b"bold" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            fmt.bold = v == "1" || v.eq_ignore_ascii_case("true");
                                        }
                                    }
                                    b"italic" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            fmt.italic = v == "1" || v.eq_ignore_ascii_case("true");
                                        }
                                    }
                                    b"underlineType" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            fmt.underline = !v.eq_ignore_ascii_case("none") && !v.is_empty();
                                        }
                                    }
                                    b"strikeOutType" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            fmt.strike = !v.eq_ignore_ascii_case("none") && !v.is_empty();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(id) = id {
                                data.char_props.insert(id, fmt);
                            }
                        }
                        b"fontRef" => {
                            // Sub-element of charPr specifying font IDs for various scripts (hangul, latin, etc.)
                            // For simplicity, take hangul or latin
                            if let Some(last_id) = data.char_props.keys().copied().max() {
                                for attr in e.attributes().flatten() {
                                    let key = attr.key.local_name();
                                    if key.as_ref() == b"hangul" || key.as_ref() == b"latin" {
                                        if let Ok(fid_str) = attr.unescape_value() {
                                            if let Ok(fid) = fid_str.parse::<u32>() {
                                                if let Some(font_name) = data.fonts.get(&fid) {
                                                    if let Some(fmt) = data.char_props.get_mut(&last_id) {
                                                        fmt.font = Some(font_name.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        b"paraPr" => {
                            let mut id = None;
                            let mut def = ParaStyleDef::default();
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    b"align" => {
                                        if let Ok(a) = attr.unescape_value() {
                                            def.align = parse_align(&a);
                                        }
                                    }
                                    b"headingType" => {
                                        if let Ok(h) = attr.unescape_value() {
                                            if h.eq_ignore_ascii_case("outline") {
                                                def.kind = Kind::Heading(1);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(id) = id {
                                current_para_pr_id = Some(id);
                                data.para_props.insert(id, def);
                            }
                        }
                        b"margin" => {
                            if let Some(para_id) = current_para_pr_id {
                                if let Some(def) = data.para_props.get_mut(&para_id) {
                                    for attr in e.attributes().flatten() {
                                        let key = attr.key.local_name();
                                        if let Ok(v) = attr.unescape_value() {
                                            if let Ok(hwpu) = v.parse::<i64>() {
                                                let pt = hwpu_to_pt(hwpu);
                                                match key.as_ref() {
                                                    b"left" => def.spacing.left = Some(pt),
                                                    b"right" => def.spacing.right = Some(pt),
                                                    b"indent" => def.spacing.first = Some(pt),
                                                    b"prev" => def.spacing.before = Some(pt),
                                                    b"next" => def.spacing.after = Some(pt),
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        b"borderFill" => {
                            let mut id = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok());
                                }
                            }
                            current_border_fill_id = id;
                            if let Some(id) = id {
                                data.border_fills.entry(id).or_insert(None);
                            }
                        }
                        b"winBrush" => {
                            if let Some(bf_id) = current_border_fill_id {
                                for attr in e.attributes().flatten() {
                                    if attr.key.local_name().as_ref() == b"faceColor" {
                                        if let Ok(color_str) = attr.unescape_value() {
                                            if let Some(color) = parse_color(&color_str) {
                                                data.border_fills.insert(bf_id, Some(color));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        b"style" => {
                            let mut id = None;
                            let mut para_pr_id = None;
                            let mut char_pr_id = None;
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"id" => id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    b"paraPrIDRef" => para_pr_id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    b"charPrIDRef" => char_pr_id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                                    _ => {}
                                }
                            }
                            if let Some(id) = id {
                                data.styles.insert(id, (para_pr_id, char_pr_id));
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(ref e) => {
                    let local = e.local_name();
                    if local.as_ref() == b"paraPr" {
                        current_para_pr_id = None;
                    } else if local.as_ref() == b"borderFill" {
                        current_border_fill_id = None;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        data
    }
}
