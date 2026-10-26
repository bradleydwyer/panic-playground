//! Part 3's water, for the modern view: part 4's water (water.wgsl and
//! [`WaterTuning`]) beneath part 3's objects, so the box lands on the water
//! part 4's slab stands in. All of it is authored but the box's pose. What
//! comes from where:
//! - Read from the port (through [`DotsPreview`]): the landed box, the
//!   last dot frame's rotation and translation and the box object, so the
//!   water at rest is the plane the box's base lies in; and the camera
//!   spline bringing the box down to it at the end.
//! - Authored: the water lying level beneath the objects, LEVEL_HEIGHT below
//!   the camera, until the box comes down, then turning, as the box comes
//!   down, until it lies in the box's base. The original's camera is still
//!   and only its objects move; read that way, the camera rises and tilts
//!   down onto the box as it lands, which only the water shows. And the
//!   drops' times and places, and everything dots_light.wgsl draws.
//!
//! Why it turns: the objects fly at the camera's height, along its axis,
//! from about 500 to 2,300 units away, and the box lands tilted 20 degrees
//! toward the camera. Its base's plane rises along the axis and passes
//! through the objects from about 1,060 units on, so it can't be the water
//! the whole time; a level plane can't be the one the box lands on.
use super::dots::{DotFrame, DotsEvents, DotsPreview};
use super::plasma::{DROP_LIFE, MAX_DROPS, WaterTuning, drop_hash};

/// The camera's height above the water while it lies level (authored, in
/// the dots' camera units): below every point of every object but a few of
/// the lattices' when they pass nearest, and below the shade bobs' tips,
/// as dots_light.wgsl places them (SHADE_DEPTH), at the lowest of their
/// path.
pub const LEVEL_HEIGHT: f64 = 260.0;
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

/// Where part 3's water lies, in the dots' camera space (x right, y down,
/// z away, a point shown at column 160 + 256 x / z, row 100 + 256 y / z).
/// A plane tilted by `a` has the up normal (0, -cos a, -sin a).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    /// At rest: the landed box's base plane's tilt (radians), and the
    /// camera's height above it.
    pub tilt: f64,
    pub height: f64,
    /// The dot frame from which the box comes down onto it: from there the
    /// water turns from level to its rest as the box's centre comes down,
    /// by `descent` (camera y) in all.
    pub descent_from: f64,
    pub descent: f64,
}

