use chrono::{Datelike, NaiveDate};

/// Represents the different AI coding agent providers we track
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provider {
    Claude,
    Codex,
    Kimi,
    OpenCode,
    Antigravity,
}

impl Provider {
    pub const ALL: [Provider; 5] = [
        Provider::Claude,
        Provider::Codex,
        Provider::Kimi,
        Provider::OpenCode,
        Provider::Antigravity,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Provider::Claude => "Claude",
            Provider::Codex => "Codex",
            Provider::Kimi => "Kimi",
            Provider::OpenCode => "OpenCode",
            Provider::Antigravity => "Antigravity",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Provider::Claude => 0,
            Provider::Codex => 1,
            Provider::Kimi => 2,
            Provider::OpenCode => 3,
            Provider::Antigravity => 4,
        }
    }
}

/// Token breakdown for one usage event.
///
/// Naming follows Waku's convention: `fresh_input` = uncached input tokens.
/// `total()` excludes `reasoning` (reasoning is a subset of output, not
/// additive) — matches Waku's `TokenTotals::total()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TokenBreakdown {
    pub fresh_input: u64,
    pub cached_input: u64,
    pub cache_write: u64,
    pub output: u64,
    pub reasoning: u64,
}

impl TokenBreakdown {
    pub fn total(&self) -> u64 {
        self.fresh_input + self.cached_input + self.cache_write + self.output
    }

    pub fn add(&mut self, other: &TokenBreakdown) {
        self.fresh_input += other.fresh_input;
        self.cached_input += other.cached_input;
        self.cache_write += other.cache_write;
        self.output += other.output;
        self.reasoning += other.reasoning;
    }
}

/// One parsed usage event from a transcript file
#[derive(Clone, Debug)]
pub struct UsageEvent {
    pub provider: Provider,
    pub timestamp_ms: i64,
    pub model_name: String,
    pub session_key: String,
    /// Absolute path of the directory the session ran in — the project the
    /// work belongs to. Empty when the provider's records do not say, which
    /// groups the event under [`UNKNOWN_PROJECT_LABEL`] rather than dropping
    /// it from the totals.
    pub project_path: String,
    pub tokens: TokenBreakdown,
    pub reported_cost: Option<f64>,
    pub dedup_id: Option<String>,
}

/// Per-model pricing rates in USD per token
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelPricing {
    pub input_rate: f64,
    pub output_rate: f64,
    pub cache_read_rate: f64,
    pub cache_write_rate: f64,
}

/// Aggregated cost and token totals for a single day
#[derive(Clone, Debug)]
pub struct DailyAggregate {
    pub date: chrono::NaiveDate,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub by_provider: Vec<ProviderMetrics>,
}

