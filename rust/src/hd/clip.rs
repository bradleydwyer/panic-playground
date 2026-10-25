use super::dots::DotsPreview;
use super::field::FieldPreview;
use super::rings::RingsPreview;
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::intro::{Intro, Presentation},
    scenes::scroller::{HdFont, HdScroll, Scroller, oscillator_unstep},
    vga::Vga,
};
use half::f16;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;

pub struct Frame {
    /// Intro presentation state; preview phases 6 and 7 represent the following
    /// square wipe and backdrop hold; 8/9 are the scroller and its exit fade.
    pub state: Presentation,
    pub indices: Vec<u8>,
    pub palette: Vec<u8>,
}

impl Frame {
    pub fn shape_key(&self) -> (u8, usize) {
        if self.state.phase >= 8 {
            // Scroller frames upload their backdrop for faithful HD instead.
            return (self.state.phase, self.state.frame);
        }
        (
            self.state.phase.min(4),
            if self.state.phase >= 4 {
                0
            } else {
                self.state.frame
            },
        )
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.indices
            .iter()
            .flat_map(|&index| {
                let p = &self.palette[index as usize * 3..][..3];
                let channel = |c: u8| {
                    let c = c & 63;
                    (c << 2) | (c >> 4)
                };
                [channel(p[0]), channel(p[1]), channel(p[2]), 255]
            })
            .collect()
    }

    /// The original part-2 picture without its letters or overlay dots, as
    /// half-float sRGB, for faithful HD to draw smooth letters over. Bit 7
    /// (the overlay) is cleared. Letter pixels (low seven bits below 64) take
    /// the nearest background pixel 8 columns or rows away, the period of the
    /// tile pattern, so the backdrop keeps its original colours.
    pub fn backdrop(&self) -> Vec<f16> {
        let background = |x: i32, y: i32| {
            (0..HEIGHT as i32 / 8).find_map(|k| {
                [
                    (x - 8 * k, y),
                    (x + 8 * k, y),
                    (x, y - 8 * k),
                    (x, y + 8 * k),
                ]
                .into_iter()
                .filter(|&(x, y)| (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y))
                .map(|(x, y)| self.indices[y as usize * WIDTH + x as usize] & 127)
                .find(|&i| i >= 64)
            })
        };
        let channel = |c: u8| f16::from_f32((((c & 63) << 2) | ((c & 63) >> 4)) as f32 / 255.0);
        (0..WIDTH * HEIGHT)
            .flat_map(|at| {
                let mut index = self.indices[at] & 127;
                if index < 64 {
                    index = background((at % WIDTH) as i32, (at / WIDTH) as i32).unwrap_or(index);
                }
                let p = &self.palette[index as usize * 3..][..3];
                [channel(p[0]), channel(p[1]), channel(p[2]), f16::ONE]
            })
            .collect()
    }

    /// Derive shape and material fields from caller-supplied indexed artwork.
    /// The contour is limited by the original raster; shading is newly generated
    /// at display resolution. Nothing here restores absent source artwork.
    /// The scroller does not use frame fields; see [`ScrollerGlyphs`].
    pub fn field(&self) -> Vec<f16> {
        let logo = self.state.phase >= 4;
        let mask: Vec<bool> = self
            .indices
            .iter()
            .map(|&i| {
                if logo {
                    (16..64).contains(&i)
                } else {
                    i >= 229
                }
            })
            .collect();
        let gold: Vec<bool> = self
            .indices
            .iter()
            .map(|&i| logo && (16..48).contains(&i))
            .collect();
        let shape = signed_distance(&mask, WIDTH, HEIGHT);
        let metal = signed_distance(&gold, WIDTH, HEIGHT);
        shape
            .iter()
            .zip(metal.iter())
            .zip(self.indices.iter())
            .flat_map(|((&s, &m), &i)| {
                let at = i as usize * 3;
                let mut light = (self.palette[at] as f32
                    + self.palette[at + 1] as f32
                    + self.palette[at + 2] as f32)
                    / 189.0;
                // Keep the cached material independent of the reveal's white
                // palette fade, so seeking and uninterrupted playback agree.
                if logo {
                    light = match i {
                        16..=31 => (31 - i) as f32 / 15.0,
                        32..=47 => (47 - i) as f32 / 15.0,
                        48..=63 => (63 - i) as f32 / 15.0,
                        _ => 0.0,
                    };
                }
                [
                    f16::from_f32(s),
                    f16::from_f32(m),
                    f16::from_f32(light),
                    f16::ONE,
                ]
            })
            .collect()
    }
}

pub struct Clip {
    pub frames: Vec<Frame>,
    pub pcm: Vec<i16>,
    pub logo_at: f64,
    pub logo_ready_at: f64,
    pub transition: Option<TileTransition>,
    pub scroller: Option<ScrollerPreview>,
    /// Pinhole motion fitted from the locally decoded reference silhouettes:
    /// camera crossing frame, depth per frame, world x and world y.
    pub flights: [[f32; 4]; 10],
    /// Lighting cues measured once from `pcm`, one per 70 Hz window; see
    /// [`audio_cues`]. They steer only authored lighting, in the intro and in
    /// the scroller's musical mode, and are there for the part-6 styles.
    pub audio: Vec<AudioCues>,
    /// Part 6, after part 2 (`--rings`). Its frames are not in `frames`; see
    /// [`RingsPreview`].
    pub rings: Option<RingsPreview>,
    /// Part 5, after part 6 (`--field`). Its frames are in neither; see
    /// [`FieldPreview`].
    pub field: Option<FieldPreview>,
    /// Part 3, after part 5 (`--dots`), with part 4's wait and first frame;
    /// see [`DotsPreview`].
    pub dots: Option<DotsPreview>,
}

/// Authored lighting cues for one 70 Hz window of the clip's PCM, each 0..=1.
/// Both are read from the same analysis; see [`audio_cues`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioCues {
    /// Modern intro (`u.audio`): energy, onset, low band, accent.
    pub intro: [f32; 4],
    /// Scroller's musical mode (`u.scroll_light` x, z, w): relative level,
    /// bass onset, treble onset.
    pub scroller: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollerLighting {
    Musical,
    Studio,
}

impl std::str::FromStr for ScrollerLighting {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "musical" => Ok(Self::Musical),
            "studio" => Ok(Self::Studio),
            _ => Err("Use --scroller-lighting musical or studio".into()),
        }
    }
}

/// What the modern view shows behind the letters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ScrollerBackdrop {
    /// The dark chamber, with the original tile wall faintly visible in it.
    Chamber = 0,
    /// Authored chrome bumps on the original tile grid, tinted by the tile image.
    Wall = 1,
    /// The same bumps as plain chrome; the default.
    Chrome = 2,
}

impl ScrollerBackdrop {
    pub fn next(self) -> Self {
        match self {
            Self::Chamber => Self::Wall,
            Self::Wall => Self::Chrome,
            Self::Chrome => Self::Chamber,
        }
    }
}

impl std::str::FromStr for ScrollerBackdrop {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "chamber" => Ok(Self::Chamber),
            "wall" => Ok(Self::Wall),
            "chrome" => Ok(Self::Chrome),
            _ => Err("Use --scroller-backdrop chamber, wall or chrome".into()),
        }
    }
}

pub struct ScrollerPreview {
    pub start_tick: usize,
    pub end_tick: usize,
    /// Musical mode reads `Clip::audio`, precomputed on the audio timeline so
    /// seeking and exports agree; studio mode ignores it.
    pub lighting: ScrollerLighting,
    /// Displayed scroll positions for every clip frame, without wrapping:
    /// upper and lower row text pixel at screen x = 0, and overlay step.
    /// Read from the scroll counters (`Scroller::hd_scroll`), not the pixels.
    pub scroll: Vec<[f64; 3]>,
    /// Letter shapes built from the locally loaded font and overlay tables.
    pub glyphs: ScrollerGlyphs,
    pub backdrop_style: ScrollerBackdrop,
    /// Authored bump profile for the wall backdrops: 0 is a low rounded
    /// square close to the original tile, 1 a hemisphere.
    pub wall_shape: f32,
    /// Authored presence of the bump wall, 0 to 1: its overall brightness.
    pub wall_strength: f32,
    /// Authored waves across the bump wall, lit by a grazing side light; on
    /// by default. The original's wall is flat; see
    /// [`ScrollerPreview::waves_at`] and [`ScrollerPreview::wave_swing`].
    pub waves: bool,
    /// The swell's depth, 0.1 (barely visible) to 1 (far too deep): how far
    /// its height swings each way.
    pub wave_depth: f32,
    /// The side light's strength, and so its shadows', 0 (none) to 1.
    pub wave_light: f32,
    /// The 8x8 background image behind palette entries 64..=127 at each frame,
    /// linear RGB, unfiltered: the image `palette()` read for that frame.
    /// Before the first call the original still shows the intro's flat grey
    /// in those entries; the HD wall instead shows the scrolling image, so
    /// the laser reveals the same wall, already moving (see `oscillator`).
    pub backdrop: Vec<[[f32; 4]; 64]>,
    /// The oscillator (`xx`, `yy`) of each frame's palette, in source pixels
    /// (divided by 128, the step `palette()` quantises to). See [`wall_pattern`].
    /// Frames before the first `palette()` call hold the authored lead-in from
    /// [`oscillator_lead_in`], not the original's (unmoving) values.
    pub oscillator: Vec<[f32; 2]>,
}

/// Schedule and destination colours read from the local Rust reconstruction.
/// The GPU's laser cut, released plates, smoke and sparks are authored effects.
pub struct TileTransition {
    pub start_tick: usize,
    pub end_tick: usize,
    pub order: [u16; 160],
    pub palette: [[f32; 4]; 64],
}

impl Clip {
    pub fn load(data: &Data) -> Result<Self, String> {
        Self::load_mode(data, false, false, false, false, false)
    }

    /// Continue through the tile transition to the next backdrop, with
    /// uninterrupted PCM. The modern renderer draws it as a laser cut.
    pub fn load_transition(data: &Data) -> Result<Self, String> {
        Self::load_mode(data, true, false, false, false, false)
    }

    pub fn load_scroller(data: &Data, lighting: ScrollerLighting) -> Result<Self, String> {
        let mut clip = Self::load_mode(data, true, true, false, false, false)?;
        clip.scroller.as_mut().unwrap().lighting = lighting;
        Ok(clip)
    }

    /// Continue past part 2 through part 6 to the first frame of part 5,
    /// with uninterrupted PCM. Part 6's frames are kept as compact state;
    /// see [`RingsPreview`].
    pub fn load_rings(data: &Data, lighting: ScrollerLighting) -> Result<Self, String> {
        let mut clip = Self::load_mode(data, true, true, true, false, false)?;
        clip.scroller.as_mut().unwrap().lighting = lighting;
        Ok(clip)
    }

    /// Continue past part 6 through part 5 to the first frame of part 3,
    /// which shows part 5's last, with uninterrupted PCM. Part 5's frames are
    /// kept as compact state; see [`FieldPreview`].
    pub fn load_field(data: &Data, lighting: ScrollerLighting) -> Result<Self, String> {
        let mut clip = Self::load_mode(data, true, true, true, true, false)?;
        clip.scroller.as_mut().unwrap().lighting = lighting;
        Ok(clip)
    }

    /// Continue past part 5 through part 3 and part 4's wait to part 4's
    /// first drawn frame, its logo slab in white, with uninterrupted PCM.
    /// Part 3's frames are kept as compact state; see [`DotsPreview`].
    pub fn load_dots(data: &Data, lighting: ScrollerLighting) -> Result<Self, String> {
        let mut clip = Self::load_mode(data, true, true, true, true, true)?;
        clip.scroller.as_mut().unwrap().lighting = lighting;
        Ok(clip)
    }

