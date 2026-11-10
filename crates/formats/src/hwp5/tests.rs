use super::header::FileHeader;
use super::record::{RecordReader, tags};

#[test]
fn test_hwp5_record_reader() {
    // Construct a mock stream with 2 records
    // Record 1: tag=0x32 (PARA_HEADER), level=0, size=4 -> payload [0x01, 0x02, 0x03, 0x04]
    // Header u32 = 0x32 | (0 << 10) | (4 << 20) = 0x00400032
    let mut stream = Vec::new();

    let header1: u32 = 0x32 | (0 << 10) | (4 << 20);
    stream.extend_from_slice(&header1.to_le_bytes());
    stream.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);

    // Record 2: tag=0x33 (PARA_TEXT), level=0, size=6 -> payload "한글" in UTF-16LE
    let text = "한글";
    let u16_bytes: Vec<u8> = text.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
    let header2: u32 = 0x33 | (0 << 10) | ((u16_bytes.len() as u32) << 20);
    stream.extend_from_slice(&header2.to_le_bytes());
    stream.extend_from_slice(&u16_bytes);

    let mut reader = RecordReader::new(&stream);

    let rec1 = reader.next_record().expect("First record should exist");
    assert_eq!(rec1.tag_id, tags::PARA_HEADER);
    assert_eq!(rec1.size, 4);
    assert_eq!(rec1.data, vec![0x01, 0x02, 0x03, 0x04]);

    let rec2 = reader.next_record().expect("Second record should exist");
    assert_eq!(rec2.tag_id, tags::PARA_TEXT);
    assert_eq!(rec2.size, u16_bytes.len());
    assert_eq!(rec2.data, u16_bytes);

    assert!(reader.next_record().is_none());
}

#[test]
fn test_hwp5_file_header_validation() {
    let mut header_bytes = vec![0u8; 256];
    let sig = b"HWP Document File";
    header_bytes[..sig.len()].copy_from_slice(sig);

    // Version: 5.0.3.0
    let ver: u32 = 0x05000300;
    header_bytes[32..36].copy_from_slice(&ver.to_le_bytes());

    // Flags: compressed = 1
    let flags: u32 = 0x01;
    header_bytes[36..40].copy_from_slice(&flags.to_le_bytes());

    let header = FileHeader::parse(&header_bytes).expect("Valid header should parse");
    assert_eq!(header.version, ver);
    assert!(header.is_compressed);
    assert!(!header.is_encrypted);
}
