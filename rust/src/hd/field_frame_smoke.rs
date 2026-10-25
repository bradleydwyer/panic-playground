//! Part 5's smoke: the frame author's fluid, a real smoke simulation round
//! the frame, lifted from a smoke prototype kept locally and adapted to the
//! real scene. The kernels are in field_frame_smoke/: solver3d.wgsl (3D,
//! with multigrid.wgsl for the pressure and light3d.wgsl for the light) and
//! solver2d.wgsl with light2d.wgsl (the 2D fallback), and terrain.wgsl,
//! which both solvers share; [`SMOKE`] picks one.
//!
//! What comes from where:
//! - The scene's own state, through the part-5 block at each step's start
//!   ([`FieldPreview::controls`]): when the mountain rises and sinks, its
//!   level, camera and orbit, when the window is cleared and when the last
//!   terrain pixel shows, as the preview observes them; and the other
//!   authors' lights, `fc.zoom_style[0]` and `fc.mountain_style[0..2]`, or
//!   the frame's stand-ins for them (field_frame.wgsl's frame_lights).
//! - Derived from the user's PSTART picture when the renderer loads: the
//!   obstacles, the frame and its window as a solid plate up to the bevels'
//!   tops, and the ornaments smoothed into solid blobs, as the cells are
//!   coarser than the filigree (field_frame.rs's [`FrameShape`]).
//! - The mountain's outline over each column, marched through the fitted
//!   relief with the code's camera and projection, where the lava's heat
//!   rises: interpretation, as the relief is.
//! - Authored: everything else, the emitters, the heat, the air's movement,
//!   the lights' placing and strength and the solver's parameters. Nothing
//!   is from the original demo.
//!
//! The simulation is a pure function of the step: each step's parameters
//! come from the block at its start, computed in f64 on the CPU, and the
//! kernels only gather. State n is the smoke at the fluid's start (part 6's
//! PSTART fade, after a pre-roll of [`PREROLL`] steps that develops it
//! unseen) plus n steps at [`FLUID`]'s rate. The pre-roll runs ahead, in
//! the frames before the start, on an authored schedule over the half
//! minute of part 6 before it ([`PREROLL_RATE`] steps a second, done
//! [`PREROLL_LEAD`] seconds early): a step every three or four frames at 60
//! a second, so that the start's frame runs one step rather than all of
//! them. A frame runs at most [`PREROLL_BATCH`], catching up after a seek
//! into part 6; a seek past the start runs what is left there, and a seek
//! back before it keeps a finished pre-roll. Each step takes the block at
//! its own time, not the frame's, so state 0 is the same whenever its steps
//! run. A state is kept every `FLUID.checkpoint` steps as it is first
//! reached (state 0 at the pre-roll's last step), so seeking back runs at
//! most that many steps; playing on runs a step a frame. Bit-exactness holds
//! on one machine, GPU and driver (the prototype measured it on Metal); the
//! simulation is chaotic, so any change to a kernel or a parameter changes
//! every later frame.
use super::{FIELD_WINDOW, FLUID, FieldBlock, FieldPreview, FluidStart, FrameShape};
use bytemuck::{Pod, Zeroable};
use std::f64::consts::PI;
use std::num::NonZeroU64;
use wgpu::*;

/// Which smoke runs ([`SMOKE`] picks one; the others stay built and tested).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SmokeKind {
    /// 3D, 208 x 124 x 64 velocity cells (192 x 108 on screen), density and
    /// temperature at twice that each way, stored as f16: the prototype's
    /// default.
    Volume,
    /// 3D on the same velocity grid, with density and temperature at its
    /// own resolution: softer and about half the cost.
    SoftVolume,
    /// The 2D fallback: 512 x 288 velocity cells over the screen, density
    /// and temperature at twice that, f32.
    Flat,
}

/// The smoke the renderer runs.
pub const SMOKE: SmokeKind = SmokeKind::Volume;

/// Steps of pre-roll before the fluid's start: 8 s, so that the smoke has
/// developed, unseen, when part 6's fade brings the frame in.
pub const PREROLL: usize = 480;

/// The pre-roll's pace ahead of the fluid's start, in the frames before it
/// ([`FrameFluid::preroll`]): steps a second of clip time, so that it runs
/// over the 30 s of part 6 before the start's last [`PREROLL_LEAD`]
/// seconds, a step every three or four frames at 60 a second. Authored, to
/// keep those frames well within 1/60 s: observed with `--bench` at
/// 2560 x 1440 on an Apple M5 Max, part 6's frames take about 3 ms and
/// those with a step about 8.
const PREROLL_RATE: f64 = 16.0;
/// Seconds before the fluid's start by which the pre-roll is due in full.
const PREROLL_LEAD: f64 = 2.0;
/// The most pre-roll steps one frame runs ahead of the start, catching up
/// after a seek (such frames take about 10 ms, as above); whatever is left
/// when the start comes runs then.
const PREROLL_BATCH: usize = 2;

/// The 16:9 screen's width in its heights.
const SCREEN_W: f64 = 16.0 / 9.0;
/// The front of the 3D volume (screen z).
const Z_FRONT: f64 = 0.32;
/// Bytes between the steps' parameters in their buffer (a multiple of the
/// 256-byte dynamic offset alignment), and the steps one submission holds.
const STEP_STRIDE: u64 = 1024;
const STEP_SLOTS: usize = 256;

/// A smoke's grids: velocity cells (with the margins), scalar cells per
/// velocity cell each way, light-grid cells per scalar cell, velocity cells
/// a screen height, and the domain's corner (screen units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grids {
    pub dim: u32,
    pub n: [u32; 3],
    pub ds: u32,
    pub ls: u32,
    pub rows: u32,
    pub origin: [f64; 3],
}

impl Grids {
    pub const fn of(kind: SmokeKind) -> Self {
        match kind {
            SmokeKind::Volume | SmokeKind::SoftVolume => {
                let (rows, margin, nz) = (108, 8, 64);
                let m = margin as f64 / rows as f64;
                Self {
                    dim: 3,
                    n: [rows * 16 / 9 + 2 * margin, rows + 2 * margin, nz],
                    ds: if matches!(kind, SmokeKind::Volume) {
                        2
                    } else {
                        1
                    },
                    ls: 2,
                    rows,
                    origin: [-m, -m, Z_FRONT - nz as f64 / rows as f64],
                }
            }
            SmokeKind::Flat => Self {
                dim: 2,
                n: [512, 288, 1],
                ds: 2,
                ls: 1,
                rows: 288,
                origin: [0.0, 0.0, 0.0],
            },
        }
    }
    /// Scalar cells.
    pub const fn s(&self) -> [u32; 3] {
        let z = if self.dim == 3 {
            self.n[2] * self.ds
        } else {
            1
        };
        [self.n[0] * self.ds, self.n[1] * self.ds, z]
    }
    /// Light-grid cells (3D).
    pub const fn l(&self) -> [u32; 3] {
        let s = self.s();
        [s[0] / self.ls, s[1] / self.ls, s[2] / self.ls]
    }
    /// A velocity cell, in screen units.
    pub const fn h(&self) -> f64 {
        1.0 / self.rows as f64
    }
    /// The source-pixel rectangle the scalar columns cover, x0, y0 (top),
    /// x1, y1: the light layers' extent, for `fluid_uv`.
    pub const fn region(&self) -> [f32; 4] {
        let x0 = self.origin[0];
        let y0 = self.origin[1];
        let x1 = x0 + self.n[0] as f64 * self.h();
        let y1 = y0 + self.n[1] as f64 * self.h();
        [
            source_x(x0) as f32,
            source_y(y1) as f32,
            source_x(x1) as f32,
            source_y(y0) as f32,
        ]
    }
}

/// Screen x and y (screen units, y up) to the original's source pixels.
const fn source_x(x: f64) -> f64 {
    160.0 + (x - SCREEN_W / 2.0) * 240.0
}
const fn source_y(y: f64) -> f64 {
    200.0 - 200.0 * y
}
/// Source pixels to screen units.
fn screen_of_source(s: [f64; 2]) -> [f64; 2] {
    [SCREEN_W / 2.0 + (s[0] - 160.0) / 240.0, 1.0 - s[1] / 200.0]
}
/// Scene (field_common.wgsl's: source rows, y up, centred) to screen units.
fn screen_of_scene(q: [f64; 3]) -> [f64; 3] {
    [
        SCREEN_W / 2.0 + q[0] / 200.0,
        0.5 + q[1] / 200.0,
        q[2] / 200.0,
    ]
}

/// The light layers' size and region, for FLUID.
pub const LAYERS: [u32; 2] = {
    let s = Grids::of(SMOKE).s();
    [s[0], s[1]]
};
pub const REGION: [f32; 4] = Grids::of(SMOKE).region();

// ---- Uniforms, as the kernels declare them -------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Grid3 {
    nx: u32,
    ny: u32,
    nz: u32,
    ds: u32,
    sx: u32,
    sy: u32,
    sz: u32,
    ls: u32,
    lx: u32,
    ly: u32,
    lz: u32,
    solid_off: u32,
    ou: u32,
    ov: u32,
    ow: u32,
    flags: u32,
    z0: f32,
    h: f32,
    inv_ds: f32,
    sponge: f32,
    x0: f32,
    y0: f32,
    _s: f32,
    _t: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Grid2 {
    nx: u32,
    ny: u32,
    sx: u32,
    sy: u32,
    ds: f32,
    inv_ds: f32,
    omega: f32,
    _pad: f32,
}

/// Emitters a step holds at most.
const MAX_EMIT: usize = 16;

