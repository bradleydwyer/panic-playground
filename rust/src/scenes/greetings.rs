//! Part7: five descriptor queues copying pre-rendered font rows through VGA latches.
//! Original main 0x12645; software and native FP yield the same601 variant bytes.
use crate::{
    data::{Data, word},
    vga::Vga,
};

pub struct Greetings {
    texts: [Vec<u8>; 5],
    indices: [usize; 5],
    rows: [[u16; 201]; 5],
    sine: [u8; 601],
    positions: [usize; 5],
    frame: usize,
    intensity: u16,
    phase: usize,
    work: Vec<u8>,
    previous_music: u16,
    stage: u8,
    fade_level: usize,
    monster: Vec<u8>,
    monster_palette: Vec<u8>,
}
fn triangle(phase: usize, intensity: u16) -> [u8; 192] {
    let mut result = [0; 192];
    for i in 0..64 {
        let j = (i + phase) & 63;
        let v = (3 * j.min(63 - j) as i32 - 33).max(0) as u16;
        result[i * 3] = (v * intensity / 64) as u8;
        result[i * 3 + 1] = ((v * 8 / 9) * intensity / 64) as u8;
        result[i * 3 + 2] = ((v * 7 / 9) * intensity / 64) as u8;
    }
    result
}
impl Greetings {
    // Preserve the original decimal constant and operation order.
    #[allow(clippy::approx_constant)]
    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let memory = data.memory(7)?;
        let texts = std::array::from_fn(|i| {
            let at = 0x16140 + 0x92 + i * 4;
            let pointer = word(&memory, at) as usize + word(&memory, at + 2) as usize * 16;
            // Original byte indices can read beyond a text's terminating NUL:
            // retain256 bytes for the second-strip termination-check quirk.
            memory[pointer..pointer + 256].to_vec()
        });
        for p in 0..4 {
            let source = data.file(&format!("TRUG_{p}.MAP"))?;
            if source.len() != 64000 {
                return Err("Invalid TRUG map size".into());
            }
            vga.planes[p].fill(0);
            vga.planes[p][..64000].copy_from_slice(&source);
        }
        vga.palette.fill(0);
        vga.palette[..24].copy_from_slice(&[
            0, 0, 0, 60, 60, 60, 44, 52, 52, 32, 44, 44, 20, 36, 36, 12, 32, 32, 4, 24, 24, 0, 0, 0,
        ]);
        vga.mode_x(0, 80, 320, 200);
        let sine = std::array::from_fn(|i| {
            (((((i as f64 * 3.1415926535 / 200.0).sin() * 40.0 + 60.0) as i32) & 15) * 7) as u8
        });
        Ok(Self {
            texts,
            indices: [1, 1, 1, 1, 0],
            rows: [[0xf490; 201]; 5],
            sine,
            positions: [400, 330, 300, 260, 230],
            frame: 0,
            intensity: 0,
            phase: 0,
            work: vec![0; 768],
            previous_music: 0,
            stage: 0,
            fade_level: 60,
            monster: data.file("MONSTER.U")?,
            monster_palette: data.file("MONSTER.PAL")?,
        })
    }
    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        if self.stage == 3 {
            return true;
        }
        if self.stage == 0
            && (self.frame >= 1100
                || (self.previous_music != 0x7fff && self.previous_music >= 0x341e))
        {
            self.stage = 1;
            self.work.copy_from_slice(&vga.palette);
        }
        if self.stage == 1 {
            // Upload precedes computation: the final feedback result is never
            // uploaded, matching the native60,56,...4 loop.
            vga.palette.copy_from_slice(&self.work);
            self.phase = (self.phase + 1) & 63;
            self.work[576..].copy_from_slice(&triangle(self.phase, 64));
            for c in &mut self.work {
                *c = (64 - self.fade_level + *c as usize * self.fade_level / 64) as u8;
            }
            if self.fade_level == 4 {
                self.stage = 2;
                self.fade_level = 0;
            } else {
                self.fade_level -= 4;
            }
            return false;
        }
        if self.stage == 2 {
            if self.fade_level == 0 {
                vga.packed(&self.monster, 320, 200);
            }
            for (c, &original) in vga.palette.iter_mut().zip(&self.monster_palette) {
                *c = (original as usize * self.fade_level / 64 + 63 * (64 - self.fade_level) / 64)
                    as u8;
            }
            if self.fade_level == 64 {
                self.stage = 3;
            } else {
                self.fade_level += 1;
            }
            return false;
        }
        self.previous_music = music;
        vga.palette[576..].copy_from_slice(&self.work[..192]);
        self.frame += 1;
        self.phase = (self.phase + 1) & 63;
        self.work[..192].copy_from_slice(&triangle(self.phase, self.intensity));
        if self.frame < 100 && self.intensity < 64 {
            self.intensity += 1;
        }
        let row_offset = ((self.frame - 1) % 18 * 112) as u16;
        // Compute all bottom descriptors before any queue shifts. The leading
        // period reads queue5[182], matching the unchecked original alphabet lookup.
        for strip in 0..5 {
            self.rows[strip][200] = if row_offset <= 0x620 {
                let ch = self.texts[strip][self.indices[strip]];
                let base = if ch == b'.' {
                    self.rows[4][182]
                } else {
                    0x3e90u16.wrapping_add((ch as u16).wrapping_sub(65).wrapping_mul(0x700))
                };
                base.wrapping_add(row_offset)
            } else {
                0xf490 + u16::from(self.texts[0][self.indices[strip]] == b'[')
            };
        }
        for strip in 0..5 {
            self.rows[strip].copy_within(1..201, 0);
        }
        for y in 0..200 {
            for strip in 0..5 {
                let source = self.rows[strip][y];
                if source > 0xf490 {
                    continue;
                }
                let target = y * 80 + 2 + strip * 16;
                let from = if source == 0xf490 {
                    target + 7
                } else {
                    source.wrapping_add(self.sine[self.positions[strip] + y] as u16) as usize
                };
                for plane in &mut vga.planes {
                    for byte in 0..7 {
                        plane[target + byte] = plane[(from + byte) & 65535];
                    }
                }
            }
        }
        for position in &mut self.positions {
            *position = if *position == 0 { 400 } else { *position - 1 };
        }
        if self.frame.is_multiple_of(18) {
            for strip in 0..5 {
                let test = if strip == 1 {
                    self.indices[0]
                } else {
                    self.indices[strip]
                };
                self.indices[strip] = if self.texts[strip][test] == 0 {
                    1
                } else {
                    (self.indices[strip] + 1) & 255
                };
            }
        }
        if self.frame == 1 {
            self.rows = [[0xf491; 201]; 5];
        }
        vga.mode_x(0, 80, 320, 200);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn music_exit_uses_previous_position_then_completes_both_fades() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        let mut scene = Greetings::new(&data, &mut vga).unwrap();
        assert!(!scene.step(&mut vga, 0x341e));
        assert_eq!(scene.frame, 1);
        for _ in 0..80 {
            assert!(!scene.step(&mut vga, 0x341e));
        }
        assert!(scene.step(&mut vga, 0x341e));
        assert_eq!(scene.frame, 1);
        assert_eq!(vga.indices, data.file("MONSTER.U").unwrap());
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn all_original_fallback_planes_and_monster_transition() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        let mut scene = Greetings::new(&data, &mut vga).unwrap();
        let palette_text =
            crate::test_support::fixture_text("testdata/567/greetings-palette-sha256.txt");
        let mut palettes = palette_text.lines();
        assert_eq!(
            &scene.sine[..],
            crate::test_support::fixture("testdata/567/greetings-sine.bin")
        );
        for line in crate::test_support::fixture_text("testdata/567/greetings-sha256.txt").lines() {
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
                "greetings frame {}",
                scene.frame
            );
        }
        for _ in 0..80 {
            assert!(!scene.step(&mut vga, 0x7fff));
            assert_eq!(
                format!("{:x}", Sha256::digest(&vga.palette)),
                palettes.next().unwrap(),
                "palette frame"
            );
        }
        assert!(scene.step(&mut vga, 0x7fff));
        assert_eq!(vga.indices, data.file("MONSTER.U").unwrap());
        assert_eq!(vga.palette, data.file("MONSTER.PAL").unwrap());
        assert!(vga.planes.iter().all(|p| p[..80].iter().all(|&v| v == 0)));
    }
}
