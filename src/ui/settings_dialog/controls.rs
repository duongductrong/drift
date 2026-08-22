use gpui::{div, prelude::*, px, transparent_black, AnyElement, App, SharedString, Window};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Controls — the row shapes and widgets every pane is built from.
//
// Kept here rather than in `ui::components` because they are the settings
// dialog's own vocabulary: a label with its explanation on the left and one
// control on the right. A pane composes these and nothing else, so the panes
// stay a list of settings rather than a pile of layout.
// ---------------------------------------------------------------------------

/// Height of a segmented option, matching the dashboard's filter pill.
const SEGMENT_HEIGHT: f32 = 24.0;

/// Label and description on the left, control pinned right.
pub(super) fn row(
    label: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: AnyElement,
    cx: &App,
) -> AnyElement {
    div()
        .py(px(7.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(label_block(label, detail, cx))
        .child(control)
        .into_any_element()
}

/// As [`row`], with the control on its own line below — for a control too wide
/// to sit beside its label.
pub(super) fn stacked_row(
    label: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: AnyElement,
    cx: &App,
) -> AnyElement {
    div()
        .py(px(7.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(label_block(label, detail, cx))
        .child(control)
        .into_any_element()
}

pub(super) fn label_block(
    label: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    let theme = Theme::current(cx);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(
            div()
                .text_size(px(11.5))
                .text_color(theme.text)
                .child(label.into()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(theme.text_tertiary)
                .truncate()
                .child(detail.into()),
        )
}

type SegmentHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

/// One choice in a [`segmented`] control.
pub(super) struct Segment {
    label: SharedString,
    selected: bool,
    on_click: SegmentHandler,
}

impl Segment {
    pub(super) fn new(
        label: impl Into<SharedString>,
        selected: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            selected,
            on_click: Box::new(on_click),
        }
    }
}

/// A joined row of mutually exclusive options, drawn as the same bordered pill
/// the dashboard's Daily/Monthly switch uses.
pub(super) fn segmented(id: &'static str, options: Vec<Segment>, cx: &App) -> impl IntoElement {
    let theme = Theme::current(cx);
    let last = options.len().saturating_sub(1);

    let mut pill = div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .overflow_hidden()
        .rounded(px(7.0))
        .border_1()
        .border_color(theme.border_strong);

    for (i, option) in options.into_iter().enumerate() {
        pill = pill.child(
            div()
                .id(SharedString::from(format!("{}-{}", id, i)))
                .flex_1()
                .h(px(SEGMENT_HEIGHT))
                .px(px(9.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_default()
                .text_size(px(10.5))
                .bg(if option.selected {
                    theme.overlay_strong
                } else {
                    transparent_black()
                })
                .text_color(if option.selected {
                    theme.text
                } else {
                    theme.text_secondary
                })
                .when(!option.selected, |el| {
                    el.hover(|style| style.text_color(theme.text))
                })
                .child(option.label)
                .on_click(option.on_click),
        );

        if i < last {
            pill = pill.child(
                div()
                    .w(px(1.0))
                    .h(px(SEGMENT_HEIGHT))
                    .flex_none()
                    .bg(theme.border_strong),
            );
        }
    }

    pill
}

/// A two-state switch. Filled with the inverse surface when on, so "on" reads
/// as the same emphasis a primary button carries.
pub(super) fn toggle(
    id: impl Into<SharedString>,
    on: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let theme = Theme::current(cx);
    div()
        .id(id.into())
        .flex_none()
        .w(px(30.0))
        .h(px(17.0))
        .p(px(2.0))
        .rounded(px(9.0))
        .flex()
        .items_center()
        .cursor_default()
        .bg(if on { theme.inverse } else { theme.overlay_strong })
        .when(on, |el| el.justify_end())
        .hover(|style| style.opacity(0.85))
        .child(
            div()
                .size(px(13.0))
                .rounded(px(7.0))
                .bg(if on {
                    theme.on_inverse
                } else {
                    theme.text_tertiary
                }),
        )
        .on_click(on_click)
}
