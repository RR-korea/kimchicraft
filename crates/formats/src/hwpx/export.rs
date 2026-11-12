//! HWPX (KS X 6101, OWPML) export: the package parts Hangul reads — `mimetype` and `version.xml`
//! (stored), `META-INF/container.xml`, `Contents/content.hpf`, `Contents/header.xml`, one
//! `Contents/section{n}.xml` per section, `settings.xml` and the pictures under `BinData/`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write};
use std::sync::Arc;

use wordcraft_doc::props::VAlign;
use wordcraft_doc::section::HeaderSet;
use wordcraft_doc::{Document, Rgb, SectionProps};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::styles::{align_name, color_hex, pt_to_hwpu};
use crate::model::{self, Cell, FBlock, FTable, FlowOpts, Fmt, Img, Inline, Para, equations_as_text, mime_of};

const NS_HP: &str = "http://www.hancom.co.kr/hwpml/2011/paragraph";
const NS_HS: &str = "http://www.hancom.co.kr/hwpml/2011/section";
const NS_HC: &str = "http://www.hancom.co.kr/hwpml/2011/core";
const NS_HH: &str = "http://www.hancom.co.kr/hwpml/2011/head";

/// The language groups a font list repeats for (HWPX names a font per script).
const LANGS: [&str; 7] = ["HANGUL", "LATIN", "HANJA", "JAPANESE", "OTHER", "SYMBOL", "USER"];

/// The font for text that names none (Hangul's default body font).
const DEFAULT_FONT: &str = "함초롬바탕";

/// Border fills every document has: 1 and 2 draw nothing (page and characters), 3 draws a thin
/// solid grid (table cells). Shading fills follow from 4.
const FILL_NONE: u32 = 2;
const FILL_GRID: u32 = 3;

/// Export a [`Document`] as an HWPX package (ZIP bytes).
pub fn export(doc: &Document) -> Result<Vec<u8>, String> {
    let opts = FlowOpts { notes: true, ..Default::default() };
    let flow = model::from_doc_with(doc, opts);
    let band = |set: &HeaderSet| set.default.and_then(|id| doc.parts.get(&id)).map(|p| model::part_flow(doc, &p.blocks, opts));

    // One section per paragraph that ends one, then the last section.
    let mut sections: Vec<(SectionProps, Vec<FBlock>)> = Vec::new();
    let mut blocks = Vec::new();
    for b in flow.blocks {
        let end = if let FBlock::Para(p) = &b { p.section.clone() } else { None };
        blocks.push(b);
        if let Some(s) = end {
            sections.push((*s, std::mem::take(&mut blocks)));
        }
    }
    sections.push((doc.last_section.clone(), blocks));

    let mut w = Writer::new(doc);
    let mut section_xml = Vec::with_capacity(sections.len());
    for (props, blocks) in &sections {
        let (header, footer) = (band(&props.headers), band(&props.footers));
        section_xml.push(w.section(props, blocks, header.as_deref(), footer.as_deref()));
    }
    let header_xml = w.header_xml(section_xml.len());
    let title = if flow.meta.title.is_empty() { "문서" } else { flow.meta.title.as_str() };
    package(&w, title, &header_xml, &section_xml).map_err(|e| format!("HWPX zip error: {e}"))
}

