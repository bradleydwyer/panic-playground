//! Part 4 (Plasma) for the HD preview: the state of every part-4 frame
//! read from the port's `Plasma` scene, from its first drawn frame (the
//! logo slab, white) to part 7's first frame, the original frames rebuilt
//! from that state on demand, and the tables the faithful and modern views
//! draw from (plasma_render.rs and the plasma_*.wgsl files). Part 4's wait,
//! before it draws, belongs to part 3's preview ([`DotsPreview`]), which
//! ends on the same slab frame this one begins with.
//!
//! What comes from where:
//! - Read from the port's code and counters (`Plasma::hd_state` and
//!   `Plasma::hd_tables`, and `plasma::raster_starts`, which the scene's
//!   own render uses): the phases, the fade from white, the stride table
//!   and its two offsets, so every raster's start address, the line
//!   compare, the palette's rotation and level, the index field, the logo
//!   and its palette, and the music cues the scene reads (29:00, 2A:00,
//!   2D:00, 2E:14 and 2F:00).
//! - Observed when the preview loads: the times of the phases
//!   ([`PlasmaEvents`]), the music's position at each frame (and so the
//!   orders' first rows), and part 7's first frame.
//! - Interpretation: interpolation between the original's 70 Hz frames
//!   (the raster starts, the palette's rotation and level, the line
//!   compare); the field and the palette's sine evaluated at continuous
//!   positions; the original's even and odd rasters read as two layers.
//! - Authored: everything the modern view adds (plasma_stone.rs,
//!   plasma_light.wgsl and the values marked authored here).
use super::dots::DotsPreview;
use super::plasma_stone::Stone;
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::{
        greetings::Greetings,
        plasma::{HdPlasma, HdPlasmaTables, Plasma, raster_starts},
    },
    vga::Vga,
};
use bytemuck::{Pod, Zeroable};

/// `u.state.x` during part 4. Part 6 is 10, part 5 11, part 3 12.
pub const PART4_PHASE: f32 = 13.0;
/// Rasters the scanout shows, and the plasma's: the first 225 take their
/// strides from the table, the 226th jumps 510 bytes (read from the code).
pub const RASTERS: usize = 400;
pub const PLASMA_RASTERS: usize = 226;
/// Where the index field starts in video memory, its row length and rows,
/// and the logo's size (read from the code).
pub const FIELD_START: usize = 0x3840;
pub const FIELD_ROW: usize = 200;
pub const FIELD_ROWS: usize = 223;
pub const LOGO_WIDTH: usize = 320;
pub const LOGO_ROWS: usize = 180;
/// The palette's period in entries (colours 32 to 255) and the line
/// compare the logo rests at (read from the code).
pub const PALETTE_PERIOD: usize = 224;
pub const REST_LINE_COMPARE: usize = 236;
/// Seconds over which modern part 4 takes over from part 3's view at the
/// slab (authored); part 3's renderer still draws until then.
pub const HANDOVER: f64 = 0.35;
/// The modern view's samples of each layer's displacement: one per raster
/// of the layer (the even rasters 0 to 224, the odd 1 to 225).
pub const LAYER_SAMPLES: usize = PLASMA_RASTERS / 2;
/// How far the modern view smooths each layer's displacement down the
/// screen: a Gaussian's deviation, in the layer's rasters (authored).
pub const LAYER_SMOOTH: f64 = 3.0;
/// How far the modern view smooths the palette's rotation and level over
/// time, a Gaussian's deviation in frames (authored): the scene steps the
/// rotation by whole entries (8, 8, 9 …) and the level every fourth frame,
/// which the view would otherwise show as an uneven glide.
pub const PALETTE_SMOOTH: (f64, f64) = (1.5, 3.0);
/// The same for the layers' displacements, in frames (authored).
pub const LAYER_GLIDE: f64 = 1.0;
/// The most frames part 4's preview may hold, part 7's first included: the
/// modern renderer keeps each frame's palette as a row of one texture, and
/// `Renderer::new` asks the device for wgpu's default limits, under which a
/// texture is at most this high (8,192 rows, about 117 s at 70 Hz; the real
/// part is about 45 s). Parts 7 and 8 keep their palettes the same way
/// within one-minute limits, well inside it.
pub const MAX_FRAMES: usize = wgpu::Limits::defaults().max_texture_dimension_2d as usize;

/// What a part-4 frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    /// The logo slab fading from white (the scene's phase 1).
    Slab = 1,
    /// The plasma above the slab (phase 2), black until its level rises.
    Plasma = 2,
    /// The slab sliding away (phase 3).
    Slide = 3,
    /// Part 7's first frame.
    Part7 = 4,
}

/// One part-4 frame as the scanout shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlasmaFrame {
    pub phase: Phase,
    /// The counters the step that drew this frame rendered with: the
    /// scene's state before the step updated them.
    pub state: HdPlasma,
    /// The palette shown: its rotation (palette entries, unwrapped from
    /// frame to frame, so it may run past 224) and its level (0 to 63). The
    /// palette a step shows was filled in the step before, from the
    /// pointers set at the end of the step before that.
    pub rotation: f64,
    pub level: f64,
}

/// Times in part 4, in seconds from its first drawn frame (the slab),
/// observed from the frames when the preview loads; -1 for none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlasmaEvents {
    /// The first plasma frame (the field is in video memory, still black).
    pub plasma: f64,
    /// The first frame with the plasma's level above 0, the first at its
    /// full level, the first below it again and the first back at 0.
    pub fade_in: f64,
    pub full: f64,
    pub fade_out: f64,
    pub dark: f64,
    /// The first frame of the slide, and part 7's first frame (the last).
    pub slide: f64,
    pub part7: f64,
}

/// The part-4 controls every part-4 shader reads, `u.plasma`.
/// renderer::CONTROLS_WGSL declares the same layout; keep the two in step
/// (the tests check them). All zero before part 4.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PlasmaBlock {
    /// Shown (1), seconds since the slab frame, phase ([`Phase`]), part-4
    /// frame (continuous).
    pub part: [f32; 4],
    /// The frame shown, the next, the fraction of the way to it, and
    /// whether the two may be blended (1; 0 across a change of phase).
    pub frame: [f32; 4],
    /// The palette: rotation and level, interpolated, and the green and
    /// blue channels' offsets from the red (palette entries).
    pub palette: [f32; 4],
    /// The logo: the line compare (continuous), the slab's slide in source
    /// rows below its rest, the fade from white (0 white to 1 done), part
    /// 7's frame shown (1).
    pub logo: [f32; 4],
    /// Part-4 seconds of [`PlasmaEvents`]: plasma, fade in, full, fade out;
    /// dark, slide, part 7, unused.
    pub events: [f32; 4],
    pub events_late: [f32; 4],
    /// The modern view's camera: vertical focal length (source rows), the
    /// column on its axis, part 3's weight in the handover (1 at the slab
    /// frame, 0 after [`HANDOVER`]), the exposure.
    pub camera: [f32; 4],
    /// The slab (plasma_stone.rs): its top face's near edge's centre in
    /// camera space, slide included, and the edges' rounding.
    pub slab_origin: [f32; 4],
    /// Its tilt's sine and cosine, and its size: width, depth.
    pub slab_axes: [f32; 4],
    /// Its height, the deepest carving, and the stone's colour (linear):
    /// red and green.
    pub slab_size: [f32; 4],
    /// The stone's blue; the ground the slab stands on (0 water, 1 the
    /// polished floor, [`PlasmaPreview::polished_floor`]); unused.
    pub stone: [f32; 4],
    /// The plasma's front sheet of light: its bottom edge's centre in
    /// camera space, slide included, and its height.
    pub sheet: [f32; 4],
    /// Its half width, and the direction up it (camera space).
    pub sheet_size: [f32; 4],
    /// The orders' first rows (the music, read from its position) nearest
    /// before the moment: seconds since each of the last four (newest
    /// first; -100 none), and their strengths (filled in by the renderer).
    pub hits: [f32; 4],
    pub hit_strength: [f32; 4],
    /// The modern view's palette, smoothed over time
    /// ([`PlasmaPreview::glide`]): its rotation per frame toward the next
    /// (for the shutter), its rotation and its level; unused.
    pub motion: [f32; 4],
    /// The back sheet: its bottom edge's centre and height; its half width
    /// and the direction up it.
    pub sheet_back: [f32; 4],
    pub sheet_back_size: [f32; 4],
    /// The water ([`WaterTuning`]): the mirror's gain, the swell's height
    /// (its slopes' scale), the swell's own clock (seconds) and its size
    /// (its wavelengths' scale).
    pub water: [f32; 4],
    /// The drops on the water ([`PlasmaPreview::drops`]): each one's place
    /// on it (across, and back from below the slab's near top edge), its
    /// age (seconds) and 1, or all 0 for none.
    pub drops: [[f32; 4]; MAX_DROPS],
}

