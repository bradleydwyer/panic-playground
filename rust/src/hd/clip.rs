use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::intro::{Intro, Presentation},
    scenes::scroller::Scroller,
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

    /// Derive shape and material fields from caller-supplied indexed artwork.
    /// The contour is limited by the original raster; shading is newly generated
    /// at display resolution. Nothing here restores absent source artwork.
    pub fn field(&self) -> Vec<f16> {
        if self.state.phase >= 8 {
            // The original uses bits 0..6 for the large font/background and
            // bit 7 for the overlaid sine text. Contours stay locally derived;
            // extrusion, coatings and ribbon deformation are authored.
            let large: Vec<bool> = self.indices.iter().map(|i| i & 127 < 64).collect();
            let mut small = vec![false; WIDTH * HEIGHT];
            // The sine overlay writes one VGA plane every four pixels/rows.
            // Join those logical samples into continuous strokes for this
            // authored treatment; the comparison retains the original dots.
            for (at, &index) in self.indices.iter().enumerate() {
                if index & 128 == 0 {
                    continue;
                }
                for dy in -1..=2 {
                    for dx in -1..=2 {
                        let x = (at % WIDTH) as i32 + dx;
                        let y = (at / WIDTH) as i32 + dy;
                        if x >= 0 && x < WIDTH as i32 && y >= 0 && y < HEIGHT as i32 {
                            small[y as usize * WIDTH + x as usize] = true;
                        }
                    }
                }
            }
            return smooth_contour(&signed_distance(&large, WIDTH, HEIGHT), WIDTH, HEIGHT)
                .iter()
                .zip(smooth_contour(
                    &signed_distance(&small, WIDTH, HEIGHT),
                    WIDTH,
                    HEIGHT,
                ))
                .flat_map(|(&a, b)| [f16::from_f32(a), f16::from_f32(b), f16::ZERO, f16::ONE])
                .collect();
        }
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
}

/// Authored treatments sharing the same locally imported reference timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TransitionStyle {
    Burn = 0,
    Falling = 1,
    Laser = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ScrollerStyle {
    Ribbons = 0,
    Sculpted = 1,
    Glass = 2,
}

impl std::str::FromStr for ScrollerStyle {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "ribbons" => Ok(Self::Ribbons),
            "sculpted" => Ok(Self::Sculpted),
            "glass" => Ok(Self::Glass),
            _ => Err("Use --scroller ribbons, sculpted or glass".into()),
        }
    }
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

pub struct ScrollerPreview {
    pub start_tick: usize,
    pub end_tick: usize,
    pub style: ScrollerStyle,
    pub lighting: ScrollerLighting,
    /// Authored lighting controls from PCM: relative level, bass and treble onsets.
    /// Precomputed on the audio timeline so seeking and exports agree.
    pub audio: Vec<[f32; 3]>,
    /// Modern ambient palette with the rapid original cycling filtered out.
    /// Reference frames retain the original palette, including its alternating phase.
    pub backdrop: Vec<[[f32; 4]; 64]>,
}

/// Schedule and destination colours read from the local Rust reconstruction.
/// The GPU's curling surfaces, burn edges and falling motion are authored effects.
pub struct TileTransition {
    pub start_tick: usize,
    pub end_tick: usize,
    pub order: [u16; 160],
    pub palette: [[f32; 4]; 64],
    pub style: TransitionStyle,
}

impl Clip {
    pub fn load(data: &Data) -> Result<Self, String> {
        Self::load_mode(data, false, false)
    }

    pub fn load_transition(data: &Data) -> Result<Self, String> {
        Self::load_mode(data, true, false)
    }

    /// Alternate material animation over exactly the same reference and audio.
    pub fn load_tear(data: &Data) -> Result<Self, String> {
        let mut clip = Self::load_transition(data)?;
        clip.transition.as_mut().unwrap().style = TransitionStyle::Falling;
        Ok(clip)
    }

    /// Laser-cut plates use the same activation schedule and uninterrupted PCM.
    pub fn load_laser(data: &Data) -> Result<Self, String> {
        let mut clip = Self::load_transition(data)?;
        clip.transition.as_mut().unwrap().style = TransitionStyle::Laser;
        Ok(clip)
    }

