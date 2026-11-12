//! What the HWP 5.x and HWPX readers share: their sections (page, header, footer and blocks)
//! into a [`Document`].

use std::sync::Arc;

use wordcraft_doc::section::HeaderSet;
use wordcraft_doc::{Block, Document, PartKind, SectionProps};

use crate::model::{self, Cell, FBlock, FTable, Flow, Para};

/// Add `table` to `out`. Hangul anchors each table in a paragraph of its own; the document model
/// joins tables that touch into one, so a table right after another gets an empty paragraph
/// between them.
pub(crate) fn push_table(out: &mut Vec<FBlock>, table: FTable) {
    if matches!(out.last(), Some(FBlock::Table(_))) {
        out.push(FBlock::Para(Para::default()));
    }
    out.push(FBlock::Table(table));
}

/// Most rows and columns a table keeps (hostile files).
pub(crate) const MAX_TABLE_ROWS: usize = 4096;
pub(crate) const MAX_TABLE_COLS: usize = 63;

/// Most covered places a table's row spans make (hostile files); spans past it are cut.
const MAX_COVERED: usize = 1 << 17;

/// A table from cells placed by address (row, column), as Hangul stores them. A row lists only
/// the cells that start in it, so a row under several row spans can hold cells far apart; the
/// places spans from above cover become covered cells, leaving each row's grid in order.
pub(crate) fn table_from_grid(cells: Vec<(usize, usize, Cell)>) -> Option<FTable> {
    let cells: Vec<_> = cells.into_iter().filter(|(r, c, _)| *r < MAX_TABLE_ROWS && *c < MAX_TABLE_COLS).collect();
    let rows = cells.iter().map(|(r, _, _)| r + 1).max()?;
    let mut grid: Vec<Vec<(usize, Cell)>> = vec![Vec::new(); rows];
    let mut covered = 0usize;
    for (r, c, cell) in &cells {
        let span = cell.rowspan.clamp(1, MAX_TABLE_ROWS as u32) as usize;
        for below in grid.iter_mut().skip(r + 1).take(span - 1) {
            covered += 1;
            if covered > MAX_COVERED {
                break;
            }
            below.push((*c, Cell { covered: true, colspan: cell.colspan, blocks: vec![FBlock::Para(Para::default())], ..Default::default() }));
        }
    }
    for (r, c, cell) in cells {
        if let Some(row) = grid.get_mut(r) {
            row.push((c, cell));
        }
    }
    let mut table = FTable::default();
    for mut row in grid {
        row.sort_by_key(|(c, _)| *c);
        // Two cells claiming one place (a broken file): the first keeps it.
        row.dedup_by_key(|(c, _)| *c);
        if !row.is_empty() {
            table.rows.push(row.into_iter().map(|(_, cell)| cell).collect());
        }
    }
    table.widths_from_cells();
    Some(table)
}

/// Largest stream or zip entry inflated (hostile files).
pub(crate) const MAX_PART: usize = 256 << 20;

/// Most sections read; later ones are left out.
pub(crate) const MAX_SECTIONS: usize = 10_000;

/// One section of a Hangul document as read.
#[derive(Clone, Debug, Default)]
pub(crate) struct Section {
    /// Its page, when the section defines one (a section without keeps the one before it).
    pub page: Option<SectionProps>,
    pub blocks: Vec<FBlock>,
    pub header: Option<Vec<FBlock>>,
    pub footer: Option<Vec<FBlock>>,
}

/// `sections` as a document. Each section but the last ends with a paragraph carrying its page
/// (an empty one after a closing table); headers and footers carry on into the sections after
/// theirs until one sets its own, as in Hangul.
pub(crate) fn to_doc(sections: Vec<Section>, default_page: &SectionProps) -> Document {
    let count = sections.len();
    let mut flow = Flow::default();
    let mut page = default_page.clone();
    let (mut header, mut footer) = (None, None);
    let mut bands = Vec::with_capacity(count);
    for (i, s) in sections.into_iter().enumerate() {
        if let Some(p) = s.page {
            page = p;
        }
        header = s.header.or(header);
        footer = s.footer.or(footer);
        bands.push((header.clone(), footer.clone()));
        let mut blocks = s.blocks;
        if i + 1 < count {
            if !matches!(blocks.last(), Some(FBlock::Para(_))) {
                blocks.push(FBlock::Para(Para::default()));
            }
            if let Some(FBlock::Para(p)) = blocks.last_mut() {
                p.section = Some(Box::new(page.clone()));
            }
        }
        flow.blocks.extend(blocks);
    }
    let mut doc = model::to_doc_in(&flow, page);
    let mut sets = Vec::with_capacity(bands.len());
    for (h, f) in bands {
        let mut set = |kind, blocks: Option<Vec<FBlock>>| {
            let mut s = HeaderSet::default();
            if let Some(b) = blocks {
                let blocks = model::part_blocks(&mut doc, &b);
                s.default = Some(doc.add_part(kind, blocks));
            }
            s
        };
        let headers = set(PartKind::Header, h);
        let footers = set(PartKind::Footer, f);
        sets.push((headers, footers));
    }
    let mut sets = sets.into_iter();
    for b in &mut doc.body {
        if let Block::Para(p) = &**b
            && p.section.is_some()
            && let Block::Para(p) = Arc::make_mut(b)
            && let Some(s) = p.section.as_deref_mut()
            && let Some((h, f)) = sets.next()
        {
            (s.headers, s.footers) = (h, f);
        }
    }
    if let Some((h, f)) = sets.next() {
        (doc.last_section.headers, doc.last_section.footers) = (h, f);
    }
    doc
}
