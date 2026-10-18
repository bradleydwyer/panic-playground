// Volume (authored): the original's two ring layers as spherical waves in a
// slab of haze. What comes from the port, through rings_common.wgsl: each
// layer's centre (ring_centre), the ring window, the palette intensity, and
// the lit symbols, their masks and windows; and, as interpretation, the
// band width and first edge (ring_width, ring_first: the band model fitted
// to M.DAT, which matches about 98% of its pixels). Everything else here is
// authored: the shells, the haze, the light, the camera and the look.
//
// Each centre is a point emitter in the screen plane z = 0. Round it, shells
// at the original's band radii, measured as the original measures them
// (circles in source pixels, so RING_FAITHFUL's metric, with z in rows),
// hold haze: a shell's section in z = 0 is the original's ring, and the
// moiré of the two layers is the original's, extended into depth. Layer 0's
// shells (the original's grey 48) are cool, layer 1's (grey 32) warm, the
// owner's cool-warm split, at about those two greys; where both layers'
// bands are set the haze is dark and absorbs, the original's XOR. Each set
// band's haze is a lens, thin at the band's edges and deepest in its
// middle, so the bands keep sharp edges from any angle; round each emitter
// the lenses deepen until the inner shells close into spheres. The haze
// stays inside the original's ring window, black outside it, and comes and
// goes with the palette fade. A side light from the upper left, low over
// the plane, lights the haze and the shells shade and shadow one another, so
// the bands read as ridges of light; with musical lighting (L) it follows
// the music. The lit
// symbols are the shared neon tubes, lying in the plane z = 0 in the middle
// of the haze; their light (neon_fluence) scatters on the shells near them
// and in a faint clear haze, which alone shows while the symbols are on
// black.
//
// The camera (volume_controls in hd/rings.rs) is the original's view while
// the rings fade in, drifts slowly off-axis while they are at full intensity
// and is back on the axis before they fade out: an authored move, a pure
// function of the clip time. Seen flat, the haze reads as the original's
// rings.
//
// The haze is ray-marched at half resolution (style_prepass), on a grid in z
// that is finest at the plane. Each step integrates the lenses' haze in z in
// closed form, and averages the bands over the radii the step spans, a
// pixel's footprint and the centres' travel over the shared 1/100 s shutter,
// also in closed form, so the march needs no noise and the bands neither
// alias along a ray nor strobe (none seen in 60 fps clips). style_color
// draws the tubes at full resolution over the haze.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl.

// ---- Tunables (authored) -----------------------------------------------------
// The haze: each set band holds a lens of it, Gaussian in z with a
// deviation that grows from VOLUME_LENS.x at the band's edges to
// VOLUME_LENS.y in its middle (rows), so the bands keep sharp edges from any
// angle; normalised so that a flat view through a band crosses the same
// haze everywhere across it. Round each emitter the lenses deepen by up to
// VOLUME_SWELL.x rows, over about VOLUME_SWELL.y band pixels, so the inner
// shells close into spheres.
const VOLUME_LENS=vec2f(.35,2.6);
const VOLUME_SWELL=vec2f(14.0,26.0);
// The march: steps through the slab on a grid z = a sinh(b v), v from 1 to
// -1, finest at the plane; and the shadow rays' steps, each twice as long
// as the one before, and reach (rows). A shadow ray's first step, through
// the lens its point lies in, is integrated in closed form; the longer ones
// further out, which mostly meet the deeper lenses round the emitters, are
// sampled at their middle.
const VOLUME_STEPS=32;
const VOLUME_GRID=vec2f(5.0,3.4);
const VOLUME_SHADOW_STEPS=3;
const VOLUME_SHADOW_REACH=60.0;
// The shells' tints, times the original's greys: layer 0 cool, layer 1
// warm, a little brighter than grey 32 alone would give so that it reads as
// amber rather than brown.
const VOLUME_COOL=vec3f(.50,.74,1.0);
const VOLUME_WARM=vec3f(1.3,.76,.30);
// How the shells show at full intensity, seen flat through a set band, in
// the original's grey: their own glow, and the side light they scatter.
const VOLUME_GLOW=.45;
const VOLUME_KEY=1.15;
// With musical lighting (L) the side light follows the music, from the
// shared cues measured from the PCM: a base, and the weights of the level
// and of the bass onsets (squared). Studio keeps it steady.
const VOLUME_MUSIC=vec3f(.72,.35,.9);
// Towards the side light, in scene axes: upper left, low over the plane.
const VOLUME_KEY_DIR=vec3f(-.78,.46,.42);
// Optical depth of the haze seen flat through a set band; and of a set
// band's middle crossed by the side light in the plane: the shadows'
// strength.
const VOLUME_TAU=.5;
const VOLUME_SHADOW=1.5;
// The neon's light scattered by the shells, and by a faint clear haze
// round the plane (its glow round the symbols, also on black).
const VOLUME_NEON_SHELLS=.35;
const VOLUME_NEON_HAZE=.045;
// The clear haze's deviation in z (rows), and how much of the haze in front
// of the tubes veils them (0 none, 1 all of it).
const VOLUME_CLEAR=1.5;
const VOLUME_VEIL=.5;
// The window's soft edge (source pixels).
const VOLUME_EDGE=1.0;
// Where the clear haze ends, softly over its last VOLUME_BOUNDS_EDGE: the
// ring window and the symbols' reach, in source pixels.
const VOLUME_BOUNDS=vec4f(0,-24,320,224);
const VOLUME_BOUNDS_EDGE=24.0;

