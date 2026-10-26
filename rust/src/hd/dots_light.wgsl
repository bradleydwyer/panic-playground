// The modern part 3: light painting. Authored throughout, on the port's
// timing, positions and motion (dots_common.wgsl).
//
// The scene is a long exposure in a dark, faintly hazy space, as a camera
// on a tripod would take it:
// - The zoom carries part 5's frame (its own modern view, drawn by
//   field_render.rs through FieldBlock::part3_view) by the original's law,
//   its scale falling by 256 a frame, on past the original's last scale
//   until the frame has left the drawable, as the camera flies through the
//   window into the dark: motion blur along the zoom, and the frame going
//   out of focus as it nears.
// - The shade bobs are a fibre-optic brush: each of the 20 discs is a tip
//   that sweeps the path at continuous iterations, its glow exposed for
//   the 16 iterations the original keeps, so the overlap count is the
//   exposure; the brightest overlaps burn to white as the original's
//   palette does from 13 on. Each tip also draws its own fine fibre.
// - The dots are 512 lights, each exposed continuously along its path for
//   the original's 32 frames of echoes, brightest at the head and fading
//   as the echo levels do, through a lens focused on the object's centre:
//   a point out of focus is a disc of light (bokeh) and its trail a swept
//   disc. The exposure along a segment is exact for a uniform disc moving
//   at constant speed, so faster points leave fainter trails and still
//   ones burn bright, as in a photograph.
// - Toward the edges of the picture the discs are clipped to a cat's eye,
//   and where they are large and bright they show the seven blades of the
//   diaphragm.
// - The air holds a faint uniform haze and drifting fog, lit by every
//   light: the light is gathered each frame in a grid in log depth, and
//   each pixel marches through it, the haze as single scattering in a
//   uniform medium, the fog wherever its noise has wisps.
// - The music: the lights breathe with the PCM's low band at the moment of
//   each part of the exposure, and each order's first row sends a plane of
//   light sweeping through the object.
// - The water: part 4's water (water.wgsl) lies beneath the objects,
//   mirroring the lights, their trails and the air's glow, its swell and
//   drops drawing the mirror image out into streaks. It lies level below
//   the objects until the box comes down, then turns, as the box comes
//   down, into the plane of its base, so the box lands on it (dots_water.rs
//   says why). A light that dips under it fades.
// - At the end the box lands, its trails collapse into its lights, and as
//   part 4 is about to draw it flares up; part 4's slab takes over in white.
// Module: the shared controls, dots_common.wgsl, this file and water.wgsl.

// ---- The look (authored) -----------------------------------------------------------
// The trails: the original's echoes span 32 frames, here taken in
// TRAIL_SAMPLES samples TRAIL_SPACING frames apart; then a faint
// afterglow, the long exposure's memory, AFTER_SAMPLES more AFTER_SPACING
// apart. The exposure of the head, the trail and the afterglow, and their
// decay in frames (the echo levels fall from the second to the eighth in
// about 9).
const TRAIL_SAMPLES=64u;
const TRAIL_SPACING=.5;
const TRAIL_FRAMES=32.0;
const AFTER_SAMPLES=40u;
const AFTER_SPACING=3.5;
const SAMPLES=TRAIL_SAMPLES+AFTER_SAMPLES;
const HEAD_EXPOSURE=1.1;
const HEAD_DECAY=.45;
const TRAIL_EXPOSURE=.11;
const TRAIL_DECAY=9.2;
const AFTER_EXPOSURE=.0035;
const AFTER_DECAY=55.0;
// The lights: power, the radius of a light in focus (rows), and their
// colour, young (the head) and old (the trail's end); the young colour
// fades into the old over AGE_COOL frames.
const POWER=1.0;
const POINT_RADIUS=.36;
const YOUNG=vec3f(.42,.62,1.0);
const OLD=vec3f(.035,.11,1.0);
const AGE_COOL=5.0;
// The lens: its aperture in camera units (the circle of confusion is
// APERTURE * 256 * |1/z - 1/focus| / 2 rows) and the largest circle. Its
// diaphragm has BLADES straight blades, turned by BLADE_TURN radians,
// their edges APOTHEM of the circle's radius from its centre and the
// corners rounded by a circle ROUNDING of it; toward the edges of the
// picture the barrel clips it to a cat's eye, a disc of the same size
// shifted CAT_EYE of the radius for each 100 rows from the centre.
// Discs of light smaller than SHAPED pixels are drawn round, as are the
// trails older than SHAPED_AGE frames, too faint for the shape to show.
const APERTURE=36.0;
const COC_MAX=5.0;
const BLADES=7u;
const BLADE_TURN=.3;
const APOTHEM=.97;
const ROUNDING=1.05;
const CAT_EYE=.32;
const SHAPED=3.0;
const SHAPED_AGE=2.5;
// The blades' outward normals, BLADE_TURN + 2 pi k / 7.
const BLADE_NORMALS=array<vec2f,7>(
    vec2f(.9553365,.2955202),
    vec2f(.3645956,.9311660),
    vec2f(-.5006933,.8656248),
    vec2f(-.9889498,.1482505),
    vec2f(-.7325070,-.6807595),
    vec2f(.0755285,-.9971436),
    vec2f(.8266896,-.5626583));