/// One step's parameters (field_frame_smoke/solver3d.wgsl's and
/// field_frame_smoke/solver2d.wgsl's Step; the 2D one reads the first 32 of
/// `emit`, two a emitter).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Step {
    dt: f32,
    buoy_t: f32,
    buoy_rho: f32,
    vort: f32,
    keep_rho: f32,
    keep_t: f32,
    keep_vel: f32,
    n_emit: u32,
    wind: [f32; 4],
    heat: [f32; 4],
    heat_z: [f32; 4],
    camera: [f32; 4],
    view: [f32; 4],
    emit: [[f32; 4]; 3 * MAX_EMIT],
}

/// The lighting's parameters at a moment (field_frame_smoke/light3d.wgsl's
/// Frame; the 2D one is the same to `lights`, then `key`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Lighting {
    t: f32,
    vis: f32,
    sigma: f32,
    half: f32,
    albedo: [f32; 4],
    ambient: [f32; 4],
    n_lights: u32,
    k_depth: u32,
    march: u32,
    mode: u32,
    lights: [[f32; 4]; 16],
    vol: [f32; 4],
    phase: [f32; 4],
}

// ---- The story, from the scene's state (authored) -------------------------------

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
fn hash01(seed: u64, i: i64) -> f64 {
    (splitmix(splitmix(seed) ^ i as u64) >> 11) as f64 / (1u64 << 53) as f64
}
/// Smooth value noise in time, a pure function of (seed, x).
fn vnoise(seed: u64, x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let a = hash01(seed, i as i64);
    let b = hash01(seed, i as i64 + 1);
    a + (b - a) * f * f * (3.0 - 2.0 * f)
}
fn fbm(seed: u64, x: f64) -> f64 {
    0.6 * vnoise(seed, x) + 0.4 * vnoise(seed + 977, 2.3 * x + 0.37)
}
fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The frame's stand-ins for the other authors' lights (field_frame.wgsl's
/// FRAME_WINDOW_STANDIN, FRAME_LAVA_STANDIN and FRAME_LAVA_SPREAD), kept in
/// step with it.
const WINDOW_STANDIN: [f64; 3] = [0.07, 0.16, 0.72];
const LAVA_STANDIN: [f64; 3] = [6.0, 1.55, 0.28];
const LAVA_SPREAD: f64 = 46.0;
/// Where the frame's outer edge roughly is (source x), for placing the
/// heat beside it: the window's left edge less the bevels' width the
/// imported picture has (field_frame.rs measures the real one).
const FRAME_OUTER_LEFT: f64 = FIELD_WINDOW[0] as f64 - 29.0;
/// field_common.wgsl's COLUMN_CENTRE and LAYER_MOUNTAIN_Z.
const COLUMN_CENTRE: f64 = 161.0;
const LAYER_MOUNTAIN_Z: f64 = 100.0;

/// The scene at a moment, as the part-5 block has it, and what the smoke
/// makes of it. Times are part-5 seconds, the block's own; an event the
/// preview never observed reads -1 and doesn't happen.
struct Moment<'a> {
    /// Clip seconds.
    t: f64,
    b: &'a FieldBlock,
}

impl Moment<'_> {
    fn seconds(&self) -> f64 {
        self.b.part[1] as f64
    }
    fn event(&self, e: f32) -> Option<f64> {
        (e >= 0.0).then_some(e as f64)
    }
    fn terrain_starts(&self) -> Option<f64> {
        self.event(self.b.events[2])
    }
    fn sinks(&self) -> Option<f64> {
        self.event(self.b.events_late[1])
    }
    fn last_terrain(&self) -> Option<f64> {
        self.event(self.b.events_late[2])
    }
    fn since(&self, e: Option<f64>, a: f64, b: f64) -> f64 {
        e.map_or(0.0, |e| smoothstep(e + a, e + b, self.seconds()))
    }
    /// Part 6's fade, in DAC units as its picture was.
    fn picture(&self) -> f64 {
        (self.b.window[2] as f64).clamp(0.0, 1.0).powf(2.2)
    }
    /// The mountain stands in front while the original shows its pixels.
    fn mountain(&self) -> f64 {
        if self.b.part[2] > 0.5 {
            (self.b.window[3] as f64).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    fn level(&self) -> f64 {
        self.b.mountain[0] as f64 / 64.0
    }
    /// How much nearer the orbiting camera is than its mean (the code's
    /// orbit radius, 70 + sine / 12).
    fn closeness(&self) -> f64 {
        let r = self.b.view[2] as f64;
        if self.b.part[2] > 0.5 && r > 1.0 {
            70.0 / r
        } else {
            1.0
        }
    }
    /// The window's light (radiance): the zoom author's, or the stand-in,
    /// going out with the window's clear.
    fn window(&self) -> [f64; 3] {
        let given = self.b.zoom_style[0];
        if given.iter().any(|&v| v != 0.0) {
            return [0, 1, 2].map(|i| given[i].max(0.0) as f64);
        }
        let on = if self.b.window[0] > 0.5 {
            1.0 - smoothstep(0.0, 0.12, self.b.window[1] as f64)
        } else {
            1.0
        };
        WINDOW_STANDIN.map(|c| c * on)
    }
    /// The lava's light: centre (scene), radius (rows) and radiance; the
    /// mountain author's, or the stand-in over the plateau.
    fn lava(&self) -> ([f64; 3], f64, [f64; 3]) {
        let at = self.b.mountain_style[0];
        let colour = self.b.mountain_style[1];
        if colour.iter().any(|&v| v != 0.0) {
            let s = [at[0] as f64, at[1] as f64];
            let scene = [(s[0] - 160.0) * 5.0 / 6.0, 100.0 - s[1], at[2] as f64];
            let c = [0, 1, 2].map(|i| colour[i].max(0.0) as f64);
            return (scene, (at[3] as f64).max(1.0), c);
        }
        // field_common.wgsl's terrain_row at the plateau's height and the
        // camera's distance from the map's centre.
        let q = self.b.mountain[1] as f64;
        let z = (self.b.view[3] as f64).max(1.0);
        let projection = q * 256.0 / z + 512.0 - 2.0 * q;
        let row = (self.b.camera[2] as f64 - self.b.relief[2] as f64) * projection / 1024.0;
        let s = [COLUMN_CENTRE, row - 0.3 * LAVA_SPREAD];
        let scene = [(s[0] - 160.0) * 5.0 / 6.0, 100.0 - s[1], LAYER_MOUNTAIN_Z];
        let on = self.mountain() * smoothstep(8.0 / 64.0, 1.0, self.level());
        (scene, LAVA_SPREAD, LAVA_STANDIN.map(|c| c * on))
    }
    /// The lava's strength, 1 for the stand-in's full glow.
    fn lava_strength(&self) -> f64 {
        luminance(self.lava().2) / luminance(LAVA_STANDIN)
    }
    /// The lava's heat: with its light, a little more as the camera comes
    /// in, catching over two seconds as the mountain rises.
    fn heat(&self) -> f64 {
        self.lava_strength()
            * (0.75 + 0.25 * self.closeness())
            * self.since(self.terrain_starts(), 0.0, 2.0)
    }
    /// The cool smoke's supply: less once the lava's heat makes its own,
    /// and waning as the mountain sinks.
    fn supply(&self) -> f64 {
        (1.0 - 0.7 * self.since(self.terrain_starts(), 0.0, 2.0))
            * (1.0 - self.since(self.sinks(), -0.8, 4.7))
    }
    /// After the last terrain pixel the smoke settles and fades.
    fn settle(&self) -> f64 {
        self.since(self.last_terrain(), 0.0, 1.2)
    }
    /// Vorticity confinement: calmer in the zoom and as the mountain goes.
    fn lively(&self) -> f64 {
        let up = self.since(self.terrain_starts(), 0.0, 1.9);
        let down = match (self.sinks(), self.last_terrain()) {
            (Some(a), Some(b)) => smoothstep(a + 2.0, b.max(a + 2.1), self.seconds()),
            _ => 0.0,
        };
        up * (1.0 - down)
    }
}

/// One emitter, screen units: centre and radius, density and temperature
/// per second, the pull toward its velocity per second, and that velocity
/// (screen heights a second).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Emitter {
    p: [f64; 3],
    r: f64,
    rho: f64,
    temp: f64,
    pull: f64,
    v: [f64; 3],
}

/// The cool smoke, from the prototype: four sources below the picture and
/// two drafts from the screen's sides, behind the picture plane, in
/// intermittent puffs; all pure functions of the step's time.
fn emitters(m: &Moment) -> Vec<Emitter> {
    let t = m.t;
    let a = m.supply();
    let mut e = Vec::new();
    for (k, &(x0, z0)) in [(0.14, -0.10), (0.58, -0.16), (1.20, -0.12), (1.64, -0.08)]
        .iter()
        .enumerate()
    {
        let kf = k as f64;
        let puff = smoothstep(0.30, 0.75, fbm(10 + k as u64, t * 0.30 + 3.1 * kf));
        let wob = 0.05 * (2.0 * PI * (0.04 * t + 0.37 * kf)).sin();
        e.push(Emitter {
            p: [
                x0 + wob,
                0.0,
                z0 + 0.04 * (2.0 * PI * (0.03 * t + kf)).sin(),
            ],
            r: 0.05,
            rho: 0.9 * a * puff,
            temp: 0.3 * a * puff,
            pull: 1.5,
            v: [0.03 * (2.0 * PI * (0.031 * t + 0.6 * kf)).sin(), 0.06, 0.0],
        });
    }
    for side in 0..2u64 {
        let s = if side == 0 { 1.0 } else { -1.0 };
        let puff = smoothstep(0.35, 0.8, fbm(30 + side, t * 0.25 + 1.7 * side as f64));
        e.push(Emitter {
            p: [
                if side == 0 { -0.03 } else { SCREEN_W + 0.03 },
                0.40 + 0.12 * (2.0 * PI * (0.023 * t + 0.5 * side as f64)).sin(),
                -0.06,
            ],
            r: 0.07,
            rho: 0.6 * a * puff,
            temp: 0.10 * a * puff,
            pull: 1.0,
            v: [0.07 * s, 0.01, 0.0],
        });
    }
    e
}

