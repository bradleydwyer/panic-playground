// Part 5's smoke, 3D: the frame author's solver (with field_frame_smoke.rs,
// which builds the module and dispatches it, multigrid.wgsl, the pressure
// solve, and light3d.wgsl, the lighting). Lifted from a smoke prototype
// kept locally and adapted to the real scene: the obstacles are the frame
// and ornaments baked from the user's PSTART picture, and the lava's heat
// follows the real mountain's outline. Authored throughout; nothing here is
// from the original demo.
//
// The method: incompressible Navier-Stokes by stable fluids (Stam 1999) on
// a 3D MAC grid, MacCormack advection with the Selle et al. limiter,
// buoyancy and vorticity confinement (Fedkiw, Stam and Jensen 2001).
//
// Units: lengths in screen units (1 = the 16:9 screen's height, x right
// from its left edge, y up from its bottom, z toward the viewer from the
// picture plane; scene = (x - 8/9, y - 1/2, z) * 200). Velocity grid
// nx x ny x nz, cubic cells of h screen units; cell (i, j, k) is centred
// at (i + .5, j + .5, k + .5) in cell units; j = 0 at the bottom (gravity
// -y), k = 0 at the back (z = z0). Screen position: x = x0 + (i + .5) h,
// y = y0 + (j + .5) h, z = z0 + (k + .5) h; the margins beyond the screen
// keep the open boundary and the sponge out of sight.
// One packed velocity buffer: u (nx+1)*ny*nz at ou, v nx*(ny+1)*nz at ov,
// w nx*ny*(nz+1) at ow. Velocities in cells per second. Pressure stores
// dt * p. Scalars (density, temperature) on a grid ds times finer.
// Boundaries: all six faces open (p = 0 outside, clean air in). Obstacles
// (the frame, its window and the ornaments) are solid cells.
// Determinism: pure gathers only; each kernel writes every element it owns.

struct Grid {
    nx: u32, ny: u32, nz: u32, ds: u32,
    sx: u32, sy: u32, sz: u32, ls: u32,
    lx: u32, ly: u32, lz: u32, _p: u32,
    ou: u32, ov: u32, ow: u32, _q: u32,
    z0: f32, h: f32, inv_ds: f32, _r: f32,
    x0: f32, y0: f32, _s: f32, _t: f32,   // domain origin (screen units; margins off screen)
}

// Per step (dynamic offset), from field_frame_smoke.rs's story. Per
// emitter three vec4: (x, y, z, radius) screen units; (density rate,
// temperature rate, pull 1/s, -); (target vx, vy, vz in cells/s, -).
// heat: the lava's heat along the mountain's outline (temperature and
// density rates, the band's depth below the outline in screen units, the
// step's time for its flicker); heat_z: the band's depth centre and radius
// (screen z), the flicker's width and the frame's half width (screen
// units); and the mountain's
// camera and projection (field_common.wgsl's): camera map x, y, height and
// the projection's q, then its view and right directions.
struct Step {
    dt: f32, buoy_t: f32, buoy_rho: f32, vort: f32,
    keep_rho: f32, keep_t: f32, keep_vel: f32, n_emit: u32,
    wind: vec4f,
    heat: vec4f,
    heat_z: vec4f,
    camera: vec4f,
    view: vec4f,
    mview: array<vec4f, 4>,
    emit: array<vec4f, 48>,
}

