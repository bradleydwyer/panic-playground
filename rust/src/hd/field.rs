//! Part 5 (Field) for the HD preview: the state of every part-5 frame read
//! from the port's `Field` scene, the zoom buffers and original frames
//! replayed on demand, the Mandelbrot fit of DEP.U and its relief, and the
//! three authors' shares of the modern view (the metal frame and its smoke,
//! the zoom, the mountain), each in its own files.
//!
//! What comes from where:
//! - Read from the port's code and counters (`Field::hd_state` and the
//!   buffers): the zoom's scale, speed and swaps, its two field buffers,
//!   the terrain level, the camera, the projection table's `q`, the window
//!   clear and the palettes, per frame.
//! - Decoded from the local data at load time: the frame picture, the
//!   prepared terrain (DEP.U and MAP.U through `prepare`) and the sine
//!   table. Nothing derived from them is stored in Git.
//! - Observed when the preview loads: the times of the zoom's and the
//!   mountain's phases ([`FieldEvents`]) and which frames show terrain.
//! - Interpretation: the Mandelbrot fit of DEP.U ([`MandelFit::fit`]) and
//!   the relief baked from it ([`Relief`]), interpolation between the
//!   original's 70 Hz frames, and the camera's formulas evaluated at
//!   continuous time without their integer truncation.
//! - Authored: everything the modern view adds, in field_frame.wgsl,
//!   field_zoom.wgsl and field_mountain.wgsl and their hooks in
//!   field_frame.rs, field_zoom.rs and field_mountain.rs.
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::field::{Field, HdField},
    vga::Vga,
};
use bytemuck::{Pod, Zeroable};
use half::f16;
use std::sync::Mutex;

#[path = "field_frame.rs"]
mod frame;
#[path = "field_mountain.rs"]
mod mountain;
#[path = "field_zoom.rs"]
mod zoom;
pub use frame::{FLUID, FRAME, FRAME_TAKES_PART6_FADE, FrameFluid, FrameShape, frame_shape};
pub use mountain::{MOUNTAIN, RELIEF_SIZE};
pub use zoom::ZOOM;

/// `u.state.x` during part 5. The intro uses 0..=5, part 2 6..=9 and part 6
/// 10.
pub const PART5_PHASE: f32 = 11.0;
/// The window the zoom draws into and the terrain step clears: 128 x 100
/// pixels at screen (96, 46), read from the code (the zoom writes from
/// 0x39e0 = 46 * 320 + 96; the clear fills rows 46 to 145, columns 96 to
/// 223). field_common.wgsl repeats it.
pub const FIELD_WINDOW: [f32; 4] = [96.0, 46.0, 224.0, 146.0];
/// The palette entry the window is cleared to (read from the code); MAP.PAL
/// makes it near black.
pub const CLEAR_INDEX: u8 = 129;
/// Width and height of the terrain map, in map units, read from `prepare`
/// and `ray` (200 by 200; heights 0 outside it).
pub const MAP_SIZE: usize = 200;
/// Map units per cell of the coarse maximum-height grids that let the
/// marchers skip empty space. field_common.wgsl repeats it.
pub const COARSE_CELL: usize = 8;
/// Coarse cells per side.
pub const COARSE_CELLS: usize = MAP_SIZE / COARSE_CELL;
/// The part of each zoom buffer the zoom reads: 256 columns by 200 rows.
pub const BUFFER_SIZE: [usize; 2] = [256, 200];
/// Frames between replay checkpoints for the original comparison and the
/// zoom buffers.
const REPLAY_INTERVAL: usize = 64;

/// One part-5 frame as the scanout shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldFrame {
    /// The scene's counters after the step that produced this frame.
    pub state: HdField,
    /// Buffer swaps so far: the level the window shows, from 1 on the first
    /// zoom frame.
    pub swaps: u32,
    /// The palette this frame shows, an index into [`FieldPreview::palettes`].
    pub palette: usize,
    /// The original shows terrain pixels: colours 128 to 191 other than the
    /// window's clear colour, 129, as the research counted them (observed).
    pub mountain: bool,
}

/// Times in part 5, in seconds from its first frame, observed from the
/// frames when the preview loads (the thresholds are the port's).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FieldEvents {
    /// The zoom reaches full speed (1024): one level every 64 frames.
    pub full_speed: f64,
    /// The zoom starts to brake.
    pub braking: f64,
    /// The zoom has stopped and the terrain begins (its first frame).
    pub terrain: f64,
    /// The window is cleared to colour 129.
    pub cleared: f64,
    /// The terrain has risen: level 64.
    pub risen: f64,
    /// The terrain starts to sink.
    pub sinking: f64,
    /// The last frame with terrain pixels.
    pub last_terrain: f64,
    /// The end: part 3 takes this frame.
    pub end: f64,
}

/// The part-5 controls shared by every part-5 shader: `u.field`, and with
/// the fluid step's own values its `fluid_step.block`. renderer::CONTROLS_WGSL
/// declares the same layout as `FieldBlock`; keep the two in step (the tests
/// check them). All zero before part 5.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct FieldBlock {
    /// Shown (1), seconds since part 5 began (negative before it), phase (0
    /// zoom, 1 terrain), part-5 frame (continuous).
    pub part: [f32; 4],
    /// The zoom: its scale in the shown buffer's pixels per window pixel
    /// (1 to 2; below 1 while the next level is due), 1 when the next frame
    /// swaps to the next level, the level (log2 of the magnification since
    /// the first zoom frame, continuous), and the shown buffer's swaps.
    pub zoom: [f32; 4],
    /// Zoom speed / 1024, rows of the next level built (0 to 100), running
    /// (1 while it zooms, 0 before its first frame and once it has stopped),
    /// shown (0 on part 5's first frame, which still shows PSTART's window).
    pub zoom_state: [f32; 4],
    /// The camera: map x and y, height (the code's height units) and angle
    /// (1024ths of a turn, unwrapped).
    pub camera: [f32; 4],
    /// The sine and cosine of the angle (the table's amplitude, 256), the
    /// orbit radius (the code's 70 + sin / 12) and the camera's distance from
    /// the map centre (2.56 times that).
    pub view: [f32; 4],
    /// Terrain level (0 to 64 rising, falling as it sinks), the projection's
    /// q (0 flat to 248 perspective), terrain frame (continuous), terrain (1)
    /// or zoom (0).
    pub mountain: [f32; 4],
    /// Window cleared (0 or 1), seconds since it was cleared (-1 before),
    /// PSTART level (1 in part 5; part 6's fade before it), and terrain
    /// pixels shown by the original (0 or 1, interpolated).
    pub window: [f32; 4],
    /// The Mandelbrot fit: c (real, imaginary) at map point (100, 100), map
    /// units per unit of c along map x (the imaginary axis), and the aspect
    /// (map y's units per unit of c, the real axis, over that).
    pub mandel: [f32; 4],
    /// DEP.U = base + slope sqrt(G(c)) on the slopes; the plateau's IoU with
    /// the set; the slopes' rms error in DEP units.
    pub mandel_fit: [f32; 4],
    /// The relief texture: texels per side, map units per texel, the highest
    /// height in it (the plateau's) and the plateau's DEP value.
    pub relief: [f32; 4],
    /// Part-5 seconds of [`FieldEvents`]: full speed, braking, terrain,
    /// cleared.
    pub events: [f32; 4],
    /// Part-5 seconds of [`FieldEvents`]: risen, sinking, last terrain, end.
    pub events_late: [f32; 4],
    /// The fluid: steps done, fraction from the previous state to the latest,
    /// seconds per step, seconds since the fluid's start (negative before).
    pub fluid: [f32; 4],
    /// The source-pixel rectangle the fluid's grid covers (`FLUID.region`):
    /// x0, y0, x1, y1.
    pub fluid_region: [f32; 4],
    /// Spare values from each author's controls hook.
    pub frame_style: [[f32; 4]; 4],
    pub zoom_style: [[f32; 4]; 4],
    pub mountain_style: [[f32; 4]; 4],
}

/// One author's share of the modern view; see rust/README.md, "Adding to
/// part 5". Each author owns one of these in their own Rust file, next to
/// its controls hook.
pub struct AuthorInfo {
    pub name: &'static str,
    /// The author's WGSL file; field_render.rs builds the module from the
    /// shared controls, field_common.wgsl, the three authors' files and
    /// field_style.wgsl.
    pub source: &'static str,
    /// Run `<name>_prepass` first, at half the resolution each way, into a
    /// texture `<name>_prepass_at(uv)` reads.
    pub prepass: bool,
    /// Values for the author's four spare vec4s at a clip time (zero by
    /// default), given the rest of the block.
    pub controls: fn(&FieldBlock, f64) -> [[f32; 4]; 4],
}

