//! BodyText section parsing for HWP 5.x files: paragraphs, their controls (tables, notes,
//! headers, footers, text boxes) and the section's page.

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::SectionProps;

use super::header::DocInfo;
use super::record::{Node, i32_at, tags, tree, u16_at, u32_at, utf16_units};
use crate::hwp_doc::{MAX_TABLE_COLS, MAX_TABLE_ROWS, Section, push_table, table_from_grid};
use crate::model::{Cell, FBlock, FTable, Fmt, Inline, MAX_DEPTH, Para, clean_text, make_img};

/// A control's id: four ASCII characters, stored as a little-endian `u32` of their big-endian value.
const fn ctrl_id(id: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*id)
}
const SECTION_DEF: u32 = ctrl_id(b"secd");
const TABLE: u32 = ctrl_id(b"tbl ");
const HEADER: u32 = ctrl_id(b"head");
const FOOTER: u32 = ctrl_id(b"foot");
const FOOTNOTE: u32 = ctrl_id(b"fn  ");
const ENDNOTE: u32 = ctrl_id(b"en  ");
const DRAWING: u32 = ctrl_id(b"gso ");

pub struct SectionParser<'a> {
    info: &'a DocInfo,
    /// Embedded pictures by the number pictures refer to them with.
    images: &'a HashMap<usize, Arc<Vec<u8>>>,
    section: Section,
}

impl<'a> SectionParser<'a> {
    pub fn new(info: &'a DocInfo, images: &'a HashMap<usize, Arc<Vec<u8>>>) -> Self {
        SectionParser { info, images, section: Section::default() }
    }

    /// The section stored in `stream` (a decompressed `BodyText/Section*`).
    pub(crate) fn parse(mut self, stream: &[u8]) -> Section {
        let nodes = tree(stream);
        let mut blocks = Vec::new();
        self.paras(&nodes, &mut blocks, 0);
        self.section.blocks = blocks;
        self.section
    }

    /// The paragraphs among `nodes`, with what their controls hold.
    fn paras(&mut self, nodes: &[Node], out: &mut Vec<FBlock>, depth: usize) {
        for n in nodes.iter().filter(|n| n.rec.tag == tags::PARA_HEADER) {
            self.para(n, out, depth);
        }
    }

    /// PARA_HEADER: the character count, control mask, paragraph shape id (u16 at 8), style id
    /// and break kind (byte 11: 0x01 section, 0x02 columns, 0x04 page, 0x08 column).
    fn para(&mut self, n: &Node, out: &mut Vec<FBlock>, depth: usize) {
        let d = n.rec.data;
        let info = self.info;
        let mut p = Para::default();
        if let Some(shape) = u16_at(d, 8).and_then(|id| info.para_shapes.get(usize::from(id))) {
            p.align = shape.align;
            p.spacing = shape.spacing;
        }
        p.page_break = d.get(11).is_some_and(|b| b & 0x04 != 0);

        let text: Vec<u16> = n.child(tags::PARA_TEXT).map(|t| utf16_units(t.rec.data)).unwrap_or_default();
        // (position in the text, character shape id), by position.
        let shapes: Vec<(usize, usize)> = n
            .child(tags::PARA_CHAR_SHAPE)
            .map(|s| s.rec.data.as_chunks::<8>().0.iter().filter_map(|c| Some((u32_at(c, 0)? as usize, u32_at(c, 4)? as usize))).collect())
            .unwrap_or_default();
        let fmt_at = |i: usize| -> Fmt {
            let k = shapes.partition_point(|(pos, _)| *pos <= i);
            k.checked_sub(1).and_then(|k| shapes.get(k)).and_then(|(_, id)| info.char_shapes.get(*id)).cloned().unwrap_or_default()
        };
        // An empty paragraph's height comes from its character shape.
        if text.iter().all(|c| *c == 13)
            && let Some((_, id)) = shapes.first()
            && let Some(f) = info.char_shapes.get(*id)
        {
            p.mark = Some(Box::new(crate::model::char_props(f)));
        }

        let mut ctrls = n.children.iter().filter(|c| c.rec.tag == tags::CTRL_HEADER);
        let mut run: Vec<u16> = Vec::new();
        let mut run_fmt = Fmt::default();
        let mut after = Vec::new();
        let mut split = false;
        let mut i = 0;
        while let Some(&c) = text.get(i) {
            let (ch, units) = match c {
                // Extended controls (8 units): their objects are the CTRL_HEADER children, in order.
                1..=3 | 11 | 12 | 14..=18 | 21..=23 => {
                    push_run(&mut p, &mut run, &run_fmt);
                    if let Some(ctrl) = ctrls.next() {
                        split |= self.control(ctrl, &mut p, out, &mut after, depth);
                    }
                    i += 8;
                    continue;
                }
                // Inline controls (8 units): a tab, field ends and title marks.
                9 => (Some(u16::from(b'\t')), 8),
                4..=8 | 19 | 20 => (None, 8),
                10 => (Some(u16::from(b'\n')), 1),
                24 => (Some(u16::from(b'-')), 1),
                30 => (Some(0x00A0), 1),
                31 => (Some(u16::from(b' ')), 1),
                0..=31 => (None, 1),
                _ => (Some(c), 1),
            };
            let at = i;
            i += units;
            if let Some(ch) = ch {
                let f = fmt_at(at);
                if f != run_fmt {
                    push_run(&mut p, &mut run, &run_fmt);
                    run_fmt = f;
                }
                run.push(ch);
            }
        }
        push_run(&mut p, &mut run, &run_fmt);
        // A paragraph that only anchored a table leaves no empty line behind it.
        if !(split && p.inlines.is_empty()) {
            out.push(FBlock::Para(p));
        }
        out.append(&mut after);
    }

