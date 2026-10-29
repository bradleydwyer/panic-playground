// The bump wall's geometry, shared by the scroller (scroller.wgsl) and the
// part-6 style modules, which include it: the tile layout read from the port, the
// authored bump shape, the authored waves and the analytic ray / bump
// intersections, including Inigo Quilez's MIT-licensed rounded box below.
// Shading stays with each user. renderer::wgsl and the module lists in
// renderer.rs and rings_render.rs concatenate it after the user's own code.
// It reads the shared controls: u.backdrop.xy (the oscillator, source pixels)
// and .w (the bump shape), u.next_palette (the 8x8 tile image), u.waves and
// u.screen.z. The waves also need find_waves() (scroller.wgsl) to have set
// the wave_* variables for the pixel; with u.waves.x at 0 they are off.
// The folds also read u.backdrop_motion.w and u.glyphs.w.
// ---- Backdrop -------------------------------------------------------------
// Read from the port (scenes/scroller.rs; the ignored clip test checks it on
// every scroll frame): pixel (sx,sy) of the 320x200 frame shows cell
// ((sy+7+y)&7, (sx+4+x)&7) of the 8x8 image, where x=(xx>>7)&7, y=(yy>>7)&7.
// The palette parity term only cancels the hardware scroll's half tile, so
// the wall does not follow the text. Here the position is continuous: xx/128
// and yy/128 interpolated between ticks, less half a pixel to centre the
// original's whole-pixel steps. The image's dark seam is column 7 and row 0.
// Before part 2's scroll starts the oscillator, the position is an authored
// lead-in, the oscillator run backwards (clip::oscillator_lead_in).
const WALL_Z=-.9; // The chamber plane, 6.4 from the eye.
fn wall_scale()->f32 {return (5.5-WALL_Z)/5.5;}
// Source-pixel image coordinate of a wall point. The 4:3 frame spans
// x=[-4/3,4/3], y=[-1,1] on screen: 120 source pixels per unit across and 100
// down, so a tile is 1.2 times taller than wide, as on a 4:3 display.
fn wall_source(p:vec2f)->vec2f {
    let s=p/wall_scale();
    return vec2f(s.x*120.0+164.0,107.0-s.y*100.0)+u.backdrop.xy+wall_shift-.5;
}
fn tile_image(at:vec2f)->vec3f {
    // Smoothed between the 8x8 image's texels (centres at +.5), wrapping.
    let q=at-.5;
    let a=vec2u(vec2i(floor(q))&vec2i(7));let f=fract(q);let s=f*f*(3.0-2.0*f);
    let b=(a+1u)%8u;
    return mix(mix(u.next_palette[a.y*8u+a.x].rgb,u.next_palette[a.y*8u+b.x].rgb,s.x),
        mix(u.next_palette[b.y*8u+a.x].rgb,u.next_palette[b.y*8u+b.x].rgb,s.x),s.y);
}
// Grid space: one unit per tile, integers on the seam centres, y downwards,
// z in tile widths above the wall plane. An affine map of world space.
fn grid_point(p:vec3f)->vec3f {
    let s=wall_source(p.xy);
    return vec3f((s.x+.5)/8.0,(s.y-.5)/8.0,(p.z-WALL_Z)*15.0/wall_scale());
}
fn grid_direction(d:vec3f)->vec3f {return vec3f(d.x*15.0,-d.y*12.5,d.z*15.0)/wall_scale();}
fn world_normal(n:vec3f)->vec3f {return normalize(vec3f(n.x*15.0,-n.y*12.5,n.z*15.0));}
// ---- Wall folds (optional) ----
// Authored, with nothing like it in the original, whose wall is flat: the
// wall is cloth on its moving support (clip::ScrollerPreview::folds_at).
// While the sway moves, the cloth is drawn taut; as it slows to a turn it
// relaxes into folds across the way it was moving, which ride with the
// wall and are drawn out again as it speeds away, lit from the side so
// their crests cast shadows. They are a pure function of the clip time, so
// seeking and captures agree. Heights and positions are in tile widths, y
// up. u.waves.x: the depth, eased in with the reveal, 0 when off; .y: the
// exposure in seconds; .z: the side light's strength, eased likewise; .w:
// how much the folds stand, 0 to 1. u.backdrop_motion.w: their axis, the
// way the cloth was drawn (radians, right and up), the crests lying across
// it; u.glyphs.w: their phase, where along the axis they gathered (tile
// widths in the wall). Three folds of different spacing, about 17, 12 and
// 26 tiles, with soft crests and sharp creases, their crests wandering and
// their height swelling along their length. At depth 1 (far too deep) the
// largest stands 2.4 tile widths; the default depth is 0.5.
const FOLD_K=vec3f(.37,.52,.24);    // Radians a tile width across the folds.
const FOLD_H=vec3f(2.4,1.0,1.3);    // Heights at depth 1.
const FOLD_CREASE=.25;              // Sharpens the creases, softens the crests.
// Along the crests, one wander and one swell shared by the three, each
// fold taking them at its own offset (radians): how far each fold's crest
// wanders (radians of phase) and its height swells.
const FOLD_WANDER=.045;             // Radians a tile width along the crests.
const FOLD_SWELL=.055;
const FOLD_SWAY=vec3f(1.3,1.0,1.5);
const FOLD_OFFSET=vec3f(.5,2.2,3.9);
// The folds' greatest curvature at depth 1, with a margin: each height times
// its wave number squared, times the creases' 1 + 4 FOLD_CREASE.
const BEND=1.4;
fn waving()->bool {return u.waves.x>0.0;}
// Whether the folds stand at all (a thousandth of a tile width): while they
// lie flat, the wall is traced flat, as the same with no folds.
fn folded()->bool {return u.waves.x*u.waves.w>1e-3;}
// Height, slope and rate of rise (per second).
struct Wave { h:f32, slope:vec2f, rate:f32 }
// Where screen point m (tile widths from the screen centre) lies in the
// cloth `lag` seconds before the frame, and how fast the cloth moves.
fn fold_point(m:vec2f,lag:f32)->vec2f {
    let o=u.backdrop.xy-u.backdrop_motion.xy*(lag/max(u.waves.y,1e-6));
    return m+vec2f(o.x,-1.2*o.y)/8.0;
}
fn fold_velocity()->vec2f {
    let v=u.backdrop_motion.xy/max(u.waves.y,1e-6);
    return vec2f(v.x,-1.2*v.y)/8.0;
}
// The folds at m, `lag` seconds before the frame.
fn wave_field(m:vec2f,lag:f32)->Wave {
    if !folded() {return Wave(0.0,vec2f(0),0.0);}
    let f=fold_point(m,lag);
    let d=vec2f(cos(u.backdrop_motion.w),sin(u.backdrop_motion.w));
    let e=vec2f(-d.y,d.x);
    let along=dot(f,d)-u.glyphs.w;
    let across=dot(f,e);
    let wa=FOLD_WANDER*across;let sw=sin(wa);let cw=cos(wa);
    let sa=FOLD_SWELL*across;let ss=sin(sa);let cs=cos(sa);
    var h=0.0;
    var g=vec2f(0); // Along and across.
    for(var i=0;i<3;i++) {
        // sin and cos of the shared angles at this fold's offset.
        let co=cos(FOLD_OFFSET[i]);let so=sin(FOLD_OFFSET[i]);
        let ph=FOLD_K[i]*along+FOLD_SWAY[i]*(sw*co+cw*so)+2.1*f32(i);
        let dph=vec2f(FOLD_K[i],FOLD_SWAY[i]*FOLD_WANDER*(cw*co-sw*so));
        let swell=.7+.3*(ss*so+cs*co);
        let dswell=.3*FOLD_SWELL*(cs*so-ss*co);
        let c=cos(ph);let s=sin(ph);
        let p=-(c+FOLD_CREASE*(2.0*c*c-1.0));
        let dp=s*(1.0+4.0*FOLD_CREASE*c);
        h+=FOLD_H[i]*swell*p;
        g+=FOLD_H[i]*(swell*dp*dph+vec2f(0.0,dswell*p));
    }
    let amp=u.waves.x*u.waves.w;
    let slope=(d*g.x+e*g.y)*amp;
    return Wave(h*amp,slope,dot(slope,fold_velocity()));
}
// Just the height, for marching towards the side light.
fn wave_height(m:vec2f,lag:f32)->f32 {return wave_field(m,lag).h;}
// The waves at this pixel, found once at the middle of its exposure
// (find_waves): the field at wave_m, its world normal there and how much of
// the side light its crests leave it.
var<private> wave_here:Wave;
var<private> wave_m:vec2f;
var<private> wave_up:vec3f;
var<private> wave_lit:f32;
var<private> wave_lag:f32; // Seconds before the frame, per exposure sample.
// Wave coordinates of grid point g. The screen centre is at grid
// ((164,106)+oscillator)/8, and a tile is 1.2 tile widths tall.
fn wave_point(g:vec2f)->vec2f {
    let o=(vec2f(164,106)+u.backdrop.xy+wall_shift)/8.0;
    return vec2f(g.x-o.x,1.2*(o.y-g.y));
}
// The folds' curvature where the pixel meets them (xx, xy, yy), and that
// point in the cloth (fold_point), set by find_waves.
var<private> wave_bend:vec3f;
var<private> wave_at:vec2f;
// The folds at grid point g, a tile's centre: height and slope along grid x
// and y. A tile rides with the folds, so this holds over the exposure. Near
// the pixel (within 1.6 tiles), from the folds' height, slope and curvature
// where the pixel meets them, to second order, rather than the field
// itself: within about a twentieth of a tile width at the folds' depth 0.5
// for the tile under the pixel.
fn grid_wave(g:vec2f)->vec3f {
    if !folded() {return vec3f(0);}
    let d=vec2f(g.x-20.5,1.2*(13.25-g.y))-wave_at;
    let b=wave_bend;
    let slope=wave_here.slope+vec2f(b.x*d.x+b.y*d.y,b.y*d.x+b.z*d.y);
    let h=wave_here.h+dot(wave_here.slope,d)+.5*(b.x*d.x*d.x+2.0*b.y*d.x*d.y+b.z*d.y*d.y);
    return vec3f(h,slope.x,-1.2*slope.y);
}
// The pixel's tangent plane to the waves, carried to this sample's time to
// first order: its height at grid point g. It bounds the bumps near the
// pixel and is the backing between them.
fn wave_plane(g:vec2f)->f32 {
    let w=wave_here;
    return w.h-(wave_lag-u.waves.y*.5)*w.rate+dot(w.slope,wave_point(g)-wave_m);
}
// How far the centre of a tile near the pixel (within 1.6 tiles) can lie
// above or below the pixel's plane, and the most its slope can differ.
fn wave_reach()->vec2f {
    let bend=BEND*u.waves.x*u.waves.w;
    return vec2f(.5*bend*1.6*1.6,bend*1.6);
}
// The bumps' height above the plane near the pixel, with a margin: a tilted
// bump stands taller, by up to the secant of its tilt in the stretched space.
fn wave_top(shape:Bump)->f32 {
    let s=length(wave_here.slope)+wave_reach().y;
    return (shape.radius*shape.flat*.5+.02)*sqrt(1.0+2.4*s*s);
}
// The rotation taking +z to the unit vector n, and its inverse.
fn tilt_from(n:vec3f,v:vec3f)->vec3f {
    let k=dot(n.xy,v.xy);
    return vec3f(v.xy-n.xy*(k/(1.0+n.z)-v.z),n.z*v.z-k);
}
fn tilt_to(n:vec3f,v:vec3f)->vec3f {
    return vec3f(v.xy-n.xy*(dot(n.xy,v.xy)/(1.0+n.z)+v.z),dot(n,v));
}
// A bump resting rigidly on the waves at the centre of tile (x,y), the same
// for every pixel: its base on their tangent plane there, tilted with it.
// Gives the tilt and the ray's start in the bump's own space; `start` is in
// stretched tile space, as in wall_trace.
struct Rest { n:vec3f, start:vec3f }
fn rest(tile:vec2f,start:vec3f,stretch:vec3f)->Rest {
    let w=grid_wave(tile+.5);
    let n=normalize(vec3f(-w.yz*stretch.z/stretch.xy,1));
    return Rest(n,tilt_to(n,start-vec3f(0,0,w.x*stretch.z)));
}
// Authored bump profile from u.backdrop.w, in a tile space where the 7/8
// tile body spans -.875..+.875. A rounded box, intersected with a dome
// through its top centre and the body's base corners. 0 gives a low cushion
// with rounded edges that fills the tile like the original image (1.2 times
// taller than wide); 1 a round hemisphere, with wider gaps above and below.
struct Bump { inner:f32, radius:f32, flat:f32, dome:f32, dome_z:f32, tall:f32 }
fn bump()->Bump {
    let s=clamp(u.backdrop.w,0.0,1.0);
    let radius=mix(.30,1.0,s)*.875;
    let corner2=2.0*.875*.875;
    let dome=(corner2+radius*radius)/(2.0*radius);
    return Bump(.875-radius,radius,mix(.55,1.0,s),dome,radius-dome,mix(1.0,1.0/1.2,s));
}
fn octant(v:vec3f)->vec3f {return select(vec3f(1),vec3f(-1),v<vec3f(0));}
// Analytic ray / rounded box intersection: bounding box, then flat faces, the
// corner sphere and the edge cylinders of the entry octant. rd must be
// normalised, and size.z 0 as for the bumps: the z edges are left out.
// Returns -1 on a miss.
//
// Adapted from roundedboxIntersect() in Inigo Quilez's "Rounded Box -
// intersection", https://www.shadertoy.com/view/WlSXRW (the same code is in
// https://iquilezles.org/articles/intersectors/). Changes: ported to WGSL,
// near-zero direction components nudged off zero, octant() for sign() and no
// z edges. bump_hit()'s box normal is his roundedboxNormal(). Both are used
// under his MIT License; the notice, from the head of that shader:
//
// The MIT License
// Copyright © 2019 Inigo Quilez
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
fn rounded_box(ro0:vec3f,rd0:vec3f,size:vec3f,rad:f32)->f32 {
    let rd1=select(rd0,octant(rd0)*1e-7,abs(rd0)<vec3f(1e-7));
    let m=1.0/rd1;let n=m*ro0;let k=abs(m)*(size+rad);
    let t1=-n-k;let t2=-n+k;
    let near=max(max(t1.x,t1.y),t1.z);let far=min(min(t2.x,t2.y),t2.z);
    if near>far || far<0.0 {return -1.0;}
    let entry=ro0+near*rd1;let s=octant(entry);
    let ro=ro0*s;let rd=rd1*s;
    var q=entry*s-size;
    q=max(q,q.yzx);
    if min(min(q.x,q.y),q.z)<0.0 {return near;}
    let oc=ro-size;let dd=rd*rd;let oo=oc*oc;let od=oc*rd;let ra2=rad*rad;
    var t=1e20;
    let bc=od.x+od.y+od.z;let hc=bc*bc-(oo.x+oo.y+oo.z-ra2);
    if hc>0.0 {t=-bc-sqrt(hc);}
    var a=dd.y+dd.z;var b=od.y+od.z;var h=b*b-a*(oo.y+oo.z-ra2);
    if h>0.0 {
        let e=(-b-sqrt(h))/a;
        if e>0.0 && e<t && abs(ro.x+rd.x*e)<size.x {t=e;}
    }
    a=dd.z+dd.x;b=od.z+od.x;h=b*b-a*(oo.z+oo.x-ra2);
    if h>0.0 {
        let e=(-b-sqrt(h))/a;
        if e>0.0 && e<t && abs(ro.y+rd.y*e)<size.y {t=e;}
    }
    return select(t,-1.0,t>1e19);
}
// Entry into one bump, with its tile-space normal; t<0 on a miss. Entering
// the intersection of two convex solids is the later of the two entries, as
// long as the ray is still inside both; that holds for rays arriving from
// above, which is all the wall needs. The box normal is Quilez's (see
// rounded_box).
fn bump_hit(ro:vec3f,rd:vec3f,shape:Bump)->vec4f {
    let size=vec3f(shape.inner,shape.inner,0);
    let t=rounded_box(ro,rd,size,shape.radius);
    if t<0.0 {return vec4f(-1);}
    let oc=ro-vec3f(0,0,shape.dome_z);
    let b=dot(oc,rd);let h=b*b-dot(oc,oc)+shape.dome*shape.dome;
    if h<0.0 {return vec4f(-1);}
    let td=-b-sqrt(h);
    if td>t {return vec4f(normalize(oc+rd*td),td);}
    let p=ro+rd*t;
    return vec4f(octant(p)*normalize(max(abs(p)-size,vec3f(0))),t);
}
struct WallHit { hit:bool, bump:bool, t:f32, n:vec3f, at:vec3f }
// Each ray tests only the tiles it crosses inside the bumps' height: one to
// four for camera rays, up to 3x3 for rays leaving the wall. With waves, each
// bump rests rigidly on the waves at its own tile's centre (rest), and the
// backing between the bumps is the pixel's tangent plane. The slab follows
// that plane, widened by how far the waves curve away from it, so a ray
// still crosses only a few tiles.
fn wall_trace(origin:vec3f,ray:vec3f,from_wall:bool)->WallHit {
    let shape=bump();
    let g=grid_point(origin);let dg=grid_direction(ray);
    let stretch=vec3f(2.0,2.0/shape.tall,2.0/shape.flat);
    // Start a little above the bumps: a ray starting on the flat top face
    // could otherwise miss it by rounding.
    var top=shape.radius*shape.flat*.5+.02;
    // With waves: the plane's slope in grid units, how far the bumps can lie
    // from it, its height under the ray's origin, and how far a tilted bump
    // can reach past its own tile.
    var lift=vec2f(0);var reach=0.0;var base=0.0;var lean=0.0;
    if folded() {
        lift=vec2f(wave_here.slope.x,-1.2*wave_here.slope.y);
        top=wave_top(shape);
        reach=wave_reach().x;
        base=wave_plane(g.xy);
        lean=.03;
    }
    // How fast the ray drops towards the plane.
    let fall=dg.z-dot(lift,dg.xy);
    var t0=0.0;var t1=0.0;
    if from_wall {
        if fall<=0.0 {return WallHit(false,false,0.0,vec3f(0,0,1),g);}
        t1=max((base+top+reach-g.z)/fall,0.0);
    } else {
        if fall>=0.0 {return WallHit(false,false,0.0,vec3f(0,0,1),g);}
        t0=(base+top+reach-g.z)/fall;t1=(base-reach-g.z)/fall;
    }
    let a=g+dg*t0;let b=g+dg*t1;
    let lo=max(floor(min(a.xy,b.xy)-lean),floor(a.xy)-1.0);
    let hi=min(floor(max(a.xy,b.xy)+lean),floor(a.xy)+1.0);
    let l=dg*stretch;let scale=length(l);let ld=l/scale;
    // A reflected ray must not find the bump it leaves.
    let home=floor(a.xy);
    var best=1e20;var normal=vec3f(0,0,1);
    for(var y=lo.y;y<=hi.y;y+=1.0) {
        for(var x=lo.x;x<=hi.x;x+=1.0) {
            if from_wall && x==home.x && y==home.y {continue;}
            let start=vec3f(a.xy-vec2f(x,y)-.5,a.z)*stretch;
            var hit=vec4f(-1);
            if folded() {
                let on=rest(vec2f(x,y),start,stretch);
                let dir=tilt_to(on.n,ld);
                hit=bump_hit(on.start,dir,shape);
                // The bump's half below its base is inside the wall; tilted,
                // it could otherwise show past the bump's edge.
                if (on.start+dir*hit.w).z< -.002 {hit=vec4f(-1);}
                hit=vec4f(tilt_from(on.n,hit.xyz),hit.w);
            } else {
                hit=bump_hit(start,ld,shape);
            }
            if hit.w>=0.0 && hit.w<best {
                best=hit.w;
                normal=hit.xyz*stretch;
            }
        }
    }
    if best<1e19 {
        let t=t0+best/scale;
        return WallHit(true,true,t,world_normal(normal),g+dg*t);
    }
    if from_wall {return WallHit(false,false,0.0,vec3f(0,0,1),g);}
    if folded() {
        let t=(base-g.z)/fall;
        return WallHit(true,false,t,wave_up,g+dg*t);
    }
    return WallHit(true,false,t1,vec3f(0,0,1),b);
}
// Whether p, in a bump's own stretched space, lies inside it, above its base.
fn in_bump(p:vec3f,shape:Bump)->bool {
    let outside=length(max(abs(p)-vec3f(shape.inner,shape.inner,0),vec3f(0)))-shape.radius;
    let dome=length(p-vec3f(0,0,shape.dome_z))-shape.dome;
    return max(outside,dome)<.002 && p.z>-.002;
}
// Whether the side light reaches grid point g past the bumps within a tile
// or so: a short ray towards it (grid direction d). Further crests are left
// to the waves' own shadow (wave_lit). The bump g lies on can't block it, as
// the bumps are convex and the light is on its side.
fn bumps_pass(g:vec3f,d:vec3f)->f32 {
    let shape=bump();
    let stretch=vec3f(2.0,2.0/shape.tall,2.0/shape.flat);
    let lift=vec2f(wave_here.slope.x,-1.2*wave_here.slope.y);
    let rise=d.z-dot(lift,d.xy);
    if rise<=0.0 {return 0.0;}
    let t1=min((wave_plane(g.xy)+wave_top(shape)+wave_reach().x-g.z)/rise,1.2/length(d.xy));
    if t1<=0.0 {return 1.0;}
    let b=g+d*t1;
    let lo=floor(min(g.xy,b.xy)-.03);let hi=floor(max(g.xy,b.xy)+.03);
    let l=d*stretch;let ld=l/length(l);
    for(var y=lo.y;y<=hi.y;y+=1.0) {
        for(var x=lo.x;x<=hi.x;x+=1.0) {
            let on=rest(vec2f(x,y),vec3f(g.xy-vec2f(x,y)-.5,g.z)*stretch,stretch);
            let dir=tilt_to(on.n,ld);
            // The later of the two solids' entries is a hit only inside both.
            let hit=bump_hit(on.start,dir,shape);
            if hit.w>1e-3 && in_bump(on.start+dir*hit.w,shape) {return 0.0;}
        }
    }
    return 1.0;
}
// Per-sample offset of the oscillator along the exposure; 0 outside it.
var<private> wall_shift:vec2f;
