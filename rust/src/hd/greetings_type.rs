//! Part 7's letterforms for the modern view, derived when the preview loads
//! from the TRUG maps the user's data supplies. No glyph or font data is
//! stored in the repository.
//!
//! - Read from the bytes: glyph `g` (A = 0) starts at plane offset 0x3e90 +
//!   g * 0x700, 16 rows of 112 bytes, each row 16 views of 7 bytes (28
//!   pixels); the scene copies rows 0 to 14. The colours 1 to 6 run from
//!   white to teal.
//! - Interpretation: the 16 views read as a square column of side about 18
//!   pixels turning 90 degrees, the same letter on every face (the faces'
//!   widths in the views follow the cosine and sine of an orthographic
//!   turn). View 0 is the column square on: its face, columns 5 to 22 of the
//!   28, is the letter as it stands on a face. The shading reads as a bevel
//!   lit from above: white (1) on the strokes' crowns, teal (6) at their
//!   edges.
//! - Authored: the letter's outline is every pixel of the front view the
//!   letter colours (1 to 6; the darkest, 5 and 6, shade the strokes' sides
//!   and insides, so leaving them out cuts notches and holes into the
//!   strokes), with enclosed specks of colour 0 of up to two pixels filled
//!   (G's broken slit; the counters, of A and O, are 8 and 14 pixels),
//!   upsampled, softened a little and cut at half, then measured as a
//!   signed distance, so its edge stays sharp at any size; the relief is
//!   that distance rounded into a bevel and tilted by the original's
//!   shading.

use half::f16;

/// Texels on a side of one letter's face, and the atlas of 8 x 4 cells
/// (26 letters).
pub const CELL: usize = 128;
pub const ATLAS_COLUMNS: usize = 8;
pub const ATLAS_SIZE: (u32, u32) = (1024, 512);
/// A face, in source rows on the 4:3 display (18 columns of 5/6 of a row,
/// 15 rows): the cube's side.
pub const FACE: f64 = 15.0;
/// Where the glyphs start in each plane, a glyph's stride, a glyph row's
/// stride (read from the code), and the front face's columns in the 28
/// (read from the data's view 0).
pub const GLYPH_BASE: usize = 0x3e90;
pub const GLYPH_STRIDE: usize = 0x700;
pub const GLYPH_ROW: usize = 112;
const FACE_FIRST_COLUMN: usize = 5;
const FACE_COLUMNS: usize = 18;
const FACE_ROWS: usize = 15;
/// Authored: the softening of the outline and of the shading, in source
/// columns; the bevel's width in rows.
const OUTLINE_SOFT: f64 = 0.22;
const SHADING_SOFT: f64 = 0.6;
const BEVEL: f64 = 0.8;

/// The letters' atlas: per texel the signed distance to the outline (rows,
/// positive outside), the relief (0 to 1), and the relief's slope across
/// and up the face (per row).
pub struct Letterforms {
    pub texels: Vec<[f16; 4]>,
}

/// One plane byte of the TRUG maps (4 x 65536, as video memory holds them).
fn byte(planes: &[u8], plane: usize, address: usize) -> u8 {
    planes[plane * 65536 + (address & 65535)]
}

/// The front view of glyph `g`: 15 rows of the face's 18 columns, colours.
pub fn front_view(planes: &[u8], g: usize) -> [[u8; FACE_COLUMNS]; FACE_ROWS] {
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            let x = FACE_FIRST_COLUMN + c;
            byte(
                planes,
                x & 3,
                GLYPH_BASE + g * GLYPH_STRIDE + r * GLYPH_ROW + x / 4,
            )
        })
    })
}

/// Enclosed specks of colour 0 up to this many pixels are filled.
const SPECK: usize = 2;

