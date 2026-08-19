use std::path::PathBuf;

use gpui::{App, Global};
use serde::{Deserialize, Serialize};

use crate::core::types::{Provider, TimeWindow};
use crate::core::update::{self as update, Channel};
use crate::theme::{self, ThemeMode};

// ---------------------------------------------------------------------------
// Settings — everything the user can see and change in the settings dialog.
//
// Deliberately outside `core`: scanning, pricing and aggregation take their
// inputs as plain arguments, so a new setting is added here and threaded in at
// the call site rather than reaching into the business logic. Adding one means
// three edits — a field with a default, a `SettingsChange` variant, and a row
// in `ui::settings_dialog`.
//
// The live values are published as a GPUI global, mirroring `Theme`: any render
// context reads them with `Settings::current(cx)`, and `update` republishes,
// persists, and repaints in one step.
// ---------------------------------------------------------------------------

/// How many model rows the dashboard offers to list.
pub const MODEL_ROW_OPTIONS: [usize; 4] = [5, 10, 15, 25];

/// Upper bound applied to a hand-edited `model_rows`, so a silly number in the
/// file cannot make the dashboard render thousands of rows.
const MODEL_ROWS_MAX: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub theme: ThemeMode,
    /// The range the dashboard opens on.
    pub default_range: TimeWindow,
    /// Whether opening the app immediately scans, or waits to be asked.
    pub scan_on_launch: bool,
    /// How many rows the "By Model" breakdown lists.
    pub model_rows: usize,
    /// Providers the user switched off, stored as the *disabled* set so a
    /// provider added in a later release is counted by default rather than
    /// silently missing from everyone's totals.
    pub disabled_providers: Vec<Provider>,
    /// Whether launching the app asks GitHub whether a newer one exists.
    ///
    /// This is the only thing Drift ever sends over the network, so it is a
    /// setting rather than an assumption: off means the app stays entirely
    /// local, and the button in Settings still works when asked.
    pub check_for_updates: bool,
    /// Which releases the check offers — see [`Channel`].
    pub update_channel: Channel,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            default_range: TimeWindow::Last30Days,
            scan_on_launch: true,
            model_rows: 15,
            disabled_providers: Vec::new(),
            check_for_updates: true,
            // Whichever kind of build this is, keep the user on that line:
            // installing a beta is how someone opts into betas, and nobody on
            // a stable build is moved onto one without asking.
            update_channel: update::current_channel(),
        }
    }
}

impl Settings {
    /// Read the published settings. Falls back to the defaults if `init`
    /// hasn't run yet.
    pub fn current(cx: &App) -> Self {
        if cx.has_global::<ActiveSettings>() {
            cx.global::<ActiveSettings>().0.clone()
        } else {
            Self::default()
        }
    }

    pub fn is_provider_enabled(&self, provider: Provider) -> bool {
        !self.disabled_providers.contains(&provider)
    }

    /// The providers a scan should read, in [`Provider::ALL`] order.
    pub fn enabled_providers(&self) -> Vec<Provider> {
        Provider::ALL
            .into_iter()
            .filter(|p| self.is_provider_enabled(*p))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Changes
// ---------------------------------------------------------------------------

/// One edit from the settings dialog.
///
/// The dialog reports intent and nothing else; applying, persisting and
/// deciding whether the change costs a rescan all live here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsChange {
    Theme(ThemeMode),
    DefaultRange(TimeWindow),
    ScanOnLaunch(bool),
    ModelRows(usize),
    ToggleProvider(Provider),
    CheckForUpdates(bool),
    UpdateChannel(Channel),
    RestoreDefaults,
}

impl SettingsChange {
    fn apply_to(self, settings: &mut Settings) {
        match self {
            SettingsChange::Theme(mode) => settings.theme = mode,
            SettingsChange::DefaultRange(range) => settings.default_range = range,
            SettingsChange::ScanOnLaunch(enabled) => settings.scan_on_launch = enabled,
            SettingsChange::ModelRows(rows) => settings.model_rows = clamp_model_rows(rows),
            SettingsChange::ToggleProvider(provider) => {
                if let Some(at) = settings
                    .disabled_providers
                    .iter()
                    .position(|p| *p == provider)
                {
                    settings.disabled_providers.remove(at);
                } else {
                    settings.disabled_providers.push(provider);
                }
            }
            SettingsChange::CheckForUpdates(enabled) => settings.check_for_updates = enabled,
            SettingsChange::UpdateChannel(channel) => settings.update_channel = channel,
            SettingsChange::RestoreDefaults => *settings = Settings::default(),
        }
    }

