//! Multi-step chord shortcut handling for HWP mode (e.g., Ctrl+N, T for Insert Table).

use egui::{Key, Modifiers};
use serde_json::json;

use crate::WordApp;
use crate::preset::KeymapPreset;

/// Pending chord prefix key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChordPrefix {
    #[default]
    None,
    /// Ctrl+N in HWP: followed by T (table), I (image), F (footnote), etc.
    CtrlN,
    /// Ctrl+K in HWP: followed by H (hyperlink), etc.
    CtrlK,
}

/// Active chord tracking state.
#[derive(Clone, Debug, Default)]
pub struct ChordState {
    pub prefix: ChordPrefix,
    pub started_at: f64,
}

impl ChordState {
    pub fn is_active(&self, now: f64) -> bool {
        self.prefix != ChordPrefix::None && (now - self.started_at) < 2.5
    }

    pub fn reset(&mut self) {
        self.prefix = ChordPrefix::None;
        self.started_at = 0.0;
    }
}

/// Process key events for chord sequences in HWP mode.
/// Returns true if the key was consumed by the chord state machine.
pub fn handle_chord(app: &mut WordApp, ctx: &egui::Context, key: Key, modifiers: Modifiers) -> bool {
    if app.ui.keymap_preset != KeymapPreset::Hwp {
        return false;
    }

    let now = ctx.input(|i| i.time);

    // If chord active, handle second key
    if app.canvas.chord.is_active(now) {
        let prefix = app.canvas.chord.prefix;
        app.canvas.chord.reset();
        app.canvas.swallow_text = true;

        match prefix {
            ChordPrefix::CtrlN => {
                match key {
                    // T: Table
                    Key::T => {
                        let _ = app.run("ui.dialog", json!({"name": "insertTable"}));
                        return true;
                    }
                    // I: Image / Picture
                    Key::I => {
                        let _ = app.run("insert.picture", json!({}));
                        return true;
                    }
                    // F: Footnote
                    Key::F => {
                        let _ = app.run("references.footnote", json!({}));
                        return true;
                    }
                    // H: Header
                    Key::H => {
                        let _ = app.run("insert.header", json!({}));
                        return true;
                    }
                    // Escape cancels chord
                    Key::Escape => return true,
                    _ => return false,
                }
            }
            ChordPrefix::CtrlK => match key {
                Key::H => {
                    let _ = app.run("ui.dialog", json!({"name": "link"}));
                    return true;
                }
                Key::Escape => return true,
                _ => return false,
            },
            ChordPrefix::None => {}
        }
    }

    // Check for chord start
    let is_ctrl = (modifiers.ctrl || modifiers.command) && !modifiers.alt && !modifiers.shift;
    if is_ctrl {
        if key == Key::N {
            app.canvas.chord = ChordState { prefix: ChordPrefix::CtrlN, started_at: now };
            app.canvas.swallow_text = true;
            return true;
        } else if key == Key::K {
            app.canvas.chord = ChordState { prefix: ChordPrefix::CtrlK, started_at: now };
            app.canvas.swallow_text = true;
            return true;
        }
    }

    false
}
