//! Part 3's water, for the modern view: part 4's water (water.wgsl and
//! [`WaterTuning`]) beneath part 3's objects, and the ground part 4's slab
//! is seen over. All of it is authored. What comes from where:
//! - Read from the port (through [`DotsPreview`]): the objects' motion,
//!   from which the lights meeting the water are found.
//! - Authored: the water lying level beneath the objects, LEVEL_HEIGHT below
//!   the camera (tuned live: its tilt and depth), through part 3 and on
//!   into part 4 (plasma_stone.rs); the ripples the lights set off and the
//!   glow they give it (their places and times found from the port's
//!   motion, their strength authored); the drops' times and places; and
//!   everything dots_light.wgsl draws.
//!
//! The objects fly at the camera's height, along its axis, from about 500
//! to 2,300 units away; the water lies just below them, so they fly
//! through it, their lower parts under it.
use super::dots::{DotFrame, DotsEvents, DotsPreview};
use super::plasma::{DROP_LIFE, MAX_DROPS, WaterTuning, drop_hash};
use crate::engine::FRAME_RATE;
use std::collections::HashMap;

/// Where the water lies by default (authored, chosen by eye with the
/// preview's keys): tilted LEVEL_TILT (radians, 3 degrees), rising further
/// off, and rising through part 3, eased, from START_HEIGHT below the
/// camera (in the dots' camera units) at its first frame to LEVEL_HEIGHT at
/// its last, where part 4's ground takes it on. The objects fly ever
/// deeper through it, their lower parts passing under it at the end; the
/// shade bobs' tips, as dots_light.wgsl places them (SHADE_DEPTH), stay
/// above it at the lowest of their path even as it lies at the end.
pub const START_HEIGHT: f64 = 400.0;
pub const LEVEL_HEIGHT: f64 = 100.0;
pub const LEVEL_TILT: f64 = 3.0 * std::f64::consts::PI / 180.0;
/// The drops (authored): the first this long after the dots begin, the
/// last in time to fall before the rotation stops; each where a point of
/// the screen, rows DROP_ROWS and DROP_ACROSS scene rows either side of
/// the centre line, meets the level water: below the objects, where their
/// mirror image is.
const DROP_FIRST: f64 = 2.0;
const DROP_ROWS: (f64, f64) = (140.0, 195.0);
const DROP_ACROSS: (f64, f64) = (10.0, 160.0);
/// The drops' seed, so they fall at other times than part 4's.
const DROP_SEED: u64 = 3;
/// The ripples (authored): a light stirs the water along its path while it
/// is in it (below CONTACT above it, so the box's base, settling onto it,
/// does too), setting off rings as a drop does (water.wgsl's rings, for as
/// long) all along the path. Its path through the water is cut into
/// stretches of at most RIPPLE_FRAMES dot frames, each a ripple along that
/// line, its rings starting from each point as the light passes. How hard
/// a stretch stirs: the light's speed through the water over RIPPLE_SPEED,
/// at most 1, less the deeper it runs (by e each STIR_DEPTH), averaged
/// over the stretch. Stretches that start within RIPPLE_BIN seconds and
/// whose middles lie within RIPPLE_CELL units of each other are one ripple
/// (the lights of an object moving together), along their mean line, its
/// strength, a drop's at 1, the root of the sum, at most RIPPLE_MOST.
/// Stretches stirring less than RIPPLE_LEAST (a light lying still) set off
/// none.
/// dots_light.wgsl has room for MAX_RIPPLES at once, the newest, more than
/// the port's motion ever sets running (a test checks; about 260 at the
/// default plankton's lifetime).
const CONTACT: f64 = 1.0;
const RIPPLE_FRAMES: usize = 24;
const RIPPLE_BIN: f64 = 0.5;
const RIPPLE_CELL: f64 = 120.0;
const RIPPLE_SPEED: f64 = 120.0;
const RIPPLE_MOST: f64 = 3.0;
const RIPPLE_LEAST: f64 = 0.02;
const STIR_DEPTH: f64 = 15.0;
pub const MAX_RIPPLES: usize = 320;
/// How long the plankton is stirred around a ripple, at most (seconds;
/// dots_light.wgsl stirs it while the rings pass), and for how many of its
/// lifetimes after that it is kept glowing (by then its light has fallen
/// below 2%).
const PLANKTON_STIRRED: f64 = 1.2;
const PLANKTON_LIVES: f64 = 4.0;

