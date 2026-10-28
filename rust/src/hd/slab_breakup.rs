//! Part 4's slab breaking up into part 7's dust, in the modern view. All of
//! it is authored: the original slides the slab down off the screen from
//! the music's 2F00 and opens part 7 on black.
//!
//! From the slide's first frame the slab stays where it rests and comes
//! apart into grains, the cells of a jittered grid over its top and front
//! faces, from the top face's far edge forward and on down the front face,
//! along a ragged edge. Each grain floats up into the dark and settles onto
//! the path of one of part 7's dust motes, so that by the time the slab has
//! all come apart and settled, the dust in part 7's air is the slab.
//!
//! The grid has a cell for every mote (a few over: those grains float up
//! and fade). Motes and cells are matched by where they show on the screen
//! when part 7 begins: both are split into columns of equal count, left to
//! right, and matched in order within each column from the bottom up, so
//! each grain rises to a mote above it, or stays level, in the same part
//! of the screen. slab_breakup.wgsl, in part 4's modern module and part
//! 7's, holds each grain's place, when it comes away and the hole it
//! leaves; greetings_light.wgsl flies it.
use super::greetings::CAMERA_DISTANCE;
use super::plasma_stone::Stone;

/// How long the break-up runs from the slide's first frame (seconds,
/// slab_breakup.wgsl's BREAKUP_END): the last grains have settled by then.
pub const BREAK_UP: f64 = 4.3;
/// A cell with no mote (slab_breakup.wgsl's BREAKUP_NO_MOTE).
pub const NO_MOTE: u32 = u32::MAX;
/// The columns the matching splits motes and cells into.
const COLUMNS: usize = 64;
/// slab_breakup.wgsl, which part 4's modern module and part 7's include.
pub(super) const WGSL: &str = include_str!("slab_breakup.wgsl");

/// The grains and their motes.
#[derive(Clone, Debug, Default)]
pub struct Breakup {
    /// The grid: cells across both faces, down the top face (from its near
    /// edge) and down the front face (from its top edge).
    pub grid: [u32; 3],
    /// Each cell's mote, or [`NO_MOTE`]: the top face's cells row by row,
    /// then the front face's.
    pub motes: Vec<u32>,
}

impl Breakup {
    /// The grid for this slab with a cell for each of `motes` (part 7's,
    /// [`super::greetings::motes`]), and the matching.
    pub fn new(stone: &Stone, motes: &[[f32; 4]]) -> Self {
        let [width, depth, height] = stone.size;
        if motes.is_empty() {
            return Self::default();
        }
        // Square cells, as near as the faces allow, enough of them.
        let mut cell = (width * (depth + height) / motes.len() as f64).sqrt();
        let grid = loop {
            let g = [width, depth, height].map(|v| (v / cell).ceil());
            if g[0] * (g[1] + g[2]) >= motes.len() as f64 {
                break g.map(|v| v as u32);
            }
            cell *= 0.999;
        };
        // Where each cell's centre shows (source column, row), at rest.
        let (back, down) = stone.axes();
        let mut cells = Vec::with_capacity((grid[0] * (grid[1] + grid[2])) as usize);
        for (rows, extent, along) in [(grid[1], depth, back), (grid[2], height, down)] {
            for r in 0..rows {
                for c in 0..grid[0] {
                    let x = ((c as f64 + 0.5) / grid[0] as f64 - 0.5) * width;
                    let v = (r as f64 + 0.5) / rows as f64 * extent;
                    let p = std::array::from_fn(|k| {
                        stone.corner[k] + v * along[k] + if k == 0 { x } else { 0.0 }
                    });
                    cells.push(stone.project(p));
                }
            }
        }
        // Where each mote shows when part 7 begins (greetings_light.wgsl's
        // project; a source column is 5/6 of a row).
        let shown: Vec<(f64, f64)> = motes
            .iter()
            .map(|m| {
                let s = CAMERA_DISTANCE / (CAMERA_DISTANCE + m[2] as f64);
                (
                    160.0 + m[0] as f64 * s * 6.0 / 5.0,
                    100.0 - m[1] as f64 * s,
                )
            })
            .collect();
        let (cells_in_order, motes_in_order) = (in_order(&cells), in_order(&shown));
        let mut of_cell = vec![NO_MOTE; cells.len()];
        for (i, &mote) in motes_in_order.iter().enumerate() {
            of_cell[cells_in_order[i * cells.len() / motes.len()]] = mote as u32;
        }
        Self {
            grid,
            motes: of_cell,
        }
    }

