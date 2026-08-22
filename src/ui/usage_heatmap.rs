use chrono::{Datelike, NaiveDate, Weekday};
use gpui::{
    div, prelude::*, px, App, FontWeight, Pixels, SharedString, Window,
};

use crate::core::types::{
    activity_weeks, ActivityStats, ActivityWeek, DailyAggregate, HeatScale, Provider, UsageMetric,
    HEAT_STEPS,
};
use crate::theme::Theme;
use crate::ui::components::{format_count, format_metric};
use crate::ui::tooltip::{Tooltip, TooltipRow};

// ---------------------------------------------------------------------------
// UsageHeatmap — the activity grid: one cell per day, seven rows of weekdays,
// one column per week.
//
// The bar chart answers how much was spent and by whom. This answers a question
// the bars cannot: rhythm. Which weekdays the work actually lands on, whether
// weekends are dead, where the streaks and the gaps are. Reading that off a
// grid only works if a cell's color means one thing, so the grid deliberately
// drops the provider dimension — a single hue carries magnitude, and the
// breakdown moves into the tooltip. Painting hue by provider and lightness by
// amount would put a dark purple next to a dark green and ask the reader to
// compare them, which nobody can do.
//
// The grid never sits beside the chart as a second section: it *swaps* into
// the chart's slot when the page's Chart/Activity switch says so, so the two
// drawings share one geometry. Like the chart it reads [header][body][footer]:
// the intensity legend where the chart puts its provider legend, and the
// grid's insights on the line below where the chart puts its Cost/Tokens
// switch. A side rail would spend width the embedded slot does not have.
//
// Everything here is a pure view transform over `UsageSnapshot::daily`, so the
// grid follows the project filter and the Cost/Tokens switch for free and never
// costs a rescan. It is inherently daily and so ignores the Daily/Monthly
// switch, which is about how many days share a bar.
//
// Usage:
//   UsageHeatmap::new(&snapshot.daily, metric)
// ---------------------------------------------------------------------------

/// Gap between cells, in both directions. Matches the chart's bar gap so the
/// two blocks read as the same drawing.
const CELL_GAP: f32 = 3.0;
/// Smallest a cell may get. Below this the rounding and the gap eat the cell.
const CELL_MIN: f32 = 11.0;
/// Largest a cell may get — deliberately small, so the grid keeps the fine,
/// dense texture a calendar of days needs and its height stays at or under
/// the bar chart's plot it swaps with (seven 18pt rows plus the month strip
/// come to ~157pt against the chart's 224pt plot). Past this a 30-day range
/// reads as a row of chunky tiles rather than a grid.
const CELL_MAX: f32 = 18.0;
/// Width of the weekday label gutter — enough for "Mon" at 9.5pt.
const DAY_GUTTER: f32 = 30.0;
/// Height of the month label strip above the grid.
const MONTH_STRIP: f32 = 13.0;
/// Horizontal chrome the grid's slot loses before it sees any of the window:
/// the dashboard's `ScrollArea` padding on both edges, the scrollbar track
/// riding the right edge, then the provider column the grid shares its row
/// with plus the gap between them.
const SLOT_CHROME: f32 = 20.0 * 2.0 + 12.0 + 300.0 + 28.0;
/// Below this cell size the weekday gutter labels every other row, since seven
/// 9.5pt labels no longer fit in seven rows that short.
const DENSE_LABELS_BELOW: f32 = 16.0;
/// How many columns a month must own before its name is worth printing.
///
/// This doubles as the collision rule: labels are only ever drawn at a month's
/// first column, so requiring two columns per label also puts consecutive
/// labels at least two columns — 28pt at the narrowest cell — apart, which
/// clears a three-letter month name at 9.5pt.
const MONTH_LABEL_MIN_COLUMNS: usize = 2;

