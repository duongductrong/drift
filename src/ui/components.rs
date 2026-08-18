use gpui::{div, prelude::*, px, App, Hsla, SharedString, Window};
use crate::theme::Theme;

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