/// The outline's pixels: every pixel the letter colours (1 to 6), and the
/// enclosed specks between them.
fn outline(view: &[[u8; FACE_COLUMNS]; FACE_ROWS]) -> Vec<bool> {
    let (w, h) = (FACE_COLUMNS, FACE_ROWS);
    let inside: Vec<bool> = view.iter().flatten().map(|c| (1..=6).contains(c)).collect();
    let neighbours = |i: usize| {
        let (x, y) = (i % w, i / w);
        [
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
        ]
        .into_iter()
        .flatten()
    };
    // Each region of colour 0, four-connected: filled when it is small and
    // does not reach the border.
    let mut filled = inside.clone();
    let mut seen = vec![false; w * h];
    for start in 0..w * h {
        if inside[start] || seen[start] {
            continue;
        }
        let mut region = Vec::new();
        let mut open = false;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            region.push(i);
            let (x, y) = (i % w, i / w);
            open |= x == 0 || y == 0 || x == w - 1 || y == h - 1;
            for j in neighbours(i) {
                if !inside[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if !open && region.len() <= SPECK {
            for i in region {
                filled[i] = true;
            }
        }
    }
    filled
}

/// A source image (`w` x `h`, row major) sampled bilinearly at texel
/// centres over the face, zero outside.
fn upsample(src: &[f64], w: usize, h: usize) -> Vec<f64> {
    let at = |x: isize, y: isize| -> f64 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            0.0
        } else {
            src[y as usize * w + x as usize]
        }
    };
    (0..CELL * CELL)
        .map(|i| {
            let (tx, ty) = ((i % CELL) as f64, (i / CELL) as f64);
            let x = (tx + 0.5) / CELL as f64 * w as f64 - 0.5;
            let y = (ty + 0.5) / CELL as f64 * h as f64 - 0.5;
            let (x0, y0) = (x.floor(), y.floor());
            let (fx, fy) = (x - x0, y - y0);
            let (x0, y0) = (x0 as isize, y0 as isize);
            at(x0, y0) * (1.0 - fx) * (1.0 - fy)
                + at(x0 + 1, y0) * fx * (1.0 - fy)
                + at(x0, y0 + 1) * (1.0 - fx) * fy
                + at(x0 + 1, y0 + 1) * fx * fy
        })
        .collect()
}

