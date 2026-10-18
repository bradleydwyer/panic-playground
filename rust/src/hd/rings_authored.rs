//! Part 6's authored symbol sets: line drawings made for the remake, not
//! recovered from the demo, which the neon shows in place of the original's
//! 16 symbols in the middle of the part, one set per pattern of the tune;
//! the music still picks which mask lights and when. Some are neon tracings
//! of company logos, which are their owners' trademarks. The drawings are
//! read from rings_symbols.txt (its header gives the format) and made into
//! symbol layers from their runs as drawn ([`SymbolFields::authored`]).
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
    /// `n` at `16 k + n`), made from their runs as drawn: each run is a
    /// tube's centreline, with the drawing's box [`AUTHORED_BOX`] rows high
    /// on the screen's centre and round on the 4:3 display. None of the
    /// originals' redrawing applies (authored, at the owner's direction):
    /// no thinning or tracing, no gaps where runs cross or meet, so their
    /// tubes overlap, no short fragments dropped and no near-circles made
    /// circles. A run whose last point is its first is a closed loop, with
    /// no ends; a run of one point is a dot. Masks a set leaves out, and
    /// sets not drawn, are empty.
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
            let runs: Vec<Run> = shape
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
                .collect();
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

    #[test]
    fn authored_runs_are_the_tubes_as_drawn() {
        // A bar, a +, a closed square, a dot and an overlapping pair.
        let (fields, texel) = invented_layers(vec![
            shape(5, vec![vec![[10.0, 50.0], [90.0, 50.0]]]),
            shape(
                6,
                vec![
                    vec![[20.0, 50.0], [80.0, 50.0]],
                    vec![[50.0, 20.0], [50.0, 80.0]],
                ],
            ),
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
            shape(8, vec![vec![[50.0, 50.0]]]),
        ]);
        assert_eq!(fields.layers, NEW_SETS * 16);
        let unit = AUTHORED_BOX / 100.0;
        let at = |p: [f32; 2], down: f32| {
            let q = authored_source(p);
            [q[0] + 0.25, q[1] + 0.25 + down]
        };
        // The bar: on its centreline across its length, and a row off it a
        // row away; ends only at its two ends, which lie 80 units apart on
        // the display, so round.
        for x in [12.0, 30.0, 50.0, 70.0, 88.0] {
            assert!(texel(5, at([x, 50.0], 0.0))[1] < 0.5, "bar at {x}");
            assert!((texel(5, at([x, 50.0], 3.0))[1] - 3.0).abs() < 0.5);
        }
        let a = authored_source([10.0, 50.0]);
        let b = authored_source([90.0, 50.0]);
        assert!(((b[0] - a[0]) * PIXEL_ASPECT - 80.0 * unit).abs() < 1e-3);
        assert!((a[1] - 100.0).abs() < 1e-3 && (a[0] + b[0] - 320.0).abs() < 1e-3);
        assert!(texel(5, at([10.0, 50.0], 0.0))[3] < 0.5);
        assert_eq!(texel(5, at([50.0, 50.0], 0.0))[3], SYMBOL_LIMIT);
        // The stroke's outline and half-width are taken as NEON_STROKE.
        let middle = texel(5, at([50.0, 50.0], 1.0));
        assert!((middle[0] - (NEON_STROKE - middle[1])).abs() < 1e-2);
        assert!((middle[2] - NEON_STROKE).abs() < 1e-2);
        // The +: both strokes run through the crossing, with no gap.
        for (dx, dy) in [(-4.0, 0.0), (4.0, 0.0), (0.0, -4.0), (0.0, 4.0), (0.0, 0.0)] {
            assert!(
                texel(6, at([50.0 + dx, 50.0 + dy], 0.0))[1] < 0.5,
                "+ at {dx}, {dy}"
            );
        }
        assert_eq!(texel(6, at([50.0, 50.0], 0.0))[3], SYMBOL_LIMIT);
        // The closed square keeps its corners and has no ends.
        assert!(texel(7, at([20.0, 20.0], 0.0))[1] < 0.5);
        assert!(texel(7, at([80.0, 80.0], 0.0))[1] < 0.5);
        let (tw, th) = fields.texels();
        let layer = (tw * th) as usize * 4;
        let square = &fields.atlas[7 * layer..8 * layer];
        assert!(square.chunks(4).all(|t| t[3].to_f32() == SYMBOL_LIMIT));
        // The dot: a tube end at its point.
        let dot = texel(8, at([50.0, 50.0], 0.0));
        assert!(dot[1] < 0.5 && dot[3] < 0.5);
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
        // No runs at all: an empty field at the centre.
        assert_eq!(
            SymbolFields::authored(&[], SymbolDetail::Tubes).layers,
            NEW_SETS * 16
        );
    }

    #[test]
    fn every_authored_drawing_fits_the_frame_and_keeps_every_run() {
        // Each run's tube, its glass included, inside the 4:3 frame.
        let sets = parse_authored(AUTHORED_SYMBOLS).unwrap();
        let reach = [NEON_RADIUS / PIXEL_ASPECT, NEON_RADIUS];
        for (k, set) in sets.iter().enumerate() {
            for shape in &set.shapes {
                for &p in shape.runs.iter().flatten() {
                    let q = authored_source(p);
                    assert!(
                        q[0] - reach[0] >= 0.0
                            && q[0] + reach[0] <= 320.0
                            && q[1] - reach[1] >= 0.0
                            && q[1] + reach[1] <= 200.0,
                        "set {} mask {} ({}) leaves the frame at {q:?}",
                        k + 1,
                        shape.mask,
                        shape.label
                    );
                }
            }
        }
        // And every point of every run lies on its layer's tube, so no
        // stroke is lost.
        let fields = SymbolFields::authored(&sets, SymbolDetail::Tubes);
        let (tw, th) = fields.texels();
        for (k, set) in sets.iter().enumerate() {
            for shape in &set.shapes {
                let layer = k * 16 + shape.mask as usize;
                for run in &shape.runs {
                    // Along each segment, every quarter row or so.
                    let mut samples = vec![run[0]];
                    for w in run.windows(2) {
                        let steps = 8;
                        samples.extend((1..=steps).map(|t| {
                            let t = t as f32 / steps as f32;
                            [
                                w[0][0] + (w[1][0] - w[0][0]) * t,
                                w[0][1] + (w[1][1] - w[0][1]) * t,
                            ]
                        }));
                    }
                    for p in samples {
                        let q = authored_source(p);
                        let i = ((q[0] - fields.origin[0] as f32) * SYMBOL_SCALE as f32) as usize;
                        let j = ((q[1] - fields.origin[1] as f32) * SYMBOL_SCALE as f32) as usize;
                        let at = ((layer * th as usize + j) * tw as usize + i) * 2;
                        let d = fields.atlas[at].to_f32();
                        assert!(
                            d < 0.6,
                            "set {} mask {} ({}) at {p:?}: {d}",
                            k + 1,
                            shape.mask,
                            shape.label
                        );
                    }
                }
            }
        }
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
    }
}