    /// Scroll positions at a clip time, interpolated between 70 Hz frames so
    /// that each frame's exact position is shown at the moment it appears.
    pub fn scroll_at(&self, time: f64) -> Option<[f64; 3]> {
        let scroll = &self.scroller.as_ref()?.scroll;
        let position = (time.max(0.0) * FRAME_RATE as f64).min((scroll.len() - 1) as f64);
        let index = position.floor() as usize;
        let next = (index + 1).min(scroll.len() - 1);
        let fraction = position - index as f64;
        Some(std::array::from_fn(|k| {
            scroll[index][k] + (scroll[next][k] - scroll[index][k]) * fraction
        }))
    }

    fn load_mode(
        data: &Data,
        extend: bool,
        play_scroller: bool,
        play_rings: bool,
        play_field: bool,
        play_dots: bool,
    ) -> Result<Self, String> {
        let mut vga = Vga::new();
        let mut intro = Intro::new(data, &mut vga)?;
        let mut music = Music::new(data)?;
        let mut frames = Vec::new();
        let mut pcm = Vec::new();
        let mut logo_at = None;
        loop {
            let n = frames.len() as u64;
            if n > FRAME_RATE * 60 {
                return Err("Intro exceeded its one-minute import limit".into());
            }
            let state = intro.presentation();
            if state.phase >= 4 {
                logo_at.get_or_insert(n as f64 / FRAME_RATE as f64);
            }
            frames.push(Frame {
                state,
                indices: vga.indices.clone(),
                palette: vga.palette.clone(),
            });
            if state.phase >= 5 {
                break;
            }
            intro.step(&mut vga, music.position());
            if let Some(cue) = intro.cue_frame() {
                music.intro_cue(cue);
            }
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
        }
        let logo_ready_at = (frames.len() - 1) as f64 / FRAME_RATE as f64;
        let flights = fit_flights(&frames)?;
        let mut scroller_preview = None;
        let mut rings: Option<RingsPreview> = None;
        let mut field: Option<FieldPreview> = None;
        let mut dots = None;
        let transition = if extend {
            // The last intro snapshot is also the first frame of part 2. Replace
            // it, rather than adding an extra frame or restarting the music.
            let logo_state = frames.pop().unwrap().state;
            let mut scroller = Scroller::new(data, &mut vga)?;
            let font = scroller.hd_font();
            let period = [
                (font.text.len() * 104) as i64,
                (font.text.len() * 104) as i64,
                2048,
            ];
            let mut unwrap = ScrollUnwrap::new(scroller.hd_scroll(), period);
            let mut scroll = vec![unwrap.position(); frames.len()];
            let mut scroll_start = 0;
            let mut scroll_end = 0;
            let mut transition = TileTransition {
                start_tick: 0,
                end_tick: 0,
                order: scroller.transition_order(),
                palette: [[0.0; 4]; 64],
            };
            // The oscillator's starting state, before any palette() call. Intro
            // frames come first; their positions come from the lead-in below.
            let oscillator_start = scroller.hd_oscillator();
            let mut walls = vec![([0.0; 2], None); frames.len()];
            loop {
                let n = frames.len();
                let (phase, tick) = scroller.presentation();
                if phase == 6 {
                    break;
                }
                if phase == 4 && scroll_start == 0 {
                    scroll_start = n;
                }
                if phase == 5 && scroll_end == 0 {
                    scroll_end = n;
                }
                if phase == 1 && tick == 0 {
                    transition.start_tick = n;
                }
                if phase == 2 {
                    transition.end_tick = n;
                }
                if phase == 3 && tick == 0 {
                    for (i, color) in transition.palette.iter_mut().enumerate() {
                        for (channel, c) in color[..3].iter_mut().enumerate() {
                            *c = vga_linear(vga.palette[(64 + i) * 3 + channel]);
                        }
                        color[3] = 1.0;
                    }
                }
                frames.push(Frame {
                    state: Presentation {
                        phase: if phase == 0 {
                            5
                        } else if phase < 3 {
                            6
                        } else if phase < 4 {
                            7
                        } else if phase == 4 {
                            8
                        } else {
                            9
                        },
                        frame: if phase >= 4 { n } else { logo_state.frame },
                        tick,
                        ..logo_state
                    },
                    indices: vga.indices.clone(),
                    palette: vga.palette.clone(),
                });
                scroll.push(unwrap.next(scroller.hd_scroll()));
                if play_scroller {
                    let (xx, yy, anim) = scroller.hd_backdrop();
                    walls.push((
                        [xx as f32 / 128.0, yy as f32 / 128.0],
                        anim.map(|anim| scroller.hd_tile_image(anim)),
                    ));
                }
                // The transition-only experiment stops on the backdrop. The
                // scroller experiment continues to the original part-2 boundary.
                if !play_scroller && phase == 3 && tick == FRAME_RATE as usize {
                    break;
                }
                if n as u64 > FRAME_RATE * 120 {
                    return Err("Scroller exceeded its two-minute import limit".into());
                }
                scroller.step(&mut vga, music.position());
                let n = n as u64;
                let samples =
                    ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
                pcm.extend(music.render(samples));
            }
            if play_scroller {
                // Every frame before the first palette() call takes the
                // authored lead-in, which arrives at the starting state.
                let first = walls.iter().position(|(_, image)| image.is_some());
                let first = first.ok_or("The scroller never set its background")?;
                let lead_in = oscillator_lead_in(oscillator_start, first);
                for (wall, state) in walls.iter_mut().zip(lead_in) {
                    wall.0 = state.map(|[position, _]| position as f32 / 128.0);
                }
                let rest = scroller.hd_tile_image(0);
                scroller_preview = Some(ScrollerPreview {
                    start_tick: scroll_start,
                    end_tick: scroll_end,
                    lighting: ScrollerLighting::Studio,
                    scroll,
                    glyphs: ScrollerGlyphs::new(&font, &frames[scroll_start].palette)?,
                    backdrop_style: ScrollerBackdrop::Chrome,
                    wall_shape: 0.5,
                    wall_strength: 0.6,
                    waves: true,
                    wave_depth: 0.5,
                    wave_light: 0.5,
                    backdrop: walls
                        .iter()
                        .map(|(_, image)| {
                            image
                                .unwrap_or(rest)
                                .map(|[r, g, b]| [vga_linear(r), vga_linear(g), vga_linear(b), 1.0])
                        })
                        .collect(),
                    oscillator: walls.iter().map(|&(position, _)| position).collect(),
                });
            }
            if play_rings {
                // The last scroller step above returned done; the engine makes
                // part 6 in that same step, so its first frame comes next.
                rings = Some(RingsPreview::load(
                    data,
                    &mut vga,
                    &mut music,
                    &mut pcm,
                    frames.len(),
                )?);
            }
            if let (true, Some(rings)) = (play_field, &rings) {
                // Part 6's last step returned done and made part 5, so its
                // first frame comes next.
                let fade = rings
                    .frames
                    .iter()
                    .position(|f| f.state.phase == 2)
                    .map(|j| (rings.start + j) as f64 / FRAME_RATE as f64);
                field = Some(FieldPreview::load(
                    data,
                    &mut vga,
                    &mut music,
                    &mut pcm,
                    rings.start + rings.frames.len(),
                    fade,
                )?);
            }
            if let (true, Some(field)) = (play_dots, &field) {
                // Part 5's last step returned done and made part 3, so its
                // first frame comes next.
                dots = Some(DotsPreview::load(
                    data,
                    &mut vga,
                    &mut music,
                    &mut pcm,
                    field.start + field.frames.len(),
                )?);
            }
            Some(transition)
        } else {
            // Standalone logo preview continues the music through a short hold.
            pcm.extend(music.render(SAMPLE_RATE as usize * 4));
            None
        };
        Ok(Self {
            frames,
            audio: audio_cues(&pcm),
            pcm,
            logo_at: logo_at.ok_or("Intro never reached the logo")?,
            logo_ready_at,
            transition,
            scroller: scroller_preview,
            flights,
            rings,
            field,
            dots,
        })
    }

    pub fn duration(&self) -> f64 {
        self.pcm.len() as f64 / (SAMPLE_RATE * 2) as f64
    }
    /// Audio lighting cues at a clip time, for the intro and the scroller
    /// alike. Window `i` is only known once it has played, at `(i + 1) / 70` s,
    /// so the lights never anticipate the music. Values interpolate between
    /// windows and fall to zero outside the PCM. Seeking reads the same
    /// precomputed table as playback.
    pub fn audio_at(&self, time: f64) -> AudioCues {
        let position = time * FRAME_RATE as f64 - 1.0;
        if !position.is_finite() || position <= -1.0 {
            return AudioCues::default();
        }
        let base = position.floor();
        let fraction = (position - base) as f32;
        let at = |i: f64| {
            if i < 0.0 {
                AudioCues::default()
            } else {
                self.audio.get(i as usize).copied().unwrap_or_default()
            }
        };
        let (a, b) = (at(base), at(base + 1.0));
        let lerp = |x: f32, y: f32| x + (y - x) * fraction;
        AudioCues {
            intro: std::array::from_fn(|k| lerp(a.intro[k], b.intro[k])),
            scroller: std::array::from_fn(|k| lerp(a.scroller[k], b.scroller[k])),
        }
    }
    pub fn frame_index(&self, time: f64) -> usize {
        ((time.max(0.0) * FRAME_RATE as f64) as usize).min(self.frames.len() - 1)
    }
    /// Part-6 frame at a clip time, when the clip has reached part 6.
    pub fn rings_frame(&self, time: f64) -> Option<usize> {
        self.rings.as_ref()?.frame_at(time)
    }
    /// Part-5 frame at a clip time, when the clip has reached part 5.
    pub fn field_frame(&self, time: f64) -> Option<usize> {
        self.field.as_ref()?.frame_at(time)
    }
    /// Part-3 frame at a clip time, when the clip has reached part 3.
    pub fn dots_frame(&self, time: f64) -> Option<usize> {
        self.dots.as_ref()?.frame_at(time)
    }
    /// The times at which [`Clip::label`] changes, found at each engine
    /// frame, with 0 first: the starts of the sections the preview steps
    /// through.
    pub fn sections(&self) -> Vec<f64> {
        let mut starts = vec![0.0];
        let mut last = self.label(0.0);
        let frames = (self.duration() * FRAME_RATE as f64) as usize;
        for k in 1..=frames {
            let time = k as f64 / FRAME_RATE as f64;
            let label = self.label(time);
            if label != last {
                starts.push(time);
                last = label;
            }
        }
        starts
    }

    pub fn label(&self, time: f64) -> &'static str {
        if let (Some(dots), Some(j)) = (&self.dots, self.dots_frame(time)) {
            return dots.label(j);
        }
        if let (Some(field), Some(j)) = (&self.field, self.field_frame(time)) {
            return field.label(j);
        }
        if let (Some(rings), Some(j)) = (&self.rings, self.rings_frame(time)) {
            return rings.label(j);
        }
        match self.frames[self.frame_index(time)].state.phase {
            0 => "Starfield",
            1 => "Future Crew",
            2 => "Music cue",
            3 => "Doors",
            6 => "Metal tile transition",
            7 => "Scroller backdrop",
            8 => "Double scroller",
            9 => "Scroller fade",
            _ => "Panic logo",
        }
    }
}

