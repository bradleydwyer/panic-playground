//! What part 5's window shows during the zoom: the zoom author's CPU side.
//! The look is field_zoom.wgsl, and the level kernel is [`KERNEL_WGSL`] here;
//! see rust/README.md, "Adding to part 5". Edit only this file and
//! field_zoom.wgsl.
//!
//! A deep zoom into the Mandelbrot set onto a miniature copy of period 3341
//! and size 1.8e-55, so that the zoom lands on the set the mountain's
//! plateau lifts out.
//!
//! What comes from where:
//! - Read from the port: the timing. The depth is the port's own
//!   magnification, `FieldBlock::zoom[2]` (`field_zoom_level()`), log2 of
//!   2^swaps / scale with the scale interpolated between the original's
//!   frames, so the doublings (63 frames each at full speed, as the port's
//!   swap frames lower the scale twice), the brake and the stop fall on the
//!   original's frames. The window and the clear are the port's too.
//! - Authored: the target (a chain of nested minibrots out of the seahorse
//!   valley, found in a local prototype), its framing, the method and the
//!   look.
//!
//! The method follows the original's own. Field resamples a level buffer at 1
//! to 2 buffer pixels per window pixel, builds the next level, twice as fine,
//! two rows a frame, and swaps at each 2x. Here level k is the exact view at
//! depth k at [`LEVEL_SCALE`] texels per window pixel, built in bands by the
//! kernel ([`KERNEL_WGSL`]) while the doubling before it plays; the window
//! resamples the level of the current doubling. A level depends only on its
//! depth and resolution, so however a moment is reached (playing, seeking,
//! scrubbing, a one-frame export) it shows the same image. The first level
//! is built the same way ahead of the first frame that shows it, in the
//! last part of part 6 ([`ZoomLevels::prebuild`]). A level that isn't
//! complete when it is needed is finished at once ([`ZoomLevels::advance`]),
//! a one-off hitch, rather than shown partly built.
//!
//! The kernel: per-pixel perturbation from one reference orbit at the
//! minibrot's nucleus, computed here at load in fixed point (no crate), with
//! deltas in f32 floatexp, bilinear approximation (BLA) to skip iterations,
//! Zhuoran's rebasing and a periodicity test for the inside.
use super::{AuthorInfo, FieldBlock, FieldPreview};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgpu::*;

// ---- Settings (authored) ------------------------------------------------------------

/// Level texels per window pixel at the level's own depth. The window shows
/// a level from 1x to 2x magnified, so a window pixel sees 2 to 1 texels a
/// side (the original resamples 1 to 2 buffer pixels per window pixel).
pub const LEVEL_SCALE: f64 = 2.0;
/// The level's view reaches this fraction of the window's height past its
/// edges, for the window's antialiased edge.
pub const LEVEL_MARGIN: f64 = 0.03;
/// Bounds on a level's height in texels (the 4K window is 1080 rows).
pub const LEVEL_ROWS: [u32; 2] = [128, 2304];
/// Levels kept on the GPU: the one shown, the next one being built and one
/// more, for stepping back.
pub const LEVEL_SLOTS: usize = 3;
/// The next level is due complete by this far into the current doubling
/// (its rows are built in proportion), and at least 1 / [`BUILD_FRAMES`] of
/// it is built each frame.
pub const BUILD_BY: f64 = 0.8;
pub const BUILD_FRAMES: u32 = 48;
/// The level the zoom first shows is built ahead, in the frames before
/// part 5's view is first drawn ([`ZoomLevels::prebuild`]): its rows in
/// proportion over [`PREBUILD_SPAN`] seconds ending [`PREBUILD_LEAD`]
/// seconds before then, after the smoke's pre-roll has finished. Authored,
/// so that the first frame doesn't build it all at once: observed with
/// `--bench` at 2560 x 1440 on an Apple M5 Max, about 18 ms of compute
/// that made that frame take 26 to 29 ms.
pub const PREBUILD_SPAN: f64 = 1.0;
pub const PREBUILD_LEAD: f64 = 0.5;
/// BLA's tolerance: the level-0 validity radius is this times |Z_m|. 2^-12
/// moves the smooth count by about 1e-3 against a direct fixed-point render
/// (the prototype's proof), with 40% fewer steps than 2^-24.
pub const BLA_EPS: f64 = 1.0 / 4096.0;
/// Loop steps (BLA skips and single iterations) a texel may take; texels at
/// the cap lie on the boundary and are drawn as such (0.5% at the stop).
pub const MAX_STEPS: u32 = 1500;
/// The inside: a texel whose orbit returns within this fraction of itself,
/// over an attracting cycle (multiplier below [`INTERIOR_MULTIPLIER`]).
pub const INTERIOR_TOL: f32 = 1e-4;
pub const INTERIOR_MULTIPLIER: f32 = 0.5;
/// Escape radius squared (|z| > 1000).
pub const BAILOUT_SQUARED: f32 = 1e6;
/// The window's light for the frame and smoke (`fc.zoom_style[0]`), linear
/// HDR radiance: the window's mean colour at depth 0 times an intensity of
/// 2, and through the zoom [`WINDOW_LIGHT_LEVELS`] times that: the window's
/// mean luminance at each whole depth relative to depth 0, raised to 0.75 so
/// the smoke swings less than the window, linear between depths. It ends
/// near the frame's own stand-in, so the smoke catches it without blowing
/// out. The colour and luminances were measured from captures of the window
/// at those depths with the look as it is in field_zoom.wgsl (the window's
/// middle, sRGB decoded, through the inverse of the shared ACES fit): they
/// are authored values, to measure again if the look changes.
pub const WINDOW_LIGHT: [f32; 3] = [0.061, 0.131, 0.425];
pub const WINDOW_LIGHT_LEVELS: [f32; 18] = [
    1.0, 1.18, 1.17, 1.22, 1.1, 0.88, 1.15, 1.24, 1.13, 1.31, 1.28, 1.2, 1.35, 1.42, 1.46, 1.55,
    1.61, 1.61,
];

// ---- The target (authored) ----------------------------------------------------------

