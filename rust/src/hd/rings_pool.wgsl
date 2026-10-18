// Pool (authored): the rings as a pool of liquid chrome seen from above,
// roughly straight on as the original's frame. The two ring layers are two
// wave sources, and their superposition makes the interference bands. Each
// source has a lamp hung low over it, cool over the first layer's and warm
// over the second's, so each source's waves catch its own colour and their
// crests cast shadows away from it. The lit symbols hover just above the
// surface as neon tubes, and the waves bend their reflections. By default
// the waves fill the screen (POOL_FILL).
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl.
//
// Where it comes from:
// - From rings_common.wgsl, which says what it read from the port and what
//   it fitted to the local data: the sources' positions (each layer's ring
//   centre, moving as the original's does, with no motion of our own), the
//   band width and the first edge's radius, the ring window (with
//   POOL_FILL 0), the ring intensity and its fades, the lit symbol and its
//   windows, and the PSTART fade.
// - Authored after the original's palette: the wave height. Each layer's
//   waves crest in the middle of the bands the original sets and trough in
//   the middle of its clear ones, and the second layer's waves are inverted
//   (as if its source ran in antiphase). Were the bands square, the surface
//   would be raised where only the grey-48 layer is set, sunken where only
//   the grey-32 layer is, and level where the original is black (both or
//   neither): there a still, black mirror, as the original's XOR is black.
// - Authored: the chrome, the waves' height and profile, the waves continued
//   past the window to fill the screen, the lamps (their colours, height and
//   shades), the softbox and room, the neon's height and everything else
//   that is seen.
//
// The timing follows the original exactly: black before the first symbol,
// then the neon on black, with no pool, until the rings fade in. The pool
// comes up with them, waves, lamps and reflections together, and the lamps'
// power follows ring_intensity, so the fade-out is the lamps going out. By
// default the waves stay whole after it, seen only by the neon they reflect
// (POOL_DARK_WAVES; at 0 their height follows ring_intensity down too, and
// the pool settles into a still black mirror). From 80.36 s the PSTART
// picture at its fade level, placed with tonemap_inverse as faithful-plus
// places it, so that at 81.27 s the screen shows the picture part 5 opens on.

// ---- Tunables (all authored) -------------------------------------------------
// The waves' depth: each layer's crest height at full intensity, in rows
// (source rows on the 4:3 display). About 1 keeps them liquid; 2 is choppy.
const POOL_DEPTH=.8;
// How much of that height the waves keep once the rings have faded out: 0
// lets them settle into a still black mirror, as the original is black; 1
// keeps them whole with the lamps off, seen only by the neon they reflect.
// Before the rings first fade in there is no pool, only the neon on black.
const POOL_DARK_WAVES=1.0;
// Sharpening of each layer's profile, from a sine (0) towards the
// original's square bands: steeper flanks at the band edges, flatter
// crests and troughs.
const POOL_SHARP=1.8;
// The waves round off within this many source pixels of each source.
const POOL_CORE=3.0;
// RING_FAITHFUL measures the waves as the original's bands, circles in
// source pixels and so 1.2 times taller than wide, which keeps the pattern
// on the original's; RING_ROUND would make them round.
const POOL_METRIC=RING_FAITHFUL;
// Where the waves are: 0 the original's ring window (256 x 184 source
// pixels at (32, 8), read from the code; the pool is level round it), 1 the
// whole screen, wide screens included (authored: the waves are continued
// past the window, from the same sources).
const POOL_FILL=1;
// The chrome: its reflectance at normal incidence and its roughness (GGX
// alpha), widened where the waves turn faster than a pixel can show.
const POOL_METAL=vec3f(.62,.64,.68);
const POOL_ROUGH=.14;
// A broad sheen under the sharp reflection, as on satin metal: its share and
// its roughness. It shows the lamps' light across the flanks turned to them,
// and so the crests' shadows.
const POOL_SHEEN=vec2f(.25,.5);
// Exposure: its length in rings_common's shutters of 1/100 s (ring_centre's
// shutter argument, extended linearly past 1), and its samples, so the
// moving fringes blur rather than strobe. 1.67 is a whole 60 Hz frame (a
// 360-degree shutter): the fringes smear over the full step between frames.
const POOL_EXPOSURE=1.67;
const POOL_SHUTTER=6;
// The lamps: height over each source and the distance (rows) at which their
// light has fallen to half. Their shades hide them from straight below:
// no light within POOL_HOOD.x of straight down, all of it beyond .y
// (radians), so they light the waves from the side and never show as a
// bright spot on the still surface.
const POOL_LAMP_HEIGHT=30.0;
const POOL_LAMP_REACH=60.0;
const POOL_HOOD=vec2f(.06,.30);
// How dark the crests' shadows are, 0 (none) to 1.
const POOL_SHADOW=1.0;
// The lamps' colours and power: the scroller's cool and warm extremes.
const POOL_COOL=vec3f(.12,.52,1.0)*2.5;
const POOL_WARM=vec3f(1.0,.27,.055)*2.2;
// A neutral studio softbox above left, seen by the waves' flanks, and a dim
// room round the pool: black overhead, so the level surface stays black, a
// faint glow on the walls, cool on the left and warm on the right.
const POOL_SOFTBOX=vec3f(.85,.9,1.0)*.35;
const POOL_SOFTBOX_AT=vec3f(-.50,.62,.60);
const POOL_ROOM=.10;
// The neon tubes' height above the level surface, in rows: with the rings
// lit, high (about a quarter of the screen), so the waves' flanks catch them
// far out across the pool; with the lamps out, low, so their reflections
// keep near them. It follows the rings' fades between the two.
const POOL_NEON_HIGH=48.0;
const POOL_NEON_LOW=8.0;
fn pool_neon_height()->f32 {return mix(POOL_NEON_LOW,POOL_NEON_HIGH,ring_intensity());}
// How strongly the surface mirrors the neon: the sharp image of the tubes
// (1 is the chrome's own reflectance; more is authored, so the neon holds
// its own against the lamps), and the soft glow round it from the chrome's
// sheen (0 none, 1 the sheen's own share).
const POOL_NEON_GLINT=2.0;
const POOL_NEON_GLOW=2.0;
// How the exposure's samples, each at its own point in the pixel, resolve
// the neon's reflection: the share of a pixel each antialiases the tubes'
// image over, and the share of the pixel's spread of normals that still
// blurs it (0: the samples resolve it; about 2 is what one sample needs).
const POOL_NEON_SHARP=vec2f(.4,0.0);

