// Part 5's mountain: the mountain author's file (with field_mountain.rs).
// field_render.rs builds the modern module from the shared controls,
// field_common.wgsl, field_frame.wgsl, field_zoom.wgsl, this file and
// field_style.wgsl. Prefix this file's own names with mountain_. It defines:
//   fn mountain_layer(px:FieldPixel)->FieldLayer   the mountain, in front of
//       the frame (LAYER_MOUNTAIN_Z by default)
//   fn mountain_prepass(px:FieldPixel)->vec4f      the half-resolution
//       prepass, when MOUNTAIN.prepass is set (read it with
//       mountain_prepass_at)
//
// The molten Mandelbrot. What comes from where:
// - Derived: the motion (the level, the camera and the projection's morph,
//   through the shared helpers, and within the exposure from the code's
//   formulas) and the shape (the fitted relief, and near the set the fitted
//   formula, base + slope sqrt(G(c)), evaluated per pixel), whose fall lines
//   the lava follows.
// - Interpretation: reading the projection as 3D, a height unit as
//   MOUNTAIN_K map units.
// - Authored: the lava (its light as a temperature, its flow down the
//   slopes and its crust, the lake's convection), the materials, the
//   lighting, the exposure and the detail method. Nothing of the lava's
//   look or motion is from the original, which doesn't move its terrain's
//   colours.
//
// The detail method (authored). Rays march the baked relief, skipping
// space by its coarse cells, and in the band round the set, where the
// relief's texels can't hold the boundary, the fitted formula itself: the
// potential and its gradient by direct iteration, stepped by a bound from
// the potential, so that the plateau's edge, the filaments' ridges and
// their normals are as sharp as the pixel at any distance. The prepass
// marches every ray at half resolution, at the middle of the exposure, and
// keeps where it first came within a pixel and a half of the surface or
// entered the band, with its hit and coverage. Each full-resolution pixel
// marches on from there. Shading evaluates the formula again, with the
// set's external angle, along which the lava runs down. Where the surface
// moves across the screen within the exposure, more samples are taken along
// it, each what the prepass saw where its ray points at the middle of the
// exposure, shaded with the middle's surface carried over to first order
// and its detail smoothed along the motion only.

// Map units a height unit, to light the projection as 3D: at full
// perspective (q = 248) a height unit shows 62 / z rows and a map unit
// across 256 * 5/6 / z rows, so a height unit looks like 0.29 map units
// (derived from the code's projection; the 3D reading is interpretation).
const MOUNTAIN_K=0.29;
// The most samples across the exposure, and the screen motion, in target
// pixels, each sample covers (authored). Each sample's detail is smoothed
// along the motion over the samples' spacing, so a few suffice.
const MOUNTAIN_SAMPLES=4;
const MOUNTAIN_SAMPLE_PIXELS=4.5;
// Where the prepass stops: the ray's margin, in its pixels, above which it
// counts as near the mountain (authored).
const MOUNTAIN_APPROACH=-1.5;
// A prepass texel whose ray came near nothing.
const MOUNTAIN_NONE=1e4;
// Where the rays start: depth 4, where the code's projection table starts,
// rather than the original's 64 (NEAR). Close to the mountain the original
// smears the terrain at depth 64 down the columns below it; the modern view
// shows the nearer slopes there instead (authored).
const MOUNTAIN_NEAR=4.0;

fn mountain_rates()->vec4f {return fc.mountain_style[2];}
fn mountain_shutter()->f32 {return fc.mountain_style[2].w;}
// The plateau's height: the height `prepare` makes of DEP.U's highest
// value, 244 with the imported data.
fn mountain_top()->f32 {return height_of_dep(mandel_plateau());}

// ---- The camera within the exposure --------------------------------------------
// The block's camera moved by dt seconds by the code's formulas (as
// FieldPreview::camera_at reads them, with sin for the code's sine table):
// the orbit's angle 10 / 1024 of a turn a step, its radius 70 + sin(8 step)
// / 12 and the camera 2.56 radii from the map's centre, looking at it; the
// height 3 radius + 470 - 5 level; and the projection's q. The rates come
// from field_mountain.rs.
struct MountainCamera { map:vec2f, fwd:vec2f, right:vec2f, height:f32, q:f32 }
fn mountain_camera(dt:f32)->MountainCamera {
    var c=MountainCamera(camera_map(),camera_forward(),camera_right(),camera_height(),projection_q());
    if dt==0.0 {return c;}
    let r=mountain_rates();
    let k=2.0*PI/1024.0;
    let tf0=terrain_frame();
    let tf1=tf0+r.x*dt;
    let a0=camera_angle();
    let a1=a0+10.0*r.x*dt*k;
    let radius0=70.0+256.0/12.0*sin(8.0*tf0*k);
    let radius1=70.0+256.0/12.0*sin(8.0*tf1*k);
    let s0=vec2f(sin(a0),cos(a0));
    let s1=vec2f(sin(a1),cos(a1));
    c.map+=-2.56*(radius1*s1-radius0*s0);
    c.fwd+=s1-s0;
    c.right=vec2f(c.fwd.y,-c.fwd.x);
    c.height+=3.0*(radius1-radius0)-5.0*r.y*dt;
    c.q=clamp(c.q+r.z*dt,0.0,248.0);
    return c;
}
fn mountain_projection(c:MountainCamera,z:f32)->f32 {return c.q*256.0/max(z,1.0)+512.0-2.0*c.q;}
// The source row at which height h shows at depth z, as terrain_row.
fn mountain_row(c:MountainCamera,z:f32,h:f32)->f32 {return (c.height-h)*mountain_projection(c,z)/1024.0;}
// The height a source row's ray is at, at depth z.
fn mountain_ray_height(c:MountainCamera,z:f32,y:f32)->f32 {return c.height-1024.0*y/mountain_projection(c,z);}
fn mountain_dir(c:MountainCamera,column:f32)->vec2f {return c.fwd+(column-COLUMN_CENTRE)/256.0*c.right;}
// Where map point m at height h shows: source x and y, and its depth.
fn mountain_project(c:MountainCamera,m:vec2f,h:f32)->vec3f {
    let d=m-c.map;
    let z=max(dot(d,c.fwd)/dot(c.fwd,c.fwd),1e-3);
    let lateral=dot(d,c.right)/dot(c.right,c.right);
    return vec3f(COLUMN_CENTRE+256.0*lateral/z,mountain_row(c,z,h),z);
}
// The screen's uv for a source point.
fn mountain_uv(s:vec2f)->vec2f {
    let xy=scene_of_source(s)/100.0;
    return vec2f(xy.x/(2.0*screen_aspect())+.5,.5-.5*xy.y);
}

