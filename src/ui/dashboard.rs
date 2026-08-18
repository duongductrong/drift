use gpui::{div, prelude::*, px, Context, SharedString, Window};
use crate::core::types::{spans_multiple_months, Granularity, TimeWindow, UsageSnapshot};
use crate::theme::Theme;
use super::components::*;
use super::empty_state::EmptyState;
use super::metric_tile::render_metric_strip;
use super::model_row::ModelRow;
use super::provider_row::ProviderRow;
use super::scroll_area::ScrollArea;
use super::skeleton::render_dashboard_skeleton;
use super::usage_chart::UsageChart;
use super::usage_filters::UsageFilters;

/// Emitted when the user picks a different time window so the parent
/// (`AppView`) can trigger a rescan. Carries no payload: the new window is
/// already stored on `Dashboard::selected_window` before this is emitted.
///
/// Granularity changes deliberately emit nothing: they re-bucket data the
/// snapshot already holds, so the switch never costs a rescan.
#[derive(Clone, Debug)]
pub struct WindowChanged;

impl gpui::EventEmitter<WindowChanged> for Dashboard {}

pub struct Dashboard {
    pub snapshot: Option<UsageSnapshot>,
    pub selected_window: TimeWindow,
    /// The granularity the user last asked for. Kept even while a range that
    /// cannot honor it is selected, so returning to a longer range restores
    /// the Monthly view rather than silently resetting it.
    pub preferred_granularity: Granularity,
    pub range_menu_open: bool,
    pub loading: bool,
}

impl Dashboard {
    pub fn new() -> Self {
        Self {
            snapshot: None,
            // 30 days daily is the widest view that still shows every day as
            // its own bar, and it spans two calendar months so the Monthly
            // switch is live out of the box.
            selected_window: TimeWindow::Last30Days,
            preferred_granularity: Granularity::Daily,
            range_menu_open: false,
            loading: false,
        }
    }

    /// The granularity actually drawn: the preference, downgraded to `Daily`
    /// when the active range sits inside one calendar month and Monthly would
    /// collapse it to a single bar.
    fn effective_granularity(&self, monthly_available: bool) -> Granularity {
        if monthly_available {
            self.preferred_granularity
        } else {
            Granularity::Daily
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

        // ── Filter bar: range + aggregation, then what they resolve to ──
        //
        // Availability is read off the snapshot's own dates rather than
        // recomputed from today, so the switch always describes the data on
        // screen — including while a rescan for a new range is still in
        // flight.
        let monthly_available = spans_multiple_months(snap.start_date, snap.end_date);
        let granularity = self.effective_granularity(monthly_available);

        // Spells out the range → aggregation relationship in words, so the
        // pill above never has to be decoded: "these dates, one bar per day".
        let caption = format!(
            "{} – {} · one bar per {}",
            snap.start_date.format("%b %d"),
            snap.end_date.format("%b %d, %Y"),
            granularity.bucket_noun(),
        );

        let filters = {
            let weak = cx.entity().downgrade();
            let select_window = weak.clone();
            let select_granularity = weak.clone();
            let toggle_menu = weak.clone();
            let dismiss_menu = weak.clone();

            UsageFilters::new(self.selected_window, granularity)
                .monthly_available(monthly_available)
                .menu_open(self.range_menu_open)
                .on_select_window(move |tw, _window, cx| {
                    let _ = select_window.update(cx, |this, cx| {
                        this.range_menu_open = false;
                        if this.selected_window != tw {
                            this.selected_window = tw;
                            // A different range means different events: only
                            // this path costs a rescan.
                            cx.emit(WindowChanged);
                        }
                        cx.notify();
                    });
                })
                .on_select_granularity(move |g, _window, cx| {
                    let _ = select_granularity.update(cx, |this, cx| {
                        if this.preferred_granularity != g {
                            this.preferred_granularity = g;
                            // No rescan: the range, the snapshot and every
                            // other panel stay exactly as they were.
                            cx.notify();
                        }
                    });
                })
                .on_toggle_menu(move |_window, cx| {
                    let _ = toggle_menu.update(cx, |this, cx| {
                        this.range_menu_open = !this.range_menu_open;
                        cx.notify();
                    });
                })
                .on_dismiss_menu(move |_window, cx| {
                    let _ = dismiss_menu.update(cx, |this, cx| {
                        if this.range_menu_open {
                            this.range_menu_open = false;
                            cx.notify();
                        }
                    });
                })
        };

        // The caption leads, the controls sit at the trailing edge: reading
        // order is "here is the range on screen", then the pill that changes
        // it. The caption takes the slack so the pill stays pinned right.
        let header = div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.5))
                    .text_color(theme.text_tertiary)
                    .child(SharedString::from(caption)),
            )
            .child(filters);

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

        // ── Provider share + usage chart (side by side) ────────────
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

        let chart = UsageChart::new(granularity.bucket(&snap.daily), granularity);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_view_is_thirty_days_of_daily_bars() {
        let dashboard = Dashboard::new();
        assert_eq!(dashboard.selected_window, TimeWindow::Last30Days);
        assert_eq!(dashboard.effective_granularity(true), Granularity::Daily);
    }

    #[test]
    fn a_single_month_range_falls_back_to_daily_without_losing_the_preference() {
        let mut dashboard = Dashboard::new();
        dashboard.preferred_granularity = Granularity::Monthly;

        // "This month" cannot honor Monthly, so the chart draws daily bars…
        assert_eq!(dashboard.effective_granularity(false), Granularity::Daily);
        // …but the preference is untouched, so a longer range restores it
        // instead of quietly resetting the user's choice.
        assert_eq!(dashboard.preferred_granularity, Granularity::Monthly);
        assert_eq!(dashboard.effective_granularity(true), Granularity::Monthly);
    }
}
