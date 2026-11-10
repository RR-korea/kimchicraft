//! HWPX (KS X 6101) document exporter.

use std::collections::HashMap;
use std::io::{Cursor, Write};
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};
use wordcraft_doc::{Align, Document, SectionProps};

use crate::model::{self, Cell, FBlock, Flow, Inline, Para};

/// Export a [`Document`] as an HWPX package (ZIP bytes).
pub fn export(doc: &Document) -> Result<Vec<u8>, String> {
    let flow = model::from_doc(doc);
    let section_props = doc.sections().first().map(|(_, s)| (*s).clone()).unwrap_or_default();
    export_flow(&flow, &section_props)
}

/// Export a [`Flow`] and [`SectionProps`] into HWPX bytes.
pub fn export_flow(flow: &Flow, section_props: &SectionProps) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));

        // 1. mimetype (MUST be stored uncompressed as the first entry)
        let stored_opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", stored_opts)
            .map_err(|e| format!("Zip error: {e}"))?;
        zip.write_all(b"application/hwp+zip")
            .map_err(|e| format!("Zip write error: {e}"))?;

        let deflated_opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // 2. META-INF/container.xml
        zip.start_file("META-INF/container.xml", deflated_opts)
            .map_err(|e| format!("Zip error: {e}"))?;
        let container_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<ocf:container xmlns:ocf="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
    <ocf:rootfiles>
        <ocf:rootfile full-path="Contents/content.hpf" media-type="application/hwp+zip"/>
    </ocf:rootfiles>
</ocf:container>"#;
        zip.write_all(container_xml.as_bytes())
            .map_err(|e| format!("Zip write error: {e}"))?;

        // 3. Contents/content.hpf
        zip.start_file("Contents/content.hpf", deflated_opts)
            .map_err(|e| format!("Zip error: {e}"))?;
        let title = if flow.meta.title.is_empty() { "문서" } else { &flow.meta.title };
        let content_hpf = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<opf:package xmlns:opf="http://www.idpf.org/2007/opf" version="2.0" unique-identifier="hwpx-doc">
    <opf:metadata>
        <dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">{}</dc:title>
        <dc:language xmlns:dc="http://purl.org/dc/elements/1.1/">ko</dc:language>
    </opf:metadata>
    <opf:manifest>
        <opf:item id="header" href="header.xml" media-type="application/xml"/>
        <opf:item id="section0" href="section0.xml" media-type="application/xml"/>
    </opf:manifest>
    <opf:spine>
        <opf:itemref idref="section0"/>
    </opf:spine>
</opf:package>"#,
            escape_xml(title)
        );
        zip.write_all(content_hpf.as_bytes())
            .map_err(|e| format!("Zip write error: {e}"))?;

        // Collect unique styles from flow blocks
        let mut collector = StyleCollector::default();
        collector.collect(flow);

        // 4. Contents/header.xml
        zip.start_file("Contents/header.xml", deflated_opts)
            .map_err(|e| format!("Zip error: {e}"))?;
        let header_xml = collector.generate_header_xml();
        zip.write_all(header_xml.as_bytes())
            .map_err(|e| format!("Zip write error: {e}"))?;

        // 5. Contents/section0.xml
        zip.start_file("Contents/section0.xml", deflated_opts)
            .map_err(|e| format!("Zip error: {e}"))?;
        let section_xml = generate_section_xml(flow, section_props, &collector);
        zip.write_all(section_xml.as_bytes())
            .map_err(|e| format!("Zip write error: {e}"))?;

        zip.finish().map_err(|e| format!("Zip finish error: {e}"))?;
    }

    Ok(buf)
}

