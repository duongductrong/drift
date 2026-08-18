use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde_json::Value;

use crate::core::types::{
    DailyAggregate, ModelAggregate, Provider, ProviderMetrics, ProviderSummary,
    TimeWindow, TokenBreakdown, UsageEvent, UsageSnapshot,
};
use crate::core::pricing::{compute_cost, compute_cache_savings, PricingTable};

// ---------------------------------------------------------------------------
// Transcript discovery
// ---------------------------------------------------------------------------

pub fn transcript_root(provider: Provider) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    match provider {
        Provider::Claude => Some(home.join(".claude").join("projects")),
        Provider::Codex => Some(home.join(".codex").join("sessions")),
    }
}

/// Lists `.jsonl` transcripts under `root` modified at or after `since_ms`.
/// Uses a 36-hour mtime slack so sessions whose last write lands just before
/// the window start are not dropped.
const MTIME_SLACK_MS: i64 = 36 * 3600 * 1000;

pub fn discover_transcripts(root: &Path, since_ms: i64) -> Vec<PathBuf> {
    let cutoff = since_ms - MTIME_SLACK_MS;
    let mut files = Vec::new();
    if !root.exists() {
        return files;
    }

    let mut dirs_to_visit = vec![root.to_path_buf()];

    while let Some(current_dir) = dirs_to_visit.pop() {
        if let Ok(entries) = fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs_to_visit.push(path);
                } else if path.is_file()
                    && path.extension().is_some_and(|e| e == "jsonl")
                {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(mtime) = metadata.modified() {
                            let mtime_ms = mtime
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                                as i64;
                            if mtime_ms >= cutoff {
                                files.push(path);
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Positive finite number as a token count; anything else is zero.
/// Accepts both integer and float JSON values (some providers use floats).
fn int(value: Option<&Value>) -> u64 {
    value
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| v.trunc() as u64)
        .unwrap_or(0)
}

/// Parse an RFC 3339 timestamp string to epoch milliseconds.
fn parse_timestamp_ms(value: Option<&Value>) -> Option<i64> {
    // Try string (RFC 3339) first — Claude uses "2026-08-08T15:18:37.487Z"
    if let Some(text) = value.and_then(Value::as_str) {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text) {
            return Some(dt.timestamp_millis());
        }
    }
    // Fall back to integer milliseconds for backward compatibility
    value.and_then(Value::as_i64)
}

/// Quick substring gate applied before JSON parsing. Transcripts are mostly
/// tool output; skipping irrelevant lines before serde_json is ~10x faster.
fn might_carry_usage(line: &str, provider: Provider) -> bool {
    match provider {
        Provider::Claude => line.contains("\"usage\""),
        Provider::Codex => line.contains("\"token_count\""),
    }
}

// ---------------------------------------------------------------------------
// Claude parser
// ---------------------------------------------------------------------------

/// Parses one line of a Claude Code transcript.
///
/// The CLI writes one record per assistant *content block*, and every one of
/// those records repeats the same complete `usage` object for the parent
/// message. Summing them overcounts severely, so callers must drop repeats by
/// `dedupe_key` and keep the first.
pub fn parse_claude_line(line: &str) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(line).ok()?;

    if v.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let message = v.get("message")?.as_object()?;
    let usage = message.get("usage")?.as_object()?;
    let timestamp_ms = parse_timestamp_ms(v.get("timestamp"))?;
    let model_name = message.get("model").and_then(Value::as_str)?;
    if model_name.is_empty() {
        return None;
    }

    // Dedup key: message_id:request_id — matches Waku/ccusage approach.
    // Claude emits one record per content block sharing the parent message's
    // cumulative usage; only the first wins.
    let message_id = message.get("id").and_then(Value::as_str);
    let request_id = v.get("requestId").and_then(Value::as_str);
    let dedup_id = (message_id.is_some() || request_id.is_some()).then(|| {
        format!(
            "{}:{}",
            message_id.unwrap_or_default(),
            request_id.unwrap_or_default()
        )
    });

    let tokens = TokenBreakdown {
        fresh_input: int(usage.get("input_tokens")),
        cached_input: int(usage.get("cache_read_input_tokens")),
        cache_write: int(usage.get("cache_creation_input_tokens")),
        output: int(usage.get("output_tokens")),
        // Anthropic folds thinking tokens into output without a breakout.
        reasoning: 0,
    };

    // Claude reports costUSD on assistant records since late 2024.
    let reported_cost = v
        .get("costUSD")
        .and_then(Value::as_f64)
        .filter(|c| c.is_finite());

    Some(UsageEvent {
        provider: Provider::Claude,
        timestamp_ms,
        model_name: model_name.to_owned(),
        session_key: v
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        working_dir: v
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        tokens,
        reported_cost,
        dedup_id,
    })
}

// ---------------------------------------------------------------------------
// Codex parser (stateful per-file)
// ---------------------------------------------------------------------------

/// Rolling state for a single Codex rollout file. `token_count` events carry
/// no model, so the model is carried forward from the most recent
/// `turn_context`; sessions that switch models mid-run attribute correctly
/// from the switch onward.
#[derive(Default)]
pub struct CodexParseState {
    pub current_model: String,
    pub current_session: String,
    pub current_dir: String,
    /// Serialised `last_token_usage` for consecutive duplicate suppression.
    pub last_usage_signature: Option<String>,
}

/// Feeds one line of a Codex rollout into `state`, returning a record when the
/// line was a usage event. Consecutive duplicate events are suppressed.
pub fn parse_codex_line(line: &str, state: &mut CodexParseState) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(line).ok()?;
    let payload = v.get("payload")?.as_object()?;

    match v.get("type").and_then(Value::as_str) {
        Some("session_meta") => {
            // Extract session ID from session_meta — fixes session counting.
            if let Some(id) = payload
                .get("id")
                .or_else(|| payload.get("session_id"))
                .and_then(Value::as_str)
            {
                state.current_session = id.to_owned();
            }
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                state.current_dir = cwd.to_owned();
            }
            return None;
        }
        Some("turn_context") => {
            if let Some(m) = payload.get("model").and_then(Value::as_str) {
                state.current_model = m.to_owned();
            }
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                state.current_dir = cwd.to_owned();
            }
            return None;
        }
        _ => {}
    }

    // Only token_count events in event_msg carry usage.
    if payload.get("type").and_then(Value::as_str) != Some("token_count") {
        return None;
    }

    let last = payload.get("info")?.get("last_token_usage")?.as_object()?;
    let timestamp_ms = parse_timestamp_ms(v.get("timestamp"))?;

    // Must have a model before we can attribute tokens.
    if state.current_model.is_empty() {
        return None;
    }

    // Codex re-emits identical token_count on stream boundaries.
    // Suppress consecutive duplicates to avoid double-counting.
    let signature = serde_json::to_string(last).ok()?;
    if state.last_usage_signature.as_deref() == Some(signature.as_str()) {
        return None;
    }
    state.last_usage_signature = Some(signature);

    // Codex reports `input_tokens` INCLUSIVE of cached portion.
    // Subtract cached + cache_write to get true uncached input.
    let raw_input = int(last.get("input_tokens"));
    let cached_input = int(last.get("cached_input_tokens"));
    let cache_write = int(last.get("cache_write_input_tokens"));
    let output = int(last.get("output_tokens"));
    let reasoning = int(last.get("reasoning_output_tokens")).min(output);

    let tokens = TokenBreakdown {
        fresh_input: raw_input.saturating_sub(cached_input + cache_write),
        cached_input,
        cache_write,
        output,
        reasoning,
    };

    if tokens.total() == 0 {
        return None;
    }

    Some(UsageEvent {
        provider: Provider::Codex,
        timestamp_ms,
        model_name: state.current_model.clone(),
        session_key: if state.current_session.is_empty() {
            "unknown".to_owned()
        } else {
            state.current_session.clone()
        },
        working_dir: state.current_dir.clone(),
        tokens,
        reported_cost: None,
        // Codex rollout files are per-session; no cross-file dedup needed.
        dedup_id: None,
    })
}

