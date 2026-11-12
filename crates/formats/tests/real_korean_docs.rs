//! Real Hangul documents, read from the folder `WORDCRAFT_HWP_SAMPLES` names (none ship with the
//! repository; the test passes without it). Each `.hwp` and `.hwpx` file there imports, and the
//! words of the preview text Hangul saved inside it (`PrvText`) are in the imported text.

use std::io::{Cursor, Read};
use std::path::Path;

use wordcraft_doc::StoryRef;

/// The preview text Hangul stores with a document: `Preview/PrvText.txt` (UTF-8) in HWPX, the
/// `PrvText` stream (UTF-16LE) in HWP 5.x.
fn preview(ext: &str, bytes: &[u8]) -> Option<String> {
    let mut raw = Vec::new();
    if ext == "hwpx" {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        zip.by_name("Preview/PrvText.txt").ok()?.read_to_end(&mut raw).ok()?;
        Some(String::from_utf8_lossy(&raw).into_owned())
    } else {
        let mut comp = cfb::CompoundFile::open(Cursor::new(bytes)).ok()?;
        comp.open_stream("/PrvText").ok()?.read_to_end(&mut raw).ok()?;
        let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        Some(String::from_utf16_lossy(&units))
    }
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(str::to_string).collect()
}

#[test]
fn sample_documents_import_with_their_preview_text() {
    let Some(dir) = std::env::var_os("WORDCRAFT_HWP_SAMPLES") else {
        eprintln!("WORDCRAFT_HWP_SAMPLES is not set: no real documents checked");
        return;
    };
    let mut entries: Vec<_> = std::fs::read_dir(Path::new(&dir)).expect("sample folder").flatten().map(|e| e.path()).collect();
    entries.sort();
    let mut checked = 0;
    for path in entries {
        let Some(ext) = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase) else { continue };
        if ext != "hwp" && ext != "hwpx" {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bytes = std::fs::read(&path).expect("read sample");
        if bytes.is_empty() {
            eprintln!("{name}: empty file, skipped");
            continue;
        }
        let doc = match wordcraft_formats::import(&ext, &bytes).expect("hwp/hwpx are ours") {
            Ok(doc) => doc,
            Err(e) => panic!("{name}: {e}"),
        };
        let text = doc.plain_text(StoryRef::Body);
        let expected = preview(&ext, &bytes).map(|p| words(&p)).unwrap_or_default();
        let found = expected.iter().filter(|w| text.contains(w.as_str())).count();
        let missing: Vec<&String> = expected.iter().filter(|w| !text.contains(w.as_str())).take(10).collect();
        eprintln!("{name}: {} chars, {found}/{} preview words, missing {missing:?}", text.chars().count(), expected.len());
        assert!(!text.trim().is_empty(), "{name}: no text");
        assert!(found * 10 >= expected.len() * 9, "{name}: only {found} of {} preview words found", expected.len());
        // Written as HWPX and read back, the text stays the same.
        let hwpx = wordcraft_formats::export("hwpx", &doc).expect("hwpx is ours").unwrap_or_else(|e| panic!("{name}: export: {e}"));
        let again = wordcraft_formats::import("hwpx", &hwpx).expect("hwpx is ours").unwrap_or_else(|e| panic!("{name}: re-import: {e}"));
        let back = again.plain_text(StoryRef::Body);
        if back != text {
            let at = back.chars().zip(text.chars()).take_while(|(a, b)| a == b).count();
            let around = |s: &str| s.chars().skip(at.saturating_sub(40)).take(80).collect::<String>();
            panic!("{name}: text changed through HWPX at char {at}:\n was {:?}\n now {:?}", around(&text), around(&back));
        }
        checked += 1;
    }
    eprintln!("{checked} documents checked");
}