/// The lava's heat along the mountain's outline (authored): temperature
/// and density a second, at full heat, in a band this deep below it (screen
/// units), centred this far behind the picture plane (screen z) with this
/// radius (nearer the plane where the outline is beside the frame rather
/// than behind it: field_frame_smoke/solver3d.wgsl's HEAT_BESIDE), flickering over
/// this width.
const HEAT_TEMPERATURE: f64 = 12.0;
const HEAT_DENSITY: f64 = 0.8;
const HEAT_BAND: f64 = 0.035;
const HEAT_Z: [f64; 2] = [-0.06, 0.07];
const HEAT_FLICKER: f64 = 0.06;

/// The parameters of the step from a moment.
fn step_params(m: &Moment, grids: &Grids, dt: f64) -> Step {
    let rows = grids.rows as f64;
    let flat = grids.dim == 2;
    let mut s = Step::zeroed();
    s.dt = dt as f32;
    s.buoy_t = (1.8 * rows) as f32;
    s.buoy_rho = (0.04 * rows) as f32;
    let calm = 0.4;
    let (vort, per) = if flat { (0.9, 288.0) } else { (1.2, 144.0) };
    s.vort = (vort * (calm + (1.0 - calm) * m.lively()) * rows / per) as f32;
    let fade = if flat { 0.08 } else { 0.15 };
    s.keep_rho = (-(fade + 0.25 * m.settle()) * dt).exp() as f32;
    s.keep_t = (-0.5 * dt).exp() as f32;
    s.keep_vel = (-(0.05 + 0.8 * m.settle()) * dt).exp() as f32;
    let heat = m.heat();
    // A slow sway of the room's air while the smoke only drifts, and a
    // draft toward the back in front of the frame while the lava burns,
    // which keeps the window and the mountain clear.
    let t = m.t;
    let sway = 0.05
        * (1.0 - m.since(m.terrain_starts(), 0.0, 1.8))
        * ((2.0 * PI * t / 11.0).sin() + 0.6 * (2.0 * PI * t / 7.3 + 1.1).sin());
    if !flat {
        s.wind = [
            (sway * rows) as f32,
            0.0,
            -(0.12 * heat.min(1.0) * rows) as f32,
            0.10,
        ];
    }
    s.heat = [
        (HEAT_TEMPERATURE * heat) as f32,
        (HEAT_DENSITY * heat) as f32,
        HEAT_BAND as f32,
        t as f32,
    ];
    // The frame's half width, screen units, about the screen's middle.
    let beside = (source_x(SCREEN_W / 2.0) - FRAME_OUTER_LEFT) / 240.0;
    s.heat_z = [
        HEAT_Z[0] as f32,
        HEAT_Z[1] as f32,
        HEAT_FLICKER as f32,
        beside as f32,
    ];
    let b = m.b;
    s.camera = [b.camera[0], b.camera[1], b.camera[2], b.mountain[1]];
    s.view = [
        b.view[0] / 256.0,
        b.view[1] / 256.0,
        b.view[1] / 256.0,
        -b.view[0] / 256.0,
    ];
    let em = emitters(m);
    s.n_emit = em.len() as u32;
    for (i, e) in em.iter().enumerate() {
        if flat {
            s.emit[2 * i] = [e.p[0], e.p[1], e.r, e.rho].map(|v| v as f32);
            s.emit[2 * i + 1] = [e.temp, e.v[0] * rows, e.v[1] * rows, e.pull].map(|v| v as f32);
        } else {
            s.emit[3 * i] = [e.p[0], e.p[1], e.p[2], e.r].map(|v| v as f32);
            s.emit[3 * i + 1] = [e.rho, e.temp, e.pull, 0.0].map(|v| v as f32);
            s.emit[3 * i + 2] =
                [e.v[0] * rows, e.v[1] * rows, e.v[2] * rows, 0.0].map(|v| v as f32);
        }
    }
    s
}

/// The smoke's lights at a moment (authored): the window's light in front
/// of the window, so that the frame shadows the smoke behind it; the lava's,
/// with a strip of it along the mountain's base and a broad fill from below;
/// and a dim ambient. Strengths are scaled from the lights' radiance.
fn lighting(m: &Moment, grids: &Grids) -> Lighting {
    let mut f = Lighting::zeroed();
    let vis = m.picture();
    f.t = m.t as f32;
    f.vis = vis as f32;
    f.march = 32;
    f.k_depth = 2;
    f.phase = [0.5, 0.5, 0.5, 0.0];
    let flat = grids.dim == 2;
    f.sigma = if flat { 7.0 } else { 55.0 };
    f.half = 0.06;
    let albedo = if flat { 0.8 } else { 0.55 };
    f.albedo = [albedo, albedo * 0.98, albedo * 0.96, 0.0];
    let fading = m.since(m.last_terrain(), 0.0, 1.8);
    let ambient = 0.014 * vis * (1.0 - 0.4 * fading);
    f.ambient = [ambient * 0.7, ambient * 0.85, ambient, 0.0].map(|v| v as f32);
    f.vol = [grids.origin[2] as f32, Z_FRONT as f32, 0.0, 0.0];
    let mut n = 0;
    let mut push = |pos: [f64; 4], rect: [f64; 4], col: [f64; 4], extra: [f64; 4]| {
        if n < 4 {
            for (k, v) in [pos, rect, col, extra].into_iter().enumerate() {
                f.lights[4 * n + k] = v.map(|x| x as f32);
            }
            n += 1;
        }
    };
    // The window's light, out of the smoke's sight as the mountain rises in
    // front of it.
    let risen = if m.b.part[2] > 0.5 {
        smoothstep(0.1, 1.0, m.level())
    } else {
        0.0
    };
    let window = m.window().map(|v| v * (1.0 - risen));
    if luminance(window) > 1e-5 {
        // A little paler than the window's light: the smoke scatters it
        // with the room's.
        let grey = luminance(window);
        let c = window.map(|v| 4.0 * (v + 0.35 * (grey - v)));
        let lo = screen_of_source([FIELD_WINDOW[0] as f64, FIELD_WINDOW[3] as f64]);
        let hi = screen_of_source([FIELD_WINDOW[2] as f64, FIELD_WINDOW[1] as f64]);
        let z = if flat { 0.02 } else { 0.25 };
        push(
            [0.0, 0.0, z, 1.0],
            [lo[0], lo[1], hi[0], hi[1]],
            [c[0], c[1], c[2], 0.5],
            [0.15, 0.3, 1.0, 0.0],
        );
    }
    let (at, size, colour) = m.lava();
    if luminance(colour) > 1e-5 {
        // The lava's glow where it is, and a broad, dimmer one from the
        // mountain's body in front of the frame; the smoke is to stay
        // secondary to the mountain and the frame.
        let p = screen_of_scene(at);
        let k = 0.45;
        let z = if flat { 0.02 } else { p[2].min(Z_FRONT - 0.02) };
        push(
            [p[0], p[1], z, 0.0],
            [0.0; 4],
            [
                colour[0] * k,
                colour[1] * k,
                colour[2] * k,
                (size / 200.0).max(0.25),
            ],
            [0.08, 0.0, 1.0, 0.0],
        );
        let c = SCREEN_W / 2.0;
        let k = 0.15 * m.closeness();
        push(
            [c, 0.3, if flat { 0.02 } else { 0.15 }, 0.0],
            [0.0; 4],
            [colour[0] * k, colour[1] * k, colour[2] * k, 0.7],
            [0.3, 0.2, 1.0, 0.0],
        );
    }
    f.n_lights = n as u32;
    if flat {
        // The 2D lighting reads `key.w`, the near field's reach, where the
        // 3D one has `vol`.
        f.vol = [0.0, 0.0, 0.0, 0.03];
    }
    f
}

// ---- Obstacles, from the frame's shape ----------------------------------------

/// field_frame.wgsl's bevel profile, rows of height per row across (the
/// foot, the inner bevel, the outer face falling and the lip), kept in step
/// with FRAME_*_SLOPE there; the obstacle only needs it roughly.
const FOOT_SLOPE: f64 = 3.0;
const INNER_SLOPE: f64 = 1.43;
const OUTER_SLOPE: f64 = 0.45;
const LIP_SLOPE: f64 = 2.6;
/// The ornaments' top (screen z): field_frame.rs's RELIEF_HEIGHT, 8 rows.
const ORNAMENT_TOP: f64 = 8.0 / 200.0;

/// The obstacles over a grid's scalar columns, rows from the bottom, as
/// Rgba16Float: the solid's top (screen z), 1 where the column is solid,
/// and the softened coverage the lighting shadows with.
fn obstacles(shape: &FrameShape, grids: &Grids) -> Vec<[half::f16; 4]> {
    let [sx, sy, _] = grids.s();
    let (w, h) = (sx as usize, sy as usize);
    let hs = grids.h() / grids.ds as f64;
    let frame = shape.frame();
    let mut top = vec![-1.0f32; w * h];
    let mut cover = vec![0.0f32; w * h];
    let mut ornament = vec![0.0f32; w * h];
    for j in 0..h {
        for i in 0..w {
            let x = grids.origin[0] + (i as f64 + 0.5) * hs;
            let y = grids.origin[1] + (j as f64 + 0.5) * hs;
            let s = [source_x(x), source_y(y)];
            let k = j * w + i;
            if let Some((outer, crease)) = frame
                && s[0] >= outer[0] as f64
                && s[0] < outer[2] as f64
                && s[1] >= outer[1] as f64
                && s[1] < outer[3] as f64
            {
                top[k] = (bevel_height(s, outer, crease) / 200.0) as f32;
                cover[k] = 1.0;
                continue;
            }
            // The ornaments, antialiased over a source pixel.
            let d = shape.ornament_distance(s);
            ornament[k] = (d + 0.5).clamp(0.0, 1.0);
        }
    }
    // The filigree is finer than a velocity cell: smooth it over one, and
    // keep what covers more than a third, so thin strands become solid
    // ribbons and specks vanish.
    let r = grids.ds as usize;
    let smooth = box_blur(&ornament, (w, h), r);
    for k in 0..w * h {
        if top[k] < 0.0 && smooth[k] > 0.33 {
            top[k] = ORNAMENT_TOP as f32;
        }
        cover[k] = cover[k].max(smooth[k]);
    }
    let shadow = box_blur(&box_blur(&cover, (w, h), r), (w, h), r);
    (0..w * h)
        .map(|k| {
            let solid = if top[k] >= 0.0 { 1.0 } else { 0.0 };
            [top[k].max(0.0), solid, shadow[k], 0.0].map(half::f16::from_f32)
        })
        .collect()
}

