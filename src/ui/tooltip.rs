use gpui::{div, prelude::*, px, AnyView, App, Div, Hsla, SharedString, Window};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Tooltip — the label an icon-only control needs to stay legible, and the
// breakdown a chart bar needs to be readable without a table beside it.
//
// GPUI builds tooltips from a view rather than an element, so these are
// entities: the `Tooltip::*` constructors hand back the `AnyView` builder that
// `InteractiveElement::tooltip` expects.
//
// Usage:
//   div().tooltip(Tooltip::text("Scan transcripts"))
//   div().tooltip(Tooltip::detail("Aug 18, 2026", "$12.34", rows))
// ---------------------------------------------------------------------------

/// The shared card: bordered, shadowed, and pushed clear of the pointer that
/// summoned it so the tooltip never sits under the cursor.
fn card(theme: &Theme) -> Div {
    div()
        .px(px(7.0))
        .py(px(5.0))
        .rounded(px(5.0))
        .bg(theme.canvas)
        .border_1()
        .border_color(theme.border_strong)
        .shadow_md()
}

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

    /// A titled tooltip with a headline value and a color-keyed breakdown
    /// under it — what a stacked bar is made of, in the order it is stacked.
    pub fn detail(
        title: impl Into<SharedString>,
        value: impl Into<SharedString>,
        rows: Vec<TooltipRow>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let title = title.into();
        let value = value.into();
        move |_window, cx| {
            let tooltip = DetailTooltip {
                title: title.clone(),
                value: value.clone(),
                rows: rows.clone(),
            };
            cx.new(|_| tooltip).into()
        }
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);
        div().mt(px(6.0)).child(
            card(&theme)
                .py(px(3.0))
                .text_size(px(10.5))
                .text_color(theme.text_secondary)
                .child(self.label.clone()),
        )
    }
}

/// One line of a [`Tooltip::detail`] breakdown: a series swatch, what it names,
/// and its value in the units the headline is quoted in.
#[derive(Clone)]
pub struct TooltipRow {
    pub color: Hsla,
    pub label: SharedString,
    pub value: SharedString,
}

struct DetailTooltip {
    title: SharedString,
    value: SharedString,
    rows: Vec<TooltipRow>,
}

impl Render for DetailTooltip {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);

        // Label and value are pushed to opposite edges, so a stack of rows
        // reads as two columns rather than ragged text.
        let mut body = div().flex().flex_col().gap(px(2.0));
        for row in &self.rows {
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(14.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(
                                div()
                                    .w(px(7.0))
                                    .h(px(7.0))
                                    .flex_none()
                                    .rounded_full()
                                    .bg(row.color),
                            )
                            .child(
                                div()
                                    .text_color(theme.text_secondary)
                                    .child(row.label.clone()),
                            ),
                    )
                    .child(div().text_color(theme.text).child(row.value.clone())),
            );
        }

        div().mt(px(6.0)).child(
            card(&theme)
                .px(px(9.0))
                .py(px(7.0))
                .min_w(px(132.0))
                .flex()
                .flex_col()
                .gap(px(5.0))
                .text_size(px(10.5))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(9.5))
                                .text_color(theme.text_tertiary)
                                .child(self.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child(self.value.clone()),
                        ),
                )
                // A period with nothing in it carries no breakdown, and the
                // headline already says so.
                .when(!self.rows.is_empty(), |el| {
                    el.child(
                        div()
                            .pt(px(5.0))
                            .border_t_1()
                            .border_color(theme.border)
                            .child(body),
                    )
                }),
        )
    }
}