@group(0) @binding(0) var<uniform> g: Grid;
@group(0) @binding(1) var<uniform> st: Step;
@group(0) @binding(2) var<storage, read_write> vel: array<f32>;
@group(0) @binding(3) var<storage, read_write> vel_hat: array<f32>;
@group(0) @binding(4) var<storage, read_write> vel_tmp: array<f32>;
// (density, temperature): vec2f, or two f16 packed in a u32 when the host
// prefixes the packed variant of Packed/pk/unpk (halves memory and traffic;
// the canonical simulation then runs at that precision, so checkpoints stay
// exact copies).
@group(0) @binding(5) var<storage, read_write> sc: array<Packed>;
@group(0) @binding(6) var<storage, read_write> sc_hat: array<Packed>;
@group(0) @binding(7) var<storage, read_write> sc_tmp: array<Packed>;
@group(0) @binding(8) var<storage, read_write> divg: array<f32>;
@group(0) @binding(9) var<storage, read_write> pr: array<f32>;
@group(0) @binding(10) var<storage, read_write> curl: array<vec4f>;
@group(0) @binding(11) var<storage, read_write> solid_v: array<u32>;
@group(0) @binding(12) var<storage, read_write> solid_s: array<u32>;
// Cell-centred velocity of the step's start, for the characteristics only
// (hardware trilinear; the advected quantities are sampled exactly from the
// buffers). Written by cell_vel through its own bind group (binding 15).
@group(0) @binding(13) var vtex: texture_3d<f32>;
@group(0) @binding(14) var vsamp: sampler;
@group(0) @binding(15) var vtex_w: texture_storage_3d<rgba16float, write>;
@group(0) @binding(16) var<storage, read_write> conf: array<vec4f>;
// The obstacles over the scalar grid's columns (field_frame_smoke.rs): r the
// top of the solid (screen z), g 1 where the column is solid, b the
// softened coverage the lighting shadows with.
@group(0) @binding(17) var obstacle: texture_2d<f32>;
// The fitted relief (field_common.wgsl's relief_at) and, per scalar
// column, the mountain's outline (screen y, -1 where none).
@group(0) @binding(18) var relief_tex: texture_2d<f32>;
@group(0) @binding(19) var<storage, read_write> outline: array<f32>;

fn ldv(w: u32, k: u32) -> f32 {
    if (w == 1u) { return vel_hat[k]; }
    if (w == 2u) { return vel_tmp[k]; }
    return vel[k];
}
fn ui(i: i32, j: i32, k: i32) -> u32 {
    let a = u32(clamp(i, 0, i32(g.nx)));
    let b = u32(clamp(j, 0, i32(g.ny) - 1));
    let c = u32(clamp(k, 0, i32(g.nz) - 1));
    return g.ou + (c * g.ny + b) * (g.nx + 1u) + a;
}
fn vi(i: i32, j: i32, k: i32) -> u32 {
    let a = u32(clamp(i, 0, i32(g.nx) - 1));
    let b = u32(clamp(j, 0, i32(g.ny)));
    let c = u32(clamp(k, 0, i32(g.nz) - 1));
    return g.ov + (c * (g.ny + 1u) + b) * g.nx + a;
}
fn wi(i: i32, j: i32, k: i32) -> u32 {
    let a = u32(clamp(i, 0, i32(g.nx) - 1));
    let b = u32(clamp(j, 0, i32(g.ny) - 1));
    let c = u32(clamp(k, 0, i32(g.nz)));
    return g.ow + (c * g.ny + b) * g.nx + a;
}
// comp 0 = u, 1 = v, 2 = w
fn face(w: u32, comp: u32, i: i32, j: i32, k: i32) -> f32 {
    if (comp == 0u) { return ldv(w, ui(i, j, k)); }
    if (comp == 1u) { return ldv(w, vi(i, j, k)); }
    return ldv(w, wi(i, j, k));
}
fn face_offset(comp: u32) -> vec3f {
    if (comp == 0u) { return vec3f(0.0, 0.5, 0.5); }
    if (comp == 1u) { return vec3f(0.5, 0.0, 0.5); }
    return vec3f(0.5, 0.5, 0.0);
}
fn samp(w: u32, comp: u32, p: vec3f) -> f32 {
    let q = p - face_offset(comp);
    let b = floor(q);
    let f = q - b;
    let i = i32(b.x); let j = i32(b.y); let k = i32(b.z);
    let c00 = mix(face(w, comp, i, j, k), face(w, comp, i + 1, j, k), f.x);
    let c10 = mix(face(w, comp, i, j + 1, k), face(w, comp, i + 1, j + 1, k), f.x);
    let c01 = mix(face(w, comp, i, j, k + 1), face(w, comp, i + 1, j, k + 1), f.x);
    let c11 = mix(face(w, comp, i, j + 1, k + 1), face(w, comp, i + 1, j + 1, k + 1), f.x);
    return mix(mix(c00, c10, f.y), mix(c01, c11, f.y), f.z);
}
// MacCormack limiter bounds: min/max of the 8 values interpolated at p.
fn bounds(comp: u32, p: vec3f) -> vec2f {
    let q = p - face_offset(comp);
    let b = floor(q);
    let i = i32(b.x); let j = i32(b.y); let k = i32(b.z);
    var lo = 1e30;
    var hi = -1e30;
    for (var c = 0; c < 8; c++) {
        let x = face(0u, comp, i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1));
        lo = min(lo, x);
        hi = max(hi, x);
    }
    return vec2f(lo, hi);
}
fn vel3_exact(p: vec3f) -> vec3f { return vec3f(samp(0u, 0u, p), samp(0u, 1u, p), samp(0u, 2u, p)); }
fn vel3(p: vec3f) -> vec3f {
    if ((g._q & 2u) != 0u) { return vel3_exact(p); }
    return textureSampleLevel(vtex, vsamp, p / vec3f(f32(g.nx), f32(g.ny), f32(g.nz)), 0.0).xyz;
}
fn trace(p: vec3f, s: f32) -> vec3f {
    let hh = st.dt * s;
    let mid = p + 0.5 * hh * vel3(p);
    return p + hh * vel3(mid);
}
fn trace_s(p: vec3f, s: f32) -> vec3f {
    let hh = st.dt * s * f32(g.ds);
    let mid = p + 0.5 * hh * vel3(p * g.inv_ds);
    return p + hh * vel3(mid * g.inv_ds);
}