/// Monday-first, matching [`ActivityWeek::days`].
const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Side length of a cell, for a grid of `columns` weeks in a `viewport`-wide
/// window.
///
/// Cells stay square at whatever size the window allows: they grow toward the
/// cap as a short range leaves width over, and shrink — never past
/// [`CELL_MIN`] — while the range still fits. A range too wide for the slot
/// at [`CELL_MIN`] is not shrunk into unreadability: the columns scroll, so
/// half-year and multi-year grids keep honest cell sizes.
fn cell_size(viewport: Pixels, columns: usize) -> Pixels {
    if columns == 0 {
        return px(CELL_MAX);
    }
    let budget = f32::from(viewport) - SLOT_CHROME - DAY_GUTTER;
    let gaps = CELL_GAP * columns.saturating_sub(1) as f32;
    px(((budget - gaps) / columns as f32).clamp(CELL_MIN, CELL_MAX))
}

/// Which columns get a month label, and what it says.
///
/// A month is named at the column holding its first in-range day — never nudged
/// to a later one, since a name sitting to the right of where its month starts
/// is worse than no name. The price of that is that a month owning too few
/// columns to be labelled clearly goes unlabelled, which is what happens to the
/// partial months a rolling range opens and closes on: a 90-day window names
/// the two or three full months it covers and leaves its ragged ends to the
/// axis dates the caption already carries.
fn month_labels(weeks: &[ActivityWeek]) -> Vec<(usize, SharedString)> {
    // Each month the range touches, in order: where it starts and how many
    // columns it holds.
    let mut months: Vec<(usize, NaiveDate, usize)> = Vec::new();

    for (column, week) in weeks.iter().enumerate() {
        let Some(first) = week.days.iter().flatten().next() else {
            continue;
        };
        let same_month = months.last().is_some_and(|(_, date, _)| {
            (date.year(), date.month()) == (first.date.year(), first.date.month())
        });
        if same_month {
            months.last_mut().unwrap().2 += 1;
        } else {
            months.push((column, first.date, 1));
        }
    }

    months
        .into_iter()
        .filter(|(_, _, columns)| *columns >= MONTH_LABEL_MIN_COLUMNS)
        .map(|(column, date, _)| (column, month_name(date)))
        .collect()
}

/// A month's name, disambiguated by year only in January — where a 90-day range
/// straddling New Year would otherwise print two identical-looking runs.
fn month_name(date: NaiveDate) -> SharedString {
    let format = if date.month() == 1 { "%b %Y" } else { "%b" };
    SharedString::from(date.format(format).to_string())
}

#[derive(IntoElement)]
pub struct UsageHeatmap {
    weeks: Vec<ActivityWeek>,
    scale: HeatScale,
    stats: ActivityStats,
    metric: UsageMetric,
}

impl UsageHeatmap {
    /// Builds the grid, its intensity scale and its insights from the
    /// snapshot's daily series — which the scan already gap-fills to one entry
    /// per calendar day, so every cell the range covers has a day behind it.
    pub fn new(daily: &[DailyAggregate], metric: UsageMetric) -> Self {
        Self {
            weeks: activity_weeks(daily),
            scale: HeatScale::from_days(daily, metric),
            stats: ActivityStats::from_days(daily, metric),
            metric,
        }
    }
}

impl RenderOnce for UsageHeatmap {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let cell = cell_size(window.viewport_size().width, self.weeks.len());
        let step = f32::from(cell) + CELL_GAP;
        let grid_width = px((step * self.weeks.len() as f32 - CELL_GAP).max(0.0));

