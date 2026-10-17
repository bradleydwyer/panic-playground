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
    /// square wipe and backdrop hold, not additional phases inside Intro.
    pub state: Presentation,
    pub indices: Vec<u8>,
    pub palette: Vec<u8>,
}

impl Frame {
    pub fn shape_key(&self) -> (u8, usize) {
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
    /// Pinhole motion fitted from the locally decoded reference silhouettes:
    /// camera crossing frame, depth per frame, world x and world y.
    pub flights: [[f32; 4]; 10],
    /// Lighting controls measured from `pcm`, one per 70 Hz window; see
    /// [`audio_features`]. They only steer authored lighting.
    pub audio: Vec<[f32; 4]>,
}

/// Authored treatments sharing the same locally imported reference timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TransitionStyle {
    Burn = 0,
    Falling = 1,
    Laser = 2,
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
        Self::load_mode(data, false)
    }

    pub fn load_transition(data: &Data) -> Result<Self, String> {
        Self::load_mode(data, true)
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

    fn load_mode(data: &Data, extend: bool) -> Result<Self, String> {
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
        let transition = if extend {
            // The last intro snapshot is also the first frame of part 2. Replace
            // it, rather than adding an extra frame or restarting the music.
            let logo_state = frames.pop().unwrap().state;
            let mut scroller = Scroller::new(data, &mut vga)?;
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
                        } else {
                            7
                        },
                        tick,
                        ..logo_state
                    },
                    indices: vga.indices.clone(),
                    palette: vga.palette.clone(),
                });
                // Show the revealed backdrop for a second, before the text
                // begins. The next scene's scrolling section is not remade yet.
                if phase == 3 && tick == FRAME_RATE as usize {
                    break;
                }
                if n as u64 > FRAME_RATE * 60 {
                    return Err("Tile transition exceeded its import limit".into());
                }
                scroller.step(&mut vga, music.position());
                let n = n as u64;
                let samples =
                    ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
                pcm.extend(music.render(samples));
            }
            Some(transition)
        } else {
            // Standalone logo preview continues the music through a short hold.
            pcm.extend(music.render(SAMPLE_RATE as usize * 4));
            None
        };
        Ok(Self {
            frames,
            audio: audio_features(&pcm),
            pcm,
            logo_at: logo_at.ok_or("Intro never reached the logo")?,
            logo_ready_at,
            transition,
            flights,
        })
    }

    pub fn duration(&self) -> f64 {
        self.pcm.len() as f64 / (SAMPLE_RATE * 2) as f64
    }
    /// Audio lighting controls at a clip time. Window `i` is only known once
    /// it has played, at `(i + 1) / 70` s, so the lights never anticipate the
    /// music. Values interpolate between windows and fall to zero outside the
    /// PCM. Seeking reads the same precomputed table as playback.
    pub fn audio_at(&self, time: f64) -> [f32; 4] {
        let position = time * FRAME_RATE as f64 - 1.0;
        if !position.is_finite() || position <= -1.0 {
            return [0.0; 4];
        }
        let base = position.floor();
        let fraction = (position - base) as f32;
        let at = |i: f64| {
            if i < 0.0 {
                [0.0; 4]
            } else {
                self.audio.get(i as usize).copied().unwrap_or([0.0; 4])
            }
        };
        let (a, b) = (at(base), at(base + 1.0));
        std::array::from_fn(|k| a[k] + (b[k] - a[k]) * fraction)
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
            _ => "Panic logo",
        }
    }
}

