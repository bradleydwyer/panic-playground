//! Part 4's slab for the modern view: a block of stone in the original's
//! place, made when the preview loads from the logo the user's data
//! supplies; nothing derived from it is stored. What comes from where:
//! - Read from the code: the logo (320 x 180, 400-line rasters, shown from
//!   raster 237) and its palette; part 3's box, whose size the block takes:
//!   490 wide, 200 deep and 100 high (`DotsPreview::tables`, object P3D7).
//! - Measured from the logo: the slab's silhouette, row by row. Its top
//!   face's left and right edges and its front face's are straight, and
//!   meet where the two faces do.
//! - Interpretation: the painting as a pinhole view of a right-angled block
//!   on square display pixels. The four edges' vanishing rows give the
//!   camera's focal length and the block's tilt (its top face's vanishing
//!   row lies f tan(tilt) above the axis, its front face's f / tan(tilt)
//!   below); the near edge's row and width its distance, with the width
//!   taken as part 3's box's. The block's depth is fitted so its far edge
//!   meets the logo's first row (it comes out within a few percent of the
//!   box's 200); its height, below the screen, is the box's. The carving:
//!   the logo shades its letters as cut into the stone (dark on the side
//!   the light comes from, lit on the other), so where each face, unwarped
//!   from the view, is darker than its stone (a threshold between the two
//!   tones its lightness falls into) it is carved, deepest a little in
//!   from the edge; thin dark lines are shallow cracks; and the stone's
//!   colour and mottling are the logo's, flattened of its painted light.
//! - Authored: the carving's depth and bevel, the cracks' depth, the
//!   rounding of the edges, the plasma's sheet of light behind the slab
//!   (where it stands and how large it is), the exposure the slab arrives
//!   with and everything in plasma_light.wgsl.
use half::f16;

/// The edges' rounding, the carving's depth and the width of its bevel,
/// and the cracks' depth, in part 3's units (the block is 490 wide).
/// Authored.
pub const ROUNDING: f64 = 2.5;
pub const CARVE_DEPTH: f64 = 5.0;
const BEVEL: f64 = 8.0;
/// The relief's softening and the carving outline's, in texels.
const DEPTH_BLUR: f64 = 1.3;
const OUTLINE_BLUR: f64 = 2.0;
const CRACK_DEPTH: f64 = 1.4;
/// No carving this near a face's edges (the logo darkens its silhouette).
const EDGE_MARGIN: f64 = 4.0;
/// Texels of the relief across the block's width (authored; the logo
/// spans about 300 of its pixels).
pub const RELIEF_TEXELS: usize = 1024;
/// The plasma's sheet of light (authored): how far behind the slab's far
/// edge it stands, the source row its bottom edge projects to (the
/// plasma's last raster, 225, read from the code: its bottom is row 113),
/// how far above the screen's top it reaches, and the widest drawable it
/// spans (aspect).
const SHEET_BEHIND: f64 = 30.0;
/// How far the sheet leans toward the camera from upright (radians), so
/// it stands over the slab a little.
const SHEET_LEAN: f64 = 0.4;
const SHEET_BOTTOM_ROW: f64 = 113.0;
const SHEET_ABOVE_ROWS: f64 = 60.0;
const SHEET_ASPECT: f64 = 21.0 / 9.0;
/// The arrival's exposure: how far over the stone is exposed on the slab
/// frame (part 3's flare), and how it falls to 1 over the original's 64
/// frames of fade from white (authored).
const ARRIVAL_EXPOSURE: f64 = 40.0;
const ARRIVAL_CURVE: f64 = 2.2;
/// The stone's albedo, its luminance in linear light; its hue is the
/// logo's (authored level).
const STONE_ALBEDO: f32 = 0.26;
/// The block's tilt, focal length and depth must fall in these (a
/// sanity check on the measurement).
const TILT_RANGE: (f64, f64) = (0.15, 1.1);
const FOCAL_RANGE: (f64, f64) = (150.0, 600.0);

/// One face's relief: rgba16float texels, `size` across and down. Red:
/// the carving's depth (units, 0 at the face); green: the albedo's
/// multiplier; blue: how carved (0 to 1); alpha: how much crack.
pub struct Relief {
    pub size: (u32, u32),
    pub texels: Vec<f16>,
}