/// The lights in the water light it (dots_light.wgsl), each run of
/// UNDER_GROUP lights, neighbours in the object, as one at their mean
/// place; room for all 512 lights' runs.
const UNDER_GROUP: usize = 4;
pub const MAX_UNDER: usize = 128;
/// The lights' effects on the water, tuned live (the preview's Shift with
/// the number keys), in steps of x1.25 from the authored (dots_light.wgsl):
/// their ripples' strength, their glow under the surface, the glow of the
/// plankton they stir (its brightness and how long it lasts). Each but
/// the last reaches 0 one step below its lowest. By default (chosen by
/// eye with the keys) the ripples are 3 steps stronger (about x1.95), the
/// glow 8 (about x6), the plankton 6 weaker (about x0.26), glowing 3 steps
/// longer (1.17 s).
const LIGHT_STEPS: (i32, i32) = (-10, 12);
const LIGHT_STEP: f64 = 1.25;
/// How long the plankton glows, authored, in seconds: its light falls by
/// e in this time once the water is still; tuned between LIFE_STEPS (about
/// 0.2 to 3 s), so the ripples kept for it stay within MAX_RIPPLES.
const PLANKTON_LIFE: f64 = 0.6;
const LIFE_STEPS: (i32, i32) = (-6, 6);

/// The lights' effects on part 3's water, as tuned (see LIGHT_STEPS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightsInWater {
    pub ripples: i32,
    pub glow: i32,
    pub plankton: i32,
    pub plankton_life: i32,
}

impl Default for LightsInWater {
    fn default() -> Self {
        Self {
            ripples: 3,
            glow: 8,
            plankton: -6,
            plankton_life: 3,
        }
    }
}

/// The knobs, as the preview's Shift+number keys step them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKnob {
    Ripples,
    Glow,
    Plankton,
    PlanktonLife,
}

impl LightsInWater {
    fn gain(steps: i32) -> f64 {
        if steps < LIGHT_STEPS.0 {
            0.0
        } else {
            LIGHT_STEP.powi(steps)
        }
    }
    /// The ripples' strength, the glow's and the plankton's, of the
    /// authored, and how long the plankton glows (seconds).
    pub fn ripple_gain(&self) -> f64 {
        Self::gain(self.ripples)
    }
    pub fn glow_gain(&self) -> f64 {
        Self::gain(self.glow)
    }
    pub fn plankton_gain(&self) -> f64 {
        Self::gain(self.plankton)
    }
    pub fn plankton_life(&self) -> f64 {
        PLANKTON_LIFE * LIGHT_STEP.powi(self.plankton_life)
    }
    /// For dots_light.wgsl: the ripples' strength, the glow's, the
    /// plankton's and how long it glows.
    pub fn controls(&self) -> [f32; 4] {
        [
            self.ripple_gain(),
            self.glow_gain(),
            self.plankton_gain(),
            self.plankton_life(),
        ]
        .map(|v| v as f32)
    }
    /// Step a knob down or up, and say where they are.
    pub fn tune(&mut self, knob: LightKnob, up: bool) -> String {
        let step = if up { 1 } else { -1 };
        let (steps, range) = match knob {
            LightKnob::Ripples => (&mut self.ripples, (LIGHT_STEPS.0 - 1, LIGHT_STEPS.1)),
            LightKnob::Glow => (&mut self.glow, (LIGHT_STEPS.0 - 1, LIGHT_STEPS.1)),
            LightKnob::Plankton => (&mut self.plankton, (LIGHT_STEPS.0 - 1, LIGHT_STEPS.1)),
            LightKnob::PlanktonLife => (&mut self.plankton_life, LIFE_STEPS),
        };
        *steps = (*steps + step).clamp(range.0, range.1);
        format!("Part 3 water: {}", self.title())
    }
    /// The settings for the window's title.
    pub fn title(&self) -> String {
        format!(
            "ripples ×{:.2}, glow under ×{:.2}, plankton ×{:.2} for {:.2} s",
            self.ripple_gain(),
            self.glow_gain(),
            self.plankton_gain(),
            self.plankton_life()
        )
    }
}

/// A ripple along a stretch of a light's path through the water: when the
/// light passed its ends (continuous dot frames), where they are (across,
/// and back from below the camera, as dots_light.wgsl's water has them)
/// and how strong it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ripple {
    pub at: f64,
    pub until: f64,
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub strength: f64,
}

