use std::sync::Arc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, transparent_black, Anchor,
    AnchoredPositionMode, App, MouseButton, SharedString, Window,
};

use crate::core::types::{Granularity, TimeWindow};
use crate::theme::Theme;
use super::tooltip::Tooltip;

/// Shared height for both halves of the control, so the joined pill reads as
/// one object rather than two adjacent ones.
const CONTROL_HEIGHT: f32 = 26.0;
/// Corner radius, matching the pill radius used elsewhere in the toolbar.
const CONTROL_RADIUS: f32 = 7.0;
/// Menu width — wide enough for "This month" plus its checkmark gutter.
const MENU_WIDTH: f32 = 148.0;
/// The project menu carries a value beside every name, so it is wider than the
/// range menu it sits next to.
const PROJECT_MENU_WIDTH: f32 = 258.0;
/// How much of the project menu is shown before it starts scrolling. Roughly
/// ten rows: enough that most machines never scroll, short enough that a
/// hundred projects cannot run the menu off the bottom of the window.
const PROJECT_MENU_MAX_HEIGHT: f32 = 264.0;
/// Widest the selected project's name gets in the trigger before it is
/// truncated, so a deeply-named project cannot push the range pill off screen.
const PROJECT_TRIGGER_MAX_WIDTH: f32 = 128.0;
/// Hairline gap between the pill and its menu. Small enough that the menu
/// reads as hanging off the trigger, wide enough that its shadow still
/// separates the two edges.
const MENU_GAP: f32 = 4.0;
/// Gap between the project pill and the range pill. They are separate objects
/// — one picks whose usage is counted, the other how it is sliced — so they sit
/// apart rather than sharing a seam.
const PILL_GAP: f32 = 8.0;

type WindowCallback = Arc<dyn Fn(TimeWindow, &mut Window, &mut App) + Send + Sync>;
type GranularityCallback = Arc<dyn Fn(Granularity, &mut Window, &mut App) + Send + Sync>;
type ProjectCallback = Arc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + Send + Sync>;
type MenuCallback = Arc<dyn Fn(FilterMenu, &mut Window, &mut App) + Send + Sync>;
type PlainCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>;

/// Which of the control's dropdowns is open. Only ever one: both hang off the
/// same backdrop, and two menus open at once would leave the user unsure which
/// one their next click lands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMenu {
    Project,
    Range,
}

/// One row of the project menu: a project, and what it spent in whichever
/// metric the page is currently read in.
#[derive(Clone, Debug)]
pub struct ProjectOption {
    /// Absolute path — the identity the selection is stored under.
    pub path: SharedString,
    /// Short name shown in the menu; the path rides along as a tooltip.
    pub label: SharedString,
    /// Pre-formatted value, so the control never has to know the metric.
    pub value: SharedString,
}

/// The dashboard's project + range + aggregation control.
///
/// Read left to right it is one sentence: *whose* usage is counted, over
/// *what* range, bucketed *how*. The last two share a pill because they are
/// two halves of one statement about the chart; the project sits in its own
/// pill because it narrows the whole page, not just the series.
#[derive(IntoElement)]
pub struct UsageFilters {
    window: TimeWindow,
    granularity: Granularity,
    monthly_available: bool,
    projects: Vec<ProjectOption>,
    selected_project: Option<SharedString>,
    all_projects_value: Option<SharedString>,
    open_menu: Option<FilterMenu>,
    on_select_window: Option<WindowCallback>,
    on_select_granularity: Option<GranularityCallback>,
    on_select_project: Option<ProjectCallback>,
    on_toggle_menu: Option<MenuCallback>,
    on_dismiss_menu: Option<PlainCallback>,
}

