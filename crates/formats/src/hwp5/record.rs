//! HWP 5.x records: the 32-bit header (tag, level, size) and the tree their levels make.

/// Record tags: HWPTAG_BEGIN (0x10) plus the index the HWP 5.0 specification gives each.
pub mod tags {
    pub const ID_MAPPINGS: u16 = 0x11;
    pub const BIN_DATA: u16 = 0x12;
    pub const FACE_NAME: u16 = 0x13;
    pub const CHAR_SHAPE: u16 = 0x15;
    pub const BULLET: u16 = 0x18;
    pub const PARA_SHAPE: u16 = 0x19;
    pub const STYLE: u16 = 0x1A;
    pub const PARA_HEADER: u16 = 0x42;
    pub const PARA_TEXT: u16 = 0x43;
    pub const PARA_CHAR_SHAPE: u16 = 0x44;
    pub const CTRL_HEADER: u16 = 0x47;
    pub const LIST_HEADER: u16 = 0x48;
    pub const PAGE_DEF: u16 = 0x49;
    pub const TABLE: u16 = 0x4D;
    pub const SHAPE_PICTURE: u16 = 0x55;
}

/// Deepest record nesting kept (hostile files); deeper records are left out.
const MAX_LEVEL: usize = 64;

/// One record: its tag, level and payload (cut short when the stream ends early).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record<'a> {
    pub tag: u16,
    pub level: u16,
    pub data: &'a [u8],
}

/// The records of a decompressed stream, in order.
pub struct RecordReader<'a> {
    stream: &'a [u8],
    offset: usize,
}

impl<'a> RecordReader<'a> {
    pub fn new(stream: &'a [u8]) -> Self {
        RecordReader { stream, offset: 0 }
    }
}

impl<'a> Iterator for RecordReader<'a> {
    type Item = Record<'a>;

    fn next(&mut self) -> Option<Record<'a>> {
        let header = u32_at(self.stream, self.offset)?;
        self.offset += 4;
        let tag = (header & 0x3FF) as u16;
        let level = ((header >> 10) & 0x3FF) as u16;
        let mut size = (header >> 20) as usize;
        // 0xFFF: the size follows as a 32-bit value.
        if size == 0xFFF {
            size = u32_at(self.stream, self.offset)? as usize;
            self.offset += 4;
        }
        let end = self.offset.saturating_add(size).min(self.stream.len());
        let data = self.stream.get(self.offset..end).unwrap_or_default();
        self.offset = end;
        Some(Record { tag, level, data })
    }
}

/// A record and the records under it (those after it one level deeper).
#[derive(Clone, Debug, PartialEq)]
pub struct Node<'a> {
    pub rec: Record<'a>,
    pub children: Vec<Node<'a>>,
}

impl<'a> Node<'a> {
    /// The first child with `tag`.
    pub fn child(&self, tag: u16) -> Option<&Node<'a>> {
        self.children.iter().find(|n| n.rec.tag == tag)
    }
}

/// The records of `stream` as a tree. A record deeper than the one before it by more than one
/// level is taken as its child all the same.
pub fn tree(stream: &[u8]) -> Vec<Node<'_>> {
    let mut roots = Vec::new();
    let mut open: Vec<Node> = Vec::new();
    for rec in RecordReader::new(stream) {
        while open.last().is_some_and(|n| n.rec.level >= rec.level) {
            close(&mut open, &mut roots);
        }
        if open.len() >= MAX_LEVEL {
            continue;
        }
        open.push(Node { rec, children: Vec::new() });
    }
    while !open.is_empty() {
        close(&mut open, &mut roots);
    }
    roots
}

fn close<'a>(open: &mut Vec<Node<'a>>, roots: &mut Vec<Node<'a>>) {
    if let Some(n) = open.pop() {
        match open.last_mut() {
            Some(parent) => parent.children.push(n),
            None => roots.push(n),
        }
    }
}

/// `d` as little-endian UTF-16 units (a trailing odd byte is left out).
pub fn utf16_units(d: &[u8]) -> Vec<u16> {
    d.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect()
}

pub fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

pub fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub fn i32_at(d: &[u8], at: usize) -> Option<i32> {
    u32_at(d, at).map(|v| v as i32)
}