/// Authored lead-in for the HD wall; not original behaviour. The original
/// starts its background oscillator with part 2's first `palette()` call, when
/// the text scroll begins, about 3.6 s after the laser cut starts; before
/// that the HD wall would stand still. Instead, the `frames` frames before
/// that call take the oscillator run backwards from its starting state
/// (`[[xx, sx], [yy, sy]]`), one exact inverse of the original's update per
/// frame. Element `frames - 1` is one step before the start, so the motion
/// arrives continuously at the original's first value, with its speed, and
/// the original's own values follow.
pub fn oscillator_lead_in(start: [[i16; 2]; 2], frames: usize) -> Vec<[[i16; 2]; 2]> {
    let mut state = start;
    let mut lead_in = vec![start; frames];
    for slot in lead_in.iter_mut().rev() {
        state = state.map(oscillator_unstep);
        *slot = state;
    }
    lead_in
}

/// Seconds over which the waves settle before part 2's fade.
const WAVE_SETTLE: f64 = 1.5;
/// Authored: seconds for the swell's height to swing from its set depth to
/// the opposite (crests turning into troughs) and back. About two of the
/// tune's patterns at their pace in part 6 (06:00 at 43.41 s, 08:00 at
/// 50.77 s), but not timed to them.
pub const WAVE_TURN: f64 = 7.36;

/// Authored ease for the wall waves, 0 to 1: in from `reveal`, when the
/// laser cut starts to open, until `start`, when the text begins; then out
/// over [`WAVE_SETTLE`] seconds until `end`, when part 2 fades. In seconds.
pub fn wave_envelope(time: f64, reveal: f64, start: f64, end: f64) -> f32 {
    let ease = |x: f64| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let rise = ease((time - reveal) / (start - reveal).max(1e-3));
    (rise * ease((end - time) / WAVE_SETTLE)) as f32
}

impl ScrollerPreview {
    /// The wall waves at a clip time, as `u.waves.x` and `u.waves.z` take
    /// them: depth and side light, each times [`wave_envelope`]; both zero
    /// when the waves are off. `reveal` is the laser cut's first tick.
    pub fn waves_at(&self, time: f64, reveal: usize) -> [f32; 2] {
        if !self.waves {
            return [0.0; 2];
        }
        let seconds = |tick: usize| tick as f64 / FRAME_RATE as f64;
        let ease = wave_envelope(
            time,
            seconds(reveal),
            seconds(self.start_tick),
            seconds(self.end_tick),
        );
        [self.wave_depth * ease, self.wave_light * ease]
    }

    /// The swell's swing at a clip time, as `u.waves.w` takes it: 1 at the
    /// set depth, through 0 (flat) to -1 (crests and troughs swapped) and
    /// back, every [`WAVE_TURN`] seconds from the laser cut's first tick,
    /// `reveal`. A pure function of the time, like the waves themselves.
    pub fn wave_swing(&self, time: f64, reveal: usize) -> f32 {
        let since = time - reveal as f64 / FRAME_RATE as f64;
        (since * std::f64::consts::TAU / WAVE_TURN).cos() as f32
    }
}

/// 6-bit VGA DAC value to linear light, as the reference scanout expands it.
pub(super) fn vga_linear(v: u8) -> f32 {
    let v = v & 63;
    let srgb = ((v << 2) | (v >> 4)) as f32 / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Image coordinate of a background pixel. Read from the code: the page is
/// filled with index `64 + 8 * ((row + 6) & 7) + (x & 7)`, displayed one row
/// down, and `palette()` loads entry `64 + 8 * r + c` from image cell
/// `(y + r, x' + c)`, with `x = (xx >> 7) & 7`, `y = (yy >> 7) & 7` and
/// `x' = x + 4 * parity`. The parity term cancels the half tile that the
/// 4-pixel hardware scroll moves the page on alternate frames, and the lower
/// band's split offset is always a whole tile, so every screen pixel
/// `(sx, sy)` shows image cell `((sy + 7 + y) & 7, (sx + 4 + x) & 7)` in both
/// bands. The ignored engine test checks this against every scroll frame.
pub fn wall_pattern(sx: usize, sy: usize, oscillator: [f32; 2]) -> usize {
    let x = (oscillator[0] * 128.0) as i32 >> 7;
    let y = (oscillator[1] * 128.0) as i32 >> 7;
    (((sy as i32 + 7 + y) & 7) * 8 + ((sx as i32 + 4 + x) & 7)) as usize
}

/// Texels per source pixel in the scroller's glyph atlas.
pub const GLYPH_SCALE: usize = 4;
/// Source pixels of margin around each 104 × 80 font cell in the atlas.
pub const GLYPH_PAD: usize = 16;
/// Atlas distances clamp here, in source rows, inside the margin.
pub const GLYPH_LIMIT: f32 = 13.0;
/// Atlas cells per row. scroller.wgsl repeats these constants.
pub const GLYPH_COLUMNS: usize = 3;
/// Width of the scroller data texture, in texels.
pub const SCROLL_DATA_WIDTH: usize = 1200;
/// A 320 × 200 frame shown at 4:3 has pixels 5/6 as wide as they are tall.
pub const PIXEL_ASPECT: f32 = 5.0 / 6.0;

/// Letter shapes and tables for the modern scroller, built in memory from the
/// locally loaded executable when the clip loads. Nothing here is stored.
pub struct ScrollerGlyphs {
    /// Two half floats per texel. Red: signed distance to the smoothed outline
    /// of a font cell's letter pixels, in source rows (positive inside). Green:
    /// the letter pixels' original grey, interpolated between letter pixels.
    pub atlas: Vec<f16>,
    pub atlas_size: (u32, u32),
    /// Four floats per texel, `SCROLL_DATA_WIDTH` wide. Row 0: atlas slot for
    /// each character of the text (0 for blank cells) and the distance to the
    /// nearest non-blank cell, in cells; its last texel holds
    /// the text length and the overlay's row period. Row 1: even words of the
    /// overlay's x table, the only ones `overlay()` reads, and the same with
    /// the rounding to whole pixels smoothed. Row 2: each overlay
    /// row's runs of dots as two [start, end) pairs, unused runs at 1000.
    /// Row 3: rows from each overlay row to the nearest row with dots.
    pub data: Vec<f32>,
    pub data_size: (u32, u32),
}

impl ScrollerGlyphs {
    pub fn new(font: &HdFont, palette: &[u8]) -> Result<Self, String> {
        let width = SCROLL_DATA_WIDTH;
        if font.text.len() >= width || font.rows.len() > width || font.sine.len() < 2 * width {
            return Err("Scroller tables exceed the HD data texture".into());
        }
        let cell = (
            (104 + 2 * GLYPH_PAD) * GLYPH_SCALE,
            (80 + 2 * GLYPH_PAD) * GLYPH_SCALE,
        );
        let mut slots = [0_u8; 256];
        let mut cells = Vec::new();
        for (code, pixels) in &font.glyphs {
            let mask: Vec<bool> = pixels.iter().map(|&i| i < 64).collect();
            if !mask.contains(&true) {
                continue;
            }
            cells.push((
                smooth_signed_distance(&mask, 104, 80, GLYPH_SCALE, GLYPH_PAD, 0.65, GLYPH_LIMIT),
                glyph_shade(pixels, palette),
            ));
            slots[*code as usize] = cells.len() as u8;
        }
        let atlas_size = (
            GLYPH_COLUMNS * cell.0,
            cells.len().div_ceil(GLYPH_COLUMNS).max(1) * cell.1,
        );
        let mut atlas = vec![f16::ZERO; atlas_size.0 * atlas_size.1 * 2];
        for (slot, (distance, shade)) in cells.iter().enumerate() {
            let origin = (slot % GLYPH_COLUMNS * cell.0, slot / GLYPH_COLUMNS * cell.1);
            for y in 0..cell.1 {
                for x in 0..cell.0 {
                    let at = ((origin.1 + y) * atlas_size.0 + origin.0 + x) * 2;
                    atlas[at] = f16::from_f32(distance[y * cell.0 + x]);
                    atlas[at + 1] = f16::from_f32(shade[y * cell.0 + x]);
                }
            }
        }
        let mut data = vec![0.0; width * 4 * 4];
        let n = font.text.len();
        for (i, &code) in font.text.iter().enumerate() {
            data[i * 4] = slots[code as usize] as f32;
            // Cells to the nearest letter, around the wrapping text, so the
            // shader can step quickly across blank stretches.
            data[i * 4 + 1] = (0..n)
                .filter(|&j| slots[font.text[j] as usize] != 0)
                .map(|j| (i + n - j) % n)
                .map(|d| d.min(n - d))
                .min()
                .unwrap_or(1000) as f32;
        }
        data[(width - 1) * 4] = font.text.len() as f32;
        data[(width - 1) * 4 + 1] = font.rows.len() as f32;
        // The table holds whole pixels. A short Gaussian over the even words
        // smooths that rounding for the continuous HD curve (authored).
        for k in 0..width {
            let (mut sum, mut total) = (0.0, 0.0);
            for d in -4_i32..=4 {
                let at = (k as i32 + d).clamp(0, width as i32 - 1) as usize;
                let w = (-(d * d) as f32 / 4.5).exp();
                sum += font.sine[2 * at] as f32 * w;
                total += w;
            }
            data[(width + k) * 4] = font.sine[2 * k] as f32;
            data[(width + k) * 4 + 1] = sum / total;
        }
        for (r, dots) in font.rows.iter().enumerate() {
            let mut runs = Vec::new();
            let mut j = 0;
            while j < 12 {
                if dots[j] {
                    let start = j;
                    while j < 12 && dots[j] {
                        j += 1;
                    }
                    runs.extend([start as f32, j as f32]);
                } else {
                    j += 1;
                }
            }
            if runs.len() > 4 {
                return Err(format!("Overlay row {r} has more than two runs"));
            }
            runs.resize(4, 1000.0);
            data[(2 * width + r) * 4..][..4].copy_from_slice(&runs);
            let rows = font.rows.len();
            data[(3 * width + r) * 4] = (0..rows)
                .filter(|&k| font.rows[k].contains(&true))
                .map(|k| (r + rows - k) % rows)
                .map(|d| d.min(rows - d))
                .min()
                .unwrap_or(1000) as f32;
        }
        Ok(Self {
            atlas,
            atlas_size: (atlas_size.0 as u32, atlas_size.1 as u32),
            data,
            data_size: (width as u32, 4),
        })
    }
}

/// The original grey of letter pixels at each atlas texel, bilinearly
/// interpolated between letter pixels only, so edges don't darken.
fn glyph_shade(pixels: &[u8], palette: &[u8]) -> Vec<f32> {
    let width = (104 + 2 * GLYPH_PAD) * GLYPH_SCALE;
    let height = (80 + 2 * GLYPH_PAD) * GLYPH_SCALE;
    let grey = |i: u8| {
        let p = &palette[i as usize * 3..][..3];
        p.iter().map(|&c| (c & 63) as f32).sum::<f32>() / 189.0
    };
    let mut shade = vec![0.0; width * height];
    for y in 0..height {
        let fy = (y as f32 + 0.5) / GLYPH_SCALE as f32 - GLYPH_PAD as f32 - 0.5;
        for x in 0..width {
            let fx = (x as f32 + 0.5) / GLYPH_SCALE as f32 - GLYPH_PAD as f32 - 0.5;
            let (mut sum, mut weight) = (0.0, 0.0);
            let (ty, tx) = (fy - fy.floor(), fx - fx.floor());
            for (dy, wy) in [(0, 1.0 - ty), (1, ty)] {
                for (dx, wx) in [(0, 1.0 - tx), (1, tx)] {
                    let (px, py) = (fx.floor() as i32 + dx, fy.floor() as i32 + dy);
                    if (0..104).contains(&px) && (0..80).contains(&py) {
                        let i = pixels[py as usize * 104 + px as usize];
                        if i < 64 {
                            sum += grey(i) * wx * wy;
                            weight += wx * wy;
                        }
                    }
                }
            }
            if weight > 0.0 {
                shade[y * width + x] = sum / weight;
            }
        }
    }
    shade
}

/// Signed distance to a smoothed outline of a pixel mask, positive inside.
/// Texel (i, j) is centred on source position ((i + 0.5) / scale - pad,
/// (j + 0.5) / scale - pad). The mask is blurred by a Gaussian of `sigma`
/// source pixels and cut at one half, rounding single-pixel stair steps: an
/// authored interpretation of the raster, not recovered vector artwork.
/// Distances are Euclidean on the 4:3 display, in source rows (a column is
/// [`PIXEL_ASPECT`] rows wide), measured to the piecewise-linear outline
/// between texels, and clamped to ±`limit`.
pub fn smooth_signed_distance(
    mask: &[bool],
    width: usize,
    height: usize,
    scale: usize,
    pad: usize,
    sigma: f32,
    limit: f32,
) -> Vec<f32> {
    assert_eq!(mask.len(), width * height);
    let (w, h) = ((width + 2 * pad) * scale, (height + 2 * pad) * scale);
    let centre = |i: usize| (i as f32 + 0.5) / scale as f32 - pad as f32;
    let radius = (3.0 * sigma).ceil() as isize + 1;
    let taps = |count: usize, extent: usize| -> Vec<Vec<(usize, f32)>> {
        (0..count)
            .map(|i| {
                let x = centre(i);
                let mut taps = Vec::new();
                let mut total = 0.0;
                for a in x.floor() as isize - radius..=x.floor() as isize + radius {
                    let g = (-(x - a as f32 - 0.5).powi(2) / (2.0 * sigma * sigma)).exp();
                    total += g;
                    if a >= 0 && (a as usize) < extent {
                        taps.push((a as usize, g));
                    }
                }
                taps.into_iter().map(|(a, g)| (a, g / total)).collect()
            })
            .collect()
    };
    let (tx, ty) = (taps(w, width), taps(h, height));
    let mut rows = vec![0.0_f32; height * w];
    for b in 0..height {
        for i in 0..w {
            rows[b * w + i] = tx[i]
                .iter()
                .filter(|(a, _)| mask[b * width + a])
                .map(|(_, g)| g)
                .sum();
        }
    }
    let mut field = vec![0.0_f32; w * h];
    for j in 0..h {
        for i in 0..w {
            field[j * w + i] = ty[j].iter().map(|&(b, g)| rows[b * w + i] * g).sum::<f32>() - 0.5;
        }
    }
    let point = |i: usize, j: usize| [centre(i) * PIXEL_ASPECT, centre(j)];
    let length = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let mut nearest: Vec<Option<[f32; 2]>> = vec![None; w * h];
    let mut best = vec![f32::INFINITY; w * h];
    // Seed the texels around each 2 x 2 cell with their closest points on the
    // outline segments crossing it (marching squares on the blurred mask).
    for j in 0..h.saturating_sub(1) {
        for i in 0..w.saturating_sub(1) {
            let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let v = corners.map(|(x, y)| field[y * w + x]);
            let crossing: [Option<[f32; 2]>; 4] = std::array::from_fn(|k| {
                let (a, b) = (v[k], v[(k + 1) % 4]);
                ((a >= 0.0) != (b >= 0.0)).then(|| {
                    let t = a / (a - b);
                    let (p, q) = (point(corners[k].0, corners[k].1), {
                        let c = corners[(k + 1) % 4];
                        point(c.0, c.1)
                    });
                    [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
                })
            });
            let mut segments = Vec::new();
            match crossing.iter().flatten().count() {
                2 => {
                    let mut ends = crossing.iter().flatten();
                    segments.push((*ends.next().unwrap(), *ends.next().unwrap()));
                }
                4 => {
                    // Saddle: separate the corners on the other side from the centre.
                    let inside = v.iter().sum::<f32>() >= 0.0;
                    for c in 0..4 {
                        if (v[c] >= 0.0) != inside {
                            segments.push((crossing[(c + 3) % 4].unwrap(), crossing[c].unwrap()));
                        }
                    }
                }
                _ => {}
            }
            for (a, b) in segments {
                for (x, y) in corners {
                    let p = point(x, y);
                    let ab = [b[0] - a[0], b[1] - a[1]];
                    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1])
                        / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12))
                    .clamp(0.0, 1.0);
                    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
                    let d = length(p, q);
                    if d < best[y * w + x] {
                        best[y * w + x] = d;
                        nearest[y * w + x] = Some(q);
                    }
                }
            }
        }
    }
    // Spread the closest outline points (an 8-neighbour sequential sweep).
    let mut adopt = |x: usize, y: usize, dx: isize, dy: isize| {
        let (nx, ny) = (x as isize + dx, y as isize + dy);
        if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
            return;
        }
        if let Some(q) = nearest[ny as usize * w + nx as usize] {
            let d = length(point(x, y), q);
            if d < best[y * w + x] {
                best[y * w + x] = d;
                nearest[y * w + x] = Some(q);
            }
        }
    };
    for _ in 0..2 {
        for y in 0..h {
            for x in 0..w {
                for (dx, dy) in [(-1, 0), (-1, -1), (0, -1), (1, -1)] {
                    adopt(x, y, dx, dy);
                }
            }
            for x in (0..w).rev() {
                adopt(x, y, 1, 0);
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                for (dx, dy) in [(1, 0), (1, 1), (0, 1), (-1, 1)] {
                    adopt(x, y, dx, dy);
                }
            }
            for x in 0..w {
                adopt(x, y, -1, 0);
            }
        }
    }
    field
        .iter()
        .zip(best)
        .map(|(&v, d)| d.min(limit) * if v >= 0.0 { 1.0 } else { -1.0 })
        .collect()
}