/// The bevels' height (rows) at source point s on the frame, by the side
/// whose mitre it is on, roughly as field_frame.wgsl draws it.
fn bevel_height(s: [f64; 2], outer: [f32; 4], crease: [f32; 4]) -> f64 {
    let w = FIELD_WINDOW.map(|v| v as f64);
    let o = outer.map(|v| v as f64);
    let c = crease.map(|v| v as f64);
    let away = [w[0] - s[0], s[0] - w[2], w[1] - s[1], s[1] - w[3]];
    let edge = [w[0] - o[0], o[2] - w[2], w[1] - o[1], o[3] - w[3]];
    let to_crease = [w[0] - c[0], c[2] - w[2], w[1] - c[1], c[3] - w[3]];
    let side = (0..4)
        .max_by(|&a, &b| (away[a] / edge[a].max(1.0)).total_cmp(&(away[b] / edge[b].max(1.0))))
        .unwrap();
    if away[side] <= 0.0 {
        return 0.0;
    }
    // Rows a source pixel across this side.
    let axis = if side < 2 { 5.0 / 6.0 } else { 1.0 };
    let x = away[side] * axis;
    let (foot, crest, lip) = (axis, to_crease[side] * axis, (edge[side] - 1.0) * axis);
    FOOT_SLOPE * x.min(foot) + INNER_SLOPE * (x - foot).clamp(0.0, (crest - foot).max(0.0))
        - OUTER_SLOPE * (x - crest).clamp(0.0, (lip - crest).max(0.0))
        - LIP_SLOPE * (x - lip).clamp(0.0, axis)
}

/// A box blur of radius r each way, clamped at the edges.
fn box_blur(field: &[f32], (w, h): (usize, usize), r: usize) -> Vec<f32> {
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; w * h];
        let r = r as isize;
        for y in 0..h as isize {
            for x in 0..w as isize {
                let mut sum = 0.0;
                for d in -r..=r {
                    let (a, b) = if horizontal { (x + d, y) } else { (x, y + d) };
                    let a = a.clamp(0, w as isize - 1) as usize;
                    let b = b.clamp(0, h as isize - 1) as usize;
                    sum += src[b * w + a];
                }
                out[y as usize * w + x as usize] = sum / (2 * r + 1) as f32;
            }
        }
        out
    };
    pass(&pass(field, true), false)
}

// ---- GPU ----------------------------------------------------------------------------

fn storage_entry(binding: u32, read_only: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn uniform_entry(binding: u32, dynamic: Option<u64>) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: dynamic.is_some(),
            min_binding_size: dynamic.and_then(NonZeroU64::new),
        },
        count: None,
    }
}
fn texture_entry(binding: u32, view_dimension: TextureViewDimension) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension,
            multisampled: false,
        },
        count: None,
    }
}
fn sampler_entry(binding: u32) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Sampler(SamplerBindingType::Filtering),
        count: None,
    }
}
fn entry(binding: u32, resource: BindingResource<'_>) -> BindGroupEntry<'_> {
    BindGroupEntry { binding, resource }
}
fn storage_buffer(device: &Device, label: &str, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: size.max(16),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn uniform_buffer(device: &Device, label: &str, size: u64) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn texture(
    device: &Device,
    label: &str,
    size: [u32; 3],
    dimension: TextureDimension,
    format: TextureFormat,
    usage: TextureUsages,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: size[2],
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension,
        format,
        usage,
        view_formats: &[],
    })
}
fn volume(device: &Device, label: &str, size: [u32; 3]) -> Texture {
    texture(
        device,
        label,
        size,
        TextureDimension::D3,
        TextureFormat::Rgba16Float,
        TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
    )
}
fn compute(
    device: &Device,
    layout: Option<&PipelineLayout>,
    module: &ShaderModule,
    entry_point: &str,
) -> ComputePipeline {
    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some(entry_point),
        layout,
        module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    })
}
fn pipeline_layout(device: &Device, layout: &BindGroupLayout) -> PipelineLayout {
    device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Part 5 smoke"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    })
}
fn workgroups(x: u32, n: u32) -> u32 {
    x.div_ceil(n)
}

/// f16 or f32 scalar storage for the 3D kernels: the prototype's prefix.
const PACKED_F16: &str = "alias Packed = u32;\nfn unpk(x: u32) -> vec2f { return unpack2x16float(x); }\nfn pk(v: vec2f) -> u32 { return pack2x16float(v); }\n";
const TERRAIN: &str = include_str!("field_frame_smoke/terrain.wgsl");
const SOLVER_3D: &str = include_str!("field_frame_smoke/solver3d.wgsl");
const MULTIGRID: &str = include_str!("field_frame_smoke/multigrid.wgsl");
const LIGHT_3D: &str = include_str!("field_frame_smoke/light3d.wgsl");
const SOLVER_2D: &str = include_str!("field_frame_smoke/solver2d.wgsl");
const LIGHT_2D: &str = include_str!("field_frame_smoke/light2d.wgsl");

