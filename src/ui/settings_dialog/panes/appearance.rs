use gpui::{App, AnyElement, IntoElement};

use crate::settings::SettingsChange;
use crate::theme::ThemeMode;

use super::super::context::PaneContext;
use super::super::controls::{row, segmented, toggle, Segment};

/// How the app is painted.
pub(in crate::ui::settings_dialog) fn rows(ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    let options = ThemeMode::ALL
        .into_iter()
        .map(|mode| {
            Segment::new(
                mode.label(),
                mode == ctx.settings.theme,
                ctx.emit(SettingsChange::Theme(mode)),
            )
        })
        .collect();

    vec![
        row(
            "Theme",
            "System follows your OS appearance.",
            segmented("settings-theme", options, cx).into_any_element(),
            cx,
        ),
        row(
            "Transparency",
            "Let the blurred desktop show through the window.",
            toggle(
                "settings-transparency",
                ctx.settings.transparency,
                ctx.emit(SettingsChange::Transparency(!ctx.settings.transparency)),
                cx,
            )
            .into_any_element(),
            cx,
        ),
    ]
}
