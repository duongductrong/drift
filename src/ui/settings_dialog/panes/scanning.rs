use gpui::{App, AnyElement, IntoElement};

use crate::settings::{ScanInterval, SettingsChange};

use super::super::context::PaneContext;
use super::super::controls::{row, segmented, stacked_row, toggle, Segment};

/// When the app reads the transcripts without being asked.
///
/// The two rows below are the whole answer: one for launch, one for the window
/// left open afterwards. The refresh button is governed by neither and works
/// even on "Off".
pub(in crate::ui::settings_dialog) fn rows(ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    let interval_options = ScanInterval::ALL
        .into_iter()
        .map(|interval| {
            Segment::new(
                interval.label(),
                interval == ctx.settings.scan_interval,
                ctx.emit(SettingsChange::ScanInterval(interval)),
            )
        })
        .collect();

    vec![
        row(
            "Scan on launch",
            "Off waits for the refresh button instead.",
            toggle(
                "settings-scan-on-launch",
                ctx.settings.scan_on_launch,
                ctx.emit(SettingsChange::ScanOnLaunch(!ctx.settings.scan_on_launch)),
                cx,
            )
            .into_any_element(),
            cx,
        ),
        stacked_row(
            "Automatic scan",
            "How often to rescan while the app is open.",
            segmented("settings-scan-interval", interval_options, cx).into_any_element(),
            cx,
        ),
    ]
}
