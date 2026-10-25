//! Part 7's letterforms for the modern view: each letter's outline as a
//! signed distance over a face of its block, with its gradient, for the
//! crystal's cut. Behind a small interface ([`LetterSource`]): the modern
//! view uses the authored outlines in greetings_letters.txt (drawn for this
//! remake in the original's spirit, not recovered: see that file's header);
//! the original's letters, derived when the preview loads from the TRUG
//! maps the user's data supplies, remain for comparison. No glyph or font
//! data from the demo is stored in the repository.
//!
//! The original's letters, as read:
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
//!   signed distance, so its edge stays sharp at any size.

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
/// Authored: the softening of the outline, in source columns.
const OUTLINE_SOFT: f64 = 0.22;
/// The farthest the distances reach either side of the outline (rows).
const REACH: f64 = 8.0;

/// Where the letters' outlines come from.
pub enum LetterSource<'a> {
    /// The authored outlines, in greetings_letters.txt's format.
    Outlines(&'a str),
    /// The front views of TRUG's glyphs: the user's TRUG maps, 4 planes of
    /// 65536 bytes as video memory holds them (the earlier placeholder,
    /// kept for comparison).
    #[cfg_attr(not(test), allow(dead_code))]
    Trug(&'a [u8]),
}

/// The authored outlines, as the repository holds them.
pub const LETTERS: &str = include_str!("greetings_letters.txt");

/// One letter's closed contours in the file's units (y up), outer
/// counter-clockwise and holes clockwise.
pub struct Outline {
    pub letter: char,
    pub contours: Vec<Vec<(f64, f64)>>,
}

/// The letters file: its box (units) and each letter's contours.
pub struct Outlines {
    pub box_size: (f64, f64),
    pub letters: Vec<Outline>,
}

/// Read greetings_letters.txt's format (described in its header).
pub fn parse_outlines(text: &str) -> Result<Outlines, String> {
    let mut box_size = None;
    let mut letters: Vec<Outline> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let bad = || format!("greetings_letters.txt line {}: not understood", number + 1);
        match fields.as_slice() {
            ["box", w, h] => {
                box_size = Some((
                    w.parse::<f64>().map_err(|_| bad())?,
                    h.parse::<f64>().map_err(|_| bad())?,
                ))
            }
            ["advance" | "pitch", _] | ["minimum", "ink", _, "counter", _] => {}
            [key, kind @ ("outer" | "hole"), points] if key.len() == 1 => {
                let letter = key.chars().next().unwrap();
                if !letter.is_ascii_uppercase() {
                    return Err(bad());
                }
                let mut contour = Vec::new();
                for pair in points.split(' ') {
                    let (x, y) = pair.split_once(',').ok_or_else(bad)?;
                    contour.push((
                        x.parse::<f64>().map_err(|_| bad())?,
                        y.parse::<f64>().map_err(|_| bad())?,
                    ));
                }
                if contour.len() < 3 {
                    return Err(bad());
                }
                // Outer contours wind counter-clockwise, holes clockwise.
                let area: f64 = (0..contour.len())
                    .map(|i| {
                        let (a, b) = (contour[i], contour[(i + 1) % contour.len()]);
                        a.0 * b.1 - b.0 * a.1
                    })
                    .sum();
                if (area > 0.0) != (*kind == "outer") {
                    return Err(format!("{}: winds the wrong way", bad()));
                }
                match letters.iter_mut().find(|o| o.letter == letter) {
                    Some(o) => o.contours.push(contour),
                    None => letters.push(Outline {
                        letter,
                        contours: vec![contour],
                    }),
                }
            }
            _ => return Err(bad()),
        }
    }
    Ok(Outlines {
        box_size: box_size.ok_or("greetings_letters.txt has no box")?,
        letters,
    })
}

