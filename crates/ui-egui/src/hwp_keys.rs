//! The HWP style's keys: the Hangul word processor's shortcuts, including its two-step keys
//! (Ctrl+N then T inserts a table). The framework's keymap binds one chord per shortcut, so these
//! are read here, from the frame's raw input before the runtime sees it, while the page has the
//! keyboard. A key the table doesn't name goes on to the keymap as usual.
//!
//! The first key of a two-step key waits for the second, with no time limit; Esc, or a second
//! key the sequence doesn't have, cancels it. Every key here runs commands, so automation reaches
//! the same features through `engine.execute`.

use egui::{Event, Key, Modifiers, RawInput};
use serde_json::Value;

use crate::WordApp;

/// One key press: a key and the modifiers held. Ctrl is the command key (⌘ on macOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    key: Key,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl Stroke {
    /// `Ctrl+N`, `Alt+Shift+L`, `F7`, `T`: modifiers, then a key by egui's name.
    fn parse(text: &str) -> Option<Stroke> {
        let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let key = Key::from_name(parts.pop()?)?;
        let mut s = Stroke { key, ctrl: false, alt: false, shift: false };
        for m in parts {
            match m {
                "Ctrl" => s.ctrl = true,
                "Alt" => s.alt = true,
                "Shift" => s.shift = true,
                _ => return None,
            }
        }
        Some(s)
    }

    fn of(key: Key, m: &Modifiers) -> Stroke {
        Stroke { key, ctrl: m.command || m.ctrl, alt: m.alt, shift: m.shift }
    }

    /// As the status bar names it.
    fn text(self) -> String {
        let mut t = String::new();
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")] {
            if on {
                t.push_str(name);
            }
        }
        t.push_str(self.key.name());
        t
    }
}

/// Where a key works. A key bound in more than one place takes the most specific one that holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// Anywhere on the page.
    Page,
    /// With the caret in a table cell.
    Table,
    /// With table cells selected (F5): plain letters act on the cells instead of typing.
    Cells,
}

