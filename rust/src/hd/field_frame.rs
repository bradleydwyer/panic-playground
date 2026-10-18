//! Part 5's metal frame and the smoke behind and round it: the frame
//! author's CPU side. The look is field_frame.wgsl; see rust/README.md,
//! "Adding to part 5". Edit only this file and field_frame.wgsl.
//!
//! The frame's shape is made when the renderer loads, from the PSTART
//! picture the user's data supplies ([`frame_shape`]); nothing derived from
//! it is stored. What comes from where:
//! - Measured from the picture: the frame's outer edge and the crease
//!   between its outer face and its inner bevel on each side, read from the
//!   runs of colour outward from the window's edge, and the bands of
//!   lighter shading that cross each side at its middle.
//! - Interpretation: the ornaments' relief. The picture shades them as
//!   rounded metal, lighter where they stand out, so their height is the
//!   picture's lightness, smoothed, falling to nothing at their outlines,
//!   which are smoothed from its pixels.
//! - Authored: everything else, in field_frame.wgsl: the bevels' angles,
//!   the material and the lighting.
use super::{AuthorInfo, FIELD_WINDOW, FieldBlock, FieldPreview, FluidInfo, FluidStart};
use half::f16;

#[path = "field_frame_smoke.rs"]
mod smoke;
pub use smoke::FrameFluid;

pub static FRAME: AuthorInfo = AuthorInfo {
    name: "frame",
    source: include_str!("field_frame.wgsl"),
    prepass: true,
    controls: frame_controls,
};

/// The fluid's settings: the smoke (field_frame_smoke.rs) steps 60 times a
/// second from part 6's PSTART fade, after its pre-roll, and keeps a state
/// every 3 s as it is first reached, for seeking back. Its two light layers,
/// `size` texels over the source-pixel rectangle `region`, are the
/// fragments' `fluid_state` (the smoke behind the frame) and
/// `fluid_previous` (in front of it).
pub static FLUID: FluidInfo = FluidInfo {
    size: smoke::LAYERS,
    region: smoke::REGION,
    rate: 60.0,
    start: FluidStart::PictureFade,
    checkpoint: 180,
};

/// Draw part 6's PSTART fade (80.36 to 81.27 s) with part 5's modern view,
/// so that the fade brings in the lit metal frame rather than the pool's
/// picture: `picture_level()` rises from 0 to 1 over it, and
/// `field_seconds()` is negative. Off, part 6 draws its own picture, as it
/// always has.
pub const FRAME_TAKES_PART6_FADE: bool = true;

/// The frame's `u.field.frame_style` values at a clip time (zero by
/// default), given the rest of the block. The frame's lights follow the
/// block's own timing in field_frame.wgsl, so it needs none.
fn frame_controls(_: &FieldBlock, _: f64) -> [[f32; 4]; 4] {
    [[0.0; 4]; 4]
}

/// The picture's size, in source pixels.
const WIDTH: usize = 320;
const HEIGHT: usize = 200;
/// Shape texels per source pixel each way.
const SHAPE_SCALE: usize = 4;
/// Texels of the parameter row (see [`FrameShape`]): the bands by column,
/// by row, and the measured rectangles.
const BANDS_BY_COLUMN: usize = 0;
const BANDS_BY_ROW: usize = WIDTH;
const RECTANGLES: usize = 1200;
/// The ornaments' highest relief, in source rows, and the width of the
/// rounding at their outlines, in source pixels (authored).
const RELIEF_HEIGHT: f32 = 8.0;
const RELIEF_EDGE: f32 = 1.6;
/// Smoothing, in source pixels: the outlines' corners and steps, the
/// picture's shading (its 16 tones would show as terraces), the relief, and
/// the neighbourhood the cavities are measured against (authored).
const OUTLINE_BLUR: f32 = 0.6;
const SHADING_BLUR: f32 = 1.0;
const RELIEF_BLUR: f32 = 0.3;
const CAVITY_BLUR: f32 = 2.0;
/// Signed distances are kept within this many source pixels.
const DISTANCE_LIMIT: f32 = 8.0;

