use std::sync::Arc;
use std::time::Duration;

use chrono::Datelike;
use gpui::prelude::*;
use gpui::*;
use crate::theme::Theme;
use crate::core::types::{UsageMetric, Granularity, PeriodBucket, Provider};
use crate::ui::components::{format_metric, format_metric_compact, provider_color};
use crate::ui::tooltip::{Tooltip, TooltipRow};

/// Chart height matching Waku's `h-56` plot.
const CHART_HEIGHT: f32 = 224.0;
/// Y-axis label gutter width.
const CHART_GUTTER: f32 = 56.0;
/// Gap between adjacent bars.
const BAR_GAP: f32 = 2.0;
/// Ceiling on a single bar's width. Without it a monthly rollup of a 90-day
/// range spreads three bars across the whole plot, which reads as a block of
/// color rather than a series. Capped bars are centered instead of stretched.
const MAX_BAR_WIDTH: f32 = 44.0;
/// How long the pointer has to rest on a bar before its tooltip appears.
/// Reading a chart is a pointing gesture, not a "did you mean" hint: GPUI's
/// half-second default reads as lag when sweeping across a month of bars.
const TOOLTIP_DELAY: Duration = Duration::from_millis(60);
/// Height of the Cost / Tokens switch below the plot.
const SWITCH_HEIGHT: f32 = 22.0;

type MetricCallback = Arc<dyn Fn(UsageMetric, &mut Window, &mut App) + Send + Sync>;

/// A stacked bar chart over the active time window. One bar is one day or one
/// calendar month, per the [`Granularity`] it is handed, measured in whichever
/// [`UsageMetric`] the page is currently read in.
#[derive(IntoElement)]
pub struct UsageChart {
    buckets: Vec<PeriodBucket>,
    granularity: Granularity,
    metric: UsageMetric,
    on_select_metric: Option<MetricCallback>,
}

impl UsageChart {
    pub fn new(buckets: Vec<PeriodBucket>, granularity: Granularity, metric: UsageMetric) -> Self {
        Self {
            buckets,
            granularity,
            metric,
            on_select_metric: None,
        }
    }

    pub fn on_select_metric(
        mut self,
        handler: impl Fn(UsageMetric, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_select_metric = Some(Arc::new(handler));
        self
    }
}

/// Width of one bar and the x where the group starts, for a plot `width` wide.
///
/// Shared by the canvas and the x-axis labels so the two stay in step: both
/// resolve to `flex-basis: 0` cells capped at [`MAX_BAR_WIDTH`] and centered.
fn bar_layout(width: Pixels, count: usize) -> (Pixels, Pixels) {
    if count == 0 {
        return (px(0.0), px(0.0));
    }
    let gaps = px(BAR_GAP) * count.saturating_sub(1) as f32;
    let available = width - gaps;
    let bar_w = (available / count as f32).min(px(MAX_BAR_WIDTH)).max(px(1.0));
    let content_w = bar_w * count as f32 + gaps;
    let x_offset = ((width - content_w) / 2.0).max(px(0.0));
    (bar_w, x_offset)
}

/// Compute a nice round ceiling and tick positions for a chart axis.
fn nice_scale(peak: f64, target_ticks: usize) -> (f64, Vec<f64>) {
    if peak <= 0.0 {
        return (1.0, vec![0.0]);
    }
    let rough = peak / target_ticks as f64;
    let mag = 10.0_f64.powf(rough.log10().floor());
    let nice = if rough / mag <= 1.5 {
        mag
    } else if rough / mag <= 3.5 {
        2.0 * mag
    } else if rough / mag <= 7.5 {
        5.0 * mag
    } else {
        10.0 * mag
    };
    let max = (peak / nice).ceil() * nice;
    let ticks: Vec<f64> = (0..=target_ticks)
        .map(|i| nice * i as f64)
        .filter(|v| *v <= max)
        .collect();
    (max, ticks)
}

impl RenderOnce for UsageChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let metric = self.metric;