/// When the fluid simulation starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FluidStart {
    /// Part 5's first frame, 81.27 s.
    Part5,
    /// Part 6's PSTART fade, 80.36 s, for a frame that part 6's fade
    /// brings in (see [`FRAME_TAKES_PART6_FADE`]).
    PictureFade,
}

/// The fluid hook's settings, owned by the frame author.
pub struct FluidInfo {
    /// Texels of the fluid state, an Rgba16Float storage texture.
    pub size: [u32; 2],
    /// The source-pixel rectangle the fluid covers: x0, y0, x1, y1 (it may
    /// reach beyond the 320 x 200 frame, for wide screens).
    pub region: [f32; 4],
    /// Steps per second, fixed.
    pub rate: f64,
    pub start: FluidStart,
    /// Steps between the states kept on the GPU for seeking back.
    pub checkpoint: usize,
}

/// No spare values.
pub fn no_controls(_: &FieldBlock, _: f64) -> [[f32; 4]; 4] {
    [[0.0; 4]; 4]
}

/// The three authors, in the order the module is built.
pub fn authors() -> [&'static AuthorInfo; 3] {
    [&FRAME, &ZOOM, &MOUNTAIN]
}

/// The replay's position: a frame, the scene and video memory after it,
/// and the zoom buffer that frame shows.
#[derive(Clone)]
struct Cursor {
    at: usize,
    field: Field,
    vga: Vga,
    shown: Vec<u8>,
}

/// The original frames and zoom buffers, replayed on demand rather than
/// stored (two 51 KB buffers and a 64 KB picture a frame, about 570 MB in
/// all): the scene, video memory and shown buffer are kept every
/// [`REPLAY_INTERVAL`] frames (about 45 MB), with the music position each
/// step read, and the port's own `Field::step` runs forward from the nearest
/// checkpoint. A cursor makes playback one step per frame.
struct Replay {
    checkpoints: Vec<Cursor>,
    music: Vec<u16>,
    cursor: Mutex<Option<Cursor>>,
}

/// The 256 x 200 bytes of a buffer the zoom reads.
fn buffer_rows(buffer: &[u8]) -> Vec<u8> {
    buffer[..BUFFER_SIZE[0] * BUFFER_SIZE[1]].to_vec()
}

impl Cursor {
    /// One step, keeping the buffer the next frame shows: the level a swap
    /// draws, before `perturb` changes it, or the unchanged source. Returns
    /// whether the scene reported done.
    fn step(&mut self, music: u16) -> bool {
        let terrain = self.field.hd_state().terrain;
        let before = (!terrain).then(|| buffer_rows(self.field.hd_buffers()[1]));
        let done = self.field.step(&mut self.vga, music);
        let state = self.field.hd_state();
        if !state.terrain {
            self.shown = if state.swapped {
                before.unwrap()
            } else {
                buffer_rows(self.field.hd_buffers()[0])
            };
        }
        self.at += 1;
        done
    }
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
            cursor.step(self.music[cursor.at]);
        }
        read(cursor)
    }
}

/// Part 5 of the HD clip: from part 6's end to part 3's first frame.
pub struct FieldPreview {
    /// Clip frame of part 5's first frame (the PSTART picture `Field::new`
    /// leaves). Part-5 frame `j` is clip frame `start + j`.
    pub start: usize,
    pub frames: Vec<FieldFrame>,
    /// The distinct palettes the frames show, 768 6-bit DAC values each.
    pub palettes: Vec<Vec<u8>>,
    /// The picture `Field::new` makes from PSTART, 320 x 200 indices: the
    /// frame round the window, which nothing in part 5 redraws, and in the
    /// window PSTART's own pixels until the first zoom frame.
    pub picture: Vec<u8>,
    /// The prepared terrain, 200 x 200 each, row x and column y for map point
    /// (x, y), as `prepare` leaves them: colour indices (128 to 191) and
    /// heights (0 to 244).
    pub colors: Vec<u8>,
    pub heights: Vec<u8>,
    /// The highest of each coarse cell's heights, with a map unit of margin
    /// for bilinear sampling: [`COARSE_CELLS`] squared, row-major in x.
    pub coarse: Vec<u8>,
    /// The scene's sine table (amplitude 256).
    pub sine: Vec<i32>,
    pub fit: MandelFit,
    pub relief: Relief,
    pub events: FieldEvents,
    /// The last zoom frame: its scale and buffer stay in the window, still,
    /// until the window is cleared.
    pub last_zoom: usize,
    /// Clip time of part 6's first PSTART fade frame (level 0), if known, for
    /// the picture level before part 5.
    pub picture_fade: Option<f64>,
    replay: Replay,
}

impl FieldPreview {
    /// Continue the clip from part 6's end: make the scene on `vga`, as the
    /// engine does in the step in which part 6 reports done (the step that
    /// starts `start`), and step it with the music until it hands over to
    /// part 3, appending its PCM. The step that returns done is the one in
    /// which the engine makes part 3; its PCM is included, so the clip ends
    /// at part 3's first frame, which shows part 5's last.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        start: usize,
        picture_fade: Option<f64>,
    ) -> Result<Self, String> {
        let field = Field::new(data, vga)?;
        let picture = field.hd_video().to_vec();
        let [colors, heights] = field.hd_terrain().map(|a| {
            (0..MAP_SIZE * MAP_SIZE)
                .map(|at| a[(at / MAP_SIZE) * 256 + at % MAP_SIZE])
                .collect::<Vec<u8>>()
        });
        let coarse = coarse_maximum(&heights, MAP_SIZE, 1);
        let sine = field.hd_sine().to_vec();
        let dep = data.file("DEP.U")?;
        if dep.len() < 320 * MAP_SIZE {
            return Err("DEP.U must hold 200 rows of 320".into());
        }
        let dep: Vec<u8> = (0..MAP_SIZE * MAP_SIZE)
            .map(|at| dep[(at / MAP_SIZE) * 320 + at % MAP_SIZE])
            .collect();
        let fit = MandelFit::fit(&dep)?;
        let relief = Relief::bake(&fit, RELIEF_SIZE);
        let mut cursor = Cursor {
            at: 0,
            shown: buffer_rows(field.hd_buffers()[0]),
            field,
            vga: vga.clone(),
        };
        let mut checkpoints = Vec::new();
        let mut frames: Vec<FieldFrame> = Vec::new();
        let mut palettes: Vec<Vec<u8>> = Vec::new();
        let mut positions = Vec::new();
        let mut swaps = 0;
        loop {
            let j = frames.len();
            if j.is_multiple_of(REPLAY_INTERVAL) {
                checkpoints.push(cursor.clone());
            }
            if j > FRAME_RATE as usize * 120 {
                return Err("Part 5 exceeded its two-minute import limit".into());
            }
            let state = cursor.field.hd_state();
            swaps += u32::from(state.swapped);
            let palette = match palettes.iter().position(|p| *p == cursor.vga.palette) {
                Some(i) => i,
                None => {
                    palettes.push(cursor.vga.palette.clone());
                    palettes.len() - 1
                }
            };
            let mountain = state.terrain
                && cursor
                    .vga
                    .indices
                    .iter()
                    .any(|&i| (128..192).contains(&i) && i != CLEAR_INDEX);
            frames.push(FieldFrame {
                state,
                swaps,
                palette,
                mountain,
            });
            let position = music.position();
            positions.push(position);
            let done = cursor.step(position);
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
            if done {
                break;
            }
        }
        *vga = cursor.vga.clone();
        // `positions` holds one more entry than the steps between frames.
        positions.pop();
        let last_zoom = frames
            .iter()
            .rposition(|f| !f.state.terrain)
            .ok_or("Part 5 never zoomed")?;
        let events = FieldEvents::observe(&frames);
        Ok(Self {
            start,
            frames,
            palettes,
            picture,
            colors,
            heights,
            coarse,
            sine,
            fit,
            relief,
            events,
            last_zoom,
            picture_fade,
            replay: Replay {
                checkpoints,
                music: positions,
                cursor: Mutex::new(None),
            },
        })
    }
}

impl FieldPreview {
    /// Part-5 frame position at a clip time: continuous, negative before
    /// part 5.
    pub fn position(&self, time: f64) -> f64 {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        // A frame's own time, as a clip time in seconds, lands within
        // rounding of it.
        if (position - position.round()).abs() < 1e-6 {
            position.round()
        } else {
            position
        }
    }

    /// Part-5 frame at a clip time, or `None` before part 5. Times past the
    /// end hold the last frame.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = self.position(time);
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Clip time of part-5 frame `j`.
    pub fn seconds(&self, j: usize) -> f64 {
        (self.start + j) as f64 / FRAME_RATE as f64
    }

    /// Indices and palette of part-5 frame `j` exactly as the engine shows
    /// it, replayed with the port's scene (see [`Replay`]).
    pub fn original(&self, j: usize) -> (Vec<u8>, Vec<u8>) {
        self.replay
            .with(j, |c| (c.vga.indices.clone(), c.vga.palette.clone()))
    }