/// The modern view's slab and the camera it is seen through.
pub struct Stone {
    /// The camera: vertical focal length in source rows, square display
    /// pixels (a source column is 5/6 of a row, so the horizontal focal is
    /// 1.2 times it in columns), the source column on its axis (row 100).
    pub focal: f64,
    pub axis: f64,
    /// The block's tilt: its top face rises away from the camera at this
    /// angle to the line of sight (radians).
    pub tilt: f64,
    /// The top face's near edge's centre, in camera space (x right, y
    /// down, z away), at rest.
    pub corner: [f64; 3],
    /// Width, depth and height.
    pub size: [f64; 3],
    /// The stone's colour, linear.
    pub colour: [f32; 3],
    /// The plasma's sheet: its bottom edge's centre in camera space, its
    /// height (up the block's vertical) and half width.
    pub sheet: [f64; 3],
    pub sheet_up: [f64; 3],
    pub sheet_height: f64,
    pub sheet_half_width: f64,
    /// The top face, across (x) and from the near edge to the far (y), and
    /// the front face, across and from the top edge down.
    pub top: Relief,
    pub front: Relief,
    /// The rms difference, in source columns, between the measured
    /// silhouette and the fitted block's, over the rows fitted.
    pub residual: f64,
}

/// A line y = a x + b fitted by least squares.
fn fit_line(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(a, b), &(x, y)| (a + x, b + y));
    let (mx, my) = (sx / n, sy / n);
    let (sxy, sxx) = points.iter().fold((0.0, 0.0), |(a, b), &(x, y)| {
        (a + (x - mx) * (y - my), b + (x - mx) * (x - mx))
    });
    let a = sxy / sxx;
    (a, my - a * mx)
}

/// A separable Gaussian blur of `values` (width `w`) where `weight` is
/// nonzero, normalised by the weights, so the black round the logo does
/// not bleed in.
fn blur(values: &[f64], weight: &[f64], w: usize, sigma: (f64, f64)) -> Vec<f64> {
    let h = values.len() / w;
    let kernel = |s: f64| -> Vec<f64> {
        let r = (3.0 * s).ceil() as i64;
        (-r..=r)
            .map(|k| (-(k * k) as f64 / (2.0 * s * s)).exp())
            .collect()
    };
    let pass = |data: &[f64], horizontal: bool, s: f64| -> Vec<f64> {
        let k = kernel(s);
        let r = (k.len() / 2) as i64;
        let mut out = vec![0.0; data.len()];
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let mut sum = 0.0;
                for (i, kv) in k.iter().enumerate() {
                    let d = i as i64 - r;
                    let (px, py) = if horizontal { (x + d, y) } else { (x, y + d) };
                    if px >= 0 && py >= 0 && px < w as i64 && py < h as i64 {
                        sum += kv * data[(py * w as i64 + px) as usize];
                    }
                }
                out[(y * w as i64 + x) as usize] = sum;
            }
        }
        out
    };
    let weighted: Vec<f64> = values.iter().zip(weight).map(|(v, m)| v * m).collect();
    let num = pass(&pass(&weighted, true, sigma.1), false, sigma.0);
    let den = pass(&pass(weight, true, sigma.1), false, sigma.0);
    num.iter()
        .zip(&den)
        .map(|(n, d)| if *d > 1e-6 { n / d } else { 0.0 })
        .collect()
}

/// Bilinear sample of a `w`-wide image at (x, y), clamped at its edges.
fn sample(image: &[f64], w: usize, x: f64, y: f64) -> f64 {
    let h = image.len() / w;
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let at = |x, y| image[y * w + x];
    (at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx) * (1.0 - fy)
        + (at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx) * fy
}

/// The threshold between a set of values' two tones (Otsu's method).
fn two_tones(values: &[f64]) -> f64 {
    let mut hist = [0.0; 100];
    for &v in values {
        hist[((v * 100.0) as usize).min(99)] += 1.0;
    }
    let centre = |k: usize| (k as f64 + 0.5) / 100.0;
    let mut best = (0.0, 0.5);
    for split in 1..100 {
        let (w0, w1): (f64, f64) = (hist[..split].iter().sum(), hist[split..].iter().sum());
        if w0 == 0.0 || w1 == 0.0 {
            continue;
        }
        let m0 = (0..split).map(|k| hist[k] * centre(k)).sum::<f64>() / w0;
        let m1 = (split..100).map(|k| hist[k] * centre(k)).sum::<f64>() / w1;
        let score = w0 * w1 * (m0 - m1).powi(2);
        if score > best.0 {
            best = (score, centre(split));
        }
    }
    best.1
}