/// The deep zoom's target, authored (found by a local search, not from the
/// demo): a minibrot of `period` at `nucleus`, of size about 1.77e-55, the
/// end of a chain of nested minibrots out of the seahorse valley (periods
/// 32, 250, 1030 and 3341). The window is centred on the copy's body
/// (`centre`, -0.493 along its own axis from the nucleus, where the set's
/// body centre is), rotated so the copy lands in the set's usual
/// orientation (a constant rotation: the whole zoom is deep, so it shows no
/// motion), and is `stop_height` high at the stop, when the copy's body
/// fills 78% of it.
pub struct ZoomTarget {
    pub nucleus: [&'static str; 2],
    pub period: usize,
    pub centre: [&'static str; 2],
    pub rotation_degrees: f64,
    pub stop_height: f64,
}

pub const TARGET: ZoomTarget = ZoomTarget {
    nucleus: [
        "-0.77349545038429951771713040030431976579756853424571375753355826404434018088414948157814405046453699635474714275",
        "0.12243760092069356693186091541375724477527345399727678787047885626786040397546694814294932334046559730482861756",
    ],
    period: 3341,
    centre: [
        "-0.773495450384299517717130400304319765797568534245713757479776913553911522502586434455766346992895370907327882581686329405",
        "0.122437600920693566931860915413757244775273453997276787802095334448560255709712718002740286902988490638872364407203352297",
    ],
    rotation_degrees: 128.18386536916373,
    stop_height: 4.048188702221738e-55,
};

// ---- The level kernel (WGSL) ---------------------------------------------------------

/// The level kernel, a compute module of its own (not part of the modern
/// module, so kept here rather than in a .wgsl file of `src/hd`, whose files
/// all go into the renderers' modules with the shared controls). Its look
/// constant, the band period, is the only one outside field_zoom.wgsl.
const KERNEL_WGSL: &str = r#"
// The level kernel: a module of its own (ZoomLevels), one texel of level k
// per invocation, rows row0 onwards (a band). Per-pixel perturbation with
// floatexp deltas, bilinear approximation (BLA) and Zhuoran rebasing, f32
// only. Authored; checked in the prototype against a direct 376-bit
// fixed-point render and mpmath at the deepest frame (rust/README.md).
//
// Reference: the target minibrot's nucleus c0 (period P), whose orbit the CPU
// computes in fixed point at load. It is exactly periodic (Z_P = 0 = Z_0), so
// P entries serve every iteration count and no pixel needs a second reference.
//   dc    = c - c0          floatexp (f32 mantissa pair, shared i32 exponent)
//   delta = z - Z_m         a plain f32 vec2 while |delta| >= 2^-45 (where
//                           |delta|^2 is still a normal f32), else floatexp
//   der   = dz/dc           scaled f32 Dm 2^Dk, renormalised lazily
// Each loop step, with z = Z_m + delta:
//   escape  |z|^2 > bail2                       -> done
//   rebase  |z| < |delta|: delta = z, m = 0      (Zhuoran)
//   m == 0  interior: Brent periodicity on delta (z == delta there); a period
//           that ran without a rebase has multiplier ~ 2 lambda delta, lambda
//           = prod 2 Z_k; the cycle must be attracting
//   then a BLA step if |delta| < eps |Z_m| (tested in registers): a
//   non-aligned table gives every m every level that fits before P, and m's
//   row of log2 radii picks the longest valid one:
//           delta = A delta + B dc, der = A der + B, m += 2^l   (floatexp)
//   else a perturbation step, in plain f32 when Z_m and delta allow:
//           der = 2 z der + 1, delta = (2 Z_m + delta) delta + dc, m += 1
//   m == P wraps to 0.
// The texel: (band phase, log2 of the distance estimate in texels, class
// (0 escaped, 1 inside, 2 at the step cap), 0).

struct Frame {
    off_m: vec2f,     // (centre - c0) mantissa
    ex_m: vec2f,      // one column step, mantissa (rotation included)
    ey_m: vec2f,      // one row step (down), mantissa
    pad0: vec2f,
    off_e: i32,       // (centre - c0) exponent
    ps_e: i32,        // shared exponent of ex_m, ey_m
    width: u32,
    height: u32,
    period: u32,
    maxiter: u32,     // iteration cap (counts skipped iterations)
    levels: u32,      // BLA levels (0 = BLA off)
    tol: f32,         // interior: |delta - saved| < tol |delta|
    bail2: f32,       // escape: |z|^2 > bail2
    ps_log2: f32,     // log2 of one texel in complex units
    row0: u32,        // first row of this band
    max_steps: u32,   // cap on loop steps (BLA + perturbation)
    wmax: f32,        // interior: cycle multiplier |w| must be below this
    eps: f32,         // BLA tolerance (level-0 radius = eps |Z_m|)
    radii_base: u32,  // this level's rows of radii
    radq: u32,        // vec4 rows of radii per m
    lam_m: vec2f,     // lambda = prod_{k=1}^{P-1} 2 Z_k (mantissa)
    lam_e: i32,       // lambda exponent
    pad1: u32,
}
// tiny == 0: v is Z_m (f32, |Z_m| >= 2^-45); else Z_m = v 2^e
struct Ref { v: vec2f, e: i32, tiny: i32 }
struct BlaE { a: vec2f, b: vec2f, ae: i32, be: i32, rm: f32, re: i32 }

@group(0) @binding(0) var<uniform> F: Frame;
@group(0) @binding(1) var<storage, read> orbit: array<Ref>;
@group(0) @binding(2) var<storage, read> bla: array<BlaE>;
@group(0) @binding(3) var<storage, read> level_off: array<u32>;
// BLA radii as log2, radq vec4 rows (levels 0..4 radq - 1) per reference
// index m, for each zoom level; -1e30 where a BLA level doesn't fit before P
@group(0) @binding(4) var<storage, read> radii: array<vec4f>;
@group(0) @binding(5) var texels: texture_storage_2d<rgba16float, write>;

// The kernel's look constant: smooth iteration bands, one every this many
// iterations (stored as a phase, 0 to 1, for zoom_sample).
const ZOOM_BAND_PERIOD: u32 = 48u;

// ---- floatexp: value = m * 2^e, max(|m.x|, |m.y|) in [0.5, 1), zero has e = ZE
const ZE: i32 = -100000000;
struct FE { m: vec2f, e: i32 }

// 2^k as f32 for k in [-126, 127]; 0 below (exact, built from the bits)
fn pow2(k: i32) -> f32 {
    return select(0.0, bitcast<f32>(u32(clamp(k, -126, 127) + 127) << 23u), k >= -126);
}
// normalise with the exponent field; subnormal or 0 -> zero
fn fe_norm(m: vec2f, e: i32) -> FE {
    let a = max(abs(m.x), abs(m.y));
    let k = i32((bitcast<u32>(a) >> 23u) & 0xffu) - 126;
    if (k <= -126) { return FE(vec2f(0.0), ZE); }
    return FE(m * pow2(-k), e + k);
}
fn cmul(a: vec2f, b: vec2f) -> vec2f { return vec2f(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x); }
fn fe_add(a: FE, b: FE) -> FE {
    let d = a.e - b.e;
    if (d >= 0) {
        if (d > 40) { return a; }
        return fe_norm(a.m + b.m * pow2(-d), a.e);
    }
    if (d < -40) { return b; }
    return fe_norm(b.m + a.m * pow2(d), b.e);
}
fn fe_mul(a: FE, b: FE) -> FE { return fe_norm(cmul(a.m, b.m), a.e + b.e); }
// a*b + c*d with a single normalisation (mantissa products stay in [1/4, 2))
fn fe_fma2(a: FE, b: FE, c: FE, d: FE) -> FE {
    let p = cmul(a.m, b.m);
    let pe = a.e + b.e;
    let q = cmul(c.m, d.m);
    let qe = c.e + d.e;
    let k = pe - qe;
    if (k >= 0) {
        if (k > 40) { return fe_norm(p, pe); }
        return fe_norm(p + q * pow2(-k), pe);
    }
    if (k < -40) { return fe_norm(q, qe); }
    return fe_norm(q + p * pow2(k), qe);
}
fn fe_is0(a: FE) -> bool { return a.m.x == 0.0 && a.m.y == 0.0; }
// |a| < |b|
fn fe_lt(a: FE, b: FE) -> bool {
    if (fe_is0(b)) { return false; }
    if (fe_is0(a)) { return true; }
    let d = a.e - b.e;
    if (d < -1) { return true; }
    if (d > 1) { return false; }
    return dot(a.m, a.m) * pow2(2 * d) < dot(b.m, b.m);
}
// |a| < r for a real floatexp r = rm 2^re (rm in [0.5, 1) or 0)
fn fe_lt_r(a: FE, rm: f32, re: i32) -> bool {
    if (rm == 0.0) { return false; }
    if (fe_is0(a)) { return true; }
    let d = a.e - re;
    if (d < -1) { return true; }
    if (d > 1) { return false; }
    return dot(a.m, a.m) * pow2(2 * d) < rm * rm;
}
fn fe_f32(a: FE) -> vec2f { return a.m * pow2(a.e); }
fn fe_log2(a: FE) -> f32 { return 0.5 * log2(dot(a.m, a.m)) + f32(a.e); }

// delta = dm 2^de. de == 0: the plain f32 regime, dm is the value; else
// floatexp (dm normalised, de <= -45).
fn delta_fe(dm: vec2f, de: i32) -> FE {
    if (de == 0) { return fe_norm(dm, 0); }
    return FE(dm, de);
}
// Z_m as floatexp
fn ref_fe(r: Ref) -> FE {
    if (r.tiny == 0) { return fe_norm(r.v, 0); }
    return FE(r.v, r.e);
}

fn write_texel(x: u32, y: u32, n: u32, cls: u32, zf: vec2f, D: FE) {
    var t = vec4f(0.0, 0.0, f32(cls), 0.0);
    if (cls == 0u) {
        let az = length(zf);
        let lz = log(az);
        // smooth iteration count n - log2(ln|z| / ln 1000), as a band phase
        let f = -log2(lz / log(1000.0));
        t.x = fract((f32(n % ZOOM_BAND_PERIOD) + f) / f32(ZOOM_BAND_PERIOD));
        // distance estimate |z| ln|z| / |dz/dc|, in texels
        t.y = clamp(log2(az * lz) - fe_log2(D) - F.ps_log2, -120.0, 120.0);
    }
    textureStore(texels, vec2u(x, y), t);
}

@compute @workgroup_size(8, 8)
fn zoom_level_kernel(@builtin(global_invocation_id) gid: vec3u) {
    let x = gid.x;
    let y = gid.y + F.row0;
    if (x >= F.width || y >= F.height) { return; }
    let u = f32(x) + 0.5 - 0.5 * f32(F.width);
    let v = f32(y) + 0.5 - 0.5 * f32(F.height);
    let dc = fe_add(FE(F.off_m, F.off_e), fe_norm(u * F.ex_m + v * F.ey_m, F.ps_e));
    let one = FE(vec2f(0.5, 0.0), 1);
    let eps2 = F.eps * F.eps;

    var dm = vec2f(0.0);
    var de = 0;
    var Dm = vec2f(0.0); var Dk = 0;
    var m = 0u;
    var n = 0u;
    var steps = 0u;
    // interior (Brent at m == 0): saved delta, log2 |multiplier| over clean
    // periods since the save, and packed counters: bit 31 have, bit 30 clean,
    // bits 25..29 log2 span, bits 0..24 events since the save
    var saved = FE(vec2f(0.0), ZE);
    var lw = 0.0;
    var bits = 0u;
    let lwmax = log2(F.wmax);
    let llam = fe_log2(FE(F.lam_m, F.lam_e + 1));   // log2 |2 lambda|

    loop {
        let r = orbit[m];
        var zt = r.tiny != 0;
        // ---- escape and rebase
        if (de == 0 && !zt) {
            let zf = r.v + dm;
            let zz = dot(zf, zf);
            if (zz > F.bail2) { write_texel(x, y, n, 0u, zf, fe_norm(Dm, Dk)); return; }
            if (zz < dot(dm, dm)) { dm = zf; m = 0u; zt = true; bits &= ~(1u << 30u); }
        } else {
            let dd = delta_fe(dm, de);
            let zfe = fe_add(ref_fe(r), dd);
            if (zfe.e >= 9) {
                let zf = fe_f32(zfe);
                if (dot(zf, zf) > F.bail2) { write_texel(x, y, n, 0u, zf, fe_norm(Dm, Dk)); return; }
            }
            if (fe_lt(zfe, dd)) {
                if (zfe.e > -45) { dm = fe_f32(zfe); de = 0; } else { dm = zfe.m; de = zfe.e; }
                m = 0u; zt = true; bits &= ~(1u << 30u);
            }
        }
        // ---- interior
        if (m == 0u) {
            let dd = delta_fe(dm, de);
            if ((bits & (1u << 30u)) != 0u) { lw += llam + fe_log2(dd); } else { lw = 1e9; }
            if ((bits & (1u << 31u)) != 0u && lw < lwmax) {
                let dif = fe_add(dd, FE(-saved.m, saved.e));
                if (fe_lt(dif, FE(dd.m * F.tol, dd.e))) { write_texel(x, y, n, 1u, vec2f(0.0), one); return; }
            }
            let ev = (bits & 0x1ffffffu) + 1u;
            let lspan = (bits >> 25u) & 31u;
            if (ev >= (1u << lspan)) {
                saved = dd; lw = 0.0;
                bits = (1u << 31u) | (min(lspan + 1u, 20u) << 25u);
            } else {
                bits = (bits & ~0x1ffffffu) | ev;
            }
            bits |= 1u << 30u;
        }
        if (n >= F.maxiter || steps >= F.max_steps) { break; }
        steps += 1u;

        // ---- BLA level
        var level = 0u;
        if (!zt && F.levels > 0u) {
            var ok = false;
            var lg = 0.0;
            if (de == 0) {
                let d2 = dot(dm, dm);
                ok = d2 < eps2 * dot(r.v, r.v);
                lg = 0.5 * log2(d2);
            } else {
                let rr = fe_norm(vec2f(length(r.v) * F.eps, 0.0), 0);
                ok = fe_lt_r(FE(dm, de), rr.m.x, rr.e);
                lg = fe_log2(FE(dm, de));
            }
            if (ok) {
                var cnt = 0.0;
                for (var q = 0u; q < F.radq; q += 1u) {
                    let rr = radii[F.radii_base + m * F.radq + q];
                    cnt += dot(select(vec4f(0.0), vec4f(1.0), vec4f(lg) < rr), vec4f(1.0));
                }
                level = u32(max(cnt, 1.0)) - 1u;
            }
        }
        if (level > 0u) {
            let e = bla[level_off[level] + m - 1u];
            let A = FE(e.a, e.ae);
            let B = FE(e.b, e.be);
            let nd = fe_fma2(A, delta_fe(dm, de), B, dc);
            if (nd.e > -45) { dm = fe_f32(nd); de = 0; } else { dm = nd.m; de = nd.e; }
            let D = fe_fma2(A, fe_norm(Dm, Dk), B, one);
            Dm = D.m; Dk = D.e;
            m += 1u << level;
            n += 1u << level;
        } else if (de == 0 && !zt) {
            let z2 = 2.0 * (r.v + dm);
            Dm = cmul(z2, Dm) + vec2f(pow2(-Dk), 0.0);
            if (max(abs(Dm.x), abs(Dm.y)) > 1e18) { let D = fe_norm(Dm, Dk); Dm = D.m; Dk = D.e; }
            dm = cmul(2.0 * r.v + dm, dm) + fe_f32(dc);
            if (max(abs(dm.x), abs(dm.y)) < 3e-14) { let t = fe_norm(dm, 0); dm = t.m; de = t.e; }
            m += 1u;
            n += 1u;
        } else {
            let dd = delta_fe(dm, de);
            var Z2 = FE(vec2f(0.0), ZE);
            var zz = dd;
            if (m != 0u) {
                Z2 = FE(r.v, r.e + 1);
                if (r.tiny == 0) { Z2 = fe_norm(2.0 * r.v, 0); }
                zz = fe_add(ref_fe(r), dd);
            }
            let D = fe_fma2(FE(zz.m, zz.e + 1), fe_norm(Dm, Dk), one, one);
            Dm = D.m; Dk = D.e;
            let nd = fe_fma2(fe_add(Z2, dd), dd, dc, one);
            if (nd.e > -45) { dm = fe_f32(nd); de = 0; } else { dm = nd.m; de = nd.e; }
            m += 1u;
            n += 1u;
        }
        if (m >= F.period) { m = 0u; }
    }
    write_texel(x, y, n, 2u, vec2f(0.0), one);
}
"#;

pub static ZOOM: AuthorInfo = AuthorInfo {
    name: "zoom",
    source: include_str!("field_zoom.wgsl"),
    prepass: false,
    controls: zoom_controls,
};

/// The zoom's `u.field.zoom_style` values at a clip time, given the rest of
/// the block: [0] the window's light for the frame and smoke, rgb radiance,
/// w 1 while part 5 is shown (so a dark window counts as given).
fn zoom_controls(block: &FieldBlock, _time: f64) -> [[f32; 4]; 4] {
    let mut style = [[0.0; 4]; 4];
    if block.part[0] < 0.5 {
        return style;
    }
    let level = block.zoom[2].max(0.0);
    let k = (level.floor() as usize).min(WINDOW_LIGHT_LEVELS.len() - 1);
    let next = (k + 1).min(WINDOW_LIGHT_LEVELS.len() - 1);
    let f = (level - k as f32).clamp(0.0, 1.0);
    let brightness =
        WINDOW_LIGHT_LEVELS[k] + (WINDOW_LIGHT_LEVELS[next] - WINDOW_LIGHT_LEVELS[k]) * f;
    // Part 6's fade brings it in, as it does the window; it goes out with
    // the window's clear, as the frame's stand-in does.
    let fade = block.window[2].clamp(0.0, 1.0).powf(2.2);
    let on = if block.window[0] > 0.5 {
        let t = (block.window[1] / 0.12).clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    } else {
        1.0
    };
    let k = brightness * fade * on;
    style[0] = [
        WINDOW_LIGHT[0] * k,
        WINDOW_LIGHT[1] * k,
        WINDOW_LIGHT[2] * k,
        1.0,
    ];
    style
}

// ---- Fixed point --------------------------------------------------------------------

/// Signed fixed point: [`fixed::N`] 64-bit limbs, two's complement, little
/// endian, 8 integer bits with the sign; 376 fractional bits (about 113
/// digits), for the target's orbit at 1e-55. A port of the prototype's.
pub mod fixed {
    pub const N: usize = 6;
    pub const FRAC: i32 = 64 * N as i32 - 8;

    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub struct Fx(pub [u64; N]);

    impl Fx {
        pub const ZERO: Fx = Fx([0; N]);
        fn negative(&self) -> bool {
            self.0[N - 1] >> 63 == 1
        }
        pub fn neg(&self) -> Fx {
            let mut r = [0u64; N];
            let mut carry = 1u64;
            for (i, out) in r.iter_mut().enumerate() {
                let (v, c) = (!self.0[i]).overflowing_add(carry);
                *out = v;
                carry = c as u64;
            }
            Fx(r)
        }
        pub fn add(&self, o: &Fx) -> Fx {
            let mut r = [0u64; N];
            let mut carry = 0u64;
            for (i, out) in r.iter_mut().enumerate() {
                let (a, c1) = self.0[i].overflowing_add(o.0[i]);
                let (b, c2) = a.overflowing_add(carry);
                *out = b;
                carry = (c1 | c2) as u64;
            }
            Fx(r)
        }
        pub fn sub(&self, o: &Fx) -> Fx {
            self.add(&o.neg())
        }
        fn magnitude(&self) -> ([u64; N], bool) {
            if self.negative() {
                (self.neg().0, true)
            } else {
                (self.0, false)
            }
        }
        pub fn mul(&self, o: &Fx) -> Fx {
            let (a, sa) = self.magnitude();
            let (b, sb) = o.magnitude();
            let mut p = [0u64; 2 * N];
            for i in 0..N {
                if a[i] == 0 {
                    continue;
                }
                let mut carry: u128 = 0;
                for j in 0..N {
                    let t = a[i] as u128 * b[j] as u128 + p[i + j] as u128 + carry;
                    p[i + j] = t as u64;
                    carry = t >> 64;
                }
                p[i + N] = carry as u64;
            }
            // Shift right by FRAC = 64 N - 8.
            let mut r = [0u64; N];
            for (i, out) in r.iter_mut().enumerate() {
                *out = (p[N - 1 + i] >> 56) | (p[N + i] << 8);
            }
            let r = Fx(r);
            if sa != sb { r.neg() } else { r }
        }
        fn scaled(&self, k: u64, divide: bool) -> Fx {
            let (a, s) = self.magnitude();
            let mut r = [0u64; N];
            if divide {
                let mut rem: u128 = 0;
                for i in (0..N).rev() {
                    let cur = (rem << 64) | a[i] as u128;
                    r[i] = (cur / k as u128) as u64;
                    rem = cur % k as u128;
                }
            } else {
                let mut carry: u128 = 0;
                for i in 0..N {
                    let t = a[i] as u128 * k as u128 + carry;
                    r[i] = t as u64;
                    carry = t >> 64;
                }
            }
            let r = Fx(r);
            if s { r.neg() } else { r }
        }
        /// m 2^e, exactly (bits below the last fractional bit dropped).
        fn of_parts(m: u64, e: i32, negative: bool) -> Fx {
            let mut r = [0u64; N];
            for bit in 0..64 {
                if m >> bit & 1 == 1 {
                    let at = e + FRAC + bit;
                    if (0..64 * N as i32 - 1).contains(&at) {
                        r[(at / 64) as usize] |= 1u64 << (at % 64);
                    }
                }
            }
            let r = Fx(r);
            if negative { r.neg() } else { r }
        }
        #[cfg(test)]
        pub fn from_f64(x: f64) -> Fx {
            if x == 0.0 || !x.is_finite() {
                return Fx::ZERO;
            }
            let bits = x.to_bits();
            let exp = ((bits >> 52) & 0x7ff) as i32;
            let frac = bits & ((1u64 << 52) - 1);
            let (m, e) = if exp == 0 {
                (frac, -1074)
            } else {
                (frac | 1u64 << 52, exp - 1075)
            };
            Fx::of_parts(m, e, bits >> 63 == 1)
        }
        /// The value as f64 (to its 53 bits).
        pub fn to_f64(self) -> f64 {
            let (a, s) = self.magnitude();
            let Some(i) = (0..N).rev().find(|&i| a[i] != 0) else {
                return 0.0;
            };
            let top = i as i32 * 64 + 63 - a[i].leading_zeros() as i32;
            let mut w: u64 = 0;
            for k in 0..64 {
                let at = top - k;
                if at < 0 {
                    break;
                }
                w |= (a[(at / 64) as usize] >> (at % 64) & 1) << (63 - k);
            }
            let m = w as f64 / 18446744073709551616.0;
            let v = super::ldexp(m, top + 1 - FRAC);
            if s { -v } else { v }
        }
        fn one() -> Fx {
            Fx::of_parts(1, 0, false)
        }
        /// A plain decimal, optionally signed ("-0.773...").
        pub fn parse(s: &str) -> Fx {
            let s = s.trim();
            let (negative, s) = match s.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, s),
            };
            let (whole, fraction) = s.split_once('.').unwrap_or((s, ""));
            let mut x = Fx::ZERO;
            for d in fraction.bytes().rev() {
                x = x
                    .add(&Fx::one().scaled((d - b'0') as u64, false))
                    .scaled(10, true);
            }
            let whole: u64 = if whole.is_empty() {
                0
            } else {
                whole.parse().unwrap()
            };
            x = x.add(&Fx::one().scaled(whole, false));
            if negative { x.neg() } else { x }
        }
    }

