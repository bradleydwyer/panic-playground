// Part 5's smoke: what both solvers (solver3d.wgsl and solver2d.wgsl)
// share about the mountain, prepended to each by field_frame_smoke.rs. They
// declare `st` (the step, with its camera and view), `relief_tex` (the
// fitted relief) and `vsamp` (a linear sampler).

// The mountain's outline over screen x: the highest screen y the fitted
// relief reaches there, with the code's camera and projection at the step
// (field_common.wgsl's terrain_dir, terrain_row and the depths
// terrain_march takes, from NEAR to FAR over the map), or -1 where it
// reaches nothing. A whole map unit a sample: the heat it places is soft.
fn smoke_outline(x: f32) -> f32 {
    // Screen x to the original's source column: scene x is (x - 8/9) 200,
    // and a source column is 5/6 of a scene unit.
    let column = 160.0 + (x - 0.8888889) * 240.0;
    let dir = st.view.xy + (column - 161.0) / 256.0 * st.view.zw;
    let origin = st.camera.xy;
    var z0 = 64.0;
    var z1 = 512.0;
    for (var a = 0; a < 2; a++) {
        if (abs(dir[a]) > 1e-6) {
            let p = -origin[a] / dir[a];
            let q = (200.0 - origin[a]) / dir[a];
            z0 = max(z0, min(p, q));
            z1 = min(z1, max(p, q));
        } else if (origin[a] < 0.0 || origin[a] > 200.0) {
            return -1.0;
        }
    }
    let q = st.camera.w;
    var top = 1e9;
    var z = z0;
    loop {
        if (z >= z1) { break; }
        let m = origin + dir * z;
        let h = textureSampleLevel(relief_tex, vsamp, m.yx / 200.0, 0.0).r;
        if (h >= 0.5) {
            let projection = q * 256.0 / max(z, 1.0) + 512.0 - 2.0 * q;
            top = min(top, (st.camera.z - h) * projection / 1024.0);
        }
        z += 1.0;
    }
    if (top > 1e8) { return -1.0; }
    // Source row (y down) to screen y.
    return 1.0 - top / 200.0;
}

fn smoke_hash(a: i32, b: i32) -> f32 {
    var x = u32(a) * 1664525u + u32(b) * 1013904223u;
    x ^= x >> 16u; x *= 0x7feb352du; x ^= x >> 15u; x *= 0x846ca68bu; x ^= x >> 16u;
    return f32(x >> 8u) / 16777216.0;
}
// Where the outline's heat burns at x (in flicker widths) and time t (s):
// smooth value noise, mostly off, so separate plumes rise.
fn smoke_flicker(x: f32, t: f32) -> f32 {
    let tt = t * 0.9;
    let i = floor(x);
    let j = floor(tt);
    let f = x - i;
    let s = tt - j;
    let a = i32(i);
    let b = i32(j);
    let fx = f * f * (3.0 - 2.0 * f);
    let fy = s * s * (3.0 - 2.0 * s);
    let v = mix(mix(smoke_hash(a, b), smoke_hash(a + 1, b), fx),
                mix(smoke_hash(a, b + 1), smoke_hash(a + 1, b + 1), fx), fy);
    return smoothstep(0.35, 0.8, v);
}
