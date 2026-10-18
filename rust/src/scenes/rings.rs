//! Part6: bitplane interference rings and the PSTART transition.
//! Original 0x1161c control, 0x1130e mask decoder, 0x11372 window copier.
use crate::{
    data::{Data, word},
    vga::Vga,
};

// Clone lets the HD preview keep checkpoints and replay reference frames on
// demand; nothing in the scene depends on it.
#[derive(Clone)]
pub struct Rings {
    source: Vec<u8>,
    sine: Vec<i32>,
    image: Vec<u8>,
    image_palette: Vec<u8>,
    frame: usize,
    intensity: i32,
    selector: bool,
    page: u16,
    phase1: u16,
    phase2: u16,
    previous_row: u8,
    rng: u32,
    started: bool,
    transition: Option<usize>,
}

/// What part 6's current scanout shows, for the HD preview, read from the
/// scene's counters rather than the pixels. See [`Rings::hd_state`].
#[cfg(feature = "hires")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HdRings {
    /// 0 while waiting for the music to reach 06:00 (nothing drawn: black),
    /// 1 while the rings and symbols run, 2 during the PSTART fade.
    pub phase: u8,
    /// Ring draw steps so far (`frame`). Step `k` (from 0) drew the rings at
    /// [`Rings::hd_offsets`]`(k)`.
    pub frame: usize,
    /// The ring intensity the displayed palette used, -100..=64. The greys
    /// are `[0, 48, 32, 0] * intensity / 64` and show only above 0.
    pub intensity: i32,
    /// The 8 KB page (0..8) the latest step drew the rings into.
    pub drawn_page: usize,
    /// The page the scanout shows: the other page of the pair, drawn one
    /// step earlier, unless the page has just advanced. 0 outside phase 1.
    pub shown_page: usize,
    /// The symbol mask lit on this scanout, `2 * shown_page + plane - 2`: on
    /// even frames only, and never before 06:00 or during the picture.
    pub lit: Option<u8>,
    /// PSTART palette level on screen, in 64ths (0..=63), during its fade.
    pub picture: Option<u8>,
}

impl Rings {
    /// Read-only state of the current scanout for the HD preview. As
    /// `step` reads it: the palette, page and lit plane come from the step
    /// just made, the drawn page from before its page flip.
    #[cfg(feature = "hires")]
    pub fn hd_state(&self) -> HdRings {
        let picture = self.transition.map(|level| (level - 1) as u8);
        let phase = if picture.is_some() {
            2
        } else if self.started {
            1
        } else {
            0
        };
        // `step` toggles the page after drawing and before the scanout. `new`
        // shows page 0; the picture replaces the planar scanout.
        let shown_page = if phase == 1 {
            self.page as usize / 0x2000
        } else {
            0
        };
        let lit = (phase == 1 && self.frame > 0 && (self.frame - 1) & 1 == 0)
            .then(|| (2 * shown_page + usize::from(self.selector)) as u8);
        HdRings {
            phase,
            frame: self.frame,
            intensity: self.intensity,
            drawn_page: if phase == 1 { shown_page ^ 1 } else { 0 },
            shown_page,
            lit,
            picture,
        }
    }

    /// The window offsets `(x, y)` into M.DAT of plane 0 and plane 1 for draw
    /// step `step` (from 0; any integer, as the phases only advance by fixed
    /// amounts): `step` copies M.DAT rows from `y` and columns from `x` into
    /// the 256 × 184 window at screen (32, 8). Read from `step`: plane 1's
    /// phase gains 35 and plane 0's loses 28 per step before drawing, and
    /// plane 0 takes 9/10 of M.SIN's offsets.
    #[cfg(feature = "hires")]
    pub fn hd_offsets(&self, step: i64) -> [[i32; 2]; 2] {
        [(-28, 9, 10), (35, 1, 1)].map(|(speed, numerator, denominator)| {
            let phase = (speed * (step + 1)).rem_euclid(4096) as usize;
            let i = phase / 16;
            [
                self.sine[i & 255] * numerator / denominator + 50,
                self.sine[(i + 64) & 255] * numerator / denominator + 50,
            ]
        })
    }

