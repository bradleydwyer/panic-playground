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
//! - Authored: the exposure, and the lava's light handed to the frame
//!   author (its place over the lake, its depth in the scene, spread,
//!   colour and strength, and how the strength follows how much of the
//!   lake the camera sees, through the code's projection).
use super::{
    AuthorInfo, BAILOUT, FieldBlock, FieldPreview, MandelFit, POTENTIAL_ITERATIONS,
    SET_ITERATIONS, in_set,
};
use crate::engine::FRAME_RATE;

pub static MOUNTAIN: AuthorInfo = AuthorInfo {
    name: "mountain",
    source: include_str!("field_mountain.wgsl"),
    // The prepass draws the mountain sharp, at the middle of the exposure
    // and full resolution, and the layer blurs it along its motion.
    prepass: true,
    full_prepass: true,
    controls: mountain_controls,
};

/// Texels per side of the volcano's relief, baked when the data loads
/// (`relief_at`), over the 200 x 200 map: 2048 is about 10 a map unit, 32 MB
/// on the GPU with its maximum mipmaps (43 MB). The mountain marches it
/// through those mipmaps; shading evaluates the set per pixel.
pub const RELIEF_SIZE: usize = 2048;

// ---- The volcano's shape (authored) -----------------------------------------------
// The modern view's mountain is a volcano made from the Mandelbrot fit of
// DEP.U (field::MandelFit): its caldera is the set itself, a lava lake
// where DEP.U has its plateau, under a rim, on flanks that fall with the
// set's potential G, so that they follow the set's shape near the top and
// round out below, with gullies down the fall lines (the field lines of G,
// along which the set's external angle is constant). The original's
// terrain is a steep-sided mesa, DEP.U's plateau and slopes following base
// + slope sqrt(G), as the research found; the volcano keeps its plateau
// and its place, and authors the rest, in heights as `prepare` makes them
// (a height unit is MOUNTAIN_K map units in the modern view).

/// The rim's crest and the lake's surface.
pub const VOLCANO_RIM: f64 = 150.0;
pub const VOLCANO_LAKE: f64 = 140.0;
/// The crater's inner wall, from the lake to the crest, in map units by
/// the distance estimate.
const VOLCANO_WALL: f64 = 1.6;
/// The potential at which the flanks reach the ground: about 60 map units
/// beyond the rim where the set's edge is smooth. The flanks fall as (1 -
/// G / VOLCANO_FOOT)^2: about 44 degrees under the rim, flattening to the
/// foot.
const VOLCANO_FOOT: f64 = 0.3;
/// The ground falls away to nothing from this distance from the map's
/// centre to the next, so that spurs along the set's filaments end on it.
const VOLCANO_EDGE: [f64; 2] = [72.0, 97.0];
/// Gullies down the flanks: so many a turn of the external angle, so deep
/// at most (height units), mid-flank, where their spacing is above
/// VOLCANO_GULLY_TEXELS texels.
const VOLCANO_GULLIES: f64 = 160.0;
const VOLCANO_GULLY_DEPTH: f64 = 10.0;
/// Lobes of old flows down the flanks, so many height units high.
const VOLCANO_LOBES: f64 = 6.0;
const VOLCANO_GULLY_TEXELS: f64 = 6.0;
/// Rock: noise so many height units deep, its finest cells this many map
/// units across.
const VOLCANO_ROCK: f64 = 1.2;
const VOLCANO_ROCK_CELL: f64 = 0.8;

/// A point of the volcano: its height, whether it is in the lake, its
/// distance from the set (map units, at most 64), the potential and the
/// external angle (turns), and the potential's gradient per map unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolcanoPoint {
    pub height: f64,
    pub lake: bool,
    pub distance: f64,
    pub g: f64,
    pub angle: f64,
    pub rate: f64,
}