/// What a key does.
#[derive(Clone, Copy, Debug)]
enum Action {
    /// Run these commands in order, each with its parameters as JSON.
    Run(&'static [(&'static str, &'static str)]),
    /// A feature WordCraft doesn't have yet (named in English): the key says so instead of doing
    /// what the default keys would.
    Missing(&'static str),
}

macro_rules! run {
    ($($c:literal $(=> $p:literal)?),+) => { Action::Run(&[$(($c, run!(@p $($p)?))),+]) };
    (@p) => { "{}" };
    (@p $p:literal) => { $p };
}

use Action::Missing;
use Place::{Cells, Page, Table};

/// The HWP style's keys, from the Hangul word processor's shortcut list (2024): the keys (a
/// second key after a comma), where they work and what they do. Keys that work as the default
/// keys do (Ctrl+C, Ctrl+Z, arrows, Home…) aren't listed: they go on to the keymap.
const KEYS: &[(&str, Place, Action)] = &[
    // File.
    ("Alt+N", Page, run!("file.new")),
    ("Alt+O", Page, run!("file.open")),
    ("Alt+S", Page, run!("file.save")),
    ("Alt+V", Page, run!("file.saveAs")),
    ("Alt+P", Page, run!("file.print")),
    ("Alt+X", Page, run!("file.close")),
    ("Ctrl+Alt+N", Page, run!("ui.backstage" => r#"{"value":true,"page":"new"}"#)),
    ("Ctrl+Q, I", Page, run!("ui.backstage" => r#"{"value":true,"page":"info"}"#)),
    ("Ctrl+Q, D", Page, run!("ui.backstage" => r#"{"value":true,"page":"info"}"#)),
    ("Ctrl+O", Page, Missing("Insert File")),
    // Moving the caret.
    ("Alt+Home", Page, run!("caret.paraUp")),
    ("Alt+End", Page, run!("caret.paraDown")),
    ("Alt+PageUp", Page, run!("caret.pageUp")),
    ("Alt+PageDown", Page, run!("caret.pageDown")),
    ("Ctrl+PageUp", Page, run!("caret.docStart")),
    ("Ctrl+PageDown", Page, run!("caret.docEnd")),
    // Deleting.
    ("Ctrl+T", Page, run!("text.deleteWordForward")),
    ("Ctrl+Y", Page, run!("select.line", "text.delete")),
    ("Alt+Y", Page, run!("caret.end" => r#"{"extend":true}"#, "text.delete")),
    ("Ctrl+E", Page, run!("text.delete")),
    // Editing.
    ("Ctrl+Alt+V", Page, run!("ui.dialog" => r#"{"name":"pasteSpecial"}"#)),
    ("Alt+C", Page, run!("edit.formatPainter")),
    ("F3", Page, run!("select.extend")),
    ("Ctrl+F", Page, run!("view.navigationPane" => r#"{"value":true}"#)),
    ("Ctrl+Q, F", Page, run!("ui.dialog" => r#"{"name":"find"}"#)),
    ("Ctrl+F2", Page, run!("ui.dialog" => r#"{"name":"replace"}"#)),
    ("Ctrl+H", Page, run!("ui.dialog" => r#"{"name":"replace"}"#)),
    ("Ctrl+Q, A", Page, run!("ui.dialog" => r#"{"name":"replace"}"#)),
    ("Ctrl+L", Page, run!("edit.findNext")),
    ("Ctrl+Q, L", Page, run!("edit.findPrevious")),
    ("Alt+G", Page, run!("ui.dialog" => r#"{"name":"goto"}"#)),
    ("F9", Page, Missing("Convert to Hanja")),
    ("Alt+F9", Page, Missing("Convert Hanja to Hangul")),
    ("Alt+I", Page, Missing("AutoText")),
    ("Ctrl+F3", Page, Missing("AutoText")),
    // View.
    ("Ctrl+G, T", Page, run!("view.marks")),
    ("Ctrl+G, C", Page, run!("view.fieldCodes")),
    ("Ctrl+G, P", Page, run!("view.zoom" => r#"{"value":"onePage"}"#)),
    ("Ctrl+G, W", Page, run!("view.zoom" => r#"{"value":"pageWidth"}"#)),
    ("Ctrl+G, Q", Page, run!("view.zoom" => r#"{"value":100}"#)),
    ("Ctrl+G, L", Page, run!("view.printLayout")),
    ("Ctrl+G, Z", Page, run!("view.fullScreen")),
    ("Ctrl+W, M", Page, Missing("Minimize All")),
    ("Ctrl+W, N", Page, Missing("Switch Pane")),
    // Insert.
    ("Ctrl+F10", Page, run!("ui.dialog" => r#"{"name":"symbol"}"#)),
    ("Ctrl+K, D", Page, run!("ui.dialog" => r#"{"name":"dateTime"}"#)),
    ("Ctrl+K, C", Page, run!("ui.dialog" => r#"{"name":"dateTime"}"#)),
    ("Ctrl+K, F", Page, run!("ui.dialog" => r#"{"name":"dateTime"}"#)),
    ("Ctrl+N, N", Page, run!("references.footnote")),
    ("Ctrl+N, E", Page, run!("references.endnote")),
    ("Ctrl+N, I", Page, run!("insert.picture")),
    ("Ctrl+N, B", Page, run!("insert.textBox")),
    ("Ctrl+N, M", Page, run!("insert.equation")),
    ("Ctrl+N, C", Page, run!("ui.dialog" => r#"{"name":"caption"}"#)),
    ("Ctrl+N, L", Page, Missing("Paragraph Rule")),
    ("Ctrl+N, O", Page, Missing("OLE Object")),
    ("Ctrl+N, K", Page, Missing("Edit Object")),
    ("Ctrl+K, B", Page, run!("ui.dialog" => r#"{"name":"bookmark"}"#)),
    ("Ctrl+K, H", Page, run!("ui.dialog" => r#"{"name":"link"}"#)),
    ("Ctrl+K, R", Page, run!("ui.dialog" => r#"{"name":"crossReference"}"#)),
    ("Ctrl+K, E", Page, run!("ui.dialog" => r#"{"name":"field"}"#)),
    // Position marks: Ctrl+K then a digit remembers the caret, Ctrl+Q then the digit goes back.
    ("Ctrl+K, 1", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark1"}"#)),
    ("Ctrl+K, 2", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark2"}"#)),
    ("Ctrl+K, 3", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark3"}"#)),
    ("Ctrl+K, 4", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark4"}"#)),
    ("Ctrl+K, 5", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark5"}"#)),
    ("Ctrl+K, 6", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark6"}"#)),
    ("Ctrl+K, 7", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark7"}"#)),
    ("Ctrl+K, 8", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark8"}"#)),
    ("Ctrl+K, 9", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark9"}"#)),
    ("Ctrl+K, 0", Page, run!("insert.bookmark" => r#"{"name":"_HwpMark0"}"#)),
    ("Ctrl+Q, 1", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark1"}"#)),
    ("Ctrl+Q, 2", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark2"}"#)),
    ("Ctrl+Q, 3", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark3"}"#)),
    ("Ctrl+Q, 4", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark4"}"#)),
    ("Ctrl+Q, 5", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark5"}"#)),
    ("Ctrl+Q, 6", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark6"}"#)),
    ("Ctrl+Q, 7", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark7"}"#)),
    ("Ctrl+Q, 8", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark8"}"#)),
    ("Ctrl+Q, 9", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark9"}"#)),
    ("Ctrl+Q, 0", Page, run!("edit.goto" => r#"{"bookmark":"_HwpMark0"}"#)),
    // Format.
    ("Alt+L", Page, run!("ui.dialog" => r#"{"name":"font"}"#)),
    ("Alt+T", Page, run!("ui.dialog" => r#"{"name":"paragraph"}"#)),
    ("F6", Page, run!("styles.pane")),
    ("Ctrl+K, N", Page, run!("ui.dialog" => r#"{"name":"defineList"}"#)),
    ("Ctrl+K, O", Page, run!("ui.dialog" => r#"{"name":"defineList"}"#)),
    ("Ctrl+Shift+Insert", Page, run!("para.numbering")),
    ("Alt+Shift+Insert", Page, run!("para.restartNumbering")),
    ("Ctrl+Shift+Delete", Page, run!("para.bullets")),
    ("Ctrl+1", Page, run!("para.style" => r#"{"style":"Normal"}"#)),
    ("Ctrl+2", Page, run!("para.style" => r#"{"style":"Body Text"}"#)),
    ("Ctrl+3", Page, run!("para.style" => r#"{"style":"Heading 1"}"#)),
    ("Ctrl+4", Page, run!("para.style" => r#"{"style":"Heading 2"}"#)),
    ("Ctrl+5", Page, run!("para.style" => r#"{"style":"Heading 3"}"#)),
    ("Ctrl+6", Page, run!("para.style" => r#"{"style":"Heading 4"}"#)),
    ("Ctrl+7", Page, run!("para.style" => r#"{"style":"Heading 5"}"#)),
    ("Ctrl+8", Page, run!("para.style" => r#"{"style":"Heading 6"}"#)),
    ("Ctrl+9", Page, run!("para.style" => r#"{"style":"Heading 7"}"#)),
    ("Ctrl+0", Page, run!("para.style" => r#"{"style":"Heading 8"}"#)),
    ("Ctrl+Minus", Page, run!("format.resetChar")),
    // Characters.
    ("Alt+Shift+E", Page, run!("format.growFont")),
    ("Ctrl+CloseBracket", Page, run!("format.growFont")),
    ("Alt+Shift+R", Page, run!("format.shrinkFont")),
    ("Ctrl+OpenBracket", Page, run!("format.shrinkFont")),
    ("Alt+Shift+U", Page, run!("format.underline")),
    ("Alt+Shift+B", Page, run!("format.bold")),
    ("Alt+Shift+I", Page, run!("format.italic")),
    ("Alt+Shift+P", Page, run!("format.superscript")),
    ("Alt+Shift+S", Page, run!("format.subscript")),
    ("Alt+Shift+C", Page, run!("format.resetChar")),
    ("Alt+Shift+W", Page, Missing("Wider Character Spacing")),
    ("Alt+Shift+N", Page, Missing("Narrower Character Spacing")),
    ("Alt+Shift+J", Page, Missing("Narrower Characters")),
    ("Alt+Shift+K", Page, Missing("Wider Characters")),
    ("Ctrl+M, K", Page, run!("format.color" => r#"{"color":"000000"}"#)),
    ("Ctrl+M, R", Page, run!("format.color" => r#"{"color":"FF0000"}"#)),
    ("Ctrl+M, B", Page, run!("format.color" => r#"{"color":"0000FF"}"#)),
    ("Ctrl+M, D", Page, run!("format.color" => r#"{"color":"800080"}"#)),
    ("Ctrl+M, G", Page, run!("format.color" => r#"{"color":"008000"}"#)),
    ("Ctrl+M, Y", Page, run!("format.color" => r#"{"color":"FFFF00"}"#)),
    ("Ctrl+M, C", Page, run!("format.color" => r#"{"color":"008080"}"#)),
    ("Ctrl+M, W", Page, run!("format.color" => r#"{"color":"FFFFFF"}"#)),
    // Paragraphs.
    ("Ctrl+Shift+M", Page, run!("para.justify")),
    ("Ctrl+Shift+L", Page, run!("para.alignLeft")),
    ("Ctrl+Shift+R", Page, run!("para.alignRight")),
    ("Ctrl+Shift+C", Page, run!("para.alignCenter")),
    ("Ctrl+Shift+T", Page, run!("para.distribute")),
    ("Ctrl+Shift+G", Page, run!("para.indent")),
    ("Ctrl+Alt+F6", Page, run!("para.indent")),
    ("Ctrl+Shift+E", Page, run!("para.outdent")),
    ("Ctrl+Alt+F5", Page, run!("para.outdent")),
    ("Alt+Shift+Z", Page, Missing("Wider Line Spacing")),
    ("Alt+Shift+A", Page, Missing("Narrower Line Spacing")),
    // Pages.
    ("F7", Page, run!("ui.dialog" => r#"{"name":"pageSetup"}"#)),
    ("Ctrl+N, H", Page, run!("insert.editHeader")),
    ("Ctrl+N, P", Page, run!("ui.dialog" => r#"{"name":"pageNumbers"}"#)),
    ("Ctrl+N, G", Page, run!("layout.section")),
    ("Ctrl+N, S", Page, Missing("Hide on This Page")),
    ("Ctrl+J", Page, run!("text.pageBreak")),
    ("Ctrl+Shift+Enter", Page, run!("text.columnBreak")),
    ("Alt+Shift+Enter", Page, run!("layout.break" => r#"{"kind":"nextPage"}"#)),
    // Tools.
    ("F8", Page, run!("review.spelling")),
    ("Ctrl+F12", Page, run!("review.thesaurus")),
    ("Shift+F8", Page, run!("ui.dialog" => r#"{"name":"autoCorrect"}"#)),
    ("Ctrl+K, U", Page, run!("references.updateToc")),
    ("Ctrl+K, A", Page, run!("references.updateToc")),
    ("Ctrl+K, I", Page, run!("ui.dialog" => r#"{"name":"markEntry"}"#)),
    ("F11", Page, run!("select.objects")),
    // Default keys the Hangul word processor gives no feature: they do nothing here.
    ("Ctrl+D", Page, Action::Run(&[])),
    ("Ctrl+R", Page, Action::Run(&[])),
    // Tables.
    ("Ctrl+N, T", Page, run!("ui.dialog" => r#"{"name":"insertTable"}"#)),
    ("Ctrl+N, A", Table, run!("table.splitTable")),
    ("Ctrl+N, Z", Table, Missing("Join Tables")),
    ("Ctrl+N, F", Table, run!("ui.dialog" => r#"{"name":"tableFormula"}"#)),
    ("F5", Table, run!("table.selectCell")),
    ("Ctrl+Enter", Table, run!("table.insertRowBelow")),
    ("Ctrl+Backspace", Table, run!("table.deleteRow")),
    ("Alt+Insert", Table, run!("table.insertRowBelow")),
    ("Alt+Delete", Table, run!("table.deleteRow")),
    ("F5", Cells, run!("table.selectTable")),
    ("F7", Cells, run!("table.selectColumn")),
    ("F8", Cells, run!("table.selectRow")),
    ("Escape", Cells, run!("select.collapse")),
    ("Enter", Cells, run!("select.collapse")),
    ("M", Cells, run!("table.merge")),
    ("S", Cells, run!("ui.dialog" => r#"{"name":"splitCells"}"#)),
    ("P", Cells, run!("ui.dialog" => r#"{"name":"tableProperties"}"#)),
    ("L", Cells, run!("ui.dialog" => r#"{"name":"borders"}"#)),
    ("B", Cells, run!("ui.dialog" => r#"{"name":"borders"}"#)),
    ("C", Cells, run!("ui.dialog" => r#"{"name":"shading"}"#)),
    ("F", Cells, run!("ui.dialog" => r#"{"name":"shading"}"#)),
    ("H", Cells, run!("table.distributeRows")),
    ("W", Cells, run!("table.distributeColumns")),
    ("A", Cells, Missing("Fill Cells")),
];

/// A row of [`KEYS`], read.
struct Binding {
    first: Stroke,
    second: Option<Stroke>,
    place: Place,
    action: Action,
}

fn bindings() -> impl Iterator<Item = Binding> {
    KEYS.iter().filter_map(|(keys, place, action)| {
        let mut strokes = keys.split(',').map(Stroke::parse);
        let first = strokes.next()??;
        let second = match strokes.next() {
            Some(s) => Some(s?),
            None => None,
        };
        Some(Binding { first, second, place: *place, action: *action })
    })
}

/// Every key of the HWP style as its keys and the commands it runs (none for a feature WordCraft
/// doesn't have yet), in table order.
pub fn keys() -> impl Iterator<Item = (&'static str, Vec<&'static str>)> {
    KEYS.iter().map(|(keys, _, action)| {
        let commands = match action {
            Action::Run(steps) => steps.iter().map(|(c, _)| *c).collect(),
            Action::Missing(_) => Vec::new(),
        };
        (*keys, commands)
    })
}

/// What a press does in the HWP style.
enum Outcome {
    /// Not an HWP-style key: the runtime has it.
    Pass,
    /// The first key of a two-step key: wait for the second.
    Wait(Stroke),
    /// Do what the key does; the keys as the status bar names them.
    Act(Action, String),
    /// Swallowed: Esc or a key the waiting sequence doesn't have.
    Cancel,
}

/// The places that hold, most specific first.
fn places(in_table: bool, in_cells: bool) -> impl Iterator<Item = Place> {
    [(in_cells, Cells), (in_table, Table), (true, Page)].into_iter().filter(|(on, _)| *on).map(|(_, p)| p)
}

fn outcome(pending: Option<Stroke>, press: Stroke, in_table: bool, in_cells: bool) -> Outcome {
    if let Some(first) = pending {
        // The second key is read with or without Shift, as Caps Lock or a held Shift doesn't change it.
        let plain = Stroke { shift: false, ..press };
        let found = places(in_table, in_cells)
            .find_map(|place| bindings().find(|b| b.place == place && b.first == first && b.second.is_some_and(|s| s == plain || s == press)));
        return match found {
            Some(b) => Outcome::Act(b.action, format!("{}, {}", first.text(), plain.text())),
            None => Outcome::Cancel,
        };
    }
    for place in places(in_table, in_cells) {
        if bindings().any(|b| b.place == place && b.first == press && b.second.is_some()) {
            return Outcome::Wait(press);
        }
        if let Some(b) = bindings().find(|b| b.place == place && b.first == press && b.second.is_none()) {
            return Outcome::Act(b.action, press.text());
        }
    }
    Outcome::Pass
}

/// From the host's raw-input hook, before `embed::before_input`: [`take_keys`] with the page's
/// focus as the last pass left it.
pub fn before_input(app: &mut WordApp, ctx: &egui::Context, raw: &mut RawInput) {
    let page_focused = app.canvas.focused(ctx);
    take_keys(app, page_focused, raw);
}

/// Takes the HWP style's keys out of `raw` and does what they do, before the runtime reads the
/// frame. Only in the HWP style, with the page holding the keyboard (`page_focused`) and no
/// dialog or File page open; anywhere else a waiting first key is dropped.
pub fn take_keys(app: &mut WordApp, page_focused: bool, raw: &mut RawInput) {
    if app.ui.ui_preset != crate::preset::UiPreset::Hwp || !page_focused || app.modal_open() || app.ui.backstage {
        app.canvas.key_sequence = None;
        return;
    }
    let in_cells = app.session.cell_range().is_some();
    let in_table = in_cells || app.session.sel.focus.path.cell().is_some();
    // The text and input-method input a taken key press would type: dropped with it.
    let mut swallow = false;
    let mut acts = Vec::new();
    raw.events.retain(|e| match e {
        Event::Key { key, physical_key, pressed: true, modifiers, .. } => {
            swallow = false;
            // The physical key, so a key works whatever the keyboard layout or the input method types.
            let press = Stroke::of(physical_key.unwrap_or(*key), modifiers);
            match outcome(app.canvas.key_sequence, press, in_table, in_cells) {
                Outcome::Pass => true,
                Outcome::Wait(first) => {
                    app.canvas.key_sequence = Some(first);
                    app.status(format!("{} — {}", first.text(), tl!("press the next key, or Esc to cancel")));
                    swallow = true;
                    false
                }
                Outcome::Act(action, keys) => {
                    app.canvas.key_sequence = None;
                    acts.push((action, keys));
                    swallow = true;
                    false
                }
                Outcome::Cancel => {
                    app.canvas.key_sequence = None;
                    app.status(String::new());
                    swallow = true;
                    false
                }
            }
        }
        Event::Text(_) | Event::Ime(_) => !swallow && app.canvas.key_sequence.is_none(),
        _ => true,
    });
    for (action, keys) in acts {
        match action {
            Action::Run(steps) => {
                for (command, params) in steps {
                    let params: Value = serde_json::from_str(params).unwrap_or_default();
                    if let Err(e) = app.run(command, params) {
                        app.status(e);
                        break;
                    }
                }
            }
            Action::Missing(feature) => app.status(format!("{keys}: {} — {}", tl!(feature), tl!("not available yet"))),
        }
    }
}

#[cfg(test)]
mod tests;