/// The water's mirror gain by default (authored; see plasma_light.wgsl,
/// shade_water): water reflects only 2 to 7% at this camera's angles.
pub const WATER_MIRROR: f64 = 3.0;
/// The live tuning (the preview's number keys): each a step of x1.25,
/// between these steps (from the authored swell and rate). The mirror's
/// gain is held to 1 (water as it is) to 8; the swell's height reaches 0 (a
/// flat mirror) one step below its lowest, and the drops' rate 0 (none)
/// likewise. The drops' rate stops where MAX_DROPS would cut drops short
/// (one every 0.9 s on average).
pub const WATER_MIRROR_RANGE: (f64, f64) = (1.0, 8.0);
pub const WATER_STEPS: (i32, i32) = (-6, 12);
pub const SWELL_HEIGHT_STEPS: (i32, i32) = (-10, 12);
pub const DROP_STEPS: (i32, i32) = (-6, 7);
const TUNING_STEP: f64 = 1.25;
/// The drops (authored): how many can be on the water at once, how long
/// each lasts (plasma_light.wgsl's DROP_LIFE), the range of the intervals
/// between them (seconds, at the default rate), and how long after the
/// plasma begins to light the first falls. The last falls in time to be
/// gone when the plasma is dark again.
pub const MAX_DROPS: usize = 6;
pub const DROP_LIFE: f64 = 4.0;
const DROP_INTERVALS: (f64, f64) = (3.0, 6.0);
const DROP_FIRST: f64 = 2.0;
/// Where drops fall (authored): rows of the screen below the plasma, the
/// distance from the frame's centre line (scene units, a 16:9 frame
/// reaching 178), and how far clear of the slab, so they fall in view and
/// never behind it.
const DROP_ROWS: (f64, f64) = (122.0, 192.0);
const DROP_ACROSS: (f64, f64) = (95.0, 170.0);
const DROP_CLEAR: f64 = 15.0;

/// The water's live tuning (authored), in steps of x1.25 from the
/// authored swell and rate. By default (chosen by eye with the keys) the
/// swell is 6 steps higher and faster, about x3.8, and the drops 6 steps
/// more often, one every 1.2 s on average.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterTuning {
    /// The mirror's gain, the swell's height, speed and size, and the
    /// drops' rate.
    pub mirror: i32,
    pub height: i32,
    pub speed: i32,
    pub size: i32,
    pub drops: i32,
    /// Where the swell's clock was when its speed last changed: part-4
    /// seconds and the clock's own, so a change of speed does not move
    /// the waves.
    anchor: (f64, f64),
}

impl Default for WaterTuning {
    fn default() -> Self {
        Self {
            mirror: 0,
            height: 6,
            speed: 6,
            size: 0,
            drops: 6,
            anchor: (0.0, 0.0),
        }
    }
}

/// The water's tuning knobs, as the preview's number keys step them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterKnob {
    Mirror,
    Height,
    Speed,
    Size,
    Drops,
}

/// A number from 0 to 1 for `k` (splitmix64), for the drops' fixed
/// schedule and places.
fn unit_hash(k: u64) -> f64 {
    let mut z = k.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as f64 / u64::MAX as f64
}

/// Part 4 of the HD clip: from its first drawn frame to part 7's first.
pub struct PlasmaPreview {
    /// Clip frame of the slab frame. Part-4 frame `j` is clip frame
    /// `start + j`; the same frame is the last of part 3's preview.
    pub start: usize,
    pub frames: Vec<PlasmaFrame>,
    /// The palette each frame shows, 768 6-bit DAC values each.
    pub palettes: Vec<u8>,
    /// The scene's tables.
    pub tables: HdPlasmaTables,
    /// The green and blue channels' offsets from the red in the palette's
    /// rotation, read from the pointers (constant: the scene adds the same
    /// motion to all three).
    pub offsets: [usize; 2],
    /// The music's position at each frame but the last, as the step that
    /// follows it read it.
    pub music: Vec<u16>,
    /// Part 7's first frame: 320 x 200 indices and its palette.
    pub part7: (Vec<u8>, Vec<u8>),
    pub events: PlasmaEvents,
    /// The modern view's slab: its pose fitted to the logo, and its relief.
    pub stone: Stone,
    /// The palette's rotation and level as the modern view takes them,
    /// smoothed over time within each phase (see [`PALETTE_SMOOTH`]).
    pub glide: Vec<[f64; 2]>,
    /// Clip times of the orders' first rows during part 4.
    order_starts: Vec<f64>,
    /// The modern view's ground: the polished stone floor instead of the
    /// water (authored, both; the preview's D key).
    pub polished_floor: bool,
    /// The water's live tuning.
    pub water: WaterTuning,
}

impl PlasmaPreview {
    /// Continue the clip from part 3's preview, which ends on part 4's
    /// first drawn frame: make part 4 again as the engine made it, replay
    /// its wait with the music positions part 3's preview read, which
    /// brings it to that frame, and step it on with the music until it
    /// reports done; the engine makes part 7 in that step, and its first
    /// frame ends the clip. The PCM of every step is appended but for that
    /// last frame's.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        dots: &DotsPreview,
    ) -> Result<Self, String> {
        let start = dots.start + dots.frames.len() - 1;
        let mut video = vga.clone();
        let mut plasma = Plasma::new(data, &mut video)?;
        let tables = plasma.hd_tables();
        let mut before = plasma.hd_state();
        for &position in &dots.music[dots.part4..] {
            before = plasma.hd_state();
            plasma.step(&mut video, position);
        }
        if video.height != RASTERS || before.phase != 1 {
            return Err("Part 4's replayed wait did not end on its slab".into());
        }
        let render = |pcm: &mut Vec<i16>, music: &mut Music, j: usize| {
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
        };
        let mut frames: Vec<PlasmaFrame> = Vec::new();
        let mut palettes = Vec::new();
        let mut positions = Vec::new();
        let mut offsets = None;
        let part7 = loop {
            let j = frames.len();
            // This frame and part 7's, after the loop, must both fit.
            if j + 2 > MAX_FRAMES {
                return Err(format!(
                    "Part 4 exceeded its import limit of {MAX_FRAMES} frames ({:.0} s), \
                     the most its palettes' texture holds",
                    MAX_FRAMES as f64 / FRAME_RATE as f64
                ));
            }
            // The palette this step showed was filled from the pointers the
            // previous frame's step rendered with.
            let pointers = frames.last().map_or([0; 3], |f| f.state.pointers);
            let rotation = (pointers[0] & 1023) as f64;
            let rotation = match frames.last() {
                Some(f) => {
                    let step = (rotation - f.rotation).rem_euclid(PALETTE_PERIOD as f64);
                    f.rotation
                        + if step >= PALETTE_PERIOD as f64 / 2.0 {
                            step - PALETTE_PERIOD as f64
                        } else {
                            step
                        }
                }
                None => rotation,
            };
            if before.phase == 2 && pointers[0] >> 10 > 0 {
                let p = PALETTE_PERIOD;
                let found = [
                    ((pointers[1] & 1023) + p - (pointers[0] & 1023)) % p,
                    ((pointers[2] & 1023) + p - (pointers[0] & 1023)) % p,
                ];
                if offsets.is_some_and(|o| o != found) {
                    return Err("Part 4's palette channels moved apart".into());
                }
                offsets = Some(found);
            }
            frames.push(PlasmaFrame {
                phase: match before.phase {
                    1 => Phase::Slab,
                    2 => Phase::Plasma,
                    _ => Phase::Slide,
                },
                state: before,
                rotation,
                level: (pointers[0] >> 10) as f64,
            });
            palettes.extend_from_slice(&video.palette);
            let position = music.position();
            positions.push(position);
            before = plasma.hd_state();
            let done = plasma.step(&mut video, position);
            render(pcm, music, j);
            if done {
                // The engine makes part 7 in this step: its first frame
                // comes next.
                Greetings::new(data, &mut video)?;
                break (video.indices.clone(), video.palette.clone());
            }
        };
        let last = frames.last().copied().ok_or("Part 4 drew nothing")?;
        frames.push(PlasmaFrame {
            phase: Phase::Part7,
            level: 0.0,
            ..last
        });
        palettes.extend_from_slice(&part7.1);
        *vga = video;
        let events = PlasmaEvents::observe(&frames);
        let glide = glide(&frames);
        let order_starts = (1..positions.len())
            .filter(|&j| positions[j] >> 8 != positions[j - 1] >> 8)
            .map(|j| (start + j) as f64 / FRAME_RATE as f64)
            .collect();
        let box_size = dots_box(dots);
        let stone = Stone::new(&tables.logo, &tables.palette, box_size)?;
        Ok(Self {
            start,
            frames,
            palettes,
            tables,
            offsets: offsets.ok_or("Part 4's plasma never lit")?,
            music: positions,
            part7,
            events,
            stone,
            glide,
            order_starts,
            polished_floor: false,
            water: WaterTuning::default(),
        })
    }
}