// ---- The fitted surface, exactly -----------------------------------------------
// The fitted height at map point m by direct iteration (as MandelFit::dep and
// Relief::texel, in f32): the plateau for c in the main cardioid or the
// period-2 bulb, or bounded after SET_ITERATIONS (an orbit that comes back
// to where it was, within 1e-5, is bounded, like in_set's periodicity check);
// outside, from the potential G at |z|^2 above BAILOUT, base + slope
// sqrt(G), with its gradient per map unit from the derivative, and the
// distance estimate G / (2 |grad G|) in map units, which the true distance to
// the set is at least (Koebe's quarter theorem, for small G). The
// derivative is carried as dz / 2^n, which stays in range.
struct MountainExact { height:f32, grad:vec2f, inside:bool, g:f32, de:f32 }
// The orbit for at most `cap` iterations. Bounded for SET_ITERATIONS, the
// plateau; still short of BAILOUT at `cap`, the potential is below
// MOUNTAIN_ORBIT_BOUND / 2^cap (|z| < 1e4 there, and the dynamic Green's
// function is at most log |z| plus about log (1 + |c|)), so the slope at
// least that high: `inside` with g that bound. While bounded it checks for
// escape every 4 iterations, which decides the plateau as in_set does (an
// orbit past 2 never comes back, and SET_ITERATIONS is a multiple of 4);
// an escaping orbit runs on to BAILOUT, possibly a few iterations past it,
// which changes G by about |c| / |z|^2 relatively, under 1e-8.
const MOUNTAIN_ORBIT_BOUND=11.0;
fn mountain_orbit(m:vec2f,cap:i32)->MountainExact {
    var e=MountainExact(mountain_top(),vec2f(0),true,0.0,0.0);
    if any(m<vec2f(0)) || any(m>vec2f(MAP_SIZE)) {return MountainExact(0.0,vec2f(0),false,1.0,64.0);}
    let c=mandel_c(m);
    let q=(c.x-.25)*(c.x-.25)+c.y*c.y;
    if q*(q+c.x-.25)<=.25*c.y*c.y || (c.x+1.0)*(c.x+1.0)+c.y*c.y<=.0625 {return e;}
    var z=vec2f(0);
    var w=vec2f(0);
    var inverse=1.0;
    var kept=vec2f(1e9);
    var n=0;
    let limit=min(cap,SET_ITERATIONS);
    loop {
        for(var k=0;k<4;k++) {
            inverse*=.5;
            w=vec2f(z.x*w.x-z.y*w.y,z.x*w.y+z.y*w.x)+vec2f(inverse,0.0);
            z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
        }
        n+=4;
        if dot(z,z)>4.0 {break;}
        if n>=limit {
            if cap<SET_ITERATIONS {e.g=MOUNTAIN_ORBIT_BOUND*inverse;}
            return e;
        }
        let back=z-kept;
        if dot(back,back)<1e-10 {return e;}
        if (n&(n-1))==0 {kept=z;}
    }
    var m2=dot(z,z);
    for(var k=0;k<12;k++) {
        if m2>BAILOUT {break;}
        inverse*=.5;
        w=vec2f(z.x*w.x-z.y*w.y,z.x*w.y+z.y*w.x)+vec2f(inverse,0.0);
        z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
        m2=dot(z,z);
    }
    let g=.5*log(m2)*inverse;
    // grad G = (Re, -Im) of (dz / z) / 2^n.
    let wz=vec2f(w.x*z.x+w.y*z.y,w.y*z.x-w.x*z.y)/m2;
    let s=mandel_scale();
    let per_map=vec2f(-wz.y/s,wz.x/(s*mandel_aspect()));
    e.inside=false;
    e.g=g;
    e.de=min(.5*g/max(length(per_map),1e-30),64.0);
    let root=sqrt(max(g,1e-30));
    let dep=mandel_base()+mandel_slope()*root;
    if dep<=128.0 {e.height=0.0;return e;}
    e.height=2.0*(dep-128.0);
    e.grad=mandel_slope()/root*per_map;
    return e;
}
// The iterations that settle whether the potential is below `need`, for
// the march: at most MOUNTAIN_MARCH_ITERATIONS, past which G is below
// 11 / 2^32, 3e-9, and the slope within 0.03 height units of the rim, at
// a distance from the set far under a pixel. Shading decides the plateau
// with the full SET_ITERATIONS.
const MOUNTAIN_MARCH_ITERATIONS=32;
fn mountain_cap(need:f32)->i32 {
    if need<=0.0 {return MOUNTAIN_MARCH_ITERATIONS;}
    return clamp(i32(ceil(log2(MOUNTAIN_ORBIT_BOUND/need))),4,MOUNTAIN_MARCH_ITERATIONS);
}
// The potential below which the slopes stand at least height h:
// base + slope sqrt(G) >= h / 2 + 128. Negative where no slope reaches h.
fn mountain_potential_at(h:f32)->f32 {
    let root=(mandel_base()-128.0-.5*h)/(-mandel_slope());
    return select(-1.0,root*root,root>0.0);
}

// ---- The march -------------------------------------------------------------------
// The band round the set where the march leaves the relief for the formula:
// within MOUNTAIN_BAND map units of the set by the relief's distance, where
// the relief is near the top and below the plateau, for rays between
// MOUNTAIN_BAND_LOW and the plateau. Outside it the relief is within half a
// height unit of the formula (sampled on the imported data's fit).
const MOUNTAIN_BAND=.15;
const MOUNTAIN_BAND_LOW=222.0;
// How fast the relief can rise outside the band, in height units a map unit,
// for its steps (the fitted slopes' steepest there is about 50).
const MOUNTAIN_RELIEF_SLOPE=64.0;
// The most steps a ray takes through the formula, half of them in the
// prepass: past them (rays that graze the crests for long) it goes on
// through the relief.
const MOUNTAIN_FORMULA_STEPS=20;
// What the march meets at a point: the height, and a step in map units that
// the surface can't rise to the ray within.
struct MountainStep { height:f32, step:f32, formula:bool, band:bool }
// mode 0: the relief. mode 1: in the band, the formula, stepping by a bound
// from its potential: within the distance estimate d of a point outside the
// set, the potential is at least G (d - r) / (d + r) at a distance r
// (Harnack's inequality for the positive harmonic G), so the ray, needing
// G* at its height, is safe for r = d (G - G*) / (G + G*); between that
// and four times it, a Newton step towards G*. `band`: the point is in the
// band (in either mode).
fn mountain_surface(p:vec2f,ray_h:f32,drop:f32,along:vec2f,mode:i32,finest:f32)->MountainStep {
    let r=relief_at(p);
    let top=mountain_top();
    let band=r.x<top-.01 && r.w<MOUNTAIN_BAND && r.x>MOUNTAIN_BAND_LOW-12.0 && ray_h<top+.5 && ray_h>MOUNTAIN_BAND_LOW;
    if !band || mode==0 {
        // The relief: free to step until the gap could close, but not past
        // the band.
        var step=clamp((ray_h-r.x)/MOUNTAIN_RELIEF_SLOPE,.25,2.0);
        if ray_h<top+.5 && r.x>MOUNTAIN_BAND_LOW*.5 {step=min(step,max(r.w-MOUNTAIN_BAND,.25));}
        return MountainStep(r.x,step,false,band);
    }
    // The potential the ray needs a little further on, where it is lower.
    let need=mountain_potential_at(ray_h-drop);
    let e=mountain_orbit(p,mountain_cap(need));
    if e.inside {return MountainStep(top,finest,true,true);}
    var step=e.de;
    if need>0.0 {
        let safe=e.de*max(e.g-need,0.0)/(e.g+need);
        // Newton towards G = need along the ray (per map unit), kept
        // between the safe step and four times it.
        let per_map=e.grad*sqrt(max(e.g,1e-30))/mandel_slope();
        let rate=dot(per_map,along)-sqrt(need)/(-mandel_slope())*drop/.25;
        step=safe;
        if rate<0.0 {step=clamp((e.g-need)/(-rate),safe,4.0*safe);}
    }
    return MountainStep(e.height,clamp(.9*step,finest,.25),true,true);
}
struct MountainHit { hit:bool, z:f32, map:vec2f, height:f32, best:f32, near:f32 }
// March source point s's ray (column s.x, row s.y; a pixel is fp rows) with
// camera c from depth `start` (and no nearer than where the ray comes down
// to the plateau's height), as terrain_march does: the nearest depth whose
// surface shows at or above the row, through the coarse cells whose highest
// relief could reach it. best is the most the surface covers the
// pixel by, in pixels; map and height are the hit's, or the nearest
// approach's. `near` is where the margin first passes `approach` (negative,
// in pixels) or the ray first enters the band below the plateau: the
// prepass's hint, from which the formula's surface could show.
fn mountain_march(c:MountainCamera,s:vec2f,fp:f32,start:f32,mode:i32,approach:f32)->MountainHit {
    var out=MountainHit(false,MOUNTAIN_NONE,vec2f(0),0.0,-1e9,MOUNTAIN_NONE);
    let y=s.y;
    let dir=mountain_dir(c,s.x);
    let o=c.map;
    let top=mountain_top();
    let floor_margin=min(-.5,approach);
    if (y-mountain_row(c,FAR,top))/fp<floor_margin {return out;}
    var z0=max(start,MOUNTAIN_NEAR);
    // Nothing stands above the plateau: start where the ray comes down to
    // it, at q 256 / (P - 512 + 2 q) for P = 1024 y / (height - top).
    if c.q>0.0 && c.height>top {
        let p=1024.0*y/(c.height-top);
        let d=p-512.0+2.0*c.q;
        if d>0.0 {z0=max(z0,c.q*256.0/d-1.0);}
    }
    var z1=FAR;
    if abs(dir.x)>1e-6 {
        let a=-o.x/dir.x;let b=(MAP_SIZE-o.x)/dir.x;
        z0=max(z0,min(a,b));z1=min(z1,max(a,b));
    } else if o.x<0.0 || o.x>MAP_SIZE {return out;}
    if abs(dir.y)>1e-6 {
        let a=-o.y/dir.y;let b=(MAP_SIZE-o.y)/dir.y;
        z0=max(z0,min(a,b));z1=min(z1,max(a,b));
    } else if o.y<0.0 || o.y>MAP_SIZE {return out;}
    if z0>=z1 {return out;}
    let unit=1.0/max(length(dir),1e-3);
    let along=dir*unit;
    var z=z0;
    var previous=z0;
    var previous_margin=-1e9;
    var cells=0;
    var after=0;
    var steps=0;
    var formula=0;
    var use_mode=mode;
    loop {
        if z>=z1 || cells>80 || out.best>=.5 || after>2 || steps>160 {break;}
        cells++;
        let m=o+dir*z;
        let cell=vec2i(floor(m/COARSE_CELL));
        let lo=vec2f(cell)*COARSE_CELL;
        var exit=z1;
        if abs(dir.x)>1e-6 {exit=min(exit,(select(lo.x,lo.x+COARSE_CELL,dir.x>0.0)-o.x)/dir.x);}
        if abs(dir.y)>1e-6 {exit=min(exit,(select(lo.y,lo.y+COARSE_CELL,dir.y>0.0)-o.y)/dir.y);}
        exit=max(exit,z+1e-3);
        var peak=coarse_height(cell,true);
        // In the band the formula's ridges can stand above the relief.
        if peak>MOUNTAIN_BAND_LOW {peak=max(peak,top);}
        let reach=(y-mountain_row(c,exit,peak))/fp;
        if peak<.5 || reach<max(out.best,floor_margin) {
            z=exit;previous=z;previous_margin=-1e9;
            continue;
        }
        loop {
            if z>=exit || out.best>=.5 || after>2 || steps>160 {break;}
            steps++;
            let p=o+dir*z;
            let finest=max(.5*z/256.0*fp/COLUMN,.002);
            let ray_h=mountain_ray_height(c,z,y);
            let drop=ray_h-mountain_ray_height(c,z+.25*unit,y);
            if use_mode==1 && formula>=select(MOUNTAIN_FORMULA_STEPS,MOUNTAIN_FORMULA_STEPS/2,approach<0.0) {use_mode=0;}
            let sv=mountain_surface(p,ray_h,drop,along,use_mode,finest);
            if sv.formula {formula++;}
            var margin=-1e9;
            if sv.height>=.5 {margin=(y-mountain_row(c,z,sv.height))/fp;}
            if margin>out.best {
                out.best=margin;
                if !out.hit {out.z=z;out.map=p;out.height=sv.height;}
            }
            if out.near>=MOUNTAIN_NONE && (margin>=approach || sv.band) {out.near=z;}
            if out.hit {after++;}
            if !out.hit && margin>=0.0 {
                out.hit=true;
                var a=previous;var b=z;var hb=sv.height;
                if previous_margin<0.0 && previous<z {
                    for(var k=0;k<4;k++) {
                        let mid=.5*(a+b);
                        let hm=mountain_surface(o+dir*mid,mountain_ray_height(c,mid,y),0.0,along,use_mode,finest).height;
                        if hm>=.5 && y>=mountain_row(c,mid,hm) {b=mid;hb=hm;} else {a=mid;}
                    }
                }
                out.z=b;out.map=o+dir*b;out.height=hb;
            }
            previous=z;previous_margin=margin;
            z+=sv.step*unit;
        }
        z=max(z,exit);
    }
    return out;
}
// The prepass's hint for a source point: the nearest depth any of the four
// texels round it came near the mountain at.
fn mountain_hint(s:vec2f)->f32 {
    let size=vec2i(textureDimensions(prepass_mountain));
    let f=mountain_uv(s)*vec2f(size)-.5;
    let i=vec2i(floor(f));
    var z=MOUNTAIN_NONE;
    for(var k=0;k<4;k++) {
        let t=clamp(i+vec2i(k&1,k>>1),vec2i(0),size-1);
        z=min(z,textureLoad(prepass_mountain,t,0).r);
    }
    return z;
}

