use std::sync::Arc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, transparent_black, Anchor,
    AnchoredPositionMode, AnyElement, App, FocusHandle, MouseButton, SharedString, Window,
};

use crate::core::scanner;
use crate::core::types::{Provider, TimeWindow};
use crate::core::update::{self, Channel, CheckState};
use crate::keymap::{Cancel, SETTINGS_DIALOG_CONTEXT};
use crate::settings::{ScanInterval, Settings, SettingsChange, MODEL_ROW_OPTIONS};
use crate::theme::{Theme, ThemeMode};

use super::components::{Button, IconButton, SectionHeader};
use super::icons::Icon;

// ---------------------------------------------------------------------------
// SettingsDialog — a modal sheet over the dashboard.
//
// The dialog is a pure view of a `Settings` value: it reads the settings it is
// handed and reports every edit as a `SettingsChange`, so it never applies or
// persists anything itself. Adding a setting is a new row here plus a variant
// in `settings::SettingsChange` — no new plumbing between the two.
//
// Edits take effect immediately; there is no OK/Cancel pair to reconcile, and
// "Done" only closes the sheet.
// ---------------------------------------------------------------------------

/// Dialog width. Wide enough for a provider's name beside its source path.
const DIALOG_WIDTH: f32 = 440.0;
/// Height kept clear above and below the sheet, so it never touches the window
/// edges on a short window and its scroll region stays obvious.
const VIEWPORT_MARGIN: f32 = 48.0;
/// Height of a segmented option, matching the dashboard's filter pill.
const SEGMENT_HEIGHT: f32 = 24.0;

type ChangeCallback = Arc<dyn Fn(SettingsChange, &mut Window, &mut App) + Send + Sync>;
type PlainCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>;

#[derive(IntoElement)]
pub struct SettingsDialog {
    settings: Settings,
    /// Focused while the dialog is open, so Escape reaches it.
    focus: FocusHandle,
    /// What the last update check found — owned by the root view, since a
    /// check outlives the dialog that asked for it.
    update_state: CheckState,
    on_change: Option<ChangeCallback>,
    on_close: Option<PlainCallback>,
    on_check_updates: Option<PlainCallback>,
    on_download: Option<PlainCallback>,
}

impl SettingsDialog {
    pub fn new(settings: Settings, focus: FocusHandle) -> Self {
        Self {
            settings,
            focus,
            update_state: CheckState::Idle,
            on_change: None,
            on_close: None,
            on_check_updates: None,
            on_download: None,
        }
    }

    pub fn update_state(mut self, state: CheckState) -> Self {
        self.update_state = state;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(SettingsChange, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_close = Some(Arc::new(handler));
        self
    }

    pub fn on_check_updates(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_check_updates = Some(Arc::new(handler));
        self
    }

    pub fn on_download(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_download = Some(Arc::new(handler));
        self
    }
}

/// Wraps a plain callback in a click handler, mirroring [`report`].
fn invoke(handler: Option<PlainCallback>) -> impl Fn(&mut Window, &mut App) + 'static {
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(window, cx);
        }
    }
}

/// Wraps `change` in a click handler that reports it. Takes the callback by
/// value so the handler it returns owns everything it needs and can outlive the
/// dialog value that built it.
fn report(
    on_change: Option<ChangeCallback>,
    change: SettingsChange,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |_event, window, cx| {
        if let Some(handler) = &on_change {
            handler(change, window, cx);
        }
    }
}

