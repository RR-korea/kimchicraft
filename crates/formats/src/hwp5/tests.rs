use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;

use wordcraft_doc::{Align, Rgb, StoryRef};

use super::header::{DocInfo, FileHeader};
use super::record::{RecordReader, tags, tree};
use super::section::SectionParser;
use crate::model::{FBlock, Inline, Para};

/// One record: header (tag, level, size) and payload.
fn rec(tag: u16, level: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let small = data.len() < 0xFFF;
    let size = if small { data.len() as u32 } else { 0xFFF };
    out.extend_from_slice(&(u32::from(tag) | (u32::from(level) << 10) | (size << 20)).to_le_bytes());
    if !small {
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    }
    out.extend_from_slice(data);
    out
}

fn utf16(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|u| u.to_le_bytes()).collect()
}

fn text(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// An extended control character (8 units) of kind `ch`.
fn ext(ch: u16) -> Vec<u16> {
    vec![ch, 0, 0, 0, 0, 0, 0, ch]
}

fn ctrl(id: &[u8; 4], level: u16) -> Vec<u8> {
    let mut d = u32::from_be_bytes(*id).to_le_bytes().to_vec();
    d.extend_from_slice(&[0; 12]);
    rec(tags::CTRL_HEADER, level, &d)
}

/// PARA_HEADER with paragraph shape `shape` and break flags `brk`.
fn para_header(level: u16, shape: u16, brk: u8) -> Vec<u8> {
    let mut d = vec![0u8; 24];
    d[8..10].copy_from_slice(&shape.to_le_bytes());
    d[11] = brk;
    rec(tags::PARA_HEADER, level, &d)
}

fn para_text(level: u16, units: &[u16]) -> Vec<u8> {
    rec(tags::PARA_TEXT, level, &utf16(units))
}

fn char_shapes(level: u16, spans: &[(u32, u32)]) -> Vec<u8> {
    let d: Vec<u8> = spans.iter().flat_map(|(p, id)| p.to_le_bytes().into_iter().chain(id.to_le_bytes())).collect();
    rec(tags::PARA_CHAR_SHAPE, level, &d)
}

/// A cell's LIST_HEADER: one paragraph, at `col`, `row`, spanning `colspan`.
fn cell_header(level: u16, col: u16, row: u16, colspan: u16, width: u32) -> Vec<u8> {
    let mut d = vec![0u8; 34];
    d[0..2].copy_from_slice(&1u16.to_le_bytes());
    d[8..10].copy_from_slice(&col.to_le_bytes());
    d[10..12].copy_from_slice(&row.to_le_bytes());
    d[12..14].copy_from_slice(&colspan.to_le_bytes());
    d[14..16].copy_from_slice(&1u16.to_le_bytes());
    d[16..20].copy_from_slice(&width.to_le_bytes());
    rec(tags::LIST_HEADER, level, &d)
}

fn face_name(name: &str) -> Vec<u8> {
    let units = text(name);
    let mut d = vec![0x61];
    d.extend_from_slice(&(units.len() as u16).to_le_bytes());
    d.extend_from_slice(&utf16(&units));
    rec(tags::FACE_NAME, 0, &d)
}

/// CHAR_SHAPE as HWP 5.0.3 writes it (74 bytes).
fn char_shape(font: u16, size: i32, attr: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut d = vec![0u8; 74];
    for k in 0..7 {
        d[k * 2..k * 2 + 2].copy_from_slice(&font.to_le_bytes());
    }
    d[42..46].copy_from_slice(&size.to_le_bytes());
    d[46..50].copy_from_slice(&attr.to_le_bytes());
    d[52..55].copy_from_slice(&rgb);
    rec(tags::CHAR_SHAPE, 0, &d)
}

/// PARA_SHAPE with alignment `align` and doubled margins as HWP stores them.
fn para_shape(align: u32, left: i32, before: i32) -> Vec<u8> {
    let mut d = vec![0u8; 54];
    d[0..4].copy_from_slice(&(align << 2).to_le_bytes());
    d[4..8].copy_from_slice(&left.to_le_bytes());
    d[16..20].copy_from_slice(&before.to_le_bytes());
    rec(tags::PARA_SHAPE, 0, &d)
}

fn id_mappings(hangul_fonts: u32) -> Vec<u8> {
    let mut d = vec![0u8; 72];
    d[4..8].copy_from_slice(&hangul_fonts.to_le_bytes());
    rec(tags::ID_MAPPINGS, 0, &d)
}

/// DocInfo: Hangul fonts "바탕", "돋움" and a Latin one; shapes 0 (10 pt), 1 (bold red 16 pt,
/// 돋움); paragraph shapes 0 (justified) and 1 (centred, 10 pt left, 5 pt before).
fn doc_info() -> Vec<u8> {
    let mut s = id_mappings(2);
    let mut bin = vec![0x21, 0x00, 0x01, 0x00, 0x03, 0x00];
    bin.extend(utf16(&text("png")));
    s.extend(rec(tags::BIN_DATA, 0, &bin));
    s.extend(face_name("바탕"));
    s.extend(face_name("돋움"));
    s.extend(face_name("Latin Face"));
    s.extend(char_shape(0, 1000, 0, [0, 0, 0]));
    s.extend(char_shape(1, 1600, 0x02, [0xFF, 0, 0]));
    s.extend(rec(tags::BULLET, 0, &[0; 10]));
    s.extend(para_shape(0, 0, 0));
    s.extend(para_shape(3, 2000, 1000));
    s
}

/// A section: a section-definition paragraph (A4 landscape), "가나\t다" with 나… bold, a
/// centred page-break paragraph, a 1×2 table, and a paragraph with a footnote.
fn section0() -> Vec<u8> {
    let mut s = para_header(0, 0, 0);
    let mut t = ext(2);
    t.extend(text("가나"));
    t.extend([9, 0, 0, 0, 0, 0, 0, 9]);
    t.extend(text("다"));
    t.push(13);
    s.extend(para_text(1, &t));
    s.extend(char_shapes(1, &[(0, 0), (9, 1)]));
    s.extend(ctrl(b"secd", 1));
    let mut page = vec![0u8; 40];
    for (k, v) in [59528u32, 84188, 5669, 5669, 2834, 2834, 1417, 1417, 0, 1].iter().enumerate() {
        page[k * 4..k * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    s.extend(rec(tags::PAGE_DEF, 2, &page));

    s.extend(para_header(0, 1, 0x04));
    let mut t = text("가운데");
    t.push(13);
    s.extend(para_text(1, &t));
    s.extend(char_shapes(1, &[(0, 0)]));

    s.extend(para_header(0, 0, 0));
    let mut t = ext(11);
    t.push(13);
    s.extend(para_text(1, &t));
    s.extend(ctrl(b"tbl ", 1));
    // A caption above the table: its list header (side 2, top) comes before the TABLE record and
    // reads like a cell at column 2 if taken for one.
    let mut cap = vec![0u8; 30];
    cap[0] = 1;
    cap[8] = 2;
    s.extend(rec(tags::LIST_HEADER, 2, &cap));
    s.extend(para_header(2, 0, 0));
    let mut t = text("(단위: 원)");
    t.push(13);
    s.extend(para_text(3, &t));
    s.extend(rec(tags::TABLE, 2, &[0, 0, 0, 0, 1, 0, 2, 0]));
    for (col, word) in [(0u16, "왼쪽"), (1, "오른쪽")] {
        s.extend(cell_header(2, col, 0, 1, 7200));
        s.extend(para_header(2, 0, 0));
        let mut t = text(word);
        t.push(13);
        s.extend(para_text(3, &t));
    }

    s.extend(para_header(0, 0, 0));
    let mut t = text("본문");
    t.extend(ext(17));
    t.push(13);
    s.extend(para_text(1, &t));
    s.extend(ctrl(b"fn  ", 1));
    s.extend(rec(tags::LIST_HEADER, 2, &[1, 0, 0, 0, 0, 0, 0, 0]));
    s.extend(para_header(2, 0, 0));
    let mut t = text("각주 내용");
    t.push(13);
    s.extend(para_text(3, &t));

    s.extend(para_header(0, 0, 0));
    let mut t = ext(11);
    t.push(13);
    s.extend(para_text(1, &t));
    let mut gso = u32::from_be_bytes(*b"gso ").to_le_bytes().to_vec();
    gso.extend_from_slice(&[0; 12]);
    gso.extend_from_slice(&14400u32.to_le_bytes());
    gso.extend_from_slice(&7200u32.to_le_bytes());
    s.extend(rec(tags::CTRL_HEADER, 1, &gso));
    s.extend(rec(0x4C, 2, &[0; 8]));
    let mut pic = vec![0u8; 91];
    pic[71] = 1;
    s.extend(rec(tags::SHAPE_PICTURE, 3, &pic));
    s
}

/// A one-pixel PNG.
fn png() -> Vec<u8> {
    let mut out = Vec::new();
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255]));
    image::DynamicImage::ImageRgba8(img).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).expect("png");
    out
}

