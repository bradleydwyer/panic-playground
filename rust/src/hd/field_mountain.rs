//! Part 5's mountain: the mountain author's CPU side. The look is
//! field_mountain.wgsl; see rust/README.md, "Adding to part 5". Edit only
//! this file and field_mountain.wgsl.
//!
//! What comes from where, for the values this file adds:
//! - Derived from the code's formulas (as `FieldPreview::camera_at` and the
//!   terrain step read them): the rates at which the terrain step, the level
//!   and the projection's q move while the mountain rises (a step and a
//!   level a frame, q eight times the level until 248), which the shader
//!   uses to place the camera within the exposure.
//! - Estimated: the level's rate while it sinks, as its mean fall since the
//!   sink began (the code lowers it two thirds of a level a tracker row).
//! - Authored: the exposure, and the lake's light handed to the frame
//!   author (its place over the lake, its depth in the scene, spread,
//!   colour and strength).
use super::{AuthorInfo, FieldBlock};

pub static MOUNTAIN: AuthorInfo = AuthorInfo {
    name: "mountain",
    source: include_str!("field_mountain.wgsl"),
    // The half-resolution prepass marches each ray once at the middle of
    // the exposure (through the relief, and near the set the formula):
    // where it first nears the mountain, and its hit and coverage, for the
    // full-resolution samples.
    prepass: true,
    controls: mountain_controls,
};

/// Texels per side of the relief baked from the Mandelbrot fit when the data
/// loads (`relief_at`), over the 200 x 200 map: 1024 is about 5 a map unit,
/// 8 MB on the GPU, baked in a few tens of milliseconds. The mountain
/// marches and skips space with it; near the set, and in shading, it
/// evaluates the fitted formula per pixel instead.
pub const RELIEF_SIZE: usize = 1024;

/// The exposure, in seconds, ending at the moment shown: 0.7 of a 70 Hz
/// frame, as part 6's rings and the scroller's wall use (authored). The
/// camera turns about 2.5 degrees in it.
const SHUTTER: f64 = 0.7 / 70.0;

/// Terrain steps a second while the terrain runs: the code steps once a
/// frame.
const STEPS_PER_SECOND: f64 = 70.0;

/// The highest level, and the projection's highest q (8 times level 31, the
/// last level the code builds the table for).
const LEVEL_TOP: f64 = 64.0;
const Q_TOP: f64 = 248.0;

/// The centroid of the Mandelbrot set's area, as c: about -0.2867 + 0i (a
/// known value, not read from the data). The lake's light sits above it.
const SET_CENTROID: [f64; 2] = [-0.2867, 0.0];

/// The lake's light as the frame sees it (authored): the mean radiance of
/// its molten surface in linear HDR, an orange like the lake's, and how far
/// above the lake (height units) the light's centre sits.
const LAKE_LIGHT: [f64; 3] = [2.4, 0.95, 0.22];
const LAKE_LIFT: f64 = 24.0;

/// The lake's radius as a light, in map units (about the main cardioid's).
const LAKE_RADIUS: f64 = 30.0;

/// The light's depth in the scene: in front of the frame at the mountain's
/// layer (`LAYER_MOUNTAIN_Z`, 100) when the lake is this far from the camera
/// in map units, nearer the viewer as it comes closer (authored).
const LAKE_DEPTH: f64 = 180.0;
const LAYER_MOUNTAIN_Z: f64 = 100.0;