#[derive(Default)]
struct StyleCollector {
    fonts: Vec<Arc<str>>,
    font_map: HashMap<Arc<str>, u32>,
    char_shapes: Vec<FmtKey>,
    char_map: HashMap<FmtKey, u32>,
    para_shapes: Vec<ParaKey>,
    para_map: HashMap<ParaKey, u32>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct FmtKey {
    font: Option<Arc<str>>,
    size_hwpu: u32,
    color_hex: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ParaKey {
    align: Option<Align>,
    indent_hwpu: i32,
    left_hwpu: i32,
    right_hwpu: i32,
    before_hwpu: i32,
    after_hwpu: i32,
}

impl StyleCollector {
    fn collect(&mut self, flow: &Flow) {
        // Ensure default font
        self.register_font(&Arc::from("맑은 고딕"));

        for block in &flow.blocks {
            self.collect_block(block);
        }
    }

    fn collect_block(&mut self, block: &FBlock) {
        match block {
            FBlock::Para(p) => self.collect_para(p),
            FBlock::Table(t) => {
                for row in &t.rows {
                    for cell in row {
                        for b in &cell.blocks {
                            self.collect_block(b);
                        }
                    }
                }
            }
        }
    }

    fn collect_para(&mut self, p: &Para) {
        let p_key = ParaKey {
            align: p.align,
            indent_hwpu: (p.spacing.first.unwrap_or(0.0) * 100.0) as i32,
            left_hwpu: (p.spacing.left.unwrap_or(0.0) * 100.0) as i32,
            right_hwpu: (p.spacing.right.unwrap_or(0.0) * 100.0) as i32,
            before_hwpu: (p.spacing.before.unwrap_or(0.0) * 100.0) as i32,
            after_hwpu: (p.spacing.after.unwrap_or(0.0) * 100.0) as i32,
        };
        self.register_para(p_key);

        for inline in &p.inlines {
            if let Inline::Text(_, fmt) = inline {
                if let Some(ref font) = fmt.font {
                    self.register_font(font);
                }
                let color_hex = fmt.color.map(|c| format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2));
                let size_hwpu = (fmt.size.unwrap_or(10.0) * 100.0) as u32;

                let c_key = FmtKey {
                    font: fmt.font.clone(),
                    size_hwpu,
                    color_hex,
                    bold: fmt.bold,
                    italic: fmt.italic,
                    underline: fmt.underline,
                    strike: fmt.strike,
                };
                self.register_char(c_key);
            }
        }
    }

    fn register_font(&mut self, font: &Arc<str>) -> u32 {
        if let Some(&id) = self.font_map.get(font) {
            return id;
        }
        let id = self.fonts.len() as u32;
        self.fonts.push(font.clone());
        self.font_map.insert(font.clone(), id);
        id
    }

    fn register_char(&mut self, key: FmtKey) -> u32 {
        if let Some(&id) = self.char_map.get(&key) {
            return id;
        }
        let id = self.char_shapes.len() as u32;
        self.char_shapes.push(key.clone());
        self.char_map.insert(key, id);
        id
    }

    fn register_para(&mut self, key: ParaKey) -> u32 {
        if let Some(&id) = self.para_map.get(&key) {
            return id;
        }
        let id = self.para_shapes.len() as u32;
        self.para_shapes.push(key.clone());
        self.para_map.insert(key, id);
        id
    }

    fn generate_header_xml(&self) -> String {
        let mut s = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head">
    <hh:fontfaces>
        <hh:fontface lang="hangul">
"#);
        for (i, font) in self.fonts.iter().enumerate() {
            s.push_str(&format!(
                r#"            <hh:font id="{i}" face="{}"/>
"#,
                escape_xml(font)
            ));
        }
        s.push_str(r#"        </hh:fontface>
    </hh:fontfaces>
    <hh:charProperties>
"#);
        for (i, cs) in self.char_shapes.iter().enumerate() {
            let color_attr = cs.color_hex.as_deref().map(|c| format!(r#" textColor="{c}""#)).unwrap_or_default();
            let bold_attr = if cs.bold { r#" bold="1""# } else { "" };
            let italic_attr = if cs.italic { r#" italic="1""# } else { "" };
            let underline_attr = if cs.underline { r#" underlineType="BOTTOM""# } else { "" };
            let strike_attr = if cs.strike { r#" strikeOutType="CONTINUOUS""# } else { "" };

            s.push_str(&format!(
                r#"        <hh:charPr id="{i}" height="{}"{}{}{}{}{}/>
"#,
                cs.size_hwpu, color_attr, bold_attr, italic_attr, underline_attr, strike_attr
            ));
        }
        s.push_str(r#"    </hh:charProperties>
    <hh:paraProperties>
"#);
        for (i, ps) in self.para_shapes.iter().enumerate() {
            let align_str = match ps.align {
                Some(Align::Left) => "LEFT",
                Some(Align::Right) => "RIGHT",
                Some(Align::Center) => "CENTER",
                Some(Align::Distribute) => "DISTRIBUTE",
                _ => "JUSTIFY",
            };
            s.push_str(&format!(
                r#"        <hh:paraPr id="{i}" align="{align_str}">
            <hh:margin left="{}" right="{}" indent="{}" prev="{}" next="{}"/>
        </hh:paraPr>
"#,
                ps.left_hwpu, ps.right_hwpu, ps.indent_hwpu, ps.before_hwpu, ps.after_hwpu
            ));
        }
        s.push_str(r#"    </hh:paraProperties>
</hh:head>"#);

        s
    }
}

fn generate_section_xml(flow: &Flow, section_props: &SectionProps, styles: &StyleCollector) -> String {
    let mut s = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
"#);

    let page_w_hwpu = (section_props.page_w * 100.0) as i64;
    let page_h_hwpu = (section_props.page_h * 100.0) as i64;
    let margin_l_hwpu = (section_props.margin_left * 100.0) as i64;
    let margin_r_hwpu = (section_props.margin_right * 100.0) as i64;
    let margin_t_hwpu = (section_props.margin_top * 100.0) as i64;
    let margin_b_hwpu = (section_props.margin_bottom * 100.0) as i64;

    s.push_str(&format!(
        r#"    <hs:secPr>
        <hp:pagePr width="{page_w_hwpu}" height="{page_h_hwpu}"/>
        <hp:margin left="{margin_l_hwpu}" right="{margin_r_hwpu}" top="{margin_t_hwpu}" bottom="{margin_b_hwpu}"/>
    </hs:secPr>
"#
    ));

    for block in &flow.blocks {
        render_block(&mut s, block, styles);
    }

    s.push_str("</hs:sec>");
    s
}

fn render_block(s: &mut String, block: &FBlock, styles: &StyleCollector) {
    match block {
        FBlock::Para(p) => render_para(s, p, styles),
        FBlock::Table(t) => render_table(s, t, styles),
    }
}

fn render_para(s: &mut String, p: &Para, styles: &StyleCollector) {
    let p_key = ParaKey {
        align: p.align,
        indent_hwpu: (p.spacing.first.unwrap_or(0.0) * 100.0) as i32,
        left_hwpu: (p.spacing.left.unwrap_or(0.0) * 100.0) as i32,
        right_hwpu: (p.spacing.right.unwrap_or(0.0) * 100.0) as i32,
        before_hwpu: (p.spacing.before.unwrap_or(0.0) * 100.0) as i32,
        after_hwpu: (p.spacing.after.unwrap_or(0.0) * 100.0) as i32,
    };
    let para_pr_id = styles.para_map.get(&p_key).copied().unwrap_or(0);
    let page_break_attr = if p.page_break { r#" pageBreak="1""# } else { "" };

    s.push_str(&format!(r#"    <hp:p paraPrIDRef="{para_pr_id}"{page_break_attr}>"#));

    for inline in &p.inlines {
        match inline {
            Inline::Text(t, fmt) => {
                let color_hex = fmt.color.map(|c| format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2));
                let size_hwpu = (fmt.size.unwrap_or(10.0) * 100.0) as u32;

                let c_key = FmtKey {
                    font: fmt.font.clone(),
                    size_hwpu,
                    color_hex,
                    bold: fmt.bold,
                    italic: fmt.italic,
                    underline: fmt.underline,
                    strike: fmt.strike,
                };
                let char_pr_id = styles.char_map.get(&c_key).copied().unwrap_or(0);

                s.push_str(&format!(
                    r#"<hp:run charPrIDRef="{char_pr_id}"><hp:t>{}</hp:t></hp:run>"#,
                    escape_xml(t)
                ));
            }
            _ => {}
        }
    }

    s.push_str("</hp:p>\n");
}

fn render_table(s: &mut String, t: &model::FTable, styles: &StyleCollector) {
    s.push_str("    <hp:p><hp:run><hp:tbl>\n");

    for row in &t.rows {
        s.push_str("        <hp:tr>\n");
        for cell in row {
            if cell.covered {
                continue;
            }
            render_cell(s, cell, styles);
        }
        s.push_str("        </hp:tr>\n");
    }

    s.push_str("    </hp:tbl></hp:run></hp:p>\n");
}

fn render_cell(s: &mut String, cell: &Cell, styles: &StyleCollector) {
    let width_hwpu = (cell.width.unwrap_or(100.0) * 100.0) as i64;
    s.push_str(&format!(
        r#"            <hp:tc>
                <hp:cellSpan colSpan="{}" rowSpan="{}"/>
                <hp:cellSz width="{width_hwpu}"/>
                <hp:subList>
"#,
        cell.colspan, cell.rowspan
    ));

    for b in &cell.blocks {
        render_block(s, b, styles);
    }

    s.push_str("                </hp:subList>\n            </hp:tc>\n");
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
