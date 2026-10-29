//! Part 6 (Rings) for the HD preview: the state of every part-6 frame read
//! from the port's `Rings` scene, the ring band model and symbol fields built
//! in memory from the local data, the original frames replayed on demand for
//! comparison, the authored symbol sets that replace the original symbols
//! in the middle of the part, and the registry of part-6 styles, now the
//! pool alone.
//!
//! What comes from where:
//! - Read from the port's code and counters (`Rings::hd_state`,
//!   `Rings::hd_offsets`): the phase, the ring window offsets per draw step,
//!   the intensity, the lit symbol per frame, the pages and the PSTART level.
//! - Decoded from the local data at load time: the 16 symbol masks (as
//!   `Rings::new` leaves them in planes 2 and 3), the ring source (M.DAT)
//!   and the PSTART picture. Nothing derived from them is stored in Git.
//! - Interpretation: the analytic band model fitted to M.DAT
//!   ([`RingBands::fit`]), the smoothed outlines and centrelines of the
//!   masks, interpolation between the original's 70 Hz frames, the
//!   one-frame-latency reading of the page flips that skips the stale-page
//!   artefact, and the symbols' time-averaged flicker.
//! - Authored: everything the style adds (neon tubes, the pool, lighting),
//!   and the 7 new symbol sets in rings_symbols.txt, line drawings made for
//!   the remake (some are tracings of company logos, their owners'
//!   trademarks). Their runs are made into neon tubes as signs are
//!   ([`SymbolFields::authored`]), shown in the 7 patterns of the tune from
//!   window [`NEW_SET_FIRST_WINDOW`]; the music still picks which mask
//!   lights.
use super::clip::{PIXEL_ASPECT, smooth_signed_distance, vga_linear};
use crate::{
    data::Data,
    engine::{FRAME_RATE, SAMPLE_RATE},
    music::Music,
    scenes::rings::{HdRings, Rings},
    vga::Vga,
};
use half::f16;
use std::sync::Mutex;

/// `u.state.x` during part 6. The intro uses 0..=5 and part 2 6..=9.
pub const PART6_PHASE: f32 = 10.0;
/// Texels per source pixel in the symbol atlas. rings_common.wgsl repeats it.
pub const SYMBOL_SCALE: usize = 2;
/// Source pixels of margin around the union of the symbols in the atlas.
pub const SYMBOL_PAD: usize = 12;
/// Atlas distances clamp here, in source rows. rings_common.wgsl repeats it.
pub const SYMBOL_LIMIT: f32 = 12.0;
/// Gaussian blur, in source pixels, before the symbols' outlines are traced
/// (as for the scroller's letters, which use 0.65): it rounds the hand-drawn
/// strokes' one-pixel stair steps. An authored interpretation.
pub const SYMBOL_SMOOTHING: f32 = 0.85;
/// Authored outer radius of the neon tubes' glass, in source rows; the
/// glowing gas fills the middle half of it. The light map is built from the
/// glass. rings_common.wgsl repeats it as NEON_RADIUS.
pub const NEON_RADIUS: f32 = 3.8;
/// The original strokes' half-width, about, in source rows (an
/// interpretation of the masks). The tubes' runs are measured against it:
/// what is too short to keep, and the air left between crossing tubes.
pub const NEON_STROKE: f32 = 2.2;
/// The neon light map covers this source-pixel rectangle (the 320 × 200
/// frame and a wide margin) at one texel per pixel, with a full mip chain.
pub const LIGHT_ORIGIN: [i32; 2] = [-96, -92];
pub const LIGHT_SIZE: [u32; 2] = [512, 384];
/// The ring window `step` copies into: 256 × 184 pixels at screen (32, 8),
/// read from the code. rings_common.wgsl repeats it.
pub const RING_WINDOW: [f32; 4] = [32.0, 8.0, 288.0, 192.0];
/// Authored exposure for the rings' motion blur, in 70 Hz ticks: about
/// 1/100 s, as for the bump wall.
pub const RING_SHUTTER_TICKS: f64 = 0.7;
/// Frames between replay checkpoints for the original comparison.
const REPLAY_INTERVAL: usize = 64;
/// The symbol window in which the tune first says "panic": read from P.P,
/// order 0A, row 0 first plays its instrument 44, a voice sample we hear as
/// the word, as window 128, set 3's first, begins (58.16 s). Until then the
/// pool style shows the rings as the original's black and white; over the
/// word they become the pool (authored).
pub const PANIC_WINDOW: usize = NEW_SET_FIRST_WINDOW + 2 * PATTERN_WINDOWS;
/// The word's length in seconds, over which the change runs: instrument
/// 44's 5,137 samples, played at E-4 from its 12,000 Hz, take about 0.34 s.
pub const PANIC_FADE: f64 = 0.34;
#[path = "rings_authored.rs"]
mod authored;
pub use authored::{
    AUTHORED_BOX, AuthoredSet, AuthoredShape, NEW_SET_FIRST_WINDOW, NEW_SETS, PATTERN_WINDOWS,
    authored_source, parse_authored, window_layer, window_set,
};
/// The original's symbol masks, symbol layers 0 to 15. rings_common.wgsl
/// repeats it.
pub const ORIGINAL_SYMBOLS: usize = 16;
/// Symbol layers: the original's 16 masks, then 16 for each new set (set
/// `k`, from 0, mask `n` at layer `16 + 16 k + n`). The first 16 are
/// [`RingsPreview::symbols`], the rest [`RingsPreview::authored`].
pub const SYMBOL_LAYERS: usize = ORIGINAL_SYMBOLS * (1 + NEW_SETS);

/// The concentric bands of M.DAT as an analytic model (an interpretation).
/// M.DAT holds 8 pre-shifted banks of 152 rows of 424 one-bit pixels: the
/// top half of a disc of 7-pixel bands, mirrored below by `step` (rows 152
/// and beyond read row `303 - r`). The model: a pixel is set where its
/// distance `r` from `centre` is at least `first` and `(r - first) / width`
/// has an even integer part. Fitted to bank 0 when the data loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingBands {
    /// Disc centre in M.DAT pixel coordinates, continuous (pixel `(u, r)`
    /// covers `[u, u + 1) × [r, r + 1)`). y is the mirror line, 152, read
    /// from the code; x is fitted.
    pub centre: [f64; 2],
    /// Width of one band, in pixels (a set and a clear band make a period).
    pub width: f64,
    /// Radius of the first edge, where the clear central disc ends.
    pub first: f64,
    /// Share of bank 0's pixels the model reproduces.
    pub matched: f64,
}

impl RingBands {
    /// Fit the model to bank 0 (152 rows of 424 bits, from
    /// `Rings::hd_source`). The centre column maximises left-right
    /// symmetry, the width is the mean spacing of the edges along the
    /// mirror row, and the first edge is the radius, in 1/64 pixel steps,
    /// that reproduces the most pixels.
    pub fn fit(bits: &[bool]) -> Result<Self, String> {
        const W: usize = 424;
        const H: usize = 152;
        if bits.len() != W * H {
            return Err("The ring source must be 424 × 152 pixels".into());
        }
        let at = |u: usize, r: usize| bits[r * W + u];
        // Mirror symmetry about x = c / 2: column u faces column c - 1 - u.
        let mut best = (0, 0);
        for c in W / 2..=3 * W / 2 {
            let score = (0..H)
                .flat_map(|r| (0..W).map(move |u| (u, r)))
                .filter(|&(u, r)| c > u && c - 1 - u < W && at(u, r) == at(c - 1 - u, r))
                .count();
            if score > best.0 {
                best = (score, c);
            }
        }
        let centre = [best.1 as f64 / 2.0, 152.0];
        // Edges along the last row, next to the mirror line.
        let edges: Vec<usize> = (1..W)
            .filter(|&u| at(u, H - 1) != at(u - 1, H - 1))
            .collect();
        if edges.len() < 4 {
            return Err("The ring source has too few band edges".into());
        }
        // The median spacing: the clear central disc spans less than two bands.
        let mut gaps: Vec<usize> = edges.windows(2).map(|e| e[1] - e[0]).collect();
        gaps.sort_unstable();
        let width = gaps[gaps.len() / 2] as f64;
        // Radius histogram of set and clear pixels, 1/64 pixel bins.
        let bins = |r: f64| (r * 64.0) as usize;
        let mut set = vec![0_u32; bins(600.0)];
        let mut clear = set.clone();
        for r in 0..H {
            for u in 0..W {
                let d = (u as f64 + 0.5 - centre[0]).hypot(r as f64 + 0.5 - centre[1]);
                if at(u, r) {
                    set[bins(d)] += 1;
                } else {
                    clear[bins(d)] += 1;
                }
            }
        }
        let model =
            |first: f64, r: f64| r >= first && ((r - first) / width).floor() as i64 % 2 == 0;
        let mut fit = (0, 0.0);
        for step in 0..(width * 64.0) as usize {
            let first = step as f64 / 64.0;
            let score: u32 = (0..set.len())
                .map(|b| {
                    let r = (b as f64 + 0.5) / 64.0;
                    if model(first, r) { set[b] } else { clear[b] }
                })
                .sum();
            if score > fit.0 {
                fit = (score, first);
            }
        }
        let matched = (0..H)
            .flat_map(|r| (0..W).map(move |u| (u, r)))
            .filter(|&(u, r)| {
                let d = (u as f64 + 0.5 - centre[0]).hypot(r as f64 + 0.5 - centre[1]);
                model(fit.1, d) == at(u, r)
            })
            .count() as f64
            / (W * H) as f64;
        Ok(Self {
            centre,
            width,
            first: fit.1,
            matched,
        })
    }

    /// Screen position (source pixels, continuous, y down) of a layer's
    /// ring centre for window offsets `(x, y)`. Screen column `c` of the
    /// window shows M.DAT column `x + c` (bank `x & 7`, from byte `x / 8 + 4`,
    /// holds the image shifted left by `x & 7`), and screen row `8 + k` shows
    /// M.DAT row `y + k`.
    pub fn screen_centre(&self, offset: [i32; 2]) -> [f64; 2] {
        [
            self.centre[0] - offset[0] as f64,
            self.centre[1] - offset[1] as f64 + 8.0,
        ]
    }
}

/// One part-6 frame as the scanout shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingsFrame {
    /// The scene's counters after the step that produced this frame.
    pub state: HdRings,
    /// The draw step whose rings this frame shows when the pages flip as
    /// intended: the step before the latest (one frame of latency). It holds
    /// still before 06:00 and during the picture, when nothing is drawn.
    pub shown_step: i64,
    /// Window offsets of that step, plane 0 then plane 1.
    pub offsets: [[i32; 2]; 2],
    /// The original shows an older page than `shown_step` here: when the
    /// page advances, the scanout can show a page last drawn a second or two
    /// earlier. The HD views don't reproduce this artefact.
    pub stale: bool,
}

/// A run of frames on which one symbol is lit, `[start, end)` in part-6
/// frames. The original lights it on even frames only; the window runs from
/// its first lit frame to the next symbol's first lit frame, or one frame
/// past its own last lit frame and the dark frame after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolWindow {
    pub mask: u8,
    pub start: usize,
    pub end: usize,
}

/// Group the lit mask of each frame into windows.
pub fn symbol_windows(lit: &[Option<u8>]) -> Vec<SymbolWindow> {
    let mut windows: Vec<SymbolWindow> = Vec::new();
    for (j, mask) in lit.iter().enumerate() {
        let Some(mask) = *mask else { continue };
        match windows.last_mut() {
            // Lit frames alternate with dark ones: a gap of one dark frame
            // keeps the window open.
            Some(w) if w.mask == mask && j <= w.end => w.end = j + 2,
            Some(w) => {
                if j <= w.end {
                    w.end = j;
                }
                windows.push(SymbolWindow {
                    mask,
                    start: j,
                    end: j + 2,
                });
            }
            None => windows.push(SymbolWindow {
                mask,
                start: j,
                end: j + 2,
            }),
        }
    }
    if let Some(last) = windows.last_mut() {
        last.end = last.end.min(lit.len());
    }
    windows
}

/// Catmull-Rom interpolation through samples `values` at a continuous index:
/// the exact value at every integer index, with continuous velocity.
/// Indices beyond either end take the end value.
pub fn catmull_rom(values: &[[f64; 2]], position: f64) -> [f64; 2] {
    let last = values.len() as i64 - 1;
    let position = position.clamp(0.0, last as f64);
    let i = position.floor();
    let t = position - i;
    let at = |k: i64| values[(i as i64 + k).clamp(0, last) as usize];
    let [a, b, c, d] = [at(-1), at(0), at(1), at(2)];
    let (t2, t3) = (t * t, t * t * t);
    std::array::from_fn(|k| {
        0.5 * (2.0 * b[k]
            + (c[k] - a[k]) * t
            + (2.0 * a[k] - 5.0 * b[k] + 4.0 * c[k] - d[k]) * t2
            + (3.0 * b[k] - a[k] - 3.0 * c[k] + d[k]) * t3)
    })
}

/// What a set of symbol fields keeps per layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolDetail {
    /// All four fields per atlas texel and a light map texel per source
    /// pixel: the original's 16 masks.
    Full,
    /// Only the tube's two fields, the centreline and free-end distances,
    /// and a light map texel per [`TUBE_LIGHT_SCALE`] × [`TUBE_LIGHT_SCALE`]
    /// pixels: the authored sets, which the neon alone draws (faithful HD
    /// shows the originals), so that their 112 layers take 86 MB rather
    /// than 202 in full detail.
    Tubes,
}

