use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde_json::Value;

use crate::core::types::{
    DailyAggregate, ModelAggregate, ProjectUsage, Provider, ProviderSummary,
    TimeWindow, TokenBreakdown, UsageEvent, UsageSnapshot,
};
use crate::core::pricing::{compute_cost, compute_cache_savings, PricingTable};

// ---------------------------------------------------------------------------
// Transcript discovery
// ---------------------------------------------------------------------------

/// Where a provider keeps its usage records on this machine.
///
/// The single place these paths are written down: the scanners below read them
/// from here, and the settings dialog shows them so the user can see what each
/// provider row actually reads.
pub fn data_source(provider: Provider) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(match provider {
        Provider::Claude => home.join(".claude").join("projects"),
        Provider::Codex => home.join(".codex").join("sessions"),
        Provider::Kimi => home.join(".kimi-code").join("sessions"),
        Provider::OpenCode => home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db"),
        Provider::Antigravity => home.join(".gemini").join("antigravity").join("conversations"),
    })
}

/// Whether a provider is read by parsing `.jsonl` transcripts, as opposed to
/// one of the SQLite stores.
fn is_transcript_provider(provider: Provider) -> bool {
    matches!(
        provider,
        Provider::Claude | Provider::Codex | Provider::Kimi
    )
}

pub fn transcript_root(provider: Provider) -> Option<PathBuf> {
    // OpenCode and Antigravity use SQLite databases, not transcript files.
    is_transcript_provider(provider)
        .then(|| data_source(provider))
        .flatten()
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
                    && let Ok(metadata) = entry.metadata()
                    && let Ok(mtime) = metadata.modified()
                {
                    let mtime_ms = mtime
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    if mtime_ms >= cutoff {
                        files.push(path);
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
    if let Some(text) = value.and_then(Value::as_str)
        && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(text)
    {
        return Some(dt.timestamp_millis());
    }
    // Fall back to integer milliseconds for backward compatibility
    value.and_then(Value::as_i64)
}

/// Normalise a working directory into the key projects are grouped by.
///
/// Providers write the same directory in more than one shape — a `file://`
/// URI, a trailing slash — and every variant that reaches the aggregator
/// becomes a project of its own, splitting one project's cost across several
/// menu entries. Anything unrecognisable collapses to the empty string, which
/// groups as "Unknown project".
fn project_key(raw: &str) -> String {
    let trimmed = raw.trim();
    let path = trimmed
        .strip_prefix("file://")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if path.is_empty() { String::new() } else { path.to_owned() }
}

/// Quick substring gate applied before JSON parsing. Transcripts are mostly
/// tool output; skipping irrelevant lines before serde_json is ~10x faster.
fn might_carry_usage(line: &str, provider: Provider) -> bool {
    match provider {
        Provider::Claude => line.contains("\"usage\""),
        Provider::Codex => line.contains("\"token_count\""),
        Provider::Kimi => line.contains("\"step.end\""),
        _ => false,
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
        // Every record carries the directory the turn ran in, so the project
        // is read per event rather than per file — a session that moves
        // between directories attributes each turn where it happened.
        project_path: project_key(v.get("cwd").and_then(Value::as_str).unwrap_or_default()),
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
    /// Working directory of the session, from `session_meta` and refreshed by
    /// each `turn_context` — carried forward exactly like the model, so a
    /// session that changes directory attributes from the change onward.
    pub current_cwd: String,
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
                state.current_cwd = project_key(cwd);
            }
            return None;
        }
        Some("turn_context") => {
            if let Some(m) = payload.get("model").and_then(Value::as_str) {
                state.current_model = m.to_owned();
            }
            if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                state.current_cwd = project_key(cwd);
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
        project_path: state.current_cwd.clone(),
        tokens,
        reported_cost: None,
        // Codex rollout files are per-session; no cross-file dedup needed.
        dedup_id: None,
    })
}

// ---------------------------------------------------------------------------
// Kimi parser (stateful per-file, reads wire.jsonl)
// ---------------------------------------------------------------------------