fn paras(blocks: &[FBlock]) -> Vec<&Para> {
    blocks.iter().filter_map(|b| if let FBlock::Para(p) = b { Some(p) } else { None }).collect()
}

#[test]
fn record_reader_reads_tags_levels_and_long_sizes() {
    let long = vec![7u8; 5000];
    let mut stream = rec(tags::PARA_HEADER, 0, &[1, 2, 3, 4]);
    stream.extend(rec(tags::PARA_TEXT, 1, &long));
    let recs: Vec<_> = RecordReader::new(&stream).collect();
    assert_eq!(recs.len(), 2);
    assert_eq!((recs[0].tag, recs[0].level, recs[0].data), (tags::PARA_HEADER, 0, &[1u8, 2, 3, 4][..]));
    assert_eq!((recs[1].tag, recs[1].level, recs[1].data.len()), (tags::PARA_TEXT, 1, 5000));
}

#[test]
fn record_reader_cuts_a_payload_longer_than_the_stream() {
    let mut stream = rec(tags::PARA_TEXT, 0, &[1, 2, 3, 4]);
    stream.truncate(6);
    let recs: Vec<_> = RecordReader::new(&stream).collect();
    assert_eq!(recs[0].data, &[1, 2]);
}

#[test]
fn tree_nests_records_by_level() {
    let nodes_src = section0();
    let nodes = tree(&nodes_src);
    assert_eq!(nodes.iter().filter(|n| n.rec.tag == tags::PARA_HEADER).count(), 5);
    let table = nodes[2].child(tags::CTRL_HEADER).expect("table control");
    assert_eq!(table.children.iter().filter(|n| n.rec.tag == tags::LIST_HEADER).count(), 3, "the caption and two cells");
}