    /// A complex fixed-point number.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Cx {
        pub re: Fx,
        pub im: Fx,
    }

    impl Cx {
        pub fn parse([re, im]: [&str; 2]) -> Cx {
            Cx {
                re: Fx::parse(re),
                im: Fx::parse(im),
            }
        }
        pub fn sub(&self, o: &Cx) -> Cx {
            Cx {
                re: self.re.sub(&o.re),
                im: self.im.sub(&o.im),
            }
        }
        /// z^2 + c
        pub fn square_add(&self, c: &Cx) -> Cx {
            let rr = self.re.mul(&self.re);
            let ii = self.im.mul(&self.im);
            let ri = self.re.mul(&self.im);
            Cx {
                re: rr.sub(&ii).add(&c.re),
                im: ri.add(&ri).add(&c.im),
            }
        }
        pub fn to_f64(self) -> [f64; 2] {
            [self.re.to_f64(), self.im.to_f64()]
        }
    }
}

/// m 2^e without overflowing 2^e.
fn ldexp(m: f64, e: i32) -> f64 {
    let (mut m, mut e) = (m, e);
    while e > 1000 {
        m *= 2f64.powi(1000);
        e -= 1000;
    }
    while e < -1000 {
        m *= 2f64.powi(-1000);
        e += 1000;
    }
    m * 2f64.powi(e)
}