/// Where part 3's water lies at a moment, in the dots' camera space (x
/// right, y down, z away, a point shown at column 160 + 256 x / z, row
/// 100 + 256 y / z): tilted by `tilt` (radians; tilted up, it rises further
/// off) its up normal is (0, -cos, -sin); the camera stands `height` above
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub tilt: f64,
    pub height: f64,
}

/// Part 3's water over the part (authored; its tilt and its last height
/// tuned live by the preview's Shift+1/2 and Shift+-/=): tilted `tilt`, the
/// camera `start` above it at part 3's first frame and `height` above it
/// at its last, `until` part-3 seconds on (part 4's slab frame), between
/// them rising, eased.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    pub tilt: f64,
    pub start: f64,
    pub height: f64,
    pub until: f64,
}

/// The level water's tuning: its tilt a step (radians) and the range, and
/// its height's step (a factor) and range.
const LEVEL_TILT_STEP: f64 = std::f64::consts::PI / 180.0;
const LEVEL_TILTS: (f64, f64) = (-15.0, 30.0);
const LEVEL_HEIGHT_STEP: f64 = 1.1;
const LEVEL_HEIGHTS: (f64, f64) = (40.0, 700.0);

/// The level water's knobs, as the preview's keys step them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelKnob {
    Tilt,
    Height,
}

/// The rotation's rows at angles `a` (1024ths of a turn), as
/// dots_common.wgsl's `rotation` has them: camera = R object + translation.
pub(super) fn rotation(a: [f64; 3]) -> [[f64; 3]; 3] {
    let r = a.map(|v| v * std::f64::consts::TAU / 1024.0);
    let (s, c) = (r.map(f64::sin), r.map(f64::cos));
    [
        [c[1] * c[2], c[1] * s[2], -s[1]],
        [
            s[0] * s[1] * c[2] - c[0] * s[2],
            s[0] * s[1] * s[2] + c[0] * c[2],
            c[1] * s[0],
        ],
        [
            c[0] * s[1] * c[2] + s[0] * s[2],
            c[0] * s[1] * s[2] - s[0] * c[2],
            c[1] * c[0],
        ],
    ]
}

/// Each point of the object at dot frame `f`, in camera space: the shaders'
/// object_point, rotation and translation, in f64, of the objects as the
/// modern view shows them ([`super::dots::DotsPreview::objects`]) in the
/// stages' `order`.
pub fn camera_points(objects: &[Vec<[f64; 3]>], order: &[usize; 8], f: &DotFrame) -> Vec<[f64; 3]> {
    let r = rotation(std::array::from_fn(|k| f.angles[k] as f64));
    let stage = (f.angles[3] as usize).min(7);
    let a = &objects[order[stage]];
    let b = &objects[order[(stage + 1).min(7)]];
    let morph = if stage == 7 {
        0.0
    } else {
        f.translation[3] as f64
    };
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let o: [f64; 3] = std::array::from_fn(|j| p[j] + (q[j] - p[j]) * morph);
            std::array::from_fn(|i| {
                (0..3).map(|j| r[i][j] * o[j]).sum::<f64>() + f.translation[i] as f64
            })
        })
        .collect()
}

impl Level {
    /// The height above the water of camera point `c`.
    pub fn height_of(&self, c: [f64; 3]) -> f64 {
        self.height - self.tilt.cos() * c[1] - self.tilt.sin() * c[2]
    }

    /// Where camera point `c` lies over the water: across it (the
    /// display's square units) and back from below the camera, as
    /// dots_light.wgsl's water has them.
    pub fn place_of(&self, c: [f64; 3]) -> [f64; 2] {
        let (height, (s, k)) = (self.height, self.tilt.sin_cos());
        let h = self.height_of(c);
        // The point on the water below c, from the point below the camera.
        let (y, z) = (c[1] + h * k - height * k, c[2] + h * s - height * s);
        [c[0] * 5.0 / 6.0, -s * y + k * z]
    }

    /// Where the ray through scene point (`across`, rows from the centre
    /// line; `row`, the source row) meets the water, as
    /// [`Level::place_of`] has it; None where it passes above it.
    pub fn place_through(&self, across: f64, row: f64) -> Option<[f64; 2]> {
        let (s, c) = self.tilt.sin_cos();
        let d = [across, row - 100.0, 256.0];
        let toward = c * d[1] + s * d[2];
        if toward <= 1e-6 {
            return None;
        }
        let t = self.height / toward;
        Some(self.place_of([d[0] * t * 6.0 / 5.0, d[1] * t, d[2] * t]))
    }
}

