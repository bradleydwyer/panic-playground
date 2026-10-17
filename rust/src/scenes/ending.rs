//! Part 8: inherited-image monitor collapse. Operations are grouped at the
//! original retrace waits. CRT line-compare stages are sampled into 200 rows.
use crate::{data::Data, vga::Vga};

const SCALES: [usize; 13] = [7, 8, 9, 10, 12, 14, 16, 19, 22, 26, 31, 37, 44];

pub struct Ending {
    original: Vec<u8>,
    frame: usize,
    latches: [u8; 4],
}

impl Ending {
    pub fn new(_data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let original = vga.palette.clone();
        for plane in &mut vga.planes {
            plane[16000..65535].fill(0);
        }
        Ok(Self {
            original,
            frame: 0,
            latches: [0; 4],
        })
    }

    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        if self.frame == 0 && music < 0x3500 {
            return false;
        }
        self.frame += 1;
        match self.frame {
            1 => vga.mode_x(0, 80, 320, 200),
            2 => {
                vga.palette[..3].fill(63);
                vga.mode_x(0, 80, 320, 200);
            }
            3 => vga.mode_x(16000, 80, 320, 200),
            4..=7 => {
                let scale = self.frame - 2;
                self.fade(vga, scale);
                self.hardware_compression(vga, scale);
            }
            8..=20 => {
                let index = self.frame - 8;
                self.fade(vga, if index == 0 { 6 } else { SCALES[index - 1] });
                self.compress(vga, SCALES[index]);
                vga.mode_x(16000, 80, 320, 200);
            }
            21 => {
                self.fade(vga, 44);
                for dest in [0x5d20, 0x5d70, 0x5e10, 0x5e60] {
                    self.copyline(vga, 0, dest);
                }
                vga.mode_x(16000, 80, 320, 200);
            }
            22..=68 => {
                let x = 20 + 3 * (self.frame - 22);
                for j in 0..4 {
                    for px in [x + j, 320 - x - j] {
                        // Write mode 1 ignores the apparent zero argument.
                        vga.planes[px & 3][24000 + (px >> 2)] = self.latches[px & 3];
                    }
                }
                if self.frame == 68 {
                    vga.planes[0][24040] = 1;
                }
                vga.mode_x(16000, 80, 320, 200);
            }
            69..=128 => {
                let i = self.frame - 69;
                let level = dot_level(i);
                vga.palette[3..6].fill(level);
                vga.mode_x(16000, 80, 320, 200);
            }
            _ => {}
        }
        // Original sleep(1) follows the final pulse. Keep the final image.
        self.frame >= 198
    }

    fn fade(&self, vga: &mut Vga, scale: usize) {
        vga.palette[..3].fill(0);
        for i in 3..768 {
            vga.palette[i] = (scale + self.original[i] as usize * (63 - scale) / 63) as u8;
        }
    }

    fn copyline(&mut self, vga: &mut Vga, source: usize, destination: usize) {
        for p in 0..4 {
            for i in 0..80 {
                vga.planes[p][destination + i] = vga.planes[p][source + i];
            }
            self.latches[p] = vga.planes[p][source + 79];
        }
    }

    fn compress(&mut self, vga: &mut Vga, scale: usize) {
        let mut row = 100 - 100 / scale;
        self.copyline(vga, 0, 16000 + (row - 1) * 80);
        for source_row in (100 % scale..200).step_by(scale) {
            self.copyline(vga, source_row * 80, 16000 + row * 80);
            row += 1;
        }
        self.copyline(vga, 0, 16000 + row * 80);
    }

    fn hardware_compression(&self, vga: &mut Vga, scale: usize) {
        // CRTC start=16000, stride=80*scale, line compare=2*(100-100/scale).
        // The half-row at the physical compare edge is sampled at its first
        // physical scanline; no claim of analog CRT interpolation is made.
        vga.mode_x(16000, 80 * scale, 320, 200);
        let boundary = 100 - 100 / scale;
        for y in boundary + 1..200 {
            for x in 0..320 {
                let at = ((y - boundary - 1) * 80 * scale + (x >> 2)) & 65535;
                vga.indices[y * 320 + x] = vga.planes[x & 3][at];
            }
        }
    }
}

// The decimal constant is recovered from Panic, not an approximation to improve.
#[allow(clippy::approx_constant)]
fn dot_level(i: usize) -> u8 {
    (31.0_f64 * ((i as f64 / 120.0) * 3.0 * 2.0 * 3.1415926535).cos() + 32.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn every_original_retrace_state_and_inherited_latches() {
        let data = crate::test_support::data();
        let fixture = crate::test_support::fixture("tests/fixtures/ending-states.sha256");
        for case in 0..2 {
            let mut vga = Vga::new();
            for p in 0..4 {
                for i in 0..65536 {
                    vga.planes[p][i] = ((i * 37 + (i >> 8) * 11 + p * 61) & 255) as u8;
                }
                vga.planes[p][..80].fill(if case == 0 { 0 } else { (p * 17 + 17) as u8 });
            }
            for i in 0..768 {
                vga.palette[i] = ((i * 17 + 3) % 64) as u8;
            }
            let mut scene = Ending::new(&data, &mut vga).unwrap();
            assert!(!scene.step(&mut vga, 0x343f));
            assert_eq!(scene.frame, 0);
            for frame in 1..=198 {
                let done = scene.step(&mut vga, 0x3500);
                let mut digest = Sha256::new();
                for plane in &vga.planes {
                    digest.update(plane);
                }
                digest.update(&vga.palette);
                let at = (case * 128 + frame.min(128) - 1) * 32;
                assert_eq!(
                    &digest.finalize()[..],
                    &fixture[at..at + 32],
                    "ending case {case} frame {frame}"
                );
                assert_eq!(done, frame == 198);
            }
        }
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_FIXTURES; run with --ignored"]
    fn dot_matches_native_and_borland_software_fp_vectors() {
        let expected = crate::test_support::fixture("tests/fixtures/ending-dot.bin");
        assert_eq!((0..60).map(dot_level).collect::<Vec<_>>(), expected);
    }
}