/// Rolling state for a single Kimi Code wire.jsonl file. Model is carried
/// forward from `profile.bind` and `config.update` events.
#[derive(Default)]
pub struct KimiParseState {
    pub current_model: String,
    pub session_key: String,
    /// Working directory of the session. Kimi's wire log never names it, so
    /// this is filled in from the session's `state.json` before the log is
    /// read — see [`kimi_session_cwd`].
    pub project_path: String,
}

/// Feeds one line of a Kimi wire.jsonl into `state`, returning a record when
/// the line was a usage event (step.end).
pub fn parse_kimi_line(line: &str, state: &mut KimiParseState) -> Option<UsageEvent> {
    let v: Value = serde_json::from_str(line).ok()?;

    match v.get("type").and_then(Value::as_str) {
        Some("profile.bind") => {
            if let Some(m) = v.get("modelAlias").and_then(Value::as_str) {
                state.current_model = m.to_owned();
            }
            return None;
        }
        Some("config.update") => {
            if let Some(m) = v.get("modelAlias").and_then(Value::as_str) {
                state.current_model = m.to_owned();
            }
            return None;
        }
        _ => {}
    }

    // Only context.append_loop_event with step.end carry usage.
    if v.get("type").and_then(Value::as_str) != Some("context.append_loop_event") {
        return None;
    }

    let event = v.get("event")?.as_object()?;
    if event.get("type").and_then(Value::as_str) != Some("step.end") {
        return None;
    }

    let usage = event.get("usage")?.as_object()?;
    let timestamp_ms = v.get("time").and_then(Value::as_i64)?;

    if state.current_model.is_empty() {
        return None;
    }

    let fresh_input = int(usage.get("inputOther"));
    let cached_input = int(usage.get("inputCacheRead"));
    let cache_write = int(usage.get("inputCacheCreation"));
    let output = int(usage.get("output"));

    let tokens = TokenBreakdown {
        fresh_input,
        cached_input,
        cache_write,
        output,
        reasoning: 0,
    };

    if tokens.total() == 0 {
        return None;
    }

    let dedup_id = event
        .get("uuid")
        .or_else(|| event.get("messageId"))
        .and_then(Value::as_str)
        .map(|s| s.to_owned());

    Some(UsageEvent {
        provider: Provider::Kimi,
        timestamp_ms,
        model_name: state.current_model.clone(),
        session_key: state.session_key.clone(),
        project_path: state.project_path.clone(),
        tokens,
        reported_cost: None,
        dedup_id,
    })
}