impl Default for DailyAggregate {
    fn default() -> Self {
        Self {
            date: chrono::NaiveDate::default(),
            total_tokens: 0,
            cost_usd: 0.0,
            by_provider: Provider::ALL
                .iter()
                .map(|_| ProviderMetrics::default())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProviderMetrics {
    pub cost_usd: f64,
    pub total_tokens: u64,
}

/// Aggregated usage for a model
#[derive(Clone, Debug)]
pub struct ModelAggregate {
    pub provider: Provider,
    pub model_name: String,
    pub cost_usd: f64,
    pub total_tokens: u64,
    pub cost_fraction: f64,
}

/// Time window for filtering historical data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeWindow {
    Last7Days,
    Last30Days,
    Last90Days,
    CurrentMonth,
    PreviousMonth,
}

impl TimeWindow {
    /// Menu order: rolling windows first, then the two calendar-month presets.
    pub const ALL: [TimeWindow; 5] = [
        TimeWindow::Last7Days,
        TimeWindow::Last30Days,
        TimeWindow::Last90Days,
        TimeWindow::CurrentMonth,
        TimeWindow::PreviousMonth,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            TimeWindow::Last7Days => "Last 7 days",
            TimeWindow::Last30Days => "Last 30 days",
            TimeWindow::Last90Days => "Last 90 days",
            TimeWindow::CurrentMonth => "This month",
            TimeWindow::PreviousMonth => "Last month",
        }
    }

    pub fn date_range(&self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        use chrono::Days;
        match self {
            TimeWindow::Last7Days => (today.checked_sub_days(Days::new(6)).unwrap_or(today), today),
            TimeWindow::Last30Days => (
                today.checked_sub_days(Days::new(29)).unwrap_or(today),
                today,
            ),
            TimeWindow::Last90Days => (
                today.checked_sub_days(Days::new(89)).unwrap_or(today),
                today,
            ),
            TimeWindow::CurrentMonth => {
                let start =
                    NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
                (start, today)
            }
            TimeWindow::PreviousMonth => {
                let (year, month) = if today.month() == 1 {
                    (today.year() - 1, 12)
                } else {
                    (today.year(), today.month() - 1)
                };
                let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(today);
                let end = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                    .unwrap_or(today)
                    .checked_sub_days(Days::new(1))
                    .unwrap_or(today);
                (start, end)
            }
        }
    }
}

/// How the usage series is bucketed for the chart.
///
/// Granularity is a pure view transform over [`UsageSnapshot::daily`] — it
/// never changes which events are counted, only how many of them share a bar.
/// That is what lets the Daily/Monthly switch re-render without a rescan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Daily,
    Monthly,
}

impl Granularity {
    pub const ALL: [Granularity; 2] = [Granularity::Daily, Granularity::Monthly];

    pub fn label(&self) -> &'static str {
        match self {
            Granularity::Daily => "Daily",
            Granularity::Monthly => "Monthly",
        }
    }

    /// Singular noun for one bucket, used in the caption that spells out the
    /// range → aggregation relationship ("one bar per day").
    pub fn bucket_noun(&self) -> &'static str {
        match self {
            Granularity::Daily => "day",
            Granularity::Monthly => "month",
        }
    }

    /// Roll the snapshot's daily series into the buckets this granularity
    /// draws. `Daily` is a straight 1:1 mapping; `Monthly` sums each calendar
    /// month, keyed by its first day.
    pub fn bucket(&self, daily: &[DailyAggregate]) -> Vec<PeriodBucket> {
        match self {
            Granularity::Daily => daily
                .iter()
                .map(|d| PeriodBucket {
                    start: d.date,
                    total_tokens: d.total_tokens,
                    cost_usd: d.cost_usd,
                    by_provider: d.by_provider.clone(),
                })
                .collect(),
            Granularity::Monthly => {
                let mut months: Vec<PeriodBucket> = Vec::new();
                for day in daily {
                    let month_start = NaiveDate::from_ymd_opt(day.date.year(), day.date.month(), 1)
                        .unwrap_or(day.date);

                    // `daily` arrives in ascending date order, so the bucket a
                    // day belongs to is always the one we most recently opened.
                    if months.last().map(|m| m.start) != Some(month_start) {
                        months.push(PeriodBucket {
                            start: month_start,
                            total_tokens: 0,
                            cost_usd: 0.0,
                            by_provider: Provider::ALL
                                .iter()
                                .map(|_| ProviderMetrics::default())
                                .collect(),
                        });
                    }

                    let bucket = months.last_mut().unwrap();
                    bucket.total_tokens += day.total_tokens;
                    bucket.cost_usd += day.cost_usd;
                    for (slot, metrics) in bucket.by_provider.iter_mut().zip(&day.by_provider) {
                        slot.total_tokens += metrics.total_tokens;
                        slot.cost_usd += metrics.cost_usd;
                    }
                }
                months
            }
        }
    }
}