    /// The original frame `j` as 8-bit RGBA, as the reference scanout shows it.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let (indices, palette) = self.original(j);
        rgba(&indices, &palette)
    }

    /// The zoom buffers for frame `j` (256 x 200 bytes each): the one the
    /// window shows at the frame's own time, and the one the next frame
    /// shows, for the interpolation between them ([`FieldBlock::zoom`]).
    /// Part 5's first frame, which shows no zoom yet, takes the first zoom
    /// frame's; once the zoom has stopped, both are the last zoom frame's.
    pub fn zoom_buffers(&self, j: usize) -> [Vec<u8>; 2] {
        let j = j.max(1).min(self.frames.len() - 1);
        let ahead = self
            .frames
            .get(j + 1)
            .filter(|f| !f.state.terrain)
            .map(|f| f.state.swapped);
        self.replay.with(j, |c| {
            let next = match ahead {
                Some(true) => buffer_rows(c.field.hd_buffers()[1]),
                Some(false) => buffer_rows(c.field.hd_buffers()[0]),
                None => c.shown.clone(),
            };
            [c.shown.clone(), next]
        })
    }

    /// Title label for part-5 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        let t = self.seconds(j.min(self.frames.len() - 1)) - self.seconds(0);
        let e = &self.events;
        if t < e.braking {
            "Part 5: field zoom"
        } else if t < e.terrain {
            "Zoom braking"
        } else if t < e.risen {
            "Mountain rising"
        } else if t < e.sinking {
            "Mountain"
        } else if t <= e.last_terrain {
            "Mountain sinking"
        } else {
            "Frame, dark window"
        }
    }

    /// The sine table at a continuous index, linear between its entries
    /// (exact at whole indices), wrapping every 1024.
    pub fn table(&self, index: f64) -> f64 {
        let n = self.sine.len();
        let i = index.rem_euclid(n as f64);
        let k = i.floor() as usize % n;
        let a = self.sine[k] as f64;
        let b = self.sine[(k + 1) % n] as f64;
        a + (b - a) * (i - i.floor())
    }

    /// The camera at a continuous terrain step `tf` (the one whose step
    /// number the code calls `terrain_frame` before it counts it) and level,
    /// by the code's formulas for the run with music, without their integer
    /// truncation (an interpretation): the camera [map x, map y, height,
    /// angle] and view [sine, cosine, radius, distance] of [`FieldBlock`].
    pub fn camera_at(&self, tf: f64, level: f64) -> ([f64; 4], [f64; 4]) {
        let angle = 512.0 + 10.0 * tf;
        let radius = 70.0 + self.table(8.0 * tf) / 12.0;
        let x = 100.0 + self.table(angle + 512.0) * radius / 100.0;
        let y = 100.0 + self.table(angle + 768.0) * radius / 100.0;
        let height = 3.0 * radius + 470.0 - 5.0 * level;
        (
            [x, y, height, angle],
            [
                self.table(angle),
                self.table(angle + 256.0),
                radius,
                (x - 100.0).hypot(y - 100.0),
            ],
        )
    }

    /// The zoom's scale in frame `j`'s shown buffer `f` of the way to the
    /// next frame, and whether the next frame swaps to the next level. The
    /// original lowers the scale linearly; between frames it is
    /// interpolated linearly, measured in the shown buffer's pixels, so
    /// across a swap it falls below 1, where the next level (twice as fine)
    /// takes over at twice the scale.
    fn zoom_scale(&self, j: usize, f: f64) -> (f64, bool) {
        let scale = |k: usize| self.frames[k].state.scale as f64 / 65536.0;
        if j == 0 {
            return (scale(1.min(self.frames.len() - 1)), false);
        }
        if j >= self.last_zoom {
            return (scale(self.last_zoom), false);
        }
        let next = &self.frames[j + 1].state;
        let end = scale(j + 1) / if next.swapped { 2.0 } else { 1.0 };
        (scale(j) + (end - scale(j)) * f, next.swapped)
    }

    /// The picture level at a clip time: 1 in part 5, and before it the
    /// level of part 6's PSTART fade (64 frames from `picture_fade`).
    fn picture_level(&self, time: f64) -> f64 {
        if time >= self.seconds(0) {
            return 1.0;
        }
        self.picture_fade.map_or(0.0, |from| {
            ((time - from) * FRAME_RATE as f64 / 64.0).clamp(0.0, 63.0 / 64.0)
        })
    }

    /// Everything part 5 puts in the shared uniforms at a clip time but the
    /// fluid, which the renderer adds. Before part 5 (for part 6's fade and
    /// the fluid's early steps) it holds part 5's first frame.
    pub fn controls(&self, time: f64) -> FieldBlock {
        let last = self.frames.len() - 1;
        let position = self.position(time).clamp(0.0, last as f64);
        let j = position.floor() as usize;
        let f = position - j as f64;
        let next = (j + 1).min(last);
        let (a, b) = (&self.frames[j], &self.frames[next]);
        let lerp = |x: f64, y: f64| x + (y - x) * f;
        let seconds = time - self.seconds(0);
        let (scale, ahead) = self.zoom_scale(j, f);
        let shown = if j == 0 {
            1.min(last)
        } else {
            j.min(self.last_zoom)
        };
        let swaps = self.frames[shown].swaps as f64;
        let state = a.state;
        // The camera of the terrain step this frame shows, moving on to the
        // next frame's.
        let tf = if state.terrain {
            let moves = b.state.terrain_frame > state.terrain_frame;
            state.terrain_frame as f64 - 1.0 + if moves { f } else { 0.0 }
        } else {
            0.0
        };
        let level = lerp(a.state.level as f64, b.state.level as f64);
        let q = lerp(
            a.state.projection.unwrap_or(0) as f64,
            b.state.projection.unwrap_or(0) as f64,
        );
        let (camera, view) = self.camera_at(tf, level);
        let e = &self.events;
        let cleared = state.cleared;
        let mut block = FieldBlock {
            part: [
                1.0,
                seconds as f32,
                if state.terrain { 1.0 } else { 0.0 },
                self.position(time).max(0.0).min(last as f64) as f32,
            ],
            zoom: [
                scale as f32,
                if ahead { 1.0 } else { 0.0 },
                (swaps - scale.log2()) as f32,
                swaps as f32,
            ],
            zoom_state: [
                (lerp(a.state.speed as f64, b.state.speed as f64) / 1024.0) as f32,
                lerp(a.state.next_row as f64, b.state.next_row as f64) as f32,
                if (1..=self.last_zoom).contains(&j) {
                    1.0
                } else {
                    0.0
                },
                if j == 0 { 0.0 } else { 1.0 },
            ],
            camera: camera.map(|v| v as f32),
            view: view.map(|v| v as f32),
            mountain: [
                level as f32,
                q as f32,
                tf as f32,
                if state.terrain { 1.0 } else { 0.0 },
            ],
            window: [
                if cleared { 1.0 } else { 0.0 },
                if cleared {
                    (seconds - e.cleared) as f32
                } else {
                    -1.0
                },
                self.picture_level(time) as f32,
                lerp(a.mountain as u8 as f64, b.mountain as u8 as f64) as f32,
            ],
            mandel: [
                self.fit.centre[0] as f32,
                self.fit.centre[1] as f32,
                self.fit.scale as f32,
                self.fit.aspect as f32,
            ],
            mandel_fit: [
                self.fit.base as f32,
                self.fit.slope as f32,
                self.fit.iou as f32,
                self.fit.rms as f32,
            ],
            relief: [
                self.relief.size as f32,
                (MAP_SIZE as f64 / self.relief.size as f64) as f32,
                self.relief.highest,
                self.fit.plateau as f32,
            ],
            events: [e.full_speed, e.braking, e.terrain, e.cleared].map(|v| v as f32),
            events_late: [e.risen, e.sinking, e.last_terrain, e.end].map(|v| v as f32),
            fluid_region: FLUID.region,
            ..FieldBlock::zeroed()
        };
        block.frame_style = (FRAME.controls)(&block, time);
        block.zoom_style = (ZOOM.controls)(&block, time);
        block.mountain_style = (MOUNTAIN.controls)(&block, time);
        block
    }
}

/// 8-bit RGBA of indices through a 6-bit palette, as the scanout expands it.
pub fn rgba(indices: &[u8], palette: &[u8]) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|&i| {
            let p = &palette[i as usize * 3..][..3];
            let c = |v: u8| {
                let v = v & 63;
                (v << 2) | (v >> 4)
            };
            [c(p[0]), c(p[1]), c(p[2]), 255]
        })
        .collect()
}

