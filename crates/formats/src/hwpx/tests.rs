use std::io::{Cursor, Read, Write};

use wordcraft_doc::{Align, Rgb, StoryRef};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::header::HeaderData;
use super::styles::{hwpu_to_pt, parse_color};
use super::*;
use crate::model::{FBlock, Inline, Para};

const HEAD: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" version="1.4" secCnt="1">
<hh:refList>
<hh:fontfaces itemCnt="2">
<hh:fontface lang="HANGUL" fontCnt="2"><hh:font id="0" face="바탕" type="TTF" isEmbedded="0"/><hh:font id="1" face="돋움" type="TTF" isEmbedded="0"/></hh:fontface>
<hh:fontface lang="LATIN" fontCnt="2"><hh:font id="0" face="Times" type="TTF" isEmbedded="0"/><hh:font id="1" face="Arial" type="TTF" isEmbedded="0"/></hh:fontface>
</hh:fontfaces>
<hh:borderFills itemCnt="2">
<hh:borderFill id="1" threeD="0" shadow="0"><hh:leftBorder type="NONE" width="0.1 mm" color="#000000"/></hh:borderFill>
<hh:borderFill id="4" threeD="0" shadow="0"><hc:fillBrush><hc:winBrush faceColor="#FFFF00" hatchColor="#000000" alpha="0"/></hc:fillBrush></hh:borderFill>
</hh:borderFills>
<hh:charProperties itemCnt="2">
<hh:charPr id="0" height="1000" textColor="#000000" shadeColor="none"><hh:fontRef hangul="0" latin="0"/><hh:underline type="NONE" shape="SOLID" color="#000000"/><hh:strikeout shape="NONE" color="#000000"/></hh:charPr>
<hh:charPr id="1" height="1600" textColor="#FF0000" shadeColor="none"><hh:fontRef hangul="1" latin="0"/><hh:bold/><hh:underline type="BOTTOM" shape="SOLID" color="#000000"/><hh:strikeout shape="NONE" color="#000000"/></hh:charPr>
</hh:charProperties>
<hh:paraProperties itemCnt="2">
<hh:paraPr id="0" tabPrIDRef="0"><hh:align horizontal="JUSTIFY" vertical="BASELINE"/></hh:paraPr>
<hh:paraPr id="1" tabPrIDRef="0"><hh:align horizontal="CENTER" vertical="BASELINE"/><hh:breakSetting keepWithNext="1" pageBreakBefore="0"/>
<hp:switch><hp:case hp:required-namespace="http://www.hancom.co.kr/hwpml/2016/HwpUnitChar"><hh:margin><hc:intent value="-1310" unit="HWPUNIT"/><hc:left value="1000" unit="HWPUNIT"/><hc:right value="0" unit="HWPUNIT"/><hc:prev value="0" unit="HWPUNIT"/><hc:next value="500" unit="HWPUNIT"/></hh:margin></hp:case>
<hp:default><hh:margin><hc:intent value="-2620" unit="HWPUNIT"/><hc:left value="2000" unit="HWPUNIT"/><hc:right value="0" unit="HWPUNIT"/><hc:prev value="0" unit="HWPUNIT"/><hc:next value="1000" unit="HWPUNIT"/></hh:margin></hp:default></hp:switch></hh:paraPr>
</hh:paraProperties>
<hh:styles itemCnt="1"><hh:style id="0" type="PARA" name="바탕글" engName="Normal" paraPrIDRef="0" charPrIDRef="0"/></hh:styles>
</hh:refList>
</hh:head>"##;

