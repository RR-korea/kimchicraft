//! HWP 5.x hierarchical record structure and parsing.

/// A single HWP 5.x record.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub tag_id: u16,
    pub level: u16,
    pub size: usize,
    pub data: Vec<u8>,
}

pub mod tags {
    // DocInfo tags (HWPTAG_BEGIN = 0x10)
    pub const DOCUMENT_PROPERTIES: u16 = 0x10;
    pub const ID_MAPPINGS: u16 = 0x11;
    pub const BIN_DATA: u16 = 0x12;
    pub const FACE_NAME: u16 = 0x13;
    pub const BORDER_FILL: u16 = 0x14;
    pub const CHAR_SHAPE: u16 = 0x15;
    pub const TAB_DEF: u16 = 0x16;
    pub const NUMBERING: u16 = 0x17;
    pub const PARA_SHAPE: u16 = 0x18;
    pub const STYLE: u16 = 0x19;

    // BodyText tags (HWPTAG_BEGIN(0x10) + index)
    pub const PARA_HEADER: u16 = 0x42;      // 66 (0x10 + 50)
    pub const PARA_TEXT: u16 = 0x43;        // 67 (0x10 + 51)
    pub const PARA_CHAR_SHAPE: u16 = 0x44;  // 68 (0x10 + 52)
    pub const PARA_LINE_SEG: u16 = 0x45;    // 69 (0x10 + 53)
    pub const PARA_RANGE_TAG: u16 = 0x46;   // 70 (0x10 + 54)
    pub const CTRL_HEADER: u16 = 0x47;      // 71 (0x10 + 55)
    pub const LIST_HEADER: u16 = 0x48;      // 72 (0x10 + 56)
    pub const PAGE_DEF: u16 = 0x49;         // 73 (0x10 + 57)
    pub const FOOTNOTE_DEF: u16 = 0x4A;     // 74 (0x10 + 58)
    pub const PAGE_BORDER_FILL: u16 = 0x4B; // 75 (0x10 + 59)
    pub const SHAPE_COMPONENT: u16 = 0x4C;  // 76 (0x10 + 60)
    pub const TABLE: u16 = 0x4D;            // 77 (0x10 + 61)
}

/// Reads records sequentially from a decompressed HWP 5.x stream.
pub struct RecordReader<'a> {
    stream: &'a [u8],
    offset: usize,
}

impl<'a> RecordReader<'a> {
    pub fn new(stream: &'a [u8]) -> Self {
        RecordReader { stream, offset: 0 }
    }

    /// Read the next record in the stream.
    pub fn next_record(&mut self) -> Option<Record> {
        if self.offset + 4 > self.stream.len() {
            return None;
        }

        let header_bytes = &self.stream[self.offset..self.offset + 4];
        let header = u32::from_le_bytes([
            header_bytes[0],
            header_bytes[1],
            header_bytes[2],
            header_bytes[3],
        ]);
        self.offset += 4;

        let tag_id = (header & 0x3FF) as u16;          // Bits 0..9 (10 bits)
        let level = ((header >> 10) & 0x3FF) as u16;   // Bits 10..19 (10 bits)
        let mut size = ((header >> 20) & 0xFFF) as usize; // Bits 20..31 (12 bits)

        // If size is 0xFFF (4095), the actual size is stored in the next 4 bytes.
        if size == 0xFFF {
            if self.offset + 4 > self.stream.len() {
                return None;
            }
            let extended_bytes = &self.stream[self.offset..self.offset + 4];
            size = u32::from_le_bytes([
                extended_bytes[0],
                extended_bytes[1],
                extended_bytes[2],
                extended_bytes[3],
            ]) as usize;
            self.offset += 4;
        }

        if self.offset + size > self.stream.len() {
            // Guard against corrupted record payload lengths
            let available = self.stream.len().saturating_sub(self.offset);
            let payload = self.stream[self.offset..self.offset + available].to_vec();
            self.offset = self.stream.len();
            return Some(Record {
                tag_id,
                level,
                size: available,
                data: payload,
            });
        }

        let payload = self.stream[self.offset..self.offset + size].to_vec();
        self.offset += size;

        Some(Record {
            tag_id,
            level,
            size,
            data: payload,
        })
    }
}