/// The palette's rotation and level smoothed over time, each frame's a
/// Gaussian mean over the frames of its phase.
fn glide(frames: &[PlasmaFrame]) -> Vec<[f64; 2]> {
    let sigma = [PALETTE_SMOOTH.0, PALETTE_SMOOTH.1];
    (0..frames.len())
        .map(|j| {
            std::array::from_fn(|c| {
                let reach = (3.0 * sigma[c]).ceil() as isize;
                let (mut sum, mut weight) = (0.0, 0.0);
                for d in -reach..=reach {
                    let k = j as isize + d;
                    if k < 0 || k as usize >= frames.len() {
                        continue;
                    }
                    let f = &frames[k as usize];
                    if f.phase != frames[j].phase {
                        continue;
                    }
                    let w = (-(d * d) as f64 / (2.0 * sigma[c] * sigma[c])).exp();
                    sum += w * if c == 0 { f.rotation } else { f.level };
                    weight += w;
                }
                sum / weight
            })
        })
        .collect()
}

/// The size of part 3's box (its object P3D7), read from its points'
/// extent: width, depth and height.
fn dots_box(dots: &DotsPreview) -> [f64; 3] {
    let points = &dots.tables.objects[6];
    std::array::from_fn(|k| {
        let (lo, hi) = points.iter().fold((i32::MAX, i32::MIN), |(lo, hi), p| {
            (lo.min(p[k]), hi.max(p[k]))
        });
        (hi - lo) as f64
    })
}

impl PlasmaEvents {
    /// Read the events from the frames.
    pub fn observe(frames: &[PlasmaFrame]) -> Self {
        let at = |j: Option<usize>| j.map_or(-1.0, |j| j as f64 / FRAME_RATE as f64);
        let first = |from: usize, test: &dyn Fn(&PlasmaFrame) -> bool| {
            frames[from.min(frames.len())..]
                .iter()
                .position(test)
                .map(|k| k + from)
        };
        let plasma = first(0, &|f| f.phase == Phase::Plasma);
        let lit = |f: &PlasmaFrame| f.phase == Phase::Plasma && f.level > 0.0;
        let fade_in = first(0, &lit);
        let full = first(fade_in.unwrap_or(0), &|f| f.level >= 63.0);
        let fade_out = full.and_then(|j| first(j, &|f| f.level < 63.0));
        let dark = fade_out.and_then(|j| first(j, &|f| f.level <= 0.0));
        Self {
            plasma: at(plasma),
            fade_in: at(fade_in),
            full: at(full),
            fade_out: at(fade_out),
            dark: at(dark),
            slide: at(first(0, &|f| f.phase == Phase::Slide)),
            part7: at(Some(frames.len() - 1)),
        }
    }
}

impl PlasmaPreview {
    /// Part-4 frame position at a clip time: continuous, negative before
    /// part 4's slab. A time within one audio sample of a frame's is that
    /// frame's (the clip's duration, a whole number of samples, falls a
    /// fraction of a sample short of its last frame).
    pub fn position(&self, time: f64) -> f64 {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        if (position - position.round()).abs() <= FRAME_RATE as f64 / SAMPLE_RATE as f64 {
            position.round()
        } else {
            position
        }
    }

    /// Part-4 frame at a clip time, or `None` before the slab. Times past
    /// the end hold the last frame.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = self.position(time);
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Clip time of part-4 frame `j`.
    pub fn seconds(&self, j: usize) -> f64 {
        (self.start + j) as f64 / FRAME_RATE as f64
    }

    /// The frame `j`, the fraction `f` of the way to the next, at a clip
    /// time.
    fn moment(&self, time: f64) -> (usize, f64) {
        let last = self.frames.len() - 1;
        let position = self.position(time).clamp(0.0, last as f64);
        let j = position.floor() as usize;
        (j, if j == last { 0.0 } else { position - j as f64 })
    }

    /// The 400 raster start addresses that drew frame `j` (none for part
    /// 7's frame), as the scene's render walks them.
    pub fn rasters(&self, j: usize) -> [u16; RASTERS] {
        let frame = &self.frames[j];
        let s = frame.state;
        raster_starts(
            &self.tables.sift,
            s.add1,
            s.add2,
            s.line_compare,
            frame.phase == Phase::Plasma,
        )
    }

    /// Each layer's displacement in frame `j` (the even rasters', then the
    /// odd ones'), as the modern view takes it: raster r's start address
    /// less the field's start and 200 bytes a raster, smoothed down the
    /// layer's rasters, so it is a smooth function of height, and over the
    /// neighbouring frames of the plasma, so it glides. Zero outside the
    /// plasma's frames.
    pub fn layers(&self, j: usize) -> [[f32; LAYER_SAMPLES]; 2] {
        self.glided(j, |k| self.layers_down(k))
    }

    /// Every frame's [`PlasmaPreview::layers`], in order, as the renderer
    /// uploads them: the same values, with each plasma frame smoothed down
    /// the screen once rather than once for every neighbour that glides
    /// over it.
    pub fn all_layers(&self) -> Vec<[[f32; LAYER_SAMPLES]; 2]> {
        let down: Vec<Option<[[f64; LAYER_SAMPLES]; 2]>> = (0..self.frames.len())
            .map(|k| (self.frames[k].phase == Phase::Plasma).then(|| self.layers_down(k)))
            .collect();
        (0..self.frames.len())
            .map(|j| self.glided(j, |k| down[k].expect("a plasma frame")))
            .collect()
    }

    /// Frame `j`'s layers, smoothed over the neighbouring plasma frames,
    /// whose smoothing down the screen `down` gives.
    fn glided(
        &self,
        j: usize,
        down: impl Fn(usize) -> [[f64; LAYER_SAMPLES]; 2],
    ) -> [[f32; LAYER_SAMPLES]; 2] {
        let mut out = [[0.0; LAYER_SAMPLES]; 2];
        if self.frames[j].phase != Phase::Plasma {
            return out;
        }
        let reach = (3.0 * LAYER_GLIDE).ceil() as isize;
        let mut weight = 0.0;
        for d in -reach..=reach {
            let k = j as isize + d;
            if k < 0
                || k as usize >= self.frames.len()
                || self.frames[k as usize].phase != Phase::Plasma
            {
                continue;
            }
            let w = (-(d * d) as f64 / (2.0 * LAYER_GLIDE * LAYER_GLIDE)).exp();
            let layers = down(k as usize);
            for (o, l) in out.iter_mut().zip(layers) {
                for (v, x) in o.iter_mut().zip(l) {
                    *v += (w * x) as f32;
                }
            }
            weight += w;
        }
        for v in out.iter_mut().flatten() {
            *v /= weight as f32;
        }
        out
    }

    /// Frame `j`'s layers' displacements smoothed down the screen only.
    fn layers_down(&self, j: usize) -> [[f64; LAYER_SAMPLES]; 2] {
        let starts = self.rasters(j);
        let reach = (3.0 * LAYER_SMOOTH).ceil() as isize;
        // The Gaussian's weights, worked out once (the same for every tap).
        let kernel: Vec<f64> = (-reach..=reach)
            .map(|d| (-(d * d) as f64 / (2.0 * LAYER_SMOOTH * LAYER_SMOOTH)).exp())
            .collect();
        let mut out = [[0.0; LAYER_SAMPLES]; 2];
        for (layer, smooth) in out.iter_mut().enumerate() {
            let offset = |k: usize| {
                let r = 2 * k + layer;
                starts[r] as f64 - FIELD_START as f64 - (FIELD_ROW * r) as f64
            };
            for (k, value) in smooth.iter_mut().enumerate() {
                let (mut sum, mut weight) = (0.0, 0.0);
                for d in -reach..=reach {
                    let i = k as isize + d;
                    if (0..LAYER_SAMPLES as isize).contains(&i) {
                        let w = kernel[(d + reach) as usize];
                        sum += w * offset(i as usize);
                        weight += w;
                    }
                }
                *value = sum / weight;
            }
        }
        out
    }

