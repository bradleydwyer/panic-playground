//! Part 8 (the ending) for the HD preview: the state of every part-8 frame
//! read from the port's `Ending` scene, from its first frame (the monster
//! part 7 leaves on screen, held until the music reaches 35:00) to the demo's
//! last, the original frames, and the timeline the faithful and modern views
//! draw from (ending_render.rs and the ending_*.wgsl files). Part 7's
//! preview ([`GreetingsPreview`]) ends on the same first frame this one
//! begins with.
//!
//! What comes from where:
//! - Read from the port's code and counters (`Ending::hd_frame`,
//!   `ending::hd_compression` and `ending::hd_line_ends`): each frame's step
//!   since 35:00, so its stage; the vertical compression each step shows;
//!   the line's ends as each step erases them. Read from video memory: the
//!   monster as part 8 inherits it (planes 0 to 15,999), every frame's
//!   indices and palette (the lift toward white, the dot's level).
//! - Observed when the preview loads: the stages' times ([`EndingEvents`])
//!   and the columns of the line's row that hold any colour.
//! - Interpretation: the stages as a television switched off (the code names
//!   nothing; a white frame, the picture squeezed to a line, the line to a
//!   dot and the dot's pulse read that way); interpolation between the
//!   70 Hz frames.
//! - Authored: everything the modern view adds (ending_crt.wgsl,
//!   ending_set.wgsl and the values marked authored here): the bank of nine
//!   monitors and the camera's pull back from the centre one's face, the
//!   tubes' shape and scanlines on the original's 200 lines, the phosphor's
//!   glow and persistence, the gains that brighten the picture as it
//!   squeezes, the light the screens throw on the sets, the CRT coming on
//!   over the hold and the fade to black over the last frames.
use super::{clip::vga_linear, greetings::GreetingsPreview};
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::ending::{self, Ending},
    vga::Vga,
};
use bytemuck::{Pod, Zeroable};
use half::f16;

/// `u.state.x` during part 8. Part 7 is 14.
pub const PART8_PHASE: f32 = 15.0;
/// The line's row and the dot's column (read from the code: the last
/// compression copies source row 100 to screen row 100 and the copies after
/// it blank the rows round it; the dot is plane 0's byte 24040 shown from
/// 16000, row 100, column 160).
pub const LINE_ROW: usize = 100;
pub const DOT_COLUMN: usize = 160;
/// The compression the modern view gives the single line (authored: one
/// row of the 200).
pub const LINE_COMPRESSION: f64 = 200.0;
/// Authored timing, in 70 Hz part-8 frames: the CRT comes on over the
/// first CRT_RAMP frames of the hold (which lasts 284); the phosphor's
/// persistence, a fast decay (the picture, the line and the dot), a faster
/// one for the white (gone, as in the original, by the squeeze's second
/// frame) and a slow one (the spot's lingering glow); the ages of the
/// trail's taps; the fade to black over the last END_FADE frames.
pub const CRT_RAMP: f64 = 45.0;
pub const PERSISTENCE: f64 = 1.6;
pub const WHITE_PERSISTENCE: f64 = 0.5;
pub const AFTERGLOW: f64 = 24.0;
pub const TRAIL_AGES: [f64; 4] = [1.0, 2.0, 3.0, 4.5];
pub const END_FADE: f64 = 24.0;
/// The halation image's blur (source pixels; authored).
pub const HALATION_SIGMA: f64 = 2.5;
/// The cells each screen's light is gathered in for lighting the sets:
/// columns and rows of the picture (authored).
pub const CELLS: (usize, usize) = (4, 3);

/// The camera (authored), in scene units (source rows on the 4:3 display,
/// the centre set's face at the origin, its front toward +z). Its vertical
/// field of view (degrees): from 100 / tan(half of it) the face's 200 rows
/// fill the frame's height, as part 7's picture does, which is where it
/// starts. It draws back from there and never stops. Its speed, in the
/// logarithm of its distance a 70 Hz frame, is the sum of a bump over the
/// first BUMP frames (as 30 x^2 (1 - x)^2, covering BUMP_SPAN: slow at
/// first, so it holds close while the CRT comes on, fastest at mid hold,
/// fading by the switch-off) and a drift of DRIFT a frame that comes up
/// over the first DRIFT_RISE frames and carries on to the end. It turns a
/// little with the bump (TURN, degrees across and down). The wall
/// (ending_set.wgsl) has a set every SET_PITCH rows up.
pub const FIELD_OF_VIEW: f64 = 30.0;
pub const BUMP: f64 = 320.0;
pub const BUMP_SPAN: f64 = 1.48;
pub const DRIFT: f64 = 0.00145;
pub const DRIFT_RISE: f64 = 120.0;
pub const TURN: (f64, f64) = (2.0, 3.0);
pub const SET_PITCH: f64 = 300.0;