impl RenderOnce for SettingsDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let viewport = window.viewport_size();
        let settings = &self.settings;
        let emit = |change| report(self.on_change.clone(), change);

        // ── Header ─────────────────────────────────────────────────
        let on_close = self.on_close.clone();
        let header = div()
            .flex_none()
            .px(px(18.0))
            .py(px(13.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Settings"),
            )
            .child(
                IconButton::new("settings-close", Icon::Close).on_click(move |window, cx| {
                    if let Some(handler) = &on_close {
                        handler(window, cx);
                    }
                }),
            );

        // ── Appearance ─────────────────────────────────────────────
        let theme_options = ThemeMode::ALL
            .into_iter()
            .map(|mode| {
                Segment::new(
                    mode.label(),
                    mode == settings.theme,
                    emit(SettingsChange::Theme(mode)),
                )
            })
            .collect();

        let appearance = section(
            "Appearance",
            vec![row(
                "Theme",
                "System follows your OS appearance.",
                segmented("settings-theme", theme_options, cx).into_any_element(),
                cx,
            )],
        );

        // ── Data sources ───────────────────────────────────────────
        //
        // Every provider is listed even when its store is missing: seeing the
        // path it would read explains an empty row better than hiding it.
        let provider_rows = Provider::ALL
            .into_iter()
            .map(|provider| {
                let enabled = settings.is_provider_enabled(provider);
                row(
                    provider.label(),
                    source_label(provider),
                    toggle(
                        SharedString::from(format!("settings-provider-{}", provider.label())),
                        enabled,
                        emit(SettingsChange::ToggleProvider(provider)),
                        cx,
                    )
                    .into_any_element(),
                    cx,
                )
            })
            .collect();

        let data_sources = section("Data Sources", provider_rows);

        // ── On launch ──────────────────────────────────────────────
        let range_options = TimeWindow::ALL
            .into_iter()
            .map(|range| {
                Segment::new(
                    range.label(),
                    range == settings.default_range,
                    emit(SettingsChange::DefaultRange(range)),
                )
            })
            .collect();

        let launch = section(
            "On Launch",
            vec![
                stacked_row(
                    "Default range",
                    "The range the dashboard opens on.",
                    segmented("settings-range", range_options, cx).into_any_element(),
                    cx,
                ),
                row(
                    "Scan on launch",
                    "Off waits for the refresh button instead.",
                    toggle(
                        "settings-scan-on-launch",
                        settings.scan_on_launch,
                        emit(SettingsChange::ScanOnLaunch(!settings.scan_on_launch)),
                        cx,
                    )
                    .into_any_element(),
                    cx,
                ),
            ],
        );

        // ── Scanning ───────────────────────────────────────────────
        //
        // Reads on from "Scan on launch" above: between them they say when
        // Drift reads the transcripts on its own. The refresh button is
        // unaffected by either and works even on "Off".
        let interval_options = ScanInterval::ALL
            .into_iter()
            .map(|interval| {
                Segment::new(
                    interval.label(),
                    interval == settings.scan_interval,
                    emit(SettingsChange::ScanInterval(interval)),
                )
            })
            .collect();

        let scanning = section(
            "Scanning",
            vec![stacked_row(
                "Automatic scan",
                "How often to rescan while the app is open.",
                segmented("settings-scan-interval", interval_options, cx).into_any_element(),
                cx,
            )],
        );

        // ── Dashboard ──────────────────────────────────────────────
        let row_options = MODEL_ROW_OPTIONS
            .into_iter()
            .map(|rows| {
                Segment::new(
                    rows.to_string(),
                    rows == settings.model_rows,
                    emit(SettingsChange::ModelRows(rows)),
                )
            })
            .collect();

        let dashboard = section(
            "Dashboard",
            vec![row(
                "Models listed",
                "Rows in the \"By Model\" breakdown.",
                segmented("settings-model-rows", row_options, cx).into_any_element(),
                cx,
            )],
        );

        // ── Updates ────────────────────────────────────────────────
        //
        // The only network feature in the app, so it says what it does: the
        // toggle governs the check on launch, and the button asks on demand
        // even when the toggle is off.
        let channel_options = Channel::ALL
            .into_iter()
            .map(|channel| {
                Segment::new(
                    channel.label(),
                    channel == settings.update_channel,
                    emit(SettingsChange::UpdateChannel(channel)),
                )
            })
            .collect();

        let updates = section(
            "Updates",
            vec![
                row(
                    "Check on launch",
                    "Asks GitHub for the latest release. Nothing else is sent.",
                    toggle(
                        "settings-check-updates",
                        settings.check_for_updates,
                        emit(SettingsChange::CheckForUpdates(!settings.check_for_updates)),
                        cx,
                    )
                    .into_any_element(),
                    cx,
                ),
                row(
                    "Channel",
                    "Beta also offers pre-releases; Stable never does.",
                    segmented("settings-update-channel", channel_options, cx).into_any_element(),
                    cx,
                ),
                update_status_row(
                    &self.update_state,
                    self.on_check_updates.clone(),
                    self.on_download.clone(),
                    cx,
                ),
            ],
        );

        let body = div()
            .id("settings-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(18.0))
            .py(px(14.0))
            .flex()
            .flex_col()
            .gap(px(18.0))
            .child(appearance)
            .child(data_sources)
            .child(launch)
            .child(scanning)
            .child(dashboard)
            .child(updates);

        // ── Footer ─────────────────────────────────────────────────
        let reset = self.on_change.clone();
        let done = self.on_close.clone();
        let footer = div()
            .flex_none()
            .px(px(18.0))
            .py(px(12.0))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                Button::new("settings-reset", "Restore defaults")
                    .subtle()
                    .on_click(move |window, cx| {
                        if let Some(handler) = &reset {
                            handler(SettingsChange::RestoreDefaults, window, cx);
                        }
                    }),
            )
            .child(Button::new("settings-done", "Done").on_click(move |window, cx| {
                if let Some(handler) = &done {
                    handler(window, cx);
                }
            }));

        // ── Sheet ──────────────────────────────────────────────────
        let on_cancel = self.on_close.clone();
        let sheet = div()
            .id("settings-sheet")
            .track_focus(&self.focus)
            // Declaring the context is what scopes `escape` to the dialog:
            // the binding only matches while this is on the focus path, so no
            // key is intercepted once the sheet is gone.
            .key_context(SETTINGS_DIALOG_CONTEXT)
            .occlude()
            .w(px(DIALOG_WIDTH))
            .max_h(viewport.height - px(VIEWPORT_MARGIN * 2.0))
            .flex()
            .flex_col()
            .rounded(px(10.0))
            .bg(theme.canvas)
            .border_1()
            .border_color(theme.border_strong)
            .shadow_lg()
            .on_action(move |_: &Cancel, window, cx| {
                if let Some(handler) = &on_cancel {
                    handler(window, cx);
                }
            })
            .child(header)
            .child(body)
            .child(footer);

        // A dimmed backdrop both signals the modal state and gives the click
        // that dismisses it somewhere to land, matching how the range menu
        // dismisses.
        let on_backdrop = self.on_close.clone();
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position_mode(AnchoredPositionMode::Window)
                .position(point(px(0.0), px(0.0)))
                .child(
                    div()
                        .id("settings-backdrop")
                        .occlude()
                        .w(viewport.width)
                        .h(viewport.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.scrim)
                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                            if let Some(handler) = &on_backdrop {
                                handler(window, cx);
                            }
                        })
                        .child(sheet),
                ),
        )
        .with_priority(3)
    }
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