impl Water {
    /// Part 3's water by default, for a part `until` seconds long.
    pub fn new(until: f64) -> Self {
        Self {
            tilt: LEVEL_TILT,
            start: START_HEIGHT,
            height: LEVEL_HEIGHT,
            until,
        }
    }

    /// Where it lies at part-3 `seconds`.
    pub fn at(&self, seconds: f64) -> Level {
        let s = (seconds / self.until.max(1e-6)).clamp(0.0, 1.0);
        Level {
            tilt: self.tilt,
            height: self.start + (self.height - self.start) * s * s * (3.0 - 2.0 * s),
        }
    }

    /// The ripples the lights set off over the dots (see CONTACT): each
    /// light's path through the water as it lay then, between each dot
    /// frame and the next, cut into stretches. The first dot frame is
    /// part-3 second `first`.
    pub fn ripples(
        &self,
        objects: &[Vec<[f64; 3]>],
        order: &[usize; 8],
        dot_frames: &[DotFrame],
        first: f64,
    ) -> Vec<Ripple> {
        /// A stretch being gathered: its start and end, the sum of its
        /// pieces' stirring, and how many.
        #[derive(Clone, Copy)]
        struct Stretch {
            at: f64,
            until: f64,
            from: [f64; 2],
            to: [f64; 2],
            stir: f64,
            pieces: usize,
        }
        #[derive(Default)]
        struct Merged {
            at: f64,
            until: f64,
            from: [f64; 2],
            to: [f64; 2],
            count: f64,
            stir: f64,
        }
        let rate = FRAME_RATE as f64;
        let mut merged: HashMap<[i64; 3], Merged> = HashMap::new();
        let mut finish = |s: Stretch| {
            let stir = s.stir / s.pieces as f64;
            if stir < RIPPLE_LEAST {
                return;
            }
            let middle = [(s.from[0] + s.to[0]) / 2.0, (s.from[1] + s.to[1]) / 2.0];
            let key = [
                (s.at / rate / RIPPLE_BIN).floor() as i64,
                (middle[0] / RIPPLE_CELL).floor() as i64,
                (middle[1] / RIPPLE_CELL).floor() as i64,
            ];
            let m = merged.entry(key).or_default();
            m.at += s.at;
            m.until += s.until;
            m.from = std::array::from_fn(|i| m.from[i] + s.from[i]);
            m.to = std::array::from_fn(|i| m.to[i] + s.to[i]);
            m.count += 1.0;
            m.stir += stir;
        };
        let frame = |k: usize| {
            let level = self.at(first + k as f64 / rate);
            (level, camera_points(objects, order, &dot_frames[k]))
        };
        let mut open: Vec<Option<Stretch>> = vec![None; objects[0].len()];
        let mut before = frame(0);
        for k in 1..dot_frames.len() {
            let after = frame(k);
            let (l0, l1) = (before.0, after.0);
            for (i, (&c0, &c1)) in before.1.iter().zip(&after.1).enumerate() {
                let h0 = l0.height_of(c0);
                let h1 = l1.height_of(c1);
                let (in0, in1) = (h0 < CONTACT, h1 < CONTACT);
                if !in0 && !in1 {
                    if let Some(s) = open[i].take() {
                        finish(s);
                    }
                    continue;
                }
                // The part of this frame's piece in the water.
                let cross = (h0 - CONTACT) / (h0 - h1);
                let (f0, f1) = (if in0 { 0.0 } else { cross }, if in1 { 1.0 } else { cross });
                let at = |f: f64| -> ([f64; 3], f64) {
                    let c = std::array::from_fn(|j| c0[j] + (c1[j] - c0[j]) * f);
                    (c, h0 + (h1 - h0) * f)
                };
                let ((a, ha), (b, hb)) = (at(f0), at(f1));
                let (p0, p1) = (l0.place_of(a), l1.place_of(b));
                let moved = (0..3).map(|j| (b[j] - a[j]).powi(2)).sum::<f64>().sqrt();
                let speed = moved / (f1 - f0).max(1e-6) * rate;
                let depth = (-(ha + hb) / 2.0).max(0.0);
                let stir = (speed / RIPPLE_SPEED).min(1.0) * (-depth / STIR_DEPTH).exp();
                let (s0, s1) = ((k - 1) as f64 + f0, (k - 1) as f64 + f1);
                let s = open[i].get_or_insert(Stretch {
                    at: s0,
                    until: s0,
                    from: p0,
                    to: p0,
                    stir: 0.0,
                    pieces: 0,
                });
                s.until = s1;
                s.to = p1;
                s.stir += stir;
                s.pieces += 1;
                if !in1 || s.pieces >= RIPPLE_FRAMES {
                    let done = open[i].take().unwrap();
                    finish(done);
                }
            }
            before = after;
        }
        for s in open.into_iter().flatten() {
            finish(s);
        }
        let mut ripples: Vec<Ripple> = merged
            .into_values()
            .map(|m| Ripple {
                at: m.at / m.count,
                until: m.until / m.count,
                from: m.from.map(|v| v / m.count),
                to: m.to.map(|v| v / m.count),
                strength: m.stir.sqrt().min(RIPPLE_MOST),
            })
            .collect();
        ripples.sort_by(|a, b| a.at.total_cmp(&b.at).then(a.from[0].total_cmp(&b.from[0])));
        ripples
    }