    /// Whether the change alters *which events are counted*, and so needs the
    /// snapshot rebuilt. Everything else is a view or launch preference the
    /// current snapshot already answers.
    pub fn requires_rescan(self) -> bool {
        match self {
            SettingsChange::ToggleProvider(_) | SettingsChange::RestoreDefaults => true,
            SettingsChange::Theme(_)
            | SettingsChange::DefaultRange(_)
            | SettingsChange::ScanOnLaunch(_)
            | SettingsChange::ModelRows(_)
            | SettingsChange::CheckForUpdates(_)
            | SettingsChange::UpdateChannel(_) => false,
        }
    }
}

fn clamp_model_rows(rows: usize) -> usize {
    rows.clamp(1, MODEL_ROWS_MAX)
}

// ---------------------------------------------------------------------------
// Publication
// ---------------------------------------------------------------------------

struct ActiveSettings(Settings);

impl Global for ActiveSettings {}

/// Load the settings from disk, publish them, and paint the theme they ask for.
/// Call once from `main` before opening any window.
pub fn init(cx: &mut App) -> Settings {
    let settings = load();
    theme::apply(cx, settings.theme);
    cx.set_global(ActiveSettings(settings.clone()));
    settings
}

/// Apply `change`, persist it, and repaint. Returns whether the caller has to
/// rescan — see [`SettingsChange::requires_rescan`].
pub fn update(cx: &mut App, change: SettingsChange) -> bool {
    let mut settings = Settings::current(cx);
    change.apply_to(&mut settings);

    theme::apply(cx, settings.theme);
    save(&settings);
    cx.set_global(ActiveSettings(settings));
    // Views hold no settings of their own, so a repaint is what makes the
    // change visible — including a theme switch, which no view is watching.
    cx.refresh_windows();

    change.requires_rescan()
}

// ---------------------------------------------------------------------------
// On-disk form
//
// Kept as its own struct of primitives so the file format is not hostage to
// the names of the enum variants in `core::types`, and so an older or
// hand-edited file still loads: every field is optional-by-default and falls
// back to the shipped default, and unknown keys are ignored.
// ---------------------------------------------------------------------------

#[derive(Default, Serialize, Deserialize)]
struct StoredSettings {
    #[serde(default)]
    theme: String,
    #[serde(default)]
    default_range: String,
    #[serde(default)]
    scan_on_launch: Option<bool>,
    #[serde(default)]
    model_rows: Option<usize>,
    #[serde(default)]
    disabled_providers: Vec<String>,
    #[serde(default)]
    check_for_updates: Option<bool>,
    #[serde(default)]
    update_channel: String,
}

impl StoredSettings {
    fn from_settings(settings: &Settings) -> Self {
        Self {
            theme: theme_key(settings.theme).to_owned(),
            default_range: range_key(settings.default_range).to_owned(),
            scan_on_launch: Some(settings.scan_on_launch),
            model_rows: Some(settings.model_rows),
            disabled_providers: settings
                .disabled_providers
                .iter()
                .map(|p| provider_key(*p).to_owned())
                .collect(),
            check_for_updates: Some(settings.check_for_updates),
            update_channel: settings.update_channel.key().to_owned(),
        }
    }

