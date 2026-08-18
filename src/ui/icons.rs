use gpui::{prelude::*, svg, Hsla, Pixels, Svg};

// ---------------------------------------------------------------------------
// Icons — the app's line-art icon set, embedded in the binary.
//
// Each icon is a 24x24 stroked SVG rendered as an alpha mask and tinted with
// the element's text color, so a single file serves both themes. Embedding
// means no asset source to register and no files to ship beside the binary.
//
// Usage:
//   Icon::Refresh.element().size(px(14.0)).text_color(theme.text)
// ---------------------------------------------------------------------------

/// Nominal icon side length. Matches the 24-unit viewBox at a size the toolbar
/// and dialog both use, so strokes land on whole pixels.
pub const ICON_SIZE: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Rescan / reload.
    Refresh,
    /// Open settings.
    Settings,
    /// Dismiss a dialog.
    Close,
}

impl Icon {
    fn data(self) -> &'static [u8] {
        match self {
            Icon::Refresh => include_bytes!("../../assets/icons/refresh.svg"),
            Icon::Settings => include_bytes!("../../assets/icons/settings.svg"),
            Icon::Close => include_bytes!("../../assets/icons/close.svg"),
        }
    }

    /// The icon as an element of side `size`, tinted `color`.
    ///
    /// The tint has to be set on the SVG itself: it is painted as a mask, and a
    /// mask with no color of its own paints nothing — text color does not
    /// cascade into it from an ancestor.
    pub fn element(self, size: Pixels, color: Hsla) -> Svg {
        svg()
            .flex_none()
            .size(size)
            .text_color(color)
            .data(self.data())
    }
}