// ---- Camera ------------------------------------------------------------------
// From volume_controls: eye (scene rows) and focal length (rows, for 100
// rows per screen unit at the pivot), then right, up and forward. Zero
// controls (part 6 not shown) give the default camera.
struct VolumeCamera { eye:vec3f, right:vec3f, up:vec3f, forward:vec3f, focal:f32 }
fn volume_camera()->VolumeCamera {
    let c=u.rings_style;
    if c[0].w<1.0 {return VolumeCamera(EYE,vec3f(1,0,0),vec3f(0,1,0),vec3f(0,0,-1),EYE_Z);}
    return VolumeCamera(c[0].xyz,c[1].xyz,c[2].xyz,c[3].xyz,c[0].w);
}
fn volume_ray(cam:VolumeCamera,xy:vec2f)->vec3f {
    return normalize(cam.right*xy.x*100.0+cam.up*xy.y*100.0+cam.forward*cam.focal);
}

// ---- Shells ------------------------------------------------------------------
// A scene point in the shells' space: source columns and rows, and z in
// rows, so a shell's section in z = 0 is a ring as the original draws it.
fn volume_source(p:vec3f)->vec3f {return vec3f(source_of_scene(p.xy),p.z);}
// The set bands' length from the centre to radius r (band pixels).
fn volume_band_integral(r:f32)->f32 {
    let w=ring_width();
    let x=max(r-ring_first(),0.0);
    let k=floor(x/(2.0*w));
    return k*w+min(x-2.0*k*w,w);
}
// The share of radii lo..hi inside a set band.
fn volume_cover(lo:f32,hi:f32)->f32 {
    let span=max(hi-lo,.02);
    let mid=.5*(lo+hi);
    return clamp((volume_band_integral(mid+.5*span)-volume_band_integral(mid-.5*span))/span,0.0,1.0);
}
// One layer over a step from q0 through qm to q1 (radii r0, rm, r1 from its
// centre c): the share of it in a set band, averaged over the radii the
// step spans, the centre's travel over the shutter (`travel`, now minus
// open) and a pixel's footprint.
fn volume_layer(r0:f32,rm:f32,r1:f32,qm:vec3f,c:vec2f,travel:vec2f,footprint:f32)->f32 {
    let shift=dot(qm.xy-c,travel)/max(rm,1e-3);
    let lo=min(min(r0,r1),rm)+min(shift,0.0)-.5*footprint;
    let hi=max(max(r0,r1),rm)+max(shift,0.0)+.5*footprint;
    return volume_cover(lo,hi);
}
// The lens's deviation in z (rows) at radius r from its emitter.
fn volume_lens(r:f32)->f32 {
    let u=clamp(band_distance(r)/(.5*ring_width()),0.0,1.0);
    let s=r/VOLUME_SWELL.y;
    let deep=VOLUME_LENS.y+VOLUME_SWELL.x*exp(-s*s);
    return mix(VOLUME_LENS.x,deep,sqrt(u*(2.0-u)));
}
fn volume_radius(q:vec3f,c:vec2f)->f32 {return length(vec3f(q.xy-c,q.z));}
// erf(x), to about 1e-4.
fn volume_erf(x:f32)->f32 {
    let t=x*x;
    return sign(x)*sqrt(1.0-exp(-t*(4.0/PI+.147*t)/(1.0+.147*t)));
}
// The haze of a lens with deviation `sigma` between heights z0 and z1, in
// closed form: 1 for a flat view through all of it.
fn volume_sheet(z0:f32,z1:f32,sigma:f32)->f32 {
    let k=1.0/(sqrt(2.0)*sigma);
    return .5*abs(volume_erf(z1*k)-volume_erf(z0*k));
}
// The lens's density at height z, per row: the integrand of volume_sheet.
fn volume_density(z:f32,sigma:f32)->f32 {
    let s=z/sigma;
    return exp(-.5*s*s)/(sqrt(2.0*PI)*sigma);
}

