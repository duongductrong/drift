use gpui::{AnyElement, App};

use super::context::PaneContext;
use super::panes;

// ---------------------------------------------------------------------------
// SettingsCategory — the sidebar's entries, and the map from an entry to the
// settings it shows.
//
// This enum is the whole registry. Adding a category is a variant here plus a
// module under `panes`: the matches below are exhaustive, so it does not
// compile until the new entry has a sidebar label and a pane to render, and
// nothing else in the dialog has to be told it exists.
//
// Which category is selected is *not* stored here — the root view owns that,
// the way it owns every other piece of dialog state, so the dialog itself stays
// a pure view of what it is handed.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsCategory {
    /// Opening on the theme keeps the first pane the one that changes
    /// something visible immediately.
    #[default]
    Appearance,
    Dashboard,
    DataSources,
    Scanning,
    Updates,
    /// What this build is. Not a setting, but it lives in the navigation next
    /// to the settings that touch it.
    About,
}

impl SettingsCategory {
    /// Sidebar order, top to bottom: what the app looks like, then what it
    /// shows, then where the numbers come from, then when it goes looking.
    pub const ALL: [SettingsCategory; 6] = [
        SettingsCategory::Appearance,
        SettingsCategory::Dashboard,
        SettingsCategory::DataSources,
        SettingsCategory::Scanning,
        SettingsCategory::Updates,
        SettingsCategory::About,
    ];

    /// The sidebar entry — also the breadcrumb segment the header shows for
    /// the selected pane.
    pub fn label(self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Dashboard => "Dashboard",
            SettingsCategory::DataSources => "Data Sources",
            SettingsCategory::Scanning => "Scanning",
            SettingsCategory::Updates => "Updates",
            SettingsCategory::About => "About",
        }
    }

    /// Stable name, used to build element ids that survive a relabelling.
    pub fn key(self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "appearance",
            SettingsCategory::Dashboard => "dashboard",
            SettingsCategory::DataSources => "data-sources",
            SettingsCategory::Scanning => "scanning",
            SettingsCategory::Updates => "updates",
            SettingsCategory::About => "about",
        }
    }

    /// The rows this category shows, from the pane that owns them.
    pub(super) fn rows(self, ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
        match self {
            SettingsCategory::Appearance => panes::appearance::rows(ctx, cx),
            SettingsCategory::Dashboard => panes::dashboard::rows(ctx, cx),
            SettingsCategory::DataSources => panes::data_sources::rows(ctx, cx),
            SettingsCategory::Scanning => panes::scanning::rows(ctx, cx),
            SettingsCategory::Updates => panes::updates::rows(ctx, cx),
            SettingsCategory::About => panes::about::rows(ctx, cx),
        }
    }
}