// ---------------------------------------------------------------------------
// File reader with in-file dedup and substring pre-filter
// ---------------------------------------------------------------------------

/// Streams one transcript and returns usage records, already de-duplicated
/// within the file. Returns None on read errors (so they are not cached as
/// empty).
fn read_transcript_records(path: &Path, provider: Provider) -> Option<Vec<UsageEvent>> {
    let file = fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();
    let mut codex_state = CodexParseState::default();
    let mut seen_in_file: HashSet<String> = HashSet::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => return None,
        }

        match provider {
            Provider::Claude => {
                if !might_carry_usage(&line, provider) {
                    continue;
                }
                if let Some(record) = parse_claude_line(&line) {
                    // Every assistant content block repeats the parent
                    // message's usage; the first record wins.
                    if let Some(key) = &record.dedup_id {
                        if !seen_in_file.insert(key.clone()) {
                            continue;
                        }
                    }
                    records.push(record);
                }
            }
            Provider::Codex => {
                // Codex carries model on turn_context lines that hold no
                // usage, so those still pass through to keep attribution.
                if !might_carry_usage(&line, provider)
                    && !line.contains("\"turn_context\"")
                    && !line.contains("\"session_meta\"")
                {
                    continue;
                }
                if let Some(record) = parse_codex_line(&line, &mut codex_state) {
                    records.push(record);
                }
            }
        }
    }
    Some(records)
}