/// The rotation's rows at angles `a` (1024ths of a turn), as
/// dots_common.wgsl's `rotation` has them: camera = R object + translation.
fn rotation(a: [f64; 3]) -> [[f64; 3]; 3] {
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

impl Water {
    /// The water under the box the dots end on (`object`, the last
    /// stage's, at the last of `dot_frames`): its base's plane, the face of
    /// the box whose normal is nearest the camera's vertical. The box must
    /// turn only about the camera's x axis, as the port's does to within
    /// 1e-14.
    pub fn new(object: &[[i32; 3]], dot_frames: &[DotFrame]) -> Result<Self, String> {
        let last = dot_frames.last().ok_or("Part 3 has no dot frames")?;
        let r = rotation(std::array::from_fn(|k| last.angles[k] as f64));
        let t: [f64; 3] = std::array::from_fn(|k| last.translation[k] as f64);
        let lo: [f64; 3] =
            std::array::from_fn(|k| object.iter().map(|p| p[k]).min().unwrap_or(0) as f64);
        let hi: [f64; 3] =
            std::array::from_fn(|k| object.iter().map(|p| p[k]).max().unwrap_or(0) as f64);
        let column = |k: usize| [r[0][k], r[1][k], r[2][k]];
        let k = (0..3)
            .max_by(|&a, &b| column(a)[1].abs().total_cmp(&column(b)[1].abs()))
            .unwrap();
        let sign = column(k)[1].signum();
        let down = column(k).map(|v| v * sign);
        if down[0].abs() > 1e-6 {
            return Err(format!("Part 3's box turns about more than x ({down:?})"));
        }
        // The base's centre, in object space and then the camera's.
        let base: [f64; 3] = std::array::from_fn(|i| {
            (lo[i] + hi[i]) / 2.0
                + if i == k {
                    sign * (hi[i] - lo[i]) / 2.0
                } else {
                    0.0
                }
        });
        let base: [f64; 3] =
            std::array::from_fn(|i| (0..3).map(|j| r[i][j] * base[j]).sum::<f64>() + t[i]);
        let descent = t[1];
        let descent_from = dot_frames
            .iter()
            .rposition(|f| (f.translation[1] as f64).abs() <= 0.005 * descent.abs())
            .unwrap_or(0) as f64;
        Ok(Self {
            tilt: down[2].atan2(down[1]),
            height: base[1] * down[1] + base[2] * down[2],
            descent_from,
            descent,
        })
    }

    /// How far the water has turned from level to its rest at continuous
    /// dot frame `tau`, 0 to 1, as the box's centre comes down (eased, as
    /// dots_light.wgsl's `water_plane` has it).
    pub fn turned(&self, tau: f64, centre_y: f64) -> f64 {
        if tau < self.descent_from || self.descent <= 0.0 {
            return 0.0;
        }
        let s = (centre_y / self.descent).clamp(0.0, 1.0);
        s * s * (3.0 - 2.0 * s)
    }

    /// The tilt and the camera's height above the water, `turned` of the
    /// way from level to rest.
    pub fn plane(&self, turned: f64) -> (f64, f64) {
        (
            self.tilt * turned,
            LEVEL_HEIGHT + (self.height - LEVEL_HEIGHT) * turned,
        )
    }

    /// The height above the water of camera point `c`, `turned` of the way.
    pub fn height_of(&self, turned: f64, c: [f64; 3]) -> f64 {
        let (a, h) = self.plane(turned);
        h - a.cos() * c[1] - a.sin() * c[2]
    }

    /// The controls: the tilt at rest, the camera's height above the water
    /// at rest and while level, and the dot frame the box comes down from.
    pub fn controls(&self) -> [f32; 4] {
        [self.tilt, self.height, LEVEL_HEIGHT, self.descent_from].map(|v| v as f32)
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
        tuning.drops(seconds, span, DROP_SEED, |k| {
            let h = |i: u64| drop_hash(k + (DROP_SEED << 40), 0, i);
            let side = if h(0) < 0.5 { -1.0 } else { 1.0 };
            let across = side * (DROP_ACROSS.0 + (DROP_ACROSS.1 - DROP_ACROSS.0) * h(1));
            let row = DROP_ROWS.0 + (DROP_ROWS.1 - DROP_ROWS.0) * h(2);
            // The ray through that point, in rows, meets the level water at
            // y = LEVEL_HEIGHT.
            let reach = LEVEL_HEIGHT / (row - 100.0);
            Some([across * reach, 256.0 * reach])
        })
    }
}

impl DotsPreview {
    /// How far the water has turned from level to its rest at continuous
    /// dot frame `tau`.
    pub fn water_turned(&self, tau: f64) -> f64 {
        let centre = self.centre_at(tau.max(0.0));
        self.water.turned(tau, centre[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    /// An invented box, 490 by 200 by 100 (its corners), and dot frames
    /// level until frame 10, then coming down to the port's resting pose,
    /// turned 200 of 1024 about x.
    fn invented() -> (Vec<[i32; 3]>, Vec<DotFrame>) {
        let corners = (0..8)
            .map(|c| {
                [
                    if c & 1 == 0 { -245 } else { 245 },
                    if c & 2 == 0 { -100 } else { 100 },
                    if c & 4 == 0 { -50 } else { 50 },
                ]
            })
            .collect();
        let frames = (0..=20)
            .map(|k| {
                let down = (k as f32 - 10.0).max(0.0) * 13.0;
                DotFrame {
                    angles: [200.0, 0.0, 0.0, 7.0],
                    translation: [0.0, down, 550.0, 1.0],
                }
            })
            .collect();
        (corners, frames)
    }

    fn corners_at_rest(object: &[[i32; 3]], last: &DotFrame) -> Vec<[f64; 3]> {
        let r = rotation(std::array::from_fn(|k| last.angles[k] as f64));
        object
            .iter()
            .map(|p| {
                std::array::from_fn(|i| {
                    (0..3).map(|j| r[i][j] * p[j] as f64).sum::<f64>() + last.translation[i] as f64
                })
            })
            .collect()
    }

    /// At rest the box stands on the water: four corners on it, four 100
    /// above; the plane tilts as the box does (200 of 1024 about x leaves
    /// its base 19.7 degrees from level).
    #[test]
    fn at_rest_the_box_stands_on_the_water() {
        let (object, frames) = invented();
        let water = Water::new(&object, &frames).unwrap();
        assert!((water.tilt - (56.0_f64 / 1024.0 * std::f64::consts::TAU)).abs() < 1e-9);
        let mut heights: Vec<f64> = corners_at_rest(&object, frames.last().unwrap())
            .iter()
            .map(|&c| water.height_of(1.0, c))
            .collect();
        heights.sort_by(f64::total_cmp);
        for (k, h) in heights.iter().enumerate() {
            let expected = if k < 4 { 0.0 } else { 100.0 };
            assert!((h - expected).abs() < 1e-3, "{heights:?}");
        }
        assert_eq!(water.descent_from, 10.0);
        assert!((water.descent - 130.0).abs() < 1e-6);
    }

    /// The water lies level, LEVEL_HEIGHT below the camera, until the box
    /// comes down, and turns with it to its rest, never back.
    #[test]
    fn the_water_turns_as_the_box_comes_down() {
        let (object, frames) = invented();
        let water = Water::new(&object, &frames).unwrap();
        assert_eq!(water.plane(0.0), (0.0, LEVEL_HEIGHT));
        assert_eq!(water.plane(1.0), (water.tilt, water.height));
        let turned = |tau: f64| {
            let k = tau.floor() as usize;
            let y = frames[k].translation[1] as f64;
            water.turned(tau, y)
        };
        assert_eq!(turned(0.0), 0.0);
        assert_eq!(turned(9.5), 0.0);
        assert_eq!(turned(20.0), 1.0);
        let mut last = 0.0;
        for k in 10..=20 {
            let t = turned(k as f64);
            assert!(t >= last, "{t} after {last} at {k}");
            last = t;
        }
        // Level, a camera point's height is LEVEL_HEIGHT less its y.
        assert!((water.height_of(0.0, [40.0, 60.0, 900.0]) - (LEVEL_HEIGHT - 60.0)).abs() < 1e-9);
    }

    /// The drops fall from DROP_FIRST after the dots begin, none so late
    /// that it outlasts the rotation's stop by more than its life, each at
    /// a place on the level water the screen shows within DROP_ROWS.
    #[test]
    fn drops_fall_in_view_while_the_dots_turn() {
        let (object, frames) = invented();
        let water = Water::new(&object, &frames).unwrap();
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
                assert!(fell + DROP_LIFE <= events.rotation_stop + DROP_LIFE + 1e-6);
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

    /// The constants water.wgsl repeats, and the shade tips' depth in
    /// dots_light.wgsl, at which the lowest of their path (row 90 + 32767 /
    /// 660 + 32767 / 700 + 8, as dots_common.wgsl's shade_tip has it) stays
    /// above the level water.
    #[test]
    fn the_shaders_repeat_the_water_constants() {
        let water = include_str!("water.wgsl");
        assert!(water.contains(&format!("const DROP_LIFE={DROP_LIFE:.1};")));
        assert!(water.contains(&format!("const MAX_DROPS={MAX_DROPS}u;")));
        let light = include_str!("dots_light.wgsl");
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
        assert!((lowest - 100.0) * depth / 256.0 < LEVEL_HEIGHT);
    }

    /// With the port's data: the box at rest stands on the water, and over
    /// the whole part, the water turning as it does, few of the points ever
    /// dip under it (about 1%: some of the lattices' and the box's corners
    /// as they pass nearest).
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn the_dots_fly_over_the_water() {
        let data = crate::test_support::data();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let dots = clip.dots.as_ref().unwrap();
        let water = dots.water;
        let order = dots.tables.order;
        let (mut dipped, mut total) = (0, 0);
        let mut lowest_at_rest = f64::MAX;
        let last = dots.dot_frames.len() - 1;
        for (k, f) in dots.dot_frames.iter().enumerate() {
            let r = rotation(std::array::from_fn(|i| f.angles[i] as f64));
            let stage = (f.angles[3] as usize).min(7);
            let a = &dots.tables.objects[order[stage]];
            let b = &dots.tables.objects[order[(stage + 1).min(7)]];
            let turned = dots.water_turned(k as f64);
            for (p, q) in a.iter().zip(b) {
                let o: [f64; 3] = std::array::from_fn(|j| {
                    let x = p[j] as f64;
                    if stage == 7 {
                        x
                    } else {
                        x + (q[j] as f64 - x) * f.translation[3] as f64
                    }
                });
                let c: [f64; 3] = std::array::from_fn(|i| {
                    (0..3).map(|j| r[i][j] * o[j]).sum::<f64>() + f.translation[i] as f64
                });
                let h = water.height_of(turned, c);
                // The first dot frames fly the sphere in from far off.
                if k >= 150 {
                    total += 1;
                    dipped += usize::from(h < 0.0);
                }
                if k == last {
                    lowest_at_rest = lowest_at_rest.min(h);
                }
            }
        }
        assert!(lowest_at_rest.abs() < 0.5, "{lowest_at_rest}");
        let share = dipped as f64 / total as f64;
        assert!(share < 0.02, "{share}");
    }
}
