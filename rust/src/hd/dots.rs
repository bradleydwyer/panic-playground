//! Part 3 (Dots) for the HD preview: the state of every part-3 frame read
//! from the port's `Dots` scene, part 4's wait and its first frame, the
//! original frames replayed on demand, and the tables the faithful and
//! modern views draw from (dots_render.rs and the dots_*.wgsl files).
//!
//! What comes from where:
//! - Read from the port's code and counters (`Dots::hd_state`,
//!   `Dots::hd_points` and `Dots::hd_tables`): the phases, the zoom's
//!   scale, the shade iterations and their stop, the shade path and disc,
//!   the eight objects, their order, the stage lengths and the morphs, the
//!   rotation and its stop, the camera spline, the camera-space points each
//!   dot frame drew, the projection (column 160 + x, row 100 + y, checked
//!   against the code's tables by a test), and the palettes.
//! - Observed when the preview loads: the times of the phases
//!   ([`DotsEvents`]), the music's position at each frame (and so the
//!   orders' first rows) and part 4's first frame.
//! - Interpretation: interpolation between the original's 70 Hz frames; the
//!   rotation, morphs and camera spline evaluated in floating point, without
//!   the code's Q7 coefficients and truncations (the port's own integer
//!   points are kept for faithful HD); the shade path evaluated at
//!   continuous iterations with a floating-point sine.
//! - Authored: everything the modern view adds (dots_light.wgsl and the
//!   values marked authored here), the box reshaped to land on part 4's
//!   slab ([`land_on_slab`]) and the exposure running on once the box lies
//!   still.
use super::dots_water::{LightsInWater, Ripple, Water, rotation};
use super::plasma::MAX_DROPS;
use super::plasma_stone::Stone;
#[cfg(doc)]
use super::plasma::WaterTuning;
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::{
        dots::{Dots, HdDots, HdDotsTables},
        plasma::Plasma,
    },
    vga::Vga,
};
use bytemuck::{Pod, Zeroable};
use std::sync::Mutex;

/// `u.state.x` during part 3. Part 6 is 10, part 5 11.
pub const PART3_PHASE: f32 = 12.0;
/// Points in every object, read from the code.
pub const POINTS: usize = 512;
/// Shade discs drawn an iteration, and the iterations kept before each is
/// erased, read from the code.
pub const SHADE_DISCS: usize = 20;
pub const SHADE_KEPT: usize = 16;
/// Dot echoes on screen and the frames between them: each frame draws into
/// one of 4 pages at one of 8 bits, so a page shows the frames 4 apart that
/// share it (read from the code).
pub const ECHOES: usize = 8;
pub const ECHO_SPACING: usize = 4;
/// The dot frame from which the rotation stops (`frame < 2601`, read from
/// the code).
pub const ROTATION_STOP: usize = 2601;
/// Frames between replay checkpoints for the original comparison.
const REPLAY_INTERVAL: usize = 64;
/// The names of the eight objects, by stage (an interpretation of the
/// generated shapes; the research names them by their storage, P3D1 to
/// P3D8).
pub const STAGE_NAMES: [&str; 8] = [
    "sphere lattice",
    "vase",
    "cube edges",
    "line grid",
    "slab lattice",
    "cube lattice",
    "pyramid edges",
    "box",
];

/// What a part-3 frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    /// Part 5's last frame, held until the music reaches 1E00.
    Hold = 0,
    /// The zoom into the window.
    Zoom = 1,
    /// The shade bobs, then their erasure.
    Shades = 2,
    /// The dots.
    Dots = 3,
    /// Part 4 has begun and waits for 29:00, holding the last dot frame.
    Wait = 4,
    /// Part 4's first frame: its logo slab, white.
    Slab = 5,
}

/// One part-3 frame as the scanout shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DotsFrame {
    /// The scene's counters after the step that produced this frame (during
    /// part 4, the last dot frame's).
    pub state: HdDots,
    pub phase: Phase,
    /// The palette this frame shows, an index into [`DotsPreview::palettes`].
    pub palette: usize,
}

/// One dot frame for the GPU: the rotation, translation and morph as the
/// code computes them, in floating point (an interpretation; see the module
/// notes). dots_common.wgsl declares the same layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct DotFrame {
    /// The rotation angles, 1024ths of a turn, unwrapped, and the stage.
    pub angles: [f32; 4],
    /// The camera spline's translation, without the code's truncation, and
    /// the morph: the fraction of the way to the next stage's object.
    pub translation: [f32; 4],
}

/// Times in part 3, in seconds from its first frame, observed from the
/// frames when the preview loads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DotsEvents {
    /// The first zoom frame, its last, and the first shade frame.
    pub zoom: f64,
    pub zoom_end: f64,
    pub shades: f64,
    /// The first frame whose iteration adds no discs (the music passed
    /// 1E3C), and the frame that finishes the erasure.
    pub shade_stop: f64,
    pub shades_end: f64,
    /// The first dot frame, the dot frame from which the rotation stops,
    /// and the last dot frame.
    pub dots: f64,
    pub rotation_stop: f64,
    pub dots_end: f64,
    /// Part 4's first frame (it waits), and the first it draws: its logo
    /// slab, white, where the clip ends.
    pub part4: f64,
    pub slab: f64,
}

