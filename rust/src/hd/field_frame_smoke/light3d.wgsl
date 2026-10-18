// Part 5's smoke, 3D: its lighting, from a smoke prototype kept locally, lit
// by the real scene's lights: the window's light and the lava's, as the
// frame reflects them (field_frame.wgsl's frame_lights, mirrored in
// field_frame_smoke.rs). An authored model:
//  prep:      density averaged ls^3 onto the light grid (a 3D texture, so
//             marches use hardware trilinear filtering).
//  light_vol: per light-grid cell and light, optical depth along the segment
//             to the light (nearest point of rectangle lights), plus the
//             frame and ornaments as an occluder where the segment crosses
//             the picture plane: their softened coverage, so they cast
//             volumetric shadows.
//  column:    the camera is orthographic along -z (the frame is drawn flat in
//             the picture plane), so each scalar column is one view ray.
//             Front to back, each cell adds single scattering
//             (Henyey-Greenstein phase) with the usual multiple-scattering
//             octaves, and ambient. Two layers, split at the picture plane,
//             so the frame lies between them: the smoke behind it and the
//             smoke in front.

struct Grid {
    nx: u32, ny: u32, nz: u32, ds: u32,
    sx: u32, sy: u32, sz: u32, ls: u32,
    lx: u32, ly: u32, lz: u32, _p: u32,
    ou: u32, ov: u32, ow: u32, _q: u32,
    z0: f32, h: f32, inv_ds: f32, _r: f32,
    x0: f32, y0: f32, _s: f32, _t: f32,   // domain origin (screen units; margins off screen)
}

struct Light {
    pos: vec4f,   // xyz screen units; w: 0 point, 1 rectangle/segment at z = pos.z
    rect: vec4f,  // x0 y0 x1 y1 (nearest point)
    col: vec4f,   // rgb; w: falloff radius
    extra: vec4f, // x: size (penumbra), y: phase g, z: 1 = cast frame shadow
}

struct Frame {
    t: f32, vis: f32, sigma: f32, half: f32,
    albedo: vec4f,
    ambient: vec4f,
    n_lights: u32, k_depth: u32, march: u32, mode: u32,
    lights: array<Light, 4>,
    vol: vec4f,    // z0, z1, -, -
    phase: vec4f,  // octave weight a, extinction b, phase c, -
}

@group(0) @binding(0) var<uniform> g: Grid;
@group(0) @binding(1) var<uniform> f: Frame;
@group(0) @binding(2) var<storage, read> sc: array<Packed>;
@group(0) @binding(3) var dens_w: texture_storage_3d<rgba16float, write>;
@group(0) @binding(4) var dens_t: texture_3d<f32>;
@group(0) @binding(5) var samp: sampler;
@group(0) @binding(6) var tau_w: texture_storage_3d<rgba16float, write>;
@group(0) @binding(7) var tau_t: texture_3d<f32>;
@group(0) @binding(8) var layer_b: texture_storage_2d<rgba16float, write>;
@group(0) @binding(9) var layer_f: texture_storage_2d<rgba16float, write>;
// The obstacles over the scalar grid's columns (solver3d.wgsl).
@group(0) @binding(10) var obstacle: texture_2d<f32>;

fn hs() -> f32 { return g.h * g.inv_ds; }
fn dom() -> vec3f { return vec3f(f32(g.sx), f32(g.sy), f32(g.sz)) * hs(); }
fn origin() -> vec3f { return vec3f(g.x0, g.y0, g.z0); }
fn uvw(p: vec3f) -> vec3f { return (p - origin()) / dom(); }

fn hash(a: u32, b: u32, c: u32) -> f32 {
    var x = a * 1664525u + b * 1013904223u + c * 2654435761u;
    x ^= x >> 16u; x *= 0x7feb352du; x ^= x >> 15u; x *= 0x846ca68bu; x ^= x >> 16u;
    return f32(x >> 8u) / 16777216.0;
}

fn light_point(l: Light, p: vec3f) -> vec3f {
    if (l.pos.w > 0.5) { return vec3f(clamp(p.xy, l.rect.xy, l.rect.zw), l.pos.z); }
    return l.pos.xyz;
}

