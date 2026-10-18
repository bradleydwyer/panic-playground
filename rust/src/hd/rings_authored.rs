//! Part 6's authored symbol sets: line drawings made for the remake, not
//! recovered from the demo, which the neon shows in place of the original's
//! 16 symbols in the middle of the part, one set per pattern of the tune;
//! the music still picks which mask lights and when. Some are neon tracings
//! of company logos, which are their owners' trademarks. The drawings are
//! read from rings_symbols.txt (its header gives the format) and made into
//! symbol layers from their runs, as neon signs: single tubes that never
//! close or join ([`SymbolFields::authored`]).
//! Faithful HD always shows the originals.
use super::*;

/// The authored symbol sets (not recovered; see the file's header).
pub(super) const AUTHORED_SYMBOLS: &str = include_str!("rings_symbols.txt");
/// Authored sets of 16 symbols, shown in place of the original's masks in
/// the middle of part 6, one set per pattern of the tune.
pub const NEW_SETS: usize = 7;
/// Symbol windows per pattern of the tune, and the first window of the new
/// sets: read from the port's windows, each run of 32 is one order of the
/// tune. The sets take the third to the ninth of part 6, windows 64 to 287,
/// from the first lit frame at 50.79 s to window 288's at 76.64 s (the HD
/// views switch over the frame before each). The ignored data test checks
/// these.
pub const PATTERN_WINDOWS: usize = 32;
pub const NEW_SET_FIRST_WINDOW: usize = 2 * PATTERN_WINDOWS;
/// Height, in source rows, of the box the authored drawings' 100 units fill,
/// centred on the screen's centre; across, a unit is the same length on the
/// display, so 6/5 as many columns. Authored, to match the originals' size:
/// their masks are 43 to 157 rows tall, 123 at the median (measured from the
/// local data), and the drawings span 20 to 99 units, 80 at the median, so
/// at 155 rows they stand 31 to 153 rows tall, 124 at the median. The
/// tallest still clears the 4:3 frame by more than a tube's radius (a
/// default test checks every drawing).
pub const AUTHORED_BOX: f32 = 155.0;

/// One authored symbol (see rings_symbols.txt): its label and its runs,
/// polylines in the 100-unit drawing box, x right and y down, each a neon
/// tube's centreline as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredShape {
    pub mask: u8,
    pub label: String,
    pub runs: Vec<Vec<[f32; 2]>>,
}

/// One authored set: its title and the masks it draws, by the original's
/// mask number. Masks it leaves out are empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthoredSet {
    pub title: String,
    pub shapes: Vec<AuthoredShape>,
}

/// Read rings_symbols.txt's format (described in its header): `set`, its
/// number and title, then a line per shape of set, mask, label and runs, all
/// separated by tabs. Points are on a 0..1000 grid, 10 to a drawing unit.
pub fn parse_authored(text: &str) -> Result<Vec<AuthoredSet>, String> {
    let mut sets: Vec<AuthoredSet> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let error = |what: &str| format!("Symbol sets, line {}: {what}", i + 1);
        match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["set", k, title] => {
                if k.parse() != Ok(sets.len() + 1) {
                    return Err(error("sets are numbered 1, 2, 3 and so on, in order"));
                }
                sets.push(AuthoredSet {
                    title: title.to_string(),
                    shapes: Vec::new(),
                });
            }
            [k, n, label, runs] => {
                let set = k
                    .parse::<usize>()
                    .ok()
                    .and_then(|k| sets.get_mut(k.checked_sub(1)?))
                    .ok_or_else(|| error("a shape before its set's line"))?;
                let mask = n
                    .parse::<u8>()
                    .ok()
                    .filter(|&n| n < 16)
                    .ok_or_else(|| error("masks are 0 to 15"))?;
                if set.shapes.iter().any(|s| s.mask == mask) {
                    return Err(error("a mask drawn twice in one set"));
                }
                let runs = runs
                    .split(" | ")
                    .map(|run| {
                        run.split_whitespace()
                            .map(|point| {
                                let (x, y) = point.split_once(',')?;
                                Some(
                                    [x.parse::<i32>().ok()?, y.parse::<i32>().ok()?]
                                        .map(|v| v as f32 / 10.0),
                                )
                            })
                            .collect::<Option<Vec<_>>>()
                            .filter(|run| !run.is_empty())
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| error("runs are points x,y separated by spaces"))?;
                set.shapes.push(AuthoredShape {
                    mask,
                    label: label.to_string(),
                    runs,
                });
            }
            _ => return Err(error("expected a set line or a shape line")),
        }
    }
    Ok(sets)
}

/// Where a point of an authored drawing's 100-unit box lies on the screen,
/// in source pixels: the box [`AUTHORED_BOX`] rows high on the screen's
/// centre, (160, 100), and a unit the same length across on the 4:3 display
/// (6/5 as many columns as rows).
pub fn authored_source(p: [f32; 2]) -> [f32; 2] {
    let unit = AUTHORED_BOX / 100.0;
    [
        160.0 + (p[0] - 50.0) * unit / PIXEL_ASPECT,
        100.0 + (p[1] - 50.0) * unit,
    ]
}

/// The authored set, from 0, shown in symbol window `index` (an index into
/// [`RingsPreview::windows`]), if any: set `k` in the windows of the tune's
/// pattern `k` from [`NEW_SET_FIRST_WINDOW`].
pub fn window_set(index: usize) -> Option<usize> {
    let k = index.checked_sub(NEW_SET_FIRST_WINDOW)? / PATTERN_WINDOWS;
    (k < NEW_SETS).then_some(k)
}

/// The atlas layer that draws `mask` in symbol window `index`: the authored
/// set's drawing of that mask in the middle of the part (unless `original`),
/// else the original mask. The music's choice of mask is kept either way.
pub fn window_layer(index: usize, mask: u8, original: bool) -> usize {
    match window_set(index) {
        Some(k) if !original => 16 + 16 * k + mask as usize,
        _ => mask as usize,
    }
}

