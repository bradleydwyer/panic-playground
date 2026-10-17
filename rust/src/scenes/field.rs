//! Part5 recursive field and adaptive integer terrain rays.
//! See analysis/effects-567.md and its original-instruction differential oracles.
use crate::{
    data::{Data, signed},
    vga::Vga,
};

fn random(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(214013).wrapping_add(2531011);
    (*state >> 16) & 32767
}
fn segment(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.resize(65536, 0);
    bytes
}
fn expand(source: &[u8], destination: &mut [u8], row: usize) {
    let taps = [
        [0, -257, -514],
        [1, -255, -510],
        [256, 255, 510],
        [257, 257, 514],
    ];
    for x in 0..128 {
        let p = ((row + 50) * 256 + 64 + x) as i32;
        let center = source[p as usize] as u32 * 5;
        for (i, offsets) in taps.iter().enumerate() {
            let sum: u32 = offsets
                .iter()
                .map(|&o| source[((p + o) as u16) as usize] as u32)
                .sum();
            destination[row * 512 + x * 2 + (i & 1) + (i >> 1) * 256] =
                ((center + sum + 4) / 8) as u8;
        }
    }
}
fn perturb(source: &mut [u8], state: &mut u32) {
    for x in -2i32..=2 {
        for y in -2i32..=2 {
            let distance = (x.abs() + y.abs()).max(1);
            let p = ((y + 100) * 256 + x + 128) as usize;
            let old = source[p] as i8 as i32;
            source[p] = ((old * distance + (random(state) & 127) as i32 - 63) / distance)
                .clamp(1, 127) as u8;
        }
    }
}
fn zoom(source: &[u8], output: &mut [u8], fraction: u16, integer: u16) {
    let start = |center: u16, multiplier: u32| {
        let product = fraction as u32 * multiplier;
        let low = (product as u16).wrapping_neg();
        let high = center
            .wrapping_sub((product >> 16) as u16)
            .wrapping_add(if integer == 2 { center } else { 0 });
        (high >> 1, ((high & 1) << 15) | (low >> 1))
    };
    let (sx, fx) = start(128, 127);
    let (mut sy, mut fy) = start(100, 99);
    for y in 0..100 {
        let row = if sy < 200 {
            sy.wrapping_mul(256)
        } else {
            (sy - 200).wrapping_mul(320).wrapping_sub(1120)
        };
        let mut p = row.wrapping_add(sx);
        let mut acc = fx;
        for x in 0..128 {
            output[0x39e0 + y * 320 + x] = source[p as usize];
            let (sum, carry) = acc.overflowing_add(fraction);
            p = p.wrapping_add(integer).wrapping_add(u16::from(carry));
            acc = sum;
        }
        let (sum, carry) = fy.overflowing_add(fraction);
        sy = sy.wrapping_add(integer).wrapping_add(u16::from(carry));
        fy = sum;
    }
}
fn prepare(colors: &mut [u8], heights: &mut [u8]) {
    for x in 0..200 {
        for y in 0..200 {
            let p = y * 320 + x;
            heights[p] = heights[p].saturating_sub(128) * 2;
        }
    }
    for x in 0..200 {
        for y in 0..200 {
            let p = y * 320 + x;
            colors[p] = ((3 * colors[p] as u16 + heights[p] as u16) / 4) as u8;
            if heights[p] < 16 {
                colors[p] = (colors[p] as u16 * (heights[p] as u16 + 16) / 32) as u8;
            }
        }
    }
    let mut maximum = 0;
    for x in 0..200 {
        for y in 0..200 {
            let p = y * 320 + x;
            heights[p] =
                ((heights[p] as u16 + heights[p + 1] as u16 + heights[p + 320] as u16) / 3) as u8;
            colors[p] =
                ((colors[p] as u16 + colors[p + 1] as u16 + colors[p + 320] as u16) / 3) as u8;
            maximum = maximum.max(heights[p]);
        }
    }
    for x in 0..200 {
        for y in 0..200 {
            let p = y * 320 + x;
            colors[p] = colors[p].clamp(4, 191);
            if x == 0 || x == 199 || y == 0 || y == 199 {
                colors[p] = 1;
                heights[p] = 0;
            }
        }
    }
    for x in 0..200 {
        for y in 0..200 {
            let p = y * 320 + x;
            colors[p] = 128 + (colors[p] as u16 * 64 / 192) as u8;
            if heights[p] == maximum {
                colors[p] = 147;
            }
        }
    }
    for y in 0..200 {
        for x in 0..200 {
            colors[y * 256 + x] = colors[y * 320 + x];
            heights[y * 256 + x] = heights[y * 320 + x];
        }
    }
}
fn ray(
    colors: &[u8],
    heights: &[u8],
    background: &[u8],
    output: &mut [u8],
    column: i32,
    camera: [i32; 5],
    projection: &[i32; 512],
) {
    let [wx, wy, height, sine, cosine] = camera;
    let endpoint = |z: i32, fractional: bool| {
        let horizontal = (((column - 160) as i16 as i32 * z) >> 7) as i16 as i32;
        let mut result = [0u32; 2];
        for (n, (side, forward, center)) in
            [(cosine, sine, wx), (-sine, cosine, wy)].iter().enumerate()
        {
            let a = *side as i16 as i32 * horizontal;
            let b = *forward as i16 as i32 * z;
            let whole = ((a >> 9) + (b >> 8) + center) as u16;
            let frac = if fractional { ((a + b) << 9) as u16 } else { 0 };
            result[n] = ((whole as u32) << 16) | frac as u32;
        }
        result
    };
    let start = endpoint(64, true);
    let finish = endpoint(512, false);
    let mut delta = [0u32; 2];
    for i in 0..2 {
        let difference = ((finish[i] >> 16) as i16 as i32) - ((start[i] >> 16) as i16 as i32);
        delta[i] = (difference.wrapping_shl(16) / 449) as u32;
    }
    let (mut x, mut y) = (start[0].rotate_left(16), start[1].rotate_left(16));
    let (dx, dy) = (delta[0].rotate_left(16), delta[1].rotate_left(16));
    let fraction = |v: u32, d: u32| v.overflowing_add(d & 0xffff0000);
    let integer = |v: u32, d: u32, carry: bool| {
        (v & 0xffff0000)
            | ((v as u16)
                .wrapping_add(d as u16)
                .wrapping_add(u16::from(carry))) as u32
    };
    let mut top = 0i32;
    let mut distance = 64usize;
    while distance < 512 {
        let (v, c) = fraction(x, dx);
        x = integer(v, dx, c);
        let ix = x as i16 as i32;
        let (v, c) = fraction(y, dy);
        let iy = v as i16 as i32;
        y = integer(v, dy, c);
        let ix = if (0..200).contains(&ix) { ix } else { 0 };
        let iy = if (0..200).contains(&iy) { iy } else { 0 };
        let p = (ix * 256 + iy) as usize;
        let level = heights[p] as i32;
        let projected =
            (((((level - height) as i16 as i32) * (projection[distance] as i16 as i32)) >> 10)
                + 200) as i16 as i32;
        if level != 0 && projected > top {
            let count = (projected - top) as u8;
            let count = if count == 0 { 256 } else { count as i32 };
            for j in 0..count {
                let q = ((199 - top - j) * 320 + column) as u16 as usize;
                output[q] = colors[p];
                output[(q + 1) & 65535] = colors[p + 1];
            }
            top = projected;
            distance += 1;
        } else {
            let (v, c) = fraction(x, dx);
            x = integer(v, dx, c);
            let (v, c) = fraction(y, dy);
            y = integer(v, dy, c);
            distance += 2;
        }
    }
    if top < 199 {
        let count = (199 - top) as u8;
        let count = if count == 0 { 256 } else { count as i32 };
        for j in 0..count {
            let q = ((199 - top - j) * 320 + column) as u16 as usize;
            output[q] = background[q];
            output[(q + 1) & 65535] = background[(q + 1) & 65535];
        }
    }
}