/// Where the camera is and what it looks at, at part-8 frame position
/// `tau` (continuous): its position, the point it looks at, and the
/// tangent of half its field of view.
pub fn camera(tau: f64) -> ([f64; 3], [f64; 3], f64) {
    let tan_half = (FIELD_OF_VIEW / 2.0).to_radians().tan();
    let first = 100.0 / tan_half;
    let x = (tau / BUMP).clamp(0.0, 1.0);
    let bump = x * x * x * (10.0 - 15.0 * x + 6.0 * x * x);
    let y = (tau / DRIFT_RISE).max(0.0);
    let drift = if y < 1.0 {
        DRIFT_RISE * (y * y * y - y * y * y * y / 2.0)
    } else {
        tau - DRIFT_RISE / 2.0
    };
    let distance = (first.ln() + BUMP_SPAN * bump + DRIFT * drift).exp();
    let (across, down) = (TURN.0.to_radians() * bump, TURN.1.to_radians() * bump);
    let direction = [
        across.sin() * down.cos(),
        down.sin(),
        across.cos() * down.cos(),
    ];
    (direction.map(|d| d * distance), [0.0; 3], tan_half)
}

/// What a part-8 frame shows, by the scene's step (read from the code).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Stage {
    /// Steps 0 and 1: the monster, held until 35:00 (step 1 shows it again).
    Hold = 0,
    /// Steps 2 and 3: colour 0 turned white behind the monster, then the
    /// whole screen white.
    White = 1,
    /// Steps 4 to 20: the picture squeezed toward row 100, lifted toward
    /// white.
    Collapse = 2,
    /// Steps 21 to 67: row 100 alone, erased from both ends.
    Line = 3,
    /// Steps 68 to 128: one pixel at (160, 100), its colour pulsing.
    Dot = 4,
    /// Steps 129 to 198: the dot at its last level until the demo ends.
    End = 5,
}

impl Stage {
    pub fn of(step: usize) -> Self {
        match step {
            0..=1 => Self::Hold,
            2..=3 => Self::White,
            4..=20 => Self::Collapse,
            21..=67 => Self::Line,
            68..=128 => Self::Dot,
            _ => Self::End,
        }
    }
}

/// One part-8 frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndingFrame {
    /// The scene's steps since 35:00 as the frame shows them.
    pub step: usize,
    pub stage: Stage,
    /// The frame's indices: an entry of [`EndingPreview::images`].
    pub image: usize,
}

/// Times in part 8, in seconds from its first frame, observed from the
/// frames when the preview loads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndingEvents {
    pub white: f64,
    pub collapse: f64,
    pub line: f64,
    pub dot: f64,
    pub end: f64,
    /// The demo's last frame.
    pub last: f64,
}

/// What the beam draws at a moment, from the scene's steps and palettes:
/// the timeline the modern view follows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Beam {
    /// Vertical compression: 1 for the whole picture, the original's 2 to
    /// 44, then [`LINE_COMPRESSION`] for the line.
    pub compression: f64,
    /// The picture (or its squeezed band) shown, 0 to 1; the line, 0 to 1.
    pub band: f64,
    pub line: f64,
    /// The line's first and last lit columns; `right < left` when none.
    pub left: f64,
    pub right: f64,
    /// Colour 0's level (0 to 1), white behind the monster at step 2; the
    /// white frame (step 3), 0 to 1.
    pub ground: f64,
    pub cover: f64,
    /// The dot's colour as the display shows it (linear), once it is the dot.
    pub dot: f64,
}

/// The part-8 controls every part-8 shader reads, `u.ending`.
/// renderer::CONTROLS_WGSL declares the same layout; keep the two in step
/// (the tests check them). All zero before part 8.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct EndingBlock {
    /// Shown (1), seconds since part 8's first frame, stage ([`Stage`]),
    /// part-8 frame (continuous).
    pub part: [f32; 4],
    /// The frame shown, the next, the fraction toward it, the shown frame's
    /// image.
    pub frame: [f32; 4],
    /// The CRT's presence (0 to 1, eased; authored), [`Beam::ground`],
    /// [`Beam::cover`], unused.
    pub tube: [f32; 4],
    /// [`Beam::compression`], [`Beam::band`], [`Beam::line`], the fade to
    /// black (1 to 0 over the last frames).
    pub beam: [f32; 4],
    /// [`Beam::left`] and [`Beam::right`]; the first and last columns of the
    /// line's row that hold any colour.
    pub ends: [f32; 4],
    /// The phosphor: the dot's fast glow and slow afterglow, the white's
    /// afterglow (linear), unused.
    pub glow: [f32; 4],
    /// The line's mean colour (linear), unused.
    pub colour: [f32; 4],
    /// The trail, at each of [`TRAIL_AGES`] before the moment: the band's
    /// half height (rows), how far the band and the line were shown (0 to
    /// 1), the line's ends. The shader weighs each by its age.
    pub trail_rows: [f32; 4],
    pub trail_band: [f32; 4],
    pub trail_line: [f32; 4],
    pub trail_left: [f32; 4],
    pub trail_right: [f32; 4],
    /// Part-8 seconds of [`EndingEvents`]: white, collapse, line, dot; end,
    /// last, unused, unused.
    pub events: [f32; 4],
    pub events_late: [f32; 4],
    /// The camera ([`camera`]): its position and the tangent of half its
    /// field of view; the point it looks at, unused.
    pub eye: [f32; 4],
    pub aim: [f32; 4],
}

