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
    /// The stone's blue, unused.
    pub stone: [f32; 4],
    /// The plasma's front sheet of light: its bottom edge's centre in
    /// camera space and its height.
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
    /// The back sheet: its bottom edge's centre and height; its half width,
    /// unused.
    pub sheet_back: [f32; 4],
    pub sheet_back_size: [f32; 4],
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
            if j > FRAME_RATE as usize * 120 {
                return Err("Part 4 exceeded its two-minute import limit".into());
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
            let layers = self.layers_down(k as usize);
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
                        let w = (-(d * d) as f64 / (2.0 * LAYER_SMOOTH * LAYER_SMOOTH)).exp();
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
            stone: [stone.colour[2], 0.0, 0.0, 0.0],
            sheet: [
                front.base[0] as f32,
                front.base[1] as f32,
                front.base[2] as f32,
                front.height as f32,
            ],
            sheet_size: [
                front.half_width as f32,
                stone.sheet_up[0] as f32,
                stone.sheet_up[1] as f32,
                stone.sheet_up[2] as f32,
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
                rear.base[0] as f32,
                rear.base[1] as f32,
                rear.base[2] as f32,
                rear.height as f32,
            ],
            sheet_back_size: [rear.half_width as f32, 0.0, 0.0, 0.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

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
}