// ---- Noise (authored) -------------------------------------------------------------
fn mountain_hashu(p:vec2i,seed:u32)->u32 {
    var h=u32(p.x)*0x8da6b343u^u32(p.y)*0xd8163841u^seed*0xcb1ab31fu;
    h^=h>>16u;h*=0x7feb352du;h^=h>>15u;h*=0x846ca68bu;h^=h>>16u;
    return h;
}
fn mountain_hash(p:vec2i,seed:u32)->f32 {return f32(mountain_hashu(p,seed)&0xffffffu)/16777216.0;}
fn mountain_hash2(p:vec2i,seed:u32)->vec2f {
    let h=mountain_hashu(p,seed);
    return vec2f(f32(h&0xffffu),f32(h>>16u))/65536.0;
}
// Gradient noise, about -1 to 1, repeating every `period` cells along x
// (none for 0).
fn mountain_corner(i:vec2i,o:vec2i,f:vec2f,seed:u32,period:i32)->f32 {
    var g=i+o;
    if period>0 {g.x=((g.x%period)+period)%period;}
    let r=mountain_hash2(g,seed)*2.0-1.0;
    return dot(r,f-vec2f(o));
}
fn mountain_gnoise(p:vec2f,seed:u32,period:i32)->f32 {
    let i=vec2i(floor(p));let f=p-floor(p);
    let u=f*f*f*(f*(f*6.0-15.0)+10.0);
    let a=mountain_corner(i,vec2i(0,0),f,seed,period);
    let b=mountain_corner(i,vec2i(1,0),f,seed,period);
    let c=mountain_corner(i,vec2i(0,1),f,seed,period);
    let d=mountain_corner(i,vec2i(1,1),f,seed,period);
    return 1.9*mix(mix(a,b,u.x),mix(c,d,u.x),u.y);
}
// Octaves of gradient noise, 0 to 1 about .5; `period` as above, doubling
// with each octave so that the sum repeats too.
fn mountain_fbm(p:vec2f,seed:u32,octaves:i32,period:i32)->f32 {
    var sum=0.0;var amp=.5;var q=p;var norm=0.0;var per=period;
    for(var o=0;o<octaves;o++) {
        sum+=amp*mountain_gnoise(q,seed+u32(o)*7u,per);
        norm+=amp;amp*=.5;q=q*2.0+vec2f(0.0,17.3);per*=2;
    }
    return .5+.5*sum/norm;
}
// Gradient noise with its gradient, (value, d/dx, d/dy), as mountain_gnoise
// (the quintic fade's derivative), so that bumps need no extra samples.
fn mountain_grad(g0:vec2i,seed:u32,period:i32)->vec2f {
    var g=g0;
    if period>0 {g.x=((g.x%period)+period)%period;}
    return mountain_hash2(g,seed)*2.0-1.0;
}
fn mountain_gnoise_d(p:vec2f,seed:u32,period:i32)->vec3f {
    let i=vec2i(floor(p));let f=p-floor(p);
    let u=f*f*f*(f*(f*6.0-15.0)+10.0);
    let du=30.0*f*f*(f*(f-2.0)+1.0);
    let ga=mountain_grad(i,seed,period);
    let gb=mountain_grad(i+vec2i(1,0),seed,period);
    let gc=mountain_grad(i+vec2i(0,1),seed,period);
    let gd=mountain_grad(i+vec2i(1,1),seed,period);
    let va=dot(ga,f);let vb=dot(gb,f-vec2f(1,0));
    let vc=dot(gc,f-vec2f(0,1));let vd=dot(gd,f-vec2f(1,1));
    let k=va-vb-vc+vd;
    let n=va+u.x*(vb-va)+u.y*(vc-va)+u.x*u.y*k;
    let g=ga+u.x*(gb-ga)+u.y*(gc-ga)+u.x*u.y*(ga-gb-gc+gd)+du*vec2f(vb-va+u.y*k,vc-va+u.x*k);
    return 1.9*vec3f(n,g);
}
// Ridged octaves, 0 to 1, sharp creases at the noise's zero lines, with
// their gradient.
fn mountain_ridged_d(p:vec2f,seed:u32,octaves:i32,period:i32)->vec3f {
    var sum=vec3f(0);var amp=.5;var q=p;var norm=0.0;var per=period;var scale=1.0;
    for(var o=0;o<octaves;o++) {
        let n=mountain_gnoise_d(q,seed+u32(o)*5u,per);
        sum+=amp*vec3f(1.0-abs(n.x),-sign(n.x)*scale*n.yz);
        norm+=amp;amp*=.5;q=q*2.1+vec2f(0.0,11.7);per*=2;scale*=2.1;
    }
    return sum/norm;
}
// A smooth wave between -1 and 1 a unit long, cheaper than a sine.
fn mountain_wave(x:vec2f)->vec2f {
    let f=abs(fract(x)-.5)*2.0;
    return (f*f*(3.0-2.0*f))*2.0-1.0;
}
// Cells, for the lake's plates and the crust's cracks: the distance to the
// nearest edge (F2 - F1), the nearest cell's id and its centre. The centres
// wander slowly with t.
struct MountainCell { edge:f32, id:f32, centre:vec2f }
fn mountain_cells(p:vec2f,t:f32,seed:u32)->MountainCell {
    let i=vec2i(floor(p));
    var d1=9.0;var d2=9.0;var id=0.0;var centre=vec2f(0);
    for(var j=-1;j<=1;j++) {
        for(var k=-1;k<=1;k++) {
            let g=i+vec2i(k,j);
            let h=mountain_hash2(g,seed);
            let wander=.22*mountain_wave(vec2f(t*.05,t*.043)+h);
            let cpt=vec2f(g)+.15+.7*h+wander;
            let d=length(p-cpt);
            if d<d1 {d2=d1;d1=d;id=h.x;centre=cpt;} else if d<d2 {d2=d;}
        }
    }
    return MountainCell(d2-d1,id,centre);
}

