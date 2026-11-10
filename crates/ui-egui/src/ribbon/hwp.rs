//! Hancom HWP-style ribbon tabs and layout.

use egui::Ui;
use serde_json::json;

use super::common::mi;
use super::home::home;
use super::insert::insert;
use super::layout::layout;
use super::review::review;
use super::view::view;
use super::{WordApp, big, group, menu_button, small};

/// Standard tabs for HWP interface mode.
pub const HWP_TABS: [&str; 8] = [
    "File",
    "Edit",
    "View",
    "Insert",
    "Format",
    "Page",
    "Review",
    "Tools",
];

/// HWP [편집 (Edit)] tab: Undo/Redo, Clipboard, Find & Replace.
pub fn hwp_edit(app: &mut WordApp, ui: &mut Ui) {
    group(ui, "실행 취소 (Undo)", None, app, |ui, app| {
        small(ui, app, "undo", Some("실행 취소"), "실행 취소 (Ctrl+Z)", crate::widgets::ButtonAction::new("edit.undo", json!({}), false));
        small(ui, app, "redo", Some("다시 실행"), "다시 실행 (Ctrl+Y)", crate::widgets::ButtonAction::new("edit.redo", json!({}), false));
    });

    group(ui, "클립보드 (Clipboard)", Some("edit.clipboardPane"), app, |ui, app| {
        menu_button(ui, app, crate::widgets::MenuButtonOptions::new("paste", Some("붙여넣기"), "붙여넣기 (Ctrl+V)", true, "edit.paste"), |ui, app| {
            mi(ui, app, "붙여넣기", "edit.paste", json!({}));
            mi(ui, app, "텍스트만 유지", "edit.pasteText", json!({}));
            mi(ui, app, "골라 붙이기…", "edit.pasteSpecial", json!({}));
        });
        small(ui, app, "cut", Some("오려두기"), "오려두기 (Ctrl+X)", crate::widgets::ButtonAction::new("edit.cut", json!({}), false));
        small(ui, app, "copy", Some("복사하기"), "복사하기 (Ctrl+C)", crate::widgets::ButtonAction::new("edit.copy", json!({}), false));
        small(ui, app, "painter", Some("모양 복사"), "모양 복사 (Alt+C)", crate::widgets::ButtonAction::new("edit.copyFormat", json!({}), false));
    });

    group(ui, "찾기 및 바꾸기", None, app, |ui, app| {
        small(ui, app, "find", Some("찾기"), "찾기 (Ctrl+F)", crate::widgets::ButtonAction::new("edit.find", json!({}), false));
        small(ui, app, "replace", Some("바꾸기"), "바꾸기 (Ctrl+H)", crate::widgets::ButtonAction::new("edit.replace", json!({}), false));
        small(ui, app, "select_all", Some("모두 선택"), "모두 선택 (Ctrl+A)", crate::widgets::ButtonAction::new("edit.selectAll", json!({}), false));
    });
}

/// HWP [서식 (Format)] tab: Font (Alt+L) & Paragraph (Alt+T) formatting.
pub fn hwp_format(app: &mut WordApp, ui: &mut Ui) {
    // Re-use rich font and paragraph controls from Home
    home(app, ui);
}

/// HWP [입력 (Insert)] tab: Tables (Ctrl+N, T), Pictures (Ctrl+N, I), Symbols (Ctrl+F10).
pub fn hwp_insert(app: &mut WordApp, ui: &mut Ui) {
    insert(app, ui);
}

/// HWP [쪽 (Page)] tab: Page setup (F7), Margins, Columns.
pub fn hwp_page(app: &mut WordApp, ui: &mut Ui) {
    group(ui, "편집 용지 (Page Setup)", Some("layout.pageSetupDialog"), app, |ui, app| {
        big(ui, app, "pageSetup", "편집 용지\n(F7)", "layout.pageSetupDialog", json!({}), false);
    });
    layout(app, ui);
}

/// HWP [보기 (View)] tab: View modes, Rulers, Zoom, Dark page.
pub fn hwp_view(app: &mut WordApp, ui: &mut Ui) {
    view(app, ui);
}

/// HWP [검토 (Review)] tab: Spelling, Track changes, Comments.
pub fn hwp_review(app: &mut WordApp, ui: &mut Ui) {
    review(app, ui);
}

/// HWP [도구 (Tools)] tab: Shortcuts customizer, Preferences.
pub fn hwp_tools(app: &mut WordApp, ui: &mut Ui) {
    group(ui, "환경 설정 (Settings)", None, app, |ui, app| {
        big(ui, app, "options", "환경\n설정", "ui.options", json!({}), false);
        big(ui, app, "keyboard", "단축키\n설정", "ui.customizeKeyboard", json!({}), false);
    });
}
