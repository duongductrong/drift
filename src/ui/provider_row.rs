use gpui::{div, prelude::*, px, relative, Hsla, SharedString, Window, App};
use crate::theme::Theme;

/// A provider share row showing brand-colored indicator, name, value,
/// share bar, and optional detail caption.
#[derive(IntoElement)]
pub struct ProviderRow {
    name: SharedString,
    value: SharedString,
    detail: Option<SharedString>,
    share: f32,
    color: Hsla,
}

impl ProviderRow {
    pub fn new(
        name: impl Into<SharedString>,
        value: impl Into<SharedString>,
        share: f32,
        color: Hsla,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            detail: None,
            share: share.clamp(0.0, 1.0),
            color,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for ProviderRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

        let mut container = div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .flex_none()
                            .rounded_full()
                            .bg(self.color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.5))
                            .text_color(theme.text)
                            .child(self.name),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(theme.text)
                            .child(self.value),
                    ),
            )
            .child(
                div()
                    .h(px(4.0))
                    .w_full()
                    .rounded_full()
                    .bg(theme.overlay_strong)
                    .child(
                        div()
                            .h_full()
                            .w(relative(self.share))
                            .rounded_full()
                            .bg(self.color),
                    ),
            );

        if let Some(detail) = self.detail {
            container = container.child(
                div()
                    .text_size(px(10.5))
                    .text_color(theme.text_tertiary)
                    .child(detail),
            );
        }

        container
    }
}
