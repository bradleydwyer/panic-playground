// Part 5's smoke, 2D: its lighting at scalar-grid resolution, from a smoke
// prototype kept locally, lit by the real scene's window and lava lights
// (field_frame_smoke.rs). It draws the layer behind the frame only.
// Authored model, not physical measurement:
//  - The 2D density is spread uniformly through a slab |z| < half around the
//    frame plane (z toward the viewer), so the view ray's optical depth is
//    sigma * rho and the volume extinction is sigma * rho / (2 half).
//  - K depth samples through the slab are composited front to back.
//  - For each sample and light, a march toward the light through the
//    density: in-plane along the light's direction, as far as the 3D ray
//    stays in the slab (or reaches the light), giving self-shadowing. Lights
//    inside the slab (the glows beside the smoke) cast long in-plane shadows;
//    lights in front of it light the front surface.
//  - Near the sample the march reads the full-resolution density; further
//    out a 4x4-averaged copy, so long marches stay smooth and cheap.
//  - Multiple scattering: the usual octave sum of attenuated transmittances.
//  - Isotropic phase, constant albedo, plus a small ambient term.

struct Grid {
    nx: u32, ny: u32, sx: u32, sy: u32,
    ds: f32, inv_ds: f32, omega: f32, _pad: f32,
}

struct Light {
    pos: vec4f,   // xyz screen units (z toward the viewer); w: 0 point, 1 rectangle
    rect: vec4f,  // rectangle lights: x0 y0 x1 y1 (nearest point is the light)
    col: vec4f,   // rgb radiance scale; w: falloff radius
    extra: vec4f, // (3D only)
}

struct Frame {
    t: f32, vis: f32, sigma: f32, half: f32,
    albedo: vec4f,
    ambient: vec4f,
    n_lights: u32, k_depth: u32, march: u32, mode: u32,
    lights: array<Light, 4>,
    key: vec4f,    // w: the near field's reach (screen units)
}

@group(0) @binding(0) var<uniform> g: Grid;
@group(0) @binding(1) var<uniform> f: Frame;
@group(0) @binding(2) var<storage, read> sc: array<vec2f>;
@group(0) @binding(3) var out_tex: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var<storage, read_write> coarse: array<f32>;

const CS: u32 = 4u;

@compute @workgroup_size(16, 16)
fn downsample(@builtin(global_invocation_id) id: vec3u) {
    let cw = g.sx / CS;
    let ch = g.sy / CS;
    if (id.x >= cw || id.y >= ch) { return; }
    var acc = 0.0;
    for (var b = 0u; b < CS; b++) {
        for (var a = 0u; a < CS; a++) {
            acc += sc[(id.y * CS + b) * g.sx + id.x * CS + a].x;
        }
    }
    coarse[id.y * cw + id.x] = acc / f32(CS * CS);
}

fn coarse_at(i: i32, j: i32) -> f32 {
    let cw = i32(g.sx / CS);
    let ch = i32(g.sy / CS);
    if (i < 0 || j < 0 || i >= cw || j >= ch) { return 0.0; }
    return coarse[u32(j * cw + i)];
}

fn dens_coarse(p: vec2f) -> f32 {
    let q = p * f32(g.sy / CS) - 0.5;
    let b = floor(q);
    let t = q - b;
    let i = i32(b.x);
    let j = i32(b.y);
    return mix(mix(coarse_at(i, j), coarse_at(i + 1, j), t.x), mix(coarse_at(i, j + 1), coarse_at(i + 1, j + 1), t.x), t.y);
}

fn rho_at(i: i32, j: i32) -> f32 {
    if (i < 0 || j < 0 || i >= i32(g.sx) || j >= i32(g.sy)) { return 0.0; }
    return sc[u32(j) * g.sx + u32(i)].x;
}

fn dens(p: vec2f) -> f32 {
    let q = p * f32(g.sy) - 0.5;
    let b = floor(q);
    let t = q - b;
    let i = i32(b.x);
    let j = i32(b.y);
    return mix(mix(rho_at(i, j), rho_at(i + 1, j), t.x), mix(rho_at(i, j + 1), rho_at(i + 1, j + 1), t.x), t.y);
}

// Fixed per-cell jitter (not per frame, so video stays stable).
fn hash(a: u32, b: u32, c: u32) -> f32 {
    var x = a * 1664525u + b * 1013904223u + c * 2654435761u;
    x ^= x >> 16u; x *= 0x7feb352du; x ^= x >> 15u; x *= 0x846ca68bu; x ^= x >> 16u;
    return f32(x >> 8u) / 16777216.0;
}

fn tms(tau: f32) -> f32 {
    return (exp(-tau) + 0.5 * exp(-0.5 * tau) + 0.25 * exp(-0.25 * tau)) / 1.75;
}

@compute @workgroup_size(16, 16)
fn light(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy) { return; }
    let p = (vec2f(id.xy) + 0.5) / f32(g.sy);
    let rho = sc[id.y * g.sx + id.x].x;
    var lo = vec3f(0.0);
    var tv = 1.0;
    if (rho > 1e-4) {
        let kd = max(f.k_depth, 1u);
        let a = 1.0 - exp(-f.sigma * rho / f32(kd));
        let vol = f.sigma / (2.0 * f.half);
        for (var k = 0u; k < kd; k++) {
            let z = f.half * (1.0 - (2.0 * f32(k) + 1.0) / f32(kd));
            var e = f.ambient.rgb * tms(0.5 * f.sigma * rho);
            for (var l = 0u; l < f.n_lights; l++) {
                let lt = f.lights[l];
                var lp = lt.pos.xy;
                if (lt.pos.w > 0.5) { lp = clamp(p, lt.rect.xy, lt.rect.zw); }
                let dxy = lp - p;
                let dl = length(dxy);
                let dz = lt.pos.z - z;
                let d3 = sqrt(dl * dl + dz * dz);
                // fraction of the segment to the light that stays in the slab
                var frac = 1.0;
                if (dz > 1e-5) { frac = min(1.0, (f.half - z) / dz); }
                if (dz < -1e-5) { frac = min(1.0, (z + f.half) / -dz); }
                let path = frac * d3;
                let reach = frac * dl;
                var dir = vec2f(0.0);
                if (dl > 1e-6) { dir = dxy / dl; }
                let jit = hash(id.x, id.y, l * 7u + k);
                // near field at full resolution, far field on the coarse copy
                let near = min(reach, f.key.w);
                var od = 0.0;
                for (var m = 0u; m < 6u; m++) {
                    od += dens(p + dir * near * (f32(m) + jit) / 6.0) * near / 6.0;
                }
                let far = reach - near;
                if (far > 1e-5) {
                    for (var m = 0u; m < f.march; m++) {
                        od += dens_coarse(p + dir * (near + far * (f32(m) + jit) / f32(f.march))) * far / f32(f.march);
                    }
                }
                // in-plane integral -> 3D optical depth along the slanted ray
                var stretch = 1.0;
                if (reach > 1e-6) { stretch = path / reach; }
                var tau = vol * od * stretch;
                if (reach <= 1e-6) { tau = vol * rho * path; }
                let r0 = lt.col.w;
                let fall = 1.0 / (1.0 + d3 * d3 / (r0 * r0));
                e += lt.col.rgb * fall * tms(tau);
            }
            lo += tv * a * f.albedo.rgb * e;
            tv *= 1.0 - a;
        }
    }
    let row = i32(g.sy) - 1 - i32(id.y);
    textureStore(out_tex, vec2i(i32(id.x), row), vec4f(lo * f.vis, (1.0 - tv) * f.vis));
}