#[test]
fn file_header_flags() {
    let mut header_bytes = vec![0u8; 256];
    header_bytes[..17].copy_from_slice(b"HWP Document File");
    header_bytes[32..36].copy_from_slice(&0x05000300u32.to_le_bytes());
    header_bytes[36..40].copy_from_slice(&0x05u32.to_le_bytes());
    let header = FileHeader::parse(&header_bytes).expect("valid header");
    assert_eq!(header.version, 0x05000300);
    assert!(header.is_compressed && header.is_distribution && !header.is_encrypted);
}

#[test]
fn doc_info_reads_fonts_char_and_para_shapes_at_their_offsets() {
    let info = DocInfo::parse(&doc_info());
    assert_eq!(info.fonts.len(), 2, "only the Hangul group");
    assert_eq!(info.fonts[1].as_deref(), Some("돋움"));
    let bold = &info.char_shapes[1];
    assert_eq!(bold.size, Some(16.0));
    assert!(bold.bold && !bold.italic);
    assert_eq!(bold.color, Some(Rgb(0xFF, 0, 0)));
    assert_eq!(bold.font.as_deref(), Some("돋움"));
    assert_eq!(info.char_shapes[0].color, None);
    // The BULLET record before them is not a paragraph shape.
    assert_eq!(info.para_shapes.len(), 2);
    assert_eq!(info.para_shapes[1].align, Some(Align::Center));
    assert_eq!(info.para_shapes[1].spacing.left, Some(10.0));
    assert_eq!(info.para_shapes[1].spacing.before, Some(5.0));
}