    /// The byte video memory holds at `address` in `plane` while frame `j`
    /// shows: the logo below 0x3840, the index field from there once the
    /// plasma has begun, zero elsewhere (the scene cleared the planes).
    fn memory(&self, j: usize, plane: usize, address: usize) -> u8 {
        if address < FIELD_START {
            self.tables.logo[address * 4 + plane]
        } else if self.frames[j].phase >= Phase::Plasma {
            self.tables.field[address]
        } else {
            0
        }
    }

    /// The indices of frame `j` as the scanout shows them: 320 x 400, or
    /// part 7's 320 x 200.
    pub fn indices(&self, j: usize) -> Vec<u8> {
        if self.frames[j].phase == Phase::Part7 {
            return self.part7.0.clone();
        }
        let starts = self.rasters(j);
        let mut indices = vec![0; 320 * RASTERS];
        for (y, &start) in starts.iter().enumerate() {
            for x in 0..320 {
                let address = (start as usize + (x >> 2)) & 65535;
                indices[y * 320 + x] = self.memory(j, x & 3, address);
            }
        }
        indices
    }

    /// The palette frame `j` shows.
    pub fn palette(&self, j: usize) -> &[u8] {
        &self.palettes[j * 768..][..768]
    }

    /// The original frame `j` as 8-bit RGBA at its own raster (320 x 400,
    /// or 320 x 200 for part 7's frame).
    pub fn original_full(&self, j: usize) -> Vec<u8> {
        let j = j.min(self.frames.len() - 1);
        super::field::rgba(&self.indices(j), self.palette(j))
    }