impl Stone {
    /// Fit the block to the logo (320 x 180 indices, shown from raster
    /// `REST_LINE_COMPARE + 1`) and carve its faces. `size` is part 3's
    /// box: width, depth, height; the depth is refitted.
    pub fn new(logo: &[u8], palette: &[u8], size: [f64; 3]) -> Result<Self, String> {
        use super::plasma::{LOGO_ROWS, LOGO_WIDTH, REST_LINE_COMPARE};
        let first_raster = (REST_LINE_COMPARE + 1) as f64;
        // The source row of the centre of the logo's row r, and its top.
        let row_y = |r: f64| (first_raster + r + 0.5) / 2.0;
        // The silhouette: each row's first and last stone pixel, rows that
        // are mostly stone (not the stray chip below the slab).
        let rows: Vec<(f64, f64, f64)> = (0..LOGO_ROWS)
            .filter_map(|r| {
                let line = &logo[r * LOGO_WIDTH..][..LOGO_WIDTH];
                let first = line.iter().position(|&i| i != 0)?;
                let last = line.iter().rposition(|&i| i != 0)?;
                (last - first > LOGO_WIDTH / 2).then_some((
                    r as f64,
                    first as f64,
                    last as f64 + 1.0,
                ))
            })
            .collect();
        if rows.len() < 60 {
            return Err("The logo's slab is too small to fit".into());
        }
        let widest = rows.iter().map(|&(_, a, b)| b - a).fold(0.0, f64::max);
        let plateau: Vec<f64> = rows
            .iter()
            .filter(|&&(_, a, b)| b - a >= widest - 0.5)
            .map(|&(r, _, _)| r)
            .collect();
        let split = plateau[plateau.len() / 2];
        let last = rows.last().unwrap().0;
        let half = |&(r, a, b): &(f64, f64, f64)| (row_y(r), (b - a) / 2.0);
        let top: Vec<_> = rows
            .iter()
            .filter(|&&(r, _, _)| r >= 3.0 && r <= split - 8.0)
            .map(half)
            .collect();
        let front: Vec<_> = rows
            .iter()
            .filter(|&&(r, _, _)| r >= split + 4.0 && r < last)
            .map(half)
            .collect();
        if top.len() < 20 || front.len() < 10 {
            return Err("The logo's slab has too few rows to fit".into());
        }
        let (at, bt) = fit_line(&top);
        let (af, bf) = fit_line(&front);
        let axis = rows.iter().map(|&(_, a, b)| (a + b) / 2.0).sum::<f64>() / rows.len() as f64;
        // The vanishing rows of the two faces' side edges.
        let (top_vanishing, front_vanishing) = (-bt / at, -bf / af);
        let focal = ((100.0 - top_vanishing) * (front_vanishing - 100.0)).sqrt();
        let tilt = ((100.0 - top_vanishing) / focal).atan();
        if !(FOCAL_RANGE.0..FOCAL_RANGE.1).contains(&focal)
            || !(TILT_RANGE.0..TILT_RANGE.1).contains(&tilt)
        {
            return Err(format!(
                "The slab does not fit a block (focal {focal:.1}, tilt {tilt:.3})"
            ));
        }
        // Where the faces meet: the near edge's row and half width.
        let near_y = (bf - bt) / (at - af);
        let near_half = at * near_y + bt;
        let width = size[0];
        let z = 0.6 * focal * width / near_half;
        let corner = [0.0, (near_y - 100.0) * z / focal, z];
        // The depth that puts the far edge on the top of the logo's first
        // row of stone.
        let far_y = (first_raster + rows[0].0) / 2.0 - 100.0;
        let (s, c) = tilt.sin_cos();
        let depth = (focal * corner[1] - far_y * corner[2]) / (far_y * c + focal * s);
        if !(0.5 * size[1]..2.0 * size[1]).contains(&depth) {
            return Err(format!("The slab's depth fits badly ({depth:.1})"));
        }
        let size = [width, depth, size[2]];
        let residual = {
            let model = |y: f64| if y < near_y { at * y + bt } else { af * y + bf };
            let squares: Vec<f64> = top
                .iter()
                .chain(&front)
                .map(|&(y, h)| (h - model(y)).powi(2))
                .collect();
            (squares.iter().sum::<f64>() / squares.len() as f64).sqrt()
        };
        let mut stone = Self {
            focal,
            axis,
            tilt,
            corner,
            size,
            colour: [0.0; 3],
            sheet: [0.0; 3],
            sheet_up: [0.0; 3],
            sheet_height: 0.0,
            sheet_half_width: 0.0,
            top: Relief {
                size: (0, 0),
                texels: Vec::new(),
            },
            front: Relief {
                size: (0, 0),
                texels: Vec::new(),
            },
            residual,
        };
        stone.place_sheet();
        stone.carve(logo, palette, row_y);
        Ok(stone)
    }