    fn into_settings(self) -> Settings {
        let defaults = Settings::default();
        Settings {
            theme: theme_from_key(&self.theme).unwrap_or(defaults.theme),
            default_range: range_from_key(&self.default_range).unwrap_or(defaults.default_range),
            scan_on_launch: self.scan_on_launch.unwrap_or(defaults.scan_on_launch),
            model_rows: self
                .model_rows
                .map(clamp_model_rows)
                .unwrap_or(defaults.model_rows),
            disabled_providers: self
                .disabled_providers
                .iter()
                .filter_map(|key| provider_from_key(key))
                .collect(),
            check_for_updates: self.check_for_updates.unwrap_or(defaults.check_for_updates),
            update_channel: Channel::from_key(&self.update_channel)
                .unwrap_or(defaults.update_channel),
        }
    }
}

// The matches below are exhaustive on purpose: adding a variant upstream fails
// to compile until it has a stable key to be written as.

fn theme_key(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "system",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }
}

fn theme_from_key(key: &str) -> Option<ThemeMode> {
    ThemeMode::ALL.into_iter().find(|m| theme_key(*m) == key)
}

fn range_key(range: TimeWindow) -> &'static str {
    match range {
        TimeWindow::Last7Days => "last_7_days",
        TimeWindow::Last30Days => "last_30_days",
        TimeWindow::Last90Days => "last_90_days",
        TimeWindow::CurrentMonth => "current_month",
        TimeWindow::PreviousMonth => "previous_month",
    }
}

fn range_from_key(key: &str) -> Option<TimeWindow> {
    TimeWindow::ALL.into_iter().find(|r| range_key(*r) == key)
}

fn provider_key(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Codex => "codex",
        Provider::Kimi => "kimi",
        Provider::OpenCode => "opencode",
        Provider::Antigravity => "antigravity",
    }
}

fn provider_from_key(key: &str) -> Option<Provider> {
    Provider::ALL.into_iter().find(|p| provider_key(*p) == key)
}

/// Where the settings file lives: `<config dir>/drift/settings.json`.
pub fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("drift").join("settings.json"))
}

fn load() -> Settings {
    let Some(path) = config_path() else {
        return Settings::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Settings::default();
    };
    // A corrupt or half-written file is not worth failing to start over: the
    // next change rewrites it.
    serde_json::from_str::<StoredSettings>(&text)
        .map(StoredSettings::into_settings)
        .unwrap_or_default()
}