// ---- The surface for shading ----------------------------------------------------------
// atan2 to within about 1e-5 radians (a minimax polynomial on [0, 1]).
fn mountain_atan2(y:f32,x:f32)->f32 {
    let ax=abs(x);let ay=abs(y);
    let a=min(ax,ay)/max(max(ax,ay),1e-30);
    let s=a*a;
    var r=((-.0464964749*s+.15931422)*s-.327622764)*s*a+a;
    if ay>ax {r=1.57079637-r;}
    if x<0.0 {r=3.14159274-r;}
    return select(r,-r,y<0.0);
}
// The terms of the angle's sum kept: past 16, each adds under 2^-16 of a
// half turn, below the finest channels' width.
const MOUNTAIN_ANGLE_TERMS=16;
// mountain_orbit with SET_ITERATIONS, and the external angle: arg Phi(c) /
// 2 pi, from the product Phi(c) = c prod (1 + c / z_k^2)^(1 / 2^k), z_1 = c
// (principal arguments; the sum is continuous on the slopes, and near the
// set it varies as fast as the set's external rays crowd in). The
// external angle is constant along the slopes' fall lines (the field lines
// of G), so lava channels drawn in it run straight down them. rate: |grad
// G| per map unit, about how fast 2 pi times the angle changes; dc: G's
// gradient in c (real, imaginary).
struct MountainDetail { inside:bool, height:f32, grad:vec2f, g:f32, de:f32, angle:f32, rate:f32, dc:vec2f }
fn mountain_detail(m:vec2f)->MountainDetail {
    let top=mountain_top();
    var e=MountainDetail(true,top,vec2f(0),0.0,0.0,0.0,0.0,vec2f(0));
    if any(m<vec2f(0)) || any(m>vec2f(MAP_SIZE)) {return MountainDetail(false,0.0,vec2f(0),1.0,64.0,0.0,0.0,vec2f(0));}
    let c=mandel_c(m);
    let q=(c.x-.25)*(c.x-.25)+c.y*c.y;
    if q*(q+c.x-.25)<=.25*c.y*c.y || (c.x+1.0)*(c.x+1.0)+c.y*c.y<=.0625 {return e;}
    var z=vec2f(0);
    var w=vec2f(0);
    var inverse=1.0;
    var kept=vec2f(1e9);
    var n=0;
    var angle=atan2(c.y,c.x);
    var escaped=false;
    loop {
        for(var k=0;k<4;k++) {
            inverse*=.5;
            w=vec2f(z.x*w.x-z.y*w.y,z.x*w.y+z.y*w.x)+vec2f(inverse,0.0);
            z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
            if n<MOUNTAIN_ANGLE_TERMS {
                let z2=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y);
                let t=vec2f(1.0,0.0)+vec2f(c.x*z2.x+c.y*z2.y,c.y*z2.x-c.x*z2.y)/max(dot(z2,z2),1e-30);
                angle+=inverse*mountain_atan2(t.y,t.x);
            }
        }
        n+=4;
        if dot(z,z)>4.0 {break;}
        if n>=SET_ITERATIONS {return e;}
        let back=z-kept;
        if dot(back,back)<1e-10 {return e;}
        if (n&(n-1))==0 {kept=z;}
    }
    var m2=dot(z,z);
    for(var k=0;k<12;k++) {
        if m2>BAILOUT {break;}
        inverse*=.5;
        w=vec2f(z.x*w.x-z.y*w.y,z.x*w.y+z.y*w.x)+vec2f(inverse,0.0);
        z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
        if n+k<MOUNTAIN_ANGLE_TERMS {
            let z2=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y);
            let t=vec2f(1.0,0.0)+vec2f(c.x*z2.x+c.y*z2.y,c.y*z2.x-c.x*z2.y)/max(dot(z2,z2),1e-30);
            angle+=inverse*mountain_atan2(t.y,t.x);
        }
        m2=dot(z,z);
    }
    let g=.5*log(m2)*inverse;
    let wz=vec2f(w.x*z.x+w.y*z.y,w.y*z.x-w.x*z.y)/m2;
    let s=mandel_scale();
    let per_map=vec2f(-wz.y/s,wz.x/(s*mandel_aspect()));
    e.inside=false;
    e.g=g;
    e.rate=length(per_map);
    e.dc=vec2f(wz.x,-wz.y);
    e.de=min(.5*g/max(e.rate,1e-30),64.0);
    e.angle=fract(angle/(2.0*PI));
    let root=sqrt(max(g,1e-30));
    let dep=mandel_base()+mandel_slope()*root;
    if dep<=128.0 {e.height=0.0;return e;}
    e.height=2.0*(dep-128.0);
    e.grad=mandel_slope()/root*per_map;
    return e;
}

// ---- Shading -------------------------------------------------------------------------
// The lava's light at temperature tc, in degrees C, as linear HDR radiance
// (authored, after a blackbody's): deep red about 700, orange about 950 and
// yellow-white about 1150, where it is 12, far brighter than anything else
// in the scene, for the shared bloom. Its brightness follows exp(-B / T),
// Wien's approximation to Planck's law, with B 14,080 K, about 0.6 of the
// physical value at 600 nm, so that dull red to white fits the tonemap. Its
// green over its red rises from 0.02 at 700 to 0.35 at 1150 (Planck's law
// gives 0.02 to 0.13 in linear sRGB; the rest stands for a camera's
// saturation), with a little blue above 900.
fn mountain_glow(tc:f32)->vec3f {
    let k=max(tc,0.0)+273.15;
    let r=12.0*exp(-14080.0*(1.0/k-1.0/1423.15));
    let x=clamp((tc-600.0)/550.0,0.0,1.3);
    let y=clamp((tc-900.0)/250.0,0.0,1.3);
    return r*vec3f(1.0,.35*x*sqrt(x),.04*y*y);
}
// The prepared terrain at map point m smoothed by a cubic B-spline (four
// bilinear taps), so that thresholds drawn in it stay round.
fn mountain_terrain_smooth(m:vec2f)->vec4f {
    let x=m.yx-.5;
    let i=floor(x);let f=x-i;
    let f2=f*f;let f3=f2*f;
    let w0=(1.0-3.0*f+3.0*f2-f3)/6.0;
    let w1=(4.0-6.0*f2+3.0*f3)/6.0;
    let w2=(1.0+3.0*f+3.0*f2-3.0*f3)/6.0;
    let w3=f3/6.0;
    let g0=w0+w1;let g1=w2+w3;
    let p0=(i-1.0+w1/g0+.5)/MAP_SIZE;
    let p1=(i+1.0+w3/g1+.5)/MAP_SIZE;
    let a=textureSampleLevel(terrain,linear,vec2f(p0.x,p0.y),0.0);
    let b=textureSampleLevel(terrain,linear,vec2f(p1.x,p0.y),0.0);
    let c=textureSampleLevel(terrain,linear,vec2f(p0.x,p1.y),0.0);
    let d=textureSampleLevel(terrain,linear,vec2f(p1.x,p1.y),0.0);
    return g0.y*(g0.x*a+g1.x*b)+g1.y*(g0.x*c+g1.x*d);
}
// The original's colour at map point m as heat 0..1, and MAP.U's speckle
// under it: `prepare` makes the colour (3 MAP.U + height) / 4 and three-pixel
// averages it, so the speckle is about (4 colour - height) / 3
// (interpretation; the darkening below height 16 is left out).
fn mountain_original(m:vec2f,filtered:bool)->vec2f {
    var t:vec4f;
    if filtered {t=mountain_terrain_smooth(m);} else {t=textureSampleLevel(terrain,linear,m.yx/MAP_SIZE,0.0);}
    let index=t.r*255.0;
    let colour=(index-128.0)*3.0;
    let speck=clamp((4.0*colour-t.g*255.0)/(3.0*238.0),0.0,1.0);
    return vec2f(clamp((index-128.0)/63.0,0.0,1.0),speck);
}
// How far the crust has formed on the lake: none as the mountain rises
// molten out of the zoom's set, fully a second after it has risen
// (authored).
fn mountain_crusting()->f32 {return smoothstep(time_risen()-.4,time_risen()+1.1,field_seconds());}
// When the lake begins to spill over its rim, in part-5 seconds: not while
// the mountain rises, so that the molten set stands out against a dark rim,
// but just before it has risen (authored). Its lava then runs down the
// channels, its front reaching each height as long after this as lava takes
// to fall there.
fn mountain_spill_time()->f32 {return time_risen()-.25;}
fn mountain_spill()->f32 {return smoothstep(mountain_spill_time(),time_risen()+1.0,field_seconds());}
// The camera-relative key: cool and dim, from above and behind the
// camera's left (authored), to show the crust's form. The lava is the main
// light.
fn mountain_key_dir(c:MountainCamera)->vec3f {return normalize(vec3f(-.5*c.right-.45*c.fwd,.8));}
const MOUNTAIN_KEY=vec3f(.55,.62,.75);
// Ambient occlusion and the key's soft shadow at map point m on the relief,
// height h: the horizon in six directions out to five map units, and the
// relief along the key's direction against its ray.
fn mountain_lighting(c:MountainCamera,m:vec2f,h:f32,n:vec3f)->vec2f {
    var occ=0.0;
    for(var k=0;k<6;k++) {
        let a=f32(k)*1.0472+.39;
        let d=vec2f(cos(a),sin(a));
        let r=select(1.5,4.5,(k&1)==1);
        let hs=relief_height(m+d*r);
        let tangent=-dot(n.xy,d)/max(n.z,.25);
        occ+=clamp((hs-h)*MOUNTAIN_K/r-tangent,0.0,1.2);
    }
    let ao=clamp(1.0-occ/4.5,0.0,1.0);
    let l=mountain_key_dir(c);
    var shade=1.0;
    let rise=l.z/max(length(l.xy),1e-3)/MOUNTAIN_K;
    let dl=normalize(l.xy);
    for(var k=1;k<=4;k++) {
        let r=f32(k*k)*1.5;
        let hs=relief_height(m+dl*r);
        shade=min(shade,clamp(1.0-(hs-(h+rise*r))*.15,0.0,1.0));
    }
    return vec2f(ao,shade);
}
// GGX specular for a light (Smith-Schlick visibility), times n.l.
fn mountain_ggx(n:vec3f,v:vec3f,l:vec3f,rough:f32)->f32 {
    let h=normalize(v+l);
    let nh=max(dot(n,h),0.0);let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),1e-3);
    let a=max(rough*rough,1e-3);let a2=a*a;
    let d=a2/(PI*pow(nh*nh*(a2-1.0)+1.0,2.0));
    let k=.5*a;
    let g=nl/(nl*(1.0-k)+k)*nv/(nv*(1.0-k)+k);
    return d*g/(4.0*nv);
}
// The surface at m for shading: the lake where the relief is all plateau,
// else the formula.
fn mountain_detail_at(m:vec2f)->MountainDetail {
    if relief_at(m).x>=mountain_top()-.01 {return MountainDetail(true,mountain_top(),vec2f(0),0.0,0.0,0.0,0.0,vec2f(0));}
    return mountain_detail(m);
}
// The surface at m from the one at m0 by first order, where m is well
// within m0's distance from the set: G and, by Cauchy-Riemann (log Phi is
// analytic), 2 pi times the angle, from G's gradient in c. Elsewhere the
// formula.
fn mountain_detail_near(d0:MountainDetail,m0:vec2f,m:vec2f)->MountainDetail {
    let dm=m-m0;
    if d0.inside || length(dm)>.5*d0.de {return mountain_detail_at(m);}
    let s=mandel_scale();let a=mandel_aspect();
    let gr=d0.dc.x;let gi=d0.dc.y;
    var e=d0;
    e.g=max(d0.g+dot(vec2f(gi/s,gr/(s*a)),dm),1e-30);
    e.angle=fract(d0.angle+dot(vec2f(gr/s,-gi/(s*a)),dm)/(2.0*PI));
    let root=sqrt(e.g);
    let dep=mandel_base()+mandel_slope()*root;
    e.height=select(0.0,2.0*(dep-128.0),dep>128.0);
    e.grad=d0.grad*sqrt(max(d0.g,1e-30))/root;
    return e;
}

