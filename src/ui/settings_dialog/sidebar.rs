use std::sync::Arc;

use gpui::{div, prelude::*, px, App, SharedString, Window};

use crate::theme::Theme;

use super::category::SettingsCategory;

// ---------------------------------------------------------------------------
// Sidebar — the dialog's navigation, and nothing else.
//
// It reports the entry that was clicked and draws the one it is told is
// selected; it holds no state, so which pane is open is decided in exactly one
// place — the root view — rather than being split between here and there.
// ---------------------------------------------------------------------------

/// Width of the navigation column. Enough for "Data Sources" on one line at the
/// dialog's text size, with the padding that keeps a selected row from looking
/// cramped.
pub(super) const SIDEBAR_WIDTH: f32 = 146.0;

pub(super) type SelectCallback = Arc<dyn Fn(SettingsCategory, &mut Window, &mut App) + Send + Sync>;

pub(super) fn sidebar(
    selected: SettingsCategory,
    on_select: Option<SelectCallback>,
    cx: &App,
) -> impl IntoElement {
    let theme = Theme::current(cx);

    // The column shares the sheet's canvas — the divider is what separates it
    // from the pane, so nothing here paints a surface of its own.
    div()
        .flex_none()
        .w(px(SIDEBAR_WIDTH))
        .p(px(8.0))
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .gap(px(1.0))
        .children(entries(selected, &on_select, cx))
}

/// The entries, in `SettingsCategory::ALL` order.
fn entries(
    selected: SettingsCategory,
    on_select: &Option<SelectCallback>,
    cx: &App,
) -> Vec<gpui::AnyElement> {
    SettingsCategory::ALL
        .into_iter()
        .map(|category| {
            entry(category, category == selected, on_select.clone(), cx).into_any_element()
        })
        .collect()
}

fn entry(
    category: SettingsCategory,
    selected: bool,
    on_select: Option<SelectCallback>,
    cx: &App,
) -> impl IntoElement {
    let theme = Theme::current(cx);

    div()
        .id(SharedString::from(format!(
            "settings-nav-{}",
            category.key()
        )))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .cursor_default()
        .text_size(px(11.5))
        .when(selected, |el| {
            el.bg(theme.overlay_strong)
                .text_color(theme.text)
                .font_weight(gpui::FontWeight::MEDIUM)
        })
        .when(!selected, |el| {
            el.text_color(theme.text_secondary)
                .hover(|style| style.bg(theme.overlay).text_color(theme.text))
        })
        .child(category.label())
        .on_click(move |_event, window, cx| {
            if let Some(handler) = &on_select {
                handler(category, window, cx);
            }
        })
}