    pub fn load_scroller(
        data: &Data,
        style: ScrollerStyle,
        lighting: ScrollerLighting,
    ) -> Result<Self, String> {
        let mut clip = Self::load_mode(data, true, true)?;
        let preview = clip.scroller.as_mut().unwrap();
        preview.style = style;
        preview.lighting = lighting;
        clip.transition.as_mut().unwrap().style = TransitionStyle::Laser;
        Ok(clip)
    }

    fn load_mode(data: &Data, extend: bool, play_scroller: bool) -> Result<Self, String> {
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
        let transition = if extend {
            // The last intro snapshot is also the first frame of part 2. Replace
            // it, rather than adding an extra frame or restarting the music.
            let logo_state = frames.pop().unwrap().state;
            let mut scroller = Scroller::new(data, &mut vga)?;
            let mut scroll_start = 0;
            let mut scroll_end = 0;
            let mut transition = TileTransition {
                start_tick: 0,
                end_tick: 0,
                order: scroller.transition_order(),
                palette: [[0.0; 4]; 64],
                style: TransitionStyle::Burn,
            };
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
                            let v = vga.palette[(64 + i) * 3 + channel] & 63;
                            let srgb = ((v << 2) | (v >> 4)) as f32 / 255.0;
                            *c = if srgb <= 0.04045 {
                                srgb / 12.92
                            } else {
                                ((srgb + 0.055) / 1.055).powf(2.4)
                            };
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
                scroller_preview = Some(ScrollerPreview {
                    start_tick: scroll_start,
                    end_tick: scroll_end,
                    style: ScrollerStyle::Glass,
                    lighting: ScrollerLighting::Studio,
                    audio: audio_lighting(&pcm, frames.len()),
                    backdrop: backdrop_palettes(&frames, scroll_start, transition.palette),
                });
            }
            Some(transition)
        } else {
            // Standalone logo preview continues the music through a short hold.
            pcm.extend(music.render(SAMPLE_RATE as usize * 4));
            None
        };
        Ok(Self {
            frames,
            pcm,
            logo_at: logo_at.ok_or("Intro never reached the logo")?,
            logo_ready_at,
            transition,
            scroller: scroller_preview,
            flights,
        })
    }

    pub fn duration(&self) -> f64 {
        self.pcm.len() as f64 / (SAMPLE_RATE * 2) as f64
    }
    pub fn frame_index(&self, time: f64) -> usize {
        ((time.max(0.0) * FRAME_RATE as f64) as usize).min(self.frames.len() - 1)
    }
    pub fn label(&self, time: f64) -> &'static str {
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

fn backdrop_palettes(
    frames: &[Frame],
    start: usize,
    initial: [[f32; 4]; 64],
) -> Vec<[[f32; 4]; 64]> {
    // The reconstructed palette changes phase on alternating ticks. Feeding
    // those values straight to an HD backdrop aliases against the display rate.
    // Two 120 ms low-pass stages retain its slow colour drift without shimmer.
    // Filtering is on the source timeline, never on wall time or GPU history.
    let alpha = 1.0 - (-1.0 / (FRAME_RATE as f32 * 0.12)).exp();
    let mut first = initial;
    let mut second = initial;
    frames
        .iter()
        .enumerate()
        .map(|(n, frame)| {
            if n >= start {
                for i in 0..64 {
                    for channel in 0..3 {
                        let v = frame.palette[(64 + i) * 3 + channel] & 63;
                        let srgb = ((v << 2) | (v >> 4)) as f32 / 255.0;
                        let linear = if srgb <= 0.04045 {
                            srgb / 12.92
                        } else {
                            ((srgb + 0.055) / 1.055).powf(2.4)
                        };
                        first[i][channel] += (linear - first[i][channel]) * alpha;
                        second[i][channel] += (first[i][channel] - second[i][channel]) * alpha;
                    }
                }
            }
            second
        })
        .collect()
}