/// The directory a Kimi session ran in, read from the `state.json` beside its
/// wire log.
///
/// The key moved between releases — older sessions write `workDir`, newer ones
/// `cwd` — so both are accepted; a session recorded under the old name still
/// lands on the same project as its newer siblings.
fn kimi_session_cwd(session_dir: &Path) -> String {
    let Ok(text) = fs::read_to_string(session_dir.join("state.json")) else {
        return String::new();
    };
    let Ok(state) = serde_json::from_str::<Value>(&text) else {
        return String::new();
    };
    project_key(
        state
            .get("cwd")
            .or_else(|| state.get("workDir"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
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
    let mut kimi_state = KimiParseState::default();
    let mut seen_in_file: HashSet<String> = HashSet::new();

    // For Kimi, extract the session key from the session directory name.
    if provider == Provider::Kimi {
        // path is .../session_<uuid>/agents/main/wire.jsonl
        if let Some(session_dir) = path.parent().and_then(|p| p.parent()).and_then(|p| p.parent()) {
            let session_name = session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            kimi_state.session_key = session_name.to_owned();
            kimi_state.project_path = kimi_session_cwd(session_dir);
        }
    }

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
                    if let Some(key) = &record.dedup_id
                        && !seen_in_file.insert(key.clone())
                    {
                        continue;
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
            Provider::Kimi => {
                // Kimi carries model on profile.bind and config.update lines
                // that hold no usage, so those still pass through.
                if !might_carry_usage(&line, provider)
                    && !line.contains("\"profile.bind\"")
                    && !line.contains("\"config.update\"")
                {
                    continue;
                }
                if let Some(record) = parse_kimi_line(&line, &mut kimi_state) {
                    if let Some(key) = &record.dedup_id
                        && !seen_in_file.insert(key.clone())
                    {
                        continue;
                    }
                    records.push(record);
                }
            }
            // OpenCode and Antigravity don't use transcript files.
            _ => return None,
        }
    }
    Some(records)
}

// ---------------------------------------------------------------------------
// OpenCode SQLite scanner
// ---------------------------------------------------------------------------

/// Scans the OpenCode SQLite database for usage events.
fn scan_opencode(since_ms: i64) -> Vec<UsageEvent> {
    let Some(db_path) = data_source(Provider::OpenCode) else {
        return Vec::new();
    };
    if !db_path.exists() {
        return Vec::new();
    }

    let Ok(conn) = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };

    let mut events = Vec::new();
    // The directory lives on the session, not the message, so it is joined in
    // rather than read per row: a message whose session has since been deleted
    // still counts, as unattributed usage.
    let query = "SELECT m.id, m.session_id, m.time_created, m.data, s.directory \
                 FROM message m LEFT JOIN session s ON s.id = m.session_id \
                 WHERE json_extract(m.data, '$.role') = 'assistant' \
                 AND m.time_created >= ?1";

    let Ok(mut stmt) = conn.prepare(query) else {
        return events;
    };

    let rows = stmt.query_map([since_ms], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    });

    let Ok(rows) = rows else {
        return events;
    };

    for row in rows.flatten() {
        let (msg_id, session_id, time_created, raw_data, directory) = row;
        let Ok(data) = serde_json::from_str::<Value>(&raw_data) else {
            continue;
        };

        let Some(tokens_obj) = data.get("tokens") else {
            continue;
        };

        let fresh_input = int(tokens_obj.get("input"));
        let output = int(tokens_obj.get("output"));
        let reasoning = int(tokens_obj.get("reasoning"));
        let (cached_input, cache_write) = if let Some(cache) = tokens_obj.get("cache") {
            (int(cache.get("read")), int(cache.get("write")))
        } else {
            (0, 0)
        };

        let tokens = TokenBreakdown {
            fresh_input,
            cached_input,
            cache_write,
            output,
            reasoning,
        };

        if tokens.total() == 0 {
            continue;
        }

        let provider_id = data
            .get("providerID")
            .and_then(Value::as_str)
            .unwrap_or("opencode");
        let model_id = data
            .get("modelID")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let model_name = if provider_id == "opencode" {
            model_id.to_owned()
        } else {
            format!("{}/{}", provider_id, model_id)
        };

        let reported_cost = data
            .get("cost")
            .and_then(Value::as_f64)
            .filter(|c| c.is_finite() && *c > 0.0);

        events.push(UsageEvent {
            provider: Provider::OpenCode,
            timestamp_ms: time_created,
            model_name,
            session_key: session_id,
            project_path: project_key(directory.as_deref().unwrap_or_default()),
            tokens,
            reported_cost,
            dedup_id: Some(msg_id),
        });
    }

    events
}

// ---------------------------------------------------------------------------
// Antigravity (Google AGY) SQLite + Protobuf scanner
// ---------------------------------------------------------------------------

/// Decode a protobuf varint from a byte slice, returning (value, bytes_consumed).
fn decode_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut val: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate() {
        val |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((val, i + 1));
        }
        if shift >= 64 {
            return None;
        }
    }
    None
}

/// Minimal protobuf wire-format field extractor. Returns varints and
/// length-delimited fields indexed by field number.
fn decode_pb_fields(data: &[u8]) -> HashMap<u32, Vec<PbValue>> {
    let mut fields: HashMap<u32, Vec<PbValue>> = HashMap::new();
    let mut idx = 0;
    while idx < data.len() {
        let Some((tag, consumed)) = decode_varint(&data[idx..]) else {
            break;
        };
        idx += consumed;
        let wire_type = (tag & 0x07) as u8;
        let field_num = (tag >> 3) as u32;

        match wire_type {
            0 => {
                // Varint
                let Some((val, consumed)) = decode_varint(&data[idx..]) else {
                    break;
                };
                idx += consumed;
                fields
                    .entry(field_num)
                    .or_default()
                    .push(PbValue::Varint(val));
            }
            2 => {
                // Length-delimited
                let Some((len, consumed)) = decode_varint(&data[idx..]) else {
                    break;
                };
                idx += consumed;
                let end = idx + len as usize;
                if end > data.len() {
                    break;
                }
                fields
                    .entry(field_num)
                    .or_default()
                    .push(PbValue::Bytes(data[idx..end].to_vec()));
                idx = end;
            }
            1 => {
                idx += 8;
            } // 64-bit
            5 => {
                idx += 4;
            } // 32-bit
            _ => break,
        }
    }
    fields
}

