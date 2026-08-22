mod category;
mod context;
mod controls;
mod panes;
mod sidebar;

pub use category::SettingsCategory;

use std::sync::Arc;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, Anchor, AnchoredPositionMode, App, FocusHandle,
    MouseButton, SharedString, Window,
};

use crate::core::update::CheckState;
use crate::keymap::{Cancel, SETTINGS_DIALOG_CONTEXT};
use crate::settings::{Settings, SettingsChange};
use crate::theme::Theme;

use super::components::{Button, ButtonSize, IconButton};
use super::icons::Icon;
use context::{ChangeCallback, PaneContext, PlainCallback};
use sidebar::{sidebar, SelectCallback, SIDEBAR_WIDTH};

// ---------------------------------------------------------------------------
// SettingsDialog — a modal sheet over the dashboard, laid out as a sidebar of
// categories beside the settings of whichever one is selected.
//
// The dialog is a pure view of the values it is handed: it reads the settings
// and the selected category, and reports every edit as a `SettingsChange` and
// every click in the sidebar as a category, so it never applies, persists or
// remembers anything itself. What it renders is decided in three separate
// places, each replaceable on its own:
//
//   * `sidebar`   — the navigation, which only reports what was clicked.
//   * `category`  — the registry mapping an entry to the pane that draws it.
//   * `panes/*`   — one module per category, owning that category's settings.
//
// So adding a setting is a row in one pane plus a variant in
// `settings::SettingsChange`, and adding a *category* is a variant in
// `SettingsCategory` plus a pane module — neither touches this file.
//
// Edits take effect immediately; there is no OK/Cancel pair to reconcile, and
// "Done" only closes the sheet.
// ---------------------------------------------------------------------------

/// Dialog width: the navigation column, plus a content pane as wide as the
/// whole sheet used to be — so every control that fitted the single-page
/// layout still fits, source paths and the five-way range switch included.
const DIALOG_WIDTH: f32 = SIDEBAR_WIDTH + 440.0;
/// Dialog height, held constant across categories so switching panes moves the
/// sidebar's selection and nothing else. Tall enough for the longest pane —
/// the five data sources — to be read without scrolling; shorter panes simply
/// leave room below.
const DIALOG_HEIGHT: f32 = 420.0;
/// Height of the sheet's title bar: the close button's own height plus the
/// clearance around it, and no more.
const HEADER_HEIGHT: f32 = 38.0;
/// Height kept clear above and below the sheet, so it never touches the window
/// edges on a short window and its scroll region stays obvious.
const VIEWPORT_MARGIN: f32 = 48.0;
/// Shortest the sheet is ever drawn. Below this the sidebar and the pane have
/// nothing left to show between the header and the footer, so a very short
/// window gets a sheet that overflows it rather than one collapsed to nothing.
const MIN_DIALOG_HEIGHT: f32 = 240.0;

#[derive(IntoElement)]
pub struct SettingsDialog {
    settings: Settings,
    /// Which category's settings to show. Owned by the root view, so a reopened
    /// dialog comes back to the pane it was left on.
    category: SettingsCategory,
    /// Focused while the dialog is open, so Escape reaches it.
    focus: FocusHandle,
    /// What the last update check found — owned by the root view, since a
    /// check outlives the dialog that asked for it.
    update_state: CheckState,
    on_change: Option<ChangeCallback>,
    on_select_category: Option<SelectCallback>,
    on_close: Option<PlainCallback>,
    on_check_updates: Option<PlainCallback>,
    on_download: Option<PlainCallback>,
}

impl SettingsDialog {
    pub fn new(settings: Settings, focus: FocusHandle) -> Self {
        Self {
            settings,
            category: SettingsCategory::default(),
            focus,
            update_state: CheckState::Idle,
            on_change: None,
            on_select_category: None,
            on_close: None,
            on_check_updates: None,
            on_download: None,
        }
    }

    pub fn category(mut self, category: SettingsCategory) -> Self {
        self.category = category;
        self
    }

