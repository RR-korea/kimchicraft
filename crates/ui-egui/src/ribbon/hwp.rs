//! The HWP-style ribbon: the Hangul word processor's menu tabs (Edit, View, Insert, Format, Page,
//! Review, Tools after File). View, Insert and Review show the default ribbon's tabs of those
//! names; Edit, Format, Page and Tools gather the default ribbon's groups as the word processor
//! sorts them, so they look and work as they do on the default tabs. Tab names are looked up in
//! the `hwp` context first, for the word processor's own terms.

use craft::behavior::IconId;
use craft::keymap::CommandRef;
use craft::menu::ribbon::RibbonGroup;
use craft::ui::buttons::{Arrangement, Button};
use craft::ui::contract::{Gap, Show as _, ShowContainer as _};
use craft::ui::layout_components::Stack;

use super::{RibbonRow, WordApp, home, insert, layout, review};

/// The tabs after File, in menu order.
pub const TABS: [&str; 7] = ["Edit", "View", "Insert", "Format", "Page", "Review", "Tools"];

/// The tab the HWP style opens on.
pub const HOME_TAB: &str = "Format";

/// The Tools tab's settings group: Options large, then the keyboard and ribbon settings, which
/// the default ribbon keeps in File › Options.
fn customize_group(row: &mut RibbonRow<'_>) {
    row.group(RibbonGroup::new("hwp.customize", crate::i18n::tc("hwp", "Customize").to_string(), IconId::new("options").into()), |ui| {
        Button::command(CommandRef::new("file.options")).icon(IconId::new("options")).arrangement(Arrangement::Stacked).show(ui);
        Stack::new().gap(Gap::Tight).show(ui, |ui, _| {
            for id in ["tools.customizeKeyboard", "tools.customizeRibbon"] {
                Button::command(CommandRef::new(id)).show(ui);
            }
        });
    });
}

/// Draws HWP-style tab `tab`; false for a tab that is not one of its own.
pub(super) fn tab_body(row: &mut RibbonRow<'_>, app: &mut WordApp, tab: &str) -> bool {
    match tab {
        "Edit" => {
            home::clipboard_group(row, app);
            home::editing_group(row, app);
        }
        "Format" => {
            home::font_group(row, app);
            home::paragraph_group(row, app);
            home::styles_group(row, app);
        }
        "Page" => {
            layout::page_setup_group(row, app);
            insert::pages_group(row, app);
            insert::header_footer_group(row, app);
        }
        "Tools" => {
            review::proofing_group(row, app);
            review::language_group(row, app);
            customize_group(row);
        }
        _ => return false,
    }
    true
}