// ---- Filtering ------------------------------------------------------------------------
// The pixel's footprint on a surface with normal n seen along v (towards the
// viewer): its two axes in world space (map x, map y and height as
// MOUNTAIN_K map units), across the view and along it, the second longer
// where the view grazes the surface, each widened by `blur` along the
// exposure's motion between its samples (x across the screen, y up it),
// so that a sample is smoothed along the motion only.
struct MountainFootprint { a:vec3f, b:vec3f }
fn mountain_footprint(n:vec3f,v:vec3f,fp_map:f32,blur:vec2f)->MountainFootprint {
    var across=cross(n,v);
    if dot(across,across)<1e-10 {across=vec3f(1,0,0);}
    let a=normalize(across);
    let b=normalize(cross(a,n));
    return MountainFootprint(a*fp_map*blur.x,b*fp_map*blur.y/max(abs(dot(n,v)),.05));
}
// How far a coordinate with map gradient g changes across the footprint.
fn mountain_spans(f:MountainFootprint,g:vec2f)->f32 {return length(vec2f(dot(g,f.a.xy),dot(g,f.b.xy)));}
// How much of a footprint f wide, centred d from a line (d >= 0), a band w
// either side of the line covers: a box filter across a straight band.
fn mountain_band(d:f32,w:f32,f:f32)->f32 {
    let r=.5*max(f,1e-4);
    return clamp((min(d+r,w)-max(d-r,-w))/(2.0*r),0.0,1.0);
}
// A crack network's coverage: cracks w either side of the plates' edges, e
// from the nearest edge and the footprint f across it, both in cells. Past
// about a cell the footprint holds many cracks and the coverage tends to
// their mean: cells of unit density have about 2 units of edge a unit of
// area, so 4 w.
fn mountain_cracks(e:f32,w:f32,f:f32)->f32 {
    return mix(mountain_band(e,w,f),min(4.0*w,1.0),smoothstep(.35,1.1,f));
}
// Plates of crust: the Voronoi cells of a jittered grid. The distance from
// p to the edge between the nearest two centres, in cells, the nearest
// cell's id and centre, and the unit direction across that edge. `period`:
// the cells along x after which it repeats, or 0.
struct MountainPlate { edge:f32, id:f32, centre:vec2f, across:vec2f }
fn mountain_plates(p:vec2f,seed:u32,period:i32)->MountainPlate {
    let i=vec2i(floor(p));
    var d1=9.0;var d2=9.0;var c1=vec2f(0);var c2=vec2f(1);var id=0.0;
    for(var j=-1;j<=1;j++) {
        for(var k=-1;k<=1;k++) {
            let o=i+vec2i(k,j);
            var g=o;
            if period>0 {g.x=((g.x%period)+period)%period;}
            let hu=mountain_hashu(g,seed);
            let cpt=vec2f(o)+.1+.8*vec2f(f32(hu&0xffffu),f32(hu>>16u))/65536.0;
            let e=p-cpt;
            let d=dot(e,e);
            if d<d1 {d2=d1;c2=c1;d1=d;c1=cpt;id=f32((hu*0x9e3779b9u)>>8u)/16777216.0;}
            else if d<d2 {d2=d;c2=cpt;}
        }
    }
    let n=normalize(c2-c1+vec2f(1e-6,0.0));
    return MountainPlate(max(dot(.5*(c1+c2)-p,n),0.0),id,c1,n);
}

