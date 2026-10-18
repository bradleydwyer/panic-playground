// Part 5's smoke, 2D: the fallback solver (with field_frame_smoke.rs and
// light2d.wgsl), lifted from a smoke prototype kept locally and adapted as
// the 3D solver is: the obstacles are the frame and ornaments baked from
// the user's PSTART picture, and the lava's heat follows the real
// mountain's outline. Authored throughout; nothing here is from the
// original demo.
//
// Stable fluids (Stam 1999) on a MAC grid (Bridson), with buoyancy and
// vorticity confinement (Fedkiw, Stam and Jensen 2001), over the whole
// 16:9 screen in screen units (1 = its height, y up from its bottom).
// Velocity grid nx x ny cells, cell (i, j) centred at (i + 0.5, j + 0.5) in
// velocity-cell units, j = 0 at the bottom (gravity -y).
//   u: (nx + 1) x ny, x-velocity on vertical faces, at (i, j + 0.5)
//   v: nx x (ny + 1), y-velocity on horizontal faces, at (i + 0.5, j)
// Velocities are in velocity cells per second. Pressure is cell-centred and
// stores dt * p (so projection is u -= grad p, no constants).
// Scalars (density, temperature) live on a grid ds times finer, sampled
// through the same velocity field.
//
// Boundaries: the screen edge is open (Dirichlet p = 0 outside, scalars 0
// outside: clean air comes in, smoke leaves). Obstacles (the frame and
// ornaments) are solid cells: face velocity 0, Neumann pressure.
//
// Determinism: every kernel is a pure gather (no atomics, no reductions, no
// races); each writes every element it owns, so scratch buffers need no
// clearing and the persistent state is exactly u, v, sc, pr.

struct Grid {
    nx: u32, ny: u32, sx: u32, sy: u32,
    ds: f32, inv_ds: f32, omega: f32, _pad: f32,
}

// One block per simulation step, chosen with a dynamic offset, from
// field_frame_smoke.rs's story; heat, camera and view as in solver3d.wgsl
// (of heat_z, only z, the flicker's width).
struct Step {
    dt: f32, buoy_t: f32, buoy_rho: f32, vort: f32,
    keep_rho: f32, keep_t: f32, keep_vel: f32, n_emit: u32,
    wind: vec4f,
    heat: vec4f,
    heat_z: vec4f,
    camera: vec4f,
    view: vec4f,
    // per emitter: (x, y, radius, density rate) in screen units, then
    // (temperature rate, target vx, target vy [velocity cells/s], pull 1/s)
    emit: array<vec4f, 32>,
}

@group(0) @binding(0) var<uniform> g: Grid;
@group(0) @binding(1) var<uniform> st: Step;
@group(0) @binding(2) var<storage, read_write> u: array<f32>;
@group(0) @binding(3) var<storage, read_write> v: array<f32>;
@group(0) @binding(4) var<storage, read_write> u_hat: array<f32>;
@group(0) @binding(5) var<storage, read_write> v_hat: array<f32>;
@group(0) @binding(6) var<storage, read_write> u_tmp: array<f32>;
@group(0) @binding(7) var<storage, read_write> v_tmp: array<f32>;
@group(0) @binding(8) var<storage, read_write> sc: array<vec2f>;      // (density, temperature)
@group(0) @binding(9) var<storage, read_write> sc_hat: array<vec2f>;
@group(0) @binding(10) var<storage, read_write> sc_tmp: array<vec2f>;
@group(0) @binding(11) var<storage, read_write> divg: array<f32>;
@group(0) @binding(12) var<storage, read_write> pr: array<f32>;
@group(0) @binding(13) var<storage, read_write> curl: array<f32>;
@group(0) @binding(14) var<storage, read_write> solid_v: array<u32>;
@group(0) @binding(15) var<storage, read_write> solid_s: array<u32>;
// The obstacles over the scalar grid (g: 1 where solid), the fitted relief,
// a linear sampler for it, and the mountain's outline per scalar column, as
// in solver3d.wgsl.
@group(0) @binding(16) var obstacle: texture_2d<f32>;
@group(0) @binding(17) var relief_tex: texture_2d<f32>;
@group(0) @binding(18) var vsamp: sampler;
@group(0) @binding(19) var<storage, read_write> outline: array<f32>;

