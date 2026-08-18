use gpui::{div, prelude::*, px, Context, SharedString, Window};
use crate::core::types::{
    spans_multiple_months, Granularity, TimeWindow, UsageMetric, UsageSnapshot,
};
use crate::settings::Settings;
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
    /// The unit the whole view is read in — the chart, the headline stats, and
    /// the way the provider and model lists are valued and ranked. Purely a
    /// view choice over the snapshot already in hand, so switching it costs a
    /// re-render and nothing more.
    pub metric: UsageMetric,
    pub range_menu_open: bool,
    pub loading: bool,
}

impl Dashboard {
    /// Opens on `window` — the range the user set as their default. 30 days
    /// daily is the shipped default: the widest view that still shows every day
    /// as its own bar, spanning two calendar months so the Monthly switch is
    /// live out of the box.
    pub fn new(window: TimeWindow) -> Self {
        Self {
            snapshot: None,
            selected_window: window,
            preferred_granularity: Granularity::Daily,
            // Cost leads: it is the question most people open the app with.
            metric: UsageMetric::Cost,
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
        // Read up front: the elements built below borrow `cx` for the rest of
        // this frame.
        let model_rows = Settings::current(cx).model_rows;

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
                    "Use the refresh button to analyze your transcripts",
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
        let metric = self.metric;
        let caption = format!(
            "{} – {} · one bar per {} · measured in {}",
            snap.start_date.format("%b %d"),
            snap.end_date.format("%b %d, %Y"),
            granularity.bucket_noun(),
            metric.label().to_lowercase(),
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
        //
        // The selected metric leads and is marked active; its counterpart sits
        // right beside it, because switching should re-rank the page, not hide
        // half of what it knows.
        let other = metric.other();
        let stat_cards = div()
            .flex()
            .gap(px(12.0))
            .child(
                StatCard::new(
                    metric.summary_label(),
                    SharedString::from(format_metric(metric, metric.of_snapshot(snap))),
                )
                .active(true),
            )
            .child(StatCard::new(
                other.summary_label(),
                SharedString::from(format_metric(other, other.of_snapshot(snap))),
            ))
            .child(StatCard::new(
                "Events",
                SharedString::from(format_count(snap.event_count)),
            ))
            .child(StatCard::new(
                "Sessions",
                SharedString::from(format_count(snap.session_count)),
            ));

        // ── Provider share + usage chart (side by side) ────────────
        let mut provider_section = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .w(px(300.0))
            .flex_none()
            .child(SectionHeader::new("By Provider").hint(metric.label()));

        // Ranked by the selected metric rather than by the scanner's cost
        // order: under Tokens the cheap-but-busy provider belongs at the top.
        let mut providers: Vec<_> = snap.by_provider.iter().collect();
        providers.sort_by(|a, b| {
            metric
                .of_provider(b)
                .partial_cmp(&metric.of_provider(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for prov in providers {
            let color = provider_color(&theme, prov.provider);
            let share = metric.share_of(prov);
            provider_section = provider_section.child(
                ProviderRow::new(
                    prov.provider.label(),
                    format_metric(metric, metric.of_provider(prov)),
                    share as f32,
                    color,
                )
                .detail(format!(
                    "{} share · {}",
                    format_percent(share),
                    format_metric(other, other.of_provider(prov))
                )),
            );
        }

        let chart = {
            let select_metric = cx.entity().downgrade();

            UsageChart::new(granularity.bucket(&snap.daily), granularity, metric)
                .on_select_metric(move |selected, _window, cx| {
                    let _ = select_metric.update(cx, |this, cx| {
                        if this.metric != selected {
                            this.metric = selected;
                            // Same events, different unit: like granularity,
                            // this re-renders the page and never reaches the
                            // scanner.
                            cx.notify();
                        }
                    });
                })
        };

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
            .child(SectionHeader::new("By Model").hint(SharedString::from(format!(
                "Top {} by {}",
                model_rows.min(snap.by_model.len()),
                metric.label().to_lowercase()
            ))));

        // Same reordering as the provider list: "top models" has to mean top
        // by whatever the page is currently measuring, or the truncation to
        // `model_rows` quietly drops the rows that matter.
        let mut models: Vec<_> = snap.by_model.iter().collect();
        models.sort_by(|a, b| {
            metric
                .of_model(b)
                .partial_cmp(&metric.of_model(a))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    other
                        .of_model(b)
                        .partial_cmp(&other.of_model(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        for (i, model) in models.into_iter().take(model_rows).enumerate() {
            let color = provider_color(&theme, model.provider);
            model_section = model_section.child(ModelRow::new(
                format!("model-{}", i),
                model.model_name.clone(),
                format_metric(metric, metric.of_model(model)),
                format_metric(other, other.of_model(model)),
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
        let dashboard = Dashboard::new(TimeWindow::Last30Days);
        assert_eq!(dashboard.selected_window, TimeWindow::Last30Days);
        assert_eq!(dashboard.effective_granularity(true), Granularity::Daily);
    }

    #[test]
    fn the_view_opens_measured_in_cost() {
        assert_eq!(Dashboard::new(TimeWindow::Last30Days).metric, UsageMetric::Cost);
    }

    #[test]
    fn a_single_month_range_falls_back_to_daily_without_losing_the_preference() {
        let mut dashboard = Dashboard::new(TimeWindow::Last30Days);
        dashboard.preferred_granularity = Granularity::Monthly;

        // "This month" cannot honor Monthly, so the chart draws daily bars…
        assert_eq!(dashboard.effective_granularity(false), Granularity::Daily);
        // …but the preference is untouched, so a longer range restores it
        // instead of quietly resetting the user's choice.
        assert_eq!(dashboard.preferred_granularity, Granularity::Monthly);
        assert_eq!(dashboard.effective_granularity(true), Granularity::Monthly);
    }
}