    /// Handle one extended control of a paragraph. Returns whether a table split the paragraph.
    fn control(&mut self, ctrl: &Node, p: &mut Para, out: &mut Vec<FBlock>, after: &mut Vec<FBlock>, depth: usize) -> bool {
        let id = u32_at(ctrl.rec.data, 0).unwrap_or_default();
        match id {
            SECTION_DEF => {
                if self.section.page.is_none()
                    && let Some(def) = ctrl.child(tags::PAGE_DEF)
                {
                    self.section.page = page_def(def.rec.data);
                }
                false
            }
            TABLE if depth < MAX_DEPTH => {
                let (table, caption, below) = self.table(ctrl, depth + 1);
                let Some(table) = table else { return false };
                if !p.inlines.is_empty() {
                    let before = p.clone();
                    p.page_break = false;
                    p.inlines.clear();
                    out.push(FBlock::Para(before));
                }
                if below {
                    push_table(out, table);
                    out.extend(caption);
                } else {
                    out.extend(caption);
                    push_table(out, table);
                }
                true
            }
            HEADER | FOOTER if depth < MAX_DEPTH => {
                let mut blocks = Vec::new();
                self.paras(&ctrl.children, &mut blocks, depth + 1);
                let slot = if id == HEADER { &mut self.section.header } else { &mut self.section.footer };
                if slot.is_none() && !blocks.is_empty() {
                    *slot = Some(blocks);
                }
                false
            }
            FOOTNOTE | ENDNOTE if depth < MAX_DEPTH => {
                let mut blocks = Vec::new();
                self.paras(&ctrl.children, &mut blocks, depth + 1);
                p.inlines.push(Inline::Note { endnote: id == ENDNOTE, custom: String::new(), blocks, fmt: Fmt::default() });
                false
            }
            // A picture sits in the line; text boxes keep their text after the paragraph.
            DRAWING if depth < MAX_DEPTH => {
                match self.picture(ctrl) {
                    Some(img) => p.inlines.push(img),
                    None => self.drawing_text(&ctrl.children, after, depth + 1),
                }
                false
            }
            _ => false,
        }
    }

    /// A picture control: its SHAPE_COMPONENT_PICTURE names the binary item (u16 at 71, from 1);
    /// the control's common object properties give its width and height (HWPUNIT at 16 and 20).
    fn picture(&self, ctrl: &Node) -> Option<Inline> {
        let pic = find(&ctrl.children, tags::SHAPE_PICTURE, 0)?;
        let data = self.images.get(&usize::from(u16_at(pic.rec.data, 71)?))?;
        let size = |at: usize| u32_at(ctrl.rec.data, at).filter(|v| (1..=1_440_000).contains(v)).map(|v| v as f32 / 100.0);
        make_img(data.clone(), size(16), size(20), "").map(Inline::Image)
    }

    /// The paragraphs anywhere inside a drawing object (text boxes, nested in shape records).
    fn drawing_text(&mut self, nodes: &[Node], out: &mut Vec<FBlock>, depth: usize) {
        if depth >= MAX_DEPTH {
            return;
        }
        if nodes.iter().any(|n| n.rec.tag == tags::PARA_HEADER) {
            self.paras(nodes, out, depth);
        }
        for n in nodes.iter().filter(|n| n.rec.tag != tags::PARA_HEADER) {
            self.drawing_text(&n.children, out, depth + 1);
        }
    }

