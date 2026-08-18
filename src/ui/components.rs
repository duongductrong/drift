use gpui::{div, prelude::*, px, relative, App, Hsla, SharedString, Window};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// StatCard — a headline metric card with label, large value, and optional
// detail caption. Mirrors Waku's summary headline pattern.
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct StatCard {
    label: SharedString,
    value: SharedString,
    detail: Option<SharedString>,
}

impl StatCard {
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            detail: None,
        }
    }

    /// Optional tertiary detail line below the value.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl RenderOnce for StatCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let mut card = div()
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
            );
        if let Some(detail) = self.detail {
            card = card.child(
                div()
                    .text_size(px(9.5))
                    .text_color(theme.text_tertiary)
                    .child(detail),
            );
        }
        card
    }
}

// ---------------------------------------------------------------------------
// SectionHeader — subdued, bottom-bordered section title with optional
// right-aligned action element
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct SectionHeader {
    title: SharedString,
    action: Option<gpui::AnyElement>,
}

impl SectionHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            action: None,
        }
    }

    /// Optional right-aligned action element (e.g. "View all" link).
    pub fn action(mut self, element: impl IntoElement) -> Self {
        self.action = Some(element.into_any_element());
        self
    }
}

impl RenderOnce for SectionHeader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let mut row = div()
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
            );
        if let Some(action) = self.action {
            row = row.child(action);
        }
        row
    }
}

// ---------------------------------------------------------------------------
// ProgressBar — horizontal fill bar, auto-colors by percentage or accepts a
// custom fill via the builder
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct ProgressBar {
    fraction: f32,
    fill_color: Option<Hsla>,
}

impl ProgressBar {
    /// Create from a 0.0–1.0 fraction.
    pub fn new(fraction: f32) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
            fill_color: None,
        }
    }

    /// Override the auto-computed fill color.
    pub fn color(mut self, color: Hsla) -> Self {
        self.fill_color = Some(color);
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::current(cx);
        let visible = if self.fraction > 0.0 {
            self.fraction.max(0.02)
        } else {
            0.0
        };
        let fill = self.fill_color.unwrap_or_else(|| {
            let pct = self.fraction * 100.0;
            if pct >= 95.0 {
                theme.danger
            } else if pct >= 80.0 {
                theme.warning
            } else {
                theme.gauge
            }
        });
        div()
            .h(px(4.0))
            .w_full()
            .rounded_full()
            .bg(theme.overlay_strong)
            .child(
                div()
                    .h_full()
                    .w(relative(visible))
                    .rounded_full()
                    .bg(fill),
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
