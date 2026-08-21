use gpui::{div, prelude::*, px, rgb, App, Pixels, RenderOnce, Window};

// ---------------------------------------------------------------------------
// MoleMark — the Mole brand mark, drawn from plain divs.
//
// A faithful trace of the mascot art: every shape below sits at the position
// and proportion measured from the original artwork — the dome fitted to its
// silhouette, the snout on the face, the paws as the lighter mounds the art
// shades against the body, and the eyes and mouth punched through in the tile
// color exactly where the artwork cuts them from the background. Drawing it
// from elements keeps the mark crisp at any size and costs no assets; its
// colors are the artwork's own and stay constant across themes, because a
// logo does not re-theme.
//
// Usage:
//   MoleMark::new(px(36.0))
// ---------------------------------------------------------------------------

/// Tile background — also the eye and mouth color, which are cutouts in the
/// original artwork.
const TILE: u32 = 0xD95D3E;
/// Body fur.
const FUR: u32 = 0x2C2B2D;
/// Snout and paws — the step lighter the art uses to model them against the
/// shadowed body.
const SNOUT: u32 = 0x464445;

#[derive(IntoElement)]
pub struct MoleMark {
    size: Pixels,
}

impl MoleMark {
    pub fn new(size: impl Into<Pixels>) -> Self {
        Self { size: size.into() }
    }
}

impl RenderOnce for MoleMark {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let s: f32 = self.size.into();
        // Every shape below is placed in fractions of the tile, so the mark
        // scales without recomputing anything by hand.
        let at = |fraction: f32| px(s * fraction);

        div()
            .flex_none()
            .relative()
            .overflow_hidden()
            .w(px(s))
            .h(px(s))
            .rounded(at(0.24))
            .bg(rgb(TILE))
            // The dome: an ellipse fitted to the measured silhouette, cropped
            // by the right and bottom edges like the art, leaving open
            // background in the upper-left.
            .child(
                div()
                    .absolute()
                    .left(at(0.23))
                    .top(at(0.21))
                    .w(at(0.86))
                    .h(at(0.84))
                    .rounded_full()
                    .bg(rgb(FUR)),
            )
            // Paws: lighter mounds overlapping the body's base, cropped by
            // the bottom edge like the art.
            .child(
                div()
                    .absolute()
                    .left(at(0.17))
                    .top(at(0.82))
                    .w(at(0.27))
                    .h(at(0.27))
                    .rounded_full()
                    .bg(rgb(SNOUT)),
            )
            .child(
                div()
                    .absolute()
                    .left(at(0.56))
                    .top(at(0.83))
                    .w(at(0.29))
                    .h(at(0.29))
                    .rounded_full()
                    .bg(rgb(SNOUT)),
            )
            // The snout, centered on the face.
            .child(
                div()
                    .absolute()
                    .left(at(0.335))
                    .top(at(0.42))
                    .w(at(0.31))
                    .h(at(0.26))
                    .rounded_full()
                    .bg(rgb(SNOUT)),
            )
            // Eyes at their measured positions flanking the snout.
            .child(
                div()
                    .absolute()
                    .left(at(0.284))
                    .top(at(0.502))
                    .w(at(0.045))
                    .h(at(0.072))
                    .rounded_full()
                    .bg(rgb(TILE)),
            )
            .child(
                div()
                    .absolute()
                    .left(at(0.639))
                    .top(at(0.481))
                    .w(at(0.049))
                    .h(at(0.073))
                    .rounded_full()
                    .bg(rgb(TILE)),
            )
            // The mouth: a small half-round smile under the snout; a fully
            // rounded bottom with this aspect clamps to a semicircle.
            .child(
                div()
                    .absolute()
                    .left(at(0.44))
                    .top(at(0.733))
                    .w(at(0.067))
                    .h(at(0.038))
                    .rounded_b_full()
                    .bg(rgb(TILE)),
            )
    }
}
