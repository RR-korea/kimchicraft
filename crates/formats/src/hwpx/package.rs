//! HWPX ZIP container: the manifest (`Contents/content.hpf`), its sections in reading order and
//! its binary items (pictures).

use std::collections::HashMap;
use std::io::{Cursor, Read};

use zip::ZipArchive;

use super::xml;
use crate::MAX_ZIP_ENTRIES;
use crate::hwp_doc::{MAX_PART, MAX_SECTIONS};

pub struct Package<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    section_paths: Vec<String>,
    header_path: Option<String>,
    /// Manifest item id → path in the zip.
    items: HashMap<String, String>,
}

impl<'a> Package<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, String> {
        let zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("HWPX zip error: {e}"))?;
        if zip.len() as u64 > MAX_ZIP_ENTRIES {
            return Err("HWPX package exceeds maximum zip entries".to_string());
        }
        let mut package = Package { zip, section_paths: Vec::new(), header_path: None, items: HashMap::new() };

        // The manifest lists every part; its spine gives the sections in reading order.
        if let Some(root) = package.read_entry("Contents/content.hpf").as_deref().and_then(xml::parse) {
            let mut items = Vec::new();
            root.find_all("item", &mut items);
            for item in items {
                if let (Some(id), Some(href)) = (item.attr("id"), item.attr("href")) {
                    package.items.insert(id.to_string(), href.trim_start_matches('/').to_string());
                }
            }
            let mut refs = Vec::new();
            root.find_all("itemref", &mut refs);
            for r in refs.iter().take(MAX_SECTIONS) {
                if let Some(path) = r.attr("idref").and_then(|id| package.items.get(id))
                    && is_section(path)
                {
                    package.section_paths.push(path.clone());
                }
            }
            package.header_path = package.items.get("header").cloned();
        }

        // Without a usable manifest: every Contents/section*.xml, in number order.
        if package.section_paths.is_empty() {
            let mut found: Vec<(u32, String)> = package
                .zip
                .file_names()
                .filter(|n| is_section(n))
                .filter_map(|n| {
                    let num = n.strip_prefix("Contents/section")?.strip_suffix(".xml")?.parse().ok()?;
                    Some((num, n.to_string()))
                })
                .collect();
            found.sort();
            package.section_paths = found.into_iter().take(MAX_SECTIONS).map(|(_, n)| n).collect();
        }
        if package.header_path.is_none() && package.zip.file_names().any(|n| n == "Contents/header.xml") {
            package.header_path = Some("Contents/header.xml".to_string());
        }
        Ok(package)
    }

    pub fn header_xml(&mut self) -> Option<Vec<u8>> {
        let path = self.header_path.clone()?;
        self.read_entry(&path)
    }

    pub fn section_paths(&self) -> &[String] {
        &self.section_paths
    }

    /// The bytes of `path`, up to [`MAX_PART`] (a hostile size field does not set the allocation).
    pub fn read_entry(&mut self, path: &str) -> Option<Vec<u8>> {
        let file = self.zip.by_name(path).ok()?;
        let mut bytes = Vec::new();
        file.take(MAX_PART as u64).read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    /// The bytes of the binary item `id` (a picture), from the manifest or the usual `BinData/` path.
    pub fn read_image(&mut self, id: &str) -> Option<Vec<u8>> {
        if let Some(path) = self.items.get(id).cloned()
            && let Some(bytes) = self.read_entry(&path)
        {
            return Some(bytes);
        }
        let prefix = format!("BinData/{id}.");
        let path = self.zip.file_names().find(|n| n.starts_with(&prefix))?.to_string();
        self.read_entry(&path)
    }
}

fn is_section(path: &str) -> bool {
    path.starts_with("Contents/section") && path.ends_with(".xml")
}