#[derive(Debug)]
enum PbValue {
    Varint(u64),
    Bytes(Vec<u8>),
}

impl PbValue {
    fn as_varint(&self) -> Option<u64> {
        match self {
            PbValue::Varint(v) => Some(*v),
            _ => None,
        }
    }
    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            PbValue::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

/// Scans all Antigravity conversation databases for usage events.
fn scan_antigravity(since_ms: i64) -> Vec<UsageEvent> {
    let Some(convos_dir) = data_source(Provider::Antigravity) else {
        return Vec::new();
    };
    if !convos_dir.exists() {
        return Vec::new();
    }

    let cutoff = since_ms - MTIME_SLACK_MS;
    let mut all_events = Vec::new();

    let Ok(entries) = fs::read_dir(&convos_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "db") {
            continue;
        }

        // Filter by mtime
        if let Ok(metadata) = entry.metadata()
            && let Ok(mtime) = metadata.modified()
        {
            let mtime_ms = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            if mtime_ms < cutoff {
                continue;
            }
        }

        // Extract conversation UUID from filename for session key
        let session_key = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_owned();

        if let Some(events) = read_antigravity_db(&path, &session_key) {
            all_events.extend(events);
        }
    }

    all_events
}

/// The workspace a conversation was opened on, from `trajectory_metadata_blob`
/// field 7 — a `file://` URI like "file:///Users/me/Developer/app".
///
/// This is the only place Antigravity records the project: the per-generation
/// rows carry tokens and nothing about where they were spent, so the whole
/// conversation is attributed to the one workspace it belongs to.
fn extract_agy_workspace(data: &[u8]) -> Option<String> {
    let fields = decode_pb_fields(data);
    let uri = std::str::from_utf8(fields.get(&7)?.first()?.as_bytes()?).ok()?;
    let key = project_key(uri);
    (!key.is_empty()).then_some(key)
}