@compute @workgroup_size(8, 8, 4)
fn prep(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.lx || id.y >= g.ly || id.z >= g.lz) { return; }
    var acc = 0.0;
    for (var c = 0u; c < g.ls; c++) {
        for (var b = 0u; b < g.ls; b++) {
            for (var a = 0u; a < g.ls; a++) {
                let q = id * g.ls + vec3u(a, b, c);
                acc += unpk(sc[(q.z * g.sy + q.y) * g.sx + q.x]).x;
            }
        }
    }
    textureStore(dens_w, id, vec4f(acc / f32(g.ls * g.ls * g.ls), 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8, 4)
fn light_vol(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.lx || id.y >= g.ly || id.z >= g.lz) { return; }
    let lh = hs() * f32(g.ls);
    let p = origin() + (vec3f(id) + 0.5) * lh;
    var taus = vec4f(0.0);
    for (var l = 0u; l < f.n_lights; l++) {
        let lt = f.lights[l];
        let lp = light_point(lt, p);
        let d = lp - p;
        let dist = length(d);
        let reach = min(dist, 1.2);
        let dir = d / max(dist, 1e-6);
        let jit = hash(id.x + id.z * 977u, id.y, l);
        var od = 0.0;
        for (var m = 0u; m < f.march; m++) {
            let q = p + dir * reach * (f32(m) + jit) / f32(f.march);
            od += textureSampleLevel(dens_t, samp, uvw(q), 0.0).r;
        }
        var tau = f.sigma * od * reach / f32(f.march);
        // the frame and ornaments, where the segment crosses the picture
        // plane: their softened coverage there
        if (lt.extra.z > 0.5 && p.z * lp.z < 0.0) {
            let t = -p.z / (lp.z - p.z);
            let q = p + t * d;
            let uv = (q.xy - origin().xy) / dom().xy;
            let cover = textureSampleLevel(obstacle, samp, uv, 0.0).b;
            tau += -log(max(1.0 - 0.995 * cover, 6e-6));
        }
        taus[l] = tau;
    }
    textureStore(tau_w, id, taus);
}

fn hg(c: f32, gg: f32) -> f32 {
    let d = 1.0 + gg * gg - 2.0 * gg * c;
    return (1.0 - gg * gg) / (d * sqrt(d)); // x 4 pi: isotropic = 1
}

@compute @workgroup_size(16, 16, 1)
fn column(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy) { return; }
    let h = hs();
    var lb = vec3f(0.0); var tb = 1.0;
    var lf = vec3f(0.0); var tf = 1.0;
    for (var kk = 0u; kk < g.sz; kk++) {
        let k = g.sz - 1u - kk;
        let rho = unpk(sc[(k * g.sy + id.y) * g.sx + id.x]).x;
        if (rho < 1e-5) { continue; }
        let p = origin() + (vec3f(f32(id.x), f32(id.y), f32(k)) + 0.5) * h;
        let a = 1.0 - exp(-f.sigma * rho * h);
        let taus = textureSampleLevel(tau_t, samp, uvw(p), 0.0);
        var e = f.ambient.rgb;
        for (var l = 0u; l < f.n_lights; l++) {
            let lt = f.lights[l];
            let lp = light_point(lt, p);
            let d = p - lp;
            let dist = max(length(d), 1e-4);
            let c = d.z / dist;              // light travels lp -> p, then to the viewer (+z)
            let fall = 1.0 / (1.0 + dist * dist / (lt.col.w * lt.col.w));
            let tau = taus[l];
            var ms = 0.0;
            var wgt = 1.0;
            var ext = 1.0;
            var gg = lt.extra.y;
            for (var o = 0; o < 3; o++) {
                ms += wgt * exp(-ext * tau) * hg(c, gg);
                wgt *= f.phase.x;
                ext *= f.phase.y;
                gg *= f.phase.z;
            }
            e += lt.col.rgb * fall * ms;
        }
        let s = a * f.albedo.rgb * e;
        if (p.z > 0.0) {
            lf += tf * s; tf *= 1.0 - a;
        } else {
            lb += tb * s; tb *= 1.0 - a;
        }
    }
    let px = vec2i(i32(id.x), i32(g.sy) - 1 - i32(id.y));
    textureStore(layer_b, px, vec4f(lb * f.vis, (1.0 - tb) * f.vis));
    textureStore(layer_f, px, vec4f(lf * f.vis, (1.0 - tf) * f.vis));
}