        // ── Header ──────────────────────────────────────────────────
        //
        // Bare like the chart panel's header rather than bordered like a page
        // section's, because this swaps into the chart's slot: the two views
        // should read as the same frame holding different content. The legend
        // takes the trailing edge, where the chart puts its provider legend.
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child("Daily Activity"),
            )
            .child(self.legend(&theme));

        // ── Body: fixed weekday gutter + scrollable columns ────────
        //
        // The weekday labels stay pinned while the week columns scroll
        // horizontally, so ranges longer than the slot is wide — half a year
        // and up — stay readable at full cell size instead of shrinking past
        // legibility. Month strip and cells scroll together: they are one
        // drawing.
        let dense = f32::from(cell) < DENSE_LABELS_BELOW;

        let mut gutter = div()
            .flex_none()
            .w(px(DAY_GUTTER))
            .flex()
            .flex_col()
            .gap(px(CELL_GAP))
            // The month strip above the grid has no gutter label; this spacer
            // keeps the weekday rows aligned with theirs.
            .child(div().h(px(MONTH_STRIP)).flex_none());
        for (slot, weekday) in WEEKDAY_LABELS.iter().enumerate() {
            gutter = gutter.child(
                div()
                    .h(cell)
                    .flex_none()
                    .flex()
                    .items_center()
                    .text_size(px(9.5))
                    .text_color(theme.text_tertiary)
                    // Every other row once the cells are too short to label
                    // all seven — the gutter still anchors the reader, and
                    // Mon/Wed/Fri/Sun is enough to count from.
                    .child(if dense && !slot.is_multiple_of(2) {
                        ""
                    } else {
                        *weekday
                    }),
            );
        }

        // ── Month strip ─────────────────────────────────────────────
        //
        // Absolutely positioned rather than one label per column: at the small
        // end of the cell range a month name is several columns wide, and a
        // label laid into a cell-width cell would be clipped to "A".
        let mut months = div().relative().h(px(MONTH_STRIP)).w(grid_width);
        for (column, label) in month_labels(&self.weeks) {
            months = months.child(
                div()
                    .absolute()
                    .left(px(step * column as f32))
                    .text_size(px(9.5))
                    .text_color(theme.text_tertiary)
                    .child(label),
            );
        }

        // ── Cells ───────────────────────────────────────────────────
        let mut rows = div().w(grid_width).flex_none().flex().flex_col().gap(px(CELL_GAP)).child(months);
        for slot in 0..WEEKDAY_LABELS.len() {
            let mut row = div().h(cell).flex().items_center().gap(px(CELL_GAP));
            for (column, week) in self.weeks.iter().enumerate() {
                row = row.child(self.cell(&theme, cell, column, slot, week.days[slot].as_ref()));
            }
            rows = rows.child(row);
        }

        // Wheel-scrollable when the range outruns the slot; a no-op that draws
        // nothing when everything fits.
        let scroller = div()
            .id("heatmap-columns")
            .min_w_0()
            .flex_1()
            .overflow_x_scroll()
            .child(rows);

        // ── Compose ─────────────────────────────────────────────────
        //
        // Same [header][body][footer] silhouette as the bar chart, so the
        // switch between the two views moves nothing but the drawing.
        div()
            .flex_1()
            .min_w(px(320.0))
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(header)
            .child(
                div()
                    .flex()
                    .items_start()
                    .child(gutter)
                    .child(scroller),
            )
            .child(self.footer(&theme))
    }
}

impl UsageHeatmap {
    /// One day's cell.
    ///
    /// Three states, and the difference between them is the whole point of the
    /// grid: a day outside the range draws nothing but still holds its place, a
    /// day the range covers with no usage draws the empty wash, and a day with
    /// usage takes its step off the ramp. The transparent border is carried at
    /// rest so hovering can color it without moving anything.
    fn cell(
        &self,
        theme: &Theme,
        size: Pixels,
        column: usize,
        slot: usize,
        day: Option<&DailyAggregate>,
    ) -> gpui::AnyElement {
        let base = div().size(size).flex_none().rounded(px(3.0));

        let Some(day) = day else {
            // Past an edge of the range: a spacer, so the week above and the
            // week below still line up.
            return base.into_any_element();
        };

        let value = self.metric.of_day(day);
        let level = self.scale.level(value);
        let fill = if level == 0 {
            theme.heat_empty
        } else {
            theme.heat[(level - 1).min(HEAT_STEPS - 1)]
        };

        let rows: Vec<TooltipRow> = Provider::ALL
            .iter()
            .zip(&day.by_provider)
            .filter_map(|(provider, metrics)| {
                let value = self.metric.of_period(metrics);
                (value > 0.0).then(|| TooltipRow {
                    color: crate::ui::components::provider_color(theme, *provider),
                    label: provider.label().into(),
                    value: format_metric(self.metric, value).into(),
                })
            })
            .collect();

        let headline = if value > 0.0 {
            format_metric(self.metric, value)
        } else {
            "No usage".to_owned()
        };

        base.id(SharedString::from(format!("heat-{}-{}", column, slot)))
            .bg(fill)
            .border_1()
            .border_color(gpui::transparent_black())
            .cursor_default()
            .hover(|style| style.border_color(theme.text_secondary))
            .tooltip(Tooltip::detail(
                SharedString::from(day.date.format("%a %b %d, %Y").to_string()),
                headline,
                rows,
            ))
            .into_any_element()
    }

