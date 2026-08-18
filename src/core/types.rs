use chrono::{Datelike, NaiveDate};

/// Represents the different AI coding agent providers we track
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Claude, Provider::Codex];

    pub fn label(&self) -> &'static str {
        match self {
            Provider::Claude => "Claude",
            Provider::Codex => "Codex",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Provider::Claude => 0,
            Provider::Codex => 1,
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
    pub working_dir: String,
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
#[derive(Clone, Debug, Default)]
pub struct DailyAggregate {
    pub date: chrono::NaiveDate,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub by_provider: [ProviderMetrics; 2],
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
    pub fn label(&self) -> &'static str {
        match self {
            TimeWindow::Last7Days => "Last 7 Days",
            TimeWindow::Last30Days => "Last 30 Days",
            TimeWindow::Last90Days => "Last 90 Days",
            TimeWindow::CurrentMonth => "Current Month",
            TimeWindow::PreviousMonth => "Previous Month",
        }
    }

    pub fn date_range(&self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        use chrono::Days;
        match self {
            TimeWindow::Last7Days => (today.checked_sub_days(Days::new(6)).unwrap_or(today), today),
            TimeWindow::Last30Days => {
                (today.checked_sub_days(Days::new(29)).unwrap_or(today), today)
            }
            TimeWindow::Last90Days => {
                (today.checked_sub_days(Days::new(89)).unwrap_or(today), today)
            }
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

/// The full snapshot of historical usage data
#[derive(Clone, Debug)]
pub struct UsageSnapshot {
    pub window: TimeWindow,
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
    pub scan_time_ms: u64,
}

#[derive(Clone, Debug)]
pub struct ProviderSummary {
    pub provider: Provider,
    pub cost_usd: f64,
    pub total_tokens: u64,
    pub cost_fraction: f64,
    pub token_fraction: f64,
}
