use gpui::{
    div, prelude::*, px, AnyElement, App, BorderStyle, Bounds, CursorStyle, DispatchPhase, Entity,
    HitboxBehavior, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Pixels, ScrollHandle, Window, canvas, point, quad, size,
};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// ScrollArea — a reusable scrollable container with consistent styling.
//
// Provides vertical overflow scrolling, standard padding, and a bottom inset
// so the last child is never flush against the window edge.
//
// Usage:
//   ScrollArea::new("my-scroll")
//       .child(expensive_content)
//       .child(more_content)
//
// The scroll container fills its parent (`size_full()`), has `px(20)`
// horizontal padding, and `px(48)` bottom padding so trailing content never
// sits behind a footer.
//
// The scrollbar thumb is always shown: whenever the content overflows, a
// thumb rides the right edge, thickening while the pointer is over its track
// and draggable to scroll.
//
// Scroll position and pointer state are keyed to the area's id and kept in
// GPUI element state, so callers hold nothing on their own view.
// ---------------------------------------------------------------------------

// ── Scrollbar metrics ─────────────────────────────────────────────────────

/// Width of the track the pointer has to hit to engage the scrollbar.
const TRACK_WIDTH: f32 = 12.0;
/// Distance kept between the thumb and the track's edges.
const TRACK_PADDING: f32 = 3.0;
/// Resting thumb width.
const THUMB_WIDTH: f32 = 6.0;
/// Thumb width while the pointer is on the track or a drag is in progress.
const THUMB_WIDTH_ENGAGED: f32 = 9.0;
/// Shortest the thumb may get, so very long content stays grabbable.
const THUMB_MIN_HEIGHT: f32 = 24.0;
/// Opacity applied to the resting thumb color.
const THUMB_RESTING_OPACITY: f32 = 0.6;

// ── Cross-frame state ─────────────────────────────────────────────────────

/// Everything a `ScrollArea` has to remember between frames. Stored as GPUI
/// element state under the area's id, so a `ScrollArea` rebuilt every render
/// keeps its scroll position, hover, and drag.
struct ScrollAreaState {
    scroll: ScrollHandle,
    /// True while the pointer sits anywhere on the scrollbar track.
    pointer_on_track: bool,
    /// While dragging, how far below the thumb's top edge the pointer grabbed.
    grab: Option<Pixels>,
}

impl ScrollAreaState {
    fn new() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            pointer_on_track: false,
            grab: None,
        }
    }

    /// The thumb is drawn emphasized whenever the pointer is on the track or
    /// the user is dragging it — a drag keeps it emphasized even once the
    /// pointer wanders off the track.
    fn engaged(&self) -> bool {
        self.pointer_on_track || self.grab.is_some()
    }
}

// ── Thumb geometry ────────────────────────────────────────────────────────

/// Where the thumb sits this frame, plus what it takes to map pointer
/// positions back onto scroll offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ThumbLayout {
    bounds: Bounds<Pixels>,
    /// Top of the region the thumb travels through (the track, less padding).
    rail_top: Pixels,
    /// Vertical distance the thumb can travel.
    travel: Pixels,
    /// Content height beyond the viewport.
    max_offset: Pixels,
}

impl ThumbLayout {
    /// The scroll offset that puts the thumb's top edge at `top`, clamped to
    /// the scrollable range.
    fn offset_for_top(&self, top: Pixels) -> Pixels {
        if self.travel <= Pixels::ZERO {
            return Pixels::ZERO;
        }
        self.max_offset * ((top - self.rail_top) / self.travel).clamp(0.0, 1.0)
    }
}

/// Lay the thumb out inside `track`, or return `None` when the content fits
/// and there is nothing to scroll. Pure, so the mapping is unit-testable.
fn thumb_layout(
    track: Bounds<Pixels>,
    viewport_height: Pixels,
    max_offset: Pixels,
    offset: Pixels,
    thumb_width: Pixels,
) -> Option<ThumbLayout> {
    let padding = px(TRACK_PADDING);
    let rail_height = track.size.height - padding * 2.0;
    if viewport_height <= Pixels::ZERO || max_offset <= px(0.5) || rail_height <= Pixels::ZERO {
        return None;
    }

    // The thumb covers as much of the rail as the viewport covers of the
    // content, down to a floor that keeps it clickable.
    let content_height = viewport_height + max_offset;
    let thumb_height = (rail_height * (viewport_height / content_height))
        .max(px(THUMB_MIN_HEIGHT))
        .min(rail_height);
    let travel = (rail_height - thumb_height).max(Pixels::ZERO);
    let rail_top = track.top() + padding;
    let progress = (offset / max_offset).clamp(0.0, 1.0);

    Some(ThumbLayout {
        bounds: Bounds::new(
            point(
                track.right() - padding - thumb_width,
                rail_top + travel * progress,
            ),
            size(thumb_width, thumb_height),
        ),
        rail_top,
        travel,
        max_offset,
    })
}

