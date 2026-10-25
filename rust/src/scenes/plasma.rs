//! Part 4 uses a fixed index field, a moving palette, and per-line CRTC strides.
use crate::{
    data::{Data, words},
    vga::Vga,
};

fn tables(sine: &[i32]) -> Vec<u8> {
    let psini = (0..1024)
        .map(|i| (sine[((i * 1024) / 224) % 1024] / 33 + 31) as u8)
        .collect::<Vec<_>>();
    (0..64)
        .flat_map(|level| {
            psini
                .iter()
                .map(move |&v| (v as i8 as i32 * level / 63) as u8)
        })
        .collect()
}
fn field(sine: &[i32]) -> Vec<u8> {
    let mut field = vec![0; 65536];
    for x in 0..200 {
        let mut index = ((sine[(x * 51 / 10) % 1024] / 10 + 200) % 224 + 32) as u8;
        for row in 0..223 {
            field[(0x3840 + x + row * 200) & 65535] = index;
            index = index.wrapping_add(1);
            if index == 0 {
                index = 32;
            }
        }
    }
    field
}
/// The CRTC start address of each of the 400 rasters `render` scans: one
/// source row per physical raster (CRTC09=0), 400 visible lines. A raster's
/// OUT13 change takes effect on the next row address; the hardware status
/// polling is represented at this scanline boundary. Animated, each of the
/// first 225 rasters advances the address by `(sift[add1 + y] + sift[add2 +
/// y] + 100) mod 256` words, the 226th by 510 bytes; otherwise, and below,
/// by 80. The raster after `line_compare` starts again at address 0, where
/// the logo is.
pub fn raster_starts(
    sift: &[u8],
    add1: usize,
    add2: usize,
    line_compare: usize,
    animated: bool,
) -> [u16; 400] {
    let mut starts = [0; 400];
    let mut address = 0x3840usize;
    for (y, start) in starts.iter_mut().enumerate() {
        if y == line_compare + 1 {
            address = 0;
        }
        *start = address as u16;
        let stride = if animated && y < 225 {
            sift[add1 + y]
                .wrapping_add(sift[add2 + y])
                .wrapping_add(100) as usize
                * 2
        } else if animated && y == 225 {
            510
        } else {
            80
        };
        address = (address + stride) & 65535;
    }
    starts
}

/// Part 4's counters, for the HD preview (hd/plasma.rs). Reading them
/// changes nothing.
#[cfg(feature = "hires")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HdPlasma {
    /// 0 waiting for 29:00, 1 the logo fading from white, 2 the plasma,
    /// 3 the logo sliding away.
    pub phase: u8,
    /// Steps of the fade from white so far (0 to 64).
    pub fade_frame: usize,
    /// The palette motion's angle (1024ths of a turn), the plasma's level
    /// (0 to 63) and the counter it is taken from.
    pub a: usize,
    pub fade: i32,
    pub counter: i32,
    /// The two offsets into the stride table.
    pub add1: usize,
    pub add2: usize,
    /// Where each colour channel reads the bank: the level's row (`fade <<
    /// 10`) plus the palette's rotation, 0 to 223.
    pub pointers: [usize; 3],
    /// The raster after which the logo starts.
    pub line_compare: usize,
}

/// Part 4's tables, for the HD preview: all fixed from the start.
#[cfg(feature = "hires")]
pub struct HdPlasmaTables {
    /// The sine table (amplitude 1023, 1024 a turn).
    pub sine: Vec<i32>,
    /// The stride table (`sift`), 5001 bytes.
    pub sift: Vec<u8>,
    /// The bank: 64 levels of the palette's sine, 1024 entries each.
    pub bank: Vec<u8>,
    /// The index field as video memory holds it from 0x3840, 200 bytes a
    /// row, 223 rows (zero elsewhere).
    pub field: Vec<u8>,
    /// The logo, 320 x 180 indices, and its palette (768 bytes).
    pub logo: Vec<u8>,
    pub palette: Vec<u8>,
}

#[cfg(feature = "hires")]
impl Plasma {
    /// Read-only counters for the HD preview.
    pub fn hd_state(&self) -> HdPlasma {
        HdPlasma {
            phase: self.phase,
            fade_frame: self.fade_frame,
            a: self.a,
            fade: self.fade,
            counter: self.counter,
            add1: self.add1,
            add2: self.add2,
            pointers: self.pointers,
            line_compare: self.line_compare,
        }
    }

    /// The scene's tables.
    pub fn hd_tables(&self) -> HdPlasmaTables {
        HdPlasmaTables {
            sine: self.sine.clone(),
            sift: self.sift.clone(),
            bank: self.bank.clone(),
            field: self.field.clone(),
            logo: self.logo.clone(),
            palette: self.original_palette.clone(),
        }
    }
}