        // ── Header: title + legend ──────────────────────────────────
        let mut legend = div().flex().items_center().gap(px(14.0));
        for provider in Provider::ALL {
            let color = provider_color(&theme, provider);
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .flex_none()
                            .rounded_full()
                            .bg(color),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(theme.text_secondary)
                            .child(provider.label()),
                    ),
            );
        }

        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    // "Daily Cost" / "Monthly Tokens" — the chart names both the
                    // aggregation and the unit it is currently drawing.
                    .child(SharedString::from(format!(
                        "{} {}",
                        self.granularity.label(),
                        metric.label()
                    ))),
            )
            .child(legend);

        // ── Series + scale ──────────────────────────────────────────
        //
        // Segments are resolved once, in the selected metric's unit, and the
        // scale is taken from their sums: the axis then describes exactly what
        // is painted rather than a total the stack may not add up to.
        let chart_colors: Vec<Hsla> = Provider::ALL
            .iter()
            .map(|p| provider_color(&theme, *p))
            .collect();
        let series: Vec<Vec<(f64, Hsla)>> = self
            .buckets
            .iter()
            .map(|bucket| {
                bucket
                    .by_provider
                    .iter()
                    .enumerate()
                    .filter_map(|(i, pm)| {
                        let value = metric.of_period(pm);
                        (value > 0.0 && i < chart_colors.len()).then(|| (value, chart_colors[i]))
                    })
                    .collect()
            })
            .collect();
        let peak = series
            .iter()
            .map(|stack| stack.iter().map(|(v, _)| *v).sum::<f64>())
            .fold(0.0_f64, f64::max);
        let (max_val, ticks) = nice_scale(peak, 4);

        // ── Y-axis gutter ───────────────────────────────────────────
        let mut gutter = div()
            .relative()
            .w(px(CHART_GUTTER))
            .h(px(CHART_HEIGHT))
            .flex_none();
        for &tick in &ticks {
            let y_frac = if max_val > 0.0 {
                1.0 - (tick / max_val)
            } else {
                1.0
            };
            let top_px = (y_frac as f32 * CHART_HEIGHT - 7.0).max(0.0);
            gutter = gutter.child(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(top_px))
                    .text_size(px(9.5))
                    .text_color(theme.text_tertiary)
                    .child(SharedString::from(if tick == 0.0 {
                        "0".to_owned()
                    } else {
                        format_metric_compact(metric, tick)
                    })),
            );
        }

        // ── Plot canvas ─────────────────────────────────────────────
        let grid_color = theme.border;

        let plot = canvas(
            |_, _, _| {},
            move |bounds: Bounds<Pixels>, _, window: &mut Window, _cx: &mut App| {
                // Gridlines
                for &tick in &ticks {
                    let y_frac = if max_val > 0.0 {
                        1.0 - (tick / max_val)
                    } else {
                        1.0
                    };
                    let y = bounds.origin.y + bounds.size.height * y_frac as f32;
                    window.paint_quad(quad(
                        Bounds::new(point(bounds.origin.x, y), size(bounds.size.width, px(1.0))),
                        px(0.0),
                        grid_color,
                        px(0.0),
                        transparent_black(),
                        BorderStyle::default(),
                    ));
                }

                if series.is_empty() || max_val <= 0.0 {
                    return;
                }

                let (bar_w, x_offset) = bar_layout(bounds.size.width, series.len());
                let mut x = bounds.origin.x + x_offset;

                for segments in &series {
                    let total: f64 = segments.iter().map(|(v, _)| v).sum();

                    if total > 0.0 {
                        let total_h = bounds.size.height * (total / max_val) as f32;
                        let mut y_offset = bounds.bottom();

                        // Draw stacked bars bottom-to-top
                        for (value, color) in segments {
                            let seg_h = total_h * (*value / total) as f32;
                            y_offset -= seg_h;
                            window.paint_quad(quad(
                                Bounds::new(point(x, y_offset), size(bar_w, seg_h)),
                                px(1.0),
                                *color,
                                px(0.0),
                                transparent_black(),
                                BorderStyle::default(),
                            ));
                        }
                    }
                    x = x + bar_w + px(BAR_GAP);
                }
            },
        )
        .w_full()
        .h_full();

        // The canvas paints; the overlay sitting on top of it is what the
        // pointer can actually reach.
        let plot_stack = div()
            .relative()
            .flex_1()
            .h(px(CHART_HEIGHT))
            .child(plot)
            .child(self.hover_overlay(&theme));

        // ── Compose ─────────────────────────────────────────────────
        div()
            .flex_1()
            .min_w(px(320.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(header)
            .child(div().flex().child(gutter).child(plot_stack))
            .child(self.x_axis(&theme))
            .child(self.metric_switch(&theme))
    }
}

