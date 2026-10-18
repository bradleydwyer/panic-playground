// Part 5's smoke, 3D: the pressure solve, lifted unchanged from a smoke
// prototype kept locally; field_frame_smoke.rs dispatches it. Authored;
// nothing here is from the original demo.
//
// Geometric multigrid V-cycle for the 3D pressure Poisson equation
//   sum over neighbours n of w_n (x_n - x_c) = b_c
// (unit spacing; neighbours outside the domain are x = 0, open boundary).
// Level 0: w = 1 to fluid or outside neighbours, 0 to solid (Neumann).
// Coarse cells are 2x2x2 fine cells, solid only if every child is solid; each
// coarse face's conductance w is the mean of the open fine faces it covers
// (so thin obstacles such as the frame plate stay closed on coarse grids; a
// plain re-discretised mask let them leak and the V-cycle diverged).
// Open boundary on coarse levels: the level-0 problem puts p = 0 half a fine
// cell beyond the domain face; a coarse ghost of 0 a whole coarse cell out
// would move it, so coarse boundary faces get conductance 2 / (1 + 2^-l) and
// the prolongation's ghosts (1 - that) times the edge value, which keeps the
// zero at the same place on every level. Without this the V-cycle degraded
// with depth and diverged at 5-6 levels (checked in a numpy model, too).
// Smoother: red-black Gauss-Seidel. Restriction: b_c = (1/2) sum of the 8
// child residuals (the fine fluxes summed over the coarse cell, at twice the
// spacing). Prolongation: trilinear. Converges about 10x per V-cycle (2 + 2
// sweeps). Deterministic: no atomics.

// _a: 1 if this level reads stored conductances; _b: the coarse level's index
struct Lvl {
    nx: u32, ny: u32, nz: u32, _a: u32,
    cx: u32, cy: u32, cz: u32, _b: u32,
}

fn wb(level: u32) -> f32 { return 2.0 / (1.0 + exp2(-f32(level))); }

@group(0) @binding(0) var<uniform> L: Lvl;
@group(0) @binding(1) var<storage, read_write> x: array<f32>;
@group(0) @binding(2) var<storage, read_write> b: array<f32>;
@group(0) @binding(3) var<storage, read_write> r: array<f32>;
@group(0) @binding(4) var<storage, read_write> m: array<u32>;
@group(0) @binding(5) var<storage, read_write> xc: array<f32>;
@group(0) @binding(6) var<storage, read_write> bc: array<f32>;
@group(0) @binding(7) var<storage, read_write> mc: array<u32>;
// face conductances (-x, +x, -y, +y), (-z, +z, -, -): this level (unused on
// level 0, which derives them from the mask) and the coarse level
@group(0) @binding(8) var<storage, read_write> wt: array<vec4f>;
@group(0) @binding(9) var<storage, read_write> wtc: array<vec4f>;
// level 0 only: bit q set if face q of the cell is open (fluid or outside)
@group(0) @binding(10) var<storage, read_write> open0: array<u32>;

fn inside(i: i32, j: i32, k: i32) -> bool {
    return i >= 0 && j >= 0 && k >= 0 && i < i32(L.nx) && j < i32(L.ny) && k < i32(L.nz);
}
fn idx(i: i32, j: i32, k: i32) -> u32 { return (u32(k) * L.ny + u32(j)) * L.nx + u32(i); }

const NB = array<vec3i, 6>(vec3i(-1, 0, 0), vec3i(1, 0, 0), vec3i(0, -1, 0), vec3i(0, 1, 0), vec3i(0, 0, -1), vec3i(0, 0, 1));

// Level-0 conductance of face q of a fluid cell: open to fluid or outside.
fn w0(i: i32, j: i32, k: i32, q: i32) -> f32 {
    let a = vec3i(i, j, k) + NB[q];
    if (!inside(a.x, a.y, a.z)) { return 1.0; }
    return select(1.0, 0.0, m[idx(a.x, a.y, a.z)] != 0u);
}
fn wface(i: i32, j: i32, k: i32, q: i32) -> f32 {
    if (L._a == 0u) { return w0(i, j, k, q); }
    let n = idx(i, j, k);
    if (q < 4) { return wt[2u * n][q]; }
    return wt[2u * n + 1u][q - 4];
}

