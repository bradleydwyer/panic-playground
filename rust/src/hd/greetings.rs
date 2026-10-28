//! Part 7 (Greetings) for the HD preview: the state of every part-7 frame
//! read from the port's `Greetings` scene, from its first frame (black,
//! the stars' colours not yet lit) to part 8's first frame (the monster),
//! the original frames rebuilt from that state on demand, and the tables the
//! faithful and modern views draw from (greetings_render.rs and the
//! greetings_*.wgsl files). Part 4's preview ([`PlasmaPreview`]) ends on the
//! same first frame this one begins with.
//!
//! What comes from where:
//! - Read from the port's code and counters (`Greetings::hd_state`,
//!   `hd_drawn` and `hd_tables`): the five strips' row descriptors as each
//!   step drew them and the sine positions it drew them with, so every
//!   strip row's source in video memory; the star colours' phase and
//!   intensity; the stages (the greetings, the feedback fade to white, the
//!   monster from white) and the fade's levels; the texts, the sine table and
//!   MONSTER.U and its palette. The TRUG maps (the sky and the glyphs) are
//!   read from video memory as the scene loads them.
//! - Observed when the preview loads: the times of the stages and of the
//!   words ([`GreetingsEvents`]) and part 8's first frame.
//! - Interpretation: the letters as cubes (see greetings_type.rs); the twist
//!   as each row of a cube turned by 5.625 degrees for each step of the
//!   sine's view index, continuous rather than in 16 views; the star colours
//!   as a light turning round (160, 85): the stars' indices run once round
//!   it (a fit), so the colour cycle sweeps a band of light round once every
//!   64 frames; the feedback fade's progress as one blend toward white,
//!   read from colour 0, black before it, which each step moves toward
//!   white with every other colour (`GreetingsFrame::white`); interpolation
//!   between the 70 Hz frames.
//! - Authored: everything the modern view adds (greetings_light.wgsl and the
//!   values marked authored here): the depths, the camera, the dust motes'
//!   depths and drift and the sky beyond the original's 320 columns, the
//!   beam and the air, the cubes' material, the gaps the modern view leaves
//!   open.
use super::greetings_type::{
    GLYPH_BASE, GLYPH_ROW, GLYPH_STRIDE, LETTERS, LetterSource, Letterforms, letterforms,
};
use super::plasma::PlasmaPreview;
use super::slab_breakup::Breakup;
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::{
        ending::Ending,
        greetings::{Greetings, HdGreetings, HdGreetingsTables},
    },
    vga::Vga,
};
use bytemuck::{Pod, Zeroable};

/// `u.state.x` during part 7. Part 6 is 10, part 5 11, part 3 12, part 4 13.
pub const PART7_PHASE: f32 = 14.0;
/// The strips: their left columns, width and the screen's rows (read from
/// the code: destination `row * 80 + 2 + strip * 16` plane bytes, 7 of
/// them).
pub const STRIPS: usize = 5;
pub const STRIP_X: [usize; STRIPS] = [8, 72, 136, 200, 264];
pub const STRIP_WIDTH: usize = 28;
pub const ROWS: usize = 200;
/// A descriptor at or above this copies the sky beside the strip (equal) or
/// leaves the row alone (above); a character's slot is 18 rows, 15 of them
/// its glyph (read from the code).
pub const BLANK: u16 = 0xf490;
pub const SLOT_ROWS: i32 = 18;
pub const GLYPH_ROWS: i32 = 15;
/// Letter slots the modern view keeps per strip and frame: enough for the
/// screen's 200 rows and one entering from below.
pub const SLOTS: usize = 16;
/// The stars' colours (read from the code): entries 192 to 255.
pub const STAR_BASE: u8 = 192;
/// The modern view's depths (authored, in source rows on the 4:3 display):
/// the camera's distance from the strips' plane, and the nearest and
/// farthest the dust lies (scene z, the strips' plane 0).
pub const CAMERA_DISTANCE: f64 = 580.0;
pub const DUST_NEAR: f64 = -340.0;
pub const DUST_FAR: f64 = 230.0;
/// The centre the stars' indices turn round (fitted to the data: the
/// brightness centroid of the stored stars under the real palette turns
/// 5.65 degrees a frame round it, residual 3.7 degrees).
pub const SWEEP_CENTRE: (f64, f64) = (160.0, 85.0);
/// The modern view's beam (authored): the engine frames a turn takes by
/// default (twice the original's 64-frame cycle, about 1.83 s), and its
/// tilt toward the camera by default and at most (degrees, the default
/// chosen by eye; the beam's soft edge ends about 43 degrees from its axis,
/// so at 51 the lens catches its faint outer edge on each turn, though never
/// its body; at the most, 90, it points straight at the camera). Its brightness (times the shader's own; the lamp's glow in the
/// air is left as it is), chosen by eye: it starts at BEAM_BRIGHTNESS (by
/// default; tuned live within BEAM_BRIGHTNESS_RANGE) and from
/// BEAM_RAMP_FROM (clip seconds) rises linearly to BEAM_BRIGHTNESS_END by
/// part 8's first frame, through the swing into the lens. Over the same
/// stretch its turn shortens linearly, from BEAM_PERIOD (tuned live) to
/// BEAM_PERIOD_END engine frames, also chosen by eye.
pub const BEAM_PERIOD: f64 = 128.0;
pub const BEAM_PERIOD_END: f64 = 32.0;
/// The modern view's beam's width (authored, tuned live with Z/X, like a
/// spotlight's focus): times the shader's own spread, by default and at
/// least and at most (at none, a shaft as wide as the lamp's aperture).
pub const BEAM_WIDTH: f64 = 1.0;
pub const BEAM_WIDTH_RANGE: (f64, f64) = (0.0, 2.0);
pub const BEAM_TILT: f64 = 51.0;
pub const BEAM_TILT_MAX: f64 = 90.0;
pub const BEAM_BRIGHTNESS: f64 = 0.25;
pub const BEAM_BRIGHTNESS_RANGE: (f64, f64) = (0.05, 4.0);
pub const BEAM_RAMP_FROM: f64 = 237.0;
pub const BEAM_BRIGHTNESS_END: f64 = 1.0;
/// Each stored star becomes this many motes (authored).
pub const MOTES_PER_STAR: usize = 6;