// ---- The reference orbit and BLA ----------------------------------------------------

type C64 = [f64; 2];
fn cmul(a: C64, b: C64) -> C64 {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}
fn cadd(a: C64, b: C64) -> C64 {
    [a[0] + b[0], a[1] + b[1]]
}
fn cabs(a: C64) -> f64 {
    a[0].hypot(a[1])
}

/// The reference orbit Z_0 .. Z_p (Z_0 = 0) at a nucleus, computed in fixed
/// point and kept as f64 (the GPU needs it only to f32; a nucleus's orbit
/// is exactly periodic, Z_p = 0).
pub fn reference_orbit(nucleus: &fixed::Cx, period: usize) -> Vec<C64> {
    let mut z = fixed::Cx::default();
    let mut orbit = Vec::with_capacity(period + 1);
    orbit.push([0.0, 0.0]);
    for _ in 0..period {
        z = z.square_add(nucleus);
        orbit.push(z.to_f64());
    }
    orbit
}

/// The non-aligned BLA table: level l at m (1 <= m, m + 2^l <= P) skips the
/// 2^l iterations from m, delta' = A delta + B dc, valid while |delta| < r.
/// A and B depend only on the orbit; r on the largest |dc| as well.
struct Bla {
    levels: Vec<Vec<(C64, C64, f64)>>,
}

impl Bla {
    fn new(orbit: &[C64], period: usize) -> Bla {
        let mut levels: Vec<Vec<(C64, C64, f64)>> = vec![
            (1..period)
                .map(|m| ([2.0 * orbit[m][0], 2.0 * orbit[m][1]], [1.0, 0.0], 0.0))
                .collect(),
        ];
        let mut l = 1;
        while (1usize << l) < period {
            let half = 1usize << (l - 1);
            let prev = &levels[l - 1];
            let next = (0..period - (1 << l))
                .map(|i| {
                    let (ax, bx, _) = prev[i];
                    let (ay, by, _) = prev[i + half];
                    (cmul(ay, ax), cadd(cmul(ay, bx), by), 0.0)
                })
                .collect();
            levels.push(next);
            l += 1;
        }
        Bla { levels }
    }
    /// Fill in the radii for a largest |dc|: r_0 = eps |Z_m|, and a merge's
    /// r = min(r_x, (r_y - |B_x| |dc|) / |A_x|).
    fn radii(&mut self, orbit: &[C64], dc_max: f64) {
        for (i, e) in self.levels[0].iter_mut().enumerate() {
            e.2 = BLA_EPS * cabs(orbit[i + 1]);
        }
        for l in 1..self.levels.len() {
            let half = 1usize << (l - 1);
            let (lo, hi) = self.levels.split_at_mut(l);
            let prev = &lo[l - 1];
            for (i, e) in hi[0].iter_mut().enumerate() {
                let (ax, bx, rx) = prev[i];
                let ry = prev[i + half].2;
                e.2 = rx.min(((ry - cabs(bx) * dc_max) / cabs(ax)).max(0.0));
            }
        }
    }
}

