//! Part 5's mountain: the mountain author's CPU side. The look is
//! field_mountain.wgsl; see rust/README.md, "Adding to part 5". Edit only
//! this file and field_mountain.wgsl.
use super::{AuthorInfo, FieldBlock, no_controls};

pub static MOUNTAIN: AuthorInfo = AuthorInfo {
    name: "mountain",
    source: include_str!("field_mountain.wgsl"),
    prepass: false,
    controls: mountain_controls,
};

/// Texels per side of the relief baked from the Mandelbrot fit when the data
/// loads (`relief_at`), over the 200 x 200 map: 1024 is about 5 a map unit,
/// 8 MB on the GPU, baked in a few tens of milliseconds.
pub const RELIEF_SIZE: usize = 1024;

/// The mountain's `u.field.mountain_style` values at a clip time (zero by
/// default), given the rest of the block.
fn mountain_controls(block: &FieldBlock, time: f64) -> [[f32; 4]; 4] {
    no_controls(block, time)
}
