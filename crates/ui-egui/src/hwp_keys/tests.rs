use egui::{Event, Key, Modifiers, RawInput};
use serde_json::json;

use super::*;

fn app() -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default())
}

fn press(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }
}

fn ctrl() -> Modifiers {
    Modifiers { ctrl: true, command: true, ..Default::default() }
}

fn footnotes(a: &WordApp) -> usize {
    a.session.doc.parts.values().filter(|p| p.kind == wordcraft_doc::PartKind::Footnote).count()
}

/// Every row of the table reads, and no two rows give the same keys in the same place: a typo in
/// a key name would leave the key silently unbound.
#[test]
fn every_key_reads_once() {
    assert_eq!(bindings().count(), KEYS.len());
    let mut seen = std::collections::HashSet::new();
    for (keys, place, _) in KEYS {
        assert!(seen.insert((*keys, *place as u8)), "{keys} twice");
    }
}

/// Every command a key runs is one the app knows, and every dialog it opens exists: a wrong id
/// would do nothing, as Ctrl+N then F once did.
#[test]
fn hwp_keys_name_real_commands() {
    let mut a = app();
    // In a table cell, where the table keys' dialogs open too.
    a.run("insert.table", json!({"rows": 2, "cols": 2})).unwrap();
    for (keys, _, action) in KEYS {
        let Action::Run(steps) = action else { continue };
        for (id, params) in *steps {
            if *id == "ui.dialog" {
                let params: Value = serde_json::from_str(params).expect("params");
                let opened = a.run(id, params.clone()).unwrap_or_else(|e| panic!("{keys}: {params}: {e}"));
                assert!(opened["dialog"].is_string(), "{keys}: no dialog {params}");
                a.run("ui.closeDialog", json!({})).unwrap();
            } else {
                assert!(a.session.registry.get(id).is_some() || id.starts_with("ui."), "{keys}: unknown command {id}");
            }
        }
    }
}

/// Ctrl+N then N inserts a footnote in the HWP style: both presses and the letter the second one
/// would type are taken, so nothing is typed and the default Ctrl+N (New) doesn't run.
#[test]
fn two_step_keys_run_their_command() {
    let mut a = app();
    a.run("ui.preset", json!({"value": "hwp"})).unwrap();
    let mut raw = RawInput { events: vec![press(Key::N, ctrl())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert!(raw.events.is_empty(), "the first key is taken");
    assert!(a.canvas.key_sequence.is_some(), "and waits for the second");
    assert!(a.status_message().is_some(), "the status bar says so");
    let mut raw = RawInput { events: vec![press(Key::N, Modifiers::NONE), Event::Text("n".into())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert!(raw.events.is_empty(), "the second key and its text are taken");
    assert_eq!(footnotes(&a), 1, "Ctrl+N, N inserted a footnote");
    assert!(a.canvas.key_sequence.is_none());
    assert!(a.status_message().is_none(), "the hint goes once the key ran");
}

/// Esc, or a second key the sequence doesn't have, ends the wait without typing.
#[test]
fn an_unknown_second_key_cancels() {
    let mut a = app();
    a.run("ui.preset", json!({"value": "hwp"})).unwrap();
    let mut raw = RawInput { events: vec![press(Key::N, ctrl()), press(Key::J, Modifiers::NONE), Event::Text("j".into())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert!(raw.events.is_empty());
    assert!(a.canvas.key_sequence.is_none());
    assert_eq!(footnotes(&a), 0);
}

/// The default style, or the keyboard elsewhere than the page, leaves every key to the keymap.
#[test]
fn keys_pass_outside_the_hwp_style_or_the_page() {
    let mut a = app();
    let events = vec![press(Key::N, ctrl()), press(Key::L, Modifiers { alt: true, ..Default::default() })];
    let mut raw = RawInput { events: events.clone(), ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert_eq!(raw.events.len(), events.len(), "default style");
    a.run("ui.preset", json!({"value": "hwp"})).unwrap();
    let mut raw = RawInput { events: events.clone(), ..Default::default() };
    take_keys(&mut a, false, &mut raw);
    assert_eq!(raw.events.len(), events.len(), "page not focused");
    // A key the table doesn't name goes on in the HWP style too.
    let mut raw = RawInput { events: vec![press(Key::B, ctrl())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert_eq!(raw.events.len(), 1, "Ctrl+B is the keymap's");
}

/// With cells selected, M merges them; with the caret in text, M types.
#[test]
fn cell_keys_act_only_on_selected_cells() {
    let mut a = app();
    a.run("ui.preset", json!({"value": "hwp"})).unwrap();
    a.run("insert.table", json!({"rows": 2, "cols": 2})).unwrap();
    let mut raw = RawInput { events: vec![press(Key::M, Modifiers::NONE), Event::Text("m".into())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert_eq!(raw.events.len(), 2, "M types in a cell");
    a.run("table.selectRow", json!({})).unwrap();
    assert!(a.session.cell_range().is_some());
    let cells = |a: &WordApp| a.session.doc.body.iter().find_map(|b| b.as_table().map(|t| t.rows[0].cells.len()));
    let before = cells(&a);
    let mut raw = RawInput { events: vec![press(Key::M, Modifiers::NONE), Event::Text("m".into())], ..Default::default() };
    take_keys(&mut a, true, &mut raw);
    assert!(raw.events.is_empty(), "M is taken");
    assert!(cells(&a) < before, "the row's cells merged");
}

/// A window that nothing in has the keyboard yet (just opened, or raised before a click) still
/// takes the HWP keys: Ctrl+N waits for its second key instead of opening a new document.
#[test]
fn keys_are_taken_before_anything_has_the_keyboard() {
    let mut a = app();
    a.run("ui.preset", json!({"value": "hwp"})).unwrap();
    let ctx = egui::Context::default();
    let mut raw = RawInput { events: vec![press(Key::N, ctrl())], ..Default::default() };
    before_input(&mut a, &ctx, &mut raw);
    assert!(raw.events.is_empty());
    assert!(a.canvas.key_sequence.is_some());
}

/// Every feature a key reports missing, and the status bar's words, have a Korean row in the
/// fork's catalog (craft-i18n doesn't carry the HWP style's words).
#[test]
fn hwp_words_have_korean_rows() {
    let rows = include_str!("../../i18n/ko/kimchicraft.tsv");
    let has = |source: &str| rows.lines().any(|l| l.split('\t').nth(1) == Some(source));
    let words = KEYS.iter().filter_map(|(_, _, a)| if let Action::Missing(f) = a { Some(*f) } else { None });
    for word in words.chain(["press the next key, or Esc to cancel", "not available yet"]) {
        assert!(has(word), "no Korean row for {word:?}");
    }
}