impl FieldEvents {
    /// Read the events from the frames; one that never happens reads -1.
    pub fn observe(frames: &[FieldFrame]) -> Self {
        let at = |j: Option<usize>| j.map_or(-1.0, |j| j as f64 / FRAME_RATE as f64);
        let first = |test: &dyn Fn(usize, &FieldFrame) -> bool| {
            frames.iter().enumerate().position(|(j, f)| test(j, f))
        };
        let full = first(&|_, f| !f.state.terrain && f.state.speed >= 1024);
        // The speed dips by 16 from 14:01 and holds; it falls for good from
        // the start's row plus 0x400. Braking is the last run of falls.
        let last_zoom = frames.iter().rposition(|f| !f.state.terrain);
        let braking = last_zoom.map(|mut j| {
            while j > 1 && frames[j].state.speed < frames[j - 1].state.speed {
                j -= 1;
            }
            j + 1
        });
        let risen = first(&|_, f| f.state.terrain && f.state.level >= 64);
        let sinking = first(&|j, f| risen.is_some_and(|k| j > k) && f.state.level < 64);
        Self {
            full_speed: at(full),
            braking: at(braking),
            terrain: at(first(&|_, f| f.state.terrain)),
            cleared: at(first(&|_, f| f.state.cleared)),
            risen: at(risen),
            sinking: at(sinking),
            last_terrain: at(frames.iter().rposition(|f| f.mountain)),
            end: at(Some(frames.len())),
        }
    }
}

// ---- The Mandelbrot fit ------------------------------------------------------

/// Iterations that decide the plateau, the set, as in the research's fit.
pub const SET_ITERATIONS: u32 = 100;
/// Iterations for the potential, and its bailout on |z|^2.
pub const POTENTIAL_ITERATIONS: u32 = 600;
pub const BAILOUT: f64 = 1e8;

/// Whether c stays bounded for `limit` iterations: the main cardioid and the
/// period-2 bulb are tested directly, and an orbit that returns to a value
/// it had (within 1e-14) is periodic, so bounded.
pub fn in_set(c: [f64; 2], limit: u32) -> bool {
    let [x, y] = c;
    let q = (x - 0.25) * (x - 0.25) + y * y;
    if q * (q + x - 0.25) <= 0.25 * y * y || (x + 1.0) * (x + 1.0) + y * y <= 0.0625 {
        return true;
    }
    let (mut zr, mut zi, mut pr, mut pi) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut checked = 8;
    for n in 1..=limit {
        let t = zr * zr - zi * zi + x;
        zi = 2.0 * zr * zi + y;
        zr = t;
        if zr * zr + zi * zi > 4.0 {
            return false;
        }
        if (zr - pr).abs() < 1e-14 && (zi - pi).abs() < 1e-14 {
            return true;
        }
        if n == checked {
            (pr, pi) = (zr, zi);
            checked *= 2;
        }
    }
    true
}

/// The Mandelbrot potential G(c) = log|z_n| / 2^n at the first n with
/// |z_n|^2 above [`BAILOUT`], and its gradient (dG/d re, dG/d im); `None` if c
/// stays bounded for `limit` iterations.
pub fn potential(c: [f64; 2], limit: u32) -> Option<(f64, [f64; 2])> {
    let (mut zr, mut zi, mut dr, mut di) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut power = 1.0_f64;
    for _ in 0..limit {
        // z' = 2 z z' + 1, then z = z^2 + c.
        (dr, di) = (2.0 * (zr * dr - zi * di) + 1.0, 2.0 * (zr * di + zi * dr));
        (zr, zi) = (zr * zr - zi * zi + c[0], 2.0 * zr * zi + c[1]);
        power *= 2.0;
        let m = zr * zr + zi * zi;
        if m > BAILOUT {
            // grad Re(log z) = (Re(z'/z), -Im(z'/z)).
            let (qr, qi) = ((dr * zr + di * zi) / m, (di * zr - dr * zi) / m);
            return Some((0.5 * m.ln() / power, [qr / power, -qi / power]));
        }
    }
    None
}

/// DEP.U's heights as a Mandelbrot mountain (an interpretation, after the
/// research's tools/fit_mandelbrot.py): its plateau is the set, after
/// [`SET_ITERATIONS`], and its slopes follow `base + slope sqrt(G(c))`, G the
/// potential. Map point (x, y), continuous, DEP.U's row x and column y,
/// is c = ((y - 100) / (scale aspect) + centre.re, (x - 100) / scale +
/// centre.im): the real axis runs along DEP.U's rows. Fitted when the data
/// loads; nothing fitted is stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MandelFit {
    /// c at map point (100, 100): real, imaginary.
    pub centre: [f64; 2],
    /// Map units per unit of c along map x, the imaginary axis.
    pub scale: f64,
    /// Map y's units per unit of c (the real axis) over `scale`.
    pub aspect: f64,
    /// DEP.U on the slopes: `base + slope sqrt(G)`.
    pub base: f64,
    pub slope: f64,
    /// DEP.U's highest value, the plateau's.
    pub plateau: u8,
    /// The plateau's intersection over union with the set.
    pub iou: f64,
    /// The slopes' rms error, in DEP units, and their correlation with
    /// sqrt(G).
    pub rms: f64,
    pub correlation: f64,
}

impl MandelFit {
    /// c at map point m.
    pub fn c(&self, m: [f64; 2]) -> [f64; 2] {
        Self::map(self.centre, self.scale, self.aspect, m)
    }

    fn map(centre: [f64; 2], scale: f64, aspect: f64, m: [f64; 2]) -> [f64; 2] {
        [
            (m[1] - 100.0) / (scale * aspect) + centre[0],
            (m[0] - 100.0) / scale + centre[1],
        ]
    }

    /// The fitted DEP.U value at map point m, and its gradient in DEP units
    /// per map unit (zero where it clamps).
    pub fn dep(&self, m: [f64; 2]) -> (f64, [f64; 2]) {
        let c = self.c(m);
        if in_set(c, SET_ITERATIONS) {
            return (self.plateau as f64, [0.0; 2]);
        }
        match potential(c, POTENTIAL_ITERATIONS) {
            Some((g, grad)) => self.slope_at(g, grad),
            None => (self.plateau as f64, [0.0; 2]),
        }
    }

    /// DEP and its gradient per map unit outside the set, from the potential
    /// and its gradient.
    fn slope_at(&self, g: f64, grad: [f64; 2]) -> (f64, [f64; 2]) {
        let plateau = self.plateau as f64;
        let root = g.max(1e-300).sqrt();
        let value = self.base + self.slope * root;
        if value <= 0.0 || value >= plateau {
            return (value.clamp(0.0, plateau), [0.0; 2]);
        }
        // d sqrt(G) = grad G / (2 sqrt G); d re / d y = 1 / (scale aspect),
        // d im / d x = 1 / scale.
        let k = self.slope / (2.0 * root);
        (
            value,
            [
                k * grad[1] / self.scale,
                k * grad[0] / (self.scale * self.aspect),
            ],
        )
    }