pub struct Plasma {
    sine: Vec<i32>,
    sift: Vec<u8>,
    bank: Vec<u8>,
    field: Vec<u8>,
    logo: Vec<u8>,
    palette: Vec<u8>,
    original_palette: Vec<u8>,
    phase: u8,
    fade_frame: usize,
    a: usize,
    fade: i32,
    counter: i32,
    add1: usize,
    add2: usize,
    pointers: [usize; 3],
    line_compare: usize,
}
impl Plasma {
    pub fn new(data: &Data, _vga: &mut Vga) -> Result<Self, String> {
        let m = data.memory(4)?;
        let sine = words(&m, 0x23cab, 1024);
        Ok(Self {
            bank: tables(&sine),
            field: field(&sine),
            sine,
            sift: m[0x22922..0x22922 + 5001].to_vec(),
            logo: m[0x12890..0x12890 + 57600].to_vec(),
            palette: m[0x244c4..0x244c4 + 771].to_vec(),
            original_palette: m[0x244c4..0x244c4 + 768].to_vec(),
            phase: 0,
            fade_frame: 0,
            a: 0,
            fade: 0,
            counter: 0,
            add1: 0,
            add2: 1,
            pointers: [0; 3],
            line_compare: 236,
        })
    }
    fn pointers(&mut self) {
        let total = (self.sine[(self.a * 3) % 1024] + self.sine[self.a]) as i16 as i32;
        let v = (total / 3 + 1000) as i16 as i32;
        self.pointers = [185, 215, 1]
            .map(|phase| ((phase + v + 5000) % 224 + (self.fade << 10)) as u16 as usize);
        self.a = (self.a + 1) % 1024;
    }
    fn render(&self, vga: &mut Vga, animated: bool) {
        vga.width = 320;
        vga.height = 400;
        vga.indices.resize(128000, 0);
        let starts = raster_starts(
            &self.sift,
            self.add1,
            self.add2,
            self.line_compare,
            animated,
        );
        for (y, &address) in starts.iter().enumerate() {
            for x in 0..320 {
                vga.indices[y * 320 + x] = vga.planes[x & 3][(address as usize + (x >> 2)) & 65535];
            }
        }
    }
    pub fn step(&mut self, vga: &mut Vga, music: u16) -> bool {
        match self.phase {
            0 => {
                if music < 0x2900 {
                    return false;
                }
                for p in &mut vga.planes {
                    p.fill(0);
                }
                for (i, &v) in self.logo.iter().enumerate() {
                    vga.planes[i & 3][i >> 2] = v;
                }
                self.phase = 1;
            }
            1 => {
                for i in 3..60 {
                    vga.palette[i] = (63 - self.fade_frame
                        + self.original_palette[i] as usize * self.fade_frame / 63)
                        as u8;
                }
                self.render(vga, false);
                self.fade_frame += 1;
                if self.fade_frame == 64 {
                    for p in &mut vga.planes {
                        for (i, &v) in self.field.iter().enumerate() {
                            if v != 0 {
                                p[i] = v;
                            }
                        }
                    }
                    self.pointers();
                    self.phase = 2;
                }
            }
            2 => {
                vga.palette.copy_from_slice(&self.palette[..768]);
                self.render(vga, true);
                for c in 0..3 {
                    for i in 0..225 {
                        self.palette[96 + i * 3 + c] = self.bank[(self.pointers[c] + i) & 65535];
                    }
                }
                if music == 0x7fff {
                    self.counter += 1;
                    if self.counter > 100 && self.counter < 1400 {
                        self.fade = (self.fade + 1).min(63);
                    } else if self.counter > 1750 {
                        if self.fade == 0 {
                            self.phase = 3;
                        } else {
                            self.fade -= 1;
                        }
                    }
                } else {
                    if music > 0x2a00 && music < 0x2d00 && self.counter < 255 {
                        self.counter += 1;
                    } else if music > 0x2e14 && self.counter > 0 {
                        self.counter -= 1;
                    }
                    self.fade = self.counter / 4;
                    if music >= 0x2f00 {
                        self.phase = 3;
                    }
                }
                self.pointers();
                self.add1 = (self.add1 + 2) & 4095;
                self.add2 = (self.add2 + 4094) & 4095;
                if self.phase == 3 {
                    self.line_compare = 234;
                }
            }
            _ => {
                self.render(vga, false);
                self.line_compare += 2;
                if self.line_compare > 400 {
                    return true;
                }
            }
        }
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn hash(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and PANIC_FIXTURES; run with --ignored"]
    fn original_field_bank_and_all_palette_phases() {
        let data = crate::test_support::data();
        let mut v = Vga::new();
        let mut p = Plasma::new(&data, &mut v).unwrap();
        let expected_text = crate::test_support::fixture_text("tests/fixtures/plasma-hashes.txt");
        let expected = expected_text.lines().collect::<Vec<_>>();
        assert_eq!(hash(&p.field), expected[0]);
        assert_eq!(hash(&p.bank), expected[1]);
        let mut pointers = Vec::new();
        let mut palettes = Sha256::new();
        for a in 0..1024 {
            p.a = a;
            p.fade = (a % 64) as i32;
            p.pointers();
            for ptr in p.pointers {
                pointers.extend((ptr as u16).to_le_bytes());
            }
            for c in 0..3 {
                for i in 0..225 {
                    p.palette[96 + i * 3 + c] = p.bank[(p.pointers[c] + i) & 65535];
                }
            }
            palettes.update(&p.palette);
        }
        assert_eq!(hash(&pointers), expected[2]);
        assert_eq!(format!("{:x}", palettes.finalize()), expected[3]);
    }
}