/// A complex value as an f32 mantissa pair (larger component in [0.5, 1))
/// and an exponent; zero has [`ZERO_EXPONENT`] (as the kernel's `ZE`).
const ZERO_EXPONENT: i32 = -100_000_000;
fn floatexp(z: C64) -> ([f32; 2], i32) {
    let a = z[0].abs().max(z[1].abs());
    if a == 0.0 || !a.is_finite() {
        return ([0.0, 0.0], ZERO_EXPONENT);
    }
    let mut e = a.log2().floor() as i32 + 1;
    let s = ldexp(1.0, -e);
    let (mut re, mut im) = (z[0] * s, z[1] * s);
    let top = re.abs().max(im.abs());
    if top >= 1.0 {
        (re, im, e) = (re * 0.5, im * 0.5, e + 1);
    } else if top < 0.5 {
        (re, im, e) = (re * 2.0, im * 2.0, e - 1);
    }
    ([re as f32, im as f32], e)
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Ref {
    v: [f32; 2],
    e: i32,
    tiny: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlaEntry {
    a: [f32; 2],
    b: [f32; 2],
    ae: i32,
    be: i32,
    rm: f32,
    re: i32,
}

/// The kernel's `Frame` ([`KERNEL_WGSL`]), one per level slot.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct KernelFrame {
    off_m: [f32; 2],
    ex_m: [f32; 2],
    ey_m: [f32; 2],
    pad0: [f32; 2],
    off_e: i32,
    ps_e: i32,
    width: u32,
    height: u32,
    period: u32,
    maxiter: u32,
    levels: u32,
    tol: f32,
    bail2: f32,
    ps_log2: f32,
    row0: u32,
    max_steps: u32,
    wmax: f32,
    eps: f32,
    radii_base: u32,
    radq: u32,
    lam_m: [f32; 2],
    lam_e: i32,
    pad1: u32,
}

/// field_zoom.wgsl's `ZoomView`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
struct ZoomView {
    level: [f32; 4],
    state: [f32; 4],
}

/// The deep zoom's fixed path at load: the reference orbit and its tables,
/// and the view of each level.
pub struct ZoomPath {
    period: usize,
    orbit: Vec<C64>,
    /// lambda = prod_{k=1}^{P-1} 2 Z_k, the period's multiplier factor.
    lambda: C64,
    bla: Bla,
    /// centre - nucleus, and the rotation (radians).
    offset: C64,
    rotation: f64,
    /// The port's magnification at the stop (log2), and the window's height
    /// at depth 0 (stop height x 2^stop).
    stop: f64,
    start_height: f64,
}

/// The port's magnification (log2) at the zoom's last frame: what
/// `FieldPreview::controls` gives as `zoom[2]` once the zoom stops (16.31
/// with the music).
pub fn stop_level(preview: &FieldPreview) -> f64 {
    let last = &preview.frames[preview.last_zoom];
    last.swaps as f64 - (last.state.scale as f64 / 65536.0).log2()
}

impl ZoomPath {
    pub fn new(stop: f64) -> Self {
        let nucleus = fixed::Cx::parse(TARGET.nucleus);
        let period = TARGET.period;
        let orbit = reference_orbit(&nucleus, period);
        let lambda = (1..period).fold([1.0, 0.0], |l, k| {
            cmul([2.0 * orbit[k][0], 2.0 * orbit[k][1]], l)
        });
        let offset = fixed::Cx::parse(TARGET.centre).sub(&nucleus).to_f64();
        ZoomPath {
            period,
            bla: Bla::new(&orbit, period),
            orbit,
            lambda,
            offset,
            rotation: TARGET.rotation_degrees.to_radians(),
            stop,
            start_height: TARGET.stop_height * 2f64.powf(stop),
        }
    }
    /// The last level, the doubling the stop is in.
    pub fn last_level(&self) -> usize {
        self.stop.floor().max(0.0) as usize
    }
    /// The window's height (complex units) at depth `level`.
    pub fn height(&self, level: f64) -> f64 {
        self.start_height * 2f64.powf(-level)
    }
    /// The largest |c - c0| a level's view reaches.
    fn dc_max(&self, k: usize) -> f64 {
        let h = self.height(k as f64) * (1.0 + LEVEL_MARGIN);
        cabs(self.offset) + 0.5 * h * (1.0f64 + (16.0f64 / 15.0).powi(2)).sqrt()
    }
    /// The kernel's frame for level k at `rows` texels high and `width` wide.
    fn frame(&self, k: usize, width: u32, rows: u32, radq: u32) -> KernelFrame {
        let dx = self.height(k as f64) * (1.0 + LEVEL_MARGIN) / rows as f64;
        let (s, c) = self.rotation.sin_cos();
        let ex = [c * dx, s * dx];
        let ey = [s * dx, -c * dx];
        let (_, pe) = floatexp([dx, 0.0]);
        let scale = ldexp(1.0, -pe);
        let (om, oe) = floatexp(self.offset);
        let (lm, le) = floatexp(self.lambda);
        KernelFrame {
            off_m: om,
            off_e: oe,
            ex_m: [(ex[0] * scale) as f32, (ex[1] * scale) as f32],
            ey_m: [(ey[0] * scale) as f32, (ey[1] * scale) as f32],
            ps_e: pe,
            width,
            height: rows,
            period: self.period as u32,
            maxiter: 2_000_000_000,
            levels: self.bla.levels.len() as u32,
            tol: INTERIOR_TOL,
            bail2: BAILOUT_SQUARED,
            ps_log2: dx.log2() as f32,
            max_steps: MAX_STEPS,
            wmax: INTERIOR_MULTIPLIER,
            eps: BLA_EPS as f32,
            radii_base: (k * self.period) as u32 * radq,
            radq,
            lam_m: lm,
            lam_e: le,
            ..Default::default()
        }
    }
    fn refs(&self) -> Vec<Ref> {
        self.orbit[..self.period]
            .iter()
            .enumerate()
            .map(|(k, &z)| {
                let (m, e) = floatexp(z);
                // An ordinary f32 when |Z_m| >= 2^-45, where its square stays
                // in f32's normal range; else floatexp (and Z_0 = 0).
                if k != 0 && e > -45 && e < 100 {
                    Ref {
                        v: [z[0] as f32, z[1] as f32],
                        e,
                        tiny: 0,
                    }
                } else {
                    Ref { v: m, e, tiny: 1 }
                }
            })
            .collect()
    }
    fn entries(&self) -> (Vec<BlaEntry>, Vec<u32>) {
        let mut entries = Vec::new();
        let mut offsets = Vec::new();
        for level in &self.bla.levels {
            offsets.push(entries.len() as u32);
            for &(a, b, _) in level {
                let (am, ae) = floatexp(a);
                let (bm, be) = floatexp(b);
                entries.push(BlaEntry {
                    a: am,
                    b: bm,
                    ae,
                    be,
                    rm: 0.0,
                    re: 0,
                });
            }
        }
        (entries, offsets)
    }
    /// log2 radii for every zoom level: rows of `radq` vec4 per reference
    /// index, -1e30 where a BLA level doesn't fit.
    fn radii(&mut self) -> (Vec<[f32; 4]>, u32) {
        let levels = self.bla.levels.len();
        let radq = levels.div_ceil(4).max(1) as u32;
        let rows = self.period * radq as usize;
        let mut out = vec![[-1e30f32; 4]; rows * (self.last_level() + 1)];
        for k in 0..=self.last_level() {
            let dc = self.dc_max(k);
            self.bla.radii(&self.orbit, dc);
            for (l, level) in self.bla.levels.iter().enumerate() {
                for (i, &(_, _, r)) in level.iter().enumerate() {
                    if r > 0.0 {
                        out[k * rows + (i + 1) * radq as usize + l / 4][l % 4] = r.log2() as f32;
                    }
                }
            }
        }
        (out, radq)
    }
}

// ---- The GPU hook -------------------------------------------------------------------

/// One level's texture and its build state.
struct Slot {
    _texture: Texture,
    frame: Buffer,
    build: BindGroup,
    drawn: BindGroup,
    level: Option<usize>,
    rows_done: u32,
    used: u64,
}

/// The zoom's GPU side, which field_render.rs creates beside the fluid and
/// brings to each moment shown: the reference orbit, the BLA tables and
/// radii, the level kernel, and [`LEVEL_SLOTS`] level textures, built in
/// bands. Group 2 of the modern module's fragments is the shown level and
/// its view ([`ZoomLevels::layout`]).
pub struct ZoomLevels {
    path: ZoomPath,
    radq: u32,
    pipeline: ComputePipeline,
    build_layout: BindGroupLayout,
    drawn_layout: BindGroupLayout,
    buffers: [Buffer; 4],
    view: Buffer,
    blank: BindGroup,
    /// The level size, width x rows, for the drawable it was made for.
    size: (u32, u32),
    slots: Vec<Slot>,
    shown: Option<usize>,
    clock: u64,
}

const LEVEL_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

impl ZoomLevels {
    /// Group 2 of the modern module's fragments: the shown level (a texture)
    /// and its view (a uniform).
    pub fn layout(device: &Device) -> BindGroupLayout {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 5 zoom, drawn"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    /// The path for a preview (its stop decides the depth at the start),
    /// computed at load: the orbit in fixed point and the tables, about 2 ms.
    pub fn new(device: &Device, preview: &FieldPreview, drawn_layout: BindGroupLayout) -> Self {
        Self::with_path(device, ZoomPath::new(stop_level(preview)), drawn_layout)
    }

    fn with_path(device: &Device, mut path: ZoomPath, drawn_layout: BindGroupLayout) -> Self {
        let refs = path.refs();
        let (entries, offsets) = path.entries();
        let (radii, radq) = path.radii();
        let storage = |label, contents: &[u8]| {
            device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: BufferUsages::STORAGE,
            })
        };
        let buffers = [
            storage("Part 5 zoom orbit", bytemuck::cast_slice(&refs)),
            storage("Part 5 zoom BLA", bytemuck::cast_slice(&entries)),
            storage("Part 5 zoom BLA levels", bytemuck::cast_slice(&offsets)),
            storage("Part 5 zoom BLA radii", bytemuck::cast_slice(&radii)),
        ];
        let buffer_entry = |binding, ty| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let read = BufferBindingType::Storage { read_only: true };
        let build_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 5 zoom level build"),
            entries: &[
                buffer_entry(0, BufferBindingType::Uniform),
                buffer_entry(1, read),
                buffer_entry(2, read),
                buffer_entry(3, read),
                buffer_entry(4, read),
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: LEVEL_FORMAT,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 5 zoom level kernel"),
            source: ShaderSource::Wgsl(KERNEL_WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Part 5 zoom level"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Part 5 zoom level"),
                bind_group_layouts: &[Some(&build_layout)],
                immediate_size: 0,
            })),
            module: &module,
            entry_point: Some("zoom_level_kernel"),
            compilation_options: Default::default(),
            cache: None,
        });
        let view = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Part 5 zoom view"),
            contents: bytemuck::bytes_of(&ZoomView::default()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let blank_view = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Part 5 zoom, none"),
            contents: bytemuck::bytes_of(&ZoomView::default()),
            usage: BufferUsages::UNIFORM,
        });
        let blank_texture = level_texture(device, (1, 1));
        let blank = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 5 zoom, none"),
            layout: &drawn_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        &blank_texture.create_view(&Default::default()),
                    ),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: blank_view.as_entire_binding(),
                },
            ],
        });
        ZoomLevels {
            path,
            radq,
            pipeline,
            build_layout,
            drawn_layout,
            buffers,
            view,
            blank,
            size: (0, 0),
            slots: Vec::new(),
            shown: None,
            clock: 0,
        }
    }

    /// The fragments' group 2: the shown level, or none.
    pub fn drawn(&self) -> &BindGroup {
        match self.shown {
            Some(i) => &self.slots[i].drawn,
            None => &self.blank,
        }
    }

    /// A level's size for a drawable: [`LEVEL_SCALE`] texels per window
    /// pixel, the window being 100 rows of the 4:3 frame's 200.
    pub fn level_size(drawable: (u32, u32)) -> (u32, u32) {
        let frame = (drawable.1 as f64).min(drawable.0 as f64 * 0.75);
        let rows = (frame / 2.0 * LEVEL_SCALE * (1.0 + LEVEL_MARGIN)).ceil() as u32;
        let rows = rows.clamp(LEVEL_ROWS[0], LEVEL_ROWS[1]);
        let width = (rows as f64 * 16.0 / 15.0).ceil() as u32;
        (width, rows)
    }

    fn make_slots(&mut self, device: &Device, size: (u32, u32)) {
        self.slots = (0..LEVEL_SLOTS)
            .map(|_| {
                let texture = level_texture(device, size);
                let view = texture.create_view(&Default::default());
                let frame = device.create_buffer(&BufferDescriptor {
                    label: Some("Part 5 zoom level frame"),
                    size: std::mem::size_of::<KernelFrame>() as u64,
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut entries = vec![BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                }];
                for (i, b) in self.buffers.iter().enumerate() {
                    entries.push(BindGroupEntry {
                        binding: i as u32 + 1,
                        resource: b.as_entire_binding(),
                    });
                }
                entries.push(BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::TextureView(&view),
                });
                let build = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Part 5 zoom level build"),
                    layout: &self.build_layout,
                    entries: &entries,
                });
                let drawn = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Part 5 zoom level, drawn"),
                    layout: &self.drawn_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(&view),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: self.view.as_entire_binding(),
                        },
                    ],
                });
                Slot {
                    _texture: texture,
                    frame,
                    build,
                    drawn,
                    level: None,
                    rows_done: 0,
                    used: 0,
                }
            })
            .collect();
        self.size = size;
        self.shown = None;
    }

    /// The slot holding level k, if any.
    fn slot_of(&self, k: usize) -> Option<usize> {
        self.slots.iter().position(|s| s.level == Some(k))
    }

    /// A slot for level k: the one holding it, or the least recently used
    /// one other than `keep`, reset for k.
    fn claim(&mut self, k: usize, keep: Option<usize>) -> usize {
        if let Some(i) = self.slot_of(k) {
            return i;
        }
        let i = (0..self.slots.len())
            .filter(|&i| Some(i) != keep)
            .min_by_key(|&i| (self.slots[i].level.is_some(), self.slots[i].used))
            .unwrap();
        let slot = &mut self.slots[i];
        slot.level = Some(k);
        slot.rows_done = 0;
        i
    }

    /// Build slot i's rows up to `rows` (a band from where it stopped).
    fn build(&mut self, queue: &Queue, encoder: &mut CommandEncoder, i: usize, rows: u32) {
        let (width, height) = self.size;
        let rows = rows.min(height);
        let slot = &self.slots[i];
        if rows <= slot.rows_done {
            return;
        }
        let k = slot.level.unwrap();
        let mut frame = self.path.frame(k, width, height, self.radq);
        frame.row0 = slot.rows_done;
        queue.write_buffer(&slot.frame, 0, bytemuck::bytes_of(&frame));
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Part 5 zoom level band"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &slot.build, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), (rows - slot.rows_done).div_ceil(8), 1);
        drop(pass);
        self.slots[i].rows_done = rows;
    }

    /// Bring the levels to the moment `block` shows on a drawable of
    /// `drawable` pixels: the level of the current doubling complete (built
    /// at once if it isn't, as after a seek: a one-off hitch), and a band of
    /// the next one, in proportion to the way through the doubling, so it is
    /// complete when it is needed.
    pub fn advance(
        &mut self,
        device: &Device,
        queue: &Queue,
        block: &FieldBlock,
        drawable: (u32, u32),
    ) {
        // Nothing to show before part 5, nor once the window is cleared
        // (field_zoom.wgsl draws it dark from then on).
        if block.part[0] < 0.5 || block.window[0] > 0.5 {
            self.shown = None;
            return;
        }
        let size = Self::level_size(drawable);
        if size != self.size {
            self.make_slots(device, size);
        }
        let last = self.path.last_level();
        let (level, k) = self.level_of(block);
        self.clock += 1;
        let mut encoder = device.create_command_encoder(&Default::default());
        let shown = self.claim(k, None);
        self.build(queue, &mut encoder, shown, self.size.1);
        self.slots[shown].used = self.clock;
        if k < last {
            let next = self.claim(k + 1, Some(shown));
            let rows = self.size.1;
            let due = ((level - k as f64) / BUILD_BY).clamp(0.0, 1.0);
            let wanted = ((rows as f64 * due).ceil() as u32)
                .max(self.slots[next].rows_done + rows.div_ceil(BUILD_FRAMES));
            self.build(queue, &mut encoder, next, wanted);
            self.slots[next].used = self.clock;
        }
        queue.submit([encoder.finish()]);
        let (width, rows) = self.size;
        let view = ZoomView {
            level: [
                k as f32,
                (rows as f64 / (1.0 + LEVEL_MARGIN)) as f32,
                width as f32,
                rows as f32,
            ],
            state: [1.0, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.view, 0, bytemuck::bytes_of(&view));
        self.shown = Some(shown);
    }

    /// The depth `block` shows, within the path, and its level.
    fn level_of(&self, block: &FieldBlock) -> (f64, usize) {
        let level = (block.zoom[2] as f64).clamp(0.0, self.path.stop);
        (level, (level.floor() as usize).min(self.path.last_level()))
    }

    /// Ahead of the first frame that shows the zoom, at clip time `first`,
    /// on a drawable of `drawable` pixels: build the level it shows there,
    /// the rows due by `time`, in proportion over [`PREBUILD_SPAN`] seconds
    /// ending [`PREBUILD_LEAD`] seconds before `first`, and nothing outside
    /// them. A level depends only on its depth and size, so it is the same
    /// as one built at once; [`ZoomLevels::advance`] finishes whatever is
    /// left at `first`, as after a seek into the span.
    pub fn prebuild(
        &mut self,
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        drawable: (u32, u32),
        time: f64,
        first: f64,
    ) {
        let begin = first - PREBUILD_LEAD - PREBUILD_SPAN;
        if time <= begin || time >= first {
            return;
        }
        let block = preview.controls(first);
        if block.part[0] < 0.5 || block.window[0] > 0.5 {
            return;
        }
        let size = Self::level_size(drawable);
        if size != self.size {
            self.make_slots(device, size);
        }
        let (_, k) = self.level_of(&block);
        let due = ((time - begin) / PREBUILD_SPAN).min(1.0);
        let rows = (self.size.1 as f64 * due).ceil() as u32;
        if self
            .slot_of(k)
            .is_some_and(|i| self.slots[i].rows_done >= rows)
        {
            return;
        }
        self.clock += 1;
        let mut encoder = device.create_command_encoder(&Default::default());
        let i = self.claim(k, None);
        self.build(queue, &mut encoder, i, rows);
        self.slots[i].used = self.clock;
        queue.submit([encoder.finish()]);
    }

    /// Levels resident and complete, for tests: (level, rows done).
    #[cfg(test)]
    fn resident(&self) -> Vec<(usize, u32)> {
        self.slots
            .iter()
            .filter_map(|s| s.level.map(|l| (l, s.rows_done)))
            .collect()
    }

    /// Level k's rows built and its texels as they are on the GPU, if it is
    /// resident; for tests.
    #[cfg(test)]
    pub fn read_level(&self, device: &Device, queue: &Queue, k: usize) -> Option<(u32, Vec<u8>)> {
        let slot = &self.slots[self.slot_of(k)?];
        let (width, height) = self.size;
        let row = width as u64 * 8;
        let stride = row.div_ceil(256) * 256;
        let read = device.create_buffer(&BufferDescriptor {
            label: Some("Part 5 zoom level readback"),
            size: stride * height as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            slot._texture.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &read,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride as u32),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        read.slice(..).map_async(MapMode::Read, |_| {});
        device.poll(PollType::wait_indefinitely()).unwrap();
        let mapped = read.slice(..).get_mapped_range().unwrap();
        let texels = mapped
            .chunks(stride as usize)
            .flat_map(|r| &r[..row as usize])
            .copied()
            .collect();
        Some((slot.rows_done, texels))
    }
}

fn level_texture(device: &Device, (width, height): (u32, u32)) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("Part 5 zoom level"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: LEVEL_FORMAT,
        usage: LEVEL_USAGE,
        view_formats: &[],
    })
}