/// What a part-7 frame shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Stage {
    /// Part 7's first frame: the TRUG maps loaded, the stars' colours black.
    Setup = 0,
    /// A greetings frame (the scene's stage 0).
    Words = 1,
    /// The feedback fade to white (stage 1).
    White = 2,
    /// MONSTER.U from white (stage 2, and the step that ends it).
    Monster = 3,
    /// Part 8's first frame: the monster, held.
    Part8 = 4,
}

/// One part-7 frame as the scanout shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GreetingsFrame {
    pub stage: Stage,
    /// The scene's counters after the step that drew this frame.
    pub state: HdGreetings,
    /// The star colours shown: the phase (unwrapped, rising one a frame)
    /// and the intensity (0 to 64). The palette a step shows was filled in
    /// the step before, with the phase that step set and the intensity
    /// before it rose.
    pub phase: f64,
    pub intensity: f64,
    /// How far the feedback fade has gone toward white (0 to 1: colour 0,
    /// black before the fade, as the frame shows it, over 63), and the
    /// monster's level (0 white to 1).
    pub white: f64,
    pub monster: f64,
    /// The sine positions the strips were drawn with (the last drawn, held
    /// through the fades), and whether this frame's step drew them.
    pub positions: [usize; STRIPS],
    pub drew: bool,
}

/// Times in part 7, in seconds from its first frame, observed from the
/// frames when the preview loads; -1 for none.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GreetingsEvents {
    /// The stars at full intensity.
    pub lit: f64,
    /// The first letter row drawn, and the first frame after the last.
    pub words: f64,
    pub words_end: f64,
    /// The feedback fade, the monster, part 8's first frame.
    pub white: f64,
    pub monster: f64,
    pub part8: f64,
}

/// The part-7 controls every part-7 shader reads, `u.greetings`.
/// renderer::CONTROLS_WGSL declares the same layout; keep the two in step
/// (the tests check them). All zero before part 7.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct GreetingsBlock {
    /// Shown (1), seconds since part 7's first frame, stage ([`Stage`]),
    /// part-7 frame (continuous).
    pub part: [f32; 4],
    /// The frame shown, the next, the fraction of the way to it, and
    /// whether the strips move toward it (1).
    pub frame: [f32; 4],
    /// The star colours: phase (continuous), intensity (0 to 1); the white
    /// fade (0 to 1), the monster's level (0 to 1).
    pub sweep: [f32; 4],
    /// The strips' sine positions (continuous), strips 0 to 3; strip 4's,
    /// the first slot's top row (continuous), the star colours' phase at
    /// the fade to white, unused.
    pub twist: [f32; 4],
    pub twist_late: [f32; 4],
    /// Part-7 seconds of [`GreetingsEvents`]: lit, words, words' end,
    /// white; monster, part 8, unused, unused.
    pub events: [f32; 4],
    pub events_late: [f32; 4],
    /// The modern view's beam (authored, tuned live): its phase (in the
    /// original's colour steps, 64 a turn), its tilt toward the camera
    /// (radians), its brightness, and how fast its phase moves against the
    /// original's colours.
    pub beam: [f32; 4],
    /// The beam's width (times the shader's own spread), the row the lamp
    /// shows on (the sweep's centre, or the frame's middle with O in the
    /// preview), unused, unused.
    pub beam_late: [f32; 4],
}

/// Part 7 of the HD clip: from its first frame to part 8's first.
pub struct GreetingsPreview {
    /// Clip frame of part 7's first frame. Part-7 frame `j` is clip frame
    /// `start + j`; the same frame is the last of part 4's preview.
    pub start: usize,
    pub frames: Vec<GreetingsFrame>,
    /// The palette each frame shows, 768 6-bit DAC values each.
    pub palettes: Vec<u8>,
    /// Each strip row's source in video memory as each frame shows it:
    /// `frame * 1000 + strip * 200 + row`, the address of the 7 plane bytes
    /// (28 pixels) the row shows. The scene copies a glyph row (the
    /// descriptor plus the sine's offset) or the sky beside the strip
    /// (destination + 7), or leaves the row as it was; before the first
    /// copy it shows the sky beneath it.
    pub shown: Vec<u16>,
    /// The letters the modern view stands in each strip: per frame, the
    /// first slot's top row, and per strip and slot the letter (0 = A) or
    /// 255; the next frame's new letters are included, a row lower, so they
    /// rise into view rather than appear.
    pub slots: Vec<(i32, [[u8; SLOTS]; STRIPS])>,
    /// The TRUG maps as video memory holds them, 4 planes of 65536 bytes.
    pub planes: Vec<u8>,
    pub tables: HdGreetingsTables,
    /// The music's position at each frame but the last, as the step that
    /// follows it read it.
    pub music: Vec<u16>,
    pub events: GreetingsEvents,
    /// The letters' faces for the modern view, from the authored outlines
    /// (greetings_type.rs).
    pub letterforms: Letterforms,
    /// The modern view's dust; see [`motes`].
    pub motes: Vec<[f32; 4]>,
    /// Part 4's slab breaking up into that dust, each grain settling as a
    /// mote ([`super::slab_breakup`]).
    pub breakup: Breakup,
    /// The star colours' phase (unwrapped) on the first frame of the fade
    /// to white, which the modern view's slowed beam is anchored to.
    pub white_phase: f64,
    /// The modern view's beam (authored, tuned live with J/K, U/I and G/H):
    /// the engine frames a turn takes (the original's colours cycle in 64),
    /// its tilt toward the camera in degrees, and its brightness, the turn
    /// and the brightness as they are before they change toward the end of
    /// the part.
    pub beam_period: f64,
    pub beam_tilt: f64,
    pub beam_brightness: f64,
    /// The beam's width (tuned live with Z/X), times the shader's spread.
    pub beam_width: f64,
    /// Whether the lamp shows on the frame's middle row rather than on the
    /// sweep's centre, 15 rows above it (toggled live with O; authored).
    pub lamp_centred: bool,
}

