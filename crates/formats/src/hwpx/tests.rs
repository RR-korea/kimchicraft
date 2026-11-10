use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;
use wordcraft_doc::{Rgb, StoryRef};

use super::*;
use super::styles::{hwpu_to_pt, parse_color};

#[test]
fn test_hwpu_to_pt() {
    assert_eq!(hwpu_to_pt(1000), 10.0);
    assert_eq!(hwpu_to_pt(59528), 595.28);
    assert_eq!(hwpu_to_pt(0), 0.0);
}

#[test]
fn test_parse_color() {
    assert_eq!(parse_color("#FF0000"), Some(Rgb(255, 0, 0)));
    assert_eq!(parse_color("0x00FF00"), Some(Rgb(0, 255, 0)));
    assert_eq!(parse_color("none"), None);
}

#[test]
fn test_hwpx_minimal_import() {
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let options = SimpleFileOptions::default();

        let header_xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head">
    <hh:fontfaces>
        <hh:fontface lang="hangul">
            <hh:font id="0" face="맑은 고딕"/>
        </hh:fontface>
    </hh:fontfaces>
    <hh:charProperties>
        <hh:charPr id="0" height="1000" textColor="#123456" bold="1"/>
    </hh:charProperties>
    <hh:paraProperties>
        <hh:paraPr id="0" align="CENTER"/>
    </hh:paraProperties>
</hh:head>"##;

        let section_xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
    <hs:secPr>
        <hp:pagePr width="59528" height="84189"/>
        <hp:margin left="8504" right="8504" top="5669" bottom="4252"/>
    </hs:secPr>
    <hp:p paraPrIDRef="0">
        <hp:run charPrIDRef="0">
            <hp:t>안녕하세요 한글 HWPX 문서입니다.</hp:t>
        </hp:run>
    </hp:p>
    <hp:p>
        <hp:run>
            <hp:tbl>
                <hp:tr>
                    <hp:tc>
                        <hp:cellSpan colSpan="1" rowSpan="1"/>
                        <hp:cellSz width="10000"/>
                        <hp:subList>
                            <hp:p><hp:run><hp:t>표 셀 1</hp:t></hp:run></hp:p>
                        </hp:subList>
                    </hp:tc>
                    <hp:tc>
                        <hp:cellSpan colSpan="1" rowSpan="1"/>
                        <hp:cellSz width="10000"/>
                        <hp:subList>
                            <hp:p><hp:run><hp:t>표 셀 2</hp:t></hp:run></hp:p>
                        </hp:subList>
                    </hp:tc>
                </hp:tr>
            </hp:tbl>
        </hp:run>
    </hp:p>
</hs:sec>"##;

        zip.start_file("Contents/header.xml", options).expect("create header.xml");
        zip.write_all(header_xml.as_bytes()).expect("write header.xml");

        zip.start_file("Contents/section0.xml", options).expect("create section0.xml");
        zip.write_all(section_xml.as_bytes()).expect("write section0.xml");

        zip.finish().expect("finish zip");
    }

    let doc = import(&buf).expect("HWPX import should succeed");
    let text = doc.plain_text(StoryRef::Body);
    assert!(text.contains("안녕하세요 한글 HWPX 문서입니다."));
    assert!(text.contains("표 셀 1"));
    assert!(text.contains("표 셀 2"));
}

#[test]
fn test_hwpx_roundtrip() {
    let doc = Document::from_text("KimchiCraft 대한민국 문서 테스트\n두 번째 문단입니다.");
    let exported_bytes = export(&doc).expect("HWPX export should succeed");

    assert!(exported_bytes.starts_with(b"PK"));

    let re_imported = import(&exported_bytes).expect("HWPX re-import should succeed");
    let text = re_imported.plain_text(StoryRef::Body);
    assert!(text.contains("KimchiCraft 대한민국 문서 테스트"));
    assert!(text.contains("두 번째 문단입니다."));
}
