use gpui::{div, prelude::*, px, Context, SharedString, Window};
use crate::core::types::{UsageSnapshot, TimeWindow};
use crate::theme::Theme;
use super::components::*;
use super::daily_chart::DailyChart;
use super::empty_state::EmptyState;
use super::metric_tile::render_metric_strip;
use super::model_row::ModelRow;
use super::provider_row::ProviderRow;
use super::scroll_area::ScrollArea;
use super::skeleton::render_dashboard_skeleton;
use super::time_window_picker::TimeWindowPicker;

/// Emitted when the user picks a different time window so the parent
/// (`AppView`) can trigger a rescan. Carries no payload: the new window is
/// already stored on `Dashboard::selected_window` before this is emitted.
#[derive(Clone, Debug)]
pub struct WindowChanged;

impl gpui::EventEmitter<WindowChanged> for Dashboard {}

pub struct Dashboard {
    pub snapshot: Option<UsageSnapshot>,
    pub selected_window: TimeWindow,
    pub loading: bool,
}

impl Dashboard {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            selected_window: TimeWindow::Last7Days,
            loading: false,
        }
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::current(cx);

        // ── Loading skeleton ───────────────────────────────────────
        if self.loading && self.snapshot.is_none() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(render_dashboard_skeleton(window, cx))
                .into_any_element();
        }

        // ── Empty state ────────────────────────────────────────────
        if self.snapshot.is_none() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .child(EmptyState::new(
                    "No usage data yet",
                    "Click \"Scan\" to analyze your transcripts",
                ))
                .into_any_element();
        }

        let snap = self.snapshot.as_ref().unwrap();

        // ── Header row: range label + time window picker ───────────
        let range_label = format!(
            "{} – {}",
            snap.start_date.format("%b %d"),
            snap.end_date.format("%b %d, %Y")
        );

        let header = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.5))
                    .text_color(theme.text_secondary)
                    .child(SharedString::from(range_label)),
            )
            .child({
                let weak = cx.entity().downgrade();
                TimeWindowPicker::new(
                    self.selected_window,
                    move |tw: TimeWindow, _window: &mut Window, cx: &mut gpui::App| {
                        let _ = weak.update(cx, |this, cx| {
                            if this.selected_window != tw {
                                this.selected_window = tw;
                                // Emit event so AppView triggers a rescan
                                cx.emit(WindowChanged);
                                cx.notify();
                            }
                        });
                    },
                )
            });

        // ── Headline stats ─────────────────────────────────────────
        let cost_str = SharedString::from(format_cost(snap.cost_usd));
        let tokens_str = SharedString::from(format_tokens(snap.total_tokens));
        let events_str = SharedString::from(snap.event_count.to_string());
        let sessions_str = SharedString::from(snap.session_count.to_string());

        let stat_cards = div()
            .flex()
            .gap(px(12.0))
            .child(StatCard::new("Total Cost", cost_str))
            .child(StatCard::new("Tokens", tokens_str))
            .child(StatCard::new("Events", events_str))
            .child(StatCard::new("Sessions", sessions_str));

        // ── Provider share + Daily chart (side by side) ────────────
        let mut provider_section = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w(px(300.0))
            .flex_none()
            .child(SectionHeader::new("By Provider"));

        for prov in &snap.by_provider {
            let color = provider_color(&theme, prov.provider);
            provider_section = provider_section.child(
                ProviderRow::new(
                    prov.provider.label(),
                    format_cost(prov.cost_usd),
                    prov.cost_fraction as f32,
                    color,
                )
                .detail(format!(
                    "{} share · {}",
                    format_percent(prov.cost_fraction),
                    format_tokens(prov.total_tokens)
                )),
            );
        }

        let chart = DailyChart::new(snap.daily.clone());

        let summary_chart_row = div()
            .flex()
            .items_start()
            .gap(px(28.0))
            .child(provider_section)
            .child(chart);

        // ── Token breakdown strip ──────────────────────────────────
        let active_days = snap.daily.iter().filter(|d| d.total_tokens > 0).count();
        let daily_avg = if active_days > 0 {
            snap.total_tokens as f64 / active_days as f64
        } else {
            0.0
        };
        let observed_input = snap.tokens.fresh_input + snap.tokens.cached_input;
        let cached_share = if observed_input > 0 {
            snap.tokens.cached_input as f64 / observed_input as f64
        } else {
            0.0
        };

        // Add cache savings tile when savings are non-zero
        let mut tiles = vec![
            (
                "Processed Tokens".to_owned(),
                format_tokens_compact(snap.total_tokens as f64),
                format!("~{} per active day", format_tokens_compact(daily_avg)),
            ),
            (
                "Cached Input".to_owned(),
                format_tokens_compact(snap.tokens.cached_input as f64),
                format!("{} of observed input", format_percent(cached_share)),
            ),
            (
                "Fresh Input".to_owned(),
                format_tokens_compact(snap.tokens.fresh_input as f64),
                format!(
                    "Cache writes: {}",
                    format_tokens_compact(snap.tokens.cache_write as f64)
                ),
            ),
            (
                "Output".to_owned(),
                format_tokens_compact(snap.tokens.output as f64),
                format!(
                    "Incl. reasoning: {}",
                    format_tokens_compact(snap.tokens.reasoning as f64)
                ),
            ),
        ];

        if snap.cache_savings_usd > 0.001 {
            tiles.push((
                "Cache Savings".to_owned(),
                format_cost(snap.cache_savings_usd),
                format!(
                    "{} of total cost",
                    format_percent(
                        if snap.cost_usd > 0.0 {
                            snap.cache_savings_usd / snap.cost_usd
                        } else {
                            0.0
                        }
                    )
                ),
            ));
        }

        let metric_strip = render_metric_strip(tiles, window, cx);

        // ── Model breakdown ────────────────────────────────────────
        let mut model_section = div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(SectionHeader::new("By Model"));

        for (i, model) in snap.by_model.iter().take(15).enumerate() {
            let color = provider_color(&theme, model.provider);
            model_section = model_section.child(ModelRow::new(
                format!("model-{}", i),
                model.model_name.clone(),
                format_cost(model.cost_usd),
                format_tokens(model.total_tokens),
                color,
            ));
        }

        // ── Scan info ──────────────────────────────────────────────
        let scan_info = div()
            .text_size(px(9.5))
            .text_color(theme.text_ghost)
            .child(SharedString::from(format!(
                "Scanned in {}ms · {} to {}",
                snap.scan_time_ms,
                snap.start_date.format("%b %d"),
                snap.end_date.format("%b %d, %Y")
            )));

        // ── Compose via ScrollArea ─────────────────────────────────
        ScrollArea::new("dashboard-scroll")
            .child(header)
            .child(stat_cards)
            .child(summary_chart_row)
            .child(metric_strip)
            .child(model_section)
            .child(scan_info)
            .into_any_element()
    }
}