impl GreetingsPreview {
    /// Continue the clip from part 4's preview, which ends on part 7's first
    /// frame: make part 7 again as the engine made it and step it with the
    /// music until it reports done; the engine makes part 8 in that step,
    /// and its first frame ends the clip. The PCM of every step is appended
    /// but for that last frame's.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        plasma: &PlasmaPreview,
    ) -> Result<Self, String> {
        let start = plasma.start + plasma.frames.len() - 1;
        let mut video = vga.clone();
        let mut scene = Greetings::new(data, &mut video)?;
        if video.indices != plasma.part7.0 || video.palette != plasma.part7.1 {
            return Err("Part 7 did not begin where part 4's preview ended".into());
        }
        let tables = scene.hd_tables();
        let planes: Vec<u8> = video.planes.iter().flatten().copied().collect();
        let render = |pcm: &mut Vec<i16>, music: &mut Music, j: usize| {
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
        };
        // Before any copy each strip row shows the sky beneath it.
        let mut current: [[u16; ROWS]; STRIPS] =
            std::array::from_fn(|s| std::array::from_fn(|y| destination(s, y) as u16));
        let mut frames: Vec<GreetingsFrame> = Vec::new();
        let mut drawn_rows: Vec<Option<[[u16; ROWS]; STRIPS]>> = Vec::new();
        let mut states = vec![scene.hd_state()];
        let mut palettes = Vec::new();
        let mut shown = Vec::new();
        let mut positions = Vec::new();
        // The phase after each step, unwrapped.
        let mut unwrapped = vec![0.0];
        let mut last_positions = [0; STRIPS];
        loop {
            let j = frames.len();
            if j > FRAME_RATE as usize * 60 {
                return Err("Part 7 exceeded its one-minute import limit".into());
            }
            let state = *states.last().unwrap();
            let drawn = if j == 0 { None } else { scene.hd_drawn() };
            if let Some(d) = &drawn {
                for (s, strip) in current.iter_mut().enumerate() {
                    for (y, source) in strip.iter_mut().enumerate() {
                        let descriptor = d.rows[s][y];
                        if descriptor < BLANK {
                            let variant = tables.sine[d.positions[s] + y] as u16;
                            *source = descriptor.wrapping_add(variant);
                        } else if descriptor == BLANK {
                            *source = (destination(s, y) + 7) as u16;
                        }
                    }
                }
                last_positions = d.positions;
            }
            // The step that ends the fade to white sets stage 2 with the
            // level at 0; the monster is loaded by the step after it.
            let stage = match (j, state.stage, state.fade_level) {
                (0, _, _) => Stage::Setup,
                (_, 0, _) => Stage::Words,
                (_, 1, _) | (_, 2, 0) => Stage::White,
                _ => Stage::Monster,
            };
            // The palette shown was filled the step before, with the phase
            // that step set (the previous frame's) and the intensity from
            // before it rose (the frame before that's).
            let shown_phase = if j == 0 { 0.0 } else { unwrapped[j - 1] };
            let intensity = if j >= 2 {
                states[j - 2].intensity as f64
            } else {
                0.0
            };
            let white = if stage == Stage::White {
                video.palette[0] as f64 / 63.0
            } else {
                0.0
            };
            let monster = match (stage, state.stage) {
                (Stage::Monster, 3) => 1.0,
                (Stage::Monster, _) => (state.fade_level as f64 - 1.0) / 64.0,
                _ => 0.0,
            };
            frames.push(GreetingsFrame {
                stage,
                state,
                phase: shown_phase,
                intensity: if stage == Stage::White {
                    64.0
                } else {
                    intensity
                },
                white,
                monster,
                positions: last_positions,
                drew: drawn.is_some(),
            });
            drawn_rows.push(drawn.map(|d| d.rows));
            palettes.extend_from_slice(&video.palette);
            for row in &current {
                shown.extend_from_slice(row);
            }
            let position = music.position();
            positions.push(position);
            let done = scene.step(&mut video, position);
            let after = scene.hd_state();
            let turned = (after.phase as f64 - state.phase as f64).rem_euclid(64.0);
            unwrapped.push(unwrapped[j] + turned);
            states.push(after);
            render(pcm, music, j);
            if done {
                // The engine makes part 8 in this step: its first frame,
                // the monster held, comes next.
                Ending::new(data, &mut video)?;
                break;
            }
        }
        let last = *frames.last().ok_or("Part 7 drew nothing")?;
        frames.push(GreetingsFrame {
            stage: Stage::Part8,
            state: *states.last().unwrap(),
            monster: 1.0,
            drew: false,
            ..last
        });
        drawn_rows.push(None);
        palettes.extend_from_slice(&video.palette);
        shown.extend_from_within(shown.len() - STRIPS * ROWS..);
        *vga = video;
        let slots = slots(&frames, &drawn_rows);
        let events = GreetingsEvents::observe(&frames, &slots);
        let white_phase = frames
            .iter()
            .find(|f| f.stage == Stage::White)
            .map_or(0.0, |f| f.phase);
        let dust = motes(&planes);
        Ok(Self {
            white_phase,
            beam_period: BEAM_PERIOD,
            beam_tilt: BEAM_TILT,
            beam_brightness: BEAM_BRIGHTNESS,
            beam_width: BEAM_WIDTH,
            lamp_centred: true,
            start,
            letterforms: letterforms(LetterSource::Outlines(LETTERS))?,
            breakup: Breakup::new(&plasma.stone, &dust),
            motes: dust,
            frames,
            palettes,
            shown,
            slots,
            planes,
            tables,
            music: positions,
            events,
        })
    }
}

/// The plane byte offset strip `s` writes row `y` to (read from the code).
pub fn destination(s: usize, y: usize) -> usize {
    y * 80 + 2 + s * 16
}