/// One bar in the usage chart: a single day or a whole calendar month,
/// depending on the active [`Granularity`].
#[derive(Clone, Debug)]
pub struct PeriodBucket {
    /// First day covered by this bucket — the day itself when daily.
    pub start: NaiveDate,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub by_provider: Vec<ProviderMetrics>,
}

/// Which quantity the usage view is measured in.
///
/// Cost and tokens are the same events measured two ways: cost is what the
/// usage was billed at, tokens is how much work it stands for. A cheap model
/// can top the token ranking while barely showing in the cost one, so the
/// metric is a lens over data already on screen: every panel reads this one
/// value, and switching it re-renders the page without touching the scanner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageMetric {
    Cost,
    Tokens,
}

impl UsageMetric {
    pub const ALL: [UsageMetric; 2] = [UsageMetric::Cost, UsageMetric::Tokens];

    pub fn label(&self) -> &'static str {
        match self {
            UsageMetric::Cost => "Cost",
            UsageMetric::Tokens => "Tokens",
        }
    }

    /// Headline wording, for the stat card that leads the page.
    pub fn summary_label(&self) -> &'static str {
        match self {
            UsageMetric::Cost => "Total Cost",
            UsageMetric::Tokens => "Total Tokens",
        }
    }

    /// The metric this one is not. Every panel that leads with the selected
    /// metric quotes this one underneath, so switching never hides a number —
    /// it only decides which of the two is doing the ranking.
    pub fn other(&self) -> Self {
        match self {
            UsageMetric::Cost => UsageMetric::Tokens,
            UsageMetric::Tokens => UsageMetric::Cost,
        }
    }

    /// One provider's share of a chart bucket. Tokens widen to `f64` so a
    /// bar's segments, its total and the axis all divide alike.
    pub fn of_period(&self, metrics: &ProviderMetrics) -> f64 {
        match self {
            UsageMetric::Cost => metrics.cost_usd,
            UsageMetric::Tokens => metrics.total_tokens as f64,
        }
    }

    pub fn of_provider(&self, summary: &ProviderSummary) -> f64 {
        match self {
            UsageMetric::Cost => summary.cost_usd,
            UsageMetric::Tokens => summary.total_tokens as f64,
        }
    }

    /// A provider's share of the whole in this metric's terms — what its share
    /// bar is drawn from. The two fractions differ sharply when a provider is
    /// cheap per token, which is exactly what the switch is there to show.
    pub fn share_of(&self, summary: &ProviderSummary) -> f64 {
        match self {
            UsageMetric::Cost => summary.cost_fraction,
            UsageMetric::Tokens => summary.token_fraction,
        }
    }

    pub fn of_project(&self, project: &ProjectUsage) -> f64 {
        match self {
            UsageMetric::Cost => project.cost_usd,
            UsageMetric::Tokens => project.total_tokens as f64,
        }
    }

    /// A project's share of the range in this metric's terms — what ranks the
    /// project menu, so switching the metric reorders it the same way it
    /// reorders the provider and model lists.
    pub fn share_of_project(&self, project: &ProjectUsage) -> f64 {
        match self {
            UsageMetric::Cost => project.cost_fraction,
            UsageMetric::Tokens => project.token_fraction,
        }
    }

    pub fn of_model(&self, model: &ModelAggregate) -> f64 {
        match self {
            UsageMetric::Cost => model.cost_usd,
            UsageMetric::Tokens => model.total_tokens as f64,
        }
    }

    pub fn of_snapshot(&self, snapshot: &UsageSnapshot) -> f64 {
        match self {
            UsageMetric::Cost => snapshot.cost_usd,
            UsageMetric::Tokens => snapshot.total_tokens as f64,
        }
    }
}

/// Whether a monthly rollup of this range would yield more than one bar.
///
/// A range that sits inside one calendar month collapses to a single bar under
/// Monthly, which says less than the daily series it replaces — so the switch
/// offers Monthly only when this holds.
pub fn spans_multiple_months(start: NaiveDate, end: NaiveDate) -> bool {
    (start.year(), start.month()) != (end.year(), end.month())
}