/// The mountain's `u.field.mountain_style` values at a clip time, given the
/// rest of the block:
/// - `[0]`: the lake's light as the frame sees it: its centre's source x and
///   y (320 x 200 pixels), its scene z in front of the frame, and its
///   spread (the lake's radius in source rows).
/// - `[1]`: the light's colour times its strength in linear HDR, 0 with no
///   mountain and fading as the lake leaves the screen; w the same as a
///   luminance.
/// - `[2]`: the rates within the exposure (terrain steps, levels and the
///   projection's q a second) and the exposure in seconds.
/// - `[3]`: spare.
///
/// All zero before the terrain, which is all the shader reads them in.
fn mountain_controls(block: &FieldBlock, _time: f64) -> [[f32; 4]; 4] {
    let mut style = [[0.0_f32; 4]; 4];
    if block.mountain[3] <= 0.5 {
        return style;
    }
    let rates = rates(block);
    style[2] = [
        rates[0] as f32,
        rates[1] as f32,
        rates[2] as f32,
        SHUTTER as f32,
    ];
    if let Some((place, colour)) = lake_light(block) {
        style[0] = place;
        style[1] = colour;
    }
    style
}

/// Terrain steps, levels and q a second at the block's moment.
fn rates(block: &FieldBlock) -> [f64; 3] {
    let seconds = block.part[1] as f64;
    let [risen, sinking, last, end] = block.events_late.map(f64::from);
    let level = block.mountain[0] as f64;
    let q = block.mountain[1] as f64;
    // The terrain steps once a frame until part 3 takes the frame.
    let steps = if seconds < end { STEPS_PER_SECOND } else { 0.0 };
    // A level a frame while it rises; while it sinks, the mean fall since it
    // began, or early on the mean fall to the last terrain pixel.
    let level_rate = if seconds < risen && level < LEVEL_TOP {
        STEPS_PER_SECOND
    } else if seconds > sinking && seconds < end {
        if seconds - sinking > 0.5 {
            (level - LEVEL_TOP) / (seconds - sinking)
        } else {
            -LEVEL_TOP / (last - sinking).max(1.0)
        }
    } else {
        0.0
    };
    // q follows eight times the level below the table's last build, and is
    // never negative.
    let q_rate = if q < Q_TOP - 0.5 && (q > 0.0 || level_rate > 0.0) {
        8.0 * level_rate
    } else {
        0.0
    };
    [steps, level_rate, q_rate]
}