/// The letters in each strip on each frame, by slot: from the glyph rows
/// the frame's step drew (glyph `(d - 0x3e90) / 0x700`, its row the
/// remainder over 112), each letter's top row; all letters share the
/// slots' grid, 18 rows apart.
fn slots(
    frames: &[GreetingsFrame],
    drawn: &[Option<[[u16; ROWS]; STRIPS]>],
) -> Vec<(i32, [[u8; SLOTS]; STRIPS])> {
    // Each frame's letters as (strip, top, glyph).
    let letters: Vec<Vec<(usize, i32, u8)>> = drawn
        .iter()
        .map(|d| {
            let mut found = Vec::new();
            if let Some(rows) = d {
                for (s, strip) in rows.iter().enumerate() {
                    for (y, &d) in strip.iter().enumerate() {
                        if d >= BLANK || (d as usize) < GLYPH_BASE {
                            continue;
                        }
                        let offset = d as usize - GLYPH_BASE;
                        let (g, row) = (offset / GLYPH_STRIDE, (offset % GLYPH_STRIDE) / GLYPH_ROW);
                        if g < 26 {
                            let top = y as i32 - row as i32;
                            if !found.contains(&(s, top, g as u8)) {
                                found.push((s, top, g as u8));
                            }
                        }
                    }
                }
            }
            found
        })
        .collect();
    let mut out = Vec::with_capacity(frames.len());
    for j in 0..frames.len() {
        // The slot grid: the row the glyph entering this frame starts on is
        // 199 less the slot's progress, (frame - 1) mod 18.
        let k = frames[j].state.frame.max(1) as i32;
        let entering = 199 - (k - 1) % SLOT_ROWS;
        // The first slot's top, -17 to 0.
        let base = match entering.rem_euclid(SLOT_ROWS) {
            0 => 0,
            r => r - SLOT_ROWS,
        };
        let mut table = [[255u8; SLOTS]; STRIPS];
        let mut place = |s: usize, top: i32, g: u8| {
            let slot = (top - base).div_euclid(SLOT_ROWS);
            if (top - base).rem_euclid(SLOT_ROWS) == 0 && (0..SLOTS as i32).contains(&slot) {
                table[s][slot as usize] = g;
            }
        };
        for &(s, top, g) in &letters[j] {
            place(s, top, g);
        }
        if frames.get(j + 1).is_some_and(|f| f.drew) && frames[j].drew {
            for &(s, top, g) in &letters[j + 1] {
                place(s, top + 1, g);
            }
        }
        out.push((base, table));
    }
    out
}

impl GreetingsEvents {
    /// Read the events from the frames.
    pub fn observe(frames: &[GreetingsFrame], slots: &[(i32, [[u8; SLOTS]; STRIPS])]) -> Self {
        let at = |j: Option<usize>| j.map_or(-1.0, |j| j as f64 / FRAME_RATE as f64);
        let first = |test: &dyn Fn(usize) -> bool| (0..frames.len()).find(|&j| test(j));
        let words = first(&|j| drawn_letter(slots, frames, j));
        let words_end = words.and_then(|w| {
            let last = (w..frames.len())
                .rev()
                .find(|&j| drawn_letter(slots, frames, j))?;
            Some(last + 1)
        });
        Self {
            lit: at(first(&|j| frames[j].intensity >= 64.0)),
            words: at(words),
            words_end: at(words_end),
            white: at(first(&|j| frames[j].stage == Stage::White)),
            monster: at(first(&|j| frames[j].stage == Stage::Monster)),
            part8: at(Some(frames.len() - 1)),
        }
    }
}

/// Whether frame `j` shows a glyph row: a letter of its slots with a row
/// between 0 and 199 (the next frame's new letters, carried a row lower,
/// start at 200).
fn drawn_letter(
    slots: &[(i32, [[u8; SLOTS]; STRIPS])],
    frames: &[GreetingsFrame],
    j: usize,
) -> bool {
    let (base, table) = &slots[j];
    frames[j].drew
        && table.iter().any(|strip| {
            strip.iter().enumerate().any(|(k, &g)| {
                let top = base + SLOT_ROWS * k as i32;
                g != 255 && top < ROWS as i32 && top + GLYPH_ROWS > 0
            })
        })
}

/// A small deterministic generator for the authored dust (SplitMix64).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        let (a, b) = (self.next().max(1e-12), self.next());
        (-2.0 * a.ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos()
    }
}

/// The modern view's dust, from the sky the TRUG maps hold (read from the
/// data: each pixel with colour 192 to 255 is a star, its colour index 0 to
/// 63). Authored: each star becomes [`MOTES_PER_STAR`] motes in the air,
/// each on the camera's ray through a point within the star's pixel, at a
/// depth between [`DUST_NEAR`] and [`DUST_FAR`] (scene z: the strips' plane
/// is 0, the camera at minus [`CAMERA_DISTANCE`]), drawn so the motes fill
/// the air evenly, not more thickly near the camera; so at the moment the
/// air starts to drift they project where the original shows its stars.
/// Beyond the original's columns, for the wider frame, the outer 60 columns'
/// stars are mirrored across the edges, thinning away from them, their
/// indices from the fitted turn (the stars' index is about 17.8 less 64 /
/// 2 pi times their angle round the centre, scattered by about 12). Per
/// mote: across, up, depth (scene), and the star's colour index plus a size
/// between 0 and 1.
pub fn motes(planes: &[u8]) -> Vec<[f32; 4]> {
    let mut rng = Rng(0x5eed_0007);
    let mut stars = Vec::new();
    for y in 0..ROWS {
        for x in 0..320 {
            let c = planes[(x & 3) * 65536 + y * 80 + x / 4];
            if c >= STAR_BASE {
                stars.push((x as f64 + 0.5, y as f64 + 0.5, (c - STAR_BASE) as f64));
            }
        }
    }
    let mut extra = Vec::new();
    for &(x, y, _) in &stars {
        let (mirrored, beyond) = if x < 60.0 {
            (-x, x)
        } else if x >= 260.0 {
            (640.0 - x, 320.0 - x)
        } else {
            continue;
        };
        if rng.next() < (-beyond / 80.0).exp() {
            let (cx, cy) = SWEEP_CENTRE;
            let angle = (y - cy).atan2(mirrored - cx);
            let index = (-angle / (2.0 * std::f64::consts::PI) * 64.0 + 17.8 + 12.0 * rng.normal())
                .rem_euclid(64.0)
                .floor();
            extra.push((mirrored, y, index));
        }
    }
    stars.extend(extra);
    // Distances from the camera, drawn with density in proportion to their
    // square, so the motes fill the frustum evenly.
    let (near, far) = (CAMERA_DISTANCE + DUST_NEAR, CAMERA_DISTANCE + DUST_FAR);
    let mut motes = Vec::with_capacity(stars.len() * MOTES_PER_STAR);
    for &(x, y, index) in &stars {
        for _ in 0..MOTES_PER_STAR {
            let sx = x + rng.next() - 0.5;
            let sy = y + rng.next() - 0.5;
            let u = rng.next();
            let distance = (near.powi(3) + u * (far.powi(3) - near.powi(3))).cbrt();
            let scale = distance / CAMERA_DISTANCE;
            let size = rng.next().powi(2);
            motes.push([
                ((sx - 160.0) * 5.0 / 6.0 * scale) as f32,
                ((100.0 - sy) * scale) as f32,
                (distance - CAMERA_DISTANCE) as f32,
                (index + size.min(0.999)) as f32,
            ]);
        }
    }
    motes
}

