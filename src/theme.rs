use gpui::{App, Global, Hsla, WindowAppearance, hsla, rgb};

/// Which palette the app paints with.
///
/// `System` follows the OS appearance; the other two pin the palette regardless
/// of it. Resolved to a concrete [`Theme`] by [`resolve`], so everything
/// downstream keeps reading a single published palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    pub fn label(&self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }
}

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
    /// Dimmed wash behind a modal sheet.
    pub scrim: Hsla,

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

    // App-specific: activity heatmap intensity ramp
    //
    // The one place in the palette where color means *how much* rather than
    // *which provider*. The grid it paints carries no provider legend and
    // never mixes hues, so a single ramp is free to reuse the brand's
    // terracotta without impersonating Claude's series color.
    /// A day the range covers but nothing happened on. Neutral gray, and
    /// deliberately hueless: the terracotta ramp is reserved for days with
    /// usage, so an empty cell must not read as "the quietest step" — it is
    /// no data at all. Gray rather than a near-canvas wash so quiet stretches
    /// still read as part of the grid instead of holes eaten out of it.
    pub heat_empty: Hsla,
    /// Four steps of rising usage. Each mode's steps are chosen against its own
    /// surface rather than flipped from the other's, and chroma climbs with
    /// lightness, so magnitude is carried by saturation as well as by tone.
    ///
    /// Both ramps are monotone in OKLCH lightness with every neighbouring pair
    /// at least 0.05 apart, and the top step clears 4.5:1 against its surface.
    pub heat: [Hsla; 4],
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
            scrim: hsla(220.0 / 360.0, 0.10, 0.06, 0.55),

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

            heat_empty: rgb(0x303030).into(),
            heat: [
                rgb(0x4B2F27).into(),
                rgb(0x794232).into(),
                rgb(0xAE5940).into(),
                rgb(0xE8724F).into(),
            ],
        }
    }

    pub fn light() -> Self {
        Self {
            canvas: rgb(0xF6F5F6).into(),
            surface: rgb(0xF6F5F6).into(),
            overlay: hsla(220.0 / 360.0, 0.10, 0.12, 0.05),
            overlay_strong: hsla(220.0 / 360.0, 0.10, 0.12, 0.09),
            scrim: hsla(220.0 / 360.0, 0.10, 0.20, 0.28),

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

            heat_empty: rgb(0xE4E4E4).into(),
            heat: [
                rgb(0xEAD3CE).into(),
                rgb(0xDBA99D).into(),
                rgb(0xCB7D69).into(),
                rgb(0xB8492C).into(),
            ],
        }
    }
}

/// Whether the OS is currently in a dark appearance.
fn system_is_dark(cx: &App) -> bool {
    matches!(
        cx.window_appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}

/// The palette `mode` asks for, with `System` resolved against the OS.
pub fn resolve(mode: ThemeMode, cx: &App) -> Theme {
    let is_dark = match mode {
        ThemeMode::System => system_is_dark(cx),
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    if is_dark { Theme::dark() } else { Theme::light() }
}

/// Publish the palette for `mode`. Call once before opening a window, and again
/// whenever the preference changes — every view reads the published palette via
/// [`Theme::current`], so republishing is all a theme switch takes.
pub fn apply(cx: &mut App, mode: ThemeMode) {
    let theme = resolve(mode, cx);
    cx.set_global(ActiveTheme(theme));
}
