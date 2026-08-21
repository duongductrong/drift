mod core;
mod keymap;
mod settings;
mod theme;
mod ui;

use gpui::{
    prelude::*, px, size, App, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowOptions,
};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        // Publishes both the settings and the theme they select.
        settings::init(cx);
        // Actions, their key bindings, and the macOS menu bar. Before the
        // window opens, so the first frame already has them.
        keymap::init(cx);
        // Unbundled binaries start behind whatever launched them, which would
        // leave the menu bar showing another app's.
        cx.activate(true);

        let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    // Kept for the Window menu and Mission Control; the visible
                    // title is drawn by our own toolbar.
                    title: Some("Mole".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: cfg!(target_os = "macos")
                        .then(ui::title_bar::traffic_light_position),
                }),
                // The toolbar is our titlebar: it drags the window itself via
                // `Window::start_window_move`, so AppKit must not also claim
                // titlebar drags (which would otherwise delay clicks in the
                // toolbar while it disambiguates double-clicks).
                app_owns_titlebar_drag: cfg!(target_os = "macos"),
                window_background: if cfg!(target_os = "macos") {
                    WindowBackgroundAppearance::Blurred
                } else {
                    WindowBackgroundAppearance::Opaque
                },
                window_min_size: Some(size(px(640.0), px(400.0))),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ui::app_view::AppView::new(window, cx)),
        )
        .unwrap();
    });
}