// ---- The lake ----------------------------------------------------------------------------
// A convecting lava lake on the plateau (authored). Hot lava wells up along
// a wavering seam down the set's real axis, through the main cardioid and
// the period-2 bulb, and spreads away from it on either side at
// MOUNTAIN_LAKE_SPEED map units a second, so the plates of crust it carries
// are as old as their distance from the seam over that speed: thin and
// glowing, with wide cracks, by the seam; dark and thick, with fine cracks,
// towards the rim, where they founder in a bright margin. Cracks pulse open
// and shut, and now and then gas bursts through, throwing open a disc of
// bright lava that crusts over within a second. Until the crust has formed,
// as the mountain rises, the plates are a bright orange skin.
const MOUNTAIN_LAKE_SPEED=4.0;
// A plate's size, map units.
const MOUNTAIN_LAKE_PLATE=4.2;
// Bursts: cells of this many map units, each with a chance of one burst in
// each period, seconds.
const MOUNTAIN_BURST_CELL=9.0;
const MOUNTAIN_BURST_PERIOD=2.9;
fn mountain_lake(c:MountainCamera,m:vec2f,v:vec3f,fp_map:f32,blur:vec2f,full:bool)->vec3f {
    let t=field_seconds();
    let top=mountain_top();
    let crusting=mountain_crusting();
    // The seam, along map y (the real axis is at map x 100 - im scale).
    let along=m.y;
    let seam=100.0-mandel_centre().y*mandel_scale()+3.5*sin(.16*along+.13*t)+2.0*sin(.37*along-.21*t+1.3);
    let y=m.x-seam;
    let side=select(0u,1u,y<0.0);
    let dist=abs(y);
    let age=dist/MOUNTAIN_LAKE_SPEED;
    // The plates, moving away from the seam on either side.
    let qp=vec2f(along,dist-MOUNTAIN_LAKE_SPEED*t)/MOUNTAIN_LAKE_PLATE;
    let pl=mountain_plates(qp,51u+side,0);
    let fpt=mountain_footprint(vec3f(0,0,1),v,fp_map,blur);
    let qa=fpt.a.yx/MOUNTAIN_LAKE_PLATE;
    let qb=fpt.b.yx/MOUNTAIN_LAKE_PLATE;
    let fq=length(vec2f(dot(pl.across,qa),dot(pl.across,qb)));
    // A plate's cracks open now and then.
    let pulse=smoothstep(.6,1.0,sin(6.2832*(.21*t+3.0*pl.id)));
    let cw=.018+.12*exp(-age/.6)+.05*pulse;
    let crack=mountain_cracks(pl.edge,cw,fq);
    // The seam itself, where the lava wells up, about a map unit wide.
    let seam_w=.55*(.7+.6*mountain_gnoise(vec2f(along*.4,.4*t),52u,0));
    let open_seam=mountain_band(dist,seam_w,max(abs(fpt.a.x),abs(fpt.b.x)));
    // The margin, where the plates founder, by the relief's fall at the
    // set's edge.
    let margin=smoothstep(top-.2,top-6.0,relief_height(m));
    // Temperatures: the plates' skin cools in a few seconds; the cracks
    // and the seam show the hot lava under it.
    let t_plate=mix(1070.0,mix(1040.0,470.0,smoothstep(.15,2.6,age))-50.0*pl.id,crusting);
    let t_crack=1145.0-80.0*smoothstep(.5,7.0,age);
    var e=mix(mountain_glow(t_plate),mountain_glow(t_crack),crack);
    // Finer cracks across the young plates, glowing dull red.
    if full {
        let small=mountain_plates(qp*2.6+vec2f(3.1,7.7),53u,0);
        let fs=2.6*length(vec2f(dot(small.across,qa),dot(small.across,qb)));
        let fine=mountain_cracks(small.edge,.03,fs)*(1.0-smoothstep(.5,3.5,age))*crusting;
        e+=mountain_glow(860.0)*fine*(1.0-crack);
    }
    let churn=mountain_gnoise(m*.35+vec2f(.25*t,-.18*t),54u,0);
    e=mix(e,mountain_glow(1160.0+20.0*churn),open_seam);
    e=mix(e,mountain_glow(1120.0+30.0*churn),margin);
    // Gas bursts: in each cell, now and then one at a random place and time.
    let bc=vec2i(floor(m/MOUNTAIN_BURST_CELL));
    let phase=t/MOUNTAIN_BURST_PERIOD+mountain_hash(bc,61u);
    let slot=i32(floor(phase));
    let hb=mountain_hash2(bc+vec2i(slot*7919,slot*104729),62u);
    let hp=mountain_hash2(bc+vec2i(slot*7919,slot*104729),63u);
    let since=fract(phase)*MOUNTAIN_BURST_PERIOD-.4-1.5*hb.y;
    var burst=0.0;
    if hb.x<.4 && since>0.0 && since<1.1 {
        let centre=(vec2f(bc)+.25+.5*hp)*MOUNTAIN_BURST_CELL;
        let radius=2.6*sqrt(since/.7)+.3;
        let fm=max(length(fpt.a),length(fpt.b));
        let fade=1.0-since/1.1;
        burst=mountain_band(length(m-centre),radius,fm)*fade*crusting;
        e=mix(e,mountain_glow(1190.0-160.0*since),burst);
    }
    // The plates are slightly domed and glossy: the key's highlight, and
    // the glow of the cracks round them.
    let skin=crusting*(1.0-crack)*(1.0-open_seam)*(1.0-margin)*(1.0-burst);
    let dome=(qp-pl.centre)*.35*skin+(vec2f(pl.id,fract(pl.id*7.0))-.5)*.25*skin;
    let nb=normalize(vec3f(-dome.yx,1.0));
    let l=mountain_key_dir(c);
    let fresnel=.04+.96*pow(1.0-max(dot(nb,v),0.0),5.0);
    e+=MOUNTAIN_KEY*mountain_ggx(nb,v,l,.3)*fresnel*1.2*skin;
    e+=vec3f(.045,.038,.035)*mountain_glow(1120.0)*.08*skin;
    return e;
}