fn audio_lighting(pcm: &[i16], frames: usize) -> Vec<[f32; 3]> {
    // Separate stereo filters avoid cancelling instruments panned out of phase.
    // These broad bands are lighting cues, not recovered tracker events.
    let bass_alpha = 1.0 - (-std::f32::consts::TAU * 180.0 / SAMPLE_RATE as f32).exp();
    let treble_alpha = 1.0 - (-std::f32::consts::TAU * 1800.0 / SAMPLE_RATE as f32).exp();
    let mut bass_filter = [0.0_f32; 2];
    let mut treble_filter = [0.0_f32; 2];
    let mut average = [0.0_f32; 3];
    let mut previous = [0.0_f32; 3];
    let mut output = [0.0_f32; 3];
    (0..frames)
        .map(|n| {
            let a = n * SAMPLE_RATE as usize / FRAME_RATE as usize * 2;
            let b = ((n + 1) * SAMPLE_RATE as usize / FRAME_RATE as usize * 2).min(pcm.len());
            let samples = &pcm[a.min(b)..b];
            let mut power = [0.0_f32; 3];
            for pair in samples.chunks_exact(2) {
                for channel in 0..2 {
                    let s = pair[channel] as f32 / 32768.0;
                    bass_filter[channel] += bass_alpha * (s - bass_filter[channel]);
                    treble_filter[channel] += treble_alpha * (s - treble_filter[channel]);
                    power[0] += s * s;
                    power[1] += bass_filter[channel].powi(2);
                    power[2] += (s - treble_filter[channel]).powi(2);
                }
            }
            for band in 0..3 {
                let rms = (power[band] / samples.len().max(1) as f32).sqrt();
                let smooth = previous[band] + (rms - previous[band]) * 0.6;
                // Relative energy keeps loud passages from pinning the controls
                // at one, as the old fixed RMS gain did. A floor rejects noise.
                average[band] += (smooth - average[band]) * 0.025;
                if band == 0 {
                    let target = ((smooth / (average[band] + 0.035) - 0.65) * 0.65).clamp(0.0, 1.0);
                    output[band] +=
                        (target - output[band]) * if target > output[band] { 0.45 } else { 0.12 };
                } else {
                    let rise = (smooth - previous[band]) / (average[band] + 0.015);
                    let target = ((rise - 0.045) * 4.0).clamp(0.0, 1.0);
                    let decay = if band == 1 { 0.88 } else { 0.80 };
                    output[band] = target.max(output[band] * decay);
                }
                previous[band] = smooth;
            }
            output
        })
        .collect()
}

