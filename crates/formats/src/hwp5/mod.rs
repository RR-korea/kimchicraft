//! Legacy HWP 5.x binary format (OLE2 Compound File) import.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use cfb::CompoundFile;
use wordcraft_doc::{Document, SectionProps};

use crate::hwp_doc::{self, MAX_PART, MAX_SECTIONS};
use crate::model;

mod header;
pub mod record;
mod section;

#[cfg(test)]
mod tests;

pub use header::{DocInfo, FileHeader, HWP5_SIGNATURE};
pub use section::SectionParser;

/// Most bytes of embedded pictures read (hostile files); later pictures are left out.
const MAX_PICTURES: usize = 512 << 20;

/// Import `bytes` of an HWP 5.x binary file into a [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import `bytes` of an HWP 5.x binary file onto `page` properties.
pub fn import_on(bytes: &[u8], page: &SectionProps) -> Result<Document, String> {
    model::begin_import();
    let sections = parse(bytes)?;
    Ok(hwp_doc::to_doc(sections, page))
}

/// The sections of an HWP 5.x file.
pub(crate) fn parse(bytes: &[u8]) -> Result<Vec<hwp_doc::Section>, String> {
    let mut comp = CompoundFile::open(Cursor::new(bytes)).map_err(|e| format!("CFB open error: {e}"))?;

    let file_header = FileHeader::parse(&read_stream(&mut comp, "/FileHeader")?)?;
    if file_header.is_encrypted {
        return Err("Password-protected HWP files are not supported".to_string());
    }
    if file_header.is_distribution {
        return Err("HWP distribution documents (encrypted ViewText) are not supported".to_string());
    }
    let inflate = |raw: Vec<u8>| if file_header.is_compressed { decompress_stream(&raw) } else { Ok(raw) };

    let doc_info = DocInfo::parse(&inflate(read_stream(&mut comp, "/DocInfo")?)?);

    // Embedded pictures by the number pictures refer to them with (from 1), up to MAX_PICTURES bytes.
    let mut images: HashMap<usize, Arc<Vec<u8>>> = HashMap::new();
    let mut total = 0usize;
    for (i, item) in doc_info.bin_items.iter().enumerate() {
        let Some(item) = item else { continue };
        let Ok(raw) = read_stream(&mut comp, &format!("/BinData/BIN{:04X}.{}", item.id, item.ext)) else { continue };
        let data = if item.compressed.unwrap_or(file_header.is_compressed) { decompress_stream(&raw).unwrap_or(raw) } else { raw };
        total = total.saturating_add(data.len());
        if total > MAX_PICTURES {
            break;
        }
        images.insert(i + 1, Arc::new(data));
    }

    let mut sections = Vec::new();
    for i in 0..MAX_SECTIONS {
        let Ok(raw) = read_stream(&mut comp, &format!("/BodyText/Section{i}")) else { break };
        sections.push(SectionParser::new(&doc_info, &images).parse(&inflate(raw)?));
    }
    if sections.is_empty() {
        return Err("No text sections found in HWP document".to_string());
    }
    Ok(sections)
}

/// An entire stream of `comp`, up to [`MAX_PART`] bytes.
fn read_stream(comp: &mut CompoundFile<Cursor<&[u8]>>, path: &str) -> Result<Vec<u8>, String> {
    let stream = comp.open_stream(path).map_err(|e| format!("Stream '{path}' not found: {e}"))?;
    let mut buf = Vec::new();
    stream.take(MAX_PART as u64).read_to_end(&mut buf).map_err(|e| format!("Read stream '{path}' failed: {e}"))?;
    Ok(buf)
}

/// Inflate a stream: raw deflate (what HWP writes), else zlib, up to [`MAX_PART`] bytes.
fn decompress_stream(data: &[u8]) -> Result<Vec<u8>, String> {
    miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_PART)
        .or_else(|_| miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(data, MAX_PART))
        .map_err(|_| "HWP stream decompression failed".to_string())
}