/// Source pixels per light map texel, each way, for [`SymbolDetail::Tubes`].
/// rings_common.wgsl repeats its log2 as TUBE_LIGHT_LOD.
pub const TUBE_LIGHT_SCALE: usize = 2;

impl SymbolDetail {
    /// Half floats per atlas texel.
    pub fn channels(self) -> usize {
        match self {
            Self::Full => 4,
            Self::Tubes => 2,
        }
    }

    /// Source pixels per light map texel, each way.
    pub fn light_scale(self) -> usize {
        match self {
            Self::Full => 1,
            Self::Tubes => TUBE_LIGHT_SCALE,
        }
    }

    /// The light map's top level, in texels: it covers [`LIGHT_SIZE`]
    /// pixels from [`LIGHT_ORIGIN`] whatever its scale.
    pub fn light_size(self) -> [u32; 2] {
        LIGHT_SIZE.map(|n| n / self.light_scale() as u32)
    }
}

/// Distance fields of symbols and their neon light map, built in memory at
/// load time, one layer per symbol: the original's 16 masks decoded locally
/// ([`SymbolFields::new`]), or the authored sets from their runs
/// ([`SymbolFields::authored`]). The smoothing, the centrelines and the light
/// map are interpretations or authored; nothing derived from the original
/// data is stored.
pub struct SymbolFields {
    /// Source-pixel rectangle the atlas covers: the masks' union and a
    /// margin of [`SYMBOL_PAD`] pixels. Origin, then size.
    pub origin: [i32; 2],
    pub size: [usize; 2],
    /// Layers, one per mask.
    pub layers: usize,
    pub detail: SymbolDetail,
    /// Half floats per texel, `SYMBOL_SCALE` texels per source pixel, layer
    /// after layer (one per mask). With [`SymbolDetail::Full`] four: red, the
    /// signed distance to the smoothed outline of the stroke pixels, positive
    /// inside; green, the distance to the stroke's centreline; blue, the
    /// stroke's half-width at the nearest centreline point; alpha, the
    /// distance to the nearest free end of a stroke's centreline (where a
    /// tube would end). With [`SymbolDetail::Tubes`] only green and alpha, as
    /// two. All in source rows on the 4:3 display (a column is 5/6 of a
    /// row), clamped to ±[`SYMBOL_LIMIT`].
    pub atlas: Vec<f16>,
    /// Neon light map: for each mask, the coverage of tubes of
    /// [`NEON_RADIUS`] around the centrelines over [`LIGHT_SIZE`] pixels from
    /// [`LIGHT_ORIGIN`], at the detail's [`SymbolDetail::light_size`], then a
    /// mip chain, each level the one before filtered by a [1 3 3 1] / 8
    /// kernel and halved. One half float per texel; each level holds every
    /// layer.
    pub light: Vec<Vec<f16>>,
}

impl SymbolFields {
    pub fn new(masks: &[Vec<bool>], detail: SymbolDetail) -> Result<Self, String> {
        if masks.is_empty() || masks.iter().any(|m| m.len() != 320 * 200) {
            return Err("Part 6 needs symbol masks of 320 × 200 pixels".into());
        }
        let mut lo = [320_i32, 200];
        let mut hi = [-1_i32, -1];
        for mask in masks {
            for (at, _) in mask.iter().enumerate().filter(|(_, set)| **set) {
                let p = [(at % 320) as i32, (at / 320) as i32];
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        if hi[0] < 0 {
            // No symbol pixels at all: a small empty field at the centre.
            (lo, hi) = ([150, 90], [170, 110]);
        }
        let pad = SYMBOL_PAD as i32;
        let origin = [lo[0] - pad, lo[1] - pad];
        let size = [
            (hi[0] - lo[0] + 1 + 2 * pad) as usize,
            (hi[1] - lo[1] + 1 + 2 * pad) as usize,
        ];
        Ok(Self::build(masks.len(), origin, size, detail, |i| {
            let mask = &masks[i];
            if !mask.contains(&true) {
                return empty_layer(size, detail);
            }
            let texels = symbol_fields(mask, origin, size);
            symbol_layer(&texels, origin, size, detail)
        }))
    }

    /// Make `count` layers over the rectangle from `origin` of `size`
    /// source pixels: a worker per core takes the next layer until none are
    /// left, then the layers are joined.
    fn build(
        count: usize,
        origin: [i32; 2],
        size: [usize; 2],
        detail: SymbolDetail,
        layer: impl Fn(usize) -> SymbolLayer + Sync,
    ) -> Self {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(count);
        let mut layers: Vec<Option<SymbolLayer>> = (0..count).map(|_| None).collect();
        std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut done = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= count {
                                return done;
                            }
                            done.push((i, layer(i)));
                        }
                    })
                })
                .collect();
            for job in jobs {
                for (i, layer) in job.join().unwrap() {
                    layers[i] = Some(layer);
                }
            }
        });
        // Join the layers, letting each go once copied.
        let (mut atlas, mut light) = (Vec::new(), Vec::<Vec<f16>>::new());
        for layer in layers.into_iter().map(Option::unwrap) {
            atlas.extend(layer.atlas);
            light.resize(layer.light.len(), Vec::new());
            for (level, texels) in light.iter_mut().zip(layer.light) {
                level.extend(texels);
            }
        }
        Self {
            origin,
            size,
            layers: count,
            detail,
            atlas,
            light,
        }
    }

    /// Atlas texels per layer, across and down.
    pub fn texels(&self) -> (u32, u32) {
        (
            (self.size[0] * SYMBOL_SCALE) as u32,
            (self.size[1] * SYMBOL_SCALE) as u32,
        )
    }

    /// Bytes the atlas and the light map hold (on the GPU, and here).
    pub fn bytes(&self) -> usize {
        (self.atlas.len() + self.light.iter().map(Vec::len).sum::<usize>()) * size_of::<f16>()
    }
}

/// One mask's atlas texels and light map levels, as half floats.
struct SymbolLayer {
    atlas: Vec<f16>,
    light: Vec<Vec<f16>>,
}

/// A layer from its atlas texels ([`symbol_fields`] for a mask, or an
/// authored drawing's runs): the channels the detail keeps, and the light
/// map at its scale with the mip chain down to 1 × 1.
fn symbol_layer(
    texels: &[[f32; 4]],
    origin: [i32; 2],
    size: [usize; 2],
    detail: SymbolDetail,
) -> SymbolLayer {
    let light = light_map(texels, origin, size, detail.light_scale());
    finish_layer(texels, light, detail)
}

/// The layer of a mask with nothing set, without tracing it: far from
/// everything, casting no light.
fn empty_layer(size: [usize; 2], detail: SymbolDetail) -> SymbolLayer {
    let [lw, lh] = detail.light_size().map(|n| n as usize);
    let count = size[0] * size[1] * SYMBOL_SCALE * SYMBOL_SCALE;
    let far = [-SYMBOL_LIMIT, SYMBOL_LIMIT, 0.0, SYMBOL_LIMIT];
    finish_layer(&vec![far; count], vec![0.0; lw * lh], detail)
}

/// Keep the detail's channels, as half floats, and build the light map's
/// mip chain.
fn finish_layer(texels: &[[f32; 4]], light: Vec<f32>, detail: SymbolDetail) -> SymbolLayer {
    let [lw, lh] = detail.light_size().map(|n| n as usize);
    let mut levels = vec![light];
    let mut extent = (lw, lh);
    while extent != (1, 1) {
        let next = ((extent.0 >> 1).max(1), (extent.1 >> 1).max(1));
        levels.push(downsample(levels.last().unwrap(), extent, next));
        extent = next;
    }
    let channels: &[usize] = match detail {
        SymbolDetail::Full => &[0, 1, 2, 3],
        SymbolDetail::Tubes => &[1, 3],
    };
    SymbolLayer {
        atlas: texels
            .iter()
            .flat_map(|t| channels.iter().map(|&c| f16::from_f32(t[c])))
            .collect(),
        light: levels
            .iter()
            .map(|level| level.iter().map(|&v| f16::from_f32(v)).collect())
            .collect(),
    }
}

/// The light map's top level for one mask: the coverage of its tubes, from
/// the atlas texels' centreline distances, averaged over each light texel of
/// `scale` × `scale` source pixels. Beyond the atlas there are no tubes.
fn light_map(texels: &[[f32; 4]], origin: [i32; 2], size: [usize; 2], scale: usize) -> Vec<f32> {
    let s = SYMBOL_SCALE;
    let (w, h) = (size[0], size[1]);
    let fw = w * s;
    // A texel is half a source row high: soften the tube's edge over that.
    let tube: Vec<f32> = texels
        .iter()
        .map(|t| ((NEON_RADIUS - t[1]) / 0.5 + 0.5).clamp(0.0, 1.0))
        .collect();
    let (lw, lh) = (
        LIGHT_SIZE[0] as usize / scale,
        LIGHT_SIZE[1] as usize / scale,
    );
    let mut light = vec![0.0; lw * lh];
    for (j, row) in light.chunks_exact_mut(lw).enumerate() {
        for (i, texel) in row.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (py, px) in (0..scale).flat_map(|b| (0..scale).map(move |a| (b, a))) {
                let y = (j * scale + py) as i32 + LIGHT_ORIGIN[1] - origin[1];
                let x = (i * scale + px) as i32 + LIGHT_ORIGIN[0] - origin[0];
                if y < 0 || y >= h as i32 || x < 0 || x >= w as i32 {
                    continue;
                }
                let (x, y) = (x as usize * s, y as usize * s);
                for dy in 0..s {
                    for dx in 0..s {
                        sum += tube[(y + dy) * fw + x + dx];
                    }
                }
            }
            *texel = sum / (s * s * scale * scale) as f32;
        }
    }
    light
}

/// One mask's atlas texels: the outline's signed distance, then the
/// distance to the redrawn tube runs, the stroke's half-width there and the
/// distance to the nearest free end.
fn symbol_fields(mask: &[bool], origin: [i32; 2], size: [usize; 2]) -> Vec<[f32; 4]> {
    let (w, h) = (size[0], size[1]);
    let crop: Vec<bool> = (0..w * h)
        .map(|at| {
            let (x, y) = ((at % w) as i32 + origin[0], (at / w) as i32 + origin[1]);
            (0..320).contains(&x) && (0..200).contains(&y) && mask[(y * 320 + x) as usize]
        })
        .collect();
    let s = SYMBOL_SCALE;
    let outline = smooth_signed_distance(&crop, w, h, s, 0, SYMBOL_SMOOTHING, SYMBOL_LIMIT);
    let (fw, fh) = (w * s, h * s);
    let mut skeleton: Vec<bool> = outline.iter().map(|&d| d > 0.0).collect();
    thin(&mut skeleton, fw, fh);
    prune(&mut skeleton, fw, fh, 3 * s);
    keep_isolated(&mut skeleton, &outline, fw, fh);
    let point = |i: usize| {
        let (x, y) = (i % fw, i / fw);
        [
            (x as f32 + 0.5) / s as f32 * PIXEL_ASPECT,
            (y as f32 + 0.5) / s as f32,
        ]
    };
    break_junctions(&mut skeleton, fw, fh, point);
    // Redraw each run as a clean curve, then measure distances to it.
    let runs: Vec<Run> = trace_runs(&skeleton, fw, fh)
        .into_iter()
        .map(|(texels, closed)| {
            let points: Vec<[f32; 2]> = texels.iter().map(|&i| point(i)).collect();
            redraw_run(&points, closed)
        })
        .collect();
    curve_fields(&runs, &outline, fw, fh, s)
}

/// Halve a light map level with a [1 3 3 1] / 8 filter each way; texels
/// beyond the edge are dark.
fn downsample(level: &[f32], (w, h): (usize, usize), (nw, nh): (usize, usize)) -> Vec<f32> {
    const K: [f32; 4] = [0.125, 0.375, 0.375, 0.125];
    let get = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            0.0
        } else {
            level[y as usize * w + x as usize]
        }
    };
    let mut out = vec![0.0; nw * nh];
    for j in 0..nh {
        for i in 0..nw {
            let mut sum = 0.0;
            for (b, ky) in K.iter().enumerate() {
                for (a, kx) in K.iter().enumerate() {
                    sum += kx * ky * get(2 * i as i64 - 1 + a as i64, 2 * j as i64 - 1 + b as i64);
                }
            }
            out[j * nw + i] = sum;
        }
    }
    out
}

/// Zhang and Suen's thinning ("A fast parallel algorithm for thinning
/// digital patterns", 1984): peel boundary texels in two alternating
/// sub-iterations until a one-texel skeleton remains. Border texels stay.
fn thin(mask: &mut [bool], w: usize, h: usize) {
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut remove = Vec::new();
            for y in 1..h.saturating_sub(1) {
                for x in 1..w.saturating_sub(1) {
                    if !mask[y * w + x] {
                        continue;
                    }
                    let n = neighbours(mask, w, x, y);
                    let count = n.iter().filter(|&&v| v).count();
                    let rises = (0..8).filter(|&i| !n[i] && n[(i + 1) % 8]).count();
                    let (p2, p4, p6, p8) = (n[0], n[2], n[4], n[6]);
                    let keep = if pass == 0 {
                        p4 && p6 && (p2 || p8)
                    } else {
                        p2 && p8 && (p4 || p6)
                    };
                    if (2..=6).contains(&count) && rises == 1 && !keep {
                        remove.push(y * w + x);
                    }
                }
            }
            changed |= !remove.is_empty();
            for at in remove {
                mask[at] = false;
            }
        }
        if !changed {
            return;
        }
    }
}