/// The smoke's shader modules' sources: the 3D solver, the multigrid, the
/// 3D light; the 2D solver and light.
pub fn sources() -> [(&'static str, String); 5] {
    [
        ("3D smoke", format!("{PACKED_F16}{TERRAIN}\n{SOLVER_3D}")),
        ("3D pressure", MULTIGRID.to_owned()),
        ("3D smoke light", format!("{PACKED_F16}{LIGHT_3D}")),
        ("2D smoke", format!("{TERRAIN}\n{SOLVER_2D}")),
        ("2D smoke light", LIGHT_2D.to_owned()),
    ]
}

fn module(device: &Device, label: &str, source: String) -> ShaderModule {
    device.create_shader_module(ShaderModuleDescriptor {
        label: Some(label),
        source: ShaderSource::Wgsl(source.into()),
    })
}

struct Level {
    group: BindGroup,
    n: [u32; 3],
    c: [u32; 3],
}

struct Multigrid {
    levels: Vec<Level>,
    red: ComputePipeline,
    black: ComputePipeline,
    restrict: ComputePipeline,
    prolong: ComputePipeline,
    _keep: Vec<Buffer>,
}

impl Multigrid {
    /// One V-cycle: 2 + 2 red-black sweeps a level and 10 at the coarsest.
    fn encode(&self, pass: &mut ComputePass) {
        let last = self.levels.len() - 1;
        let sweep = |pass: &mut ComputePass, l: &Level, n: u32| {
            for _ in 0..n {
                for p in [&self.red, &self.black] {
                    pass.set_pipeline(p);
                    pass.dispatch_workgroups(
                        workgroups(l.n[0].div_ceil(2), 8),
                        workgroups(l.n[1], 8),
                        workgroups(l.n[2], 4),
                    );
                }
            }
        };
        for l in &self.levels[..last] {
            pass.set_bind_group(0, &l.group, &[]);
            sweep(pass, l, 2);
            pass.set_pipeline(&self.restrict);
            pass.dispatch_workgroups(
                workgroups(l.c[0], 8),
                workgroups(l.c[1], 8),
                workgroups(l.c[2], 4),
            );
        }
        let l = &self.levels[last];
        pass.set_bind_group(0, &l.group, &[]);
        sweep(pass, l, 10);
        for l in self.levels[..last].iter().rev() {
            pass.set_bind_group(0, &l.group, &[]);
            pass.set_pipeline(&self.prolong);
            pass.dispatch_workgroups(
                workgroups(l.n[0], 8),
                workgroups(l.n[1], 8),
                workgroups(l.n[2], 4),
            );
            sweep(pass, l, 2);
        }
    }
}

/// A solver's passes over its grids: the pipeline, the bind group (with the
/// step's dynamic offset, or none) and the cells it runs over, per workgroup
/// size.
enum Pass {
    Step(usize, [u32; 3], [u32; 3]),
    Plain(usize, usize, [u32; 3], [u32; 3]),
    Multigrid,
}

/// One solver, 3D or 2D: its pipelines and bind groups in order, its state
/// and lighting.
struct Solver {
    pipelines: Vec<ComputePipeline>,
    groups: Vec<BindGroup>,
    /// The step's bind group, with the step's dynamic offset.
    step_group: BindGroup,
    passes: Vec<Pass>,
    light: Vec<Pass>,
    multigrid: Option<Multigrid>,
    /// Everything a step reads that it did not write itself: velocity,
    /// density and temperature, and pressure (the solver's warm start).
    state: Vec<Buffer>,
    _keep: Vec<Buffer>,
    _textures: Vec<Texture>,
}

impl Solver {
    fn run(&self, pass: &mut ComputePass, list: &[Pass], offset: u32) {
        for p in list {
            match *p {
                Pass::Step(k, n, size) => {
                    pass.set_bind_group(0, &self.step_group, &[offset]);
                    pass.set_pipeline(&self.pipelines[k]);
                    pass.dispatch_workgroups(
                        workgroups(n[0], size[0]),
                        workgroups(n[1], size[1]),
                        workgroups(n[2], size[2]),
                    );
                }
                Pass::Plain(k, g, n, size) => {
                    pass.set_bind_group(0, &self.groups[g], &[]);
                    pass.set_pipeline(&self.pipelines[k]);
                    pass.dispatch_workgroups(
                        workgroups(n[0], size[0]),
                        workgroups(n[1], size[1]),
                        workgroups(n[2], size[2]),
                    );
                }
                Pass::Multigrid => self.multigrid.as_ref().unwrap().encode(pass),
            }
        }
    }
}

/// The smoke's GPU side: one solver, the steps' parameters, the light
/// layers the fragments draw and the kept states.
pub struct FrameFluid {
    grids: Grids,
    solver: Solver,
    steps: Buffer,
    lighting: Buffer,
    lit: Option<(usize, Lighting)>,
    drawn: BindGroup,
    _layers: [Texture; 2],
    kept: Vec<Option<Vec<Buffer>>>,
    /// The state the solver holds, once the first has been made.
    done: Option<usize>,
    /// The pre-roll's steps that have run, all of them once state 0 is made.
    prerolled: usize,
    start: f64,
}

/// When the fluid starts, in clip seconds.
pub fn fluid_start(preview: &FieldPreview) -> f64 {
    match FLUID.start {
        FluidStart::Part5 => preview.seconds(0),
        FluidStart::PictureFade => preview.picture_fade.unwrap_or(preview.seconds(0)),
    }
}

impl FrameFluid {
    /// The smoke for a preview, with the frame's shape and the fitted
    /// relief; `drawn` is the fragments' group-1 layout (field_render.rs),
    /// whose two textures it fills with the light behind the frame and in
    /// front of it.
    pub fn new(
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        shape: &FrameShape,
        relief: &Texture,
        drawn: &BindGroupLayout,
    ) -> Self {
        let grids = Grids::of(SMOKE);
        let [sx, sy, _] = grids.s();
        let layer_usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
        let layers = ["Part 5 smoke behind", "Part 5 smoke in front"].map(|label| {
            texture(
                device,
                label,
                [sx, sy, 1],
                TextureDimension::D2,
                TextureFormat::Rgba16Float,
                layer_usage,
            )
        });
        let layer_views = layers
            .each_ref()
            .map(|t| t.create_view(&Default::default()));
        let obstacle = texture(
            device,
            "Part 5 smoke obstacles",
            [sx, sy, 1],
            TextureDimension::D2,
            TextureFormat::Rgba16Float,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        );
        let texels = obstacles(shape, &grids);
        queue.write_texture(
            obstacle.as_image_copy(),
            bytemuck::cast_slice(&texels),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(sx * 8),
                rows_per_image: Some(sy),
            },
            Extent3d {
                width: sx,
                height: sy,
                depth_or_array_layers: 1,
            },
        );
        let steps = device.create_buffer(&BufferDescriptor {
            label: Some("Part 5 smoke steps"),
            size: STEP_STRIDE * STEP_SLOTS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lighting = uniform_buffer(
            device,
            "Part 5 smoke lighting",
            std::mem::size_of::<Lighting>() as u64,
        );
        let relief_view = relief.create_view(&Default::default());
        let obstacle_view = obstacle.create_view(&Default::default());
        let solver = if grids.dim == 3 {
            solver_3d(
                device,
                queue,
                &grids,
                &steps,
                &lighting,
                &obstacle_view,
                &relief_view,
                &layer_views,
            )
        } else {
            solver_2d(
                device,
                queue,
                &grids,
                &steps,
                &lighting,
                &obstacle_view,
                &relief_view,
                &layer_views,
            )
        };
        let drawn = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 5 smoke, drawn"),
            layout: drawn,
            entries: &[
                entry(1, BindingResource::TextureView(&layer_views[0])),
                entry(2, BindingResource::TextureView(&layer_views[1])),
            ],
        });
        let start = fluid_start(preview);
        let span = preview.seconds(preview.frames.len()) - start;
        let count = (span * FLUID.rate) as usize / FLUID.checkpoint.max(1) + 2;
        let mut solver = solver;
        solver._textures.push(obstacle);
        Self {
            grids,
            solver,
            steps,
            lighting,
            lit: None,
            drawn,
            _layers: layers,
            kept: (0..count).map(|_| None).collect(),
            done: None,
            prerolled: 0,
            start,
        }
    }

    /// The fragments' bind group: the light behind the frame and in front.
    pub fn drawn(&self) -> &BindGroup {
        &self.drawn
    }

    /// Bytes of one state, and of the states kept so far.
    pub fn memory(&self) -> (u64, u64) {
        let one: u64 = self.solver.state.iter().map(Buffer::size).sum();
        (one, one * self.kept.iter().flatten().count() as u64)
    }

    /// Pre-roll steps due by `time`: [`PREROLL_RATE`] a second, ending
    /// with all of them [`PREROLL_LEAD`] seconds before the fluid's start,
    /// so beginning 32 s before it.
    fn preroll_due(&self, time: f64) -> usize {
        let begin = self.start - PREROLL_LEAD - PREROLL as f64 / PREROLL_RATE;
        ((time - begin) * PREROLL_RATE)
            .floor()
            .clamp(0.0, PREROLL as f64) as usize
    }

    /// Pre-roll steps `steps` of [`PREROLL`] from rest: each step's time,
    /// and the state it makes, state 0 at the last and none before.
    fn preroll_runs(&self, steps: std::ops::Range<usize>) -> Vec<(f64, Option<usize>)> {
        let dt = 1.0 / FLUID.rate;
        steps
            .map(|k| {
                let t = self.start - (PREROLL - k) as f64 * dt;
                (t, (k + 1 == PREROLL).then_some(0))
            })
            .collect()
    }

    /// Bring the solver's state to rest, when the pre-roll is still to
    /// make its first step.
    fn start_at_rest(&self, encoder: &mut CommandEncoder) {
        if self.prerolled == 0 {
            for b in &self.solver.state {
                encoder.clear_buffer(b, 0, None);
            }
        }
    }

    /// Ahead of the fluid's start, run the pre-roll steps due by `time`
    /// that have not run, at most [`PREROLL_BATCH`] of them. Nothing once
    /// the pre-roll is done: state 0 is kept, and seeking back before the
    /// start keeps it.
    pub fn preroll(&mut self, device: &Device, queue: &Queue, preview: &FieldPreview, time: f64) {
        if self.prerolled == PREROLL {
            return;
        }
        let to = self.preroll_due(time).min(self.prerolled + PREROLL_BATCH);
        if to <= self.prerolled {
            return;
        }
        let mut encoder = device.create_command_encoder(&Default::default());
        self.start_at_rest(&mut encoder);
        let runs = self.preroll_runs(self.prerolled..to);
        self.run(device, queue, preview, encoder, &runs, None);
        self.prerolled = to;
        if to == PREROLL {
            self.done = Some(0);
        }
    }

    /// Bring the smoke to the moment `time` shows, the state one step past
    /// it, and light it; before the fluid's start, run the pre-roll steps
    /// due ([`FrameFluid::preroll`]). Returns `u.field.fluid`.
    pub fn advance(
        &mut self,
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        time: f64,
    ) -> [f32; 4] {
        let dt = 1.0 / FLUID.rate;
        let elapsed = time - self.start;
        if elapsed < 0.0 {
            self.preroll(device, queue, preview, time);
            return [0.0, 0.0, dt as f32, elapsed as f32];
        }
        let position = elapsed * FLUID.rate;
        let target = position.floor() as usize + 1;
        let every = FLUID.checkpoint.max(1);
        let mut encoder = device.create_command_encoder(&Default::default());
        // Seeking back: the latest kept state at or before the target, or
        // the start.
        if self.done.is_none_or(|done| done > target) {
            let wanted = (target / every).min(self.kept.len() - 1);
            self.done = None;
            if let Some(k) = (0..=wanted).rev().find(|&k| self.kept[k].is_some()) {
                for (from, to) in self.kept[k]
                    .as_ref()
                    .unwrap()
                    .iter()
                    .zip(&self.solver.state)
                {
                    encoder.copy_buffer_to_buffer(from, 0, to, 0, None);
                }
                self.done = Some(k * every);
            }
        }
        // The steps to run: state 0 is the pre-roll from rest, the rest of
        // it when it has not finished ahead of the start; state n is made
        // by the step from state n - 1, with the block at its start.
        let mut runs = Vec::new();
        if self.done.is_none() {
            self.start_at_rest(&mut encoder);
            runs = self.preroll_runs(self.prerolled..PREROLL);
            self.prerolled = PREROLL;
        }
        let first = self.done.map_or(1, |done| done + 1);
        for n in first..=target {
            runs.push((self.start + (n - 1) as f64 * dt, Some(n)));
        }
        self.run(device, queue, preview, encoder, &runs, Some((time, target)));
        self.done = Some(target);
        [
            target as f32,
            (position - (target - 1) as f64) as f32,
            dt as f32,
            elapsed as f32,
        ]
    }

    /// Run `runs` after what `encoder` holds, and light state `target` for
    /// the moment `time` when `light` is `Some((time, target))`, unless it
    /// is lit already; submit it all. Each run is a step's time and the
    /// state it makes (none for a pre-roll step before the last). State n
    /// is kept as it is first made, every `FLUID.checkpoint` steps.
    fn run(
        &mut self,
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        mut encoder: CommandEncoder,
        runs: &[(f64, Option<usize>)],
        light: Option<(f64, usize)>,
    ) {
        let dt = 1.0 / FLUID.rate;
        let every = FLUID.checkpoint.max(1);
        let mut slot = 0;
        let mut pass = encoder.begin_compute_pass(&Default::default());
        for &(t, made) in runs {
            if slot == STEP_SLOTS {
                drop(pass);
                queue.submit([encoder.finish()]);
                encoder = device.create_command_encoder(&Default::default());
                pass = encoder.begin_compute_pass(&Default::default());
                slot = 0;
            }
            let block = preview.controls(t);
            let step = step_params(&Moment { t, b: &block }, &self.grids, dt);
            let offset = slot as u64 * STEP_STRIDE;
            queue.write_buffer(&self.steps, offset, bytemuck::bytes_of(&step));
            self.solver
                .run(&mut pass, &self.solver.passes, offset as u32);
            slot += 1;
            // Keep state n as it is first made, at the last pre-roll step for
            // state 0.
            let Some(n) = made else { continue };
            let k = n / every;
            if n % every == 0 && k < self.kept.len() && self.kept[k].is_none() {
                drop(pass);
                let copies: Vec<Buffer> = self
                    .solver
                    .state
                    .iter()
                    .map(|b| {
                        let copy = device.create_buffer(&BufferDescriptor {
                            label: Some("Part 5 smoke kept"),
                            size: b.size(),
                            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        encoder.copy_buffer_to_buffer(b, 0, &copy, 0, None);
                        copy
                    })
                    .collect();
                self.kept[k] = Some(copies);
                pass = encoder.begin_compute_pass(&Default::default());
            }
        }
        // Light the state for this moment, unless it is lit already.
        if let Some((time, target)) = light {
            let block = preview.controls(time);
            let light = lighting(&Moment { t: time, b: &block }, &self.grids);
            if self.lit != Some((target, light)) {
                queue.write_buffer(&self.lighting, 0, bytemuck::bytes_of(&light));
                self.solver.run(&mut pass, &self.solver.light, 0);
                self.lit = Some((target, light));
            }
        }
        drop(pass);
        queue.submit([encoder.finish()]);
    }

    /// The pre-roll's steps that have run, and its length.
    #[cfg(test)]
    pub fn prerolled(&self) -> (usize, usize) {
        (self.prerolled, PREROLL)
    }

    /// A hash of the state the solver holds, and its number.
    #[cfg(test)]
    pub fn read(&self, device: &Device, queue: &Queue) -> (u64, usize) {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for b in &self.solver.state {
            let read = device.create_buffer(&BufferDescriptor {
                label: Some("Part 5 smoke readback"),
                size: b.size(),
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(b, 0, &read, 0, None);
            queue.submit([encoder.finish()]);
            read.slice(..).map_async(MapMode::Read, |_| {});
            device.poll(PollType::wait_indefinitely()).unwrap();
            for &byte in read.slice(..).get_mapped_range().unwrap().iter() {
                hash = (hash ^ byte as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
        (hash, self.done.unwrap_or(0))
    }
}

#[allow(clippy::too_many_arguments)]
fn solver_3d(
    device: &Device,
    queue: &Queue,
    grids: &Grids,
    steps: &Buffer,
    lighting: &Buffer,
    obstacle: &TextureView,
    relief: &TextureView,
    layers: &[TextureView; 2],
) -> Solver {
    let [nx, ny, nz] = grids.n;
    let [sx, sy, sz] = grids.s();
    let [lx, ly, lz] = grids.l();
    let (nu, nv, nw) = ((nx + 1) * ny * nz, nx * (ny + 1) * nz, nx * ny * (nz + 1));
    let grid = Grid3 {
        nx,
        ny,
        nz,
        ds: grids.ds,
        sx,
        sy,
        sz,
        ls: grids.ls,
        lx,
        ly,
        lz,
        solid_off: 0,
        ou: 0,
        ov: nu,
        ow: nu + nv,
        flags: 0,
        z0: grids.origin[2] as f32,
        h: grids.h() as f32,
        inv_ds: 1.0 / grids.ds as f32,
        sponge: 6.0,
        x0: grids.origin[0] as f32,
        y0: grids.origin[1] as f32,
        _s: 0.0,
        _t: 0.0,
    };
    let grid_buffer = uniform_buffer(
        device,
        "Part 5 smoke grid",
        std::mem::size_of::<Grid3>() as u64,
    );
    queue.write_buffer(&grid_buffer, 0, bytemuck::bytes_of(&grid));
    let faces = (nu + nv + nw) as u64 * 4;
    let cells = (nx * ny * nz) as u64;
    let scalars = (sx * sy * sz) as u64;
    let b = |label: &str, size: u64| storage_buffer(device, label, size);
    let vel = b("Part 5 smoke velocity", faces);
    let vel_hat = b("Part 5 smoke velocity, predicted", faces);
    let vel_tmp = b("Part 5 smoke velocity, advected", faces);
    let sc = b("Part 5 smoke density and temperature", scalars * 4);
    let sc_hat = b("Part 5 smoke scalars, predicted", scalars * 4);
    let sc_tmp = b("Part 5 smoke scalars, advected", scalars * 4);
    let divergence = b("Part 5 smoke divergence", cells * 4);
    let pressure = b("Part 5 smoke pressure", cells * 4);
    let curl = b("Part 5 smoke curl", cells * 16);
    let solid_v = b("Part 5 smoke solid cells", cells * 4);
    let solid_s = b("Part 5 smoke solid scalar cells", scalars * 4);
    let confinement = b("Part 5 smoke confinement", cells * 16);
    let outline = b("Part 5 smoke mountain outline", sx as u64 * 4);
    let mut entries = vec![
        uniform_entry(0, None),
        uniform_entry(1, Some(std::mem::size_of::<Step>() as u64)),
    ];
    entries.extend((2..13).map(|i| storage_entry(i, false)));
    entries.push(storage_entry(16, false));
    entries.push(storage_entry(19, false));
    entries.push(texture_entry(13, TextureViewDimension::D3));
    entries.push(sampler_entry(14));
    entries.push(texture_entry(17, TextureViewDimension::D2));
    entries.push(texture_entry(18, TextureViewDimension::D2));
    let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Part 5 smoke 3D"),
        entries: &entries,
    });
    let cell_velocity = volume(device, "Part 5 smoke cell velocity", [nx, ny, nz]);
    let cell_view = cell_velocity.create_view(&Default::default());
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("Part 5 smoke"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });
    let step_binding = BindingResource::Buffer(BufferBinding {
        buffer: steps,
        offset: 0,
        size: NonZeroU64::new(std::mem::size_of::<Step>() as u64),
    });
    let all = [
        &vel,
        &vel_hat,
        &vel_tmp,
        &sc,
        &sc_hat,
        &sc_tmp,
        &divergence,
        &pressure,
        &curl,
        &solid_v,
        &solid_s,
    ];
    let mut group_entries = vec![
        entry(0, grid_buffer.as_entire_binding()),
        entry(1, step_binding),
    ];
    for (i, x) in all.iter().enumerate() {
        group_entries.push(entry(2 + i as u32, x.as_entire_binding()));
    }
    group_entries.push(entry(16, confinement.as_entire_binding()));
    group_entries.push(entry(19, outline.as_entire_binding()));
    group_entries.push(entry(13, BindingResource::TextureView(&cell_view)));
    group_entries.push(entry(14, BindingResource::Sampler(&sampler)));
    group_entries.push(entry(17, BindingResource::TextureView(obstacle)));
    group_entries.push(entry(18, BindingResource::TextureView(relief)));
    let step_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke 3D"),
        layout: &layout,
        entries: &group_entries,
    });
    let pl = pipeline_layout(device, &layout);
    let [solver_source, mg_source, light_source, _, _] = sources();
    let m = module(device, solver_source.0, solver_source.1);
    let mut pipelines = Vec::new();
    let mut add = |p: ComputePipeline| {
        pipelines.push(p);
        pipelines.len() - 1
    };
    let cp = |e: &str| compute(device, Some(&pl), &m, e);
    let cell_vel = compute(device, None, &m, "cell_vel");
    let cell_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke cell velocity"),
        layout: &cell_vel.get_bind_group_layout(0),
        entries: &[
            entry(0, grid_buffer.as_entire_binding()),
            entry(2, vel.as_entire_binding()),
            entry(15, BindingResource::TextureView(&cell_view)),
        ],
    });
    let k_cell = add(cell_vel);
    let init_masks = add(cp("init_masks"));
    let outline_k = add(cp("outline_columns"));
    let comps = ["u", "v", "w"];
    let advect_1 = comps.map(|c| add(cp(&format!("advect_vel_1_{c}"))));
    let advect_2 = comps.map(|c| add(cp(&format!("advect_vel_2_{c}"))));
    let advect_s_1 = add(cp("advect_s_1"));
    let advect_s_2 = add(cp("advect_s_2"));
    let sources_k = add(cp("sources"));
    let vorticity = add(cp("vorticity"));
    let confine = add(cp("confinement"));
    let forces = comps.map(|c| add(cp(&format!("forces_{c}"))));
    let divergence_k = add(cp("divergence"));
    let project = comps.map(|c| add(cp(&format!("project_{c}"))));
    let fd = [[nx + 1, ny, nz], [nx, ny + 1, nz], [nx, ny, nz + 1]];
    let wg = [8, 8, 4];
    let mut passes = vec![Pass::Plain(k_cell, 0, [nx, ny, nz], wg)];
    passes.extend((0..3).map(|c| Pass::Step(advect_1[c], fd[c], wg)));
    passes.extend((0..3).map(|c| Pass::Step(advect_2[c], fd[c], wg)));
    passes.push(Pass::Step(advect_s_1, [sx, sy, sz], wg));
    passes.push(Pass::Step(advect_s_2, [sx, sy, sz], wg));
    passes.push(Pass::Step(outline_k, [sx, 1, 1], [64, 1, 1]));
    passes.push(Pass::Step(sources_k, [sx, sy, sz], wg));
    passes.push(Pass::Step(vorticity, [nx, ny, nz], wg));
    passes.push(Pass::Step(confine, [nx, ny, nz], wg));
    passes.extend((0..3).map(|c| Pass::Step(forces[c], fd[c], wg)));
    passes.push(Pass::Step(divergence_k, [nx, ny, nz], wg));
    passes.push(Pass::Multigrid);
    passes.extend((0..3).map(|c| Pass::Step(project[c], fd[c], wg)));

    // The multigrid's levels: level 0 is the pressure itself.
    let mgm = module(device, mg_source.0, mg_source.1);
    let mut mentries = vec![uniform_entry(0, None)];
    mentries.extend((1..11).map(|i| storage_entry(i, false)));
    let mlayout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Part 5 smoke pressure"),
        entries: &mentries,
    });
    let mpl = pipeline_layout(device, &mlayout);
    let mp = |e: &str| compute(device, Some(&mpl), &mgm, e);
    let mut dims = vec![[nx, ny, nz]];
    loop {
        let l = *dims.last().unwrap();
        let c = [l[0].div_ceil(2), l[1].div_ceil(2), l[2].div_ceil(2)];
        if c.iter().any(|&x| x < 3) || dims.len() >= 7 {
            break;
        }
        dims.push(c);
    }
    let mut keep = Vec::new();
    let mut xs = vec![pressure.clone()];
    let mut bs = vec![divergence.clone()];
    let mut ms = vec![solid_v.clone()];
    let dummy_w = b("Part 5 smoke pressure, level-0 conductances", 32);
    let mut ws = vec![dummy_w.clone()];
    for dm in dims.iter().skip(1) {
        let n = (dm[0] * dm[1] * dm[2]) as u64 * 4;
        xs.push(b("Part 5 smoke pressure level", n));
        bs.push(b("Part 5 smoke pressure level", n));
        ms.push(b("Part 5 smoke pressure level", n));
        ws.push(b("Part 5 smoke pressure level", n * 8));
    }
    let dummy = b("Part 5 smoke pressure, none", 16);
    let open0 = b("Part 5 smoke pressure, open faces", cells * 4);
    let mut levels = Vec::new();
    for (i, dm) in dims.iter().enumerate() {
        let r = b(
            "Part 5 smoke pressure residual",
            (dm[0] * dm[1] * dm[2]) as u64 * 4,
        );
        let last = i + 1 == dims.len();
        let c = if last { [1, 1, 1] } else { dims[i + 1] };
        let lu = uniform_buffer(device, "Part 5 smoke pressure level", 32);
        let words: [u32; 8] = [
            dm[0],
            dm[1],
            dm[2],
            (i > 0) as u32,
            c[0],
            c[1],
            c[2],
            i as u32 + 1,
        ];
        queue.write_buffer(&lu, 0, bytemuck::cast_slice(&words));
        let (xc, bc, mc, wc) = if last {
            (&dummy, &dummy, &dummy, &dummy_w)
        } else {
            (&xs[i + 1], &bs[i + 1], &ms[i + 1], &ws[i + 1])
        };
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 5 smoke pressure level"),
            layout: &mlayout,
            entries: &[
                entry(0, lu.as_entire_binding()),
                entry(1, xs[i].as_entire_binding()),
                entry(2, bs[i].as_entire_binding()),
                entry(3, r.as_entire_binding()),
                entry(4, ms[i].as_entire_binding()),
                entry(5, xc.as_entire_binding()),
                entry(6, bc.as_entire_binding()),
                entry(7, mc.as_entire_binding()),
                entry(8, ws[i].as_entire_binding()),
                entry(9, wc.as_entire_binding()),
                entry(
                    10,
                    if i == 0 {
                        open0.as_entire_binding()
                    } else {
                        dummy.as_entire_binding()
                    },
                ),
            ],
        });
        keep.push(lu);
        keep.push(r);
        levels.push(Level { group, n: *dm, c });
    }
    keep.extend(xs.into_iter().skip(1));
    keep.extend(bs.into_iter().skip(1));
    keep.extend(ms.into_iter().skip(1));
    keep.extend(ws);
    keep.push(dummy);
    keep.push(open0);
    let open_bits = mp("open_bits");
    let coarsen = mp("coarsen_mask");
    let multigrid = Multigrid {
        levels,
        red: mp("smooth_red"),
        black: mp("smooth_black"),
        restrict: mp("restrict_down"),
        prolong: mp("prolong"),
        _keep: keep,
    };

    // The lighting.
    let density = volume(device, "Part 5 smoke density, light grid", [lx, ly, lz]);
    let depths = volume(device, "Part 5 smoke optical depths", [lx, ly, lz]);
    let dv = density.create_view(&Default::default());
    let tv = depths.create_view(&Default::default());
    let lm = module(device, light_source.0, light_source.1);
    let prep = compute(device, None, &lm, "prep");
    let light_vol = compute(device, None, &lm, "light_vol");
    let column = compute(device, None, &lm, "column");
    let view = BindingResource::TextureView;
    let prep_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke light prep"),
        layout: &prep.get_bind_group_layout(0),
        entries: &[
            entry(0, grid_buffer.as_entire_binding()),
            entry(2, sc.as_entire_binding()),
            entry(3, view(&dv)),
        ],
    });
    let light_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke light to the lights"),
        layout: &light_vol.get_bind_group_layout(0),
        entries: &[
            entry(0, grid_buffer.as_entire_binding()),
            entry(1, lighting.as_entire_binding()),
            entry(4, view(&dv)),
            entry(5, BindingResource::Sampler(&sampler)),
            entry(6, view(&tv)),
            entry(10, view(obstacle)),
        ],
    });
    let column_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke light columns"),
        layout: &column.get_bind_group_layout(0),
        entries: &[
            entry(0, grid_buffer.as_entire_binding()),
            entry(1, lighting.as_entire_binding()),
            entry(2, sc.as_entire_binding()),
            entry(5, BindingResource::Sampler(&sampler)),
            entry(7, view(&tv)),
            entry(8, view(&layers[0])),
            entry(9, view(&layers[1])),
        ],
    });
    let k_prep = add(prep);
    let k_light = add(light_vol);
    let k_column = add(column);
    let light = vec![
        Pass::Plain(k_prep, 1, [lx, ly, lz], wg),
        Pass::Plain(k_light, 2, [lx, ly, lz], wg),
        Pass::Plain(k_column, 3, [sx, sy, 1], [16, 16, 1]),
    ];

    let solver = Solver {
        groups: vec![cell_group, prep_group, light_group, column_group],
        step_group,
        passes,
        light,
        multigrid: Some(multigrid),
        state: vec![vel.clone(), sc.clone(), pressure.clone()],
        _keep: vec![
            grid_buffer,
            vel,
            vel_hat,
            vel_tmp,
            sc,
            sc_hat,
            sc_tmp,
            divergence,
            pressure,
            curl,
            solid_v,
            solid_s,
            confinement,
            outline,
        ],
        _textures: vec![cell_velocity, density, depths],
        pipelines,
    };
    // The masks, once: the solid cells and the multigrid's faces.
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &solver.step_group, &[0]);
        pass.set_pipeline(&solver.pipelines[init_masks]);
        pass.dispatch_workgroups(workgroups(sx, 8), workgroups(sy, 8), workgroups(sz, 4));
        let mg = solver.multigrid.as_ref().unwrap();
        pass.set_bind_group(0, &mg.levels[0].group, &[]);
        pass.set_pipeline(&open_bits);
        pass.dispatch_workgroups(workgroups(nx, 8), workgroups(ny, 8), workgroups(nz, 4));
        pass.set_pipeline(&coarsen);
        for l in &mg.levels[..mg.levels.len() - 1] {
            pass.set_bind_group(0, &l.group, &[]);
            pass.dispatch_workgroups(
                workgroups(l.c[0], 8),
                workgroups(l.c[1], 8),
                workgroups(l.c[2], 4),
            );
        }
    }
    queue.write_buffer(steps, 0, bytemuck::bytes_of(&Step::zeroed()));
    queue.submit([encoder.finish()]);
    solver
}