    /// The original frame `j` as 8-bit RGBA, 320 x 200, as the shared
    /// comparison shows it: part 4's 400 lines with each pair averaged.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let full = self.original_full(j);
        let rows = full.len() / (320 * 4);
        if rows == 200 {
            return full;
        }
        (0..200)
            .flat_map(|y| {
                (0..320 * 4)
                    .map(|k| {
                        let (p, q) = (full[2 * y * 1280 + k], full[(2 * y + 1) * 1280 + k]);
                        ((p as u16 + q as u16).div_ceil(2)) as u8
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Title label for part-4 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        let frame = &self.frames[j.min(self.frames.len() - 1)];
        let e = &self.events;
        let t = j as f64 / FRAME_RATE as f64;
        match frame.phase {
            Phase::Slab => "Part 4: logo slab from white",
            Phase::Plasma => {
                if frame.level <= 0.0 {
                    if e.fade_out >= 0.0 && t >= e.fade_out {
                        "Logo slab, the plasma gone"
                    } else {
                        "Logo slab"
                    }
                } else if frame.level >= 63.0 {
                    "Plasma"
                } else if e.full >= 0.0 && t >= e.full {
                    "Plasma fading out"
                } else {
                    "Plasma fading in"
                }
            }
            Phase::Slide => "Logo slab sliding away",
            Phase::Part7 => "Part 7: first frame",
        }
    }

    /// Clip times of the orders' first rows during part 4.
    pub fn order_starts(&self) -> &[f64] {
        &self.order_starts
    }

    /// The water's mirror gain, the swell's height, speed and size, and the
    /// drops' rate, as tuned ([`WaterTuning`]'s defaults).
    pub fn water_mirror(&self) -> f64 {
        (WATER_MIRROR * TUNING_STEP.powi(self.water.mirror))
            .clamp(WATER_MIRROR_RANGE.0, WATER_MIRROR_RANGE.1)
    }
    pub fn swell_height(&self) -> f64 {
        if self.water.height < SWELL_HEIGHT_STEPS.0 {
            0.0
        } else {
            TUNING_STEP.powi(self.water.height)
        }
    }
    pub fn swell_speed(&self) -> f64 {
        TUNING_STEP.powi(self.water.speed)
    }
    pub fn swell_size(&self) -> f64 {
        TUNING_STEP.powi(self.water.size)
    }
    pub fn drop_rate(&self) -> f64 {
        if self.water.drops < DROP_STEPS.0 {
            0.0
        } else {
            TUNING_STEP.powi(self.water.drops)
        }
    }

    /// The swell's own clock at part-4 `seconds`: the part's seconds at the
    /// default speed, running faster or slower from where it was when the
    /// speed last changed.
    pub fn swell_time(&self, seconds: f64) -> f64 {
        let (at, clock) = self.water.anchor;
        clock + self.swell_speed() * (seconds - at)
    }

    /// Step a knob down or up at clip time `time`, and say where it is.
    pub fn tune_water(&mut self, knob: WaterKnob, up: bool, time: f64) -> String {
        let step = if up { 1 } else { -1 };
        let w = &mut self.water;
        match knob {
            WaterKnob::Mirror => {
                let lowest = (WATER_MIRROR_RANGE.0 / WATER_MIRROR).ln() / TUNING_STEP.ln();
                let highest = (WATER_MIRROR_RANGE.1 / WATER_MIRROR).ln() / TUNING_STEP.ln();
                w.mirror = (w.mirror + step).clamp(lowest.floor() as i32, highest.ceil() as i32);
                format!("Part 4 water: mirror x{:.2}", self.water_mirror())
            }
            WaterKnob::Height => {
                w.height = (w.height + step).clamp(SWELL_HEIGHT_STEPS.0 - 1, SWELL_HEIGHT_STEPS.1);
                format!("Part 4 water: swell height x{:.2}", self.swell_height())
            }
            WaterKnob::Speed => {
                let seconds = time - self.seconds(0);
                let clock = self.swell_time(seconds);
                let w = &mut self.water;
                w.anchor = (seconds, clock);
                w.speed = (w.speed + step).clamp(WATER_STEPS.0, WATER_STEPS.1);
                format!("Part 4 water: swell speed x{:.2}", self.swell_speed())
            }
            WaterKnob::Size => {
                w.size = (w.size + step).clamp(WATER_STEPS.0, WATER_STEPS.1);
                format!("Part 4 water: swell size x{:.2}", self.swell_size())
            }
            WaterKnob::Drops => {
                w.drops = (w.drops + step).clamp(DROP_STEPS.0 - 1, DROP_STEPS.1);
                format!("Part 4 water: {}", self.drops_label())
            }
        }
    }

    /// The drops' rate in words: their mean interval, or none.
    pub fn drops_label(&self) -> String {
        let rate = self.drop_rate();
        if rate <= 0.0 {
            "no drops".to_owned()
        } else {
            let mean = (DROP_INTERVALS.0 + DROP_INTERVALS.1) / 2.0 / rate;
            format!("a drop every {mean:.1} s")
        }
    }

    /// The water's settings for the window's title.
    pub fn water_title(&self) -> String {
        format!(
            "water: mirror ×{:.2}, swell ×{:.2} high ×{:.2} fast ×{:.2} long, {}",
            self.water_mirror(),
            self.swell_height(),
            self.swell_speed(),
            self.swell_size(),
            self.drops_label()
        )
    }

    /// The drops on the water at part-4 `seconds` (authored, fixed): drop
    /// k falls at a fixed time, the intervals between them each between
    /// DROP_INTERVALS (from a hash of k) over the rate, the first
    /// DROP_FIRST after the plasma begins to light and none so late that
    /// it would outlast the plasma's light; each falls at a fixed place
    /// (from a hash of k), in view beside the slab. Up to MAX_DROPS at
    /// once: their places, ages and 1.
    pub fn drops(&self, seconds: f64) -> [[f32; 4]; MAX_DROPS] {
        let mut out = [[0.0; 4]; MAX_DROPS];
        let rate = self.drop_rate();
        let e = &self.events;
        if rate <= 0.0 || e.fade_in < 0.0 || e.dark < 0.0 {
            return out;
        }
        let mut at = e.fade_in + DROP_FIRST;
        let mut n = 0;
        for k in 0_u64.. {
            if at > seconds || at + DROP_LIFE > e.dark || n == MAX_DROPS {
                break;
            }
            let age = seconds - at;
            if age < DROP_LIFE
                && let Some([x, back]) = self.drop_place(k)
            {
                out[n] = [x as f32, back as f32, age as f32, 1.0];
                n += 1;
            }
            let (lo, hi) = DROP_INTERVALS;
            at += (lo + (hi - lo) * unit_hash(2 * k)) / rate;
        }
        out
    }

    /// Where drop k falls on the water (across, back), from a hash of k: a
    /// point of the screen beside the slab, left or right, taken down to
    /// the water; tried again where the slab, with a margin, would hide it.
    fn drop_place(&self, k: u64) -> Option<[f64; 2]> {
        (0..16).find_map(|attempt| {
            let h = |i: u64| unit_hash(((2 * k + 1) << 8) + 4 * attempt + i);
            let side = if h(0) < 0.5 { -1.0 } else { 1.0 };
            let across = side * (DROP_ACROSS.0 + (DROP_ACROSS.1 - DROP_ACROSS.0) * h(1));
            let row = DROP_ROWS.0 + (DROP_ROWS.1 - DROP_ROWS.0) * h(2);
            self.stone.ground_point(160.0 + across * 1.2, row, DROP_CLEAR)
        })
    }

    /// Everything part 4 puts in `u.plasma` at a clip time, but the music's
    /// cue strengths, which the renderer adds.
    pub fn controls(&self, time: f64) -> PlasmaBlock {
        let (j, f) = self.moment(time);
        let frame = &self.frames[j];
        let last = self.frames.len() - 1;
        let k = (j + 1).min(last);
        let next = &self.frames[k];
        // Blend only within a phase (the slab's fade and the slide's line
        // compare run on from frame to frame; part 7's frame does not).
        let blend = next.phase == frame.phase && k != j;
        let g = if blend { f } else { 0.0 };
        let lerp = |a: f64, b: f64| a + (b - a) * g;
        let seconds = time - self.seconds(0);
        let e = &self.events;
        let line_compare = lerp(
            frame.state.line_compare as f64,
            next.state.line_compare as f64,
        );
        let fade = match frame.phase {
            Phase::Slab => lerp(
                frame.state.fade_frame as f64,
                next.state.fade_frame as f64 + if next.phase == Phase::Slab { 0.0 } else { 1.0 },
            ),
            _ => 64.0,
        };
        let mut hits = [-100.0_f32; 4];
        let now = self.order_starts.partition_point(|&t| t <= time);
        for (h, &t) in self.order_starts[..now].iter().rev().take(4).enumerate() {
            hits[h] = (time - t) as f32;
        }
        let stone = &self.stone;
        let slide_rows = (line_compare - REST_LINE_COMPARE as f64) / 2.0;
        let pose = stone.pose(slide_rows);
        let handover = (1.0 - seconds.max(0.0) / HANDOVER).clamp(0.0, 1.0);
        let [front, rear] = stone.sheets;
        // The sheets, the floor (the slab's base) and the slab slide as one,
        // as if the camera rose (authored): the slab exactly as before, the
        // floor and the sheets, dark by then, with it.
        let slid = |p: [f64; 3]| -> [f32; 3] {
            std::array::from_fn(|k| (p[k] + pose[k] - stone.corner[k]) as f32)
        };
        let (front_base, rear_base) = (slid(front.base), slid(rear.base));
        PlasmaBlock {
            part: [
                1.0,
                seconds as f32,
                frame.phase as u8 as f32,
                (j as f64 + f) as f32,
            ],
            frame: [j as f32, k as f32, g as f32, if blend { 1.0 } else { 0.0 }],
            palette: [
                lerp(frame.rotation, next.rotation) as f32,
                lerp(frame.level, next.level) as f32,
                self.offsets[0] as f32,
                self.offsets[1] as f32,
            ],
            logo: [
                line_compare as f32,
                slide_rows as f32,
                (fade / 64.0) as f32,
                if frame.phase == Phase::Part7 {
                    1.0
                } else {
                    0.0
                },
            ],
            events: [e.plasma, e.fade_in, e.full, e.fade_out].map(|v| v as f32),
            events_late: [e.dark, e.slide, e.part7, 0.0].map(|v| v as f32),
            camera: [
                stone.focal as f32,
                stone.axis as f32,
                (handover * handover * (3.0 - 2.0 * handover)) as f32,
                stone.exposure(fade / 64.0) as f32,
            ],
            slab_origin: [
                pose[0] as f32,
                pose[1] as f32,
                pose[2] as f32,
                super::plasma_stone::ROUNDING as f32,
            ],
            slab_axes: [
                stone.tilt.sin() as f32,
                stone.tilt.cos() as f32,
                stone.size[0] as f32,
                stone.size[1] as f32,
            ],
            slab_size: [
                stone.size[2] as f32,
                super::plasma_stone::CARVE_DEPTH as f32,
                stone.colour[0],
                stone.colour[1],
            ],
            stone: [
                stone.colour[2],
                if self.polished_floor { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ],
            sheet: [
                front_base[0],
                front_base[1],
                front_base[2],
                front.height as f32,
            ],
            sheet_size: [
                front.half_width as f32,
                front.up[0] as f32,
                front.up[1] as f32,
                front.up[2] as f32,
            ],
            hits,
            hit_strength: [0.0; 4],
            motion: [
                if blend {
                    (self.glide[k][0] - self.glide[j][0]) as f32
                } else {
                    0.0
                },
                lerp(self.glide[j][0], self.glide[k][0]) as f32,
                lerp(self.glide[j][1], self.glide[k][1]) as f32,
                0.0,
            ],
            sheet_back: [
                rear_base[0],
                rear_base[1],
                rear_base[2],
                rear.height as f32,
            ],
            sheet_back_size: [
                rear.half_width as f32,
                rear.up[0] as f32,
                rear.up[1] as f32,
                rear.up[2] as f32,
            ],
            water: [
                self.water_mirror() as f32,
                self.swell_height() as f32,
                self.swell_time(seconds) as f32,
                self.swell_size() as f32,
            ],
            drops: self.drops(seconds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};
    use crate::scenes::plasma::HdPlasmaTables;

    /// An invented part 4: the invented slab of plasma_stone.rs (about the
    /// size and pose of the one fitted to the real logo) and events a
    /// little longer than the real part's, in part-4 seconds. Observed when
    /// the real preview loads, the plasma begins to light 7.56 s into the
    /// part and is dark again 35.36 s later; here it is dark 36 s later, so
    /// the drops' schedule, fixed from the first light, covers the real
    /// one's. The part starts at about the real one's clip time (179.96 s,
    /// also observed).
    fn invented() -> PlasmaPreview {
        let events = PlasmaEvents {
            plasma: 1.0,
            fade_in: 7.5,
            full: 11.0,
            fade_out: 40.0,
            dark: 43.5,
            slide: 44.5,
            part7: 45.5,
        };
        let last = (events.part7 * FRAME_RATE as f64).round() as usize;
        let phase = |j: usize| {
            let t = j as f64 / FRAME_RATE as f64;
            if j == last {
                Phase::Part7
            } else if t >= events.slide {
                Phase::Slide
            } else if t >= events.plasma {
                Phase::Plasma
            } else {
                Phase::Slab
            }
        };
        let frames: Vec<PlasmaFrame> = (0..=last)
            .map(|j| PlasmaFrame {
                phase: phase(j),
                state: HdPlasma {
                    line_compare: REST_LINE_COMPARE,
                    ..HdPlasma::default()
                },
                rotation: 0.0,
                level: 0.0,
            })
            .collect();
        let n = frames.len();
        PlasmaPreview {
            start: 12597,
            frames,
            palettes: vec![0; 768 * n],
            tables: HdPlasmaTables {
                sine: Vec::new(),
                sift: Vec::new(),
                bank: Vec::new(),
                field: Vec::new(),
                logo: Vec::new(),
                palette: Vec::new(),
            },
            offsets: [0, 0],
            music: Vec::new(),
            part7: (Vec::new(), Vec::new()),
            events,
            stone: super::super::plasma_stone::tests::invented_stone(),
            glide: vec![[0.0; 2]; n],
            order_starts: Vec::new(),
            polished_floor: false,
            water: WaterTuning::default(),
        }
    }

    /// Press a water key `times` times at part-4 `seconds`.
    fn press(p: &mut PlasmaPreview, knob: WaterKnob, up: bool, times: usize, seconds: f64) {
        for _ in 0..times {
            p.tune_water(knob, up, p.seconds(0) + seconds);
        }
    }

    /// One drop as the water showed it: where it fell, when (part-4
    /// seconds), and its age when first and last seen.
    struct Seen {
        place: [f32; 2],
        at: f64,
        first: f64,
        last: f64,
    }

    /// How finely [`drops_seen`] samples the part (seconds).
    const SAMPLE: f64 = 1.0 / 280.0;

    /// Every drop the water shows over the whole part, sampled every
    /// SAMPLE seconds, in the order they fall, and the most shown at once.
    /// Each sample's slots hold the drops first, each its place, its age
    /// and 1, and the rest are empty.
    fn drops_seen(p: &PlasmaPreview) -> (Vec<Seen>, usize) {
        let mut seen: Vec<Seen> = Vec::new();
        let mut most = 0;
        let samples = ((p.events.part7 + 1.0) / SAMPLE) as usize;
        for i in 0..=samples {
            let t = i as f64 * SAMPLE;
            let drops = p.drops(t);
            let on = drops.iter().take_while(|d| d[3] == 1.0).count();
            assert!(
                drops[on..].iter().all(|d| *d == [0.0; 4]),
                "at {t}: {drops:?}"
            );
            most = most.max(on);
            for d in &drops[..on] {
                let age = d[2] as f64;
                assert!((0.0..DROP_LIFE).contains(&age), "age {age} at {t}");
                let at = t - age;
                match seen.iter_mut().find(|s| s.place == [d[0], d[1]]) {
                    Some(s) => {
                        assert!((s.at - at).abs() < 1e-4, "a drop's time moved at {t}");
                        s.last = age;
                    }
                    None => seen.push(Seen {
                        place: [d[0], d[1]],
                        at,
                        first: age,
                        last: age,
                    }),
                }
            }
        }
        seen.sort_by(|a, b| a.at.total_cmp(&b.at));
        (seen, most)
    }

    /// Run `check` at every drops' rate the keys allow, from the lowest to
    /// the highest, set with the keys; then put the tuning back.
    fn at_every_rate(p: &mut PlasmaPreview, mut check: impl FnMut(&PlasmaPreview)) {
        let tuning = p.water;
        press(p, WaterKnob::Drops, false, 40, 0.0);
        press(p, WaterKnob::Drops, true, 1, 0.0);
        loop {
            let rate = p.drop_rate();
            check(p);
            press(p, WaterKnob::Drops, true, 1, 0.0);
            if p.drop_rate() == rate {
                break;
            }
        }
        p.water = tuning;
    }

    /// Every drop falls once the plasma has been lighting for DROP_FIRST,
    /// the first just then, and is gone (its time plus DROP_LIFE) by the
    /// time the plasma is dark again; there are none before or after.
    fn check_drops_fall_while_lit(p: &PlasmaPreview) {
        let e = p.events;
        let (seen, _) = drops_seen(p);
        let rate = p.drop_rate();
        assert!(!seen.is_empty(), "no drops at x{rate}");
        assert!(
            (seen[0].at - (e.fade_in + DROP_FIRST)).abs() < 1e-4,
            "the first falls at {} at x{rate}",
            seen[0].at
        );
        for s in &seen {
            assert!(s.at >= e.fade_in + DROP_FIRST - 1e-4, "{} at x{rate}", s.at);
            assert!(s.at + DROP_LIFE <= e.dark + 1e-4, "{} at x{rate}", s.at);
        }
        for t in [
            0.0,
            e.fade_in,
            e.fade_in + DROP_FIRST - 0.01,
            e.dark,
            e.part7,
        ] {
            assert_eq!(p.drops(t), [[0.0; 4]; MAX_DROPS], "drops at {t} at x{rate}");
        }
    }

    /// Each drop lands in view, below the plasma's bottom edge (row 113)
    /// and within a 16:9 frame, beside the slab and never behind it: its
    /// place on the water, seen from the camera, lies outside the slab's
    /// silhouette grown by DROP_CLEAR all round (the block ground_point
    /// keeps the drops' rays clear of), and the ray through it meets the
    /// water there. Gives the number of drops checked.
    fn check_drops_in_view(p: &PlasmaPreview) -> usize {
        let stone = &p.stone;
        let (back, down) = stone.axes();
        let floor = stone.floor();
        // The silhouette of the block grown by DROP_CLEAR: its corners'
        // projections, which bound it (it is convex), in screen units
        // (columns are 5/6 of a row).
        let c = DROP_CLEAR;
        let mut corners = Vec::new();
        for x in [-stone.size[0] / 2.0 - c, stone.size[0] / 2.0 + c] {
            for v in [-c, stone.size[1] + c] {
                for w in [-c, stone.size[2] + c] {
                    let q: [f64; 3] =
                        std::array::from_fn(|k| stone.corner[k] + v * back[k] + w * down[k]);
                    let (column, row) = stone.project([q[0] + x, q[1], q[2]]);
                    corners.push((column * 5.0 / 6.0, row));
                }
            }
        }
        let inside_hull = |(x, y): (f64, f64)| {
            // Inside unless an edge of the corners' hull (a line with
            // every corner on one side) has the point on its other side.
            let n = corners.len();
            !(0..n).any(|a| {
                (0..n).any(|b| {
                    let (p, q) = (corners[a], corners[b]);
                    let side =
                        |r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
                    a != b && corners.iter().all(|&r| side(r) >= -1e-9) && side((x, y)) < 0.0
                })
            })
        };
        let half_width = 160.0 * (16.0 / 9.0) / (4.0 / 3.0);
        let seen = drops_seen(p).0;
        for s in &seen {
            let [x, b] = s.place.map(|v| v as f64);
            let point: [f64; 3] =
                std::array::from_fn(|k| floor[k] + b * back[k] + if k == 0 { x } else { 0.0 });
            let (column, row) = stone.project(point);
            assert!(
                (113.0..200.0).contains(&row) && (column - 160.0).abs() < half_width,
                "a drop out of view at column {column:.1}, row {row:.1}"
            );
            assert!(
                !inside_hull((column * 5.0 / 6.0, row)),
                "a drop behind the slab at column {column:.1}, row {row:.1}"
            );
            let found = stone.ground_point(column, row, 0.0).unwrap();
            assert!((found[0] - x).abs() < 1e-3 && (found[1] - b).abs() < 1e-3);
        }
        seen.len()
    }

    /// The shader's MAX_DROPS show every drop for its whole life over the
    /// whole part: none is shown late or cut short, and none is left out
    /// (each falls an interval of the schedule after the one before, and
    /// the next after the last would outlast the plasma's light); so the
    /// water has as many at once as the schedule has falling, never more
    /// than MAX_DROPS.
    fn check_drops_shown_whole(p: &PlasmaPreview) {
        let rate = p.drop_rate();
        let (seen, most) = drops_seen(p);
        let (lo, hi) = (DROP_INTERVALS.0 / rate, DROP_INTERVALS.1 / rate);
        for s in &seen {
            assert!(
                s.first < SAMPLE + 1e-4,
                "shown {} s late at x{rate}",
                s.first
            );
            assert!(
                s.last > DROP_LIFE - SAMPLE - 1e-4,
                "cut short at {} s at x{rate}",
                s.last
            );
        }
        for w in seen.windows(2) {
            let gap = w[1].at - w[0].at;
            assert!(
                gap > lo - 1e-4 && gap < hi + 1e-4,
                "{gap} s between drops at {} at x{rate}",
                w[0].at
            );
        }
        let latest = seen.last().unwrap().at + hi;
        assert!(
            latest + DROP_LIFE > p.events.dark,
            "one left out at the end at x{rate}"
        );
        let alive = |t: f64| {
            seen.iter()
                .filter(|s| s.at <= t && t < s.at + DROP_LIFE)
                .count()
        };
        let scheduled = seen.iter().map(|s| alive(s.at)).max().unwrap();
        assert!(
            most <= MAX_DROPS && scheduled == most,
            "{scheduled} {most} at x{rate}"
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part4_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let started = std::time::Instant::now();
        let clip = Clip::load_plasma(&data, ScrollerLighting::Studio).unwrap();
        let loaded = started.elapsed().as_secs_f64();
        let dots = clip.dots.as_ref().unwrap();
        let plasma = clip.plasma.as_ref().unwrap();
        assert_eq!(plasma.start, dots.start + dots.frames.len() - 1);
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in 0..plasma.start {
            pcm.extend(engine.step().unwrap());
        }
        let mut lit = 0;
        for (j, frame) in plasma.frames.iter().enumerate() {
            let video = &engine.video;
            let part = if frame.phase == Phase::Part7 { 7 } else { 4 };
            assert_eq!(engine.part(), part, "part at frame {j}");
            assert!(plasma.indices(j) == video.indices, "pixels at frame {j}");
            assert!(plasma.palette(j) == &video.palette[..], "palette at {j}");
            // The palette's rotation and level rebuild the plasma's colours
            // from the bank, as the scene filled them.
            if frame.phase == Phase::Plasma && j > 0 {
                let level = frame.level as usize;
                let base = frame.rotation.rem_euclid(PALETTE_PERIOD as f64) as usize;
                let rotations = [
                    base,
                    (base + plasma.offsets[0]) % PALETTE_PERIOD,
                    (base + plasma.offsets[1]) % PALETTE_PERIOD,
                ];
                for i in 0..PALETTE_PERIOD {
                    for (c, r) in rotations.iter().enumerate() {
                        assert_eq!(
                            video.palette[96 + i * 3 + c],
                            plasma.tables.bank[(level << 10) + r + i],
                            "colour {} channel {c} at frame {j}",
                            32 + i
                        );
                    }
                }
                lit += (level > 0) as usize;
            }
            if j + 1 < plasma.frames.len() {
                pcm.extend(engine.step().unwrap());
            }
        }
        assert!(lit > 1000, "{lit} lit plasma frames");
        assert_eq!(engine.part(), 7);
        assert!(pcm == clip.pcm, "PCM");
        let e = plasma.events;
        println!(
            "Loaded in {loaded:.2}s. Part 4's slab at {:.3}s: plasma {:.3}, fade in {:.3}, full {:.3}, \
             fade out {:.3}, dark {:.3}, slide {:.3}, part 7 {:.3}; offsets {:?}; stone {:?}",
            plasma.seconds(0),
            e.plasma + plasma.seconds(0),
            e.fade_in + plasma.seconds(0),
            e.full + plasma.seconds(0),
            e.fade_out + plasma.seconds(0),
            e.dark + plasma.seconds(0),
            e.slide + plasma.seconds(0),
            e.part7 + plasma.seconds(0),
            plasma.offsets,
            plasma.stone.summary(),
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part4s_sections_start_where_its_labels_change() {
        let data = crate::test_support::data();
        let clip = Clip::load_plasma(&data, ScrollerLighting::Studio).unwrap();
        let sections = clip.sections();
        let frame = 1.0 / FRAME_RATE as f64;
        assert!(sections.windows(2).all(|w| w[0] < w[1]));
        for &s in &sections[1..] {
            assert_ne!(clip.label(s), clip.label(s - frame));
        }
        let plasma = clip.plasma.as_ref().unwrap();
        let start = plasma.seconds(0);
        let part4: Vec<(f64, &str)> = sections
            .iter()
            .filter(|&&s| s >= start - 1e-9)
            .map(|&s| (s, clip.label(s)))
            .collect();
        for (s, label) in &part4 {
            println!("{s:8.3} {label}");
        }
        let labels: Vec<&str> = part4.iter().map(|p| p.1).collect();
        assert_eq!(
            labels,
            [
                "Part 4: logo slab from white",
                "Logo slab",
                "Plasma fading in",
                "Plasma",
                "Plasma fading out",
                "Logo slab, the plasma gone",
                "Logo slab sliding away",
                "Part 7: first frame",
            ]
        );
        // Each starts at its event, as the frames observed it.
        let e = plasma.events;
        for (k, event) in [
            0.0, e.plasma, e.fade_in, e.full, e.fade_out, e.dark, e.slide, e.part7,
        ]
        .iter()
        .enumerate()
        {
            // (The last within a sample: it is taken at the duration.)
            assert!(
                (part4[k].0 - start - event).abs() < 1.0 / SAMPLE_RATE as f64,
                "{}",
                labels[k]
            );
        }
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn the_clip_ends_on_part7s_first_frame() {
        let data = crate::test_support::data();
        let clip = Clip::load_plasma(&data, ScrollerLighting::Studio).unwrap();
        let plasma = clip.plasma.as_ref().unwrap();
        let last = plasma.frames.len() - 1;
        assert_eq!(plasma.frames[last].phase, Phase::Part7);
        assert_eq!(plasma.frame_at(clip.duration()), Some(last));
        assert_eq!(
            plasma.frame_at(plasma.seconds(last) - 0.001),
            Some(last - 1)
        );
        // Part 3's preview hands over on the slab.
        let dots = clip.dots.as_ref().unwrap();
        assert_eq!(
            plasma.frame_at(dots.seconds(dots.frames.len() - 1)),
            Some(0)
        );
    }

    /// Part 4's import limit is the palette texture's height under the
    /// limits the renderer asks the device for (wgpu's defaults), and leaves
    /// the real part (about 45 s, observed) and the invented one room.
    #[test]
    fn part4s_import_limit_fits_its_palette_texture() {
        assert_eq!(
            MAX_FRAMES as u32,
            wgpu::Limits::default().max_texture_dimension_2d
        );
        assert!(MAX_FRAMES > FRAME_RATE as usize * 100);
        assert!(invented().frames.len() < MAX_FRAMES);
    }

    /// The layers the renderer uploads, each plasma frame smoothed down the
    /// screen once, are every frame's own layers to the bit, over the whole
    /// invented part: its edges (before the plasma, the slide) included.
    #[test]
    fn all_layers_are_each_frames_layers_to_the_bit() {
        let mut p = invented();
        // An invented stride table and strides that move from frame to
        // frame, so the displacements vary down the screen and over time.
        p.tables.sift = (0..1024u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        for (j, frame) in p.frames.iter_mut().enumerate() {
            frame.state.add1 = (j * 7) % 512;
            frame.state.add2 = (j * 13 + 5) % 512;
        }
        let all = p.all_layers();
        assert_eq!(all.len(), p.frames.len());
        let mut moving = 0;
        for (j, layers) in all.iter().enumerate() {
            let own = p.layers(j);
            for (a, b) in layers.iter().flatten().zip(own.iter().flatten()) {
                assert_eq!(a.to_bits(), b.to_bits(), "frame {j}");
            }
            moving += own.iter().flatten().any(|&v| v != 0.0) as usize;
        }
        assert!(moving > 1000, "{moving} frames with displaced layers");
    }

    /// The drops are a function of the moment and the rate alone: the same
    /// moment shows the same drops whether it is reached by seeking
    /// straight to it, by playing through every frame before it, by seeking
    /// back to it, in another preview, or after the rate and the swell have
    /// been changed and the rate put back.
    #[test]
    fn the_drops_are_the_same_however_their_moment_is_reached() {
        let fresh = invented();
        let e = fresh.events;
        let moments = [
            e.fade_in + DROP_FIRST + 0.01,
            e.fade_in + 9.3,
            e.fade_in + 20.0,
            e.dark - DROP_LIFE - 0.5,
        ];
        let direct: Vec<_> = moments.iter().map(|&m| fresh.drops(m)).collect();
        assert!(direct.iter().all(|d| d[0][3] == 1.0), "{direct:?}");
        let clip = |m: f64| fresh.seconds(0) + m;
        // Played through every frame (at 60 fps) up to each moment.
        let played = invented();
        let mut next = 0;
        for k in 0.. {
            let t = k as f64 / 60.0;
            if next == moments.len() {
                break;
            }
            if t >= moments[next] {
                assert_eq!(played.controls(clip(moments[next])).drops, direct[next]);
                next += 1;
            }
            played.controls(clip(t));
        }
        // Sought back to, last first.
        for (m, d) in moments.iter().zip(&direct).rev() {
            assert_eq!(played.controls(clip(*m)).drops, *d);
        }
        // After the keys: the rate down three steps and back, the swell
        // faster.
        let mut tuned = invented();
        press(&mut tuned, WaterKnob::Drops, false, 3, 12.0);
        assert_ne!(tuned.drops(moments[2]), direct[2]);
        press(&mut tuned, WaterKnob::Drops, true, 3, 31.0);
        press(&mut tuned, WaterKnob::Speed, true, 3, 17.0);
        for (m, d) in moments.iter().zip(&direct) {
            assert_eq!(tuned.drops(*m), *d);
        }
    }

    /// At every rate the keys allow, every drop falls once the plasma has
    /// been lighting for DROP_FIRST, the first just then, and is gone (its
    /// time plus DROP_LIFE) by the time the plasma is dark again; there are
    /// none before or after.
    #[test]
    fn drops_fall_while_the_plasma_is_lit_and_are_gone_by_dark() {
        at_every_rate(&mut invented(), check_drops_fall_while_lit);
    }

    /// At every rate the keys allow, each drop lands in view, below the
    /// plasma and within a 16:9 frame, beside the slab and never behind it.
    #[test]
    fn drops_land_in_view_beside_the_slab() {
        let mut count = 0;
        at_every_rate(&mut invented(), |p| count += check_drops_in_view(p));
        assert!(count > 100, "{count} drops");
    }

    /// At the highest rate the keys allow, a drop every 0.9 s on average,
    /// the shader's MAX_DROPS still show every drop for its whole life over
    /// the whole part, none shown late, cut short or left out, so the water
    /// never has more than MAX_DROPS at once; as at every lower rate.
    #[test]
    fn at_the_highest_rate_no_drop_is_cut_short_or_left_out() {
        let mut p = invented();
        press(&mut p, WaterKnob::Drops, true, 40, 0.0);
        assert_eq!(p.drops_label(), "a drop every 0.9 s");
        check_drops_shown_whole(&p);
        at_every_rate(&mut p, check_drops_shown_whole);
    }

    /// The drops on the real part, its slab as fitted to the user's logo
    /// and its events as observed: at every rate the keys allow they fall
    /// while the plasma is lit, land in view beside the slab and show whole.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part4s_drops_fall_beside_the_slab_and_show_whole_at_every_rate() {
        let data = crate::test_support::data();
        let mut clip = Clip::load_plasma(&data, ScrollerLighting::Studio).unwrap();
        let plasma = clip.plasma.as_mut().unwrap();
        let mut count = 0;
        at_every_rate(plasma, |p| {
            check_drops_fall_while_lit(p);
            count += check_drops_in_view(p);
            check_drops_shown_whole(p);
        });
        assert!(count > 100, "{count} drops");
    }

    /// The rate one step below the lowest the keys name is none: no drops
    /// fall over the whole part; a step up, they fall again.
    #[test]
    fn the_drops_lowest_rate_is_none() {
        let mut p = invented();
        press(&mut p, WaterKnob::Drops, false, 40, 0.0);
        assert_eq!(p.drop_rate(), 0.0);
        assert_eq!(p.drops_label(), "no drops");
        assert!(drops_seen(&p).0.is_empty());
        press(&mut p, WaterKnob::Drops, true, 1, 0.0);
        assert!(p.drop_rate() > 0.0);
        assert!(!drops_seen(&p).0.is_empty());
    }

    /// By default (chosen by eye with the keys) the mirror's gain is 3, the
    /// swell is 6 steps of x1.25 higher and faster than authored, about
    /// x3.8, at its authored size, and a drop falls every 1.2 s on average,
    /// as the falls over the part show.
    #[test]
    fn the_water_starts_at_its_chosen_tuning() {
        let p = invented();
        let six = 1.25_f64.powi(6);
        assert_eq!(p.water_mirror(), 3.0);
        assert!((p.swell_height() - six).abs() < 1e-12);
        assert!((p.swell_speed() - six).abs() < 1e-12);
        assert_eq!(p.swell_size(), 1.0);
        assert!((p.swell_time(10.0) - 10.0 * six).abs() < 1e-9);
        assert_eq!(p.drops_label(), "a drop every 1.2 s");
        let (seen, _) = drops_seen(&p);
        let mean = (seen.last().unwrap().at - seen[0].at) / (seen.len() - 1) as f64;
        assert!((mean - 1.2).abs() < 0.12, "a drop every {mean} s");
        let block = p.controls(p.seconds(0) + 20.0);
        assert_eq!(block.water[0], 3.0);
        assert_eq!(block.water[3], 1.0);
    }

    /// The water's keys hold each knob within its range at both ends, and a
    /// step back comes straight off an end: the mirror's gain from 1 (water
    /// as it is) to 8; the swell's height down to exactly 0, a flat mirror,
    /// one step below its lowest, and up to x1.25^12, about x15; its speed
    /// and size from x1.25^-6, about x0.26, to x1.25^12; the drops' rate
    /// down to none and up to x1.25^7, a drop every 0.9 s.
    #[test]
    fn the_water_keys_hold_at_both_ends() {
        type Value = fn(&PlasmaPreview) -> f64;
        let knobs: [(WaterKnob, Value, f64, f64); 5] = [
            (WaterKnob::Mirror, PlasmaPreview::water_mirror, 1.0, 8.0),
            (
                WaterKnob::Height,
                PlasmaPreview::swell_height,
                0.0,
                1.25_f64.powi(12),
            ),
            (
                WaterKnob::Speed,
                PlasmaPreview::swell_speed,
                1.25_f64.powi(-6),
                1.25_f64.powi(12),
            ),
            (
                WaterKnob::Size,
                PlasmaPreview::swell_size,
                1.25_f64.powi(-6),
                1.25_f64.powi(12),
            ),
            (
                WaterKnob::Drops,
                PlasmaPreview::drop_rate,
                0.0,
                1.25_f64.powi(7),
            ),
        ];
        for (knob, value, lowest, highest) in knobs {
            for (up, end) in [(false, lowest), (true, highest)] {
                let mut p = invented();
                press(&mut p, knob, up, 40, 5.0);
                assert!((value(&p) - end).abs() < 1e-12, "{knob:?}: {}", value(&p));
                press(&mut p, knob, up, 1, 5.0);
                assert!((value(&p) - end).abs() < 1e-12, "{knob:?} past its end");
                press(&mut p, knob, !up, 1, 5.0);
                assert!((value(&p) - end).abs() > 1e-3, "{knob:?} stuck at its end");
            }
        }
        // The swell's height reaches 0 one step below its lowest.
        let mut p = invented();
        press(&mut p, WaterKnob::Height, false, 40, 5.0);
        press(&mut p, WaterKnob::Height, true, 1, 5.0);
        assert!((p.swell_height() - 1.25_f64.powi(SWELL_HEIGHT_STEPS.0)).abs() < 1e-12);
        press(&mut p, WaterKnob::Height, false, 1, 5.0);
        assert_eq!(p.swell_height(), 0.0);
        assert_eq!(p.controls(p.seconds(0) + 5.0).water[1], 0.0);
        let mut p = invented();
        press(&mut p, WaterKnob::Drops, true, 40, 5.0);
        assert_eq!(p.drops_label(), "a drop every 0.9 s");
    }

    /// A change of the swell's speed takes effect from the moment of the
    /// change: its clock reads the same just after as just before, at any
    /// moment of the part and however often it is changed, then runs on at
    /// the new speed; so the waves never jump. The other keys leave the
    /// clock alone.
    #[test]
    fn changing_the_swell_speed_does_not_move_its_clock() {
        let mut p = invented();
        let changes = [
            (3.0, true, 1),
            (9.25, false, 4),
            (9.25, true, 1),
            (20.0, true, 30),
            (26.5, false, 40),
            (33.0, true, 3),
        ];
        let mut last = p.swell_time(0.0);
        for (seconds, up, times) in changes {
            let before = p.swell_time(seconds);
            let shown = p.controls(p.seconds(0) + seconds).water[2];
            assert!(before >= last, "the clock ran back by {seconds}");
            press(&mut p, WaterKnob::Speed, up, times, seconds);
            let after = p.swell_time(seconds);
            assert!(
                (after - before).abs() < 1e-9,
                "{before} -> {after} at {seconds}"
            );
            let shown_after = p.controls(p.seconds(0) + seconds).water[2];
            assert!(
                (shown_after - shown).abs() <= 1e-6 * shown.abs(),
                "{shown} -> {shown_after}"
            );
            let ran = p.swell_time(seconds + 0.5) - after;
            assert!((ran - 0.5 * p.swell_speed()).abs() < 1e-9);
            last = after;
        }
        let before = p.swell_time(38.0);
        for knob in [
            WaterKnob::Mirror,
            WaterKnob::Height,
            WaterKnob::Size,
            WaterKnob::Drops,
        ] {
            press(&mut p, knob, true, 2, 38.0);
            press(&mut p, knob, false, 3, 38.0);
        }
        assert_eq!(p.swell_time(38.0), before);
    }
}
