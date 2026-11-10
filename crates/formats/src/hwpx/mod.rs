//! HWPX (Hancom Office HWPX, KS X 6101) document import.

use wordcraft_doc::{Document, SectionProps};

use crate::model::{self, Flow};

mod header;
mod package;
mod section;
pub mod styles;

#[cfg(test)]
mod tests;

pub use package::Package;

/// Import `bytes` of an HWPX package into a [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import `bytes` of an HWPX package onto `page` properties.
pub fn import_on(bytes: &[u8], page: &SectionProps) -> Result<Document, String> {
    model::begin_import();
    let (flow, section_props) = parse(bytes, page)?;
    let mut doc = model::to_doc_in(&flow, section_props);
    doc.ensure_nonempty();
    Ok(doc)
}

/// Parse an HWPX package into a [`Flow`] and [`SectionProps`].
pub(crate) fn parse(bytes: &[u8], default_page: &SectionProps) -> Result<(Flow, SectionProps), String> {
    let mut package = Package::open(bytes)?;

    let header = if let Some(header_xml) = package.header_xml() {
        header::HeaderData::parse(&header_xml)
    } else {
        header::HeaderData::default()
    };

    let mut flow = Flow::default();
    let mut section_props = default_page.clone();

    let section_paths = package.section_paths().to_vec();
    if section_paths.is_empty() {
        return Err("No sections found in HWPX package".to_string());
    }

    for path in section_paths {
        if let Some(xml) = package.read_entry(&path) {
            let mut parser = section::SectionParser::new(&header, &mut package);
            let blocks = parser.parse_section(&xml, &mut section_props);
            flow.blocks.extend(blocks);
        }
    }

    Ok((flow, section_props))
}