    /// The directions across the top face from its near edge to its far
    /// edge, and down the front face, in camera space.
    pub fn axes(&self) -> ([f64; 3], [f64; 3]) {
        let (s, c) = self.tilt.sin_cos();
        ([0.0, -s, c], [0.0, c, s])
    }

    /// Source column and row of a camera-space point.
    pub fn project(&self, p: [f64; 3]) -> (f64, f64) {
        (
            self.axis + 1.2 * self.focal * p[0] / p[2],
            100.0 + self.focal * p[1] / p[2],
        )
    }

    /// The near edge's centre with the slab slid `rows` source rows down
    /// the screen: it moves along the camera's y, so its far edge, the top
    /// of its silhouette, follows the original's line compare exactly (the
    /// near edge, closer, a little more).
    pub fn pose(&self, rows: f64) -> [f64; 3] {
        let [x, y, z] = self.corner;
        let far = z + self.size[1] * self.tilt.cos();
        [x, y + rows * far / self.focal, z]
    }

    /// The exposure at the fade's progress (0 the slab frame, white, to 1
    /// the fade's end): very high, falling to 1.
    pub fn exposure(&self, progress: f64) -> f64 {
        1.0 + ARRIVAL_EXPOSURE * (1.0 - progress.clamp(0.0, 1.0)).powf(ARRIVAL_CURVE)
    }

    /// Stand the sheet of light behind the slab, leaning toward the camera
    /// from upright, its bottom edge on the plasma's last raster.
    fn place_sheet(&mut self) {
        let (back, down) = self.axes();
        let (s, c) = SHEET_LEAN.sin_cos();
        let up: [f64; 3] = std::array::from_fn(|k| -c * down[k] - s * back[k]);
        self.sheet_up = up;
        let behind = self.size[1] + SHEET_BEHIND;
        let base: [f64; 3] = std::array::from_fn(|k| self.corner[k] + behind * back[k]);
        // The height along `up` at which a point of the sheet projects to
        // source row `row`.
        let at_row = |row: f64| {
            let r = row - 100.0;
            (r * base[2] - self.focal * base[1]) / (self.focal * up[1] - r * up[2])
        };
        let bottom = at_row(SHEET_BOTTOM_ROW);
        let top = at_row(-SHEET_ABOVE_ROWS);
        self.sheet = std::array::from_fn(|k| base[k] + bottom * up[k]);
        self.sheet_height = top - bottom;
        let columns = 160.0 * SHEET_ASPECT / (4.0 / 3.0) + 40.0;
        let far = self.sheet[2] + self.sheet_height * up[2].max(0.0);
        self.sheet_half_width = columns * far.max(self.sheet[2]) / (1.2 * self.focal);
    }