/// Low-pass the locally derived contour to remove one-pixel stair steps from
/// the new bevels. This deliberately rounds the raster; it is not a recovered
/// vector outline. Kept separate from the faithful comparison and intro.
fn smooth_contour(input: &[f32], width: usize, height: usize) -> Vec<f32> {
    let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
    let mut a = input.to_vec();
    let mut b = vec![0.0; input.len()];
    for _ in 0..2 {
        for y in 0..height {
            for x in 0..width {
                b[y * width + x] = weights
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        a[y * width
                            + (x as isize + i as isize - 2).clamp(0, width as isize - 1) as usize]
                            * w
                            / 16.0
                    })
                    .sum();
            }
        }
        for y in 0..height {
            for x in 0..width {
                a[y * width + x] = weights
                    .iter()
                    .enumerate()
                    .map(|(i, w)| {
                        b[(y as isize + i as isize - 2).clamp(0, height as isize - 1) as usize
                            * width
                            + x]
                            * w
                            / 16.0
                    })
                    .sum();
            }
        }
    }
    a
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
    fn backdrop_filters_alternating_palette_but_tracks_a_sustained_change() {
        let frames: Vec<_> = (0..210)
            .map(|n| Frame {
                state: Presentation {
                    phase: 8,
                    frame: n,
                    tick: 0,
                    door: 76,
                },
                indices: vec![],
                palette: vec![if n < 140 && n % 2 == 0 { 0 } else { 63 }; 768],
            })
            .collect();
        let filtered = backdrop_palettes(&frames, 0, [[0.0, 0.0, 0.0, 1.0]; 64]);
        assert!((filtered[138][0][0] - filtered[139][0][0]).abs() < 0.005);
        assert!((filtered[139][0][0] - 0.5).abs() < 0.01);
        assert!(
            filtered[209][0][0] > 0.995,
            "Sustained colours must come through"
        );
        assert_eq!(
            frames[138].palette[192], 0,
            "Reference palette remains unfiltered"
        );
        assert_eq!(filtered[209][0][3], 1.0);
    }

    #[test]
    fn audio_lighting_keeps_accents_in_a_loud_mix_and_releases_to_silence() {
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
                // Anti-phase stereo must retain the same energy as in-phase.
                [sample, -sample]
            })
            .collect();
        let signal = audio_lighting(&pcm, 280);
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
        assert_eq!(audio_lighting(&[], 3), vec![[0.0; 3]; 3]);
        assert_eq!(audio_lighting(&vec![0; 32000], 70), vec![[0.0; 3]; 70]);
    }

    #[test]
    fn scroller_fields_separate_font_overlay_and_background() {
        let mut frame = Frame {
            state: Presentation {
                phase: 8,
                frame: 1700,
                tick: 0,
                door: 76,
            },
            indices: vec![80; WIDTH * HEIGHT],
            palette: vec![0; 768],
        };
        for y in 40..70 {
            for x in 50..80 {
                frame.indices[y * WIDTH + x] = 20;
            }
        }
        for y in (100..124).step_by(4) {
            for x in (120..144).step_by(4) {
                frame.indices[y * WIDTH + x] |= 128;
            }
        }
        let f = frame.field();
        let sample = |x: usize, y: usize, channel: usize| f[(y * WIDTH + x) * 4 + channel].to_f32();
        assert!(sample(65, 55, 0) > 0.0 && sample(65, 55, 1) < 0.0);
        assert!(sample(130, 110, 0) < 0.0 && sample(130, 110, 1) > 0.0);
        assert!(sample(200, 150, 0) < 0.0 && sample(200, 150, 1) < 0.0);
        assert_eq!(frame.shape_key(), (8, 1700));
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn scroller_matches_complete_engine_reference_and_pcm() {
        let data = crate::test_support::data();
        let clip =
            Clip::load_scroller(&data, ScrollerStyle::Ribbons, ScrollerLighting::Musical).unwrap();
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
        assert_eq!(scroll.audio.len(), clip.frames.len());
        assert_eq!(scroll.backdrop.len(), clip.frames.len());
        assert!(
            scroll
                .audio
                .iter()
                .flatten()
                .all(|e| e.is_finite() && (0.0..=1.0).contains(e))
        );
        assert_eq!(clip.frames[scroll.start_tick].state.phase, 8);
        assert_eq!(clip.frames[scroll.end_tick].state.phase, 9);
        println!(
            "Scroller {:.3}–{:.3}s; {} frames and full PCM match",
            scroll.start_tick as f64 / 70.0,
            clip.duration(),
            clip.frames.len()
        );
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
        let falling = Clip::load_tear(&data).unwrap();
        let laser = Clip::load_laser(&data).unwrap();
        let intro = Clip::load(&data).unwrap();
        let alternatives = [("falling", &falling), ("laser", &laser)];
        let burn_schedule = clip.transition.as_ref().unwrap();
        for (_, alternative) in alternatives {
            assert_eq!(alternative.pcm, clip.pcm);
            assert_eq!(alternative.frames.len(), clip.frames.len());
            let schedule = alternative.transition.as_ref().unwrap();
            assert_eq!(schedule.start_tick, burn_schedule.start_tick);
            assert_eq!(schedule.end_tick, burn_schedule.end_tick);
            assert_eq!(schedule.order, burn_schedule.order);
        }
        assert_eq!(clip.logo_ready_at, intro.logo_ready_at);
        assert_eq!(clip.pcm[..intro.pcm.len()], intro.pcm);
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
            for (label, alternative) in alternatives {
                assert_eq!(
                    alternative.frames[i].indices, frame.indices,
                    "{label} reference at tick {i}"
                );
                assert_eq!(
                    alternative.frames[i].palette, frame.palette,
                    "{label} palette at tick {i}"
                );
            }
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
