//! Part 1: original delta streams, ASTARS, door DAC updates and logo fade.
//! One step is one chosen 70 Hz IRQ tick; stars run on alternate ticks.
use crate::{data::Data, vga::Vga};

const CUES: [usize; 10] = [58, 71, 80, 91, 103, 113, 140, 152, 165, 180];

#[derive(Clone, Copy, Default)]
struct Star {
    x: i16,
    y: i16,
    z: u16,
    px: u16,
    py: u16,
    old: u16,
    read: u8,
    write: u8,
    bg: u8,
}

pub struct Intro {
    memory: Vec<u8>,
    stars: [Star; 256],
    rng: (u16, u16),
    phase: u8,
    tick: usize,
    irq: usize,
    frame: usize,
    stream: usize,
    threshold: u8,
    door: usize,
    cue: Option<usize>,
}

/// Read-only presentation state for the experimental renderer. These are the
/// reconstruction's timeline values, not a new claim about DOS hardware timing.
#[cfg(feature = "hires")]
#[derive(Clone, Copy, Debug)]
pub struct Presentation {
    pub phase: u8,
    pub frame: usize,
    pub tick: usize,
    pub door: usize,
}

impl Intro {
    #[cfg(feature = "hires")]
    pub fn presentation(&self) -> Presentation {
        Presentation {
            phase: self.phase,
            frame: self.frame,
            tick: self.tick,
            door: self.door,
        }
    }

    pub fn new(data: &Data, vga: &mut Vga) -> Result<Self, String> {
        let mut memory = data.memory(1)?;
        for base in [0x12ce0, 0x42840] {
            for i in 1..32 {
                memory[base + 3 * i..base + 3 * i + 3].copy_from_slice(&[
                    (32 - i) as u8,
                    (32 - i) as u8,
                    (64 - 2 * i) as u8,
                ]);
            }
        }
        let mut stars = [Star::default(); 256];
        let mut seed = u32::from_le_bytes(memory[0x67ec6..0x67eca].try_into().unwrap());
        let mut rand = || {
            seed = seed.wrapping_mul(0x015a4e35).wrapping_add(1);
            ((seed >> 16) & 0x7fff) as i32
        };
        for star in &mut stars {
            star.x = ((rand() - 16384) * 2) as i16;
            star.y = ((rand() - 16384) * 2) as i16;
            star.z = (rand() % 256 + 255) as u16;
        }
        for plane in &mut vga.planes {
            plane.fill(0);
        }
        // Main preloads the logo into an offscreen page before the animation.
        for i in 0..64000 {
            vga.planes[i & 3][0x4000 + (i >> 2)] = memory[0x58260 + i];
        }
        vga.palette.copy_from_slice(&memory[0x12ce0..0x12fe0]);
        vga.mode_x(0, 80, 320, 200);
        Ok(Self {
            memory,
            stars,
            rng: (0x98dc, 0x5a46),
            phase: 0,
            tick: 0,
            irq: 0,
            frame: 0,
            stream: 0x12fe0,
            threshold: 228,
            door: 200,
            cue: None,
        })
    }

    /// A one-step pulse for the original INT60/AX=10 music cue.
    pub fn cue_frame(&self) -> Option<usize> {
        self.cue
    }

    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        self.cue = None;
        if self.phase < 4 {
            self.irq += 1;
            if self.irq & 1 != 0 {
                star_step(
                    &mut self.stars,
                    &mut self.rng,
                    vga,
                    &self.memory[0x124bc..0x12584],
                    self.threshold,
                );
            }
        }
        self.tick += 1;
        match self.phase {
            0 => {
                if self.tick == 230 {
                    self.phase = 1;
                    self.tick = 0;
                }
            }
            1 => {
                if self.tick == 2 {
                    if CUES.contains(&self.frame) {
                        self.cue = Some(self.frame);
                    }
                    self.stream = decode(&self.memory, self.stream, vga);
                    self.frame += 1;
                    self.tick = 0;
                    if self.frame == 193 {
                        self.phase = 2;
                        if music >= 0x011c {
                            self.start_doors(vga);
                        }
                    }
                }
            }
            2 => {
                if music >= 0x011c {
                    self.start_doors(vga);
                }
            }
            3 => {
                if self.frame >= 108 {
                    self.door -= 2;
                    self.threshold -= 1;
                    doors(&mut vga.palette, self.door);
                }
                if self.tick == 2 {
                    self.stream = decode(&self.memory, self.stream, vga);
                    self.frame += 1;
                    self.tick = 0;
                    if self.frame == 139 {
                        self.phase = 4;
                        self.threshold = 0;
                        vga.palette.copy_from_slice(&self.memory[0x57f60..0x58260]);
                        for c in 16 * 3..64 * 3 {
                            vga.palette[c] = 63;
                        }
                        for i in 0..64000 {
                            vga.planes[i & 3][i >> 2] = self.memory[0x58260 + i];
                        }
                    }
                }
            }
            4 => {
                let amount = self.tick - 1;
                for c in 16 * 3..64 * 3 {
                    vga.palette[c] =
                        (63 - amount + self.memory[0x57f60 + c] as usize * amount / 63) as u8;
                }
                if self.tick == 64 {
                    self.phase = 5;
                }
            }
            _ => return true,
        }
        vga.mode_x(0, 80, 320, 200);
        self.phase == 5
    }

    fn start_doors(&mut self, vga: &mut Vga) {
        self.phase = 3;
        self.tick = 0;
        self.frame = 0;
        self.stream = 0x42b40;
        self.threshold -= 2;
        vga.palette.copy_from_slice(&self.memory[0x42840..0x42b40]);
    }
}