// (sum of w x over neighbours, sum of w). A solid neighbour's conductance is
// 0, so only open faces read x.
fn stencil(i: i32, j: i32, k: i32) -> vec2f {
    var s = 0.0;
    var c = 0.0;
    var bits = 0u;
    if (L._a == 0u) { bits = open0[idx(i, j, k)]; }
    for (var q = 0; q < 6; q++) {
        var w: f32;
        if (L._a == 0u) {
            w = select(0.0, 1.0, (bits & (1u << u32(q))) != 0u);
        } else {
            let n = idx(i, j, k);
            if (q < 4) { w = wt[2u * n][q]; } else { w = wt[2u * n + 1u][q - 4]; }
        }
        if (w == 0.0) { continue; }
        let a = vec3i(i, j, k) + NB[q];
        c += w;
        if (inside(a.x, a.y, a.z)) { s += w * x[idx(a.x, a.y, a.z)]; }
    }
    return vec2f(s, c);
}

@compute @workgroup_size(8, 8, 4)
fn open_bits(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= L.nx || id.y >= L.ny || id.z >= L.nz) { return; }
    var bits = 0u;
    for (var q = 0; q < 6; q++) {
        if (w0(i32(id.x), i32(id.y), i32(id.z), q) > 0.0) { bits |= 1u << u32(q); }
    }
    open0[(id.z * L.ny + id.y) * L.nx + id.x] = bits;
}

fn gs_sweep(id: vec3u, colour: u32) {
    let j = id.y;
    let k = id.z;
    let i = 2u * id.x + ((j + k + colour) & 1u);
    if (i >= L.nx || j >= L.ny || k >= L.nz) { return; }
    let n = (k * L.ny + j) * L.nx + i;
    if (m[n] != 0u) { x[n] = 0.0; return; }
    let st = stencil(i32(i), i32(j), i32(k));
    if (st.y == 0.0) { x[n] = 0.0; return; }
    x[n] = (st.x - b[n]) / st.y;
}

@compute @workgroup_size(8, 8, 4)
fn smooth_red(@builtin(global_invocation_id) id: vec3u) { gs_sweep(id, 0u); }

@compute @workgroup_size(8, 8, 4)
fn smooth_black(@builtin(global_invocation_id) id: vec3u) { gs_sweep(id, 1u); }

@compute @workgroup_size(8, 8, 4)
fn residual(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= L.nx || id.y >= L.ny || id.z >= L.nz) { return; }
    let n = (id.z * L.ny + id.y) * L.nx + id.x;
    if (m[n] != 0u) { r[n] = 0.0; return; }
    let st = stencil(i32(id.x), i32(id.y), i32(id.z));
    r[n] = b[n] - (st.x - st.y * x[n]);
}

fn cidx(i: u32, j: u32, k: u32) -> u32 { return (k * L.cy + j) * L.cx + i; }

@compute @workgroup_size(8, 8, 4)
fn restrict_down(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= L.cx || id.y >= L.cy || id.z >= L.cz) { return; }
    let n = cidx(id.x, id.y, id.z);
    // fine residuals of the 8 children, computed here (no separate pass)
    var s = 0.0;
    for (var c = 0u; c < 8u; c++) {
        let a = 2u * id + vec3u(c & 1u, (c >> 1u) & 1u, (c >> 2u) & 1u);
        if (a.x < L.nx && a.y < L.ny && a.z < L.nz) {
            let fn_ = (a.z * L.ny + a.y) * L.nx + a.x;
            if (m[fn_] == 0u) {
                let stn = stencil(i32(a.x), i32(a.y), i32(a.z));
                s += b[fn_] - (stn.x - stn.y * x[fn_]);
            }
        }
    }
    bc[n] = select(0.5 * s, 0.0, mc[n] != 0u);
    xc[n] = 0.0;
}

// Coarse mask (solid only if every existing child is solid) and coarse face
// conductances (mean over the fine faces each coarse face covers; a solid
// fine cell's faces are closed).
fn wfine(i: i32, j: i32, k: i32, q: i32) -> f32 {
    if (m[idx(i, j, k)] != 0u) { return 0.0; }
    return wface(i, j, k, q);
}