fn s_at(w: u32, i: i32, j: i32, k: i32) -> vec2f {
    if (i < 0 || j < 0 || k < 0 || i >= i32(g.sx) || j >= i32(g.sy) || k >= i32(g.sz)) { return vec2f(0.0); }
    let n = (u32(k) * g.sy + u32(j)) * g.sx + u32(i);
    if (w == 1u) { return unpk(sc_hat[n]); }
    return unpk(sc[n]);
}
fn samp_s(w: u32, p: vec3f) -> vec2f {
    let q = p - 0.5;
    let b = floor(q);
    let f = q - b;
    let i = i32(b.x); let j = i32(b.y); let k = i32(b.z);
    let c00 = mix(s_at(w, i, j, k), s_at(w, i + 1, j, k), f.x);
    let c10 = mix(s_at(w, i, j + 1, k), s_at(w, i + 1, j + 1, k), f.x);
    let c01 = mix(s_at(w, i, j, k + 1), s_at(w, i + 1, j, k + 1), f.x);
    let c11 = mix(s_at(w, i, j + 1, k + 1), s_at(w, i + 1, j + 1, k + 1), f.x);
    return mix(mix(c00, c10, f.y), mix(c01, c11, f.y), f.z);
}
fn limit_s(p: vec3f, x: vec2f) -> vec2f {
    let q = p - 0.5;
    let b = floor(q);
    let i = i32(b.x); let j = i32(b.y); let k = i32(b.z);
    var lo = vec2f(1e30);
    var hi = vec2f(-1e30);
    for (var c = 0; c < 8; c++) {
        let s = s_at(0u, i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1));
        lo = min(lo, s);
        hi = max(hi, s);
    }
    return clamp(x, lo, hi);
}

fn solid_cell(i: i32, j: i32, k: i32) -> bool {
    if (i < 0 || j < 0 || k < 0 || i >= i32(g.nx) || j >= i32(g.ny) || k >= i32(g.nz)) { return false; }
    return solid_v[(u32(k) * g.ny + u32(j)) * g.nx + u32(i)] != 0u;
}
fn face_solid(comp: u32, i: i32, j: i32, k: i32) -> bool {
    if (comp == 0u) { return solid_cell(i - 1, j, k) || solid_cell(i, j, k); }
    if (comp == 1u) { return solid_cell(i, j - 1, k) || solid_cell(i, j, k); }
    return solid_cell(i, j, k - 1) || solid_cell(i, j, k);
}
fn face_in(comp: u32, id: vec3u) -> bool {
    if (comp == 0u) { return id.x <= g.nx && id.y < g.ny && id.z < g.nz; }
    if (comp == 1u) { return id.x < g.nx && id.y <= g.ny && id.z < g.nz; }
    return id.x < g.nx && id.y < g.ny && id.z <= g.nz;
}
fn face_index(comp: u32, i: i32, j: i32, k: i32) -> u32 {
    if (comp == 0u) { return ui(i, j, k); }
    if (comp == 1u) { return vi(i, j, k); }
    return wi(i, j, k);
}
fn screen_v(p: vec3f) -> vec3f { return vec3f(g.x0 + p.x * g.h, g.y0 + p.y * g.h, g.z0 + p.z * g.h); }

