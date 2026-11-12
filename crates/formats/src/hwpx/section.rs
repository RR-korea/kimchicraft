//! Section parser for HWPX documents (`Contents/section*.xml`): paragraphs and their runs, with
//! the section's page, tables, pictures, notes, headers, footers and text boxes.

use wordcraft_doc::SectionProps;
use wordcraft_doc::props::VAlign;

use super::header::HeaderData;
use super::package::Package;
use super::styles::hwpu_to_pt;
use super::xml::{self, Elem, Node};
use crate::hwp_doc::{Section, push_table, table_from_grid};
use crate::model::{Cell, FBlock, FTable, Fmt, Inline, MAX_DEPTH, Para, char_props, clean_text, make_img};

pub struct SectionParser<'a, 'p> {
    header: &'a HeaderData,
    package: &'a mut Package<'p>,
    section: Section,
}

impl<'a, 'p> SectionParser<'a, 'p> {
    pub fn new(header: &'a HeaderData, package: &'a mut Package<'p>) -> Self {
        SectionParser { header, package, section: Section::default() }
    }

    /// The section stored in `xml` (an `hs:sec`).
    pub(crate) fn parse(mut self, xml: &[u8]) -> Section {
        let mut blocks = Vec::new();
        if let Some(root) = xml::parse(xml) {
            self.paras(&root, &mut blocks, 0);
        }
        self.section.blocks = blocks;
        self.section
    }

    /// The `hp:p` children of `parent` (a section or an `hp:subList`).
    fn paras(&mut self, parent: &Elem, out: &mut Vec<FBlock>, depth: usize) {
        for p in parent.elems().filter(|e| e.name == "p") {
            self.para(p, out, depth);
        }
    }

    fn para(&mut self, e: &Elem, out: &mut Vec<FBlock>, depth: usize) {
        let mut p = Para::default();
        let style_para = e.int("styleIDRef").and_then(|s| self.header.styles.get(&(s as u32))).and_then(|(p, _)| *p);
        let para_pr = e.int("paraPrIDRef").map(|v| v as u32).or(style_para);
        if let Some(def) = para_pr.and_then(|id| self.header.para_props.get(&id)) {
            p.align = def.align;
            p.spacing = def.spacing;
            p.page_break = def.page_break;
            p.keep_next = def.keep_next;
        }
        p.page_break |= e.flag("pageBreak");

        let mut after = Vec::new();
        let mut split = false;
        let mut first_fmt = None;
        for run in e.elems().filter(|r| r.name == "run") {
            let fmt = run.int("charPrIDRef").and_then(|id| self.header.char_props.get(&(id as u32))).cloned().unwrap_or_default();
            first_fmt.get_or_insert_with(|| fmt.clone());
            for obj in run.elems() {
                match obj.name.as_str() {
                    "t" => push_t(&mut p, obj, &fmt),
                    "secPr" => {
                        if self.section.page.is_none() {
                            self.section.page = sec_pr(obj);
                        }
                    }
                    "ctrl" if depth < MAX_DEPTH => self.ctrl(obj, &mut p, depth + 1),
                    "tbl" if depth < MAX_DEPTH => {
                        if let Some(table) = self.table(obj, depth + 1) {
                            if !p.inlines.is_empty() {
                                out.push(FBlock::Para(p.clone()));
                                p.inlines.clear();
                                p.page_break = false;
                            }
                            let (caption, below) = self.caption(obj, depth + 1);
                            if below {
                                push_table(out, table);
                                out.extend(caption);
                            } else {
                                out.extend(caption);
                                push_table(out, table);
                            }
                            split = true;
                        }
                    }
                    "pic" if depth < MAX_DEPTH => {
                        if let Some(img) = self.picture(obj) {
                            p.inlines.push(img);
                        }
                        after.extend(self.caption(obj, depth + 1).0);
                    }
                    // Drawing objects keep the text of their text boxes, after the paragraph.
                    _ if depth < MAX_DEPTH => {
                        let mut lists = Vec::new();
                        obj.find_all("subList", &mut lists);
                        for list in lists {
                            self.paras(list, &mut after, depth + 1);
                        }
                    }
                    _ => {}
                }
            }
        }
        if p.inlines.is_empty()
            && let Some(f) = first_fmt
        {
            p.mark = Some(Box::new(char_props(&f)));
        }
        // A paragraph that only anchored a table leaves no empty line behind it.
        if !(split && p.inlines.is_empty()) {
            out.push(FBlock::Para(p));
        }
        out.append(&mut after);
    }

    /// The paragraphs of an object's `hp:caption`, and whether they go after it (`side` RIGHT or
    /// BOTTOM).
    fn caption(&mut self, obj: &Elem, depth: usize) -> (Vec<FBlock>, bool) {
        let mut blocks = Vec::new();
        let Some(cap) = obj.child("caption") else { return (blocks, false) };
        if let Some(list) = cap.child("subList") {
            self.paras(list, &mut blocks, depth);
        }
        (blocks, matches!(cap.attr("side"), Some("BOTTOM" | "RIGHT")))
    }

    /// `hp:ctrl`: headers, footers and notes (column, field and number controls hold no text).
    fn ctrl(&mut self, ctrl: &Elem, p: &mut Para, depth: usize) {
        for c in ctrl.elems() {
            let mut blocks = Vec::new();
            if let Some(list) = c.child("subList") {
                self.paras(list, &mut blocks, depth);
            }
            match c.name.as_str() {
                "header" if self.section.header.is_none() && !blocks.is_empty() => self.section.header = Some(blocks),
                "footer" if self.section.footer.is_none() && !blocks.is_empty() => self.section.footer = Some(blocks),
                "footNote" | "endNote" => {
                    p.inlines.push(Inline::Note { endnote: c.name == "endNote", custom: String::new(), blocks, fmt: Fmt::default() });
                }
                _ => {}
            }
        }
    }