/// The frame's shape, made from the PSTART picture when the renderer
/// loads, as Rgba16Float texels, `SHAPE_SCALE` a source pixel each way, in
/// two pages and a row of parameters (field_frame.wgsl reads them):
/// - Page 0 (rows 0 to 799): the ornaments' height in source rows, its
///   gradient per source column and per source row (y down), and the signed
///   distance to their outlines in source pixels (inside positive).
/// - Page 1 (rows 800 to 1599): the cavity (the height less its
///   surroundings'), the smoothed lightness of the picture's shading (0 to
///   1) and the ornaments' smoothed coverage.
/// - The last row: at texel x < 320, the bands across the top and bottom
///   sides at column x (1 in a band); at 320 + y, the bands across the left
///   and right sides at row y; at 1200 the frame's outer rectangle and at
///   1201 its crease's (x0, y0, x1, y1, source pixels); at 1202, 1 if the
///   frame was found.
pub struct FrameShape {
    pub size: (usize, usize),
    pub texels: Vec<[f16; 4]>,
}

impl FrameShape {
    fn texel(&self, x: usize, y: usize) -> [f32; 4] {
        self.texels[y * self.size.0 + x].map(f16::to_f32)
    }
    /// The measured frame's outer rectangle and its crease's (x0, y0, x1,
    /// y1, source pixels), if the frame was found.
    pub fn frame(&self) -> Option<([f32; 4], [f32; 4])> {
        let row = self.size.1 - 1;
        (self.texel(RECTANGLES + 2, row)[0] > 0.5)
            .then(|| (self.texel(RECTANGLES, row), self.texel(RECTANGLES + 1, row)))
    }
    /// The ornaments' signed distance at source point s, source pixels,
    /// inside positive (bilinear between texels; far outside beyond the
    /// picture).
    pub fn ornament_distance(&self, s: [f64; 2]) -> f32 {
        let k = SHAPE_SCALE as f64;
        let (w, h) = (WIDTH * SHAPE_SCALE, HEIGHT * SHAPE_SCALE);
        let q = [s[0] * k - 0.5, s[1] * k - 0.5];
        if q[0] < -1.0 || q[1] < -1.0 || q[0] > w as f64 || q[1] > h as f64 {
            return -DISTANCE_LIMIT;
        }
        let (x0, y0) = (q[0].floor(), q[1].floor());
        let (fx, fy) = ((q[0] - x0) as f32, (q[1] - y0) as f32);
        let at = |x: f64, y: f64| {
            let x = (x as isize).clamp(0, w as isize - 1) as usize;
            let y = (y as isize).clamp(0, h as isize - 1) as usize;
            self.texel(x, y)[3]
        };
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// The frame's shape for a preview: from its picture, through the palette
/// part 5 opens with.
pub fn frame_shape(preview: &FieldPreview) -> FrameShape {
    let palette = preview
        .frames
        .first()
        .and_then(|f| preview.palettes.get(f.palette));
    bake(&preview.picture, palette.map(Vec::as_slice))
}

/// The frame round the window, measured from the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    /// x0, y0, x1, y1 in source pixels.
    outer: [f32; 4],
    crease: [f32; 4],
}

/// Each side's line outward from the window's edge: the source pixel `k`
/// pixels out (0 is the frame's first pixel) at a point `along` the side.
fn side_pixel(side: usize, along: usize, k: usize) -> Option<(usize, usize)> {
    let [x0, y0, x1, y1] = FIELD_WINDOW.map(|v| v as usize);
    let (x, y) = match side {
        0 => (x0.checked_sub(k + 1)?, along),
        1 => (x1 + k, along),
        2 => (along, y0.checked_sub(k + 1)?),
        _ => (along, y1 + k),
    };
    (x < WIDTH && y < HEIGHT).then_some((x, y))
}

/// The window's extent along each side.
fn side_span(side: usize) -> (usize, usize) {
    let [x0, y0, x1, y1] = FIELD_WINDOW.map(|v| v as usize);
    if side < 2 { (y0, y1) } else { (x0, x1) }
}

/// One line's frame, walking outward from the window: the distance to the
/// background (the outer edge) and to the crease, the middle of the pixels
/// between the first run of three or more (the inner bevel, after a foot of
/// a pixel or two) and the next.
fn measure_line(picture: &[u8], background: u8, side: usize, along: usize) -> Option<(f32, f32)> {
    let mut colours = Vec::new();
    for k in 0..64 {
        let (x, y) = side_pixel(side, along, k)?;
        let c = picture[y * WIDTH + x];
        if c == background {
            break;
        }
        colours.push(c);
    }
    let edge = colours.len();
    // Runs of one colour: (start, length).
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (k, &c) in colours.iter().enumerate() {
        match runs.last_mut() {
            Some(run) if colours[run.0] == c => run.1 += 1,
            _ => runs.push((k, 1)),
        }
    }
    let inner = runs.iter().position(|&(_, n)| n >= 3)?;
    let end = runs[inner].0 + runs[inner].1;
    let next = runs[inner + 1..].iter().find(|&&(_, n)| n >= 3)?;
    // The crease's pixels must be few, and the outer face must end before
    // the edge.
    (next.0 - end <= 2 && next.0 + next.1 <= edge)
        .then_some((edge as f32, (end + next.0) as f32 / 2.0))
}

fn median(mut values: Vec<f32>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f32::total_cmp);
    Some(values[values.len() / 2])
}