fn save(settings: &Settings) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&StoredSettings::from_settings(settings)) {
        let _ = std::fs::write(&path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(settings: &Settings) -> Settings {
        let text = serde_json::to_string(&StoredSettings::from_settings(settings)).unwrap();
        serde_json::from_str::<StoredSettings>(&text)
            .unwrap()
            .into_settings()
    }

    #[test]
    fn every_field_survives_a_trip_through_the_file() {
        let settings = Settings {
            theme: ThemeMode::Light,
            default_range: TimeWindow::PreviousMonth,
            scan_on_launch: false,
            model_rows: 5,
            disabled_providers: vec![Provider::Kimi, Provider::Antigravity],
            check_for_updates: false,
            update_channel: Channel::Beta,
        };
        assert_eq!(round_trip(&settings), settings);
    }

    #[test]
    fn defaults_round_trip_unchanged() {
        let defaults = Settings::default();
        assert_eq!(round_trip(&defaults), defaults);
    }

    #[test]
    fn an_empty_or_unknown_file_loads_as_the_defaults() {
        let from_empty: StoredSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(from_empty.into_settings(), Settings::default());

        // Keys we have never written, and values we no longer recognise, are
        // ignored rather than fatal — old files keep opening.
        let stored: StoredSettings = serde_json::from_str(
            r#"{"theme":"solarized","default_range":"last_decade",
                "disabled_providers":["claude","typewriter"],"future_setting":true}"#,
        )
        .unwrap();
        let settings = stored.into_settings();
        assert_eq!(settings.theme, Settings::default().theme);
        assert_eq!(settings.default_range, Settings::default().default_range);
        assert_eq!(settings.disabled_providers, vec![Provider::Claude]);
    }

    #[test]
    fn a_hand_edited_row_count_is_clamped() {
        let stored: StoredSettings =
            serde_json::from_str(r#"{"model_rows":100000}"#).unwrap();
        assert_eq!(stored.into_settings().model_rows, MODEL_ROWS_MAX);

        let stored: StoredSettings = serde_json::from_str(r#"{"model_rows":0}"#).unwrap();
        assert_eq!(stored.into_settings().model_rows, 1);
    }

    #[test]
    fn toggling_a_provider_turns_it_off_then_back_on() {
        let mut settings = Settings::default();
        assert!(settings.is_provider_enabled(Provider::Codex));

        SettingsChange::ToggleProvider(Provider::Codex).apply_to(&mut settings);
        assert!(!settings.is_provider_enabled(Provider::Codex));
        assert_eq!(settings.enabled_providers().len(), Provider::ALL.len() - 1);

        SettingsChange::ToggleProvider(Provider::Codex).apply_to(&mut settings);
        assert!(settings.is_provider_enabled(Provider::Codex));
        assert_eq!(settings.enabled_providers().len(), Provider::ALL.len());
    }

    #[test]
    fn enabled_providers_keeps_the_canonical_order() {
        let settings = Settings {
            disabled_providers: vec![Provider::Codex],
            ..Default::default()
        };
        assert_eq!(
            settings.enabled_providers(),
            vec![
                Provider::Claude,
                Provider::Kimi,
                Provider::OpenCode,
                Provider::Antigravity
            ]
        );
    }

    #[test]
    fn an_older_file_without_the_update_keys_keeps_the_shipped_defaults() {
        // The keys arrived after the first release, so a settings file written
        // by it has neither — and must not read as "updates off".
        let stored: StoredSettings =
            serde_json::from_str(r#"{"theme":"dark","model_rows":10}"#).unwrap();
        let settings = stored.into_settings();
        assert_eq!(settings.check_for_updates, Settings::default().check_for_updates);
        assert_eq!(settings.update_channel, Settings::default().update_channel);
    }

    #[test]
    fn an_unknown_channel_falls_back_rather_than_opting_into_betas() {
        let stored: StoredSettings =
            serde_json::from_str(r#"{"update_channel":"nightly"}"#).unwrap();
        assert_eq!(
            stored.into_settings().update_channel,
            Settings::default().update_channel
        );

        let stored: StoredSettings =
            serde_json::from_str(r#"{"update_channel":"beta"}"#).unwrap();
        assert_eq!(stored.into_settings().update_channel, Channel::Beta);
    }

    #[test]
    fn only_changes_to_what_is_counted_cost_a_rescan() {
        assert!(SettingsChange::ToggleProvider(Provider::Claude).requires_rescan());
        assert!(SettingsChange::RestoreDefaults.requires_rescan());

        assert!(!SettingsChange::Theme(ThemeMode::Dark).requires_rescan());
        assert!(!SettingsChange::ModelRows(5).requires_rescan());
        assert!(!SettingsChange::ScanOnLaunch(false).requires_rescan());
        assert!(!SettingsChange::DefaultRange(TimeWindow::Last7Days).requires_rescan());
        assert!(!SettingsChange::CheckForUpdates(false).requires_rescan());
        assert!(!SettingsChange::UpdateChannel(Channel::Beta).requires_rescan());
    }

    #[test]
    fn the_update_channel_is_a_plain_switch() {
        let mut settings = Settings::default();

        SettingsChange::UpdateChannel(Channel::Beta).apply_to(&mut settings);
        assert_eq!(settings.update_channel, Channel::Beta);

        SettingsChange::UpdateChannel(Channel::Stable).apply_to(&mut settings);
        assert_eq!(settings.update_channel, Channel::Stable);

        SettingsChange::CheckForUpdates(false).apply_to(&mut settings);
        assert!(!settings.check_for_updates);
    }

    #[test]
    fn restoring_defaults_clears_every_edit() {
        let mut settings = Settings {
            theme: ThemeMode::Dark,
            default_range: TimeWindow::Last7Days,
            scan_on_launch: false,
            model_rows: 25,
            disabled_providers: vec![Provider::Claude],
            check_for_updates: false,
            update_channel: Channel::Beta,
        };
        SettingsChange::RestoreDefaults.apply_to(&mut settings);
        assert_eq!(settings, Settings::default());
    }
}
