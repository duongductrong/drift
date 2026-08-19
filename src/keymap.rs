use gpui::{actions, App, KeyBinding};

#[cfg(target_os = "macos")]
use gpui::{Menu, MenuItem, SystemMenuType};

// ---------------------------------------------------------------------------
// Keymap — the app's actions, their key bindings, and the macOS menu bar.
//
// Every shortcut is an action declared here rather than a `on_key_down` buried
// in a view, for three reasons:
//
//   * AppKit takes a menu item's key equivalent from the binding registered
//     for its action, so declaring both in one place is what keeps a menu item
//     and its shortcut from drifting apart.
//   * A raw key handler sees *every* keystroke and has to recognise its own;
//     an action only fires for the binding, so nothing else is intercepted.
//   * Bindings carry a context predicate, so a key can mean one thing inside a
//     dialog and nothing outside it — see `SETTINGS_DIALOG_CONTEXT`.
//
// Handlers live next to the state they change. App-wide commands (hide, quit)
// need nothing but the `App` and are handled below; window commands (minimize,
// zoom, close, full screen) and view commands (refresh, settings) need a
// `Window` or the root view's state, so `ui::app_view` handles those.
//
// A caveat that shapes `ui::app_view`: GPUI dispatches an action along the
// *focus path*, from the window root down to the focused element. An element's
// handler is therefore only reachable while it is on that path, which is why
// the root view tracks focus and takes it back when the settings dialog
// closes. Actions handled here, on the `App`, are the exception — they run
// regardless of what is focused.
// ---------------------------------------------------------------------------

/// Key context the settings dialog declares while it is open, so `escape`
/// means "dismiss" there and stays unbound everywhere else.
pub const SETTINGS_DIALOG_CONTEXT: &str = "SettingsDialog";

actions!(
    drift,
    [
        /// Close the focused window.
        CloseWindow,
        /// Minimize the focused window into the Dock.
        Minimize,
        /// Toggle the focused window between its standard and zoomed size.
        Zoom,
        /// Toggle the focused window's full screen state.
        ToggleFullScreen,
        /// Rescan the providers' transcripts on disk.
        Refresh,
        /// Open the settings dialog.
        OpenSettings,
        /// Dismiss the frontmost dialog.
        Cancel,
        /// Hide the application.
        Hide,
        /// Hide every application except this one.
        HideOthers,
        /// Reveal every hidden application.
        ShowAll,
        /// Quit the application.
        Quit,
    ]
);

/// Register the key bindings, the app-wide action handlers, and — on macOS —
/// the menu bar. Call once, before the first window opens.
pub fn init(cx: &mut App) {
    // `secondary` is cmd on macOS and ctrl elsewhere, which is exactly how
    // each platform spells these four.
    cx.bind_keys([
        KeyBinding::new("secondary-w", CloseWindow, None),
        KeyBinding::new("secondary-m", Minimize, None),
        KeyBinding::new("secondary-r", Refresh, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("escape", Cancel, Some(SETTINGS_DIALOG_CONTEXT)),
    ]);

    cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

    // Closing the only window would otherwise leave the app running with
    // nothing on screen and no way back to it.
    cx.on_window_closed(|cx, _window_id| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();

    #[cfg(target_os = "macos")]
    {
        cx.bind_keys([
            KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("alt-cmd-h", HideOthers, None),
        ]);

        cx.on_action(|_: &Hide, cx: &mut App| cx.hide());
        cx.on_action(|_: &HideOthers, cx: &mut App| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx: &mut App| cx.unhide_other_apps());

        set_menus(cx);
    }

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([KeyBinding::new("f11", ToggleFullScreen, None)]);
}

/// Build the macOS menu bar.
///
/// No item is given an explicit key equivalent: AppKit reads those from the
/// bindings registered in [`init`], so the menu always shows what the keyboard
/// actually does.
#[cfg(target_os = "macos")]
fn set_menus(cx: &mut App) {
    cx.set_menus([
        // The leading menu is the application menu; macOS titles it with the
        // process name, so the name given here is never shown.
        Menu::new("Drift").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Drift", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit Drift", Quit),
        ]),
        // No full screen item here on purpose: AppKit appends its own to any
        // menu named "View", and unlike one of ours that item retitles itself
        // between "Enter" and "Exit". Adding a second would just duplicate it.
        // `ctrl-cmd-f` still reaches `ToggleFullScreen` through the keymap,
        // since no menu item claims that key equivalent.
        Menu::new("View").items([MenuItem::action("Refresh", Refresh)]),
        // Naming a menu "Window" is what makes AppKit adopt it as the Windows
        // menu and append the window list to it.
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
            MenuItem::separator(),
            MenuItem::action("Close Window", CloseWindow),
        ]),
    ]);
}