/// Measure authored lighting controls from the clip's interleaved stereo PCM,
/// one value per 70 Hz window (the same sample split `Clip::load_mode` uses).
/// This reads only the rendered audio, so the music itself is untouched. The
/// filters are causal: a prefix of the PCM always gives a prefix of the result.
///
/// The components are new display controls in 0..=1, not anything recovered
/// from the original program:
/// - `[0]` energy: broadband RMS with fast attack and a 0.25 s release.
/// - `[1]` onset: a rise in high-band (above about 150 Hz) energy over its
///   recent level, gated by loudness, held and decayed with a 0.12 s constant.
/// - `[2]` low: RMS below about 150 Hz, fast attack and a 0.18 s release.
/// - `[3]` accent: loudness well above the last two seconds, such as a strong
///   downbeat after a quieter passage, decayed with a 0.25 s constant.
///
/// The time constants and scales were tuned by eye and ear to the intro.
pub fn audio_features(pcm: &[i16]) -> Vec<[f32; 4]> {
    let rate = SAMPLE_RATE as usize;
    let frames = pcm.len() / 2;
    let windows = (frames * FRAME_RATE as usize).div_ceil(rate);
    let release = |seconds: f64| 1.0 - (-1.0 / (seconds * FRAME_RATE as f64)).exp();
    let decay = |seconds: f64| (-1.0 / (seconds * FRAME_RATE as f64)).exp();
    let crossover = 1.0 - (-std::f64::consts::TAU * 150.0 / rate as f64).exp();
    let (mut lp1, mut lp2) = (0.0_f64, 0.0_f64);
    let (mut energy, mut low, mut onset, mut accent) = (0.0_f64, 0.0, 0.0, 0.0);
    let (mut recent_high, mut long_power) = (0.0_f64, 0.0);
    let mut result = Vec::with_capacity(windows);
    for i in 0..windows {
        let start = i * rate / FRAME_RATE as usize;
        let end = ((i + 1) * rate / FRAME_RATE as usize).min(frames);
        let (mut power, mut low_power, mut high_power) = (0.0, 0.0, 0.0);
        for frame in pcm[start * 2..end * 2].chunks_exact(2) {
            let mono = (frame[0] as f64 + frame[1] as f64) / 65536.0;
            lp1 += crossover * (mono - lp1);
            lp2 += crossover * (lp1 - lp2);
            let high = mono - lp2;
            power += mono * mono;
            low_power += lp2 * lp2;
            high_power += high * high;
        }
        let count = (end - start).max(1) as f64;
        let (power, low_power, high_power) = (power / count, low_power / count, high_power / count);
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
        result.push([
            (1.0 - (-energy / 0.25).exp()) as f32,
            onset as f32,
            (1.0 - (-low / 0.12).exp()) as f32,
            accent as f32,
        ]);
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
        let features = audio_features(&vec![0; 4 * SAMPLE_RATE as usize]); // 2 s stereo
        assert_eq!(features.len(), 2 * FRAME_RATE as usize);
        assert!(features.iter().flatten().all(|&v| v == 0.0));
        assert!(audio_features(&[]).is_empty());
        // A trailing partial window still gets a value.
        assert_eq!(audio_features(&[0; 2 * 300]).len(), 2);
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
        let f = audio_features(&pcm);
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
        let bass = audio_features(&tone(60.0));
        let treble = audio_features(&tone(3000.0));
        assert!(bass[60][2] > 0.8 && treble[60][2] < 0.1);
        assert!((bass[60][0] - treble[60][0]).abs() < 0.05);
        // Truncating the PCM at a window boundary never changes earlier values,
        // so the shorter logo-only clip agrees with the longer transition clip.
        let pcm = tone(440.0);
        let prefix = audio_features(&pcm[..window(41) * 2]);
        assert_eq!(prefix, audio_features(&pcm)[..41]);
    }

    #[test]
    fn audio_lighting_is_sampled_by_time_and_ends_with_the_pcm() {
        let audio = vec![[0.5, 1.0, 0.25, 0.0], [1.0, 0.0, 0.75, 1.0]];
        let clip = Clip {
            frames: Vec::new(),
            pcm: Vec::new(),
            logo_at: 0.0,
            logo_ready_at: 0.0,
            transition: None,
            flights: [[0.0; 4]; 10],
            audio,
        };
        // Window 0 is known once it has played, at 1/70 s.
        assert_eq!(clip.audio_at(0.0), [0.0; 4]);
        assert_eq!(clip.audio_at(0.5 / 70.0), [0.25, 0.5, 0.125, 0.0]);
        assert_eq!(clip.audio_at(1.0 / 70.0), [0.5, 1.0, 0.25, 0.0]);
        assert_eq!(clip.audio_at(1.5 / 70.0), [0.75, 0.5, 0.5, 0.5]);
        assert_eq!(clip.audio_at(2.5 / 70.0), [0.5, 0.0, 0.375, 0.5]);
        assert_eq!(clip.audio_at(3.0 / 70.0), [0.0; 4]);
        assert_eq!(clip.audio_at(100.0), [0.0; 4]);
        assert_eq!(clip.audio_at(-1.0), [0.0; 4]);
        assert_eq!(clip.audio_at(f64::NAN), [0.0; 4]);
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
        // Causal audio lighting agrees wherever both clips have whole windows.
        let shared = intro.audio.len() - 1;
        assert_eq!(clip.audio[..shared], intro.audio[..shared]);
        assert_eq!(laser.audio, clip.audio);
        assert!(
            intro
                .audio
                .iter()
                .flatten()
                .all(|v| (0.0..=1.0).contains(v))
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