/// Part 8 of the HD clip: from its first frame to the demo's last.
pub struct EndingPreview {
    /// Clip frame of part 8's first frame. Part-8 frame `j` is clip frame
    /// `start + j`; the same frame is the last of part 7's preview.
    pub start: usize,
    pub frames: Vec<EndingFrame>,
    /// The palette each frame shows, 768 6-bit DAC values each.
    pub palettes: Vec<u8>,
    /// The distinct pictures the frames show, 320 x 200 indices each.
    pub images: Vec<Vec<u8>>,
    /// MONSTER.U as part 8 inherits it in video memory, 320 x 200 indices.
    pub monster: Vec<u8>,
    /// The music's position at each frame but the last, as the step that
    /// follows it read it.
    pub music: Vec<u16>,
    pub events: EndingEvents,
    /// The first and last columns of the line's row with an index other
    /// than 0, and the row's mean colour under the line's palette (linear).
    pub content: (usize, usize),
    pub line_colour: [f64; 3],
    /// Each column's mean colour under the hold's palette (linear), for the
    /// squeezed band's trail.
    pub profile: Vec<[f32; 4]>,
    /// Per frame, the phosphor's state after it (authored persistence over
    /// the frames' values): the white's afterglow, the dot's fast glow, its
    /// slow afterglow.
    pub phosphor: Vec<[f64; 3]>,
    /// Per frame, the original picture's mean colour (linear) in each of
    /// [`CELLS`], left to right and top to bottom: the light each screen
    /// throws on the sets.
    pub cells: Vec<[f32; 4]>,
}