#[allow(clippy::too_many_arguments)]
fn solver_2d(
    device: &Device,
    queue: &Queue,
    grids: &Grids,
    steps: &Buffer,
    lighting: &Buffer,
    obstacle: &TextureView,
    relief: &TextureView,
    layers: &[TextureView; 2],
) -> Solver {
    let [nx, ny, _] = grids.n;
    let [sx, sy, _] = grids.s();
    let grid = Grid2 {
        nx,
        ny,
        sx,
        sy,
        ds: grids.ds as f32,
        inv_ds: 1.0 / grids.ds as f32,
        omega: 1.8,
        _pad: 0.0,
    };
    let grid_buffer = uniform_buffer(
        device,
        "Part 5 smoke grid",
        std::mem::size_of::<Grid2>() as u64,
    );
    queue.write_buffer(&grid_buffer, 0, bytemuck::bytes_of(&grid));
    let (nxu, nyu, sxu, syu) = (nx as u64, ny as u64, sx as u64, sy as u64);
    let b = |label: &str, size: u64| storage_buffer(device, label, size);
    let u = b("Part 5 smoke u", (nxu + 1) * nyu * 4);
    let v = b("Part 5 smoke v", nxu * (nyu + 1) * 4);
    let u_hat = b("Part 5 smoke u, predicted", (nxu + 1) * nyu * 4);
    let v_hat = b("Part 5 smoke v, predicted", nxu * (nyu + 1) * 4);
    let u_tmp = b("Part 5 smoke u, advected", (nxu + 1) * nyu * 4);
    let v_tmp = b("Part 5 smoke v, advected", nxu * (nyu + 1) * 4);
    let sc = b("Part 5 smoke density and temperature", sxu * syu * 8);
    let sc_hat = b("Part 5 smoke scalars, predicted", sxu * syu * 8);
    let sc_tmp = b("Part 5 smoke scalars, advected", sxu * syu * 8);
    let divergence = b("Part 5 smoke divergence", nxu * nyu * 4);
    let pressure = b("Part 5 smoke pressure", nxu * nyu * 4);
    let curl = b("Part 5 smoke curl", nxu * nyu * 4);
    let solid_v = b("Part 5 smoke solid cells", nxu * nyu * 4);
    let solid_s = b("Part 5 smoke solid scalar cells", sxu * syu * 4);
    let coarse = b("Part 5 smoke coarse density", (sxu / 4) * (syu / 4) * 4);
    let outline = b("Part 5 smoke mountain outline", sxu * 4);
    let mut entries = vec![
        uniform_entry(0, None),
        uniform_entry(1, Some(std::mem::size_of::<Step>() as u64)),
    ];
    entries.extend((2..16).map(|i| storage_entry(i, false)));
    entries.push(texture_entry(16, TextureViewDimension::D2));
    entries.push(texture_entry(17, TextureViewDimension::D2));
    entries.push(sampler_entry(18));
    entries.push(storage_entry(19, false));
    let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Part 5 smoke 2D"),
        entries: &entries,
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
        label: Some("Part 5 smoke"),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });
    let all = [
        &u,
        &v,
        &u_hat,
        &v_hat,
        &u_tmp,
        &v_tmp,
        &sc,
        &sc_hat,
        &sc_tmp,
        &divergence,
        &pressure,
        &curl,
        &solid_v,
        &solid_s,
    ];
    let mut group_entries = vec![
        entry(0, grid_buffer.as_entire_binding()),
        entry(
            1,
            BindingResource::Buffer(BufferBinding {
                buffer: steps,
                offset: 0,
                size: NonZeroU64::new(std::mem::size_of::<Step>() as u64),
            }),
        ),
    ];
    for (i, x) in all.iter().enumerate() {
        group_entries.push(entry(2 + i as u32, x.as_entire_binding()));
    }
    group_entries.push(entry(16, BindingResource::TextureView(obstacle)));
    group_entries.push(entry(17, BindingResource::TextureView(relief)));
    group_entries.push(entry(18, BindingResource::Sampler(&sampler)));
    group_entries.push(entry(19, outline.as_entire_binding()));
    let step_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke 2D"),
        layout: &layout,
        entries: &group_entries,
    });
    let pl = pipeline_layout(device, &layout);
    let [_, _, _, solver_source, light_source] = sources();
    let m = module(device, solver_source.0, solver_source.1);
    let mut pipelines = Vec::new();
    let mut add = |p: ComputePipeline| {
        pipelines.push(p);
        pipelines.len() - 1
    };
    let cp = |e: &str| compute(device, Some(&pl), &m, e);
    let init_masks = add(cp("init_masks"));
    let wg = [16, 16, 1];
    let mut passes = Vec::new();
    for (name, n) in [
        ("advect_vel_1", [nx + 1, ny + 1, 1]),
        ("advect_vel_2", [nx + 1, ny + 1, 1]),
        ("advect_s_1", [sx, sy, 1]),
        ("advect_s_2", [sx, sy, 1]),
    ] {
        passes.push(Pass::Step(add(cp(name)), n, wg));
    }
    passes.push(Pass::Step(
        add(cp("outline_columns")),
        [sx, 1, 1],
        [64, 1, 1],
    ));
    for (name, n) in [
        ("sources", [sx, sy, 1]),
        ("vorticity", [nx, ny, 1]),
        ("forces", [nx + 1, ny + 1, 1]),
        ("divergence", [nx, ny, 1]),
    ] {
        passes.push(Pass::Step(add(cp(name)), n, wg));
    }
    let red = add(cp("sor_red"));
    let black = add(cp("sor_black"));
    for _ in 0..60 {
        passes.push(Pass::Step(red, [nx.div_ceil(2), ny, 1], wg));
        passes.push(Pass::Step(black, [nx.div_ceil(2), ny, 1], wg));
    }
    passes.push(Pass::Step(add(cp("project")), [nx + 1, ny + 1, 1], wg));

    let lm = module(device, light_source.0, light_source.1);
    let llayout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Part 5 smoke 2D light"),
        entries: &[
            uniform_entry(0, None),
            uniform_entry(1, None),
            storage_entry(2, true),
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: TextureFormat::Rgba16Float,
                    view_dimension: TextureViewDimension::D2,
                },
                count: None,
            },
            storage_entry(4, false),
        ],
    });
    let lpl = pipeline_layout(device, &llayout);
    let light_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Part 5 smoke 2D light"),
        layout: &llayout,
        entries: &[
            entry(0, grid_buffer.as_entire_binding()),
            entry(1, lighting.as_entire_binding()),
            entry(2, sc.as_entire_binding()),
            entry(3, BindingResource::TextureView(&layers[0])),
            entry(4, coarse.as_entire_binding()),
        ],
    });
    let down = add(compute(device, Some(&lpl), &lm, "downsample"));
    let light_k = add(compute(device, Some(&lpl), &lm, "light"));
    let light = vec![
        Pass::Plain(down, 0, [sx / 4, sy / 4, 1], wg),
        Pass::Plain(light_k, 0, [sx, sy, 1], wg),
    ];
    let solver = Solver {
        groups: vec![light_group],
        step_group,
        passes,
        light,
        multigrid: None,
        state: vec![u.clone(), v.clone(), sc.clone(), pressure.clone()],
        _keep: vec![
            grid_buffer,
            u,
            v,
            u_hat,
            v_hat,
            u_tmp,
            v_tmp,
            sc,
            sc_hat,
            sc_tmp,
            divergence,
            pressure,
            curl,
            solid_v,
            solid_s,
            coarse,
            outline,
        ],
        _textures: vec![],
        pipelines,
    };
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_bind_group(0, &solver.step_group, &[0]);
        pass.set_pipeline(&solver.pipelines[init_masks]);
        pass.dispatch_workgroups(workgroups(sx, 16), workgroups(sy, 16), 1);
    }
    queue.write_buffer(steps, 0, bytemuck::bytes_of(&Step::zeroed()));
    queue.submit([encoder.finish()]);
    solver
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every smoke module parses and validates.
    #[test]
    fn smoke_modules_validate() {
        for (name, source) in sources() {
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }

    /// The uniforms' sizes, as the kernels lay them out.
    #[test]
    fn uniforms_match_the_kernels() {
        assert_eq!(std::mem::size_of::<Grid3>(), 96);
        assert_eq!(std::mem::size_of::<Grid2>(), 32);
        assert_eq!(std::mem::size_of::<Step>(), 32 + 5 * 16 + 48 * 16);
        assert!(std::mem::size_of::<Step>() as u64 <= STEP_STRIDE);
        assert_eq!(std::mem::size_of::<Lighting>(), 16 * 4 + 16 * 16 + 32);
    }

    /// The grids cover the 16:9 screen, the 3D one with its margins.
    #[test]
    fn grids_cover_the_screen() {
        for kind in [SmokeKind::Volume, SmokeKind::SoftVolume, SmokeKind::Flat] {
            let g = Grids::of(kind);
            let r = g.region();
            assert!(
                r[0] <= -53.3 && r[2] >= 373.3 && r[1] <= 0.0 && r[3] >= 200.0,
                "{kind:?} {r:?}"
            );
            assert_eq!(g.s()[0] % 4, 0);
        }
        assert_eq!(Grids::of(SmokeKind::Volume).n, [208, 124, 64]);
        assert_eq!(Grids::of(SmokeKind::Volume).s(), [416, 248, 128]);
    }

    fn block() -> FieldBlock {
        FieldBlock {
            part: [1.0, 5.0, 0.0, 300.0],
            window: [0.0, -1.0, 1.0, 0.0],
            events: [0.9, 14.8, 15.7, 16.8],
            events_late: [16.6, 40.6, 46.4, 48.8],
            view: [0.0, 256.0, 70.0, 179.0],
            ..Default::default()
        }
    }

    /// In the zoom the smoke drifts, cool, with no heat; the mountain's heat
    /// follows its light, whoever gives it; after the last terrain pixel the
    /// smoke settles.
    #[test]
    fn the_story_follows_the_scene() {
        let grids = Grids::of(SmokeKind::Volume);
        let zoom = block();
        let m = Moment { t: 86.27, b: &zoom };
        assert_eq!(m.heat(), 0.0);
        let s = step_params(&m, &grids, 1.0 / 60.0);
        assert_eq!(s.heat[0], 0.0);
        assert_eq!(s.n_emit, 6);
        assert!(s.wind[0] != 0.0 && s.wind[2] == 0.0);
        // The stand-in's lava, once risen.
        let mut terrain = block();
        terrain.part = [1.0, 30.0, 1.0, 2000.0];
        terrain.mountain = [64.0, 248.0, 900.0, 1.0];
        terrain.window = [1.0, 12.0, 1.0, 1.0];
        let m = Moment {
            t: 111.27,
            b: &terrain,
        };
        assert!((m.lava_strength() - 1.0).abs() < 1e-9);
        assert!(m.heat() > 0.9);
        assert_eq!(m.window(), [0.0; 3]);
        // The mountain author's lava, at half the stand-in's.
        terrain.mountain_style[1] = [3.0, 0.775, 0.14, 0.0];
        let m = Moment {
            t: 111.27,
            b: &terrain,
        };
        assert!((m.lava_strength() - 0.5).abs() < 1e-6);
        // Settled.
        terrain.part[1] = 50.0;
        let m = Moment {
            t: 131.27,
            b: &terrain,
        };
        assert!(m.settle() > 0.99 && m.supply() < 0.01);
        // A pure function of the moment.
        let a = step_params(&m, &grids, 1.0 / 60.0);
        let b = step_params(&m, &grids, 1.0 / 60.0);
        assert_eq!(bytemuck::bytes_of(&a), bytemuck::bytes_of(&b));
    }
}