// The music: the breathing (power at no bass and its rise with it) and the
// waves, planes of light that sweep through the object (where they start
// and their speed in object units a second, their width, gain and how long
// they last in seconds).
const BREATH=vec2f(.8,.4);
const WAVE_START=-330.0;
const WAVE_SPEED=1300.0;
const WAVE_WIDTH=38.0;
const WAVE_GAIN=4.5;
const WAVE_LIFE=.5;
// The haze: its scattering, how far light carries in it (camera units) and
// its tint.
const HAZE=.0045;
const HAZE_REACH=90.0;
const HAZE_TINT=vec3f(.45,.65,1.0);
// The fog in the haze: drifting, slowly changing wisps lit by every light.
// The light is gathered in a grid of FOG_COLUMNS by FOG_ROWS cells across
// the drawable and FOG_SLICES deep, from FOG_NEAR to FOG_FAR (camera
// units, spaced evenly in log depth); each pixel marches FOG_STEPS steps
// through the haze and the fog, skipping the fog where the light is below
// FOG_DARK. The fog's noise repeats every FOG_TILE units; FOG_LEVEL
// and FOG_SOFT shape its wisps, FOG_WARP how far they are twisted, and the
// drifts (units a second) move them. FOG_REACH is how far a light carries
// in it, FOG_SCATTER how much it scatters.
const FOG_COLUMNS=96u;
const FOG_ROWS=54u;
const FOG_SLICES=48u;
const FOG_NEAR=180.0;
const FOG_FAR=7200.0;
const FOG_STEPS=48u;
const FOG_TILE=1400.0;
const FOG_LEVEL=.5;
const FOG_SOFT=.22;
const FOG_WARP=260.0;
const FOG_DRIFT=vec3f(18.0,-11.0,9.0);
const FOG_WARP_DRIFT=vec3f(-8.0,6.0,-13.0);
const FOG_REACH=260.0;
const FOG_SCATTER=.035;
const FOG_DARK=1e-6;
const NOISE_SIZE=128u;
// The shade bobs: samples along each tip's 16 iterations, the glow's radius
// in rows (the disc's area), its depth for the haze, how the film takes
// the exposure past the palette's white (count 25) and the fibres.
const SHADE_SAMPLES=64u;
const SHADE_ROWS=7.1;
const SHADE_DEPTH=700.0;
const SHADE_OVER=.22;
const FIBRE_RADIUS=.3;
const FIBRE_LIGHT=vec3f(.55,.75,1.0)*2.2;
const FIBRE_DECAY=5.0;
const TIP_LIGHT=25.0;
// The zoom: the exposure (seconds) and the defocus as the frame nears.
const ZOOM_SHUTTER=.012;
const ZOOM_DEFOCUS=.012;
// The end: the flare's lead (seconds before the slab), its gain and how
// much the lights swell; the slab's flash over its own colours, how long
// it lasts, and how fast the box's light gives way to it.
const FLARE_LEAD=1.1;
const FLARE_GAIN=9.0;
const FLARE_SWELL=1.4;
const SLAB_FLASH=2.5;
const SLAB_SETTLE=.9;
const SLAB_HANDOVER=.16;
// The water: how deep under it a light still shows (camera units; it
// fades exponentially). The mirror's gain (dc.water.x, part 4's, chosen
// for part 4's steep view, where water reflects only 2 to 7%) raises the
// reflectance F to 1 - (1 - F)^gain, about gain F where F is small, as in
// part 4, but never past a perfect mirror where this level view meets the
// water near grazing.
const UNDER_REACH=12.0;
// The swell's height here, of part 4's as tuned (authored): this level
// view meets the water far nearer grazing than part 4's, where the same
// slopes turn the mirror image much further, so the water is calmer while
// part 3's objects fly over it.
const CALM=.35;

// ---- Bindings -----------------------------------------------------------------------
// The samples: for each light, SAMPLES + 1 along its trail, then for each
// shade tip SHADE_SAMPLES + 1, then the lights the haze sees (532).
// a: scene xy, radius (rows), camera z; b: power, the exposure from the
// head (dots) or the iteration (shade bobs), age (frames or iterations),
// alive.
struct Sample { a:vec4f, b:vec4f }
@group(1) @binding(1) var<storage,read_write> samples_out:array<Sample>;
@group(1) @binding(2) var<storage,read> samples:array<Sample>;
// The fog's light grid (rgb) and its noise (four tileable fbm fields), each
// read through a sampler (the noise repeating) and written by its compute
// step.
@group(1) @binding(3) var fog_light:texture_3d<f32>;
@group(1) @binding(4) var fog_noise:texture_3d<f32>;
@group(1) @binding(5) var repeat:sampler;
@group(1) @binding(6) var fog_light_out:texture_storage_3d<rgba16float,write>;
@group(1) @binding(7) var fog_noise_out:texture_storage_3d<rgba16float,write>;
// The composite's inputs: part 5's modern view at the zoom's magnification
// (HDR), the light painted (rgb, and the shade count in a), the haze (half
// resolution) and the light's mirror image in the water, painted as the
// light is.
@group(2) @binding(1) var underlay:texture_2d<f32>;
@group(2) @binding(2) var painted:texture_2d<f32>;
@group(2) @binding(3) var haze:texture_2d<f32>;
@group(2) @binding(4) var mirror_image:texture_2d<f32>;