    /// A table control, with its caption's paragraphs and whether they go below it. The control
    /// holds an optional caption first (a LIST_HEADER whose side is u32 at 8 — 0 left, 1 right,
    /// 2 top, 3 bottom — and its paragraphs), then TABLE (attributes, rows u16 at 4, columns u16
    /// at 6), then per cell a LIST_HEADER followed by the cell's paragraphs. A cell's LIST_HEADER
    /// holds the paragraph count, the list attributes, then column, row, column span, row span
    /// (u16 at 8..16) and width (HWPUNIT at 16).
    fn table(&mut self, ctrl: &Node, depth: usize) -> (Option<FTable>, Vec<FBlock>, bool) {
        let mut caption = Vec::new();
        let mut below = false;
        let mut in_cells = false;
        let mut cells: Vec<(usize, usize, Cell)> = Vec::new();
        let mut current: Option<(usize, usize, Cell)> = None;
        for n in &ctrl.children {
            match n.rec.tag {
                tags::TABLE => in_cells = true,
                tags::LIST_HEADER if !in_cells => below = u32_at(n.rec.data, 8).is_some_and(|side| side & 1 == 1),
                tags::PARA_HEADER if !in_cells => self.para(n, &mut caption, depth),
                tags::LIST_HEADER => {
                    cells.extend(current.take());
                    let d = n.rec.data;
                    let (Some(col), Some(row)) = (u16_at(d, 8), u16_at(d, 10)) else { continue };
                    let (col, row) = (usize::from(col), usize::from(row));
                    if col >= MAX_TABLE_COLS || row >= MAX_TABLE_ROWS {
                        continue;
                    }
                    let colspan = u16_at(d, 12).map_or(1, |s| u32::from(s).clamp(1, MAX_TABLE_COLS as u32));
                    let rowspan = u16_at(d, 14).map_or(1, |s| u32::from(s).clamp(1, MAX_TABLE_ROWS as u32));
                    let width = u32_at(d, 16).filter(|w| *w > 0 && colspan == 1).map(|w| w as f32 / 100.0);
                    current = Some((row, col, Cell { colspan, rowspan, width, ..Default::default() }));
                }
                tags::PARA_HEADER => {
                    if let Some((_, _, cell)) = current.as_mut() {
                        let mut blocks = std::mem::take(&mut cell.blocks);
                        self.para(n, &mut blocks, depth);
                        cell.blocks = blocks;
                    }
                }
                _ => {}
            }
        }
        cells.extend(current);
        for (_, _, cell) in &mut cells {
            if cell.blocks.is_empty() {
                cell.blocks.push(FBlock::Para(Para::default()));
            }
        }
        (table_from_grid(cells), caption, below)
    }
}

/// The first record with `tag` among `nodes` and below them.
fn find<'n, 'a>(nodes: &'n [Node<'a>], tag: u16, depth: usize) -> Option<&'n Node<'a>> {
    if depth >= MAX_DEPTH * 4 {
        return None;
    }
    nodes.iter().find_map(|n| if n.rec.tag == tag { Some(n) } else { find(&n.children, tag, depth + 1) })
}

fn push_run(p: &mut Para, run: &mut Vec<u16>, fmt: &Fmt) {
    if !run.is_empty() {
        p.push_text(&clean_text(&String::from_utf16_lossy(run)), fmt);
        run.clear();
    }
}

/// PAGE_DEF: paper width and height, then the left, right, top, bottom, header and footer
/// margins (HWPUNIT, 1/100 pt) and, after the gutter, the attributes (bit 0: landscape).
fn page_def(d: &[u8]) -> Option<SectionProps> {
    let pt = |at: usize| i32_at(d, at).filter(|v| (0..=1_440_000).contains(v)).map(|v| v as f32 / 100.0);
    let (mut w, mut h) = (pt(0).filter(|v| *v > 0.0)?, pt(4).filter(|v| *v > 0.0)?);
    if u32_at(d, 36).is_some_and(|a| a & 1 != 0) {
        std::mem::swap(&mut w, &mut h);
    }
    let mut page = SectionProps { page_w: w, page_h: h, ..SectionProps::default() };
    if let (Some(l), Some(r), Some(t), Some(b)) = (pt(8), pt(12), pt(16), pt(20)) {
        (page.margin_left, page.margin_right, page.margin_top, page.margin_bottom) = (l, r, t, b);
    }
    if let (Some(hd), Some(ft)) = (pt(24), pt(28)) {
        (page.header, page.footer) = (hd, ft);
    }
    Some(page)
}