const SECTION: &str = r##"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0"><hp:run charPrIDRef="0"><hp:secPr id=""><hp:pagePr landscape="NARROWLY" width="59528" height="84188" gutterType="LEFT_ONLY"><hp:margin header="2834" footer="2834" gutter="0" left="5669" right="5669" top="2834" bottom="2834"/></hp:pagePr></hp:secPr><hp:ctrl><hp:colPr id="" type="NEWSPAPER" colCount="1"/></hp:ctrl><hp:ctrl><hp:header id="1" applyPageType="BOTH"><hp:subList id="" vertAlign="TOP"><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>머리말</hp:t></hp:run></hp:p></hp:subList></hp:header></hp:ctrl></hp:run><hp:run charPrIDRef="0"><hp:t>앞 A &amp; B<hp:tab width="4000" leader="0" type="1"/>탭<hp:lineBreak/>둘째 줄</hp:t></hp:run><hp:run charPrIDRef="1"><hp:t>굵게</hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0"/></hp:linesegarray></hp:p>
<hp:p id="0" paraPrIDRef="1" styleIDRef="0" pageBreak="1"><hp:run charPrIDRef="0"><hp:t>가운데</hp:t></hp:run></hp:p>
<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0"><hp:run charPrIDRef="0"><hp:tbl id="9" rowCnt="2" colCnt="2" borderFillIDRef="1">
<hp:tr><hp:tc borderFillIDRef="4"><hp:subList vertAlign="CENTER"><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>병합</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="2"/><hp:cellSz width="10000" height="282"/></hp:tc>
<hp:tc borderFillIDRef="1"><hp:subList><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>오른쪽 위</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="20000" height="282"/></hp:tc></hp:tr>
<hp:tr><hp:tc borderFillIDRef="1"><hp:subList><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>오른쪽 아래</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="20000" height="282"/></hp:tc></hp:tr>
</hp:tbl></hp:run></hp:p>
<hp:p id="0" paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:t>본문</hp:t><hp:ctrl><hp:footNote number="1"><hp:subList><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>각주 내용</hp:t></hp:run></hp:p></hp:subList></hp:footNote></hp:ctrl><hp:t>뒤 글</hp:t></hp:run></hp:p>
<hp:p id="0" paraPrIDRef="0" styleIDRef="0"><hp:run charPrIDRef="0"><hp:pic id="5"><hp:orgSz width="7200" height="3600"/><hc:img binaryItemIDRef="image1" bright="0"/><hp:sz width="14400" height="7200"/></hp:pic><hp:rect id="6"><hp:drawText><hp:subList><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>글상자</hp:t></hp:run></hp:p></hp:subList></hp:drawText></hp:rect></hp:run></hp:p>
</hs:sec>"##;

/// A one-pixel PNG.
fn png() -> Vec<u8> {
    let mut out = Vec::new();
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    image::DynamicImage::ImageRgba8(img).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).expect("png");
    out
}

