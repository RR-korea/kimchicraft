//! Section parser for HWPX documents (`Contents/section*.xml`).

use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::Event;
use wordcraft_doc::SectionProps;

use crate::model::{
    Cell, FBlock, FTable, Flow, Fmt, Inline, Kind, Para, clean_text, make_img,
};
use super::header::HeaderData;
use super::package::Package;
use super::styles::hwpu_to_pt;

pub struct SectionParser<'a, 'p> {
    header: &'a HeaderData,
    package: &'a mut Package<'p>,
}

impl<'a, 'p> SectionParser<'a, 'p> {
    pub fn new(header: &'a HeaderData, package: &'a mut Package<'p>) -> Self {
        SectionParser { header, package }
    }

    pub fn parse_section(&mut self, xml: &[u8], page: &mut SectionProps) -> Vec<FBlock> {
        let mut reader = Reader::from_reader(Cursor::new(xml));
        reader.config_mut().trim_text(false);
        let mut buf = Vec::with_capacity(1024);

        let mut blocks = Vec::new();

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"secPr" => {
                            self.parse_sec_pr(&mut reader, page);
                        }
                        b"p" => {
                            if let Some(para) = self.parse_para(e, &mut reader, &mut blocks) {
                                blocks.push(FBlock::Para(para));
                            }
                        }
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        blocks
    }