// ---------------------------------------------------------------- sampling
// Index-clamped loads: velocity outside the domain is extrapolated (zero
// gradient), which suits open boundaries. w selects the buffer.
fn u_at(w: u32, i: i32, j: i32) -> f32 {
    let ii = u32(clamp(i, 0, i32(g.nx)));
    let jj = u32(clamp(j, 0, i32(g.ny) - 1));
    let k = jj * (g.nx + 1u) + ii;
    if (w == 1u) { return u_hat[k]; }
    if (w == 2u) { return u_tmp[k]; }
    return u[k];
}

fn v_at(w: u32, i: i32, j: i32) -> f32 {
    let ii = u32(clamp(i, 0, i32(g.nx) - 1));
    let jj = u32(clamp(j, 0, i32(g.ny)));
    let k = jj * g.nx + ii;
    if (w == 1u) { return v_hat[k]; }
    if (w == 2u) { return v_tmp[k]; }
    return v[k];
}

fn samp_u(w: u32, p: vec2f) -> f32 {
    let q = vec2f(p.x, p.y - 0.5);
    let b = floor(q);
    let f = q - b;
    let i = i32(b.x);
    let j = i32(b.y);
    return mix(mix(u_at(w, i, j), u_at(w, i + 1, j), f.x),
               mix(u_at(w, i, j + 1), u_at(w, i + 1, j + 1), f.x), f.y);
}

fn samp_v(w: u32, p: vec2f) -> f32 {
    let q = vec2f(p.x - 0.5, p.y);
    let b = floor(q);
    let f = q - b;
    let i = i32(b.x);
    let j = i32(b.y);
    return mix(mix(v_at(w, i, j), v_at(w, i + 1, j), f.x),
               mix(v_at(w, i, j + 1), v_at(w, i + 1, j + 1), f.x), f.y);
}

fn vel(p: vec2f) -> vec2f { return vec2f(samp_u(0u, p), samp_v(0u, p)); }

// RK2 (midpoint) characteristic through the velocity at the start of the
// step; s = -1 traces back, +1 forward. Velocity-cell units.
fn trace(p: vec2f, s: f32) -> vec2f {
    let h = st.dt * s;
    let mid = p + 0.5 * h * vel(p);
    return p + h * vel(mid);
}

// The same in scalar-cell units.
fn trace_s(p: vec2f, s: f32) -> vec2f {
    let h = st.dt * s * g.ds;
    let mid = p + 0.5 * h * vel(p * g.inv_ds);
    return p + h * vel(mid * g.inv_ds);
}

// Scalars outside the domain are clean air.
fn s_at(w: u32, i: i32, j: i32) -> vec2f {
    if (i < 0 || j < 0 || i >= i32(g.sx) || j >= i32(g.sy)) { return vec2f(0.0); }
    let k = u32(j) * g.sx + u32(i);
    if (w == 1u) { return sc_hat[k]; }
    return sc[k];
}

fn samp_s(w: u32, p: vec2f) -> vec2f {
    let q = p - 0.5;
    let b = floor(q);
    let f = q - b;
    let i = i32(b.x);
    let j = i32(b.y);
    return mix(mix(s_at(w, i, j), s_at(w, i + 1, j), f.x),
               mix(s_at(w, i, j + 1), s_at(w, i + 1, j + 1), f.x), f.y);
}

fn solid_cell(i: i32, j: i32) -> bool {
    if (i < 0 || j < 0 || i >= i32(g.nx) || j >= i32(g.ny)) { return false; }
    return solid_v[u32(j) * g.nx + u32(i)] != 0u;
}
fn u_face_solid(i: i32, j: i32) -> bool { return solid_cell(i - 1, j) || solid_cell(i, j); }
fn v_face_solid(i: i32, j: i32) -> bool { return solid_cell(i, j - 1) || solid_cell(i, j); }