/// A Gaussian blur of a CELL x CELL image, `sigma` in texels, zero outside.
fn blur(img: &[f64], sigma: f64) -> Vec<f64> {
    let radius = (sigma * 3.0).ceil() as isize;
    let kernel: Vec<f64> = (-radius..=radius)
        .map(|k| (-(k * k) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let total: f64 = kernel.iter().sum();
    let pass = |src: &[f64], horizontal: bool| -> Vec<f64> {
        (0..CELL * CELL)
            .map(|i| {
                let (x, y) = ((i % CELL) as isize, (i / CELL) as isize);
                let mut sum = 0.0;
                for (k, w) in (-radius..=radius).zip(&kernel) {
                    let (sx, sy) = if horizontal { (x + k, y) } else { (x, y + k) };
                    if sx >= 0 && sy >= 0 && sx < CELL as isize && sy < CELL as isize {
                        sum += w * src[sy as usize * CELL + sx as usize];
                    }
                }
                sum / total
            })
            .collect()
    };
    pass(&pass(img, true), false)
}

/// Squared distance transform along one line (Felzenszwalb and
/// Huttenlocher): `f` holds 0 at features and a large value elsewhere.
fn edt_line(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0; n + 1];
    let mut k = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let meet = |q: usize, p: usize| {
        ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * (q - p) as f64)
    };
    for q in 1..n {
        // z[0] is minus infinity, so k never passes below 0.
        let mut s = meet(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = meet(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f64 - p as f64).powi(2) + f[p];
    }
    d
}

/// Distance in texels from each texel to the nearest `true` texel.
fn distance_to(mask: &[bool]) -> Vec<f64> {
    const FAR: f64 = 1e12;
    let mut grid: Vec<f64> = mask.iter().map(|&m| if m { 0.0 } else { FAR }).collect();
    for y in 0..CELL {
        let row = edt_line(&grid[y * CELL..(y + 1) * CELL]);
        grid[y * CELL..(y + 1) * CELL].copy_from_slice(&row);
    }
    for x in 0..CELL {
        let column: Vec<f64> = (0..CELL).map(|y| grid[y * CELL + x]).collect();
        for (y, v) in edt_line(&column).into_iter().enumerate() {
            grid[y * CELL + x] = v;
        }
    }
    grid.into_iter().map(f64::sqrt).collect()
}

/// One letter's face: signed distance (rows), relief and its slopes.
pub fn letter(planes: &[u8], g: usize) -> Vec<[f64; 4]> {
    let view = front_view(planes, g);
    let mask = outline(&view);
    let (w, h) = (FACE_COLUMNS, FACE_ROWS);
    let coverage: Vec<f64> = mask.iter().map(|&m| m as u8 as f64).collect();
    let texels_per_column = CELL as f64 / w as f64;
    let soft = blur(&upsample(&coverage, w, h), OUTLINE_SOFT * texels_per_column);
    let inside: Vec<bool> = soft.iter().map(|&v| v > 0.5).collect();
    let outside: Vec<bool> = inside.iter().map(|&v| !v).collect();
    let row_per_texel = FACE / CELL as f64;
    let to_inside = distance_to(&inside);
    let to_outside = distance_to(&outside);
    let sd: Vec<f64> = (0..CELL * CELL)
        .map(|i| ((to_inside[i] - to_outside[i]) * row_per_texel).clamp(-8.0, 8.0))
        .collect();
    // The original's shading as height: white 1 high, teal 6 low.
    let shading: Vec<f64> = (0..w * h)
        .map(|i| {
            let c = view[i / w][i % w];
            if c == 0 || c > 6 {
                0.0
            } else {
                (7.0 - c as f64) / 6.0
            }
        })
        .collect();
    let shading = blur(&upsample(&shading, w, h), SHADING_SOFT * texels_per_column);
    let top = shading.iter().cloned().fold(1e-6, f64::max);
    let relief: Vec<f64> = (0..CELL * CELL)
        .map(|i| (-sd[i] / BEVEL).clamp(0.0, 1.0).powf(0.6) * (0.75 + 0.25 * shading[i] / top))
        .collect();
    let relief = blur(&relief, 0.12 * texels_per_column);
    (0..CELL * CELL)
        .map(|i| {
            let (x, y) = (i % CELL, i / CELL);
            let at = |x: usize, y: usize| relief[y.min(CELL - 1) * CELL + x.min(CELL - 1)];
            let across = (at(x + 1, y) - at(x.saturating_sub(1), y)) / (2.0 * row_per_texel);
            // Up the face is up the texture's rows backwards.
            let up = (at(x, y.saturating_sub(1)) - at(x, y + 1)) / (2.0 * row_per_texel);
            [sd[i], relief[i], across, up]
        })
        .collect()
}

/// All 26 letters in the atlas.
pub fn letterforms(planes: &[u8]) -> Letterforms {
    let (width, height) = (ATLAS_SIZE.0 as usize, ATLAS_SIZE.1 as usize);
    let mut texels = vec![[f16::from_f32(8.0), f16::ZERO, f16::ZERO, f16::ZERO]; width * height];
    for g in 0..26 {
        let face = letter(planes, g);
        let (cx, cy) = ((g % ATLAS_COLUMNS) * CELL, (g / ATLAS_COLUMNS) * CELL);
        for (i, t) in face.iter().enumerate() {
            let (x, y) = (cx + i % CELL, cy + i / CELL);
            texels[y * width + x] = t.map(f16::from_f64);
        }
    }
    Letterforms { texels }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic planes holding one glyph: a filled square with a hole.
    fn planes_with(g: usize, pixel: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        let mut planes = vec![0; 4 * 65536];
        for r in 0..FACE_ROWS {
            for x in 0..28 {
                let address = GLYPH_BASE + g * GLYPH_STRIDE + r * GLYPH_ROW + x / 4;
                planes[(x & 3) * 65536 + address] = pixel(x, r);
            }
        }
        planes
    }

    #[test]
    fn shading_stays_inside_and_counters_stay_open() {
        // A block of colour 2 over columns 7..=20, rows 2..=12, with a
        // one-pixel speck of colour 6, one of colour 0 and a 3 x 3 counter
        // of colour 0.
        let planes = planes_with(3, |x, r| {
            if !(7..=20).contains(&x) || !(2..=12).contains(&r) {
                0
            } else if x == 9 && r == 4 {
                6
            } else if x == 9 && r == 10 {
                0
            } else if (14..=16).contains(&x) && (6..=8).contains(&r) {
                0
            } else {
                2
            }
        });
        let face = letter(&planes, 3);
        // Texel at a face position (column of the 18, row of the 15).
        let at = |c: f64, r: f64| {
            let x = (c / 18.0 * CELL as f64) as usize;
            let y = (r / 15.0 * CELL as f64) as usize;
            face[y * CELL + x]
        };
        assert!(at(9.0 - 5.0 + 0.5, 4.5)[0] < 0.0, "shading inside");
        assert!(at(9.0 - 5.0 + 0.5, 10.5)[0] < 0.0, "speck filled");
        assert!(at(15.0 - 5.0 + 0.5, 7.5)[0] > 0.0, "counter kept");
        assert!(at(10.5 - 5.0, 10.0)[0] < -0.4, "inside");
        assert!(at(0.5, 0.5)[0] > 1.0, "outside");
        assert!(at(10.5 - 5.0, 10.0)[1] > 0.5, "relief inside");
    }

    #[test]
    fn distances_are_euclidean() {
        let mut mask = vec![false; CELL * CELL];
        mask[64 * CELL + 64] = true;
        let d = distance_to(&mask);
        assert_eq!(d[64 * CELL + 64], 0.0);
        assert!((d[64 * CELL + 67] - 3.0).abs() < 1e-9);
        assert!((d[68 * CELL + 67] - 5.0).abs() < 1e-9);
    }
}
