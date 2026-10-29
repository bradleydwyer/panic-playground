// Authored glass scroller. Letter outlines come from distance fields of the
// original font cells and overlay rows, built in memory from the local data
// (clip::ScrollerGlyphs). The text, the glyphs, both rows' positions, the
// overlay's sine x table and its row timing are read from the data and code;
// see Scroller::hd_scroll. The 4:3 proportions follow the original display.
// Depth, extrusion, the arc the rows follow, glass and lighting are new; none
// of it is a recovered 3D model. The tile wall's layout, image and motion are
// read from the port (see the backdrop section); the chrome bumps and their
// lighting, the wall's lead-in motion before part 2's scroll and its optional
// waves are authored.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl.
@group(0) @binding(4) var linear:sampler;
@group(0) @binding(5) var glyph_atlas:texture_2d<f32>;
@group(0) @binding(6) var scroll_data:texture_2d<f32>;
// Both distance fields for this moment, rendered by glyph_field() below.
@group(0) @binding(7) var glyph_fields:texture_2d<f32>;
const PI=3.14159265;
// World units per source row: the 200 rows span 1.72 units, as before. A
// source column is 5/6 of a row on the original 4:3 display.
const ROW=.0086;
const ASPECT=5.0/6.0;
const COLUMN=ROW*ASPECT;
// clip::GLYPH_SCALE, GLYPH_PAD, GLYPH_COLUMNS and GLYPH_LIMIT.
const ATLAS_SCALE=4.0;
const ATLAS_PAD=16.0;
const ATLAS_COLUMNS=3u;
const CELL=vec2f(104,80);
const FAR=13.0;
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
fn source(p:vec2f)->vec2f {return vec2f(p.x/COLUMN+160.0,100.0-p.y/ROW);}
fn table(x:i32,row:i32)->vec4f {return textureLoad(scroll_data,vec2i(x,row),0);}
fn wrap(i:i32,n:i32)->i32 {return ((i%n)+n)%n;}
fn text_length()->i32 {return i32(table(1199,0).x);}
// Signed distance in source rows, positive inside, to the letter of one atlas
// cell; q is in source pixels from the cell's top left.
fn cell_distance(slot:f32,q:vec2f)->f32 {
    let outside=length(max(max(-q,q-CELL),vec2f(0))*vec2f(ASPECT,1));
    if slot<.5 || outside>FAR {return -max(FAR,outside);}
    let s=u32(slot)-1u;
    let size=(CELL+2.0*ATLAS_PAD)*ATLAS_SCALE;
    let origin=vec2f(f32(s%ATLAS_COLUMNS),f32(s/ATLAS_COLUMNS))*size;
    let texel=clamp((q+ATLAS_PAD)*ATLAS_SCALE,vec2f(.5),size-.5);
    return textureSampleLevel(glyph_atlas,linear,(origin+texel)/vec2f(textureDimensions(glyph_atlas)),0).r;
}
// One row of text at text pixel x (104 per character, wrapping) and glyph row y.
fn strip_distance(x:f32,y:f32)->f32 {
    let n=text_length();
    let cell=floor(x/104.0);
    let i=wrap(i32(cell),n);
    let q=vec2f(x-cell*104.0,y);
    let here=table(i,0);
    // Blank stretches: no letter within this many cells (precomputed).
    if here.y>1.5 {return -max(FAR,(here.y-1.0)*104.0*ASPECT);}
    var d=cell_distance(here.x,q);
    if q.x<ATLAS_PAD {d=max(d,cell_distance(table(wrap(i-1,n),0).x,q+vec2f(104,0)));}
    if q.x>104.0-ATLAS_PAD {d=max(d,cell_distance(table(wrap(i+1,n),0).x,q-vec2f(104,0)));}
    return d;
}
// The letter pixels' original grey at text pixel x and glyph row y.
fn strip_shade(x:f32,y:f32)->f32 {
    let cell=floor(x/104.0);
    let slot=table(wrap(i32(cell),text_length()),0).x;
    if slot<.5 {return 0.0;}
    let s=u32(slot)-1u;
    let size=(CELL+2.0*ATLAS_PAD)*ATLAS_SCALE;
    let origin=vec2f(f32(s%ATLAS_COLUMNS),f32(s/ATLAS_COLUMNS))*size;
    let texel=clamp((vec2f(x-cell*104.0,y)+ATLAS_PAD)*ATLAS_SCALE,vec2f(.5),size-.5);
    return textureSampleLevel(glyph_atlas,linear,(origin+texel)/vec2f(textureDimensions(glyph_atlas)),0).g;
}
fn rows_shade(p:vec2f)->f32 {
    let s=source(p);
    if s.y<100.0 {return strip_shade(s.x+u.glyphs.x,s.y-8.0);}
    return strip_shade(s.x+u.glyphs.y,s.y-112.0);
}
// The original rows: upper on source lines 8..88, lower on 112..192.
fn rows_distance(p:vec2f)->f32 {
    let s=source(p);
    if s.y<100.0 {return max(strip_distance(s.x+u.glyphs.x,s.y-8.0),s.y-112.0);}
    return max(strip_distance(s.x+u.glyphs.y,s.y-112.0),88.0-s.y);
}
// Overlay x position (source pixels) and slope (pixels per line) at a screen
// line. overlay() reads word 2044 + line - step of its table for each row; it
// only ever reads even words. A Catmull-Rom spline through those words, with
// their whole-pixel rounding smoothed, gives a continuous curve (authored).
fn sine_x(line:f32)->vec2f {
    let k=clamp((2044.0+line-u.glyphs.z)*.5,1.0,1196.0);
    let i=i32(floor(k));let t=k-floor(k);
    let a=table(i-1,1).y;let b=table(i,1).y;let c=table(i+1,1).y;let d=table(i+2,1).y;
    let m0=(c-a)*.5;let m1=(d-b)*.5;
    let t2=t*t;let t3=t2*t;
    let x=(2.0*t3-3.0*t2+1.0)*b+(t3-2.0*t2+t)*m0+(-2.0*t3+3.0*t2)*c+(t3-t2)*m1;
    let slope=(6.0*t2-6.0*t)*b+(3.0*t2-4.0*t+1.0)*m0+(-6.0*t2+6.0*t)*c+(3.0*t2-2.0*t)*m1;
    return vec2f(x,slope*.5);
}
fn box(p:vec2f,lo:vec2f,hi:vec2f,kx:f32)->f32 {
    var q=abs(p-(lo+hi)*.5)-(hi-lo)*.5;
    q.x*=kx;
    return length(max(q,vec2f(0)))+min(max(q.x,q.y),0.0);
}
// Signed distance in source rows, positive inside, to the overlay text: rows
// of up to 12 dots, 4 pixels apart, every 4 lines, moving up one line per
// step. Each dot becomes a 4 x 4 block centred on it, so rows join into
// strokes (authored). The displacement is continuous rather than per row.
// `rows` neighbouring rows each side are examined; beyond them only a lower
// bound is known. The cached field uses three, covering the glow's range.
fn sine_distance(s:vec2f,rows:i32)->f32 {
    let line=s.y+u.glyphs.z-4.0;
    let c0=i32(floor((line+1.5)*.25));
    let n=i32(table(1199,0).y);
    // Lower bounds: across blank rows between letters, and beside the 12 dots.
    let blank=(table(wrap(c0,n),3).x-1.0)*4.0;
    if blank>4.0 {return -blank;}
    let shear=sine_x(s.y);
    // Measure across the sheared strokes, not along the scanlines.
    let kx=ASPECT*inverseSqrt(1.0+shear.y*shear.y*ASPECT*ASPECT);
    let p=vec2f(s.x-shear.x,line);
    let reach=max(-1.5-p.x,p.x-46.5)*kx;
    if reach>f32(rows)*4.0 {return -reach;}
    // Rows beyond those examined are at least this far.
    let cap=min(f32(c0+rows)*4.0+2.5-p.y,p.y-f32(c0-rows)*4.0+1.5);
    var outside=cap;
    var gap=cap;
    for(var c=c0-rows;c<=c0+rows;c++) {
        let r=table(wrap(c,n),2)*4.0-1.5;
        let y=vec2f(f32(c)*4.0-1.5,f32(c)*4.0+2.5);
        outside=min(outside,box(p,vec2f(r.x,y.x),vec2f(r.y,y.y),kx));
        outside=min(outside,box(p,vec2f(r.z,y.x),vec2f(r.w,y.y),kx));
        gap=min(gap,box(p,vec2f(-1e3,y.x),vec2f(r.x,y.y),kx));
        gap=min(gap,box(p,vec2f(r.y,y.x),vec2f(r.z,y.y),kx));
        gap=min(gap,box(p,vec2f(r.w,y.x),vec2f(1e3,y.y),kx));
    }
    return select(-outside,gap,outside<0.0);
}
// World-space extent of the cached fields (renderer GLYPH_FIELD_SIZE texels).
// It reaches past the top and bottom of the screen at the overlay's depth.
const FIELD_EXTENT=vec2f(2.2,1.3);
// The rows' authored path: one arc of constant curvature, a vertical section
// of a cylinder whose front touches z = 0 at the screen centre. Letters wrap
// round it at their original size, measured along the surface, so they stay
// legible in the centre and turn away towards the sides. The radius scales
// with the screen so the cylinder's silhouette, where letters are edge-on,
// falls just beyond the left and right edges.
fn arc_radius()->f32 {return 1.55*aspect();}
// Half-angle of the visible arc, from the camera 5.5 in front of z = 0.
fn arc_limit()->f32 {let r=arc_radius();return acos(r/(5.5+r));}
// Widest surface x the camera can see on the arc.
fn arc_extent()->f32 {return arc_radius()*sin(arc_limit());}
// Distance along the surface from the centre to the point at world x.
fn arc_length(x:f32)->f32 {
    let r=arc_radius();
    return r*asin(clamp(x/r,-1.0,1.0));
}
// The cached fields cover the arc's whole visible width. Faithful HD uses the
// flat extent; the overlay channel is always flat world x, y.
fn field_extent()->vec2f {
    if u.misc.w>.5 {return vec2f(max(arc_extent(),FIELD_EXTENT.x),FIELD_EXTENT.y);}
    return FIELD_EXTENT;
}
// Letters and the sine text fade in over part 2's first 0.8 s. Both rows
// start empty, as the original's do, and the text then arrives;
// this only stops the first sine row, already at the screen's bottom edge,
// appearing at once. The end fades with the whole scene.
fn arrival()->f32 {return smoothstep(0.0,.8,u.scroller.y);}
// Render both fields for this moment: large letters and overlay, in source
// rows, positive inside, over the world plane the rows and overlay occupy.
@fragment fn glyph_field(v:Vertex)->@location(0) vec4f {
    let p=(v.uv*2.0-1.0)*vec2f(1,-1)*field_extent();
    var letters=0.0;
    if u.misc.w<.5 {
        letters=rows_distance(p);
    } else {
        // Letters at surface x lie this far along the arc from the centre.
        letters=rows_distance(vec2f(arc_length(p.x),p.y));
    }
    // Faithful HD also needs the letters' original grey and the plain dots.
    if u.misc.w<.5 {return vec4f(letters,-FAR,rows_shade(p),dot_distance(source(p)));}
    return vec4f(letters,sine_distance(source(p),3),0,1);
}
// Both cached fields at a world position, with a lower bound outside them.
fn fields(p:vec2f)->vec2f {
    let extent=field_extent();
    let uv=p/extent*vec2f(.5,-.5)+.5;
    let f=textureSampleLevel(glyph_fields,linear,clamp(uv,vec2f(0),vec2f(1)),0).rg;
    let outside=length(max(abs(p)-extent,vec2f(0)))/ROW;
    return select(f,min(f,vec2f(-outside)),outside>0.0);
}
fn letter_field(p:vec2f)->f32 {return fields(p).x;}
// Glow falling off with distance (rows), ending before the fields' exact range
// does; beyond it they hold only lower bounds.
fn halo(d:f32,falloff:f32)->f32 {return exp(-abs(d)*falloff)*(1.0-smoothstep(8.0,12.0,abs(d)));}
// Faithful HD: distance in source rows, positive inside, to the overlay's
// dots as overlay() draws them, one pixel each, every row sharing the x
// position at its line, but moving continuously.
fn dot_distance(s:vec2f)->f32 {
    let line=s.y+u.glyphs.z-4.0;
    let c0=i32(floor(line*.25+.5));
    let n=i32(table(1199,0).y);
    let blank=(table(wrap(c0,n),3).x-1.0)*4.0;
    if blank>4.0 {return -blank;}
    var best=3.0;
    for(var c=c0-1;c<=c0+1;c++) {
        let top=f32(c)*4.0;
        let x=sine_x(top-u.glyphs.z+4.0).x;
        let r=table(wrap(c,n),2);
        let j=round((s.x-x-.5)*.25);
        for(var k=0;k<2;k++) {
            let run=select(r.xy,r.zw,k==1);
            if run.x<run.y {
                let d=x+4.0*clamp(j,run.x,run.y-1.0);
                best=min(best,box(vec2f(s.x,line),vec2f(d,top),vec2f(d+1.0,top+1.0),ASPECT));
            }
        }
    }
    return -best;
}
// Projected shadows and reflections fade where the rows turn away.
fn field_coverage(p:vec2f)->f32 {
    let x=arc_extent();
    return 1.0-smoothstep(.8*x,x,abs(p.x));
}
// Depth of the rows' surface at world x.
fn ribbon_z(x:f32)->f32 {
    let r=arc_radius();
    return sqrt(max(r*r-x*x,0.0))-r;
}
fn filament_z(p:vec2f)->f32 {
    // Keep a fixed row order: the vertical text passes over the lower row and
    // under the upper row. Follow the rows' arc so this holds across it.
    // The depth crossover stays in the gap between the two horizontal rows.
    return ribbon_z(p.x)-.075+mix(.30,-.30,smoothstep(-.08,.08,p.y));
}
fn extrusion(d:f32,z:f32,half_depth:f32,bevel:f32)->f32 {
    let q=vec2f(d+bevel,abs(z)-half_depth+bevel);
    return min(max(q.x,q.y),0.0)+length(max(q,vec2f(0)))-bevel;
}
fn sine_at(p:vec2f)->f32 {return fields(p).y;}
// Radial depth of a point from the rows' surface, and the surface x on its
// radius, where the rows' field channel holds the arc's letters.
fn on_arc(p:vec3f)->vec2f {
    let r=arc_radius();
    let c=vec2f(p.x,p.z+r);
    let rho=length(c);
    return vec2f(rho-r,c.x*r/max(rho,1e-4));
}
const BEVEL=.018; // The letters' rounded edge; the faces are flat inside it.
fn letter_shape(p:vec3f)->f32 {
    let a=on_arc(p);
    return extrusion(-fields(vec2f(a.y,p.y)).x*ROW,a.x+.075,.12,BEVEL);
}
fn filament_shape(p:vec3f)->f32 {
    // Authored depth fixes the overlay's layering relative to the rows.
    return extrusion(-(fields(p.xy).y+.20)*ROW,p.z-filament_z(p.xy),.018,.009);
}
fn shape(p:vec3f,material:f32)->f32 {
    if material>.5 {return filament_shape(p);}
    return letter_shape(p);
}
fn scene(p:vec3f)->vec2f {
    // Each slab's depth alone bounds its distance: skip the field lookup of
    // whichever can't be nearer.
    let letter_bound=abs(on_arc(p).x+.075)-.12;
    let filament_bound=abs(p.z-filament_z(p.xy))-.018;
    if filament_bound<letter_bound {
        let b=filament_shape(p);
        if letter_bound>=b {return vec2f(b,1);}
        let a=letter_shape(p);
        return select(vec2f(a,0),vec2f(b,1),b<a);
    }
    let a=letter_shape(p);
    if filament_bound>=a {return vec2f(a,0);}
    let b=filament_shape(p);
    return select(vec2f(a,0),vec2f(b,1),b<a);
}
fn normal(p:vec3f,material:f32)->vec3f {
    let e=vec2f(.008,0);
    return normalize(vec3f(
        shape(p+e.xyy,material)-shape(p-e.xyy,material),
        shape(p+e.yxy,material)-shape(p-e.yxy,material),
        shape(p+e.yyx,material)-shape(p-e.yyx,material)));
}
// ---- Backdrop -------------------------------------------------------------
// The wall's layout, bumps, waves and ray intersections are in wall.wgsl,
// which this module is built with; its shading follows.
// The bump wall is drawn as a defocused, motion-blurred background: soft,
// dim and low in contrast, so the letters stay the brightest and sharpest
// thing on screen and the fast original sway doesn't strobe. All authored.
// u.backdrop_motion.xy: the oscillator's movement during the exposure (source
// pixels); .z: wall strength, the owner's presence control. The per-sample
// offset itself, wall_shift, is declared in wall.wgsl.
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
// scene() a few steps through the slab they occupy, else the studio. The slab
// follows the rows' arc back from the screen plane, but not behind the wall. Reflected letters are seen from behind, so they are
// shaded simply: the travelling filament glows, the bodies are dark glass that
// hides the studio behind them.
fn reflected(p:vec3f,r:vec3f)->vec3f {
    let env=studio(r,p.x);
    if u.state.x<8.0 || r.z<.05 {return env;}
    let start=max(min(-.42,ribbon_z(p.x)-.42),p.z);
    var t=(start-p.z)/r.z;let end=(.30-p.z)/r.z;
    let wide=vec2f(field_extent().x+.1,1.25);
    for(var i=0;i<14;i++) {
        let q=p+r*t;
        if t>end || abs(q.x)>wide.x || abs(q.y)>wide.y {break;}
        let d=scene(q);
        if d.x<.004 {
            var seen=mix(vec3f(.05,.10,.14),vec3f(.14,.08,.04),warm(q.x))+heat(q.x)*.02;
            if d.y>.5 {seen=heat(q.x)*lighting(.8,.08+musical()*.2+bass()*2.0);}
            return mix(env,seen,arrival());
        }
        t+=max(d.x,.012);
    }
    return env;
}
// Light from a bump with normal n, before it is pulled towards the level.
// up: how far n faces away from the backing (n.z on a flat wall).
fn bump_light(n:vec3f,p:vec3f,ray:vec3f,metal:vec3f,up:f32)->vec3f {
    let r=reflect(ray,n);
    let fresnel=metal+(1.0-metal)*pow(1.0-max(dot(n,-ray),0.0),5.0)*.5;
    let c=fresnel*studio(r,p.x)+wall_glint(p,n,-ray,metal);
    // Faint occlusion where a bump meets the backing.
    return c*mix(.55,1.0,smoothstep(.0,.5,up));
}
// Shading of one wall sample, without the letter reflections.
fn wall_surface(hit:WallHit,p:vec3f,ray:vec3f)->vec3f {
    let metal=wall_metal(hit.at);
    // The level a flat tile would have: the backing and the relief are pulled
    // towards it, for roughly the original's subtle relief.
    let level=metal*studio(vec3f(0,0,1),p.x);
    if !hit.bump {return level*.45;}
    // On waves, the bump's base faces along the wave's normal.
    let up=select(hit.n.z,dot(hit.n,wave_up),waving());
    return mix(level,bump_light(hit.n,p,ray,metal,up),.6);
}
// Soft contact shadow of the letters on the chamber or wall point p: the
// letters seen straight on (the cached field is indexed by surface x on the
// arc), displaced slightly as if lit from the front. An analytic
// approximation, not a traced shadow.
fn letter_shadow(p:vec3f)->f32 {
    if u.state.x<8.0 {return 0.0;}
    let at=p.xy+vec2f(.045,-.065);
    return smoothstep(-9.0,4.0,letter_field(at))*field_coverage(at)*arrival();
}
// How far the letters' soft shadow darkens the bump wall and the chamber, and
// the same backdrop seen through a letter, lit through that glass (authored).
const WALL_SHADOW=.7;
const CHAMBER_SHADOW=.65;
const GLASS_SHADOW=.25;
// Overall presence: the strength control, a gentle corner vignette and the
// letters' shade. No horizontal banding: darkening the text rows, or an
// ellipse stretched across the wide frame, read as a bright stripe.
fn wall_presence(p:vec3f,darken:f32)->f32 {
    let xy=p.xy*(5.5/6.4); // Back to screen units: x in +-aspect, y in +-1.
    let corner=length(vec2f(xy.x/aspect(),xy.y))*.7071;
    let vignette=mix(.6,1.0,1.0-smoothstep(.75,1.05,corner));
    let entrance=smoothstep(-1.7,1.2,u.scroller.y);
    // .58: the calm level the row bands used to give, now applied evenly.
    return u.backdrop_motion.z*.9*.58*vignette*mix(.55,1.0,entrance)*(1.0-letter_shadow(p)*darken);
}
// The wall's own grazing side light, for the folds only (authored): from the
// upper left, the scene's cool side, 35 degrees above the horizontal and 18
// over the wall, so that folds lying either way throw shadows across their
// creases. The letters' lamps and the studio are unchanged.
const SIDE=vec3f(-.779,.546,.309);
const SIDE_COLOR=vec3f(.72,.86,1.0);
// The waves at the pixel at screen point xy (a screen unit is 15 tile widths
// both ways), once, at the middle of its exposure, where its ray meets them.
// Then how much of the side light the crests leave it: the steepest rise of
// the waves towards the light, with the bumps on top, against the light's
// elevation, softened over the light's breadth.
fn find_waves(xy:vec2f) {
    let lag=u.waves.y*.5;
    if !folded() {
        wave_m=xy*15.0;
        wave_here=Wave(0.0,vec2f(0),0.0);
        wave_up=vec3f(0,0,1);
        wave_lit=1.0;
        wave_bend=vec3f(0);
        wave_at=vec2f(0);
        return;
    }
    // A raised point lies nearer the centre along the pixel's ray: by its
    // height over 82.5 tile widths, 15 to a screen unit times the 5.5 focal
    // length.
    wave_m=xy*15.0;
    wave_m*=1.0-wave_field(wave_m,lag).h/82.5;
    wave_here=wave_field(wave_m,lag);
    wave_up=normalize(vec3f(-wave_here.slope,1));
    // The curvature, from the slope a quarter tile on each way, for the
    // tiles near the pixel (grid_wave).
    let e=.25;
    let ax=(wave_field(wave_m+vec2f(e,0.0),lag).slope-wave_here.slope)/e;
    let ay=(wave_field(wave_m+vec2f(0.0,e),lag).slope-wave_here.slope)/e;
    wave_bend=vec3f(ax.x,.5*(ax.y+ay.x),ay.y);
    wave_at=fold_point(wave_m,lag);
    let toward=normalize(SIDE.xy);let rise=SIDE.z/length(SIDE.xy);
    var horizon=-9.0;
    for(var i=1;i<=6;i++) {
        let s=f32(i*i)*.45+.6;
        horizon=max(horizon,(wave_height(wave_m+toward*s,lag)+.12-wave_here.h)/s);
    }
    wave_lit=1.0-smoothstep(rise-.08,rise+.08,horizon);
}
// The side light on one sample of rough metal: a little diffuse catch on
// the faces turned to it and a broad sheen between it and the eye, cut off by
// the bumps (bumps_pass). `spread` moves it over the light's breadth. It
// follows the waves' slope more than each bump's, so the bumps don't sparkle.
fn side_light(hit:WallHit,p:vec3f,ray:vec3f,spread:vec2f)->vec3f {
    let l=normalize(SIDE+vec3f(spread,0));
    if dot(hit.n,l)<=0.0 {return vec3f(0);}
    let clear=bumps_pass(grid_point(p+hit.n*.002),grid_direction(l));
    if clear<=0.0 {return vec3f(0);}
    let n=normalize(hit.n+wave_up*1.5);
    let sheen=pow(max(dot(n,normalize(l-ray)),0.0),6.0);
    var c=SIDE_COLOR*wall_metal(hit.at)*(max(dot(n,l),0.0)*.35+sheen*.9);
    if !hit.bump {c*=.45;}
    return c*clear;
}
fn wall_point(ray:vec3f)->vec3f {return vec3f(0,0,5.5)+ray*(6.4/-ray.z);}
// Eight samples per pixel spread over a small defocus disc (about a tenth of
// a tile) and over the exposure, so the wall blurs along its own motion. The
// letter reflections are traced once per pixel and softened.
fn wall_pixel(xy:vec2f)->vec3f {
    let eye=vec3f(0,0,5.5);
    let defocus=.0075; // Screen units; a tile is .067 wide.
    var c=vec3f(0);
    var side=vec3f(0);
    if waving() {find_waves(xy);}
    for(var i=0;i<8;i++) {
        let f=f32(i);
        let disc=sqrt((f+.5)/8.0)*defocus*vec2f(cos(f*2.39996),sin(f*2.39996));
        // Stratified in time, in an order that doesn't follow the disc.
        wall_shift=-u.backdrop_motion.xy*fract((f*5.0+.5)/8.0);
        wave_lag=u.waves.y*fract((f*5.0+.5)/8.0);
        let ray=normalize(vec3f(xy+disc,-5.5));
        let hit=wall_trace(eye,ray,false);
        c+=wall_surface(hit,eye+ray*hit.t,ray);
        // Every fourth sample takes the side light, each from another part
        // of it, about 9 degrees across; none reaches a pixel deep in a
        // crest's shadow.
        if waving() && i%4==0 && wave_lit*u.waves.z>0.0 {
            side+=side_light(hit,eye+ray*hit.t,ray,disc*(.08/defocus));
        }
    }
    c*=.125;
    // The side light takes over from the studio as it strengthens, so the
    // wall's level stays calm and the shadows read.
    if waving() {c=c*mix(1.0,.1,u.waves.z)+side*(.5*wave_lit*u.waves.z);}
    let ray=normalize(vec3f(xy,-5.5));
    wall_shift=-u.backdrop_motion.xy*.5;
    wave_lag=u.waves.y*.5;
    let hit=wall_trace(eye,ray,false);
    let p=eye+ray*hit.t;
    if hit.bump && u.state.x>=8.0 {
        let r=reflect(ray,hit.n);
        let metal=wall_metal(hit.at);
        c+=(reflected(p,r)-studio(r,p.x))*metal*.35;
    }
    wall_shift=vec2f(0);
    return max(c,vec3f(0))*wall_presence(wall_point(ray),WALL_SHADOW);
}
// The wall as seen through the glass letters: never its tile detail, only its
// light at low frequency, so the glass doesn't look textured. A few bump
// normals stand in for a tile's spread of them, so the studio and lamp light
// the wall carries shows through, blurred.
const SOFT_TILT=array<vec2f,5>(vec2f(0),vec2f(.5,.5),vec2f(-.5,.5),vec2f(.5,-.5),vec2f(-.5,-.5));
fn wall_soft(ray:vec3f)->vec3f {
    let p=wall_point(ray);
    let metal=wall_metal(vec3f(.5,.5,0));
    let level=metal*studio(vec3f(0,0,1),p.x);
    var bumps=vec3f(0);
    for(var i=0;i<5;i++) {let n=normalize(vec3f(SOFT_TILT[i],1));bumps+=bump_light(n,p,ray,metal,n.z);}
    // About a quarter of a tile is the dimmer backing between bumps.
    return mix(mix(level,bumps*.2,.6),level*.45,.3)*wall_presence(p,GLASS_SHADOW);
}
fn ripple(p:vec2f)->f32 {
    let t=clock();
    return sin(p.x*4.0+p.y*2.0-t*.7)*.014
        +sin(length(p-vec2f(.2,-.1))*9.0-t*1.5)*.006;
}
fn chamber(ray:vec3f,darken:f32)->vec3f {
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
        // Soft projected contact shadows and rough reflected silhouettes of
        // the letters on the arc, as on the bump wall.
        // These are analytic approximations, not traced indirect lighting.
        let present=arrival();
        color*=1.0-letter_shadow(p)*darken;
        let reflection_at=p.xy+vec2f(-.035,.12);
        let reflected=halo(letter_field(reflection_at),.20)*field_coverage(reflection_at);
        color+=heat(p.x)*reflected*.022*present;
        let glow_at=p.xy+vec2f(.045,-.065);
        color+=heat(p.x)*halo(sine_at(glow_at),.23)*field_coverage(glow_at)*present*lighting(.0376,.012+musical()*.025+bass()*.14);
    }
    let haze=noise(p.xy*2.5+vec2f(clock()*.055,-clock()*.035));
    color+=heat(p.x)*pow(haze,3.0)*.022*entrance;
    return color;
}
fn backdrop(ray:vec3f)->vec3f {
    if u.backdrop.z>.5 {return wall_soft(ray);}
    return chamber(ray,GLASS_SHADOW);
}
fn shadow(p:vec3f,light:vec3f)->f32 {
    let direction=normalize(light-p);var shade=1.0;var t=.035;
    for(var i=0;i<7;i++) {
        let q=p+direction*t;
        // Every letter and the overlay lie behind z = .25, and the lamps are
        // well in front: past this point no later step can darken the shade.
        if (q.z-.25)*10.0>t {break;}
        let d=scene(q).x;
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
// Glass letters (authored): the share of the backdrop's light that passes
// through, the lamp light a face gathers and lets out in a band just inside
// its bevel, that band's width (source rows), and the filament's light
// spreading through the glass as it passes.
const GLASS_LIGHT=1.6;
const GLASS_RIM=.08;
const GLASS_EDGE_ROWS=2.5;
const FILAMENT_IN_GLASS=.25;
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
    let key=vec3f(.8,1.3,2.2)*lighting(1.098,.08+energy*.12+bass()*4.0);
    let warm_lamp=vec3f(2.4,.9,.30)*lighting(1.168,.08+energy*.15+bass()*4.8);
    let top=vec3f(1.0,1.1,1.2)*lighting(1.0,.08+attack()*1.8);
    var color=f0*lighting(.07,.025);
    color+=lamp(p,n,eye,vec3f(-2.5,2.1,3.8),key,f0,rough);
    color+=lamp(p,n,eye,vec3f(2.5,-1.3,3.2),warm_lamp,f0,rough);
    color+=lamp(p,n,eye,vec3f(sin(sweep)*2.0,1.4,3.0),top,f0,.36);
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
    let small=sine_at(p.xy);
    color+=heat(p.x)*halo(small,.16)*lighting(.234,.025+energy*.08+bass()*.65);
    // Tinted solid glass over the coated metal shading above: refracted
    // backdrop plus absorption, a reflective coating and lit cut edges. Not a
    // full multi-bounce trace. The bump wall is fine detail: refract it
    // through the smooth surface, without the brushed grain.
    let seen=backdrop(normalize(ray+select(n,n0,u.backdrop.z>.5)*.085));
    // Lamp light gathered through the face crosses the body, tinted twice by
    // the long path, and leaves where the glass ends: a band just inside the
    // bevel, so the crisp bevel line stays dark. The letter's field gives the
    // distance from the outline in rows. Not traced light transport.
    let tint=mix(vec3f(.55,.83,.96),vec3f(.96,.65,.36),warm(p.x));
    let a=on_arc(p);
    let inset=fields(vec2f(a.y,p.y)).x-BEVEL/ROW;
    let rim=smoothstep(-.5,.5,inset)*exp(-max(inset,0.0)/GLASS_EDGE_ROWS);
    let gathered=(key+warm_lamp+top)*tint*tint*rim*GLASS_RIM;
    let transmitted=seen*tint*GLASS_LIGHT+gathered;
    color=mix(transmitted,color,.065+fresnel*.75);
    let transmitted_xy=p.xy+n.xy*.025;
    let behind=sine_at(transmitted_xy);
    if filament_z(transmitted_xy)<p.z-.02 {
        color+=heat(p.x)*smoothstep(-.5,.8,behind)*.55;
    }
    // The filament's light spreads through the glass on either side of it.
    // Added after the mix, so undamped, it must be gone by 8 rows: from there
    // the sine field holds a lower bound per blank row, which steps line by
    // line and would draw stripes across every letter.
    color+=heat(p.x)*tint*halo(behind,.25)*(1.0-smoothstep(5.0,8.0,abs(behind)))*FILAMENT_IN_GLASS;
    color+=heat(p.x)*pow(1.0-face,2.0)*lighting(.462,.20+energy*.25+bass()*.85+attack()*.7);
    return color;
}
// Authored exposure of part 2's whole scene, before the shared tonemap.
const SCROLLER_EXPOSURE=1.2;
// During the laser cut, nothing reads the scene under a plate that still lies
// flat: in transition.wgsl a plate hides its own tile until it has been cut,
// it takes nothing of the scene before then (tile_fragment) and, while it
// lies at z = 0, laser_material's fade is exactly 0. Such pixels skip the
// scene. A margin at each tile's edges leaves alone any crack between
// plates, cut walls seen past a neighbour that has fallen, and the filtered
// reads, which reach the next pixel.
const CUT_GRID=vec2f(16,10);   // transition.wgsl's GRID
const CUT_FLAT_AGE=1.4;        // in its tile ages: the cut ends at 7 × LASER_CUT, 1.47
const CUT_MARGIN=3.0;          // pixels
fn under_flat_plate(pixel:vec2f)->bool {
    // The renderer draws the plates until 78 ticks after the cut's end.
    if u.transition.y<.5 || u.transition.x<0.0 || u.transition.x>=u.transition.z+78.0 {return false;}
    let scale=CUT_GRID/u.screen.xy;
    let cell=floor(pixel*scale);
    if any(floor((pixel-CUT_MARGIN)*scale)!=cell) || any(floor((pixel+CUT_MARGIN)*scale)!=cell) {return false;}
    let id=u32(cell.y)*16u+u32(cell.x);
    return (u.transition.x-u.tile_order[id/4u][id%4u])/10.0<CUT_FLAT_AGE;
}
@fragment fn scroller(v:Vertex)->@location(0) vec4f {
    if under_flat_plate(v.position.xy) {return vec4f(0,0,0,1);}
    let xy=(v.uv-.5)*vec2f(2.0*aspect(),-2.0);
    let ray=normalize(vec3f(xy,-5.5));
    var color=vec3f(0);
    if u.backdrop.z>.5 {color=wall_pixel(xy);} else {color=chamber(ray,CHAMBER_SHADOW);}
    if u.state.x>=8.0 {
        let eye=vec3f(0,0,5.5);var t=5.05/-ray.z;var hit=false;var material=0.0;
        var p=eye+ray*t;
        let deepest=arc_radius()*(cos(arc_limit())-1.0)-.3;
        let wide=vec2f(field_extent().x+.1,1.25);
        for(var i=0;i<80;i++) {
            p=eye+ray*t;
            if p.z<deepest || abs(p.x)>wide.x || abs(p.y)>wide.y {break;}
            // Behind both the rows' slab and the overlay, a ray can only leave
            // the cylinder past its silhouette, where there are no letters.
            if on_arc(p).x< -.3 && p.z<filament_z(p.xy)-.05 {break;}
            let sample=scene(p);
            if sample.x<.0012 {hit=true;material=sample.y;break;}
            t+=max(sample.x*.50,.001);
        }
        if hit {
            let n=normal(p,material);
            let surface=shade(p,n,ray,material);
            let depth=1.0-exp(-max(-p.z,0.0));
            // Letters on the arc fade as they turn edge-on, past the sides.
            let turn=abs(atan2(p.x,p.z+arc_radius()));
            let limit=arc_limit();
            let edge=smoothstep(limit-.35,limit-.04,turn);
            color=mix(color,mix(surface,color,max(depth*.65,edge)),arrival());
        }
    }
    // Thin local haze receives the same cool/hot lighting. Keep the central
    // readable tracks clear and let the chamber depth show around the edges.
    let mist=noise(xy*3.0+vec2f(clock()*.07,-clock()*.045));
    let edge_haze=smoothstep(.35,1.5,abs(xy.x));
    color+=heat(xy.x)*pow(mist,3.0)*edge_haze*.025*smoothstep(-1.0,1.0,u.scroller.y);
    return vec4f(color*SCROLLER_EXPOSURE*u.scroller.w,1);
}