/// Removes the counters' wrap-around so positions interpolate across it.
struct ScrollUnwrap {
    raw: [i64; 3],
    position: [f64; 3],
    period: [i64; 3],
}

impl ScrollUnwrap {
    fn new(scroll: HdScroll, period: [i64; 3]) -> Self {
        let raw = [scroll.upper, scroll.lower, scroll.overlay];
        Self {
            raw,
            position: raw.map(|v| v as f64),
            period,
        }
    }

    fn position(&self) -> [f64; 3] {
        self.position
    }

    fn next(&mut self, scroll: HdScroll) -> [f64; 3] {
        let raw = [scroll.upper, scroll.lower, scroll.overlay];
        for (k, position) in self.position.iter_mut().enumerate() {
            let period = self.period[k];
            let mut delta = (raw[k] - self.raw[k]).rem_euclid(period);
            if delta > period / 2 {
                delta -= period;
            }
            *position += delta as f64;
        }
        self.raw = raw;
        self.position
    }
}

/// Measure authored lighting cues from the clip's interleaved stereo PCM, one
/// per 70 Hz window (the same sample split `Clip::load_mode` uses). This is the
/// only analysis of the audio: one causal pass over the mono mix with one
/// filter bank, read by both the intro and the scroller. It reads only the
/// rendered audio, so the music itself is untouched. A prefix of the PCM always
/// gives a prefix of the result.
///
/// The mono mix cannot cancel an instrument against itself: the port's mixer
/// routes each channel to the left and right with non-negative gains.
///
/// All values are new display controls in 0..=1, not anything recovered from
/// the original program, and not tracker events. Intro cues (`intro`):
/// - `[0]` energy: broadband RMS with fast attack and a 0.25 s release.
/// - `[1]` onset: a rise in high-band (above about 150 Hz) energy over its
///   recent level, gated by loudness, held and decayed with a 0.12 s constant.
/// - `[2]` low: RMS below about 150 Hz, fast attack and a 0.18 s release.
/// - `[3]` accent: loudness well above the last two seconds, such as a strong
///   downbeat after a quieter passage, decayed with a 0.25 s constant.
///
/// Scroller cues (`scroller`) for its musical mode, relative to the recent
/// level so loud passages keep their contrast:
/// - `[0]` level: smoothed broadband RMS over its average of about half a
///   second, released in about 0.1 s.
/// - `[1]` bass: a rise in the band below about 150 Hz, decayed in about 0.11 s.
/// - `[2]` treble: a rise in the band above about 1.8 kHz (mostly percussion),
///   decayed in about 0.06 s.
///
/// The time constants and scales were tuned by eye and ear.
pub fn audio_cues(pcm: &[i16]) -> Vec<AudioCues> {
    let rate = SAMPLE_RATE as usize;
    let frames = pcm.len() / 2;
    let windows = (frames * FRAME_RATE as usize).div_ceil(rate);
    let release = |seconds: f64| 1.0 - (-1.0 / (seconds * FRAME_RATE as f64)).exp();
    let decay = |seconds: f64| (-1.0 / (seconds * FRAME_RATE as f64)).exp();
    let crossover = 1.0 - (-std::f64::consts::TAU * 150.0 / rate as f64).exp();
    let treble_cut = 1.0 - (-std::f64::consts::TAU * 1800.0 / rate as f64).exp();
    let (mut lp1, mut lp2, mut treble_lp) = (0.0_f64, 0.0, 0.0);
    let (mut energy, mut low, mut onset, mut accent) = (0.0_f64, 0.0, 0.0, 0.0);
    let (mut recent_high, mut long_power) = (0.0_f64, 0.0);
    // Scroller state per band (broadband, bass, treble): lightly smoothed RMS,
    // its slow average and the previous smoothed value.
    let (mut smooth, mut average) = ([0.0_f64; 3], [0.0_f64; 3]);
    let mut scroller = [0.0_f64; 3];
    let mut result = Vec::with_capacity(windows);
    for i in 0..windows {
        let start = i * rate / FRAME_RATE as usize;
        let end = ((i + 1) * rate / FRAME_RATE as usize).min(frames);
        let (mut power, mut low_power, mut high_power) = (0.0, 0.0, 0.0);
        let mut treble_power = 0.0;
        for frame in pcm[start * 2..end * 2].chunks_exact(2) {
            let mono = (frame[0] as f64 + frame[1] as f64) / 65536.0;
            lp1 += crossover * (mono - lp1);
            lp2 += crossover * (lp1 - lp2);
            treble_lp += treble_cut * (mono - treble_lp);
            let high = mono - lp2;
            let treble = mono - treble_lp;
            power += mono * mono;
            low_power += lp2 * lp2;
            high_power += high * high;
            treble_power += treble * treble;
        }
        let count = (end - start).max(1) as f64;
        let (power, low_power, high_power) = (power / count, low_power / count, high_power / count);
        let treble_power = treble_power / count;
        let follow = |state: &mut f64, value: f64, seconds: f64| {
            *state += (value - *state)
                * if value > *state {
                    0.5
                } else {
                    release(seconds)
                };
        };
        follow(&mut energy, power.sqrt(), 0.25);
        follow(&mut low, low_power.sqrt(), 0.18);
        // Near-silent material cannot trigger pulses, however sudden.
        let gate = (high_power.sqrt() / 0.1).min(1.0);
        let flux = ((high_power + 1e-6) / (recent_high + 1e-6)).ln().max(0.0);
        onset = ((flux / 1.2).min(1.0) * gate).max(onset * decay(0.12));
        recent_high += (high_power - recent_high) * 0.15;
        let rise = ((power + 1e-6) / (long_power + 1e-6)).ln() / 6.0_f64.ln();
        accent = (((rise - 0.35) / 0.65).clamp(0.0, 1.0) * gate).max(accent * decay(0.25));
        long_power += (power - long_power) * release(2.0);
        for (band, band_power) in [power, low_power, treble_power].into_iter().enumerate() {
            let previous = smooth[band];
            smooth[band] += (band_power.sqrt() - previous) * 0.6;
            average[band] += (smooth[band] - average[band]) * 0.025;
            if band == 0 {
                // A floor keeps near-silence from reading as a loud level.
                let target = ((smooth[0] / (average[0] + 0.035) - 0.65) * 0.65).clamp(0.0, 1.0);
                scroller[0] +=
                    (target - scroller[0]) * if target > scroller[0] { 0.45 } else { 0.12 };
            } else {
                // The bass band is the intro's steeper low band; its higher
                // floor and threshold keep about the old analyser's activity.
                let (floor, threshold, hold) = if band == 1 {
                    (0.03, 0.08, 0.88)
                } else {
                    (0.015, 0.045, 0.80)
                };
                let rise = (smooth[band] - previous) / (average[band] + floor);
                let target = ((rise - threshold) * 4.0).clamp(0.0, 1.0);
                scroller[band] = target.max(scroller[band] * hold);
            }
        }
        result.push(AudioCues {
            intro: [
                (1.0 - (-energy / 0.25).exp()) as f32,
                onset as f32,
                (1.0 - (-low / 0.12).exp()) as f32,
                accent as f32,
            ],
            scroller: scroller.map(|v| v as f32),
        });
    }
    result
}

