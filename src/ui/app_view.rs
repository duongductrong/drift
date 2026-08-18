use gpui::{div, prelude::*, px, Context, Entity, FocusHandle, SharedString, Window};
use crate::theme::Theme;
use crate::core::scanner;
use crate::core::pricing::PricingTable;
use crate::settings::{self, Settings, SettingsChange};
use super::components::IconButton;
use super::dashboard::{Dashboard, WindowChanged};
use super::icons::Icon;
use super::settings_dialog::SettingsDialog;
use super::title_bar::Toolbar;

pub struct AppView {
    dashboard: Entity<Dashboard>,
    settings_open: bool,
    /// Focused while the settings dialog is open, so Escape closes it.
    settings_focus: FocusHandle,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>) -> Self {
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

        Self {
            dashboard: dash,
            settings_open: false,
            settings_focus: cx.focus_handle(),
        }
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
            let _ = dashboard.update(cx, |d, cx| {
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
            this.settings_open = true;
            // Focus is what makes Escape reach the dialog.
            window.focus(&this.settings_focus, cx);
            cx.notify();
        });

        let toolbar = Toolbar::new()
            .left(
                div()
                    .text_size(px(15.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Mana"),
            )
            .left(
                div()
                    .text_size(px(11.0))
                    .text_color(theme.text_tertiary)
                    .child(range_label),
            )
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
            let close = cx.listener(|this, _event: &(), _window, cx| {
                this.settings_open = false;
                cx.notify();
            });

            SettingsDialog::new(Settings::current(cx), self.settings_focus.clone())
                .on_change(move |c, window, cx| change(&c, window, cx))
                .on_close(move |window, cx| close(&(), window, cx))
        });

        // ── Layout ─────────────────────────────────────────────────
        div()
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
