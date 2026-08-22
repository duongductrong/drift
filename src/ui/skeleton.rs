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
///
/// It mirrors the *default* page — Chart view: header controls, four stat
/// cards, the provider column beside the chart block, the token strip, and
/// the leading model rows. Blocks are placed where their real counterparts
/// land, at the same heights (the chart placeholder is exactly the chart's
/// 224pt plot), so nothing shifts when real content replaces these bars.
pub fn render_dashboard_skeleton(_window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = Theme::current(cx);
    let bg = theme.overlay_strong;

    // ── Header: caption taking the slack, filter pills pinned right ──
    let header = div()
        .flex()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(skeleton_bar(260.0, 11.0).bg(bg)),
        )
        .child(skeleton_bar(118.0, 26.0).bg(bg))
        .child(skeleton_bar(196.0, 26.0).bg(bg));

    // ── Stat cards ────────────────────────────────────────────────
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

    // ── Provider column: section title, then share rows ──────────
    let provider_row = || {
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

    let provider_section = div()
        .w(px(300.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(16.0))
        .child(skeleton_bar(92.0, 12.0).bg(bg))
        .child(provider_row())
        .child(provider_row())
        .child(provider_row());

    // ── Chart block: title + legend, the plot itself, the switch ──
    //
    // The placeholder box is the chart's exact plot height, so the loading
    // page and the data page end this section at the same y.
    let chart_block = div()
        .flex_1()
        .min_w(px(320.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(skeleton_bar(78.0, 13.0).bg(bg))
                .child(skeleton_bar(190.0, 9.0).bg(bg)),
        )
        .child(
            div()
                .w_full()
                .h(px(224.0))
                .rounded(px(8.0))
                .bg(theme.overlay),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .child(skeleton_bar(104.0, 22.0).bg(bg)),
        );

    // ── Token strip: bordered tiles separated by hairlines ───────
    let tile = |leading_border: bool| {
        let wrapper = div()
            .flex_1()
            .py(px(11.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(skeleton_bar(84.0, 10.0).bg(bg))
            .child(skeleton_bar(64.0, 15.0).bg(bg))
            .child(skeleton_bar(110.0, 9.5).bg(bg));
        if leading_border {
            wrapper.border_l_1().border_color(theme.border)
        } else {
            wrapper
        }
    };

    let metric_strip = div()
        .border_t_1()
        .border_b_1()
        .border_color(theme.border)
        .flex()
        .child(tile(false))
        .child(tile(true))
        .child(tile(true))
        .child(tile(true));

    // ── Model rows: swatch + name left, value right ──────────────
    let model_row = || {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(7.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .size(px(7.0))
                            .flex_none()
                            .rounded_full()
                            .bg(bg),
                    )
                    .child(skeleton_bar(130.0, 11.0).bg(bg)),
            )
            .child(skeleton_bar(96.0, 11.0).bg(bg))
    };

    let model_section = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(skeleton_bar(72.0, 12.0).bg(bg))
        .children((0..5).map(|_| model_row()));

    // ── Compose in the page's own order ──────────────────────────
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(20.0))
        .child(header)
        .child(
            div()
                .flex()
                .gap(px(12.0))
                .child(stat_card())
                .child(stat_card())
                .child(stat_card())
                .child(stat_card()),
        )
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(28.0))
                .child(provider_section)
                .child(chart_block),
        )
        .child(metric_strip)
        .child(model_section)
}