/// The part-3 controls every part-3 shader reads, `u.dots`.
/// renderer::CONTROLS_WGSL declares the same layout; keep the two in step
/// (the tests check them). All zero before part 3.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct DotsBlock {
    /// Shown (1), seconds since part 3 began, phase ([`Phase`]), part-3
    /// frame (continuous).
    pub part: [f32; 4],
    /// The zoom: the magnification the original shows (65536 / scale,
    /// interpolated; 1 before), the modern view's (the same law carried on
    /// past the original's last scale), the modern one's rate (d ln M / dt,
    /// per second), and whether the modern view still shows part 5's frame
    /// (1) or has flown through its window (0).
    pub zoom: [f32; 4],
    /// The shade bobs: iterations drawn (continuous: the shown frame's and
    /// the fraction of the way to the next), the stop (the first iteration
    /// that adds no discs), shown (1), and the shown frame's iterations.
    pub shades: [f32; 4],
    /// The dots: the dot frame (continuous; -1 before the first), the last
    /// dot frame, the dot frame from which the rotation stops, shown (1).
    pub dots: [f32; 4],
    /// The camera: the focus distance (the object's centre, camera units),
    /// and the object's centre in camera space at the dot frame shown.
    pub focus: [f32; 4],
    /// The end: seconds since the rotation stopped (-1 before), seconds
    /// until part 4 draws (its slab), the slab shown (1), seconds since it.
    pub end: [f32; 4],
    /// Part-3 seconds of [`DotsEvents`]: zoom, zoom end, shades, shade stop.
    pub events: [f32; 4],
    /// Shades end, dots, rotation stop, part 4.
    pub events_late: [f32; 4],
    /// The orders' first rows (the music, read from its position) nearest
    /// before the moment: seconds since each of the last four (newest
    /// first; -100 none), and their strengths from the PCM (filled in by
    /// the renderer).
    pub hits: [f32; 4],
    pub hit_strength: [f32; 4],
    /// The PCM's low band (`AudioCues::intro[2]`) at the moment and each
    /// 70 Hz tick before it, 40 ticks, for the trails (filled in by the
    /// renderer).
    pub low: [[f32; 4]; 10],
    /// The port's eight echo levels (6-bit blue), echo k being the frame 4k
    /// before; read from the trail palettes.
    pub echo_levels: [[f32; 4]; 2],
    /// The water (authored; dots_water.rs): part 4's water controls
    /// ([`WaterTuning::controls`], filled in by the renderer); where it
    /// lies ([`Water::controls`]); and the drops on it, their places
    /// across and back, ages and 1, or all 0 for none (filled in by the
    /// renderer).
    pub water: [f32; 4],
    pub water_plane: [f32; 4],
    pub drops: [[f32; 4]; MAX_DROPS],
    /// The lights in the water, as tuned ([`LightsInWater::controls`]):
    /// their ripples' strength, their glow's and the plankton's, of the
    /// authored, and how long the plankton glows (seconds).
    pub water_lights: [f32; 4],
}

/// The replay's position: a frame, the scene and video memory after it.
#[derive(Clone)]
struct Cursor {
    at: usize,
    dots: Dots,
    vga: Vga,
}

/// The original frames, replayed on demand from checkpoints every
/// [`REPLAY_INTERVAL`] frames, with the music position each step read.
struct Replay {
    checkpoints: Vec<Cursor>,
    music: Vec<u16>,
    cursor: Mutex<Option<Cursor>>,
}

impl Replay {
    fn with<R>(&self, j: usize, read: impl FnOnce(&Cursor) -> R) -> R {
        let j = j.min(self.music.len());
        let mut cursor = self.cursor.lock().unwrap_or_else(|e| e.into_inner());
        let base = (j / REPLAY_INTERVAL).min(self.checkpoints.len() - 1);
        let usable = matches!(&*cursor, Some(c) if c.at <= j && c.at >= base * REPLAY_INTERVAL);
        if !usable {
            *cursor = Some(self.checkpoints[base].clone());
        }
        let cursor = cursor.as_mut().unwrap();
        while cursor.at < j {
            cursor.dots.step(&mut cursor.vga, self.music[cursor.at]);
            cursor.at += 1;
        }
        read(cursor)
    }
}

/// Part 3 of the HD clip: from part 5's end to part 4's first frame.
pub struct DotsPreview {
    /// Clip frame of part 3's first frame (part 5's last, held). Part-3
    /// frame `j` is clip frame `start + j`.
    pub start: usize,
    /// Part 3's frames, then part 4's wait, then part 4's first frame.
    pub frames: Vec<DotsFrame>,
    /// The first frame of part 4 (its wait) and its first drawn frame, the
    /// last of `frames`.
    pub part4: usize,
    /// The distinct palettes the frames show, 768 6-bit DAC values each.
    pub palettes: Vec<Vec<u8>>,
    /// The scene's tables.
    pub tables: HdDotsTables,
    /// The objects as the modern view shows them: the tables', in floating
    /// point, the box reshaped to land on part 4's slab ([`land_on_slab`]).
    pub objects: Vec<Vec<[f64; 3]>>,
    /// Each dot frame's rotation, translation and morph (see [`DotFrame`]).
    pub dot_frames: Vec<DotFrame>,
    /// The camera-space points each dot frame drew, as the port computed
    /// them: 512 a frame.
    pub points: Vec<[i16; 3]>,
    /// The music's position at each frame but the last, as the step that
    /// follows it read it.
    pub music: Vec<u16>,
    /// Part 4's first frame: 320 x 400 indices and the palette.
    pub slab: (Vec<u8>, Vec<u8>),
    /// Clip times of the orders' first rows from the shade bobs on, as the
    /// music's position reads at each frame: the first frame of each new
    /// order.
    order_starts: Vec<f64>,
    /// The last dot frame as part 4's wait holds it.
    wait: (Vec<u8>, Vec<u8>),
    pub events: DotsEvents,
    /// The modern view's water (authored; dots_water.rs), and the ripples
    /// the lights set off in it, in time order.
    pub water: Water,
    pub ripples: Vec<Ripple>,
    /// The lights' ripples and glow in the water, as tuned.
    pub lights: LightsInWater,
    replay: Replay,
}