/// Extract the model name from an `executor_metadata` row's protobuf blob.
/// Path: field 10 → field 1 → field 28 (string like "gemini-3-flash-agent")
fn extract_agy_model_name(data: &[u8]) -> Option<String> {
    let fields = decode_pb_fields(data);
    let f10 = fields.get(&10)?;
    let f10_bytes = f10.first()?.as_bytes()?;
    let f10_sub = decode_pb_fields(f10_bytes);
    let f1 = f10_sub.get(&1)?;
    let f1_bytes = f1.first()?.as_bytes()?;
    let f1_sub = decode_pb_fields(f1_bytes);
    let f28 = f1_sub.get(&28)?;
    let f28_bytes = f28.first()?.as_bytes()?;
    let name = std::str::from_utf8(f28_bytes).ok()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

/// Read usage events from a single Antigravity conversation database.
fn read_antigravity_db(path: &Path, session_key: &str) -> Option<Vec<UsageEvent>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;

    // Build model name lookup from executor_metadata.
    // Path: data → field10 → field1 → field28 (string, e.g. "gemini-3-flash-agent")
    // Each executor_metadata row covers all gen_metadata rows from its idx up to the
    // next executor_metadata idx. We use bisect to find the matching model.
    let mut model_entries: Vec<(i64, String)> = Vec::new();
    if let Ok(mut em_stmt) = conn.prepare("SELECT idx, data FROM executor_metadata ORDER BY idx")
        && let Ok(em_rows) =
            em_stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?)))
    {
        for em_row in em_rows.flatten() {
            let (em_idx, em_data) = em_row;
            if let Some(model) = extract_agy_model_name(&em_data) {
                model_entries.push((em_idx, model));
            }
        }
    }
    let em_indices: Vec<i64> = model_entries.iter().map(|(idx, _)| *idx).collect();

    // One workspace per conversation, read once and stamped on every event the
    // file yields. A database written before the field existed simply reports
    // no project.
    let project_path = conn
        .prepare("SELECT data FROM trajectory_metadata_blob")
        .ok()
        .and_then(|mut stmt| {
            let rows = stmt.query_map([], |row| row.get::<_, Vec<u8>>(0)).ok()?;
            rows.flatten().find_map(|data| extract_agy_workspace(&data))
        })
        .unwrap_or_default();

    let mut events = Vec::new();

    let mut stmt = conn
        .prepare("SELECT idx, data, size FROM gen_metadata")
        .ok()?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .ok()?;

    for row in rows.flatten() {
        let (gm_idx, data, _size) = row;

        // Look up model name via bisect: find largest executor_metadata idx <= gm_idx
        let model_name = {
            let pos = em_indices.partition_point(|&i| i <= gm_idx);
            if pos > 0 {
                model_entries[pos - 1].1.clone()
            } else {
                "gemini-auto".to_owned()
            }
        };

        let fields = decode_pb_fields(&data);

        // Field 1 contains the execution telemetry sub-message
        let Some(f1_vals) = fields.get(&1) else {
            continue;
        };
        let Some(f1_bytes) = f1_vals.first().and_then(|v| v.as_bytes()) else {
            continue;
        };
        let sub = decode_pb_fields(f1_bytes);

        // Token usage is in sub-field 4 (NOT field 2, which is a different sub-message)
        let Some(usage) = sub
            .get(&4)
            .and_then(|v| v.first())
            .and_then(|v| v.as_bytes())
            .map(decode_pb_fields)
        else {
            continue;
        };

        let fresh_input = usage
            .get(&2)
            .and_then(|v| v.first())
            .and_then(|v| v.as_varint())
            .unwrap_or(0);
        let cached_input = usage
            .get(&5)
            .and_then(|v| v.first())
            .and_then(|v| v.as_varint())
            .unwrap_or(0);
        let output = usage
            .get(&3)
            .and_then(|v| v.first())
            .and_then(|v| v.as_varint())
            .unwrap_or(0);
        let reasoning = usage
            .get(&10)
            .and_then(|v| v.first())
            .and_then(|v| v.as_varint())
            .unwrap_or(0);

        let tokens = TokenBreakdown {
            fresh_input,
            cached_input,
            cache_write: 0,
            output,
            reasoning,
        };

        if tokens.total() == 0 {
            continue;
        }

        // Extract request ID as dedup key from usage sub-field 11
        let dedup_id = usage
            .get(&11)
            .and_then(|v| v.first())
            .and_then(|v| v.as_bytes())
            .and_then(|b| std::str::from_utf8(b).ok())
            .map(|s| format!("agy:{}:{}", session_key, s));

        // Timestamp is in sub.field9 → sub-field 4 → field 1 (epoch seconds)
        let timestamp_ms = sub
            .get(&9)
            .and_then(|v| v.first())
            .and_then(|v| v.as_bytes())
            .and_then(|b| {
                let ts_fields = decode_pb_fields(b);
                ts_fields
                    .get(&4)
                    .and_then(|v| v.first())
                    .and_then(|v| v.as_bytes())
                    .and_then(|b2| {
                        let inner = decode_pb_fields(b2);
                        inner
                            .get(&1)
                            .and_then(|v| v.first())
                            .and_then(|v| v.as_varint())
                    })
            })
            .map(|secs| secs as i64 * 1000)
            .unwrap_or_else(|| {
                // Fall back to file mtime
                std::fs::metadata(path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64
                    })
                    .unwrap_or(0)
            });

        events.push(UsageEvent {
            provider: Provider::Antigravity,
            timestamp_ms,
            model_name,
            session_key: session_key.to_owned(),
            project_path: project_path.clone(),
            tokens,
            reported_cost: None,
            dedup_id,
        });
    }

    Some(events)
}

// ---------------------------------------------------------------------------
// Scan + Aggregate
// ---------------------------------------------------------------------------