/// The orbit of c outside the set, as `field::potential` with the external
/// angle: G, its gradient in c, and arg Phi(c) / 2 pi from the product
/// Phi(c) = c prod (1 + c / z_k^2)^(1 / 2^k), z_1 = c (principal arguments;
/// continuous on the flanks). `None` if c is bounded.
fn orbit(c: [f64; 2]) -> Option<(f64, [f64; 2], f64)> {
    let (mut zr, mut zi) = (c[0], c[1]);
    let (mut dr, mut di) = (1.0_f64, 0.0_f64);
    let mut power = 2.0_f64;
    let mut angle = c[1].atan2(c[0]);
    let mut weight = 0.5_f64;
    for _ in 0..POTENTIAL_ITERATIONS {
        let m = zr * zr + zi * zi;
        if m > BAILOUT {
            let (qr, qi) = ((dr * zr + di * zi) / m, (di * zr - dr * zi) / m);
            let angle = (angle / std::f64::consts::TAU).rem_euclid(1.0);
            return Some((0.5 * m.ln() / power, [qr / power, -qi / power], angle));
        }
        let (sr, si) = (zr * zr - zi * zi, 2.0 * zr * zi);
        if weight > 1e-12 {
            let s2 = sr * sr + si * si;
            let (tr, ti) = (1.0 + (c[0] * sr + c[1] * si) / s2, (c[1] * sr - c[0] * si) / s2);
            angle += weight * ti.atan2(tr);
            weight *= 0.5;
        }
        (dr, di) = (2.0 * (zr * dr - zi * di) + 1.0, 2.0 * (zr * di + zi * dr));
        (zr, zi) = (sr + c[0], si + c[1]);
        power *= 2.0;
    }
    None
}

/// The gully a point of the flanks lies in, at external angle `angle` and
/// x = G / VOLCANO_FOOT down the flank: its distance across from the
/// gully's floor, in gullies (-0.5 to 0.5), and its number. The gullies
/// meander smoothly down the flank (authored). field_mountain.wgsl's
/// mountain_gully repeats it.
pub fn gully(angle: f64, x: f64) -> (f64, i64) {
    // Whole numbers of waves a turn, so the pattern closes round.
    let a = std::f64::consts::TAU * angle;
    let wander = 0.3 * (9.0 * a + 7.0 * x + 1.3).sin() + 0.2 * (3.0 * a - 11.0 * x + 0.4).sin();
    let u = VOLCANO_GULLIES * angle;
    let k = (u + wander).round();
    (u + wander - k, k as i64)
}

/// A gully's number hashed to [0, 1), with a seed: field_mountain.wgsl's
/// mountain_gully_hash.
pub fn gully_hash(k: i64, seed: u32) -> f64 {
    let n = VOLCANO_GULLIES as i64;
    let mut h = (k.rem_euclid(n) as u32).wrapping_mul(0x9E37_79B1) ^ seed.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f64 / 16_777_216.0
}

