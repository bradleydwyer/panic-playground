//! Part 2: original tile borders, paired font columns, palette oscillator,
//! and bit-7 sine text. Planes are exact logical VGA storage; the raster split
//! is sampled at logical line 100 rather than emulating PIT/CRT timing.
use crate::{
    data::{Data, word},
    vga::Vga,
};

pub struct Scroller {
    memory: Vec<u8>,
    text: Vec<u8>,
    descriptors: Vec<(usize, usize)>,
    tiles: [(usize, usize); 160],
    phase: u8,
    tick: usize,
    a: usize,
    b: usize,
    forward: usize,
    backward: usize,
    page: usize,
    previous_a: usize,
    previous_b: usize,
    previous_page: usize,
    sx: i16,
    sy: i16,
    xx: i16,
    yy: i16,
    history: [[usize; 50]; 2],
    s: usize,
    y: usize,
    t: usize,
    display: usize,
    split: i32,
    parity: usize,
}

impl Scroller {
    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let memory = data.memory(2)?;
        let text: Vec<u8> = memory[0x4c5f2..0x4c9f2]
            .iter()
            .take_while(|&&x| x != 0)
            .map(|&x| {
                if x as i8 >= 65 {
                    x - 64
                } else if x == 32 {
                    0
                } else {
                    x
                }
            })
            .collect();
        let mut descriptors = Vec::new();
        for segment in [0x2a82, 0x3a22] {
            for page in 0..2 {
                for row in 0..12 {
                    for col in 0..6 {
                        descriptors.push((0x1400 + page * 0x7800 + row * 26 + col * 4, segment));
                    }
                }
            }
        }
        descriptors.extend([(0x153c, 0x2a82), (0x8d3c, 0x2a82)]);
        let mut tiles = [(0, 0); 160];
        for row in 0..10 {
            for col in 0..16 {
                let order = word(&memory, 0x4bc20 + 2 * (row * 16 + col)) as usize;
                tiles[order - 1] = (row * 3200 + col * 5, 0x8000 + row * 1600 + col * 5);
            }
        }
        let history = std::array::from_fn(|s| {
            std::array::from_fn(|i| word(&memory, 0x4ebca + s * 1024 + i * 2) as usize)
        });
        vga.mode_x(0, 80, 320, 200);
        Ok(Self {
            memory,
            text,
            descriptors,
            tiles,
            phase: 0,
            tick: 0,
            a: 0,
            b: 0,
            forward: 1,
            backward: 0,
            page: 160,
            previous_a: 79,
            previous_b: 79,
            previous_page: 0x80a0,
            sx: 384,
            sy: 0,
            xx: 0,
            yy: 12800,
            history,
            s: 0,
            y: 0,
            t: 0,
            display: 0,
            split: 0,
            parity: 0,
        })
    }

    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        match self.phase {
            0 => {
                self.tick += 1;
                if self.tick == 200 {
                    self.begin_transition(vga);
                    self.phase = 1;
                    self.tick = 0;
                }
            }
            1 => {
                for step in 0..10 {
                    if self.tick >= step && self.tick - step < 160 {
                        let (src, dst) = self.tiles[self.tick - step];
                        tile(vga, step, src, dst);
                    }
                }
                vga.mode_x(0x8000, 80, 320, 200);
                self.tick += 1;
                if self.tick == 169 {
                    self.phase = 2;
                    self.tick = 0;
                }
            }
            2 => {
                self.finish_transition(vga);
                self.phase = 3;
                self.tick = 0;
            }
            3 => {
                self.tick += 1;
                if self.tick == 101 {
                    self.phase = 4;
                    self.tick = 0;
                }
            }
            4 => {
                if self.forward / 13 >= self.text.len() || (music != 0x7fff && music >= 0x0534) {
                    self.phase = 5;
                    self.tick = 0;
                    self.fade_frame(vga);
                } else {
                    self.render_frame(vga);
                }
            }
            5 => self.fade_frame(vga),
            _ => return true,
        }
        self.phase == 6
    }

    fn begin_transition(&mut self, vga: &mut Vga) {
        for plane in &mut vga.planes {
            plane.copy_within(0..0x4000, 0x8000);
        }
        for p in 0..4 {
            for row in 0..202 {
                for col in 0..160 {
                    vga.planes[p][row * 160 + col] =
                        (64 + 8 * ((row + 6) & 7) + 4 * (col & 1) + p) as u8;
                }
            }
        }
        vga.mode_x(0x8000, 80, 320, 200);
    }

    fn finish_transition(&mut self, vga: &mut Vga) {
        for plane in &mut vga.planes {
            plane.copy_within(0..0x8000, 0x8000);
        }
        for i in 128..256 {
            vga.palette[i * 3..i * 3 + 3].fill(63);
        }
        for i in 1..64 {
            vga.palette[i * 3..i * 3 + 3].fill((63 - i) as u8);
        }
        self.display = 0;
        self.split = 0;
        vga.mode_x(0, 160, 320, 200);
    }

    fn palette(&mut self, vga: &mut Vga, anim: usize) {
        let x = ((self.xx >> 7) & 7) as usize;
        let y = ((self.yy >> 7) & 7) as usize;
        self.yy = self.yy.wrapping_add(self.sy);
        self.xx = self.xx.wrapping_add(self.sx);
        self.sy = self.sy.wrapping_add(if self.yy <= 0 { 4 } else { -4 });
        self.sx = self.sx.wrapping_add(if self.xx <= 0 { 4 } else { -4 });
        let offset = word(&self.memory, 0x4c9f2 + 2 * anim) as usize * 768
            + 3 * (((x + 4 * self.parity) & 7) + 16 * y);
        for row in 0..8 {
            for i in 0..24 {
                vga.palette[192 + row * 24 + i] =
                    self.memory[0x14130 + ((offset + 48 * row + i) & 65535)];
            }
        }
    }

    fn font(&self, vga: &mut Vga, position: usize, destination: usize) {
        let ch = self.text[position / 26] as usize;
        let descriptor = word(&self.memory, 0x4bdf2 + 2 * (ch * 32 + position % 26)) as usize;
        let (offset, segment) = self.descriptors[descriptor];
        for p in 0..4 {
            for row in 0..80 {
                let color = self.memory[segment * 16 + ((offset + p + 320 * row) & 65535)];
                vga.planes[p][(destination + 160 * row + 79) & 65535] = color;
                vga.planes[p][(destination + 160 * row).wrapping_sub(1) & 65535] = color;
            }
        }
    }

    fn overlay(&mut self, vga: &mut Vga, base: usize, split: i32) {
        self.s ^= 1;
        for &at in &self.history[self.s] {
            for j in 0..12 {
                for plane in &mut vga.planes {
                    plane[(at + j) & 65535] &= 127;
                }
            }
        }
        let base = (base + 640 - (self.y & 3) * 160) & 65535;
        let sine = (2 * (2048_i32 - self.y as i32 - (self.y & 3) as i32)) as usize & 65535;
        for i in 0..50 {
            let x = word(&self.memory, 0x49c20 + ((sine + i * 8) & 65535)) as usize;
            let at = ((base + i * 640 + (x >> 2)) as i32 + if i >= 25 { split } else { 0 })
                as usize
                & 65535;
            self.history[self.s][i] = at;
            let code = self.memory[0x4cda2 + 2 * (self.t + i)];
            for j in 0..12 {
                if glyph(code, j) {
                    vga.planes[x & 3][(at + j) & 65535] |= 128;
                }
            }
        }
        self.y = (self.y + 1) & 2047;
        if self.y & 3 == 0 {
            self.t = (self.t + 1) % 375;
        }
    }

    fn render_frame(&mut self, vga: &mut Vga) {
        self.display = (self.previous_page + self.previous_a) & 65535;
        self.split = self.b as i32 - self.a as i32;
        self.parity = self.a & 1;
        self.palette(vga, 0);
        self.font(vga, self.forward, (self.a + self.page + 0x500) & 65535);
        column(
            vga,
            (self.previous_a + self.previous_page + 0x4ff) & 65535,
            (self.previous_a + self.page + 0x500) & 65535,
        );
        self.font(vga, self.backward, (self.b + self.page + 0x45ff) & 65535);
        column(
            vga,
            (self.previous_b + self.previous_page + 0x45fe) & 65535,
            (self.previous_b + self.page + 0x45ff) & 65535,
        );
        self.forward = (self.forward + 1) % (self.text.len() * 26);
        self.backward = if self.backward == 0 {
            self.text.len() * 26 - 1
        } else {
            self.backward - 1
        };
        self.previous_a = self.a;
        self.previous_b = self.b;
        self.previous_page = self.page;
        self.a = (self.a + 1) % 80;
        self.b = if self.b == 0 { 79 } else { self.b - 1 };
        self.page = if self.page == 160 { 0x80a0 } else { 160 };
        self.overlay(
            vga,
            self.previous_page + self.previous_a,
            self.b as i32 - self.a as i32,
        );
        self.scanout(vga);
    }

    fn scanout(&self, vga: &mut Vga) {
        vga.mode_x(self.display, 160, 320, 200);
        // The original changes row stride for a short horizontal interval.
        // Sample its accumulated address displacement at the band boundary.
        for y in 100..200 {
            for x in 0..320 {
                let at = ((self.display + y * 160 + (x >> 2)) as i32 + self.split) as usize & 65535;
                vga.indices[y * 320 + x] = vga.planes[x & 3][at];
            }
        }
    }

    fn fade_frame(&mut self, vga: &mut Vga) {
        self.palette(vga, self.tick / 2 + 1);
        self.scanout(vga);
        self.tick += 1;
        if self.tick == 12 {
            self.phase = 6;
        }
    }
}