/// The frame round the window: each side measured along lines a fifth and
/// three tenths of the way in from either end of the window's edge (clear of
/// the mitres and of the bands at the middle), taking the median.
fn measure_frame(picture: &[u8], background: u8) -> Option<Frame> {
    let mut edge = [0.0; 4];
    let mut crease = [0.0; 4];
    for side in 0..4 {
        let (a, b) = side_span(side);
        let lines: Vec<(f32, f32)> = [0.2, 0.3, 0.7, 0.8]
            .iter()
            .filter_map(|f| {
                let along = a + ((b - a) as f32 * f) as usize;
                measure_line(picture, background, side, along)
            })
            .collect();
        edge[side] = median(lines.iter().map(|l| l.0).collect())?;
        crease[side] = median(lines.iter().map(|l| l.1).collect())?;
    }
    let [x0, y0, x1, y1] = FIELD_WINDOW;
    Some(Frame {
        outer: [x0 - edge[0], y0 - edge[2], x1 + edge[1], y1 + edge[3]],
        crease: [
            x0 - crease[0],
            y0 - crease[2],
            x1 + crease[1],
            y1 + crease[3],
        ],
    })
}

/// The bands across each side: where the middle of its outer face is
/// lighter than that line's median, in each side's own trapezoid. Left and
/// right by row, top and bottom by column.
fn measure_bands(picture: &[u8], lightness: &[f32; 256], frame: &Frame) -> [Vec<f32>; 4] {
    let window = FIELD_WINDOW.map(|v| v as usize);
    let edges = [
        window[0] as f32 - frame.outer[0],
        frame.outer[2] - window[2] as f32,
        window[1] as f32 - frame.outer[1],
        frame.outer[3] - window[3] as f32,
    ];
    let creases = [
        window[0] as f32 - frame.crease[0],
        frame.crease[2] - window[2] as f32,
        window[1] as f32 - frame.crease[1],
        frame.crease[3] - window[3] as f32,
    ];
    std::array::from_fn(|side| {
        let length = if side < 2 { HEIGHT } else { WIDTH };
        let mut band = vec![0.0; length];
        let k = ((edges[side] + creases[side]) / 2.0) as usize;
        let (a, b) = side_span(side);
        // Within the mitres at this depth, a pixel clear of them.
        let span = a.saturating_sub(k) + 1..(b + k).min(length) - 1;
        let line: Vec<(usize, f32)> = span
            .filter_map(|along| {
                let (x, y) = side_pixel(side, along, k)?;
                Some((along, lightness[picture[y * WIDTH + x] as usize]))
            })
            .collect();
        if let Some(typical) = median(line.iter().map(|l| l.1).collect()) {
            for (along, l) in line {
                if l > typical + 0.01 {
                    band[along] = 1.0;
                }
            }
        }
        band
    })
}