impl EndingPreview {
    /// Continue the clip from part 7's preview, which ends on part 8's first
    /// frame: make part 8 again as the engine made it and step it with the
    /// music until it reports done, which ends the demo; the frame that step
    /// drew is the last. The PCM of every step is appended.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        greetings: &GreetingsPreview,
    ) -> Result<Self, String> {
        let first = greetings.frames.len() - 1;
        let start = greetings.start + first;
        let mut video = vga.clone();
        let mut scene = Ending::new(data, &mut video)?;
        if video.indices != greetings.indices(first) || video.palette != greetings.palette(first) {
            return Err("Part 8 did not begin where part 7's preview ended".into());
        }
        let monster: Vec<u8> = (0..320 * 200)
            .map(|i| {
                let (x, y) = (i % 320, i / 320);
                video.planes[x & 3][y * 80 + x / 4]
            })
            .collect();
        let mut frames = Vec::new();
        let mut palettes = Vec::new();
        let mut images: Vec<Vec<u8>> = Vec::new();
        let mut positions = Vec::new();
        let mut push = |video: &Vga, step: usize, frames: &mut Vec<EndingFrame>| {
            if images.last() != Some(&video.indices) {
                images.push(video.indices.clone());
            }
            frames.push(EndingFrame {
                step,
                stage: Stage::of(step),
                image: images.len() - 1,
            });
            palettes.extend_from_slice(&video.palette);
        };
        loop {
            let j = frames.len();
            if j > FRAME_RATE as usize * 60 {
                return Err("Part 8 exceeded its one-minute import limit".into());
            }
            push(&video, scene.hd_frame(), &mut frames);
            let position = music.position();
            positions.push(position);
            let done = scene.step(&mut video, position);
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
            if done {
                break;
            }
        }
        // The engine finishes with this step: the demo ends on its frame.
        push(&video, scene.hd_frame(), &mut frames);
        *vga = video;
        let row = monster[LINE_ROW * 320..][..320].to_vec();
        let lit: Vec<usize> = (0..320).filter(|&x| row[x] != 0).collect();
        let content = (
            *lit.first().ok_or("Part 8's line row is empty")?,
            *lit.last().unwrap(),
        );
        let mut preview = Self {
            start,
            frames,
            palettes,
            images,
            monster,
            music: positions,
            events: EndingEvents::default(),
            content,
            line_colour: [0.0; 3],
            profile: Vec::new(),
            phosphor: Vec::new(),
            cells: Vec::new(),
        };
        let at = |stage: Stage| {
            let j = preview.frames.iter().position(|f| f.stage >= stage);
            j.map_or(-1.0, |j| j as f64 / FRAME_RATE as f64)
        };
        preview.events = EndingEvents {
            white: at(Stage::White),
            collapse: at(Stage::Collapse),
            line: at(Stage::Line),
            dot: at(Stage::Dot),
            end: at(Stage::End),
            last: (preview.frames.len() - 1) as f64 / FRAME_RATE as f64,
        };
        let line = preview
            .frames
            .iter()
            .position(|f| f.stage == Stage::Line)
            .ok_or("Part 8 never showed its line")?;
        let palette = preview.palette(line).to_vec();
        let mut sum = [0.0; 3];
        for &x in &lit {
            for (c, s) in sum.iter_mut().enumerate() {
                *s += vga_linear(palette[row[x] as usize * 3 + c]) as f64;
            }
        }
        preview.line_colour = sum.map(|s| s / lit.len() as f64);
        let monster = &preview.monster;
        let palette = preview.palette(0).to_vec();
        let columns: Vec<[f32; 3]> = (0..320)
            .map(|x| {
                let mut sum = [0.0f32; 3];
                for y in 0..200 {
                    let i = monster[y * 320 + x] as usize;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += vga_linear(palette[i * 3 + c]) / 200.0;
                    }
                }
                sum
            })
            .collect();
        // Softened over a few columns (authored), as the trail is a smear.
        preview.profile = (0..320i32)
            .map(|x| {
                let (mut sum, mut total) = ([0.0f32, 0.0, 0.0, 1.0], 0.0);
                for o in -4..=4i32 {
                    let w = (-(o * o) as f32 / 8.0).exp();
                    let c = columns[(x + o).clamp(0, 319) as usize];
                    for (s, v) in sum.iter_mut().zip(c) {
                        *s += v * w;
                    }
                    total += w;
                }
                [sum[0] / total, sum[1] / total, sum[2] / total, 1.0]
            })
            .collect();
        preview.phosphor = preview.persistence();
        preview.cells = (0..preview.frames.len())
            .flat_map(|j| cell_means(preview.indices(j), preview.palette(j)))
            .collect();
        Ok(preview)
    }

    /// The phosphor's state after each frame (authored): the white's
    /// afterglow and the dot's fast glow each take the frame's light or what
    /// is left of the frame before's, whichever is more, decaying with
    /// [`WHITE_PERSISTENCE`] and [`PERSISTENCE`]; the slow afterglow gathers the dot's light over
    /// [`AFTERGLOW`] frames and lets it go as slowly.
    fn persistence(&self) -> Vec<[f64; 3]> {
        let fast = (-1.0 / PERSISTENCE).exp();
        let white = (-1.0 / WHITE_PERSISTENCE).exp();
        let slow = (-1.0 / AFTERGLOW).exp();
        let mut state = [0.0; 3];
        (0..self.frames.len())
            .map(|j| {
                let b = self.beam_of_frame(j);
                state = [
                    b.ground.max(b.cover).max(state[0] * white),
                    b.dot.max(state[1] * fast),
                    state[2] * slow + b.dot * (1.0 - slow),
                ];
                state
            })
            .collect()
    }

    /// Part-8 frame position at a clip time: continuous, negative before
    /// part 8. A time within one audio sample of a frame's is that frame's.
    pub fn position(&self, time: f64) -> f64 {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        if (position - position.round()).abs() <= FRAME_RATE as f64 / SAMPLE_RATE as f64 {
            position.round()
        } else {
            position
        }
    }

    /// Part-8 frame at a clip time, or `None` before part 8. Times past the
    /// end hold the last frame.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = self.position(time);
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Clip time of part-8 frame `j`.
    pub fn seconds(&self, j: usize) -> f64 {
        (self.start + j) as f64 / FRAME_RATE as f64
    }

    fn moment(&self, time: f64) -> (usize, f64) {
        let last = self.frames.len() - 1;
        let position = self.position(time).clamp(0.0, last as f64);
        let j = position.floor() as usize;
        (j, if j == last { 0.0 } else { position - j as f64 })
    }

    /// The palette frame `j` shows.
    pub fn palette(&self, j: usize) -> &[u8] {
        &self.palettes[j * 768..][..768]
    }

    /// The indices of frame `j` as the scanout shows it, 320 x 200.
    pub fn indices(&self, j: usize) -> &[u8] {
        &self.images[self.frames[j].image]
    }

    /// The original frame `j` as 8-bit RGBA, 320 x 200.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let j = j.min(self.frames.len() - 1);
        super::field::rgba(self.indices(j), self.palette(j))
    }

    /// Title label for part-8 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        match self.frames[j.min(self.frames.len() - 1)].stage {
            Stage::Hold => "Part 8: the monster held",
            Stage::White => "Switch-off: white",
            Stage::Collapse => "Collapse to a line",
            Stage::Line => "The line shrinks to a dot",
            Stage::Dot => "The dot pulses",
            Stage::End => "The end",
        }
    }

    /// What the beam draws on frame `j` (read from the code and the
    /// frame's palette).
    pub fn beam_of_frame(&self, j: usize) -> Beam {
        let step = self.frames[j].step;
        let palette = self.palette(j);
        let (left, right) = ending::hd_line_ends(step);
        let line = (21..=67).contains(&step);
        Beam {
            compression: ending::hd_compression(step).map_or(LINE_COMPRESSION, |c| c as f64),
            band: if step <= 20 { 1.0 } else { 0.0 },
            line: if line { 1.0 } else { 0.0 },
            left: left.max(self.content.0 as i32) as f64,
            right: right.min(self.content.1 as i32) as f64,
            ground: palette[0] as f64 / 63.0,
            cover: if step == 3 { 1.0 } else { 0.0 },
            dot: if step >= 68 {
                vga_linear(palette[3]) as f64
            } else {
                0.0
            },
        }
    }

    /// What the beam draws at part-8 frame position `tau` (continuous):
    /// between frames, each value moves straight toward the next frame's,
    /// the compression by its logarithm.
    pub fn beam_at(&self, tau: f64) -> Beam {
        let last = self.frames.len() - 1;
        let tau = tau.clamp(0.0, last as f64);
        let j = tau.floor() as usize;
        let a = self.beam_of_frame(j);
        if j == last {
            return a;
        }
        let b = self.beam_of_frame(j + 1);
        let f = tau - j as f64;
        let lerp = |x: f64, y: f64| x + (y - x) * f;
        Beam {
            compression: lerp(a.compression.ln(), b.compression.ln()).exp(),
            band: lerp(a.band, b.band),
            line: lerp(a.line, b.line),
            left: lerp(a.left, b.left),
            right: lerp(a.right, b.right),
            ground: lerp(a.ground, b.ground),
            cover: lerp(a.cover, b.cover),
            dot: lerp(a.dot, b.dot),
        }
    }

    /// Everything part 8 puts in `u.ending` at a clip time.
    pub fn controls(&self, time: f64) -> EndingBlock {
        let (j, f) = self.moment(time);
        let last = self.frames.len() - 1;
        let k = (j + 1).min(last);
        let g = if k != j { f } else { 0.0 };
        let tau = j as f64 + g;
        let b = self.beam_at(tau);
        let phosphor = {
            let (p, q) = (self.phosphor[j], self.phosphor[k]);
            std::array::from_fn::<f64, 3, _>(|i| p[i] + (q[i] - p[i]) * g)
        };
        let fade = 1.0 - smoothstep(last as f64 - END_FADE, last as f64, tau);
        let mut trail = [[0.0f32; 4]; 5];
        for (i, &age) in TRAIL_AGES.iter().enumerate() {
            let t = self.beam_at(tau - age);
            trail[0][i] = (100.0 / t.compression) as f32;
            if tau >= age {
                trail[1][i] = t.band as f32;
                trail[2][i] = t.line as f32;
            }
            trail[3][i] = t.left as f32;
            trail[4][i] = t.right as f32;
        }
        let e = &self.events;
        let c = self.line_colour;
        let (eye, aim, tan_half) = camera(tau);
        EndingBlock {
            part: [
                1.0,
                (time - self.seconds(0)) as f32,
                self.frames[j].stage as u8 as f32,
                tau as f32,
            ],
            frame: [j as f32, k as f32, g as f32, self.frames[j].image as f32],
            tube: [
                smoothstep(0.0, CRT_RAMP, tau) as f32,
                b.ground as f32,
                b.cover as f32,
                0.0,
            ],
            beam: [
                b.compression as f32,
                b.band as f32,
                b.line as f32,
                fade as f32,
            ],
            ends: [
                b.left as f32,
                b.right as f32,
                self.content.0 as f32,
                self.content.1 as f32,
            ],
            glow: [
                phosphor[1] as f32,
                phosphor[2] as f32,
                phosphor[0] as f32,
                0.0,
            ],
            colour: [c[0] as f32, c[1] as f32, c[2] as f32, 0.0],
            trail_rows: trail[0],
            trail_band: trail[1],
            trail_line: trail[2],
            trail_left: trail[3],
            trail_right: trail[4],
            events: [e.white, e.collapse, e.line, e.dot].map(|v| v as f32),
            events_late: [e.end, e.last, 0.0, 0.0].map(|v| v as f32),
            eye: [eye[0] as f32, eye[1] as f32, eye[2] as f32, tan_half as f32],
            aim: [aim[0] as f32, aim[1] as f32, aim[2] as f32, 0.0],
        }
    }
}