fn column(vga: &mut Vga, source: usize, destination: usize) {
    for row in 0..80 {
        for plane in &mut vga.planes {
            let color = plane[(source + row * 160) & 65535];
            plane[(destination + row * 160 + 79) & 65535] = color;
            plane[(destination + row * 160).wrapping_sub(1) & 65535] = color;
        }
    }
}

fn tile(vga: &mut Vga, step: usize, source: usize, destination: usize) {
    let left = step / 4;
    let right = 4 - left;
    let first = step;
    let last = 19 - step;
    let mut copy = |row: usize, col: usize, mask: usize| {
        let src = (source + 160 * row + col) & 65535;
        let dst = (destination + 80 * row + col) & 65535;
        for p in 0..4 {
            if mask & (1 << p) != 0 {
                vga.planes[p][dst] = vga.planes[p][src];
            }
        }
    };
    for row in first..=last {
        copy(row, left, 1 << (step & 3));
    }
    for row in first..=last {
        copy(row, right, 8 >> (step & 3));
    }
    for col in left..=right {
        for row in [first, last] {
            copy(row, col, 15);
        }
    }
}

fn glyph(code: u8, x: usize) -> bool {
    match code {
        0 => false,
        2 => true,
        4 => !(3..9).contains(&x),
        6 => (1..=10).contains(&x),
        8 => x < 3,
        10 => x >= 9,
        12 => x < 10,
        14 => (4..=7).contains(&x),
        _ => panic!("Invalid original sine glyph {code}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn every_plane_matches_original_transition_and_1520_frames() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        for p in 0..4 {
            for i in 0..65536 {
                vga.planes[p][i] = ((i * 37 + (i >> 8) * 11 + p * 61) & 255) as u8;
            }
        }
        let mut scene = Scroller::new(&data, &mut vga).unwrap();
        scene.begin_transition(&mut vga);
        for f in 0..169 {
            for k in 0..10 {
                if f >= k && f - k < 160 {
                    let (s, d) = scene.tiles[f - k];
                    tile(&mut vga, k, s, d);
                }
            }
        }
        scene.finish_transition(&mut vga);
        let fixture = crate::test_support::fixture("tests/fixtures/scroller-planes.sha256");
        let palettes = crate::test_support::fixture("tests/fixtures/scroller-palettes.sha256");
        for frame in 0..=1520 {
            if frame != 0 {
                scene.render_frame(&mut vga);
            }
            for p in 0..4 {
                let got = Sha256::digest(&vga.planes[p]);
                assert_eq!(
                    &got[..],
                    &fixture[(frame * 4 + p) * 32..(frame * 4 + p + 1) * 32],
                    "frame {frame} plane {p}"
                );
            }
            assert_eq!(
                &Sha256::digest(&vga.palette)[..],
                &palettes[frame * 32..(frame + 1) * 32],
                "palette frame {frame}"
            );
        }
        assert_eq!(scene.forward, 1521);
        assert_eq!(scene.t, 5);
        assert_eq!(scene.y, 1520);
        scene.tick = 0;
        for fade in 0..12 {
            scene.fade_frame(&mut vga);
            let frame = 1521 + fade;
            assert_eq!(
                &Sha256::digest(&vga.palette)[..],
                &palettes[frame * 32..(frame + 1) * 32],
                "exit palette {fade}"
            );
        }
        assert_eq!(scene.phase, 6);
    }
}
