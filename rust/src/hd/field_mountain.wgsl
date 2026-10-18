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
//   formula, base + slope sqrt(G(c)), evaluated per pixel).
// - Interpretation: reading the projection as 3D, a height unit as
//   MOUNTAIN_K map units.
// - Authored: the materials (crust, lava channels, the lake and its plates),
//   the lighting, the exposure and the detail method.
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
// exposure, shaded with the middle's surface carried over to first order.

// Map units a height unit, to light the projection as 3D: at full
// perspective (q = 248) a height unit shows 62 / z rows and a map unit
// across 256 * 5/6 / z rows, so a height unit looks like 0.29 map units
// (derived from the code's projection; the 3D reading is interpretation).
const MOUNTAIN_K=0.29;
// The most samples across the exposure, and the screen motion, in target
// pixels, each sample covers (authored).
const MOUNTAIN_SAMPLES=6;
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
// Ridged octaves, 0 to 1, sharp creases at the noise's zero lines.
fn mountain_ridged(p:vec2f,seed:u32,octaves:i32,period:i32)->f32 {
    var sum=0.0;var amp=.5;var q=p;var norm=0.0;var per=period;
    for(var o=0;o<octaves;o++) {
        sum+=amp*(1.0-abs(mountain_gnoise(q,seed+u32(o)*5u,per)));
        norm+=amp;amp*=.5;q=q*2.1+vec2f(0.0,11.7);per*=2;
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
// The lava's colour at heat 0..1: the original's terrain ramp (MAP.PAL's
// entries 128 to 191 during the terrain), dark red to yellow.
fn mountain_lava(heat:f32)->vec3f {return palette_color(128.0+63.0*clamp(heat,0.0,1.0));}
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
// molten out of the zoom's set, fully two seconds after it has risen
// (authored).
fn mountain_crusting()->f32 {return smoothstep(time_risen()-.3,time_risen()+2.2,field_seconds());}
// How far the lake has spilled over its rim: not at all while the mountain
// rises, so that the molten set stands out against a dark rim, and fully a
// second after it has risen (authored).
fn mountain_spill()->f32 {return smoothstep(time_risen()-.25,time_risen()+1.0,field_seconds());}
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
// The slow masks the shading draws with, which the exposure's samples share
// with its middle: where lava flows on the slopes (by the angle and height),
// and where the lake's plates gather.
fn mountain_masks(m:vec2f,sd:MountainDetail)->vec2f {
    let t=field_seconds();
    if sd.inside {return vec2f(0.0,mountain_fbm(m*.07+vec2f(3.0,.015*t),9u,3,0));}
    let n=normalize(vec3f(-MOUNTAIN_K*sd.grad,1.0));
    let heat=mix(mountain_original(m,false).x,.42,smoothstep(.3,.8,1.0-n.z));
    let along=(mountain_top()-sd.height)-t*5.0;
    return vec2f(smoothstep(.42,.70,mountain_fbm(vec2f(sd.angle*32.0,along*.008+5.0),10u,2,32)+.25*heat-.1),0.0);
}
// The light leaving a surface sample towards the viewer: linear HDR. `v` is
// towards the viewer in world space (map x, map y and height as MOUNTAIN_K
// map units), fp_map the size in map units over which the sample's detail
// is to be smooth (a pixel, or more under motion), `light` its ambient
// occlusion and key shadow, `masks` its mountain_masks. `full` for the full
// detail (the middle of the exposure); the exposure's other samples, which
// motion smooths, take fewer octaves.
fn mountain_shade(c:MountainCamera,m:vec2f,sd:MountainDetail,v:vec3f,fp_map:f32,light:vec2f,masks:vec2f,full:bool)->vec3f {
    let t=field_seconds();
    let top=mountain_top();
    let l=mountain_key_dir(c);
    let orig=mountain_original(m,true);
    let heat=orig.x;
    let speck=orig.y;
    let fine_fade=clamp(1.6-fp_map*2.0,0.0,1.0);
    let oct=select(1,0,full);
    if sd.inside {
        // The lake: molten, with dark plates of crust drifting and cracking
        // on it (authored). Near the rim, where the relief falls below the
        // plateau, it wells up and stays molten.
        let r=relief_height(m);
        let crusting=mountain_crusting();
        let flow=masks.y;
        let warp=vec2f(mountain_fbm(m*.04+vec2f(.01*t,1.0),3u,2-oct,0),mountain_fbm(m*.04+vec2f(5.2,-.008*t),4u,2-oct,0))-.5;
        let q=m*.2+warp*2.5;
        let cell=mountain_cells(q,t*.6,21u);
        let crack=1.0-smoothstep(0.0,.07+fp_map*.2,cell.edge);
        var crack_fine=0.0;
        if full {
            let small=mountain_cells(q*2.7+3.1,t*.9,22u);
            crack_fine=(1.0-smoothstep(0.0,.06+fp_map*.55,small.edge))*fine_fade;
        }
        let rim=smoothstep(top-.2,top-10.0,r);
        // Plates where the slow flow gathers them; open lava elsewhere.
        let gather=smoothstep(.40,.62,flow+.22*(cell.id-.5)+.12*crusting);
        let skin=gather*crusting*(1.0-rim)*(1.0-crack);
        let molten=.70+.22*mountain_fbm(m*.5-vec2f(.04*t,0.0),6u,2-oct,0)+.08*rim;
        var col=mountain_lava(molten)*(2.2+3.0*molten*molten+2.0*rim);
        let plate=vec3f(.045,.022,.012)+mountain_lava(.40)*(.18+.9*crack_fine);
        col=mix(col,plate,skin);
        col+=mountain_lava(.92)*7.0*crack*gather*crusting*(1.0-rim);
        // The plates are slightly domed, and glossy: the key's highlight.
        let dome=(q-cell.centre)*skin*.5;
        let nb=normalize(vec3f(-dome,1.0));
        let fresnel=.04+.96*pow(1.0-max(dot(nb,v),0.0),5.0);
        col+=MOUNTAIN_KEY*mountain_ggx(nb,v,l,.3)*fresnel*1.2*skin;
        return col;
    }
    // The slopes: dark crust, lit from within. Lava runs down in narrow
    // rivers along the fall lines (the external angle), flecks where the
    // original's speckle is bright, and glows in the crust's cracks; it is
    // hottest near the rim, where it spills from the lake (authored).
    let h=sd.height;
    let n0=normalize(vec3f(-MOUNTAIN_K*sd.grad,1.0));
    // The pixel's footprint on the surface, longer where the view grazes it
    // (on the silhouette the view runs along the fall lines, and the
    // angle's patterns crowd into a pixel).
    let fp_s=fp_map/max(abs(dot(n0,v)),.01);
    // The original's colour is in the map, which stretches down steep
    // slopes into streaks: there the heat holds a middling value.
    let stretch=smoothstep(.3,.8,1.0-n0.z);
    let heat_s=mix(heat,.42,stretch);
    // Turns of the angle a pixel spans: rivers finer than a pixel fade out.
    let turns=sd.rate*fp_s/(2.0*PI);
    let along=(top-h)-t*5.0;
    let fade1=clamp(1.5-turns*128.0*2.5,0.0,1.0);
    let fade2=clamp(1.5-turns*384.0*2.5,0.0,1.0);
    let river_n=mountain_fbm(vec2f(sd.angle*128.0,along*.022),7u,3-oct,128);
    let river=mix(.12,smoothstep(.84,.97,1.0-abs(2.0*river_n-1.0)),fade1);
    let flowing=masks.x;
    var streak=.5;
    if fade2>0.0 {streak=mix(.5,mountain_fbm(vec2f(sd.angle*384.0+3.0,along*.07),8u,2-oct,384),fade2);}
    let fine_fade_s=clamp(1.6-fp_s*2.0,0.0,1.0);
    var fine=.5;
    if full {fine=mix(.5,mountain_fbm(m*1.9,3u,3,0),fine_fade_s);}
    // On the steep walls round the set the map's speckle stretches; the
    // flecks there follow the angle.
    let wall=smoothstep(.55,.9,1.0-n0.z);
    let fleck=smoothstep(.56,.76,.62*mix(speck,streak,stretch)+.38*streak+.22*(fine-.5));
    // The crust's texture: in the map on the slopes, and on the steep walls
    // round the set in the angle and height, where the map's would stretch.
    var cracked=0.0;
    var bump=vec2f(0);
    if full {
        let fade=fine_fade_s*smoothstep(0.0,20.0,h);
        if wall<.98 {
            let w=mountain_fbm(m*.25,13u,2,0)-.5;
            let cells=mountain_cells(m*.7+vec2f(w,-w)*1.6,0.0,31u);
            let open=smoothstep(.45,.62,mountain_fbm(m*.18+3.0,14u,2,0)+.3*heat_s);
            cracked=(1.0-smoothstep(0.0,.03+.04*cells.id+fp_map*.5,cells.edge))*open*fade*(1.0-wall);
            let e=.07;
            let b0=mountain_ridged(m*.8,12u,2,0);
            let bx=mountain_ridged((m+vec2f(e,0.0))*.8,12u,2,0);
            let by=mountain_ridged((m+vec2f(0.0,e))*.8,12u,2,0);
            bump=vec2f(bx-b0,by-b0)/e*7.0*fade*(1.0-wall);
        }
        if wall>.02 {
            // Ridges down the walls: the angle across, the height along.
            // Across the wall is along G's level line, where the height
            // holds, so the ridges tilt the normal sideways.
            let q=vec2f(sd.angle*512.0,h*.18);
            let wf=fade*clamp(1.5-turns*512.0*2.5,0.0,1.0);
            let r0=mountain_ridged(q,15u,2,512);
            let r1=mountain_ridged(q+vec2f(.15,0.0),15u,2,512);
            let side=normalize(vec2f(-sd.grad.y,sd.grad.x)+1e-6);
            bump+=side*(r1-r0)/.15*3.0*wf*wall;
        }
    }
    let n=normalize(vec3f(-MOUNTAIN_K*(sd.grad+bump),1.0));
    let near_rim=smoothstep(170.0,238.0,h);
    // Until the lake spills, the rim is cooled crust.
    let rim_dark=1.0-smoothstep(200.0,236.0,h)*(1.0-mountain_spill())*.85;
    let cool=.35+.65*smoothstep(10.0,215.0,h);
    let foot=smoothstep(1.0,30.0,h);
    var lava=max(river*flowing*(.72+.28*heat_s),fleck*(.40+.45*heat_s));
    lava=max(lava,cracked*(.30+.35*heat_s)*smoothstep(.2,.7,heat_s+.4*flowing));
    // Cascades over the rim: narrow falls along the angle, fed by the lake.
    let spill=mountain_spill()*near_rim*near_rim;
    if spill>.01 {
        let fall_n=mountain_fbm(vec2f(sd.angle*256.0+7.0,along*.05),16u,2-oct,256);
        let fall=mix(.15,smoothstep(.78,.95,1.0-abs(2.0*fall_n-1.0)),clamp(1.5-turns*256.0*2.5,0.0,1.0));
        lava=max(lava,fall*(.65+.3*flowing)*spill);
    }
    // Low down, where the flows have cooled, a few seams still glow.
    let seam_n=mountain_fbm(vec2f(sd.angle*64.0,along*.015+9.0),17u,2-oct,64);
    let seam_mask=smoothstep(.55,.75,mountain_fbm(vec2f(sd.angle*16.0,along*.006+2.0),18u,1,16));
    let seams=smoothstep(.95,.99,1.0-abs(2.0*seam_n-1.0))*seam_mask*(1.0-smoothstep(50.0,110.0,h))*fade1;
    lava=max(lava,seams*(.30+.25*heat_s));
    lava*=cool*foot*rim_dark;
    // Hot rock between the flows: a dull red, more where the original is
    // hot.
    let dull=(.08+.30*heat_s*heat_s)*(.5+.5*flowing)*cool*foot;
    var col=mountain_lava(.30+.62*lava)*(dull+.4*lava+2.6*lava*lava+5.5*pow(lava,4.0));
    // The crust: dark basalt lit by the lava round it, with the relief's
    // occlusion, and dimly by the key.
    let albedo=vec3f(.045,.038,.035)*(.55+.45*smoothstep(0.0,120.0,h));
    let nl=max(dot(n,l),0.0);
    let glow_light=mountain_lava(.5)*(.15+1.4*flowing*heat_s+1.2*near_rim)*cool*foot*light.x;
    col+=albedo*(glow_light+MOUNTAIN_KEY*(nl*1.1*light.y+.06*light.x))*(1.0-lava);
    col+=MOUNTAIN_KEY*mountain_ggx(n,v,l,.65)*.03*light.y*(1.0-lava);
    return col;
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
// The prepass at a source point: its nearest texel's hint, coverage margin
// (its pixels) and the relief's hit depth, with the margin interpolated
// between texels.
fn mountain_prepass_texel(s:vec2f)->vec4f {
    let uv=mountain_uv(s);
    let size=vec2i(textureDimensions(prepass_mountain));
    let i=clamp(vec2i(floor(uv*vec2f(size))),vec2i(0),size-1);
    var t=textureLoad(prepass_mountain,i,0);
    t.y=clamp(textureSampleLevel(prepass_mountain,linear,uv,0.0).y,-4.0,4.0);
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
    // Detail is smoothed over a pixel, and under motion over what the
    // exposure sweeps between its samples.
    let fp_map=h0.z/256.0*fp/COLUMN;
    let blur_map=fp_map*max(1.0,.5*motion/f32(count));
    // The middle of the exposure, marched and shaded at full detail.
    let a0=clamp(.5+h0.best,0.0,1.0);
    let d0=mountain_detail_at(h0.map);
    let masks=mountain_masks(h0.map,d0);
    var sum=vec3f(0);
    var cover=0.0;
    if a0>0.0 {
        sum=mountain_shade(mid,h0.map,d0,mountain_view(mid,s,h0.z),blur_map,light,masks,true)*a0;
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
        // A sample on the other side of the lake's edge has its own masks.
        var mk=masks;
        if d.inside!=d0.inside {mk=mountain_masks(m,d);}
        sum+=mountain_shade(mid,m,d,mountain_view(mid,back,pre.z),blur_map,light,mk,false)*a;
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