/// Shown for events whose provider does not record a working directory.
pub const UNKNOWN_PROJECT_LABEL: &str = "Unknown project";

/// The name a project path goes by on screen: its last component, which is
/// what the directory is called and what the user thinks of the project as.
///
/// Paths that share a leaf ("zlp/crm-platform-mf" and "old/crm-platform-mf")
/// therefore read alike — the full path rides along as a tooltip wherever this
/// is shown, so the short form never has to carry the disambiguation.
pub fn project_label(path: &str) -> &str {
    if path.is_empty() {
        return UNKNOWN_PROJECT_LABEL;
    }
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
}

/// The full snapshot of historical usage data
#[derive(Clone, Debug)]
pub struct UsageSnapshot {
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
    pub tokens: TokenBreakdown,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub cache_savings_usd: f64,
    pub event_count: u64,
    pub session_count: u64,
    pub by_provider: Vec<ProviderSummary>,
    pub by_model: Vec<ModelAggregate>,
    pub daily: Vec<DailyAggregate>,
    /// One entry per project the range touched, each carrying the whole page's
    /// worth of data narrowed to that project. Ordered by cost, descending.
    ///
    /// Holding a full view per project is what makes the project filter a pure
    /// view transform, like [`Granularity`]: selecting one swaps which
    /// snapshot the page draws rather than costing a rescan.
    pub by_project: Vec<ProjectUsage>,
    pub scan_time_ms: u64,
}

impl UsageSnapshot {
    /// The view of this snapshot narrowed to one project, or `None` when the
    /// range holds nothing for that path — which is what a filter left over
    /// from a previous range looks like, and why callers fall back to the
    /// unfiltered snapshot rather than showing an empty page.
    pub fn project(&self, path: &str) -> Option<&ProjectUsage> {
        self.by_project.iter().find(|p| p.path == path)
    }
}

/// One project's slice of a snapshot.
///
/// `view` is a snapshot in its own right — same dates, same shape, only the
/// events from this project — so every panel on the page reads it unchanged.
/// Its own `by_project` is empty: the split has already happened by then.
#[derive(Clone, Debug)]
pub struct ProjectUsage {
    /// Absolute path of the project directory; empty for unattributed usage.
    pub path: String,
    pub cost_usd: f64,
    pub total_tokens: u64,
    pub cost_fraction: f64,
    pub token_fraction: f64,
    pub view: UsageSnapshot,
}

impl ProjectUsage {
    pub fn label(&self) -> &str {
        project_label(&self.path)
    }
}