@compute @workgroup_size(8, 8, 4)
fn coarsen_mask(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= L.cx || id.y >= L.cy || id.z >= L.cz) { return; }
    var all_solid = 1u;
    for (var c = 0u; c < 8u; c++) {
        let a = 2u * id + vec3u(c & 1u, (c >> 1u) & 1u, (c >> 2u) & 1u);
        if (a.x < L.nx && a.y < L.ny && a.z < L.nz && m[(a.z * L.ny + a.y) * L.nx + a.x] == 0u) { all_solid = 0u; }
    }
    let n = cidx(id.x, id.y, id.z);
    mc[n] = all_solid;
    var w = array<f32, 6>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let f = vec3i(2 * i32(id.x), 2 * i32(id.y), 2 * i32(id.z));
    let top = vec3i(i32(L.nx) - 1, i32(L.ny) - 1, i32(L.nz) - 1);
    for (var q = 0; q < 6; q++) {
        let axis = q / 2;
        let plus = (q & 1) == 1;
        var acc = 0.0;
        var cnt = 0.0;
        for (var b = 0; b < 2; b++) {
            for (var a = 0; a < 2; a++) {
                var c = f;
                if (axis == 0) { c.y += a; c.z += b; } else if (axis == 1) { c.x += a; c.z += b; } else { c.x += a; c.y += b; }
                if (plus) { c[axis] = min(c[axis] + 1, top[axis]); }
                if (c.x > top.x || c.y > top.y || c.z > top.z) { continue; }
                acc += wfine(c.x, c.y, c.z, q);
                cnt += 1.0;
            }
        }
        var wq = select(0.0, acc / cnt, cnt > 0.0 && all_solid == 0u);
        // a coarse face on the domain boundary: keep the open boundary in place
        let nbc = vec3i(id) + NB[q];
        if (nbc.x < 0 || nbc.y < 0 || nbc.z < 0 || nbc.x >= i32(L.cx) || nbc.y >= i32(L.cy) || nbc.z >= i32(L.cz)) {
            wq *= wb(L._b) / wb(L._b - 1u);
        }
        w[q] = wq;
    }
    wtc[2u * n] = vec4f(w[0], w[1], w[2], w[3]);
    wtc[2u * n + 1u] = vec4f(w[4], w[5], 0.0, 0.0);
}

fn xc_at(i: i32, j: i32, k: i32) -> f32 {
    let a = u32(clamp(i, 0, i32(L.cx) - 1));
    let bb = u32(clamp(j, 0, i32(L.cy) - 1));
    let c = u32(clamp(k, 0, i32(L.cz) - 1));
    // ghosts outside the coarse grid, so the interpolant is 0 where level 0 has it
    let out = select(0.0, 1.0, i < 0 || i >= i32(L.cx)) + select(0.0, 1.0, j < 0 || j >= i32(L.cy)) + select(0.0, 1.0, k < 0 || k >= i32(L.cz));
    return xc[cidx(a, bb, c)] * pow(1.0 - wb(L._b), out);
}

@compute @workgroup_size(8, 8, 4)
fn prolong(@builtin(global_invocation_id) id: vec3u) {
    if (id.x >= L.nx || id.y >= L.ny || id.z >= L.nz) { return; }
    let n = (id.z * L.ny + id.y) * L.nx + id.x;
    if (m[n] != 0u) { return; }
    let q = (vec3f(id) + 0.5) * 0.5 - 0.5;
    let bq = floor(q);
    let f = q - bq;
    let i = i32(bq.x); let j = i32(bq.y); let k = i32(bq.z);
    let c00 = mix(xc_at(i, j, k), xc_at(i + 1, j, k), f.x);
    let c10 = mix(xc_at(i, j + 1, k), xc_at(i + 1, j + 1, k), f.x);
    let c01 = mix(xc_at(i, j, k + 1), xc_at(i + 1, j, k + 1), f.x);
    let c11 = mix(xc_at(i, j + 1, k + 1), xc_at(i + 1, j + 1, k + 1), f.x);
    x[n] += mix(mix(c00, c10, f.y), mix(c01, c11, f.y), f.z);
}