/// Builds a snapshot for `window`, reading only `providers`.
///
/// The provider set is an argument rather than a lookup: the scanner stays
/// unaware of where the choice came from, so the settings layer can narrow a
/// scan without the aggregation logic knowing settings exist.
pub fn scan_all(
    window: TimeWindow,
    pricing: &PricingTable,
    providers: &[Provider],
) -> UsageSnapshot {
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

    // ── Scan transcript-based providers (Claude, Codex, Kimi) ──────
    let mut all_events = Vec::new();

    for provider in providers.iter().copied().filter(|p| is_transcript_provider(*p)) {
        if let Some(root) = transcript_root(provider) {
            let files = discover_transcripts(&root, start_ts_ms);
            for file in files {
                if let Some(records) = read_transcript_records(&file, provider) {
                    all_events.extend(records);
                }
            }
        }
    }

    // ── Scan SQLite-based providers ────────────────────────────────
    if providers.contains(&Provider::OpenCode) {
        all_events.extend(scan_opencode(start_ts_ms));
    }
    if providers.contains(&Provider::Antigravity) {
        all_events.extend(scan_antigravity(start_ts_ms));
    }

    // ── Cross-file dedup + time filter ─────────────────────────────
    let mut unique_events = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for event in all_events {
        // Filter to window bounds (both sides)
        if event.timestamp_ms < start_ts_ms || event.timestamp_ms > end_ts_ms {
            continue;
        }
        if let Some(id) = &event.dedup_id
            && !seen_ids.insert(id.clone())
        {
            continue;
        }
        unique_events.push(event);
    }

    // ── Aggregate, whole range then project by project ─────────────
    //
    // The per-project views are built from the same deduplicated events as the
    // overall one, so a project's numbers are the overall numbers restricted to
    // its rows — never a second, differently-filtered scan.
    let mut snapshot = aggregate(
        unique_events.iter().collect::<Vec<_>>().as_slice(),
        start_date,
        end_date,
        pricing,
    );
    snapshot.by_project = aggregate_projects(&unique_events, start_date, end_date, pricing);

    let scan_time_ms = start_time.elapsed().as_millis() as u64;
    snapshot.scan_time_ms = scan_time_ms;
    // The scan the page footer reports is the one that produced whatever view
    // is on screen, so every project view quotes the same figure.
    for project in &mut snapshot.by_project {
        project.view.scan_time_ms = scan_time_ms;
    }
    snapshot
}

/// Split `events` by project and aggregate each group into a view of its own,
/// ranked by cost like the provider and model lists.
fn aggregate_projects(
    events: &[UsageEvent],
    start_date: NaiveDate,
    end_date: NaiveDate,
    pricing: &PricingTable,
) -> Vec<ProjectUsage> {
    // Grouped by reference: the events are already in hand, and a project view
    // is built from the same rows the overall one counted.
    let mut groups: HashMap<&str, Vec<&UsageEvent>> = HashMap::new();
    for event in events {
        groups
            .entry(event.project_path.as_str())
            .or_default()
            .push(event);
    }

    let mut projects: Vec<ProjectUsage> = groups
        .into_iter()
        .map(|(path, group)| {
            let view = aggregate(&group, start_date, end_date, pricing);
            ProjectUsage {
                path: path.to_owned(),
                cost_usd: view.cost_usd,
                total_tokens: view.total_tokens,
                cost_fraction: 0.0,
                token_fraction: 0.0,
                view,
            }
        })
        .collect();

    // Shares are taken against the sum of the projects rather than against the
    // overall snapshot: the two are the same total, and reading it from here
    // keeps this function answerable on its own.
    let cost_total: f64 = projects.iter().map(|p| p.cost_usd).sum();
    let token_total: u64 = projects.iter().map(|p| p.total_tokens).sum();
    for project in &mut projects {
        if cost_total > 0.0 {
            project.cost_fraction = project.cost_usd / cost_total;
        }
        if token_total > 0 {
            project.token_fraction = project.total_tokens as f64 / token_total as f64;
        }
    }

    projects.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.total_tokens.cmp(&a.total_tokens))
    });
    projects
}

