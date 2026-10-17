//! Part 3: inherited-image zoom, overlap-count circles, then 512 morphing dots.
//! Arithmetic mirrors src/semantic_34.py and its original-instruction probes.
use crate::{
    data::{Data, word, words},
    vga::Vga,
};

const ORDER: [usize; 8] = [0, 7, 1, 3, 4, 2, 5, 6];
const SCRIPT: [(i32, i32); 9] = [
    (500, 0),
    (300, 2500),
    (300, 4000),
    (300, 5500),
    (300, 7000),
    (300, 8500),
    (300, 10000),
    (360, 11500),
    (0, 0),
];
fn s16(v: i32) -> i32 {
    v as i16 as i32
}
#[derive(Clone, Copy, Default)]
struct Point {
    xyz: [i32; 3],
    delta: [i32; 3],
}
fn high(objects: &mut [Vec<Point>], obj: usize, index: usize, xyz: [i32; 3]) {
    objects[obj][index].xyz = xyz.map(|v| s16(v) << 16);
}
// Point indices deliberately address several original parallel arrays.
#[allow(clippy::needless_range_loop)]
fn objects(s: &[i32], profile: &[i32]) -> Vec<Vec<Point>> {
    let mut out = vec![vec![Point::default(); 520]; 8];
    for i in 0..512 {
        high(
            &mut out,
            2,
            i,
            [1, 8, 64].map(|scale| 2 * ((i / scale) % 8) as i32 * 30 - 210),
        );
        let a = 2 * ((i / 2) as i32 - 128);
        let b = 2 * (((i / 2) % 3) as i32 * 85 - 85);
        high(
            &mut out,
            3,
            i,
            if i & 1 != 0 { [a, b, 0] } else { [b, a, 0] },
        );
        let lat = (i % 23) * 23;
        let lon = (i / 22) * 45;
        let r = s[lat] >> 8;
        high(
            &mut out,
            0,
            i,
            [
                (s16((s[lon] >> 8) * r) >> 7) << 1,
                (s16((s[lon + 256] >> 8) * r) >> 7) << 1,
                (s[lat + 256] >> 8) << 1,
            ],
        );
    }
    for i in 0..42 {
        let q = i as i32 * 6 - 123;
        let xyz = [
            [-123, -123, q],
            [123, -123, q],
            [-123, 123, q],
            [123, 123, q],
            [-123, q, -123],
            [-123, q, 123],
            [123, q, -123],
            [123, q, 123],
            [q, 123, 123],
            [q, -123, 123],
            [q, 123, -123],
            [q, -123, -123],
        ];
        for (edge, p) in xyz.iter().enumerate() {
            high(&mut out, 1, edge * 42 + i, *p);
        }
    }
    for a in 0..4 {
        for b in 0..8 {
            for c in 0..16 {
                high(
                    &mut out,
                    4,
                    c * 32 + b * 4 + a,
                    [c as i32 * 16 - 128, b as i32 * 16 - 64, a as i32 * 8 - 16],
                );
            }
        }
    }
    let pyramid = vec![
        ([-200, -200, -200], [0, 0, 200]),
        ([-200, 200, -200], [0, 0, 200]),
        ([200, -200, -200], [0, 0, 200]),
        ([200, 200, -200], [0, 0, 200]),
        ([-200, -200, -200], [-200, 200, -200]),
        ([-200, 200, -200], [200, 200, -200]),
        ([200, 200, -200], [200, -200, -200]),
        ([200, -200, -200], [-200, -200, -200]),
    ];
    let box_edges = vec![
        ([245, 100, 50], [-245, 100, 50]),
        ([-245, 100, 50], [-245, -100, 50]),
        ([-245, -100, 50], [245, -100, 50]),
        ([245, -100, 50], [245, 100, 50]),
        ([245, 100, 50], [245, 100, -50]),
        ([-245, 100, 50], [-245, 100, -50]),
        ([245, -100, 50], [245, -100, -50]),
        ([-245, -100, 50], [-245, -100, -50]),
        ([245, 100, -50], [-245, 100, -50]),
        ([-245, 100, -50], [-245, -100, -50]),
        ([-245, -100, -50], [245, -100, -50]),
        ([245, -100, -50], [245, 100, -50]),
    ];
    for (obj, edges, count) in [(5, pyramid, 64), (6, box_edges, 42)] {
        for (edge, (start, end)) in edges.iter().enumerate() {
            for i in 0..count {
                let xyz = std::array::from_fn(|k| {
                    s16(s16(start[k] * (count - 1 - i) as i32) + s16(end[k] * i as i32))
                        / (count - 1) as i32
                });
                high(&mut out, obj, edge * count + i, xyz);
            }
        }
    }
    for a in 0..23 {
        for b in 0..21 {
            high(
                &mut out,
                7,
                a + b * 23,
                [
                    s16((s[a * 46] >> 8) * profile[b]) / 50,
                    s16((s[a * 46 + 256] >> 8) * profile[b]) / 50,
                    (b as i32 * 11 - 126) * 2,
                ],
            );
        }
    }
    for a in 0..11 {
        for (off, scale) in [(0, 35), (11, 23)] {
            high(
                &mut out,
                7,
                483 + a + off,
                [
                    s16((s[a * 92] >> 8) * scale) / 50,
                    s16((s[a * 92 + 256] >> 8) * scale) / 50,
                    210,
                ],
            );
        }
    }
    high(&mut out, 7, 511, [0, 0, 105]);
    for pair in ORDER.windows(2) {
        for i in 0..512 {
            for axis in 0..3 {
                out[pair[0]][i].delta[axis] =
                    s16((out[pair[1]][i].xyz[axis] >> 16) - (out[pair[0]][i].xyz[axis] >> 16)) << 9;
            }
        }
    }
    out
}
fn coefficients(s: &[i32], angles: [usize; 3]) -> [i32; 9] {
    let [sx, sy, sz] = angles.map(|a| s[a]);
    let [cx, cy, cz] = angles.map(|a| s[a + 256]);
    let q = |v: i32| s16(v >> 23);
    let h = |a: i32, b: i32| a.wrapping_mul(b).wrapping_add(0x4000) >> 15;
    [
        q(cy * cz),
        q(cy * sz),
        s16(-sy >> 8),
        q(h(sx, cz).wrapping_mul(sy).wrapping_sub(cx * sz)),
        q(h(sx, sy).wrapping_mul(sz).wrapping_add(cx * cz)),
        q(cy * sx),
        q(h(cx, cz).wrapping_mul(sy).wrapping_add(sx * sz)),
        q(h(cx, sy).wrapping_mul(sz).wrapping_sub(sx * cz)),
        q(cy * cx),
    ]
}
fn banks() -> [Vec<u8>; 2] {
    let mut b = std::array::from_fn(|_| vec![0; 65536]);
    for c in -128..128 {
        for x in -128..128 {
            let at = ((c << 9) + (x << 1)) as u16 as usize;
            b[usize::from(c >= 0)][at..at + 2]
                .copy_from_slice(&((s16(x * c + 32) / 64) as i16).to_le_bytes());
        }
    }
    b
}
fn palettes() -> Vec<u8> {
    let mut p = vec![0; 6144];
    for phase in 0..8 {
        for color in 0..256 {
            for (age, v) in [15, 20, 25, 30, 35, 40, 45, 60].iter().enumerate() {
                if color & (1 << ((phase + age) % 8)) != 0 {
                    p[phase * 768 + color * 3 + 2] = *v;
                }
            }
        }
    }
    p
}
pub fn zoom(source: &[u8], scale: u16) -> Vec<u8> {
    let scale = scale as usize;
    let x0 = 160usize.wrapping_sub((160 * scale) >> 16) & 65535;
    let fx0 = 0usize.wrapping_sub(160 * scale) & 65535;
    let mut y = 100usize.wrapping_sub((100 * scale) >> 16) & 65535;
    let mut fy = 0usize.wrapping_sub(100 * scale) & 65535;
    let mut out = vec![0; 64000];
    for row in 0..200 {
        let whole = (320 * y) >> 16;
        let mut at = (320 * y + x0) & 65535;
        let mut fx = fx0;
        for col in 0..320 {
            out[row * 320 + col] = source[at];
            at = (at + whole + ((fx + scale) >> 16)) & 65535;
            fx = (fx + scale) & 65535;
        }
        y = (y + whole + ((fy + scale) >> 16)) & 65535;
        fy = (fy + scale) & 65535;
    }
    out
}
const ROWS: [(usize, usize); 16] = [
    (6, 10),
    (4, 12),
    (3, 13),
    (2, 14),
    (1, 15),
    (1, 15),
    (0, 16),
    (0, 16),
    (0, 16),
    (0, 16),
    (1, 15),
    (1, 15),
    (2, 14),
    (3, 13),
    (4, 12),
    (6, 10),
];
fn circle(buffer: &mut [u8], x: i32, y: i32, delta: i8) {
    let base = (x + 320 * y) as u16 as usize;
    for (row, (l, r)) in ROWS.iter().enumerate() {
        for col in *l..*r {
            let at = (base + 320 * row + col) & 65535;
            buffer[at] = buffer[at].wrapping_add(delta as u8);
        }
    }
}