// ---------------------------------------------------------------- setup
// The frame and ornaments stand from the plate's back, PLATE behind the
// picture plane, to their own tops (the bevels' and the relief's heights).
const PLATE: f32 = 0.045;
fn solid_at(column: vec2u, z: f32) -> bool {
    let o = textureLoad(obstacle, vec2i(column), 0);
    return o.g > 0.5 && z >= -PLATE && z <= o.r;
}
@compute @workgroup_size(8, 8, 4)
fn init_masks(@builtin(global_invocation_id) id: vec3u) {
    if (id.x < g.nx && id.y < g.ny && id.z < g.nz) {
        let p = screen_v(vec3f(id) + 0.5);
        let c = id.xy * g.ds + vec2u(g.ds / 2u);
        solid_v[(id.z * g.ny + id.y) * g.nx + id.x] = select(0u, 1u, solid_at(c, p.z) && g._p == 0u);
    }
    if (id.x < g.sx && id.y < g.sy && id.z < g.sz) {
        let p = screen_v((vec3f(id) + 0.5) * g.inv_ds);
        solid_s[(id.z * g.sy + id.y) * g.sx + id.x] = select(0u, 1u, solid_at(id.xy, p.z) && g._p == 0u);
    }
}

// The mountain's outline over each scalar column, by field_common.wgsl's
// terrain_march's camera and projection through the relief.
@compute @workgroup_size(64, 1, 1)
fn outline_columns(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx) { return; }
    let x = g.x0 + (f32(id.x) + 0.5) * g.h * g.inv_ds;
    outline[id.x] = smoke_outline(x);
}

// ---------------------------------------------------------------- advection
@compute @workgroup_size(8, 8, 4)
fn cell_vel(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny || id.z >= g.nz) { return; }
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    let c = 0.5 * vec3f(vel[ui(i, j, k)] + vel[ui(i + 1, j, k)],
                        vel[vi(i, j, k)] + vel[vi(i, j + 1, k)],
                        vel[wi(i, j, k)] + vel[wi(i, j, k + 1)]);
    textureStore(vtex_w, id, vec4f(c, 0.0));
}

// Face kernels run once per component (entry points *_u, *_v, *_w), so the
// component is a compile-time constant after inlining.
fn advect_vel_1_c(id: vec3u, comp: u32) {
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    if (!face_in(comp, id)) { return; }
    var r = 0.0;
    if (!face_solid(comp, i, j, k)) {
        r = samp(0u, comp, trace(vec3f(id) + face_offset(comp), -1.0));
    }
    vel_hat[face_index(comp, i, j, k)] = r;
}
@compute @workgroup_size(8, 8, 4) fn advect_vel_1_u(@builtin(global_invocation_id) id: vec3u) { advect_vel_1_c(id, 0u); }
@compute @workgroup_size(8, 8, 4) fn advect_vel_1_v(@builtin(global_invocation_id) id: vec3u) { advect_vel_1_c(id, 1u); }
@compute @workgroup_size(8, 8, 4) fn advect_vel_1_w(@builtin(global_invocation_id) id: vec3u) { advect_vel_1_c(id, 2u); }