/// A letter's signed distance over a face, CELL x CELL texels (rows of the
/// face, positive outside): the letter's box fills the face, its distance
/// to the nearest edge, negative inside (an odd number of crossings).
fn outline_distances(outline: &Outline, box_size: (f64, f64)) -> Vec<f64> {
    let rows_per_unit = FACE / box_size.1;
    let edges: Vec<((f64, f64), (f64, f64))> = outline
        .contours
        .iter()
        .flat_map(|c| (0..c.len()).map(move |i| (c[i], c[(i + 1) % c.len()])))
        .collect();
    (0..CELL * CELL)
        .map(|i| {
            let (tx, ty) = ((i % CELL) as f64 + 0.5, (i / CELL) as f64 + 0.5);
            let x = tx / CELL as f64 * box_size.0;
            let y = box_size.1 - ty / CELL as f64 * box_size.1;
            let mut nearest = f64::INFINITY;
            let mut inside = false;
            for &((ax, ay), (bx, by)) in &edges {
                let (ex, ey) = (bx - ax, by - ay);
                let t = (((x - ax) * ex + (y - ay) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
                let (dx, dy) = (x - ax - ex * t, y - ay - ey * t);
                nearest = nearest.min((dx * dx + dy * dy).sqrt());
                if (ay > y) != (by > y) && x < ax + (y - ay) / (by - ay) * ex {
                    inside = !inside;
                }
            }
            let d = nearest * rows_per_unit;
            (if inside { -d } else { d }).clamp(-REACH, REACH)
        })
        .collect()
}

/// The letters' atlas: per texel the signed distance to the outline (rows
/// of the face, positive outside), unused, and the distance's gradient
/// across and up the face.
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

/// Glyph `g`'s coverage over a face from TRUG's front view, CELL x CELL
/// texels, softened (0 to 1).
fn trug_coverage(planes: &[u8], g: usize) -> Vec<f64> {
    let view = front_view(planes, g);
    let mask = outline(&view);
    let (w, h) = (FACE_COLUMNS, FACE_ROWS);
    let coverage: Vec<f64> = mask.iter().map(|&m| m as u8 as f64).collect();
    let texels_per_column = CELL as f64 / w as f64;
    blur(&upsample(&coverage, w, h), OUTLINE_SOFT * texels_per_column)
}

/// A face from its coverage: per texel the signed distance to the outline
/// (rows, positive outside; the outline where the coverage crosses a half),
/// 0, and the distance's gradient across and up the face.
pub fn face_of(coverage: &[f64]) -> Vec<[f64; 4]> {
    let inside: Vec<bool> = coverage.iter().map(|&v| v > 0.5).collect();
    let outside: Vec<bool> = inside.iter().map(|&v| !v).collect();
    let row_per_texel = FACE / CELL as f64;
    let to_inside = distance_to(&inside);
    let to_outside = distance_to(&outside);
    let sd: Vec<f64> = (0..CELL * CELL)
        .map(|i| ((to_inside[i] - to_outside[i]) * row_per_texel).clamp(-REACH, REACH))
        .collect();
    face_from(&sd)
}

/// A face from its signed distances: with their gradient.
fn face_from(sd: &[f64]) -> Vec<[f64; 4]> {
    let row_per_texel = FACE / CELL as f64;
    (0..CELL * CELL)
        .map(|i| {
            let (x, y) = (i % CELL, i / CELL);
            let at = |x: usize, y: usize| sd[y.min(CELL - 1) * CELL + x.min(CELL - 1)];
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(CELL - 1));
            let (y0, y1) = (y.saturating_sub(1), (y + 1).min(CELL - 1));
            let across = (at(x1, y) - at(x0, y)) / ((x1 - x0).max(1) as f64 * row_per_texel);
            // Up the face is up the texture's rows backwards.
            let up = (at(x, y0) - at(x, y1)) / ((y1 - y0).max(1) as f64 * row_per_texel);
            [sd[i], 0.0, across, up]
        })
        .collect()
}

/// One letter's face from TRUG: signed distance (rows), 0, its gradient.
pub fn letter(planes: &[u8], g: usize) -> Vec<[f64; 4]> {
    face_of(&trug_coverage(planes, g))
}

/// The letters in the atlas, from `source` (those it has; the rest
/// empty).
pub fn letterforms(source: LetterSource) -> Result<Letterforms, String> {
    let (width, height) = (ATLAS_SIZE.0 as usize, ATLAS_SIZE.1 as usize);
    let mut texels = vec![[f16::from_f64(REACH), f16::ZERO, f16::ZERO, f16::ZERO]; width * height];
    let mut place = |g: usize, face: Vec<[f64; 4]>| {
        let (cx, cy) = ((g % ATLAS_COLUMNS) * CELL, (g / ATLAS_COLUMNS) * CELL);
        for (i, t) in face.iter().enumerate() {
            let (x, y) = (cx + i % CELL, cy + i / CELL);
            texels[y * width + x] = t.map(f16::from_f64);
        }
    };
    match source {
        LetterSource::Outlines(text) => {
            let outlines = parse_outlines(text)?;
            for outline in &outlines.letters {
                let g = (outline.letter as u8 - b'A') as usize;
                place(g, face_from(&outline_distances(outline, outlines.box_size)));
            }
        }
        LetterSource::Trug(planes) => {
            for g in 0..26 {
                place(g, letter(planes, g));
            }
        }
    }
    Ok(Letterforms { texels })
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
        // The gradient points out of the stroke: at the block's left edge,
        // across is negative (outside lies to the left).
        let edge = at(7.0 - 5.0 + 0.3, 7.0);
        assert!(edge[2] < -0.5 && edge[3].abs() < 0.3, "gradient {edge:?}");
    }

    /// The authored outlines parse: the ten letters the words use, each
    /// with its contours; and a square's distances come out right.
    #[test]
    fn the_authored_outlines_parse() {
        let outlines = parse_outlines(LETTERS).unwrap();
        assert_eq!(outlines.box_size, (180.0, 180.0));
        let mut letters: Vec<char> = outlines.letters.iter().map(|o| o.letter).collect();
        letters.sort();
        assert_eq!(letters.iter().collect::<String>(), "ACEGINORST");
        for word in ["GREETINGS", "GOTO", "RENAISSANCE", "TRITON", "SONIC"] {
            assert!(word.chars().all(|c| letters.contains(&c)));
        }
        assert!(parse_outlines("box\t10\t10\nA\touter\t0,0 0,10 10,10 10,0\n").is_err());
        let square = Outline {
            letter: 'X',
            contours: vec![vec![
                (30.0, 30.0),
                (150.0, 30.0),
                (150.0, 150.0),
                (30.0, 150.0),
            ]],
        };
        let sd = outline_distances(&square, (180.0, 180.0));
        let middle = sd[(CELL / 2) * CELL + CELL / 2];
        assert!((middle + 60.0 * FACE / 180.0).abs() < 0.2, "{middle}");
        assert!(sd[0] > 2.0);
        // All ten letters build into the atlas.
        let atlas = letterforms(LetterSource::Outlines(LETTERS)).unwrap();
        let inside = atlas.texels.iter().filter(|t| t[0].to_f64() < 0.0).count();
        assert!(inside > 10 * CELL * CELL / 4, "{inside}");
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
