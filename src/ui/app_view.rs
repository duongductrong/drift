use gpui::{div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use crate::theme::Theme;
use crate::core::scanner;
use crate::core::pricing::PricingTable;
use crate::core::update::{self, CheckState};
use crate::keymap::{
    CheckForUpdates, CloseWindow, Minimize, OpenSettings, Refresh, ToggleFullScreen, Zoom,
};
use crate::settings::{self, Settings, SettingsChange};
use super::components::{Button, IconButton};
use super::dashboard::{Dashboard, WindowChanged};
use super::icons::Icon;
use super::settings_dialog::SettingsDialog;
use super::title_bar::Toolbar;

/// How long after launch the update check runs.
///
/// Late enough that the first frame and the scan have the machine to
/// themselves: nothing about the check is urgent, and it must never be what
/// the user is waiting on.
const UPDATE_CHECK_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

pub struct AppView {
    dashboard: Entity<Dashboard>,
    settings_open: bool,
    /// Result of the last update check — see `core::update`.
    update_state: CheckState,
    /// Holds focus whenever no dialog does. GPUI dispatches an action along
    /// the focus path, so the root has to be on it for the window and view
    /// shortcuts registered in `render` to be reachable at all.
    focus: FocusHandle,
    /// Focused while the settings dialog is open, so Escape closes it.
    settings_focus: FocusHandle,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::current(cx);
        let dashboard = cx.new(|_| Dashboard::new(settings.default_range));

        // Subscribe to window-change events so we trigger a rescan when the
        // user picks a different time window.
        let dash = dashboard.clone();
        cx.subscribe(&dash, |this: &mut Self, _dash, _event: &WindowChanged, cx| {
            this.start_scan(cx);
        }).detach();

        // Auto-scan on startup so the user sees data immediately — unless the
        // user asked to be left alone until they press refresh.
        if settings.scan_on_launch {
            cx.spawn(async move |this, cx| {
                // Tiny delay so the window renders the skeleton first.
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.start_scan(cx);
                });
            })
            .detach();
        }

        let focus = cx.focus_handle();
        window.focus(&focus, cx);

        // The check is deliberately not tied to opening the settings dialog:
        // someone who never opens it should still learn that a new build
        // exists, via the toolbar badge.
        if settings.check_for_updates {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(UPDATE_CHECK_DELAY).await;
                let _ = this.update(cx, |this, cx| this.start_update_check(cx));
            })
            .detach();
        }

        Self {
            dashboard: dash,
            settings_open: false,
            update_state: CheckState::Idle,
            focus,
            settings_focus: cx.focus_handle(),
        }
    }

    /// Ask GitHub whether a newer build exists, on the channel the user is
    /// subscribed to.
    ///
    /// Every failure mode ends as a `CheckState::Failed` the dialog can show;
    /// nothing here can take the app down or block a frame, since the request
    /// itself runs on the background executor.
    fn start_update_check(&mut self, cx: &mut Context<Self>) {
        // A second click while one is in flight would only race the first.
        if self.update_state.is_checking() {
            return;
        }

        let channel = Settings::current(cx).update_channel;
        self.update_state = CheckState::Checking;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move { update::check(channel) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.update_state = match outcome {
                    Ok(status) => CheckState::Done(status),
                    Err(error) => CheckState::Failed(error),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Open the release page for the update on offer. Downloading and
    /// installing stay the user's business — see `core::update`.
    fn open_release_page(&mut self, cx: &mut Context<Self>) {
        if let Some(release) = self.update_state.available() {
            cx.open_url(&release.url);
        }
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = true;
        // Focus is what makes Escape reach the dialog.
        window.focus(&self.settings_focus, cx);
        cx.notify();
    }

    fn close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings_open {
            return;
        }
        self.settings_open = false;
        // The dialog's focus handle leaves the element tree with it, which
        // would strand focus off the dispatch path and take every shortcut
        // down with it.
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn start_scan(&mut self, cx: &mut Context<Self>) {
        let dashboard = self.dashboard.clone();
        dashboard.update(cx, |d, cx| {
            d.loading = true;
            cx.notify();
        });
        let window = dashboard.read(cx).selected_window;
        // Which providers are counted is settings, not scanning: resolve it here
        // and hand the scanner a plain list.
        let providers = Settings::current(cx).enabled_providers();
        cx.spawn(async move |_this, cx| {
            let snapshot = cx
                .background_executor()
                .spawn(async move {
                    let pricing = PricingTable::builtin();
                    scanner::scan_all(window, &pricing, &providers)
                })
                .await;
            dashboard.update(cx, |d, cx| {
                d.snapshot = Some(snapshot);
                d.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn rescan(&mut self, cx: &mut Context<Self>) {
        self.start_scan(cx);
    }

    /// Apply one edit from the settings dialog, rescanning when the edit
    /// changes which events are counted.
    fn apply_setting(&mut self, change: SettingsChange, cx: &mut Context<Self>) {
        if settings::update(cx, change) {
            self.start_scan(cx);
        }
        // Switching channels asks a different question, so the answer on
        // screen is stale the moment it changes.
        if let SettingsChange::UpdateChannel(_) | SettingsChange::RestoreDefaults = change {
            self.update_state = CheckState::Idle;
            self.start_update_check(cx);
        }
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);

        // Read the active window's date range for the subtitle.
        let range_label = {
            let snap = self.dashboard.read(cx);
            if let Some(s) = &snap.snapshot {
                SharedString::from(format!(
                    "{} – {}",
                    s.start_date.format("%b %d"),
                    s.end_date.format("%b %d, %Y")
                ))
            } else {
                SharedString::from(snap.selected_window.label())
            }
        };
        let is_loading = self.dashboard.read(cx).loading;

        // ── Toolbar ────────────────────────────────────────────────
        let rescan = cx.listener(|this, _event: &(), _window: &mut Window, cx| this.rescan(cx));
        let open_settings = cx.listener(|this, _event: &(), window: &mut Window, cx| {
            this.open_settings(window, cx)
        });

        // Shown only when there is something to act on, so the chrome stays
        // empty in the normal case.
        let update_badge = self.update_state.available().map(|release| {
            let open = cx.listener(|this, _event: &(), _window: &mut Window, cx| {
                this.open_release_page(cx)
            });
            let label = SharedString::from(format!("Update to {}", release.version));
            Button::new("update-badge", label)
                .subtle()
                .on_click(move |window, cx| open(&(), window, cx))
        });

        let toolbar = Toolbar::new()
            .left(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Drift"),
            )
            .left(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.text_tertiary)
                    .child(range_label),
            )
            .children_right(update_badge)
            .right(
                IconButton::new("scan-button", Icon::Refresh)
                    .tooltip(if is_loading {
                        "Scanning…"
                    } else {
                        "Scan transcripts"
                    })
                    .busy(is_loading)
                    .on_click(move |window, cx| rescan(&(), window, cx)),
            )
            .right(
                IconButton::new("settings-button", Icon::Settings)
                    .tooltip("Settings")
                    .selected(self.settings_open)
                    .on_click(move |window, cx| open_settings(&(), window, cx)),
            );

        // ── Settings dialog ────────────────────────────────────────
        let settings_dialog = self.settings_open.then(|| {
            let change = cx.listener(|this, change: &SettingsChange, _window, cx| {
                this.apply_setting(*change, cx);
            });
            let close = cx.listener(|this, _event: &(), window: &mut Window, cx| {
                this.close_settings(window, cx)
            });

            let check = cx.listener(|this, _event: &(), _window: &mut Window, cx| {
                this.start_update_check(cx)
            });
            let download = cx.listener(|this, _event: &(), _window: &mut Window, cx| {
                this.open_release_page(cx)
            });

            SettingsDialog::new(Settings::current(cx), self.settings_focus.clone())
                .update_state(self.update_state.clone())
                .on_change(move |c, window, cx| change(&c, window, cx))
                .on_check_updates(move |window, cx| check(&(), window, cx))
                .on_download(move |window, cx| download(&(), window, cx))
                .on_close(move |window, cx| close(&(), window, cx))
        });

        // ── Layout ─────────────────────────────────────────────────
        div()
            // Keeps the root on the focus path — see `focus` above — and lets
            // a click on empty chrome hand focus back after a dialog closes.
            .track_focus(&self.focus)
            // Window commands. Handled here rather than globally so they act
            // on the window the keystroke was aimed at.
            .on_action(|_: &Minimize, window: &mut Window, _| window.minimize_window())
            .on_action(|_: &Zoom, window: &mut Window, _| window.zoom_window())
            .on_action(|_: &ToggleFullScreen, window: &mut Window, _| window.toggle_fullscreen())
            .on_action(|_: &CloseWindow, window: &mut Window, _| window.remove_window())
            // View commands.
            .on_action(cx.listener(|this, _: &Refresh, _window, cx| this.rescan(cx)))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings(window, cx)
            }))
            // The menu item opens the dialog as well as checking: the status
            // line there is where the answer appears, and an update that is
            // asked for should never answer silently.
            .on_action(cx.listener(|this, _: &CheckForUpdates, window, cx| {
                this.open_settings(window, cx);
                this.start_update_check(cx);
            }))
            .size_full()
            .bg(theme.canvas)
            .text_color(theme.text)
            .flex()
            .flex_col()
            .child(toolbar)
            // Main content — dashboard manages its own scrolling
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.dashboard.clone()),
            )
            // Footer
            .child(
                div()
                    .h(px(28.0))
                    .px(px(20.0))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme.text_ghost)
                            .child("Reads Claude · Codex · Kimi · OpenCode · Antigravity"),
                    ),
            )
            .children(settings_dialog)
    }
}