/// The eight neighbours clockwise from north; outside the grid is clear.
fn neighbours(mask: &[bool], w: usize, x: usize, y: usize) -> [bool; 8] {
    let h = mask.len() / w;
    let at = |dx: i64, dy: i64| {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 && mask[ny as usize * w + nx as usize]
    };
    [
        at(0, -1),
        at(1, -1),
        at(1, 0),
        at(1, 1),
        at(0, 1),
        at(-1, 1),
        at(-1, 0),
        at(-1, -1),
    ]
}

/// Separate groups of set texels around a texel: 1 at a stroke's end, 2
/// along it, 3 or more at a junction. Unlike a count of neighbours, this
/// treats a staircase step (an edge and a corner neighbour together) as one.
fn branches(mask: &[bool], w: usize, at: usize) -> usize {
    let n = neighbours(mask, w, at % w, at / w);
    (0..8).filter(|&i| !n[i] && n[(i + 1) % 8]).count()
}

/// Remove spurs: branches of at most `length` texels from an end to a
/// junction. Thinning leaves them where a stroke's outline has a corner.
fn prune(skeleton: &mut [bool], w: usize, h: usize, length: usize) {
    let ends: Vec<usize> = (0..w * h)
        .filter(|&at| skeleton[at] && branches(skeleton, w, at) == 1)
        .collect();
    for end in ends {
        if !skeleton[end] || branches(skeleton, w, end) != 1 {
            continue;
        }
        let (mut path, mut junction) = (vec![end], false);
        while path.len() <= length {
            let at = *path.last().unwrap();
            let (x, y) = (at % w, at / w);
            // Follow edge neighbours before corner ones.
            let next = [
                (0, -1),
                (1, 0),
                (0, 1),
                (-1, 0),
                (1, -1),
                (1, 1),
                (-1, 1),
                (-1, -1),
            ]
            .into_iter()
            .map(|(dx, dy)| (x as i64 + dx, y as i64 + dy))
            .filter(|&(nx, ny)| nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64)
            .map(|(nx, ny)| ny as usize * w + nx as usize)
            .find(|&n| skeleton[n] && !path.contains(&n));
            let Some(next) = next else { break };
            if branches(skeleton, w, next) >= 3 {
                junction = true;
                break;
            }
            path.push(next);
        }
        if junction && path.len() <= length {
            for at in path {
                skeleton[at] = false;
            }
        }
    }
}

/// One tube run, redrawn: dense points along it in rows, whether it closes,
/// and its free ends (open runs end in two; a dot is a single point).
struct Run {
    points: Vec<[f32; 2]>,
    closed: bool,
}

/// Follow the skeleton into runs of texels between ends and junctions, and
/// closed loops. Each run is ordered along the stroke.
fn trace_runs(skeleton: &[bool], w: usize, h: usize) -> Vec<(Vec<usize>, bool)> {
    let node = |i: usize| branches(skeleton, w, i) != 2;
    let mut visited = vec![false; w * h];
    let mut runs = Vec::new();
    // Edge neighbours first, so a staircase is followed step by step.
    let next = |at: usize, visited: &[bool], path: &[usize]| -> Option<usize> {
        let mut candidates = skeleton_neighbours(skeleton, w, at);
        candidates.retain(|&m| !visited[m] && !path.contains(&m));
        let (x, y) = (at % w, at / w);
        candidates.sort_by_key(|&m| (m % w != x) as u8 + (m / w != y) as u8);
        candidates.first().copied()
    };
    let mut joined = std::collections::HashSet::new();
    for n in (0..w * h).filter(|&i| skeleton[i] && node(i)) {
        let mut around = skeleton_neighbours(skeleton, w, n);
        if around.is_empty() {
            runs.push((vec![n], false));
            continue;
        }
        let (x, y) = (n % w, n / w);
        around.sort_by_key(|&m| (m % w != x) as u8 + (m / w != y) as u8);
        for first in around {
            if visited[first] {
                continue;
            }
            if node(first) {
                // Two nodes side by side: one short run, once.
                if joined.insert((n.min(first), n.max(first))) {
                    runs.push((vec![n, first], false));
                }
                continue;
            }
            visited[first] = true;
            let mut path = vec![n, first];
            let mut at = first;
            while let Some(m) = next(at, &visited, &path) {
                path.push(m);
                if node(m) {
                    break;
                }
                visited[m] = true;
                at = m;
            }
            runs.push((path, false));
        }
    }
    // What is left are loops.
    for start in 0..w * h {
        if !skeleton[start] || visited[start] || node(start) {
            continue;
        }
        visited[start] = true;
        let mut path = vec![start];
        let mut at = start;
        while let Some(m) = next(at, &visited, &path) {
            visited[m] = true;
            path.push(m);
            at = m;
        }
        runs.push((path, true));
    }
    runs
}

/// Simplify a polyline to its key points (Ramer and Douglas-Peucker),
/// keeping both ends.
fn key_points(points: &[[f32; 2]], tolerance: f32) -> Vec<[f32; 2]> {
    fn split(points: &[[f32; 2]], tolerance: f32, out: &mut Vec<[f32; 2]>) {
        let (a, b) = (points[0], points[points.len() - 1]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let length = ab[0].hypot(ab[1]);
        let mut far = (0.0, 0);
        for (k, p) in points
            .iter()
            .enumerate()
            .skip(1)
            .take(points.len().saturating_sub(2))
        {
            let d = if length < 1e-6 {
                (p[0] - a[0]).hypot(p[1] - a[1])
            } else {
                ((p[0] - a[0]) * ab[1] - (p[1] - a[1]) * ab[0]).abs() / length
            };
            if d > far.0 {
                far = (d, k);
            }
        }
        if far.0 > tolerance {
            split(&points[..=far.1], tolerance, out);
            out.pop();
            split(&points[far.1..], tolerance, out);
        } else {
            out.push(a);
            out.push(b);
        }
    }
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out = Vec::new();
    split(points, tolerance, &mut out);
    out
}

/// Centripetal Catmull-Rom through `keys`, sampled about every quarter row,
/// with ends continuing their first and last chords.
fn spline(keys: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let n = keys.len();
    let at = |i: i64| -> [f32; 2] {
        if i < 0 {
            [2.0 * keys[0][0] - keys[1][0], 2.0 * keys[0][1] - keys[1][1]]
        } else if i as usize >= n {
            [
                2.0 * keys[n - 1][0] - keys[n - 2][0],
                2.0 * keys[n - 1][1] - keys[n - 2][1],
            ]
        } else {
            keys[i as usize]
        }
    };
    let mut out = vec![keys[0]];
    for k in 0..n - 1 {
        let p = [
            at(k as i64 - 1),
            at(k as i64),
            at(k as i64 + 1),
            at(k as i64 + 2),
        ];
        let knot = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]).sqrt().max(1e-4);
        let t1 = knot(p[0], p[1]);
        let t2 = t1 + knot(p[1], p[2]);
        let t3 = t2 + knot(p[2], p[3]);
        let length = (p[2][0] - p[1][0]).hypot(p[2][1] - p[1][1]);
        let steps = ((length / 0.25).ceil() as usize).max(1);
        for step in 1..=steps {
            let t = t1 + (t2 - t1) * step as f32 / steps as f32;
            let lerp = |a: [f32; 2], b: [f32; 2], ta: f32, tb: f32| {
                let f = (t - ta) / (tb - ta);
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
            };
            let a1 = lerp(p[0], p[1], 0.0, t1);
            let a2 = lerp(p[1], p[2], t1, t2);
            let a3 = lerp(p[2], p[3], t2, t3);
            let b1 = lerp(a1, a2, 0.0, t2);
            let b2 = lerp(a2, a3, t1, t3);
            out.push(lerp(b1, b2, t1, t2));
        }
    }
    out
}