    /// The controls at part-3 `seconds`: the tilt, the camera's height
    /// above the water, and two unused.
    pub fn controls(&self, seconds: f64) -> [f32; 4] {
        let level = self.at(seconds);
        [level.tilt as f32, level.height as f32, 0.0, 0.0]
    }

    /// The drops on the water at part-3 `seconds`, by `tuning`'s rate
    /// (authored; see DROP_ROWS): from DROP_FIRST after the dots begin to
    /// the rotation's stop, places across and back from below the camera.
    pub fn drops(
        &self,
        tuning: &WaterTuning,
        events: &DotsEvents,
        seconds: f64,
    ) -> [[f32; 4]; MAX_DROPS] {
        if events.dots < 0.0 || events.rotation_stop < 0.0 {
            return [[0.0; 4]; MAX_DROPS];
        }
        let span = (events.dots + DROP_FIRST, events.rotation_stop + DROP_LIFE);
        tuning.drops(seconds, span, DROP_SEED, |k, at| {
            let h = |i: u64| drop_hash(k + (DROP_SEED << 40), 0, i);
            let side = if h(0) < 0.5 { -1.0 } else { 1.0 };
            let across = side * (DROP_ACROSS.0 + (DROP_ACROSS.1 - DROP_ACROSS.0) * h(1));
            let row = DROP_ROWS.0 + (DROP_ROWS.1 - DROP_ROWS.0) * h(2);
            self.at(at).place_through(across, row)
        })
    }
}

impl DotsPreview {
    /// Step the water's tilt (1 degree) or its height at the end (x1.1)
    /// down or up, find the ripples again on it, and say where it lies.
    pub fn tune_level(&mut self, knob: LevelKnob, up: bool) -> String {
        let w = &mut self.water;
        match knob {
            LevelKnob::Tilt => {
                let step = if up { 1.0 } else { -1.0 };
                let degrees = (w.tilt / LEVEL_TILT_STEP).round() + step;
                w.tilt = degrees.clamp(LEVEL_TILTS.0, LEVEL_TILTS.1) * LEVEL_TILT_STEP;
            }
            LevelKnob::Height => {
                let step = if up {
                    LEVEL_HEIGHT_STEP
                } else {
                    1.0 / LEVEL_HEIGHT_STEP
                };
                w.height = (w.height * step).clamp(LEVEL_HEIGHTS.0, LEVEL_HEIGHTS.1);
            }
        }
        self.ripples = self
            .water
            .ripples(&self.objects, &self.tables.order, &self.dot_frames, self.events.dots);
        format!("Part 3 water: {}", self.level_title())
    }

    /// The level water for the window's title.
    pub fn level_title(&self) -> String {
        format!(
            "water tilted {:.0}°, rising from {:.0} to {:.0} below",
            self.water.tilt.to_degrees(),
            self.water.start,
            self.water.height
        )
    }

