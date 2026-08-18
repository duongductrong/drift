use gpui::{div, prelude::*, px, Context, Entity, SharedString, Window};
use crate::theme::Theme;
use crate::core::scanner;
use crate::core::pricing::PricingTable;
use super::dashboard::{Dashboard, WindowChanged};
use super::title_bar::Toolbar;

pub struct AppView {
    dashboard: Entity<Dashboard>,
}

impl AppView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let dashboard = cx.new(|_| Dashboard::new());

        // Subscribe to window-change events so we trigger a rescan when the
        // user picks a different time window.
        let dash = dashboard.clone();
        cx.subscribe(&dash, |this: &mut Self, _dash, _event: &WindowChanged, cx| {
            this.start_scan(cx);
        }).detach();

        // Auto-scan on startup so the user sees data immediately.
        let dash = dashboard.clone();
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

        Self { dashboard: dash }
    }

    fn start_scan(&mut self, cx: &mut Context<Self>) {
        let dashboard = self.dashboard.clone();
        dashboard.update(cx, |d, cx| {
            d.loading = true;
            cx.notify();
        });
        let window = dashboard.read(cx).selected_window;
        cx.spawn(async move |_this, cx| {
            let snapshot = cx
                .background_executor()
                .spawn(async move {
                    let pricing = PricingTable::builtin();
                    scanner::scan_all(window, &pricing)
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
                div()
                    .id("scan-button")
                    .px(px(14.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .bg(theme.inverse)
                    .text_size(px(12.0))
                    .text_color(theme.on_inverse)
                    .cursor_pointer()
                    .hover(move |style| style.opacity(0.85))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.rescan(cx);
                    }))
                    .child(if is_loading {
                        "Scanning…"
                    } else {
                        "Scan Transcripts"
                    }),
            );

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
                            .child("Reads ~/.claude/projects & ~/.codex/sessions"),
                    ),
            )
    }
}