/// Least-squares circle through points (Kasa's fit): centre, radius and
/// the RMS distance of the points from it.
fn fit_circle(points: &[[f32; 2]]) -> Option<([f32; 2], f32, f32)> {
    let m = points.len() as f32;
    if points.len() < 3 {
        return None;
    }
    let c = points
        .iter()
        .fold([0.0, 0.0], |c, p| [c[0] + p[0] / m, c[1] + p[1] / m]);
    let (mut suu, mut svv, mut suv, mut suuu, mut svvv, mut suvv, mut svuu) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in points {
        let (u, v) = (p[0] - c[0], p[1] - c[1]);
        suu += u * u;
        svv += v * v;
        suv += u * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-6 {
        return None;
    }
    let (r1, r2) = (0.5 * (suuu + suvv), 0.5 * (svvv + svuu));
    let uc = (r1 * svv - r2 * suv) / det;
    let vc = (r2 * suu - r1 * suv) / det;
    let centre = [c[0] + uc, c[1] + vc];
    let radius = (uc * uc + vc * vc + (suu + svv) / m).sqrt();
    let rms = (points
        .iter()
        .map(|p| ((p[0] - centre[0]).hypot(p[1] - centre[1]) - radius).powi(2))
        .sum::<f32>()
        / m)
        .sqrt();
    Some((centre, radius, rms))
}

/// A circle fits points closely enough to draw them as one: within a
/// twentieth of its radius, or half a row.
fn round_enough(radius: f32, rms: f32) -> bool {
    rms < (0.05 * radius).max(0.5)
}

/// The arc of a circle from `points`' first to its last point, passing its
/// middle one, sampled about every quarter row.
fn arc(points: &[[f32; 2]], centre: [f32; 2], radius: f32) -> Vec<[f32; 2]> {
    let angle = |p: [f32; 2]| (p[1] - centre[1]).atan2(p[0] - centre[0]);
    let wrap = |a: f32| {
        let t = std::f32::consts::TAU;
        a - t * ((a + std::f32::consts::PI) / t).floor()
    };
    let a0 = angle(points[0]);
    let sweep_to = wrap(angle(points[points.len() - 1]) - a0);
    let middle = wrap(angle(points[points.len() / 2]) - a0);
    let sweep = if middle * sweep_to > 0.0 && middle.abs() < sweep_to.abs() {
        sweep_to
    } else {
        sweep_to - sweep_to.signum() * std::f32::consts::TAU
    };
    let steps = ((sweep.abs() * radius / 0.25).ceil() as usize).max(2);
    (0..=steps)
        .map(|k| {
            let a = a0 + sweep * k as f32 / steps as f32;
            [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()]
        })
        .collect()
}

/// Turns sharper than this (radians) are corners; gentler ones are curves.
const CORNER: f32 = 0.7;
/// How far each side of a corner its bend begins, in rows (authored: about
/// a bent glass tube's radius of bend).
const BEND: f32 = 2.5;
/// Ramer-Douglas-Peucker tolerance for the key points, in rows: wobbles
/// smaller than this in the original strokes go.
const WOBBLE: f32 = 1.1;
/// An open run's end stretch shorter than this, in rows, beyond a sharp
/// corner is dropped as an overshoot.
const HOOK: f32 = 4.0;
/// A stretch between corners that strays less than this from its chord, in
/// rows (or a twentieth of its length), is drawn straight.
const STRAIGHT: f32 = 2.0;
/// The coarser key-point tolerance for freehand curves, in rows.
const FREEHAND: f32 = 1.8;
/// The gap, in rows, where a loop's two ends turn into the backing board:
/// both ends' glass and a stroke's half-width of air.
const RETURN: f32 = 2.0 * NEON_RADIUS + NEON_STROKE;

/// Redraw a traced run as a clean curve (authored; the neon symbols are
/// recreated from the originals rather than traced): a closed run that is
/// nearly a circle becomes one; otherwise its key points are found, and each
/// stretch between sharp turns becomes a straight line if it nearly is one,
/// an exact arc if a circle fits it, or else a smooth spline; each sharp
/// turn becomes a short bend tangent to both sides.
fn redraw_run(points: &[[f32; 2]], closed: bool) -> Run {
    let run = redraw_shape(points, closed);
    if run.closed { open_loop(run) } else { run }
}

/// Neon can't close a loop: a tube's two ends turn back into the backing
/// board. Open a loop with a gap of RETURN rows at its lowest point (largest
/// y), where a sign maker would hide the return.
fn open_loop(run: Run) -> Run {
    let points = run.points;
    let n = points.len();
    if n < 8 {
        return Run {
            points,
            closed: false,
        };
    }
    let lowest = (0..n)
        .max_by(|&a, &b| points[a][1].total_cmp(&points[b][1]))
        .unwrap();
    let mut ring: Vec<[f32; 2]> = (0..n).map(|k| points[(lowest + k) % n]).collect();
    ring.push(ring[0]);
    // Walk half the gap from each side of the lowest point.
    let mut start = 0;
    let mut travelled = 0.0;
    while start + 1 < ring.len() && travelled < RETURN * 0.5 {
        travelled +=
            (ring[start + 1][0] - ring[start][0]).hypot(ring[start + 1][1] - ring[start][1]);
        start += 1;
    }
    let mut end = ring.len() - 1;
    travelled = 0.0;
    while end > start + 2 && travelled < RETURN * 0.5 {
        travelled += (ring[end][0] - ring[end - 1][0]).hypot(ring[end][1] - ring[end - 1][1]);
        end -= 1;
    }
    Run {
        points: ring[start..=end].to_vec(),
        closed: false,
    }
}

/// The run's shape: see [`redraw_run`].
fn redraw_shape(points: &[[f32; 2]], closed: bool) -> Run {
    let n = points.len();
    if n < 3 {
        return Run {
            points: points.to_vec(),
            closed: false,
        };
    }
    // Remove the texel staircase before looking for key points.
    let smooth: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let reach = if closed { 3 } else { 3.min(i).min(n - 1 - i) };
            let (mut sum, mut weight) = ([0.0, 0.0], 0.0);
            for k in -(reach as i64)..=reach as i64 {
                let j = (i as i64 + k).rem_euclid(n as i64) as usize;
                let g = (-(k * k) as f32 / 4.5).exp();
                sum = [sum[0] + points[j][0] * g, sum[1] + points[j][1] * g];
                weight += g;
            }
            [sum[0] / weight, sum[1] / weight]
        })
        .collect();
    if closed
        && let Some((centre, radius, rms)) = fit_circle(&smooth)
        && rms < (0.08 * radius).max(0.6)
    {
        // Nearly a circle: make it one.
        let steps = ((std::f32::consts::TAU * radius / 0.25).ceil() as usize).max(12);
        return Run {
            points: (0..steps)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / steps as f32;
                    [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()]
                })
                .collect(),
            closed: true,
        };
    }
    // Key points. A loop is opened at its sharpest key point, or anywhere.
    let mut keys = if closed {
        let mut ring = smooth.clone();
        ring.push(smooth[0]);
        let mut keys = key_points(&ring, WOBBLE);
        keys.pop();
        keys
    } else {
        key_points(&smooth, WOBBLE)
    };
    let turn = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
        let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1])).max(1e-6);
        cos.clamp(-1.0, 1.0).acos()
    };
    let k = keys.len();
    if closed {
        if k < 3 {
            return Run {
                points: smooth,
                closed: true,
            };
        }
        let sharpest = (0..k)
            .max_by(|&a, &b| {
                let t = |i: usize| turn(keys[(i + k - 1) % k], keys[i], keys[(i + 1) % k]);
                t(a).total_cmp(&t(b))
            })
            .unwrap();
        keys.rotate_left(sharpest);
        keys.push(keys[0]);
    }
    // Pieces between corners, then the bends that join them.
    let corner =
        |i: usize| i > 0 && i + 1 < keys.len() && turn(keys[i - 1], keys[i], keys[i + 1]) > CORNER;
    let loop_corner = closed && turn(keys[keys.len() - 2], keys[0], keys[1]) > CORNER;
    let mut pieces: Vec<Vec<[f32; 2]>> = vec![vec![keys[0]]];
    for (i, &key) in keys.iter().enumerate().skip(1) {
        pieces.last_mut().unwrap().push(key);
        if corner(i) {
            pieces.push(vec![key]);
        }
    }
    // An open run's short first or last stretch beyond a sharp corner is an
    // overshoot of the hand-drawn stroke, not a bend worth a tube: drop it.
    if !closed {
        let length = |piece: &Vec<[f32; 2]>| {
            piece
                .windows(2)
                .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
                .sum::<f32>()
        };
        while pieces.len() > 1 && length(&pieces[0]) < HOOK {
            pieces.remove(0);
        }
        while pieces.len() > 1 && length(&pieces[pieces.len() - 1]) < HOOK {
            pieces.pop();
        }
    }
    // A stretch that strays from its chord by less than STRAIGHT rows, or
    // than a twentieth of its length, was meant straight: draw it so. Real
    // curves stray much further (a quarter circle by a fifth of its length).
    let straight = |piece: &Vec<[f32; 2]>| {
        let (a, b) = (piece[0], piece[piece.len() - 1]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let length = ab[0].hypot(ab[1]);
        length > 1e-3
            && piece.iter().all(|p| {
                ((p[0] - a[0]) * ab[1] - (p[1] - a[1]) * ab[0]).abs() / length
                    < STRAIGHT.max(0.05 * length)
            })
    };
    // A curved stretch that a circle fits becomes an exact arc of it; the
    // rest, freehand curves, stay smooth splines through their key points.
    let mut curves: Vec<Vec<[f32; 2]>> = pieces
        .iter()
        .map(|p| {
            if p.len() > 2 && straight(p) {
                return spline(&[p[0], p[p.len() - 1]]);
            }
            let curve = spline(p);
            match fit_circle(&curve) {
                Some((centre, radius, rms)) if p.len() > 2 && round_enough(radius, rms) => {
                    arc(&curve, centre, radius)
                }
                // Freehand: fewer key points again, for one smooth sweep.
                _ if p.len() > 3 => spline(&key_points(&curve, FREEHAND)),
                _ => curve,
            }
        })
        .collect();
    if closed && !loop_corner && curves.len() > 1 {
        // The loop's start was no corner: join its first and last pieces.
        let first = curves.remove(0);
        curves.last_mut().unwrap().extend(first.into_iter().skip(1));
    }
    let along = |curve: &[[f32; 2]]| -> Vec<f32> {
        let mut total = 0.0;
        let mut out = vec![0.0];
        for pair in curve.windows(2) {
            total += (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            out.push(total);
        }
        out
    };
    // Trim each piece back from its corner ends and join with a bend.
    let corners = curves.len()
        - if closed && (loop_corner || curves.len() == 1) {
            0
        } else {
            1
        };
    let mut out: Vec<[f32; 2]> = Vec::new();
    let trimmed: Vec<Vec<[f32; 2]>> = curves
        .iter()
        .enumerate()
        .map(|(i, curve)| {
            let s = along(curve);
            let length = *s.last().unwrap();
            let cut = BEND.min(0.4 * length);
            let start_cut = if i > 0 || (closed && loop_corner) {
                cut
            } else {
                0.0
            };
            let end_cut = if i + 1 < curves.len() || (closed && loop_corner) {
                cut
            } else {
                0.0
            };
            curve
                .iter()
                .zip(&s)
                .filter(|(_, d)| **d >= start_cut && **d <= length - end_cut)
                .map(|(p, _)| *p)
                .collect()
        })
        .collect();
    for i in 0..trimmed.len() {
        if trimmed[i].is_empty() {
            continue;
        }
        out.extend(&trimmed[i]);
        let j = (i + 1) % trimmed.len();
        if (i + 1 < trimmed.len() || corners == trimmed.len())
            && !trimmed[j].is_empty()
            && (i + 1 < trimmed.len() || closed)
        {
            // A quadratic bend from this piece's end to the next one's start,
            // with the corner as its control point.
            let (a, b) = (*trimmed[i].last().unwrap(), trimmed[j][0]);
            let c = curves[i][curves[i].len() - 1];
            let steps = 16;
            for step in 1..steps {
                let t = step as f32 / steps as f32;
                let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                out.push([
                    u * a[0] + v * c[0] + w * b[0],
                    u * a[1] + v * c[1] + w * b[1],
                ]);
            }
        }
    }
    if out.is_empty() {
        out = smooth;
    }
    Run {
        points: out,
        closed,
    }
}

/// Atlas texels for the redrawn runs: the outline's signed distance as
/// given, the exact distance to the nearest run, the original stroke's
/// half-width there and the distance to the nearest free end of an open run,
/// all clamped to SYMBOL_LIMIT.
fn curve_fields(
    runs: &[Run],
    outline: &[f32],
    fw: usize,
    fh: usize,
    scale: usize,
) -> Vec<[f32; 4]> {
    let (best, nearest, ends) = run_distances(runs, fw, fh, scale);
    let texel_of = |p: [f32; 2]| {
        [
            p[0] / PIXEL_ASPECT * scale as f32 - 0.5,
            p[1] * scale as f32 - 0.5,
        ]
    };
    (0..fw * fh)
        .map(|i| {
            let half = if nearest[i][0].is_nan() {
                0.0
            } else {
                let t = texel_of(nearest[i]);
                let (x, y) = (
                    (t[0].round().max(0.0) as usize).min(fw - 1),
                    (t[1].round().max(0.0) as usize).min(fh - 1),
                );
                outline[y * fw + x].max(0.0)
            };
            [outline[i], best[i], half, ends[i]]
        })
        .collect()
}

/// For each atlas texel (`fw` × `fh`, `scale` to a source pixel, positions
/// in rows from the atlas origin): the exact distance to the nearest run,
/// the nearest point on it (NaN beyond SYMBOL_LIMIT) and the distance to the
/// nearest free end of an open run, the distances clamped to SYMBOL_LIMIT.
fn run_distances(
    runs: &[Run],
    fw: usize,
    fh: usize,
    scale: usize,
) -> (Vec<f32>, Vec<[f32; 2]>, Vec<f32>) {
    let mut best = vec![SYMBOL_LIMIT; fw * fh];
    let mut nearest = vec![[f32::NAN; 2]; fw * fh];
    let mut ends = vec![SYMBOL_LIMIT; fw * fh];
    let texel_of = |p: [f32; 2]| {
        [
            p[0] / PIXEL_ASPECT * scale as f32 - 0.5,
            p[1] * scale as f32 - 0.5,
        ]
    };
    let centre = |x: usize, y: usize| {
        [
            (x as f32 + 0.5) / scale as f32 * PIXEL_ASPECT,
            (y as f32 + 0.5) / scale as f32,
        ]
    };
    // Every texel within SYMBOL_LIMIT of a segment, or of a point.
    let visit = |a: [f32; 2], b: [f32; 2], update: &mut dyn FnMut(usize, [f32; 2], f32)| {
        let (ta, tb) = (texel_of(a), texel_of(b));
        let reach = [
            SYMBOL_LIMIT / PIXEL_ASPECT * scale as f32,
            SYMBOL_LIMIT * scale as f32,
        ];
        let x0 = (ta[0].min(tb[0]) - reach[0]).floor().max(0.0) as usize;
        let x1 = ((ta[0].max(tb[0]) + reach[0]).ceil() as usize).min(fw - 1);
        let y0 = (ta[1].min(tb[1]) - reach[1]).floor().max(0.0) as usize;
        let y1 = ((ta[1].max(tb[1]) + reach[1]).ceil() as usize).min(fh - 1);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let ll = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = centre(x, y);
                let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / ll).clamp(0.0, 1.0);
                let q = [a[0] + ab[0] * t, a[1] + ab[1] * t];
                update(y * fw + x, q, (p[0] - q[0]).hypot(p[1] - q[1]));
            }
        }
    };
    for run in runs {
        let points = &run.points;
        let segments = if run.closed {
            points.len()
        } else {
            points.len().saturating_sub(1).max(1)
        };
        for k in 0..segments {
            let a = points[k % points.len()];
            let b = points
                [(k + 1).min(if run.closed { k + 1 } else { points.len() - 1 }) % points.len()];
            visit(a, b, &mut |i, q, d| {
                if d < best[i] {
                    best[i] = d;
                    nearest[i] = q;
                }
            });
        }
        if !run.closed {
            for end in [points[0], points[points.len() - 1]] {
                visit(end, end, &mut |i, _, d| ends[i] = ends[i].min(d));
            }
        }
    }
    (best, nearest, ends)
}

/// Set skeleton texels next to texel `at`, 8-connected.
fn skeleton_neighbours(skeleton: &[bool], w: usize, at: usize) -> Vec<usize> {
    let h = skeleton.len() / w;
    let (x, y) = ((at % w) as i64, (at / w) as i64);
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (nx, ny) = (x + dx, y + dy);
            if (dx, dy) != (0, 0) && nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                let n = ny as usize * w + nx as usize;
                if skeleton[n] {
                    out.push(n);
                }
            }
        }
    }
    out
}