// The clear haze's extent, 1 inside.
fn volume_bounds(s:vec2f)->f32 {
    let inside=min(min(s.x-VOLUME_BOUNDS.x,VOLUME_BOUNDS.z-s.x),min(s.y-VOLUME_BOUNDS.y,VOLUME_BOUNDS.w-s.y));
    return smoothstep(0.0,VOLUME_BOUNDS_EDGE,inside);
}

// Everything the march needs at one moment.
struct VolumeState {
    c0:vec2f, c1:vec2f,           // centres, source pixels
    t0:vec2f, t1:vec2f,           // their travel over the shutter
    fade:vec2f,                   // each layer's palette fade, 0..1
}
fn volume_state()->VolumeState {
    let i=ring_intensity();
    let fade=vec2f(dac_linear(48.0*i)/dac_linear(48.0),dac_linear(32.0*i)/dac_linear(32.0));
    let c0=ring_centre(0u,0.0);
    let c1=ring_centre(1u,0.0);
    return VolumeState(c0,c1,c0-ring_centre(0u,1.0),c1-ring_centre(1u,1.0),fade);
}

// Transmittance of the side light from scene point p.
fn volume_shadow(st:VolumeState,p:vec3f,footprint:f32)->f32 {
    let l=normalize(VOLUME_KEY_DIR);
    var ds=VOLUME_SHADOW_REACH/f32((1<<u32(VOLUME_SHADOW_STEPS))-1u);
    // Optical depth per unit of haze: VOLUME_SHADOW across one set band's
    // middle in the plane, where the lens is deepest (away from the
    // emitters).
    let k=VOLUME_SHADOW*sqrt(2.0*PI)*VOLUME_LENS.y/ring_width();
    var p0=p;
    var q0=volume_source(p);
    var a0=volume_radius(q0,st.c0);
    var b0=volume_radius(q0,st.c1);
    var tau=0.0;
    for(var i=1;i<=VOLUME_SHADOW_STEPS;i++) {
        let p1=p0+l*ds;
        let pm=.5*(p0+p1);
        let q1=volume_source(p1);
        let qm=volume_source(pm);
        let a1=volume_radius(q1,st.c0);
        let b1=volume_radius(q1,st.c1);
        let am=volume_radius(qm,st.c0);
        let bm=volume_radius(qm,st.c1);
        let a=volume_layer(a0,am,a1,qm,st.c0,st.t0,footprint)*st.fade.x;
        let b=volume_layer(b0,bm,b1,qm,st.c1,st.t1,footprint)*st.fade.y;
        let w=ring_window(qm.xy,VOLUME_EDGE+footprint);
        // The first step, through the lens the point lies in, in closed
        // form; the longer ones further out at their middle.
        let sa=volume_lens(am);
        let sb=volume_lens(bm);
        var ha=volume_density(pm.z,sa)*ds;
        var hb=volume_density(pm.z,sb)*ds;
        if i==1 {
            let slant=1.0/max(abs(l.z),1e-3);
            ha=volume_sheet(p0.z,p1.z,sa)*slant;
            hb=volume_sheet(p0.z,p1.z,sb)*slant;
        }
        tau+=(a*ha+b*hb)*w;
        p0=p1;q0=q1;a0=a1;b0=b1;
        ds*=2.0;
    }
    return exp(-k*tau);
}