// ---------------------------------------------------------------------------
// Scan + Aggregate
// ---------------------------------------------------------------------------

pub fn scan_all(window: TimeWindow, pricing: &PricingTable) -> UsageSnapshot {
    let start_time = std::time::Instant::now();
    let today = Local::now().date_naive();
    let (start_date, end_date) = window.date_range(today);

    let start_ts_ms = start_date
        .and_hms_opt(0, 0, 0)
        .and_then(|midnight| Local.from_local_datetime(&midnight).earliest())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| Utc::now().timestamp_millis() - 7 * 86_400_000);

    let end_ts_ms = end_date
        .and_hms_opt(23, 59, 59)
        .and_then(|end| Local.from_local_datetime(&end).earliest())
        .map(|dt| dt.timestamp_millis() + 999) // include full last second
        .unwrap_or(i64::MAX);

    // ── Scan files ─────────────────────────────────────────────────
    let mut all_events = Vec::new();

    for provider in Provider::ALL {
        if let Some(root) = transcript_root(provider) {
            let files = discover_transcripts(&root, start_ts_ms);
            for file in files {
                if let Some(records) = read_transcript_records(&file, provider) {
                    all_events.extend(records);
                }
            }
        }
    }

    // ── Cross-file dedup + time filter ─────────────────────────────
    let mut unique_events = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for event in all_events {
        // Filter to window bounds (both sides)
        if event.timestamp_ms < start_ts_ms || event.timestamp_ms > end_ts_ms {
            continue;
        }
        if let Some(id) = &event.dedup_id {
            if !seen_ids.insert(id.clone()) {
                continue;
            }
        }
        unique_events.push(event);
    }

    // ── Aggregate ──────────────────────────────────────────────────
    let mut total_tokens: u64 = 0;
    let mut global_tokens = TokenBreakdown::default();
    let mut total_cost = 0.0;
    let mut total_cache_savings = 0.0;
    let mut session_keys: HashSet<String> = HashSet::new();

    let mut daily_map: HashMap<NaiveDate, DailyAggregate> = HashMap::new();
    let mut model_map: HashMap<(Provider, String), ModelAggregate> = HashMap::new();

    for event in &unique_events {
        // Convert to local timezone date for day grouping
        let date = DateTime::from_timestamp_millis(event.timestamp_ms)
            .unwrap_or_default()
            .with_timezone(&Local)
            .date_naive();

        let cost = if let Some(reported) = event.reported_cost {
            reported
        } else if let Some(rate) = pricing.get_rate(&event.model_name) {
            compute_cost(&event.tokens, rate)
        } else {
            0.0
        };

        let cache_savings = pricing
            .get_rate(&event.model_name)
            .map(|rate| compute_cache_savings(&event.tokens, rate))
            .unwrap_or(0.0);

        let event_total_tokens = event.tokens.total();

        total_tokens += event_total_tokens;
        global_tokens.fresh_input += event.tokens.fresh_input;
        global_tokens.cached_input += event.tokens.cached_input;
        global_tokens.cache_write += event.tokens.cache_write;
        global_tokens.output += event.tokens.output;
        global_tokens.reasoning += event.tokens.reasoning;
        total_cost += cost;
        total_cache_savings += cache_savings;
        session_keys.insert(event.session_key.clone());

        let daily = daily_map.entry(date).or_insert_with(|| DailyAggregate {
            date,
            ..Default::default()
        });
        daily.total_tokens += event_total_tokens;
        daily.cost_usd += cost;

        let provider_idx = event.provider.index();
        daily.by_provider[provider_idx].total_tokens += event_total_tokens;
        daily.by_provider[provider_idx].cost_usd += cost;

        let model_agg = model_map
            .entry((event.provider, event.model_name.clone()))
            .or_insert_with(|| ModelAggregate {
                provider: event.provider,
                model_name: event.model_name.clone(),
                cost_usd: 0.0,
                total_tokens: 0,
                cost_fraction: 0.0,
            });
        model_agg.total_tokens += event_total_tokens;
        model_agg.cost_usd += cost;
    }

    // ── Fill in zero-activity days so the chart is continuous ───────
    let daily: Vec<DailyAggregate> = {
        let mut cursor = start_date;
        let mut filled = Vec::new();
        while cursor <= end_date {
            let agg = daily_map.remove(&cursor).unwrap_or(DailyAggregate {
                date: cursor,
                total_tokens: 0,
                cost_usd: 0.0,
                by_provider: [ProviderMetrics::default(), ProviderMetrics::default()],
            });
            filled.push(agg);
            cursor = cursor
                .checked_add_days(chrono::Days::new(1))
                .unwrap_or(end_date);
            if cursor == end_date && filled.last().map(|d| d.date) == Some(end_date) {
                break;
            }
        }
        filled
    };

    let mut by_model: Vec<ModelAggregate> = model_map
        .into_values()
        .map(|mut m| {
            if total_cost > 0.0 {
                m.cost_fraction = m.cost_usd / total_cost;
            }
            m
        })
        .collect();
    by_model.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.total_tokens.cmp(&a.total_tokens))
    });

    let mut provider_summary: HashMap<Provider, ProviderSummary> = HashMap::new();
    for m in &by_model {
        let entry = provider_summary.entry(m.provider).or_insert_with(|| {
            ProviderSummary {
                provider: m.provider,
                cost_usd: 0.0,
                total_tokens: 0,
                cost_fraction: 0.0,
                token_fraction: 0.0,
            }
        });
        entry.cost_usd += m.cost_usd;
        entry.total_tokens += m.total_tokens;
    }

    let mut by_provider: Vec<ProviderSummary> = provider_summary
        .into_values()
        .map(|mut p| {
            if total_cost > 0.0 {
                p.cost_fraction = p.cost_usd / total_cost;
            }
            if total_tokens > 0 {
                p.token_fraction = p.total_tokens as f64 / total_tokens as f64;
            }
            p
        })
        .collect();
    by_provider.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    UsageSnapshot {
        window,
        start_date,
        end_date,
        tokens: global_tokens,
        total_tokens,
        cost_usd: total_cost,
        cache_savings_usd: total_cache_savings,
        event_count: unique_events.len() as u64,
        session_count: session_keys.len() as u64,
        by_provider,
        by_model,
        daily,
        scan_time_ms: start_time.elapsed().as_millis() as u64,
    }
}
