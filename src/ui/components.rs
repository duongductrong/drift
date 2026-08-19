use std::sync::Arc;

use gpui::{div, prelude::*, px, App, Hsla, SharedString, Window};
use crate::core::types::UsageMetric;
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
    active: bool,
}

impl StatCard {
    pub fn new(label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            active: false,
        }
    }

    /// Marks the card the rest of the page is currently ranked by, so the
    /// metric switch has something to point at up here too.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
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
            .border_color(if self.active {
                theme.border_strong
            } else {
                theme.border
            })
            .when(self.active, |el| el.bg(theme.overlay))
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(if self.active {
                        theme.text_secondary
                    } else {
                        theme.text_tertiary
                    })
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
    hint: Option<SharedString>,
}

impl SectionHeader {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            hint: None,
        }
    }

    /// A trailing note on the section's terms — which metric it is ranked and
    /// quoted in, so a list read out of context still says what its numbers
    /// mean.
    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
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
            .children(self.hint.map(|hint| {
                div()
                    .text_size(px(10.0))
                    .text_color(theme.text_tertiary)
                    .child(hint)
            }))
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers (pure functions, no UI dependency)
// ---------------------------------------------------------------------------

/// The magnitude ladder every large number on the page climbs: thousands,
/// millions, billions, trillions.
///
/// Precision tracks the leading digits rather than the unit, so a value keeps
/// about three significant figures wherever it lands — `8.81B`, `616M`,
/// `71.8k`. Fixed decimals per unit would print either a `1.1B` and a `1.4B`
/// that hide 300 million between them, or a `616.00M` that is two digits of
/// noise. Trailing zeros are dropped, so a round number reads as `4k` rather
/// than `4.00k`.
fn compact(value: f64) -> String {
    let magnitude = value.abs();
    let (scaled, unit) = if magnitude >= 1e12 {
        (value / 1e12, "T")
    } else if magnitude >= 1e9 {
        (value / 1e9, "B")
    } else if magnitude >= 1e6 {
        (value / 1e6, "M")
    } else if magnitude >= 1e3 {
        (value / 1e3, "k")
    } else {
        // Below a thousand there is nothing to compact: print it whole.
        return format!("{:.0}", value);
    };

    let scaled_magnitude = scaled.abs();
    let digits = if scaled_magnitude >= 100.0 {
        0
    } else if scaled_magnitude >= 10.0 {
        1
    } else {
        2
    };
    format!("{}{}", trimmed(scaled, digits), unit)
}

/// A fixed-precision number with its trailing zeros — and any bare decimal
/// point left behind — removed.
fn trimmed(value: f64, digits: usize) -> String {
    let text = format!("{:.*}", digits, value);
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// Thousands separators for a whole number, so exact counts stay scannable.
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A cost, at the precision the amount deserves: cents once there are dollars
/// to count, more decimals as the amount shrinks below one, and the compact
/// ladder once an exact figure would be a wall of digits.
pub fn format_cost(usd: f64) -> String {
    let magnitude = usd.abs();
    if magnitude >= 1e6 {
        format!("${}", compact(usd))
    } else if magnitude >= 1000.0 {
        // Rounded to cents before it is split, so $1,000.999 does not print as
        // $1,000.00.
        let cents = (magnitude * 100.0).round() as u64;
        let sign = if usd < 0.0 { "-" } else { "" };
        format!("{}${}.{:02}", sign, grouped(cents / 100), cents % 100)
    } else if magnitude >= 1.0 {
        format!("${:.2}", usd)
    } else if magnitude >= 0.01 {
        format!("${:.3}", usd)
    } else if magnitude > 0.0 {
        format!("${:.4}", usd)
    } else {
        "$0".to_owned()
    }
}

/// A cost with the digits traded away for width — for axis ticks, where a
/// `$11,093.03` would swallow the gutter.
pub fn format_cost_compact(usd: f64) -> String {
    let magnitude = usd.abs();
    if usd == 0.0 {
        "$0".to_owned()
    } else if magnitude >= 1000.0 {
        format!("${}", compact(usd))
    } else if magnitude >= 1.0 && usd.fract() == 0.0 {
        // Axis ticks land on round numbers: cents there are two dead columns.
        format!("${:.0}", usd)
    } else {
        format_cost(usd)
    }
}

/// A token count on the shared ladder. Takes an `f64` so per-day averages and
/// whole counts print identically.
pub fn format_tokens_compact(value: f64) -> String {
    compact(value)
}

/// A plain count — events, sessions — kept exact, since these are small enough
/// to read and rounding them says less than the digits do.
pub fn format_count(count: u64) -> String {
    grouped(count)
}

/// A value in the units the page is currently measured in.
pub fn format_metric(metric: UsageMetric, value: f64) -> String {
    match metric {
        UsageMetric::Cost => format_cost(value),
        UsageMetric::Tokens => format_tokens_compact(value),
    }
}

/// The same value, narrowed for axis ticks and other tight quarters.
pub fn format_metric_compact(metric: UsageMetric, value: f64) -> String {
    match metric {
        UsageMetric::Cost => format_cost_compact(value),
        UsageMetric::Tokens => format_tokens_compact(value),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_counts_climb_the_unit_ladder() {
        assert_eq!(format_tokens_compact(940.0), "940");
        assert_eq!(format_tokens_compact(4_000.0), "4k");
        assert_eq!(format_tokens_compact(71_820.0), "71.8k");
        assert_eq!(format_tokens_compact(615_800_000.0), "616M");
        assert_eq!(format_tokens_compact(8_804_300_000.0), "8.8B");
        assert_eq!(format_tokens_compact(2_500_000_000_000.0), "2.5T");
    }

    #[test]
    fn precision_follows_the_leading_digits_not_the_unit() {
        // Two counts 300 million apart have to stay apart on screen.
        assert_eq!(format_tokens_compact(1_130_000_000.0), "1.13B");
        assert_eq!(format_tokens_compact(1_430_000_000.0), "1.43B");
    }

    #[test]
    fn costs_keep_their_cents_and_group_their_thousands() {
        assert_eq!(format_cost(0.0), "$0");
        assert_eq!(format_cost(0.0012), "$0.0012");
        assert_eq!(format_cost(0.717), "$0.717");
        assert_eq!(format_cost(12.3), "$12.30");
        assert_eq!(format_cost(11_093.034), "$11,093.03");
        // Rounding happens before the split, not after it.
        assert_eq!(format_cost(1_000.999), "$1,001.00");
        // Past a million the exact figure is a wall of digits, so it compacts.
        assert_eq!(format_cost(2_500_000.0), "$2.5M");
    }

    #[test]
    fn an_axis_tick_trades_digits_for_width() {
        assert_eq!(format_cost_compact(0.0), "$0");
        assert_eq!(format_cost_compact(4.0), "$4");
        assert_eq!(format_cost_compact(4_000.0), "$4k");
        assert_eq!(format_cost_compact(11_093.03), "$11.1k");
    }

    #[test]
    fn exact_counts_stay_exact() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(1_051), "1,051");
        assert_eq!(format_count(71_820), "71,820");
    }

    #[test]
    fn a_metric_formats_in_its_own_unit() {
        assert_eq!(format_metric(UsageMetric::Cost, 11_093.03), "$11,093.03");
        assert_eq!(format_metric(UsageMetric::Tokens, 8.8043e9), "8.8B");
        assert_eq!(format_metric_compact(UsageMetric::Cost, 4000.0), "$4k");
    }
}