    /// `hp:tbl` → `hp:tr` → `hp:tc`, each cell an `hp:subList` with `hp:cellAddr`, `hp:cellSpan`
    /// and `hp:cellSz`. Cells go where their address puts them; without addresses, in row order.
    fn table(&mut self, tbl: &Elem, depth: usize) -> Option<FTable> {
        // (row of the hp:tr, address, cell)
        let mut cells: Vec<(usize, Option<(usize, usize)>, Cell)> = Vec::new();
        for (r, tr) in tbl.elems().filter(|e| e.name == "tr").enumerate() {
            for tc in tr.elems().filter(|e| e.name == "tc") {
                let span = tc.child("cellSpan");
                let colspan = span.and_then(|s| s.int("colSpan")).map_or(1, |v| v.clamp(1, 63) as u32);
                let rowspan = span.and_then(|s| s.int("rowSpan")).map_or(1, |v| v.clamp(1, 4096) as u32);
                let mut cell = Cell { colspan, rowspan, ..Default::default() };
                if colspan == 1 {
                    cell.width = tc.child("cellSz").and_then(|s| s.int("width")).filter(|w| (1..=1_440_000).contains(w)).map(hwpu_to_pt);
                }
                cell.shading = tc.int("borderFillIDRef").and_then(|id| self.header.border_fills.get(&(id as u32))).copied();
                if let Some(list) = tc.child("subList") {
                    cell.valign = match list.attr("vertAlign") {
                        Some("CENTER") => Some(VAlign::Center),
                        Some("BOTTOM") => Some(VAlign::Bottom),
                        _ => None,
                    };
                    self.paras(list, &mut cell.blocks, depth);
                }
                if cell.blocks.is_empty() {
                    cell.blocks.push(FBlock::Para(Para::default()));
                }
                let addr =
                    tc.child("cellAddr").and_then(|a| Some((usize::try_from(a.int("rowAddr")?).ok()?, usize::try_from(a.int("colAddr")?).ok()?)));
                cells.push((r, addr, cell));
            }
        }
        if cells.iter().all(|(_, addr, _)| addr.is_some()) {
            return table_from_grid(cells.into_iter().filter_map(|(_, addr, cell)| addr.map(|(r, c)| (r, c, cell))).collect());
        }
        let mut table = FTable::default();
        for (r, _, cell) in cells {
            if table.rows.len() <= r {
                table.rows.resize_with(r + 1, Vec::new);
            }
            if let Some(row) = table.rows.get_mut(r) {
                row.push(cell);
            }
        }
        table.rows.retain(|row| !row.is_empty());
        if table.rows.is_empty() {
            return None;
        }
        table.insert_covered();
        table.widths_from_cells();
        Some(table)
    }

    /// `hp:pic`: the picture `hc:img binaryItemIDRef` names, at its `hp:sz` (else `hp:orgSz`).
    fn picture(&mut self, pic: &Elem) -> Option<Inline> {
        let id = pic.find("img")?.attr("binaryItemIDRef")?.to_string();
        let size = |name: &str| {
            let s = pic.child(name)?;
            let (w, h) = (s.int("width")?, s.int("height")?);
            ((1..=1_440_000).contains(&w) && (1..=1_440_000).contains(&h)).then(|| (hwpu_to_pt(w), hwpu_to_pt(h)))
        };
        let (w, h) = size("sz").or_else(|| size("orgSz")).unzip();
        let bytes = self.package.read_image(&id)?;
        make_img(bytes, w, h, "").map(Inline::Image)
    }
}

/// The text of `hp:t`, with its tab, line break and space elements.
fn push_t(p: &mut Para, t: &Elem, fmt: &Fmt) {
    for n in &t.children {
        let s = match n {
            Node::Text(s) => clean_text(s),
            Node::Elem(e) => match e.name.as_str() {
                "tab" => "\t".to_string(),
                "lineBreak" => "\n".to_string(),
                "nbSpace" => "\u{a0}".to_string(),
                "fwSpace" => " ".to_string(),
                "hyphen" => "-".to_string(),
                _ => continue,
            },
        };
        p.push_text(&s, fmt);
    }
}

/// `hp:secPr` → `hp:pagePr landscape width height` with its `hp:margin`. `NARROWLY` turns the
/// paper (its width and height stay those of the upright sheet).
fn sec_pr(sec: &Elem) -> Option<SectionProps> {
    let pp = sec.child("pagePr")?;
    let pt = |e: &Elem, k: &str| e.int(k).filter(|v| (0..=1_440_000).contains(v)).map(hwpu_to_pt);
    let (mut w, mut h) = (pt(pp, "width").filter(|v| *v > 0.0)?, pt(pp, "height").filter(|v| *v > 0.0)?);
    if pp.attr("landscape") == Some("NARROWLY") {
        std::mem::swap(&mut w, &mut h);
    }
    let mut page = SectionProps { page_w: w, page_h: h, ..SectionProps::default() };
    if let Some(m) = pp.child("margin") {
        let set = |slot: &mut f32, k: &str| {
            if let Some(v) = pt(m, k) {
                *slot = v;
            }
        };
        set(&mut page.margin_left, "left");
        set(&mut page.margin_right, "right");
        set(&mut page.margin_top, "top");
        set(&mut page.margin_bottom, "bottom");
        set(&mut page.header, "header");
        set(&mut page.footer, "footer");
    }
    Some(page)
}