/// The monster in linear light on a palette, with its mipmaps (each level
/// half the last, as wgpu sizes them, each texel the mean of the area of
/// the last it covers), for the modern view's far tubes.
pub fn picture_levels(monster: &[u8], palette: &[u8]) -> Vec<(u32, u32, Vec<[f16; 4]>)> {
    let mut level: Vec<[f32; 3]> = monster[..64000]
        .iter()
        .map(|&i| {
            let p = &palette[i as usize * 3..][..3];
            [vga_linear(p[0]), vga_linear(p[1]), vga_linear(p[2])]
        })
        .collect();
    let (mut w, mut h) = (320usize, 200usize);
    let mut levels = Vec::new();
    loop {
        let texels = level
            .iter()
            .map(|c| c.map(f16::from_f32))
            .map(|c| [c[0], c[1], c[2], f16::ONE])
            .collect();
        levels.push((w as u32, h as u32, texels));
        if w == 1 && h == 1 {
            return levels;
        }
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        // How much of each source texel along an axis an output texel covers.
        let cover = |i: usize, n: usize, from: usize| -> Vec<(usize, f32)> {
            let (a, b) = (
                i as f32 * from as f32 / n as f32,
                (i + 1) as f32 * from as f32 / n as f32,
            );
            (a.floor() as usize..(b.ceil() as usize).min(from))
                .map(|k| (k, (b.min(k as f32 + 1.0) - a.max(k as f32)).max(0.0)))
                .collect()
        };
        let mut next = vec![[0.0f32; 3]; nw * nh];
        for y in 0..nh {
            let rows = cover(y, nh, h);
            for x in 0..nw {
                let mut sum = [0.0f32; 3];
                let mut total = 0.0;
                for &(sy, wy) in &rows {
                    for (sx, wx) in cover(x, nw, w) {
                        for (s, v) in sum.iter_mut().zip(level[sy * w + sx]) {
                            *s += v * wx * wy;
                        }
                        total += wx * wy;
                    }
                }
                next[y * nw + x] = sum.map(|s| s / total);
            }
        }
        (level, w, h) = (next, nw, nh);
    }
}