    /// The intensity legend.
    ///
    /// Wordless on purpose. A step is a day's rank among the range's active
    /// days rather than an amount, so numbering the swatches would promise a
    /// reading they do not support — the exact figure is one hover away.
    fn legend(&self, theme: &Theme) -> impl IntoElement {
        let mut swatches = div().flex().items_center().gap(px(3.0));
        for fill in std::iter::once(theme.heat_empty).chain(theme.heat) {
            swatches = swatches.child(div().size(px(9.0)).flex_none().rounded(px(2.0)).bg(fill));
        }

        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .text_size(px(9.5))
            .text_color(theme.text_tertiary)
            .child("Less")
            .child(swatches)
            .child("More")
    }

    /// The reads the grid supports, as one quiet line under it.
    ///
    /// These are exactly the numbers a reader would try to take off the grid
    /// by eye and get wrong — streaks and weekday averages especially. One
    /// truncated line rather than a side rail, because the slot this swaps
    /// into is shared with the provider column; whatever does not fit was
    /// already on the line ahead of it.
    fn footer(&self, theme: &Theme) -> impl IntoElement {
        let metric = self.metric;
        let stats = &self.stats;
        let mut parts: Vec<String> = Vec::new();

        parts.push(format!(
            "{} of {} days active",
            format_count(stats.active_days as u64),
            format_count(stats.total_days as u64)
        ));

        // A one-day "streak" is every range ever scanned, so it says nothing.
        if stats.longest_streak > 1 {
            parts.push(format!("{}-day streak", stats.longest_streak));
        }

        if let Some((weekday, mean)) = stats.busiest_weekday {
            parts.push(format!(
                "{} busiest · {} avg",
                weekday_label(weekday),
                format_metric(metric, mean)
            ));
        }

        if let Some((weekday, mean)) = stats.quietest_weekday {
            parts.push(format!(
                "{} quietest · {} avg",
                weekday_label(weekday),
                format_metric(metric, mean)
            ));
        }

        if let Some((date, value)) = stats.peak_day {
            parts.push(format!(
                "peak {} · {}",
                date.format("%b %d"),
                format_metric(metric, value)
            ));
        }

        div()
            .min_w_0()
            .truncate()
            .text_size(px(10.0))
            .text_color(theme.text_tertiary)
            .child(SharedString::from(parts.join("  ·  ")))
    }
}