pub struct Dots {
    sine: Vec<i32>,
    objects: Vec<Vec<Point>>,
    banks: [Vec<u8>; 2],
    matrix: Vec<u8>,
    camera: Vec<[i32; 3]>,
    weights: [Vec<i32>; 4],
    palettes: Vec<u8>,
    shade_palette: Vec<u8>,
    phase: u8,
    scale: i32,
    source: Vec<u8>,
    shade: Vec<u8>,
    path: Vec<[i32; 2]>,
    shade_frame: usize,
    shade_stop: usize,
    angles: [usize; 3],
    script: usize,
    a: i32,
    d: i32,
    frame: usize,
    history: Vec<Vec<usize>>,
}
impl Dots {
    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let m = data.memory(3)?;
        let sine = words(&m, 0x63ba0, 1292);
        let objs = objects(&sine, &words(&m, 0x63b66, 23));
        let path = (0..1500)
            .map(|i| {
                [
                    150 - sine[(5 * i + 256) & 1023] / 660 - sine[(9 * i) & 1023] / 350,
                    sine[(4 * i + 150) & 1023] / 660 + sine[(7 * i) & 1023] / 700 + 90,
                ]
            })
            .collect();
        let mut source = vec![0; 65536];
        for (i, pixel) in source.iter_mut().take(64000).enumerate() {
            *pixel = vga.indices.get(i).copied().unwrap_or(0);
        }
        Ok(Self {
            sine,
            objects: objs,
            banks: banks(),
            matrix: m[0x136a0..0x236a0].to_vec(),
            camera: (0..92)
                .map(|i| std::array::from_fn(|k| crate::data::signed(&m, 0x1d858 + i * 8 + k * 2)))
                .collect(),
            weights: [0x8e8, 0x6e8, 0x4e8, 0x2e8].map(|o| words(&m, 0x1d850 + o, 256)),
            palettes: palettes(),
            shade_palette: m[0x645b7..0x648b7].to_vec(),
            phase: 0,
            scale: 65535,
            source,
            shade: vec![0; 65536],
            path,
            shade_frame: 0,
            shade_stop: 366,
            angles: [589, 860, 307],
            script: 0,
            a: 0,
            d: 0,
            frame: 0,
            history: vec![vec![0; 512]; 32],
        })
    }
    fn dot_frame(&mut self, vga: &mut Vga) -> bool {
        self.d = s16(self.d + 5);
        let deadline = SCRIPT[self.script].0;
        if deadline - 128 <= self.a {
            for point in &mut self.objects[ORDER[self.script]][..512] {
                for axis in 0..3 {
                    point.xyz[axis] = point.xyz[axis].wrapping_add(point.delta[axis]);
                }
            }
        }
        if deadline <= self.a {
            self.script += 1;
            self.a = 0;
            self.d = SCRIPT[self.script].1;
        }
        self.a = s16(self.a + 1);
        if self.frame < 2601 {
            for (a, delta) in self.angles.iter_mut().zip([3, 4, 5]) {
                *a = (*a + delta) & 1023;
            }
        }
        if self.script == 8 {
            return true;
        }
        let coeff = coefficients(&self.sine, self.angles);
        let k = (self.d as u16 as usize) >> 8;
        let t = self.d as usize & 255;
        let translation: [i32; 3] = std::array::from_fn(|axis| {
            s16((0..4).fold(0i32, |sum, j| {
                sum.wrapping_add(self.camera[k + j][axis] * self.weights[j][t])
            }) >> 15)
        });
        let page = self.frame & 31;
        let base = (page & 3) * 0x4000;
        let bit = 0x80u8.rotate_left((page >> 2) as u32);
        for &a in &self.history[page] {
            for plane in &mut vga.planes {
                plane[(base + a) & 65535] &= !bit;
            }
        }
        for (i, point) in self.objects[ORDER[self.script]][..512].iter().enumerate() {
            let indices = point.xyz.map(|v| ((v >> 16) as u16 & 0xfffe) as i32);
            let transformed: [i32; 3] = std::array::from_fn(|row| {
                s16((0..3)
                    .map(|axis| {
                        let c = coeff[row * 3 + axis];
                        let at = ((c << 9) + indices[axis]) as u16 as usize;
                        word(&self.banks[usize::from(c >= 0)], at) as i16 as i32
                    })
                    .sum::<i32>()
                    + translation[row])
            });
            let x = transformed[0] * 256 / transformed[2];
            let y = transformed[1] * 256 / transformed[2];
            let w = |at: i32| word(&self.matrix, at as u16 as usize) as usize;
            let offset = (w(0x5f8 + 2 * x) + w(0x260 + 2 * y)) & 0x3fff;
            let mask = w(0x1048 + 2 * x) >> 8;
            for p in 0..4 {
                if mask & (1 << p) != 0 {
                    vga.planes[p][(base + offset) & 65535] |= bit;
                }
            }
            self.history[page][i] = offset;
        }
        // The original publishes the preceding page and palette asynchronously.
        // This port presents each completed page once per deterministic timer step.
        let phase = page >> 2;
        vga.palette
            .copy_from_slice(&self.palettes[phase * 768..phase * 768 + 768]);
        vga.mode_x(base, 80, 320, 200);
        self.frame += 1;
        false
    }
    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        match self.phase {
            0 => {
                if music >= 0x1e00 {
                    self.phase = 1;
                }
                false
            }
            1 => {
                vga.packed(&zoom(&self.source, self.scale as u16), 320, 200);
                self.scale -= 256;
                if self.scale <= 23000 {
                    self.phase = 2;
                    vga.palette.copy_from_slice(&self.shade_palette);
                }
                false
            }
            2 => {
                let f = self.shade_frame;
                if f >= 16 && music != 0x7fff && music >= 0x1e3c {
                    self.shade_stop = self.shade_stop.min(f);
                }
                let px = (f + 1) * 4;
                let py = (f + 1) * 3;
                for i in 0..20 {
                    if f < self.shade_stop {
                        circle(
                            &mut self.shade,
                            self.path[(px + i * 5) & 1023][0],
                            self.path[(py + i * 5) & 1023][1],
                            1,
                        );
                    }
                    if f >= 16 {
                        circle(
                            &mut self.shade,
                            self.path[(px + i * 5 - 64) & 1023][0],
                            self.path[(py + i * 5 - 48) & 1023][1],
                            -1,
                        );
                    }
                }
                vga.packed(&self.shade, 320, 200);
                self.shade_frame += 1;
                if self.shade_frame >= self.shade_stop + 16 {
                    self.phase = 3;
                    for p in &mut vga.planes {
                        p.fill(0);
                    }
                }
                false
            }
            _ => self.dot_frame(vga),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn data() -> Data {
        crate::test_support::data()
    }
    fn hash(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn original_objects_tables_and_full_dot_schedule() {
        let mut v = Vga::new();
        let mut d = Dots::new(&data(), &mut v).unwrap();
        let expected_text = crate::test_support::fixture_text("tests/fixtures/dots-hashes.txt");
        let expected = expected_text.lines().collect::<Vec<_>>();
        let mut raw = Vec::new();
        for obj in &d.objects {
            for p in obj {
                for x in p.xyz.into_iter().chain(p.delta) {
                    raw.extend(x.to_le_bytes());
                }
                raw.extend([0; 8]);
            }
        }
        assert_eq!(hash(&raw), expected[0]);
        assert_eq!(hash(&d.banks.concat()), expected[1]);
        assert_eq!(hash(&d.palettes), expected[2]);
        let mut frames = Sha256::new();
        let mut count = 0;
        while !d.dot_frame(&mut v) {
            for p in &v.planes {
                frames.update(Sha256::digest(p));
            }
            count += 1;
        }
        assert_eq!(count, 2660);
        assert_eq!(format!("{:x}", frames.finalize()), expected[3]);
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn inherited_zoom_and_complete_shade_erase() {
        let data = data();
        let m = data.memory(3).unwrap();
        let s = words(&m, 0x63ba0, 1292);
        let source = (0..65536).map(|i| ((i >> 8) ^ i) as u8).collect::<Vec<_>>();
        let hashes_text = crate::test_support::fixture_text("tests/fixtures/zoom-hashes.txt");
        let hashes = hashes_text.lines().collect::<Vec<_>>();
        for (i, scale) in [0, 23000, 23455, 32768, 45000, 65000, 65535, 0x5678]
            .iter()
            .enumerate()
        {
            assert_eq!(hash(&zoom(&source, *scale)), hashes[i]);
        }
        let mut v = Vga::new();
        let mut d = Dots::new(&data, &mut v).unwrap();
        d.phase = 2;
        for _ in 0..382 {
            assert!(!d.step(&mut v, 0x7fff));
        }
        assert!(d.shade.iter().all(|&v| v == 0));
        assert_eq!(s[0], d.sine[0]);
    }
}