fn package(w: &Writer, title: &str, header: &str, sections: &[String]) -> zip::result::ZipResult<Vec<u8>> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    // The mimetype comes first, stored, so the package can be recognised by its first bytes.
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/hwp+zip")?;
    zip.start_file("version.xml", stored)?;
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><hv:HCFVersion xmlns:hv="http://www.hancom.co.kr/hwpml/2011/version" tagetApplication="WORDPROCESSOR" major="5" minor="1" micro="0" buildNumber="0" os="1" xmlVersion="1.4" application="WordCraft" appVersion="{}"/>"#,
            env!("CARGO_PKG_VERSION")
        )
        .as_bytes(),
    )?;
    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><ocf:container xmlns:ocf="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:hpf="http://www.hancom.co.kr/schema/2011/hpf"><ocf:rootfiles><ocf:rootfile full-path="Contents/content.hpf" media-type="application/hwpml-package+xml"/></ocf:rootfiles></ocf:container>"#)?;
    zip.start_file("META-INF/manifest.xml", deflated)?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><odf:manifest xmlns:odf="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"/>"#,
    )?;

    let mut items = String::from(r#"<opf:item id="header" href="Contents/header.xml" media-type="application/xml"/>"#);
    for (id, path, ext, _) in &w.images {
        let _ = write!(items, r#"<opf:item id="{id}" href="{path}" media-type="{}" isEmbeded="1"/>"#, mime_of(ext));
    }
    let mut spine = String::from(r#"<opf:itemref idref="header" linear="yes"/>"#);
    for i in 0..sections.len() {
        let _ = write!(items, r#"<opf:item id="section{i}" href="Contents/section{i}.xml" media-type="application/xml"/>"#);
        let _ = write!(spine, r#"<opf:itemref idref="section{i}" linear="yes"/>"#);
    }
    items.push_str(r#"<opf:item id="settings" href="settings.xml" media-type="application/xml"/>"#);
    zip.start_file("Contents/content.hpf", deflated)?;
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><opf:package xmlns:opf="http://www.idpf.org/2007/opf/" xmlns:dc="http://purl.org/dc/elements/1.1/" version="" unique-identifier="" id=""><opf:metadata><opf:title>{}</opf:title><opf:language>ko</opf:language></opf:metadata><opf:manifest>{items}</opf:manifest><opf:spine>{spine}</opf:spine></opf:package>"#,
            esc(title)
        )
        .as_bytes(),
    )?;
    zip.start_file("Contents/header.xml", deflated)?;
    zip.write_all(header.as_bytes())?;
    for (i, s) in sections.iter().enumerate() {
        zip.start_file(format!("Contents/section{i}.xml"), deflated)?;
        zip.write_all(s.as_bytes())?;
    }
    zip.start_file("settings.xml", deflated)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><ha:HWPApplicationSetting xmlns:ha="http://www.hancom.co.kr/hwpml/2011/app" xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0"><ha:CaretPosition listIDRef="0" paraIDRef="0" pos="0"/></ha:HWPApplicationSetting>"#)?;
    for (_, path, _, data) in &w.images {
        zip.start_file(path.as_str(), stored)?;
        zip.write_all(data)?;
    }
    zip.finish()?;
    Ok(buf)
}

/// Character properties as written (`hh:charPr`).
#[derive(Clone, PartialEq, Eq, Hash)]
struct CharKey {
    font: u32,
    height: i64,
    color: Option<(u8, u8, u8)>,
    shade: Option<(u8, u8, u8)>,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    sup: bool,
    sub: bool,
}

/// Paragraph properties as written (`hh:paraPr`), HWPUNIT.
#[derive(Clone, PartialEq, Eq, Hash)]
struct ParaKey {
    align: &'static str,
    left: i64,
    right: i64,
    indent: i64,
    before: i64,
    after: i64,
    keep_next: bool,
}

/// Collects the fonts, properties, fills and pictures the sections use, numbering them as the
/// header lists them.
struct Writer {
    default_font: Arc<str>,
    default_size: f32,
    fonts: Vec<Arc<str>>,
    chars: Vec<CharKey>,
    paras: Vec<ParaKey>,
    shades: Vec<Rgb>,
    char_ids: HashMap<CharKey, u32>,
    para_ids: HashMap<ParaKey, u32>,
    /// (manifest id, path, extension, bytes).
    images: Vec<(String, String, String, Arc<Vec<u8>>)>,
    next_object: u32,
}

impl Writer {
    fn new(doc: &Document) -> Self {
        let chr = &doc.styles.default_chr;
        let mut w = Writer {
            default_font: chr.font.as_deref().filter(|f| !f.is_empty()).unwrap_or(DEFAULT_FONT).into(),
            default_size: chr.size.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(10.0),
            fonts: Vec::new(),
            chars: Vec::new(),
            paras: Vec::new(),
            shades: Vec::new(),
            char_ids: HashMap::new(),
            para_ids: HashMap::new(),
            images: Vec::new(),
            next_object: 1,
        };
        // Id 0 of each list is the default: plain text in a left-aligned paragraph.
        w.char_id(&Fmt::default());
        w.para_id(&Para::default());
        w
    }

    fn font_id(&mut self, name: &Arc<str>) -> u32 {
        if let Some(i) = self.fonts.iter().position(|f| f == name) {
            return i as u32;
        }
        self.fonts.push(name.clone());
        (self.fonts.len() - 1) as u32
    }

    fn char_id(&mut self, f: &Fmt) -> u32 {
        let font = f.font.clone().unwrap_or_else(|| self.default_font.clone());
        let rgb = |c: Rgb| (c.0, c.1, c.2);
        let key = CharKey {
            font: self.font_id(&font),
            height: pt_to_hwpu(f.size.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(self.default_size)).clamp(100, 409_600),
            color: f.color.map(rgb),
            shade: f.background.map(rgb),
            bold: f.bold,
            italic: f.italic,
            underline: f.underline,
            strike: f.strike,
            sup: f.sup,
            sub: f.sub,
        };
        if let Some(id) = self.char_ids.get(&key) {
            return *id;
        }
        let id = self.chars.len() as u32;
        self.chars.push(key.clone());
        self.char_ids.insert(key, id);
        id
    }

    fn para_id(&mut self, p: &Para) -> u32 {
        let v = |x: Option<f32>| x.filter(|v| v.is_finite()).map_or(0, |v| pt_to_hwpu(v).clamp(-1_440_000, 1_440_000));
        let key = ParaKey {
            align: align_name(p.align),
            left: v(p.spacing.left),
            right: v(p.spacing.right),
            indent: v(p.spacing.first),
            before: v(p.spacing.before),
            after: v(p.spacing.after),
            keep_next: p.keep_next,
        };
        if let Some(id) = self.para_ids.get(&key) {
            return *id;
        }
        let id = self.paras.len() as u32;
        self.paras.push(key.clone());
        self.para_ids.insert(key, id);
        id
    }

    /// The border fill shading cells with `c`.
    fn shade_fill(&mut self, c: Rgb) -> u32 {
        let i = self.shades.iter().position(|s| *s == c).unwrap_or_else(|| {
            self.shades.push(c);
            self.shades.len() - 1
        });
        FILL_GRID + 1 + i as u32
    }

    fn object_id(&mut self) -> u32 {
        self.next_object += 1;
        self.next_object
    }

    fn section(&mut self, props: &SectionProps, blocks: &[FBlock], header: Option<&[FBlock]>, footer: Option<&[FBlock]>) -> String {
        let mut s = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><hs:sec xmlns:hp="{NS_HP}" xmlns:hs="{NS_HS}" xmlns:hc="{NS_HC}" xmlns:hh="{NS_HH}">"#
        );
        // The section's first paragraph carries its definition, columns, header and footer.
        let mut lead = sec_pr(props);
        lead.push_str(r#"<hp:ctrl><hp:colPr id="" type="NEWSPAPER" layout="LEFT" colCount="1" sameSz="1" sameGap="0"/></hp:ctrl>"#);
        let text_w = pt_to_hwpu(props.page_w - props.margin_left - props.margin_right).max(1000);
        for (tag, band) in [("header", header), ("footer", footer)] {
            if let Some(band) = band {
                let id = self.object_id();
                let _ = write!(lead, r#"<hp:ctrl><hp:{tag} id="{id}" applyPageType="BOTH">"#);
                self.sub_list(&mut lead, band, "TOP", text_w);
                let _ = write!(lead, "</hp:{tag}></hp:ctrl>");
            }
        }
        let mut lead = Some(lead);
        let avail = (props.page_w - props.margin_left - props.margin_right).max(10.0);
        if blocks.is_empty() {
            self.para(&mut s, &Para::default(), lead.take().as_deref());
        }
        for b in blocks {
            self.block(&mut s, b, lead.take().as_deref(), avail, 0);
        }
        s.push_str("</hs:sec>");
        s
    }

    fn block(&mut self, s: &mut String, b: &FBlock, lead: Option<&str>, avail: f32, depth: usize) {
        match b {
            FBlock::Para(p) => self.para(s, p, lead),
            FBlock::Table(t) if depth < model::MAX_DEPTH => self.table(s, t, lead, avail, depth + 1),
            FBlock::Table(_) => {}
        }
    }

    fn para(&mut self, s: &mut String, p: &Para, lead: Option<&str>) {
        let pid = self.para_id(p);
        let _ = write!(s, r#"<hp:p id="0" paraPrIDRef="{pid}" styleIDRef="0" pageBreak="{}" columnBreak="0" merged="0">"#, u8::from(p.page_break));
        let mut runs = 0;
        if let Some(lead) = lead {
            let _ = write!(s, r#"<hp:run charPrIDRef="0">{lead}</hp:run>"#);
            runs += 1;
        }
        for inline in equations_as_text(&p.inlines).iter() {
            match inline {
                Inline::Text(t, f) if !t.is_empty() => {
                    let cid = self.char_id(f);
                    let _ = write!(s, r#"<hp:run charPrIDRef="{cid}"><hp:t>"#);
                    text(s, t);
                    s.push_str("</hp:t></hp:run>");
                    runs += 1;
                }
                Inline::Image(img) => {
                    let pic = self.picture(img);
                    let _ = write!(s, r#"<hp:run charPrIDRef="0">{pic}</hp:run>"#);
                    runs += 1;
                }
                Inline::Note { endnote, blocks, .. } => {
                    let (tag, id) = (if *endnote { "endNote" } else { "footNote" }, self.object_id());
                    let _ = write!(s, r#"<hp:run charPrIDRef="0"><hp:ctrl><hp:{tag} number="0" instId="{id}">"#);
                    self.sub_list(s, blocks, "TOP", 0);
                    let _ = write!(s, "</hp:{tag}></hp:ctrl></hp:run>");
                    runs += 1;
                }
                _ => {}
            }
        }
        if runs == 0 {
            s.push_str(r#"<hp:run charPrIDRef="0"/>"#);
        }
        s.push_str("</hp:p>");
    }

    /// `hp:subList` holding `blocks` (a header, footer, note or cell).
    fn sub_list(&mut self, s: &mut String, blocks: &[FBlock], valign: &str, width: i64) {
        let _ = write!(
            s,
            r#"<hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="{valign}" linkListIDRef="0" linkListNextIDRef="0" textWidth="{width}" textHeight="0" hasTextRef="0" hasNumRef="0">"#
        );
        if blocks.is_empty() {
            self.para(s, &Para::default(), None);
        }
        let avail = (width as f32 / 100.0).max(10.0);
        for b in blocks {
            self.block(s, b, None, avail, 1);
        }
        s.push_str("</hp:subList>");
    }

    fn table(&mut self, s: &mut String, t: &FTable, lead: Option<&str>, avail: f32, depth: usize) {
        let cols = t.cols().max(1);
        let widths: Vec<f32> = if t.widths.len() == cols && t.widths.iter().all(|w| w.is_finite() && *w > 0.0) {
            t.widths.clone()
        } else {
            vec![avail / cols as f32; cols]
        };
        let total: i64 = widths.iter().map(|w| pt_to_hwpu(*w)).sum();
        const ROW_H: i64 = 1000;
        let id = self.object_id();
        let _ = write!(s, r#"<hp:p id="0" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0"><hp:run charPrIDRef="0">"#);
        if let Some(lead) = lead {
            s.push_str(lead);
        }
        let _ = write!(
            s,
            r#"<hp:tbl id="{id}" zOrder="0" numberingType="TABLE" textWrap="TOP_AND_BOTTOM" textFlow="BOTH_SIDES" lock="0" dropcapstyle="None" pageBreak="CELL" repeatHeader="1" rowCnt="{}" colCnt="{cols}" cellSpacing="0" borderFillIDRef="{FILL_GRID}" noAdjust="0"><hp:sz width="{total}" widthRelTo="ABSOLUTE" height="{}" heightRelTo="ABSOLUTE" protect="0"/><hp:pos treatAsChar="1" affectLSpacing="0" flowWithText="1" allowOverlap="0" holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" horzAlign="LEFT" vertOffset="0" horzOffset="0"/><hp:outMargin left="0" right="0" top="0" bottom="0"/><hp:inMargin left="141" right="141" top="141" bottom="141"/>"#,
            t.rows.len(),
            ROW_H * t.rows.len() as i64
        );
        for (r, row) in t.rows.iter().enumerate() {
            s.push_str("<hp:tr>");
            let mut g = 0usize;
            for cell in row {
                let span = cell.colspan.clamp(1, 63) as usize;
                if !cell.covered {
                    let w: i64 = widths.iter().skip(g).take(span).map(|w| pt_to_hwpu(*w)).sum();
                    self.cell(s, cell, g, r, w, ROW_H, depth);
                }
                g += span;
            }
            s.push_str("</hp:tr>");
        }
        s.push_str("</hp:tbl></hp:run></hp:p>");
    }

    #[allow(clippy::too_many_arguments)]
    fn cell(&mut self, s: &mut String, cell: &Cell, col: usize, row: usize, width: i64, height: i64, depth: usize) {
        let fill = cell.shading.map_or(FILL_GRID, |c| self.shade_fill(c));
        let _ = write!(
            s,
            r#"<hp:tc name="" header="{}" hasMargin="0" protect="0" editable="0" dirty="0" borderFillIDRef="{fill}">"#,
            u8::from(cell.header)
        );
        let valign = match cell.valign {
            Some(VAlign::Center) => "CENTER",
            Some(VAlign::Bottom) => "BOTTOM",
            _ => "TOP",
        };
        let _ = write!(
            s,
            r#"<hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="{valign}" linkListIDRef="0" linkListNextIDRef="0" textWidth="{width}" textHeight="0" hasTextRef="0" hasNumRef="0">"#
        );
        if cell.blocks.is_empty() {
            self.para(s, &Para::default(), None);
        }
        let avail = (width as f32 / 100.0).max(10.0);
        for b in &cell.blocks {
            self.block(s, b, None, avail, depth);
        }
        let _ = write!(
            s,
            r#"</hp:subList><hp:cellAddr colAddr="{col}" rowAddr="{row}"/><hp:cellSpan colSpan="{}" rowSpan="{}"/><hp:cellSz width="{width}" height="{height}"/><hp:cellMargin left="141" right="141" top="141" bottom="141"/></hp:tc>"#,
            cell.colspan.clamp(1, 63),
            cell.rowspan.clamp(1, 4096)
        );
    }

    /// `hp:pic` for `img`, as a character in its line, its bytes added to `BinData/`.
    fn picture(&mut self, img: &Img) -> String {
        let n = self.images.len() + 1;
        let ext = if img.ext.is_empty() { "png".to_string() } else { img.ext.to_ascii_lowercase() };
        let item = format!("image{n}");
        self.images.push((item.clone(), format!("BinData/{item}.{ext}"), ext, img.data.clone()));
        let (w, h) = (pt_to_hwpu(img.w).clamp(1, 1_440_000), pt_to_hwpu(img.h).clamp(1, 1_440_000));
        let (id, inst) = (self.object_id(), self.object_id());
        format!(
            r#"<hp:pic id="{id}" zOrder="0" numberingType="PICTURE" textWrap="TOP_AND_BOTTOM" textFlow="BOTH_SIDES" lock="0" dropcapstyle="None" href="" groupLevel="0" instid="{inst}" reverse="0"><hp:offset x="0" y="0"/><hp:orgSz width="{w}" height="{h}"/><hp:curSz width="{w}" height="{h}"/><hp:flip horizontal="0" vertical="0"/><hp:rotationInfo angle="0" centerX="{}" centerY="{}" rotateimage="1"/><hp:renderingInfo><hc:transMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/><hc:scaMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/><hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/></hp:renderingInfo><hc:img binaryItemIDRef="{item}" bright="0" contrast="0" effect="REAL_PIC" alpha="0"/><hp:imgRect><hc:pt0 x="0" y="0"/><hc:pt1 x="{w}" y="0"/><hc:pt2 x="{w}" y="{h}"/><hc:pt3 x="0" y="{h}"/></hp:imgRect><hp:imgClip left="0" right="{w}" top="0" bottom="{h}"/><hp:inMargin left="0" right="0" top="0" bottom="0"/><hp:imgDim dimwidth="{w}" dimheight="{h}"/><hp:effects/><hp:sz width="{w}" widthRelTo="ABSOLUTE" height="{h}" heightRelTo="ABSOLUTE" protect="0"/><hp:pos treatAsChar="1" affectLSpacing="0" flowWithText="1" allowOverlap="0" holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" horzAlign="LEFT" vertOffset="0" horzOffset="0"/><hp:outMargin left="0" right="0" top="0" bottom="0"/></hp:pic>"#,
            w / 2,
            h / 2
        )
    }

    fn header_xml(&self, sections: usize) -> String {
        let mut s = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?><hh:head xmlns:hh="{NS_HH}" xmlns:hc="{NS_HC}" xmlns:hp="{NS_HP}" version="1.4" secCnt="{sections}"><hh:beginNum page="1" footnote="1" endnote="1" pic="1" tbl="1" equation="1"/><hh:refList>"#
        );
        let _ = write!(s, r#"<hh:fontfaces itemCnt="{}">"#, LANGS.len());
        for lang in LANGS {
            let _ = write!(s, r#"<hh:fontface lang="{lang}" fontCnt="{}">"#, self.fonts.len());
            for (i, f) in self.fonts.iter().enumerate() {
                let _ = write!(s, r#"<hh:font id="{i}" face="{}" type="TTF" isEmbedded="0"/>"#, esc(f));
            }
            s.push_str("</hh:fontface>");
        }
        s.push_str("</hh:fontfaces>");

        let _ = write!(s, r#"<hh:borderFills itemCnt="{}">"#, FILL_GRID as usize + self.shades.len());
        border_fill(&mut s, 1, "NONE", None);
        border_fill(&mut s, FILL_NONE, "NONE", None);
        border_fill(&mut s, FILL_GRID, "SOLID", None);
        for (i, c) in self.shades.iter().enumerate() {
            border_fill(&mut s, FILL_GRID + 1 + i as u32, "SOLID", Some(*c));
        }
        s.push_str("</hh:borderFills>");

        let _ = write!(s, r#"<hh:charProperties itemCnt="{}">"#, self.chars.len());
        for (i, c) in self.chars.iter().enumerate() {
            let hex = |c: Option<(u8, u8, u8)>, none: &str| c.map_or(none.to_string(), |(r, g, b)| color_hex(Rgb(r, g, b)));
            let _ = write!(
                s,
                r#"<hh:charPr id="{i}" height="{}" textColor="{}" shadeColor="{}" useFontSpace="0" useKerning="0" symMark="NONE" borderFillIDRef="{FILL_NONE}">"#,
                c.height,
                hex(c.color, "#000000"),
                hex(c.shade, "none")
            );
            let f = c.font;
            let _ = write!(s, r#"<hh:fontRef hangul="{f}" latin="{f}" hanja="{f}" japanese="{f}" other="{f}" symbol="{f}" user="{f}"/>"#);
            for (tag, v) in [("ratio", 100), ("spacing", 0), ("relSz", 100), ("offset", 0)] {
                let _ = write!(s, r#"<hh:{tag} hangul="{v}" latin="{v}" hanja="{v}" japanese="{v}" other="{v}" symbol="{v}" user="{v}"/>"#);
            }
            if c.italic {
                s.push_str("<hh:italic/>");
            }
            if c.bold {
                s.push_str("<hh:bold/>");
            }
            let _ = write!(s, r##"<hh:underline type="{}" shape="SOLID" color="#000000"/>"##, if c.underline { "BOTTOM" } else { "NONE" });
            let _ = write!(s, r##"<hh:strikeout shape="{}" color="#000000"/>"##, if c.strike { "SOLID" } else { "NONE" });
            s.push_str(r##"<hh:outline type="NONE"/><hh:shadow type="NONE" color="#B2B2B2" offsetX="10" offsetY="10"/>"##);
            if c.sup {
                s.push_str("<hh:supscript/>");
            } else if c.sub {
                s.push_str("<hh:subscript/>");
            }
            s.push_str("</hh:charPr>");
        }
        s.push_str(r#"</hh:charProperties><hh:tabProperties itemCnt="1"><hh:tabPr id="0" autoTabLeft="0" autoTabRight="0"/></hh:tabProperties>"#);

        let _ = write!(s, r#"<hh:paraProperties itemCnt="{}">"#, self.paras.len());
        for (i, p) in self.paras.iter().enumerate() {
            let _ = write!(
                s,
                r#"<hh:paraPr id="{i}" tabPrIDRef="0" condense="0" fontLineHeight="0" snapToGrid="1" suppressLineNumbers="0" checked="0"><hh:align horizontal="{}" vertical="BASELINE"/><hh:heading type="NONE" idRef="0" level="0"/><hh:breakSetting breakLatinWord="KEEP_WORD" breakNonLatinWord="KEEP_WORD" widowOrphan="0" keepWithNext="{}" keepLines="0" pageBreakBefore="0" lineWrap="BREAK"/><hh:autoSpacing eAsianEng="0" eAsianNum="0"/>"#,
                p.align,
                u8::from(p.keep_next)
            );
            // The true values for readers of the HwpUnitChar namespace, doubled for older ones.
            let margin = |k: i64| {
                format!(
                    r#"<hh:margin><hc:intent value="{}" unit="HWPUNIT"/><hc:left value="{}" unit="HWPUNIT"/><hc:right value="{}" unit="HWPUNIT"/><hc:prev value="{}" unit="HWPUNIT"/><hc:next value="{}" unit="HWPUNIT"/></hh:margin><hh:lineSpacing type="PERCENT" value="160" unit="HWPUNIT"/>"#,
                    p.indent * k,
                    p.left * k,
                    p.right * k,
                    p.before * k,
                    p.after * k
                )
            };
            let _ = write!(
                s,
                r#"<hp:switch><hp:case hp:required-namespace="http://www.hancom.co.kr/hwpml/2016/HwpUnitChar">{}</hp:case><hp:default>{}</hp:default></hp:switch><hh:border borderFillIDRef="{FILL_NONE}" offsetLeft="0" offsetRight="0" offsetTop="0" offsetBottom="0" connect="0" ignoreMargin="0"/></hh:paraPr>"#,
                margin(1),
                margin(2)
            );
        }
        s.push_str(r#"</hh:paraProperties><hh:styles itemCnt="1"><hh:style id="0" type="PARA" name="바탕글" engName="Normal" paraPrIDRef="0" charPrIDRef="0" nextStyleIDRef="0" langID="1042" lockForm="0"/></hh:styles>"#);
        s.push_str(r#"</hh:refList><hh:compatibleDocument targetProgram="HWP201X"><hh:layoutCompatibility/></hh:compatibleDocument><hh:docOption><hh:linkinfo path="" pageInherit="0" footnoteInherit="0"/></hh:docOption><hh:trackchageConfig flags="56"/></hh:head>"#);
        s
    }
}

fn border_fill(s: &mut String, id: u32, line: &str, fill: Option<Rgb>) {
    let _ = write!(
        s,
        r#"<hh:borderFill id="{id}" threeD="0" shadow="0" centerLine="NONE" breakCellSeparateLine="0"><hh:slash type="NONE" Crooked="0" isCounter="0"/><hh:backSlash type="NONE" Crooked="0" isCounter="0"/>"#
    );
    for side in ["leftBorder", "rightBorder", "topBorder", "bottomBorder"] {
        let _ = write!(s, r##"<hh:{side} type="{line}" width="0.12 mm" color="#000000"/>"##);
    }
    s.push_str(r##"<hh:diagonal type="SOLID" width="0.1 mm" color="#000000"/>"##);
    if let Some(c) = fill {
        let _ = write!(s, r##"<hc:fillBrush><hc:winBrush faceColor="{}" hatchColor="#000000" alpha="0"/></hc:fillBrush>"##, color_hex(c));
    }
    s.push_str("</hh:borderFill>");
}

/// `hp:secPr` with the page of `p` (HWPX keeps the upright sheet and turns it with `NARROWLY`).
fn sec_pr(p: &SectionProps) -> String {
    let landscape = p.page_w > p.page_h;
    let (w, h) = if landscape { (p.page_h, p.page_w) } else { (p.page_w, p.page_h) };
    let u = |v: f32| if v.is_finite() { pt_to_hwpu(v).clamp(0, 1_440_000) } else { 0 };
    let mut s = String::from(
        r#"<hp:secPr id="" textDirection="HORIZONTAL" spaceColumns="1134" tabStop="8000" tabStopVal="4000" tabStopUnit="HWPUNIT" outlineShapeIDRef="0" memoShapeIDRef="0" textVerticalWidthHead="0" masterPageCnt="0"><hp:grid lineGrid="0" charGrid="0" wonggojiFormat="0"/><hp:startNum pageStartsOn="BOTH" page="0" pic="0" tbl="0" equation="0"/><hp:visibility hideFirstHeader="0" hideFirstFooter="0" hideFirstMasterPage="0" border="SHOW_ALL" fill="SHOW_ALL" hideFirstPageNum="0" hideFirstEmptyLine="0" showLineNumber="0"/><hp:lineNumberShape restartType="0" countBy="0" distance="0" startNumber="0"/>"#,
    );
    let _ = write!(
        s,
        r#"<hp:pagePr landscape="{}" width="{}" height="{}" gutterType="LEFT_ONLY"><hp:margin header="{}" footer="{}" gutter="0" left="{}" right="{}" top="{}" bottom="{}"/></hp:pagePr>"#,
        if landscape { "NARROWLY" } else { "WIDELY" },
        u(w),
        u(h),
        u(p.header),
        u(p.footer),
        u(p.margin_left),
        u(p.margin_right),
        u(p.margin_top),
        u(p.margin_bottom)
    );
    for (tag, place) in [("footNotePr", "EACH_COLUMN"), ("endNotePr", "END_OF_DOCUMENT")] {
        let _ = write!(
            s,
            r##"<hp:{tag}><hp:autoNumFormat type="DIGIT" userChar="" prefixChar="" suffixChar=")" supscript="0"/><hp:noteLine length="-1" type="SOLID" width="0.12 mm" color="#000000"/><hp:noteSpacing betweenNotes="283" belowLine="567" aboveLine="850"/><hp:numbering type="CONTINUOUS" newNum="1"/><hp:placement place="{place}" beneathText="0"/></hp:{tag}>"##
        );
    }
    for kind in ["BOTH", "EVEN", "ODD"] {
        let _ = write!(
            s,
            r#"<hp:pageBorderFill type="{kind}" borderFillIDRef="1" textBorder="PAPER" headerInside="0" footerInside="0" fillArea="PAPER"><hp:offset left="1417" right="1417" top="1417" bottom="1417"/></hp:pageBorderFill>"#
        );
    }
    s.push_str("</hp:secPr>");
    s
}

/// `t` inside `hp:t`: tabs and line breaks as their elements.
fn text(s: &mut String, t: &str) {
    for (i, line) in t.split('\n').enumerate() {
        if i > 0 {
            s.push_str("<hp:lineBreak/>");
        }
        for (j, piece) in line.split('\t').enumerate() {
            if j > 0 {
                s.push_str(r#"<hp:tab width="4000" leader="0" type="1"/>"#);
            }
            s.push_str(&esc(piece));
        }
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}