/// A level texture's uses, and in tests read back ([`ZoomLevels::read_level`]).
#[cfg(not(test))]
const LEVEL_USAGE: TextureUsages =
    TextureUsages::STORAGE_BINDING.union(TextureUsages::TEXTURE_BINDING);
#[cfg(test)]
const LEVEL_USAGE: TextureUsages = TextureUsages::STORAGE_BINDING
    .union(TextureUsages::TEXTURE_BINDING)
    .union(TextureUsages::COPY_SRC);

#[cfg(test)]
mod tests {
    use super::fixed::{Cx, Fx};
    use super::*;

    #[test]
    fn fixed_point_arithmetic_is_exact_where_f64_is() {
        for s in [
            "-0.25",
            "1.5",
            "-1.75",
            "0.0001220703125",
            "-3.814697265625e-6",
        ] {
            let v: f64 = s.parse().unwrap();
            let text = format!("{v:.30}");
            assert_eq!(Fx::parse(&text).to_f64(), v, "{s}");
            assert_eq!(Fx::from_f64(v).to_f64(), v, "{s}");
        }
        let a = Fx::parse("-0.7734847928153418");
        let b = Fx::parse("0.1224476274596419");
        assert!((a.mul(&b).to_f64() - -0.7734847928153418 * 0.1224476274596419).abs() < 1e-16);
        assert_eq!(a.add(&a.neg()), Fx::ZERO);
    }

