//! What part 5's window shows during the zoom: the zoom author's CPU side.
//! The look is field_zoom.wgsl; see rust/README.md, "Adding to part 5".
//! Edit only this file and field_zoom.wgsl.
use super::{AuthorInfo, FieldBlock, no_controls};

pub static ZOOM: AuthorInfo = AuthorInfo {
    name: "zoom",
    source: include_str!("field_zoom.wgsl"),
    prepass: false,
    controls: zoom_controls,
};

/// The zoom's `u.field.zoom_style` values at a clip time (zero by default),
/// given the rest of the block.
fn zoom_controls(block: &FieldBlock, time: f64) -> [[f32; 4]; 4] {
    no_controls(block, time)
}