// The haze along one ray: radiance, and the transmittance from the eye to
// the tubes' plane z = 0.
fn volume_march(cam:VolumeCamera,dir:vec3f,pixel:f32)->vec4f {
    let st=volume_state();
    let rings_on=max(st.fade.x,st.fade.y)>0.0;
    let zmax=VOLUME_GRID.x*sinh(VOLUME_GRID.y);
    if dir.z>-1e-3 {return vec4f(0,0,0,1);}
    // Skip rays that cross the slab outside everything.
    let enter=cam.eye+dir*((zmax-cam.eye.z)/dir.z);
    let leave=cam.eye+dir*((-zmax-cam.eye.z)/dir.z);
    let s_in=source_of_scene(enter.xy);
    let s_out=source_of_scene(leave.xy);
    if any(max(s_in,s_out)<VOLUME_BOUNDS.xy) || any(min(s_in,s_out)>VOLUME_BOUNDS.zw) {return vec4f(0,0,0,1);}
    let music=VOLUME_MUSIC.x+VOLUME_MUSIC.y*cue_level()+VOLUME_MUSIC.z*cue_bass()*cue_bass();
    let key_col=vec3f(1.0,.97,.92)*VOLUME_KEY*mix(1.0,music,musical_lighting());
    let glow0=VOLUME_COOL*dac_linear(48.0);
    let glow1=VOLUME_WARM*dac_linear(32.0);
    let slant=1.0/abs(dir.z);
    var light=vec3f(0);
    var trans=1.0;
    var front=1.0;
    var z_prev=zmax;
    var t_prev=(zmax-cam.eye.z)/dir.z;
    var q_prev=volume_source(cam.eye+dir*t_prev);
    var a_prev=volume_radius(q_prev,st.c0);
    var b_prev=volume_radius(q_prev,st.c1);
    for(var k=1;k<=VOLUME_STEPS;k++) {
        let v=1.0-2.0*f32(k)/f32(VOLUME_STEPS);
        let z=VOLUME_GRID.x*sinh(VOLUME_GRID.y*v);
        let t=(z-cam.eye.z)/dir.z;
        let tm=.5*(t+t_prev);
        let ds=t-t_prev;
        let pm=cam.eye+dir*tm;
        let q=volume_source(cam.eye+dir*t);
        let qm=volume_source(pm);
        let ra=volume_radius(q,st.c0);
        let rb=volume_radius(q,st.c1);
        let am=volume_radius(qm,st.c0);
        let bm=volume_radius(qm,st.c1);
        // How much haze this step crosses in each layer's lenses, and in the
        // clear haze, which only the neon lights.
        let ha=volume_sheet(z_prev,z,volume_lens(am))*slant;
        let hb=volume_sheet(z_prev,z,volume_lens(bm))*slant;
        let sheet=volume_sheet(z_prev,z,VOLUME_CLEAR)*slant;
        let clear=volume_bounds(qm.xy);
        var neon=vec3f(0);
        if (rings_on && max(ha,hb)>1e-6) || (clear>0.0 && sheet>1e-6) {neon=neon_fluence(pm,0.0);}
        var emit=neon*VOLUME_NEON_HAZE*sheet*clear;
        var tau=0.0;
        if rings_on && max(ha,hb)>1e-6 {
            let footprint=pixel*tm/cam.focal;
            let w=ring_window(qm.xy,VOLUME_EDGE+footprint);
            if w>0.0 {
                let a=volume_layer(a_prev,am,ra,qm,st.c0,st.t0,footprint);
                let b=volume_layer(b_prev,bm,rb,qm,st.c1,st.t1,footprint);
                let m0=a*ha*st.fade.x*w;
                let m1=b*hb*st.fade.y*w;
                if m0+m1>1e-7 {
                    // Lit only outside the other layer's set bands: the XOR.
                    let tint=glow0*(m0*(1.0-b))+glow1*(m1*(1.0-a));
                    let key=key_col*volume_shadow(st,pm,footprint);
                    emit+=tint*(VOLUME_GLOW+key+neon*VOLUME_NEON_SHELLS);
                    tau=VOLUME_TAU*(m0+m1);
                }
            }
        }
        let through=exp(-tau);
        light+=trans*emit*select(1.0,(1.0-through)/tau,tau>1e-4);
        trans*=through;
        if k==VOLUME_STEPS/2 {front=trans;}
        z_prev=z;
        t_prev=t;
        a_prev=ra;
        b_prev=rb;
    }
    return vec4f(light,front);
}

// One pixel in source rows at the pivot, at full resolution. Computed, not
// px.pixel: that comes from fwidth, which the compiler may evaluate after
// the march's early exits, where derivatives are undefined (seen as dashed
// lines beside the window on Metal).
fn volume_pixel()->f32 {return 200.0/u.screen.y;}

fn style_prepass(px:RingsPixel)->vec4f {
    if rings_phase()<.5 || rings_phase()>1.5 {return vec4f(0,0,0,1);}
    let cam=volume_camera();
    return volume_march(cam,volume_ray(cam,px.xy),2.0*volume_pixel());
}

fn style_color(px:RingsPixel)->vec3f {
    if rings_phase()<.5 {return vec3f(0);}
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let cam=volume_camera();
    let dir=volume_ray(cam,px.xy);
    let haze=prepass_at(px.uv);
    if dir.z>-1e-3 {return haze.rgb;}
    // The tubes, in the plane z = 0, behind the haze in front of them.
    let t=-cam.eye.z/dir.z;
    let hit=cam.eye+dir*t;
    let s=source_of_scene(hit.xy);
    var tubes=neon_glass(s,volume_pixel()*t/cam.focal);
    if tubes.glass<=0.0 && dot(tubes.light,tubes.light)<=0.0 {return haze.rgb;}
    // Through the glass, the haze a little to the side.
    let per_uv=vec2f(200.0*screen_aspect()/COLUMN,200.0);
    let seen=prepass_at(px.uv+tubes.bend/per_uv).rgb;
    tubes.light*=mix(1.0,haze.a,VOLUME_VEIL);
    return neon_over(haze.rgb,seen,tubes);
}