impl UsageChart {
    /// Transparent hit targets, one per bar, laid over the plot.
    ///
    /// The bars are painted into a canvas, which has nothing to hover: this row
    /// mirrors the bar layout with real elements so each period can carry a
    /// column highlight and a tooltip. The cells run the full height of the
    /// plot, so a bar only a few pixels tall is still easy to hit.
    fn hover_overlay(&self, theme: &Theme) -> impl IntoElement {
        let metric = self.metric;
        // A daily bar names its day; a monthly bar names the month it rolls up.
        let title_fmt = match self.granularity {
            Granularity::Daily => "%b %d, %Y",
            Granularity::Monthly => "%B %Y",
        };

        let mut overlay = div()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .flex()
            .justify_center()
            .gap(px(BAR_GAP));

        for (i, bucket) in self.buckets.iter().enumerate() {
            let rows: Vec<TooltipRow> = Provider::ALL
                .iter()
                .zip(&bucket.by_provider)
                .filter_map(|(provider, pm)| {
                    let value = metric.of_period(pm);
                    (value > 0.0).then(|| TooltipRow {
                        color: provider_color(theme, *provider),
                        label: provider.label().into(),
                        value: format_metric(metric, value).into(),
                    })
                })
                .collect();

            let total: f64 = bucket.by_provider.iter().map(|pm| metric.of_period(pm)).sum();
            let headline = if total > 0.0 {
                format_metric(metric, total)
            } else {
                "No usage".to_owned()
            };
            let title = SharedString::from(bucket.start.format(title_fmt).to_string());

            overlay = overlay.child(
                div()
                    .id(SharedString::from(format!("chart-bar-{}", i)))
                    .flex_1()
                    .min_w_0()
                    .max_w(px(MAX_BAR_WIDTH))
                    .h_full()
                    .rounded(px(2.0))
                    .cursor_default()
                    .hover(|style| style.bg(theme.overlay))
                    .tooltip(Tooltip::detail(title, headline, rows))
                    .tooltip_show_delay(TOOLTIP_DELAY),
            );
        }

        overlay
    }

    /// The Cost / Tokens switch.
    ///
    /// It sits under the plot's trailing corner rather than in the header: the
    /// header already carries the title and the provider legend, and the choice
    /// of unit reads as a footnote about the axis, not a third thing to scan on
    /// the way in. Styled as the same segmented pill as the Daily/Monthly
    /// switch, because it is the same kind of choice.
    fn metric_switch(&self, theme: &Theme) -> impl IntoElement {
        let mut segments = div()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border_strong);

        for option in UsageMetric::ALL {
            let is_selected = option == self.metric;
            let on_select = self.on_select_metric.clone();

            segments = segments.child(
                div()
                    .id(SharedString::from(format!("metric-{}", option.label())))
                    .h(px(SWITCH_HEIGHT))
                    .px(px(9.0))
                    .flex()
                    .items_center()
                    .cursor_default()
                    .text_size(px(10.0))
                    .bg(if is_selected {
                        theme.overlay_strong
                    } else {
                        transparent_black()
                    })
                    .text_color(if is_selected {
                        theme.text
                    } else {
                        theme.text_secondary
                    })
                    .when(!is_selected, |el| {
                        el.hover(|style| style.text_color(theme.text))
                    })
                    .child(option.label())
                    .on_click(move |_event, window, cx| {
                        if let Some(handler) = &on_select {
                            handler(option, window, cx);
                        }
                    }),
            );
        }

        div().flex().justify_end().child(segments)
    }

    /// X-axis labels.
    ///
    /// Monthly bars are few and wide, so every bucket gets a label sitting in a
    /// cell that mirrors the bar layout. A daily series has far too many bars
    /// for that, so it falls back to first/middle/last markers spread across
    /// the plot.
    fn x_axis(&self, theme: &Theme) -> impl IntoElement {
        let row = div()
            .pl(px(CHART_GUTTER))
            .text_size(px(9.5))
            .text_color(theme.text_tertiary);

        if self.buckets.is_empty() {
            return row;
        }

        match self.granularity {
            Granularity::Monthly => {
                // Disambiguate the month names only when the range straddles a
                // year boundary (Dec → Jan).
                let multi_year = self.buckets.first().map(|b| b.start.year())
                    != self.buckets.last().map(|b| b.start.year());
                let fmt = if multi_year { "%b %y" } else { "%b" };

                let mut row = row.flex().justify_center().gap(px(BAR_GAP));
                for bucket in &self.buckets {
                    row = row.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .max_w(px(MAX_BAR_WIDTH))
                            .overflow_hidden()
                            .text_center()
                            .child(SharedString::from(bucket.start.format(fmt).to_string())),
                    );
                }
                row
            }
            Granularity::Daily => {
                let mut marks = vec![
                    self.buckets.first().unwrap().start,
                    self.buckets[self.buckets.len() / 2].start,
                    self.buckets.last().unwrap().start,
                ];
                marks.dedup();

                let mut row = row.flex().justify_between();
                for date in marks {
                    row = row.child(SharedString::from(date.format("%b %d").to_string()));
                }
                row
            }
        }
    }
}
