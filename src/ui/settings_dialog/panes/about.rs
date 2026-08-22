use gpui::{div, prelude::*, px, AnyElement, App, SharedString};

use crate::core::update;
use crate::theme::Theme;
use crate::ui::mole_mark::MoleMark;

use super::super::context::PaneContext;

/// Size of the mark on this pane. Large enough to read as the app's own icon
/// rather than as decoration on a row.
const MARK_SIZE: f32 = 64.0;

/// What the app says about itself, centered in the pane: the mark, the name and
/// what it is for, then the version this build was cut from.
///
/// The only pane that draws no heading — the mark and the name are the heading —
/// so it is also the only one that has the pane's full height to center in.
pub(in crate::ui::settings_dialog) fn rows(_ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    let theme = Theme::current(cx);
    // Taken from Cargo, so it is the version of the build in front of you and
    // cannot drift from what the update check compares against.
    let version = SharedString::from(format!("Version {}", update::current_version()));

    vec![div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(MoleMark::new(px(MARK_SIZE)))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child("Mole"),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.text_secondary)
                        .child("Local usage dashboard for AI coding agents"),
                ),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme.text_tertiary)
                .child(version),
        )
        .into_any_element()]
}