impl SymbolFields {
    /// The authored sets' layers, `NEW_SETS` × 16 (set `k`, from 0, mask
    /// `n` at `16 k + n`), made from their runs as drawn, with the drawing's
    /// box [`AUTHORED_BOX`] rows high on the screen's centre and round on
    /// the 4:3 display. The originals' thinning, tracing and redrawing don't
    /// apply: the runs are made into neon tubes directly, single runs that
    /// never close or join ([`neon_runs`]). A run whose last point is its
    /// first is a loop, and a run of one point is a dot. Masks a set leaves
    /// out, and sets not drawn, are empty.
    pub fn authored(sets: &[AuthoredSet], detail: SymbolDetail) -> Self {
        let mut shapes: Vec<Option<&AuthoredShape>> = vec![None; NEW_SETS * 16];
        for (k, set) in sets.iter().enumerate().take(NEW_SETS) {
            for shape in &set.shapes {
                shapes[k * 16 + shape.mask as usize] = Some(shape);
            }
        }
        // The runs' extent, and the strokes' half-width round them, as the
        // originals' masks have; then the same margin.
        let points = || {
            shapes
                .iter()
                .flatten()
                .flat_map(|s| s.runs.iter().flatten())
        };
        let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        for &p in points() {
            let q = authored_source(p);
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        if lo[0] > hi[0] {
            (lo, hi) = ([150.0, 90.0], [170.0, 110.0]);
        }
        let reach = [NEON_STROKE / PIXEL_ASPECT, NEON_STROKE];
        let pad = SYMBOL_PAD as i32;
        let origin: [i32; 2] = std::array::from_fn(|k| (lo[k] - reach[k]).floor() as i32 - pad);
        let size: [usize; 2] =
            std::array::from_fn(|k| ((hi[k] + reach[k]).ceil() as i32 + pad - origin[k]) as usize);
        Self::build(shapes.len(), origin, size, detail, |i| {
            let Some(shape) = shapes[i] else {
                return empty_layer(size, detail);
            };
            // Positions in rows from the atlas origin, as symbol_fields
            // measures its texels.
            let local = |p: [f32; 2]| {
                let q = authored_source(p);
                [
                    (q[0] - origin[0] as f32) * PIXEL_ASPECT,
                    q[1] - origin[1] as f32,
                ]
            };
            let runs = neon_runs(
                shape
                    .runs
                    .iter()
                    .map(|run| {
                        let mut points: Vec<[f32; 2]> = run.iter().map(|&p| local(p)).collect();
                        let closed = points.len() > 2 && run[0] == run[run.len() - 1];
                        if closed {
                            points.pop();
                        }
                        Run { points, closed }
                    })
                    .collect(),
            );
            let s = SYMBOL_SCALE;
            let (fw, fh) = (size[0] * s, size[1] * s);
            let (best, _, ends) = run_distances(&runs, fw, fh, s);
            // The stroke is taken as NEON_STROKE round each run, as
            // rings_common.wgsl reads the authored layers.
            let texels: Vec<[f32; 4]> = (0..fw * fh)
                .map(|i| {
                    let d = best[i];
                    let half = if d < SYMBOL_LIMIT { NEON_STROKE } else { 0.0 };
                    [(NEON_STROKE - d).max(-SYMBOL_LIMIT), d, half, ends[i]]
                })
                .collect();
            symbol_layer(&texels, origin, size, detail)
        })
    }
}

// ---- Neon signs from the drawn runs -----------------------------------------
// A neon tube is one run, bent but never branching or closing (the owner's
// direction, as for the originals: rings::break_junctions and open_loop),
// so the drawn runs are made into tubes like a sign maker's. Authored rules,
// in display rows:
// 1. Runs whose ends meet are one tube, bent there; where three or more ends
//    meet, the two that continue each other most nearly straight are.
// 2. An end whose glass touches another tube stops short of it, leaving that
//    tube's glass, its own and a stroke's half-width of air (GAP), as the
//    originals' cut tubes do.
// 3. Where tubes still cross or touch, the shorter one (or a run's later
//    pass over itself) breaks there with the same gap each side.
// 4. A loop still closed opens at the middle of its lowest stretch, where a
//    sign maker would hide the tube's return, with the same gap.
// What the cuts leave is kept, however short, unless under half a row.

/// The run spacing the pass works at, in rows.
const STEP: f32 = 0.25;
/// Ends this close, in rows, are one point of the drawing.
const JOIN: f32 = 1.0;
/// Between a cut tube's end and another tube's centreline, in rows.
const GAP: f32 = RETURN;
/// Glass touches glass closer than this, in rows.
const TOUCH: f32 = 2.0 * NEON_RADIUS;

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The drawn runs as neon tubes; see the rules above.
fn neon_runs(runs: Vec<Run>) -> Vec<Run> {
    // Dots stay as drawn; the rest are resampled every STEP rows.
    let (dots, runs): (Vec<Run>, Vec<Run>) = runs.into_iter().partition(|r| r.points.len() < 2);
    let mut runs: Vec<Run> = runs.into_iter().map(resample).collect();
    join_ends(&mut runs);
    cut_ends(&mut runs);
    break_crossings(&mut runs);
    let mut runs: Vec<Run> = runs
        .into_iter()
        .map(|r| if r.closed { open_lowest(r) } else { r })
        .filter(|r| arc_lengths(r).last().copied().unwrap_or(0.0) >= 0.5)
        .collect();
    runs.extend(dots);
    runs
}

/// A run with points no more than STEP apart, keeping its own.
fn resample(run: Run) -> Run {
    let n = run.points.len();
    let segments = if run.closed { n } else { n - 1 };
    let mut points = vec![run.points[0]];
    for k in 0..segments {
        let (a, b) = (run.points[k], run.points[(k + 1) % n]);
        let steps = (distance(a, b) / STEP).ceil().max(1.0) as usize;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            points.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    if run.closed {
        points.pop();
    }
    Run {
        points,
        closed: run.closed,
    }
}

/// Distance along a run to each point from its first.
fn arc_lengths(run: &Run) -> Vec<f32> {
    let mut total = 0.0;
    let mut out = Vec::with_capacity(run.points.len());
    for (k, &p) in run.points.iter().enumerate() {
        if k > 0 {
            total += distance(run.points[k - 1], p);
        }
        out.push(total);
    }
    out
}

/// Every run's points in cells GAP wide, for finding near ones.
struct Near {
    cells: std::collections::HashMap<(i32, i32), Vec<(usize, usize)>>,
    arcs: Vec<Vec<f32>>,
    lengths: Vec<f32>,
}

impl Near {
    fn new(runs: &[Run]) -> Self {
        let mut cells: std::collections::HashMap<(i32, i32), Vec<(usize, usize)>> =
            Default::default();
        for (i, run) in runs.iter().enumerate() {
            for (k, p) in run.points.iter().enumerate() {
                let cell = ((p[0] / GAP).floor() as i32, (p[1] / GAP).floor() as i32);
                cells.entry(cell).or_default().push((i, k));
            }
        }
        let arcs: Vec<Vec<f32>> = runs.iter().map(arc_lengths).collect();
        let lengths = runs
            .iter()
            .zip(&arcs)
            .map(|(r, a)| {
                let last = a.last().copied().unwrap_or(0.0);
                if r.closed {
                    last + distance(r.points[r.points.len() - 1], r.points[0])
                } else {
                    last
                }
            })
            .collect();
        Self {
            cells,
            arcs,
            lengths,
        }
    }

    /// How far along run `i` points `a` and `b` lie from each other.
    fn apart(&self, runs: &[Run], i: usize, a: usize, b: usize) -> f32 {
        let d = (self.arcs[i][a] - self.arcs[i][b]).abs();
        if runs[i].closed {
            d.min(self.lengths[i] - d)
        } else {
            d
        }
    }

    /// The nearest point to `p` (within about GAP) that `keep` accepts:
    /// its distance, run and index.
    fn nearest(
        &self,
        runs: &[Run],
        p: [f32; 2],
        keep: impl Fn(usize, usize) -> bool,
    ) -> Option<(f32, usize, usize)> {
        let (cx, cy) = ((p[0] / GAP).floor() as i32, (p[1] / GAP).floor() as i32);
        let mut best: Option<(f32, usize, usize)> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                for &(j, b) in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    if !keep(j, b) {
                        continue;
                    }
                    let d = distance(p, runs[j].points[b]);
                    if best.is_none_or(|(e, _, _)| d < e) {
                        best = Some((d, j, b));
                    }
                }
            }
        }
        best
    }

    /// Whether points `a` and `b` of run `i` are separate passes of it: the
    /// run goes more than 2 GAP from both between them. The arms of one bend
    /// are not, however sharp.
    fn separate(&self, runs: &[Run], i: usize, a: usize, b: usize) -> bool {
        if self.apart(runs, i, a, b) <= 2.0 * GAP {
            return false;
        }
        let points = &runs[i].points;
        let (pa, pb) = (points[a], points[b]);
        let n = points.len();
        let (lo, hi) = (a.min(b), a.max(b));
        // Between them the shorter way round, for a loop.
        let between: Box<dyn Iterator<Item = usize>> =
            if runs[i].closed && self.arcs[i][hi] - self.arcs[i][lo] > self.lengths[i] / 2.0 {
                Box::new((hi..n).chain(0..=lo))
            } else {
                Box::new(lo..=hi)
            };
        let far = 2.0 * GAP;
        between
            .into_iter()
            .any(|k| distance(points[k], pa) > far && distance(points[k], pb) > far)
    }

    /// The distance from run `i`'s point `a` to every other tube, and to
    /// separate passes of its own run.
    fn clearance(&self, runs: &[Run], i: usize, a: usize) -> f32 {
        self.nearest(runs, runs[i].points[a], |j, b| {
            j != i || self.separate(runs, i, a, b)
        })
        .map_or(f32::INFINITY, |(d, _, _)| d)
    }
}

