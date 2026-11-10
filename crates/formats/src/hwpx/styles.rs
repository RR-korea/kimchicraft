//! Style parsing and unit conversion for HWPX documents.

use wordcraft_doc::{Align, Rgb};

/// 1 pt = 100 HWPU (1 inch = 72 pt = 7200 HWPU).
pub fn hwpu_to_pt(hwpu: i64) -> f32 {
    hwpu as f32 / 100.0
}

/// Parse a color string from HWPX attributes (hex `#RRGGBB`, `0xRRGGBB`, or decimal).
pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#').or_else(|| s.strip_prefix("0x")) {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Rgb(r, g, b));
        } else if hex.len() == 8 {
            // ARGB or RGBA - take the RGB portion
            let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(Rgb(r, g, b));
        }
    }
    // Try decimal integer
    if let Ok(num) = s.parse::<u32>() {
        let r = ((num >> 16) & 0xFF) as u8;
        let g = ((num >> 8) & 0xFF) as u8;
        let b = (num & 0xFF) as u8;
        return Some(Rgb(r, g, b));
    }
    None
}

/// Map HWPX paragraph alignment strings to WordCraft Align enum.
pub fn parse_align(s: &str) -> Option<Align> {
    match s.trim().to_ascii_uppercase().as_str() {
        "LEFT" => Some(Align::Left),
        "CENTER" => Some(Align::Center),
        "RIGHT" => Some(Align::Right),
        "JUSTIFY" => Some(Align::Justify),
        "DISTRIBUTE" => Some(Align::Distribute),
        _ => None,
    }
}