    /// Bank 0 of M.DAT, the ring source, as 152 rows of 424 bits (most
    /// significant first, as `Vga::ega` reads them). The other banks hold it
    /// shifted left by 1 to 7 pixels; `step` mirrors rows 152 and beyond.
    #[cfg(feature = "hires")]
    pub fn hd_source(&self) -> Vec<bool> {
        (0..152 * 424)
            .map(|at| {
                let (row, column) = (at / 424, at % 424);
                (self.source[row * 53 + column / 8] >> (7 - column % 8)) & 1 != 0
            })
            .collect()
    }

    /// The PSTART picture (320 × 200 indices) and its 6-bit palette.
    #[cfg(feature = "hires")]
    pub fn hd_picture(&self) -> (&[u8], &[u8]) {
        (&self.image, &self.image_palette)
    }

    /// Symbol mask `n` (0..16) as 320 × 200 pixels, read from planes 2 and 3
    /// as the scanout shows them: plane `2 + n % 2` at page `n / 2`. `new`
    /// decodes M.SUM there and nothing else writes those planes until the
    /// PSTART picture replaces everything.
    #[cfg(feature = "hires")]
    pub fn hd_mask(vga: &Vga, n: usize) -> Vec<bool> {
        let plane = &vga.planes[2 + n % 2];
        let page = (n / 2) * 0x2000;
        (0..320 * 200)
            .map(|at| {
                let (y, x) = (at / 320, at % 320);
                (plane[(page + y * 40 + x / 8) & 65535] >> (7 - x % 8)) & 1 != 0
            })
            .collect()
    }
}

