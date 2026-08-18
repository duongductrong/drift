use std::sync::Arc;
use gpui::{div, prelude::*, px, transparent_black, SharedString, Window, App};
use crate::theme::Theme;
use crate::core::types::TimeWindow;

const ALL_WINDOWS: [TimeWindow; 5] = [
    TimeWindow::Last7Days,
    TimeWindow::Last30Days,
    TimeWindow::Last90Days,
    TimeWindow::CurrentMonth,
    TimeWindow::PreviousMonth,
];

/// A segmented pill-style toggle for time window selection.
#[derive(IntoElement)]
pub struct TimeWindowPicker {
    selected: TimeWindow,
    on_select: Arc<dyn Fn(TimeWindow, &mut Window, &mut App) + Send + Sync + 'static>,
}

impl TimeWindowPicker {
    pub fn new(
        selected: TimeWindow,
        on_select: impl Fn(TimeWindow, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        Self {
            selected,
            on_select: Arc::new(on_select),
        }
    }
}

impl RenderOnce for TimeWindowPicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

        let mut container = div()
            .flex()
            .overflow_hidden()
            .rounded(px(7.0))
            .border_1()
            .border_color(theme.border_strong);

        for tw in ALL_WINDOWS {
            let is_selected = tw == self.selected;
            let on_select = self.on_select.clone();
            let id = SharedString::from(format!("tw-{}", tw.label()));
            let bg = if is_selected {
                theme.overlay
            } else {
                transparent_black()
            };
            let text_color = if is_selected {
                theme.text
            } else {
                theme.text_secondary
            };

            container = container.child(
                div()
                    .id(id)
                    .h(px(26.0))
                    .px(px(11.0))
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(px(10.5))
                    .bg(bg)
                    .text_color(text_color)
                    .when(!is_selected, |el| el.hover(|s| s.text_color(theme.text)))
                    .child(tw.label())
                    .on_click(move |_event, window, cx| {
                        on_select(tw, window, cx);
                    }),
            );
        }

        container
    }
}