impl DotsPreview {
    /// Continue the clip from part 5's end: make the scene on `vga`, as the
    /// engine does in the step in which part 5 reports done (the step that
    /// starts `start`), and step it with the music until it reports done;
    /// then make part 4 and step it, still with the music, until it draws
    /// its first frame. The PCM of every step is appended but for that last
    /// frame's, so the clip ends at part 4's first drawn frame.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        start: usize,
    ) -> Result<Self, String> {
        let dots = Dots::new(data, vga)?;
        let tables = dots.hd_tables();
        let mut cursor = Cursor {
            at: 0,
            dots,
            vga: vga.clone(),
        };
        let mut checkpoints = Vec::new();
        let mut frames: Vec<DotsFrame> = Vec::new();
        let mut palettes: Vec<Vec<u8>> = Vec::new();
        let mut positions = Vec::new();
        let mut dot_frames = Vec::new();
        let mut points = Vec::new();
        let palette_of =
            |palettes: &mut Vec<Vec<u8>>, p: &[u8]| match palettes.iter().position(|q| q == p) {
                Some(i) => i,
                None => {
                    palettes.push(p.to_vec());
                    palettes.len() - 1
                }
            };
        let render = |pcm: &mut Vec<i16>, music: &mut Music, j: usize| {
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
        };
        loop {
            let j = frames.len();
            if j.is_multiple_of(REPLAY_INTERVAL) {
                checkpoints.push(cursor.clone());
            }
            if j > FRAME_RATE as usize * 120 {
                return Err("Part 3 exceeded its two-minute import limit".into());
            }
            let state = cursor.dots.hd_state();
            let palette = palette_of(&mut palettes, &cursor.vga.palette);
            frames.push(DotsFrame {
                state,
                phase: match state.drawn {
                    0 => Phase::Hold,
                    1 => Phase::Zoom,
                    2 => Phase::Shades,
                    _ => Phase::Dots,
                },
                palette,
            });
            let position = music.position();
            positions.push(position);
            let done = cursor.dots.step(&mut cursor.vga, position);
            cursor.at += 1;
            render(pcm, music, j);
            let next = cursor.dots.hd_state();
            if !done && next.drawn == 3 && next.frame > dot_frames.len() {
                dot_frames.push(dot_frame(&tables, &next));
                points.extend(cursor.dots.hd_points().iter().map(|p| p.map(|v| v as i16)));
            }
            if done {
                break;
            }
        }
        if dot_frames.is_empty() {
            return Err("Part 3 drew no dots".into());
        }
        // Part 4 waits for 29:00 holding the last dot frame, then draws its
        // logo slab in white at 400 lines.
        let wait = (cursor.vga.indices.clone(), cursor.vga.palette.clone());
        let held = cursor.dots.hd_state();
        let part4 = frames.len();
        let mut video = cursor.vga.clone();
        let mut plasma = Plasma::new(data, &mut video)?;
        loop {
            let j = frames.len();
            if j > part4 + FRAME_RATE as usize * 30 {
                return Err("Part 4 waited more than 30 seconds".into());
            }
            frames.push(DotsFrame {
                state: held,
                phase: Phase::Wait,
                palette: palette_of(&mut palettes, &video.palette),
            });
            let position = music.position();
            positions.push(position);
            plasma.step(&mut video, position);
            render(pcm, music, j);
            if video.height != 200 {
                break;
            }
        }
        frames.push(DotsFrame {
            state: held,
            phase: Phase::Slab,
            palette: palette_of(&mut palettes, &video.palette),
        });
        let slab = (video.indices.clone(), video.palette.clone());
        *vga = video;
        let events = DotsEvents::observe(&frames, part4);
        let shades = frames
            .iter()
            .position(|f| f.phase >= Phase::Shades)
            .unwrap_or(0);
        let mut objects = objects_of(&tables);
        let size = box_size(&objects[tables.order[7]]);
        let stone = Stone::fit(&plasma.hd_tables().logo, size)?;
        land_on_slab(&mut objects[tables.order[7]], dot_frames.last().unwrap(), &stone);
        let water = Water::new(events.slab);
        let ripples = water.ripples(&objects, &tables.order, &dot_frames, events.dots);
        let order_starts = (shades.max(1)..positions.len())
            .filter(|&j| positions[j] >> 8 != positions[j - 1] >> 8)
            .map(|j| (start + j) as f64 / FRAME_RATE as f64)
            .collect();
        Ok(Self {
            start,
            frames,
            part4,
            palettes,
            tables,
            objects,
            dot_frames,
            points,
            music: positions.clone(),
            slab,
            order_starts,
            wait,
            events,
            water,
            ripples,
            lights: LightsInWater::default(),
            replay: Replay {
                checkpoints,
                music: positions,
                cursor: Mutex::new(None),
            },
        })
    }
}

/// The tables' objects in floating point.
pub fn objects_of(tables: &HdDotsTables) -> Vec<Vec<[f64; 3]>> {
    let float = |p: &[i32; 3]| p.map(f64::from);
    tables.objects.iter().map(|o| o.iter().map(float).collect()).collect()
}

/// An object's extent along each axis: the box's width, depth and height.
fn box_size(points: &[[f64; 3]]) -> [f64; 3] {
    std::array::from_fn(|k| {
        let (lo, hi) = points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[k]), hi.max(p[k])));
        hi - lo
    })
}

