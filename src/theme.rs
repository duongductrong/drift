use gpui::{App, Global, Hsla, WindowAppearance, hsla, rgb};

/// Waku-aligned global theme — neutral graphite surfaces with color reserved
/// for meaning. Brand coral marks live activity; blue fills meters and gauges.
/// Accessed via `Theme::current(cx)` from any render context.
#[derive(Clone, Copy)]
pub struct Theme {
    pub is_dark: bool,

    // Surfaces
    pub canvas: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub inset: Hsla,
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

    // Semantic
    pub accent: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,

    // Interactive surfaces
    pub inverse: Hsla,
    pub on_inverse: Hsla,
    pub gauge: Hsla,

    // App-specific: provider chart colors
    pub chart_claude: Hsla,
    pub chart_codex: Hsla,
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
            is_dark: true,

            canvas: rgb(0x1A1A1A).into(),
            surface: rgb(0x1A1A1A).into(),
            raised: rgb(0x232323).into(),
            inset: rgb(0x151515).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.90, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.90, 0.07),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.90, 0.14),

            text: rgb(0xE2E2E2).into(),
            text_secondary: rgb(0xA3A3A3).into(),
            text_tertiary: rgb(0x7D7D7D).into(),
            text_ghost: rgb(0x575757).into(),

            accent: rgb(0xE2795B).into(),
            success: rgb(0x62C987).into(),
            warning: rgb(0xE0B36A).into(),
            danger: rgb(0xE2726A).into(),

            inverse: rgb(0xE7E9EC).into(),
            on_inverse: rgb(0x17181C).into(),
            gauge: rgb(0x3B82F6).into(),

            chart_claude: rgb(0xD97757).into(),
            chart_codex: rgb(0x62C987).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,

            canvas: rgb(0xF6F5F6).into(),
            surface: rgb(0xF6F5F6).into(),
            raised: rgb(0xECECEC).into(),
            inset: rgb(0xE6E6E6).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.12, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.09),

            border: hsla(220.0 / 360.0, 0.10, 0.12, 0.08),
            border_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.15),

            text: rgb(0x242424).into(),
            text_secondary: rgb(0x666666).into(),
            text_tertiary: rgb(0x858585).into(),
            text_ghost: rgb(0xA4A4A4).into(),

            accent: rgb(0xC85F44).into(),
            success: rgb(0x2F8F52).into(),
            warning: rgb(0xA66B20).into(),
            danger: rgb(0xC64A42).into(),

            inverse: rgb(0x202227).into(),
            on_inverse: rgb(0xF8F8F9).into(),
            gauge: rgb(0x2563EB).into(),

            chart_claude: rgb(0xC85F44).into(),
            chart_codex: rgb(0x2F8F52).into(),
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
