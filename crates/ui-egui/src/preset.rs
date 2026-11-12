//! UI and keymap presets: Word style or Hancom Hangul (HWP) style. Chosen together in File ›
//! Options › General (`ui.preset`), kept between runs in `ui.json`.

use serde::{Deserialize, Serialize};

/// Shortcut keymap preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeymapPreset {
    #[default]
    Word,
    Hwp,
}

/// Ribbon tab layout preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiPreset {
    #[default]
    Word,
    Hwp,
}

impl UiPreset {
    pub const ALL: [UiPreset; 2] = [UiPreset::Word, UiPreset::Hwp];

    /// The value used by the `ui.preset` command.
    pub fn code(self) -> &'static str {
        match self {
            UiPreset::Word => "word",
            UiPreset::Hwp => "hwp",
        }
    }

    pub fn from_code(code: &str) -> Option<UiPreset> {
        UiPreset::ALL.into_iter().find(|p| p.code().eq_ignore_ascii_case(code.trim()))
    }

    /// The keymap that goes with this layout.
    pub fn keymap(self) -> KeymapPreset {
        match self {
            UiPreset::Word => KeymapPreset::Word,
            UiPreset::Hwp => KeymapPreset::Hwp,
        }
    }

    /// The ribbon tab the layout opens on.
    pub fn home_tab(self) -> &'static str {
        match self {
            UiPreset::Word => "Home",
            UiPreset::Hwp => "Format",
        }
    }

    /// The English label (translated with `tl!` where shown).
    pub fn label(self) -> &'static str {
        match self {
            UiPreset::Word => "Word (Home, Insert, Layout…)",
            UiPreset::Hwp => "Hangul (File, Edit, View, Insert, Format, Page, Review, Tools)",
        }
    }
}
