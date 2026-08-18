use gpui::{div, prelude::*, px, Hsla, SharedString, Window, App};
use crate::theme::Theme;

/// A model breakdown row with colored provider dot, name, cost, and tokens.
#[derive(IntoElement)]
pub struct ModelRow {
    id: SharedString,
    name: SharedString,
    cost: SharedString,
    tokens: SharedString,
    provider_color: Hsla,
}

impl ModelRow {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        cost: impl Into<SharedString>,
        tokens: impl Into<SharedString>,
        provider_color: Hsla,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cost: cost.into(),
            tokens: tokens.into(),
            provider_color,
        }
    }
}

impl RenderOnce for ModelRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .id(self.id)
            .flex()
            .justify_between()
            .items_center()
            .py(px(6.0))
            .px(px(4.0))
            .rounded(px(4.0))
            .hover(|style| style.bg(theme.overlay))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .flex_none()
                            .rounded_full()
                            .bg(self.provider_color),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.text)
                            .child(self.name),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.text_secondary)
                    .child(SharedString::from(format!(
                        "{} · {}",
                        self.cost, self.tokens
                    ))),
            )
    }
}