// ---- Waves -----------------------------------------------------------------
// One layer's profile across its bands, from the phase (rings_common's
// ring_phase: 0 at the first edge, 1 per period): +1 in the middle of a band
// the original sets, -1 in a clear one, and the slope per period.
fn pool_profile(phase:f32)->vec2f {
    let a=2.0*PI*phase;
    let t=tanh(POOL_SHARP*sin(a));
    let n=1.0/tanh(POOL_SHARP);
    return vec2f(t*n,(1.0-t*t)*POOL_SHARP*cos(a)*2.0*PI*n);
}
// One layer at source point s: its profile and slope in scene axes (per
// row). The phase is ring_phase's, from the layer's centre, band width and
// first edge, with the radius rounded off within POOL_CORE of the source.
fn pool_layer(layer:u32,s:vec2f,shutter:f32)->vec3f {
    let d=(s-ring_centre(layer,shutter))*POOL_METRIC;
    let r=sqrt(dot(d,d)+POOL_CORE*POOL_CORE);
    let p=pool_profile((r-ring_first())/(2.0*ring_width()));
    let g=POOL_METRIC*d/(r*2.0*ring_width())*p.y;
    return vec3f(p.x,g.x/COLUMN,-g.y);
}
fn pool_amp()->f32 {return POOL_DEPTH*mix(ring_intensity(),ring_peak(),POOL_DARK_WAVES);}
// Where the waves run at source point s: 1 in them, antialiased over `blur`
// pixels at the ring window's edge, or everywhere when they fill the screen.
fn pool_window(s:vec2f,blur:f32)->f32 {
    if POOL_FILL==1 {return 1.0;}
    return ring_window(s,blur);
}
// The surface at source point s: height in rows and slope in scene axes.
// The first layer's waves less the second's, where the waves run (the pool
// is level beyond).
fn pool_wave(s:vec2f,shutter:f32,window:f32)->vec3f {
    return (pool_layer(0u,s,shutter)-pool_layer(1u,s,shutter))*window*pool_amp();
}
fn pool_height(s:vec2f)->f32 {return pool_wave(s,.5*POOL_EXPOSURE,pool_window(s,1.0)).x;}