fn decode(memory: &[u8], mut cursor: usize, vga: &mut Vga) -> usize {
    let mut pixel = 0;
    loop {
        let count = memory[cursor];
        cursor += 1;
        match count {
            0 => return (cursor + 15) & !15,
            128..=255 => pixel += 256 - count as usize,
            _ => {
                let color = memory[cursor];
                cursor += 1;
                for i in pixel..pixel + count as usize {
                    vga.planes[i & 3][i >> 2] = color;
                }
                pixel += count as usize;
            }
        }
    }
}

fn random(state: &mut (u16, u16)) -> u16 {
    let mut value = state.0.wrapping_add(state.1);
    for (i, rotate) in [2, 1, 2, 3, 1].into_iter().enumerate() {
        value = value.rotate_left(rotate);
        if i < 4 {
            value = value.wrapping_add(state.1);
        }
    }
    value = value.wrapping_add(0x73b6);
    *state = (state.1, value);
    value
}

fn star_step(
    stars: &mut [Star; 256],
    rng: &mut (u16, u16),
    vga: &mut Vga,
    background: &[u8],
    threshold: u8,
) {
    for s in stars {
        if vga.planes[s.read as usize][s.old as usize] <= threshold {
            for p in 0..4 {
                if s.write & (1 << p) != 0 {
                    vga.planes[p][s.old as usize] = s.bg;
                }
            }
        }
        let previous = s.z;
        s.z = s.z.wrapping_sub(2);
        let mut reseed = previous <= 2;
        if !reseed && s.z <= 255 {
            let py = (s.y as i32 / s.z as i32 + 100) as u16;
            if py >= 200 {
                reseed = true;
            } else {
                s.py = py;
                s.bg = background[py as usize];
                let px = (s.x as i32 / s.z as i32 + 160) as u16;
                if px >= 320 {
                    reseed = true;
                } else {
                    s.px = px;
                    let pixel = py as usize * 320 + px as usize;
                    s.old = (pixel >> 2) as u16;
                    s.read = (pixel & 3) as u8;
                    s.write = 1 << s.read;
                    if vga.planes[s.read as usize][s.old as usize] <= threshold {
                        vga.planes[s.read as usize][s.old as usize] = (s.z >> 3) as u8;
                    }
                }
            }
        }
        if reseed {
            s.x = random(rng) as i16;
            s.y = random(rng) as i16;
            s.z = 255;
        }
    }
}

fn doors(palette: &mut [u8], amount: usize) {
    let mut i = amount / 2;
    while i < amount * 8 / 14 {
        palette[(128 + i) * 3..(129 + i) * 3].fill(26);
        i += 1;
    }
    palette[(128 + i) * 3..(129 + i) * 3].fill(40);
    i += 1;
    let mut gradient = 0;
    while i <= 100 {
        let level = 45 - ((gradient / 5 + (gradient & 1)) / 2);
        palette[(128 + i) * 3..(129 + i) * 3].fill(level as u8);
        i += 1;
        gradient += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn data() -> Data {
        crate::test_support::data()
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn all_original_delta_frames() {
        let memory = data().memory(1).unwrap();
        let fixture = crate::test_support::fixture("tests/fixtures/intro-frames.sha256");
        let mut index = 0;
        for (start, count) in [(0x12fe0, 195), (0x42b40, 141)] {
            let mut vga = Vga::new();
            let mut cursor = start;
            for _ in 0..count {
                cursor = decode(&memory, cursor, &mut vga);
                vga.mode_x(0, 80, 320, 200);
                assert_eq!(
                    &Sha256::digest(&vga.indices)[..],
                    &fixture[index * 40..index * 40 + 32]
                );
                assert_eq!(
                    cursor as u64,
                    u64::from_le_bytes(
                        fixture[index * 40 + 32..index * 40 + 40]
                            .try_into()
                            .unwrap()
                    )
                );
                index += 1;
            }
        }
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn deterministic_full_intro() {
        let mut vga = Vga::new();
        let mut scene = Intro::new(&data(), &mut vga).unwrap();
        let fixture = crate::test_support::fixture("tests/fixtures/intro-full.sha256");
        let mut ticks = 0;
        let mut cues = Vec::new();
        loop {
            let done = scene.step(&mut vga, 0x011c);
            if let Some(cue) = scene.cue_frame() {
                cues.push(cue);
            }
            let mut digest = Sha256::new();
            for plane in &vga.planes {
                digest.update(plane);
            }
            digest.update(&vga.palette);
            assert_eq!(
                &digest.finalize()[..],
                &fixture[ticks * 32..ticks * 32 + 32],
                "intro tick {ticks}"
            );
            ticks += 1;
            if done {
                break;
            }
        }
        assert_eq!(ticks, fixture.len() / 32);
        assert_eq!(cues, CUES);
        assert_eq!(scene.threshold, 0);
    }
}