const DOT_SAMPLES=POINTS*(SAMPLES+1u);
const SHADE_BASE=DOT_SAMPLES;
const LIGHTS_BASE=SHADE_BASE+SHADE_DISCS*(SHADE_SAMPLES+1u);
const LIGHTS=POINTS+SHADE_DISCS;
// The mirror images of the trails' samples and the tips', in the same
// order.
const MIRROR_BASE=LIGHTS_BASE+LIGHTS;

// ---- The water ---------------------------------------------------------------------------
// Where it lies at continuous dot frame tau (dots_water.rs, Water): level,
// the camera dc.water_plane.z above it, until the box comes down; then
// turning as the box's centre comes down, eased, until it lies in the
// box's base, tilted by dc.water_plane.x, the camera dc.water_plane.y above
// it. Its up normal, in the dots' camera space (its x is 0, so the same in
// the display's square units), and the camera's height above it.
struct Plane { up:vec3f, height:f32 }
fn water_plane(tau:f32)->Plane {
    var s=0.0;
    let last=dot_frames[u32(max(last_dot_frame(),0.0))].translation.y;
    if tau>=dc.water_plane.w && last>0.0 {
        let k=clamp(dot_state(tau).translation.y/last,0.0,1.0);
        s=k*k*(3.0-2.0*k);
    }
    let a=s*dc.water_plane.x;
    return Plane(vec3f(0.0,-cos(a),-sin(a)),mix(dc.water_plane.z,dc.water_plane.y,s));
}
// The water now: as the dots stand, or level before them.
fn water_now()->Plane {return water_plane(select(-1.0,dot_position(),dots_shown()));}
fn water_height(w:Plane,c:vec3f)->f32 {return dot(c,w.up)+w.height;}
fn mirrored(w:Plane,c:vec3f)->vec3f {return c-2.0*water_height(w,c)*w.up;}
// The share of light it mirrors at reflectance f (see UNDER_REACH).
fn water_mirror(f:f32)->f32 {return 1.0-pow(1.0-clamp(f,0.0,1.0),max(dc.water.x,1.0));}

// ---- The exposure -----------------------------------------------------------------------
// The age of sample k, in frames.
fn sample_age(k:u32)->f32 {
    if k<=TRAIL_SAMPLES {return f32(k)*TRAIL_SPACING;}
    return TRAIL_FRAMES+f32(k-TRAIL_SAMPLES)*AFTER_SPACING;
}
// The exposure from the head to `age` frames back, per unit power.
fn exposure_to(age:f32)->f32 {
    return HEAD_EXPOSURE*(1.0-exp(-age/HEAD_DECAY))+TRAIL_EXPOSURE*TRAIL_DECAY*(1.0-exp(-age/TRAIL_DECAY))
        +AFTER_EXPOSURE*AFTER_DECAY*(1.0-exp(-age/AFTER_DECAY));
}
// The PCM's low band `age` frames (70 Hz ticks) ago.
fn low_at(age:f32)->f32 {
    let a=clamp(age,0.0,39.0);
    let k=u32(floor(a));
    let n=min(k+1u,39u);
    return mix(dc.low[k/4u][k%4u],dc.low[n/4u][n%4u],a-f32(k));
}
// The waves from the orders' first rows at object point p, `age` frames
// ago: each a plane sweeping through the object along a direction of its
// own, picked by the hit's time.
fn waves(p:vec3f,age:f32)->f32 {
    var w=0.0;
    for(var h=0u;h<4u;h++) {
        let t=dc.hits[h]-age/70.0;
        if t<0.0 || t>WAVE_LIFE {continue;}
        let at=dots_seconds()-dc.hits[h];
        let a=vec2f(hash2(vec2f(at,1.7)),hash2(vec2f(at,9.1)));
        let z=2.0*a.x-1.0;
        let dir=vec3f(sqrt(1.0-z*z)*vec2f(cos(2.0*PI*a.y),sin(2.0*PI*a.y)),z);
        let d=(dot(p,dir)-WAVE_START-WAVE_SPEED*t)/WAVE_WIDTH;
        w+=dc.hit_strength[h]*exp(-d*d)*smoothstep(WAVE_LIFE,.6*WAVE_LIFE,t);
    }
    return WAVE_GAIN*w;
}
// The flare as part 4 nears, 0 to 1, `age` frames ago.
fn flare(age:f32)->f32 {
    let until=dc.end.y+age/70.0;
    if slab_shown() {return 1.0;}
    let x=clamp(1.0-until/FLARE_LEAD,0.0,1.0);
    return x*x*x;
}