    /// Fit DEP.U's first 200 columns of its 200 rows (`dep`, 200 x 200, row
    /// x then column y). The plateau is its highest value. The set's place,
    /// scale and aspect start from the plateau's moments against the set's
    /// own, then a pattern search maximises the intersection over union; the
    /// slopes' base and slope are then a least-squares line through DEP
    /// against sqrt(G) where DEP lies strictly between 0 and the plateau.
    pub fn fit(dep: &[u8]) -> Result<Self, String> {
        let n = MAP_SIZE;
        if dep.len() != n * n {
            return Err("The height map must be 200 x 200".into());
        }
        let plateau = *dep.iter().max().unwrap();
        let target: Vec<bool> = dep.iter().map(|&d| d == plateau).collect();
        let count = target.iter().filter(|&&t| t).count();
        if plateau < 129 || count < 50 {
            return Err("The height map has no plateau to fit".into());
        }
        let point = |at: usize| [(at / n) as f64 + 0.5, (at % n) as f64 + 0.5];
        let (mut mean, mut square) = ([0.0; 2], [0.0; 2]);
        for at in (0..n * n).filter(|&at| target[at]) {
            let p = point(at);
            for k in 0..2 {
                mean[k] += p[k];
                square[k] += p[k] * p[k];
            }
        }
        let variance: [f64; 2] = std::array::from_fn(|k| {
            mean[k] /= count as f64;
            square[k] / count as f64 - mean[k] * mean[k]
        });
        let set = SetMoments::compute();
        let scale = (variance[0] / set.variance[1]).sqrt();
        let aspect = (variance[1] / set.variance[0]).sqrt() / scale;
        let centre = [
            set.mean - (mean[1] - 100.0) / (scale * aspect),
            -(mean[0] - 100.0) / scale,
        ];
        // The pixels near the plateau's edge decide the overlap; elsewhere
        // the fitted set agrees with it once the start is close.
        let band: Vec<usize> = (0..n * n)
            .filter(|&at| {
                let (x, y) = ((at / n) as i64, (at % n) as i64);
                (-3..=3).any(|dx| {
                    (-3..=3).any(|dy| {
                        let (u, v) = (x + dx, y + dy);
                        (0..n as i64).contains(&u)
                            && (0..n as i64).contains(&v)
                            && target[u as usize * n + v as usize] != target[at]
                    })
                })
            })
            .collect();
        let inner = count - band.iter().filter(|&&at| target[at]).count();
        // The search moves the centre and each axis's scale (the logs of map
        // units per unit of c along the imaginary and real axes), which act
        // on the set's outline independently.
        let geometry = |p: [f64; 4]| ([p[0], p[1]], p[2].exp(), (p[3] - p[2]).exp());
        let overlap = |p: [f64; 4], pixels: &mut dyn Iterator<Item = usize>, inner: usize| {
            let (mut both, mut either) = (inner, inner);
            let (centre, scale, aspect) = geometry(p);
            for at in pixels {
                let inside = in_set(Self::map(centre, scale, aspect, point(at)), SET_ITERATIONS);
                both += usize::from(inside && target[at]);
                either += usize::from(inside || target[at]);
            }
            both as f64 / either.max(1) as f64
        };
        let score = |p: [f64; 4]| overlap(p, &mut band.iter().copied(), inner);
        // Nelder-Mead on the overlap, as the research's fit, restarted from
        // its best point with a smaller simplex: first steps of half a
        // pixel, a shift and a stretch 60 pixels out.
        let mut p = [centre[0], centre[1], scale.ln(), (scale * aspect).ln()];
        let mut size = [0.5 / (scale * aspect), 0.5 / scale, 0.5 / 60.0, 0.5 / 60.0];
        for _ in 0..4 {
            p = nelder_mead(&score, p, size, 120);
            size = size.map(|s| s * 0.3);
        }
        let (centre, scale, aspect) = geometry(p);
        let iou = overlap(p, &mut (0..n * n), 0);
        // The slopes: DEP against sqrt(G).
        let mut sums = [0.0_f64; 5];
        let mut samples = Vec::new();
        for (at, &value) in dep.iter().enumerate() {
            let d = value as f64;
            if value == 0 || value == plateau {
                continue;
            }
            let c = Self::map(centre, scale, aspect, point(at));
            if in_set(c, SET_ITERATIONS) {
                continue;
            }
            if let Some((g, _)) = potential(c, POTENTIAL_ITERATIONS) {
                let x = g.max(0.0).sqrt();
                for (s, v) in sums.iter_mut().zip([1.0, x, d, x * x, x * d]) {
                    *s += v;
                }
                samples.push((x, d));
            }
        }
        let [k, sx, sy, sxx, sxy] = sums;
        let denominator = k * sxx - sx * sx;
        if samples.len() < 50 || denominator.abs() < 1e-12 {
            return Err("The height map has too few slope pixels to fit".into());
        }
        let slope = (k * sxy - sx * sy) / denominator;
        let base = (sy - slope * sx) / k;
        let rms = (samples
            .iter()
            .map(|&(x, d)| (base + slope * x - d).powi(2))
            .sum::<f64>()
            / k)
            .sqrt();
        let syy: f64 = samples.iter().map(|&(_, d)| d * d).sum();
        let correlation = (k * sxy - sx * sy) / (denominator * (k * syy - sy * sy)).sqrt();
        Ok(Self {
            centre,
            scale,
            aspect,
            base,
            slope,
            plateau,
            iou,
            rms,
            correlation,
        })
    }
}

/// Maximise `score` from `start` by Nelder-Mead with an initial simplex of
/// `size` along each axis, for at most `limit` iterations (deterministic).
fn nelder_mead(
    score: &dyn Fn([f64; 4]) -> f64,
    start: [f64; 4],
    size: [f64; 4],
    limit: usize,
) -> [f64; 4] {
    let mut simplex: Vec<([f64; 4], f64)> = (0..5)
        .map(|k| {
            let mut p = start;
            if k > 0 {
                p[k - 1] += size[k - 1];
            }
            (p, score(p))
        })
        .collect();
    let along =
        |a: [f64; 4], b: [f64; 4], t: f64| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
    for _ in 0..limit {
        simplex.sort_by(|a, b| b.1.total_cmp(&a.1));
        if simplex[0].1 - simplex[4].1 < 1e-9 {
            break;
        }
        let centroid: [f64; 4] =
            std::array::from_fn(|i| simplex[..4].iter().map(|v| v.0[i]).sum::<f64>() / 4.0);
        let worst = simplex[4];
        let reflected = along(centroid, worst.0, -1.0);
        let r = score(reflected);
        if r > simplex[0].1 {
            let expanded = along(centroid, worst.0, -2.0);
            let e = score(expanded);
            simplex[4] = if e > r { (expanded, e) } else { (reflected, r) };
        } else if r > simplex[3].1 {
            simplex[4] = (reflected, r);
        } else {
            let contracted = along(centroid, worst.0, 0.5);
            let c = score(contracted);
            if c > worst.1 {
                simplex[4] = (contracted, c);
            } else {
                let best = simplex[0].0;
                for v in &mut simplex[1..] {
                    v.0 = along(best, v.0, 0.5);
                    v.1 = score(v.0);
                }
            }
        }
    }
    simplex.sort_by(|a, b| b.1.total_cmp(&a.1));
    simplex[0].0
}

/// The set's own moments, from a raster of it (0.005 units, cell centres,
/// [`SET_ITERATIONS`]): the mean and variance of the real part, and the
/// variance of the imaginary part (its mean is 0).
struct SetMoments {
    mean: f64,
    variance: [f64; 2],
}

impl SetMoments {
    fn compute() -> Self {
        let h = 0.005;
        let (mut count, mut sr, mut srr, mut sii) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..(2.7 / h) as usize {
            let re = -2.1 + (i as f64 + 0.5) * h;
            for k in 0..(1.25 / h) as usize {
                let im = (k as f64 + 0.5) * h;
                if in_set([re, im], SET_ITERATIONS) {
                    count += 1.0;
                    sr += re;
                    srr += re * re;
                    sii += im * im;
                }
            }
        }
        let mean = sr / count;
        Self {
            mean,
            variance: [srr / count - mean * mean, sii / count],
        }
    }
}

/// The fitted mountain baked for the shaders (an interpretation of DEP.U,
/// see [`MandelFit`]): a square texture over the 200 x 200 map, row x then
/// column y as DEP.U, each texel at its centre holding the height `prepare`
/// would make of the fitted DEP value (2 (DEP - 128), from 0 to 244 on the
/// plateau; without its three-pixel average), its gradient in height units
/// per map unit, and the distance outside the set in map units (a potential
/// estimate, G / (2 |grad G|); 0 inside, at most 64).
pub struct Relief {
    pub size: usize,
    pub texels: Vec<[f16; 4]>,
    /// The highest of each coarse cell's heights, with a texel of margin,
    /// as [`FieldPreview::coarse`].
    pub coarse: Vec<u8>,
    pub highest: f32,
}

impl Relief {
    pub fn bake(fit: &MandelFit, size: usize) -> Self {
        let mut texels = vec![[f16::ZERO; 4]; size * size];
        let rows = size.div_ceil(
            std::thread::available_parallelism()
                .map_or(4, |n| n.get())
                .max(1),
        );
        std::thread::scope(|scope| {
            for (chunk, block) in texels.chunks_mut(rows * size).enumerate() {
                scope.spawn(move || {
                    for (k, texel) in block.iter_mut().enumerate() {
                        let at = chunk * rows * size + k;
                        let unit = MAP_SIZE as f64 / size as f64;
                        let m = [
                            ((at / size) as f64 + 0.5) * unit,
                            ((at % size) as f64 + 0.5) * unit,
                        ];
                        *texel = Self::texel(fit, m).map(f16::from_f64);
                    }
                });
            }
        });
        let height: Vec<f32> = texels.iter().map(|t| t[0].to_f32()).collect();
        let highest = height.iter().copied().fold(0.0, f32::max);
        Self {
            size,
            coarse: coarse_cells(size, 1, |x, y| height[x * size + y]),
            texels,
            highest,
        }
    }

    /// One texel's values at map point m.
    pub fn texel(fit: &MandelFit, m: [f64; 2]) -> [f64; 4] {
        let c = fit.c(m);
        let top = 2.0 * (fit.plateau as f64 - 128.0).max(0.0);
        let found = (!in_set(c, SET_ITERATIONS))
            .then(|| potential(c, POTENTIAL_ITERATIONS))
            .flatten();
        let Some((g, grad)) = found else {
            return [top, 0.0, 0.0, 0.0];
        };
        let (dep, slope) = fit.slope_at(g, grad);
        let per_map = (grad[1] / fit.scale).hypot(grad[0] / (fit.scale * fit.aspect));
        let distance = (0.5 * g / per_map.max(1e-300)).min(64.0);
        if dep <= 128.0 {
            return [0.0, 0.0, 0.0, distance];
        }
        [
            2.0 * (dep - 128.0),
            2.0 * slope[0],
            2.0 * slope[1],
            distance,
        ]
    }
}

