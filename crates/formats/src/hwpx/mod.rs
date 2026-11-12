//! HWPX (Hancom Office HWPX, KS X 6101) document import and export.

use wordcraft_doc::{Document, SectionProps};

use crate::hwp_doc;
use crate::model;

pub mod export;
mod header;
mod package;
mod section;
pub mod styles;
mod xml;

#[cfg(test)]
mod tests;

pub use export::export;
pub use package::Package;

/// Import `bytes` of an HWPX package into a [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import `bytes` of an HWPX package onto `page` properties.
pub fn import_on(bytes: &[u8], page: &SectionProps) -> Result<Document, String> {
    model::begin_import();
    let sections = parse(bytes)?;
    Ok(hwp_doc::to_doc(sections, page))
}

/// The sections of an HWPX package.
pub(crate) fn parse(bytes: &[u8]) -> Result<Vec<hwp_doc::Section>, String> {
    let mut package = Package::open(bytes)?;
    let header = package.header_xml().map(|xml| header::HeaderData::parse(&xml)).unwrap_or_default();
    let paths = package.section_paths().to_vec();
    if paths.is_empty() {
        return Err("No sections found in HWPX package".to_string());
    }
    let mut sections = Vec::new();
    for path in paths {
        if let Some(xml) = package.read_entry(&path) {
            sections.push(section::SectionParser::new(&header, &mut package).parse(&xml));
        }
    }
    if sections.is_empty() {
        return Err("No readable sections in HWPX package".to_string());
    }
    Ok(sections)
}