/// Reshape the box, the last stage's object, so that at rest (the last dot
/// frame) it lies exactly on part 4's slab as part 4's camera shows it
/// (authored, at the owner's request; the original's box and logo slab
/// differ a little, and part 3's projection is 1.2 times narrower on the
/// display than the slab's fitted camera). Each point keeps its place on
/// the box: across it, from the top face's near edge to its far edge, and
/// down. That place on the slab, in the slab's camera space, is carried to
/// the point part 3's projection shows in the same place at the same
/// depth, and back through the rest pose into object space, so the box is
/// that shape from its stage's start and flies as before.
pub fn land_on_slab(points: &mut [[f64; 3]], rest: &DotFrame, stone: &Stone) {
    let r = rotation(std::array::from_fn(|k| rest.angles[k] as f64));
    let t: [f64; 3] = std::array::from_fn(|k| rest.translation[k] as f64);
    // Object axis k in camera space at rest: column k of r.
    let along = |k: usize| [r[0][k], r[1][k], r[2][k]];
    let mut axes = [0, 1, 2];
    // Across: the axis nearest the camera's x; then the top face's depth,
    // the nearer to the line of sight; the third is the height.
    axes.sort_by(|&a, &b| along(b)[0].abs().total_cmp(&along(a)[0].abs()));
    if along(axes[2])[2].abs() > along(axes[1])[2].abs() {
        axes.swap(1, 2);
    }
    let [across, deep, down] = axes;
    let (lo, hi): ([f64; 3], [f64; 3]) = (
        std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::MAX, f64::min)),
        std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::MIN, f64::max)),
    );
    // A coordinate's share of the way from the end whose camera
    // coordinate `c` is lower (nearer, or higher on the screen).
    let share = |p: &[f64; 3], k: usize, c: usize| {
        let x = (p[k] - lo[k]) / (hi[k] - lo[k]);
        if along(k)[c] >= 0.0 { x } else { 1.0 - x }
    };
    let (top, front) = stone.axes();
    for p in points.iter_mut() {
        let x = (share(p, across, 0) - 0.5) * stone.size[0];
        let d = share(p, deep, 2) * stone.size[1];
        let h = share(p, down, 1) * stone.size[2];
        let s: [f64; 3] = std::array::from_fn(|k| {
            stone.corner[k] + [x, 0.0, 0.0][k] + top[k] * d + front[k] * h
        });
        // Where part 3's projection (column 160 + 256 x / z, row 100 +
        // 256 y / z) shows what the slab's shows at s.
        let c = [
            (1.2 * stone.focal * s[0] + (stone.axis - 160.0) * s[2]) / 256.0,
            stone.focal * s[1] / 256.0,
            s[2],
        ];
        // Back through the rest pose: r is a rotation.
        let o: [f64; 3] = std::array::from_fn(|k| c[k] - t[k]);
        *p = std::array::from_fn(|k| (0..3).map(|i| r[i][k] * o[i]).sum());
    }
}

/// The GPU's record of the dot frame `state` drew.
fn dot_frame(tables: &HdDotsTables, state: &HdDots) -> DotFrame {
    let n = state.frame - 1;
    // The angles advance by (3, 4, 5) before each dot frame below 2601,
    // from (589, 860, 307); unwrapped here.
    let turns = (n.min(ROTATION_STOP - 1) + 1) as f64;
    let angles = [
        589.0 + 3.0 * turns,
        860.0 + 4.0 * turns,
        307.0 + 5.0 * turns,
    ];
    debug_assert!(
        angles
            .iter()
            .zip(state.angles)
            .all(|(&a, b)| (a as usize) & 1023 == b)
    );
    let translation = spline(tables, state.d as f64);
    let deadline = tables.script[state.script].0;
    let morph = (state.a - deadline + 128).clamp(0, 128) as f64 / 128.0;
    DotFrame {
        angles: [
            angles[0] as f32,
            angles[1] as f32,
            angles[2] as f32,
            state.script as f32,
        ],
        translation: [
            translation[0] as f32,
            translation[1] as f32,
            translation[2] as f32,
            morph as f32,
        ],
    }
}

/// The camera spline at parameter `d`, as the code sums it (control points
/// `d >> 8` to 3 more, weights at `d & 255`, over 32768) without its
/// truncation, linear between whole `d`.
pub fn spline(tables: &HdDotsTables, d: f64) -> [f64; 3] {
    let at = |d: i64| -> [f64; 3] {
        let k = (d >> 8) as usize;
        let t = (d & 255) as usize;
        std::array::from_fn(|axis| {
            (0..4)
                .map(|j| {
                    let p = tables.camera.get(k + j).map_or(0, |c| c[axis]);
                    p as f64 * tables.weights[j][t] as f64
                })
                .sum::<f64>()
                / 32768.0
        })
    };
    let base = d.floor();
    let (a, b) = (at(base as i64), at(base as i64 + 1));
    let f = d - base;
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * f)
}

impl DotsEvents {
    /// Read the events from the frames; one that never happens reads -1.
    pub fn observe(frames: &[DotsFrame], part4: usize) -> Self {
        let at = |j: Option<usize>| j.map_or(-1.0, |j| j as f64 / FRAME_RATE as f64);
        let first = |test: &dyn Fn(&DotsFrame) -> bool| frames.iter().position(test);
        let last = |test: &dyn Fn(&DotsFrame) -> bool| frames.iter().rposition(test);
        let shades = |f: &DotsFrame| f.phase == Phase::Shades;
        Self {
            zoom: at(first(&|f| f.phase == Phase::Zoom)),
            zoom_end: at(last(&|f| f.phase == Phase::Zoom)),
            shades: at(first(&shades)),
            shade_stop: at(first(&|f| {
                shades(f) && f.state.shade_frame > f.state.shade_stop
            })),
            shades_end: at(last(&shades)),
            dots: at(first(&|f| f.phase == Phase::Dots)),
            rotation_stop: at(first(&|f| {
                f.phase == Phase::Dots && f.state.frame > ROTATION_STOP
            })),
            dots_end: at(last(&|f| f.phase == Phase::Dots)),
            part4: at(Some(part4)),
            slab: at(Some(frames.len() - 1)),
        }
    }
}

impl DotsPreview {
    /// Part-3 frame position at a clip time: continuous, negative before
    /// part 3. A time within one audio sample of a frame's is that frame's:
    /// the clip's duration, a whole number of samples, falls a fraction of
    /// a sample short of part 4's slab, its last frame, which seeking to
    /// the end must show.
    pub fn position(&self, time: f64) -> f64 {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        if (position - position.round()).abs() <= FRAME_RATE as f64 / SAMPLE_RATE as f64 {
            position.round()
        } else {
            position
        }
    }

