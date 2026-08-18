use gpui::{div, prelude::*, px, SharedString, Window, App};
use crate::theme::Theme;

/// A compact metric tile with three text layers: label, value, detail.
/// Used in the token breakdown strip.
#[derive(IntoElement)]
pub struct MetricTile {
    label: SharedString,
    value: SharedString,
    detail: SharedString,
}

impl MetricTile {
    pub fn new(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        detail: impl Into<SharedString>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: detail.into(),
        }
    }
}

impl RenderOnce for MetricTile {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .flex_1()
            .min_w_0()
            .px(px(14.0))
            .py(px(11.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme.text_tertiary)
                    .truncate()
                    .child(self.label),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .text_color(theme.text)
                    .truncate()
                    .child(self.value),
            )
            .child(
                div()
                    .text_size(px(9.5))
                    .text_color(theme.text_tertiary)
                    .truncate()
                    .child(self.detail),
            )
    }
}

/// Render a horizontal strip of metric tiles separated by vertical borders.
/// Each tuple is (label, value, detail).
pub fn render_metric_strip(
    tiles: Vec<(String, String, String)>,
    _window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = Theme::current(cx);

    let mut container = div()
        .mt(px(24.0))
        .border_t_1()
        .border_b_1()
        .border_color(theme.border)
        .flex();

    for (i, (label, value, detail)) in tiles.into_iter().enumerate() {
        let mut wrapper = div().flex_1();
        if i > 0 {
            wrapper = wrapper.border_l_1().border_color(theme.border);
        }
        container = container.child(wrapper.child(MetricTile::new(label, value, detail)));
    }

    container
}
