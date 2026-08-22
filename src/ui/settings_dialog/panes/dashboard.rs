use gpui::{App, AnyElement, IntoElement};

use crate::core::types::TimeWindow;
use crate::settings::{SettingsChange, MODEL_ROW_OPTIONS};

use super::super::context::PaneContext;
use super::super::controls::{row, segmented, stacked_row, Segment};

/// What the dashboard opens on, and how much of it it lists.
pub(in crate::ui::settings_dialog) fn rows(ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    let range_options = TimeWindow::ALL
        .into_iter()
        .map(|range| {
            Segment::new(
                range.label(),
                range == ctx.settings.default_range,
                ctx.emit(SettingsChange::DefaultRange(range)),
            )
        })
        .collect();

    let row_options = MODEL_ROW_OPTIONS
        .into_iter()
        .map(|rows| {
            Segment::new(
                rows.to_string(),
                rows == ctx.settings.model_rows,
                ctx.emit(SettingsChange::ModelRows(rows)),
            )
        })
        .collect();

    vec![
        // Five ranges is wider than the pane has room for beside a label, so
        // this one sits on its own line.
        stacked_row(
            "Default range",
            "The range the dashboard opens on.",
            segmented("settings-range", range_options, cx).into_any_element(),
            cx,
        ),
        row(
            "Models listed",
            "Rows in the \"By Model\" breakdown.",
            segmented("settings-model-rows", row_options, cx).into_any_element(),
            cx,
        ),
    ]
}