/// The picture's mean colour (linear) in each of [`CELLS`].
pub fn cell_means(indices: &[u8], palette: &[u8]) -> Vec<[f32; 4]> {
    let (columns, rows) = CELLS;
    let mut cells = vec![[0.0f32, 0.0, 0.0, 1.0]; columns * rows];
    let mut counts = vec![0.0f32; columns * rows];
    for (i, &index) in indices.iter().enumerate().take(64000) {
        let (x, y) = (i % 320, i / 320);
        let cell = (y * rows / 200) * columns + x * columns / 320;
        let p = &palette[index as usize * 3..][..3];
        for (c, &v) in cells[cell].iter_mut().zip(p) {
            *c += vga_linear(v);
        }
        counts[cell] += 1.0;
    }
    for (cell, count) in cells.iter_mut().zip(counts) {
        for c in cell.iter_mut().take(3) {
            *c /= count;
        }
    }
    cells
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The modern view's halation image (authored): the monster's colours on
/// the hold's palette in linear light, blurred by [`HALATION_SIGMA`] source
/// pixels across and down (zero beyond the picture), 320 x 200.
pub fn halation(monster: &[u8], palette: &[u8]) -> Vec<[f16; 4]> {
    let radius = (3.0 * HALATION_SIGMA).ceil() as i32;
    let kernel: Vec<f64> = (-radius..=radius)
        .map(|i| (-(i * i) as f64 / (2.0 * HALATION_SIGMA * HALATION_SIGMA)).exp())
        .collect();
    let total: f64 = kernel.iter().sum();
    let colour = |i: usize| {
        let p = &palette[monster[i] as usize * 3..][..3];
        [0, 1, 2].map(|c| vga_linear(p[c]) as f64)
    };
    let blur = |get: &dyn Fn(i32, i32) -> [f64; 3], across: bool| {
        let mut out = vec![[0.0; 3]; 320 * 200];
        for y in 0..200 {
            for x in 0..320 {
                let mut sum = [0.0; 3];
                for (k, &w) in kernel.iter().enumerate() {
                    let o = k as i32 - radius;
                    let (sx, sy) = if across { (x + o, y) } else { (x, y + o) };
                    if (0..320).contains(&sx) && (0..200).contains(&sy) {
                        let c = get(sx, sy);
                        for i in 0..3 {
                            sum[i] += c[i] * w;
                        }
                    }
                }
                out[(y * 320 + x) as usize] = sum.map(|s| s / total);
            }
        }
        out
    };
    let across = blur(&|x, y| colour((y * 320 + x) as usize), true);
    let down = blur(&|x, y| across[(y * 320 + x) as usize], false);
    down.iter()
        .map(|c| {
            [
                f16::from_f64(c[0]),
                f16::from_f64(c[1]),
                f16::from_f64(c[2]),
                f16::ONE,
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    #[test]
    fn stages_follow_the_scenes_steps() {
        let stages: Vec<Stage> = [0, 1, 2, 3, 4, 20, 21, 67, 68, 128, 129, 198]
            .map(Stage::of)
            .to_vec();
        use Stage::*;
        assert_eq!(
            stages,
            [
                Hold, Hold, White, White, Collapse, Collapse, Line, Line, Dot, Dot, End, End
            ]
        );
    }

    /// The camera starts where the face's 200 rows fill the frame, straight
    /// on and at rest; it holds close while the CRT comes on, then draws
    /// back, never stopping, smoothly, the centre set about a quarter of the
    /// frame's height at the switch-off and a fifth at the end.
    #[test]
    fn camera_starts_on_the_face_and_never_stops_drawing_back() {
        let (eye, aim, tan_half) = camera(0.0);
        assert_eq!(aim, [0.0; 3]);
        assert_eq!(&eye[..2], &[0.0, 0.0]);
        assert!((eye[2] * tan_half - 100.0).abs() < 1e-9);
        let length = |e: [f64; 3]| (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
        let share = |tau: f64| SET_PITCH / (2.0 * length(camera(tau).0) * tan_half);
        assert!(share(CRT_RAMP) > 0.6, "{}", share(CRT_RAMP));
        assert!((0.2..0.3).contains(&share(285.0)), "{}", share(285.0));
        assert!((0.15..0.22).contains(&share(481.0)), "{}", share(481.0));
        // Tenths of a frame: always moving away once started, each step
        // small against the distance, the speed never changing suddenly.
        let mut previous = camera(0.0).0;
        let mut speed = 0.0;
        for k in 1..=4810 {
            let tau = k as f64 / 10.0;
            let now = camera(tau).0;
            let step = length(now) - length(previous);
            assert!(step > 0.0, "the camera stops at frame {tau}");
            assert!(
                step < 0.002 * length(now),
                "the camera jumps at frame {tau}"
            );
            assert!(
                (step - speed).abs() < 0.02,
                "its speed jumps at frame {tau}"
            );
            (previous, speed) = (now, step);
        }
    }

    #[test]
    fn picture_levels_halve_to_one_texel_keeping_the_mean() {
        let mut palette = vec![0u8; 768];
        palette[3..6].fill(63);
        let mut monster = vec![0u8; 64000];
        monster[..32000].fill(1);
        let levels = picture_levels(&monster, &palette);
        let sizes: Vec<(u32, u32)> = levels.iter().map(|l| (l.0, l.1)).collect();
        assert_eq!(
            sizes,
            [
                (320, 200),
                (160, 100),
                (80, 50),
                (40, 25),
                (20, 12),
                (10, 6),
                (5, 3),
                (2, 1),
                (1, 1)
            ]
        );
        // Each level keeps the picture's mean (half white, half black).
        for (w, h, texels) in &levels {
            let mean = texels.iter().map(|t| t[0].to_f32()).sum::<f32>() / (w * h) as f32;
            assert!((mean - 0.5).abs() < 0.01, "{w} x {h}: {mean}");
        }
    }

    #[test]
    fn cell_means_average_each_cell() {
        let mut palette = vec![0u8; 768];
        palette[3..6].fill(63);
        let mut indices = vec![0u8; 64000];
        // The top-left cell (80 x 67 pixels) all white, one pixel elsewhere.
        for y in 0..67 {
            indices[y * 320..y * 320 + 80].fill(1);
        }
        indices[199 * 320 + 319] = 1;
        let cells = cell_means(&indices, &palette);
        assert_eq!(cells.len(), CELLS.0 * CELLS.1);
        assert!((cells[0][0] - 1.0).abs() < 1e-6);
        assert!(cells[1][0] == 0.0);
        let last = cells[11][1];
        assert!(last > 0.0 && last < 0.001, "{last}");
    }

    #[test]
    fn halation_keeps_a_flat_field_and_spreads_a_point() {
        let mut palette = vec![0u8; 768];
        palette[3..6].fill(63);
        // A flat white picture stays white away from the edges.
        let flat = halation(&vec![1u8; 64000], &palette);
        assert!((flat[100 * 320 + 160][0].to_f64() - 1.0).abs() < 1e-3);
        assert!(flat[0][0].to_f64() < 0.4);
        // A point spreads symmetrically and keeps its light.
        let mut point = vec![0u8; 64000];
        point[100 * 320 + 160] = 1;
        let spread = halation(&point, &palette);
        let at = |x: usize, y: usize| spread[y * 320 + x][1].to_f64();
        assert!((at(157, 100) - at(163, 100)).abs() < 1e-6);
        assert!((at(160, 97) - at(160, 103)).abs() < 1e-6);
        let sum: f64 = spread.iter().map(|c| c[1].to_f64()).sum();
        assert!((sum - 1.0).abs() < 0.01, "{sum}");
    }

    /// Every part-8 frame against the engine: its part, pixels and palette,
    /// the stages by the scene's steps and the PCM; and the geometry the
    /// modern view reads from the code against what each frame shows.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part8_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let clip = Clip::load_ending(&data, ScrollerLighting::Studio).unwrap();
        let greetings = clip.greetings.as_ref().unwrap();
        let ending = clip.ending.as_ref().unwrap();
        assert_eq!(ending.start, greetings.start + greetings.frames.len() - 1);
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in 0..ending.start {
            pcm.extend(engine.step().unwrap());
        }
        for (j, frame) in ending.frames.iter().enumerate() {
            let video = &engine.video;
            assert_eq!(engine.part(), 8, "part at frame {j}");
            assert!(
                ending.indices(j) == &video.indices[..],
                "pixels at frame {j}"
            );
            assert!(
                ending.palette(j) == &video.palette[..],
                "palette at frame {j}"
            );
            // The steps count from the first step that reads 35:00; until
            // then the monster is held. Frame j shows the step that read the
            // music at frame j - 1.
            if j > 0 {
                let read = ending.music[j - 1];
                assert_eq!(frame.step == 0, read < 0x3500, "frame {j} at {read:04x}");
            }
            // What the frame lights lies within what the modern view draws.
            let b = ending.beam_of_frame(j);
            let indices = ending.indices(j);
            for y in 0..200 {
                for x in 0..320 {
                    let i = indices[y * 320 + x] as usize;
                    if ending.palette(j)[i * 3..][..3].iter().all(|&v| v == 0) {
                        continue;
                    }
                    match frame.stage {
                        Stage::Hold | Stage::White => {}
                        Stage::Collapse => {
                            let half = 100.0 / b.compression + 1.0;
                            assert!((y as f64 - 100.5).abs() <= half, "row {y} at frame {j}");
                        }
                        Stage::Line => {
                            assert_eq!(y, LINE_ROW, "frame {j}");
                            assert!(
                                (b.left..=b.right).contains(&(x as f64)),
                                "column {x} at frame {j}"
                            );
                        }
                        Stage::Dot | Stage::End => {
                            assert_eq!((x, y), (DOT_COLUMN, LINE_ROW), "frame {j}");
                            assert!(b.dot > 0.0);
                        }
                    }
                }
            }
            if j + 1 < ending.frames.len() {
                pcm.extend(engine.step().unwrap());
            }
        }
        assert!(engine.finished, "the demo ends on part 8's last frame");
        assert_eq!(ending.frames.last().unwrap().step, 198);
        assert!(pcm == clip.pcm, "PCM");
        let e = ending.events;
        let t = |v: f64| v + ending.seconds(0);
        println!(
            "Part 8 at {:.3}s: white {:.3}, collapse {:.3}, line {:.3}, dot {:.3}, end {:.3}, \
             last {:.3}; {} frames, {} images, line row's colour in columns {:?}",
            ending.seconds(0),
            t(e.white),
            t(e.collapse),
            t(e.line),
            t(e.dot),
            t(e.end),
            t(e.last),
            ending.frames.len(),
            ending.images.len(),
            ending.content
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part8s_sections_start_where_its_labels_change() {
        let data = crate::test_support::data();
        let clip = Clip::load_ending(&data, ScrollerLighting::Studio).unwrap();
        let sections = clip.sections();
        let frame = 1.0 / FRAME_RATE as f64;
        assert!(sections.windows(2).all(|w| w[0] < w[1]));
        for &s in &sections[1..] {
            assert_ne!(clip.label(s), clip.label(s - frame));
        }
        let ending = clip.ending.as_ref().unwrap();
        let start = ending.seconds(0);
        let part8: Vec<(f64, &str)> = sections
            .iter()
            .filter(|&&s| s >= start - 1e-9)
            .map(|&s| (s, clip.label(s)))
            .collect();
        for (s, label) in &part8 {
            println!("{s:8.3} {label}");
        }
        let labels: Vec<&str> = part8.iter().map(|p| p.1).collect();
        assert_eq!(
            labels,
            [
                "Part 8: the monster held",
                "Switch-off: white",
                "Collapse to a line",
                "The line shrinks to a dot",
                "The dot pulses",
                "The end",
            ]
        );
        let e = ending.events;
        for (k, event) in [0.0, e.white, e.collapse, e.line, e.dot, e.end]
            .iter()
            .enumerate()
        {
            assert!(
                (part8[k].0 - start - event).abs() < 1.0 / SAMPLE_RATE as f64,
                "{}",
                labels[k]
            );
        }
        // The clip ends on the demo's last frame, and part 7's preview hands
        // over on part 8's first.
        let last = ending.frames.len() - 1;
        assert_eq!(ending.frame_at(clip.duration()), Some(last));
        let greetings = clip.greetings.as_ref().unwrap();
        assert_eq!(
            ending.frame_at(greetings.seconds(greetings.frames.len() - 1)),
            Some(0)
        );
        // The CRT starts from nothing, and everything has faded by the end.
        assert_eq!(ending.controls(start).tube[0], 0.0);
        assert_eq!(ending.controls(clip.duration()).beam[3], 0.0);
    }
}