/// The lake's light from the block's camera and projection: where a point
/// a little above the set's centroid shows, how far it is, and how much of
/// the lake is above the screen's bottom edge.
fn lake_light(block: &FieldBlock) -> Option<([f32; 4], [f32; 4])> {
    let [re, im, scale, aspect] = block.mandel.map(f64::from);
    if scale <= 0.0 {
        return None;
    }
    // Map x runs along the imaginary axis, map y along the real one.
    let m = [
        100.0 + (SET_CENTROID[1] - im) * scale,
        100.0 + (SET_CENTROID[0] - re) * scale * aspect,
    ];
    let [cx, cy, height, _] = block.camera.map(f64::from);
    let forward = [block.view[0] as f64 / 256.0, block.view[1] as f64 / 256.0];
    let right = [forward[1], -forward[0]];
    let dot = |a: [f64; 2], b: [f64; 2]| a[0] * b[0] + a[1] * b[1];
    let d = [m[0] - cx, m[1] - cy];
    let z = dot(d, forward) / dot(forward, forward).max(1e-9);
    if z < 1.0 {
        return None;
    }
    let lateral = dot(d, right) / dot(right, right).max(1e-9);
    let q = block.mountain[1] as f64;
    let projection = q * 256.0 / z + 512.0 - 2.0 * q;
    let top = 2.0 * (block.relief[3] as f64 - 128.0).max(0.0);
    let x = 161.0 + 256.0 * lateral / z;
    let y = (height - top - LAKE_LIFT) * projection / 1024.0;
    // The lake's radius on the screen: across, in columns, as rows.
    let spread = LAKE_RADIUS * 256.0 / z * 5.0 / 6.0;
    // Shown as the lake's own row rises past the screen's bottom edge.
    let lake = (height - top) * projection / 1024.0;
    let shown = ((200.0 - lake) / (0.5 * spread)).clamp(0.0, 1.0);
    let shown = shown * shown * (3.0 - 2.0 * shown);
    let depth = LAYER_MOUNTAIN_Z * (LAKE_DEPTH / z).clamp(0.5, 2.0);
    let c = LAKE_LIGHT.map(|v| v * shown);
    let luminance = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    Some((
        [x as f32, y as f32, depth as f32, spread as f32],
        [c[0] as f32, c[1] as f32, c[2] as f32, luminance as f32],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A terrain block with an invented fit and the camera on its orbit
    /// looking at the map's centre (synthetic values).
    fn invented(seconds: f32, level: f32, q: f32) -> FieldBlock {
        // Angle 256 of 1024: looking along +x from x = 100 - 2.56 r.
        let radius = 70.0;
        FieldBlock {
            part: [1.0, seconds, 1.0, 0.0],
            mountain: [level, q, 0.0, 1.0],
            events_late: [16.6, 40.6, 46.4, 48.8],
            // c = -0.5 at map (100, 100), 46 map units per unit of c.
            mandel: [-0.5, 0.0, 46.0, 1.2],
            relief: [1024.0, 200.0 / 1024.0, 244.0, 250.0],
            camera: [
                100.0 - 2.56 * radius,
                100.0,
                3.0 * radius + 470.0 - 5.0 * level,
                256.0,
            ],
            view: [256.0, 0.0, radius, 2.56 * radius],
            ..FieldBlock::default()
        }
    }

    #[test]
    fn no_light_or_motion_before_the_terrain() {
        let mut block = invented(10.0, 0.0, 0.0);
        block.mountain[3] = 0.0;
        assert_eq!(mountain_controls(&block, 91.0), [[0.0; 4]; 4]);
        block.mountain[3] = 1.0;
        let style = mountain_controls(&block, 91.0);
        assert!((style[2][3] as f64 - SHUTTER).abs() < 1e-9);
    }

    #[test]
    fn rates_follow_the_rise_and_the_sink() {
        let rising = rates(&invented(16.0, 20.0, 152.0));
        assert_eq!(rising, [70.0, 70.0, 560.0]);
        // Risen: the orbit goes on, the level and q hold.
        assert_eq!(rates(&invented(30.0, 64.0, 248.0)), [70.0, 0.0, 0.0]);
        // Two seconds into the sink, 24 levels down.
        let sinking = rates(&invented(42.6, 40.0, 248.0));
        assert!((sinking[1] + 12.0).abs() < 1e-3, "{sinking:?}");
        assert_eq!(sinking[2], 0.0);
        let low = rates(&invented(44.6, 16.0, 128.0));
        assert!(low[1] < 0.0 && (low[2] - 8.0 * low[1]).abs() < 1e-9);
        // Past the end nothing moves.
        assert_eq!(rates(&invented(49.0, -20.0, 0.0)), [0.0; 3]);
    }

    #[test]
    fn the_light_sits_over_the_lake_and_fades_below_the_screen() {
        let block = invented(30.0, 64.0, 248.0);
        let (place, colour) = lake_light(&block).unwrap();
        // The centroid is 0.2133 along the real axis from c at the map's
        // centre: map y 111.8, so left of the centre seen along +x.
        let m = [100.0, 100.0 + (-0.2867 + 0.5) * 46.0 * 1.2];
        let z = m[0] - block.camera[0] as f64;
        let lateral = -(m[1] - 100.0);
        assert!((place[0] as f64 - (161.0 + 256.0 * lateral / z)).abs() < 1e-3);
        let projection = 248.0 * 256.0 / z + 16.0;
        let y = (block.camera[2] as f64 - 244.0 - LAKE_LIFT) * projection / 1024.0;
        assert!((place[1] as f64 - y).abs() < 1e-3);
        assert!(place[2] > 0.0 && place[3] > 0.0);
        assert!(colour[0] > colour[1] && colour[1] > colour[2] && colour[3] > 0.0);
        // As the rise begins, the camera high and the projection flat, the
        // lake is below the screen: no light.
        let low = invented(15.0, 0.0, 0.0);
        assert_eq!(lake_light(&low).unwrap().1, [0.0; 4]);
    }
}
