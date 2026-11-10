//! BodyText stream parsing for HWP 5.x files.

use wordcraft_doc::SectionProps;

use crate::model::{FBlock, Fmt, Para, clean_text};
use super::header::DocInfo;
use super::record::{RecordReader, tags};

pub struct SectionParser<'a> {
    doc_info: &'a DocInfo,
}

impl<'a> SectionParser<'a> {
    pub fn new(doc_info: &'a DocInfo) -> Self {
        SectionParser { doc_info }
    }

    pub fn parse_section(&self, stream_bytes: &[u8], page: &mut SectionProps) -> Vec<FBlock> {
        let mut blocks = Vec::new();
        let mut reader = RecordReader::new(stream_bytes);

        let mut current_para: Option<Para> = None;
        let mut current_text: Vec<u16> = Vec::new();
        let mut char_shapes_map: Vec<(usize, usize)> = Vec::new();

        while let Some(record) = reader.next_record() {
            match record.tag_id {
                tags::PARA_HEADER => {
                    // Flush previous paragraph if any
                    if let Some(mut p) = current_para.take() {
                        self.assemble_para_runs(&mut p, &current_text, &char_shapes_map);
                        if !p.is_empty() {
                            blocks.push(FBlock::Para(p));
                        }
                    }
                    current_text.clear();
                    char_shapes_map.clear();

                    let mut p = Para::default();
                    if record.data.len() >= 12 {
                        let para_shape_id = u16::from_le_bytes([record.data[8], record.data[9]]) as usize;
                        if let Some((align, spacing, kind)) = self.doc_info.para_shapes.get(para_shape_id) {
                            p.align = *align;
                            p.spacing = *spacing;
                            p.kind = *kind;
                        }

                        // Page break flag (bit 1 of byte 11)
                        let break_flags = record.data[11];
                        if (break_flags & 0x02) != 0 {
                            p.page_break = true;
                        }
                    }
                    current_para = Some(p);
                }
                tags::PARA_TEXT => {
                    // Raw UTF-16LE characters
                    current_text = record
                        .data
                        .chunks_exact(2)
                        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                        .collect();
                }
                tags::PARA_CHAR_SHAPE => {
                    // Array of (pos: u32, shape_id: u32) pairs
                    char_shapes_map.clear();
                    for chunk in record.data.chunks_exact(8) {
                        let pos = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
                        let shape_id = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as usize;
                        char_shapes_map.push((pos, shape_id));
                    }
                }
                tags::PAGE_DEF => {
                    // Extract page width, height and margins
                    if record.data.len() >= 24 {
                        let w_hwpu = u32::from_le_bytes([record.data[0], record.data[1], record.data[2], record.data[3]]);
                        let h_hwpu = u32::from_le_bytes([record.data[4], record.data[5], record.data[6], record.data[7]]);
                        if w_hwpu > 0 && h_hwpu > 0 {
                            page.page_w = w_hwpu as f32 / 100.0;
                            page.page_h = h_hwpu as f32 / 100.0;
                        }

                        let left = u32::from_le_bytes([record.data[8], record.data[9], record.data[10], record.data[11]]);
                        let right = u32::from_le_bytes([record.data[12], record.data[13], record.data[14], record.data[15]]);
                        let top = u32::from_le_bytes([record.data[16], record.data[17], record.data[18], record.data[19]]);
                        let bottom = u32::from_le_bytes([record.data[20], record.data[21], record.data[22], record.data[23]]);

                        page.margin_left = left as f32 / 100.0;
                        page.margin_right = right as f32 / 100.0;
                        page.margin_top = top as f32 / 100.0;
                        page.margin_bottom = bottom as f32 / 100.0;
                    }
                }
                _ => {}
            }
        }

        // Flush last paragraph
        if let Some(mut p) = current_para.take() {
            self.assemble_para_runs(&mut p, &current_text, &char_shapes_map);
            if !p.is_empty() {
                blocks.push(FBlock::Para(p));
            }
        }

        blocks
    }

    /// Assemble UTF-16 characters and char shape mappings into runs of text inside the paragraph.
    fn assemble_para_runs(&self, para: &mut Para, raw_u16: &[u16], shapes_map: &[(usize, usize)]) {
        if raw_u16.is_empty() {
            return;
        }

        // Filter out inline/extended control codes and extract clean characters
        let mut filtered = Vec::with_capacity(raw_u16.len());
        let mut idx = 0;
        while idx < raw_u16.len() {
            let c = raw_u16[idx];
            match c {
                // Extended controls (8 UTF-16 units = 16 bytes)
                1 | 2 | 3 | 11 | 12 | 14 | 15 | 16 | 17 | 18 | 21 | 22 | 23 => {
                    idx = (idx + 8).min(raw_u16.len());
                }
                // Inline controls (8 UTF-16 units = 16 bytes)
                4 | 5 | 6 | 7 | 8 | 19 | 20 => {
                    idx = (idx + 8).min(raw_u16.len());
                }
                0x09 | 0x0A => {
                    filtered.push(c);
                    idx += 1;
                }
                0x0D => {
                    // Carriage return
                    idx += 1;
                }
                c if c >= 0x20 => {
                    filtered.push(c);
                    idx += 1;
                }
                _ => {
                    idx += 1;
                }
            }
        }

        if filtered.is_empty() {
            return;
        }

        if shapes_map.is_empty() {
            let s = String::from_utf16_lossy(&filtered);
            para.push_text(&clean_text(&s), &Fmt::default());
            return;
        }

        // Iterate over char shape spans
        for i in 0..shapes_map.len() {
            let (start_pos, shape_id) = shapes_map[i];
            let end_pos = if i + 1 < shapes_map.len() {
                shapes_map[i + 1].0
            } else {
                filtered.len()
            };

            let start = start_pos.min(filtered.len());
            let end = end_pos.min(filtered.len());

            if start < end {
                let slice = &filtered[start..end];
                let s = String::from_utf16_lossy(slice);
                let fmt = self
                    .doc_info
                    .char_shapes
                    .get(shape_id)
                    .cloned()
                    .unwrap_or_default();
                para.push_text(&clean_text(&s), &fmt);
            }
        }
    }
}