#[test]
fn section_keeps_text_runs_tabs_breaks_tables_and_notes() {
    let info = DocInfo::parse(&doc_info());
    let section = SectionParser::new(&info, &HashMap::new()).parse(&section0());

    let page = section.page.expect("page from secd");
    assert_eq!((page.page_w, page.page_h), (841.88, 595.28), "landscape swaps the paper");
    assert_eq!(page.margin_left, 56.69);

    let first = paras(&section.blocks)[0];
    let runs: Vec<(&str, bool)> =
        first.inlines.iter().filter_map(|i| if let Inline::Text(t, f) = i { Some((t.as_str(), f.bold)) } else { None }).collect();
    // The section control before the text does not shift where the bold shape starts.
    assert_eq!(runs, vec![("가", false), ("나\t다", true)]);

    let centred = paras(&section.blocks)[1];
    assert_eq!(centred.align, Some(Align::Center));
    assert!(centred.page_break);

    let FBlock::Para(caption) = &section.blocks[2] else { panic!("caption expected, got {:?}", section.blocks[2]) };
    assert_eq!(caption.text(), "(단위: 원)");
    let FBlock::Table(table) = &section.blocks[3] else { panic!("table expected, got {:?}", section.blocks[3]) };
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].len(), 2);
    assert_eq!(table.widths, vec![72.0, 72.0]);
    let cell_text: Vec<String> = table.rows[0].iter().map(|c| paras(&c.blocks)[0].text()).collect();
    assert_eq!(cell_text, vec!["왼쪽", "오른쪽"]);

    let FBlock::Para(last) = &section.blocks[4] else { panic!("paragraph after the table") };
    assert_eq!(last.text(), "본문");
    let images = HashMap::from([(1usize, Arc::new(png()))]);
    let with_pictures = SectionParser::new(&info, &images).parse(&section0());
    let FBlock::Para(pic) = &with_pictures.blocks[5] else { panic!("picture paragraph") };
    let Some(Inline::Image(img)) = pic.inlines.first() else { panic!("picture expected: {pic:?}") };
    assert_eq!((img.w, img.h), (144.0, 72.0));
    assert!(SectionParser::new(&info, &HashMap::new()).parse(&section0()).blocks.len() == 6, "a missing picture drops nothing else");
    let note = last.inlines.iter().find_map(|i| if let Inline::Note { blocks, endnote: false, .. } = i { Some(blocks) } else { None });
    assert_eq!(note.map(|b| paras(b)[0].text()), Some("각주 내용".to_string()));
}

#[test]
fn hostile_sections_do_not_panic() {
    let info = DocInfo::parse(&doc_info());
    let good = section0();
    for cut in 0..good.len() {
        let _ = SectionParser::new(&info, &HashMap::new()).parse(&good[..cut]);
    }
    let mut deep = Vec::new();
    for level in 0..1000u16 {
        deep.extend(para_header(level, 0, 0));
    }
    let _ = SectionParser::new(&info, &HashMap::new()).parse(&deep);
    let _ = DocInfo::parse(&good);
}

/// A compound file holding `doc_info()` and `section0()`, compressed as HWP writes them.
fn hwp_file(compressed: bool) -> Vec<u8> {
    let pack = |d: Vec<u8>| if compressed { miniz_oxide::deflate::compress_to_vec(&d, 6) } else { d };
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).expect("create cfb");
    let mut header = vec![0u8; 256];
    header[..17].copy_from_slice(b"HWP Document File");
    header[32..36].copy_from_slice(&0x05000300u32.to_le_bytes());
    header[36..40].copy_from_slice(&u32::from(compressed).to_le_bytes());
    comp.create_stream("/FileHeader").expect("stream").write_all(&header).expect("write");
    comp.create_stream("/DocInfo").expect("stream").write_all(&pack(doc_info())).expect("write");
    comp.create_storage("/BinData").expect("storage");
    comp.create_stream("/BinData/BIN0001.png").expect("stream").write_all(&png()).expect("write");
    comp.create_storage("/BodyText").expect("storage");
    comp.create_stream("/BodyText/Section0").expect("stream").write_all(&pack(section0())).expect("write");
    comp.create_stream("/BodyText/Section1").expect("stream").write_all(&pack(para_text_only("둘째 구역"))).expect("write");
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

fn para_text_only(s: &str) -> Vec<u8> {
    let mut out = para_header(0, 0, 0);
    let mut t = text(s);
    t.push(13);
    out.extend(para_text(1, &t));
    out
}

#[test]
fn import_reads_every_section_compressed_or_not() {
    for compressed in [true, false] {
        let doc = super::import(&hwp_file(compressed)).expect("import");
        let text = doc.plain_text(StoryRef::Body);
        for word in ["가나", "가운데", "왼쪽", "오른쪽", "본문", "둘째 구역"] {
            assert!(text.contains(word), "{word:?} missing from {text:?}");
        }
        // The first section's landscape page ends with its last paragraph; the second keeps it.
        assert_eq!(doc.last_section.page_w, 841.88);
        let pictures = doc.body.iter().filter(|b| matches!(&***b, wordcraft_doc::Block::Para(p) if p.objects.iter().any(|o| matches!(o, wordcraft_doc::InlineObject::Image { .. })))).count();
        assert_eq!(pictures, 1, "the embedded picture is read from BinData");
    }
}

#[test]
fn import_refuses_what_it_cannot_read() {
    assert!(super::import(b"not an ole file").is_err());
    let mut file = hwp_file(false);
    let n = file.len();
    file.truncate(n / 2);
    assert!(super::import(&file).is_err());
}