// ---- The slopes ---------------------------------------------------------------------------
// Lava runs down the slopes in channels (authored). It falls at a rate
// that depends on the height alone: MOUNTAIN_FALL_FOOT height units a
// second at the foot, rising exponentially to MOUNTAIN_FALL_TOP at the rim,
// as the fitted slopes steepen towards the set (their median steepness
// about triples from the foot to the rim, sampled on the imported data's
// fit). So the time it has taken from the rim is in closed form, and
// anything drawn in that time less the clock moves exactly with the flow,
// without stretching or repeating. Within that, the flow speeds and slows
// in rapids, MOUNTAIN_RAPIDS of the speed every MOUNTAIN_RAPIDS_PERIOD
// seconds of travel, so its crust tears open where it speeds up and
// buckles into ropes where it slows. The channels follow the slopes' fall
// lines, the set's external angle, meandering a little; each has its own
// width and speed. A channel's middle is its hottest and fastest; towards
// its stiff, cooled levees the crust is sheared into streaks. The lava cools
// as it travels, its skin crusting over within a few seconds, so it runs
// open and bright over the rim and down the steep walls, and crusts over
// on the gentler ground below, its cracks still glowing.
const MOUNTAIN_FALL_TOP=42.0;
const MOUNTAIN_FALL_FOOT=9.0;
// ln(MOUNTAIN_FALL_TOP / MOUNTAIN_FALL_FOOT).
const MOUNTAIN_FALL_K=1.5404;
const MOUNTAIN_RAPIDS=.45;
const MOUNTAIN_RAPIDS_PERIOD=3.2;
// Channel cells a turn of the external angle (a multiple of 4), and the
// plates of crust across a cell and along a second of travel.
const MOUNTAIN_CHANNELS=48;
const MOUNTAIN_PLATES_ACROSS=4;
const MOUNTAIN_RIVULETS=192;
const MOUNTAIN_PLATES_ALONG=1.3;
// The rate lava falls at height h, and the time it takes from the rim
// there; hs is the slopes' top, where G is 0.
fn mountain_fall_rate(h:f32,hs:f32)->f32 {return MOUNTAIN_FALL_FOOT*exp(MOUNTAIN_FALL_K*clamp(h/hs,0.0,1.0));}
fn mountain_fall_time(h:f32,hs:f32)->f32 {
    let x=clamp(h/hs,0.0,1.0);
    return hs/(MOUNTAIN_FALL_K*MOUNTAIN_FALL_FOOT)*(exp(-MOUNTAIN_FALL_K*x)-exp(-MOUNTAIN_FALL_K));
}
fn mountain_slope(c:MountainCamera,m:vec2f,sd:MountainDetail,v:vec3f,fp_map:f32,blur:vec2f,light:vec2f,full:bool)->vec3f {
    let t=field_seconds();
    let h=sd.height;
    let hs=max(2.0*(mandel_base()-128.0),1.0);
    let orig=mountain_original(m,full);
    let heat=orig.x;
    let speck=orig.y;
    let n0=normalize(vec3f(-MOUNTAIN_K*sd.grad,1.0));
    let fpt=mountain_footprint(n0,v,fp_map,blur);
    let steep=length(sd.grad);
    let up=sd.grad/max(steep,1e-6);
    let level=vec2f(-up.y,up.x);
    // The channel cells and the height the footprint spans.
    let nch=f32(MOUNTAIN_CHANNELS);
    let f_x=nch*sd.rate/(2.0*PI)*mountain_spans(fpt,level);
    let f_h=steep*mountain_spans(fpt,up);
    // The channel in this cell of the angle, if any.
    let wander=mountain_gnoise(vec2f(sd.angle*nch*.25,sd.g*7.0),41u,MOUNTAIN_CHANNELS/4);
    let x=sd.angle*nch+.45*wander;
    let cell=floor(x);
    let ci=((i32(cell)%MOUNTAIN_CHANNELS)+MOUNTAIN_CHANNELS)%MOUNTAIN_CHANNELS;
    let hc=mountain_hash2(vec2i(ci,0),43u);
    let hd=mountain_hash2(vec2i(ci,1),43u);
    let he=mountain_hash2(vec2i(ci,2),43u);
    let live=step(.35,hd.x);
    // Wider channels carry more lava and stay hot for longer.
    let vigour=.45+.55*hc.y;
    // Near the rim, once the lake spills, the channels widen into the
    // falls that feed them; and each narrows and widens a little down its
    // length.
    let spill=mountain_spill();
    let near_rim=clamp((h-hs+30.0)/27.0,0.0,1.0);
    let along=mountain_gnoise(vec2f(f32(ci)*1.7,sd.g*20.0),42u,0);
    let width=mix(.07,.32,hc.y*hc.y)*(.8+.4*along);
    let half=mix(width,max(width,.3),near_rim*near_rim*spill*live);
    // Its banks are ragged.
    let rag_fade=clamp(1.5-f_x*5.0,0.0,1.0);
    var rag=0.0;
    if rag_fade>0.0 {rag=.12*rag_fade*mountain_gnoise(vec2f(x*5.0,sd.g*60.0),39u,MOUNTAIN_CHANNELS*5);}
    let across=x-cell-(.4+.2*hc.x)+rag*width;
    let w=clamp(across/half,-1.0,1.0);
    let ww=w*w;
    let many=smoothstep(.4,1.2,f_x);
    let inside=mix(mountain_band(abs(across),half,f_x),.25,many);
    // The flow's time from the rim, with the rapids, and how fast it runs
    // there against the mean.
    let speed=.8+.45*hd.y;
    let tau=mountain_fall_time(h,hs);
    let kr=2.0*PI/(MOUNTAIN_RAPIDS_PERIOD*(.7+.6*he.x));
    let pr=kr*tau+6.2832*he.y;
    let lambda=1.0/(1.0+MOUNTAIN_RAPIDS*cos(pr));
    let age=(tau+MOUNTAIN_RAPIDS/kr*sin(pr))/speed;
    let f_age=f_h/(mountain_fall_rate(h,hs)*lambda*speed);
    // The lava's front after the lake spills: below it the channel still
    // holds older, crusted lava.
    let fed=live*smoothstep(-.06,.06,t-mountain_spill_time()-age);
    // Lower down, stretches of a channel roof over into tubes, the lava
    // running on unseen beneath a glowing crust.
    let tube=smoothstep(.25,.55,-along)*smoothstep(50.0,110.0,hs-h);
    // Plates of crust, carried as a raft down the channel's middle.
    let pa=f32(MOUNTAIN_PLATES_ACROSS);
    let qp=vec2f(x*pa,(age-t)*MOUNTAIN_PLATES_ALONG);
    // Past a plate a pixel each way, only the cracks' mean shows.
    var pl=MountainPlate(1.0,.5,vec2f(0),vec2f(1,0));
    var fq=2.0;
    if min(f_x*pa,f_age*MOUNTAIN_PLATES_ALONG)<1.1 && many<1.0 {
        pl=mountain_plates(qp,45u,MOUNTAIN_CHANNELS*MOUNTAIN_PLATES_ACROSS);
        fq=length(pl.across*vec2f(f_x*pa,f_age*MOUNTAIN_PLATES_ALONG));
    }
    // Clots of hotter lava ride the flow, holding the crust open.
    let cf=clamp(1.5-f_x*2.1,0.0,1.0)*clamp(1.5-f_age*2.5,0.0,1.0);
    let clot=cf*mountain_gnoise(vec2f(x*3.0,(age-t)*1.8),48u,MOUNTAIN_CHANNELS*3);
    let crust=smoothstep(.1,1.3,age*(1.0+2.5*ww)/vigour)*(1.0-.55*smoothstep(.2,.9,clot));
    let stretch=max(lambda-1.0,0.0);
    let squeeze=smoothstep(1.0,.72,lambda);
    // Rafts of crust ride the channel's middle with open lava between them;
    // towards the levees the crust closes up.
    let gap=.07*vigour*(1.0-ww)*smoothstep(.3,1.5,age);
    let crack=mountain_cracks(pl.edge,.02+.25*exp(-age/.6)+.07*stretch+gap,fq);
    // Streaks in the open lava, drawn out by the flow and sheared, faster
    // in the middle than towards the levees, most where it runs fast.
    let shear=1.0+.8*ww*exp(-age/4.0);
    let sf=clamp(1.5-f_x*8.0,0.0,1.0)*clamp(1.5-f_age*shear*.9,0.0,1.0);
    var streak=0.0;
    if sf>0.0 {streak=sf*mountain_gnoise(vec2f(x*10.0,(age*shear-t)*.7),47u,MOUNTAIN_CHANNELS*10);}
    // Ropes where the crust buckles as the flow slows, bowed downstream.
    let rf=clamp(1.5-f_age*3.5*2.0,0.0,1.0)*squeeze*crust;
    let rope=rf*sin(6.2832*(3.5*(age-t)+.7*ww));
    // Temperatures: the lava cools as it travels and towards the levees.
    let t_core=1170.0-(22.0+14.0*(1.0-vigour))*pow(max(age,0.0),.8)-50.0*ww;
    // Surges along each channel, the same across it, so that the motion's
    // blur across the screen keeps them: the open lava's thin skin, torn
    // into streaks, comes and goes with them.
    let surge=clamp(1.5-f_age*1.6,0.0,1.0)*mountain_gnoise(vec2f(f32(ci)*7.31+.5,(age-t)*1.6),49u,0);
    let skin_open=smoothstep(0.0,.5,.6*streak+surge);
    let t_open=t_core-25.0-170.0*skin_open+30.0*clot;
    let t_skin=mix(t_core-120.0,470.0+60.0*heat,smoothstep(.2,2.2,age));
    let t_crack=t_core-30.0-(20.0*age+50.0*squeeze)*(1.0-3.0*gap);
    var e_ch=mix(mountain_glow(t_skin),mountain_glow(t_crack),crack);
    e_ch=mix(mountain_glow(t_open),e_ch,crust);
    // The levees: stiff, cooled crust.
    let levee=smoothstep(.72,.98,abs(w))*(1.0-many);
    e_ch=mix(e_ch,mountain_glow(560.0+80.0*(1.0-crust)),levee);
    // How near a channel the point is, for the old crust's glow.
    let near_any=live*exp(-max(abs(across)-half,0.0)*6.0);
    // Unfed channels hold older lava, crusted, with a few glowing cracks.
    let e_old_ch=mix(mountain_glow(520.0),mountain_glow(820.0),crack*.5);
    // Old flows between the channels: dark crust, with cracks still
    // glowing where the original is hot and flecks where its speckle is
    // bright; on the steep walls round the set, where the map's texture
    // would stretch, ridges down the fall lines.
    let wall=smoothstep(.55,.9,1.0-n0.z);
    let turns=sd.rate*fp_map/max(abs(dot(n0,v)),.01)/(2.0*PI);
    var cracked=0.0;
    var bump=vec2f(0);
    let fine_fade=clamp(1.6-fp_map*2.0/max(abs(dot(n0,v)),.01),0.0,1.0);
    if full {
        let fade=fine_fade*smoothstep(0.0,20.0,h);
        if wall<.98 {
            let wn=mountain_fbm(m*.25,13u,2,0)-.5;
            let cells=mountain_cells(m*.7+vec2f(wn,-wn)*1.6,0.0,31u);
            let opened=smoothstep(.55,.72,mountain_fbm(m*.18+3.0,14u,2,0)+.25*heat+.3*near_any);
            cracked=(1.0-smoothstep(0.0,.02+.03*cells.id+fp_map*.5,cells.edge))*opened*fade*(1.0-wall);
            bump=mountain_ridged_d(m*.8,12u,2,0).yz*.8*7.0*fade*(1.0-wall);
        }
        if wall>.02 {
            let q=vec2f(sd.angle*512.0,h*.18);
            let wf=fade*clamp(1.5-turns*512.0*2.5,0.0,1.0);
            bump+=level*mountain_ridged_d(q,15u,2,512).y*3.0*wf*wall;
        }
    }
    let fleck=smoothstep(.62,.85,speck)*(1.0-wall)*smoothstep(10.0,60.0,h);
    // Until the lake spills, the rim is cooled crust.
    let rim_dark=1.0-near_rim*(1.0-spill)*.85;
    var e_old=mountain_glow(430.0+50.0*heat);
    e_old=mix(e_old,mountain_glow(720.0+100.0*heat+80.0*near_any),cracked);
    e_old=mix(e_old,mountain_glow(720.0),fleck*.5);
    let lava=inside*fed*(1.0-tube);
    let roof=mix(mountain_glow(600.0),mountain_glow(880.0),crack*.6);
    var col=mix(e_old,e_ch,lava)+inside*(live-fed)*e_old_ch+inside*fed*tube*roof;
    // Rivulets between the channels, MOUNTAIN_RIVULETS a turn: thinner and
    // quicker to crust over, with clots of hotter lava running down them.
    let nr=f32(MOUNTAIN_RIVULETS);
    let xr=sd.angle*nr+1.8*wander;
    let cr=floor(xr);
    let ri=((i32(cr)%MOUNTAIN_RIVULETS)+MOUNTAIN_RIVULETS)%MOUNTAIN_RIVULETS;
    let hr=mountain_hash2(vec2i(ri,3),44u);
    let f_r=f_x*nr/nch;
    let in_r=step(.62,hr.x)*mix(mountain_band(abs(xr-cr-.3-.4*fract(7.0*hr.x)),mix(.07,.18,hr.y),f_r),.12,smoothstep(.4,1.2,f_r));
    let age_r=tau/(.6+.5*hr.y);
    let fed_r=smoothstep(-.06,.06,t-mountain_spill_time()-age_r)*(1.0-inside*live);
    if in_r*fed_r>0.0 {
        let clots=mountain_gnoise(vec2f(f32(ri)*3.1,(age_r-t)*1.4),46u,0)*clamp(1.5-f_age*1.4,0.0,1.0);
        let t_r=1140.0-70.0*pow(age_r,.8);
        let skin_r=smoothstep(-.15,.45,-clots);
        let crust_r=smoothstep(.3,2.5,age_r)*skin_r;
        var e_r=mix(mountain_glow(t_r+30.0*clots),mountain_glow(t_r-190.0),.85*skin_r);
        e_r=mix(e_r,mountain_glow(500.0)+.2*mountain_glow(t_r-60.0),crust_r);
        col=mix(col,e_r,in_r*fed_r);
    }
    col*=rim_dark*smoothstep(0.0,6.0,h);
    // The crust's form, lit dimly by the key and by the lava's glow round
    // it, with the relief's occlusion: dark glassy basalt.
    let near=fed*near_any;
    let ropes=up*steep*.35*rope*inside*fed;
    let n=normalize(vec3f(-MOUNTAIN_K*(sd.grad+bump+ropes),1.0));
    let l=mountain_key_dir(c);
    let albedo=vec3f(.045,.038,.035)*(.55+.45*smoothstep(0.0,120.0,h));
    let nl=max(dot(n,l),0.0);
    let hot=clamp(lava*(1.0-crust*(1.0-crack)),0.0,1.0);
    let glow_light=mountain_glow(1080.0)*(.015+.12*near+.08*near_rim*spill)*light.x;
    col+=albedo*(glow_light+MOUNTAIN_KEY*(nl*1.1*light.y+.06*light.x))*(1.0-hot);
    col+=MOUNTAIN_KEY*mountain_ggx(n,v,l,.55)*.04*light.y*(1.0-hot);
    return col;
}
// The light leaving a surface sample towards the viewer: linear HDR. `v` is
// towards the viewer in world space (map x, map y and height as MOUNTAIN_K
// map units), fp_map a pixel's size in map units at the sample's depth,
// `blur` how far its footprint is widened along the exposure's motion (x
// across the screen, y up it), `light` its ambient occlusion and key
// shadow. `full` for the full detail (the middle of the exposure); the
// exposure's other samples leave out the old crust's fine texture.
fn mountain_shade(c:MountainCamera,m:vec2f,sd:MountainDetail,v:vec3f,fp_map:f32,blur:vec2f,light:vec2f,full:bool)->vec3f {
    if sd.inside {return mountain_lake(c,m,v,fp_map,blur,full);}
    return mountain_slope(c,m,sd,v,fp_map,blur,light,full);
}