/// A package with `HEAD`, the given sections (listed in the manifest's spine in that order) and
/// a picture whose manifest path is not the `BinData/<id>.<ext>` guess.
fn package(sections: &[(&str, &str)], spine: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let o = SimpleFileOptions::default();
        zip.start_file("mimetype", o).expect("zip");
        zip.write_all(b"application/hwp+zip").expect("zip");
        let mut items = String::from(
            r#"<opf:item id="header" href="Contents/header.xml" media-type="application/xml"/><opf:item id="image1" href="BinData/picture.PNG" media-type="image/png"/>"#,
        );
        let mut refs = String::from(r#"<opf:itemref idref="header"/>"#);
        for (i, (path, _)) in sections.iter().enumerate() {
            items.push_str(&format!(r#"<opf:item id="s{i}" href="{path}" media-type="application/xml"/>"#));
            refs.push_str(&format!(r#"<opf:itemref idref="s{i}"/>"#));
        }
        if !spine {
            refs.clear();
        }
        zip.start_file("Contents/content.hpf", o).expect("zip");
        let hpf = format!(
            r#"<opf:package xmlns:opf="http://www.idpf.org/2007/opf/"><opf:manifest>{items}</opf:manifest><opf:spine>{refs}</opf:spine></opf:package>"#
        );
        zip.write_all(hpf.as_bytes()).expect("zip");
        zip.start_file("Contents/header.xml", o).expect("zip");
        zip.write_all(HEAD.as_bytes()).expect("zip");
        for (path, xml) in sections {
            zip.start_file(*path, o).expect("zip");
            zip.write_all(xml.as_bytes()).expect("zip");
        }
        zip.start_file("BinData/picture.PNG", o).expect("zip");
        zip.write_all(&png()).expect("zip");
        zip.finish().expect("zip");
    }
    buf
}

fn one_para(text: &str) -> String {
    format!(r#"<hs:sec xmlns:hs="s" xmlns:hp="p"><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>{text}</hp:t></hp:run></hp:p></hs:sec>"#)
}

fn paras(blocks: &[FBlock]) -> Vec<&Para> {
    blocks.iter().filter_map(|b| if let FBlock::Para(p) = b { Some(p) } else { None }).collect()
}

fn runs(p: &Para) -> Vec<(String, bool)> {
    p.inlines.iter().filter_map(|i| if let Inline::Text(t, f) = i { Some((t.clone(), f.bold)) } else { None }).collect()
}

#[test]
fn units_and_colours() {
    assert_eq!(hwpu_to_pt(1000), 10.0);
    assert_eq!(parse_color("#FF0000"), Some(Rgb(255, 0, 0)));
    assert_eq!(parse_color("#80FF0000"), Some(Rgb(255, 0, 0)));
    assert_eq!(parse_color("none"), None);
}

#[test]
fn header_reads_child_elements_hangul_fonts_and_true_margins() {
    let h = HeaderData::parse(HEAD.as_bytes());
    let bold = &h.char_props[&1];
    assert!(bold.bold && bold.underline && !bold.italic);
    assert_eq!(bold.size, Some(16.0));
    assert_eq!(bold.color, Some(Rgb(255, 0, 0)));
    assert_eq!(bold.font.as_deref(), Some("돋움"), "the Hangul font, not the Latin one with the same id");
    assert_eq!(h.char_props[&0].color, None);
    let centred = &h.para_props[&1];
    assert_eq!(centred.align, Some(Align::Center));
    assert!(centred.keep_next);
    assert_eq!(centred.spacing.left, Some(10.0), "the hp:case value, not the doubled hp:default one");
    assert_eq!(centred.spacing.first, Some(-13.1));
    assert_eq!(centred.spacing.after, Some(5.0));
    assert_eq!(h.border_fills.get(&4), Some(&Rgb(255, 255, 0)));
}

#[test]
fn section_reads_real_structure() {
    let pkg = package(&[("Contents/section0.xml", SECTION)], true);
    let sections = parse(&pkg).expect("parse");
    let s = &sections[0];

    let page = s.page.as_ref().expect("secPr inside the first paragraph");
    assert_eq!((page.page_w, page.page_h), (841.88, 595.28));
    assert_eq!(page.margin_left, 56.69);

    let header = s.header.as_ref().expect("header");
    assert_eq!(paras(header)[0].text(), "머리말");

    let first = paras(&s.blocks)[0];
    assert_eq!(runs(first), vec![("앞 A & B\t탭\n둘째 줄".to_string(), false), ("굵게".to_string(), true)]);

    let centred = paras(&s.blocks)[1];
    assert_eq!(centred.align, Some(Align::Center));
    assert!(centred.page_break);

    let FBlock::Table(t) = &s.blocks[2] else { panic!("table expected: {:?}", s.blocks[2]) };
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.rows[0][0].rowspan, 2);
    assert_eq!(t.rows[0][0].shading, Some(Rgb(255, 255, 0)));
    assert!(t.rows[1][0].covered, "the merged cell covers the row below");
    assert_eq!(paras(&t.rows[1][1].blocks)[0].text(), "오른쪽 아래");
    assert_eq!(t.widths, vec![100.0, 200.0]);

    // A note's inner paragraph does not end the paragraph around it.
    let FBlock::Para(note_para) = &s.blocks[3] else { panic!("paragraph") };
    assert_eq!(note_para.text(), "본문뒤 글");
    assert!(note_para.inlines.iter().any(|i| matches!(i, Inline::Note { blocks, .. } if paras(blocks)[0].text() == "각주 내용")));

    let FBlock::Para(pic_para) = &s.blocks[4] else { panic!("paragraph") };
    let Some(Inline::Image(img)) = pic_para.inlines.first() else { panic!("picture from the manifest path: {pic_para:?}") };
    assert_eq!((img.w, img.h), (144.0, 72.0));
    assert_eq!(paras(&s.blocks)[4].text(), "글상자", "a text box keeps its text after its paragraph");
    assert!(matches!(s.blocks[5], FBlock::Para(_)));
}

#[test]
fn sections_follow_the_spine_or_their_numbers() {
    let s2 = one_para("둘");
    let s10 = one_para("열");
    let order = |pkg: &[u8]| -> Vec<String> { parse(pkg).expect("parse").iter().map(|s| paras(&s.blocks)[0].text()).collect() };
    let listed = package(&[("Contents/section10.xml", &s10), ("Contents/section2.xml", &s2)], true);
    assert_eq!(order(&listed), vec!["열", "둘"]);
    let unlisted = package(&[("Contents/section10.xml", &s10), ("Contents/section2.xml", &s2)], false);
    assert_eq!(order(&unlisted), vec!["둘", "열"]);
}

#[test]
fn import_builds_sections_headers_and_text() {
    let s1 = one_para("둘째 구역");
    let doc = import(&package(&[("Contents/section0.xml", SECTION), ("Contents/section1.xml", &s1)], true)).expect("import");
    let text = doc.plain_text(StoryRef::Body);
    for w in ["A & B", "굵게", "가운데", "병합", "오른쪽 아래", "본문", "글상자", "둘째 구역"] {
        assert!(text.contains(w), "{w:?} missing from {text:?}");
    }
    assert_eq!(doc.sections().len(), 2);
    assert!(doc.last_section.headers.default.is_some(), "the first section's header carries on");
}

#[test]
fn export_writes_a_hangul_package() {
    let doc = import(&package(&[("Contents/section0.xml", SECTION)], true)).expect("import");
    let bytes = export(&doc).expect("export");
    let mut zip = zip::ZipArchive::new(Cursor::new(&bytes[..])).expect("zip");
    {
        let first = zip.by_index(0).expect("first entry");
        assert_eq!(first.name(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
    }
    let read = |zip: &mut zip::ZipArchive<Cursor<&[u8]>>, n: &str| {
        let mut s = String::new();
        zip.by_name(n).expect(n).read_to_string(&mut s).expect("utf-8");
        s
    };
    let header = read(&mut zip, "Contents/header.xml");
    assert_eq!(header.matches("<hh:fontface ").count(), 7);
    assert!(header.contains("<hh:bold/>") && header.contains(r#"<hh:align horizontal="CENTER""#));
    assert!(header.contains("<hp:case ") && header.contains(r#"<hc:left value="1000" unit="HWPUNIT"/>"#));
    assert!(header.contains(r#"<hc:left value="2000" unit="HWPUNIT"/>"#), "doubled in hp:default");
    let section = read(&mut zip, "Contents/section0.xml");
    assert!(section.contains("<hp:secPr ") && section.contains(r#"landscape="NARROWLY""#));
    assert!(section.contains("A &amp; B<hp:tab ") && section.contains("<hp:lineBreak/>"));
    assert!(section.contains("<hp:cellAddr ") && section.contains("<hp:footNote ") && section.contains("<hp:header "));
    let hpf = read(&mut zip, "Contents/content.hpf");
    assert!(hpf.contains(r#"href="Contents/section0.xml""#) && hpf.contains(r#"<opf:itemref idref="section0""#));
    assert!(zip.file_names().any(|n| n.starts_with("BinData/image1.")));
}

#[test]
fn export_then_import_keeps_formatting() {
    let doc = import(&package(&[("Contents/section0.xml", SECTION)], true)).expect("import");
    let again = import(&export(&doc).expect("export")).expect("re-import");
    let sections = parse(&export(&again).expect("export")).expect("parse");
    let s = &sections[0];
    let first = paras(&s.blocks)[0];
    assert_eq!(runs(first), vec![("앞 A & B\t탭\n둘째 줄".to_string(), false), ("굵게".to_string(), true)]);
    let bold = first.inlines.iter().find_map(|i| if let Inline::Text(_, f) = i { f.bold.then_some(f) } else { None }).expect("bold run");
    assert_eq!((bold.size, bold.color, bold.underline), (Some(16.0), Some(Rgb(255, 0, 0)), true));
    let centred = paras(&s.blocks)[1];
    assert_eq!((centred.align, centred.spacing.left, centred.page_break), (Some(Align::Center), Some(10.0), true));
    let page = s.page.as_ref().expect("page");
    assert_eq!((page.page_w, page.page_h), (841.88, 595.28));
    let FBlock::Table(t) = &s.blocks[2] else { panic!("table") };
    assert_eq!((t.rows.len(), t.rows[0][0].rowspan, t.rows[0][0].shading), (2, 2, Some(Rgb(255, 255, 0))));
    assert!(s.header.is_some());
    assert!(paras(&s.blocks)[2].inlines.iter().any(|i| matches!(i, Inline::Note { .. })));
    assert!(matches!(paras(&s.blocks)[3].inlines.first(), Some(Inline::Image(_))));
}

#[test]
fn hostile_packages_do_not_panic() {
    let good = package(&[("Contents/section0.xml", SECTION)], true);
    for cut in (0..good.len()).step_by(97) {
        let _ = import(&good[..cut]);
    }
    for cut in 0..SECTION.len() {
        if SECTION.is_char_boundary(cut) {
            let _ = parse(&package(&[("Contents/section0.xml", &SECTION[..cut])], true));
        }
    }
    let deep = format!("<hs:sec><hp:p><hp:run>{}</hp:run></hp:p></hs:sec>", "<hp:tbl><hp:tr><hp:tc><hp:subList><hp:p><hp:run>".repeat(500));
    let _ = parse(&package(&[("Contents/section0.xml", &deep)], true));
    let _ = HeaderData::parse(b"<hh:head><hh:charPr id=\"1\" height=\"-5\"><hh:fontRef hangul=\"99999999999\"/></hh:charPr>");
}