    /// Known orbits: c = -1 cycles 0, -1, 0 exactly; the period-3 nucleus
    /// on the real axis (-1.7548776662466927600495088963585286918946066177727931...)
    /// returns to 0 to the digits given; and the target's own nucleus returns
    /// to 0 after its period, staying bounded on the way, as a nucleus does.
    #[test]
    fn fixed_point_orbits_return_to_zero_at_known_nuclei() {
        let orbit = reference_orbit(&Cx::parse(["-1", "0"]), 4);
        assert_eq!(
            orbit,
            vec![[0.0, 0.0], [-1.0, 0.0], [0.0, 0.0], [-1.0, 0.0], [0.0, 0.0]]
        );
        let airplane = Cx::parse([
            "-1.75487766624669276004950889635852869189460661777279314398",
            "0",
        ]);
        let orbit = reference_orbit(&airplane, 3);
        assert!((orbit[1][0] - -1.7548776662466927).abs() < 1e-15);
        assert!(cabs(orbit[3]) < 1e-55, "{:?}", orbit[3]);
        let target = Cx::parse(TARGET.nucleus);
        let orbit = reference_orbit(&target, TARGET.period);
        assert!(orbit.iter().all(|z| cabs(*z) <= 2.0));
        assert!(
            cabs(orbit[TARGET.period]) < 1e-80,
            "{:?}",
            orbit[TARGET.period]
        );
        // Its smallest values (the parents' near-zeros) stay in f32's range.
        let smallest = orbit[1..TARGET.period]
            .iter()
            .map(|z| cabs(*z))
            .fold(1.0, f64::min);
        assert!(smallest > 1e-30 && smallest < 1e-10, "{smallest}");
    }

    #[test]
    fn floatexp_round_trips() {
        for z in [[1.0, 0.0], [-3.5e-55, 2.0e-56], [7.0e40, -1.0], [0.75, 0.5]] {
            let (m, e) = floatexp(z);
            let top = m[0].abs().max(m[1].abs());
            assert!((0.5..1.0).contains(&top), "{z:?}");
            for i in 0..2 {
                let back = ldexp(m[i] as f64, e);
                assert!((back - z[i]).abs() <= 1e-7 * cabs(z), "{z:?}");
            }
        }
        assert_eq!(floatexp([0.0, 0.0]).1, ZERO_EXPONENT);
    }

    /// The depth is the port's magnification: level k covers depths k to
    /// k + 1, the view's height halves each level and is continuous where
    /// the next level takes over (level k at 2x is level k + 1 at 1x), and
    /// the light follows the window through the part's fade and clear.
    #[test]
    fn depth_follows_the_ports_magnification() {
        let path = ZoomPath::new(16.311531117267);
        assert_eq!(path.last_level(), 16);
        assert!((path.height(path.stop) - TARGET.stop_height).abs() < 1e-9 * TARGET.stop_height);
        for k in 0..16 {
            let a = path.height(k as f64) / 2.0;
            let b = path.height(k as f64 + 1.0);
            assert!((a - b).abs() < 1e-12 * a);
            // The kernel's texel at level k is twice level k + 1's.
            let fa = path.frame(k, 1024, 960, 3);
            let fb = path.frame(k + 1, 1024, 960, 3);
            assert!((fa.ps_log2 - fb.ps_log2 - 1.0).abs() < 1e-5);
            assert_eq!(fb.radii_base - fa.radii_base, TARGET.period as u32 * 3);
        }
        // The start is 81,000 times the stop: the port's 16.31 doublings.
        let ratio = path.height(0.0) / path.height(path.stop);
        assert!((ratio.log2() - 16.311531117267).abs() < 1e-9);
        let block = |level: f32, fade: f32, cleared: f32, since: f32| FieldBlock {
            part: [1.0, 0.0, 0.0, 0.0],
            zoom: [1.5, 0.0, level, 0.0],
            window: [cleared, since, fade, 0.0],
            ..FieldBlock::zeroed()
        };
        let light = |b: FieldBlock| zoom_controls(&b, 0.0)[0];
        assert_eq!(light(FieldBlock::default()), [0.0; 4]);
        let full = light(block(3.0, 1.0, 0.0, -1.0));
        assert!(full[2] > 0.1 && full[3] == 1.0);
        assert!(light(block(3.0, 0.5, 0.0, -1.0))[2] < full[2]);
        let out = light(block(16.3, 1.0, 1.0, 0.5));
        assert_eq!([out[0], out[1], out[2]], [0.0; 3]);
        assert_eq!(out[3], 1.0);
    }

    #[test]
    fn level_sizes_follow_the_window() {
        // The window is 100 rows of the 4:3 frame's 200.
        assert_eq!(ZoomLevels::level_size((3840, 2160)).1, 2225);
        assert_eq!(ZoomLevels::level_size((2560, 1440)).1, 1484);
        let (w, h) = ZoomLevels::level_size((960, 540));
        assert!(h >= LEVEL_ROWS[0] && w as f64 >= h as f64 * 16.0 / 15.0);
    }