    /// The ripples on the water at a clip time, for dots_light.wgsl: the
    /// MAX_RIPPLES newest still running or glowing, newest first, each its
    /// line (as far as the light has gone along it), then its age at its
    /// start (seconds), how long the light took along it, its strength
    /// (untuned) and 0; then zeros.
    pub fn ripples_at(&self, time: f64) -> [[f32; 8]; MAX_RIPPLES] {
        let mut out = [[0.0; 8]; MAX_RIPPLES];
        if self.events.dots < 0.0 {
            return out;
        }
        // A dot frame a frame: frame n at part-3 seconds dots + n / rate.
        // Kept for a drop's life, or while the plankton it stirs glows
        // (its last stirring, then PLANKTON_LIVES of its lifetimes).
        let since = (time - self.seconds(0) - self.events.dots) * FRAME_RATE as f64;
        let rate = FRAME_RATE as f64;
        let keep =
            DROP_LIFE.max(PLANKTON_STIRRED + PLANKTON_LIVES * self.lights.plankton_life()) * rate;
        let end = self.ripples.partition_point(|r| r.at <= since);
        let running = self.ripples[..end]
            .iter()
            .rev()
            .filter(|r| since - r.until < keep)
            .take(MAX_RIPPLES);
        for (slot, r) in out.iter_mut().zip(running) {
            let ran = (since.min(r.until) - r.at).max(0.0);
            let u = if r.until > r.at {
                ran / (r.until - r.at)
            } else {
                1.0
            };
            let to: [f64; 2] = std::array::from_fn(|j| r.from[j] + (r.to[j] - r.from[j]) * u);
            *slot = [
                r.from[0],
                r.from[1],
                to[0],
                to[1],
                (since - r.at) / rate,
                ran / rate,
                r.strength,
                0.0,
            ]
            .map(|v| v as f32);
        }
        out
    }

