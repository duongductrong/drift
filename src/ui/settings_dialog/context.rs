use std::sync::Arc;

use gpui::{App, Window};

use crate::core::update::CheckState;
use crate::settings::{Settings, SettingsChange};

// ---------------------------------------------------------------------------
// PaneContext — what a pane is handed to draw itself.
//
// A pane reads the settings it is given and reports every edit back through
// the callbacks here; it never touches the live settings or the `App` state
// directly. That is what keeps a pane a pure function of the dialog's inputs,
// and what lets the shell decide which pane runs without any of them knowing
// about the others.
// ---------------------------------------------------------------------------

pub(super) type ChangeCallback = Arc<dyn Fn(SettingsChange, &mut Window, &mut App) + Send + Sync>;
pub(super) type PlainCallback = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>;

pub(super) struct PaneContext<'a> {
    /// The settings the dialog was handed — the values every control reflects.
    pub(super) settings: &'a Settings,
    /// What the last update check found. Owned by the root view, since a check
    /// outlives the dialog that asked for it.
    pub(super) update_state: &'a CheckState,
    on_change: Option<ChangeCallback>,
    on_check_updates: Option<PlainCallback>,
    on_download: Option<PlainCallback>,
}

impl<'a> PaneContext<'a> {
    pub(super) fn new(
        settings: &'a Settings,
        update_state: &'a CheckState,
        on_change: Option<ChangeCallback>,
        on_check_updates: Option<PlainCallback>,
        on_download: Option<PlainCallback>,
    ) -> Self {
        Self {
            settings,
            update_state,
            on_change,
            on_check_updates,
            on_download,
        }
    }

    /// A click handler that reports `change`.
    ///
    /// The callback is cloned into the handler rather than borrowed, so the
    /// handler owns everything it needs and can outlive the pane that built it.
    pub(super) fn emit(
        &self,
        change: SettingsChange,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let on_change = self.on_change.clone();
        move |_event, window, cx| {
            if let Some(handler) = &on_change {
                handler(change, window, cx);
            }
        }
    }

    /// Ask for an update check now.
    pub(super) fn check_updates(&self) -> impl Fn(&mut Window, &mut App) + 'static {
        invoke(self.on_check_updates.clone())
    }

    /// Open the release page for the update on offer.
    pub(super) fn download(&self) -> impl Fn(&mut Window, &mut App) + 'static {
        invoke(self.on_download.clone())
    }
}

/// Wraps a plain callback in a handler that tolerates its absence, mirroring
/// [`PaneContext::emit`].
pub(super) fn invoke(handler: Option<PlainCallback>) -> impl Fn(&mut Window, &mut App) + 'static {
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(window, cx);
        }
    }
}