fn advect_vel_2_c(id: vec3u, comp: u32) {
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    if (!face_in(comp, id)) { return; }
    let n = face_index(comp, i, j, k);
    var r = 0.0;
    if (!face_solid(comp, i, j, k)) {
        let p = vec3f(id) + face_offset(comp);
        // MacCormack with Selle et al.'s revert limiter: where the corrected
        // value leaves the bounds of the values it interpolated, fall back to
        // the first-order value (clamping instead gained energy in 3D and
        // blew up within ~15 s).
        r = vel_hat[n];
        if ((g._q & 1u) == 0u) {
            let c = r + 0.5 * (vel[n] - samp(1u, comp, trace(p, 1.0)));
            let b = bounds(comp, trace(p, -1.0));
            if (c >= b.x && c <= b.y) { r = c; }
        }
    }
    vel_tmp[n] = r;
}
@compute @workgroup_size(8, 8, 4) fn advect_vel_2_u(@builtin(global_invocation_id) id: vec3u) { advect_vel_2_c(id, 0u); }
@compute @workgroup_size(8, 8, 4) fn advect_vel_2_v(@builtin(global_invocation_id) id: vec3u) { advect_vel_2_c(id, 1u); }
@compute @workgroup_size(8, 8, 4) fn advect_vel_2_w(@builtin(global_invocation_id) id: vec3u) { advect_vel_2_c(id, 2u); }

@compute @workgroup_size(8, 8, 4)
fn advect_s_1(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy || id.z >= g.sz) { return; }
    let n = (id.z * g.sy + id.y) * g.sx + id.x;
    var r = vec2f(0.0);
    if (solid_s[n] == 0u) { r = samp_s(0u, trace_s(vec3f(id) + 0.5, -1.0)); }
    sc_hat[n] = pk(r);
}

@compute @workgroup_size(8, 8, 4)
fn advect_s_2(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy || id.z >= g.sz) { return; }
    let n = (id.z * g.sy + id.y) * g.sx + id.x;
    var r = vec2f(0.0);
    if (solid_s[n] == 0u) {
        let p = vec3f(id) + 0.5;
        r = unpk(sc_hat[n]) + 0.5 * (unpk(sc[n]) - samp_s(1u, trace_s(p, 1.0)));
        r = limit_s(trace_s(p, -1.0), r);
    }
    sc_tmp[n] = pk(r);
}

// ---------------------------------------------------------------- sources
// How much nearer the picture plane (screen z) the lava's heat lies where
// the mountain's outline is beside the frame rather than behind it, so that
// plumes rise beside the frame (authored).
const HEAT_BESIDE: f32 = 0.07;
fn emitter_weight(e: u32, p: vec3f) -> f32 {
    let a = st.emit[3u * e];
    let d = p - a.xyz;
    let q = dot(d, d) / (a.w * a.w);
    if (q > 12.0) { return 0.0; }   // exp(-12) ~ 6e-6: cull (saves the exp)
    return exp(-q);
}

@compute @workgroup_size(8, 8, 4)
fn sources(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy || id.z >= g.sz) { return; }
    let n = (id.z * g.sy + id.y) * g.sx + id.x;
    if (solid_s[n] != 0u) { sc[n] = pk(vec2f(0.0)); return; }
    let p = screen_v((vec3f(id) + 0.5) * g.inv_ds);
    var r = unpk(sc_tmp[n]);
    for (var e = 0u; e < st.n_emit; e++) {
        let w = emitter_weight(e, p);
        r += st.dt * w * st.emit[3u * e + 1u].xy;
    }
    // The lava's heat along the mountain's outline: in a band below it, at
    // depth heat_z, flickering along it so that plumes rise apart.
    if (st.heat.x > 0.0) {
        let top = outline[id.x];
        let d = top - p.y;
        if (top > -0.5 && d > 0.0 && d < st.heat.z) {
            // Nearer the picture plane beside the frame.
            let beside = smoothstep(st.heat_z.w - 0.02, st.heat_z.w + 0.03, abs(p.x - 0.8888889));
            let dz = (p.z - st.heat_z.x - beside * HEAT_BESIDE) / st.heat_z.y;
            if (dz * dz < 9.0) {
                let w = smoothstep(0.0, 0.25 * st.heat.z, d) * exp(-dz * dz)
                    * smoke_flicker(p.x / st.heat_z.z, st.heat.w);
                r += st.dt * w * st.heat.yx;
            }
        }
    }
    sc[n] = pk(max(r * vec2f(st.keep_rho, st.keep_t), vec2f(0.0)));
}