/// Neon tubes are single runs, bent but never branching, so where strokes
/// meet or cross a sign has a break (authored, after the owner's direction
/// and photographs of neon signs). At each junction of the centrelines (junction texels within
/// three rows of each other count as one, as thinning splits a crossing into
/// two forks joined by a short bar), the two branches that continue each
/// other most nearly straight stay one run through it, and the others stop
/// short of that run's glass, with a gap; fragments left shorter than a
/// tube's width are removed. `point` gives texel positions in rows.
fn break_junctions(skeleton: &mut [bool], w: usize, h: usize, point: impl Fn(usize) -> [f32; 2]) {
    let distance = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let before = skeleton_parts(skeleton, w);
    let junctions: Vec<usize> = (0..w * h)
        .filter(|&i| skeleton[i] && branches(skeleton, w, i) >= 3)
        .collect();
    // Group junction texels within three rows of each other.
    let mut group: Vec<usize> = (0..junctions.len()).collect();
    fn root(group: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while group[r] != r {
            r = group[r];
        }
        group[i] = r;
        r
    }
    for a in 0..junctions.len() {
        for b in a + 1..junctions.len() {
            if distance(point(junctions[a]), point(junctions[b])) <= 3.0 {
                let (ra, rb) = (root(&mut group, a), root(&mut group, b));
                group[ra] = rb;
            }
        }
    }
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut index = std::collections::HashMap::new();
    for (i, &junction) in junctions.iter().enumerate() {
        let r = root(&mut group, i);
        let k = *index.entry(r).or_insert_with(|| {
            clusters.push(Vec::new());
            clusters.len() - 1
        });
        clusters[k].push(junction);
    }
    for cluster in clusters {
        if !cluster.iter().all(|&j| skeleton[j]) {
            continue;
        }
        let n = cluster.len() as f32;
        let centre = cluster.iter().fold([0.0, 0.0], |c, &j| {
            let p = point(j);
            [c[0] + p[0] / n, c[1] + p[1] / n]
        });
        let core = cluster
            .iter()
            .map(|&j| distance(point(j), centre))
            .fold(0.0_f32, f32::max)
            + 1.0;
        let in_core = |i: usize| distance(point(i), centre) <= core;
        // Exits: the first texels outside the core on each branch.
        let mut exits: Vec<usize> = Vec::new();
        let near: Vec<usize> = {
            let (cx, cy) = (centre[0] / PIXEL_ASPECT, centre[1]);
            let reach = (core + 2.0) * SYMBOL_SCALE as f32 * 1.3;
            let (x0, x1) = (
                ((cx * SYMBOL_SCALE as f32 - reach).max(0.0)) as usize,
                ((cx * SYMBOL_SCALE as f32 + reach) as usize).min(w - 1),
            );
            let (y0, y1) = (
                ((cy * SYMBOL_SCALE as f32 - reach).max(0.0)) as usize,
                ((cy * SYMBOL_SCALE as f32 + reach) as usize).min(h - 1),
            );
            (y0..=y1)
                .flat_map(|y| (x0..=x1).map(move |x| y * w + x))
                .filter(|&i| skeleton[i])
                .collect()
        };
        for &i in &near {
            if in_core(i)
                || exits
                    .iter()
                    .any(|&e| skeleton_neighbours(skeleton, w, e).contains(&i))
            {
                continue;
            }
            if skeleton_neighbours(skeleton, w, i)
                .iter()
                .any(|&m| in_core(m))
            {
                exits.push(i);
            }
        }
        if exits.len() < 3 {
            continue;
        }
        // Each branch's direction, about four rows out from the centre.
        let walk = |skeleton: &[bool], start: usize, until: f32| -> Vec<usize> {
            let mut path = vec![start];
            loop {
                let at = *path.last().unwrap();
                if distance(point(at), centre) >= until {
                    return path;
                }
                let next = skeleton_neighbours(skeleton, w, at)
                    .into_iter()
                    .filter(|&m| !in_core(m) && !path.contains(&m))
                    .max_by(|&a, &b| {
                        distance(point(a), centre).total_cmp(&distance(point(b), centre))
                    });
                match next {
                    Some(m) if distance(point(m), centre) > distance(point(at), centre) - 0.25 => {
                        path.push(m)
                    }
                    _ => return path,
                }
            }
        };
        let directions: Vec<[f32; 2]> = exits
            .iter()
            .map(|&e| {
                let end = point(*walk(skeleton, e, core + 4.0).last().unwrap());
                let d = [end[0] - centre[0], end[1] - centre[1]];
                let l = d[0].hypot(d[1]).max(1e-5);
                [d[0] / l, d[1] / l]
            })
            .collect();
        let mut pair = (0, 1);
        let mut straightest = f32::INFINITY;
        for a in 0..exits.len() {
            for b in a + 1..exits.len() {
                let dot = directions[a][0] * directions[b][0] + directions[a][1] * directions[b][1];
                if dot < straightest {
                    straightest = dot;
                    pair = (a, b);
                }
            }
        }
        // The run through: the shortest path between its two exits within
        // the core. The rest of the core goes.
        let (from, to) = (exits[pair.0], exits[pair.1]);
        let mut previous = std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::from([from]);
        previous.insert(from, from);
        while let Some(at) = queue.pop_front() {
            if at == to {
                break;
            }
            for m in skeleton_neighbours(skeleton, w, at) {
                if (in_core(m) || m == to) && !previous.contains_key(&m) {
                    previous.insert(m, at);
                    queue.push_back(m);
                }
            }
        }
        if !previous.contains_key(&to) {
            continue;
        }
        let mut keep = vec![to];
        while *keep.last().unwrap() != from {
            keep.push(previous[keep.last().unwrap()]);
        }
        for &i in &near {
            if in_core(i) && !keep.contains(&i) {
                skeleton[i] = false;
            }
        }
        let through = [
            directions[pair.0][0] - directions[pair.1][0],
            directions[pair.0][1] - directions[pair.1][1],
        ];
        let tl = through[0].hypot(through[1]).max(1e-5);
        for (k, &exit) in exits.iter().enumerate() {
            if k == pair.0 || k == pair.1 {
                continue;
            }
            // Clear the through run's glass, the cut tube's own rounded end
            // and a stroke's half-width of air between them, measured across
            // the run: further along an oblique branch.
            let d = directions[k];
            let sin = ((d[0] * through[1] - d[1] * through[0]) / tl)
                .abs()
                .max(0.5);
            let gap = (2.0 * NEON_RADIUS + NEON_STROKE) / sin;
            let cut = walk(skeleton, exit, gap);
            for &i in &cut {
                if branches(skeleton, w, i) < 3 {
                    skeleton[i] = false;
                }
            }
            // Drop what is left of the branch if it is no longer than two
            // strokes' widths.
            if let Some(&last) = cut.last()
                && let Some(&rest) = skeleton_neighbours(skeleton, w, last).first()
            {
                let mut piece = vec![rest];
                let mut seen = std::collections::HashSet::from([rest]);
                let limit = (4.0 * NEON_STROKE * SYMBOL_SCALE as f32) as usize;
                let mut i = 0;
                while i < piece.len() && piece.len() <= limit {
                    for m in skeleton_neighbours(skeleton, w, piece[i]) {
                        if seen.insert(m) {
                            piece.push(m);
                        }
                    }
                    i += 1;
                }
                if piece.len() <= limit {
                    for i in piece {
                        skeleton[i] = false;
                    }
                }
            }
        }
    }
    // Fragments the cuts left shorter than a stroke's width go; parts of the
    // shape that were that small to begin with (dots) stay.
    let after = skeleton_parts(skeleton, w);
    let short = (2.0 * NEON_STROKE * SYMBOL_SCALE as f32) as usize;
    let mut size = std::collections::HashMap::new();
    for part in after.iter().flatten() {
        *size.entry(*part).or_insert(0_usize) += 1;
    }
    let mut whole = std::collections::HashMap::new();
    for part in before.iter().flatten() {
        *whole.entry(*part).or_insert(0_usize) += 1;
    }
    for i in 0..w * h {
        if let (Some(a), Some(b)) = (after[i], before[i])
            && size[&a] < short
            && whole[&b] > size[&a]
        {
            skeleton[i] = false;
        }
    }
}

/// The 8-connected parts of a skeleton: a label per set texel.
fn skeleton_parts(skeleton: &[bool], w: usize) -> Vec<Option<usize>> {
    let mut label = vec![None; skeleton.len()];
    let mut next = 0;
    for start in 0..skeleton.len() {
        if !skeleton[start] || label[start].is_some() {
            continue;
        }
        label[start] = Some(next);
        let mut stack = vec![start];
        while let Some(at) = stack.pop() {
            for m in skeleton_neighbours(skeleton, w, at) {
                if label[m].is_none() {
                    label[m] = Some(next);
                    stack.push(m);
                }
            }
        }
        next += 1;
    }
    label
}

/// Thinning can erase a small blob entirely; give each part of the shape
/// with no skeleton left its deepest texel.
fn keep_isolated(skeleton: &mut [bool], outline: &[f32], w: usize, h: usize) {
    let mut seen = vec![false; w * h];
    for start in 0..w * h {
        if seen[start] || outline[start] <= 0.0 {
            continue;
        }
        let (mut stack, mut part) = (vec![start], Vec::new());
        seen[start] = true;
        while let Some(at) = stack.pop() {
            part.push(at);
            let (x, y) = (at % w, at / w);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let n = ny as usize * w + nx as usize;
                if !seen[n] && outline[n] > 0.0 {
                    seen[n] = true;
                    stack.push(n);
                }
            }
        }
        if !part.iter().any(|&at| skeleton[at]) {
            let deepest = part
                .into_iter()
                .max_by(|&a, &b| outline[a].total_cmp(&outline[b]))
                .unwrap();
            skeleton[deepest] = true;
        }
    }
}

/// Original part-6 frames replayed on demand for `--original` and
/// `--split`, instead of storing about 177 MB of pixels: the scene and video
/// memory are kept every [`REPLAY_INTERVAL`] frames, with the music position
/// each step read, and the port's own `Rings::step` runs forward from the
/// nearest checkpoint. A cursor makes playback one step per frame.
struct Replay {
    checkpoints: Vec<(Rings, Vga)>,
    music: Vec<u16>,
    cursor: Mutex<Option<(usize, Rings, Vga)>>,
}

impl Replay {
    /// Indices and palette of part-6 frame `j`, as the engine shows it.
    fn frame(&self, j: usize) -> (Vec<u8>, Vec<u8>) {
        let j = j.min(self.music.len());
        let mut cursor = self.cursor.lock().unwrap_or_else(|e| e.into_inner());
        let base = (j / REPLAY_INTERVAL).min(self.checkpoints.len() - 1);
        let usable =
            matches!(&*cursor, Some((at, _, _)) if *at <= j && *at >= base * REPLAY_INTERVAL);
        if !usable {
            let (rings, vga) = self.checkpoints[base].clone();
            *cursor = Some((base * REPLAY_INTERVAL, rings, vga));
        }
        let (at, rings, vga) = cursor.as_mut().unwrap();
        while *at < j {
            rings.step(vga, self.music[*at]);
            *at += 1;
        }
        (vga.indices.clone(), vga.palette.clone())
    }
}

/// Part 6 of the HD clip: from part 2's end to part 5's first frame.
pub struct RingsPreview {
    /// Clip frame of part 6's first frame (the black scanout `Rings::new`
    /// leaves). Part-6 frame `j` is clip frame `start + j`.
    pub start: usize,
    pub frames: Vec<RingsFrame>,
    pub windows: Vec<SymbolWindow>,
    pub bands: RingBands,
    /// Symbol layers 0 to 15: the original's masks, in full detail.
    pub symbols: SymbolFields,
    /// Symbol layers 16 and on: the authored sets, 16 masks each, with only
    /// the tubes' fields ([`SymbolDetail::Tubes`]).
    pub authored: SymbolFields,
    /// The authored sets, as read from rings_symbols.txt.
    pub symbol_sets: Vec<AuthoredSet>,
    /// Show the original symbols throughout (`--original-symbols`, N)
    /// instead of the authored sets in the middle of the part. Faithful HD
    /// always shows the originals.
    pub original_symbols: bool,
    /// The PSTART picture at its full palette, as 8-bit sRGB RGBA (the 6-bit
    /// DAC values expanded as the scanout does).
    pub picture: Vec<u8>,
    /// The selected style; see [`RingsStyle`].
    pub style: RingsStyle,
    /// Each frame's ring centres on screen (source pixels), per layer.
    centres: [Vec<[f64; 2]>; 2],
    replay: Replay,
}

impl RingsPreview {
    /// Continue the clip from part 2's end: make the scene on `vga`, as the
    /// engine does in the step in which part 2 reports done (the step that
    /// starts `start`), and step it with the music
    /// until it hands over to part 5, appending its PCM. The step that
    /// returns done is the one in which the engine makes part 5; its PCM is
    /// included, so the clip ends at part 5's first frame.
    pub fn load(
        data: &Data,
        vga: &mut Vga,
        music: &mut Music,
        pcm: &mut Vec<i16>,
        start: usize,
    ) -> Result<Self, String> {
        let mut rings = Rings::new(data, vga)?;
        let symbol_sets = parse_authored(authored::AUTHORED_SYMBOLS)?;
        let masks: Vec<Vec<bool>> = (0..ORIGINAL_SYMBOLS)
            .map(|n| Rings::hd_mask(vga, n))
            .collect();
        let bands = RingBands::fit(&rings.hd_source())?;
        let (image, palette) = rings.hd_picture();
        let picture = image
            .iter()
            .flat_map(|&i| {
                let expand = |c: u8| {
                    let c = c & 63;
                    (c << 2) | (c >> 4)
                };
                let p = &palette[i as usize * 3..][..3];
                [expand(p[0]), expand(p[1]), expand(p[2]), 255]
            })
            .collect();
        let symbols = SymbolFields::new(&masks, SymbolDetail::Full)?;
        let authored = SymbolFields::authored(&symbol_sets, SymbolDetail::Tubes);
        let mut states = Vec::new();
        let mut checkpoints = Vec::new();
        let mut positions = Vec::new();
        loop {
            let j = states.len();
            if j % REPLAY_INTERVAL == 0 {
                checkpoints.push((rings.clone(), vga.clone()));
            }
            states.push(rings.hd_state());
            if j > FRAME_RATE as usize * 120 {
                return Err("Part 6 exceeded its two-minute import limit".into());
            }
            let position = music.position();
            positions.push(position);
            let done = rings.step(vga, position);
            let n = (start + j) as u64;
            let samples =
                ((n + 1) * SAMPLE_RATE / FRAME_RATE - n * SAMPLE_RATE / FRAME_RATE) as usize;
            pcm.extend(music.render(samples));
            if done {
                break;
            }
        }
        // `positions` holds one more entry than the steps between frames.
        positions.pop();
        // Which draw step last wrote each page, to find the stale scanouts.
        let mut written: [Option<i64>; 8] = [None; 8];
        let mut frames = Vec::with_capacity(states.len());
        let mut previous_frame = 0;
        for state in &states {
            if state.phase == 1 && state.frame > previous_frame {
                written[state.drawn_page] = Some(state.frame as i64 - 1);
            }
            previous_frame = state.frame;
            let shown_step = state.frame as i64 - 2;
            frames.push(RingsFrame {
                state: *state,
                shown_step,
                offsets: rings.hd_offsets(shown_step),
                stale: state.phase == 1 && written[state.shown_page] != Some(shown_step),
            });
        }
        let lit: Vec<Option<u8>> = states.iter().map(|s| s.lit).collect();
        let centres = [0, 1].map(|layer| {
            frames
                .iter()
                .map(|f| bands.screen_centre(f.offsets[layer]))
                .collect()
        });
        Ok(Self {
            centres,
            start,
            windows: symbol_windows(&lit),
            frames,
            bands,
            symbols,
            authored,
            symbol_sets,
            original_symbols: false,
            picture,
            style: RingsStyle::Pool,
            replay: Replay {
                checkpoints,
                music: positions,
                cursor: Mutex::new(None),
            },
        })
    }