    /// Unwarp both faces from the logo and carve them.
    fn carve(&mut self, logo: &[u8], palette: &[u8], row_y: impl Fn(f64) -> f64) {
        use super::plasma::{LOGO_ROWS, LOGO_WIDTH};
        let lightness: Vec<f64> = logo
            .iter()
            .map(|&i| {
                let p = &palette[i as usize * 3..][..3];
                (p[0] as f64 + p[1] as f64 + p[2] as f64) / 189.0
            })
            .collect();
        let inside: Vec<f64> = logo.iter().map(|&i| (i != 0) as u8 as f64).collect();
        // The logo's rows are half as tall as its columns are wide on the
        // display, so its blurs are twice as long down.
        let smooth = blur(&lightness, &inside, LOGO_WIDTH, (1.6, 0.9));
        let fine = blur(&lightness, &inside, LOGO_WIDTH, (0.9, 0.5));
        let broad = blur(&lightness, &inside, LOGO_WIDTH, (5.0, 2.8));
        // The stone's colour: the mean of the logo's stone (its lighter
        // tone, not the carving).
        let tone = two_tones(
            &lightness
                .iter()
                .zip(&inside)
                .filter(|&(_, &m)| m > 0.0)
                .map(|(&l, _)| l)
                .collect::<Vec<_>>(),
        );
        let mut sum = [0.0_f64; 3];
        let mut count = 0.0_f64;
        for (&i, &l) in logo.iter().zip(&lightness) {
            if i != 0 && l > tone {
                for (k, s) in sum.iter_mut().enumerate() {
                    *s += super::clip::vga_linear(palette[i as usize * 3 + k]) as f64;
                }
                count += 1.0;
            }
        }
        let mean = sum.map(|s| s / count.max(1.0));
        let luminance = 0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2];
        self.colour = mean.map(|v| (v / luminance.max(1e-6)) as f32 * STONE_ALBEDO);
        let (back, down) = self.axes();
        let texel = self.size[0] / RELIEF_TEXELS as f64;
        let face = |extent: f64, direction: [f64; 3]| -> Relief {
            let (w, h) = (RELIEF_TEXELS, (extent / texel).round() as usize);
            // Each texel's place in the logo.
            let mut places = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let u = (x as f64 + 0.5) * texel - self.size[0] / 2.0;
                    let v = (y as f64 + 0.5) * texel;
                    let p: [f64; 3] = std::array::from_fn(|k| {
                        self.corner[k] + v * direction[k] + if k == 0 { u } else { 0.0 }
                    });
                    let (column, row) = self.project(p);
                    let first_raster = 2.0 * row_y(0.0) - 0.5;
                    places.push((column - 0.5, 2.0 * row - first_raster - 0.5));
                }
            }
            let shown = |&(c, r): &(f64, f64)| {
                (0.0..LOGO_WIDTH as f64 - 1.0).contains(&c)
                    && (0.0..LOGO_ROWS as f64 - 1.0).contains(&r)
                    && sample(&inside, LOGO_WIDTH, c, r) > 0.99
            };
            let at = |image: &[f64], &(c, r): &(f64, f64)| sample(image, LOGO_WIDTH, c, r);
            let seen: Vec<bool> = places.iter().map(shown).collect();
            // The lightness, smoothed a little more on the face, so the
            // carving's outline is smooth at the relief's resolution.
            let light: Vec<f64> = places.iter().map(|p| at(&smooth, p)).collect();
            let weights: Vec<f64> = seen.iter().map(|&s| s as u8 as f64).collect();
            let light = blur(&light, &weights, w, (OUTLINE_BLUR, OUTLINE_BLUR));
            let border = (EDGE_MARGIN / texel).ceil() as usize;
            let threshold = two_tones(
                &light
                    .iter()
                    .zip(&seen)
                    .filter(|&(_, &s)| s)
                    .map(|(&l, _)| l)
                    .collect::<Vec<_>>(),
            ) - 0.05;
            let carved: Vec<bool> = (0..w * h)
                .map(|i| {
                    let (x, y) = (i % w, i / w);
                    seen[i] && light[i] < threshold && x >= border && y >= border && x + border < w
                })
                .collect();
            let distance = super::clip::signed_distance(&carved, w, h);
            // The stone's own tone on this face, to flatten the painted
            // light out of its colour.
            let stone_light = {
                let values: Vec<f64> = light
                    .iter()
                    .zip(&carved)
                    .zip(&seen)
                    .filter(|&((_, &c), &s)| s && !c)
                    .map(|((&l, _), _)| l)
                    .collect();
                values.iter().sum::<f64>() / values.len().max(1) as f64
            };
            // The carving's depth, deepest a bevel in from its edge, the
            // cracks shallower, then softened by a texel or so.
            let carve: Vec<f64> = (0..w * h)
                .map(|i| {
                    let x = (distance[i] as f64 * texel / BEVEL).clamp(0.0, 1.0);
                    if carved[i] {
                        x * x * (3.0 - 2.0 * x)
                    } else {
                        0.0
                    }
                })
                .collect();
            let crack: Vec<f64> = (0..w * h)
                .map(|i| {
                    let p = &places[i];
                    if seen[i] {
                        ((at(&broad, p) - at(&fine, p) - 0.035) / 0.06).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                })
                .collect();
            let depth: Vec<f64> = carve
                .iter()
                .zip(&crack)
                .map(|(&c, &k)| CARVE_DEPTH * c + CRACK_DEPTH * k * (1.0 - c))
                .collect();
            let depth = blur(&depth, &vec![1.0; w * h], w, (DEPTH_BLUR, DEPTH_BLUR));
            let mut texels = Vec::with_capacity(w * h * 4);
            for i in 0..w * h {
                let albedo = if seen[i] {
                    (1.0 + 0.9 * (light[i] - stone_light)).clamp(0.55, 1.35)
                        * if carved[i] { 1.15 } else { 1.0 }
                } else {
                    1.0
                };
                texels.extend([depth[i], albedo, carve[i], crack[i]].map(f16::from_f64));
            }
            Relief {
                size: (w as u32, h as u32),
                texels,
            }
        };
        let (top, front) = (face(self.size[1], back), face(self.size[2], down));
        self.top = top;
        self.front = front;
    }

    /// A line for the tests and the load report.
    pub fn summary(&self) -> String {
        format!(
            "focal {:.1} rows, axis column {:.2}, tilt {:.2} deg, near edge at z {:.1}, \
             size {:.0} x {:.1} x {:.0}, silhouette rms {:.2} columns",
            self.focal,
            self.axis,
            self.tilt.to_degrees(),
            self.corner[2],
            self.size[0],
            self.size[1],
            self.size[2],
            self.residual
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An invented slab: a block of known size drawn through a known
    /// camera, row by row, as the logo would be. The fit finds the camera
    /// and the block again.
    #[test]
    fn the_fit_finds_an_invented_block_again() {
        let (focal, tilt, width, depth): (f64, f64, f64, f64) = (270.0, 0.55, 490.0, 190.0);
        let z = 520.0;
        let near = [0.0, 150.0, z];
        let (s, c) = tilt.sin_cos();
        let project = |p: [f64; 3]| {
            (
                160.0 + 1.2 * focal * p[0] / p[2],
                100.0 + focal * p[1] / p[2],
            )
        };
        let mut logo = vec![0u8; 320 * 180];
        for r in 0..180 {
            let y = (237.0 + r as f64 + 0.5) / 2.0;
            // The block's edges at this row: on the top face (between the
            // far and near edges) or on the front face (below).
            let edge = |v: f64, w: f64| {
                let p = [
                    width / 2.0,
                    near[1] - v * s + w * c,
                    near[2] + v * c + w * s,
                ];
                project(p)
            };
            let half = (0..4000).map(|k| k as f64 * 0.05).find_map(|t| {
                let (top, front) = (edge(depth - t, 0.0), edge(0.0, t));
                if (top.1 - y).abs() < 0.03 && t <= depth {
                    Some(top.0 - 160.0)
                } else if (front.1 - y).abs() < 0.03 && t <= 100.0 {
                    Some(front.0 - 160.0)
                } else {
                    None
                }
            });
            if let Some(h) = half {
                for x in (160.0 - h).round() as usize..(160.0 + h).round() as usize {
                    logo[r * 320 + x] = 5;
                }
            }
        }
        let mut palette = vec![0u8; 768];
        palette[15..18].copy_from_slice(&[40, 30, 20]);
        let stone = Stone::new(&logo, &palette, [490.0, 200.0, 100.0]).unwrap();
        assert!((stone.focal - focal).abs() < 6.0, "{}", stone.summary());
        assert!((stone.tilt - tilt).abs() < 0.02, "{}", stone.summary());
        assert!((stone.size[1] - depth).abs() < 6.0, "{}", stone.summary());
        assert!((stone.corner[2] - z).abs() < 15.0, "{}", stone.summary());
        assert!(stone.residual < 0.6, "{}", stone.summary());
        // Uncarved: one tone, no carving deeper than a crack.
        assert!(
            stone
                .top
                .texels
                .chunks(4)
                .all(|t| t[0].to_f64() <= CRACK_DEPTH + 1e-3),
            "carved an even slab"
        );
    }

    #[test]
    fn two_tones_split_between_them() {
        let values: Vec<f64> = (0..200)
            .map(|i| if i % 3 == 0 { 0.2 } else { 0.7 })
            .collect();
        let t = two_tones(&values);
        assert!(t > 0.2 && t < 0.7, "{t}");
    }
}
