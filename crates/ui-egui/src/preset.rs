//! UI and Keymap presets for WordCraft (Word style vs Hancom HWP style).

use serde::{Deserialize, Serialize};

/// High-level application preset: controls both UI and keymap style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppPreset {
    #[default]
    Word,
    Hwp,
}

impl AppPreset {
    pub const ALL: [AppPreset; 2] = [AppPreset::Word, AppPreset::Hwp];

    pub fn label(self) -> &'static str {
        match self {
            AppPreset::Word => "Microsoft Word 스타일 (기본)",
            AppPreset::Hwp => "한글(HWP) 스타일",
        }
    }
}

/// Shortcut keymap preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeymapPreset {
    #[default]
    Word,
    Hwp,
}

impl KeymapPreset {
    pub const ALL: [KeymapPreset; 2] = [KeymapPreset::Word, KeymapPreset::Hwp];

    pub fn label(self) -> &'static str {
        match self {
            KeymapPreset::Word => "Word 스타일 단축키 (Ctrl+D 글자, Ctrl+P 인쇄...)",
            KeymapPreset::Hwp => "한글(HWP) 스타일 단축키 (F7 편집용지, Alt+L 글자, Alt+T 문단, Ctrl+N,T 표...)",
        }
    }
}

/// Ribbon & Toolbar menu layout preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UiPreset {
    #[default]
    Word,
    Hwp,
}

impl UiPreset {
    pub const ALL: [UiPreset; 2] = [UiPreset::Word, UiPreset::Hwp];

    pub fn label(self) -> &'static str {
        match self {
            UiPreset::Word => "Word 리본 탭 (홈, 삽입, 레이아웃...)",
            UiPreset::Hwp => "한글 메뉴 탭 (파일, 편집, 보기, 입력, 서식, 쪽, 검토, 도구)",
        }
    }
}
