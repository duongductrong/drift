use std::sync::Arc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, transparent_black, Anchor,
    AnchoredPositionMode, App, MouseButton, SharedString, Window,
};

use crate::core::types::{Granularity, TimeWindow};
use crate::theme::Theme;

/// Shared height for both halves of the control, so the joined pill reads as
/// one object rather than two adjacent ones.
const CONTROL_HEIGHT: f32 = 26.0;
/// Corner radius, matching the pill radius used elsewhere in the toolbar.
const CONTROL_RADIUS: f32 = 7.0;
/// Menu width — wide enough for "This month" plus its checkmark gutter.
const MENU_WIDTH: f32 = 148.0;
/// Hairline gap between the pill and its menu. Small enough that the menu
/// reads as hanging off the trigger, wide enough that its shadow still
/// separates the two edges.
const MENU_GAP: f32 = 4.0;

type WindowCallback = Arc<dyn Fn(TimeWindow, &mut Window, &mut App) + Send + Sync>;
type GranularityCallback = Arc<dyn Fn(Granularity, &mut Window, &mut App) + Send + Sync>;
type PlainCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>;

/// The dashboard's range + aggregation control.
///
/// The two live in one bordered pill on purpose: the range picks *what* is
/// counted, the Daily/Monthly switch picks *how it is bucketed*, and joining
/// them makes that read as a single "this range, bucketed this way" statement
/// instead of two unrelated filters.
#[derive(IntoElement)]
pub struct UsageFilters {
    window: TimeWindow,
    granularity: Granularity,
    monthly_available: bool,
    menu_open: bool,
    on_select_window: Option<WindowCallback>,
    on_select_granularity: Option<GranularityCallback>,
    on_toggle_menu: Option<PlainCallback>,
    on_dismiss_menu: Option<PlainCallback>,
}

impl UsageFilters {
    pub fn new(window: TimeWindow, granularity: Granularity) -> Self {
        Self {
            window,
            granularity,
            monthly_available: true,
            menu_open: false,
            on_select_window: None,
            on_select_granularity: None,
            on_toggle_menu: None,
            on_dismiss_menu: None,
        }
    }

    /// Whether Monthly is offered. False collapses the range to a single bar,
    /// so the segment is shown inert rather than hidden — keeping the control's
    /// width stable and the option discoverable.
    pub fn monthly_available(mut self, available: bool) -> Self {
        self.monthly_available = available;
        self
    }

    pub fn menu_open(mut self, open: bool) -> Self {
        self.menu_open = open;
        self
    }