/// Fit the original FUTURE CREW fly-bys, not a replacement animation. The
/// executable contains projected delta frames, not recoverable 3D keyframes.
/// Constant-depth-speed perspective is an interpretation fitted to those
/// frames. No original silhouettes or pose tables are embedded in the shader.
fn fit_flights(frames: &[Frame]) -> Result<[[f32; 4]; 10], String> {
    let cues = [58, 71, 80, 91, 103, 113, 140, 152, 165, 180];
    let widths = [0.60, 0.88, 0.78, 0.88, 0.86, 0.60, 0.80, 0.86, 0.60, 1.50];
    let mut result = [[0.0; 4]; 10];
    for (letter, cue) in cues.into_iter().enumerate() {
        let mut samples = Vec::new();
        for frame in frames.iter().filter(|f| {
            f.state.phase == 1
                && f.state.tick == 0
                && f.state.frame >= cue + 3
                && f.state.frame <= cue + 9
        }) {
            let points: Vec<(f64, f64)> = frame
                .indices
                .iter()
                .enumerate()
                .filter(|(_, v)| **v >= 229)
                .map(|(i, _)| ((i % WIDTH) as f64, (i / WIDTH) as f64))
                .collect();
            if points.is_empty() {
                continue;
            }
            let min_x = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
            let max_x = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            let min_y = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
            let max_y = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
            let height = max_y - min_y + 1.0;
            if height < 10.0 || min_y <= 0.0 || max_y >= 199.0 {
                continue;
            }
            let x = if letter < 7 {
                (min_x - 160.0) / height * 0.94 * 5.0 / 6.0 + widths[letter] / 2.0
            } else {
                (max_x + 1.0 - 160.0) / height * 0.94 * 5.0 / 6.0 - widths[letter] / 2.0
            };
            let y = -((min_y + max_y + 1.0) / 2.0 - 100.0) / height * 0.94;
            samples.push((frame.state.frame as f64, 1.0 / height, x, y));
        }
        if samples.len() < 3 {
            return Err(format!("Cannot fit reference flight {letter}"));
        }
        let n = samples.len() as f64;
        let mean_t = samples.iter().map(|s| s.0).sum::<f64>() / n;
        let mean_h = samples.iter().map(|s| s.1).sum::<f64>() / n;
        let slope = samples
            .iter()
            .map(|s| (s.0 - mean_t) * (s.1 - mean_h))
            .sum::<f64>()
            / samples.iter().map(|s| (s.0 - mean_t).powi(2)).sum::<f64>();
        if slope >= 0.0 {
            return Err(format!("Reference flight {letter} is not approaching"));
        }
        result[letter] = [
            (mean_t - mean_h / slope) as f32,
            (-slope * 100.0 * 3.8 * 0.94) as f32,
            (samples.iter().map(|s| s.2).sum::<f64>() / n) as f32,
            (samples.iter().map(|s| s.3).sum::<f64>() / n) as f32,
        ];
    }
    Ok(result)
}

/// Continuous authored reconstruction of the plate's tumble, not recovered
/// 3D keyframes. The original Intro::decode plays a raster delta stream.
/// A near-diagonal axis, roughly 70 frames per turn and constant forward speed
/// follow that stream's two turns and accelerating perspective approach. Unlike
/// per-frame silhouette PCA/acos, a quaternion preserves full turns and their
/// direction through edge-on views. Frame 139 lands face-on for the logo cue.
pub fn plate_transform(frame: f32) -> ([f32; 4], [f32; 4]) {
    let remaining = 139.0 - frame;
    let angle = -remaining * std::f32::consts::TAU / 70.0;
    let axis = [0.57_f32, 0.57, 0.59];
    let length = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
    let (s, c) = (angle * 0.5).sin_cos();
    (
        [0.0, 0.0, -remaining * 2.5, 0.0],
        [
            axis[0] / length * s,
            axis[1] / length * s,
            axis[2] / length * s,
            c,
        ],
    )
}

