use gpui::{div, prelude::*, px, AnyView, App, SharedString, Window};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Tooltip — the label an icon-only control needs to stay legible.
//
// GPUI builds tooltips from a view rather than an element, so this is an
// entity: `Tooltip::build` hands back the `AnyView` that
// `InteractiveElement::tooltip` expects.
//
// Usage:
//   div().tooltip(Tooltip::text("Scan transcripts"))
// ---------------------------------------------------------------------------

pub struct Tooltip {
    label: SharedString,
}

impl Tooltip {
    /// A builder suitable for `InteractiveElement::tooltip`.
    pub fn text(
        label: impl Into<SharedString>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let label = label.into();
        move |_window, cx| {
            let label = label.clone();
            cx.new(|_| Tooltip { label }).into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        // Offset from the pointer so the tooltip never sits under the cursor
        // that summoned it.
        div().mt(px(6.0)).child(
            div()
                .px(px(7.0))
                .py(px(3.0))
                .rounded(px(5.0))
                .bg(theme.canvas)
                .border_1()
                .border_color(theme.border_strong)
                .shadow_md()
                .text_size(px(10.5))
                .text_color(theme.text_secondary)
                .child(self.label.clone()),
        )
    }
}