    fn parse_sec_pr(&mut self, reader: &mut Reader<Cursor<&[u8]>>, page: &mut SectionProps) {
        let mut buf = Vec::with_capacity(512);
        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"pagePr" => {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"width" => {
                                        if let Ok(w) = attr.unescape_value() {
                                            if let Ok(hwpu) = w.parse::<i64>() {
                                                page.page_w = hwpu_to_pt(hwpu);
                                            }
                                        }
                                    }
                                    b"height" => {
                                        if let Ok(h) = attr.unescape_value() {
                                            if let Ok(hwpu) = h.parse::<i64>() {
                                                page.page_h = hwpu_to_pt(hwpu);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b"margin" => {
                            for attr in e.attributes().flatten() {
                                if let Ok(val) = attr.unescape_value() {
                                    if let Ok(hwpu) = val.parse::<i64>() {
                                        let pt = hwpu_to_pt(hwpu);
                                        match attr.key.local_name().as_ref() {
                                            b"left" => page.margin_l = pt,
                                            b"right" => page.margin_r = pt,
                                            b"top" => page.margin_t = pt,
                                            b"bottom" => page.margin_b = pt,
                                            b"header" => page.header_margin = pt,
                                            b"footer" => page.footer_margin = pt,
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(ref e) if e.local_name().as_ref() == b"secPr" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
    }

    fn parse_para(
        &mut self,
        start_elem: &quick_xml::events::BytesStart,
        reader: &mut Reader<Cursor<&[u8]>>,
        blocks_out: &mut Vec<FBlock>,
    ) -> Option<Para> {
        let mut para = Para::default();

        let mut para_pr_id = None;
        let mut style_id = None;

        for attr in start_elem.attributes().flatten() {
            match attr.key.local_name().as_ref() {
                b"paraPrIDRef" => para_pr_id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                b"styleIDRef" => style_id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok()),
                b"pageBreak" => {
                    if let Ok(v) = attr.unescape_value() {
                        para.page_break = v == "1" || v.eq_ignore_ascii_case("true");
                    }
                }
                _ => {}
            }
        }

        // Apply style and paraPr
        if let Some(sid) = style_id {
            if let Some(&(p_id, _)) = self.header.styles.get(&sid) {
                if para_pr_id.is_none() {
                    para_pr_id = p_id;
                }
            }
        }
        if let Some(pid) = para_pr_id {
            if let Some(def) = self.header.para_props.get(&pid) {
                para.align = def.align;
                para.kind = def.kind;
                para.spacing = def.spacing;
            }
        }

        let mut current_char_pr_id = None;
        let mut in_text = false;
        let mut buf = Vec::with_capacity(512);

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"run" => {
                            current_char_pr_id = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"charPrIDRef" {
                                    current_char_pr_id = attr.unescape_value().ok().and_then(|v| v.parse::<u32>().ok());
                                }
                            }
                        }
                        b"t" => {
                            in_text = true;
                        }
                        b"tbl" => {
                            if let Some(table) = self.parse_table(reader) {
                                // If current paragraph has text, save it first
                                if !para.is_empty() {
                                    blocks_out.push(FBlock::Para(std::mem::take(&mut para)));
                                }
                                blocks_out.push(FBlock::Table(table));
                            }
                        }
                        b"pic" => {
                            if let Some(inline_img) = self.parse_picture(reader) {
                                para.inlines.push(inline_img);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Text(ref e) => {
                    if in_text {
                        if let Ok(t) = e.unescape() {
                            let fmt = current_char_pr_id
                                .and_then(|cid| self.header.char_props.get(&cid).cloned())
                                .unwrap_or_default();
                            para.push_text(&clean_text(&t), &fmt);
                        }
                    }
                }
                Event::End(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"t" => in_text = false,
                        b"run" => current_char_pr_id = None,
                        b"p" => break,
                        _ => {}
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Some(para)
    }

    fn parse_table(&mut self, reader: &mut Reader<Cursor<&[u8]>>) -> Option<FTable> {
        let mut table = FTable::default();
        let mut buf = Vec::with_capacity(512);

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) => {
                    if e.local_name().as_ref() == b"tr" {
                        let row = self.parse_table_row(reader);
                        if !row.is_empty() {
                            table.rows.push(row);
                        }
                    }
                }
                Event::End(ref e) if e.local_name().as_ref() == b"tbl" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        if table.rows.is_empty() {
            return None;
        }

        table.widths_from_cells();
        table.insert_covered();
        Some(table)
    }

    fn parse_table_row(&mut self, reader: &mut Reader<Cursor<&[u8]>>) -> Vec<Cell> {
        let mut row = Vec::new();
        let mut buf = Vec::with_capacity(512);

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) => {
                    if e.local_name().as_ref() == b"tc" {
                        let cell = self.parse_table_cell(e, reader);
                        row.push(cell);
                    }
                }
                Event::End(ref e) if e.local_name().as_ref() == b"tr" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        row
    }

    fn parse_table_cell(
        &mut self,
        start_elem: &quick_xml::events::BytesStart,
        reader: &mut Reader<Cursor<&[u8]>>,
    ) -> Cell {
        let mut cell = Cell::default();

        for attr in start_elem.attributes().flatten() {
            if attr.key.local_name().as_ref() == b"borderFillIDRef" {
                if let Ok(id_str) = attr.unescape_value() {
                    if let Ok(id) = id_str.parse::<u32>() {
                        if let Some(&Some(color)) = self.header.border_fills.get(&id) {
                            cell.shading = Some(color);
                        }
                    }
                }
            }
        }

        let mut buf = Vec::with_capacity(512);

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"cellSpan" => {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"colSpan" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            if let Ok(span) = v.parse::<u32>() {
                                                cell.colspan = span.max(1);
                                            }
                                        }
                                    }
                                    b"rowSpan" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            if let Ok(span) = v.parse::<u32>() {
                                                cell.rowspan = span.max(1);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        b"cellSz" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"width" {
                                    if let Ok(v) = attr.unescape_value() {
                                        if let Ok(hwpu) = v.parse::<i64>() {
                                            cell.width = Some(hwpu_to_pt(hwpu));
                                        }
                                    }
                                }
                            }
                        }
                        b"p" => {
                            let mut cell_blocks = Vec::new();
                            if let Some(p) = self.parse_para(e, reader, &mut cell_blocks) {
                                cell_blocks.push(FBlock::Para(p));
                            }
                            cell.blocks.extend(cell_blocks);
                        }
                        _ => {}
                    }
                }
                Event::End(ref e) if e.local_name().as_ref() == b"tc" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        if cell.blocks.is_empty() {
            cell.blocks.push(FBlock::Para(Para::default()));
        }

        cell
    }

    fn parse_picture(&mut self, reader: &mut Reader<Cursor<&[u8]>>) -> Option<Inline> {
        let mut bin_id = None;
        let mut width_pt = 100.0f32;
        let mut height_pt = 100.0f32;
        let mut buf = Vec::with_capacity(512);

        while let Ok(event) = reader.read_event_into(&mut buf) {
            match event {
                Event::Start(ref e) | Event::Empty(ref e) => {
                    let local = e.local_name();
                    match local.as_ref() {
                        b"img" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"binItemIDRef" {
                                    bin_id = attr.unescape_value().ok().map(|v| v.to_string());
                                }
                            }
                        }
                        b"sz" => {
                            for attr in e.attributes().flatten() {
                                match attr.key.local_name().as_ref() {
                                    b"width" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            if let Ok(hwpu) = v.parse::<i64>() {
                                                width_pt = hwpu_to_pt(hwpu);
                                            }
                                        }
                                    }
                                    b"height" => {
                                        if let Ok(v) = attr.unescape_value() {
                                            if let Ok(hwpu) = v.parse::<i64>() {
                                                height_pt = hwpu_to_pt(hwpu);
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(ref e) if e.local_name().as_ref() == b"pic" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        let id = bin_id?;
        let (bytes, ext) = self.package.read_image(&id)?;
        let img = make_img(&bytes, &ext, width_pt, height_pt, String::new())?;
        Some(Inline::Image(img))
    }
}
