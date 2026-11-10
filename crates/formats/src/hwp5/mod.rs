//! Legacy HWP 5.x binary format (OLE2 Compound File) import.

use std::io::{Cursor, Read};
use cfb::CompoundFile;
use wordcraft_doc::{Document, SectionProps};

use crate::model::{self, Flow};

mod header;
pub mod record;
mod section;

#[cfg(test)]
mod tests;

pub use header::{DocInfo, FileHeader, HWP5_SIGNATURE};
pub use section::SectionParser;

/// Import `bytes` of an HWP 5.x binary file into a [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import `bytes` of an HWP 5.x binary file onto `page` properties.
pub fn import_on(bytes: &[u8], page: &SectionProps) -> Result<Document, String> {
    model::begin_import();
    let (flow, section_props) = parse(bytes, page)?;
    let mut doc = model::to_doc_in(&flow, section_props);
    doc.ensure_nonempty();
    Ok(doc)
}

/// Parse an HWP 5.x binary stream into a [`Flow`] and [`SectionProps`].
pub(crate) fn parse(bytes: &[u8], default_page: &SectionProps) -> Result<(Flow, SectionProps), String> {
    let cursor = Cursor::new(bytes);
    let mut comp = CompoundFile::open(cursor).map_err(|e| format!("CFB open error: {e}"))?;

    // 1. Read FileHeader stream
    let file_header_bytes = read_stream(&mut comp, "/FileHeader")?;
    let file_header = FileHeader::parse(&file_header_bytes)?;

    if file_header.is_encrypted {
        return Err("Password-protected HWP files are not supported".to_string());
    }

    // 2. Read DocInfo stream
    let doc_info_raw = read_stream(&mut comp, "/DocInfo")?;
    let doc_info_bytes = if file_header.is_compressed {
        decompress_stream(&doc_info_raw)?
    } else {
        doc_info_raw
    };
    let doc_info = DocInfo::parse(&doc_info_bytes);

    let mut flow = Flow::default();
    let mut section_props = default_page.clone();

    // 3. Read BodyText/Section* streams
    let mut section_idx = 0;
    loop {
        let stream_path = format!("/BodyText/Section{section_idx}");
        let Ok(raw_section) = read_stream(&mut comp, &stream_path) else {
            break;
        };

        let section_bytes = if file_header.is_compressed {
            decompress_stream(&raw_section)?
        } else {
            raw_section
        };

        let parser = SectionParser::new(&doc_info);
        let blocks = parser.parse_section(&section_bytes, &mut section_props);
        flow.blocks.extend(blocks);

        section_idx += 1;
        if section_idx > 100 {
            // Safety bound against corrupted section sequences
            break;
        }
    }

    if flow.blocks.is_empty() {
        return Err("No text sections found in HWP document".to_string());
    }

    Ok((flow, section_props))
}

/// Helper to read an entire stream from a CompoundFile.
fn read_stream(comp: &mut CompoundFile<Cursor<&[u8]>>, path: &str) -> Result<Vec<u8>, String> {
    let mut stream = comp.open_stream(path).map_err(|e| format!("Stream '{path}' not found: {e}"))?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|e| format!("Read stream '{path}' failed: {e}"))?;
    Ok(buf)
}

/// Decompress raw deflate or zlib stream data.
fn decompress_stream(data: &[u8]) -> Result<Vec<u8>, String> {
    if let Ok(vec) = miniz_oxide::inflate::decompress_to_vec(data) {
        return Ok(vec);
    }
    if let Ok(vec) = miniz_oxide::inflate::decompress_to_vec_zlib(data) {
        return Ok(vec);
    }
    Err("HWP stream decompression failed".to_string())
}