// MacCormack limiter: clamp to the four values the backtrace interpolated.
fn limit_u(p: vec2f, x: f32) -> f32 {
    let q = vec2f(p.x, p.y - 0.5);
    let b = floor(q);
    let i = i32(b.x);
    let j = i32(b.y);
    let a = u_at(0u, i, j); let c = u_at(0u, i + 1, j);
    let d = u_at(0u, i, j + 1); let e = u_at(0u, i + 1, j + 1);
    return clamp(x, min(min(a, c), min(d, e)), max(max(a, c), max(d, e)));
}
fn limit_v(p: vec2f, x: f32) -> f32 {
    let q = vec2f(p.x - 0.5, p.y);
    let b = floor(q);
    let i = i32(b.x);
    let j = i32(b.y);
    let a = v_at(0u, i, j); let c = v_at(0u, i + 1, j);
    let d = v_at(0u, i, j + 1); let e = v_at(0u, i + 1, j + 1);
    return clamp(x, min(min(a, c), min(d, e)), max(max(a, c), max(d, e)));
}
fn limit_s(p: vec2f, x: vec2f) -> vec2f {
    let q = p - 0.5;
    let b = floor(q);
    let i = i32(b.x);
    let j = i32(b.y);
    let a = s_at(0u, i, j); let c = s_at(0u, i + 1, j);
    let d = s_at(0u, i, j + 1); let e = s_at(0u, i + 1, j + 1);
    return clamp(x, min(min(a, c), min(d, e)), max(max(a, c), max(d, e)));
}

// ---------------------------------------------------------------- setup
@compute @workgroup_size(16, 16)
fn init_masks(@builtin(global_invocation_id) id: vec3u) {
    let ds = u32(g.ds);
    if (id.x < g.nx && id.y < g.ny) {
        let c = vec2i(id.xy * ds + vec2u(ds / 2u));
        solid_v[id.y * g.nx + id.x] = select(0u, 1u, textureLoad(obstacle, c, 0).g > 0.5);
    }
    if (id.x < g.sx && id.y < g.sy) {
        solid_s[id.y * g.sx + id.x] = select(0u, 1u, textureLoad(obstacle, vec2i(id.xy), 0).g > 0.5);
    }
}

// The mountain's outline over each scalar column.
@compute @workgroup_size(64, 1, 1)
fn outline_columns(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx) { return; }
    outline[id.x] = smoke_outline((f32(id.x) + 0.5) / f32(g.sy));
}

// ---------------------------------------------------------------- advection
// Pass 1: semi-Lagrangian prediction  u_hat = u(trace(x, -dt)).
@compute @workgroup_size(16, 16)
fn advect_vel_1(@builtin(global_invocation_id) id: vec3u) {
    let i = i32(id.x);
    let j = i32(id.y);
    if (id.x <= g.nx && id.y < g.ny) {
        var r = 0.0;
        if (!u_face_solid(i, j)) { r = samp_u(0u, trace(vec2f(f32(i), f32(j) + 0.5), -1.0)); }
        u_hat[id.y * (g.nx + 1u) + id.x] = r;
    }
    if (id.x < g.nx && id.y <= g.ny) {
        var r = 0.0;
        if (!v_face_solid(i, j)) { r = samp_v(0u, trace(vec2f(f32(i) + 0.5, f32(j)), -1.0)); }
        v_hat[id.y * g.nx + id.x] = r;
    }
}

// Pass 2: MacCormack correction  u_hat + (u - u_hat(trace(x, +dt))) / 2,
// clamped (Selle et al. 2008) to keep it unconditionally stable.
@compute @workgroup_size(16, 16)
fn advect_vel_2(@builtin(global_invocation_id) id: vec3u) {
    let i = i32(id.x);
    let j = i32(id.y);
    if (id.x <= g.nx && id.y < g.ny) {
        let k = id.y * (g.nx + 1u) + id.x;
        var r = 0.0;
        if (!u_face_solid(i, j)) {
            let p = vec2f(f32(i), f32(j) + 0.5);
            r = u_hat[k] + 0.5 * (u[k] - samp_u(1u, trace(p, 1.0)));
            r = limit_u(trace(p, -1.0), r);
        }
        u_tmp[k] = r;
    }
    if (id.x < g.nx && id.y <= g.ny) {
        let k = id.y * g.nx + id.x;
        var r = 0.0;
        if (!v_face_solid(i, j)) {
            let p = vec2f(f32(i) + 0.5, f32(j));
            r = v_hat[k] + 0.5 * (v[k] - samp_v(1u, trace(p, 1.0)));
            r = limit_v(trace(p, -1.0), r);
        }
        v_tmp[k] = r;
    }
}

@compute @workgroup_size(16, 16)
fn advect_s_1(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy) { return; }
    let k = id.y * g.sx + id.x;
    var r = vec2f(0.0);
    if (solid_s[k] == 0u) { r = samp_s(0u, trace_s(vec2f(id.xy) + 0.5, -1.0)); }
    sc_hat[k] = r;
}