/// Roll `events` up into a snapshot covering `start_date..=end_date`.
///
/// Pure over the events it is handed, which is what lets the same code produce
/// the overall view and each project's: the caller decides which rows belong,
/// and nothing here reads the disk or the clock. Leaves `by_project` empty and
/// `scan_time_ms` zero for the caller to fill in.
fn aggregate(
    events: &[&UsageEvent],
    start_date: NaiveDate,
    end_date: NaiveDate,
    pricing: &PricingTable,
) -> UsageSnapshot {
    let mut total_tokens: u64 = 0;
    let mut global_tokens = TokenBreakdown::default();
    let mut total_cost = 0.0;
    let mut total_cache_savings = 0.0;
    let mut session_keys: HashSet<&str> = HashSet::new();

    let mut daily_map: HashMap<NaiveDate, DailyAggregate> = HashMap::new();
    let mut model_map: HashMap<(Provider, &str), ModelAggregate> = HashMap::new();

    for event in events {
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
        global_tokens.add(&event.tokens);
        total_cost += cost;
        total_cache_savings += cache_savings;
        session_keys.insert(event.session_key.as_str());

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
            .entry((event.provider, event.model_name.as_str()))
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
                ..Default::default()
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
        start_date,
        end_date,
        tokens: global_tokens,
        total_tokens,
        cost_usd: total_cost,
        cache_savings_usd: total_cache_savings,
        event_count: events.len() as u64,
        session_count: session_keys.len() as u64,
        by_provider,
        by_model,
        daily,
        by_project: Vec::new(),
        scan_time_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(project: &str, cost: f64, tokens: u64, session: &str) -> UsageEvent {
        UsageEvent {
            provider: Provider::Claude,
            // 2026-08-10T12:00:00Z, comfortably inside the test range.
            timestamp_ms: 1_786_708_800_000,
            model_name: "test-model".to_owned(),
            session_key: session.to_owned(),
            project_path: project.to_owned(),
            tokens: TokenBreakdown {
                fresh_input: tokens,
                ..Default::default()
            },
            reported_cost: Some(cost),
            dedup_id: None,
        }
    }

    fn test_range() -> (NaiveDate, NaiveDate) {
        (
            "2026-08-01".parse().unwrap(),
            "2026-08-31".parse().unwrap(),
        )
    }

    #[test]
    fn one_directory_written_several_ways_is_one_project() {
        // Antigravity reports a URI, the rest report a path, and a trailing
        // slash comes and goes — all the same project.
        assert_eq!(project_key("file:///Users/me/app"), "/Users/me/app");
        assert_eq!(project_key("/Users/me/app/"), "/Users/me/app");
        assert_eq!(project_key("  /Users/me/app  "), "/Users/me/app");
        // Nothing usable groups as unattributed rather than as a project
        // named after a stray separator.
        assert!(project_key("").is_empty());
        assert!(project_key("/").is_empty());
        assert!(project_key("file://").is_empty());
    }

    #[test]
    fn a_claude_record_is_attributed_to_the_directory_it_ran_in() {
        let line = r#"{"type":"assistant","timestamp":"2026-08-10T12:00:00.000Z",
            "cwd":"/Users/me/Developer/app","sessionId":"s1","requestId":"r1",
            "message":{"id":"m1","model":"claude-opus-5",
            "usage":{"input_tokens":10,"output_tokens":5}}}"#;
        let event = parse_claude_line(line).unwrap();
        assert_eq!(event.project_path, "/Users/me/Developer/app");
    }

    #[test]
    fn a_claude_record_without_a_directory_still_counts() {
        let line = r#"{"type":"assistant","timestamp":"2026-08-10T12:00:00.000Z",
            "sessionId":"s1","requestId":"r1","message":{"id":"m1",
            "model":"claude-opus-5","usage":{"input_tokens":10,"output_tokens":5}}}"#;
        let event = parse_claude_line(line).unwrap();
        assert!(event.project_path.is_empty());
        assert_eq!(event.tokens.total(), 15);
    }

    #[test]
    fn codex_carries_the_directory_forward_and_follows_it_when_it_moves() {
        let mut state = CodexParseState::default();

        // The session's directory arrives on session_meta, before any usage.
        assert!(parse_codex_line(
            r#"{"type":"session_meta","timestamp":"2026-08-10T12:00:00Z",
                "payload":{"id":"s1","cwd":"/Users/me/first"}}"#,
            &mut state
        )
        .is_none());
        assert!(parse_codex_line(
            r#"{"type":"turn_context","timestamp":"2026-08-10T12:00:00Z",
                "payload":{"model":"gpt-5.6","cwd":"/Users/me/first"}}"#,
            &mut state
        )
        .is_none());

        let usage = r#"{"type":"event_msg","timestamp":"2026-08-10T12:00:01Z",
            "payload":{"type":"token_count","info":{"last_token_usage":
            {"input_tokens":100,"output_tokens":20}}}}"#;
        let first = parse_codex_line(usage, &mut state).unwrap();
        assert_eq!(first.project_path, "/Users/me/first");

        // A turn in another directory attributes from the move onward, the
        // same way a mid-session model switch does.
        parse_codex_line(
            r#"{"type":"turn_context","timestamp":"2026-08-10T13:00:00Z",
                "payload":{"model":"gpt-5.6","cwd":"/Users/me/second"}}"#,
            &mut state,
        );
        let moved = parse_codex_line(
            r#"{"type":"event_msg","timestamp":"2026-08-10T13:00:01Z",
            "payload":{"type":"token_count","info":{"last_token_usage":
            {"input_tokens":300,"output_tokens":40}}}}"#,
            &mut state,
        )
        .unwrap();
        assert_eq!(moved.project_path, "/Users/me/second");
    }

    #[test]
    fn a_project_view_is_the_range_restricted_to_its_own_rows() {
        let (start, end) = test_range();
        let pricing = PricingTable::builtin();
        let events = vec![
            event("/w/a", 3.0, 300, "s1"),
            event("/w/a", 1.0, 100, "s1"),
            event("/w/b", 2.0, 200, "s2"),
        ];

        let projects = aggregate_projects(&events, start, end, &pricing);
        assert_eq!(projects.len(), 2);

        // Ranked by cost, like the provider and model lists.
        assert_eq!(projects[0].path, "/w/a");
        assert_eq!(projects[0].cost_usd, 4.0);
        assert_eq!(projects[0].total_tokens, 400);
        // Two events, one session — counted within the project, not across it.
        assert_eq!(projects[0].view.event_count, 2);
        assert_eq!(projects[0].view.session_count, 1);
        // The view is a snapshot in its own right: same dates, full daily
        // series, and no further split inside it.
        assert_eq!(projects[0].view.start_date, start);
        assert_eq!(projects[0].view.daily.len(), 31);
        assert!(projects[0].view.by_project.is_empty());
    }

    #[test]
    fn the_projects_add_back_up_to_the_range() {
        let (start, end) = test_range();
        let pricing = PricingTable::builtin();
        let events = vec![
            event("/w/a", 3.0, 300, "s1"),
            event("/w/b", 2.0, 200, "s2"),
            // Usage the provider could not attribute is a project of its own
            // rather than a hole in the totals.
            event("", 1.0, 100, "s3"),
        ];

        let overall = aggregate(&events.iter().collect::<Vec<_>>(), start, end, &pricing);
        let projects = aggregate_projects(&events, start, end, &pricing);

        assert_eq!(projects.len(), 3);
        let cost: f64 = projects.iter().map(|p| p.cost_usd).sum();
        let tokens: u64 = projects.iter().map(|p| p.total_tokens).sum();
        assert!((cost - overall.cost_usd).abs() < 1e-9);
        assert_eq!(tokens, overall.total_tokens);

        let shares: f64 = projects.iter().map(|p| p.cost_fraction).sum();
        assert!((shares - 1.0).abs() < 1e-9);
        assert!(projects.iter().any(|p| p.path.is_empty()));
    }

    #[test]
    fn a_range_with_no_usage_has_no_projects_to_filter_by() {
        let (start, end) = test_range();
        assert!(aggregate_projects(&[], start, end, &PricingTable::builtin()).is_empty());
    }
}