    pub fn on_select_window(
        mut self,
        handler: impl Fn(TimeWindow, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_select_window = Some(Arc::new(handler));
        self
    }

    pub fn on_select_granularity(
        mut self,
        handler: impl Fn(Granularity, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_select_granularity = Some(Arc::new(handler));
        self
    }

    pub fn on_toggle_menu(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_toggle_menu = Some(Arc::new(handler));
        self
    }

    pub fn on_dismiss_menu(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_dismiss_menu = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for UsageFilters {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

        // ── Range trigger ──────────────────────────────────────────
        let on_toggle = self.on_toggle_menu.clone();
        let trigger = div()
            .id("range-trigger")
            .h(px(CONTROL_HEIGHT))
            .pl(px(11.0))
            .pr(px(9.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .cursor_default()
            .text_size(px(10.5))
            .text_color(theme.text)
            .bg(if self.menu_open {
                theme.overlay
            } else {
                transparent_black()
            })
            .hover(|style| style.bg(theme.overlay))
            .child(self.window.label())
            .child(
                div()
                    .text_size(px(8.0))
                    .text_color(theme.text_tertiary)
                    .child("▼"),
            )
            .on_click(move |_event, window, cx| {
                if let Some(handler) = &on_toggle {
                    handler(window, cx);
                }
            });

        // ── Range menu ─────────────────────────────────────────────
        let menu = self.menu_open.then(|| {
            let mut list = div()
                .id("range-menu")
                .occlude()
                .w(px(MENU_WIDTH))
                .p(px(4.0))
                .rounded(px(8.0))
                .bg(theme.canvas)
                .border_1()
                .border_color(theme.border_strong)
                .shadow_md()
                .flex()
                .flex_col();

            for option in TimeWindow::ALL {
                let is_selected = option == self.window;
                let on_select = self.on_select_window.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("range-{}", option.label())))
                        .h(px(24.0))
                        .px(px(7.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .rounded(px(5.0))
                        .cursor_default()
                        .text_size(px(10.5))
                        .text_color(if is_selected {
                            theme.text
                        } else {
                            theme.text_secondary
                        })
                        .hover(|style| style.bg(theme.overlay).text_color(theme.text))
                        .child(
                            div()
                                .w(px(9.0))
                                .flex_none()
                                .text_size(px(9.0))
                                .text_color(theme.text)
                                .child(if is_selected { "✓" } else { "" }),
                        )
                        .child(option.label())
                        .on_click(move |_event, window, cx| {
                            if let Some(handler) = &on_select {
                                handler(option, window, cx);
                            }
                        }),
                );
            }

            // The menu hangs off a zero-height rail pinned to the pill's
            // bottom edge. `top_full` resolves against the pill's own measured
            // height, so the seam stays right whatever the pill's height,
            // border or padding become — and the rail, being out of flow with
            // no in-flow sibling of its own, gives `Local` mode a definite
            // origin instead of the static position it would otherwise inherit
            // from whatever sits above it. All the anchor then has to say is
            // how far below that seam the menu sits.
            deferred(
                div().absolute().top_full().left_0().child(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position_mode(AnchoredPositionMode::Local)
                        .position(point(px(0.0), px(MENU_GAP)))
                        // Keeps the menu on screen when the pill sits near an
                        // edge, rather than flipping it away from its trigger.
                        .snap_to_window_with_margin(px(8.0))
                        .child(list),
                ),
            )
            .with_priority(2)
        });

        // Dismissal runs off a backdrop rather than the menu's own
        // mouse-down-out: the backdrop swallows the press that would otherwise
        // reach the trigger, so clicking the trigger while the menu is open
        // closes it instead of closing and immediately reopening it.
        let backdrop = self.menu_open.then(|| {
            let on_dismiss = self.on_dismiss_menu.clone();
            let viewport = window.viewport_size();

            deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(px(0.0), px(0.0)))
                    .child(
                        div()
                            .id("range-menu-backdrop")
                            .occlude()
                            .w(viewport.width)
                            .h(viewport.height)
                            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                if let Some(handler) = &on_dismiss {
                                    handler(window, cx);
                                }
                            }),
                    ),
            )
            .with_priority(1)
        });

        // ── Daily / Monthly switch ─────────────────────────────────
        let mut switch = div().flex().items_center();
        for option in Granularity::ALL {
            let is_selected = option == self.granularity;
            let is_enabled = self.monthly_available || option == Granularity::Daily;
            let on_select = self.on_select_granularity.clone();

            switch = switch.child(
                div()
                    .id(SharedString::from(format!("granularity-{}", option.label())))
                    .h(px(CONTROL_HEIGHT))
                    .px(px(10.0))
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(px(10.5))
                    .bg(if is_selected {
                        theme.overlay_strong
                    } else {
                        transparent_black()
                    })
                    .text_color(match (is_enabled, is_selected) {
                        (false, _) => theme.text_ghost,
                        (true, true) => theme.text,
                        (true, false) => theme.text_secondary,
                    })
                    .when(is_enabled && !is_selected, |el| {
                        el.hover(|style| style.text_color(theme.text))
                    })
                    .child(option.label())
                    .when(is_enabled, |el| {
                        el.on_click(move |_event, window, cx| {
                            if let Some(handler) = &on_select {
                                handler(option, window, cx);
                            }
                        })
                    }),
            );
        }

        let pill = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(theme.border_strong)
            .child(trigger)
            // Hairline seam: the two controls are distinct choices, but parts
            // of the same statement.
            .child(
                div()
                    .w(px(1.0))
                    .h(px(CONTROL_HEIGHT))
                    .flex_none()
                    .bg(theme.border_strong),
            )
            .child(switch);

        // The menu is a sibling of the pill, not a descendant: the pill clips
        // its own children to get the rounded seam, which would swallow a
        // dropdown nested inside it.
        div()
            .relative()
            .flex_none()
            .child(pill)
            .children(backdrop)
            .children(menu)
    }
}