// ---- Light -----------------------------------------------------------------
// GGX reflection of a light from unit direction l, per unit irradiance, for
// a surface normal n seen from v.
fn pool_ggx(n:vec3f,v:vec3f,l:vec3f,alpha:f32)->vec3f {
    let h=normalize(v+l);
    let nh=max(dot(n,h),0.0);let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),1e-3);
    let a2=alpha*alpha;
    let q=nh*nh*(a2-1.0)+1.0;
    let k=alpha*.5;
    let g=nl/(nl*(1.0-k)+k)*nv/(nv*(1.0-k)+k);
    let f=POOL_METAL+(1.0-POOL_METAL)*pow(1.0-max(dot(v,h),0.0),5.0);
    return f*a2/(PI*q*q)*g/(4.0*nv);
}
// A layer's lamp, in scene space.
fn pool_lamp(layer:u32,shutter:f32)->vec3f {
    return vec3f(scene_of_source(ring_centre(layer,shutter)),POOL_LAMP_HEIGHT);
}
// How much of a lamp's light reaches surface point p (source point s) past
// the crests: the steepest rise of the surface towards it, within the
// distance to its foot, against the light's own elevation, a little soft.
fn pool_lit(s:vec2f,p:vec3f,lamp:vec3f)->f32 {
    let d=lamp-p;
    let flat=length(d.xy);
    if flat<1e-3 {return 1.0;}
    let dir=vec2f(d.x/COLUMN,-d.y)/flat;
    let rise=d.z/flat;
    var horizon=-9.0;
    for(var i=1;i<=7;i++) {
        let t=min(f32(i*i)*.22+.5,flat);
        horizon=max(horizon,(pool_height(s+dir*t)-p.z)/t);
    }
    return 1.0-POOL_SHADOW*smoothstep(rise-.06,rise+.06,horizon);
}
// A lamp's light reflected towards the eye by a lobe of roughness alpha,
// with the lamp's falloff and shade.
fn pool_lamp_light(n:vec3f,v:vec3f,p:vec3f,lamp:vec3f,alpha:f32)->vec3f {
    let d=lamp-p;
    let l=normalize(d);
    let fall=POOL_LAMP_REACH*POOL_LAMP_REACH/(dot(d,d)+POOL_LAMP_REACH*POOL_LAMP_REACH);
    let shade=smoothstep(cos(POOL_HOOD.y),cos(POOL_HOOD.x),l.z);
    return pool_ggx(n,v,l,alpha)*fall*(1.0-shade);
}
// The room the pool reflects along unit direction r, without the lamps.
fn pool_room(r:vec3f,alpha:f32)->vec3f {
    let box=smoothstep(.96-alpha*.6,.97,dot(r,normalize(POOL_SOFTBOX_AT)));
    let side=r.x/max(length(r.xy),1e-4);
    let wall=mix(vec3f(.55,.75,1.0),vec3f(1.0,.75,.55),.5+.5*side)*(1.0-smoothstep(.55,.95,r.z));
    return POOL_SOFTBOX*box+wall*POOL_ROOM;
}
// Lamp power (and the room's): on with the rings. With musical lighting
// (L), it breathes with the music and swells on the bass (authored, from
// the shared cues; studio lighting holds it steady).
fn pool_power()->f32 {
    return ring_intensity()*mix(1.0,.6+.5*cue_level()+.9*cue_bass(),musical_lighting());
}

// The lit tubes as the surface mirrors them from scene point p along unit
// r: the tubes themselves where the reflected ray meets their plane, as
// sharp as the reflection is. `spread` (radians) is how far the chrome's
// roughness and the pixel's spread of normals scatter the ray; measured
// where it meets the plane (hence the obliquity), with the pixel's own
// footprint. Past about two source pixels, the light map blurred as far.
// Through the fade-in it comes up with the rings (ring_peak).
// The tubes are drawn where they are seen, at their source points; hanging
// nearer the eye than the surface, a tube drawn at source point s is at
// scene_of_source(s) times (EYE_Z - height) / EYE_Z in its plane, so a
// point there is scaled back out to find the source point it is drawn at.
fn pool_neon(p:vec3f,r:vec3f,pixel:f32,spread:f32)->vec3f {
    let height=pool_neon_height();
    let hit=neon_plane_hit(p,r,height);
    if hit.w<0.0 {return vec3f(0);}
    let out=EYE_Z/(EYE_Z-height);
    let s=source_of_scene(hit.xy*out);
    let footprint=(pixel+spread*hit.w/max(r.z,.05))*out;
    let sigma=max(footprint,.6);
    let soft=NEON_LIGHT*(neon_map(neon_mask(),s,sigma)*neon_on()+neon_map(neon_previous(),s,sigma)*neon_previous_on());
    if footprint>2.0 {return soft*ring_peak();}
    let sharp=neon_emission(s,footprint);
    return mix(sharp,soft,smoothstep(.8,2.0,footprint))*ring_peak();
}

