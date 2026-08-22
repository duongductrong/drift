use gpui::{div, prelude::*, px, App, AnyElement, SharedString};

use crate::core::update::{self, Channel, CheckState};
use crate::settings::SettingsChange;
use crate::theme::Theme;

use crate::ui::components::Button;

use super::super::context::PaneContext;
use super::super::controls::{row, segmented, toggle, Segment};

/// The only network feature in the app, so it says what it does: the toggle
/// governs the check on launch, and the button asks on demand even when the
/// toggle is off.
pub(in crate::ui::settings_dialog) fn rows(ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    let channel_options = Channel::ALL
        .into_iter()
        .map(|channel| {
            Segment::new(
                channel.label(),
                channel == ctx.settings.update_channel,
                ctx.emit(SettingsChange::UpdateChannel(channel)),
            )
        })
        .collect();

    vec![
        row(
            "Check on launch",
            "Asks GitHub for the latest release. Nothing else is sent.",
            toggle(
                "settings-check-updates",
                ctx.settings.check_for_updates,
                ctx.emit(SettingsChange::CheckForUpdates(
                    !ctx.settings.check_for_updates,
                )),
                cx,
            )
            .into_any_element(),
            cx,
        ),
        row(
            "Channel",
            "Beta also offers pre-releases; Stable never does.",
            segmented("settings-update-channel", channel_options, cx).into_any_element(),
            cx,
        ),
        status_row(ctx, cx),
    ]
}

/// The status line, with the button that acts on it.
///
/// Which button that is *is* the state: an offer becomes "Download", anything
/// else stays "Check now", and a check in flight disables both so it cannot be
/// started twice.
fn status_row(ctx: &PaneContext, cx: &App) -> AnyElement {
    let theme = Theme::current(cx);
    let state: &CheckState = ctx.update_state;
    let summary = SharedString::from(state.summary(&update::current_version()));
    let checking = state.is_checking();

    let action = match state.available() {
        Some(_) => Button::new("settings-update-download", "Download").on_click(ctx.download()),
        None => Button::new("settings-update-check", "Check now")
            .subtle()
            .on_click(ctx.check_updates()),
    };

    div()
        .py(px(7.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(11.5))
                .text_color(if state.available().is_some() {
                    theme.text
                } else {
                    theme.text_secondary
                })
                .truncate()
                .child(summary),
        )
        .child(
            div()
                .flex_none()
                // A check in flight has nothing to click: the line above
                // already says so, and a second request would only race it.
                .when(checking, |el| el.opacity(0.5))
                .when(!checking, |el| el.child(action)),
        )
        .into_any_element()
}