fn weekday_label(weekday: Weekday) -> &'static str {
    WEEKDAY_LABELS[weekday.num_days_from_monday() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week(start: &str) -> ActivityWeek {
        ActivityWeek {
            start: start.parse().unwrap(),
            days: std::array::from_fn(|_| None),
        }
    }

    /// A week whose Monday-slot day sits on `date`, so `month_labels` has
    /// something in-range to name the column from.
    fn week_from(date: &str) -> ActivityWeek {
        let date: NaiveDate = date.parse().unwrap();
        let mut week = ActivityWeek {
            start: date,
            days: std::array::from_fn(|_| None),
        };
        week.days[date.weekday().num_days_from_monday() as usize] = Some(DailyAggregate {
            date,
            ..Default::default()
        });
        week
    }

    /// `month_labels` in a form that is readable in an assertion.
    fn labelled(weeks: &[ActivityWeek]) -> Vec<(usize, String)> {
        month_labels(weeks)
            .into_iter()
            .map(|(column, label)| (column, label.to_string()))
            .collect()
    }

    #[test]
    fn cells_grow_into_the_space_a_short_range_leaves() {
        // Five columns in a default 900pt window have room to spare, so the
        // cell sits at its ceiling rather than stretching.
        assert_eq!(cell_size(px(900.0), 5), px(CELL_MAX));
    }

    #[test]
    fn cells_shrink_rather_than_overflow_a_narrow_window() {
        // Fourteen columns at the minimum window width, sharing the row with
        // the provider column: the grid has to give.
        let cell = cell_size(px(640.0), 14);
        assert!(cell < px(CELL_MAX), "expected a shrunk cell, got {cell:?}");
        assert!(cell >= px(CELL_MIN));

        // And whatever it gives, the grid still fits the space it was sized
        // against.
        let width = f32::from(cell) * 14.0 + CELL_GAP * 13.0;
        let budget = 640.0 - SLOT_CHROME - DAY_GUTTER;
        assert!(width <= budget + 0.01, "{width} overflows {budget}");
    }

    #[test]
    fn a_cell_never_shrinks_past_legibility() {
        // A window narrow enough to leave no budget at all still gets cells,
        // even though the columns will overflow their slot: an unreadable
        // grid is a bug, a scrolled one is normal — half-year and multi-year
        // ranges always land here by design.
        assert_eq!(cell_size(px(320.0), 14), px(CELL_MIN));
    }

    #[test]
    fn a_multi_year_range_keeps_honest_cells_and_leans_on_the_scroller() {
        // ~157 weeks of columns cannot fit any real window at a size worth
        // reading; sizing stops at CELL_MIN and hands the overflow to the
        // horizontal scroller instead of shrinking every cell into noise.
        let weeks_3y = 157;
        assert_eq!(cell_size(px(1180.0), weeks_3y), px(CELL_MIN));

        // While a half-year range still shrinks gracefully before reaching
        // that floor.
        assert!(cell_size(px(900.0), 26) > px(CELL_MIN));
    }

    #[test]
    fn an_empty_grid_asks_for_no_geometry() {
        assert_eq!(cell_size(px(900.0), 0), px(CELL_MAX));
    }

    #[test]
    fn each_month_is_named_at_its_first_column() {
        // Jun and Aug own one ragged column each; Jul owns four and is the
        // month the grid is actually about.
        let weeks = vec![
            week_from("2026-06-29"), // Jun
            week_from("2026-07-06"), // Jul starts here
            week_from("2026-07-13"),
            week_from("2026-07-20"),
            week_from("2026-07-27"),
            week_from("2026-08-03"), // Aug
        ];
        assert_eq!(labelled(&weeks), vec![(1, "Jul".to_owned())]);
    }

    #[test]
    fn every_full_month_in_a_long_range_is_named() {
        // The shape a 90-day range takes: a partial month at each end, whole
        // months in between, and a label for each of those.
        let weeks: Vec<ActivityWeek> = (0..14)
            .map(|i| {
                let monday = "2026-05-25".parse::<NaiveDate>().unwrap()
                    + chrono::Days::new(i * 7);
                week_from(&monday.to_string())
            })
            .collect();

        assert_eq!(
            labelled(&weeks),
            vec![
                (1, "Jun".to_owned()),
                (6, "Jul".to_owned()),
                (10, "Aug".to_owned()),
            ]
        );
    }

    #[test]
    fn a_month_too_narrow_to_label_is_left_unlabelled_rather_than_nudged() {
        // Jul owns exactly one column between Jun and Aug. Printing its name
        // one column to the right would point at the wrong week, so it goes
        // unnamed.
        let weeks = vec![
            week_from("2026-06-22"),
            week_from("2026-06-29"),
            week_from("2026-07-06"),
            week_from("2026-08-03"),
        ];
        let labels = labelled(&weeks);
        assert_eq!(labels, vec![(0, "Jun".to_owned())]);
    }

    #[test]
    fn january_carries_its_year_so_a_new_year_range_reads() {
        assert_eq!(month_name("2026-01-05".parse().unwrap()), "Jan 2026");
        assert_eq!(month_name("2025-12-29".parse().unwrap()), "Dec");
    }

    #[test]
    fn a_column_with_no_in_range_day_is_never_labelled() {
        // Only possible at an edge, but a label positioned off a column with
        // nothing in it would name a month the range does not touch.
        assert!(month_labels(&[week("2026-07-06")]).is_empty());
    }

    #[test]
    fn weekday_labels_are_monday_first() {
        assert_eq!(weekday_label(Weekday::Mon), "Mon");
        assert_eq!(weekday_label(Weekday::Sun), "Sun");
    }
}