impl GreetingsPreview {
    /// Part-7 frame position at a clip time: continuous, negative before
    /// part 7. A time within one audio sample of a frame's is that frame's.
    pub fn position(&self, time: f64) -> f64 {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        if (position - position.round()).abs() <= FRAME_RATE as f64 / SAMPLE_RATE as f64 {
            position.round()
        } else {
            position
        }
    }

    /// Part-7 frame at a clip time, or `None` before part 7. Times past the
    /// end hold the last frame.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = self.position(time);
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Clip time of part-7 frame `j`.
    pub fn seconds(&self, j: usize) -> f64 {
        (self.start + j) as f64 / FRAME_RATE as f64
    }

    fn moment(&self, time: f64) -> (usize, f64) {
        let last = self.frames.len() - 1;
        let position = self.position(time).clamp(0.0, last as f64);
        let j = position.floor() as usize;
        (j, if j == last { 0.0 } else { position - j as f64 })
    }

    /// Strip `s`'s row `y` source address as frame `j` shows it.
    pub fn shown_at(&self, j: usize, s: usize, y: usize) -> usize {
        self.shown[j * STRIPS * ROWS + s * ROWS + y] as usize
    }

    /// The indices of frame `j` as the scanout shows it, 320 x 200: the sky
    /// with the strips' rows from their sources, or the monster.
    pub fn indices(&self, j: usize) -> Vec<u8> {
        if self.frames[j].stage >= Stage::Monster {
            return self.tables.monster[..320 * ROWS].to_vec();
        }
        let plane = |p: usize, a: usize| self.planes[p * 65536 + (a & 65535)];
        let mut indices: Vec<u8> = (0..320 * ROWS)
            .map(|i| {
                let (x, y) = (i % 320, i / 320);
                plane(x & 3, y * 80 + x / 4)
            })
            .collect();
        for (s, &x0) in STRIP_X.iter().enumerate() {
            for y in 0..ROWS {
                let source = self.shown_at(j, s, y);
                for k in 0..STRIP_WIDTH {
                    indices[y * 320 + x0 + k] = plane(k & 3, source + k / 4);
                }
            }
        }
        indices
    }

    /// The palette frame `j` shows.
    pub fn palette(&self, j: usize) -> &[u8] {
        &self.palettes[j * 768..][..768]
    }