#[derive(Clone, Debug)]
pub struct ProviderSummary {
    pub provider: Provider,
    pub cost_usd: f64,
    pub total_tokens: u64,
    pub cost_fraction: f64,
    pub token_fraction: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date: &str, claude: f64, codex: f64) -> DailyAggregate {
        let mut agg = DailyAggregate {
            date: date.parse().unwrap(),
            ..Default::default()
        };
        agg.by_provider[Provider::Claude.index()] = ProviderMetrics {
            cost_usd: claude,
            total_tokens: (claude * 1000.0) as u64,
        };
        agg.by_provider[Provider::Codex.index()] = ProviderMetrics {
            cost_usd: codex,
            total_tokens: (codex * 1000.0) as u64,
        };
        agg.cost_usd = claude + codex;
        agg.total_tokens = ((claude + codex) * 1000.0) as u64;
        agg
    }

    #[test]
    fn daily_bucketing_is_one_bar_per_day() {
        let daily = vec![day("2026-07-30", 1.0, 0.5), day("2026-07-31", 2.0, 0.0)];
        let buckets = Granularity::Daily.bucket(&daily);

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].start, daily[0].date);
        assert_eq!(buckets[1].cost_usd, 2.0);
    }

    #[test]
    fn monthly_bucketing_sums_each_calendar_month() {
        let daily = vec![
            day("2026-07-30", 1.0, 0.5),
            day("2026-07-31", 2.0, 0.0),
            day("2026-08-01", 4.0, 0.25),
        ];
        let buckets = Granularity::Monthly.bucket(&daily);

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].cost_usd, 3.5);
        assert_eq!(buckets[1].cost_usd, 4.25);

        // Totals survive the rollup intact, per provider as well as overall.
        assert_eq!(
            buckets[0].by_provider[Provider::Claude.index()].cost_usd,
            3.0
        );
        assert_eq!(
            buckets[0].by_provider[Provider::Codex.index()].cost_usd,
            0.5
        );
        assert_eq!(buckets[0].total_tokens, 3500);

        let daily_total: f64 = daily.iter().map(|d| d.cost_usd).sum();
        let monthly_total: f64 = buckets.iter().map(|b| b.cost_usd).sum();
        assert!((daily_total - monthly_total).abs() < f64::EPSILON);
    }

    #[test]
    fn monthly_buckets_are_keyed_by_the_first_of_their_month() {
        // A range starting mid-month still labels its bucket from day one, so
        // the axis reads "Jul" rather than "Jul 30".
        let buckets = Granularity::Monthly.bucket(&[day("2026-07-30", 1.0, 0.0)]);
        assert_eq!(buckets[0].start, "2026-07-01".parse::<NaiveDate>().unwrap());
    }

    #[test]
    fn monthly_bucketing_keeps_months_in_order_across_a_year_boundary() {
        let daily = vec![day("2025-12-31", 1.0, 0.0), day("2026-01-01", 2.0, 0.0)];
        let buckets = Granularity::Monthly.bucket(&daily);

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].start.year(), 2025);
        assert_eq!(buckets[1].start.year(), 2026);
    }

    #[test]
    fn empty_input_produces_no_buckets() {
        assert!(Granularity::Monthly.bucket(&[]).is_empty());
        assert!(Granularity::Daily.bucket(&[]).is_empty());
    }

    #[test]
    fn a_metric_reads_the_same_bucket_in_its_own_unit() {
        let daily = vec![day("2026-08-01", 2.0, 1.0)];
        let bucket = &Granularity::Daily.bucket(&daily)[0];
        let claude = &bucket.by_provider[Provider::Claude.index()];

        assert_eq!(UsageMetric::Cost.of_period(claude), 2.0);
        assert_eq!(UsageMetric::Tokens.of_period(claude), 2000.0);
    }

    #[test]
    fn a_provider_ranks_differently_under_each_metric() {
        // A provider can be a rounding error on the bill and still be most of
        // the work — the whole reason the page can be read either way.
        let cheap = ProviderSummary {
            provider: Provider::Codex,
            cost_usd: 1.0,
            total_tokens: 900,
            cost_fraction: 0.1,
            token_fraction: 0.9,
        };

        assert_eq!(UsageMetric::Cost.of_provider(&cheap), 1.0);
        assert_eq!(UsageMetric::Tokens.of_provider(&cheap), 900.0);
        assert_eq!(UsageMetric::Cost.share_of(&cheap), 0.1);
        assert_eq!(UsageMetric::Tokens.share_of(&cheap), 0.9);
        assert_eq!(UsageMetric::Cost.other(), UsageMetric::Tokens);
    }

    /// A snapshot with one project per (path, cost) pair, shares filled in.
    fn snapshot_with_projects(projects: &[(&str, f64, u64)]) -> UsageSnapshot {
        let cost_total: f64 = projects.iter().map(|(_, cost, _)| cost).sum();
        let token_total: u64 = projects.iter().map(|(_, _, tokens)| tokens).sum();
        let mut snapshot = empty_snapshot();
        snapshot.cost_usd = cost_total;
        snapshot.total_tokens = token_total;
        snapshot.by_project = projects
            .iter()
            .map(|(path, cost, tokens)| ProjectUsage {
                path: (*path).to_owned(),
                cost_usd: *cost,
                total_tokens: *tokens,
                cost_fraction: if cost_total > 0.0 { cost / cost_total } else { 0.0 },
                token_fraction: if token_total > 0 {
                    *tokens as f64 / token_total as f64
                } else {
                    0.0
                },
                view: UsageSnapshot {
                    cost_usd: *cost,
                    total_tokens: *tokens,
                    ..empty_snapshot()
                },
            })
            .collect();
        snapshot
    }

    fn empty_snapshot() -> UsageSnapshot {
        UsageSnapshot {
            start_date: "2026-08-01".parse().unwrap(),
            end_date: "2026-08-31".parse().unwrap(),
            tokens: TokenBreakdown::default(),
            total_tokens: 0,
            cost_usd: 0.0,
            cache_savings_usd: 0.0,
            event_count: 0,
            session_count: 0,
            by_provider: Vec::new(),
            by_model: Vec::new(),
            daily: Vec::new(),
            by_project: Vec::new(),
            scan_time_ms: 0,
        }
    }

    #[test]
    fn a_project_goes_by_the_last_component_of_its_path() {
        assert_eq!(project_label("/Users/me/Developer/Snapzy"), "Snapzy");
        // A trailing slash is not a nameless project.
        assert_eq!(project_label("/Users/me/Developer/usage/"), "usage");
        // Usage the provider could not attribute still has somewhere to go.
        assert_eq!(project_label(""), UNKNOWN_PROJECT_LABEL);
    }

    #[test]
    fn selecting_a_project_finds_the_view_scanned_for_it() {
        let snapshot = snapshot_with_projects(&[("/w/a", 3.0, 300), ("/w/b", 1.0, 100)]);

        let a = snapshot.project("/w/a").unwrap();
        assert_eq!(a.label(), "a");
        assert_eq!(a.view.cost_usd, 3.0);
        // A path the range holds nothing for — a filter left over from another
        // range — reports absent rather than empty.
        assert!(snapshot.project("/w/never").is_none());
    }

    #[test]
    fn a_project_ranks_differently_under_each_metric() {
        // The same inversion the provider list shows: cheap-but-busy work is a
        // rounding error on the bill and most of the tokens.
        let snapshot = snapshot_with_projects(&[("/w/pricey", 9.0, 100), ("/w/busy", 1.0, 900)]);
        let pricey = snapshot.project("/w/pricey").unwrap();
        let busy = snapshot.project("/w/busy").unwrap();

        assert!(UsageMetric::Cost.of_project(pricey) > UsageMetric::Cost.of_project(busy));
        assert!(UsageMetric::Tokens.of_project(busy) > UsageMetric::Tokens.of_project(pricey));
        assert_eq!(UsageMetric::Cost.share_of_project(busy), 0.1);
        assert_eq!(UsageMetric::Tokens.share_of_project(busy), 0.9);
    }

    #[test]
    fn project_shares_account_for_the_whole_range() {
        let snapshot = snapshot_with_projects(&[("/w/a", 3.0, 300), ("/w/b", 1.0, 100)]);
        let cost: f64 = snapshot
            .by_project
            .iter()
            .map(|p| p.cost_fraction)
            .sum();
        assert!((cost - 1.0).abs() < 1e-9);
    }

    #[test]
    fn monthly_is_offered_only_for_ranges_crossing_a_month() {
        let within =
            |a: &str, b: &str| spans_multiple_months(a.parse().unwrap(), b.parse().unwrap());

        // A full calendar month is still one bar — not worth the switch.
        assert!(!within("2026-08-01", "2026-08-31"));
        assert!(!within("2026-08-12", "2026-08-18"));
        assert!(within("2026-07-30", "2026-08-18"));
        // Same month number, different year.
        assert!(within("2025-08-18", "2026-08-18"));
    }
}