impl Rings {
    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let source = data.file("M.DAT")?;
        let mask = data.file("M.SUM")?;
        let sine = data.file("M.SIN")?;
        if source.len() < 64448 || sine.len() != 512 || mask.len() < 34 {
            return Err("Invalid ring resources".into());
        }
        for p in &mut vga.planes {
            p.fill(0);
        }
        vga.palette.fill(0);
        for n in 0..16 {
            let mut at = word(&mask, 2 + n * 2) as usize;
            let page = (n / 2) * 8192;
            let plane = &mut vga.planes[2 + n % 2];
            let mut dest = page + 2;
            loop {
                if at + 2 > mask.len() {
                    return Err("Truncated M.SUM skip".into());
                }
                let skip = word(&mask, at) as usize;
                at += 2;
                if skip == 0 {
                    break;
                }
                if at + 2 > mask.len() || dest + skip * 2 > page + 8192 {
                    return Err("Invalid M.SUM skip".into());
                }
                plane[dest..dest + skip * 2].fill(0);
                dest += skip * 2;
                let count = word(&mask, at) as usize;
                at += 2;
                if at + count * 2 > mask.len() || dest + count * 2 > page + 8192 {
                    return Err("Invalid M.SUM literal".into());
                }
                plane[dest..dest + count * 2].copy_from_slice(&mask[at..at + count * 2]);
                at += count * 2;
                dest += count * 2;
            }
        }
        vga.ega(0, 40, 320, 200);
        Ok(Self {
            source,
            sine: sine
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as i32)
                .collect(),
            image: data.file("PSTART.U")?,
            image_palette: data.file("PSTART.PAL")?,
            frame: 0,
            intensity: -100,
            selector: false,
            page: 0x2000,
            phase1: 0,
            phase2: 0,
            previous_row: 0,
            rng: 1,
            started: false,
            transition: None,
        })
    }
    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        if let Some(level) = self.transition {
            if level >= 64 {
                return true;
            }
            for (out, &input) in vga.palette[3..].iter_mut().zip(&self.image_palette[3..]) {
                *out = (input as usize * level / 64) as u8 & 63;
            }
            self.transition = Some(level + 1);
            return false;
        }
        if !self.started {
            if music < 0x600 {
                return false;
            }
            self.started = true;
        }
        if self.frame >= 4464
            || (music == 0x7fff && self.frame > 1750)
            || (music != 0x7fff && music > 0x1000)
        {
            for p in &mut vga.planes {
                p.fill(0);
            }
            vga.palette.fill(0);
            vga.packed(&self.image, 320, 200);
            self.transition = Some(0);
            return self.step(vga, music);
        }
        let change = if music == 0x7fff {
            if self.frame < 700 && self.intensity < 64 {
                self.intensity += 1;
            }
            if self.frame > 1400 && self.intensity > 0 {
                self.intensity -= 1;
            }
            self.frame.is_multiple_of(8)
        } else {
            if music > 0xe00 {
                if self.intensity != 0 {
                    self.intensity -= 1;
                }
            } else if music > 0x71e && self.intensity < 64 {
                self.intensity += 1;
            }
            let row = music as u8;
            let change = row != self.previous_row && row & 1 == 0;
            self.previous_row = row;
            change
        };
        if change {
            self.selector = !self.selector;
            if !self.selector {
                self.page = self.page.wrapping_add(0x2000);
            }
        }
        let draw = self.page as usize;
        self.page ^= 0x2000;
        let active = if self.frame & 1 != 0 {
            64
        } else if self.selector {
            8
        } else {
            4
        };
        for index in 0..16 {
            let shade = if self.intensity <= 0 {
                0
            } else {
                [0, 48, 32, 0][index & 3] * self.intensity / 64
            };
            let rgb = if index & active != 0 {
                self.rng = self.rng.wrapping_mul(214013).wrapping_add(2531011);
                [63, 63, ((self.rng >> 16) & 31) as u8]
            } else {
                [shade as u8; 3]
            };
            vga.palette[index * 3..index * 3 + 3].copy_from_slice(&rgb);
        }
        self.phase1 = (self.phase1 & 4095).wrapping_add(35);
        self.phase2 = (self.phase2 & 4095).wrapping_sub(28);
        for (plane, phase, numerator, denominator) in
            [(1, self.phase1, 1, 1), (0, self.phase2, 9, 10)]
        {
            let i = (phase / 16) as usize;
            let x = self.sine[i & 255] * numerator / denominator + 50;
            let y = self.sine[(i + 64) & 255] * numerator / denominator + 50;
            let bank = (x & 7) as usize;
            for row in 0..184 {
                let r = y as usize + row;
                let mirror = if r < 152 { r } else { 303 - r };
                let src = bank * 8056 + mirror * 53 + (x / 8) as usize + 4;
                let dest = (draw + 324 + row * 40) & 65535;
                vga.planes[plane][dest..dest + 32].copy_from_slice(&self.source[src..src + 32]);
            }
        }
        self.frame += 1;
        vga.ega(self.page as usize, 40, 320, 200);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    /// Invented part-6 files in a temporary directory: blank rings, a
    /// sine table of amplitude 40, empty masks and a plain picture.
    #[cfg(feature = "hires")]
    fn invented_data() -> (std::path::PathBuf, Data) {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("panic-rings-hd-test-{}", std::process::id()));
        let files = dir.join("files");
        fs::create_dir_all(&files).unwrap();
        fs::write(files.join("P.P"), b"invented test marker").unwrap();
        fs::write(files.join("M.DAT"), vec![0; 64448]).unwrap();
        let sine: Vec<u8> = (0..256)
            .flat_map(|i| {
                let v = 40.0 * (i as f64 * std::f64::consts::TAU / 256.0).sin();
                (v.round() as i16).to_le_bytes()
            })
            .collect();
        fs::write(files.join("M.SIN"), sine).unwrap();
        let mut sum = vec![0; 36];
        for n in 0..16 {
            sum[2 + 2 * n..4 + 2 * n].copy_from_slice(&34_u16.to_le_bytes());
        }
        fs::write(files.join("M.SUM"), sum).unwrap();
        fs::write(files.join("PSTART.U"), vec![1; 64000]).unwrap();
        let palette: Vec<u8> = (0..768).map(|i| (i % 64) as u8).collect();
        fs::write(files.join("PSTART.PAL"), palette).unwrap();
        let data = Data::new(&dir).unwrap();
        (dir, data)
    }

    #[cfg(feature = "hires")]
    #[test]
    fn hd_state_reads_the_counters_on_invented_data() {
        let (dir, data) = invented_data();
        let mut vga = Vga::new();
        let mut scene = Rings::new(&data, &mut vga).unwrap();
        let waiting = scene.hd_state();
        assert_eq!((waiting.phase, waiting.frame, waiting.lit), (0, 0, None));
        assert!(!scene.step(&mut vga, 0x5ff));
        assert_eq!(scene.hd_state(), waiting);
        // Rows advance every step: the selector toggles on every even row.
        let mut lit = Vec::new();
        for k in 0..16_u16 {
            assert!(!scene.step(&mut vga, 0x600 + k));
            let state = scene.hd_state();
            assert_eq!((state.phase, state.frame), (1, k as usize + 1));
            assert_eq!(state.shown_page, state.drawn_page ^ 1);
            assert_eq!(state.intensity, -100);
            // Lit on alternate frames: exactly the lit plane's entries are yellow.
            let yellow: Vec<usize> = (0..16)
                .filter(|&i| vga.palette[i * 3] == 63 && vga.palette[i * 3 + 1] == 63)
                .collect();
            match state.lit {
                Some(n) => {
                    assert_eq!(n as usize / 2, state.shown_page);
                    let bit = 4 << (n % 2);
                    assert_eq!(yellow, (0..16).filter(|i| i & bit != 0).collect::<Vec<_>>());
                }
                None => assert!(yellow.is_empty()),
            }
            lit.push(state.lit.is_some());
        }
        assert_eq!(lit, (0..16).map(|k| k % 2 == 0).collect::<Vec<_>>());
        // The offsets are a function of the step alone.
        let sine =
            |i: usize| (40.0 * (i as f64 * std::f64::consts::TAU / 256.0).sin()).round() as i32;
        for step in [-3_i64, 0, 1, 15, 200] {
            let a = ((-28 * (step + 1)).rem_euclid(4096) / 16) as usize;
            let b = ((35 * (step + 1)).rem_euclid(4096) / 16) as usize;
            assert_eq!(
                scene.hd_offsets(step),
                [
                    [sine(a) * 9 / 10 + 50, sine(a + 64) * 9 / 10 + 50],
                    [sine(b) + 50, sine((b + 64) & 255) + 50],
                ],
                "step {step}"
            );
        }
        // The PSTART fade: levels 0 to 63, then done.
        for level in 0..64 {
            assert!(!scene.step(&mut vga, 0x1001));
            let state = scene.hd_state();
            assert_eq!(
                (state.phase, state.picture, state.lit),
                (2, Some(level), None)
            );
        }
        assert!(scene.step(&mut vga, 0x1001));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn music_wait_and_strict_exit_threshold() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        let mut scene = Rings::new(&data, &mut vga).unwrap();
        assert!(!scene.step(&mut vga, 0x5ff));
        assert_eq!(scene.frame, 0);
        for position in [0x600, 0x71e, 0x71f, 0xe00, 0xe01, 0x1000] {
            assert!(!scene.step(&mut vga, position));
        }
        assert_eq!(scene.frame, 6);
        assert_eq!(scene.intensity, -100);
        for _ in 0..64 {
            assert!(!scene.step(&mut vga, 0x1001));
        }
        assert!(scene.step(&mut vga, 0x1001));
        assert_eq!(scene.frame, 6);
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn all_original_fallback_planes_and_transition() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        let mut scene = Rings::new(&data, &mut vga).unwrap();
        let palette_text =
            crate::test_support::fixture_text("testdata/567/rings-palette-sha256.txt");
        let mut palettes = palette_text.lines();
        for line in crate::test_support::fixture_text("testdata/567/rings-sha256.txt").lines() {
            assert!(!scene.step(&mut vga, 0x7fff));
            assert_eq!(
                format!("{:x}", Sha256::digest(&vga.palette)),
                palettes.next().unwrap(),
                "palette frame"
            );
            let mut hash = Sha256::new();
            for plane in &vga.planes {
                hash.update(plane);
            }
            assert_eq!(
                format!("{:x}", hash.finalize()),
                line,
                "rings frame {}",
                scene.frame
            );
        }
        for _ in 0..64 {
            assert!(!scene.step(&mut vga, 0x7fff));
            assert_eq!(
                format!("{:x}", Sha256::digest(&vga.palette)),
                palettes.next().unwrap(),
                "palette frame"
            );
        }
        assert!(scene.step(&mut vga, 0x7fff));
        assert_eq!(vga.indices, data.file("PSTART.U").unwrap());
        let mut expected = data.file("PSTART.PAL").unwrap();
        for value in &mut expected {
            *value = (*value as u16 * 63 / 64) as u8;
        }
        expected[..3].fill(0);
        assert_eq!(vga.palette, expected);
    }
}