/// Rule 1: join runs whose ends meet, the straightest pair at each point.
fn join_ends(runs: &mut Vec<Run>) {
    loop {
        // Each open run's ends: run, which end, point and inward direction.
        let mut ends = Vec::new();
        for (i, run) in runs.iter().enumerate() {
            if run.closed || run.points.len() < 2 {
                continue;
            }
            let n = run.points.len();
            let reach = (3.0 / STEP) as usize;
            for (last, at, inner) in [
                (false, 0, reach.min(n - 1)),
                (true, n - 1, n - 1 - reach.min(n - 1)),
            ] {
                let (p, q) = (run.points[at], run.points[inner]);
                let l = distance(p, q).max(1e-6);
                ends.push((i, last, p, [(q[0] - p[0]) / l, (q[1] - p[1]) / l]));
            }
        }
        // The straightest pair of meeting ends, if any meet.
        let mut pair: Option<(f32, usize, usize)> = None;
        for a in 0..ends.len() {
            for b in a + 1..ends.len() {
                let (ea, eb) = (&ends[a], &ends[b]);
                if distance(ea.2, eb.2) > JOIN || (ea.0 == eb.0 && runs[ea.0].points.len() < 3) {
                    continue;
                }
                let dot = ea.3[0] * eb.3[0] + ea.3[1] * eb.3[1];
                if pair.is_none_or(|(d, _, _)| dot < d) {
                    pair = Some((dot, a, b));
                }
            }
        }
        let Some((_, a, b)) = pair else { return };
        let ((i, i_last, _, _), (j, j_last, _, _)) = (ends[a], ends[b]);
        if i == j {
            // A run's own two ends: a loop.
            runs[i].points.pop();
            runs[i].closed = true;
            continue;
        }
        let mut first = std::mem::take(&mut runs[i].points);
        let mut second = std::mem::take(&mut runs[j].points);
        if !i_last {
            first.reverse();
        }
        if j_last {
            second.reverse();
        }
        first.extend(second.into_iter().skip(1));
        runs[i].points = first;
        runs.remove(j);
    }
}

