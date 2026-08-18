use gpui::{div, prelude::*, px, SharedString, Window, App};
use crate::theme::Theme;

#[derive(IntoElement)]
pub struct EmptyState {
    title: SharedString,
    description: SharedString,
}

impl EmptyState {
    pub fn new(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
        }
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_color(theme.text)
                    .text_size(px(16.0))
                    .child(self.title),
            )
            .child(
                div()
                    .text_color(theme.text_tertiary)
                    .text_size(px(13.0))
                    .child(self.description),
            )
    }
}
