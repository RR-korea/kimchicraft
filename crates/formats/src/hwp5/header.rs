//! FileHeader and DocInfo stream parsing for HWP 5.x files.

use std::sync::Arc;
use wordcraft_doc::{Align, Rgb};

use crate::model::{Fmt, Kind, Spacing, kept_font};
use super::record::{RecordReader, tags};

pub const HWP5_SIGNATURE: &[u8] = b"HWP Document File";

/// The 256-byte fixed FileHeader stream of an HWP 5.x file.
#[derive(Clone, Debug, PartialEq)]
pub struct FileHeader {
    pub version: u32,
    pub is_compressed: bool,
    pub is_encrypted: bool,
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

        let version = u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]);
        let flags = u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]);

        let is_compressed = (flags & 0x01) != 0;
        let is_encrypted = (flags & 0x02) != 0;
        let is_distribution = (flags & 0x04) != 0;

        Ok(FileHeader {
            version,
            is_compressed,
            is_encrypted,
            is_distribution,
        })
    }
}

/// Information extracted from the `DocInfo` stream.
#[derive(Clone, Debug, Default)]
pub struct DocInfo {
    pub font_faces: Vec<Arc<str>>,
    pub char_shapes: Vec<Fmt>,
    pub para_shapes: Vec<(Option<Align>, Spacing, Kind)>,
}

impl DocInfo {
    pub fn parse(stream_bytes: &[u8]) -> Self {
        let mut info = DocInfo::default();
        let mut reader = RecordReader::new(stream_bytes);

        while let Some(record) = reader.next_record() {
            match record.tag_id {
                tags::FACE_NAME => {
                    if record.data.len() >= 2 {
                        // Skip property byte, rest is UTF-16LE string with length prefix
                        let name_bytes = &record.data[2..];
                        let u16_chars: Vec<u16> = name_bytes
                            .chunks_exact(2)
                            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                            .take_while(|&c| c != 0)
                            .collect();
                        let name_str = String::from_utf16_lossy(&u16_chars);
                        if let Some(font_arc) = kept_font(&name_str) {
                            info.font_faces.push(font_arc);
                        } else {
                            info.font_faces.push(Arc::from("맑은 고딕"));
                        }
                    }
                }
                tags::CHAR_SHAPE => {
                    if record.data.len() >= 72 {
                        let mut fmt = Fmt::default();

                        // Font ID (Hangul font at bytes 0..2)
                        let hangul_font_id = u16::from_le_bytes([record.data[0], record.data[1]]) as usize;
                        if let Some(font_name) = info.font_faces.get(hangul_font_id) {
                            fmt.font = Some(font_name.clone());
                        }

                        // Base font size (DWORD at bytes 56..60 in HWPU: 1000 = 10pt)
                        let size_hwpu = u32::from_le_bytes([
                            record.data[56],
                            record.data[57],
                            record.data[58],
                            record.data[59],
                        ]);
                        fmt.size = Some(size_hwpu as f32 / 100.0);

                        // Attributes (DWORD at bytes 60..64)
                        let attr = u32::from_le_bytes([
                            record.data[60],
                            record.data[61],
                            record.data[62],
                            record.data[63],
                        ]);
                        fmt.bold = (attr & 0x01) != 0;
                        fmt.italic = (attr & 0x02) != 0;
                        fmt.underline = ((attr >> 2) & 0x03) != 0;
                        fmt.strike = ((attr >> 18) & 0x07) != 0;

                        // Text color (COLORREF 0x00BBGGRR at bytes 68..72)
                        let r = record.data[68];
                        let g = record.data[69];
                        let b = record.data[70];
                        if !(r == 0 && g == 0 && b == 0) {
                            fmt.color = Some(Rgb(r, g, b));
                        }

                        info.char_shapes.push(fmt);
                    }
                }
                tags::PARA_SHAPE => {
                    if record.data.len() >= 24 {
                        let attr = u32::from_le_bytes([
                            record.data[0],
                            record.data[1],
                            record.data[2],
                            record.data[3],
                        ]);

                        let align_val = (attr >> 2) & 0x07;
                        let align = match align_val {
                            0 => Some(Align::Justify),
                            1 => Some(Align::Left),
                            2 => Some(Align::Right),
                            3 => Some(Align::Center),
                            4 => Some(Align::Distribute),
                            _ => None,
                        };

                        let left_hwpu = i32::from_le_bytes([
                            record.data[4],
                            record.data[5],
                            record.data[6],
                            record.data[7],
                        ]);
                        let right_hwpu = i32::from_le_bytes([
                            record.data[8],
                            record.data[9],
                            record.data[10],
                            record.data[11],
                        ]);
                        let indent_hwpu = i32::from_le_bytes([
                            record.data[12],
                            record.data[13],
                            record.data[14],
                            record.data[15],
                        ]);
                        let before_hwpu = i32::from_le_bytes([
                            record.data[16],
                            record.data[17],
                            record.data[18],
                            record.data[19],
                        ]);
                        let after_hwpu = i32::from_le_bytes([
                            record.data[20],
                            record.data[21],
                            record.data[22],
                            record.data[23],
                        ]);

                        let spacing = Spacing {
                            left: if left_hwpu != 0 { Some(left_hwpu as f32 / 100.0) } else { None },
                            right: if right_hwpu != 0 { Some(right_hwpu as f32 / 100.0) } else { None },
                            first: if indent_hwpu != 0 { Some(indent_hwpu as f32 / 100.0) } else { None },
                            before: if before_hwpu != 0 { Some(before_hwpu as f32 / 100.0) } else { None },
                            after: if after_hwpu != 0 { Some(after_hwpu as f32 / 100.0) } else { None },
                            ..Default::default()
                        };

                        info.para_shapes.push((align, spacing, Kind::Normal));
                    }
                }
                _ => {}
            }
        }

        info
    }
}