@compute @workgroup_size(16, 16)
fn advect_s_2(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy) { return; }
    let k = id.y * g.sx + id.x;
    var r = vec2f(0.0);
    if (solid_s[k] == 0u) {
        let p = vec2f(id.xy) + 0.5;
        r = sc_hat[k] + 0.5 * (sc[k] - samp_s(1u, trace_s(p, 1.0)));
        r = limit_s(trace_s(p, -1.0), r);
    }
    sc_tmp[k] = r;
}

// ---------------------------------------------------------------- sources
fn emitter_weight(e: u32, p: vec2f) -> f32 {
    let a = st.emit[2u * e];
    let d = p - a.xy;
    return exp(-dot(d, d) / (a.z * a.z));
}

// Emission and dissipation of density and temperature: sc_tmp -> sc.
@compute @workgroup_size(16, 16)
fn sources(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.sx || id.y >= g.sy) { return; }
    let k = id.y * g.sx + id.x;
    if (solid_s[k] != 0u) { sc[k] = vec2f(0.0); return; }
    let p = (vec2f(id.xy) + 0.5) / f32(g.sy);
    var r = sc_tmp[k];
    for (var e = 0u; e < st.n_emit; e++) {
        let w = emitter_weight(e, p);
        r += st.dt * w * vec2f(st.emit[2u * e].w, st.emit[2u * e + 1u].x);
    }
    // The lava's heat in a band below the mountain's outline, flickering.
    if (st.heat.x > 0.0) {
        let top = outline[id.x];
        let d = top - p.y;
        if (top > -0.5 && d > 0.0 && d < st.heat.z) {
            let w = smoothstep(0.0, 0.25 * st.heat.z, d) * smoke_flicker(p.x / st.heat_z.z, st.heat.w);
            r += st.dt * w * st.heat.yx;
        }
    }
    sc[k] = max(r * vec2f(st.keep_rho, st.keep_t), vec2f(0.0));
}

// ---------------------------------------------------------------- forces
@compute @workgroup_size(16, 16)
fn vorticity(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny) { return; }
    let i = i32(id.x);
    let j = i32(id.y);
    let k = id.y * g.nx + id.x;
    if (solid_v[k] != 0u) { curl[k] = 0.0; return; }
    let vr = 0.5 * (v_at(2u, i + 1, j) + v_at(2u, i + 1, j + 1));
    let vl = 0.5 * (v_at(2u, i - 1, j) + v_at(2u, i - 1, j + 1));
    let ut = 0.5 * (u_at(2u, i, j + 1) + u_at(2u, i + 1, j + 1));
    let ub = 0.5 * (u_at(2u, i, j - 1) + u_at(2u, i + 1, j - 1));
    curl[k] = 0.5 * (vr - vl) - 0.5 * (ut - ub);
}

fn curl_at(i: i32, j: i32) -> f32 {
    let ii = u32(clamp(i, 0, i32(g.nx) - 1));
    let jj = u32(clamp(j, 0, i32(g.ny) - 1));
    return curl[jj * g.nx + ii];
}

// Confinement acceleration at a cell centre: eps (N x omega), N the unit
// gradient of |omega| (eps already scaled per resolution on the host).
fn confine(i: i32, j: i32) -> vec2f {
    let w = curl_at(i, j);
    let gx = 0.5 * (abs(curl_at(i + 1, j)) - abs(curl_at(i - 1, j)));
    let gy = 0.5 * (abs(curl_at(i, j + 1)) - abs(curl_at(i, j - 1)));
    let n = vec2f(gx, gy) / (length(vec2f(gx, gy)) + 1e-5);
    return st.vort * vec2f(n.y * w, -n.x * w);
}

// Mean (density, temperature) over the scalar cells of velocity cell (i, j).
fn cell_scalars(i: i32, j: i32) -> vec2f {
    let n = i32(g.ds);
    var acc = vec2f(0.0);
    for (var b = 0; b < n; b++) {
        for (var a = 0; a < n; a++) {
            acc += s_at(0u, i * n + a, j * n + b);
        }
    }
    return acc / f32(n * n);
}

fn pull(c: f32, face: vec2f, comp: u32) -> f32 {
    var x = c;
    let p = face / f32(g.ny);
    for (var e = 0u; e < st.n_emit; e++) {
        let b = st.emit[2u * e + 1u];
        let w = emitter_weight(e, p);
        let target_v = select(b.y, b.z, comp == 1u);
        x = mix(x, target_v, 1.0 - exp(-b.w * w * st.dt));
    }
    return x;
}

