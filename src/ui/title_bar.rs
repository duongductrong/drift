use gpui::{
    div, prelude::*, point, px, AnyElement, App, MouseButton, Pixels, Point, Window,
    WindowControlArea,
};

// ---------------------------------------------------------------------------
// Window chrome geometry
//
// The window hides the native titlebar (`appears_transparent`), so the toolbar
// below stands in for it: it draws the title, hosts the toolbar controls, and
// owns window dragging. Because the traffic lights then float inside our own
// content area, their placement and the toolbar's leading padding have to be
// derived from the same numbers — otherwise the title drifts into the buttons.
//
// The button metrics are AppKit's, measured from `standardWindowButton`: a
// 14x14pt button, with the gap between buttons widening from 6pt to 9pt in the
// macOS 26 SDK. They are fixed-size and deliberately do not scale with the
// app's text sizes.
// ---------------------------------------------------------------------------

/// Height of the toolbar that stands in for the native titlebar.
pub const TOOLBAR_HEIGHT: f32 = 52.0;

/// Horizontal padding on the toolbar's trailing edge — and on its leading edge
/// on platforms that have no traffic lights to clear.
pub const TOOLBAR_PADDING: f32 = 20.0;

/// Whether we were built against the macOS 26 SDK or later. Set by `build.rs`.
const MACOS_SDK_26_OR_LATER: bool = cfg!(macos_sdk_26_or_later);

/// Side length of a single traffic-light button.
const TRAFFIC_LIGHT_SIZE: f32 = 14.0;

/// Gap between adjacent traffic-light buttons.
const TRAFFIC_LIGHT_GAP: f32 = if MACOS_SDK_26_OR_LATER { 9.0 } else { 6.0 };

/// Distance from the window's leading edge to the close button's leading edge.
/// Matches [`TOOLBAR_PADDING`] so the chrome reads as evenly inset on both
/// sides, and lands close enough to the vertical inset below that the cluster
/// looks optically centered in its corner.
const TRAFFIC_LIGHT_INSET: f32 = TOOLBAR_PADDING;

/// Leading padding the toolbar reserves so its title clears the traffic
/// lights: the inset, the three buttons and the two gaps between them, plus one
/// more gap of breathing room before the title starts.
const TRAFFIC_LIGHT_CLEARANCE: f32 =
    TRAFFIC_LIGHT_INSET + 3.0 * TRAFFIC_LIGHT_SIZE + 3.0 * TRAFFIC_LIGHT_GAP;

/// Where AppKit should place the traffic lights, as an inset from the window's
/// top-left corner.
///
/// GPUI sizes the button container to `button_height + 2 * y` and pins it to the
/// top of the window, so the buttons end up centered on `y + button_height / 2`.
/// Solving that for the toolbar's midline is what vertically centers the
/// cluster against the title next to it.
pub fn traffic_light_position() -> Point<Pixels> {
    point(
        px(TRAFFIC_LIGHT_INSET),
        px((TOOLBAR_HEIGHT - TRAFFIC_LIGHT_SIZE) / 2.0),
    )
}

// ---------------------------------------------------------------------------
// Toolbar — the custom titlebar.
//
// Elements added via `left` are titlebar content: they are inert, so dragging
// across them moves the window, exactly like dragging a native title. Elements
// added via `right` are controls: they swallow the press so clicking a button
// never starts a window move.
//
// Usage:
//   Toolbar::new()
//       .left(title_element)
//       .right(button_element)
// ---------------------------------------------------------------------------

/// Tracks a left press on the drag region that has not yet turned into a drag.
///
/// Handing the window to the platform on press would swallow the click, so we
/// wait for the pointer to actually move. Lives in element state, so it resets
/// with the window rather than leaking across windows.
#[derive(Default)]
struct DragRegionState {
    pending_move: bool,
}

#[derive(IntoElement)]
pub struct Toolbar {
    left: Vec<AnyElement>,
    right: Vec<AnyElement>,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    /// Add titlebar content to the left side. This area is draggable.
    pub fn left(mut self, element: impl IntoElement) -> Self {
        self.left.push(element.into_any_element());
        self
    }

    /// Add a control to the right side. This area is not draggable.
    pub fn right(mut self, element: impl IntoElement) -> Self {
        self.right.push(element.into_any_element());
        self
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Fullscreen restores the traffic lights to AppKit's own titlebar, so
        // there is nothing left in the content area to clear.
        let leading_padding = if cfg!(target_os = "macos") && !window.is_fullscreen() {
            TRAFFIC_LIGHT_CLEARANCE
        } else {
            TOOLBAR_PADDING
        };

        let drag_region =
            window.use_keyed_state("toolbar-drag-region", cx, |_, _| DragRegionState::default());

        let mut title_group = div().flex().items_center().gap(px(10.0));
        for element in self.left {
            title_group = title_group.child(element);
        }

        // Claim the press before it reaches the drag region below. Mouse
        // listeners bubble from the innermost element outwards, so this runs
        // first and keeps a button click from being read as a window drag.
        let mut control_group = div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for element in self.right {
            control_group = control_group.child(element);
        }

        div()
            .id("toolbar")
            .flex_none()
            .h(px(TOOLBAR_HEIGHT))
            .pl(px(leading_padding))
            .pr(px(TOOLBAR_PADDING))
            .flex()
            .items_center()
            .justify_between()
            // Deliberately paints neither a background nor a bottom border, so
            // the toolbar and the content below it read as one continuous
            // surface — the root view owns the background for both.
            //
            // Declares the drag region to platforms that route window moves
            // through their own compositor rather than through the events below.
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, {
                let drag_region = drag_region.clone();
                move |_, _, cx| drag_region.update(cx, |state, _| state.pending_move = true)
            })
            .on_mouse_up(MouseButton::Left, {
                let drag_region = drag_region.clone();
                move |_, _, cx| drag_region.update(cx, |state, _| state.pending_move = false)
            })
            .on_mouse_down_out({
                let drag_region = drag_region.clone();
                move |_, _, cx| drag_region.update(cx, |state, _| state.pending_move = false)
            })
            // Only now, once the pointer has actually moved, hand the window to
            // the platform — a press that never moves stays a plain click.
            .on_mouse_move({
                let drag_region = drag_region.clone();
                move |_, window, cx| {
                    if drag_region.read(cx).pending_move {
                        drag_region.update(cx, |state, _| state.pending_move = false);
                        window.start_window_move();
                    }
                }
            })
            .when(cfg!(target_os = "macos"), |toolbar| {
                // Honors the "double-click a window's title bar to" system
                // setting, which the hidden native titlebar no longer can.
                toolbar.on_click(|event, window, _| {
                    if event.click_count() == 2 {
                        window.titlebar_double_click();
                    }
                })
            })
            .child(title_group)
            .child(control_group)
    }
}