/// Rule 2: cut back each end whose glass touches another tube until it
/// clears it by GAP; a run cut away entirely goes.
fn cut_ends(runs: &mut Vec<Run>) {
    let mut i = 0;
    while i < runs.len() {
        for last in [false, true] {
            if runs[i].closed || runs[i].points.len() < 2 {
                break;
            }
            let near = Near::new(runs);
            let n = runs[i].points.len();
            let at = |k: usize| if last { n - 1 - k } else { k };
            if near.clearance(runs, i, at(0)) >= TOUCH {
                continue;
            }
            // Walk in until clear, but no further than 3 GAP: past that the
            // tube runs alongside the other, which is no join.
            let arcs = &near.arcs[i];
            let mut k = 0;
            while k < n && near.clearance(runs, i, at(k)) < GAP {
                if (arcs[at(k)] - arcs[at(0)]).abs() > 3.0 * GAP {
                    break;
                }
                k += 1;
            }
            let points = &mut runs[i].points;
            if last {
                points.truncate(n - k);
            } else {
                points.drain(..k);
            }
        }
        if runs[i].points.len() < 2 {
            runs.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Rule 3: where tubes still touch, break the shorter (or a run's later
/// pass over itself) with GAP each side of the other.
fn break_crossings(runs: &mut Vec<Run>) {
    for _ in 0..10_000 {
        let near = Near::new(runs);
        let mut contact = None;
        'search: for (i, run) in runs.iter().enumerate() {
            for a in 0..run.points.len() {
                let found = near.nearest(runs, run.points[a], |j, b| {
                    j != i || near.separate(runs, i, a, b)
                });
                if let Some((d, j, b)) = found
                    && d < TOUCH
                {
                    contact = Some((i, a, j, b));
                    break 'search;
                }
            }
        }
        let Some((i, a, j, b)) = contact else { return };
        // The run that breaks, at which point, and the tube it clears.
        let (x, c, y) = if i == j {
            (i, a.max(b), i)
        } else if near.lengths[i] < near.lengths[j] || (near.lengths[i] == near.lengths[j] && i > j)
        {
            (i, a, j)
        } else {
            (j, b, i)
        };
        let n = runs[x].points.len();
        let clear = |k: usize| {
            near.nearest(runs, runs[x].points[k], |r, q| {
                r == y && (r != x || near.separate(runs, x, k, q))
            })
            .is_none_or(|(d, _, _)| d >= GAP)
        };
        // The stretch round c within GAP of the other tube, both ways.
        let wrap = runs[x].closed;
        let step = |k: usize, forward: bool| -> Option<usize> {
            match (forward, wrap) {
                (true, _) if k + 1 < n => Some(k + 1),
                (true, true) => Some(0),
                (false, _) if k > 0 => Some(k - 1),
                (false, true) => Some(n - 1),
                _ => None,
            }
        };
        let (mut lo, mut hi, mut count) = (c, c, 1);
        while count < n {
            match step(lo, false) {
                Some(k) if !clear(k) => lo = k,
                _ => break,
            }
            count += 1;
        }
        while count < n {
            match step(hi, true) {
                Some(k) if !clear(k) => hi = k,
                _ => break,
            }
            count += 1;
        }
        let run = runs.remove(x);
        if count >= n {
            continue; // All of it lies on the other tube.
        }
        let pieces: Vec<Vec<[f32; 2]>> = if wrap {
            // What is left runs from past hi round to before lo.
            let mut piece = Vec::new();
            let mut k = (hi + 1) % n;
            while k != lo {
                piece.push(run.points[k]);
                k = (k + 1) % n;
            }
            vec![piece]
        } else {
            vec![run.points[..lo].to_vec(), run.points[hi + 1..].to_vec()]
        };
        for points in pieces.into_iter().filter(|p| p.len() >= 2) {
            runs.push(Run {
                points,
                closed: false,
            });
        }
    }
}

/// Rule 4: open a loop at the middle of its lowest stretch, its new ends
/// GAP apart (across, so a sharp corner opens wider along the tube). A loop
/// too small for that keeps half of itself, an arch.
fn open_lowest(run: Run) -> Run {
    let n = run.points.len();
    let y = |k: usize| run.points[k % n][1];
    let lowest = (0..n).max_by(|&a, &b| y(a).total_cmp(&y(b))).unwrap();
    let flat = |k: usize| y(k) >= y(lowest) - STEP;
    // The stretch about as low, round the lowest point, and its middle.
    let (mut back, mut on) = (0, 0);
    while back < n && flat(lowest + n - back - 1) {
        back += 1;
    }
    while on < n && flat(lowest + on + 1) {
        on += 1;
    }
    let middle = (lowest + n - back + (back + on) / 2) % n;
    let mut ring: Vec<[f32; 2]> = (0..n).map(|k| run.points[(middle + k) % n]).collect();
    ring.push(ring[0]);
    // Step the two new ends back in turn until they are GAP apart.
    let (mut start, mut end) = (0, ring.len() - 1);
    while start + (ring.len() - 1 - end) < n / 2 && distance(ring[start], ring[end]) < GAP {
        if start + ring.len() - 1 - end <= 2 * start {
            end -= 1;
        } else {
            start += 1;
        }
    }
    Run {
        points: ring[start..=end].to_vec(),
        closed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_sets_parse_and_reject_bad_lines() {
        let text = "# A comment\n\nset\t1\tInvented\n1\t3\ta bar and a dot\t0,500 1000,500 | 500,100\nset\t2\tEmpty\n";
        let sets = parse_authored(text).unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].title, "Invented");
        assert_eq!(
            sets[0].shapes,
            [AuthoredShape {
                mask: 3,
                label: "a bar and a dot".into(),
                runs: vec![vec![[0.0, 50.0], [100.0, 50.0]], vec![[50.0, 10.0]]],
            }]
        );
        assert!(sets[1].shapes.is_empty());
        for bad in [
            "set\t2\tOut of order\n",
            "1\t0\tno set yet\t0,0 10,10\n",
            "set\t1\tA\n1\t16\tmask too big\t0,0\n",
            "set\t1\tA\n1\t0\tA\t0,0\n1\t0\ttwice\t0,0\n",
            "set\t1\tA\n1\t0\tbad point\t0;0\n",
            "set\t1\tA\n1\t0\tempty run\t0,0 |  | 5,5\n",
            "just words\n",
        ] {
            assert!(parse_authored(bad).is_err(), "{bad:?}");
        }
        // The committed file parses, with every set in order.
        let sets = parse_authored(AUTHORED_SYMBOLS).unwrap();
        assert_eq!(sets.len(), NEW_SETS);
        for set in &sets {
            assert!(set.shapes.iter().all(|s| s.mask < 16 && !s.runs.is_empty()));
        }
    }

    /// An invented set's runs as authored layers, in full detail, and a
    /// reader of one layer's texel at a source point.
    fn invented_layers(
        shapes: Vec<AuthoredShape>,
    ) -> (SymbolFields, impl Fn(usize, [f32; 2]) -> [f32; 4]) {
        let sets = [AuthoredSet {
            title: "Invented".into(),
            shapes,
        }];
        let fields = SymbolFields::authored(&sets, SymbolDetail::Full);
        let (tw, th) = fields.texels();
        let (origin, atlas) = (fields.origin, fields.atlas.clone());
        let texel = move |layer: usize, q: [f32; 2]| {
            let i = ((q[0] - origin[0] as f32) * SYMBOL_SCALE as f32) as usize;
            let j = ((q[1] - origin[1] as f32) * SYMBOL_SCALE as f32) as usize;
            let at = ((layer * th as usize + j) * tw as usize + i) * 4;
            [0, 1, 2, 3].map(|k| atlas[at + k].to_f32())
        };
        (fields, texel)
    }

    fn shape(mask: u8, runs: Vec<Vec<[f32; 2]>>) -> AuthoredShape {
        AuthoredShape {
            mask,
            label: String::new(),
            runs,
        }
    }

    /// Runs in rows, open, from point lists.
    fn rows(runs: &[&[[f32; 2]]]) -> Vec<Run> {
        runs.iter()
            .map(|p| Run {
                points: p.to_vec(),
                closed: false,
            })
            .collect()
    }

    /// The least distance from p to any of the runs, in rows.
    fn to_tubes(runs: &[Run], p: [f32; 2]) -> f32 {
        runs.iter()
            .flat_map(|r| &r.points)
            .map(|&q| distance(p, q))
            .fold(f32::INFINITY, f32::min)
    }

    /// Glass never touches: points of different tubes, or of one tube more
    /// than 2 GAP apart along it, stay at least a tube's width apart.
    fn glass_apart(runs: &[Run]) -> Option<f32> {
        let near = Near::new(runs);
        let mut least = f32::INFINITY;
        for (i, run) in runs.iter().enumerate() {
            for a in 0..run.points.len() {
                least = least.min(near.clearance(runs, i, a));
            }
        }
        (least < TOUCH - 0.01).then_some(least)
    }

    #[test]
    fn authored_runs_become_neon_signs() {
        // A + of two bars: one runs through, the other breaks either side
        // with GAP to the first's centreline.
        let plus = neon_runs(rows(&[
            &[[100.0, 100.0], [180.0, 100.0]],
            &[[140.0, 70.0], [140.0, 130.0]],
        ]));
        assert_eq!(plus.len(), 3, "the longer bar and two halves");
        assert!(to_tubes(&plus, [140.0, 100.0]) < 0.3);
        assert!(to_tubes(&plus, [140.0, 100.0 - GAP + 0.75]) > 0.5);
        assert!(to_tubes(&plus, [140.0, 100.0 - GAP - 0.5]) < 0.3);
        assert_eq!(glass_apart(&plus), None);
        // A T: the stem ending on the bar stops GAP short of it.
        let tee = neon_runs(rows(&[
            &[[100.0, 100.0], [180.0, 100.0]],
            &[[140.0, 100.0], [140.0, 140.0]],
        ]));
        assert_eq!(tee.len(), 2);
        let stem = tee.iter().find(|r| r.points[0][0] == 140.0).unwrap();
        let top = stem
            .points
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        assert!((top - (100.0 + GAP)).abs() < STEP + 1e-3, "{top}");
        // Two runs meeting end to end are one bent tube, with no end there.
        let ell = neon_runs(rows(&[
            &[[100.0, 100.0], [140.0, 100.0]],
            &[[140.0, 100.0], [140.0, 140.0]],
        ]));
        assert_eq!(ell.len(), 1);
        let (first, last) = (ell[0].points[0], ell[0].points[ell[0].points.len() - 1]);
        assert!(distance(first, [100.0, 100.0]) < 1e-3 && distance(last, [140.0, 140.0]) < 1e-3);
        // A closed square, and a loop of two arcs: each opens at the middle
        // of its lowest stretch with a gap of GAP along it.
        let square = neon_runs(vec![Run {
            points: vec![
                [100.0, 100.0],
                [140.0, 100.0],
                [140.0, 140.0],
                [100.0, 140.0],
            ],
            closed: true,
        }]);
        assert_eq!(square.len(), 1);
        assert!(!square[0].closed);
        let (first, last) = (
            square[0].points[0],
            square[0].points[square[0].points.len() - 1],
        );
        assert!((first[1] - 140.0).abs() < 1e-3 && (last[1] - 140.0).abs() < 1e-3);
        assert!(
            (first[0] - (120.0 - GAP / 2.0)).abs() < STEP + 1e-3,
            "{first:?}"
        );
        assert!(
            (last[0] - (120.0 + GAP / 2.0)).abs() < STEP + 1e-3,
            "{last:?}"
        );
        let arc = |a0: f32, a1: f32| -> Vec<[f32; 2]> {
            (0..=20)
                .map(|k| {
                    let a = (a0 + (a1 - a0) * k as f32 / 20.0).to_radians();
                    [140.0 + 30.0 * a.cos(), 100.0 + 30.0 * a.sin()]
                })
                .collect()
        };
        let (upper, lower) = (arc(180.0, 360.0), arc(0.0, 180.0));
        let ring = neon_runs(rows(&[&upper, &lower]));
        assert_eq!(ring.len(), 1);
        assert!(
            to_tubes(&ring, [140.0, 130.0]) > GAP / 2.0 - 0.5,
            "open at the bottom"
        );
        assert!(to_tubes(&ring, [140.0, 70.0]) < 0.3, "whole at the top");
        // A dot stays a dot.
        let dot = neon_runs(rows(&[&[[150.0, 150.0]]]));
        assert_eq!(dot.len(), 1);
        assert_eq!(dot[0].points, [[150.0, 150.0]]);
    }

    #[test]
    fn authored_layers_measure_the_neon_tubes() {
        // A bar and a closed square, through the atlas: the bar's ends and
        // the square's opening are free ends; the rest is a tube.
        let (fields, texel) = invented_layers(vec![
            shape(5, vec![vec![[10.0, 50.0], [90.0, 50.0]]]),
            shape(
                7,
                vec![vec![
                    [20.0, 20.0],
                    [80.0, 20.0],
                    [80.0, 80.0],
                    [20.0, 80.0],
                    [20.0, 20.0],
                ]],
            ),
        ]);
        assert_eq!(fields.layers, NEW_SETS * 16);
        let at = |p: [f32; 2], down: f32| {
            let q = authored_source(p);
            [q[0] + 0.25, q[1] + 0.25 + down]
        };
        for x in [12.0, 30.0, 50.0, 70.0, 88.0] {
            assert!(texel(5, at([x, 50.0], 0.0))[1] < 0.5, "bar at {x}");
            assert!((texel(5, at([x, 50.0], 3.0))[1] - 3.0).abs() < 0.5);
        }
        let a = authored_source([10.0, 50.0]);
        let b = authored_source([90.0, 50.0]);
        let unit = AUTHORED_BOX / 100.0;
        assert!(
            ((b[0] - a[0]) * PIXEL_ASPECT - 80.0 * unit).abs() < 1e-3,
            "round"
        );
        assert!((a[1] - 100.0).abs() < 1e-3 && (a[0] + b[0] - 320.0).abs() < 1e-3);
        assert!(texel(5, at([10.0, 50.0], 0.0))[3] < 0.5);
        assert_eq!(texel(5, at([50.0, 50.0], 0.0))[3], SYMBOL_LIMIT);
        let middle = texel(5, at([50.0, 50.0], 1.0));
        assert!((middle[0] - (NEON_STROKE - middle[1])).abs() < 1e-2);
        assert!((middle[2] - NEON_STROKE).abs() < 1e-2);
        // The square: its corners are tube, its bottom middle the gap, with
        // free ends either side of it.
        assert!(texel(7, at([20.0, 20.0], 0.0))[1] < 0.5);
        assert!(texel(7, at([80.0, 80.0], 0.0))[1] < 0.5);
        let gap = texel(7, at([50.0, 80.0], 0.0));
        assert!(
            gap[1] > GAP / 2.0 - 0.5 && gap[3] < GAP / 2.0 + 0.5,
            "{gap:?}"
        );
        // Masks left out are far everywhere, and dark in the light map.
        assert_eq!(
            texel(0, at([50.0, 50.0], 0.0))[..2],
            [-SYMBOL_LIMIT, SYMBOL_LIMIT]
        );
        let (lw, lh) = (LIGHT_SIZE[0] as usize, LIGHT_SIZE[1] as usize);
        let light = |layer: usize, q: [f32; 2]| {
            let (i, j) = (
                (q[0] as i32 - LIGHT_ORIGIN[0]) as usize,
                (q[1] as i32 - LIGHT_ORIGIN[1]) as usize,
            );
            fields.light[0][(layer * lh + j) * lw + i].to_f32()
        };
        assert!(light(5, at([50.0, 50.0], 0.0)) > 0.9);
        assert_eq!(light(0, at([50.0, 50.0], 0.0)), 0.0);
        assert_eq!(
            SymbolFields::authored(&[], SymbolDetail::Tubes).layers,
            NEW_SETS * 16
        );
    }

    #[test]
    fn every_authored_drawing_fits_the_frame_as_a_neon_sign() {
        // Each run's tube, its glass included, inside the 4:3 frame.
        let sets = parse_authored(AUTHORED_SYMBOLS).unwrap();
        let reach = [NEON_RADIUS / PIXEL_ASPECT, NEON_RADIUS];
        let mut lost = Vec::new();
        for (k, set) in sets.iter().enumerate() {
            for shape in &set.shapes {
                let name = format!("set {} mask {} ({})", k + 1, shape.mask, shape.label);
                for &p in shape.runs.iter().flatten() {
                    let q = authored_source(p);
                    assert!(
                        q[0] - reach[0] >= 0.0
                            && q[0] + reach[0] <= 320.0
                            && q[1] - reach[1] >= 0.0
                            && q[1] + reach[1] <= 200.0,
                        "{name} leaves the frame at {q:?}"
                    );
                }
                // As neon: no loops, no glass touching, and something of
                // every drawn run left, but for runs too short to keep a gap
                // at both ends.
                let rows_of = |p: [f32; 2]| {
                    let q = authored_source(p);
                    [q[0] * PIXEL_ASPECT, q[1]]
                };
                let drawn: Vec<Run> = shape
                    .runs
                    .iter()
                    .map(|run| {
                        let mut points: Vec<[f32; 2]> = run.iter().map(|&p| rows_of(p)).collect();
                        let closed = points.len() > 2 && run[0] == run[run.len() - 1];
                        if closed {
                            points.pop();
                        }
                        Run { points, closed }
                    })
                    .collect();
                let tubes = neon_runs(drawn);
                assert!(tubes.iter().all(|r| !r.closed), "{name}: a loop");
                assert_eq!(glass_apart(&tubes), None, "{name}: glass touches");
                for (r, run) in shape.runs.iter().enumerate() {
                    let drawn = resample(Run {
                        points: run.iter().map(|&p| rows_of(p)).collect(),
                        closed: false,
                    });
                    let length = arc_lengths(&drawn).last().copied().unwrap_or(0.0);
                    let kept = drawn.points.iter().any(|&p| to_tubes(&tubes, p) < 0.5);
                    if !kept && length >= 2.0 * GAP {
                        lost.push(format!("{name} run {r}: {length:.1} rows"));
                    }
                }
            }
        }
        assert!(lost.is_empty(), "runs lost:\n{}", lost.join("\n"));
    }

    #[test]
    fn tube_detail_keeps_the_tubes_fields_at_half_the_light() {
        // An invented ring and bar, in full and in tube detail.
        let ring = (0..=36)
            .map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 36.0;
                [50.0 + 30.0 * a.cos(), 50.0 + 30.0 * a.sin()]
            })
            .collect();
        let sets = [AuthoredSet {
            title: "Invented".into(),
            shapes: vec![shape(0, vec![ring, vec![[10.0, 95.0], [90.0, 95.0]]])],
        }];
        let full = SymbolFields::authored(&sets, SymbolDetail::Full);
        let tubes = SymbolFields::authored(&sets, SymbolDetail::Tubes);
        assert_eq!((full.origin, full.size), (tubes.origin, tubes.size));
        assert_eq!(tubes.atlas.len() * 2, full.atlas.len());
        let (two, four) = (
            tubes.atlas.as_chunks::<2>().0,
            full.atlas.as_chunks::<4>().0,
        );
        for (t, f) in two.iter().zip(four) {
            assert_eq!(*t, [f[1], f[3]]);
        }
        // The tubes' light map: half the texels each way, one level fewer,
        // and the same light in all; none in the empty layers.
        assert_eq!(SymbolDetail::Tubes.light_size(), LIGHT_SIZE.map(|n| n / 2));
        assert_eq!(tubes.light.len() + 1, full.light.len());
        let total = |level: &[f16], scale: f32| {
            level.iter().map(|v| v.to_f32()).sum::<f32>() * scale * scale
        };
        let (fl, tl) = (&full.light[0], &tubes.light[0]);
        let (fn_, tn) = (fl.len() / full.layers, tl.len() / tubes.layers);
        let (a, b) = (total(&fl[..fn_], 1.0), total(&tl[..tn], 2.0));
        assert!(a > 100.0 && (a / b - 1.0).abs() < 1e-3, "{a} {b}");
        assert!(fl[fn_..].iter().chain(&tl[tn..]).all(|v| v.to_f32() == 0.0));
        assert!(tubes.bytes() * 2 < full.bytes());
        // An empty original mask skips the tracing, which would give the
        // same far field.
        let empty = vec![false; 320 * 200];
        let fields = SymbolFields::new(std::slice::from_ref(&empty), SymbolDetail::Full).unwrap();
        let traced = symbol_fields(&empty, fields.origin, fields.size);
        assert!(
            traced
                .iter()
                .all(|t| *t == [-SYMBOL_LIMIT, SYMBOL_LIMIT, 0.0, SYMBOL_LIMIT])
        );
        for (k, v) in fields.atlas.iter().enumerate() {
            assert_eq!(v.to_f32(), traced[k / 4][k % 4]);
        }
    }

    /// A contact sheet of the authored sets' tubes, for looking at: per
    /// mask, the tubes' glass amber and their centrelines white, free ends
    /// red. Writes artifacts/new-symbols/tubes.png.
    #[test]
    #[ignore = "writes a contact sheet under artifacts/; run with --ignored"]
    fn authored_tube_sheet() {
        let sets = parse_authored(AUTHORED_SYMBOLS).unwrap();
        let fields = SymbolFields::authored(&sets, SymbolDetail::Tubes);
        let (tw, th) = fields.texels();
        let (cw, ch) = (tw as usize, th as usize);
        let (sw, sh) = (16 * cw, NEW_SETS * ch);
        let mut sheet = vec![0_u8; sw * sh * 3];
        for layer in 0..fields.layers {
            let (k, n) = (layer / 16, layer % 16);
            for j in 0..ch {
                for i in 0..cw {
                    let t = ((layer * ch + j) * cw + i) * 2;
                    let (d, end) = (fields.atlas[t].to_f32(), fields.atlas[t + 1].to_f32());
                    let mut rgb = if d < 0.6 {
                        [255, 255, 255]
                    } else if d < NEON_RADIUS {
                        [200, 140, 30]
                    } else {
                        [0, 0, 0]
                    };
                    if end < 1.5 && d < NEON_RADIUS {
                        rgb = [255, 40, 40];
                    }
                    if i == 0 || j == 0 {
                        rgb = [40, 40, 40];
                    }
                    let at = ((k * ch + j) * sw + n * cw + i) * 3;
                    sheet[at..at + 3].copy_from_slice(&rgb);
                }
            }
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts/new-symbols");
        std::fs::create_dir_all(&dir).unwrap();
        let file = std::fs::File::create(dir.join("tubes.png")).unwrap();
        let mut png = png::Encoder::new(std::io::BufWriter::new(file), sw as u32, sh as u32);
        png.set_color(png::ColorType::Rgb);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&sheet)
            .unwrap();
        println!(
            "{} x {} texels per layer, origin {:?}",
            cw, ch, fields.origin
        );
        // The drawn runs that the neon rules leave nothing of.
        let rows_of = |p: [f32; 2]| {
            let q = authored_source(p);
            [q[0] * PIXEL_ASPECT, q[1]]
        };
        for (k, set) in sets.iter().enumerate() {
            for shape in &set.shapes {
                let drawn = || -> Vec<Run> {
                    shape
                        .runs
                        .iter()
                        .map(|run| {
                            let mut points: Vec<[f32; 2]> =
                                run.iter().map(|&p| rows_of(p)).collect();
                            let closed = points.len() > 2 && run[0] == run[run.len() - 1];
                            if closed {
                                points.pop();
                            }
                            Run { points, closed }
                        })
                        .collect()
                };
                let tubes = neon_runs(drawn());
                for (r, run) in drawn().into_iter().enumerate() {
                    let run = if run.points.len() > 1 {
                        resample(run)
                    } else {
                        run
                    };
                    let near = |p: [f32; 2]| {
                        tubes
                            .iter()
                            .flat_map(|t| &t.points)
                            .any(|&q| distance(p, q) < 0.5)
                    };
                    if !run.points.iter().any(|&p| near(p)) {
                        println!(
                            "lost: set {} mask {} ({}) run {r}, {:.1} rows",
                            k + 1,
                            shape.mask,
                            shape.label,
                            arc_lengths(&run).last().copied().unwrap_or(0.0)
                        );
                    }
                }
            }
        }
    }

    /// How alike each symbol is to the next one the music shows: the
    /// overlap (intersection over union) of their tubes' glass on screen,
    /// for the authored sets and, as a baseline, for the original symbols in
    /// the same windows. Prints the sets' means and their most alike pairs.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA; run with --ignored"]
    fn consecutive_symbols_contrast() {
        use crate::hd::clip::{Clip, ScrollerLighting};
        let data = crate::test_support::data();
        let clip = Clip::load_rings(&data, ScrollerLighting::Studio).unwrap();
        let rings = clip.rings.as_ref().unwrap();
        // Each layer's glass as 320 x 200 source pixels.
        let glass = |fields: &SymbolFields, layer: usize| -> Vec<bool> {
            let (tw, th) = fields.texels();
            let c = fields.detail.channels();
            (0..320 * 200)
                .map(|at| {
                    let (x, y) = ((at % 320) as f32 + 0.5, (at / 320) as f32 + 0.5);
                    let i = ((x - fields.origin[0] as f32) * SYMBOL_SCALE as f32) as i64;
                    let j = ((y - fields.origin[1] as f32) * SYMBOL_SCALE as f32) as i64;
                    if i < 0 || j < 0 || i >= tw as i64 || j >= th as i64 {
                        return false;
                    }
                    let t = ((layer * th as usize + j as usize) * tw as usize + i as usize) * c;
                    fields.atlas[t + c / 4].to_f32() < NEON_RADIUS
                })
                .collect()
        };
        let layer_glass: Vec<Vec<bool>> = (0..SYMBOL_LAYERS)
            .map(|l| {
                if l < ORIGINAL_SYMBOLS {
                    glass(&rings.symbols, l)
                } else {
                    glass(&rings.authored, l - ORIGINAL_SYMBOLS)
                }
            })
            .collect();
        let overlap = |a: usize, b: usize| {
            let (ga, gb) = (&layer_glass[a], &layer_glass[b]);
            let both = ga.iter().zip(gb).filter(|(x, y)| **x && **y).count();
            let either = ga.iter().zip(gb).filter(|(x, y)| **x || **y).count();
            both as f32 / either.max(1) as f32
        };
        let w = &rings.windows;
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
        let originals: Vec<f32> = (1..w.len())
            .filter(|&i| w[i].mask != w[i - 1].mask)
            .map(|i| overlap(w[i - 1].mask as usize, w[i].mask as usize))
            .collect();
        println!(
            "Original symbols, all windows: mean overlap {:.2}",
            mean(&originals)
        );
        for k in 0..NEW_SETS {
            let first = NEW_SET_FIRST_WINDOW + k * PATTERN_WINDOWS;
            let mut new = Vec::new();
            let mut old = Vec::new();
            let mut pairs: std::collections::BTreeMap<(u8, u8), (f32, usize)> = Default::default();
            for i in first + 1..first + PATTERN_WINDOWS {
                let (a, b) = (w[i - 1].mask, w[i].mask);
                let o = overlap(window_layer(i - 1, a, false), window_layer(i, b, false));
                new.push(o);
                old.push(overlap(a as usize, b as usize));
                pairs.entry((a.min(b), a.max(b))).or_insert((o, 0)).1 += 1;
            }
            let set = &rings.symbol_sets[k];
            let label = |n: u8| {
                set.shapes
                    .iter()
                    .find(|s| s.mask == n)
                    .map_or("?", |s| s.label.as_str())
            };
            println!(
                "Set {} ({}): mean overlap {:.2} (the originals here {:.2})",
                k + 1,
                set.title,
                mean(&new),
                mean(&old)
            );
            let mut alike: Vec<_> = pairs.into_iter().collect();
            alike.sort_by(|a, b| b.1.0.total_cmp(&a.1.0));
            for ((a, b), (o, times)) in alike.into_iter().take(4) {
                println!("    {o:.2}  {} / {} ({times} flips)", label(a), label(b));
            }
        }
    }
}