// Buoyancy, confinement, emitter pull and damping: u_tmp -> u.
@compute @workgroup_size(16, 16)
fn forces(@builtin(global_invocation_id) id: vec3u) {
    let i = i32(id.x);
    let j = i32(id.y);
    if (id.x <= g.nx && id.y < g.ny) {
        let k = id.y * (g.nx + 1u) + id.x;
        var r = 0.0;
        if (!u_face_solid(i, j)) {
            let f = 0.5 * (confine(i - 1, j).x + confine(i, j).x) + st.wind.x;
            r = u_tmp[k] + st.dt * f;
            r = pull(r, vec2f(f32(i), f32(j) + 0.5), 0u) * st.keep_vel;
        }
        u[k] = r;
    }
    if (id.x < g.nx && id.y <= g.ny) {
        let k = id.y * g.nx + id.x;
        var r = 0.0;
        if (!v_face_solid(i, j)) {
            let s = 0.5 * (cell_scalars(i, j - 1) + cell_scalars(i, j));
            let b = st.buoy_t * s.y - st.buoy_rho * s.x;
            let f = 0.5 * (confine(i, j - 1).y + confine(i, j).y) + st.wind.y + b;
            r = v_tmp[k] + st.dt * f;
            r = pull(r, vec2f(f32(i) + 0.5, f32(j)), 1u) * st.keep_vel;
        }
        v[k] = r;
    }
}

// ---------------------------------------------------------------- projection
@compute @workgroup_size(16, 16)
fn divergence(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= g.nx || id.y >= g.ny) { return; }
    let k = id.y * g.nx + id.x;
    if (solid_v[k] != 0u) { divg[k] = 0.0; return; }
    let ku = id.y * (g.nx + 1u) + id.x;
    divg[k] = u[ku + 1u] - u[ku] + v[k + g.nx] - v[k];
}

// Red-black SOR on  sum_n (p_n - p_c) = div  over non-solid neighbours;
// neighbours outside the domain are p = 0 (open). Warm-started from the
// previous step's pressure, which is therefore part of the state.
fn sor(id: vec3u, colour: u32) {
    let j = id.y;
    let i = 2u * id.x + ((j + colour) & 1u);
    if (i >= g.nx || j >= g.ny) { return; }
    let k = j * g.nx + i;
    if (solid_v[k] != 0u) { pr[k] = 0.0; return; }
    var sum = 0.0;
    var cnt = 0.0;
    let ii = i32(i);
    let jj = i32(j);
    let nb = array<vec2i, 4>(vec2i(ii - 1, jj), vec2i(ii + 1, jj), vec2i(ii, jj - 1), vec2i(ii, jj + 1));
    for (var n = 0; n < 4; n++) {
        let q = nb[n];
        if (q.x < 0 || q.y < 0 || q.x >= i32(g.nx) || q.y >= i32(g.ny)) {
            cnt += 1.0;
        } else {
            let kq = u32(q.y) * g.nx + u32(q.x);
            if (solid_v[kq] == 0u) { cnt += 1.0; sum += pr[kq]; }
        }
    }
    if (cnt == 0.0) { pr[k] = 0.0; return; }
    pr[k] = mix(pr[k], (sum - divg[k]) / cnt, g.omega);
}

@compute @workgroup_size(16, 16)
fn sor_red(@builtin(global_invocation_id) id: vec3u) { sor(id, 0u); }

@compute @workgroup_size(16, 16)
fn sor_black(@builtin(global_invocation_id) id: vec3u) { sor(id, 1u); }

fn p_at(i: i32, j: i32) -> f32 {
    if (i < 0 || j < 0 || i >= i32(g.nx) || j >= i32(g.ny)) { return 0.0; }
    return pr[u32(j) * g.nx + u32(i)];
}

@compute @workgroup_size(16, 16)
fn project(@builtin(global_invocation_id) id: vec3u) {
    let i = i32(id.x);
    let j = i32(id.y);
    if (id.x <= g.nx && id.y < g.ny) {
        let k = id.y * (g.nx + 1u) + id.x;
        if (u_face_solid(i, j)) { u[k] = 0.0; } else { u[k] -= p_at(i, j) - p_at(i - 1, j); }
    }
    if (id.x < g.nx && id.y <= g.ny) {
        let k = id.y * g.nx + id.x;
        if (v_face_solid(i, j)) { v[k] = 0.0; } else { v[k] -= p_at(i, j) - p_at(i, j - 1); }
    }
}