/// The highest value in each coarse cell of a `size` x `size` grid over the
/// map, with `margin` texels round it, rounded up: [`COARSE_CELLS`] squared,
/// row-major in x.
fn coarse_cells(size: usize, margin: usize, at: impl Fn(usize, usize) -> f32) -> Vec<u8> {
    let range = |c: usize| {
        let lo = (c * size / COARSE_CELLS).saturating_sub(margin);
        let hi = ((c + 1) * size).div_ceil(COARSE_CELLS) + margin;
        lo..hi.min(size)
    };
    let mut cells = vec![0; COARSE_CELLS * COARSE_CELLS];
    for cx in 0..COARSE_CELLS {
        for cy in 0..COARSE_CELLS {
            let highest = range(cx)
                .flat_map(|x| range(cy).map(move |y| (x, y)))
                .map(|(x, y)| at(x, y))
                .fold(0.0, f32::max);
            cells[cx * COARSE_CELLS + cy] = highest.ceil().min(255.0) as u8;
        }
    }
    cells
}

/// [`coarse_cells`] of a 200 x 200 byte map.
fn coarse_maximum(map: &[u8], size: usize, margin: usize) -> Vec<u8> {
    coarse_cells(size, margin, |x, y| map[x * size + y] as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    /// An invented Mandelbrot mountain, not the original's.
    fn invented_fit() -> MandelFit {
        MandelFit {
            centre: [-0.52, 0.013],
            scale: 44.0,
            aspect: 1.18,
            base: 247.0,
            slope: -251.0,
            plateau: 250,
            iou: 1.0,
            rms: 0.0,
            correlation: -1.0,
        }
    }

    /// The invented mountain as a 200 x 200 height map of bytes, sampled at
    /// cell centres as DEP.U would be.
    fn invented_heights(fit: &MandelFit) -> Vec<u8> {
        (0..MAP_SIZE * MAP_SIZE)
            .map(|at| {
                let m = [(at / MAP_SIZE) as f64 + 0.5, (at % MAP_SIZE) as f64 + 0.5];
                fit.dep(m).0.round() as u8
            })
            .collect()
    }

    #[test]
    fn mandel_fit_recovers_an_invented_mountain() {
        let truth = invented_fit();
        let fit = MandelFit::fit(&invented_heights(&truth)).unwrap();
        assert_eq!(fit.plateau, 250);
        // Within a tenth of a pixel the set's ragged edge still flips a few
        // percent of the plateau's pixels.
        assert!(fit.iou > 0.97, "{fit:?}");
        assert!((fit.centre[0] - truth.centre[0]).abs() < 0.004, "{fit:?}");
        assert!((fit.centre[1] - truth.centre[1]).abs() < 0.004, "{fit:?}");
        assert!((fit.scale / truth.scale - 1.0).abs() < 0.005, "{fit:?}");
        assert!((fit.aspect / truth.aspect - 1.0).abs() < 0.005, "{fit:?}");
        assert!((fit.base - truth.base).abs() < 1.5, "{fit:?}");
        assert!((fit.slope - truth.slope).abs() < 3.0, "{fit:?}");
        // Rounding to bytes leaves about 0.3 of noise.
        assert!(fit.rms < 1.0 && fit.correlation < -0.99, "{fit:?}");
        // A map with no plateau has nothing to fit.
        assert!(MandelFit::fit(&vec![7; MAP_SIZE * MAP_SIZE]).is_err());
    }

    #[test]
    fn relief_texels_follow_the_fit() {
        let fit = invented_fit();
        let relief = Relief::bake(&fit, 50);
        assert_eq!(relief.texels.len(), 2500);
        assert_eq!(relief.coarse.len(), COARSE_CELLS * COARSE_CELLS);
        assert_eq!(relief.highest, 244.0);
        // Inside the set: the plateau, flat, no distance.
        let inside = Relief::texel(&fit, [100.0, 100.0 + 0.3 * fit.scale * fit.aspect]);
        assert_eq!(inside, [244.0, 0.0, 0.0, 0.0]);
        // On a slope: the gradient matches the height's differences, and the
        // distance grows away from the set.
        let mut slopes = 0;
        for k in 0..40 {
            let m = [60.0 + k as f64 * 2.1, 30.0 + k as f64 * 0.7];
            let t = Relief::texel(&fit, m);
            let e = 1e-4;
            let h = |dx: f64, dy: f64| Relief::texel(&fit, [m[0] + dx, m[1] + dy])[0];
            if t[0] <= 1.0 || t[0] >= 243.0 || h(e, 0.0) == 0.0 || h(0.0, e) == 0.0 {
                continue;
            }
            let numeric = [
                (h(e, 0.0) - h(-e, 0.0)) / (2.0 * e),
                (h(0.0, e) - h(0.0, -e)) / (2.0 * e),
            ];
            for (a, b) in t[1..3].iter().zip(numeric) {
                assert!(
                    (a - b).abs() < 1e-3 * (1.0 + b.abs()),
                    "{t:?} {numeric:?} at {m:?}"
                );
            }
            assert!(t[3] > 0.0);
            slopes += 1;
        }
        assert!(slopes > 10, "{slopes} slope samples");
        // The coarse cells bound every texel in them.
        for (at, t) in relief.texels.iter().enumerate() {
            let cell = (at / 50) * COARSE_CELLS / 50 * COARSE_CELLS + (at % 50) * COARSE_CELLS / 50;
            assert!(t[0].to_f32() <= relief.coarse[cell] as f32);
        }
    }

    /// Invented part-5 frames: `zooms` zoom frames (scale falling by
    /// `speed` a frame, swapping below 65536 as the code does), then
    /// `terrains` terrain frames with the code's music-run camera.
    fn invented_preview(zooms: usize, speed: i32, terrains: usize) -> FieldPreview {
        let sine: Vec<i32> = (0..1024)
            .map(|i| (256.0 * (i as f64 * std::f64::consts::TAU / 1024.0).sin()).round() as i32)
            .collect();
        let base = HdField {
            terrain: false,
            frame: 0,
            scale: 65536,
            speed: 16,
            swapped: false,
            next_row: 0,
            terrain_frame: 0,
            level: 0,
            projection: None,
            camera: [0; 5],
            cleared: false,
            done: false,
        };
        let mut frames = vec![FieldFrame {
            state: base,
            swaps: 0,
            palette: 0,
            mountain: false,
        }];
        let (mut scale, mut swaps) = (65536, 0);
        for k in 1..=zooms {
            scale -= speed;
            let mut state = HdField {
                frame: k,
                speed,
                next_row: 2,
                ..base
            };
            if scale < 65536 {
                scale += 65536;
                swaps += 1;
                state.swapped = true;
                state.next_row = 0;
            }
            state.scale = scale;
            frames.push(FieldFrame {
                state,
                swaps,
                palette: 0,
                mountain: false,
            });
        }
        for tf in 0..terrains {
            let level = (tf as i32 + 1).min(64);
            let radius = 70 + sine[(tf * 8) & 1023] / 12;
            let angle = (512 + tf * 10) & 1023;
            let camera = [
                100 + sine[(angle + 512) & 1023] * radius / 100,
                100 + sine[(angle + 768) & 1023] * radius / 100,
                3 * radius + 470 - 5 * level,
                sine[angle],
                sine[(angle + 256) & 1023],
            ];
            frames.push(FieldFrame {
                state: HdField {
                    terrain: true,
                    frame: zooms,
                    scale,
                    terrain_frame: tf + 1,
                    level,
                    projection: Some((tf as i32 * 8).min(248)),
                    camera,
                    ..base
                },
                swaps,
                palette: 0,
                mountain: true,
            });
        }
        let fit = invented_fit();
        FieldPreview {
            start: 0,
            events: FieldEvents::observe(&frames),
            last_zoom: zooms,
            frames,
            palettes: vec![vec![0; 768]],
            picture: vec![0; 64000],
            colors: vec![128; MAP_SIZE * MAP_SIZE],
            heights: vec![0; MAP_SIZE * MAP_SIZE],
            coarse: vec![0; COARSE_CELLS * COARSE_CELLS],
            sine,
            relief: Relief::bake(&fit, 25),
            fit,
            picture_fade: Some(-1.0),
            replay: Replay {
                checkpoints: Vec::new(),
                music: Vec::new(),
                cursor: Mutex::new(None),
            },
        }
    }

    #[test]
    fn zoom_scale_carries_on_into_the_next_level() {
        let preview = invented_preview(200, 1024, 0);
        let at = |position: f64| preview.controls(position / FRAME_RATE as f64);
        // At each frame's own time, its scale and its shown buffer's swaps.
        for j in 1..200 {
            let state = preview.frames[j].state;
            let z = at(j as f64).zoom;
            assert_eq!(z[0], state.scale as f32 / 65536.0, "frame {j}");
            assert_eq!(z[3], preview.frames[j].swaps as f32);
        }
        // Into a swap the scale falls below 1 and the next level takes over
        // at twice it: the level rises steadily, without a step.
        let swap = (2..200).find(|&j| preview.frames[j].state.swapped).unwrap();
        let mut last = at(swap as f64 - 2.0).zoom[2];
        for k in 1..=40 {
            let position = swap as f64 - 2.0 + k as f64 * 0.05;
            let z = at(position).zoom;
            assert_eq!(
                z[1] == 1.0,
                position < swap as f64 && position >= swap as f64 - 1.0
            );
            assert!(
                z[2] > last && z[2] - last < 0.02,
                "level at {position}: {z:?}"
            );
            last = z[2];
        }
        // At full speed a level takes 64 frames.
        let level = |j: usize| at(j as f64).zoom[2];
        assert!((level(150) - level(86) - 1.0).abs() < 0.05);
        // Part 5's first frame shows no zoom yet; later frames do.
        assert_eq!(at(0.5).zoom_state[3], 0.0);
        assert_eq!(at(1.0).zoom_state[3], 1.0);
        // Before part 5: its first frame's state, with part 6's fade.
        let before = preview.controls(-0.5);
        assert_eq!(before.part[1], -0.5);
        assert!(before.window[2] > 0.0 && before.window[2] < 1.0);
    }

    #[test]
    fn camera_follows_the_codes_orbit_without_truncation() {
        let preview = invented_preview(4, 1024, 300);
        for (j, frame) in preview.frames.iter().enumerate().skip(5) {
            let block = preview.controls(j as f64 / FRAME_RATE as f64);
            let state = frame.state;
            let [x, y, height, sine, cosine] = state.camera.map(|v| v as f32);
            let [cx, cy, ch, _] = block.camera;
            // The code truncates the radius, then its products; the HD camera
            // does neither: under 2.56 + 1 map units, and 3 height units.
            assert!((cx - x).abs() < 3.56 && (cy - y).abs() < 3.56, "frame {j}");
            assert!((ch - height).abs() < 3.0, "frame {j}");
            assert_eq!([block.view[0], block.view[1]], [sine, cosine]);
            assert_eq!(block.mountain[0], state.level as f32);
            assert_eq!(block.mountain[1], state.projection.unwrap() as f32);
            // The camera looks at the map's centre from 2.56 radii.
            let to_centre = [100.0 - cx, 100.0 - cy];
            let along = (to_centre[0] * sine + to_centre[1] * cosine) / 256.0;
            assert!((along - block.view[3]).abs() < 1e-2 * block.view[3]);
            assert!((block.view[3] - 2.56 * block.view[2]).abs() < 1e-2 * block.view[3]);
        }
        // Between frames it moves on smoothly.
        let a = preview.controls(100.0 / 70.0).camera;
        let b = preview.controls(100.5 / 70.0).camera;
        let c = preview.controls(101.0 / 70.0).camera;
        assert!((b[3] - (a[3] + c[3]) / 2.0).abs() < 1e-3);
    }

    #[test]
    fn authors_keep_their_entry_points_and_prefixes() {
        let entries = [
            (
                &FRAME,
                &[
                    "fn frame_layers(px:FieldPixel)->FrameLayers",
                    "fn frame_fluid_init(cell:FluidCell)->vec4f",
                    "fn frame_fluid(cell:FluidCell)->vec4f",
                    "fn frame_prepass(px:FieldPixel)->vec4f",
                ][..],
            ),
            (
                &ZOOM,
                &[
                    "fn zoom_layer(px:FieldPixel)->FieldLayer",
                    "fn zoom_prepass(px:FieldPixel)->vec4f",
                ][..],
            ),
            (
                &MOUNTAIN,
                &[
                    "fn mountain_layer(px:FieldPixel)->FieldLayer",
                    "fn mountain_prepass(px:FieldPixel)->vec4f",
                ][..],
            ),
        ];
        let common = include_str!("field_common.wgsl");
        for (author, wanted) in entries {
            for entry in wanted {
                assert!(
                    author.source.contains(entry),
                    "{} lacks {entry}",
                    author.name
                );
            }
            // The shared code leaves each author's prefix to the author, but
            // for the prepass reader it provides.
            let prefix = format!("fn {}_", author.name);
            let shared: Vec<&str> = common
                .lines()
                .filter(|l| l.starts_with(&prefix) && !l.contains("_prepass_at("))
                .collect();
            assert!(shared.is_empty(), "{shared:?}");
            assert_eq!(
                (author.controls)(&FieldBlock::default(), 100.0),
                [[0.0; 4]; 4]
            );
        }
        assert!(FLUID.rate > 0.0 && FLUID.checkpoint > 0);
        assert!(FLUID.region[2] > FLUID.region[0] && FLUID.region[3] > FLUID.region[1]);
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part5_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let started = std::time::Instant::now();
        let clip = Clip::load_field(&data, ScrollerLighting::Studio).unwrap();
        let loaded = started.elapsed().as_secs_f64();
        let rings = clip.rings.as_ref().unwrap();
        let field = clip.field.as_ref().unwrap();
        assert_eq!(field.start, rings.start + rings.frames.len());
        let map_palette = data.file("MAP.PAL").unwrap();
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in 0..field.start {
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(
            engine.part(),
            5,
            "part 5 starts at clip frame {}",
            field.start
        );
        let seconds = |j: usize| field.seconds(j);
        let (mut window_checked, mut window_matched) = (0_u64, 0_u64);
        let mut silhouette: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        let mut camera_error = [0.0_f64; 3];
        for (j, frame) in field.frames.iter().enumerate() {
            let video = &engine.video;
            assert_eq!((video.width, video.height), (320, 200));
            // The replayed original equals the engine's frame.
            let (indices, palette) = field.original(j);
            assert!(
                indices == video.indices,
                "replayed pixels at part-5 frame {j}"
            );
            assert!(
                palette == video.palette,
                "replayed palette at part-5 frame {j}"
            );
            assert!(
                field.palettes[frame.palette] == video.palette,
                "palette at {j}"
            );
            let state = frame.state;
            let window = |x: usize, y: usize| video.indices[(46 + y) * 320 + 96 + x];
            // Outside the window the frame is the picture, but for the terrain.
            for (at, (&got, &picture)) in video.indices.iter().zip(&field.picture).enumerate() {
                let (x, y) = (at % 320, at / 320);
                let inside = (96..224).contains(&x) && (46..146).contains(&y);
                if !inside && !(state.terrain && (128..192).contains(&got)) {
                    assert_eq!(got, picture, "frame pixel {x}, {y} at {j}");
                }
            }
            let zoomed = (j >= 1).then(|| field.zoom_buffers(j)[0].clone());
            if j >= 1 && !state.terrain {
                // The shown buffer at the drawn scale is the window, exactly,
                // as the port's own `zoom` draws it...
                let expected = Field::hd_window(zoomed.as_ref().unwrap(), state.scale);
                for y in 0..100 {
                    for x in 0..128 {
                        assert_eq!(window(x, y), expected[y * 128 + x], "zoom {x}, {y} at {j}");
                    }
                }
                // ...and, nearly everywhere, the HD reading of it: the
                // window's centre on the buffer's, at the scale.
                let buffer = zoomed.as_ref().unwrap();
                let sigma = state.scale as f64 / 65536.0;
                for y in 0..100 {
                    for x in 0..128 {
                        let u = 128.0 + (x as f64 + 0.5 - 64.0) * sigma;
                        let v = 100.0 + (y as f64 + 0.5 - 50.0) * sigma;
                        let b = buffer[(v.floor() as usize).min(199) * 256 + u.floor() as usize];
                        window_checked += 1;
                        window_matched += u64::from(b == window(x, y));
                    }
                }
            }
            if state.terrain {
                for i in 0..64 {
                    assert_eq!(
                        video.palette[(128 + i) * 3..][..3],
                        map_palette[i * 9..][..3],
                        "terrain palette at {j}"
                    );
                }
                let terrain = |x: usize, y: usize| {
                    let i = video.indices[y * 320 + x];
                    (128..192).contains(&i)
                        && !(state.cleared
                            && i == CLEAR_INDEX
                            && (96..224).contains(&x)
                            && (46..146).contains(&y))
                };
                // The window: still the last zoom frame's until cleared, then
                // colour 129, wherever the terrain leaves it.
                let last = Field::hd_window(
                    &field.zoom_buffers(field.last_zoom)[0],
                    field.frames[field.last_zoom].state.scale,
                );
                for y in 0..100 {
                    for x in 0..128 {
                        if !terrain(96 + x, 46 + y) {
                            let want = if state.cleared {
                                CLEAR_INDEX
                            } else {
                                last[y * 128 + x]
                            };
                            assert_eq!(window(x, y), want, "window {x}, {y} at {j}");
                        }
                    }
                }
                assert_eq!(
                    frame.mountain,
                    (0..64000).any(|at| terrain(at % 320, at / 320)),
                    "terrain shown at {j}"
                );
                // The camera: the code's formula, untruncated, within its
                // truncation of the frame's own.
                let block = field.controls(seconds(j));
                let [x, y, height, sine, cosine] = state.camera.map(|v| v as f64);
                for (k, (a, b)) in [
                    (block.camera[0], x),
                    (block.camera[1], y),
                    (block.camera[2], height),
                ]
                .into_iter()
                .enumerate()
                {
                    camera_error[k] = camera_error[k].max((a as f64 - b).abs());
                }
                assert_eq!([block.view[0] as f64, block.view[1] as f64], [sine, cosine]);
                // The HD terrain's silhouette, as terrain_march finds it for
                // each column's centre, against the original's top terrain
                // pixel, every tenth frame: with the frame's own integer
                // camera, which checks the march, and with the smooth one.
                if j % 10 == 0 && frame.mountain {
                    let q = state.projection.unwrap() as f64;
                    let smooth = [
                        block.camera[0],
                        block.camera[1],
                        block.camera[2],
                        block.view[0],
                        block.view[1],
                    ]
                    .map(|v| v as f64);
                    for column in 0..320 {
                        let original = (1..200).find(|&y| terrain(column, y));
                        let top = original.map_or(200.0, |a| a as f64);
                        for (k, camera) in [[x, y, height, sine, cosine], smooth].iter().enumerate()
                        {
                            let hd = silhouette_row(field, *camera, q, column as f64 + 0.5);
                            // The original's first row is the projection's
                            // ceiling (`>> 10` floors the height above the
                            // bottom).
                            if original.is_some() || hd.is_some() {
                                silhouette[k].push(top - hd.map_or(200.0, |r| r.ceil()));
                            }
                        }
                    }
                }
            }
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(
            engine.part(),
            3,
            "part 3 starts right after the last part-5 frame"
        );
        assert_eq!(pcm, clip.pcm, "uninterrupted PCM through part 5");
        assert!((clip.duration() - seconds(field.frames.len())).abs() < 1.0 / SAMPLE_RATE as f64);
        let share = window_matched as f64 / window_checked as f64;
        assert!(share > 0.99, "the HD zoom mapping matches {share}");
        assert!(camera_error[0] < 3.56 && camera_error[1] < 3.56 && camera_error[2] < 3.0);
        // Mean difference, mean absolute difference, share within a row, and
        // the 95th percentile of the absolute difference.
        let spread = |v: &Vec<f64>| {
            let n = v.len() as f64;
            let mut a: Vec<f64> = v.iter().map(|d| d.abs()).collect();
            a.sort_by(f64::total_cmp);
            let near = a.iter().filter(|&&d| d <= 1.0).count() as f64 / n;
            (
                v.iter().sum::<f64>() / n,
                a.iter().sum::<f64>() / n,
                near,
                a[a.len() * 95 / 100],
            )
        };
        let exact = spread(&silhouette[0]);
        let smooth = spread(&silhouette[1]);
        assert!(
            exact.1 < 1.5 && exact.2 > 0.75,
            "silhouette {exact:?} {smooth:?}"
        );
        // The timeline, as the research measured it with the same port, to
        // within a frame and its rounding to hundredths.
        let e = field.events;
        let at = |t: f64| seconds(0) + t;
        for (name, time, research) in [
            ("part 5 begins", at(0.0), 81.27),
            ("zoom at full speed", at(e.full_speed), 82.17),
            ("zoom braking", at(e.braking), 96.04),
            ("terrain begins", at(e.terrain), 96.96),
            ("risen to level 64", at(e.risen), 97.86),
            ("window cleared", at(e.cleared), 98.11),
            ("sinking", at(e.sinking), 121.83),
            ("last terrain pixel", at(e.last_terrain), 127.69),
            ("part 3 takes the frame", at(e.end), 130.03),
        ] {
            assert!(
                (time - research).abs() < 0.022,
                "{name} at {time}, not {research}"
            );
        }
        let last = &field.frames[field.last_zoom];
        let level = field.controls(seconds(field.last_zoom)).zoom[2];
        assert_eq!(last.swaps, 17);
        let fit = &field.fit;
        println!(
            "Part 5: clip frames {}..{} ({:.3}–{:.3} s), loaded with parts 1 to 6 in {loaded:.2} s",
            field.start,
            field.start + field.frames.len(),
            seconds(0),
            seconds(field.frames.len()),
        );
        for (name, t) in [
            ("zoom at full speed", e.full_speed),
            ("zoom braking", e.braking),
            ("terrain begins", e.terrain),
            ("risen to level 64", e.risen),
            ("window cleared", e.cleared),
            ("sinking", e.sinking),
            ("last terrain pixel", e.last_terrain),
            ("part 3 takes the frame", e.end),
        ] {
            println!("  {:8.3} s  {name}", at(t));
        }
        println!(
            "  zoom: {} swaps, level {level:.2} at its last frame ({:.0}x); {} palettes",
            last.swaps,
            2f64.powf(level as f64),
            field.palettes.len()
        );
        println!(
            "  HD zoom mapping matches {:.3}% of window pixels; camera within {:.2}, {:.2}, {:.2} \
             of the code's truncated x, y, height",
            share * 100.0,
            camera_error[0],
            camera_error[1],
            camera_error[2]
        );
        for (name, (bias, mean, near, high)) in [
            ("the frames' own camera", exact),
            ("the smooth camera", smooth),
        ] {
            println!(
                "  HD terrain silhouette with {name}: {mean:.2} rows from the original's on \
                 average ({bias:+.2} lower), {:.1}% within a row, 95th percentile {high:.1}",
                near * 100.0,
            );
        }
        println!(
            "  Mandelbrot fit: centre {:.4} {:+.4}i, {:.2} map units a unit, aspect {:.4}; \
             DEP = {:.1} {:+.1} sqrt(G); plateau {} IoU {:.3}; slopes rms {:.2}, r {:.3}",
            fit.centre[0],
            fit.centre[1],
            fit.scale,
            fit.aspect,
            fit.base,
            fit.slope,
            fit.plateau,
            fit.iou,
            fit.rms,
            fit.correlation
        );
    }

    /// The top of the HD terrain in source column `column` (continuous), with
    /// a camera (map x and y, height, sine, cosine) and projection q:
    /// the highest row any terrain point on its ray shows at, as
    /// terrain_march's coverage test finds it, by marching the column a
    /// quarter map unit at a time; `None` if nothing shows on screen.
    fn silhouette_row(field: &FieldPreview, camera: [f64; 5], q: f64, column: f64) -> Option<f64> {
        let [wx, wy, height, sine, cosine] = camera;
        let (f, r) = (
            [sine / 256.0, cosine / 256.0],
            [cosine / 256.0, -sine / 256.0],
        );
        let p = (column - 161.0) / 256.0;
        let dir = [f[0] + p * r[0], f[1] + p * r[1]];
        let step = 0.25 / dir[0].hypot(dir[1]);
        let sample = |m: [f64; 2]| -> f64 {
            // Bilinear between cell centres, as the texture sampler does.
            let at = |x: i64, y: i64| -> f64 {
                let (x, y) = (x.clamp(0, 199) as usize, y.clamp(0, 199) as usize);
                field.heights[x * MAP_SIZE + y] as f64
            };
            if m[0] < 0.0 || m[1] < 0.0 || m[0] > 200.0 || m[1] > 200.0 {
                return 0.0;
            }
            let (u, v) = (m[0] - 0.5, m[1] - 0.5);
            let (i, k) = (u.floor() as i64, v.floor() as i64);
            let (a, b) = (u - i as f64, v - k as f64);
            (at(i, k) * (1.0 - b) + at(i, k + 1) * b) * (1.0 - a)
                + (at(i + 1, k) * (1.0 - b) + at(i + 1, k + 1) * b) * a
        };
        let mut top = f64::INFINITY;
        let mut z = 64.0;
        while z < 512.0 {
            let h = sample([wx + dir[0] * z, wy + dir[1] * z]);
            if h >= 0.5 {
                let projection = q * 256.0 / z + 512.0 - 2.0 * q;
                top = top.min((height - h) * projection / 1024.0);
            }
            z += step;
        }
        (top < 200.0).then_some(top.max(1.0))
    }
}
