//! Unit and value conversion for HWPX documents.

use wordcraft_doc::{Align, Rgb};

/// 1 pt = 100 HWPUNIT (1 inch = 72 pt = 7200 HWPUNIT).
pub fn hwpu_to_pt(hwpu: i64) -> f32 {
    hwpu as f32 / 100.0
}

/// Points as HWPUNIT, rounded.
pub fn pt_to_hwpu(pt: f32) -> i64 {
    (pt * 100.0).round() as i64
}

/// A colour attribute: `#RRGGBB` (`#AARRGGBB` keeps its RGB); `none` and anything else is none.
pub fn parse_color(s: &str) -> Option<Rgb> {
    let hex = s.trim().strip_prefix('#')?;
    let hex = match hex.len() {
        6 => hex,
        8 => hex.get(2..)?,
        _ => return None,
    };
    let byte = |i: usize| hex.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok());
    Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
}

/// `#RRGGBB` for `c`.
pub fn color_hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// HWPX paragraph alignment (`hh:align horizontal`).
pub fn parse_align(s: &str) -> Option<Align> {
    match s.trim().to_ascii_uppercase().as_str() {
        "LEFT" => Some(Align::Left),
        "CENTER" => Some(Align::Center),
        "RIGHT" => Some(Align::Right),
        "JUSTIFY" => Some(Align::Justify),
        "DISTRIBUTE" | "DISTRIBUTE_SPACE" => Some(Align::Distribute),
        _ => None,
    }
}

/// The `hh:align horizontal` value for `a`.
pub fn align_name(a: Option<Align>) -> &'static str {
    match a {
        None | Some(Align::Left) => "LEFT",
        Some(Align::Right) => "RIGHT",
        Some(Align::Center) => "CENTER",
        Some(Align::Distribute) => "DISTRIBUTE",
        Some(Align::Justify) => "JUSTIFY",
    }
}