    /// The cells: each face's columns times its rows.
    pub fn cells(&self) -> u32 {
        self.grid[0] * (self.grid[1] + self.grid[2])
    }
}

/// Points (source column, row) in the matching's order: columns of equal
/// count from the left, each from the bottom up.
fn in_order(points: &[(f64, f64)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..points.len()).collect();
    order.sort_by(|&a, &b| points[a].0.total_cmp(&points[b].0));
    let n = order.len();
    for k in 0..COLUMNS {
        order[k * n / COLUMNS..(k + 1) * n / COLUMNS]
            .sort_by(|&a, &b| points[b].1.total_cmp(&points[a].1));
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Motes spread evenly over the frame and the dust's depths, as
    /// greetings::motes spreads them (a SplitMix64 stream).
    fn invented_motes(n: usize) -> Vec<[f32; 4]> {
        let mut state = 0x5eed_0004_u64;
        let mut next = || {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
        };
        (0..n)
            .map(|_| {
                let (x, y, depth) = (next(), next(), next());
                let z = -300.0 + 520.0 * depth;
                let s = (CAMERA_DISTANCE as f32 + z) / CAMERA_DISTANCE as f32;
                [(x - 0.5) * 300.0 * s, (y - 0.5) * 200.0 * s, z, 3.5]
            })
            .collect()
    }

    #[test]
    fn the_shaders_constants_are_these() {
        for constant in [
            format!("const BREAKUP_END={BREAK_UP};"),
            format!("const BREAKUP_NO_MOTE=0x{NO_MOTE:x}u;"),
        ] {
            assert!(WGSL.contains(&constant), "slab_breakup.wgsl lacks {constant}");
        }
    }

    #[test]
    fn every_mote_has_its_own_cell() {
        let stone = super::super::plasma_stone::tests::invented_stone();
        let motes = invented_motes(20_000);
        let b = Breakup::new(&stone, &motes);
        assert_eq!(b.motes.len() as u32, b.cells());
        assert!(b.cells() as usize >= motes.len());
        assert!(b.cells() as f64 <= motes.len() as f64 * 1.02, "{} cells", b.cells());
        let mut seen = vec![false; motes.len()];
        for &m in b.motes.iter().filter(|&&m| m != NO_MOTE) {
            assert!(!seen[m as usize], "mote {m} twice");
            seen[m as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "a mote without a cell");
    }

    #[test]
    fn grains_rise_to_their_motes() {
        let stone = super::super::plasma_stone::tests::invented_stone();
        let motes = invented_motes(20_000);
        let b = Breakup::new(&stone, &motes);
        // The cells' rows on the screen, as Breakup::new places them.
        let (back, down) = stone.axes();
        let [width, depth, height] = stone.size;
        let mut rows = Vec::new();
        for (count, extent, along) in [(b.grid[1], depth, back), (b.grid[2], height, down)] {
            for r in 0..count {
                for c in 0..b.grid[0] {
                    let x = ((c as f64 + 0.5) / b.grid[0] as f64 - 0.5) * width;
                    let v = (r as f64 + 0.5) / count as f64 * extent;
                    let p = std::array::from_fn(|k| {
                        stone.corner[k] + v * along[k] + if k == 0 { x } else { 0.0 }
                    });
                    rows.push(stone.project(p).1);
                }
            }
        }
        let mut rising = 0;
        let mut matched = 0;
        for (cell, &m) in b.motes.iter().enumerate().filter(|(_, m)| **m != NO_MOTE) {
            let mote = motes[m as usize];
            let s = CAMERA_DISTANCE / (CAMERA_DISTANCE + mote[2] as f64);
            let row = 100.0 - mote[1] as f64 * s;
            matched += 1;
            rising += (row <= rows[cell] + 2.0) as usize;
        }
        assert!(rising * 100 >= matched * 95, "{rising} of {matched} rise");
    }
}
