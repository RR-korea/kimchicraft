//! The HWP-style ribbon: the Hangul word processor's menu tabs (Edit, View, Insert, Format, Page,
//! Review, Tools after File). View, Insert and Review show the default ribbon's tabs of those
//! names; the others are groups of commands, drawn as Customize Ribbon draws its groups.

use wordcraft_engine::ribbon::CustomGroup;

use super::common::custom_groups;
use super::{RibbonRow, WordApp};

/// The tabs after File, in menu order.
pub const TABS: [&str; 7] = ["Edit", "View", "Insert", "Format", "Page", "Review", "Tools"];

/// The tab the HWP style opens on.
pub const HOME_TAB: &str = "Format";

/// Each tab the HWP style draws itself: its groups, each a name and its commands in order.
const GROUPS: &[(&str, &[(&str, &[&str])])] = &[
    (
        "Edit",
        &[
            ("Undo", &["edit.undo", "edit.redo"]),
            ("Clipboard", &["edit.paste", "edit.cut", "edit.copy", "edit.pasteSpecial", "edit.formatPainter"]),
            ("Find", &["edit.find", "edit.replace", "edit.goto"]),
            ("Select", &["select.all", "select.similar"]),
        ],
    ),
    (
        "Format",
        &[
            (
                "Character Shape",
                &[
                    "format.fontDialog",
                    "format.bold",
                    "format.italic",
                    "format.underline",
                    "format.strikethrough",
                    "format.superscript",
                    "format.subscript",
                    "format.growFont",
                    "format.shrinkFont",
                ],
            ),
            (
                "Paragraph Shape",
                &[
                    "para.dialog",
                    "para.justify",
                    "para.alignLeft",
                    "para.alignCenter",
                    "para.alignRight",
                    "para.indent",
                    "para.outdent",
                    "para.lineSpacing",
                ],
            ),
            ("Lists", &["para.bullets", "para.numbering"]),
            ("Styles", &["styles.pane", "styles.create", "styles.modify", "format.clear"]),
        ],
    ),
    (
        "Page",
        &[
            ("Page Setup", &["layout.pageSetup", "layout.margins", "layout.orientation", "layout.size", "layout.columns"]),
            ("Breaks", &["insert.pageBreak", "text.columnBreak", "layout.break"]),
            ("Header and Footer", &["insert.header", "insert.footer", "insert.pageNumber", "layout.pageNumberFormat"]),
            ("Section", &["layout.section", "layout.differentFirstPage", "layout.differentOddEven"]),
        ],
    ),
    (
        "Tools",
        &[
            ("Proofing", &["review.spelling", "review.wordCount", "tools.autocorrect"]),
            ("Customize", &["tools.customizeKeyboard", "tools.customizeRibbon", "file.options"]),
        ],
    ),
];

/// Draws HWP-style tab `tab`; false for a tab that is not one of its own.
pub(super) fn tab_body(row: &mut RibbonRow<'_>, app: &mut WordApp, tab: &str) -> bool {
    let Some((_, groups)) = GROUPS.iter().find(|(name, _)| *name == tab) else { return false };
    let groups: Vec<CustomGroup> = groups
        .iter()
        .map(|(name, commands)| CustomGroup { name: tl!(*name).to_string(), commands: commands.iter().map(|c| c.to_string()).collect() })
        .collect();
    custom_groups(row, app, &groups);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command an HWP-style group names is one the app knows: an unknown id would leave a
    /// silent gap in the group (Customize Ribbon's drawing skips it).
    #[test]
    fn hwp_groups_name_real_commands() {
        let app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default());
        for (tab, groups) in GROUPS {
            assert!(TABS.contains(tab), "{tab} is listed");
            for (group, commands) in *groups {
                for id in *commands {
                    assert!(app.session.registry.get(id).is_some(), "{tab} › {group}: unknown command {id}");
                }
            }
        }
    }
}