// ---------------------------------------------------------------- forces
// Cell-centred velocity from the advected faces (vel_tmp).
fn cvel(i: i32, j: i32, k: i32) -> vec3f {
    return 0.5 * vec3f(face(2u, 0u, i, j, k) + face(2u, 0u, i + 1, j, k),
                       face(2u, 1u, i, j, k) + face(2u, 1u, i, j + 1, k),
                       face(2u, 2u, i, j, k) + face(2u, 2u, i, j, k + 1));
}

@compute @workgroup_size(8, 8, 4)
fn vorticity(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny || id.z >= g.nz) { return; }
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    let n = (id.z * g.ny + id.y) * g.nx + id.x;
    if (solid_v[n] != 0u) { curl[n] = vec4f(0.0); return; }
    let dx = 0.5 * (cvel(i + 1, j, k) - cvel(i - 1, j, k));
    let dy = 0.5 * (cvel(i, j + 1, k) - cvel(i, j - 1, k));
    let dz = 0.5 * (cvel(i, j, k + 1) - cvel(i, j, k - 1));
    let w = vec3f(dy.z - dz.y, dz.x - dx.z, dx.y - dy.x);
    curl[n] = vec4f(w, length(w));
}

fn curl_at(i: i32, j: i32, k: i32) -> vec4f {
    let a = u32(clamp(i, 0, i32(g.nx) - 1));
    let b = u32(clamp(j, 0, i32(g.ny) - 1));
    let c = u32(clamp(k, 0, i32(g.nz) - 1));
    return curl[(c * g.ny + b) * g.nx + a];
}

// Confinement acceleration eps (N x omega) at a cell centre.
fn confine_at(i: i32, j: i32, k: i32) -> vec3f {
    let w = curl_at(i, j, k);
    let gr = 0.5 * vec3f(curl_at(i + 1, j, k).w - curl_at(i - 1, j, k).w,
                         curl_at(i, j + 1, k).w - curl_at(i, j - 1, k).w,
                         curl_at(i, j, k + 1).w - curl_at(i, j, k - 1).w);
    let nrm = gr / (length(gr) + 1e-5);
    return st.vort * cross(nrm, w.xyz);
}

// Once per cell, into conf (xyz); forces averages it onto the faces.
@compute @workgroup_size(8, 8, 4)
fn confinement(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny || id.z >= g.nz) { return; }
    let n = (id.z * g.ny + id.y) * g.nx + id.x;
    conf[n] = vec4f(confine_at(i32(id.x), i32(id.y), i32(id.z)), 0.0);
}

fn confine(i: i32, j: i32, k: i32) -> vec3f {
    let a = u32(clamp(i, 0, i32(g.nx) - 1));
    let b = u32(clamp(j, 0, i32(g.ny) - 1));
    let c = u32(clamp(k, 0, i32(g.nz) - 1));
    return conf[(c * g.ny + b) * g.nx + a].xyz;
}

fn cell_scalars(i: i32, j: i32, k: i32) -> vec2f {
    let d = i32(g.ds);
    var acc = vec2f(0.0);
    for (var c = 0; c < d; c++) {
        for (var b = 0; b < d; b++) {
            for (var a = 0; a < d; a++) {
                acc += s_at(0u, i * d + a, j * d + b, k * d + c);
            }
        }
    }
    return acc / f32(d * d * d);
}

fn neighbour(comp: u32) -> vec3i {
    if (comp == 0u) { return vec3i(1, 0, 0); }
    if (comp == 1u) { return vec3i(0, 1, 0); }
    return vec3i(0, 0, 1);
}

