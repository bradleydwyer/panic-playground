// Authored glass scroller. Letter outlines come from distance fields of the
// original font cells and overlay rows, built in memory from the local data
// (clip::ScrollerGlyphs). The text, the glyphs, both rows' positions, the
// overlay's sine x table and its row timing are read from the data and code;
// see Scroller::hd_scroll. The 4:3 proportions follow the original display.
// Depth, extrusion, the arc the rows follow, the rings, glass and lighting are
// new; none of it is a recovered 3D model.
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
fn ring()->bool {return u.glyphs.w>.5;}
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
// The cached fields cover the arc's whole visible width. Faithful HD and the
// ring use the flat extent; the overlay channel is always flat world x, y.
fn field_extent()->vec2f {
    if u.misc.w>.5 && !ring() {return vec2f(max(arc_extent(),FIELD_EXTENT.x),FIELD_EXTENT.y);}
    return FIELD_EXTENT;
}
// The authored ring experiment: two rings of letters around vertical axes,
// one per row, seen level from between them. Each ring's front, facing the
// camera on its row's lines (8-88 and 112-192), shows exactly what that row
// shows at the centre of the 4:3 frame, and turns the row's way: the upper
// ring's front moves left, the lower's right. Going round, the letters pass
// behind and cross the far side, seen from behind (mirrored), in the
// background between and behind the fronts. The text is longer than a ring,
// so it changes at a seam at the back, where letters fade out and in.
const RING_RADIUS=2.0;
// Angle over which letters fade either side of the seam at the back.
const RING_FADE=1.0;
// The front of ring i (0 upper, 1 lower) is centred on line 48 or 152.
fn ring_centre(i:i32)->vec3f {return vec3f(0,select(.4472,-.4472,i==1),-RING_RADIUS);}
fn ring_of(p:vec3f)->i32 {return select(0,1,p.y<0.0);}
// Angle round ring i from its front, radial depth from its surface, glyph row.
fn ring_local(p:vec3f,i:i32)->vec3f {
    let q=p-ring_centre(i);
    return vec3f(atan2(q.x,q.z),length(q.xz)-RING_RADIUS,40.0-q.y/ROW);
}
// Letters and the sine text fade in over part 2's first 0.8 s. Both rows
// and rings start empty, as the original's do, and the text then arrives;
// this only stops the first sine row, already at the screen's bottom edge,
// appearing at once. The end fades with the whole scene.
fn arrival()->f32 {return smoothstep(0.0,.8,u.scroller.y);}
// Letters fade out towards the seam at the back and in after it.
fn ring_fade(phi:f32)->f32 {return smoothstep(0.0,1.0,(PI-abs(phi))/RING_FADE);}
// Each ring's turn maps to two bands of the field texture, the front half and
// the back half, each 112 glyph rows (the cell and its margins) tall.
fn ring_texel(phi:f32,gy:f32,i:i32)->vec2f {
    let size=vec2f(textureDimensions(glyph_fields));
    let turn=fract((phi+.5*PI)/(2.0*PI))*2.0;
    let band=min(floor(turn),1.0);
    let x=clamp((turn-band)*size.x,.5,size.x-.5);
    let y=clamp((gy+ATLAS_PAD)/(CELL.y+2.0*ATLAS_PAD)*size.y*.25,.5,size.y*.25-.5);
    return vec2f(x,(f32(i)*2.0+band)*size.y*.25+y)/size;
}
fn ring_distance(phi:f32,gy:f32,i:i32)->f32 {
    let offset=select(u.glyphs.x,u.glyphs.y,i==1);
    return strip_distance(160.0+offset+RING_RADIUS*phi/COLUMN,gy);
}
// Render both fields for this moment: large letters and overlay, in source
// rows, positive inside, over the world plane the rows and overlay occupy.
// In the ring layout, the letters' channel holds the ring's bands instead.
@fragment fn glyph_field(v:Vertex)->@location(0) vec4f {
    let p=(v.uv*2.0-1.0)*vec2f(1,-1)*field_extent();
    var letters=0.0;
    if ring() {
        let band=floor(v.uv.y*4.0);
        // The back band runs on past pi: wrap it, so the seam sits at the back.
        var phi=(band%2.0+v.uv.x)*PI-.5*PI;
        if phi>PI {phi-=2.0*PI;}
        let gy=fract(v.uv.y*4.0)*(CELL.y+2.0*ATLAS_PAD)-ATLAS_PAD;
        letters=ring_distance(phi,gy,i32(band/2.0));
    } else if u.misc.w<.5 {
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
// Depth of the rows' surface (or the rings' fronts) at world x.
fn ribbon_z(x:f32)->f32 {
    let r=select(arc_radius(),RING_RADIUS,ring());
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
// Letters on the nearer ring. As they fade near the seam they also sink a
// little into the ring.
fn ring_letters(p:vec3f)->f32 {
    let i=ring_of(p);
    let l=ring_local(p,i);
    var d=-max(FAR,max(-ATLAS_PAD-l.z,l.z-CELL.y-ATLAS_PAD));
    if d>-FAR-.5 {d=textureSampleLevel(glyph_fields,linear,ring_texel(l.x,l.z,i),0).r;}
    let sink=(1.0-ring_fade(l.x))*.15;
    return extrusion(-d*ROW*min(1.0+l.y/RING_RADIUS,1.0),l.y+sink,.12,.018);
}
// Radial depth of a point from the rows' surface, and the surface x on its
// radius, where the rows' field channel holds the arc's letters.
fn on_arc(p:vec3f)->vec2f {
    let r=arc_radius();
    let c=vec2f(p.x,p.z+r);
    let rho=length(c);
    return vec2f(rho-r,c.x*r/max(rho,1e-4));
}
fn letter_shape(p:vec3f)->f32 {
    if ring() {return ring_letters(p);}
    let a=on_arc(p);
    return extrusion(-fields(vec2f(a.y,p.y)).x*ROW,a.x+.075,.12,.018);
}
fn filament_shape(p:vec3f)->f32 {
    // Authored depth fixes the overlay's layering relative to the rows: in
    // the ring layout it stands inside the upper ring and before the lower.
    return extrusion(-(fields(p.xy).y+.20)*ROW,p.z-filament_z(p.xy),.018,.009);
}
fn shape(p:vec3f,material:f32)->f32 {
    if material>.5 {return filament_shape(p);}
    return letter_shape(p);
}
fn scene(p:vec3f)->vec2f {
    // Each slab's depth alone bounds its distance: skip the field lookup of
    // whichever can't be nearer.
    var letter_bound=0.0;
    var filament_bound=0.0;
    if !ring() {
        letter_bound=abs(on_arc(p).x+.075)-.12;
        filament_bound=abs(p.z-filament_z(p.xy))-.018;
    }
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
fn palette(p:vec2f)->vec3f {
    let at=fract(p)*8.0;
    let a=vec2u(floor(at));let f=fract(at);let s=f*f*(3.0-2.0*f);
    let b=(a+1u)%8u;
    return mix(mix(u.next_palette[a.y*8u+a.x].rgb,u.next_palette[a.y*8u+b.x].rgb,s.x),
        mix(u.next_palette[b.y*8u+a.x].rgb,u.next_palette[b.y*8u+b.x].rgb,s.x),s.y);
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
    let refl=reflect(ray,n);
    let entrance=smoothstep(-1.7,1.2,u.scroller.y);
    let pattern=palette(p.xy*.22+vec2f(ripple(p.xy)));
    var color=mix(vec3f(.012,.020,.030),vec3f(.006,.011,.019),entrance);
    color+=pattern*.045*entrance;
    let grid=abs(fract(p.xy*vec2f(7,9))-.5);
    let seams=exp(-min(grid.x,grid.y)*160.0);
    color+=heat(p.x)*seams*.006*entrance;
    let strip=exp(-pow((refl.x+.62)*13.0,2.0))*.11+exp(-pow((refl.x-.65)*16.0,2.0))*.15;
    color+=heat(p.x)*strip*lighting(.88,.30+musical()*.5+bass()*1.4)*entrance;
    if u.state.x>=8.0 {
        let shadow_at=p.xy+vec2f(.045,-.065);
        // The ring's letters cast no projected shadow; its field holds bands.
        let f=select(fields(shadow_at),vec2f(-99.0,sine_at(shadow_at)),ring());
        // Soft projected contact shadows and rough reflected silhouettes.
        // These are analytic approximations, not traced indirect lighting.
        let present=arrival();
        let shadow=smoothstep(-9.0,4.0,f.x)*field_coverage(shadow_at)*present;
        color*=1.0-shadow*.65;
        let reflection_at=p.xy+vec2f(-.035,.12);
        let reflected=select(halo(letter_field(reflection_at),.20),0.0,ring())*field_coverage(reflection_at);
        color+=heat(p.x)*reflected*.022*present;
        color+=heat(p.x)*halo(f.y,.23)*field_coverage(shadow_at)*present*lighting(.0376,.012+musical()*.025+bass()*.14);
    }
    let haze=noise(p.xy*2.5+vec2f(clock()*.055,-clock()*.035));
    color+=heat(p.x)*pow(haze,3.0)*.022*entrance;
    return color;
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
// Ring layout only: glass shows this colour, traced behind it, instead of the
// chamber; and surfaces seen through other glass skip their shadow rays.
var<private> traced:bool=false;
var<private> traced_behind:vec3f=vec3f(0);
var<private> unshadowed:bool=false;
fn lamp(p:vec3f,n:vec3f,v:vec3f,position:vec3f,color:vec3f,f0:vec3f,rough:f32)->vec3f {
    let l=normalize(position-p);let h=normalize(l+v);
    let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),.001);let nh=max(dot(n,h),0.0);
    let vh=max(dot(v,h),0.0);let a=rough*rough;let a2=a*a;
    let d=a2/(PI*pow(nh*nh*(a2-1.0)+1.0,2.0));
    let k=(rough+1.0)*(rough+1.0)/8.0;
    let g=nv/(nv*(1.0-k)+k)*nl/(nl*(1.0-k)+k);
    let fresnel=f0+(1.0-f0)*pow(1.0-vh,5.0);
    let spec=d*g*fresnel/max(4.0*nv*nl,.001);
    var shaded=1.0;
    if !unshadowed {shaded=shadow(p+n*.008,position);}
    return (spec+vec3f(.055))*color*nl*shaded;
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
    let small=sine_at(p.xy);
    color+=heat(p.x)*halo(small,.16)*lighting(.234,.025+energy*.08+bass()*.65);
    // Tinted solid glass over the coated metal shading above: refracted
    // chamber plus absorption, a reflective coating and lit cut edges. Not a
    // full multi-bounce trace.
    var seen=traced_behind;
    if !traced {seen=chamber(normalize(ray+n*.085));}
    let transmitted=seen*mix(vec3f(.55,.83,.96),vec3f(.96,.65,.36),warm(p.x));
    color=mix(transmitted*1.35,color,.065+fresnel*.75);
    let transmitted_xy=p.xy+n.xy*.025;
    let behind=sine_at(transmitted_xy);
    // A traced view behind already holds the sine text.
    if !traced && filament_z(transmitted_xy)<p.z-.02 {
        color+=heat(p.x)*smoothstep(-.5,.8,behind)*.55;
    }
    color+=heat(p.x)*pow(1.0-face,2.0)*lighting(.462,.20+energy*.25+bass()*.85+attack()*.7);
    return color;
}
struct Hit {p:vec3f,material:f32,found:bool}
// Ring layout: march from `start` along `dir` to the first surface.
fn ring_march(origin:vec3f,dir:vec3f,steps:i32)->Hit {
    var t=0.0;
    for(var i=0;i<steps;i++) {
        let p=origin+dir*t;
        if p.z< -4.4 || abs(p.x)>2.4 || abs(p.y)>1.7 {break;}
        let sample=scene(p);
        if sample.x<.0012 {return Hit(p,sample.y,true);}
        t+=max(sample.x*.5,.001);
    }
    return Hit(vec3f(0),0.0,false);
}
// Step through the solid a ray has just entered, to where it leaves; w is 0
// if it doesn't get out (grazing an edge-on letter, say).
fn ring_exit(p:vec3f,dir:vec3f)->vec4f {
    var t=.004;
    for(var i=0;i<40;i++) {
        let d=scene(p+dir*t).x;
        if d>.002 {return vec4f(p+dir*t,1);}
        t+=max(-d,.006);
    }
    return vec4f(p,0);
}
fn ring_opacity(p:vec3f,material:f32)->f32 {
    if material>.5 {return arrival();}
    return ring_fade(ring_local(p,ring_of(p)).x)*arrival();
}
fn ring_haze(p:vec3f)->f32 {return (1.0-exp(-max(-p.z,0.0)*.22))*.65;}
// Ring layout: glass and fading letters show what lies behind them. A second
// march finds it (through the glass, slightly refracted); that layer is shaded
// without shadow rays, over the chamber. Not a full multi-bounce trace.
fn ring_view(eye:vec3f,ray:vec3f,background:vec3f)->vec3f {
    let first=ring_march(eye+ray*(4.8/-ray.z),ray,128);
    if !first.found {return background;}
    let n=normal(first.p,first.material);
    let alpha=ring_opacity(first.p,first.material);
    let glass=first.material<.5;
    let dir=select(ray,normalize(ray+n*.085),glass);
    var behind=chamber(dir);
    let out=ring_exit(first.p,dir);
    if (glass || alpha<.999) && out.w>.5 {
        let second=ring_march(out.xyz,dir,96);
        if second.found {
            unshadowed=true;
            let far=shade(second.p,normal(second.p,second.material),dir,second.material);
            unshadowed=false;
            behind=mix(behind,mix(far,behind,ring_haze(second.p)),ring_opacity(second.p,second.material));
        }
    }
    // Through thick glass the far layer is dimmer and softer than the chamber.
    traced=true;traced_behind=mix(chamber(dir),behind,.55);
    let near=shade(first.p,n,ray,first.material);
    traced=false;
    return mix(behind,mix(near,background,ring_haze(first.p)),alpha);
}
@fragment fn scroller(v:Vertex)->@location(0) vec4f {
    let xy=(v.uv-.5)*vec2f(2.0*aspect(),-2.0);
    let ray=normalize(vec3f(xy,-5.5));
    var color=chamber(ray);
    if u.state.x>=8.0 && ring() {
        color=ring_view(vec3f(0,0,5.5),ray,color);
    } else if u.state.x>=8.0 {
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
    return vec4f(color*u.scroller.w,1);
}