/// Smooth value noise in [-1, 1] on a unit lattice (authored).
fn noise(p: [f64; 2], seed: u64) -> f64 {
    let hash = |x: i64, y: i64| {
        let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ seed.wrapping_mul(0x1656_67B1_9E37_79F9);
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
        (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let (x, y) = (p[0].floor(), p[1].floor());
    let (fx, fy) = (p[0] - x, p[1] - y);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (x, y) = (x as i64, y as i64);
    let a = hash(x, y) + (hash(x + 1, y) - hash(x, y)) * sx;
    let b = hash(x, y + 1) + (hash(x + 1, y + 1) - hash(x, y + 1)) * sx;
    a + (b - a) * sy
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The volcano at map point m, for a relief of `texel` map units a texel.
pub fn volcano(fit: &MandelFit, m: [f64; 2], texel: f64) -> VolcanoPoint {
    let c = fit.c(m);
    let mut point = VolcanoPoint {
        height: VOLCANO_LAKE,
        lake: true,
        distance: 0.0,
        g: 0.0,
        angle: 0.0,
        rate: 0.0,
    };
    let edge = smoothstep(VOLCANO_EDGE[1], VOLCANO_EDGE[0], (m[0] - 100.0).hypot(m[1] - 100.0));
    if in_set(c, SET_ITERATIONS) {
        return point;
    }
    let Some((g, grad, angle)) = orbit(c) else {
        return point;
    };
    let rate = (grad[1] / fit.scale).hypot(grad[0] / (fit.scale * fit.aspect));
    let distance = (0.5 * g / rate.max(1e-300)).min(64.0);
    point.lake = false;
    point.g = g;
    point.angle = angle;
    point.rate = rate;
    point.distance = distance;
    // The flank by the potential, and the crater's inner wall by the
    // distance, meeting in a rounded crest.
    let x = (g / VOLCANO_FOOT).min(1.0);
    let outer = VOLCANO_RIM * (1.0 - x) * (1.0 - x);
    let w = (distance / VOLCANO_WALL).min(1.0);
    let inner = VOLCANO_LAKE + (VOLCANO_RIM - VOLCANO_LAKE) * w * (2.0 - w);
    let k = 3.0;
    let blend = (0.5 + 0.5 * (outer - inner) / k).clamp(0.0, 1.0);
    let mut height = outer + (inner - outer) * blend - k * blend * (1.0 - blend);
    // Gullies down the fall lines, meandering, mid-flank, where they are
    // wide enough to bake.
    let turns_per_unit = rate / std::f64::consts::TAU;
    let spacing = 1.0 / (VOLCANO_GULLIES * turns_per_unit).max(1e-9) / texel;
    let fade = smoothstep(VOLCANO_GULLY_TEXELS, 2.0 * VOLCANO_GULLY_TEXELS, spacing);
    let envelope = smoothstep(0.03, 0.2, x) * (1.0 - smoothstep(0.35, 0.85, x));
    if fade * envelope > 0.0 {
        let (along, gully) = gully(angle, x);
        // Sharp crests between rounded floors.
        let ridge = 1.0 - (std::f64::consts::PI * along).cos().abs().powf(1.5);
        // Some gullies deep, many shallow, some barely there, each starting
        // its own way down.
        let depth = gully_hash(gully, 1).powi(2) * smoothstep(0.0, 0.15, x - 0.2 * gully_hash(gully, 2));
        height += VOLCANO_GULLY_DEPTH * fade * envelope * depth * (ridge - 1.0);
    }
    // Lobes of old flows and outcrops down the flanks, stretched along the
    // fall lines.
    let lobes = noise([VOLCANO_GULLIES * angle / 6.0, x * 5.0], 7) + 0.5 * noise([VOLCANO_GULLIES * angle / 2.5, x * 11.0], 8);
    height += VOLCANO_LOBES * lobes * smoothstep(0.02, 0.15, x) * (1.0 - x) * (1.0 - x);
    // Rock, along and across the fall lines and in the map.
    let mut rock = 0.0;
    let mut amp = 1.0;
    let mut cell = VOLCANO_ROCK_CELL * 8.0;
    for octave in 0..4 {
        if cell < 2.0 * texel {
            break;
        }
        rock += amp * noise([m[0] / cell, m[1] / cell], 10 + octave);
        amp *= 0.5;
        cell *= 0.5;
    }
    height += VOLCANO_ROCK * rock * smoothstep(0.0, 0.05, x) * (1.0 - x);
    // Nothing the noise digs below the flank's own foot.
    point.height = (height.max(0.15 * outer) * edge).max(0.0);
    point
}

/// The exposure, in seconds, ending at the moment shown: 0.7 of a 70 Hz
/// frame, as part 6's rings and the scroller's wall use (authored). The
/// camera turns about 2.5 degrees in it.
const SHUTTER: f64 = 0.7 / 70.0;

/// Map units a height unit, to see the projection as 3D: at full
/// perspective (q = 248) a height unit shows 62 / z rows and a map unit
/// across 256 * 5/6 / z rows (derived from the code's projection; the 3D
/// reading is interpretation). field_mountain.wgsl's MOUNTAIN_K.
pub const MOUNTAIN_K: f64 = 0.29;

/// Scene rows a unit of lateral offset over depth, at full perspective: a
/// source column is 5/6 of a row, and the code's rays spread 1/256 of the
/// depth a column.
const COLUMN_ROWS: f64 = 256.0 * 5.0 / 6.0;

/// The centroid of the Mandelbrot set's area, as c: about -0.2867 + 0i (a
/// known value, not read from the data). The lake's light sits above it.
const SET_CENTROID: [f64; 2] = [-0.2867, 0.0];

// ---- The camera ------------------------------------------------------------------
// The modern mountain is drawn by a pinhole camera in the volcano's world:
// map x and y, and height, MOUNTAIN_K map units a height unit. Through the
// orbit it is the code's camera (derived): its place, heading and height,
// read from the port at each moment, and its projection, whose rows are
// perspective and an orthographic share, (height - h) (q / (4 z) + (512 -
// 2 q) / 1024). A pinhole can't have both, so it matches them at the depth
// of the map's centre, in value and in how fast they fall with depth, by
// standing back along its view and narrowing (interpretation): at q = 248
// that is 8 map units back and 5% narrower vertically; as q falls to 0 as
// the mountain sinks, it stands far back, with a long lens, flattening the
// mountain as the code's projection does.
//
// As the mountain rises it joins the zoom (authored, at the owner's
// request): where the code's camera falls from high above with its
// projection flat, the modern view begins by looking straight down on the
// lake, the set, framed and turned as the zoom's minibrot lands in the
// window, and over JOIN seconds swings down and round into the orbit,
// while the mountain rises out through the window.

/// The join's length, from the terrain's first frame (authored).
pub const JOIN: f64 = 1.5;
/// The zoom's landed minibrot in the window, as measured from the modern
/// zoom's last frame (authored target, field_zoom.rs): the scene x of its
/// cusp (c = 1/4), the scene y of its real axis, and scene rows a unit of
/// c. It lands upright.
const MINIBROT_CUSP: f64 = 32.78;
const MINIBROT_AXIS: f64 = 4.07;
const MINIBROT_SCALE: f64 = 44.0;
/// How far above the lake the join starts, in map units (authored): the
/// focal lengths follow, to frame the lake as the minibrot.
const JOIN_HEIGHT: f64 = 246.0;
/// Where the frame's plane lies along the camera's view as the join begins
/// and ends, over the distance to the lake (authored): the mountain beyond
/// it is behind the frame, seen only through the window; nearer, it stands
/// in front.
const JOIN_WINDOW: [f64; 2] = [0.6, 4.0];

/// A pinhole camera: its eye, its view and up (unit, at right angles), its
/// focal lengths (scene rows a unit of offset over depth, across and up),
/// the scene point its view passes through, the depth of the frame's plane
/// along its view, and how far the join has come (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountainView {
    pub eye: [f64; 3],
    pub forward: [f64; 3],
    pub up: [f64; 3],
    pub focal: [f64; 2],
    pub centre: [f64; 2],
    pub window: f64,
    pub join: f64,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn smootherstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

impl MountainView {
    /// The code's camera, [map x, map y, height, angle], at heading `view`
    /// (the sine and cosine, amplitude 256) and projection q, as a pinhole.
    pub fn port(camera: [f64; 4], view: [f64; 4], q: f64) -> Self {
        let l = view[0].hypot(view[1]).max(1e-9);
        let f = [view[0] / l, view[1] / l];
        let z0 = ((100.0 - camera[0]) * f[0] + (100.0 - camera[1]) * f[1]).max(20.0);
        let a = (q / 4.0).max(0.25);
        let b = (512.0 - 2.0 * q) / 1024.0;
        let ratio = b * z0 / a;
        let back = ratio * z0;
        Self {
            eye: [
                camera[0] - f[0] * back,
                camera[1] - f[1] * back,
                camera[2] * MOUNTAIN_K,
            ],
            forward: [f[0], f[1], 0.0],
            up: [0.0, 0.0, 1.0],
            focal: [
                COLUMN_ROWS * (1.0 + ratio),
                a * (1.0 + ratio) * (1.0 + ratio) / MOUNTAIN_K,
            ],
            centre: [5.0 / 6.0, 100.0],
            window: 1e5,
            join: 1.0,
        }
    }

    /// The camera at a clip time.
    pub fn at(preview: &FieldPreview, time: f64) -> Self {
        let (tf, level, q) = preview.terrain_at(time);
        let seconds = time - preview.seconds(0);
        let start = preview.events.terrain;
        let s = (seconds - start) / JOIN;
        if s >= 1.0 || start < 0.0 {
            let (camera, view) = preview.camera_at(tf, level);
            return Self::port(camera, view, q);
        }
        // The orbit as it will be once risen, and the minibrot's lake.
        let (camera, view) = preview.camera_at(tf, 64.0);
        let orbit = Self::port(camera, view, 248.0);
        let fit = &preview.fit;
        // c at the window's centre, scene x 0, on the real axis.
        let window_c = 0.25 - MINIBROT_CUSP / MINIBROT_SCALE;
        let lake = [
            100.0 + (0.0 - fit.centre[1]) * fit.scale,
            100.0 + (window_c - fit.centre[0]) * fit.scale * fit.aspect,
            VOLCANO_LAKE * MOUNTAIN_K,
        ];
        let s = s.max(0.0);
        let w = smootherstep(s);
        // The heading: straight down, the window's up along -map x (the
        // set's imaginary axis, which it mirrors), turning with the orbit
        // more and more and landing on its heading.
        let tau = std::f64::consts::TAU;
        let heading_of = |angle: f64| std::f64::consts::FRAC_PI_2 - angle * tau / 1024.0;
        let spin = -10.0 * 70.0 * tau / 1024.0;
        let start_heading = std::f64::consts::PI;
        let end_time = preview.seconds(0) + start + JOIN;
        let (end_tf, _, _) = preview.terrain_at(end_time);
        let end_heading = heading_of(preview.camera_at(end_tf, 64.0).0[3]);
        let turned = start_heading + spin * JOIN * 0.5;
        let error = (end_heading - turned + std::f64::consts::PI).rem_euclid(tau)
            - std::f64::consts::PI;
        let integral = s * s * s * s * (s * (s - 3.0) + 2.5);
        let heading = start_heading + spin * JOIN * integral + w * error;
        // The eye comes down round the lake, from straight above it to the
        // orbit's height and distance, behind the heading.
        let rel = [
            orbit.eye[0] - lake[0],
            orbit.eye[1] - lake[1],
            orbit.eye[2] - lake[2],
        ];
        let reach = dot(rel, rel).sqrt();
        let elevation = (rel[2] / reach).asin();
        let radius = mix(JOIN_HEIGHT, reach, w);
        let e = mix(std::f64::consts::FRAC_PI_2, elevation, w);
        let (sh, ch) = heading.sin_cos();
        let round = [
            lake[0] - radius * e.cos() * ch,
            lake[1] - radius * e.cos() * sh,
            lake[2] + radius * e.sin(),
        ];
        let b = smootherstep((s - 0.6) / 0.4);
        let eye = [0, 1, 2].map(|k| mix(round[k], orbit.eye[k], b));
        // Framed as the minibrot to begin with: rows a map unit across (map
        // y, the real axis) and up (map x) at the lake.
        let rows = [
            MINIBROT_SCALE / (fit.scale * fit.aspect),
            MINIBROT_SCALE / fit.scale,
        ];
        let focal = [
            mix(rows[0] * JOIN_HEIGHT, orbit.focal[0], w),
            mix(rows[1] * JOIN_HEIGHT, orbit.focal[1], w),
        ];
        let centre = [
            mix(0.0, orbit.centre[0], w),
            mix(MINIBROT_AXIS, orbit.centre[1], w),
        ];
        // Pitched so that the lake shows at a point going smoothly from the
        // window's middle to where the orbit shows it.
        let to_lake = [lake[0] - eye[0], lake[1] - eye[1], lake[2] - eye[2]];
        let level = to_lake[0].hypot(to_lake[1]);
        let down = to_lake[2].atan2(level);
        let shown = mix(MINIBROT_AXIS, orbit.project(lake)[1], w);
        let pitch = (down - ((shown - centre[1]) / focal[1]).atan())
            .clamp(-std::f64::consts::FRAC_PI_2, 0.3);
        let (sp, cp) = pitch.sin_cos();
        let forward = [cp * ch, cp * sh, sp];
        let up = [-sp * ch, -sp * sh, cp];
        let window = radius * mix(JOIN_WINDOW[0], JOIN_WINDOW[1], smootherstep((s - 0.1) / 0.6));
        Self {
            eye,
            forward,
            up,
            focal,
            centre,
            window,
            join: w,
        }
    }

    /// `FieldBlock::mountain_view`'s four vec4s for this camera: the eye and
    /// the exposure, the view and the focal length across, up and the focal
    /// length up, and the scene point the view passes through, the frame's
    /// depth and the join.
    fn vectors(&self) -> [[f32; 4]; 4] {
        [
            [self.eye[0], self.eye[1], self.eye[2], SHUTTER],
            [self.forward[0], self.forward[1], self.forward[2], self.focal[0]],
            [self.up[0], self.up[1], self.up[2], self.focal[1]],
            [self.centre[0], self.centre[1], self.window, self.join],
        ]
        .map(|v| v.map(|x| x as f32))
    }

    /// Project a world point: scene x and y, and its depth along the view.
    pub fn project(&self, p: [f64; 3]) -> [f64; 3] {
        let d = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        let right = cross(self.forward, self.up);
        let z = dot(d, self.forward).max(1e-6);
        [
            self.centre[0] + self.focal[0] * dot(d, right) / z,
            self.centre[1] + self.focal[1] * dot(d, self.up) / z,
            z,
        ]
    }
}

/// `FieldBlock::mountain_view` at a clip time: the camera at the exposure's
/// opening, then at its close, the moment shown.
pub fn view_block(preview: &FieldPreview, time: f64) -> [[f32; 4]; 8] {
    let open = MountainView::at(preview, time - SHUTTER).vectors();
    let close = MountainView::at(preview, time).vectors();
    [
        open[0], open[1], open[2], open[3], close[0], close[1], close[2], close[3],
    ]
}

/// Rows of the tune between the beats the lava spatters on, and between
/// the accents its fountains leap on (authored, from the tune's own rows
/// as part 5 played them: a beat every 8 rows, 0.46 seconds at speed 3 and
/// tempo 130; the loudest every 32, on row 8 of each 32, as the PCM's
/// onsets show it). The generator's row leads what is heard by about a
/// row (observed: the onsets come 0.06 to 0.1 seconds after the rows
/// start).
const BEAT_ROWS: u16 = 8;
const ACCENT_ROWS: u16 = 32;
const ACCENT_ROW: u16 = 8;
const HEARD: f64 = 0.06;
/// How long after each the shader still draws its lava (seconds).
const ACCENT_LIFE: f64 = 2.0;
const BEAT_LIFE: f64 = 1.0;

/// `u.field.mountain_style[2]` and `[3]`: the seconds since the last two
/// accents and the frames they began on (as seeds), then the same for the
/// last two beats; -1 for none. From the terrain's first frame until the
/// last terrain pixel.
pub fn hits(preview: &FieldPreview, time: f64) -> [[f32; 4]; 2] {
    let mut out = [[-1.0_f32; 4]; 2];
    let Some(j) = preview.frame_at(time) else {
        return out;
    };
    let first = preview.events.terrain * FRAME_RATE as f64;
    let last = preview.events.last_terrain * FRAME_RATE as f64;
    if (j as f64) < first || (j as f64) > last + ACCENT_LIFE * FRAME_RATE as f64 {
        return out;
    }
    let mut found = [0usize; 2];
    let mut k = j;
    while k > 0 && (found[0] < 2 || found[1] < 2) {
        let (Some(now), Some(before)) = (preview.music_position(k), preview.music_position(k - 1))
        else {
            break;
        };
        let since = time - preview.seconds(k) - HEARD;
        if since > ACCENT_LIFE {
            break;
        }
        if now != before && (k as f64) >= first && since >= 0.0 {
            let row = now & 0xff;
            for (slot, rows, at, life) in [
                (0, ACCENT_ROWS, ACCENT_ROW, ACCENT_LIFE),
                (1, BEAT_ROWS, 0, BEAT_LIFE),
            ] {
                if row % rows == at && found[slot] < 2 && since <= life {
                    out[slot][found[slot]] = since as f32;
                    out[slot][2 + found[slot]] = k as f32;
                    found[slot] += 1;
                }
            }
        }
        k -= 1;
    }
    out
}

/// The camera at the close of the exposure from the block.
fn close_view(block: &FieldBlock) -> MountainView {
    let v = block.mountain_view.map(|v| v.map(f64::from));
    MountainView {
        eye: [v[4][0], v[4][1], v[4][2]],
        forward: [v[5][0], v[5][1], v[5][2]],
        up: [v[6][0], v[6][1], v[6][2]],
        focal: [v[5][3], v[6][3]],
        centre: [v[7][0], v[7][1]],
        window: v[7][2],
        join: v[7][3],
    }
}

/// The lava's light as the frame sees it (authored): the glowing mountain's
/// radiance in linear HDR, an orange like its lava's about 1050 C
/// (field_mountain.wgsl's mountain_glow), as bright as the frame's stand-in
/// for it (luminance 2.4), which the frame and its smoke were tuned to; and
/// how far above the lake (height units) the light's centre sits.
const LAVA_LIGHT: [f64; 3] = [6.0, 1.6, 0.14];
const LAKE_LIFT: f64 = 24.0;

/// The lake's radius, in map units (about the main cardioid's), and the
/// glow's: the lake and the rim round it.
const LAKE_RADIUS: f64 = 30.0;
const GLOW_RADIUS: f64 = 36.0;

/// The lake's share of the light, which follows how much of the lake the
/// camera sees: its area on the screen against its area at the orbit's mean
/// (the lake 180 map units away, the camera 63 map units above it), within
/// LAKE_FACING. The rest is the plume's and the flanks', which always face
/// the camera (authored).
const LAKE_SHARE: f64 = 0.6;
const LAKE_FACING: [f64; 2] = [0.3, 1.25];

/// The light's depth in the scene: in front of the frame at the mountain's
/// layer (`LAYER_MOUNTAIN_Z`, 100) when the lake is this far from the camera
/// in map units, nearer the viewer as it comes closer (authored).
const LAKE_DEPTH: f64 = 180.0;
const LAYER_MOUNTAIN_Z: f64 = 100.0;

/// The mountain's `u.field.mountain_style` values at a clip time, given the
/// rest of the block:
/// - `[0]`: the lava's light as the frame sees it: its centre's source x
///   and y (320 x 200 pixels), a little above the lake, its scene z in
///   front of the frame, and its spread (the glow's radius in source rows,
///   GLOW_RADIUS map units on the screen).
/// - `[1]`: the light's colour times its strength in linear HDR, brighter
///   the more of the lake the camera sees, 0 with no mountain and fading as
///   the lake leaves the screen; w the same as a luminance.
/// - `[2]`, `[3]`: the lava's beats and accents ([`hits`], which
///   `FieldPreview::controls` fills in).
///
/// All zero before the terrain.
fn mountain_controls(block: &FieldBlock, _time: f64) -> [[f32; 4]; 4] {
    let mut style = [[0.0_f32; 4]; 4];
    if block.mountain[3] <= 0.5 {
        return style;
    }
    if let Some((place, colour)) = lake_light(block) {
        style[0] = place;
        style[1] = colour;
    }
    style
}

/// The lava's light from the mountain's camera: where a point a little
/// above the set's centroid shows, how far it is, how much of the lake is
/// above the screen's bottom edge and in front of the frame, and how large
/// the lake shows.
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
    let view = close_view(block);
    let lake = VOLCANO_LAKE * MOUNTAIN_K;
    let at = view.project([m[0], m[1], lake + LAKE_LIFT * MOUNTAIN_K]);
    let centre = view.project([m[0], m[1], lake]);
    let z = centre[2];
    if z < 1.0 {
        return None;
    }
    // Scene rows to source pixels (y down).
    let x = 160.0 + at[0] * 6.0 / 5.0;
    let y = 100.0 - at[1];
    // The lake's radius on the screen across, and the glow's.
    let radius = LAKE_RADIUS * view.focal[0] / z;
    let spread = radius * GLOW_RADIUS / LAKE_RADIUS;
    // Shown as the lake rises past the screen's bottom edge, and as it
    // comes out of the window.
    let row = 100.0 - centre[1];
    let shown = ((200.0 - row) / (0.5 * radius)).clamp(0.0, 1.0);
    let shown = shown * shown * (3.0 - 2.0 * shown) * smootherstep((view.join - 0.3) / 0.5);
    // The lake's area on the screen, its width by the rows between its far
    // and near edges, against its area at the orbit's mean, 180 map units
    // away and 63 below.
    let l = view.forward[0].hypot(view.forward[1]).max(1e-9);
    let dir = [view.forward[0] / l, view.forward[1] / l];
    let edge = |k: f64| {
        view.project([m[0] + k * dir[0] * LAKE_RADIUS, m[1] + k * dir[1] * LAKE_RADIUS, lake])[1]
    };
    let area = 2.0 * radius * (edge(1.0) - edge(-1.0)).abs();
    let mean = 2.0 * LAKE_RADIUS * view.focal[0] / 180.0
        * view.focal[1]
        * 63.0
        * (1.0 / (180.0 - LAKE_RADIUS) - 1.0 / (180.0 + LAKE_RADIUS));
    let facing = (area / mean.max(1e-6)).clamp(LAKE_FACING[0], LAKE_FACING[1]);
    let strength = shown * (1.0 - LAKE_SHARE + LAKE_SHARE * facing);
    let depth = LAYER_MOUNTAIN_Z * (LAKE_DEPTH / z).clamp(0.5, 2.0);
    let c = LAVA_LIGHT.map(|v| v * strength);
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
    /// looking at the map's centre (synthetic values), its pinhole in
    /// `mountain_view`.
    fn invented(seconds: f32, level: f32, q: f32) -> FieldBlock {
        // Angle 256 of 1024: looking along +x from x = 100 - 2.56 r.
        let radius = 70.0;
        let mut block = FieldBlock {
            part: [1.0, seconds, 1.0, 0.0],
            mountain: [level, q, 0.0, 1.0],
            events_late: [16.6, 40.6, 46.4, 48.8],
            // c = -0.5 at map (100, 100), 46 map units per unit of c.
            mandel: [-0.5, 0.0, 46.0, 1.2],
            relief: [1024.0, 200.0 / 1024.0, 150.0, 250.0],
            camera: [
                100.0 - 2.56 * radius,
                100.0,
                3.0 * radius + 470.0 - 5.0 * level,
                256.0,
            ],
            view: [256.0, 0.0, radius, 2.56 * radius],
            ..FieldBlock::default()
        };
        view_into(&mut block);
        block
    }

    /// The block's camera as the pinhole, at both ends of the exposure.
    fn view_into(block: &mut FieldBlock) {
        let v = MountainView::port(
            block.camera.map(f64::from),
            block.view.map(f64::from),
            block.mountain[1] as f64,
        )
        .vectors();
        block.mountain_view = [v[0], v[1], v[2], v[3], v[0], v[1], v[2], v[3]];
    }

    #[test]
    fn no_light_before_the_terrain() {
        let mut block = invented(10.0, 0.0, 0.0);
        block.mountain[3] = 0.0;
        assert_eq!(mountain_controls(&block, 91.0), [[0.0; 4]; 4]);
        block.mountain[3] = 1.0;
        let style = mountain_controls(&block, 91.0);
        assert_eq!(style[2], [0.0; 4]);
    }

    #[test]
    fn the_pinhole_matches_the_codes_projection_at_the_centres_depth() {
        for q in [248.0, 128.0, 32.0] {
            let block = invented(30.0, 64.0, q as f32);
            let view = close_view(&block);
            let [cx, cy, height, _] = block.camera.map(f64::from);
            let code = |m: [f64; 2], h: f64| {
                // The code's column and row, as scene rows (y up).
                let z = m[0] - cx;
                let lateral = -(m[1] - cy);
                let column = 161.0 + 256.0 * lateral / z;
                let row = (height - h) * (q * 256.0 / z + 512.0 - 2.0 * q) / 1024.0;
                [(column - 160.0) * 5.0 / 6.0, 100.0 - row]
            };
            for (m, h) in [([100.0, 100.0], 150.0), ([100.0, 130.0], 40.0), ([100.0, 70.0], 0.0)] {
                let p = view.project([m[0], m[1], h * MOUNTAIN_K]);
                let c = code(m, h);
                assert!((p[0] - c[0]).abs() < 1e-6 * (1.0 + c[0].abs()), "{q} {p:?} {c:?}");
                assert!((p[1] - c[1]).abs() < 1e-6 * (1.0 + c[1].abs()), "{q} {p:?} {c:?}");
            }
            // Nearer and farther the two part, a little at full perspective.
            let near = view.project([60.0, 100.0, 0.0]);
            let c = code([60.0, 100.0], 0.0);
            if q == 248.0 {
                assert!((near[1] - c[1]).abs() < 0.03 * (100.0 - c[1]), "{near:?} {c:?}");
            }
        }
    }

    #[test]
    fn the_light_sits_over_the_lake_and_fades_below_the_screen() {
        let block = invented(30.0, 64.0, 248.0);
        let (place, colour) = lake_light(&block).unwrap();
        // Over the set's centroid, a little above the lake.
        let m = [100.0, 100.0 + (-0.2867 + 0.5) * 46.0 * 1.2];
        let view = close_view(&block);
        let p = view.project([m[0], m[1], (VOLCANO_LAKE + LAKE_LIFT) * MOUNTAIN_K]);
        assert!((place[0] as f64 - (160.0 + p[0] * 6.0 / 5.0)).abs() < 1e-3);
        assert!((place[1] as f64 - (100.0 - p[1])).abs() < 1e-3);
        assert!(place[2] > 0.0 && place[3] > 0.0);
        assert!(colour[0] > colour[1] && colour[1] > colour[2] && colour[3] > 0.0);
        // With the camera high and the projection flat, as the code's rise
        // begins, the lake is below the screen: no light.
        let low = invented(15.0, 0.0, 0.0);
        assert_eq!(lake_light(&low).unwrap().1, [0.0; 4]);
    }

    #[test]
    fn the_light_follows_how_much_of_the_lake_shows() {
        let mean = invented(30.0, 64.0, 248.0);
        let base = lake_light(&mean).unwrap().1[3] as f64;
        let full = 0.2126 * LAVA_LIGHT[0] + 0.7152 * LAVA_LIGHT[1] + 0.0722 * LAVA_LIGHT[2];
        assert!(base > 0.6 * full && base < 1.3 * full, "{base} {full}");
        // Seen from higher up, more of the lake shows: brighter, but never
        // past the lake's most.
        let mut above = mean;
        above.camera[2] += 120.0;
        view_into(&mut above);
        let high = lake_light(&above).unwrap().1[3] as f64;
        assert!(high > base, "{high} {base}");
        let most = full * (1.0 - LAKE_SHARE + LAKE_SHARE * LAKE_FACING[1]);
        assert!(high <= most + 1e-4);
        // Seen nearly edge on, less: dimmer, but the plume and flanks still
        // glow.
        let mut level = mean;
        level.camera[2] = 160.0;
        view_into(&mut level);
        let edge = lake_light(&level).unwrap().1[3] as f64;
        assert!(
            edge < base && edge >= full * (1.0 - LAKE_SHARE) - 1e-4,
            "{edge}"
        );
    }

    #[test]
    fn gullies_close_round_and_hash_by_their_number() {
        for x in [0.1, 0.5, 0.9] {
            let a = gully(0.0, x);
            let b = gully(1.0, x);
            assert!((a.0 - b.0).abs() < 1e-9);
            assert_eq!(gully_hash(a.1, 1), gully_hash(b.1, 1));
        }
        let (across, _) = gully(0.3172, 0.4);
        assert!((-0.5..=0.5).contains(&across));
    }
}