// ---- The samples (compute) -----------------------------------------------------------
// A light at camera point c through the lens focused at depth `focus`, of
// radius r0 in focus: where it shows (scene), its disc's radius (rows) and
// its depth.
fn light_disc(c:vec3f,focus:f32,r0:f32)->vec4f {
    let coc=min(APERTURE*FOCAL*abs(1.0/max(c.z,1.0)-1.0/max(focus,1.0))*.5,COC_MAX);
    return vec4f(scene_of_source(project(c)),max(sqrt(r0*r0+coc*coc),.7*pixel_rows()),c.z);
}
fn dot_sample(n:u32,i:u32,k:u32) {
    let now=dot_position();
    let alive=dots_shown() && now>=0.0;
    let wanted=sample_age(k);
    // The trail can't reach back before the first dot frame.
    let tau=max(now-wanted,0.0);
    let age=now-tau;
    let s=dot_state(tau);
    let obj=object_point(s.stage,i,s.morph);
    let c=rotate(rotation(s.angles),obj)+s.translation;
    let fl=flare(age);
    let r0=POINT_RADIUS*(1.0+FLARE_SWELL*fl);
    let end=1.0-smoothstep(.7,1.0,age/sample_age(SAMPLES));
    // The water as it lay then: a light under it fades, and only one above
    // it has a mirror image.
    let w=water_plane(tau);
    let h=water_height(w,c);
    let under=exp(-max(-h,0.0)/UNDER_REACH);
    // The waves light the lights and their trails, not the air.
    let steady=POWER*(BREATH.x+BREATH.y*low_at(age))*(1.0+FLARE_GAIN*fl)*end;
    let power=steady*(1.0+waves(obj,age));
    let b=vec4f(power*under,exposure_to(age),age,select(0.0,1.0,alive));
    let focus=s.translation.z;
    samples_out[n]=Sample(light_disc(c,focus,r0),b);
    samples_out[MIRROR_BASE+n]=Sample(light_disc(mirrored(w,c),focus,r0),vec4f(power*smoothstep(-.5,.5,h),b.yzw));
    if k==0u {
        samples_out[LIGHTS_BASE+i]=Sample(vec4f(c,select(0.0,steady*under,alive)),vec4f(0));
    }
}
fn shade_sample(n:u32,t:u32,k:u32) {
    // The newest iteration drawn, continuous, and the window the original
    // keeps: 16 iterations back, none before the first or from the stop.
    let head=dc.shades.x-1.0;
    let lo=max(head-SHADE_KEPT,0.0);
    let hi=min(head,shade_stop());
    let alive=shades_shown() && hi>lo;
    let g=clamp(head-f32(k)*SHADE_KEPT/f32(SHADE_SAMPLES),lo,max(hi,lo));
    let q=scene_of_source(shade_tip(g,t));
    let b=vec4f(1.0,g,head-g,select(0.0,1.0,alive));
    samples_out[n]=Sample(vec4f(q,SHADE_ROWS,SHADE_DEPTH),b);
    // The tip in the camera's space, and its mirror image: smaller, as it
    // is further, its glow as bright (a in b, its share of the count, is
    // 0 for a tip under the water).
    let s=source_of_scene(q);
    let c=vec3f((s-vec2f(160.0,100.0))*SHADE_DEPTH/FOCAL,SHADE_DEPTH);
    let w=water_plane(-1.0);
    let m=mirrored(w,c);
    let above=smoothstep(-.5,.5,water_height(w,c));
    samples_out[MIRROR_BASE+n]=Sample(vec4f(scene_of_source(project(m)),SHADE_ROWS*SHADE_DEPTH/m.z,m.z),vec4f(above,b.yzw));
    if k==0u {
        let lit=select(0.0,TIP_LIGHT*clamp((hi-lo)/4.0,0.0,1.0),alive);
        samples_out[LIGHTS_BASE+POINTS+t]=Sample(vec4f(c,lit),vec4f(0));
    }
}
@compute @workgroup_size(64) fn dots_samples(@builtin(global_invocation_id) id:vec3u) {
    dots_begin();
    let n=id.x;
    if n<DOT_SAMPLES {
        dot_sample(n,n/(SAMPLES+1u),n%(SAMPLES+1u));
    } else if n<LIGHTS_BASE {
        let m=n-SHADE_BASE;
        shade_sample(n,m/(SHADE_SAMPLES+1u),m%(SHADE_SAMPLES+1u));
    }
}

