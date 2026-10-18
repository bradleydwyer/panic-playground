//! Part 5's metal frame and the smoke behind and round it: the frame
//! author's CPU side. The look is field_frame.wgsl; see rust/README.md,
//! "Adding to part 5". Edit only this file and field_frame.wgsl.
use super::{AuthorInfo, FieldBlock, FluidInfo, FluidStart, no_controls};

pub static FRAME: AuthorInfo = AuthorInfo {
    name: "frame",
    source: include_str!("field_frame.wgsl"),
    prepass: false,
    controls: frame_controls,
};

/// The fluid hook's settings: `frame_fluid` in field_frame.wgsl runs once a
/// step on a grid of this many texels over this source-pixel rectangle, at
/// a fixed rate from part 5's start (or from part 6's PSTART fade), and the
/// renderer keeps a state every `checkpoint` steps for seeking back. The
/// stand-in does nothing visible, so its grid is small.
pub static FLUID: FluidInfo = FluidInfo {
    size: [256, 160],
    region: [-96.0, -36.0, 416.0, 236.0],
    rate: 70.0,
    start: FluidStart::Part5,
    checkpoint: 256,
};

/// Draw part 6's PSTART fade (80.36 to 81.27 s) with part 5's modern view,
/// so that the fade brings in the lit metal frame rather than the pool's
/// picture: `picture_level()` rises from 0 to 1 over it, and
/// `field_seconds()` is negative. Off, part 6 draws its own picture, as it
/// always has.
pub const FRAME_TAKES_PART6_FADE: bool = false;

/// The frame's `u.field.frame_style` values at a clip time (zero by
/// default), given the rest of the block.
fn frame_controls(block: &FieldBlock, time: f64) -> [[f32; 4]; 4] {
    no_controls(block, time)
}
