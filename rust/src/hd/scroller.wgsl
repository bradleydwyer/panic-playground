// Authored glass scroller. Letter outlines come from distance fields of the
// original font cells and overlay rows, built in memory from the local data
// (clip::ScrollerGlyphs). The text, the glyphs, both rows' positions, the
// overlay's sine x table and its row timing are read from the data and code;
// see Scroller::hd_scroll. The 4:3 proportions follow the original display.
// Depth, extrusion, the arc the rows follow, glass and lighting are new; none
// of it is a recovered 3D model. The tile wall's layout, image and motion are
// read from the port (see the backdrop section); the chrome bumps and their
// lighting, and the wall's lead-in motion before part 2's scroll, are authored.
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
fn bump_light(n:vec3f,p:vec3f,ray:vec3f,metal:vec3f)->vec3f {
    let r=reflect(ray,n);
    let fresnel=metal+(1.0-metal)*pow(1.0-max(dot(n,-ray),0.0),5.0)*.5;
    let c=fresnel*studio(r,p.x)+wall_glint(p,n,-ray,metal);
    // Faint occlusion where a bump meets the backing.
    return c*mix(.55,1.0,smoothstep(.0,.5,n.z));
}
// Shading of one wall sample, without the letter reflections.
fn wall_surface(hit:WallHit,p:vec3f,ray:vec3f)->vec3f {
    let metal=wall_metal(hit.at);
    // The level a flat tile would have: the backing and the relief are pulled
    // towards it, for roughly the original's subtle relief.
    let level=metal*studio(vec3f(0,0,1),p.x);
    if !hit.bump {return level*.45;}
    return mix(level,bump_light(hit.n,p,ray,metal),.6);
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
    for(var i=0;i<5;i++) {bumps+=bump_light(normalize(vec3f(SOFT_TILT[i],1)),p,ray,metal);}
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
@fragment fn scroller(v:Vertex)->@location(0) vec4f {
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