impl UsageFilters {
    pub fn new(window: TimeWindow, granularity: Granularity) -> Self {
        Self {
            window,
            granularity,
            monthly_available: true,
            projects: Vec::new(),
            selected_project: None,
            all_projects_value: None,
            open_menu: None,
            on_select_window: None,
            on_select_granularity: None,
            on_select_project: None,
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

    /// The projects the range touched, in the order they should be listed.
    /// Empty hides the project pill: a range with no usage in it has nothing
    /// to filter, and an empty menu is worse than no menu.
    pub fn projects(mut self, projects: Vec<ProjectOption>) -> Self {
        self.projects = projects;
        self
    }

    /// The project currently filtered to, by path. `None` is "All projects".
    pub fn selected_project(mut self, path: Option<SharedString>) -> Self {
        self.selected_project = path;
        self
    }

    /// The range total, shown against "All projects" so that row is quoted in
    /// the same terms as the ones under it.
    pub fn all_projects_value(mut self, value: impl Into<SharedString>) -> Self {
        self.all_projects_value = Some(value.into());
        self
    }

    pub fn open_menu(mut self, menu: Option<FilterMenu>) -> Self {
        self.open_menu = menu;
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

    pub fn on_select_project(
        mut self,
        handler: impl Fn(Option<SharedString>, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_select_project = Some(Arc::new(handler));
        self
    }

    pub fn on_toggle_menu(
        mut self,
        handler: impl Fn(FilterMenu, &mut Window, &mut App) + Send + Sync + 'static,
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

    /// Name shown on the project trigger: the selected project's, or the
    /// all-projects wording when nothing is filtered.
    fn project_trigger_label(&self) -> SharedString {
        self.selected_project
            .as_ref()
            .and_then(|path| self.projects.iter().find(|p| &p.path == path))
            .map(|p| p.label.clone())
            .unwrap_or_else(|| SharedString::from("All projects"))
    }
}

/// A dropdown trigger: its label, a caret, and a background while its menu is
/// open so the pill shows which half was pressed.
fn trigger(
    id: &'static str,
    label: SharedString,
    is_open: bool,
    max_label_width: Option<f32>,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(CONTROL_HEIGHT))
        .pl(px(11.0))
        .pr(px(9.0))
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_default()
        .text_size(px(10.5))
        .text_color(theme.text)
        .bg(if is_open {
            theme.overlay
        } else {
            transparent_black()
        })
        .hover(|style| style.bg(theme.overlay))
        .child(
            div()
                .when_some(max_label_width, |el, width| {
                    el.max_w(px(width)).overflow_hidden().truncate()
                })
                .child(label),
        )
        .child(
            div()
                .text_size(px(8.0))
                .text_color(theme.text_tertiary)
                .child("▼"),
        )
}

/// The shared menu surface: a bordered card that hangs off its trigger.
fn menu_surface(id: &'static str, width: f32, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .occlude()
        .w(px(width))
        .p(px(4.0))
        .rounded(px(8.0))
        .bg(theme.canvas)
        .border_1()
        .border_color(theme.border_strong)
        .shadow_md()
        .flex()
        .flex_col()
}

/// One selectable row, with the checkmark gutter that marks the active choice.
fn menu_row(id: SharedString, is_selected: bool, theme: &Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(24.0))
        .px(px(7.0))
        .flex()
        .flex_none()
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
}

/// Hang `menu` off the bottom-left corner of whatever it is a child of.
///
/// The menu hangs off a zero-height rail pinned to the trigger's bottom edge.
/// `top_full` resolves against the trigger's own measured height, so the seam
/// stays right whatever the trigger's height, border or padding become — and
/// the rail, being out of flow with no in-flow sibling of its own, gives
/// `Local` mode a definite origin instead of the static position it would
/// otherwise inherit from whatever sits above it. All the anchor then has to
/// say is how far below that seam the menu sits.
fn hang_menu(menu: impl IntoElement) -> impl IntoElement {
    deferred(
        div().absolute().top_full().left_0().child(
            anchored()
                .anchor(Anchor::TopLeft)
                .position_mode(AnchoredPositionMode::Local)
                .position(point(px(0.0), px(MENU_GAP)))
                // Keeps the menu on screen when the trigger sits near an
                // edge, rather than flipping it away from its trigger.
                .snap_to_window_with_margin(px(8.0))
                .child(menu),
        ),
    )
    .with_priority(2)
}

impl RenderOnce for UsageFilters {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

        // ── Project trigger + menu ─────────────────────────────────
        let project_pill = (!self.projects.is_empty()).then(|| {
            let is_open = self.open_menu == Some(FilterMenu::Project);
            let on_toggle = self.on_toggle_menu.clone();

            let pill = trigger(
                "project-trigger",
                self.project_trigger_label(),
                is_open,
                Some(PROJECT_TRIGGER_MAX_WIDTH),
                &theme,
            )
            .on_click(move |_event, window, cx| {
                if let Some(handler) = &on_toggle {
                    handler(FilterMenu::Project, window, cx);
                }
            });

            let menu = is_open.then(|| {
                let mut list = menu_surface("project-menu", PROJECT_MENU_WIDTH, &theme)
                    .max_h(px(PROJECT_MENU_MAX_HEIGHT))
                    // A machine with dozens of projects would otherwise run
                    // the menu past the bottom of the window.
                    .overflow_y_scroll();

                // "All projects" leads: it is the state the page opens in, and
                // the row a filtered view comes back through.
                let on_select_all = self.on_select_project.clone();
                list = list.child(
                    menu_row(
                        SharedString::from("project-all"),
                        self.selected_project.is_none(),
                        &theme,
                    )
                    .child(div().flex_1().min_w_0().truncate().child("All projects"))
                    .children(self.all_projects_value.clone().map(|value| {
                        div()
                            .flex_none()
                            .text_size(px(10.0))
                            .text_color(theme.text_tertiary)
                            .child(value)
                    }))
                    .on_click(move |_event, window, cx| {
                        if let Some(handler) = &on_select_all {
                            handler(None, window, cx);
                        }
                    }),
                );

                for option in &self.projects {
                    let is_selected = self.selected_project.as_ref() == Some(&option.path);
                    let on_select = self.on_select_project.clone();
                    let path = option.path.clone();
                    list = list.child(
                        menu_row(
                            SharedString::from(format!("project-{}", option.path)),
                            is_selected,
                            &theme,
                        )
                        // Two projects can share a leaf name, so the full path
                        // is always one hover away.
                        .when(!option.path.is_empty(), |el| {
                            el.tooltip(Tooltip::text(option.path.clone()))
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(option.label.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(10.0))
                                .text_color(theme.text_tertiary)
                                .child(option.value.clone()),
                        )
                        .on_click(move |_event, window, cx| {
                            if let Some(handler) = &on_select {
                                handler(Some(path.clone()), window, cx);
                            }
                        }),
                    );
                }

                hang_menu(list)
            });

            // The menu is a sibling of the pill, not a descendant: the pill
            // clips its own children to get the rounded edge, which would
            // swallow a dropdown nested inside it.
            div()
                .relative()
                .flex_none()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .rounded(px(CONTROL_RADIUS))
                        .border_1()
                        .border_color(theme.border_strong)
                        .child(pill),
                )
                .children(menu)
        });

        // ── Range trigger ──────────────────────────────────────────
        let range_open = self.open_menu == Some(FilterMenu::Range);
        let on_toggle = self.on_toggle_menu.clone();
        let range_trigger = trigger(
            "range-trigger",
            SharedString::from(self.window.label()),
            range_open,
            None,
            &theme,
        )
        .on_click(move |_event, window, cx| {
            if let Some(handler) = &on_toggle {
                handler(FilterMenu::Range, window, cx);
            }
        });

        // ── Range menu ─────────────────────────────────────────────
        let range_menu = range_open.then(|| {
            let mut list = menu_surface("range-menu", MENU_WIDTH, &theme);

            for option in TimeWindow::ALL {
                let is_selected = option == self.window;
                let on_select = self.on_select_window.clone();
                list = list.child(
                    menu_row(
                        SharedString::from(format!("range-{}", option.label())),
                        is_selected,
                        &theme,
                    )
                    .child(option.label())
                    .on_click(move |_event, window, cx| {
                        if let Some(handler) = &on_select {
                            handler(option, window, cx);
                        }
                    }),
                );
            }

            hang_menu(list)
        });

        // Dismissal runs off a backdrop rather than the menu's own
        // mouse-down-out: the backdrop swallows the press that would otherwise
        // reach the trigger, so clicking the trigger while the menu is open
        // closes it instead of closing and immediately reopening it.
        let backdrop = self.open_menu.is_some().then(|| {
            let on_dismiss = self.on_dismiss_menu.clone();
            let viewport = window.viewport_size();

            deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position_mode(AnchoredPositionMode::Window)
                    .position(point(px(0.0), px(0.0)))
                    .child(
                        div()
                            .id("filter-menu-backdrop")
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

        let range_pill = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(CONTROL_RADIUS))
            .border_1()
            .border_color(theme.border_strong)
            .child(range_trigger)
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

        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(PILL_GAP))
            .children(project_pill)
            .child(
                div()
                    .relative()
                    .flex_none()
                    .child(range_pill)
                    .children(range_menu),
            )
            // One backdrop for both menus: whichever is open, a press anywhere
            // outside it closes it.
            .children(backdrop)
    }
}