    /// Part-6 frame at a clip time, or `None` before part 6.
    pub fn frame_at(&self, time: f64) -> Option<usize> {
        let position = time * FRAME_RATE as f64 - self.start as f64;
        (position >= 0.0).then(|| (position as usize).min(self.frames.len() - 1))
    }

    /// Indices and palette of part-6 frame `j` exactly as the engine shows
    /// it, replayed with the port's scene (see [`Replay`]).
    pub fn original(&self, j: usize) -> (Vec<u8>, Vec<u8>) {
        self.replay.frame(j)
    }

    /// The original frame `j` as 8-bit RGBA, as the reference scanout shows it.
    pub fn original_rgba(&self, j: usize) -> Vec<u8> {
        let (indices, palette) = self.original(j);
        indices
            .iter()
            .flat_map(|&i| {
                let p = &palette[i as usize * 3..][..3];
                let c = |v: u8| {
                    let v = v & 63;
                    (v << 2) | (v >> 4)
                };
                [c(p[0]), c(p[1]), c(p[2]), 255]
            })
            .collect()
    }

    /// Title label for part-6 frame `j`.
    pub fn label(&self, j: usize) -> &'static str {
        let state = self.frames[j.min(self.frames.len() - 1)].state;
        match state.phase {
            0 => "Part 6: waiting for 06:00",
            2 => "PSTART picture",
            _ if state.intensity <= 0 => "Symbols on black",
            _ if state.intensity < 64 => "Rings fading",
            _ => "Rings and symbols",
        }
    }

    /// The authored set shown at a clip time, by its title, if one is.
    pub fn symbol_set_at(&self, time: f64) -> Option<&str> {
        if self.original_symbols {
            return None;
        }
        let j = self.frame_at(time)?;
        let i = self
            .windows
            .partition_point(|w| w.start <= j)
            .checked_sub(1)?;
        let set = self.symbol_sets.get(window_set(i)?)?;
        Some(&set.title)
    }

    /// Both layers' ring centres (source pixels) at a clip time,
    /// interpolated through the frames' exact centres (Catmull-Rom; an
    /// interpretation, as the original moves once per frame in whole
    /// pixels). Plane 0's layer first.
    pub fn centres_at(&self, time: f64) -> [[f64; 2]; 2] {
        let position = (time * FRAME_RATE as f64 - self.start as f64).max(0.0);
        [0, 1].map(|layer| catmull_rom(&self.centres[layer], position))
    }

    /// A frame value interpolated linearly between frames at a clip time.
    fn lerp_at(&self, time: f64, value: impl Fn(&RingsFrame) -> f64) -> f64 {
        let position = (time * FRAME_RATE as f64 - self.start as f64).max(0.0);
        let i = (position as usize).min(self.frames.len() - 1);
        let next = (i + 1).min(self.frames.len() - 1);
        let f = (position - i as f64).clamp(0.0, 1.0);
        let (a, b) = (value(&self.frames[i]), value(&self.frames[next]));
        a + (b - a) * f
    }

    /// Everything part 6 puts in the shared uniforms at a clip time; see
    /// renderer.rs (`rings` to `rings_style`) for the layout.
    /// How far the tune has said "panic" ([`PANIC_WINDOW`]) at a clip time:
    /// 0 before the word, up to 1 over it, eased. Without that window, 1.
    pub fn panic_said(&self, time: f64) -> f64 {
        let Some(window) = self.windows.get(PANIC_WINDOW) else {
            return 1.0;
        };
        let said = (self.start + window.start) as f64 / FRAME_RATE as f64;
        let x = ((time - said) / PANIC_FADE).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    pub fn controls(&self, time: f64) -> RingsControls {
        let j = self.frame_at(time).unwrap_or(0);
        let seconds = |frame: usize| (self.start + frame) as f64 / FRAME_RATE as f64;
        let state = self.frames[j].state;
        let now = self.centres_at(time);
        let open = self.centres_at(time - RING_SHUTTER_TICKS / FRAME_RATE as f64);
        let level = |f: &RingsFrame| {
            if f.state.phase == 1 {
                f.state.intensity.max(0) as f64 / 64.0
            } else {
                0.0
            }
        };
        let intensity = self.lerp_at(time, level);
        // The most the rings have been lit so far: 0 before the fade-in, up
        // with it, then 1 to the end of the part, through the fade-out.
        let peak = self.frames[..=j]
            .iter()
            .map(level)
            .fold(intensity, f64::max);
        let picture = self.lerp_at(time, |f| f.state.picture.map_or(0.0, |l| l as f64 / 64.0));
        // The windows as of the next frame, so that a switch completes on the
        // original's own frame and each frame time shows exactly that frame's
        // symbol: during the frame before, the times below are negative.
        // `symbol` holds the original's masks; `symbol_layers` the atlas
        // layers that draw them.
        let mut symbol = [-1.0, -1.0, 0.0, -1.0];
        let mut symbol_layers = [-1.0, -1.0, 0.0, 0.0];
        let layer = |i: usize| window_layer(i, self.windows[i].mask, self.original_symbols) as f32;
        let ahead = self.frame_at(time + 1.0 / FRAME_RATE as f64).unwrap_or(0);
        let current = self.windows.partition_point(|w| w.start <= ahead);
        if let Some(i) = current.checked_sub(1) {
            let w = self.windows[i];
            if ahead < w.end {
                symbol[0] = w.mask as f32;
                symbol_layers[0] = layer(i);
                symbol[2] = (time - seconds(w.start)) as f32;
                symbol[3] = (seconds(w.end) - time) as f32;
                if let Some(p) = i.checked_sub(1)
                    && self.windows[p].end == w.start
                {
                    symbol[1] = self.windows[p].mask as f32;
                    symbol_layers[1] = layer(p);
                }
            } else {
                // Between windows: the last one, and the time since it ended.
                symbol[1] = w.mask as f32;
                symbol_layers[1] = layer(i);
                symbol[2] = (time - seconds(w.end)) as f32;
            }
        }
        let mut controls = RingsControls {
            rings: [
                1.0,
                (time - seconds(0)) as f32,
                self.style as u32 as f32,
                state.phase as f32,
            ],
            ring_centres: [now[0][0], now[0][1], now[1][0], now[1][1]].map(|v| v as f32),
            ring_open: [open[0][0], open[0][1], open[1][0], open[1][1]].map(|v| v as f32),
            ring_bands: [
                self.bands.width as f32,
                self.bands.first as f32,
                self.bands.matched as f32,
                0.0,
            ],
            ring_fades: [
                intensity as f32,
                picture as f32,
                peak as f32,
                self.panic_said(time) as f32,
            ],
            symbol,
            symbol_layers,
            symbol_atlas: [
                self.symbols.origin[0] as f32,
                self.symbols.origin[1] as f32,
                self.symbols.size[0] as f32,
                self.symbols.size[1] as f32,
            ],
            authored_atlas: [
                self.authored.origin[0] as f32,
                self.authored.origin[1] as f32,
                self.authored.size[0] as f32,
                self.authored.size[1] as f32,
            ],
            neon_map: [
                LIGHT_ORIGIN[0] as f32,
                LIGHT_ORIGIN[1] as f32,
                LIGHT_SIZE[0] as f32,
                LIGHT_SIZE[1] as f32,
            ],
            rings_style: [[0.0; 4]; 4],
        };
        controls.rings_style = (self.style.info().controls)(&controls, time);
        controls
    }
}

/// Part 6's share of the shared uniforms at one moment. renderer.rs copies
/// these into `Uniforms`; the WGSL declaration documents each field.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingsControls {
    pub rings: [f32; 4],
    pub ring_centres: [f32; 4],
    pub ring_open: [f32; 4],
    pub ring_bands: [f32; 4],
    pub ring_fades: [f32; 4],
    pub symbol: [f32; 4],
    pub symbol_layers: [f32; 4],
    pub symbol_atlas: [f32; 4],
    pub authored_atlas: [f32; 4],
    pub neon_map: [f32; 4],
    pub rings_style: [[f32; 4]; 4],
}

/// Part 6's style. Pool is the only one: faithful-plus, glass, room and
/// volume were tried and removed, and remain in the Git history (see
/// rust/README.md). The style framework stays, so another can be added.
/// Faithful HD (M, `--faithful`) is separate: it is the original look at
/// display resolution, not a style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RingsStyle {
    /// A liquid-chrome interference pool.
    Pool = 0,
}

impl RingsStyle {
    pub const ALL: [Self; 1] = [Self::Pool];

    pub fn info(self) -> &'static StyleInfo {
        match self {
            Self::Pool => &POOL,
        }
    }

    pub fn name(self) -> &'static str {
        self.info().name
    }
}

/// One part-6 style: its WGSL, whether it has a half-resolution prepass and
/// the CPU hook for its own uniforms. See rust/README.md, "Adding a part-6
/// style". Each style owns one of the constants below and the hook function
/// next to it, so styles can be developed in parallel without touching the
/// same lines.
pub struct StyleInfo {
    pub name: &'static str,
    /// Defines `fn style_color(px: RingsPixel) -> vec3f` and
    /// `fn style_prepass(px: RingsPixel) -> vec4f`; renderer.rs builds the
    /// module from the shared controls, rings_common.wgsl, wall.wgsl, this
    /// and rings_style.wgsl.
    pub source: &'static str,
    /// Run `style_prepass` first, at half the resolution each way, into the
    /// `prepass` texture that `style_color` can read.
    pub prepass: bool,
    /// Values for `u.rings_style` at a clip time (all zero by default).
    pub controls: fn(&RingsControls, f64) -> [[f32; 4]; 4],
}

fn no_controls(_: &RingsControls, _: f64) -> [[f32; 4]; 4] {
    [[0.0; 4]; 4]
}

// ---- Pool ------------------------------------------------------------------

pub static POOL: StyleInfo = StyleInfo {
    name: "pool",
    source: include_str!("rings_pool.wgsl"),
    prepass: false,
    controls: pool_controls,
};

/// The pool style's `u.rings_style` values.
fn pool_controls(controls: &RingsControls, time: f64) -> [[f32; 4]; 4] {
    no_controls(controls, time)
}

// ---------------------------------------------------------------------------

