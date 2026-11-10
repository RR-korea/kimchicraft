//! HWPX ZIP container and resource management.

use std::io::{Cursor, Read};
use zip::ZipArchive;

use crate::MAX_ZIP_ENTRIES;

pub struct Package<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    section_paths: Vec<String>,
    header_path: Option<String>,
}

impl<'a> Package<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, String> {
        let cursor = Cursor::new(bytes);
        let mut zip = ZipArchive::new(cursor).map_err(|e| format!("HWPX zip error: {e}"))?;

        if zip.len() as u64 > MAX_ZIP_ENTRIES {
            return Err("HWPX package exceeds maximum zip entries".to_string());
        }

        let mut header_path = None;
        let mut section_paths = Vec::new();

        for i in 0..zip.len() {
            if let Ok(file) = zip.by_index(i) {
                let name = file.name().to_string();
                if name.eq_ignore_ascii_case("Contents/header.xml") || name.ends_with("/header.xml") {
                    header_path = Some(name);
                } else if name.starts_with("Contents/section") && name.ends_with(".xml") {
                    section_paths.push(name);
                }
            }
        }

        // Sort section paths (e.g., section0.xml, section1.xml)
        section_paths.sort();

        // Fallback: if no section paths found, look for any .xml in Contents/
        if section_paths.is_empty() {
            for i in 0..zip.len() {
                if let Ok(file) = zip.by_index(i) {
                    let name = file.name().to_string();
                    if name.ends_with(".xml") && !name.ends_with("header.xml") && !name.ends_with("content.hpf") && !name.ends_with("container.xml") {
                        section_paths.push(name);
                    }
                }
            }
            section_paths.sort();
        }

        Ok(Package {
            zip,
            section_paths,
            header_path,
        })
    }

    pub fn header_xml(&mut self) -> Option<Vec<u8>> {
        let path = self.header_path.clone()?;
        self.read_entry(&path)
    }

    pub fn section_paths(&self) -> &[String] {
        &self.section_paths
    }

    pub fn read_entry(&mut self, path: &str) -> Option<Vec<u8>> {
        let mut file = self.zip.by_name(path).ok()?;
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    pub fn read_image(&mut self, id: &str) -> Option<(Vec<u8>, String)> {
        // BinData/image1.png or matching ID
        let candidates = [
            format!("BinData/{id}"),
            format!("BinData/{id}.png"),
            format!("BinData/{id}.jpg"),
            format!("BinData/{id}.jpeg"),
            format!("Contents/BinData/{id}"),
        ];
        for cand in &candidates {
            if let Some(bytes) = self.read_entry(cand) {
                let ext = cand.rsplit('.').next().unwrap_or("png").to_string();
                return Some((bytes, ext));
            }
        }
        None
    }
}