    /// The lights in the water at a clip time (under it, or touching it,
    /// within CONTACT, as the box's base at rest), for dots_light.wgsl's
    /// glow, in runs of UNDER_GROUP: each run's mean place in the camera's
    /// space, as the shaders have it at that continuous dot frame (in
    /// f64), and how many of its lights are in the water; then zeros.
    pub fn under_at(&self, time: f64) -> [[f32; 4]; MAX_UNDER] {
        let mut out = [[0.0; 4]; MAX_UNDER];
        let tau = self.dot_position(time);
        let level = self.water.at(time - self.seconds(0));
        if tau < 0.0 || self.lights.glow_gain() <= 0.0 {
            return out;
        }
        // dots_common.wgsl's dot_state: linear between frames, the morph
        // reaching the next stage's object at a stage's last frame.
        let last = self.dot_frames.len() - 1;
        let k = (tau.floor() as usize).min(last);
        let f = if k == last { 0.0 } else { tau - k as f64 } as f32;
        let (a, b) = (self.dot_frames[k], self.dot_frames[(k + 1).min(last)]);
        let lerp = |x: f32, y: f32| x + (y - x) * f;
        let morph = if b.angles[3] == a.angles[3] {
            b.translation[3]
        } else {
            1.0
        };
        let frame = DotFrame {
            angles: [
                lerp(a.angles[0], b.angles[0]),
                lerp(a.angles[1], b.angles[1]),
                lerp(a.angles[2], b.angles[2]),
                a.angles[3],
            ],
            translation: [
                lerp(a.translation[0], b.translation[0]),
                lerp(a.translation[1], b.translation[1]),
                lerp(a.translation[2], b.translation[2]),
                lerp(a.translation[3], morph),
            ],
        };
        // Each run of UNDER_GROUP lights, neighbours in the object, as one,
        // each light weighed by how far it is in the water (wholly at the
        // surface or below, none from CONTACT above), so the glow moves on
        // smoothly as lights go in and out.
        let points = camera_points(&self.objects, &self.tables.order, &frame);
        let groups = points.chunks(UNDER_GROUP).filter_map(|group| {
            let (mut sum, mut weight) = ([0.0; 3], 0.0);
            for &c in group {
                let w = (1.0 - level.height_of(c) / CONTACT).clamp(0.0, 1.0);
                sum = std::array::from_fn(|i| sum[i] + w * c[i]);
                weight += w;
            }
            (weight > 0.0).then(|| [sum[0] / weight, sum[1] / weight, sum[2] / weight, weight])
        });
        for (slot, g) in out.iter_mut().zip(groups) {
            *slot = g.map(|v| v as f32);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::dots::objects_of;
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};
    use crate::scenes::dots::HdDotsTables;

    /// Invented tables: every stage's object one point at its centre, so a
    /// dot frame's light is where its translation puts it.
    fn one_light() -> HdDotsTables {
        HdDotsTables {
            objects: vec![vec![[0, 0, 0]]; 8],
            order: [0, 1, 2, 3, 4, 5, 6, 7],
            script: [(0, 0); 9],
            camera: Vec::new(),
            weights: Default::default(),
            path: Vec::new(),
            shade_palette: Vec::new(),
            dot_palettes: Vec::new(),
            sine: Vec::new(),
        }
    }

    fn frame(y: f32, z: f32) -> DotFrame {
        DotFrame {
            angles: [0.0; 4],
            translation: [0.0, y, z, 0.0],
        }
    }

    /// The water lies `height` below the camera, level or tilted; the ray
    /// through a point of the screen meets it where place_through says,
    /// which is on it.
    #[test]
    fn the_water_lies_below_the_camera() {
        let mut water = Level {
            tilt: 0.0,
            height: LEVEL_HEIGHT,
        };
        assert!((water.height_of([40.0, 60.0, 900.0]) - (LEVEL_HEIGHT - 60.0)).abs() < 1e-9);
        for tilt in [0.0, 0.2, -0.1] {
            water.tilt = tilt;
            let [x, back] = water.place_through(-50.0, 160.0).unwrap();
            // Back along the water from below the camera to that point.
            let (s, c) = tilt.sin_cos();
            let below = [0.0, water.height * c, water.height * s];
            let p = [x * 6.0 / 5.0, below[1] - s * back, below[2] + c * back];
            assert!(water.height_of(p).abs() < 1e-6, "tilt {tilt}");
            let q = water.place_of(p);
            assert!((q[0] - x).abs() < 1e-9 && (q[1] - back).abs() < 1e-6);
            // On the ray through that point of the screen.
            assert!((100.0 + 256.0 * p[1] / p[2] - 160.0).abs() < 1e-6);
            assert!((256.0 * p[0] * 5.0 / 6.0 / p[2] + 50.0).abs() < 1e-6);
        }
    }

    /// By default the water rises through the part, eased, from
    /// START_HEIGHT below the camera to LEVEL_HEIGHT at its end, tilted
    /// LEVEL_TILT, and holds there.
    #[test]
    fn the_water_rises_through_the_part() {
        let water = Water::new(50.0);
        assert_eq!(water.at(0.0).height, START_HEIGHT);
        assert_eq!(water.at(-3.0).height, START_HEIGHT);
        assert_eq!(water.at(50.0).height, LEVEL_HEIGHT);
        assert_eq!(water.at(60.0).height, LEVEL_HEIGHT);
        assert!((water.at(25.0).height - (START_HEIGHT + LEVEL_HEIGHT) / 2.0).abs() < 1e-9);
        let mut last = START_HEIGHT;
        for k in 1..=100 {
            let level = water.at(k as f64 / 2.0);
            assert!(level.height <= last && level.tilt == LEVEL_TILT);
            last = level.height;
        }
    }

    /// A light that comes down into the water and runs along under it sets
    /// off ripples along its path, one stretch after another from where it
    /// went in to where it stopped, fainter the deeper it runs; lying still
    /// it sets off none.
    #[test]
    fn ripples_follow_a_lights_path_through_the_water() {
        let water = Water {
            tilt: 0.0,
            start: 260.0,
            height: 260.0,
            until: 50.0,
        };
        let frames: Vec<DotFrame> = (0..100)
            .map(|k| match k {
                0..10 => frame(200.0 + 10.0 * k as f32, 600.0),
                10..70 => frame(290.0, 600.0 + 20.0 * (k - 10) as f32),
                _ => frame(290.0, 1800.0),
            })
            .collect();
        let light = one_light();
        let ripples = water.ripples(&objects_of(&light), &light.order, &frames, 0.0);
        assert!(ripples.len() >= 2, "{ripples:?}");
        let entered = (water.height - CONTACT - 200.0) / 10.0;
        assert!((ripples[0].at - entered).abs() < 1e-6, "{:?}", ripples[0]);
        let mut reached = ripples[0].from;
        for r in &ripples {
            // None starts once the light lies still.
            assert!(r.until > r.at && r.at < 70.0, "{r:?}");
            assert!((r.from[1] - reached[1]).abs() < 1e-6, "a gap before {r:?}");
            assert!(r.strength > 0.0 && r.strength < 1.0, "{r:?}");
            reached = r.to;
        }
        // On to where it stopped (its last stretch, mostly lying still,
        // stirring too little to count, left out).
        assert!(
            reached[1] >= 1800.0 - 20.0 * RIPPLE_FRAMES as f64 - 1e-6,
            "{reached:?}"
        );
        // Deeper, fainter: the same run 30 units further down.
        let deeper: Vec<DotFrame> = frames
            .iter()
            .map(|f| frame(f.translation[1] + 30.0, f.translation[2]))
            .collect();
        let faint = water.ripples(&objects_of(&light), &light.order, &deeper, 0.0);
        let under = ripples.last().unwrap();
        assert!(under.at >= 10.0);
        assert!(
            faint
                .iter()
                .filter(|r| r.at >= 10.0)
                .all(|r| r.strength < under.strength)
        );
    }

    /// The drops fall from DROP_FIRST after the dots begin, none so late
    /// that it outlasts the rotation's stop by more than its life, each at
    /// a place on the water the screen shows within DROP_ROWS.
    #[test]
    fn drops_fall_in_view_while_the_dots_turn() {
        let water = Water {
            tilt: 0.0,
            start: LEVEL_HEIGHT,
            height: LEVEL_HEIGHT,
            until: 50.0,
        };
        let events = DotsEvents {
            dots: 9.0,
            rotation_stop: 46.0,
            ..Default::default()
        };
        let tuning = WaterTuning::default();
        let mut seen = 0;
        for k in 0..6000 {
            let seconds = k as f64 / 100.0;
            for d in water.drops(&tuning, &events, seconds) {
                if d[3] == 0.0 {
                    continue;
                }
                seen += 1;
                let fell = seconds - d[2] as f64;
                assert!(fell >= events.dots + DROP_FIRST - 1e-6, "{fell}");
                assert!(fell <= events.rotation_stop + 1e-6);
                let row = 100.0 + 256.0 * LEVEL_HEIGHT / d[1] as f64;
                assert!(
                    (DROP_ROWS.0 - 1e-3..=DROP_ROWS.1 + 1e-3).contains(&row),
                    "{row}"
                );
            }
        }
        assert!(seen > 0);
        assert_eq!(
            water.drops(&tuning, &DotsEvents::default(), 20.0),
            [[0.0; 4]; MAX_DROPS]
        );
    }

    /// The constants the shaders repeat, and the shade tips' depth in
    /// dots_light.wgsl, at which the lowest of their path (row 90 + 32767 /
    /// 660 + 32767 / 700 + 8, as dots_common.wgsl's shade_tip has it) stays
    /// above the water where it lies by default.
    #[test]
    fn the_shaders_repeat_the_water_constants() {
        let water = include_str!("water.wgsl");
        assert!(water.contains(&format!("const DROP_LIFE={DROP_LIFE:.1};")));
        assert!(water.contains(&format!("const MAX_DROPS={MAX_DROPS}u;")));
        let light = include_str!("dots_light.wgsl");
        for constant in [
            format!("const MAX_RIPPLES={MAX_RIPPLES}u;"),
            format!("const MAX_UNDER={MAX_UNDER}u;"),
            format!("const PLANKTON_STIRRED={PLANKTON_STIRRED:.1};"),
        ] {
            assert!(
                light.contains(&constant),
                "dots_light.wgsl lacks {constant}"
            );
        }
        let depth: f64 = light
            .split("const SHADE_DEPTH=")
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let lowest = 90.0 + 32767.0 / 660.0 + 32767.0 / 700.0 + 8.0;
        let tip = [0.0, (lowest - 100.0) * depth / 256.0, depth];
        let water = Water::new(50.0);
        assert!(water.at(50.0).height_of(tip) > 0.0);
    }

    /// With the port's data: the ripples running at once, kept as long as
    /// the plankton they stir may glow, fit the shader.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn the_ripples_fit_the_shader() {
        let data = crate::test_support::data();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let dots = clip.dots.as_ref().unwrap();
        let rate = FRAME_RATE as f64;
        let keep =
            (PLANKTON_STIRRED + PLANKTON_LIVES * dots.lights.plankton_life()).max(DROP_LIFE) * rate;
        let most = (0..dots.dot_frames.len() * 2)
            .map(|k| {
                let tau = k as f64 / 2.0;
                let r = &dots.ripples;
                r.iter()
                    .filter(|r| r.at <= tau && tau - r.until < keep)
                    .count()
            })
            .max()
            .unwrap();
        assert!(most <= MAX_RIPPLES, "{most} ripples at once");
    }
}
