use chrono::Datelike;
use gpui::*;
use crate::theme::Theme;
use crate::core::types::{Granularity, PeriodBucket, Provider};
use crate::ui::components::provider_color;

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

/// A stacked cost bar chart over the active time window. One bar is one day or
/// one calendar month, per the [`Granularity`] it is handed.
#[derive(IntoElement)]
pub struct UsageChart {
    buckets: Vec<PeriodBucket>,
    granularity: Granularity,
}

impl UsageChart {
    pub fn new(buckets: Vec<PeriodBucket>, granularity: Granularity) -> Self {
        Self {
            buckets,
            granularity,
        }
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

fn format_usd(value: f64) -> String {
    if value >= 1.0 {
        format!("${:.2}", value)
    } else if value >= 0.01 {
        format!("${:.3}", value)
    } else if value > 0.0 {
        format!("${:.4}", value)
    } else {
        "$0".to_owned()
    }
}

impl RenderOnce for UsageChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

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
                    // "Daily Cost" / "Monthly Cost" — the chart names the
                    // aggregation it is currently drawing.
                    .child(SharedString::from(format!(
                        "{} Cost",
                        self.granularity.label()
                    ))),
            )
            .child(legend);

        // ── Compute scale ───────────────────────────────────────────
        let peak = self
            .buckets
            .iter()
            .map(|b| b.cost_usd)
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
                        format_usd(tick)
                    })),
            );
        }

        // ── Plot canvas ─────────────────────────────────────────────
        let bucket_data = self.buckets.clone();
        let chart_colors: Vec<Hsla> = Provider::ALL
            .iter()
            .map(|p| provider_color(&theme, *p))
            .collect();
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

                if bucket_data.is_empty() || max_val <= 0.0 {
                    return;
                }

                let (bar_w, x_offset) = bar_layout(bounds.size.width, bucket_data.len());
                let mut x = bounds.origin.x + x_offset;

                for bucket in &bucket_data {
                    // Collect per-provider costs and colors for this bucket
                    let segments: Vec<(f64, Hsla)> = bucket
                        .by_provider
                        .iter()
                        .enumerate()
                        .filter_map(|(i, pm)| {
                            if pm.cost_usd > 0.0 && i < chart_colors.len() {
                                Some((pm.cost_usd, chart_colors[i]))
                            } else {
                                None
                            }
                        })
                        .collect();

                    let total: f64 = segments.iter().map(|(c, _)| c).sum();

                    if total > 0.0 {
                        let total_h = bounds.size.height * (total / max_val) as f32;
                        let mut y_offset = bounds.bottom();

                        // Draw stacked bars bottom-to-top
                        for (cost, color) in &segments {
                            let seg_h = total_h * (*cost / total) as f32;
                            y_offset = y_offset - seg_h;
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
        .flex_1()
        .h(px(CHART_HEIGHT));

        // ── Compose ─────────────────────────────────────────────────
        div()
            .flex_1()
            .min_w(px(320.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(header)
            .child(div().flex().child(gutter).child(plot))
            .child(self.x_axis(&theme))
    }
}

impl UsageChart {
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
