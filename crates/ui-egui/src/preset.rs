//! The interface style: the default ribbon and shortcuts, or the HWP style, whose menu tabs and
//! keys (two-step keys such as Ctrl+N then T among them) follow the Hangul word processor. Chosen
//! in File › Options › General (`ui.preset`) and kept between runs.

use serde::{Deserialize, Serialize};

/// Which ribbon tabs and shortcuts the window uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum UiPreset {
    #[default]
    Default,
    Hwp,
}

impl UiPreset {
    /// In menu order.
    pub const ALL: [UiPreset; 2] = [UiPreset::Default, UiPreset::Hwp];

    /// The value used in the settings and by the `ui.preset` command.
    pub fn code(self) -> &'static str {
        match self {
            UiPreset::Default => "default",
            UiPreset::Hwp => "hwp",
        }
    }

    /// A preset by its code, any case. `word`, the default's name in earlier builds, reads as the default.
    pub fn from_code(code: &str) -> Option<UiPreset> {
        let code = code.trim();
        if code.eq_ignore_ascii_case("word") {
            return Some(UiPreset::Default);
        }
        UiPreset::ALL.into_iter().find(|p| p.code().eq_ignore_ascii_case(code))
    }

    /// The English label (translated with `tl!` where shown).
    pub fn label(self) -> &'static str {
        match self {
            UiPreset::Default => "Default",
            UiPreset::Hwp => "HWP style",
        }
    }

    /// The ribbon tab the style opens on.
    pub fn home_tab(self) -> &'static str {
        match self {
            UiPreset::Default => crate::DEFAULT_TAB,
            UiPreset::Hwp => crate::ribbon::hwp::HOME_TAB,
        }
    }
}

impl From<UiPreset> for String {
    fn from(p: UiPreset) -> Self {
        p.code().to_string()
    }
}

/// A code that is neither preset is an error, so a settings reader reports it and the field reads
/// as the default.
impl TryFrom<String> for UiPreset {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        UiPreset::from_code(&s).ok_or_else(|| format!("unknown interface style {s:?}"))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::UiPreset;
    use crate::prefs::{Prefs, legacy_ui_json};
    use crate::{Services, WordApp};

    /// `ui.preset` reads and sets the style, refuses a value it doesn't know and opens the style's
    /// first tab; the choice is saved, and a `ui.json` of an earlier build (`word`, `hwp`) or junk reads.
    #[test]
    fn the_interface_style_is_a_ui_command_kept_between_runs() {
        let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Services::default());
        assert_eq!(a.run("ui.preset", json!({})).unwrap(), json!({"preset": "default"}));
        assert!(a.run("ui.preset", json!({"value": "emacs"})).is_err());
        assert_eq!(a.run("ui.preset", json!({"value": " HWP "})).unwrap()["preset"], "hwp");
        assert_eq!(a.ui.tab, crate::ribbon::hwp::HOME_TAB);
        let saved: Prefs = serde_json::from_value(serde_json::to_value(a.prefs()).unwrap()).unwrap();
        assert_eq!(saved.ui_preset, UiPreset::Hwp, "kept between runs");
        let read = |text: &str| legacy_ui_json(format!(r#"{{"uiPreset": {text}}}"#).as_bytes()).unwrap().0.ui_preset;
        assert_eq!(read(r#""hwp""#), UiPreset::Hwp);
        assert_eq!(read(r#""word""#), UiPreset::Default);
        assert_eq!(read(r#""emacs""#), UiPreset::Default);
    }
}