/// Scroll the surface to `offset` pixels down, reporting whether it moved.
fn scroll_to(handle: &ScrollHandle, offset: Pixels) -> bool {
    let current = handle.offset();
    if (current.y + offset).abs() <= px(0.01) {
        return false;
    }
    handle.set_offset(point(current.x, -offset));
    true
}

// ── ScrollArea ────────────────────────────────────────────────────────────

/// Vertical gap between children.
const CHILD_GAP: f32 = 20.0;
/// Horizontal padding on the scrolling surface.
const SURFACE_PX: f32 = 20.0;
/// Bottom inset so trailing content never sits behind a footer.
const BOTTOM_INSET: f32 = 48.0;

#[derive(IntoElement)]
pub struct ScrollArea {
    id: &'static str,
    children: Vec<AnyElement>,
}

impl ScrollArea {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            children: Vec::new(),
        }
    }

    /// Add a child element.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Keyed by the caller's id, with an index that keeps this key clear of
        // the content div's own id-scoped element state.
        let state = window.use_keyed_state((self.id, 0usize), cx, |_, _| ScrollAreaState::new());
        let scroll = state.read(cx).scroll.clone();

        let mut content = div()
            .id(self.id)
            .size_full()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(CHILD_GAP))
            .overflow_y_scroll()
            .px(px(SURFACE_PX))
            .pb(px(BOTTOM_INSET))
            .track_scroll(&scroll);

        for child in self.children {
            content = content.child(child);
        }

        // The scrollbar is a sibling of the scrolling surface rather than one
        // of its children: children are translated by the scroll offset, which
        // would carry the thumb off with the content.
        div()
            .relative()
            .size_full()
            .min_h_0()
            .child(content)
            .child(scrollbar(scroll, state))
    }
}

// ── Scrollbar overlay ─────────────────────────────────────────────────────