// ---- The light painted -----------------------------------------------------------------
// A disc of radius r swept from a to b: each fragment's offset along the
// segment and across it (rows), the segment's length and the radius, and
// the light: rgb, and in a the shade count.
struct Swept {
    @builtin(position) position:vec4f,
    @location(0) local:vec2f,
    @location(1) shape:vec2f,
    @location(2) light:vec4f,
    // The lens's shape, when it shows (0 for a round disc): the segment's
    // direction (a unit vector) and the barrel's offset (rows, along the
    // segment and across it).
    @location(3) lens:vec4f,
}
fn no_sweep()->Swept {return Swept(vec4f(-2,-2,0,1),vec2f(0),vec2f(0,1),vec4f(0),vec4f(0));}
// The lens's shape for a disc of light of radius r whose segment runs from
// a to b: none for a small disc, else the diaphragm turned into the
// segment's frame and the barrel's offset, toward the picture's centre.
fn lens_shape(a:vec2f,b:vec2f,r:f32)->vec4f {
    if r<SHAPED*pixel_rows() {return vec4f(0);}
    let d=b-a;
    let axis=select(vec2f(1,0),normalize(d),length(d)>1e-5);
    let barrel=-.5*(a+b)/100.0*CAT_EYE*r;
    return vec4f(axis,dot(barrel,axis),dot(barrel,vec2f(-axis.y,axis.x)));
}
fn swept_quad(vi:u32,a:vec2f,b:vec2f,r:f32,light:vec4f,lens:vec4f)->Swept {
    let px=pixel_rows();
    let d=b-a;
    let l=length(d);
    let axis=select(vec2f(1,0),d/max(l,1e-6),l>1e-5);
    let perp=vec2f(-axis.y,axis.x);
    let m=r+1.5*px;
    let corner=array<vec2f,6>(vec2f(0,-1),vec2f(1,-1),vec2f(1,1),vec2f(0,-1),vec2f(1,1),vec2f(0,1))[vi];
    let along=mix(-m,l+m,corner.x);
    let across=corner.y*m;
    return Swept(clip_of_scene(a+axis*along+perp*across),vec2f(along,across),vec2f(l,r),light,lens);
}
// The offsets w along the segment for which (w, v) is within the lens's
// shape of radius r: the diaphragm, rounded, and the barrel's disc. Empty
// when x > y.
fn aperture_span(v:f32,r:f32,lens:vec4f)->vec2f {
    let round=r*ROUNDING;
    let k=round*round-v*v;
    let dv=v-lens.w;
    let kb=r*r-dv*dv;
    if k<0.0 || kb<0.0 {return vec2f(1,-1);}
    var lo=max(-sqrt(k),lens.z-sqrt(kb));
    var hi=min(sqrt(k),lens.z+sqrt(kb));
    let h=r*APOTHEM;
    for(var i=0u;i<BLADES;i++) {
        // The blade's normal in the segment's frame.
        let b=BLADE_NORMALS[i];
        let n=vec2f(b.x*lens.x+b.y*lens.y,b.y*lens.x-b.x*lens.y);
        let rhs=h-v*n.y;
        if abs(n.x)<1e-5 {
            if rhs<0.0 {return vec2f(1,-1);}
            continue;
        }
        if n.x>0.0 {hi=min(hi,rhs/n.x);} else {lo=max(lo,rhs/n.x);}
    }
    return vec2f(lo,hi);
}
// The share of a segment of length l within the lens's shape of the
// fragment at (u, v).
fn shaped_share(u:f32,v:f32,l:f32,r:f32,lens:vec4f)->f32 {
    let span=aperture_span(v,r,lens);
    if span.x>span.y {return 0.0;}
    if l<=1e-4 {return select(0.0,1.0,u>=span.x && u<=span.y);}
    return max(min(u-span.x,l)-max(u-span.y,0.0),0.0)/l;
}
// Light in rows^-2 per unit of the swept light: the share of the segment
// within the lens's disc of the fragment (exact for a uniform disc at
// constant speed), its edge antialiased; for a large disc, the lens's own
// shape, sampled four times across the pixel.
@fragment fn swept(s:Swept)->@location(0) vec4f {
    let px=pixel_rows();
    let l=s.shape.x;
    let u=s.local.x;
    if dot(s.lens.xy,s.lens.xy)>.5 {
        var share=0.0;
        for(var k=0u;k<4u;k++) {
            let o=(vec2f(f32(k&1u),f32(k>>1u))-.5)*.5*px;
            share+=shaped_share(u+o.x,s.local.y+o.y,l,s.shape.y,s.lens);
        }
        return s.light*share/4.0;
    }
    let ro=s.shape.y+.5*px;
    let v=abs(s.local.y);
    let c=sqrt(max(ro*ro-v*v,0.0));
    var share=select(0.0,1.0,abs(u)<c);
    if l>1e-4 {share=max(min(u+c,l)-max(u-c,0.0),0.0)/l;}
    let edge=clamp((ro-v)/px,0.0,1.0);
    return s.light*share*edge;
}
// Segment k of light i's trail: its exposure, coloured by its age, spread
// over the disc's area; from the samples at `base` (0, or MIRROR_BASE for
// its mirror image).
fn trail(vi:u32,ii:u32,base:u32)->Swept {
    let i=ii/SAMPLES;
    let k=ii%SAMPLES;
    let n=base+i*(SAMPLES+1u)+k;
    let a=samples[n];
    let b=samples[n+1u];
    if a.b.w<.5 {return no_sweep();}
    let energy=.5*(a.b.x+b.b.x)*(b.b.y-a.b.y);
    if energy<=1e-6 {return no_sweep();}
    let r=.5*(a.a.z+b.a.z);
    let age=.5*(a.b.z+b.b.z);
    let colour=mix(OLD,YOUNG,exp(-age/AGE_COOL));
    let lens=select(vec4f(0),lens_shape(a.a.xy,b.a.xy,r),a.b.z<SHAPED_AGE);
    return swept_quad(vi,a.a.xy,b.a.xy,r,vec4f(colour*energy/(PI*r*r),0),lens);
}
@vertex fn dots_trail(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Swept {
    dots_begin();
    return trail(vi,ii,0u);
}
@vertex fn dots_trail_mirrored(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Swept {
    dots_begin();
    return trail(vi,ii,MIRROR_BASE);
}
// Segment k of shade tip t: its glow, counted in a (one count for each
// iteration's worth swept over a point), and from SHADE_DISCS *
// SHADE_SAMPLES on, its fibre; from the samples at `base` (0, or
// MIRROR_BASE for its mirror image, where the glow's radius gives the
// fibre's scale).
fn shade(vi:u32,ii:u32,base:u32)->Swept {
    let count=SHADE_DISCS*SHADE_SAMPLES;
    let fibre=ii>=count;
    let j=ii%count;
    let t=j/SHADE_SAMPLES;
    let k=j%SHADE_SAMPLES;
    let n=base+SHADE_BASE+t*(SHADE_SAMPLES+1u)+k;
    let a=samples[n];
    let b=samples[n+1u];
    let swept=(a.b.y-b.b.y)*a.b.x;
    if a.b.w<.5 || swept<=1e-6 {return no_sweep();}
    if fibre {
        // Each fibre carries its own share of the light, and is a little
        // thicker or thinner than the next.
        let own=hash2(vec2f(f32(t),3.7));
        let r=max(FIBRE_RADIUS*a.a.z/SHADE_ROWS*(.75+.5*hash2(vec2f(f32(t),8.3))),.7*pixel_rows());
        let age=.5*(a.b.z+b.b.z);
        let light=FIBRE_LIGHT*(.45+1.1*own)*swept*exp(-age/FIBRE_DECAY)*(1.0+3.0*exp(-age/.6))/(PI*r*r);
        return swept_quad(vi,a.a.xy,b.a.xy,r,vec4f(light,0),vec4f(0));
    }
    return swept_quad(vi,a.a.xy,b.a.xy,a.a.z,vec4f(0,0,0,swept),vec4f(0));
}
@vertex fn shade_trail(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Swept {
    dots_begin();
    return shade(vi,ii,0u);
}
@vertex fn shade_trail_mirrored(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Swept {
    dots_begin();
    return shade(vi,ii,MIRROR_BASE);
}

// ---- The haze and the fog ------------------------------------------------------------------------------
// A cell's depth in the grid (0..1) at camera depth z, and back.
fn fog_slice(z:f32)->f32 {return log(z/FOG_NEAR)/log(FOG_FAR/FOG_NEAR);}
fn fog_depth(w:f32)->f32 {return FOG_NEAR*pow(FOG_FAR/FOG_NEAR,w);}
// Camera-space point at depth z on the ray through drawable point uv.
fn fog_point(uv:vec2f,z:f32)->vec3f {
    let s=source_of_scene(scene_of_uv(uv));
    return vec3f((s-vec2f(160.0,100.0))*z/FOCAL,z);
}
// The light every lamp sends to cell id, softened over the cell's size:
// dimmed over FOG_REACH for the fog (rgb), and over HAZE_REACH for the
// haze (a). A cell under the water holds the light at its mirror image
// above it, which the march sees in the water.
@compute @workgroup_size(4,4,4) fn dots_fog_light(@builtin(global_invocation_id) id:vec3u) {
    dots_begin();
    if any(id>=vec3u(FOG_COLUMNS,FOG_ROWS,FOG_SLICES)) {return;}
    let uv=(vec2f(id.xy)+.5)/vec2f(f32(FOG_COLUMNS),f32(FOG_ROWS));
    let z=fog_depth((f32(id.z)+.5)/f32(FOG_SLICES));
    let w=water_now();
    var p=fog_point(uv,z);
    if water_height(w,p)<0.0 {p=mirrored(w,p);}
    let size=z*200.0/(f32(FOG_ROWS)*FOCAL);
    var sum=vec4f(0);
    if dots_shown() {
        for(var i=0u;i<POINTS;i++) {
            let l=samples[LIGHTS_BASE+i].a;
            if l.w<=0.0 {continue;}
            let d=l.xyz-p;
            let dd=dot(d,d);
            let e=sqrt(dd);
            sum+=vec4f(YOUNG*exp(-e/FOG_REACH),exp(-e/HAZE_REACH))*l.w/(dd+size*size);
        }
    }
    if shades_shown() {
        for(var t=0u;t<SHADE_DISCS;t++) {
            let l=samples[LIGHTS_BASE+POINTS+t].a;
            if l.w<=0.0 {continue;}
            let d=l.xyz-p;
            let dd=dot(d,d);
            let e=sqrt(dd);
            sum+=vec4f(vec3f(.3,.45,1.0)*exp(-e/FOG_REACH),exp(-e/HAZE_REACH))*l.w/(dd+size*size);
        }
    }
    textureStore(fog_light_out,id,min(sum,vec4f(6e4)));
}
// Tileable gradient noise of period `period` cells over the unit cube.
fn noise_hash(p:vec3u,seed:u32)->u32 {
    var h=p.x*0x8da6b343u^p.y*0xd8163841u^p.z*0xcb1ab31fu^seed*0x9e3779b9u;
    h^=h>>16u;h*=0x7feb352du;h^=h>>15u;h*=0x846ca68bu;h^=h>>16u;
    return h;
}
fn noise_gradient(h:u32)->vec3f {
    let a=f32(h&0xffffu)/65535.0*2.0*PI;
    let z=f32(h>>16u)/65535.0*2.0-1.0;
    return vec3f(sqrt(1.0-z*z)*vec2f(cos(a),sin(a)),z);
}
fn tile_noise(x:vec3f,period:u32,seed:u32)->f32 {
    let q=x*f32(period);
    let i=vec3i(floor(q));
    let f=q-floor(q);
    let u=f*f*f*(f*(f*6.0-15.0)+10.0);
    var n=0.0;
    for(var c=0u;c<8u;c++) {
        let o=vec3i(i32(c&1u),i32((c>>1u)&1u),i32((c>>2u)&1u));
        let cell=vec3u((i+o+vec3i(i32(period)))%vec3i(i32(period)));
        let g=noise_gradient(noise_hash(cell,seed));
        let w=select(1.0-u,u,o==vec3i(1));
        n+=dot(g,f-vec3f(o))*w.x*w.y*w.z;
    }
    return n;
}
// Four fbm fields, one a channel, of three octaves each, 0..1.
@compute @workgroup_size(4,4,4) fn dots_fog_noise(@builtin(global_invocation_id) id:vec3u) {
    if any(id>=vec3u(NOISE_SIZE)) {return;}
    let x=(vec3f(id)+.5)/f32(NOISE_SIZE);
    var v=vec4f(0);
    for(var c=0u;c<4u;c++) {
        var n=0.0;
        var a=.6;
        for(var o=0u;o<3u;o++) {
            n+=a*tile_noise(x,4u<<o,c*7u+o);
            a*=.5;
        }
        v[c]=.5+.75*n;
    }
    textureStore(fog_noise_out,id,v);
}
// The fog's density at camera point p, 0..1: wisps of the noise twisted by
// a second noise, drifting.
fn fog_density(p:vec3f,t:f32)->f32 {
    let q=p/FOG_TILE;
    let warp=textureSampleLevel(fog_noise,repeat,q*1.6+FOG_WARP_DRIFT*t/FOG_TILE,0.0).gba*2.0-1.0;
    let n=textureSampleLevel(fog_noise,repeat,q+(FOG_DRIFT*t+FOG_WARP*warp)/FOG_TILE,0.0).r;
    return smoothstep(FOG_LEVEL,FOG_LEVEL+FOG_SOFT,n);
}
// Light scattered towards the camera along the ray through uv by the haze
// and the fog, marched in log depth, the steps offset per pixel. The
// haze's is single scattering in a uniform medium: along the ray, the sum
// of each light's power over its distance squared. Past the water, the
// air's in the mirror image, by the calm water's reflectance there (the
// grid holds its light; the fog's wisps are mirrored too).
fn air_scatter(uv:vec2f,pixel:vec2f)->vec3f {
    let s=source_of_scene(scene_of_uv(uv));
    let ray=vec3f((s-vec2f(160.0,100.0))/FOCAL,1.0);
    let stretch=length(ray);
    let w=water_now();
    let toward=dot(ray,w.up);
    let surface=select(1e9,-w.height/toward,toward<0.0);
    let d=normalize(vec3f(ray.x*COLUMN,ray.yz));
    let reflected=water_mirror(water_fresnel(-dot(d,w.up)));
    // Interleaved gradient noise, which the composite's 3 x 3 filter
    // evens out.
    let jitter=fract(52.9829189*fract(dot(floor(pixel),vec2f(.06711056,.00583715))));
    let t=dots_seconds();
    var fog=vec3f(0);
    var haze=0.0;
    for(var k=0u;k<FOG_STEPS;k++) {
        let w0=(f32(k)+jitter)/f32(FOG_STEPS);
        let light=textureSampleLevel(fog_light,linear,vec3f(uv,w0),0.0);
        if max(max(light.r,light.g),max(light.b,light.a))<FOG_DARK {continue;}
        let z=fog_depth(w0);
        let below=z>surface;
        let share=select(1.0,reflected,below);
        let dz=z*log(FOG_FAR/FOG_NEAR)/f32(FOG_STEPS)*stretch*share;
        haze+=light.a*dz;
        var p=ray*z;
        if below {p=mirrored(w,p);}
        fog+=light.rgb*fog_density(p,t)*dz;
    }
    return FOG_SCATTER*fog+HAZE*HAZE_TINT*haze;
}
@fragment fn dots_haze(v:Vertex)->@location(0) vec4f {
    dots_begin();
    if !dots_shown() && !shades_shown() {return vec4f(0,0,0,1);}
    return vec4f(air_scatter(v.uv,v.position.xy),1);
}

// ---- The composite --------------------------------------------------------------------------
// Part 5's frame as the camera flies at it: blurred along the zoom over the
// exposure and, as it nears, out of focus; gone once it has left the
// drawable.
fn zoom_underlay(uv:vec2f,pixel:vec2i)->vec3f {
    let m=dc.zoom.y;
    let flown=max(screen_aspect()*120.0/64.0,100.0/46.0)*1.15;
    let fade=1.0-smoothstep(.8*flown,flown,m);
    if fade<=0.0 {return vec3f(0);}
    let streak=dc.zoom.z*ZOOM_SHUTTER;
    let defocus=ZOOM_DEFOCUS*smoothstep(1.6,flown,m)*smoothstep(1.6,flown,m);
    if streak<1e-4 && defocus<1e-5 {return textureLoad(underlay,pixel,0).rgb;}
    var c=vec3f(0);
    let taps=12;
    let aspect=vec2f(1.0/screen_aspect(),1.0);
    for(var i=0;i<taps;i++) {
        let f=(f32(i)+.5)/f32(taps);
        // Along the zoom, and round a disc by the golden angle.
        let a=f32(i)*2.39996;
        let ring=sqrt(f)*defocus*vec2f(cos(a),sin(a))*aspect;
        let p=.5+(uv-.5)*(1.0-streak*f)+ring;
        c+=textureSampleLevel(underlay,linear,p,0.0).rgb;
    }
    return c/f32(taps)*fade;
}
// The shade bobs' exposure as the film takes it: its response is the
// original's palette, so through the shared tonemap a count shows the
// palette's colour for it (blue rising to 11, burning to white from 13 to
// 25), and past the palette's white the light keeps rising, into the bloom.
fn shade_light(count:f32)->vec3f {
    let at=(clamp(count,0.0,25.0)+.5)/256.0;
    let film=srgb_decode(textureSampleLevel(shade_palette,linear,vec2f(at,.5),0.0).rgb);
    return tonemap_inverse(min(film,vec3f(.93)))+SHADE_OVER*max(count-25.0,0.0);
}
// The display's square units of a camera point (its x as the display
// shows it), and the drawable point a point in them shows at.
fn square(c:vec3f)->vec3f {return vec3f(c.x*COLUMN,c.yz);}
fn uv_of_square(p:vec3f)->vec2f {
    let q=FOCAL*p.xy/max(p.z,1.0);
    return vec2f(q.x/(200.0*screen_aspect())+.5,.5+q.y/200.0);
}
// The water where the ray through uv meets it: the light's mirror image
// (painted at the mirror images of the lights, so on calm water it shows
// where it is painted), moved by the swell's and the drops' slopes as the
// reflected ray turns, taken at the distance the mirror image of what the
// camera is focused on lies from the water, and spread over the slopes too
// small to draw in the plane of incidence, which draws it out into
// streaks; by the water's reflectance, raised by the mirror's gain.
fn water_light(uv:vec2f)->vec3f {
    let s=source_of_scene(scene_of_uv(uv));
    let d=normalize(vec3f((s.x-160.0)*COLUMN,s.y-100.0,FOCAL));
    let w=water_now();
    let toward=dot(d,w.up);
    if toward>=-1e-4 {return vec3f(0);}
    let reach=-w.height/toward;
    let p=d*reach;
    // The water's own coordinates: across, and back from below the
    // camera, which stays put as the water turns.
    let back=vec3f(0.0,w.up.z,-w.up.y);
    let q=vec2f(p.x,dot(p+w.height*w.up,back));
    var g=water_slope(q,vec4f(dc.water.x,dc.water.y*CALM,dc.water.zw));
    let footprint=reach*200.0/(FOCAL*u.screen.y)/max(-toward,.2);
    g+=drop_slope(q,dc.drops[0],footprint)+drop_slope(q,dc.drops[1],footprint)
        +drop_slope(q,dc.drops[2],footprint)+drop_slope(q,dc.drops[3],footprint)
        +drop_slope(q,dc.drops[4],footprint)+drop_slope(q,dc.drops[5],footprint);
    let n=normalize(w.up-g.x*vec3f(1,0,0)-g.y*back);
    // The mirror image lies about as far beyond the water as the mirror
    // image of what the camera is focused on; water further off than that
    // shows it where the slopes turn the ray back, which is left out: there
    // the calm water's image (none, as nothing lies so far) is taken.
    let subject=select(vec3f(0.0,0.0,SHADE_DEPTH),dc.focus.yzw,dots_shown());
    let image=max(length(square(mirrored(w,subject)))-reach,0.0);
    let away=normalize(d-toward*w.up);
    var c=vec3f(0);
    var weight=0.0;
    for(var j=0;j<5;j++) {
        let x=f32(j-2)*.8;
        let nj=normalize(n+away*WATER_FINE*x);
        let r=reflect(d,nj);
        let rise=dot(r,w.up);
        if rise<=0.0 {continue;}
        let wj=exp(-.5*x*x);
        weight+=wj;
        let at=uv_of_square(p+image*(r-2.0*rise*w.up));
        if any(at<vec2f(0)) || any(at>vec2f(1)) {continue;}
        let m=textureSampleLevel(mirror_image,linear,at,0.0);
        c+=wj*water_mirror(water_fresnel(dot(nj,-d)))*(m.rgb+shade_light(m.a));
    }
    return c/max(weight,1e-4);
}
@fragment fn dots_style(v:Vertex)->@location(0) vec4f {
    dots_begin();
    let pixel=vec2i(v.position.xy);
    var c=vec3f(0);
    if dc.zoom.w>.5 && dots_phase()<3.5 {c=zoom_underlay(v.uv,pixel);}
    let p=textureLoad(painted,pixel,0);
    // The haze at half resolution, softened over its steps' offsets.
    let texel=1.0/vec2f(textureDimensions(haze));
    var air=vec3f(0);
    for(var y=-1;y<=1;y++) {
        for(var x=-1;x<=1;x++) {
            let w=select(.5,1.0,x==0)*select(.5,1.0,y==0);
            air+=w*textureSampleLevel(haze,linear,v.uv+vec2f(f32(x),f32(y))*texel,0.0).rgb;
        }
    }
    var light=p.rgb+shade_light(p.a)+air/4.0+water_light(v.uv);
    if slab_shown() {
        let since=max(dc.end.w,0.0);
        light*=exp(-since/SLAB_HANDOVER);
        let s=source_of_scene(scene_of_uv(v.uv));
        if in_frame(s) {
            // The slab at its own colours through the tonemap, and a flash
            // that fades.
            let white=srgb_decode(textureSampleLevel(slab,linear,s/vec2f(320,200),0.0).rgb);
            light+=tonemap_inverse(min(white,vec3f(.97)))+white*SLAB_FLASH*exp(-since/SLAB_SETTLE);
        }
    }
    return vec4f(max(c+light,vec3f(0)),1);
}
