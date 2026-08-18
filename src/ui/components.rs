use std::sync::Arc;

use gpui::{div, prelude::*, px, App, Hsla, SharedString, Window};
use crate::theme::Theme;
use super::icons::{Icon, ICON_SIZE};
use super::tooltip::Tooltip;

type ClickHandler = Arc<dyn Fn(&mut Window, &mut App) + Send + Sync>;

// ---------------------------------------------------------------------------
// Button — the app's full-size action button.
//
// `Primary` is the filled, inverse-on-canvas style; `Subtle` is the same
// geometry with no fill, for the secondary action sitting beside one.
//
// Usage:
//   Button::new("done", "Done").on_click(|_window, _cx| { … })
//   Button::new("reset", "Restore defaults").subtle().on_click(…)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Primary,
    Subtle,
}

#[derive(IntoElement)]
pub struct Button {
    id: SharedString,
    label: SharedString,
    style: ButtonStyle,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            style: ButtonStyle::Primary,
            on_click: None,
        }
    }

    pub fn subtle(mut self) -> Self {
        self.style = ButtonStyle::Subtle;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let on_click = self.on_click.clone();

        div()
            .id(self.id)
            .flex_none()
            .px(px(14.0))
            .py(px(6.0))
            .rounded(px(6.0))
            .text_size(px(12.0))
            .cursor_pointer()
            .map(|el| match self.style {
                ButtonStyle::Primary => el
                    .bg(theme.inverse)
                    .text_color(theme.on_inverse)
                    .hover(|style| style.opacity(0.85)),
                ButtonStyle::Subtle => el
                    .text_color(theme.text_secondary)
                    .hover(|style| style.bg(theme.overlay).text_color(theme.text)),
            })
            .child(self.label)
            .on_click(move |_event, window, cx| {
                if let Some(handler) = &on_click {
                    handler(window, cx);
                }
            })
    }
}

// ---------------------------------------------------------------------------
// IconButton — a square, icon-only control for the toolbar.
//
// Sized to the same 26pt height as the dashboard's filter pill so the chrome
// lines up, and labelled by a tooltip since the glyph carries no text. `busy`
// stands in for an action already running: the icon dims and clicks stop, so a
// scan cannot be started twice.
//
// Usage:
//   IconButton::new("scan", Icon::Refresh)
//       .tooltip("Scan transcripts")
//       .on_click(|_window, _cx| { … })
// ---------------------------------------------------------------------------

/// Side length of an icon button — matches the filter pill's height.
pub const ICON_BUTTON_SIZE: f32 = 26.0;

#[derive(IntoElement)]
pub struct IconButton {
    id: SharedString,
    icon: Icon,
    tooltip: Option<SharedString>,
    selected: bool,
    busy: bool,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<SharedString>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            icon,
            tooltip: None,
            selected: false,
            busy: false,
            on_click: None,
        }
    }

    pub fn tooltip(mut self, label: impl Into<SharedString>) -> Self {
        self.tooltip = Some(label.into());
        self
    }

    /// Held-open state, for a button whose panel is currently showing.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The action is already running: dimmed and inert.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let on_click = self.on_click.clone();

        // The icon is painted as a mask, so its tint cannot be inherited from a
        // hover style on the button. A group hover keyed to this button's id
        // lets the icon brighten with the button it sits in.
        let group = SharedString::from(format!("{}-icon", self.id));
        let resting = if self.busy {
            theme.text_ghost
        } else if self.selected {
            theme.text
        } else {
            theme.text_secondary
        };

        div()
            .id(self.id)
            .group(group.clone())
            .flex_none()
            .size(px(ICON_BUTTON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(7.0))
            .cursor_default()
            .when(self.selected, |el| el.bg(theme.overlay_strong))
            .when(!self.busy, |el| el.hover(|style| style.bg(theme.overlay)))
            .child(
                self.icon
                    .element(px(ICON_SIZE), resting)
                    .when(!self.busy, |icon| {
                        icon.group_hover(group, |style| style.text_color(theme.text))
                    }),
            )
            .when_some(self.tooltip, |el, label| el.tooltip(Tooltip::text(label)))
            .when(!self.busy, |el| {
                el.on_click(move |_event, window, cx| {
                    if let Some(handler) = &on_click {
                        handler(window, cx);
                    }
                })
            })
    }
}

// ---------------------------------------------------------------------------
// StatCard — a headline metric card with label and large value. Mirrors
// Waku's summary headline pattern.
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct StatCard {
    label: SharedString,
    value: SharedString,
}

impl StatCard {
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

impl RenderOnce for StatCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .flex_1()
            .p(px(14.0))
            .rounded(px(8.0))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme.text_tertiary)
                    .child(self.label),
            )
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text)
                    .child(self.value),
            )
    }
}

// ---------------------------------------------------------------------------
// SectionHeader — subdued, bottom-bordered section title.
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct SectionHeader {
    title: SharedString,
}

impl SectionHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
        }
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        div()
            .pb(px(8.0))
            .mb(px(4.0))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_secondary)
                    .child(self.title),
            )
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers (pure functions, no UI dependency)
// ---------------------------------------------------------------------------

pub fn format_cost(usd: f64) -> String {
    if usd >= 1.0 {
        format!("${:.2}", usd)
    } else if usd >= 0.01 {
        format!("${:.3}", usd)
    } else {
        format!("${:.4}", usd)
    }
}

pub fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}k", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

/// Compact token format for floats (used in metric tiles).
pub fn format_tokens_compact(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{:.0}", value)
    }
}

pub fn format_percent(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

/// Provider brand color for chart fills and row indicators.
pub fn provider_color(theme: &Theme, provider: crate::core::types::Provider) -> Hsla {
    match provider {
        crate::core::types::Provider::Claude => theme.chart_claude,
        crate::core::types::Provider::Codex => theme.chart_codex,
        crate::core::types::Provider::Kimi => theme.chart_kimi,
        crate::core::types::Provider::OpenCode => theme.chart_opencode,
        crate::core::types::Provider::Antigravity => theme.chart_antigravity,
    }
}