pub struct Field {
    source: Vec<u8>,
    destination: Vec<u8>,
    video: Vec<u8>,
    palette: Vec<u8>,
    scale: i32,
    speed: i32,
    next_row: usize,
    rng: u32,
    frame: usize,
    start_music: Option<u16>,
    terrain: bool,
    terrain_frame: usize,
    colors: Vec<u8>,
    heights: Vec<u8>,
    terrain_palette: Vec<u8>,
    sine: Vec<i32>,
    projection: [i32; 512],
    background: Vec<u8>,
    output: Vec<u8>,
    level: i32,
    previous_music: u16,
    cleared: bool,
    done: bool,
}
impl Field {
    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let memory = data.memory(5)?;
        let mut palette = data.file("WELLPAL.PAL")?;
        let window = data.file("WINDOW.PAL")?;
        if palette.len() != 768 || window.len() != 768 {
            return Err("Invalid field palette size".into());
        }
        palette[..10].fill(16);
        palette[384..].copy_from_slice(&window[384..]);
        vga.palette[3..].copy_from_slice(&palette[3..]);
        palette.copy_within(..384, 384);
        let mut video = segment(if vga.indices.len() == 64000 {
            vga.indices.clone()
        } else {
            data.file("PSTART.U")?
        });
        video[..320].fill(255);
        for y in 0..200 {
            video[y * 320] = 255;
        }
        vga.packed(&video, 320, 200);
        let mut colors = segment(data.file("MAP.U")?);
        let mut heights = segment(data.file("DEP.U")?);
        prepare(&mut colors, &mut heights);
        Ok(Self {
            source: segment(data.file("PBUF1.PRE")?),
            destination: segment(data.file("PBUF2.PRE")?),
            video,
            palette,
            scale: 65536,
            speed: 16,
            next_row: 0,
            rng: 1,
            frame: 0,
            start_music: None,
            terrain: false,
            terrain_frame: 0,
            colors,
            heights,
            terrain_palette: data.file("MAP.PAL")?,
            sine: (0..1024)
                .map(|i| signed(&memory, 0x1a8a0 + i * 2))
                .collect(),
            projection: [0; 512],
            background: vec![0; 65536],
            output: vec![0; 65536],
            level: 0,
            previous_music: 0,
            cleared: false,
            done: false,
        })
    }
    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        if self.done {
            return true;
        }
        if !self.terrain && self.speed <= 15 {
            self.terrain = true;
            self.background[320..].copy_from_slice(&self.video[320..]);
            for i in 0..64 {
                vga.palette[(128 + i) * 3..(129 + i) * 3]
                    .copy_from_slice(&self.terrain_palette[i * 9..i * 9 + 3]);
            }
        }
        if self.terrain {
            return self.terrain_step(vga, music);
        }
        let start = *self.start_music.get_or_insert(music);
        if music == 0x7fff {
            if self.frame < 64 {
                self.speed += 16;
            }
            if self.frame > 1000 {
                self.speed -= 16;
            }
        } else {
            if (music as i32) < start as i32 + 0x400 && self.speed < 1024 {
                self.speed += 16;
            }
            if music > 0x1400 {
                self.speed -= 16;
            }
        }
        for (out, &input) in vga.palette[..512].iter_mut().zip(&self.palette[..512]) {
            *out = input & 63;
        }
        self.scale -= self.speed;
        if self.scale < 65536 {
            std::mem::swap(&mut self.source, &mut self.destination);
            self.scale += 65536;
            zoom(
                &self.source,
                &mut self.video,
                self.scale as u16,
                (self.scale >> 16) as u16,
            );
            self.next_row = 0;
            perturb(&mut self.source, &mut self.rng);
            self.scale -= self.speed;
        } else {
            zoom(
                &self.source,
                &mut self.video,
                self.scale as u16,
                (self.scale >> 16) as u16,
            );
            for _ in 0..2 {
                expand(&self.source, &mut self.destination, self.next_row);
                self.next_row = (self.next_row + 1).min(100);
                random(&mut self.rng);
            }
        }
        self.frame += 1;
        vga.packed(&self.video, 320, 200);
        false
    }
    fn terrain_step(&mut self, vga: &mut Vga, music: u16) -> bool {
        if self.previous_music >= 0x1d1e {
            self.done = true;
            return true;
        }
        let frame = self.terrain_frame as i32;
        let q = (self.level * 8).max(0);
        if q < 256 {
            for z in 4..512 {
                self.projection[z] = q * 256 / z as i32 + 512 - 2 * q;
            }
        }
        let radius = 70 + self.sine[(self.terrain_frame * 8) & 1023] / 12;
        let angle = (512 + self.terrain_frame * 10) & 1023;
        let wx = 100 + self.sine[(angle + 512) & 1023] * radius / 100;
        let wy = 100 + self.sine[(angle + 768) & 1023] * radius / 100;
        self.previous_music = music;
        let height = if music == 0x7fff {
            self.previous_music = 0;
            if frame < 251 {
                if frame < 64 {
                    if self.level < 64 {
                        self.level = frame;
                    }
                    3 * radius + 470 - 5 * frame
                } else {
                    3 * radius + 150
                }
            } else {
                if self.level < 1 {
                    self.previous_music = 0x7fff;
                } else {
                    self.level = 64 - (frame - 250);
                }
                3 * radius + 150 + (frame - 250) * 10
            }
        } else {
            if music < 0x1b00 {
                if self.level < 64 {
                    self.level += 1;
                }
            } else if music > 0x1b0e && self.level > -200 {
                let progress = music.wrapping_sub(0x1b00);
                let amount = (progress & 63) as i32 + ((progress & 0x300) >> 2) as i32 - 14;
                self.level = self.level.min(64 - amount * 2 / 3);
            }
            3 * radius + 470 - 5 * self.level
        };
        if frame > 80 && frame < 131 && self.level > 63 && !self.cleared {
            for y in 46..146 {
                self.background[y * 320 + 96..y * 320 + 224].fill(129);
            }
            self.cleared = true;
        }
        let camera = [
            wx,
            wy,
            height,
            self.sine[angle],
            self.sine[(angle + 256) & 1023],
        ];
        for column in (0..320).step_by(2) {
            ray(
                &self.colors,
                &self.heights,
                &self.background,
                &mut self.output,
                column,
                camera,
                &self.projection,
            );
        }
        self.video[320..64000].copy_from_slice(&self.output[320..64000]);
        self.terrain_frame += 1;
        vga.packed(&self.video, 320, 200);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn original_music_threshold_control_schedule() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        vga.packed(&data.file("PSTART.U").unwrap(), 320, 200);
        let mut scene = Field::new(&data, &mut vga).unwrap();
        for n in 0..127 {
            assert!(!scene.step(&mut vga, if n < 63 { 0x1000 } else { 0x1500 }));
        }
        assert_eq!(scene.frame, 127);
        assert_eq!(scene.speed, 0);
        for n in 0..103 {
            let position = if n < 100 {
                0x1a00
            } else {
                [0x1b0f, 0x1c00, 0x1d1e][n - 100]
            };
            assert!(!scene.step(&mut vga, position));
            let level = if n < 100 {
                (n as i32 + 1).min(64)
            } else {
                [64, 31, -32][n - 100]
            };
            assert_eq!(scene.level, level);
            assert_eq!(scene.cleared, n >= 81);
        }
        assert!(scene.step(&mut vga, 0x1d1e));
        assert_eq!(scene.terrain_frame, 103);
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn all_original_fallback_field_and_terrain_frames() {
        let data = crate::test_support::data();
        let mut vga = Vga::new();
        vga.packed(&data.file("PSTART.U").unwrap(), 320, 200);
        let mut scene = Field::new(&data, &mut vga).unwrap();
        let palette_text =
            crate::test_support::fixture_text("testdata/567/field-terrain-palette-sha256.txt");
        let mut palettes = palette_text.lines();
        for (frame, line) in
            crate::test_support::fixture_text("testdata/567/field-terrain-sha256.txt")
                .lines()
                .enumerate()
        {
            assert!(!scene.step(&mut vga, 0x7fff));
            assert_eq!(
                format!("{:x}", Sha256::digest(&vga.palette)),
                palettes.next().unwrap(),
                "palette frame"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&vga.indices)),
                line,
                "field/terrain frame {frame}"
            );
        }
        assert!(scene.step(&mut vga, 0x7fff));
        assert_eq!(scene.frame, 1066);
        assert_eq!(scene.terrain_frame, 316);
    }
}