    /// Part-3 frame at a clip time, or `None` before part 3. Times past the
    /// end hold the last frame.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = self.position(time);
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Clip time of part-3 frame `j`.
    pub fn seconds(&self, j: usize) -> f64 {
        (self.start + j) as f64 / FRAME_RATE as f64
    }

    /// The original frame `j` as 8-bit RGBA, 320 x 200, as the reference
    /// scanout shows it; part 4's 400-line frame with each pair of lines
    /// averaged.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let j = j.min(self.frames.len() - 1);
        match self.frames[j].phase {
            Phase::Slab => {
                let full = super::field::rgba(&self.slab.0, &self.slab.1);
                let rows = full.len() / (320 * 4);
                (0..200)
                    .flat_map(|y| {
                        let (a, b) = (2 * y * rows / 400, (2 * y + 1) * rows / 400);
                        (0..320 * 4)
                            .map(|k| {
                                let (p, q) = (full[a * 1280 + k], full[b * 1280 + k]);
                                ((p as u16 + q as u16).div_ceil(2)) as u8
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            }
            Phase::Wait => super::field::rgba(&self.wait.0, &self.wait.1),
            _ => self
                .replay
                .with(j, |c| super::field::rgba(&c.vga.indices, &c.vga.palette)),
        }
    }

    /// Indices and palette of part-3 frame `j` (part 3's frames only),
    /// replayed with the port's scene.
    pub fn original(&self, j: usize) -> (Vec<u8>, Vec<u8>) {
        self.replay
            .with(j, |c| (c.vga.indices.clone(), c.vga.palette.clone()))
    }

    /// Title label for part-3 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        let frame = &self.frames[j.min(self.frames.len() - 1)];
        match frame.phase {
            Phase::Hold => "Part 3: part 5's frame held",
            Phase::Zoom => "Zoom into the window",
            Phase::Shades => "Shade bobs",
            Phase::Dots => {
                let stage = frame.state.script.min(7);
                let deadline = self.tables.script[stage].0;
                if stage < 7 && frame.state.a > deadline - 128 {
                    [
                        "Sphere lattice morphing",
                        "Vase morphing",
                        "Cube edges morphing",
                        "Line grid morphing",
                        "Slab lattice morphing",
                        "Cube lattice morphing",
                        "Pyramid edges morphing",
                        "Box",
                    ][stage]
                } else {
                    [
                        "Dots: sphere lattice",
                        "Vase",
                        "Cube edges",
                        "Line grid",
                        "Slab lattice",
                        "Cube lattice",
                        "Pyramid edges",
                        "Box",
                    ][stage]
                }
            }
            Phase::Wait => "Part 4: waiting, the box held",
            Phase::Slab => "Part 4: logo slab, white",
        }
    }

    /// The frame `j`, the fraction `f` of the way to the next, at a clip
    /// time.
    fn moment(&self, time: f64) -> (usize, f64) {
        let last = self.frames.len() - 1;
        let position = self.position(time).clamp(0.0, last as f64);
        let j = position.floor() as usize;
        (j, if j == last { 0.0 } else { position - j as f64 })
    }

    /// The continuous dot frame at a clip time: -1 before the dots. From
    /// the last, where the box lies still, it runs on at the same rate
    /// (authored): the exposure goes on, so the trails catch up with the
    /// lights, while the points stay where the last frame put them (the
    /// shaders and [`Self::centre_at`] hold them there).
    pub fn dot_position(&self, time: f64) -> f64 {
        let (j, f) = self.moment(time);
        let frame = &self.frames[j];
        if frame.phase < Phase::Dots {
            return -1.0;
        }
        let n = (frame.state.frame - 1) as f64;
        let next = self.frames.get(j + 1);
        let moves = frame.phase == Phase::Dots
            && next.is_some_and(|g| g.phase == Phase::Dots && g.state.frame > frame.state.frame);
        if moves {
            return n + f;
        }
        let seconds = time - self.start as f64 / FRAME_RATE as f64;
        n + (seconds - self.events.dots_end).max(0.0) * FRAME_RATE as f64
    }

    /// The object's centre in camera space at a continuous dot frame.
    pub fn centre_at(&self, tau: f64) -> [f64; 3] {
        let last = self.dot_frames.len() - 1;
        let tau = tau.clamp(0.0, last as f64);
        let k = tau.floor() as usize;
        let f = tau - k as f64;
        let a = self.dot_frames[k].translation;
        let b = self.dot_frames[(k + 1).min(last)].translation;
        std::array::from_fn(|i| a[i] as f64 + (b[i] - a[i]) as f64 * f)
    }

    /// The modern view's magnification `u` frames after the first zoom
    /// frame: the original's law, its scale falling by 256 a frame from
    /// 65535, carried on past its last scale (authored beyond it), down to
    /// a scale of 1024.
    pub fn modern_magnification(&self, u: f64) -> f64 {
        65536.0 / (65535.0 - 256.0 * u.max(0.0)).max(1024.0)
    }

    /// The modern view's magnification at which the window covers a
    /// drawable of `aspect`, with a margin for its blur: the window is 128
    /// source pixels wide, and the drawable shows 120 aspect / M of them
    /// each side of the centre; 100 rows, of which it shows 100 / M (from
    /// the window's 46 rows above the centre). Authored margin.
    pub fn flown_through(aspect: f64) -> f64 {
        (aspect * 120.0 / 64.0).max(100.0 / 46.0) * 1.15
    }

    /// Clip times of the orders' first rows from the shade bobs on.
    pub fn order_starts(&self) -> &[f64] {
        &self.order_starts
    }

    /// Everything part 3 puts in `u.dots` at a clip time on a drawable of
    /// `aspect`, but the music's cues, which the renderer adds.
    pub fn controls(&self, time: f64, aspect: f64) -> DotsBlock {
        let (j, f) = self.moment(time);
        let frame = &self.frames[j];
        let next = self.frames.get(j + 1).unwrap_or(frame);
        let seconds = time - self.seconds(0);
        let e = &self.events;
        let state = frame.state;
        // The zoom: the scale drawn, towards the next frame's while it zooms.
        let scale = |g: &DotsFrame| g.state.scale as f64;
        let original_m = match frame.phase {
            Phase::Hold => 1.0,
            Phase::Zoom => {
                let b = if next.phase == Phase::Zoom {
                    scale(next)
                } else {
                    scale(frame)
                };
                65536.0 / (scale(frame) + (b - scale(frame)) * f)
            }
            _ => 65536.0 / scale(&self.frames[(e.zoom_end * 70.0).round() as usize]),
        };
        let zoom_from = e.zoom * FRAME_RATE as f64;
        let u = self.position(time) - zoom_from;
        let modern_m = if u <= 0.0 {
            1.0
        } else {
            self.modern_magnification(u)
        };
        let rate = if u <= 0.0 {
            0.0
        } else {
            256.0 * FRAME_RATE as f64 / (65535.0 - 256.0 * u).max(1024.0)
        };
        let underlay = modern_m < Self::flown_through(aspect);
        // The shade bobs: iterations drawn, towards the next frame's.
        let shading = frame.phase == Phase::Shades;
        let shade_f = if shading && next.phase == Phase::Shades {
            f
        } else {
            0.0
        };
        let tau = self.dot_position(time);
        let centre = self.centre_at(tau.max(0.0));
        let since = |event: f64| {
            if event < 0.0 || seconds < event {
                -1.0
            } else {
                seconds - event
            }
        };
        let slab = frame.phase == Phase::Slab;
        // The orders' first rows before now, newest first.
        let mut hits = [-100.0_f32; 4];
        let starts = &self.order_starts;
        let now = starts.partition_point(|&t| t <= time);
        for (k, &t) in starts[..now].iter().rev().take(4).enumerate() {
            hits[k] = (time - t) as f32;
        }
        let levels = echo_levels(&self.tables.dot_palettes);
        DotsBlock {
            part: [
                1.0,
                seconds as f32,
                frame.phase as u8 as f32,
                (j as f64 + f) as f32,
            ],
            zoom: [
                original_m as f32,
                modern_m as f32,
                rate as f32,
                if underlay { 1.0 } else { 0.0 },
            ],
            shades: [
                (state.shade_frame as f64 + shade_f) as f32,
                state.shade_stop as f32,
                if shading { 1.0 } else { 0.0 },
                state.shade_frame as f32,
            ],
            dots: [
                tau as f32,
                (self.dot_frames.len() - 1) as f32,
                ROTATION_STOP as f32,
                if frame.phase >= Phase::Dots { 1.0 } else { 0.0 },
            ],
            focus: [
                centre[2] as f32,
                centre[0] as f32,
                centre[1] as f32,
                centre[2] as f32,
            ],
            end: [
                since(e.rotation_stop) as f32,
                (e.slab - seconds) as f32,
                if slab { 1.0 } else { 0.0 },
                since(e.slab) as f32,
            ],
            events: [e.zoom, e.zoom_end, e.shades, e.shade_stop].map(|v| v as f32),
            events_late: [e.shades_end, e.dots, e.rotation_stop, e.part4].map(|v| v as f32),
            hits,
            hit_strength: [0.0; 4],
            low: [[0.0; 4]; 10],
            echo_levels: [
                [levels[0], levels[1], levels[2], levels[3]],
                [levels[4], levels[5], levels[6], levels[7]],
            ],
            water: [0.0; 4],
            water_plane: self.water.controls(seconds),
            drops: [[0.0; 4]; MAX_DROPS],
            water_lights: self.lights.controls(),
        }
    }
}

/// The port's level for echo k (the frame 4k before), 6-bit blue, read from
/// the trail palettes: the colour with only that frame's bit set, in the
/// palette of the frame that shows it. Frame n draws bit (n mod 32) / 4 - 1
/// (0x80 rotated left by the group) and shows palette (n mod 32) / 4, so
/// echo k is bit -k - 1 of palette 0: the newest is the brightest.
pub fn echo_levels(dot_palettes: &[u8]) -> [f32; ECHOES] {
    std::array::from_fn(|k| {
        let bit = (2 * ECHOES - 1 - k) % ECHOES;
        dot_palettes[(1usize << bit) * 3 + 2] as f32
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    /// Trail palettes built as the code builds them (dots.rs `palettes`):
    /// for each phase and colour, the blue of the oldest-numbered age whose
    /// bit is set.
    fn invented_palettes() -> Vec<u8> {
        let mut p = vec![0; 6144];
        for phase in 0..8 {
            for color in 0..256 {
                for (age, v) in [15, 20, 25, 30, 35, 40, 45, 60].iter().enumerate() {
                    if color & (1 << ((phase + age) % 8)) != 0 {
                        p[phase * 768 + color * 3 + 2] = *v;
                    }
                }
            }
        }
        p
    }

    #[test]
    fn echo_levels_put_the_newest_echo_brightest() {
        let levels = echo_levels(&invented_palettes());
        assert_eq!(levels, [60.0, 45.0, 40.0, 35.0, 30.0, 25.0, 20.0, 15.0]);
        // Every frame shows its echoes at those levels: frame n draws bit
        // 0x80 rotated left by (n mod 32) / 4 and shows that palette.
        let p = invented_palettes();
        for n in 0..64usize {
            let phase = (n & 31) >> 2;
            for (k, &level) in levels.iter().enumerate() {
                let Some(m) = n.checked_sub(4 * k) else {
                    continue;
                };
                let bit = 0x80u8.rotate_left(((m & 31) >> 2) as u32) as usize;
                assert_eq!(
                    p[phase * 768 + bit * 3 + 2] as f32,
                    level,
                    "frame {n}, echo {k}"
                );
            }
        }
    }

    #[test]
    fn spline_sums_four_points_and_is_linear_between_whole_parameters() {
        let mut weights: [Vec<i32>; 4] = std::array::from_fn(|_| vec![0; 256]);
        // Invented weights: linear between the second and third points.
        let [_, second, third, _] = &mut weights;
        for (t, (a, b)) in second.iter_mut().zip(third.iter_mut()).enumerate() {
            *a = 32768 - 128 * t as i32;
            *b = 128 * t as i32;
        }
        let tables = HdDotsTables {
            objects: Vec::new(),
            order: [0; 8],
            script: [(0, 0); 9],
            camera: (0..8).map(|i| [100 * i, -50 * i, 7]).collect(),
            weights,
            path: Vec::new(),
            shade_palette: Vec::new(),
            dot_palettes: Vec::new(),
            sine: Vec::new(),
        };
        // d = 256 + 64: points 1 to 4, a quarter of the way from 2 to 3.
        let at = spline(&tables, 320.0);
        assert!((at[0] - 225.0).abs() < 1e-9 && (at[1] + 112.5).abs() < 1e-9);
        assert!((at[2] - 7.0).abs() < 1e-9);
        let half = spline(&tables, 320.5);
        let next = spline(&tables, 321.0);
        assert!((half[0] - (at[0] + next[0]) / 2.0).abs() < 1e-9);
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn the_clip_ends_on_part4s_slab() {
        let data = crate::test_support::data();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let dots = clip.dots.as_ref().unwrap();
        let last = dots.frames.len() - 1;
        assert_eq!(dots.frames[last].phase, Phase::Slab);
        // Seeking to the end, as --seek and End clamp to the duration.
        assert_eq!(dots.frame_at(clip.duration()), Some(last));
        assert_eq!(dots.frame_at(dots.seconds(last) - 0.001), Some(last - 1));
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn sections_start_where_the_labels_change() {
        let data = crate::test_support::data();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let sections = clip.sections();
        let frame = 1.0 / crate::engine::FRAME_RATE as f64;
        assert_eq!(sections[0], 0.0);
        assert!(sections.windows(2).all(|w| w[0] < w[1]));
        for &s in &sections[1..] {
            assert_ne!(clip.label(s), clip.label(s - frame));
            println!("{s:8.3} {}", clip.label(s));
        }
        // Part 3's phases and its first and last objects each start one.
        for label in [
            "Zoom into the window",
            "Shade bobs",
            "Dots: sphere lattice",
            "Box",
        ] {
            assert!(sections.iter().any(|&s| clip.label(s) == label), "{label}");
        }
    }

    #[test]
    fn the_box_lands_on_the_slab() {
        use super::super::plasma_stone::{Relief, Sheet};
        // An invented slab and camera, near the fit to the logo.
        let relief = || Relief {
            size: (0, 0),
            texels: Vec::new(),
        };
        let stone = Stone {
            focal: 262.8,
            axis: 161.0,
            tilt: 0.546,
            corner: [0.0, 147.8, 503.6],
            size: [490.0, 194.0, 100.0],
            colour: [0.0; 3],
            sheets: [Sheet::default(); 2],
            top: relief(),
            front: relief(),
            residual: 0.0,
            ground_tilt: 0.0,
            ground_depth: 0.0,
        };
        // The port's box, its corners and a point midway along an edge, at
        // the rest pose part 3 ends on: its x across, +y the near edge and
        // +z down.
        let mut points = Vec::new();
        for k in 0..8 {
            let sign = |bit: usize| if k & bit == 0 { -1.0 } else { 1.0 };
            points.push([245.0 * sign(1), 100.0 * sign(2), 50.0 * sign(4)]);
        }
        points.push([0.0, 100.0, -50.0]);
        let rest = DotFrame {
            angles: [8392.0, 11264.0, 13312.0, 7.0],
            translation: [0.0, 129.8, 549.8, 0.0],
        };
        let before = points.clone();
        land_on_slab(&mut points, &rest, &stone);
        let r = rotation([8392.0, 11264.0, 13312.0]);
        let shown = |p: &[f64; 3]| {
            let c: [f64; 3] = std::array::from_fn(|i| {
                (0..3).map(|j| r[i][j] * p[j]).sum::<f64>() + rest.translation[i] as f64
            });
            (160.0 + 256.0 * c[0] / c[2], 100.0 + 256.0 * c[1] / c[2], c[2])
        };
        let (top, front) = stone.axes();
        for (o, p) in before.iter().zip(&points) {
            let x = o[0];
            let d = (100.0 - o[1]) / 200.0 * stone.size[1];
            let h = (o[2] + 50.0) / 100.0 * stone.size[2];
            let s: [f64; 3] = std::array::from_fn(|k| {
                stone.corner[k] + [x, 0.0, 0.0][k] + top[k] * d + front[k] * h
            });
            let want = stone.project(s);
            let got = shown(p);
            assert!(
                (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
                "{o:?} shows at ({:.3}, {:.3}), the slab's at ({:.3}, {:.3})",
                got.0,
                got.1,
                want.0,
                want.1
            );
            // At the slab's own depth.
            assert!((got.2 - s[2]).abs() < 1e-6);
        }
    }

    #[test]
    fn modern_zoom_follows_the_original_law_and_flies_through() {
        // At each zoom frame the modern magnification is the scale the
        // original drew: 65535 less 256 a frame.
        let m = |u: f64| 65536.0 / (65535.0 - 256.0 * u).max(1024.0);
        assert!((m(0.0) - 65536.0 / 65535.0).abs() < 1e-12);
        assert!((m(166.0) - 65536.0 / (65535.0 - 256.0 * 166.0)).abs() < 1e-12);
        // A 4:3 drawable is covered by the window at the original's 2.85,
        // within the margin; 16:9 needs more.
        let four_three = DotsPreview::flown_through(4.0 / 3.0);
        assert!(four_three > 2.5 && four_three < 3.0);
        assert!(DotsPreview::flown_through(16.0 / 9.0) > 3.3);
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part3_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let started = std::time::Instant::now();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let loaded = started.elapsed().as_secs_f64();
        let field = clip.field.as_ref().unwrap();
        let dots = clip.dots.as_ref().unwrap();
        assert_eq!(dots.start, field.start + field.frames.len());
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in 0..dots.start {
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(
            engine.part(),
            3,
            "part 3 starts at clip frame {}",
            dots.start
        );
        let mut worst = 0.0_f64;
        let mut stray = 0;
        for (j, frame) in dots.frames.iter().enumerate() {
            let video = &engine.video;
            if frame.phase < Phase::Wait {
                assert_eq!(engine.part(), 3, "part 3 at {j}");
                let (indices, palette) = dots.original(j);
                assert!(
                    indices == video.indices,
                    "replayed pixels at part-3 frame {j}"
                );
                assert!(
                    palette == video.palette,
                    "replayed palette at part-3 frame {j}"
                );
            } else {
                assert_eq!(engine.part(), 4, "part 4 at {j}");
            }
            assert!(
                dots.palettes[frame.palette] == video.palette,
                "palette at {j}"
            );
            let rgba = dots.original_rgba(j);
            if frame.phase != Phase::Slab {
                assert!(rgba == super::super::field::rgba(&video.indices, &video.palette));
            } else {
                assert_eq!((video.width, video.height), (320, 400));
                assert_eq!(rgba.len(), 320 * 200 * 4);
                assert!(dots.slab.0 == video.indices);
            }
            // Each dot frame: the float camera against the port's points,
            // and the projection: every point the port plotted, and none
            // else, is where column 160 + x and row 100 + y put it.
            if frame.phase == Phase::Dots && j + 1 < dots.frames.len() {
                let n = frame.state.frame - 1;
                let d = dots.dot_frames[n];
                let [ax, ay, az] = [d.angles[0], d.angles[1], d.angles[2]]
                    .map(|a| a as f64 * std::f64::consts::TAU / 1024.0);
                let (sx, sy, sz) = (ax.sin(), ay.sin(), az.sin());
                let (cx, cy, cz) = (ax.cos(), ay.cos(), az.cos());
                let r = [
                    [cy * cz, cy * sz, -sy],
                    [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, cy * sx],
                    [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cy * cx],
                ];
                let stage = d.angles[3] as usize;
                let base = &dots.tables.objects[dots.tables.order[stage]];
                let target = &dots.tables.objects[dots.tables.order[(stage + 1).min(7)]];
                let mut lit = vec![false; 64000];
                for i in 0..POINTS {
                    let m = d.translation[3] as f64;
                    let obj: [f64; 3] = std::array::from_fn(|k| {
                        base[i][k] as f64 + (target[i][k] - base[i][k]) as f64 * m
                    });
                    let p = dots.points[n * POINTS + i];
                    for row in 0..3 {
                        let v = (0..3).map(|k| r[row][k] * obj[k]).sum::<f64>()
                            + d.translation[row] as f64;
                        worst = worst.max((v - p[row] as f64).abs());
                    }
                    let (x, y) = (
                        p[0] as i32 * 256 / p[2] as i32,
                        p[1] as i32 * 256 / p[2] as i32,
                    );
                    if (-160..160).contains(&x) && (-100..100).contains(&y) {
                        lit[(100 + y) as usize * 320 + (160 + x) as usize] = true;
                    } else if !(-160..160).contains(&x) || y.abs() < 300 {
                        // Off screen: the tables park it past the page.
                    } else {
                        stray += 1;
                    }
                }
                // The newest frame's bit, set exactly where its points are.
                let bit = 0x80u8.rotate_left(((n & 31) >> 2) as u32);
                for (at, &lit) in lit.iter().enumerate() {
                    assert_eq!(
                        video.indices[at] & bit != 0,
                        lit,
                        "dot frame {n} pixel {at}"
                    );
                }
            }
            if j + 1 < dots.frames.len() {
                pcm.extend(engine.step().unwrap());
            }
        }
        assert_eq!(engine.part(), 4);
        assert!(pcm == clip.pcm, "PCM");
        assert_eq!(stray, 0, "points the tables would wrap onto the screen");
        let e = dots.events;
        println!(
            "Loaded in {loaded:.2}s. Part 3 from {:.3}s: zoom {:.3}-{:.3}, shades {:.3}, stop {:.3}, \
             end {:.3}, dots {:.3}, rotation stops {:.3}, dots end {:.3}, part 4 {:.3}, slab {:.3}; \
             float camera within {worst:.2} of the port's points",
            dots.seconds(0),
            e.zoom + dots.seconds(0),
            e.zoom_end + dots.seconds(0),
            e.shades + dots.seconds(0),
            e.shade_stop + dots.seconds(0),
            e.shades_end + dots.seconds(0),
            e.dots + dots.seconds(0),
            e.rotation_stop + dots.seconds(0),
            e.dots_end + dots.seconds(0),
            e.part4 + dots.seconds(0),
            e.slab + dots.seconds(0),
        );
        println!("Echo levels {:?}", echo_levels(&dots.tables.dot_palettes));
        for &t in dots.order_starts() {
            let cue = clip.audio_at(t + 0.03).intro;
            println!(
                "order {:02X} at {t:.3}: energy {:.2} onset {:.2} low {:.2} accent {:.2}",
                dots.music[((t * 70.0).round() as usize) - dots.start] >> 8,
                cue[0],
                cue[1],
                cue[2],
                cue[3]
            );
        }
        assert!(worst < 8.0, "float camera {worst} from the port's points");
    }
}