/// The ring greys `[0, 48, 32, 0] * intensity / 64` in linear light, as the
/// reference scanout shows them; for tests and documentation.
pub fn ring_grey(layer: usize, intensity: i32) -> f32 {
    let grey = [48, 32][layer] * intensity.max(0) / 64;
    vga_linear(grey as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::clip::{Clip, ScrollerLighting};

    /// The ring planes `step` writes for window offsets `(x, y)`, read from
    /// M.DAT exactly as `step` copies them: bits 0 and 1 of each screen pixel.
    fn window_bits(source: &[u8], offsets: [[i32; 2]; 2]) -> Vec<u8> {
        let mut bits = vec![0; 320 * 200];
        for (plane, [x, y]) in offsets.into_iter().enumerate() {
            let bank = (x & 7) as usize;
            for row in 0..184 {
                let r = y as usize + row;
                let mirror = if r < 152 { r } else { 303 - r };
                let src = bank * 8056 + mirror * 53 + (x / 8) as usize + 4;
                for i in 0..256 {
                    let bit = (source[src + i / 8] >> (7 - i % 8)) & 1;
                    bits[(8 + row) * 320 + 32 + i] |= bit << plane;
                }
            }
        }
        bits
    }

    /// Invented bands: 424 x 152 bits of the model with known parameters.
    fn synthetic_bands(centre_x: f64, first: f64) -> Vec<bool> {
        (0..152 * 424)
            .map(|at| {
                let (u, r) = ((at % 424) as f64 + 0.5, (at / 424) as f64 + 0.5);
                let d = (u - centre_x).hypot(r - 152.0);
                d >= first && ((d - first) / 7.0).floor() as i64 % 2 == 0
            })
            .collect()
    }

    #[test]
    fn band_fit_recovers_invented_bands() {
        let fit = RingBands::fit(&synthetic_bands(198.5, 5.25)).unwrap();
        assert_eq!(fit.centre, [198.5, 152.0]);
        assert_eq!(fit.width, 7.0);
        assert!((fit.first - 5.25).abs() <= 1.0 / 32.0, "{fit:?}");
        assert!(fit.matched > 0.99, "{fit:?}");
        // Window offsets move the centre the other way on screen.
        assert_eq!(fit.screen_centre([50, 50]), [148.5, 110.0]);
        assert!(RingBands::fit(&[false; 10]).is_err());
        assert!(RingBands::fit(&vec![false; 424 * 152]).is_err());
    }

    #[test]
    fn symbol_windows_span_the_dark_frames_between_lit_ones() {
        let lit = [
            None,
            Some(3),
            None,
            Some(3),
            None,
            Some(4),
            None,
            Some(4),
            None,
            None,
            None,
            Some(4),
        ];
        let window = |mask, start, end| SymbolWindow { mask, start, end };
        assert_eq!(
            symbol_windows(&lit),
            [window(3, 1, 5), window(4, 5, 9), window(4, 11, 12)]
        );
        assert!(symbol_windows(&[None; 4]).is_empty());
    }

    #[test]
    fn catmull_rom_passes_through_each_frame() {
        let values: Vec<[f64; 2]> = (0..6).map(|i| [i as f64 * 2.0, (i * i) as f64]).collect();
        for (i, v) in values.iter().enumerate() {
            assert_eq!(catmull_rom(&values, i as f64), *v);
        }
        // Linear motion stays linear between frames.
        assert!((catmull_rom(&values, 2.25)[0] - 4.5).abs() < 1e-12);
        // Beyond either end it holds the end value.
        assert_eq!(catmull_rom(&values, -3.0), values[0]);
        assert_eq!(catmull_rom(&values, 40.0), values[5]);
        assert_eq!(catmull_rom(&values[..1], 0.5), values[0]);
    }

    #[test]
    fn symbol_fields_of_an_invented_stroke() {
        // A bar 5 rows high (rows 98..=102) from column 100 to 199, in mask
        // 2; the others are empty.
        let mut masks = vec![vec![false; 320 * 200]; 16];
        for y in 98..103 {
            for x in 100..200 {
                masks[2][y * 320 + x] = true;
            }
        }
        let fields = SymbolFields::new(&masks, SymbolDetail::Full).unwrap();
        let pad = SYMBOL_PAD as i32;
        assert_eq!(fields.origin, [100 - pad, 98 - pad]);
        assert_eq!(fields.size, [100 + 2 * SYMBOL_PAD, 5 + 2 * SYMBOL_PAD]);
        let (tw, th) = fields.texels();
        let texel = |layer: usize, x: f32, y: f32| {
            let i = ((x - fields.origin[0] as f32) * SYMBOL_SCALE as f32) as usize;
            let j = ((y - fields.origin[1] as f32) * SYMBOL_SCALE as f32) as usize;
            let at = ((layer * th as usize + j) * tw as usize + i) * 4;
            [0, 1, 2, 3].map(|k| fields.atlas[at + k].to_f32())
        };
        // Down the column at x = 150: the centreline lies within a texel of
        // the bar's middle (thinning a stroke an even number of texels high
        // leaves it half a texel off), half a stroke inside the outline.
        let column: Vec<(f32, [f32; 4])> = (0..20)
            .map(|k| 96.25 + k as f32 * 0.5)
            .map(|y| (y, texel(2, 150.2, y)))
            .collect();
        let (y, centre) = column
            .iter()
            .min_by(|a, b| a.1[1].total_cmp(&b.1[1]))
            .copied()
            .unwrap();
        assert!(
            (y - 100.5).abs() <= 0.25 && centre[1] < 0.3,
            "{y} {centre:?}"
        );
        assert!(centre[0] > 2.0 && centre[0] < 2.7, "{centre:?}");
        assert!((centre[2] - 2.5).abs() < 0.4, "{centre:?}");
        // The bar's middle is far from either end; its ends are near one.
        assert_eq!(centre[3], SYMBOL_LIMIT);
        assert!(texel(2, 100.2, y)[3] < 4.5 && texel(2, 199.2, y)[3] < 4.5);
        // The texel centred on row 96.75, above the stroke: as far from the
        // centreline as their rows are apart.
        let above = texel(2, 150.2, 96.6);
        assert!(
            above[0] < 0.0 && (above[1] - (y - 96.75)).abs() < 0.3,
            "{above:?}"
        );
        // Empty masks read as far away everywhere.
        assert_eq!(texel(5, 150.2, 100.6)[..2], [-SYMBOL_LIMIT, SYMBOL_LIMIT]);
        // The light map: the tube's coverage, a full mip chain to 1 x 1,
        // each level keeping roughly the same average.
        assert_eq!(fields.light.len(), 10);
        let (lw, lh) = (LIGHT_SIZE[0] as usize, LIGHT_SIZE[1] as usize);
        let base = &fields.light[0];
        let at = |x: i32, y: i32| {
            let (i, j) = (
                (x - LIGHT_ORIGIN[0]) as usize,
                (y - LIGHT_ORIGIN[1]) as usize,
            );
            base[(2 * lh + j) * lw + i].to_f32()
        };
        // Clear of the glass a row beyond its radius.
        let clear = 100 + NEON_RADIUS.ceil() as i32 + 1;
        assert!(at(150, 100) > 0.9 && at(150, clear) == 0.0 && at(20, 20) == 0.0);
        let mean = |level: &[f16], w: usize, h: usize| {
            level[2 * w * h..3 * w * h]
                .iter()
                .map(|v| v.to_f32())
                .sum::<f32>()
                / (w * h) as f32
        };
        let top = mean(base, lw, lh);
        for (k, level) in fields.light.iter().enumerate() {
            let (w, h) = ((lw >> k).max(1), (lh >> k).max(1));
            assert_eq!(level.len(), 16 * w * h);
            if k < 8 {
                assert!((mean(level, w, h) / top - 1.0).abs() < 0.05, "level {k}");
            }
        }
        // Any number of masks, but at least one, all 320 × 200.
        assert_eq!(
            SymbolFields::new(&masks[..3], SymbolDetail::Full)
                .unwrap()
                .layers,
            3
        );
        assert!(SymbolFields::new(&[], SymbolDetail::Full).is_err());
        assert!(SymbolFields::new(&[vec![false; 10]], SymbolDetail::Full).is_err());
    }

    #[test]
    fn crossing_strokes_become_one_run_and_a_broken_one() {
        // A + of two bars 5 pixels thick, crossing at (160.5, 100.5): one
        // stays a run through the crossing, the other stops short of its
        // glass on both sides, each new end with an electrode.
        let mut mask = vec![false; 320 * 200];
        for y in 60..141 {
            for x in 158..163 {
                mask[y * 320 + x] = true;
            }
        }
        for y in 98..103 {
            for x in 110..211 {
                mask[y * 320 + x] = true;
            }
        }
        let mut masks = vec![vec![false; 320 * 200]; 16];
        masks[0] = mask;
        let fields = SymbolFields::new(&masks, SymbolDetail::Full).unwrap();
        let (tw, _) = fields.texels();
        let texel = |x: f32, y: f32| {
            let i = ((x - fields.origin[0] as f32) * SYMBOL_SCALE as f32) as usize;
            let j = ((y - fields.origin[1] as f32) * SYMBOL_SCALE as f32) as usize;
            let at = (j * tw as usize + i) * 4;
            [0, 1, 2, 3].map(|k| fields.atlas[at + k].to_f32())
        };
        // Is the centreline near (x, y)? Search a texel either way.
        let on_line = |x: f32, y: f32| {
            (-2..=2).any(|k| {
                let d = k as f32 * 0.25;
                texel(x + d, y)[1].min(texel(x, y + d)[1]) < 0.6
            })
        };
        assert!(on_line(160.5, 100.5), "a run passes through the crossing");
        let vertical = on_line(160.5, 95.0) && on_line(160.5, 106.0);
        let horizontal = on_line(154.0, 100.5) && on_line(167.0, 100.5);
        assert!(vertical != horizontal, "exactly one bar runs through");
        // The other bar is cut back on both sides, to new ends: nothing of
        // it within 4.5 rows of the crossing, and it carries on a little
        // beyond the gap (both tubes' glass and a stroke's half-width).
        let gap = 2.0 * NEON_RADIUS + NEON_STROKE;
        let (near, far) = if vertical {
            (
                [(155.1, 100.5), (165.9, 100.5)],
                [(155.1 - gap, 100.5), (165.9 + gap, 100.5)],
            )
        } else {
            (
                [(160.5, 96.0), (160.5, 105.0)],
                [(160.5, 96.1 - gap), (160.5, 104.9 + gap)],
            )
        };
        for (x, y) in near {
            assert!(!on_line(x, y), "gap at {x}, {y}");
        }
        for (x, y) in far {
            assert!(on_line(x, y), "the broken bar continues at {x}, {y}");
            assert!(texel(x, y)[3] < SYMBOL_LIMIT);
        }
    }

    #[test]
    fn styles_keep_their_order_and_entry_points() {
        for (i, style) in RingsStyle::ALL.into_iter().enumerate() {
            assert_eq!(style as usize, i);
            assert!(
                style
                    .info()
                    .source
                    .contains("fn style_color(px:RingsPixel)->vec3f")
            );
            assert!(
                style
                    .info()
                    .source
                    .contains("fn style_prepass(px:RingsPixel)->vec4f")
            );
            let controls = RingsControls::default();
            assert_eq!((style.info().controls)(&controls, 60.0), [[0.0; 4]; 4]);
        }
    }

    #[test]
    fn ring_greys_follow_the_palette_intensity() {
        assert_eq!(ring_grey(0, -100), 0.0);
        assert_eq!(ring_grey(1, 0), 0.0);
        assert_eq!(ring_grey(0, 64), vga_linear(48));
        assert_eq!(ring_grey(1, 64), vga_linear(32));
        assert_eq!(ring_grey(0, 32), vga_linear(24));
    }

    #[test]
    fn invented_windows_map_to_the_sets_patterns() {
        // Invented windows: indices as the preview counts them.
        assert_eq!(window_set(0), None);
        assert_eq!(window_set(NEW_SET_FIRST_WINDOW - 1), None);
        assert_eq!(window_set(NEW_SET_FIRST_WINDOW), Some(0));
        assert_eq!(
            window_set(NEW_SET_FIRST_WINDOW + PATTERN_WINDOWS - 1),
            Some(0)
        );
        assert_eq!(window_set(NEW_SET_FIRST_WINDOW + PATTERN_WINDOWS), Some(1));
        let last = NEW_SET_FIRST_WINDOW + NEW_SETS * PATTERN_WINDOWS;
        assert_eq!(window_set(last - 1), Some(NEW_SETS - 1));
        assert_eq!(window_set(last), None);
        // Before and after the sets, and with --original-symbols, a window
        // draws the original mask; in pattern k, set k's drawing of it.
        assert_eq!(window_layer(10, 7, false), 7);
        assert_eq!(window_layer(last + 3, 7, false), 7);
        assert_eq!(window_layer(64, 0, false), 16);
        assert_eq!(window_layer(64 + 31, 15, false), 31);
        assert_eq!(window_layer(64 + 32 * 3 + 5, 9, false), 16 + 16 * 3 + 9);
        assert_eq!(window_layer(64 + 32 * 3 + 5, 9, true), 9);
        assert_eq!(window_layer(last - 1, 15, false), SYMBOL_LAYERS - 1);
        // In the controls: invented windows, two before the sets and two in
        // the first, back to back from part-6 frame 100, 8 frames each.
        let windows: Vec<SymbolWindow> = (0..NEW_SET_FIRST_WINDOW + 2)
            .map(|i| SymbolWindow {
                mask: (i % 16) as u8,
                start: 100 + 8 * i,
                end: 108 + 8 * i,
            })
            .collect();
        let starts: Vec<usize> = windows.iter().map(|w| w.start).collect();
        let at = |i: usize| (starts[i] as f64 + 2.5) / FRAME_RATE as f64;
        let preview = invented_preview(windows.clone());
        let before = preview.controls(at(NEW_SET_FIRST_WINDOW - 1));
        assert_eq!(before.symbol[..2], [15.0, 14.0]);
        assert_eq!(before.symbol_layers[..2], [15.0, 14.0]);
        let first = preview.controls(at(NEW_SET_FIRST_WINDOW));
        assert_eq!(first.symbol[..2], [0.0, 15.0]);
        assert_eq!(first.symbol_layers[..2], [16.0, 15.0]);
        let second = preview.controls(at(NEW_SET_FIRST_WINDOW + 1));
        assert_eq!(second.symbol_layers[..2], [17.0, 16.0]);
        assert_eq!(
            preview.symbol_set_at(at(NEW_SET_FIRST_WINDOW)),
            Some("Invented")
        );
        assert_eq!(preview.symbol_set_at(at(3)), None);
        // After the last window: none lit, the last one as the previous.
        let after = preview.controls((windows.last().unwrap().end as f64 + 3.0) / 70.0);
        assert_eq!(after.symbol[..2], [-1.0, 1.0]);
        assert_eq!(after.symbol_layers[..2], [-1.0, 17.0]);
        let mut original = invented_preview(windows);
        original.original_symbols = true;
        let first = original.controls(at(NEW_SET_FIRST_WINDOW));
        assert_eq!(first.symbol_layers[..2], [0.0, 15.0]);
        assert_eq!(original.symbol_set_at(at(NEW_SET_FIRST_WINDOW)), None);
    }

    #[test]
    fn the_pool_rises_as_the_tune_says_panic() {
        // Invented windows, 8 frames each from part-6 frame 100: the word
        // comes as PANIC_WINDOW begins, at frame 100 + 8 * 128.
        let windows: Vec<SymbolWindow> = (0..PANIC_WINDOW + 4)
            .map(|i| SymbolWindow {
                mask: (i % 16) as u8,
                start: 100 + 8 * i,
                end: 108 + 8 * i,
            })
            .collect();
        let said = (100 + 8 * PANIC_WINDOW) as f64 / FRAME_RATE as f64;
        let preview = invented_preview(windows);
        let at = |t: f64| preview.controls(t).ring_fades[3];
        assert_eq!(at(said - 0.01), 0.0);
        assert_eq!(at(said), 0.0);
        let half = at(said + PANIC_FADE / 2.0);
        assert!((half - 0.5).abs() < 1e-6, "{half}");
        assert!(at(said + 0.25 * PANIC_FADE) < half);
        assert_eq!(at(said + PANIC_FADE), 1.0);
        assert_eq!(at(said + 1.0), 1.0);
        // Without the word's window, the pool throughout.
        let short: Vec<SymbolWindow> = (0..4)
            .map(|i| SymbolWindow {
                mask: 0,
                start: 8 * i,
                end: 8 + 8 * i,
            })
            .collect();
        assert_eq!(invented_preview(short).controls(0.1).ring_fades[3], 1.0);
        // The shader reads the word from that slot.
        assert!(
            include_str!("rings_common.wgsl")
                .contains("fn panic_said()->f32 {return u.ring_fades.w;}")
        );
    }

    /// A preview of invented frames: part 6 from clip frame 0, running, with
    /// the given symbol windows, empty masks and one invented set.
    fn invented_preview(windows: Vec<SymbolWindow>) -> RingsPreview {
        let frames = windows.last().map_or(1, |w| w.end + 8);
        let state = HdRings {
            phase: 1,
            frame: 1,
            intensity: 64,
            drawn_page: 0,
            shown_page: 1,
            lit: None,
            picture: None,
        };
        let empty = vec![vec![false; 320 * 200]; 1];
        RingsPreview {
            start: 0,
            frames: vec![
                RingsFrame {
                    state,
                    shown_step: 0,
                    offsets: [[0; 2]; 2],
                    stale: false,
                };
                frames
            ],
            windows,
            bands: RingBands::fit(&synthetic_bands(198.5, 5.25)).unwrap(),
            symbols: SymbolFields::new(&empty, SymbolDetail::Full).unwrap(),
            authored: SymbolFields::new(&empty, SymbolDetail::Tubes).unwrap(),
            symbol_sets: vec![AuthoredSet {
                title: "Invented".into(),
                shapes: Vec::new(),
            }],
            original_symbols: false,
            picture: vec![0; 320 * 200 * 4],
            style: RingsStyle::Pool,
            centres: [vec![[160.0, 100.0]; frames], vec![[160.0, 100.0]; frames]],
            replay: Replay {
                checkpoints: Vec::new(),
                music: Vec::new(),
                cursor: Mutex::new(None),
            },
        }
    }

    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn part6_state_matches_every_engine_frame() {
        let data = crate::test_support::data();
        let clip = Clip::load_rings(&data, ScrollerLighting::Studio).unwrap();
        let rings = clip.rings.as_ref().unwrap();
        assert_eq!(rings.start, clip.frames.len());
        let source = data.file("M.DAT").unwrap();
        let picture = data.file("PSTART.U").unwrap();
        let picture_palette = data.file("PSTART.PAL").unwrap();
        let masks: Vec<Vec<bool>> = {
            let mut vga = Vga::new();
            Rings::new(&data, &mut vga).unwrap();
            (0..16).map(|n| Rings::hd_mask(&vga, n)).collect()
        };
        let mut engine = crate::engine::Engine::new(data, false).unwrap();
        let mut pcm = Vec::new();
        for _ in &clip.frames {
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(
            engine.part(),
            6,
            "part 6 starts at clip frame {}",
            rings.start
        );
        let seconds = |j: usize| (rings.start + j) as f64 / FRAME_RATE as f64;
        let (mut stale, mut checked, mut matched) = (Vec::new(), 0_u64, 0_u64);
        let mut events: Vec<(String, f64)> = Vec::new();
        let note = |events: &mut Vec<(String, f64)>, name: &str, j: usize| {
            if !events.iter().any(|(n, _)| n == name) {
                events.push((name.into(), seconds(j)));
            }
        };
        let seen = |events: &Vec<(String, f64)>, name: &str| events.iter().any(|(n, _)| n == name);
        for (j, frame) in rings.frames.iter().enumerate() {
            let video = &engine.video;
            assert_eq!((video.width, video.height), (320, 200));
            // The replayed original equals the engine's frame.
            let (indices, palette) = rings.original(j);
            assert!(
                indices == video.indices,
                "replayed pixels at part-6 frame {j}"
            );
            assert!(
                palette == video.palette,
                "replayed palette at part-6 frame {j}"
            );
            let state = frame.state;
            match state.phase {
                0 => assert!(video.palette.iter().all(|&c| c == 0), "black at {j}"),
                1 => {
                    note(&mut events, "first ring step", j);
                    // Ring greys from the intensity.
                    let shade = |i: usize| {
                        if state.intensity <= 0 {
                            0
                        } else {
                            [0, 48, 32, 0][i & 3] * state.intensity / 64
                        }
                    };
                    let yellow =
                        |i: usize| video.palette[i * 3] == 63 && video.palette[i * 3 + 1] == 63;
                    for i in (0..16).filter(|&i| !yellow(i)) {
                        assert_eq!(
                            video.palette[i * 3..i * 3 + 3],
                            [shade(i) as u8; 3],
                            "grey {i} at {j}"
                        );
                    }
                    if state.intensity > 0 {
                        note(&mut events, "rings visible", j);
                    }
                    if state.intensity == 64 {
                        note(&mut events, "rings at full intensity", j);
                    }
                    if seen(&events, "rings at full intensity") && state.intensity < 64 {
                        note(&mut events, "rings fading out", j);
                    }
                    if seen(&events, "rings fading out") && state.intensity == 0 {
                        note(&mut events, "rings gone", j);
                    }
                    // The most they have been lit so far: none until they
                    // show, all of it from full intensity on.
                    let peak = rings.controls(seconds(j)).ring_fades[2];
                    if !seen(&events, "rings visible") {
                        assert_eq!(peak, 0.0, "ring peak at {j}");
                    }
                    if seen(&events, "rings at full intensity") {
                        assert!(peak > 0.999, "ring peak at {j}");
                    }
                    // The lit symbol: exactly its mask's pixels are yellow.
                    let lit: Vec<bool> =
                        video.indices.iter().map(|&i| yellow(i as usize)).collect();
                    match state.lit {
                        Some(n) => {
                            note(&mut events, "first symbol", j);
                            assert!(lit == masks[n as usize], "symbol {n} at {j}");
                            // At the frame's own time the HD views show it fully.
                            let symbol = rings.controls(seconds(j)).symbol;
                            assert_eq!(symbol[0], n as f32, "HD symbol at {j}");
                            assert!(symbol[2] * 70.0 + 1.0 >= 0.999, "HD symbol at {j} fading");
                        }
                        None => assert!(!lit.contains(&true), "unlit frame {j}"),
                    }
                    // The rings shown, under the one-frame latency model.
                    let expected = window_bits(&source, frame.offsets);
                    let shown: Vec<u8> = video.indices.iter().map(|&i| i & 3).collect();
                    if shown != expected {
                        assert!(frame.stale, "rings at {j} differ but are not marked stale");
                        stale.push(j);
                    } else {
                        assert!(
                            !frame.stale || state.frame <= 1,
                            "rings at {j} marked stale"
                        );
                    }
                    // The analytic band model against the window's pixels.
                    let centres = frame.offsets.map(|o| rings.bands.screen_centre(o));
                    for y in 8..192 {
                        for x in 32..288 {
                            let mut bits = 0;
                            for (layer, c) in centres.iter().enumerate() {
                                let r = (x as f64 + 0.5 - c[0]).hypot(y as f64 + 0.5 - c[1]);
                                let b = &rings.bands;
                                let set = r >= b.first
                                    && ((r - b.first) / b.width).floor() as i64 % 2 == 0;
                                bits |= (set as u8) << layer;
                            }
                            for layer in 0..2 {
                                checked += 1;
                                matched +=
                                    ((bits ^ expected[y * 320 + x]) >> layer & 1 == 0) as u64;
                            }
                        }
                    }
                }
                _ => {
                    note(&mut events, "picture fade", j);
                    let level = state.picture.unwrap() as usize;
                    assert!(video.indices == picture, "picture pixels at {j}");
                    for (k, (&got, &input)) in video
                        .palette
                        .iter()
                        .zip(&picture_palette)
                        .enumerate()
                        .skip(3)
                    {
                        assert_eq!(
                            got,
                            (input as usize * level / 64) as u8 & 63,
                            "picture level {level} entry {k}"
                        );
                    }
                }
            }
            pcm.extend(engine.step().unwrap());
        }
        assert_eq!(
            engine.part(),
            5,
            "part 5 starts right after the last part-6 frame"
        );
        events.push(("part 5".into(), seconds(rings.frames.len())));
        assert_eq!(pcm, clip.pcm, "uninterrupted PCM through part 6");
        // The PCM holds whole samples.
        assert!((clip.duration() - seconds(rings.frames.len())).abs() < 1.0 / SAMPLE_RATE as f64);
        // The stale scanouts all come where the page has just advanced: a
        // step normally draws into the page shown before it. The first ring
        // step shows a page nothing has drawn yet.
        let advanced = |k: usize| {
            let f = &rings.frames;
            k >= 1
                && f[k - 1].state.phase == 1
                && f[k].state.drawn_page != f[k - 1].state.shown_page
        };
        for &j in &stale {
            let first = rings.frames[j].state.frame == 1;
            assert!(first || advanced(j) || advanced(j - 1), "stale frame {j}");
        }
        let running = rings.frames.iter().filter(|f| f.state.phase == 1).count();
        assert!(
            stale.len() * 10 < running,
            "{} stale of {running}",
            stale.len()
        );
        let visible: Vec<usize> = stale
            .iter()
            .copied()
            .filter(|&j| rings.frames[j].state.intensity > 0)
            .collect();
        let lit_frames = rings
            .frames
            .iter()
            .filter(|f| f.state.phase == 1 && f.state.intensity > 0)
            .count();
        let share = matched as f64 / checked as f64;
        assert!(share > 0.97, "analytic bands match {share}");
        // Every lit frame is inside its symbol's window.
        for w in &rings.windows {
            for j in w.start..w.end {
                assert!(rings.frames[j].state.lit.is_none_or(|n| n == w.mask));
            }
        }
        // The tune's patterns hold 32 windows each: the order of the tune
        // that selected each window's first lit frame (in the step before
        // it) is the same through each run of 32 from the first, and changes
        // between them. The authored sets take the 7 from window 64.
        assert_eq!(rings.windows.len(), 321, "symbol windows");
        let order = |i: usize| rings.replay.music[rings.windows[i].start - 1] >> 8;
        for (i, _) in rings.windows.iter().enumerate().skip(1) {
            let same = order(i) == order(i - 1);
            assert_eq!(same, i % PATTERN_WINDOWS != 0, "order at window {i}");
        }
        let last = NEW_SET_FIRST_WINDOW + NEW_SETS * PATTERN_WINDOWS;
        let start = |i: usize| seconds(rings.windows[i].start);
        // Their first lit frames; the HD views switch over the frame before.
        let (from, to) = (start(NEW_SET_FIRST_WINDOW), start(last));
        assert!(
            (from - 50.786).abs() < 0.001 && (to - 76.643).abs() < 0.001,
            "{from} {to}"
        );
        // Each set draws every mask its pattern lights, and the controls
        // show its drawings there and the originals elsewhere.
        let mut patterns = Vec::new();
        for k in 0..NEW_SETS {
            let first = NEW_SET_FIRST_WINDOW + k * PATTERN_WINDOWS;
            let mut lit: Vec<u8> = rings.windows[first..first + PATTERN_WINDOWS]
                .iter()
                .map(|w| w.mask)
                .collect();
            lit.sort_unstable();
            lit.dedup();
            let set = &rings.symbol_sets[k];
            for &n in &lit {
                assert!(
                    set.shapes.iter().any(|s| s.mask == n),
                    "set {} ({}) has no drawing of mask {n}, which its pattern lights",
                    k + 1,
                    set.title
                );
            }
            patterns.push((start(first), order(first), lit));
        }
        // The drawings stand about as tall as the originals (AUTHORED_BOX):
        // their median heights, a drawing's with its stroke as a mask's has,
        // within a tenth of each other.
        let median = |mut v: Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v[v.len() / 2]
        };
        let original = median(
            masks
                .iter()
                .map(|m| {
                    let rows: Vec<usize> = (0..200)
                        .filter(|&y| m[y * 320..][..320].contains(&true))
                        .collect();
                    (rows[rows.len() - 1] - rows[0] + 1) as f32
                })
                .collect(),
        );
        let authored = median(
            rings
                .symbol_sets
                .iter()
                .flat_map(|set| &set.shapes)
                .map(|shape| {
                    let ys = shape.runs.iter().flatten().map(|&p| authored_source(p)[1]);
                    let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
                    hi - lo + 2.0 * NEON_STROKE
                })
                .collect(),
        );
        assert!(
            (authored / original - 1.0).abs() < 0.1,
            "{authored} {original}"
        );
        for (i, w) in rings.windows.iter().enumerate() {
            let t = seconds(w.start) + 0.5 / FRAME_RATE as f64;
            let layers = rings.controls(t).symbol_layers;
            assert_eq!(
                layers[0],
                window_layer(i, w.mask, false) as f32,
                "layer at window {i}"
            );
            let new = (NEW_SET_FIRST_WINDOW..last).contains(&i);
            assert_eq!(layers[0] >= ORIGINAL_SYMBOLS as f32, new, "window {i}");
        }
        println!(
            "Part 6: clip frames {}..{} ({:.3}–{:.3} s); bands {:?}",
            rings.start,
            rings.start + rings.frames.len(),
            seconds(0),
            seconds(rings.frames.len()),
            rings.bands
        );
        for (name, at) in &events {
            println!("  {at:8.3} s  {name}");
        }
        println!(
            "  {} symbol windows; the analytic bands match {:.2}% of shown ring bits",
            rings.windows.len(),
            share * 100.0
        );
        println!("  symbol heights, median: original {original} rows, authored {authored:.1}");
        println!(
            "  originals again from {:.3} s; the last window from {:.3} s",
            seconds(rings.windows[last].start),
            seconds(rings.windows[rings.windows.len() - 1].start)
        );
        for (k, (at, order, lit)) in patterns.iter().enumerate() {
            println!(
                "  set {} ({}) from {at:.3} s, order {order:02X}: masks {lit:?}",
                k + 1,
                rings.symbol_sets[k].title
            );
        }
        println!(
            "  {} of {running} ring frames show a stale page; {} of the {lit_frames} with \
             visible rings (skipped in HD; clip frames): {:?}",
            stale.len(),
            visible.len(),
            visible.iter().map(|&j| rings.start + j).collect::<Vec<_>>()
        );
    }
}