    pub fn update_state(mut self, state: CheckState) -> Self {
        self.update_state = state;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(SettingsChange, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    pub fn on_select_category(
        mut self,
        handler: impl Fn(SettingsCategory, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_select_category = Some(Arc::new(handler));
        self
    }

    pub fn on_close(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_close = Some(Arc::new(handler));
        self
    }

    pub fn on_check_updates(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_check_updates = Some(Arc::new(handler));
        self
    }

    pub fn on_download(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_download = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for SettingsDialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let viewport = window.viewport_size();
        let category = self.category;

        // ── Header ─────────────────────────────────────────────────
        //
        // Spans both columns, so the sheet has one title rather than one per
        // panel. Kept to the height of the close button and nothing more: the
        // sheet's own chrome should not cost the settings a band of empty
        // space, and the app's mark belongs on the About pane, where it is the
        // subject rather than decoration.
        //
        // The title is a breadcrumb — Settings › category — so "where am I"
        // is answered once, up here, and every pane can open straight onto
        // its rows instead of restating its own name first.
        let on_close = self.on_close.clone();
        let header = div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .px(px(18.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(13.0))
                    .child(div().text_color(theme.text_secondary).child("Settings"))
                    .child(div().text_color(theme.text_tertiary).child("›"))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(category.label()),
                    ),
            )
            .child(
                IconButton::new("settings-close", Icon::Close).on_click(move |window, cx| {
                    if let Some(handler) = &on_close {
                        handler(window, cx);
                    }
                }),
            );

        // ── Content ────────────────────────────────────────────────
        //
        // Only the selected category's pane is built: an unshown pane costs
        // nothing, so a category added later cannot slow this one down.
        let pane_context = PaneContext::new(
            &self.settings,
            &self.update_state,
            self.on_change.clone(),
            self.on_check_updates.clone(),
            self.on_download.clone(),
        );
        let content = div()
            // Keyed to the category so each pane keeps its own scroll
            // position, rather than inheriting one from the pane before it.
            .id(SharedString::from(format!(
                "settings-pane-{}",
                category.key()
            )))
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(18.0))
            // The header's breadcrumb already names the pane, so its rows
            // open straight away rather than under a heading of their own.
            .pt(px(10.0))
            .pb(px(12.0))
            .flex()
            .flex_col()
            .children(category.rows(&pane_context, cx));

        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(sidebar(category, self.on_select_category.clone(), cx))
            .child(content);

        // ── Footer ─────────────────────────────────────────────────
        //
        // Spans both columns like the header: "Restore defaults" resets every
        // category, not the one on screen, so it does not belong inside a pane.
        // The band mirrors the header — same height, same gutter — so the
        // sheet's top and bottom chrome read as a pair.
        let reset = self.on_change.clone();
        let done = self.on_close.clone();
        let footer = div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .px(px(18.0))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                Button::new("settings-reset", "Restore defaults")
                    .subtle()
                    .size(ButtonSize::Sm)
                    .on_click(move |window, cx| {
                        if let Some(handler) = &reset {
                            handler(SettingsChange::RestoreDefaults, window, cx);
                        }
                    }),
            )
            .child(
                Button::new("settings-done", "Done")
                    .size(ButtonSize::Sm)
                    .on_click(move |window, cx| {
                        if let Some(handler) = &done {
                            handler(window, cx);
                        }
                    }),
            );

        // ── Sheet ──────────────────────────────────────────────────
        //
        // A fixed height, capped by the window: the sheet keeps its shape as
        // panes change, and still fits a window too short to hold it. The floor
        // is what keeps a very short window from asking for a negative height.
        let available = (viewport.height - px(VIEWPORT_MARGIN * 2.0)).max(px(MIN_DIALOG_HEIGHT));
        let height = px(DIALOG_HEIGHT).min(available);
        let on_cancel = self.on_close.clone();
        let sheet = div()
            .id("settings-sheet")
            .track_focus(&self.focus)
            // Declaring the context is what scopes `escape` to the dialog:
            // the binding only matches while this is on the focus path, so no
            // key is intercepted once the sheet is gone.
            .key_context(SETTINGS_DIALOG_CONTEXT)
            .occlude()
            .w(px(DIALOG_WIDTH))
            .h(height)
            .flex()
            .flex_col()
            .rounded(px(10.0))
            // The sidebar's wash runs to the sheet's edge, so the corners have
            // to clip it or it squares them off.
            .overflow_hidden()
            .bg(theme.canvas)
            .border_1()
            .border_color(theme.border_strong)
            .shadow_lg()
            .on_action(move |_: &Cancel, window, cx| {
                if let Some(handler) = &on_cancel {
                    handler(window, cx);
                }
            })
            .child(header)
            .child(body)
            .child(footer);

        // A dimmed backdrop both signals the modal state and gives the click
        // that dismisses it somewhere to land, matching how the range menu
        // dismisses.
        let on_backdrop = self.on_close.clone();
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position_mode(AnchoredPositionMode::Window)
                .position(point(px(0.0), px(0.0)))
                .child(
                    div()
                        .id("settings-backdrop")
                        .occlude()
                        .w(viewport.width)
                        .h(viewport.height)
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.scrim)
                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                            if let Some(handler) = &on_backdrop {
                                handler(window, cx);
                            }
                        })
                        .child(sheet),
                ),
        )
        .with_priority(3)
    }
}

