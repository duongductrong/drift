use gpui::{App, AnyElement, IntoElement, SharedString};

use crate::core::scanner;
use crate::core::types::Provider;
use crate::settings::SettingsChange;

use super::super::context::PaneContext;
use super::super::controls::{row, toggle};

/// Which assistants' transcripts are counted.
///
/// Every provider is listed even when its store is missing: seeing the path it
/// would read explains an empty row better than hiding it.
pub(in crate::ui::settings_dialog) fn rows(ctx: &PaneContext, cx: &App) -> Vec<AnyElement> {
    Provider::ALL
        .into_iter()
        .map(|provider| {
            let enabled = ctx.settings.is_provider_enabled(provider);
            row(
                provider.label(),
                source_label(provider),
                toggle(
                    SharedString::from(format!("settings-provider-{}", provider.label())),
                    enabled,
                    ctx.emit(SettingsChange::ToggleProvider(provider)),
                    cx,
                )
                .into_any_element(),
                cx,
            )
        })
        .collect()
}

/// The provider's store, with the home directory shortened to `~`.
fn source_label(provider: Provider) -> SharedString {
    let Some(path) = scanner::data_source(provider) else {
        return SharedString::from("No known location");
    };
    let display = path.to_string_lossy().to_string();
    let shortened = dirs::home_dir()
        .and_then(|home| {
            let home = home.to_string_lossy().to_string();
            display
                .strip_prefix(&home)
                .map(|rest| format!("~{}", rest))
        })
        .unwrap_or(display);
    SharedString::from(shortened)
}