/// Run `f` on each w-long row of `field` with its number, the rows shared
/// among threads.
fn rows_at<T: Send>(field: &mut [T], w: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let h = field.len() / w;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = h.div_ceil(threads.max(1)).max(1);
    std::thread::scope(|scope| {
        for (k, block) in field.chunks_mut(per * w).enumerate() {
            let f = &f;
            scope.spawn(move || {
                for (j, row) in block.chunks_mut(w).enumerate() {
                    f(k * per + j, row);
                }
            });
        }
    });
}

/// Run `f` on each w-long row of `field`, the rows shared among threads.
fn rows<T: Send>(field: &mut [T], w: usize, f: impl Fn(&mut [T]) + Sync) {
    rows_at(field, w, |_, row| f(row));
}

/// A w x h field transposed: h x w, in tiles for the cache's sake.
fn transpose<T: Copy + Default>(field: &[T], (w, h): (usize, usize)) -> Vec<T> {
    const TILE: usize = 32;
    let mut out = vec![T::default(); w * h];
    for y0 in (0..h).step_by(TILE) {
        for x0 in (0..w).step_by(TILE) {
            for y in y0..(y0 + TILE).min(h) {
                for x in x0..(x0 + TILE).min(w) {
                    out[x * h + y] = field[y * w + x];
                }
            }
        }
    }
    out
}

/// Box widths whose three passes match a Gaussian of `sigma`'s variance.
fn box_widths(sigma: f32) -> [usize; 3] {
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let lower = (ideal.floor() as usize) | 1;
    let lower = if lower as f32 > ideal {
        lower - 2
    } else {
        lower
    }
    .max(1);
    let upper = lower + 2;
    let m = ((12.0 * sigma * sigma - n * (lower * lower) as f32 - 4.0 * n * lower as f32 - 3.0 * n)
        / (-4.0 * lower as f32 - 4.0))
        .round()
        .max(0.0) as usize;
    std::array::from_fn(|i| if i < m { lower } else { upper })
}

/// Three box blurs along one line, clamped at its ends.
fn blur_line(line: &mut [f32], widths: &[usize; 3]) {
    let n = line.len();
    let mut padded = Vec::with_capacity(n + 2 * widths[2] + 2);
    for &width in widths {
        let r = width / 2;
        padded.clear();
        padded.resize(r, line[0]);
        padded.extend_from_slice(line);
        padded.resize(n + 2 * r + 1, line[n - 1]);
        let mut sum: f32 = padded[..width].iter().sum();
        let scale = 1.0 / width as f32;
        for (i, out) in line.iter_mut().enumerate() {
            *out = sum * scale;
            sum += padded[i + width] - padded[i];
        }
    }
}

/// Blur a w x h field by a Gaussian of `sigma` texels, approximated by
/// three box blurs each way: along the rows, then down the columns a row at
/// a time, clamped at the edges.
fn blur(field: &mut [f32], (w, h): (usize, usize), sigma: f32) {
    let widths = box_widths(sigma);
    rows(field, w, |line| blur_line(line, &widths));
    let mut source = vec![0.0; w * h];
    let mut sum = vec![0.0; w];
    for width in widths {
        let r = width as isize / 2;
        source.copy_from_slice(field);
        let row = |y: isize| &source[y.clamp(0, h as isize - 1) as usize * w..][..w];
        sum.fill(0.0);
        for y in -r..=r {
            sum.iter_mut().zip(row(y)).for_each(|(s, v)| *s += v);
        }
        let scale = 1.0 / width as f32;
        for y in 0..h as isize {
            let out = &mut field[y as usize * w..][..w];
            out.iter_mut().zip(&sum).for_each(|(o, s)| *o = s * scale);
            let (add, remove) = (row(y + r + 1), row(y - r));
            for x in 0..w {
                sum[x] += add[x] - remove[x];
            }
        }
    }
}