    /// The original frame `j` as 8-bit RGBA, 320 x 200.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let j = j.min(self.frames.len() - 1);
        super::field::rgba(&self.indices(j), self.palette(j))
    }

    /// Title label for part-7 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        let frame = &self.frames[j.min(self.frames.len() - 1)];
        let e = &self.events;
        let t = j as f64 / FRAME_RATE as f64;
        let after = |event: f64| event >= 0.0 && t >= event - 1e-9;
        match frame.stage {
            Stage::Setup => "Part 7: stars from black",
            Stage::Words => {
                if after(e.words_end) {
                    "Stars, the words gone"
                } else if after(e.words) {
                    "Greetings rising"
                } else if after(e.lit) {
                    "Stars"
                } else {
                    "Part 7: stars from black"
                }
            }
            Stage::White => "Fade to white",
            Stage::Monster => "Monster from white",
            Stage::Part8 => "Part 8: first frame",
        }
    }

    /// The original's colour phase (unwrapped) at a clip time.
    fn phase_at(&self, time: f64) -> f64 {
        let (j, f) = self.moment(time);
        let k = (j + 1).min(self.frames.len() - 1);
        let g = if k != j { f } else { 0.0 };
        self.frames[j].phase + (self.frames[k].phase - self.frames[j].phase) * g
    }

    /// The modern view's beam's phase (in the original's colour steps, 64 a
    /// turn) when the original's is `q`, and how fast it moves against it.
    /// It turns `k` times slower than the original's colours: `k` is
    /// beam_period / 64 until BEAM_RAMP_FROM, then falls linearly with the
    /// original's phase (so with time) to BEAM_PERIOD_END / 64 by part 8's
    /// first frame. The beam's phase is the integral of 1 / `k`, equal to
    /// the original's on the first frame of the fade to white, so the swing
    /// into the lens lands where the original's band is then.
    fn beam_turn(&self, q: f64) -> (f64, f64) {
        let (k0, k1) = (self.beam_period / 64.0, BEAM_PERIOD_END / 64.0);
        let q0 = self.phase_at(BEAM_RAMP_FROM);
        let q1 = self
            .phase_at(self.seconds(0) + self.events.part8)
            .max(q0 + 1e-6);
        let slope = (k1 - k0) / (q1 - q0);
        let k = |x: f64| k0 + slope * (x.clamp(q0, q1) - q0);
        // From q0 to x.
        let ramp = |x: f64| {
            if slope.abs() < 1e-12 {
                (x - q0) / k0
            } else {
                (k(x) / k0).ln() / slope
            }
        };
        let turned = |x: f64| {
            if x <= q0 {
                (x - q0) / k0
            } else if x <= q1 {
                ramp(x)
            } else {
                ramp(q1) + (x - q1) / k1
            }
        };
        (
            self.white_phase + turned(q) - turned(self.white_phase),
            1.0 / k(q),
        )
    }

    /// Part 7's controls while part 4's slab breaks up into its dust
    /// ([`super::slab_breakup`]), which starts `ahead` seconds before part
    /// 7's first frame (authored): its light is there from then, the
    /// original's fade in from black played `ahead` seconds early, and
    /// before its first frame its colours' phase, and with it the beam,
    /// turns on at the pace of its first second.
    pub fn breaking_controls(&self, time: f64, ahead: f64) -> GreetingsBlock {
        let mut block = self.controls(time);
        block.sweep[1] = block.sweep[1].max(self.controls(time + ahead).sweep[1]);
        let start = self.seconds(0);
        if time < start {
            let span = (FRAME_RATE as usize).min(self.frames.len() - 1);
            let pace = (self.frames[span].phase - self.frames[0].phase) / span as f64;
            let phase = self.frames[0].phase - (start - time) * FRAME_RATE as f64 * pace;
            let (beam_phase, beam_rate) = self.beam_turn(phase);
            block.sweep[0] = phase as f32;
            block.beam[0] = beam_phase as f32;
            block.beam[3] = beam_rate as f32;
        }
        block
    }

    /// Everything part 7 puts in `u.greetings` at a clip time.
    pub fn controls(&self, time: f64) -> GreetingsBlock {
        let (j, f) = self.moment(time);
        let frame = &self.frames[j];
        let last = self.frames.len() - 1;
        let k = (j + 1).min(last);
        let next = &self.frames[k];
        let moving = frame.drew && next.drew && k != j;
        let g = if k != j { f } else { 0.0 };
        let lerp = |a: f64, b: f64| a + (b - a) * g;
        let positions: [f64; STRIPS] =
            std::array::from_fn(|s| frame.positions[s] as f64 - if moving { f } else { 0.0 });
        let base = self.slots[j].0 as f64 - if moving { f } else { 0.0 };
        let e = &self.events;
        // The beam's brightness: held, then rising linearly from
        // BEAM_RAMP_FROM to BEAM_BRIGHTNESS_END by part 8's first frame.
        let end = self.seconds(0) + e.part8;
        let rise = ((time - BEAM_RAMP_FROM) / (end - BEAM_RAMP_FROM).max(1e-6)).clamp(0.0, 1.0);
        let brightness = self.beam_brightness + (BEAM_BRIGHTNESS_END - self.beam_brightness) * rise;
        let (beam_phase, beam_rate) = self.beam_turn(lerp(frame.phase, next.phase));
        GreetingsBlock {
            part: [
                1.0,
                (time - self.seconds(0)) as f32,
                frame.stage as u8 as f32,
                (j as f64 + f) as f32,
            ],
            frame: [j as f32, k as f32, g as f32, if moving { 1.0 } else { 0.0 }],
            sweep: [
                lerp(frame.phase, next.phase) as f32,
                (lerp(frame.intensity, next.intensity) / 64.0) as f32,
                lerp(frame.white, next.white) as f32,
                lerp(frame.monster, next.monster) as f32,
            ],
            twist: [
                positions[0] as f32,
                positions[1] as f32,
                positions[2] as f32,
                positions[3] as f32,
            ],
            twist_late: [
                positions[4] as f32,
                base as f32,
                self.white_phase as f32,
                0.0,
            ],
            events: [e.lit, e.words, e.words_end, e.white].map(|v| v as f32),
            events_late: [e.monster, e.part8, 0.0, 0.0].map(|v| v as f32),
            beam: [
                beam_phase as f32,
                self.beam_tilt.to_radians() as f32,
                brightness as f32,
                beam_rate as f32,
            ],
            beam_late: [
                self.beam_width as f32,
                if self.lamp_centred { 100.0 } else { SWEEP_CENTRE.1 as f32 },
                0.0,
                0.0,
            ],
        }
    }

    /// Whether the modern view's photons can move any light at the moment
    /// `block` ([`GreetingsPreview::controls`]) describes. Where they can't,
    /// greetings_light.wgsl's trace_photons leaves every photon at zero, so
    /// the caustics draw nothing and their map stays cleared, and the
    /// renderer skips both and only clears the map. The same tests as the
    /// shader's, on the same f32 values: the stage outside the words and the
    /// white, the stars dark (intensity() is then pow(0, 2.2), 0), or no
    /// letter in the frame shown (frame_now()'s slots), as photons only
    /// move light through the letters' blocks. Any other moment is traced,
    /// whether or not its photons meet a block.
    pub fn photons_can_move_light(&self, block: &GreetingsBlock) -> bool {
        let stage = block.part[2];
        if stage < Stage::Words as u8 as f32 - 0.5 || stage > Stage::White as u8 as f32 + 0.5 {
            return false;
        }
        // Only +0 for certain: a negative or NaN level is left to the shader.
        let level = block.sweep[1];
        if level == 0.0 && level.is_sign_positive() {
            return false;
        }
        let shown = block.frame[0] as usize;
        self.slots
            .get(shown)
            .is_none_or(|(_, strips)| strips.iter().flatten().any(|&letter| letter != 255))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    /// An invented part 7, starting at about the real part's clip time
    /// (225.43 s, observed): its first frame, a few greetings frames, the
    /// fade to white, the monster and part 8's first frame, with the beam
    /// as loaded.
    fn invented() -> GreetingsPreview {
        let mut stages = vec![Stage::Setup];
        stages.extend([Stage::Words; 12]);
        stages.extend([Stage::White, Stage::White, Stage::Monster, Stage::Part8]);
        let frames: Vec<GreetingsFrame> = stages
            .iter()
            .enumerate()
            .map(|(j, &stage)| GreetingsFrame {
                stage,
                state: HdGreetings::default(),
                phase: j as f64,
                intensity: 64.0,
                white: if stage == Stage::White { 0.5 } else { 0.0 },
                monster: if stage >= Stage::Monster { 1.0 } else { 0.0 },
                positions: [0; STRIPS],
                drew: stage == Stage::Words,
            })
            .collect();
        let n = frames.len();
        let at = |stage: Stage| {
            stages.iter().position(|&s| s == stage).unwrap() as f64 / FRAME_RATE as f64
        };
        GreetingsPreview {
            start: 15780,
            frames,
            palettes: vec![0; 768 * n],
            shown: vec![0; n * STRIPS * ROWS],
            slots: vec![(0, [[255; SLOTS]; STRIPS]); n],
            planes: vec![0; 4 * 65536],
            tables: HdGreetingsTables {
                sine: [0; 601],
                texts: std::array::from_fn(|_| Vec::new()),
                monster: vec![0; 64000],
                monster_palette: vec![0; 768],
            },
            music: Vec::new(),
            events: GreetingsEvents {
                lit: at(Stage::Words),
                words: at(Stage::Words),
                words_end: at(Stage::White),
                white: at(Stage::White),
                monster: at(Stage::Monster),
                part8: at(Stage::Part8),
            },
            letterforms: letterforms(LetterSource::Outlines(LETTERS)).unwrap(),
            motes: Vec::new(),
            breakup: Breakup::default(),
            white_phase: 13.0,
            beam_period: BEAM_PERIOD,
            beam_tilt: BEAM_TILT,
            beam_brightness: BEAM_BRIGHTNESS,
            beam_width: BEAM_WIDTH,
            lamp_centred: true,
        }
    }

    /// The row the controls put the lamp on (`beam_late.y`), at every
    /// quarter frame of the part.
    fn lamp_rows(g: &GreetingsPreview) -> Vec<f32> {
        (0..4 * g.frames.len())
            .map(|k| {
                g.controls(g.seconds(0) + k as f64 / (4.0 * FRAME_RATE as f64))
                    .beam_late[1]
            })
            .collect()
    }

    /// The star colours as the scene fills them (read from the code).
    fn stars(phase: usize, intensity: u16) -> Vec<u8> {
        (0..64)
            .flat_map(|i| {
                let j = (i + phase) & 63;
                let v = (3 * j.min(63 - j) as i32 - 33).max(0) as u16;
                [
                    (v * intensity / 64) as u8,
                    ((v * 8 / 9) * intensity / 64) as u8,
                    ((v * 7 / 9) * intensity / 64) as u8,
                ]
            })
            .collect()
    }

    /// The photons are traced only through the words and the white, while
    /// the stars are lit and the frame shown has a letter; the renderer
    /// skips them at every other moment of the invented part.
    #[test]
    fn photons_are_traced_only_where_they_can_move_light() {
        let mut p = invented();
        let traced =
            |p: &GreetingsPreview, j: usize| p.photons_can_move_light(&p.controls(p.seconds(j)));
        // No letters yet: nothing to trace anywhere.
        assert!((0..p.frames.len()).all(|j| !traced(&p, j)));
        // One letter in every frame: traced in the words and the white only.
        for (_, strips) in p.slots.iter_mut() {
            strips[2][7] = 4;
        }
        let stages: Vec<Stage> = p.frames.iter().map(|f| f.stage).collect();
        for (j, stage) in stages.iter().enumerate() {
            let expected = matches!(stage, Stage::Words | Stage::White);
            assert_eq!(traced(&p, j), expected, "frame {j}, {stage:?}");
        }
        // The stars dark: no light to move, letters or not.
        let words = stages.iter().position(|&s| s == Stage::Words).unwrap();
        p.frames[words].intensity = 0.0;
        p.frames[words + 1].intensity = 0.0;
        assert!(!traced(&p, words));
        assert!(traced(&p, words + 2));
        // Halfway to a lit frame the stars light again.
        let between = p.seconds(words + 1) + 0.5 / FRAME_RATE as f64;
        assert!(p.photons_can_move_light(&p.controls(between)));
        // It is the frame shown whose letters count, not the next.
        p.slots[words + 3].1 = [[255; SLOTS]; STRIPS];
        assert!(!traced(&p, words + 3));
        let into = p.seconds(words + 2) + 0.5 / FRAME_RATE as f64;
        assert!(p.photons_can_move_light(&p.controls(into)));
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part7_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let started = std::time::Instant::now();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let loaded = started.elapsed().as_secs_f64();
        let plasma = clip.plasma.as_ref().unwrap();
        let greetings = clip.greetings.as_ref().unwrap();
        assert_eq!(greetings.start, plasma.start + plasma.frames.len() - 1);
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in 0..greetings.start {
            pcm.extend(engine.step().unwrap());
        }
        let mut words = 0;
        for (j, frame) in greetings.frames.iter().enumerate() {
            let video = &engine.video;
            let part = if frame.stage == Stage::Part8 { 8 } else { 7 };
            assert_eq!(engine.part(), part, "part at frame {j}");
            assert!(greetings.indices(j) == video.indices, "pixels at frame {j}");
            assert!(greetings.palette(j) == &video.palette[..], "palette at {j}");
            // The phase and intensity read rebuild the star colours shown.
            if frame.stage == Stage::Words {
                let phase = frame.phase.rem_euclid(64.0) as usize;
                assert_eq!(
                    &video.palette[576..],
                    &stars(phase, frame.intensity as u16)[..],
                    "star colours at frame {j}"
                );
            }
            if frame.stage == Stage::White {
                // One blend toward white for every colour: c' = c + (63 -
                // c) * white, within the 6-bit truncation.
                assert!(frame.white >= 0.0 && frame.white <= 1.0);
            }
            words += frame.drew as usize;
            if j + 1 < greetings.frames.len() {
                pcm.extend(engine.step().unwrap());
            }
        }
        assert_eq!(words, 1100);
        assert_eq!(engine.part(), 8);
        assert!(pcm == clip.pcm, "PCM");
        let e = greetings.events;
        let t = |v: f64| v + greetings.seconds(0);
        println!(
            "Loaded in {loaded:.2}s. Part 7 at {:.3}s: lit {:.3}, words {:.3} to {:.3}, white {:.3}, \
             monster {:.3}, part 8 {:.3}; {} motes",
            greetings.seconds(0),
            t(e.lit),
            t(e.words),
            t(e.words_end),
            t(e.white),
            t(e.monster),
            t(e.part8),
            greetings.motes.len()
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part7s_letters_are_the_texts_in_order() {
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let greetings = clip.greetings.as_ref().unwrap();
        // Each strip's letters in the order they rise, as the slots hold
        // them, are its word; the words enter one after another.
        let mut words = vec![String::new(); STRIPS];
        let mut seen = vec![std::collections::BTreeMap::new(); STRIPS];
        let mut first = [usize::MAX; STRIPS];
        for (j, (base, table)) in greetings.slots.iter().enumerate() {
            for s in 0..STRIPS {
                for (k, &g) in table[s].iter().enumerate() {
                    if g == 255 {
                        continue;
                    }
                    // A letter's identity: the frame its top passes row 0.
                    let entered = j as i32 + base + SLOT_ROWS * k as i32;
                    seen[s].entry(entered).or_insert(g);
                    first[s] = first[s].min(j);
                }
            }
        }
        for s in 0..STRIPS {
            words[s] = seen[s].values().map(|&g| (b'A' + g) as char).collect();
        }
        assert_eq!(
            words,
            ["GREETINGS", "GOTO", "RENAISSANCE", "TRITON", "SONIC"]
        );
        assert!(first.windows(2).all(|w| w[0] < w[1]), "{first:?}");
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part7s_sections_start_where_its_labels_change() {
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let sections = clip.sections();
        let frame = 1.0 / FRAME_RATE as f64;
        assert!(sections.windows(2).all(|w| w[0] < w[1]));
        for &s in &sections[1..] {
            assert_ne!(clip.label(s), clip.label(s - frame));
        }
        let greetings = clip.greetings.as_ref().unwrap();
        let start = greetings.seconds(0);
        let part7: Vec<(f64, &str)> = sections
            .iter()
            .filter(|&&s| s >= start - 1e-9)
            .map(|&s| (s, clip.label(s)))
            .collect();
        for (s, label) in &part7 {
            println!("{s:8.3} {label}");
        }
        let labels: Vec<&str> = part7.iter().map(|p| p.1).collect();
        assert_eq!(
            labels,
            [
                "Part 7: stars from black",
                "Stars",
                "Greetings rising",
                "Stars, the words gone",
                "Fade to white",
                "Monster from white",
                "Part 8: first frame",
            ]
        );
        let e = greetings.events;
        for (k, event) in [
            0.0,
            e.lit,
            e.words,
            e.words_end,
            e.white,
            e.monster,
            e.part8,
        ]
        .iter()
        .enumerate()
        {
            assert!(
                (part7[k].0 - start - event).abs() < 1.0 / SAMPLE_RATE as f64,
                "{}",
                labels[k]
            );
        }
        // The clip ends on part 8's first frame, and part 4's preview hands
        // over on part 7's first.
        let last = greetings.frames.len() - 1;
        assert_eq!(greetings.frame_at(clip.duration()), Some(last));
        let plasma = clip.plasma.as_ref().unwrap();
        assert_eq!(
            greetings.frame_at(plasma.seconds(plasma.frames.len() - 1)),
            Some(0)
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part7s_beam_quickens_to_the_end_and_meets_the_original_at_the_fade() {
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let g = clip.greetings.as_ref().unwrap();
        let end = g.seconds(0) + g.events.part8;
        let period = |t: f64| 64.0 / g.controls(t).beam[3] as f64;
        // Held, then shortened to BEAM_PERIOD_END by part 8's first frame.
        assert!((period(BEAM_RAMP_FROM - 1.0) - BEAM_PERIOD).abs() < 1e-3);
        assert!((period(end) - BEAM_PERIOD_END).abs() < 1e-3);
        // Its phase moves on smoothly, never back and never by a jump.
        let step = 0.25 / FRAME_RATE as f64;
        let mut t = g.seconds(0);
        let mut last = g.controls(t).beam[0];
        while t < end {
            t += step;
            let phase = g.controls(t).beam[0];
            assert!(
                phase >= last - 1e-3 && phase - last < 1.0,
                "{t}: {last} -> {phase}"
            );
            last = phase;
        }
        // On the fade's first frame its direction is the original's.
        let white = g.seconds(0) + g.events.white;
        assert!((g.controls(white).beam[0] as f64 - g.white_phase).abs() < 1e-2);
    }

    /// The lamp shows on the frame's middle row, 100, when centred, and
    /// back on the sweep's centre (the point the stars' colours were fitted
    /// to turn round), 15 rows above, when the preview puts it back (O), at
    /// every moment of the part; nothing else the controls give changes
    /// with it.
    /// While part 4's slab breaks up, part 7's light runs `ahead` early,
    /// never below its own, from black at the slab's first breaking; and
    /// before its first frame its colours, and with them the beam, turn on
    /// at its first frames' pace, meeting its own at its first frame.
    #[test]
    fn part7s_light_comes_up_with_the_slabs_break_up() {
        let mut p = invented();
        for (j, f) in p.frames.iter_mut().enumerate() {
            f.intensity = (j as f64 * 8.0).min(64.0);
        }
        let start = p.seconds(0);
        let ahead = 5.0 / FRAME_RATE as f64;
        assert_eq!(p.breaking_controls(start - ahead, ahead).sweep[1], 0.0);
        for k in 0..40 {
            let t = start - ahead + k as f64 * 0.005;
            let (early, own) = (p.breaking_controls(t, ahead), p.controls(t));
            assert_eq!(early.sweep[1], p.controls(t + ahead).sweep[1], "{t}");
            assert!(early.sweep[1] >= own.sweep[1], "{t}");
        }
        let (at, own) = (p.breaking_controls(start, ahead), p.controls(start));
        assert_eq!(
            (at.sweep[0], at.beam[0], at.beam[3]),
            (own.sweep[0], own.beam[0], own.beam[3])
        );
        let before = p.breaking_controls(start - 1.0 / FRAME_RATE as f64, ahead);
        assert!((at.sweep[0] - before.sweep[0] - 1.0).abs() < 1e-5);
        assert!(before.beam[0] < at.beam[0]);
    }

    #[test]
    fn the_lamp_shows_on_the_frames_middle_or_back_on_the_sweeps_centre() {
        let mut g = invented();
        assert!(
            lamp_rows(&g).iter().all(|&r| r == 100.0),
            "{:?}",
            lamp_rows(&g)
        );
        let time = g.seconds(5) + 0.3 / FRAME_RATE as f64;
        let centred = g.controls(time);
        g.lamp_centred = false;
        assert!(lamp_rows(&g).iter().all(|&r| r == SWEEP_CENTRE.1 as f32));
        let mut put_back = g.controls(time);
        assert_eq!(centred.beam_late[1] - put_back.beam_late[1], 15.0);
        put_back.beam_late[1] = centred.beam_late[1];
        assert_eq!(put_back, centred);
    }

    /// As loaded, the lamp is centred: the controls put it on the frame's
    /// middle row throughout the part.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part7s_lamp_is_centred_by_default() {
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let g = clip.greetings.as_ref().unwrap();
        assert!(g.lamp_centred);
        for j in (0..g.frames.len()).step_by(7) {
            assert_eq!(g.controls(g.seconds(j)).beam_late[1], 100.0, "frame {j}");
        }
    }
}