    #[test]
    fn the_kernel_is_its_own_valid_module() {
        let module = naga::front::wgsl::parse_str(KERNEL_WGSL)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(KERNEL_WGSL)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(KERNEL_WGSL)));
        // The kernel's Frame matches KernelFrame's size.
        assert_eq!(std::mem::size_of::<KernelFrame>(), 112);
        assert_eq!(std::mem::size_of::<ZoomView>(), 32);
    }

    #[test]
    fn bla_tables_skip_like_the_iterations_they_replace() {
        let path = ZoomPath::new(16.3);
        // Level 3 at m = 1 is 8 iterations of delta' = 2 Z delta + dc,
        // linearised.
        let (a, b, _) = path.bla.levels[3][0];
        let dc = [1e-60, -3e-61];
        // delta_1 = delta_0^2 + dc = dc, from m = 1 on.
        let d = dc;
        let lin = cadd(cmul(a, d), cmul(b, dc));
        let mut step = d;
        for m in 1..9 {
            step = cadd(
                cmul([2.0 * path.orbit[m][0], 2.0 * path.orbit[m][1]], step),
                dc,
            );
        }
        assert!(cabs([lin[0] - step[0], lin[1] - step[1]]) < 1e-9 * cabs(step));
    }

    /// Levels are resident where advance leaves them: needs a GPU, so it
    /// runs with the ignored tests.
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn levels_are_complete_when_shown() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut levels = test_levels(&device);
        let at = |level: f32| FieldBlock {
            part: [1.0, 0.0, 0.0, 0.0],
            zoom: [1.5, 0.0, level, 0.0],
            ..FieldBlock::zeroed()
        };
        levels.advance(&device, &queue, &at(5.7), (320, 200));
        let rows = levels.size.1;
        assert!(levels.resident().contains(&(5, rows)));
        levels.advance(&device, &queue, &at(5.71), (320, 200));
        assert!(
            levels
                .resident()
                .iter()
                .any(|&(l, r)| l == 6 && r > 0 && r <= rows)
        );
        levels.advance(&device, &queue, &at(2.0), (320, 200));
        assert!(levels.resident().contains(&(2, rows)));
    }

    /// What a seek costs: building a level at once, at the level sizes of
    /// 2560 x 1440 and 3840 x 2160 (prints; needs a GPU).
    #[test]
    #[ignore = "needs a GPU; run with --ignored --nocapture"]
    fn level_build_costs() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let t = std::time::Instant::now();
        let mut path = ZoomPath::new(16.3115);
        let path_ms = t.elapsed().as_secs_f64() * 1e3;
        let t = std::time::Instant::now();
        path.radii();
        let radii_ms = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "orbit and BLA table in {path_ms:.1} ms, radii for {} levels in {radii_ms:.1} ms",
            path.last_level() + 1
        );
        let mut levels = test_levels(&device);
        for drawable in [(2560, 1440), (3840, 2160)] {
            let size = ZoomLevels::level_size(drawable);
            levels.make_slots(&device, size);
            for k in [0usize, 4, 8, 12, 16] {
                let mut best = f64::INFINITY;
                for _ in 0..3 {
                    let i = levels.claim(k, None);
                    levels.slots[i].rows_done = 0;
                    let mut encoder = device.create_command_encoder(&Default::default());
                    let t = std::time::Instant::now();
                    levels.build(&queue, &mut encoder, i, size.1);
                    queue.submit([encoder.finish()]);
                    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                    best = best.min(t.elapsed().as_secs_f64() * 1e3);
                }
                println!("{drawable:?}: level {k} at {size:?} built in {best:.1} ms");
            }
        }
    }

    /// The whole part-5 picture is the same however a moment is reached:
    /// playing on frame by frame (across a doubling, where the next level
    /// takes over), seeking and scrubbing back and forth, against a new
    /// renderer at each moment, byte for byte.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn zoom_is_the_same_however_it_is_reached() {
        use crate::hd::clip::{Clip, ScrollerLighting};
        use crate::hd::renderer::{Renderer, View};
        let data = crate::test_support::data();
        let clip = Clip::load_field(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || {
            pollster::block_on(Renderer::new(&instance, None))
                .unwrap()
                .0
        };
        let shot = |r: &mut Renderer, time: f64| {
            let view = View {
                time,
                size: (640, 360),
                comparison: 0,
                modern: true,
                hud: None,
            };
            r.render_rgba(&clip, view, wgpu::TextureFormat::Rgba8UnormSrgb)
                .unwrap()
        };
        let field = clip.field.as_ref().unwrap();
        // A doubling: the frames either side of a swap.
        let swap = (1..field.frames.len())
            .find(|&j| field.frames[j].swaps == 8 && field.frames[j].state.swapped)
            .unwrap();
        let from = field.seconds(swap) - 0.25;
        let mut playing = renderer();
        let shots: Vec<Vec<u8>> = (0..30)
            .map(|n| shot(&mut playing, from + n as f64 / 60.0))
            .collect();
        for n in [0, 14, 15, 29] {
            let time = from + n as f64 / 60.0;
            assert!(
                shots[n] == shot(&mut renderer(), time),
                "playing on, {time}"
            );
        }
        for time in [95.0, 83.2, 96.9, 97.6, 90.05] {
            let seen = shot(&mut playing, time);
            assert!(seen == shot(&mut renderer(), time), "seeking, {time}");
        }
    }

    /// The zoom's first level is built ahead, in the frames before part 5's
    /// view is first drawn (part 6's fade), and is the same as one built
    /// there at once: played on at 60 frames a second, nothing is built
    /// before the span, part of it during it and all of it by its end, and
    /// at the fade its texels and the frames drawn from then on match a new
    /// renderer's byte for byte; a seek into the span, or back before it,
    /// carries on from the rows built, and the fade finishes the rest.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn first_level_is_built_ahead_the_same() {
        use crate::hd::clip::{Clip, ScrollerLighting};
        use crate::hd::renderer::{Renderer, View};
        let data = crate::test_support::data();
        let clip = Clip::load_field(&data, ScrollerLighting::Studio).unwrap();
        let field = clip.field.as_ref().unwrap();
        let first = field.picture_fade.unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || pollster::block_on(Renderer::new(&instance, None)).unwrap();
        let view = |time| View {
            time,
            size: (640, 360),
            comparison: 0,
            modern: true,
            hud: None,
        };
        let level = |r: &Renderer| {
            let field = r.field_renderer().unwrap();
            field.zoom_level(&r.device, &r.queue, 0)
        };
        let rows = |r: &Renderer| level(r).map_or(0, |l| l.0);
        let play = |r: &mut Renderer, from: f64, to: f64| {
            let mut k = 0;
            while from + k as f64 / 60.0 < to {
                r.prepare(&clip, view(from + k as f64 / 60.0));
                k += 1;
            }
        };
        let (mut fresh, format) = renderer();
        fresh.prepare(&clip, view(first));
        let at_once = level(&fresh).unwrap();
        let height = ZoomLevels::level_size(view(first).size).1;
        assert_eq!(at_once.0, height, "built at the fade");
        let begin = first - PREBUILD_LEAD - PREBUILD_SPAN;
        // Played on through part 6: built in the span, done before the fade.
        let (mut playing, _) = renderer();
        play(&mut playing, begin - 0.5, begin);
        assert_eq!(rows(&playing), 0, "nothing before the span");
        play(&mut playing, begin, begin + 0.5 * PREBUILD_SPAN);
        let part = rows(&playing);
        assert!(part > 0 && part < height, "{part} rows half way");
        play(&mut playing, begin + 0.5 * PREBUILD_SPAN, first - 0.1);
        assert!(level(&playing).unwrap() == at_once, "built ahead");
        for time in [first, first + 0.5, first + 0.85] {
            let pixels = playing.render_rgba(&clip, view(time), format).unwrap();
            let new = renderer().0.render_rgba(&clip, view(time), format).unwrap();
            assert!(pixels == new, "frame at {time}");
        }
        assert!(level(&playing).unwrap() == at_once, "shown at the fade");
        // Seeks into the span, back before it, and on to the fade.
        let (mut seeking, _) = renderer();
        play(&mut seeking, first - 0.9, first - 0.85);
        let part = rows(&seeking);
        assert!(part > 0 && part < height, "{part} rows after a seek");
        play(&mut seeking, first - 20.0, first - 19.9);
        assert_eq!(rows(&seeking), part, "kept before the span");
        play(&mut seeking, first, first + 0.01);
        assert!(level(&seeking).unwrap() == at_once, "finished at the fade");
    }

    /// Levels for the music run's stop, without a preview.
    fn test_levels(device: &Device) -> ZoomLevels {
        ZoomLevels::with_path(device, ZoomPath::new(16.3115), ZoomLevels::layout(device))
    }
}
