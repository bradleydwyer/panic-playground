//! Part6: bitplane interference rings and the PSTART transition.
//! Original 0x1161c control, 0x1130e mask decoder, 0x11372 window copier.
use crate::{
    data::{Data, word},
    vga::Vga,
};

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