/// One line of Felzenszwalb and Huttenlocher's exact squared distance
/// transform, in place: each value becomes the least (q - p)^2 + f(p).
fn distance_line(f: &mut [f32]) {
    let n = f.len();
    let before = f.to_vec();
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    let at = |q: usize| before[q] + (q * q) as f32;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = (at(q) - at(p)) / (2.0 * (q - p) as f32);
            if s > z[k] || k == 0 {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in f.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f32 - p as f32).powi(2) + before[p];
    }
}

/// Squared distance from each texel to the nearest texel where `seed` is
/// set (exact, one axis at a time; beyond reach where no texel is set).
fn distance_squared(seed: &[bool], (w, h): (usize, usize)) -> Vec<f32> {
    const FAR: f32 = 1e10;
    let mut d: Vec<f32> = seed.iter().map(|&s| if s { 0.0 } else { FAR }).collect();
    rows(&mut d, w, distance_line);
    let mut t = transpose(&d, (w, h));
    rows(&mut t, h, distance_line);
    transpose(&t, (h, w))
}

/// Bake the frame's shape from a 320 x 200 picture of palette indices and
/// its 6-bit palette (without one, an index's lightness is the index over
/// 255). The background is the colour of the picture's top-left pixel
/// (`Field::new` sets the top row and left column to 255).
fn bake(picture: &[u8], palette: Option<&[u8]>) -> FrameShape {
    let k = SHAPE_SCALE;
    let (w, h) = (WIDTH * k, HEIGHT * k);
    let lightness: [f32; 256] = std::array::from_fn(|i| match palette {
        Some(p) if p.len() >= 768 => {
            let c = |j: usize| (p[i * 3 + j] & 63) as f32 / 63.0;
            0.2126 * c(0) + 0.7152 * c(1) + 0.0722 * c(2)
        }
        _ => i as f32 / 255.0,
    });
    let background = picture[0];
    let frame = measure_frame(picture, background);
    let in_rect = |r: [f32; 4], x: usize, y: usize| {
        let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
        x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
    };
    // The ornaments: everything but the background outside the frame and
    // the window.
    let ornament: Vec<bool> = (0..WIDTH * HEIGHT)
        .map(|i| {
            let (x, y) = (i % WIDTH, i / WIDTH);
            picture[i] != background
                && !in_rect(FIELD_WINDOW, x, y)
                && !frame.is_some_and(|f| in_rect(f.outer, x, y))
        })
        .collect();
    // Their lightness, 0 to 1 over the tones they use.
    let (lo, hi) = ornament
        .iter()
        .zip(picture)
        .filter(|(o, _)| **o)
        .fold((f32::MAX, f32::MIN), |(lo, hi), (_, &c)| {
            (lo.min(lightness[c as usize]), hi.max(lightness[c as usize]))
        });
    let range = (hi - lo).max(1e-3);
    let upscale = |value: &dyn Fn(usize) -> f32| -> Vec<f32> {
        (0..w * h)
            .map(|i| value((i / w / k) * WIDTH + (i % w) / k))
            .collect()
    };
    let mut cover = upscale(&|i| if ornament[i] { 1.0 } else { 0.0 });
    blur(&mut cover, (w, h), OUTLINE_BLUR * k as f32);
    let inside: Vec<bool> = cover.iter().map(|&c| c > 0.5).collect();
    let outside: Vec<bool> = inside.iter().map(|&i| !i).collect();
    let to_outside = distance_squared(&outside, (w, h));
    let to_inside = distance_squared(&inside, (w, h));
    let limit = DISTANCE_LIMIT;
    let mut distance: Vec<f32> = (0..w * h)
        .map(|i| {
            let d = if inside[i] {
                to_outside[i].sqrt() - 0.5
            } else {
                0.5 - to_inside[i].sqrt()
            };
            (d / k as f32).clamp(-limit, limit)
        })
        .collect();
    blur(&mut distance, (w, h), 1.0);
    let mut shade = upscale(&|i| {
        if ornament[i] {
            (lightness[picture[i] as usize] - lo) / range
        } else {
            0.0
        }
    });
    blur(&mut shade, (w, h), SHADING_BLUR * k as f32);
    let mut height: Vec<f32> = shade
        .iter()
        .zip(&distance)
        .map(|(&s, &d)| {
            let e = 1.0 - (1.0 - (d / RELIEF_EDGE).clamp(0.0, 1.0)).powi(2);
            RELIEF_HEIGHT * (0.25 + 0.75 * s.clamp(0.0, 1.0)) * e.sqrt()
        })
        .collect();
    blur(&mut height, (w, h), RELIEF_BLUR * k as f32);
    let mut surround = height.clone();
    blur(&mut surround, (w, h), CAVITY_BLUR * k as f32);
    let at = |x: isize, y: isize| {
        height[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize]
    };
    let mut texels = vec![[f16::ZERO; 4]; w * (2 * h + 1)];
    rows_at(&mut texels[..2 * h * w], w, |row, out| {
        let y = row % h;
        for (x, texel) in out.iter_mut().enumerate() {
            let i = y * w + x;
            *texel = if row < h {
                let (xi, yi) = (x as isize, y as isize);
                let gx = (at(xi + 1, yi) - at(xi - 1, yi)) * k as f32 / 2.0;
                let gy = (at(xi, yi + 1) - at(xi, yi - 1)) * k as f32 / 2.0;
                [height[i], gx, gy, distance[i]]
            } else {
                [height[i] - surround[i], shade[i], cover[i], 0.0]
            }
            .map(f16::from_f32);
        }
    });
    let row = 2 * h * w;
    if let Some(frame) = frame {
        let bands = measure_bands(picture, &lightness, &frame);
        for x in 0..WIDTH {
            texels[row + BANDS_BY_COLUMN + x] =
                [bands[2][x], bands[3][x], 0.0, 0.0].map(f16::from_f32);
        }
        for y in 0..HEIGHT {
            texels[row + BANDS_BY_ROW + y] =
                [bands[0][y], bands[1][y], 0.0, 0.0].map(f16::from_f32);
        }
        texels[row + RECTANGLES] = frame.outer.map(f16::from_f32);
        texels[row + RECTANGLES + 1] = frame.crease.map(f16::from_f32);
        texels[row + RECTANGLES + 2] = [1.0, 0.0, 0.0, 0.0].map(f16::from_f32);
    }
    FrameShape {
        size: (w, 2 * h + 1),
        texels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic picture shaped like PSTART's: a background of 255; round
    /// the window a frame whose sides run, outward, a foot pixel, an inner
    /// bevel of 14, a crease pixel, an outer face of 12 and a lip pixel; a
    /// band of lighter pixels across the left side; and a round ornament,
    /// lighter towards its middle. A grey palette, lighter at lower
    /// indices from 224.
    fn picture() -> (Vec<u8>, Vec<u8>) {
        let mut p = vec![255u8; WIDTH * HEIGHT];
        let [x0, y0, x1, y1] = FIELD_WINDOW.map(|v| v as i32);
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                let u = [x0 - 1 - x, x - x1, y0 - 1 - y, y - y1];
                let side = (0..4).max_by_key(|&s| u[s]).unwrap();
                let k = u[side];
                if !(0..29).contains(&k) {
                    continue;
                }
                let mut c = match k {
                    0 => 236,
                    1..=14 => 232 + side as u8,
                    15 => 231,
                    16..=27 => 226 + side as u8,
                    _ => 224,
                };
                if side == 0 && (90..96).contains(&y) && (1..28).contains(&k) {
                    c -= 1;
                }
                p[y as usize * WIDTH + x as usize] = c;
            }
        }
        for y in 80..120 {
            for x in 10..50 {
                let r = (((x as f32 - 30.0).powi(2) + (y as f32 - 100.0).powi(2)).sqrt()) as u8;
                if r < 15 {
                    p[y * WIDTH + x] = 224 + r;
                }
            }
        }
        let mut palette = vec![0u8; 768];
        for i in 0..256 {
            let v = if i >= 224 { 63 - 3 * (i - 224) } else { 0 } as u8;
            palette[i * 3..i * 3 + 3].fill(v);
        }
        (p, palette)
    }

    fn texel(shape: &FrameShape, x: usize, y: usize) -> [f32; 4] {
        shape.texels[y * shape.size.0 + x].map(f16::to_f32)
    }

    #[test]
    fn frame_is_measured_from_the_picture() {
        let (p, palette) = picture();
        let shape = bake(&p, Some(&palette));
        let k = SHAPE_SCALE;
        assert_eq!(shape.size, (WIDTH * k, 2 * HEIGHT * k + 1));
        let row = 2 * HEIGHT * k;
        let [x0, y0, x1, y1] = FIELD_WINDOW;
        assert_eq!(
            texel(&shape, RECTANGLES, row),
            [x0 - 29.0, y0 - 29.0, x1 + 29.0, y1 + 29.0]
        );
        // The crease is the middle of the pixel 15 out.
        assert_eq!(
            texel(&shape, RECTANGLES + 1, row),
            [x0 - 15.5, y0 - 15.5, x1 + 15.5, y1 + 15.5]
        );
        assert_eq!(texel(&shape, RECTANGLES + 2, row)[0], 1.0);
        // The band across the left side, and none elsewhere.
        for y in 0..HEIGHT {
            let band = texel(&shape, BANDS_BY_ROW + y, row);
            assert_eq!(
                band,
                [if (90..96).contains(&y) { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]
            );
        }
        for x in 0..WIDTH {
            assert_eq!(texel(&shape, BANDS_BY_COLUMN + x, row), [0.0; 4]);
        }
    }

    #[test]
    fn ornaments_rise_towards_their_lighter_middles() {
        let (p, palette) = picture();
        let shape = bake(&p, Some(&palette));
        let k = SHAPE_SCALE;
        let at = |x: f32, y: f32| texel(&shape, (x * k as f32) as usize, (y * k as f32) as usize);
        let middle = at(30.0, 100.0);
        // Highest in the middle, as far inside its outline as distances go.
        assert!(middle[0] > 0.8 * RELIEF_HEIGHT, "{middle:?}");
        assert_eq!(middle[3], DISTANCE_LIMIT, "{middle:?}");
        // Falling away to the right (x) and downward (y down).
        let right = at(38.0, 100.0);
        assert!(right[0] < middle[0] && right[1] < 0.0, "{right:?}");
        let below = at(30.0, 108.0);
        assert!(below[0] < middle[0] && below[2] < 0.0, "{below:?}");
        // Nothing at the outline, outside it and on the frame.
        assert!(at(30.0, 100.0 + 15.0)[3].abs() < 0.6);
        let outside = at(30.0, 70.0);
        assert!(outside[0].abs() < 1e-3 && outside[3] < -5.0, "{outside:?}");
        let frame = at(80.0, 60.0);
        assert!(
            frame[0].abs() < 1e-3 && frame[3] <= -DISTANCE_LIMIT + 0.1,
            "{frame:?}"
        );
        // The cavity page: the middle stands above its surroundings.
        let cavity = texel(&shape, 30 * k, (HEIGHT + 100) * k);
        assert!(cavity[0] > 0.0 && cavity[2] > 0.99, "{cavity:?}");
    }

    /// The frame measured from the user's PSTART picture, as `Field::new`
    /// leaves it (its top row and left column 255), through WINDOW.PAL,
    /// whose upper half part 5 shows: printed, and checked against the
    /// runs of colour read from the picture.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn frame_is_measured_from_pstart() {
        let data = crate::test_support::data();
        let mut p = data.file("PSTART.U").unwrap();
        p[..WIDTH].fill(255);
        for y in 0..HEIGHT {
            p[y * WIDTH] = 255;
        }
        let palette = data.file("WINDOW.PAL").unwrap();
        let started = std::time::Instant::now();
        let shape = bake(&p, Some(&palette));
        let row = 2 * HEIGHT * SHAPE_SCALE;
        let outer = texel(&shape, RECTANGLES, row);
        let crease = texel(&shape, RECTANGLES + 1, row);
        let bands = |base: usize, n: usize, k: usize| {
            (0..n)
                .filter(|&i| texel(&shape, base + i, row)[k] > 0.5)
                .collect::<Vec<_>>()
        };
        let sides = [
            bands(BANDS_BY_ROW, HEIGHT, 0),
            bands(BANDS_BY_ROW, HEIGHT, 1),
            bands(BANDS_BY_COLUMN, WIDTH, 0),
            bands(BANDS_BY_COLUMN, WIDTH, 1),
        ];
        let ornament = shape.texels[..HEIGHT * SHAPE_SCALE * shape.size.0]
            .iter()
            .filter(|t| t[3].to_f32() > 0.0)
            .count();
        println!(
            "baked in {:.0} ms: outer {outer:?}, crease {crease:?}, bands {sides:?}, ornament texels {ornament}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        assert_eq!(outer, [67.0, 17.0, 253.0, 175.0]);
        assert_eq!(crease, [80.5, 30.5, 239.5, 161.5]);
        assert!(sides.iter().all(|s| !s.is_empty()));
        assert!(ornament > 10_000);
    }

    #[test]
    fn a_blank_picture_has_no_frame_or_ornaments() {
        let shape = bake(&vec![0; WIDTH * HEIGHT], None);
        assert!(
            shape
                .texels
                .iter()
                .all(|t| t.iter().all(|v| v.to_f32().abs() <= DISTANCE_LIMIT))
        );
        let row = 2 * HEIGHT * SHAPE_SCALE;
        assert_eq!(texel(&shape, RECTANGLES + 2, row)[0], 0.0);
        assert!(shape.texels[..row].iter().all(|t| t[0].to_f32() == 0.0));
    }

    #[test]
    fn distances_and_blurs_are_right() {
        // One seed: distances are exact.
        let (w, h) = (7, 5);
        let mut seed = vec![false; w * h];
        seed[2 * w + 3] = true;
        let d = distance_squared(&seed, (w, h));
        for y in 0..h {
            for x in 0..w {
                let want = (x as f32 - 3.0).powi(2) + (y as f32 - 2.0).powi(2);
                assert_eq!(d[y * w + x], want);
            }
        }
        // A blur keeps a constant and the total of a spike.
        let mut flat = vec![2.0; 40 * 30];
        blur(&mut flat, (40, 30), 3.0);
        assert!(flat.iter().all(|&v| (v - 2.0).abs() < 1e-4));
        let mut spike = vec![0.0; 40 * 30];
        spike[15 * 40 + 20] = 1.0;
        blur(&mut spike, (40, 30), 2.0);
        let total: f32 = spike.iter().sum();
        assert!((total - 1.0).abs() < 1e-4, "{total}");
        let var: f32 = (0..40)
            .map(|x| (x as f32 - 20.0).powi(2) * spike[15 * 40 + x])
            .sum::<f32>()
            / (0..40).map(|x| spike[15 * 40 + x]).sum::<f32>();
        assert!((var.sqrt() - 2.0).abs() < 0.3, "{}", var.sqrt());
    }
}
