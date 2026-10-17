// Authored scroller experiments. The two distance fields are derived at run
// time from locally rendered part-2 frames. They supply contours and motion,
// not original 3D models. Lighting, depth, coating and ribbons are new.
// The tile wall's layout, image and motion are read from the port (see the
// backdrop section); the chrome bumps and their lighting are authored.
struct Controls {
    screen:vec4f, state:vec4f, misc:vec4f,
    flights:array<vec4f,10>, plate_center:vec4f, plate_rotation:vec4f,
    // next_palette: the scroller's 8x8 background image (clip.rs).
    transition:vec4f, tile_order:array<vec4f,40>, next_palette:array<vec4f,64>,
    scroller:vec4f, scroll_light:vec4f,
    audio:vec4f, plate_open_center:vec4f, plate_open_rotation:vec4f, hud:vec4f,
    // Oscillator xx/128 and yy/128 (source pixels, interpolated between
    // ticks), backdrop (0 chamber, 1 tinted wall, 2 chrome wall) and the
    // authored bump shape (0 rounded square .. 1 hemisphere).
    backdrop:vec4f,
    // Oscillator movement over the wall's exposure (source pixels) and the
    // wall strength.
    backdrop_motion:vec4f,
}
@group(0) @binding(0) var<uniform> u:Controls;
@group(0) @binding(1) var current:texture_2d<f32>;
@group(0) @binding(2) var following:texture_2d<f32>;
@group(0) @binding(4) var linear:sampler;
const PI=3.14159265;
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}
fn hash(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
fn noise(p:vec2f)->f32 {
    let i=floor(p);let f=fract(p);let s=f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2f(1,0)),s.x),mix(hash(i+vec2f(0,1)),hash(i+1.0),s.x),s.y);
}
fn warm(x:f32)->f32 {return smoothstep(-.55,1.1,x);}
fn heat(x:f32)->vec3f {return mix(vec3f(.12,.52,1.0),vec3f(1.0,.27,.055),warm(x));}
fn musical()->f32 {return mix(.28,u.scroll_light.x,u.scroll_light.y);}
fn bass()->f32 {return u.scroll_light.z*u.scroll_light.z*u.scroll_light.y;}
fn attack()->f32 {return pow(u.scroll_light.w,1.5)*u.scroll_light.y;}
fn lighting(studio:f32,reactive:f32)->f32 {return mix(studio,reactive,u.scroll_light.y);}
fn clock()->f32 {return max(u.scroller.y,0.0);}
fn aspect()->f32 {return u.screen.x/u.screen.y;}
fn field(uv:vec2f)->vec2f {
    if any(uv<vec2f(0)) || any(uv>vec2f(1)) || u.state.x<8.0 {return vec2f(-32);}
    let a=textureSampleLevel(current,linear,uv,0).rg;
    let b=textureSampleLevel(following,linear,uv,0).rg;
    return mix(a,b,u.state.w);
}
fn uv_at(p:vec2f)->vec2f {return p/vec2f(2.0*aspect(),-1.72)+.5;}
fn field_coverage(uv:vec2f)->f32 {
    // Projected light/shadow must fade before the source raster ends. Otherwise
    // field()'s outside value cuts a moving reflection into a horizontal bar.
    let edge=min(uv,1.0-uv);
    return smoothstep(0.0,.045,edge.x)*smoothstep(0.0,.06,edge.y);
}
fn ribbon_z(x:f32)->f32 {
    if u.scroller.z>.5 && u.scroller.z<1.5 {return 0.0;}
    // A legible central track, with the ends turning into the chamber.
    let edge=max(abs(x)/aspect()-.48,0.0)/.52;
    return -1.5*edge*edge;
}
fn filament_z(p:vec2f)->f32 {
    // Keep a fixed row order: the vertical text passes over the lower row and
    // under the upper row. Follow the ribbon's bend so this holds at its ends.
    // The depth crossover stays in the gap between the two horizontal rows.
    return ribbon_z(p.x)-.075+mix(.30,-.30,smoothstep(-.08,.08,p.y));
}
fn extrusion(d:f32,z:f32,half_depth:f32,bevel:f32)->f32 {
    let q=vec2f(d+bevel,abs(z)-half_depth+bevel);
    return min(max(q.x,q.y),0.0)+length(max(q,vec2f(0)))-bevel;
}
fn shapes(p:vec3f)->vec2f {
    let f=field(uv_at(p.xy));
    let distance=-(f-vec2f(0,.20))*.0086;
    let letter=extrusion(distance.x,p.z-ribbon_z(p.x)+.075,.12,.018);
    // The original sine displacement is already in the field. Authored depth
    // fixes its layering relative to the horizontal rows throughout playback.
    let filament=extrusion(distance.y,p.z-filament_z(p.xy),.018,.009);
    return vec2f(letter,filament);
}
fn scene(p:vec3f)->vec2f {
    let d=shapes(p);
    return select(vec2f(d.x,0),vec2f(d.y,1),d.y<d.x);
}
fn normal(p:vec3f,material:f32)->vec3f {
    let e=vec2f(.008,0);
    let id=u32(material);
    return normalize(vec3f(
        shapes(p+e.xyy)[id]-shapes(p-e.xyy)[id],
        shapes(p+e.yxy)[id]-shapes(p-e.yxy)[id],
        shapes(p+e.yyx)[id]-shapes(p-e.yyx)[id]));
}
// ---- Backdrop -------------------------------------------------------------
// Read from the port (scenes/scroller.rs; the ignored clip test checks it on
// every scroll frame): pixel (sx,sy) of the 320x200 frame shows cell
// ((sy+7+y)&7, (sx+4+x)&7) of the 8x8 image, where x=(xx>>7)&7, y=(yy>>7)&7.
// The palette parity term only cancels the hardware scroll's half tile, so
// the wall does not follow the text. Here the position is continuous: xx/128
// and yy/128 interpolated between ticks, less half a pixel to centre the
// original's whole-pixel steps. The image's dark seam is column 7 and row 0.
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
// Analytic ray / rounded box intersection, adapted from Inigo Quilez's
// MIT-licensed rounded box intersector (iquilezles.org/articles/intersectors):
// bounding box, then flat faces, the corner sphere and the edge cylinders of
// the entry octant. rd must be normalised. Returns -1 on a miss.
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
// above, which is all the wall needs.
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
// four for camera rays, up to 3x3 for rays leaving the wall.
fn wall_trace(origin:vec3f,ray:vec3f,from_wall:bool)->WallHit {
    let shape=bump();
    let g=grid_point(origin);let dg=grid_direction(ray);
    // Start a little above the bumps: a ray starting on the flat top face
    // could otherwise miss it by rounding.
    let top=shape.radius*shape.flat*.5+.02;
    var t0=0.0;var t1=0.0;
    if from_wall {
        if dg.z<=0.0 {return WallHit(false,false,0.0,vec3f(0,0,1),g);}
        t1=max((top-g.z)/dg.z,0.0);
    } else {
        if dg.z>=0.0 {return WallHit(false,false,0.0,vec3f(0,0,1),g);}
        t0=(top-g.z)/dg.z;t1=-g.z/dg.z;
    }
    let a=g+dg*t0;let b=g+dg*t1;
    let lo=max(floor(min(a.xy,b.xy)),floor(a.xy)-1.0);
    let hi=min(floor(max(a.xy,b.xy)),floor(a.xy)+1.0);
    let stretch=vec3f(2.0,2.0/shape.tall,2.0/shape.flat);
    let l=dg*stretch;let scale=length(l);let ld=l/scale;
    // A reflected ray must not find the bump it leaves.
    let home=floor(a.xy);
    var best=1e20;var normal=vec3f(0,0,1);
    for(var y=lo.y;y<=hi.y;y+=1.0) {
        for(var x=lo.x;x<=hi.x;x+=1.0) {
            if from_wall && x==home.x && y==home.y {continue;}
            let start=vec3f(a.xy-vec2f(x,y)-.5,a.z)*stretch;
            let hit=bump_hit(start,ld,shape);
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
    return WallHit(true,false,t1,vec3f(0,0,1),b);
}
// The bump wall is drawn as a defocused, motion-blurred background: soft,
// dim and low in contrast, so the letters stay the brightest and sharpest
// thing on screen and the fast original sway doesn't strobe. All authored.
// u.backdrop_motion.xy: the oscillator's movement during the exposure (source
// pixels); .z: wall strength, the owner's presence control.
var<private> wall_shift:vec2f; // Per-sample offset along the exposure.
// Authored studio surround, prefiltered for a rough metal: a broad frontal
// softbox, a soft top light and the chamber's cool and warm side light, which
// follow the scroller's lighting mode.
fn studio(r:vec3f,x:f32)->vec3f {
    var c=vec3f(.012,.016,.022)+vec3f(.04,.05,.06)*smoothstep(-.6,1.0,r.y)*smoothstep(-.6,.6,r.z);
    let soft=r.xy-vec2f(-.30,.38);
    c+=vec3f(.24,.28,.34)*exp(-dot(soft,soft)*2.2)*smoothstep(-.4,.6,r.z);
    c+=vec3f(.10,.11,.13)*exp(-pow((r.y-.72)*2.4,2.0));
    let sides=exp(-pow((r.x+.62)*3.0,2.0))*.30+exp(-pow((r.x-.66)*3.5,2.0))*.36;
    c+=mix(heat(-.8),heat(1.0),smoothstep(-.3,.3,r.x))*sides*lighting(.88,.30+musical()*.5+bass()*1.4);
    return c;
}
fn wall_metal(at:vec3f)->vec3f {
    let silver=vec3f(.56,.60,.66);
    if u.backdrop.z>1.5 {return silver;}
    // Optional base tint: the unfiltered tile image at this point, scaled so
    // its brightest texel is about 1. It keeps the original's blue steel and
    // fades with the image during the exit. Towards the ball shape the
    // image's own baked shading is mostly replaced by the tile's centre colour.
    let image=mix(tile_image(vec2f(at.x*8.0-.5,at.y*8.0+.5)),
        tile_image(floor(at.xy)*8.0+vec2f(3.5,4.5)),clamp(u.backdrop.w,0.0,1.0)*.6)*4.6;
    return mix(silver*.25,image,.85);
}
fn wall_glint(p:vec3f,n:vec3f,v:vec3f,f0:vec3f)->vec3f {
    // The letters' three lamps as broad, dim highlights on rough metal (no
    // shadow rays): no small sparkle on every bump.
    var c=vec3f(0);
    let lamps=array<vec3f,3>(vec3f(-2.5,2.1,3.8),vec3f(2.5,-1.3,3.2),vec3f(sin(clock()*.23*u.scroll_light.y)*2.0,1.4,3.0));
    let colors=array<vec3f,3>(vec3f(.8,1.3,2.2)*lighting(1.098,.08+musical()*.12+bass()*4.0),
        vec3f(2.4,.9,.30)*lighting(1.168,.08+musical()*.15+bass()*4.8),
        vec3f(1.0,1.1,1.2)*lighting(1.0,.08+attack()*1.8));
    for(var i=0;i<3;i++) {
        let l=normalize(lamps[i]-p);let h=normalize(l+v);
        let nh=max(dot(n,h),0.0);let nl=max(dot(n,l),0.0);
        c+=colors[i]*f0*pow(nh,16.0)*nl*.06;
    }
    return c;
}
// What a bump reflects in a given direction: the letters, found by marching
// their distance field a few steps through the slab they occupy, else the
// studio. Reflected letters are seen from behind, so they are shaded simply:
// the travelling filament glows, the bodies are dark glass or metal that
// hides the studio behind them.
fn reflected(p:vec3f,r:vec3f)->vec3f {
    let env=studio(r,p.x);
    if u.state.x<8.0 || r.z<.05 {return env;}
    var t=(-.42-p.z)/r.z;let end=(.30-p.z)/r.z;
    for(var i=0;i<14;i++) {
        let q=p+r*t;
        if t>end || abs(q.x)>aspect()*1.04 || abs(q.y)>.91 {break;}
        let d=scene(q);
        if d.x<.004 {
            if d.y>.5 {return heat(q.x)*lighting(.8,.08+musical()*.2+bass()*2.0);}
            let glass=select(vec3f(.05,.055,.06),mix(vec3f(.05,.10,.14),vec3f(.14,.08,.04),warm(q.x)),u.scroller.z>1.5);
            return glass+heat(q.x)*.02;
        }
        t+=max(d.x,.012);
    }
    return env;
}
// Shading of one wall sample, without the letter reflections.
fn wall_surface(hit:WallHit,p:vec3f,ray:vec3f)->vec3f {
    let metal=wall_metal(hit.at);
    // The level a flat tile would have: the backing and the relief are pulled
    // towards it, for roughly the original's subtle relief.
    let level=metal*studio(vec3f(0,0,1),p.x);
    if !hit.bump {return level*.45;}
    let n=hit.n;let r=reflect(ray,n);
    let fresnel=metal+(1.0-metal)*pow(1.0-max(dot(n,-ray),0.0),5.0)*.5;
    var c=fresnel*studio(r,p.x)+wall_glint(p,n,-ray,metal);
    // Faint occlusion where a bump meets the backing.
    c*=mix(.55,1.0,smoothstep(.0,.5,n.z));
    return mix(level,c,.6);
}
fn letter_shadow(p:vec3f)->f32 {
    if u.state.x<8.0 {return 0.0;}
    let shadow_uv=uv_at(p.xy+vec2f(.045,-.065));
    return smoothstep(-9.0,4.0,field(shadow_uv).x)*field_coverage(shadow_uv);
}
// Overall presence: the strength control, a gentle corner vignette and the
// letters' shade. No horizontal banding: darkening the text rows, or an
// ellipse stretched across the wide frame, read as a bright stripe.
fn wall_presence(p:vec3f)->f32 {
    let xy=p.xy*(5.5/6.4); // Back to screen units: x in +-aspect, y in +-1.
    let corner=length(vec2f(xy.x/aspect(),xy.y))*.7071;
    let vignette=mix(.6,1.0,1.0-smoothstep(.75,1.05,corner));
    let entrance=smoothstep(-1.7,1.2,u.scroller.y);
    // .58: the calm level the row bands used to give, now applied evenly.
    return u.backdrop_motion.z*.9*.58*vignette*mix(.55,1.0,entrance)*(1.0-letter_shadow(p)*.7);
}
fn wall_point(ray:vec3f)->vec3f {return vec3f(0,0,5.5)+ray*(6.4/-ray.z);}
// Eight samples per pixel spread over a small defocus disc (about a tenth of
// a tile) and over the exposure, so the wall blurs along its own motion. The
// letter reflections are traced once per pixel and softened.
fn wall_pixel(xy:vec2f)->vec3f {
    let eye=vec3f(0,0,5.5);
    let defocus=.0075; // Screen units; a tile is .067 wide.
    var c=vec3f(0);
    for(var i=0;i<8;i++) {
        let f=f32(i);
        let disc=sqrt((f+.5)/8.0)*defocus*vec2f(cos(f*2.39996),sin(f*2.39996));
        // Stratified in time, in an order that doesn't follow the disc.
        wall_shift=-u.backdrop_motion.xy*fract((f*5.0+.5)/8.0);
        let ray=normalize(vec3f(xy+disc,-5.5));
        let hit=wall_trace(eye,ray,false);
        c+=wall_surface(hit,eye+ray*hit.t,ray);
    }
    c*=.125;
    let ray=normalize(vec3f(xy,-5.5));
    wall_shift=-u.backdrop_motion.xy*.5;
    let hit=wall_trace(eye,ray,false);
    let p=eye+ray*hit.t;
    if hit.bump && u.state.x>=8.0 {
        let r=reflect(ray,hit.n);
        let metal=wall_metal(hit.at);
        c+=(reflected(p,r)-studio(r,p.x))*metal*.35;
    }
    wall_shift=vec2f(0);
    return max(c,vec3f(0))*wall_presence(wall_point(ray));
}
// The wall as seen through the glass letters: no detail at all, just its
// average tint and level, so the glass doesn't look textured.
fn wall_soft(ray:vec3f)->vec3f {
    let p=wall_point(ray);
    let metal=wall_metal(vec3f(.5,.5,0));
    return metal*studio(reflect(ray,vec3f(0,0,1)),p.x)*.8*wall_presence(p);
}
fn ripple(p:vec2f)->f32 {
    let t=clock();
    return sin(p.x*4.0+p.y*2.0-t*.7)*.014
        +sin(length(p-vec2f(.2,-.1))*9.0-t*1.5)*.006;
}
fn chamber(ray:vec3f)->vec3f {
    let p=vec3f(0,0,5.5)+ray*(6.4/-ray.z);
    let e=.012;
    let n=normalize(vec3f((ripple(p.xy-vec2f(e,0))-ripple(p.xy+vec2f(e,0)))/(2.0*e),
        (ripple(p.xy-vec2f(0,e))-ripple(p.xy+vec2f(0,e)))/(2.0*e),1));
    let entrance=smoothstep(-1.7,1.2,u.scroller.y);
    // The original's tile wall at its own scale and position, moving with the
    // oscillator, kept dim inside the chamber.
    var color=mix(vec3f(.012,.020,.030),vec3f(.006,.011,.019),entrance);
    color+=tile_image(wall_source(p.xy))*mix(.12,.30,entrance);
    let refl=reflect(ray,n);
    let strip=exp(-pow((refl.x+.62)*13.0,2.0))*.11+exp(-pow((refl.x-.65)*16.0,2.0))*.15;
    color+=heat(p.x)*strip*lighting(.88,.30+musical()*.5+bass()*1.4)*entrance;
    if u.state.x>=8.0 {
        let shadow_uv=uv_at(p.xy+vec2f(.045,-.065));
        let f=field(shadow_uv);
        // Soft projected contact shadows and rough reflected silhouettes.
        // These are analytic approximations, not traced indirect lighting.
        let shadow=smoothstep(-9.0,4.0,f.x)*field_coverage(shadow_uv);
        color*=1.0-shadow*.65;
        let reflection_uv=uv_at(p.xy+vec2f(-.035,.12));
        let reflected=exp(-abs(field(reflection_uv).x)*.20)*field_coverage(reflection_uv);
        color+=heat(p.x)*reflected*.022;
        color+=heat(p.x)*exp(-abs(f.y)*.23)*field_coverage(shadow_uv)*lighting(.0376,.012+musical()*.025+bass()*.14);
    }
    let haze=noise(p.xy*2.5+vec2f(clock()*.055,-clock()*.035));
    color+=heat(p.x)*pow(haze,3.0)*.022*entrance;
    return color;
}
fn backdrop(ray:vec3f)->vec3f {
    if u.backdrop.z>.5 {return wall_soft(ray);}
    return chamber(ray);
}
fn shadow(p:vec3f,light:vec3f)->f32 {
    let direction=normalize(light-p);var shade=1.0;var t=.035;
    for(var i=0;i<7;i++) {
        let d=scene(p+direction*t).x;
        shade=min(shade,10.0*max(d,0.0)/t);
        t+=clamp(d,.025,.16);
    }
    return clamp(shade,.18,1.0);
}
fn lamp(p:vec3f,n:vec3f,v:vec3f,position:vec3f,color:vec3f,f0:vec3f,rough:f32)->vec3f {
    let l=normalize(position-p);let h=normalize(l+v);
    let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),.001);let nh=max(dot(n,h),0.0);
    let vh=max(dot(v,h),0.0);let a=rough*rough;let a2=a*a;
    let d=a2/(PI*pow(nh*nh*(a2-1.0)+1.0,2.0));
    let k=(rough+1.0)*(rough+1.0)/8.0;
    let g=nv/(nv*(1.0-k)+k)*nl/(nl*(1.0-k)+k);
    let fresnel=f0+(1.0-f0)*pow(1.0-vh,5.0);
    let spec=d*g*fresnel/max(4.0*nv*nl,.001);
    return (spec+vec3f(.055))*color*nl*shadow(p+n*.008,position);
}
fn shade(p:vec3f,n0:vec3f,ray:vec3f,material:f32)->vec3f {
    let eye=-ray;let energy=musical();
    if material>.5 {
        let rim=pow(1.0-max(dot(n0,eye),0.0),2.0);
        let body=.55+.45*max(dot(n0,normalize(vec3f(-.4,.7,1))),0.0);
        return heat(p.x)*lighting(1.136,.10+energy*.25+bass()*4.0+attack()*.4)*body
            +vec3f(.10,.12,.14)*(1.0-rim);
    }
    let face=abs(n0.z);
    let grain=noise(p.xy*vec2f(260,900));
    let n=normalize(n0+vec3f((grain-.5)*.025,0,0)*face);
    let rough=mix(.15,.30,smoothstep(.6,.98,face));
    let f0=mix(vec3f(.61,.70,.78),vec3f(.75,.62,.49),warm(p.x)*.3);
    let sweep=clock()*.23*u.scroll_light.y;
    var color=f0*lighting(.07,.025);
    color+=lamp(p,n,eye,vec3f(-2.5,2.1,3.8),vec3f(.8,1.3,2.2)*lighting(1.098,.08+energy*.12+bass()*4.0),f0,rough);
    color+=lamp(p,n,eye,vec3f(2.5,-1.3,3.2),vec3f(2.4,.9,.30)*lighting(1.168,.08+energy*.15+bass()*4.8),f0,rough);
    color+=lamp(p,n,eye,vec3f(sin(sweep)*2.0,1.4,3.0),vec3f(1.0,1.1,1.2)*lighting(1.0,.08+attack()*1.8),f0,.36);
    let r=reflect(ray,n);
    let softbox=exp(-pow((r.y-.32)*4.8,2.0));
    let edgebox=exp(-pow((r.x+.6)*11.0,2.0));
    // Reflected sources must react too: a steady softbox previously dominated
    // the metal while the small direct-light changes disappeared in tonemapping.
    let reflected_light=mix(vec3f(1),heat(p.x)*1.7,u.scroll_light.y*.80);
    color+=f0*reflected_light*(softbox*.65+edgebox*.35)*lighting(1.0,.08+energy*.12+bass()*.80+attack()*.12);
    let fresnel=pow(1.0-max(dot(n,eye),0.0),4.0);
    let coating=mix(vec3f(.30,.55,.80),vec3f(.75,.40,.18),.5+.5*sin(dot(n,eye)*13.0+p.x));
    color+=coating*fresnel*.24;
    // A travelling filament lights the metal as it passes through its depth.
    let small=field(uv_at(p.xy)).y;
    color+=heat(p.x)*exp(-abs(small)*.16)*lighting(.234,.025+energy*.08+bass()*.65);
    if u.scroller.z>1.5 {
        // Experimental tinted solid glass: refracted chamber plus absorption,
        // a reflective coating and lit cut edges. Not a full multi-bounce trace.
        // The bump wall is fine detail: refract it through the smooth surface,
        // without the brushed grain, so it doesn't break up into noise.
        let refracted=normalize(ray+select(n,n0,u.backdrop.z>.5)*.085);
        let transmitted=backdrop(refracted)*mix(vec3f(.55,.83,.96),vec3f(.96,.65,.36),warm(p.x));
        color=mix(transmitted*1.35,color,.065+fresnel*.75);
        let transmitted_xy=p.xy+n.xy*.025;
        let behind=field(uv_at(transmitted_xy)).y;
        if filament_z(transmitted_xy)<p.z-.02 {
            color+=heat(p.x)*smoothstep(-.5,.8,behind)*.55;
        }
        color+=heat(p.x)*pow(1.0-face,2.0)*lighting(.462,.20+energy*.25+bass()*.85+attack()*.7);
    }
    return color;
}
@fragment fn scroller(v:Vertex)->@location(0) vec4f {
    let xy=(v.uv-.5)*vec2f(2.0*aspect(),-2.0);
    let ray=normalize(vec3f(xy,-5.5));
    var color=vec3f(0);
    if u.backdrop.z>.5 {color=wall_pixel(xy);} else {color=chamber(ray);}
    if u.state.x>=8.0 {
        let eye=vec3f(0,0,5.5);var t=5.05/-ray.z;var hit=false;var material=0.0;
        var p=eye+ray*t;
        for(var i=0;i<80;i++) {
            p=eye+ray*t;
            if p.z< -2.2 || abs(p.x)>aspect()*1.04 || abs(p.y)>.91 {break;}
            let sample=scene(p);
            if sample.x<.0012 {hit=true;material=sample.y;break;}
            t+=max(sample.x*.50,.001);
        }
        if hit {
            let n=normal(p,material);
            let surface=shade(p,n,ray,material);
            let depth=1.0-exp(-max(-p.z,0.0)*1.0);
            let edge=smoothstep(.87,1.0,abs(p.x)/aspect());
            color=mix(surface,color,max(depth*.65,edge));
        }
    }
    // Thin local haze receives the same cool/hot lighting. Keep the central
    // readable tracks clear and let the chamber depth show around the edges.
    let mist=noise(xy*3.0+vec2f(clock()*.07,-clock()*.045));
    let edge_haze=smoothstep(.35,1.5,abs(xy.x));
    color+=heat(xy.x)*pow(mist,3.0)*edge_haze*.025*smoothstep(-1.0,1.0,u.scroller.y);
    return vec4f(color*u.scroller.w,1);
}
