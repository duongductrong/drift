use gpui::{App, Global, Hsla, WindowAppearance, hsla, rgb};

/// Waku-aligned global theme — neutral graphite surfaces with color reserved
/// for the provider chart series. Accessed via `Theme::current(cx)` from any
/// render context.
#[derive(Clone, Copy)]
pub struct Theme {
    // Surfaces
    pub canvas: Hsla,
    pub surface: Hsla,
    pub overlay: Hsla,
    pub overlay_strong: Hsla,

    // Borders
    pub border: Hsla,
    pub border_strong: Hsla,

    // Text hierarchy (4 levels, matching Waku)
    pub text: Hsla,
    pub text_secondary: Hsla,
    pub text_tertiary: Hsla,
    pub text_ghost: Hsla,

    // Interactive surfaces
    pub inverse: Hsla,
    pub on_inverse: Hsla,

    // App-specific: provider chart colors
    pub chart_claude: Hsla,
    pub chart_codex: Hsla,
    pub chart_kimi: Hsla,
    pub chart_opencode: Hsla,
    pub chart_antigravity: Hsla,
}

#[derive(Clone, Copy)]
struct ActiveTheme(Theme);

impl Global for ActiveTheme {}

impl Theme {
    /// Read the published theme from GPUI globals. Falls back to dark if
    /// `init` hasn't been called yet.
    pub fn current(cx: &App) -> Self {
        if cx.has_global::<ActiveTheme>() {
            cx.global::<ActiveTheme>().0
        } else {
            Self::dark()
        }
    }

    pub fn dark() -> Self {
        Self {
            canvas: rgb(0x1A1A1A).into(),
            surface: rgb(0x1A1A1A).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.90, 0.07),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.14),

            text: rgb(0xE2E2E2).into(),
            text_secondary: rgb(0xA3A3A3).into(),
            text_tertiary: rgb(0x7D7D7D).into(),
            text_ghost: rgb(0x575757).into(),

            inverse: rgb(0xE7E9EC).into(),
            on_inverse: rgb(0x17181C).into(),

            chart_claude: rgb(0xD97757).into(),
            chart_codex: rgb(0x62C987).into(),
            chart_kimi: rgb(0x8B5CF6).into(),
            chart_opencode: rgb(0x06B6D4).into(),
            chart_antigravity: rgb(0x4285F4).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            canvas: rgb(0xF6F5F6).into(),
            surface: rgb(0xF6F5F6).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.12, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.12, 0.08),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.15),

            text: rgb(0x242424).into(),
            text_secondary: rgb(0x666666).into(),
            text_tertiary: rgb(0x858585).into(),
            text_ghost: rgb(0xA4A4A4).into(),

            inverse: rgb(0x202227).into(),
            on_inverse: rgb(0xF8F8F9).into(),

            chart_claude: rgb(0xC85F44).into(),
            chart_codex: rgb(0x2F8F52).into(),
            chart_kimi: rgb(0x7C3AED).into(),
            chart_opencode: rgb(0x0891B2).into(),
            chart_antigravity: rgb(0x1A73E8).into(),
        }
    }
}

/// Resolve system appearance and publish the startup theme. Call once from
/// `main` before opening any window.
pub fn init(cx: &mut App) {
    let appearance = cx.window_appearance();
    let is_dark = matches!(
        appearance,
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    );
    let theme = if is_dark { Theme::dark() } else { Theme::light() };
    cx.set_global(ActiveTheme(theme));
}