// ---- The pool ----------------------------------------------------------------
// The pool's radiance at source point s, seen along the pixel's ray.
fn pool_surface(px:RingsPixel,s:vec2f)->vec3f {
    let ray=camera_ray(px.xy);
    let v=-ray;
    let flat_p=vec3f(scene_of_source(s),0.0);
    let flat_r=reflect(ray,vec3f(0,0,1));
    let f0=POOL_METAL+(1.0-POOL_METAL)*pow(1.0-v.z,5.0);
    let window=pool_window(s,px.pixel*1.5);
    // Level: a black mirror showing only the neon (and, faintly, the room).
    if pool_amp()<=0.0 || window<=0.0 {
        return f0*(pool_neon(flat_p,flat_r,px.pixel,POOL_ROUGH*.6)*POOL_NEON_GLINT
            +pool_room(flat_r,POOL_ROUGH)*pool_power());
    }
    let mid=pool_wave(s,.5*POOL_EXPOSURE,window);
    let here=vec3f(flat_p.xy,mid.x);
    // How fast the normal turns across a pixel: it widens the lamps'
    // highlights where the waves would otherwise shrink them below a pixel
    // and alias (and blurs the neon's reflection by POOL_NEON_SHARP.y).
    let e=.5;
    let wx=pool_wave(s+vec2f(e,0),.5*POOL_EXPOSURE,window).yz-mid.yz;
    let wy=pool_wave(s+vec2f(0,e),.5*POOL_EXPOSURE,window).yz-mid.yz;
    let turn=sqrt(dot(wx,wx)/(e*e*COLUMN*COLUMN)+dot(wy,wy)/(e*e))*px.pixel;
    let alpha=sqrt(POOL_ROUGH*POOL_ROUGH+turn*turn);
    let power=pool_power();
    let cool=POOL_COOL*pool_lit(s,here,pool_lamp(0u,.5*POOL_EXPOSURE))*power;
    let warm=POOL_WARM*pool_lit(s,here,pool_lamp(1u,.5*POOL_EXPOSURE))*power;
    // The broad sheen, once, at the middle of the exposure.
    let mid_n=normalize(vec3f(-mid.yz,1));
    let sheen_alpha=max(alpha,POOL_SHEEN.y);
    var c=(cool*pool_lamp_light(mid_n,v,here,pool_lamp(0u,.5*POOL_EXPOSURE),sheen_alpha)
        +warm*pool_lamp_light(mid_n,v,here,pool_lamp(1u,.5*POOL_EXPOSURE),sheen_alpha))*POOL_SHEEN.x*f32(POOL_SHUTTER);
    // The neon in the sheen too: a soft glow round its sharp reflection.
    let mid_r=reflect(ray,mid_n);
    let mid_f=POOL_METAL+(1.0-POOL_METAL)*pow(1.0-max(dot(mid_n,v),0.0),5.0);
    c+=mid_f*pool_neon(here,mid_r,px.pixel,sheen_alpha)*POOL_SHEEN.x*POOL_NEON_GLOW*f32(POOL_SHUTTER);
    for(var i=0;i<POOL_SHUTTER;i++) {
        let t=(f32(i)+.5)/f32(POOL_SHUTTER)*POOL_EXPOSURE;
        // Each sample at its own point in the pixel too (an R2 sequence), so
        // the samples resolve the neon's reflection across the pixel.
        let o=(fract(f32(i+1)*vec2f(.7548777,.5698403))-.5)*px.pixel;
        let q=s+vec2f(o.x/COLUMN,o.y);
        let w=pool_wave(q,t,window);
        let n=normalize(vec3f(-w.yz,1));
        let r=reflect(ray,n);
        let p=vec3f(scene_of_source(q),w.x);
        let f=POOL_METAL+(1.0-POOL_METAL)*pow(1.0-max(dot(n,v),0.0),5.0);
        // The neon reflected, blurred by the chrome's roughness and by what
        // is left of the pixel's spread of normals between the samples.
        let neon=pool_neon(p,r,px.pixel*POOL_NEON_SHARP.x,POOL_ROUGH*.6+POOL_NEON_SHARP.y*turn);
        c+=f*(neon*POOL_NEON_GLINT+pool_room(r,alpha)*power);
        let sharp=1.0-POOL_SHEEN.x;
        c+=cool*pool_lamp_light(n,v,p,pool_lamp(0u,t),alpha)*sharp;
        c+=warm*pool_lamp_light(n,v,p,pool_lamp(1u,t),alpha)*sharp;
    }
    return c/f32(POOL_SHUTTER);
}

fn style_color(px:RingsPixel)->vec3f {
    if rings_phase()<.5 {return vec3f(0);}
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let tubes=neon_glass(px.source,px.pixel);
    // Until the rings first show, no pool: the neon on black.
    if ring_peak()<=0.0 {return neon_over(vec3f(0),vec3f(0),tubes)+neon_haze(px.source);}
    let behind=pool_surface(px,px.source);
    // Through the glass, the pool a little to the side.
    var seen=behind;
    if tubes.glass>0.0 {seen=pool_surface(px,px.source+tubes.bend);}
    return neon_over(behind,seen,tubes)+neon_haze(px.source);
}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