/// Signed chamfer distance, positive inside. Distances are in source-pixel units.
/// Border pixels are treated as having outside neighbours, including full masks.
pub fn signed_distance(mask: &[bool], w: usize, h: usize) -> Vec<f32> {
    assert_eq!(mask.len(), w * h);
    let distance = |inside: bool| {
        let mut d: Vec<f32> = mask
            .iter()
            .map(|&p| if p == inside { 1000.0 } else { 0.0 })
            .collect();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if inside && (x == 0 || y == 0 || x + 1 == w || y + 1 == h) {
                    d[i] = d[i].min(1.0);
                }
                if x > 0 {
                    d[i] = d[i].min(d[i - 1] + 1.0);
                }
                if y > 0 {
                    d[i] = d[i].min(d[i - w] + 1.0);
                    if x > 0 {
                        d[i] = d[i].min(d[i - w - 1] + std::f32::consts::SQRT_2);
                    }
                    if x + 1 < w {
                        d[i] = d[i].min(d[i - w + 1] + std::f32::consts::SQRT_2);
                    }
                }
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                if x + 1 < w {
                    d[i] = d[i].min(d[i + 1] + 1.0);
                }
                if y + 1 < h {
                    d[i] = d[i].min(d[i + w] + 1.0);
                    if x > 0 {
                        d[i] = d[i].min(d[i + w - 1] + std::f32::consts::SQRT_2);
                    }
                    if x + 1 < w {
                        d[i] = d[i].min(d[i + w + 1] + std::f32::consts::SQRT_2);
                    }
                }
            }
        }
        d
    };
    let a = distance(true);
    let b = distance(false);
    a.iter()
        .zip(b)
        .map(|(&a, b)| {
            if a > 0.0 {
                (a - 0.5).min(32.0)
            } else {
                -(b - 0.5).min(32.0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wall_pattern_follows_whole_oscillator_pixels() {
        // Pixel (0, 0) shows image cell (7, 4) at rest.
        assert_eq!(wall_pattern(0, 0, [0.0, 0.0]), 7 * 8 + 4);
        // Fractions of a pixel don't move the original; whole ones do.
        assert_eq!(wall_pattern(0, 0, [0.99, 0.5]), 7 * 8 + 4);
        assert_eq!(wall_pattern(0, 0, [1.0, 1.0]), 5);
        // Negative positions round down, as the arithmetic shift does.
        assert_eq!(wall_pattern(0, 0, [-0.5, -1.0 / 128.0]), 6 * 8 + 3);
        // The image repeats every 8 pixels in both directions.
        assert_eq!(
            wall_pattern(13, 21, [-37.25, 99.0]),
            wall_pattern(13 + 8, 21 + 16, [-37.25, 99.0])
        );
    }

    #[test]
    fn oscillator_lead_in_runs_forward_into_the_starting_state() {
        use crate::scenes::scroller::oscillator_step;
        let start = [[0, 384], [12800, 0]];
        let lead_in = oscillator_lead_in(start, 300);
        assert_eq!(lead_in.len(), 300);
        // Stepping forward through the lead-in reaches the start exactly.
        for pair in lead_in.windows(2) {
            assert_eq!(pair[0].map(oscillator_step), pair[1]);
        }
        assert_eq!(lead_in[299].map(oscillator_step), start);
        // It keeps moving (each axis rests only where its speed passes
        // zero), within the original's range of motion.
        let still = lead_in.windows(2).filter(|p| p[0][0][0] == p[1][0][0]);
        assert!(still.count() < 3);
        assert!(
            lead_in
                .iter()
                .flatten()
                .all(|[p, _]| p.unsigned_abs() < 20_000)
        );
        assert!(oscillator_lead_in(start, 0).is_empty());
    }

    #[test]
    fn scroller_backdrop_options_cycle() {
        let mut backdrop: ScrollerBackdrop = "chamber".parse().unwrap();
        for expected in [
            ScrollerBackdrop::Wall,
            ScrollerBackdrop::Chrome,
            ScrollerBackdrop::Chamber,
        ] {
            backdrop = backdrop.next();
            assert_eq!(backdrop, expected);
        }
        assert_eq!("wall".parse(), Ok(ScrollerBackdrop::Wall));
        assert_eq!("chrome".parse(), Ok(ScrollerBackdrop::Chrome));
        assert!("balls".parse::<ScrollerBackdrop>().is_err());
    }

    #[test]
    fn wave_envelope_eases_in_with_the_reveal_and_out_before_the_fade() {
        let at = |t| wave_envelope(t, 18.0, 22.0, 40.0);
        assert_eq!(at(10.0), 0.0);
        assert_eq!(at(18.0), 0.0);
        assert_eq!(at(22.0), 1.0);
        assert_eq!(at(30.0), 1.0);
        assert_eq!(at(40.0 - WAVE_SETTLE), 1.0);
        assert_eq!(at(40.0), 0.0);
        assert_eq!(at(45.0), 0.0);
        assert!((at(20.0) - 0.5).abs() < 1e-6);
        let mut previous = 0.0;
        for step in 0..=3000 {
            let t = 17.0 + step as f64 * 0.01;
            let value = at(t);
            assert!((value - previous).abs() < 0.011, "jump at {t}");
            assert!(if t < 30.0 {
                value >= previous
            } else {
                value <= previous
            });
            previous = value;
        }
    }

    #[test]
    fn wall_waves_ease_their_depth_and_light_and_vanish_when_off() {
        let mut scroll = ScrollerPreview {
            start_tick: 280,
            end_tick: 840,
            lighting: ScrollerLighting::Studio,
            scroll: Vec::new(),
            glyphs: ScrollerGlyphs::new(&synthetic_font(), &vec![0; 768]).unwrap(),
            backdrop_style: ScrollerBackdrop::Chrome,
            wall_shape: 0.5,
            wall_strength: 0.6,
            waves: false,
            wave_depth: 0.7,
            wave_light: 0.3,
            backdrop: Vec::new(),
            oscillator: Vec::new(),
        };
        for step in 0..1300 {
            let time = step as f64 * 0.01;
            assert_eq!(scroll.waves_at(time, 70), [0.0; 2]);
        }
        scroll.waves = true;
        for step in 0..1300 {
            let time = step as f64 * 0.01;
            let ease = wave_envelope(time, 1.0, 4.0, 12.0);
            assert_eq!(scroll.waves_at(time, 70), [0.7 * ease, 0.3 * ease]);
        }
        assert_eq!(scroll.waves_at(6.0, 70), [0.7, 0.3]);
        assert_eq!(scroll.waves_at(0.5, 70), [0.0; 2]);
    }

    #[test]
    fn wall_waves_swing_from_their_depth_to_the_opposite_and_back() {
        let scroll = ScrollerPreview {
            start_tick: 280,
            end_tick: 840,
            lighting: ScrollerLighting::Studio,
            scroll: Vec::new(),
            glyphs: ScrollerGlyphs::new(&synthetic_font(), &vec![0; 768]).unwrap(),
            backdrop_style: ScrollerBackdrop::Chrome,
            wall_shape: 0.5,
            wall_strength: 0.6,
            waves: true,
            wave_depth: 0.7,
            wave_light: 0.3,
            backdrop: Vec::new(),
            oscillator: Vec::new(),
        };
        // From the reveal at tick 70 (1 s): full, flat, turned over, flat,
        // full again.
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(close(scroll.wave_swing(1.0, 70), 1.0));
        assert!(close(scroll.wave_swing(1.0 + WAVE_TURN / 4.0, 70), 0.0));
        assert!(close(scroll.wave_swing(1.0 + WAVE_TURN / 2.0, 70), -1.0));
        assert!(close(scroll.wave_swing(1.0 + WAVE_TURN * 0.75, 70), 0.0));
        assert!(close(scroll.wave_swing(1.0 + WAVE_TURN, 70), 1.0));
        // Smooth: no step between nearby moments.
        for step in 0..2000 {
            let time = step as f64 * 0.01;
            let d = scroll.wave_swing(time + 0.01, 70) - scroll.wave_swing(time, 70);
            assert!(d.abs() < 0.01);
        }
    }

    #[test]
    fn scroller_cues_keep_accents_in_a_loud_mix_and_release_to_silence() {
        let pcm: Vec<i16> = (0..SAMPLE_RATE * 4)
            .flat_map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                let phase = std::f32::consts::TAU * t;
                let bed = if t < 3.0 {
                    0.32 * (phase * 440.0).sin() + 0.12 * (phase * 880.0).sin()
                } else {
                    0.0
                };
                let age = t - t.floor();
                let hit = if (1.0..3.0).contains(&t) {
                    0.30 * (-age / 0.07).exp() * (phase * 75.0).sin()
                        + 0.18 * (-age / 0.025).exp() * (phase * 3000.0).sin()
                } else {
                    0.0
                };
                let sample = ((bed + hit) * 32767.0) as i16;
                [sample, sample]
            })
            .collect();
        let cues = audio_cues(&pcm);
        assert_eq!(cues.len(), 280);
        let signal: Vec<[f32; 3]> = cues.iter().map(|c| c.scroller).collect();
        assert!(
            signal
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        );
        for band in 1..3 {
            let peak = signal[70..77]
                .iter()
                .map(|s| s[band])
                .fold(0.0_f32, f32::max);
            let rest = signal[119..133]
                .iter()
                .map(|s| s[band])
                .fold(0.0_f32, f32::max);
            assert!(
                peak > 0.65 && rest < 0.15,
                "band {band}: peak {peak}, rest {rest}"
            );
        }
        assert!(
            signal[60][0] < 0.6,
            "A loud sustained bed must not pin the level"
        );
        assert!(signal[279].iter().all(|v| *v < 0.005));
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn scroller_matches_complete_engine_reference_and_pcm() {
        let data = crate::test_support::data();
        let clip = Clip::load_scroller(&data, ScrollerLighting::Musical).unwrap();
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for (i, frame) in clip.frames.iter().enumerate() {
            assert_eq!(engine.video.indices, frame.indices, "pixels at {i}");
            assert_eq!(engine.video.palette, frame.palette, "palette at {i}");
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(pcm, clip.pcm);
        assert_eq!(engine.part(), 6, "Stop exactly at the next scene boundary");
        let scroll = clip.scroller.as_ref().unwrap();
        // One shared analysis covers every frame, for the intro and scroller.
        assert_eq!(clip.audio.len(), clip.frames.len());
        assert_eq!(scroll.backdrop.len(), clip.frames.len());
        assert_eq!(scroll.oscillator.len(), clip.frames.len());
        // The original's first palette() call is shown one frame into the
        // scroll, at the starting state (xx 0, yy 12800). Before it, the
        // authored lead-in runs the oscillator backwards; from it on, the
        // values are the original's (the pixel check below). Each axis moves
        // by its speed and the speed changes by exactly 4/128 every frame, so
        // there is no jump or kink anywhere, including across the start.
        let first = scroll.start_tick + 1;
        assert_eq!(scroll.oscillator[first], [0.0, 100.0]);
        let back = oscillator_lead_in([[0, 384], [12800, 0]], first);
        for (i, state) in back.iter().enumerate() {
            let position = state.map(|[p, _]| p as f32 / 128.0);
            assert_eq!(scroll.oscillator[i], position, "lead-in at {i}");
        }
        for (i, w) in scroll.oscillator.windows(3).enumerate() {
            for ((a, b), c) in w[0].iter().zip(w[1]).zip(w[2]) {
                let accel = (c - 2.0 * b + a) * 128.0;
                assert_eq!(accel.abs(), 4.0, "frame {}", i + 1);
            }
        }
        for i in first - 3..first + 3 {
            let [a, b] = [scroll.oscillator[i], scroll.oscillator[i + 1]];
            println!(
                "frame {i} ({:.3} s): x {:8.3} y {:8.3} px; speed x {:+.3} y {:+.3} px/frame",
                i as f64 / 70.0,
                a[0],
                a[1],
                b[0] - a[0],
                b[1] - a[1]
            );
        }
        // The HD wall replaces the palette window with an image and an
        // oscillator position; check that they reproduce every background
        // pixel the engine displays while the wall image is shown.
        let (mut checked, mut seam) = (0, 0);
        for (i, frame) in clip.frames.iter().enumerate() {
            // The first scroll frame still shows the intro's grey entries.
            if frame.state.phase < 8 || i == scroll.start_tick {
                continue;
            }
            for (at, &index) in frame.indices.iter().enumerate() {
                if !(64..128).contains(&index) {
                    continue;
                }
                let (x, y) = (at % WIDTH, at / WIDTH);
                let cell = wall_pattern(x, y, scroll.oscillator[i]);
                let same = (0..3).all(|channel| {
                    vga_linear(frame.palette[index as usize * 3 + channel])
                        == scroll.backdrop[i][cell][channel]
                });
                // Once per 80 frames the split carries the lower band's last
                // byte column into the next page row: a one-frame, 4-pixel
                // seam at the right edge that the HD wall does not reproduce.
                if !same {
                    assert!(
                        x >= 316 && y >= 100,
                        "background pixel {x},{y} at frame {i}"
                    );
                    seam += 1;
                }
                checked += 1;
            }
        }
        assert!(checked > 10_000_000, "checked {checked} pixels");
        assert!(seam < checked / 10_000, "{seam} right-edge seam pixels");
        assert!(
            clip.audio
                .iter()
                .flat_map(|c| c.intro.into_iter().chain(c.scroller))
                .all(|e| e.is_finite() && (0.0..=1.0).contains(&e))
        );
        // The scroller's musical mode has accents to show during the text.
        let text = &clip.audio[scroll.start_tick..scroll.end_tick];
        for band in 1..3 {
            let lit = text.iter().filter(|c| c.scroller[band] > 0.5).count();
            assert!(lit > 20 && lit < text.len() / 2, "band {band}: {lit}");
        }
        assert_eq!(clip.frames[scroll.start_tick].state.phase, 8);
        assert_eq!(clip.frames[scroll.end_tick].state.phase, 9);
        assert_eq!(scroll.scroll.len(), clip.frames.len());
        println!(
            "Scroller {:.3}–{:.3}s; {} frames and full PCM match",
            scroll.start_tick as f64 / 70.0,
            clip.duration(),
            clip.frames.len()
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn scroller_glyphs_and_positions_rebuild_every_reference_frame() {
        // Rebuild both rows and the overlay of every part-2 frame from the font,
        // text and tables alone, placed by Scroller::hd_scroll, and compare
        // them with the reference scanout pixel for pixel.
        let data = crate::test_support::data();
        let clip = Clip::load_scroller(&data, ScrollerLighting::Studio).unwrap();
        let font = Scroller::new(&data, &mut Vga::new()).unwrap().hd_font();
        let preview = clip.scroller.as_ref().unwrap();
        let strip = (font.text.len() * 104) as i64;
        let pixel = |u: i64, row: usize| {
            let u = u.rem_euclid(strip) as usize;
            let code = font.text[u / 104];
            let cell = &font.glyphs.iter().find(|g| g.0 == code).unwrap().1;
            cell[row * 104 + u % 104]
        };
        let (mut letters, mut dots, mut skipped) = (0, 0, 0);
        for (n, frame) in clip.frames.iter().enumerate() {
            if frame.state.phase < 8 {
                continue;
            }
            let [upper, lower, overlay] = preview.scroll[n].map(|v| v as i64);
            for row in 0..80 {
                for x in 0..320 {
                    // Columns the original has not written yet show the backdrop.
                    let u = x as i64 + upper;
                    if u.div_euclid(4) >= 1 {
                        let got = frame.indices[(row + 8) * WIDTH + x] & 127;
                        assert_eq!(got, pixel(u, row), "upper {x},{row} at frame {n}");
                        letters += 1;
                    }
                    // When `a` wraps, the lower window starts one byte past
                    // its usual range, and its last byte shows the next glyph
                    // row (an original one-frame artefact, not reproduced).
                    let wrapped = x >= 316 && (1 - lower / 4) % 80 == 0;
                    let u = x as i64 + lower;
                    if u.div_euclid(4) <= 0 && !wrapped {
                        let got = frame.indices[(row + 112) * WIDTH + x] & 127;
                        assert_eq!(got, pixel(u, row), "lower {x},{row} at frame {n}");
                        letters += 1;
                    }
                }
            }
            // The overlay as overlay() drew it at step `overlay`: one 12-dot row
            // every four lines, only even words of the x table.
            let mut expected = vec![false; WIDTH * HEIGHT];
            if overlay >= 0 {
                let y = overlay as usize;
                let t = y / 4 % font.rows.len();
                for i in 0..50 {
                    let line = 4 + 4 * i - (y & 3);
                    let x = font.sine[(2048 - y - (y & 3) + 4 * i) & 32767] as usize;
                    let dots = font.rows[(t + i) % font.rows.len()];
                    for (j, _) in dots.iter().enumerate().filter(|d| *d.1) {
                        if line < HEIGHT {
                            expected[line * WIDTH + x + 4 * j] = true;
                        }
                    }
                }
            }
            for line in 0..HEIGHT {
                // overlay() places its row 24 on line 100 without the lower
                // band's offset, so the scanout shows it displaced there.
                if line == 100 && overlay >= 0 && overlay & 3 == 0 {
                    skipped += 1;
                    continue;
                }
                for x in 0..WIDTH {
                    let got = frame.indices[line * WIDTH + x] & 128 != 0;
                    assert_eq!(got, expected[line * WIDTH + x], "dot {x},{line} at {n}");
                    dots += got as usize;
                }
            }
        }
        assert!(letters > 50_000_000 && dots > 100_000, "{letters} {dots}");
        // The smoothed outlines keep each glyph's pixels: compare the atlas
        // sign at every pixel centre with the font's letter mask.
        let glyphs = &preview.glyphs;
        let cell = (
            (104 + 2 * GLYPH_PAD) * GLYPH_SCALE,
            (80 + 2 * GLYPH_PAD) * GLYPH_SCALE,
        );
        let (mut pixels, mut flipped) = (0, 0);
        for (slot, (_, cell_pixels)) in font
            .glyphs
            .iter()
            .filter(|(_, p)| p.iter().any(|&i| i < 64))
            .enumerate()
        {
            let origin = (slot % GLYPH_COLUMNS * cell.0, slot / GLYPH_COLUMNS * cell.1);
            for y in 0..80 {
                for x in 0..104 {
                    let texel = |v: usize| (v + GLYPH_PAD) * GLYPH_SCALE + GLYPH_SCALE / 2;
                    let at =
                        (origin.1 + texel(y)) * glyphs.atlas_size.0 as usize + origin.0 + texel(x);
                    let inside = glyphs.atlas[at * 2].to_f32() > 0.0;
                    pixels += 1;
                    flipped += (inside != (cell_pixels[y * 104 + x] < 64)) as usize;
                }
            }
        }
        assert!(
            flipped * 200 < pixels,
            "{flipped} of {pixels} pixel centres changed side"
        );
        println!(
            "Rebuilt {letters} letter pixels and {dots} overlay dots; {skipped} split rows; \
             smoothing moved {flipped} of {pixels} glyph pixel centres across the outline"
        );
    }

    fn synthetic_font() -> HdFont {
        let mut block = vec![70; 104 * 80];
        for y in 10..70 {
            for x in 8..90 {
                block[y * 104 + x] = 20;
            }
        }
        let mut rows = vec![[false; 12]; 375];
        rows[100] = std::array::from_fn(|j| !(3..9).contains(&j));
        rows[101] = [true; 12];
        HdFont {
            text: vec![0, 5, 0, 0, 5],
            glyphs: vec![(0, vec![70; 104 * 80]), (5, block)],
            sine: (0..2400).map(|i| 100 + (i as u16 / 2) % 3).collect(),
            rows,
        }
    }

    #[test]
    fn smoothed_outline_distances_are_euclidean_on_the_4_3_display() {
        // A 20 x 10 pixel block with a one-pixel staircase on its right side.
        let (w, h) = (40, 30);
        let mut mask = vec![false; w * h];
        for y in 10..20 {
            for x in 10..30 + (y - 10) {
                mask[y * w + x] = true;
            }
        }
        let (scale, pad) = (4, 4);
        let d = smooth_signed_distance(&mask, w, h, scale, pad, 0.65, 6.0);
        let tw = (w + 2 * pad) * scale;
        // Texel centre for a source position given in quarter pixels.
        let at = |qx: usize, qy: usize| d[(qy + pad * scale) * tw + qx + pad * scale];
        assert!(at(60, 60) > 0.0 && at(4, 4) < 0.0);
        // Left edge at x = 10: horizontal distances are 5/6 of a row per column.
        for qx in [30, 38, 42, 50] {
            let expected = ((qx as f32 + 0.5) / 4.0 - 10.0) * PIXEL_ASPECT;
            let got = at(qx, 60);
            assert!((got - expected).abs() < 0.03, "{qx}: {got} vs {expected}");
        }
        // Top edge at y = 10: vertical distances are in rows.
        for (qy, expected) in [(30, -2.375), (39, -0.125), (45, 1.375)] {
            let got = at(80, qy);
            assert!((got - expected).abs() < 0.03, "{qy}: {got}");
        }
        // The staircase becomes a straight diagonal through the middles of
        // its risers, within a tenth of a pixel.
        for y in 12..18 {
            let qy = y * 4 + 2;
            let qx = (100..160).find(|&qx| at(qx, qy) < 0.0).unwrap();
            let (a, b) = (at(qx - 1, qy), at(qx, qy));
            let crossing = (qx as f32 - 0.5 + a / (a - b)) / 4.0;
            let ideal = 30.0 + (qy as f32 + 0.5) / 4.0 - 10.5;
            assert!(
                (crossing - ideal).abs() < 0.1,
                "row {y}: {crossing} vs {ideal}"
            );
        }
        assert!(d.iter().all(|v| v.abs() <= 6.0));
        assert_eq!(d[0], -6.0);
    }

    #[test]
    fn scroller_glyphs_pack_atlas_slots_tables_and_runs() {
        let font = synthetic_font();
        let glyphs = ScrollerGlyphs::new(&font, &vec![32; 768]).unwrap();
        let cell = (
            (104 + 2 * GLYPH_PAD) * GLYPH_SCALE,
            (80 + 2 * GLYPH_PAD) * GLYPH_SCALE,
        );
        // Blank cells take no atlas space.
        assert_eq!(glyphs.atlas_size, ((3 * cell.0) as u32, cell.1 as u32));
        assert_eq!(glyphs.data_size, (SCROLL_DATA_WIDTH as u32, 4));
        let texel = |x: usize, y: usize| &glyphs.data[(y * SCROLL_DATA_WIDTH + x) * 4..][..4];
        let slots: Vec<_> = (0..5).map(|i| texel(i, 0)[0]).collect();
        assert_eq!(slots, [0.0, 1.0, 0.0, 0.0, 1.0]);
        let nearest: Vec<_> = (0..5).map(|i| texel(i, 0)[1]).collect();
        assert_eq!(
            nearest,
            [1.0, 0.0, 1.0, 1.0, 0.0],
            "Distances wrap round the text"
        );
        assert_eq!(&texel(SCROLL_DATA_WIDTH - 1, 0)[..2], &[5.0, 375.0]);
        // Raw even words, and the same smoothed.
        assert_eq!(texel(7, 1)[0], font.sine[14] as f32);
        assert!((texel(7, 1)[1] - 101.0).abs() < 0.5);
        assert_eq!(texel(100, 2), &[0.0, 3.0, 9.0, 12.0]);
        assert_eq!(texel(101, 2), &[0.0, 12.0, 1000.0, 1000.0]);
        assert_eq!(texel(0, 2), &[1000.0; 4]);
        assert_eq!(
            [texel(99, 3)[0], texel(100, 3)[0], texel(98, 3)[0]],
            [1.0, 0.0, 2.0]
        );
        // The block glyph's distance is positive at its centre, grey from the palette.
        let texel_of = |v: usize| (v + GLYPH_PAD) * GLYPH_SCALE + GLYPH_SCALE / 2;
        let centre = texel_of(40) * glyphs.atlas_size.0 as usize + texel_of(49);
        assert!(glyphs.atlas[centre * 2].to_f32() > 10.0);
        assert!((glyphs.atlas[centre * 2 + 1].to_f32() - 32.0 / 63.0).abs() < 0.01);
        let mut wide = synthetic_font();
        wide.rows[7] = std::array::from_fn(|j| j % 4 == 0);
        assert!(ScrollerGlyphs::new(&wide, &vec![32; 768]).is_err());
    }

    #[test]
    fn scroll_positions_unwrap_and_interpolate_between_frames() {
        let at = |upper, lower, overlay| HdScroll {
            upper,
            lower,
            overlay,
        };
        let mut unwrap = ScrollUnwrap::new(at(-320, 8, -2), [520, 520, 2048]);
        assert_eq!(unwrap.next(at(-316, 524, -1)), [-316.0, 4.0, -1.0]);
        assert_eq!(unwrap.next(at(-312, 520, 0)), [-312.0, 0.0, 0.0]);
        assert_eq!(unwrap.next(at(-312, 516, 2047)), [-312.0, -4.0, -1.0]);
        let mut clip = Clip {
            frames: Vec::new(),
            pcm: Vec::new(),
            logo_at: 0.0,
            logo_ready_at: 0.0,
            transition: None,
            scroller: None,
            flights: [[0.0; 4]; 10],
            audio: Vec::new(),
            rings: None,
            field: None,
            dots: None,
        };
        assert_eq!(clip.scroll_at(1.0), None);
        clip.scroller = Some(ScrollerPreview {
            start_tick: 0,
            end_tick: 3,
            lighting: ScrollerLighting::Studio,
            scroll: vec![
                [0.0, 8.0, 0.0],
                [4.0, 4.0, 1.0],
                [8.0, 0.0, 2.0],
                [8.0, 0.0, 2.0],
            ],
            glyphs: ScrollerGlyphs::new(&synthetic_font(), &vec![0; 768]).unwrap(),
            backdrop_style: ScrollerBackdrop::Chamber,
            wall_shape: 0.5,
            wall_strength: 0.4,
            waves: false,
            wave_depth: 0.5,
            wave_light: 0.5,
            backdrop: Vec::new(),
            oscillator: Vec::new(),
        });
        // Each frame's own position at the moment it appears, linear between.
        let near = |time: f64, expected: [f64; 3]| {
            let got = clip.scroll_at(time).unwrap();
            assert!(
                got.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-9),
                "{time}: {got:?}"
            );
        };
        near(1.0 / 70.0, [4.0, 4.0, 1.0]);
        near(1.25 / 70.0, [5.0, 3.0, 1.25]);
        near(2.5 / 70.0, [8.0, 0.0, 2.0]); // Holds when the counters stop.
        near(-1.0, [0.0, 8.0, 0.0]);
        near(10.0, [8.0, 0.0, 2.0]);
    }

    #[test]
    fn faithful_backdrop_replaces_letters_and_dots_with_the_tiles() {
        let mut frame = Frame {
            state: Presentation {
                phase: 8,
                frame: 1700,
                tick: 0,
                door: 76,
            },
            indices: (0..WIDTH * HEIGHT)
                .map(|at| (64 + at % WIDTH % 8 + 8 * (at / WIDTH % 8)) as u8)
                .collect(),
            palette: (0..768).map(|i| (i / 3 % 64) as u8).collect(),
        };
        let tiles = frame.backdrop();
        for y in 30..60 {
            for x in 100..140 {
                frame.indices[y * WIDTH + x] = 20;
            }
        }
        frame.indices[150 * WIDTH + 200] |= 128;
        let original = frame.rgba();
        assert_eq!(frame.backdrop(), tiles);
        assert_eq!(frame.rgba(), original, "The reference picture is unchanged");
        assert!((tiles[(41 * WIDTH + 121) * 4].to_f32() - 36.0 / 255.0).abs() < 1e-3);
        assert_eq!(frame.shape_key(), (8, 1700));
    }

    #[test]
    fn shaders_repeat_the_glyph_atlas_layout() {
        let scroller = include_str!("scroller.wgsl");
        for constant in [
            format!("const ATLAS_SCALE={GLYPH_SCALE}.0;"),
            format!("const ATLAS_PAD={GLYPH_PAD}.0;"),
            format!("const ATLAS_COLUMNS={GLYPH_COLUMNS}u;"),
            format!("const FAR={GLYPH_LIMIT:.1};"),
            format!("table({},0)", SCROLL_DATA_WIDTH - 1),
        ] {
            assert!(
                scroller.contains(&constant),
                "scroller.wgsl lacks {constant}"
            );
        }
        let extent = "const FIELD_EXTENT=vec2f(2.2,1.3);";
        assert!(scroller.contains(extent) && include_str!("intro.wgsl").contains(extent));
    }

    #[test]
    fn plate_tumble_is_continuous_and_lands_face_on() {
        let mut previous = plate_transform(0.0);
        for step in 1..=1390 {
            let current = plate_transform(step as f32 / 10.0);
            let quaternion = current.1;
            assert!((quaternion.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 1e-5);
            // Adjacent orientations must remain close even at half-turns and
            // edge-on projections, where the old PCA/acos pose could flip.
            let dot: f32 = previous.1.iter().zip(quaternion).map(|(a, b)| a * b).sum();
            assert!(dot > 0.9999, "rotation discontinuity at step {step}");
            assert!((current.0[2] - previous.0[2] - 0.25).abs() < 0.0001);
            previous = current;
        }
        assert_eq!(previous, ([0.0; 4], [0.0, 0.0, 0.0, 1.0]));
        let q = plate_transform(104.0).1; // Half a full turn before arrival.
        assert!(q[3].abs() < 1e-6);
        assert!(q[..3].iter().all(|v| v.abs() > 0.5));
    }

    fn stereo(mono: impl IntoIterator<Item = f64>) -> Vec<i16> {
        mono.into_iter()
            .flat_map(|v| {
                let s = (v * 32767.0).round() as i16;
                [s, s]
            })
            .collect()
    }

    fn window(i: usize) -> usize {
        i * SAMPLE_RATE as usize / FRAME_RATE as usize
    }

    #[test]
    fn silence_gives_no_audio_lighting() {
        let cues = audio_cues(&vec![0; 4 * SAMPLE_RATE as usize]); // 2 s stereo
        assert_eq!(cues.len(), 2 * FRAME_RATE as usize);
        assert!(cues.iter().all(|c| *c == AudioCues::default()));
        assert!(audio_cues(&[]).is_empty());
        // A trailing partial window still gets a value.
        assert_eq!(audio_cues(&[0; 2 * 300]).len(), 2);
    }

    #[test]
    fn burst_pulses_then_decays_and_attack_is_faster_than_release() {
        // Silence, 0.3 s of a loud 2 kHz square, then silence again.
        let (on, off) = (window(35), window(56));
        let total = window(140);
        let pcm = stereo((0..total).map(|i| {
            if (on..off).contains(&i) {
                if (i / 4) % 2 == 0 { 0.6 } else { -0.6 }
            } else {
                0.0
            }
        }));
        let cues = audio_cues(&pcm);
        let f: Vec<[f32; 4]> = cues.iter().map(|c| c.intro).collect();
        assert_eq!(f.len(), 140);
        assert!(f[..35].iter().flatten().all(|&v| v == 0.0));
        // The onset and accent fire on the first loud window.
        assert!(f[35][1] > 0.9 && f[35][3] > 0.9, "{:?}", f[35]);
        // A steady tone does not retrigger: the onset only decays.
        for i in 36..140 {
            assert!(f[i][1] <= f[i - 1][1], "onset rose at {i}");
            assert!(f[i][3] <= f[i - 1][3], "accent rose at {i}");
        }
        assert!(f[55][1] < 0.25 * f[35][1]);
        // Energy attacks within a few windows and rises monotonically...
        assert!(f[37][0] > 0.8 * f[55][0]);
        for i in 36..56 {
            assert!(f[i][0] >= f[i - 1][0]);
        }
        // ...then releases monotonically, more slowly than it attacked.
        for i in 57..140 {
            assert!(f[i][0] < f[i - 1][0], "energy did not fall at {i}");
        }
        assert!(f[58][0] > 0.5 * f[55][0]);
        assert!(f[139][0] < 0.05);
        // A 2 kHz tone is not bass.
        assert!(f[50][2] < 0.2 * f[50][0], "{:?}", f[50]);
        assert!(f.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
        // The scroller's treble cue fires once on the burst; its level releases.
        let s: Vec<[f32; 3]> = cues.iter().map(|c| c.scroller).collect();
        assert!(s[..35].iter().flatten().all(|&v| v == 0.0));
        assert!(s[35..38].iter().any(|v| v[2] > 0.9), "{:?}", &s[35..38]);
        assert!(s[55][2] < 0.05, "A steady tone does not retrigger");
        assert!(s[139][0] < 0.005);
        assert!(s.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn bass_is_separated_and_features_are_causal() {
        let total = window(70);
        let tone =
            |hz: f64| {
                stereo((0..total).map(|i| {
                    0.5 * (std::f64::consts::TAU * hz * i as f64 / SAMPLE_RATE as f64).sin()
                }))
            };
        let bass = audio_cues(&tone(60.0));
        let treble = audio_cues(&tone(3000.0));
        assert!(bass[60].intro[2] > 0.8 && treble[60].intro[2] < 0.1);
        assert!((bass[60].intro[0] - treble[60].intro[0]).abs() < 0.05);
        // A tone starting from silence is a strong scroller bass onset only
        // if it is bass. (Any start leaks into the gentler treble filter.)
        let peak = |cues: &[AudioCues], band: usize| {
            cues[..5]
                .iter()
                .map(|c| c.scroller[band])
                .fold(0.0_f32, f32::max)
        };
        assert!(peak(&bass, 1) > 0.9 && peak(&treble, 1) < 0.4);
        assert!(peak(&treble, 2) > 0.9);
        // Truncating the PCM at a window boundary never changes earlier values,
        // so the shorter logo-only clip agrees with the longer transition clip.
        let pcm = tone(440.0);
        let prefix = audio_cues(&pcm[..window(41) * 2]);
        assert_eq!(prefix, audio_cues(&pcm)[..41]);
    }

    #[test]
    fn audio_lighting_is_sampled_by_time_and_ends_with_the_pcm() {
        let audio = vec![
            AudioCues {
                intro: [0.5, 1.0, 0.25, 0.0],
                scroller: [0.5, 0.0, 1.0],
            },
            AudioCues {
                intro: [1.0, 0.0, 0.75, 1.0],
                scroller: [1.0, 0.5, 0.0],
            },
        ];
        let clip = Clip {
            frames: Vec::new(),
            pcm: Vec::new(),
            logo_at: 0.0,
            logo_ready_at: 0.0,
            transition: None,
            scroller: None,
            flights: [[0.0; 4]; 10],
            audio,
            rings: None,
            field: None,
            dots: None,
        };
        let intro = |time: f64| clip.audio_at(time).intro;
        // Window 0 is known once it has played, at 1/70 s.
        assert_eq!(clip.audio_at(0.0), AudioCues::default());
        assert_eq!(intro(0.5 / 70.0), [0.25, 0.5, 0.125, 0.0]);
        assert_eq!(intro(1.0 / 70.0), [0.5, 1.0, 0.25, 0.0]);
        assert_eq!(intro(1.5 / 70.0), [0.75, 0.5, 0.5, 0.5]);
        assert_eq!(intro(2.5 / 70.0), [0.5, 0.0, 0.375, 0.5]);
        // The scroller's cues follow the same clock.
        assert_eq!(clip.audio_at(0.5 / 70.0).scroller, [0.25, 0.0, 0.5]);
        assert_eq!(clip.audio_at(1.5 / 70.0).scroller, [0.75, 0.25, 0.5]);
        for time in [3.0 / 70.0, 100.0, -1.0, f64::NAN] {
            assert_eq!(clip.audio_at(time), AudioCues::default());
        }
    }

    #[test]
    fn field_preserves_holes_and_signs() {
        let mut m = vec![false; 49];
        for y in 1..6 {
            for x in 1..6 {
                m[y * 7 + x] = true;
            }
        }
        m[24] = false;
        let d = signed_distance(&m, 7, 7);
        assert!(d[0] < 0.0 && d[24] < 0.0);
        assert!(d[8] > 0.0 && d[23] > 0.0);
        assert_eq!(d[8], 0.5);
        assert!(
            signed_distance(&[false; 9], 3, 3)
                .iter()
                .all(|&d| d == -32.0)
        );
        assert!(
            signed_distance(&[true; 9], 3, 3)
                .iter()
                .all(|&d| d > 0.0 && d.is_finite())
        );
    }
    #[test]
    fn source_palette_selects_logo_materials_without_background() {
        let mut frame = Frame {
            state: Presentation {
                phase: 4,
                frame: 0,
                tick: 1,
                door: 76,
            },
            indices: vec![180; WIDTH * HEIGHT],
            palette: vec![32; 768],
        };
        frame.indices[100 * WIDTH + 160] = 24;
        frame.indices[100 * WIDTH + 161] = 52;
        let f = frame.field();
        assert!(f[(100 * WIDTH + 160) * 4].to_f32() > 0.0);
        assert!(f[(100 * WIDTH + 160) * 4 + 1].to_f32() > 0.0);
        assert!(f[(100 * WIDTH + 161) * 4 + 1].to_f32() < 0.0);
        assert!(f[0].to_f32() < 0.0);
        frame.palette.fill(63);
        assert_eq!(
            f,
            frame.field(),
            "Seeking must not cache a different material during the white reveal"
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn transition_matches_engine_frames_audio_and_schedule() {
        let data = crate::test_support::data();
        let clip = Clip::load_transition(&data).unwrap();
        let intro = Clip::load(&data).unwrap();
        assert_eq!(clip.logo_ready_at, intro.logo_ready_at);
        assert_eq!(clip.pcm[..intro.pcm.len()], intro.pcm);
        // Causal audio lighting agrees wherever both clips have whole windows.
        let shared = intro.audio.len() - 1;
        assert_eq!(clip.audio[..shared], intro.audio[..shared]);
        assert!(
            intro
                .audio
                .iter()
                .flat_map(|c| c.intro.into_iter().chain(c.scroller))
                .all(|v| (0.0..=1.0).contains(&v))
        );
        let cut = clip.transition.as_ref().unwrap();
        assert_eq!(cut.end_tick - cut.start_tick, 169);
        assert_eq!(
            cut.start_tick,
            (clip.logo_ready_at * 70.0).round() as usize + 200
        );
        let mut sorted = cut.order;
        sorted.sort();
        assert_eq!(sorted, std::array::from_fn(|i| i as u16));
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for (i, frame) in clip.frames.iter().enumerate() {
            assert_eq!(engine.video.indices, frame.indices, "pixels at tick {i}");
            assert_eq!(engine.video.palette, frame.palette, "palette at tick {i}");
            if i + 1 < clip.frames.len() {
                pcm.extend(engine.step().unwrap());
            }
        }
        assert_eq!(pcm, clip.pcm);
        assert_eq!(engine.part(), 2);
        assert_eq!(clip.frames.last().unwrap().state.phase, 7);
        println!(
            "Wipe {:.3}–{:.3}s; full {}-frame reference and PCM matched",
            cut.start_tick as f64 / 70.0,
            cut.end_tick as f64 / 70.0,
            clip.frames.len()
        );
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn reference_timing_audio_and_flyby_projections() {
        let data = crate::test_support::data();
        let clip = Clip::load(&data).unwrap();
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for frame in clip.frames.iter().take(clip.frames.len() - 1) {
            assert_eq!(engine.part(), 1);
            assert_eq!(engine.video.indices, frame.indices);
            assert_eq!(engine.video.palette, frame.palette);
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(engine.part(), 2);
        assert_eq!(pcm, clip.pcm[..pcm.len()]);
        let sample_frames = [66, 78, 90, 100, 110, 120, 146, 160, 174, 186];
        let mut max_error = 0.0_f32;
        for (letter, source_frame) in sample_frames.into_iter().enumerate() {
            let frame = clip
                .frames
                .iter()
                .find(|f| f.state.phase == 1 && f.state.frame == source_frame + 1)
                .unwrap();
            let rows: Vec<_> = frame
                .indices
                .iter()
                .enumerate()
                .filter(|(_, v)| **v >= 229)
                .map(|(i, _)| i / WIDTH)
                .collect();
            let observed = (rows.iter().max().unwrap() - rows.iter().min().unwrap() + 1) as f32;
            let [crossing, speed, _, _] = clip.flights[letter];
            let predicted = 100.0 * 3.8 * 0.94 / ((crossing - frame.state.frame as f32) * speed);
            let error = (observed - predicted).abs();
            max_error = max_error.max(error);
            assert!(
                error < 3.0,
                "Letter {letter}: observed {observed}px, predicted {predicted}px"
            );
            assert!(crossing > frame.state.frame as f32 && speed > 0.0);
        }
        println!(
            "Original timeline and PCM matched; maximum sampled flyby height error: {max_error:.2} source pixels"
        );
    }
}