// ---- The layer -------------------------------------------------------------------
// The view ray's direction in world space at depth z for source row y: the
// map direction of its column, and its descent from the projection.
fn mountain_view(c:MountainCamera,s:vec2f,z:f32)->vec3f {
    let dir=mountain_dir(c,s.x);
    let pr=mountain_projection(c,z);
    let dh=-1024.0*s.y*c.q*256.0/(max(z*z,1.0)*pr*pr);
    return -normalize(vec3f(dir,MOUNTAIN_K*dh));
}
// The prepass at a source point: its nearest texel's hint, the coverage
// margin (its pixels, interpolated between texels) and the relief's hit
// depth, interpolated too where the four texels round the point see one
// surface (within 3% in depth), so that the exposure's samples land smoothly
// on it rather than in half-resolution steps.
fn mountain_prepass_texel(s:vec2f)->vec4f {
    let uv=mountain_uv(s);
    let size=vec2i(textureDimensions(prepass_mountain));
    let f=uv*vec2f(size)-.5;
    let i=vec2i(floor(f));
    let w=f-floor(f);
    let a=textureLoad(prepass_mountain,clamp(i,vec2i(0),size-1),0);
    let b=textureLoad(prepass_mountain,clamp(i+vec2i(1,0),vec2i(0),size-1),0);
    let c=textureLoad(prepass_mountain,clamp(i+vec2i(0,1),vec2i(0),size-1),0);
    let d=textureLoad(prepass_mountain,clamp(i+vec2i(1,1),vec2i(0),size-1),0);
    var t=select(select(a,b,w.x>=.5),select(c,d,w.x>=.5),w.y>=.5);
    let lo=min(min(a.z,b.z),min(c.z,d.z));
    let hi=max(max(a.z,b.z),max(c.z,d.z));
    if hi<MOUNTAIN_NONE && hi-lo<.03*lo {t.z=mix(mix(a.z,b.z,w.x),mix(c.z,d.z,w.x),w.y);}
    t.y=clamp(mix(mix(a.y,b.y,w.x),mix(c.y,d.y,w.x),w.y),-4.0,4.0);
    return t;
}
fn mountain_layer(px:FieldPixel)->FieldLayer {
    if field_phase()<.5 {return no_layer();}
    let shutter=mountain_shutter();
    let mid=mountain_camera(-.5*shutter);
    let s=px.source;
    let fp=px.pixel;
    let hint=mountain_hint(s);
    if hint>=MOUNTAIN_NONE {return no_layer();}
    let h0=mountain_march(mid,s,fp,hint-1.0,1,0.0);
    if h0.best<-40.0 {return no_layer();}
    // How far the surface moves on the screen in the exposure, in pixels.
    let open=mountain_camera(-shutter);
    let close=mountain_camera(0.0);
    let p_open=mountain_project(open,h0.map,h0.height);
    let p_close=mountain_project(close,h0.map,h0.height);
    let p_mid=mountain_project(mid,h0.map,h0.height);
    let sweep=p_close.xy-p_open.xy;
    let motion=length(vec2f(sweep.x*COLUMN,sweep.y))/fp;
    let count=i32(clamp(ceil(motion/MOUNTAIN_SAMPLE_PIXELS),1.0,f32(MOUNTAIN_SAMPLES)));
    if !h0.hit && h0.best<-(motion+2.0) {return no_layer();}
    // The relief's occlusion and the key's shadow, once, where the middle
    // of the exposure sees the surface.
    let n0=normalize(vec3f(-MOUNTAIN_K*relief_at(h0.map).yz,1.0));
    let light=mountain_lighting(mid,h0.map,h0.height,n0);
    // Detail is smoothed over a pixel, and along the motion over what the
    // exposure sweeps between its samples (across the screen and up it), so
    // that the samples blend into a streak along the motion while the
    // detail across it stays sharp.
    let fp_map=h0.z/256.0*fp/COLUMN;
    let spacing=vec2f(sweep.x*COLUMN,sweep.y)/(fp*f32(count));
    let blur=max(vec2f(1.0),abs(spacing));
    // The middle of the exposure, marched and shaded at full detail.
    let a0=clamp(.5+h0.best,0.0,1.0);
    let d0=mountain_detail_at(h0.map);
    var sum=vec3f(0);
    var cover=0.0;
    if a0>0.0 {
        sum=mountain_shade(mid,h0.map,d0,mountain_view(mid,s,h0.z),fp_map,blur,light,true)*a0;
        cover=a0;
    }
    // The rest of the exposure: what the middle's rays saw where this
    // pixel's ray points then, from the prepass (the relief's hit and
    // coverage), with the surface carried over from the middle.
    // Jittered per pixel and per sixtieth of a second, so that the noise
    // moves rather than sitting on the screen.
    let jitter=mountain_hash(vec2i(floor(px.uv*u.screen.xy)),91u+u32(max(field_seconds(),0.0)*60.0));
    for(var i=1;i<count;i++) {
        // Strata either side of the middle one.
        var k=f32(i-1)+jitter;
        if k>=.5*f32(count-1) {k+=1.0;}
        let dt=-shutter*k/f32(count);
        let cam=mountain_camera(dt);
        let p_i=mountain_project(cam,h0.map,h0.height);
        let back=s-(p_i.xy-p_mid.xy);
        let pre=mountain_prepass_texel(back);
        let a=clamp(.5+2.0*pre.y,0.0,1.0);
        if a<=0.0 || pre.z>=MOUNTAIN_NONE {continue;}
        let m=mid.map+mountain_dir(mid,back.x)*pre.z;
        let d=mountain_detail_near(d0,h0.map,m);
        sum+=mountain_shade(mid,m,d,mountain_view(mid,back,pre.z),fp_map,blur,light,false)*a;
        cover+=a;
    }
    let n=f32(count);
    if cover<=0.0 {return no_layer();}
    return FieldLayer(sum/n,cover/n,LAYER_MOUNTAIN_Z);
}
fn mountain_prepass(px:FieldPixel)->vec4f {
    if field_phase()<.5 {return vec4f(MOUNTAIN_NONE,-1e4,MOUNTAIN_NONE,0);}
    let cam=mountain_camera(-.5*mountain_shutter());
    let h=mountain_march(cam,px.source,px.pixel,MOUNTAIN_NEAR,1,MOUNTAIN_APPROACH);
    return vec4f(h.near,h.best,select(MOUNTAIN_NONE,h.z,h.hit),0);
}