/// An overlay scrollbar pinned to the right edge of the (relative) parent. It
/// takes no part in layout, so showing it never reflows the content.
fn scrollbar(scroll: ScrollHandle, state: Entity<ScrollAreaState>) -> impl IntoElement {
    let paint_scroll = scroll.clone();
    let paint_state = state.clone();

    canvas(
        // Prepaint runs after the scrolling sibling has been laid out, so the
        // handle reports this frame's viewport, extent, and (clamped) offset —
        // window resizes land on the thumb immediately rather than a frame late.
        move |track, window, cx| {
            let thumb_width = px(if state.read(cx).engaged() {
                THUMB_WIDTH_ENGAGED
            } else {
                THUMB_WIDTH
            });
            let layout = thumb_layout(
                track,
                scroll.bounds().size.height,
                scroll.max_offset().y,
                -scroll.offset().y,
                thumb_width,
            )?;
            Some((layout, window.insert_hitbox(track, HitboxBehavior::Normal)))
        },
        move |_, layout, window: &mut Window, cx: &mut App| {
            let (scroll, state) = (paint_scroll, paint_state);

            let Some((layout, hitbox)) = layout else {
                // Nothing to scroll — the window may have just grown past the
                // content. Drop any drag so a later resize cannot resume it.
                if state.read(cx).grab.is_some() {
                    state.update(cx, |state, cx| {
                        state.grab = None;
                        cx.notify();
                    });
                }
                return;
            };

            let theme = Theme::current(cx);
            let engaged = state.read(cx).engaged();
            let color = if engaged {
                theme.text_tertiary
            } else {
                theme.text_ghost.opacity(THUMB_RESTING_OPACITY)
            };

            window.paint_quad(quad(
                layout.bounds,
                layout.bounds.size.width / 2.0,
                color,
                px(0.0),
                gpui::transparent_black(),
                BorderStyle::default(),
            ));
            window.set_cursor_style(CursorStyle::Arrow, &hitbox);

            let view = window.current_view();

            // One move listener covers both jobs: tracking whether the pointer
            // is on the track, and driving an in-flight drag. A drag has to be
            // followed even when the pointer leaves the track, so this is a
            // window-level listener rather than an interactive child.
            window.on_mouse_event({
                let scroll = scroll.clone();
                let state = state.clone();
                let hitbox = hitbox.clone();
                move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    if let Some(grab) = state.read(cx).grab
                        && scroll_to(&scroll, layout.offset_for_top(event.position.y - grab))
                    {
                        cx.notify(view);
                    }
                    let on_track = hitbox.is_hovered(window);
                    if state.read(cx).pointer_on_track != on_track {
                        state.update(cx, |state, cx| {
                            state.pointer_on_track = on_track;
                            cx.notify();
                        });
                    }
                }
            });

            window.on_mouse_event({
                let scroll = scroll.clone();
                let state = state.clone();
                let hitbox = hitbox.clone();
                move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble
                        || event.button != MouseButton::Left
                        || !hitbox.is_hovered(window)
                    {
                        return;
                    }
                    let grab = if layout.bounds.contains(&event.position) {
                        event.position.y - layout.bounds.top()
                    } else {
                        // A press on bare track centers the thumb under the
                        // pointer and carries on as a drag from its middle.
                        let half = layout.bounds.size.height / 2.0;
                        scroll_to(&scroll, layout.offset_for_top(event.position.y - half));
                        half
                    };
                    state.update(cx, |state, cx| {
                        state.grab = Some(grab);
                        cx.notify();
                    });
                    cx.stop_propagation();
                }
            });

            window.on_mouse_event({
                let state = state.clone();
                move |_: &MouseUpEvent, phase, _window, cx| {
                    if phase != DispatchPhase::Bubble || state.read(cx).grab.is_none() {
                        return;
                    }
                    state.update(cx, |state, cx| {
                        state.grab = None;
                        cx.notify();
                    });
                }
            });
        },
    )
    .absolute()
    .top_0()
    .right_0()
    .h_full()
    .w(px(TRACK_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 400px-tall track at the right edge of a viewport.
    fn track() -> Bounds<Pixels> {
        Bounds::new(point(px(600.0), px(50.0)), size(px(TRACK_WIDTH), px(400.0)))
    }

    fn layout_at(offset: Pixels) -> ThumbLayout {
        thumb_layout(track(), px(400.0), px(1200.0), offset, px(THUMB_WIDTH)).unwrap()
    }

    #[test]
    fn content_that_fits_has_no_thumb() {
        assert!(thumb_layout(track(), px(400.0), Pixels::ZERO, Pixels::ZERO, px(6.0)).is_none());
        // Sub-pixel overflow from rounding is not worth a thumb either.
        assert!(thumb_layout(track(), px(400.0), px(0.4), Pixels::ZERO, px(6.0)).is_none());
        // A collapsed viewport (mid-resize) has nothing to measure against.
        assert!(thumb_layout(track(), Pixels::ZERO, px(900.0), Pixels::ZERO, px(6.0)).is_none());
    }

    #[test]
    fn thumb_covers_the_visible_fraction_of_the_rail() {
        // The viewport is a quarter of the content, so the thumb is a quarter
        // of the rail — the track less its padding at each end.
        let rail = px(400.0) - px(TRACK_PADDING) * 2.0;
        let layout = layout_at(Pixels::ZERO);
        assert_eq!(layout.bounds.size.height, rail / 4.0);
        assert_eq!(layout.bounds.top(), track().top() + px(TRACK_PADDING));
        assert_eq!(layout.travel, rail - rail / 4.0);
    }

    #[test]
    fn a_long_document_still_leaves_a_grabbable_thumb() {
        let layout = thumb_layout(track(), px(400.0), px(200_000.0), Pixels::ZERO, px(6.0)).unwrap();
        assert_eq!(layout.bounds.size.height, px(THUMB_MIN_HEIGHT));
    }

    #[test]
    fn the_thumb_sits_within_the_padded_track() {
        let layout = layout_at(px(600.0));
        assert_eq!(
            layout.bounds.right(),
            track().right() - px(TRACK_PADDING)
        );
        assert!(layout.bounds.top() >= track().top() + px(TRACK_PADDING));
        assert!(layout.bounds.bottom() <= track().bottom() - px(TRACK_PADDING) + px(0.001));
    }

    #[test]
    fn thumb_position_and_scroll_offset_are_inverse() {
        // Halfway down the content puts the thumb halfway along its travel.
        let layout = layout_at(px(600.0));
        assert_eq!(layout.bounds.top(), layout.rail_top + layout.travel / 2.0);
        assert_eq!(layout.offset_for_top(layout.bounds.top()), px(600.0));
    }

    #[test]
    fn dragging_past_either_end_clamps() {
        let layout = layout_at(Pixels::ZERO);
        assert_eq!(layout.offset_for_top(px(-9_999.0)), Pixels::ZERO);
        assert_eq!(layout.offset_for_top(px(9_999.0)), px(1200.0));
    }

    #[test]
    fn an_overscrolled_offset_does_not_push_the_thumb_off_the_rail() {
        // Momentum overscroll can report more than max for a frame.
        let layout = layout_at(px(5_000.0));
        assert_eq!(layout.bounds.top(), layout.rail_top + layout.travel);
    }
}
