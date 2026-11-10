use std::fs;
use std::path::Path;
use wordcraft_doc::StoryRef;

#[test]
fn test_real_hwpx_parsing() {
    let path = Path::new(r"C:\Temp\PC\공고제안요청서소독_예찰_드론_및_관련_장비임차_2026102621시.hwpx");
    if !path.exists() {
        println!("HWPX test file not found at: {:?}", path);
        return;
    }

    let bytes = fs::read(path).expect("Failed to read HWPX test file");
    let result = wordcraft_formats::import("hwpx", &bytes);
    assert!(result.is_some(), "hwpx extension should be recognized");

    let doc = result.expect("hwpx import option").expect("hwpx parsing should succeed");
    let plain_text = doc.plain_text(StoryRef::Body);

    println!("=== Real HWPX Import Results ===");
    println!("File size: {} bytes", bytes.len());
    println!("Extracted text length: {} chars", plain_text.chars().count());
    println!("Preview (first 300 chars):\n{}", plain_text.chars().take(300).collect::<String>());

    assert!(!plain_text.trim().is_empty(), "Extracted plain text should not be empty");
    assert!(
        plain_text.contains("제안요청서") || plain_text.contains("드론") || plain_text.contains("소독"),
        "Extracted text should contain keywords from the document title"
    );
}

#[test]
fn test_real_hwp5_parsing() {
    let path = Path::new(r"C:\Temp\PC\제안요청서.hwp");
    if !path.exists() {
        println!("HWP test file not found at: {:?}", path);
        return;
    }

    let bytes = fs::read(path).expect("Failed to read HWP test file");
    let result = wordcraft_formats::import("hwp", &bytes);
    assert!(result.is_some(), "hwp extension should be recognized");

    let doc = result.expect("hwp import option").expect("hwp parsing should succeed");
    let plain_text = doc.plain_text(StoryRef::Body);

    println!("=== Real HWP 5.x Import Results ===");
    println!("File size: {} bytes", bytes.len());
    println!("Extracted text length: {} chars", plain_text.chars().count());
    println!("Preview (first 300 chars):\n{}", plain_text.chars().take(300).collect::<String>());

    assert!(!plain_text.trim().is_empty(), "Extracted plain text should not be empty");
    assert!(
        plain_text.contains("제안") || plain_text.contains("요청서"),
        "Extracted text should contain keywords from the document title"
    );
}