/// A titled group of rows, using the same header the dashboard sections do.
fn section(title: &'static str, rows: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(SectionHeader::new(title))
        .children(rows)
}

/// Label and description on the left, control pinned right.
fn row(
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
fn stacked_row(
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

/// The status line, with the button that acts on it.
///
/// Which button that is *is* the state: an offer becomes "Download", anything
/// else stays "Check now", and a check in flight disables both so it cannot be
/// started twice.
fn update_status_row(
    state: &CheckState,
    on_check: Option<PlainCallback>,
    on_download: Option<PlainCallback>,
    cx: &App,
) -> AnyElement {
    let theme = Theme::current(cx);
    let summary = SharedString::from(state.summary(&update::current_version()));
    let checking = state.is_checking();

    let action = match state.available() {
        Some(_) => Button::new("settings-update-download", "Download").on_click(invoke(on_download)),
        None => Button::new("settings-update-check", "Check now")
            .subtle()
            .on_click(invoke(on_check)),
    };

    div()
        .py(px(7.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(11.5))
                .text_color(if state.available().is_some() {
                    theme.text
                } else {
                    theme.text_secondary
                })
                .truncate()
                .child(summary),
        )
        .child(
            div()
                .flex_none()
                // A check in flight has nothing to click: the line above
                // already says so, and a second request would only race it.
                .when(checking, |el| el.opacity(0.5))
                .when(!checking, |el| el.child(action)),
        )
        .into_any_element()
}

fn label_block(
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

/// The provider's store, with the home directory shortened to `~`.
fn source_label(provider: Provider) -> SharedString {
    let Some(path) = scanner::data_source(provider) else {
        return SharedString::from("No known location");
    };
    let display = path.to_string_lossy().to_string();
    let shortened = dirs::home_dir()
        .and_then(|home| {
            let home = home.to_string_lossy().to_string();
            display
                .strip_prefix(&home)
                .map(|rest| format!("~{}", rest))
        })
        .unwrap_or(display);
    SharedString::from(shortened)
}

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

type SegmentHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

/// One choice in a [`segmented`] control.
struct Segment {
    label: SharedString,
    selected: bool,
    on_click: SegmentHandler,
}

impl Segment {
    fn new(
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
fn segmented(id: &'static str, options: Vec<Segment>, cx: &App) -> impl IntoElement {
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
fn toggle(
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