fn forces_c(id: vec3u, comp: u32) {
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    if (!face_in(comp, id)) { return; }
    let n = face_index(comp, i, j, k);
    var r = 0.0;
    if (!face_solid(comp, i, j, k)) {
        let o = neighbour(comp);
        let fc = 0.5 * (confine(i - o.x, j - o.y, k - o.z) + confine(i, j, k));
        var f = fc[comp];
        // authored air movement: a breeze (wind.xy) everywhere, and a draft
        // toward the back (wind.z) in front of the picture plane only, fading
        // in over wind.w screen units (keeps the frame readable)
        let p = screen_v(vec3f(id) + face_offset(comp));
        var wk = 1.0;
        if (comp == 2u) { wk = smoothstep(-0.02, st.wind.w, p.z); }
        f += st.wind[comp] * wk;
        if (comp == 1u) {
            let s = 0.5 * (cell_scalars(i, j - 1, k) + cell_scalars(i, j, k));
            f += st.buoy_t * s.y - st.buoy_rho * s.x;
        }
        r = vel_tmp[n] + st.dt * f;
        for (var e = 0u; e < st.n_emit; e++) {
            let w = emitter_weight(e, p);
            if (w == 0.0) { continue; }
            let tv = st.emit[3u * e + 2u][comp];
            r = mix(r, tv, 1.0 - exp(-st.emit[3u * e + 1u].z * w * st.dt));
        }
        r *= st.keep_vel * sponge(comp, vec3f(id) + face_offset(comp));
    }
    vel[n] = r;
}
@compute @workgroup_size(8, 8, 4) fn forces_u(@builtin(global_invocation_id) id: vec3u) { forces_c(id, 0u); }
@compute @workgroup_size(8, 8, 4) fn forces_v(@builtin(global_invocation_id) id: vec3u) { forces_c(id, 1u); }
@compute @workgroup_size(8, 8, 4) fn forces_w(@builtin(global_invocation_id) id: vec3u) { forces_c(id, 2u); }

// Far-field sponge: the volume is a window onto still air. Within SPONGE cells
// of the sides, bottom, front and back, velocity relaxes toward 0; at the top
// only the tangential components, so smoke can leave. Without it the all-open
// boundaries fed back on themselves and speeds grew without bound.
const SPONGE: f32 = 8.0;
fn ramp(d: f32) -> f32 { let a = clamp(1.0 - d / SPONGE, 0.0, 1.0); return a * a; }
fn sponge(comp: u32, p: vec3f) -> f32 {
    let dims = vec3f(f32(g.nx), f32(g.ny), f32(g.nz));
    var w = max(max(ramp(p.x), ramp(dims.x - p.x)), max(ramp(p.y), max(ramp(p.z), ramp(dims.z - p.z))));
    if (comp != 1u) { w = max(w, ramp(dims.y - p.y)); }
    return exp(-g._r * w * st.dt);
}

// ---------------------------------------------------------------- projection
@compute @workgroup_size(8, 8, 4)
fn divergence(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny || id.z >= g.nz) { return; }
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    let n = (id.z * g.ny + id.y) * g.nx + id.x;
    if (solid_v[n] != 0u) { divg[n] = 0.0; return; }
    divg[n] = vel[ui(i + 1, j, k)] - vel[ui(i, j, k)]
            + vel[vi(i, j + 1, k)] - vel[vi(i, j, k)]
            + vel[wi(i, j, k + 1)] - vel[wi(i, j, k)];
}

fn p_at(i: i32, j: i32, k: i32) -> f32 {
    if (i < 0 || j < 0 || k < 0 || i >= i32(g.nx) || j >= i32(g.ny) || k >= i32(g.nz)) { return 0.0; }
    return pr[(u32(k) * g.ny + u32(j)) * g.nx + u32(i)];
}

fn project_c(id: vec3u, comp: u32) {
    let i = i32(id.x); let j = i32(id.y); let k = i32(id.z);
    if (!face_in(comp, id)) { return; }
    let n = face_index(comp, i, j, k);
    if (face_solid(comp, i, j, k)) {
        vel[n] = 0.0;
    } else {
        let o = neighbour(comp);
        vel[n] -= p_at(i, j, k) - p_at(i - o.x, j - o.y, k - o.z);
    }
}
@compute @workgroup_size(8, 8, 4) fn project_u(@builtin(global_invocation_id) id: vec3u) { project_c(id, 0u); }
@compute @workgroup_size(8, 8, 4) fn project_v(@builtin(global_invocation_id) id: vec3u) { project_c(id, 1u); }
@compute @workgroup_size(8, 8, 4) fn project_w(@builtin(global_invocation_id) id: vec3u) { project_c(id, 2u); }
