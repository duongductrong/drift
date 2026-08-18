use gpui::*;
use crate::theme::Theme;
use crate::core::types::DailyAggregate;

/// Chart height matching Waku's `h-56` plot.
const CHART_HEIGHT: f32 = 224.0;
/// Y-axis label gutter width.
const CHART_GUTTER: f32 = 56.0;

/// A daily cost bar chart rendered with GPUI's canvas element.
#[derive(IntoElement)]
pub struct DailyChart {
    daily: Vec<DailyAggregate>,
}

impl DailyChart {
    pub fn new(daily: Vec<DailyAggregate>) -> Self {
        Self { daily }
    }
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

impl RenderOnce for DailyChart {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);

        // ── Header: title + legend ──────────────────────────────────
        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Daily Cost"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(
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
                                    .bg(theme.chart_claude),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_secondary)
                                    .child("Claude"),
                            ),
                    )
                    .child(
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
                                    .bg(theme.chart_codex),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme.text_secondary)
                                    .child("Codex"),
                            ),
                    ),
            );

        // ── Compute scale ───────────────────────────────────────────
        let peak = self
            .daily
            .iter()
            .map(|d| d.cost_usd)
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
        let daily_data = self.daily.clone();
        let chart_claude = theme.chart_claude;
        let chart_codex = theme.chart_codex;
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

                if daily_data.is_empty() || max_val <= 0.0 {
                    return;
                }

                let day_count = daily_data.len();
                let gap = px(2.0);
                let available = bounds.size.width - gap * (day_count.saturating_sub(1)) as f32;
                let bar_w = (available / day_count as f32).max(px(1.0));
                let mut x = bounds.origin.x;

                for day in &daily_data {
                    let claude_val = day.by_provider[0].cost_usd;
                    let codex_val = day.by_provider[1].cost_usd;
                    let total = claude_val + codex_val;

                    if total > 0.0 {
                        let total_h = bounds.size.height * (total / max_val) as f32;
                        let claude_h = total_h * (claude_val / total) as f32;
                        let codex_h = total_h - claude_h;

                        // Claude bar (bottom)
                        if claude_val > 0.0 {
                            window.paint_quad(quad(
                                Bounds::new(
                                    point(x, bounds.bottom() - claude_h),
                                    size(bar_w, claude_h),
                                ),
                                px(1.0),
                                chart_claude,
                                px(0.0),
                                transparent_black(),
                                BorderStyle::default(),
                            ));
                        }
                        // Codex bar (stacked above Claude)
                        if codex_val > 0.0 {
                            window.paint_quad(quad(
                                Bounds::new(
                                    point(x, bounds.bottom() - claude_h - codex_h),
                                    size(bar_w, codex_h),
                                ),
                                px(1.0),
                                chart_codex,
                                px(0.0),
                                transparent_black(),
                                BorderStyle::default(),
                            ));
                        }
                    }
                    x = x + bar_w + gap;
                }
            },
        )
        .flex_1()
        .h(px(CHART_HEIGHT));

        // ── X-axis labels ───────────────────────────────────────────
        let mut x_axis = div()
            .pl(px(CHART_GUTTER + 8.0))
            .flex()
            .justify_between()
            .text_size(px(9.5))
            .text_color(theme.text_tertiary);

        if !self.daily.is_empty() {
            let first = self.daily.first().unwrap().date;
            let mid = self.daily[self.daily.len() / 2].date;
            let last = self.daily.last().unwrap().date;
            x_axis = x_axis
                .child(SharedString::from(first.format("%b %d").to_string()))
                .child(SharedString::from(mid.format("%b %d").to_string()))
                .child(SharedString::from(last.format("%b %d").to_string()));
        }

        // ── Compose ─────────────────────────────────────────────────
        div()
            .flex_1()
            .min_w(px(320.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(header)
            .child(div().flex().child(gutter).child(plot))
            .child(x_axis)
    }
}
