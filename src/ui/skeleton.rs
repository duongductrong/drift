use gpui::{div, prelude::*, px, Div, Window, App};
use crate::theme::Theme;

/// A rounded pill placeholder simulating content shape during loading.
pub fn skeleton_bar(width: f32, height: f32) -> Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded(px(height / 2.0))
}

/// A full-width 4px rounded track placeholder.
pub fn skeleton_track() -> Div {
    div().w_full().h(px(4.0)).flex_none().rounded_full()
}

/// Dashboard skeleton matching the data layout's silhouette, so the swap
/// from loading → data does not jump.
pub fn render_dashboard_skeleton(_window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = Theme::current(cx);
    let bg = theme.overlay_strong;

    let stat_card = || {
        div()
            .flex_1()
            .p(px(14.0))
            .rounded(px(8.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(skeleton_bar(60.0, 10.0).bg(bg))
            .child(skeleton_bar(90.0, 20.0).bg(bg))
    };

    let provider_group = || {
        div()
            .flex()
            .flex_col()
            .gap(px(7.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(skeleton_bar(110.0, 12.0).bg(bg))
                    .child(div().flex_1())
                    .child(skeleton_bar(56.0, 12.0).bg(bg)),
            )
            .child(skeleton_track().bg(bg))
            .child(skeleton_bar(150.0, 8.0).bg(bg))
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(20.0))
        // Stat cards row
        .child(
            div()
                .flex()
                .gap(px(12.0))
                .child(stat_card())
                .child(stat_card())
                .child(stat_card())
                .child(stat_card()),
        )
        // Provider groups
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(16.0))
                .child(provider_group())
                .child(provider_group()),
        )
        // Chart placeholder
        .child(
            div()
                .w_full()
                .h(px(224.0))
                .rounded(px(8.0))
                .bg(theme.overlay),
        )
}
