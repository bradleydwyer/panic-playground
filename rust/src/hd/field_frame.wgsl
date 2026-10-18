// Part 5's metal frame and the smoke behind and round it: the frame
// author's file (with field_frame.rs). field_render.rs builds the modern
// module from the shared controls, field_common.wgsl, this file,
// field_zoom.wgsl, field_mountain.wgsl and field_style.wgsl. Prefix this
// file's own names with frame_. It defines:
//   fn frame_layers(px:FieldPixel)->FrameLayers   smoke behind the frame,
//       the frame, smoke in front; each a FieldLayer (premultiplied linear
//       HDR light, opacity, scene z)
//   fn frame_fluid_init(cell:FluidCell)->vec4f     the fluid's first state
//   fn frame_fluid(cell:FluidCell)->vec4f          one fixed step: the cell's
//       next state from the state before (fluid_load, fluid_sample) and the
//       step's block (fc, the usual helpers), deterministic
//   fn frame_prepass(px:FieldPixel)->vec4f         the half-resolution
//       prepass, when FRAME.prepass is set (read it with frame_prepass_at)
// The fluid step is a compute shader: use textureSampleLevel or
// textureLoad, not textureSample, fwidth or the music cues, and not
// fluid_blend, which is for drawing.
//
// The frame is dark polished gunmetal, drawn in the frame's plane where the
// PSTART picture has it, lit by what is round it.
// - Derived from the picture when the renderer loads (field_frame.rs,
//   frame_shape): the frame's outer edge and crease on each side, the bands
//   across its sides, and the ornaments' relief (heights and normals from
//   the picture's shading, an interpretation) and outlines.
// - Authored: the bevels' profile (a steep foot at the window, the inner
//   bevel at 55 degrees, a rounded crest, the outer face at 24 degrees and a
//   rounded lip), the brushed, polished and cast metal, and the lights: the
//   window's light during the zoom and the lava's during the mountain (the
//   other authors' values, or stand-ins until they give them), a dim key
//   from the upper left, as the picture is shaded, a small bright light by
//   it for catch-lights, a dim cool fill and a dim ambient. The lighting is
//   cheap reflections of round lights and of the window's opening, not a
//   physical model. The mountain's silhouette (frame_prepass) shades the
//   frame from the lava and the key and darkens it behind its base.
// Nothing is displaced: the frame shows exactly where the picture has it.
// The smoke is still the stand-in: none.

// The frame's shape (field_frame.rs's FrameShape), bound by field_render.rs:
// two pages of SHAPE_SCALE texels a source pixel and a row of parameters.
@group(0) @binding(13) var frame_shape:texture_2d<f32>;

// ---- The profile and the material (authored) -----------------------------------
// The bevels' slopes, rows of height per row across, outward from the
// window's edge: the foot (a source pixel wide), the inner bevel, the outer
// face (falling) and the lip (its last pixel, falling); and how far each
// change of slope is rounded, in rows.
const FRAME_FOOT_SLOPE=3.0;
const FRAME_INNER_SLOPE=1.43;
const FRAME_OUTER_SLOPE=.45;
const FRAME_LIP_SLOPE=2.6;
const FRAME_ROUND=vec3f(.25,.55,.4);
// The bands across each side stand this far proud (rows), rounded over this
// many source pixels at their edges.
const FRAME_BAND_RAISE=.3;
const FRAME_BAND_EDGE=.22;
// Gunmetal's reflectance at normal incidence (linear), and the bands', a
// brighter polished inlay.
const FRAME_METAL=vec3f(.205,.215,.235);
const FRAME_BAND_METAL=vec3f(.36,.37,.39);
// The reflection's spread in radians: brushed along the bevels (along the
// brushing and across it), the polished bands, and the cast ornaments; and
// the haze, the share of light the metal scatters broadly.
const FRAME_BRUSHED=vec2f(.30,.055);
const FRAME_POLISHED=vec2f(.09,.04);
const FRAME_CAST=vec2f(.085,.085);
const FRAME_HAZE=.32;

// ---- The lights (authored) -----------------------------------------------------
// Round lights in the room in front (scene centre, radius in rows, and
// radiance): the key, a dim soft light from the upper left, as the picture
// is shaded; beside it a small bright one for the catch-lights on edges; and
// a dim cool fill from the lower right. Then the dim ambient.
const FRAME_KEY_AT=vec3f(-330,370,400);
const FRAME_KEY_SIZE=250.0;
const FRAME_KEY=vec3f(.55,.57,.62);
const FRAME_SPARK_AT=vec3f(-260,330,520);
const FRAME_SPARK_SIZE=70.0;
const FRAME_SPARK=vec3f(3.2,3.3,3.5);
const FRAME_FILL_AT=vec3f(420,-190,500);
const FRAME_FILL_SIZE=380.0;
const FRAME_FILL=vec3f(.18,.20,.25);
const FRAME_AMBIENT=vec3f(.022,.024,.028);
// The window's light until the zoom author gives it (fc.zoom_style[0]): a
// cool blue standing for the zoom's average colour made a light (radiance),
// chosen by eye, not measured.
const FRAME_WINDOW_STANDIN=vec3f(.07,.16,.72);
// The lava's until the mountain author gives it (fc.mountain_style[0] and
// [1]): over the plateau, in front of the frame, its radius in rows and
// radiance.
const FRAME_LAVA_SPREAD=46.0;
const FRAME_LAVA_STANDIN=vec3f(6.0,1.55,.28);
// The mountain's body, for its shadow on the frame: its silhouette stands
// this far in front of the frame (scene z).
const FRAME_MOUNTAIN_DEPTH=45.0;

// ---- The shape --------------------------------------------------------------------
fn frame_shape_size()->vec2f {return vec2f(textureDimensions(frame_shape));}
fn frame_param(i:i32)->vec4f {
    return textureLoad(frame_shape,vec2i(i,i32(textureDimensions(frame_shape).y)-1),0);
}
// Page 0 or 1 of the shape at source point s, bilinear, kept off the other
// page and the parameters.
fn frame_page(s:vec2f,page:f32)->vec4f {
    let size=frame_shape_size();
    let k=size.x/320.0;
    let rows=(size.y-1.0)*.5;
    let t=vec2f(s.x*k,clamp(s.y*k,.5,rows-.5)+page*rows);
    return textureSampleLevel(frame_shape,linear,t/size,0.0);
}
// The parameter row's layout (field_frame.rs).
const FRAME_BANDS_BY_COLUMN=0;
const FRAME_BANDS_BY_ROW=320;
const FRAME_RECTANGLES=1200;

// A lit point of metal.
struct FrameSurface {
    p:vec3f,          // scene position; z its height
    n:vec3f,          // normal
    t:vec3f,          // the brushing's direction, perpendicular to n
    spread:vec2f,     // the reflection's spread along t and across it
    f0:vec3f,         // reflectance at normal incidence
    occlusion:f32,
}

// One side's profile at u source pixels out from the window's edge (crease
// c, edge e, `axis` rows a source pixel along it): height and slope, in
// rows, and how far the slope changes are rounded given the pixel's size.
fn frame_profile(u:f32,c:f32,e:f32,axis:f32,footprint:f32)->vec2f {
    let x=u*axis;let foot=axis;let crest=c*axis;let lip=(e-1.0)*axis;
    let h=FRAME_FOOT_SLOPE*min(x,foot)+FRAME_INNER_SLOPE*clamp(x-foot,0.0,crest-foot)
        -FRAME_OUTER_SLOPE*clamp(x-crest,0.0,lip-crest)-FRAME_LIP_SLOPE*clamp(x-lip,0.0,axis);
    let w=max(FRAME_ROUND,vec3f(footprint));
    var slope=FRAME_FOOT_SLOPE;
    slope+=(FRAME_INNER_SLOPE-FRAME_FOOT_SLOPE)*smoothstep(foot-w.x,foot+w.x,x);
    slope+=(-FRAME_OUTER_SLOPE-FRAME_INNER_SLOPE)*smoothstep(crest-w.y,crest+w.y,x);
    slope+=(FRAME_OUTER_SLOPE-FRAME_LIP_SLOPE)*smoothstep(lip-w.z,lip+w.z,x);
    return vec2f(h,slope);
}

// The band across a side at `along` source pixels (columns for the top and
// bottom, rows for the sides): how much of the pixel is in a band, and the
// slope its edges give along the side, rows per source pixel.
fn frame_band(side:i32,along:f32,footprint:f32)->vec2f {
    let by_row=side<2;
    let base=select(FRAME_BANDS_BY_COLUMN,FRAME_BANDS_BY_ROW,by_row);
    let last=select(319,199,by_row);
    let q=along-.5;let i=i32(floor(q));let f=q-floor(q);
    let a=frame_param(base+clamp(i,0,last));
    let b=frame_param(base+clamp(i+1,0,last));
    let pick=side%2==1;
    let v0=select(a.x,a.y,pick);let v1=select(b.x,b.y,pick);
    let w=max(FRAME_BAND_EDGE,.5*footprint);
    let x=clamp((f-.5+w)/(2.0*w),0.0,1.0);
    let v=mix(v0,v1,x*x*(3.0-2.0*x));
    return vec2f(v,(v1-v0)*FRAME_BAND_RAISE*6.0*x*(1.0-x)/(2.0*w));
}

// The scene direction outward from the window across each side: left,
// right, top, bottom.
fn frame_outward(side:i32)->vec2f {
    return array<vec2f,4>(vec2f(-1,0),vec2f(1,0),vec2f(0,1),vec2f(0,-1))[side];
}

// The frame's surface at source point s, on a given side.
fn frame_side_surface(side:i32,s:vec2f,u:f32,c:f32,e:f32,footprint:f32)->FrameSurface {
    let horizontal=side>=2;
    // Rows a source pixel across this side, and along it.
    let axis=select(COLUMN,1.0,horizontal);
    let along_axis=select(1.0,COLUMN,horizontal);
    let profile=frame_profile(u,c,e,axis,footprint);
    let along=select(s.y,s.x,horizontal);
    let band=frame_band(side,along,footprint/along_axis);
    let o=frame_outward(side);
    // Along the side in the scene, the way `along` grows.
    let t2=select(vec2f(0,-1),vec2f(1,0),horizontal);
    let grad=o*profile.y+t2*band.y/along_axis;
    let n=normalize(vec3f(-grad,1));
    let t=normalize(vec3f(t2,0)-n*dot(vec3f(t2,0),n));
    let q=scene_of_source(s);
    var sf=FrameSurface(vec3f(q,profile.x+band.x*FRAME_BAND_RAISE),n,t,
        mix(FRAME_BRUSHED,FRAME_POLISHED,band.x),mix(FRAME_METAL,FRAME_BAND_METAL,band.x),1.0);
    // A little darker in the foot's corner and under the lip.
    sf.occlusion=mix(.55,1.0,smoothstep(0.0,1.5,u))*mix(.7,1.0,smoothstep(0.0,1.0,e-u));
    return sf;
}

// ---- The brushing -----------------------------------------------------------------
fn frame_noise(x:f32)->f32 {
    let i=floor(x);let f=x-i;
    return mix(hash2(vec2f(i,7.0)),hash2(vec2f(i+1.0,7.0)),f*f*(3.0-2.0*f));
}
// Fine lines along the brushing: tilt the normal across it and vary the
// spread, fading out where they would be finer than a pixel.
fn frame_brush(sf:FrameSurface,footprint:f32)->FrameSurface {
    var out=sf;
    let b=cross(sf.n,sf.t);
    let across=dot(sf.p,b);let along=dot(sf.p,sf.t);
    let frequency=1.6;
    let fade=1.0-smoothstep(.18,.4,footprint*frequency);
    let lines=frame_noise(across*frequency+3.0*frame_noise(along*.05))*.65
        +frame_noise(across*frequency*2.7+11.0)*.35;
    out.n=normalize(sf.n+b*(lines-.5)*.06*fade);
    out.t=normalize(sf.t-out.n*dot(sf.t,out.n));
    out.spread=sf.spread*(1.0+(lines-.5)*.5*fade);
    return out;
}

// ---- Light --------------------------------------------------------------------------
// The glow of a round light seen in a reflection: the light's centre along
// unit vector l, its angular radius rho, the reflection round r with spread
// w along tt and bb. A Gaussian lobe over a Gaussian disc: 1 for a mirror
// looking straight at it, keeping the light's energy as the lobe widens.
fn frame_glow(l:vec3f,rho:f32,r:vec3f,tt:vec3f,bb:vec3f,w:vec2f)->f32 {
    let d=l-r;
    let a=dot(d,tt);let b=dot(d,bb);let c=dot(d,r);
    let rr=rho*rho;
    let vt=rr+2.0*w.x*w.x;let vb=rr+2.0*w.y*w.y;
    return rr/sqrt(vt*vb)*exp(-a*a/vt-b*b/vb-c*c/min(vt,vb));
}
// The broadly scattered share of a round light: as a matte surface's.
fn frame_haze(l:vec3f,rho:f32,n:vec3f)->f32 {return rho*rho*max(dot(n,l),0.0);}
// A round light of radius `size` at scene point `at`, seen from p: its glow
// in the reflection, with `haze` of the haze, while it is above the surface.
fn frame_lamp(p:vec3f,at:vec3f,size:f32,n:vec3f,r:vec3f,tt:vec3f,bb:vec3f,w:vec2f,haze:f32)->f32 {
    let to=at-p;
    let d=length(to);let l=to/d;
    let rho=min(size/d,1.0);
    let above=smoothstep(-rho,rho+.1,dot(n,l));
    return above*((1.0-FRAME_HAZE)*frame_glow(l,rho,r,tt,bb,w)+haze*FRAME_HAZE*frame_haze(l,rho,n));
}
fn frame_erf(x:f32)->f32 {
    let a=.147;let x2=x*x;
    return sign(x)*sqrt(1.0-exp(-x2*(1.27324+a*x2)/(1.0+a*x2)));
}
// How much of a Gaussian of width `blur` round x lies between lo and hi.
fn frame_box(x:vec2f,lo:vec2f,hi:vec2f,blur:vec2f)->f32 {
    let k=1.0/(1.41421*max(blur,vec2f(1e-3)));
    let e=vec2f(frame_erf((hi.x-x.x)*k.x),frame_erf((hi.y-x.y)*k.y))
        -vec2f(frame_erf((lo.x-x.x)*k.x),frame_erf((lo.y-x.y)*k.y));
    return .25*e.x*e.y;
}
// The window seen in a reflection from p along r: through the opening in
// the frame's plane, which the window's light fills, brightest where the
// ray goes on to meet the window's content behind it; blurred by the lobe's
// spread over the distance.
fn frame_window_seen(p:vec3f,r:vec3f,tt:vec3f,bb:vec3f,w:vec2f)->f32 {
    if r.z>.08 {return 0.0;}
    let down=min(r.z,-.04);
    let lo=scene_of_source(vec2f(FIELD_WINDOW.x,FIELD_WINDOW.w));
    let hi=scene_of_source(vec2f(FIELD_WINDOW.z,FIELD_WINDOW.y));
    // How the lobe's spread lands on a plane facing the viewer.
    let spread=sqrt(vec2f(w.x*w.x*tt.x*tt.x+w.y*w.y*bb.x*bb.x,w.x*w.x*tt.y*tt.y+w.y*w.y*bb.y*bb.y))/max(-down,.25);
    let t0=max(p.z,0.0)/-down;
    let t1=(max(p.z,0.0)-LAYER_WINDOW_Z)/-down;
    let opening=frame_box(p.xy+r.xy*t0,lo,hi,spread*t0+.05);
    let q=p.xy+r.xy*t1;
    let content=frame_box(q,lo-24.0,hi+24.0,spread*t1+.05);
    // Brighter towards the window's middle, where the zoom heads.
    let middle=length((q-.5*(lo+hi))/(.5*(hi-lo)));
    let glow=mix(.4,1.0,content)*(1.25-.45*smoothstep(0.0,1.6,middle));
    return opening*glow*smoothstep(.08,-.06,r.z);
}

// The lights at this moment.
struct FrameLights {
    level:f32,        // part 6's fade: 0 to 1
    window:vec3f,     // the window's radiance
    lava:vec3f,       // the lava's radiance
    lava_at:vec3f,    // its centre (scene)
    lava_size:f32,    // its radius (rows)
    mountain:f32,     // how much the mountain stands in front, for shadows
}
fn frame_lights()->FrameLights {
    var lights:FrameLights;
    // Part 6's fade is in DAC units, as its picture's was.
    lights.level=pow(clamp(picture_level(),0.0,1.0),2.2);
    // The window: the zoom author's light (given once any of its four values
    // is non-zero, so w can mark a dark window as given), or until then a
    // cool blue that goes out with the window's clear.
    let given=fc.zoom_style[0];
    if any(given!=vec4f(0)) {lights.window=max(given.rgb,vec3f(0));}
    else {
        var on=1.0;
        if window_cleared() {on=1.0-smoothstep(0.0,.12,cleared_seconds());}
        lights.window=FRAME_WINDOW_STANDIN*on;
    }
    // The mountain stands in front while the original shows its pixels.
    lights.mountain=select(0.0,clamp(terrain_shown(),0.0,1.0),field_phase()>.5);
    // The lava: the mountain author's light (given likewise by [1]), or
    // until then a glow over the plateau, rising and sinking with the
    // terrain.
    let at=fc.mountain_style[0];
    let colour=fc.mountain_style[1];
    if any(colour!=vec4f(0)) {
        lights.lava_at=vec3f(scene_of_source(at.xy),at.z);
        lights.lava_size=max(at.w,1.0);
        lights.lava=max(colour.rgb,vec3f(0));
    } else {
        let row=terrain_row(camera_distance(),relief_highest());
        lights.lava_at=vec3f(scene_of_source(vec2f(COLUMN_CENTRE,row-.3*FRAME_LAVA_SPREAD)),LAYER_MOUNTAIN_Z);
        lights.lava_size=FRAME_LAVA_SPREAD;
        lights.lava=FRAME_LAVA_STANDIN*lights.mountain*smoothstep(8.0,64.0,terrain_level());
    }
    return lights;
}

// ---- The mountain's shadow ------------------------------------------------------------
// The mountain's silhouette over source column x, as frame_prepass leaves
// it in its first row: the top source row it covers (400 if none).
fn frame_silhouette_at(x:f32)->f32 {
    let rows=ceil(u.screen.y*.5);
    let uvx=(x-160.0)*COLUMN/(200.0*screen_aspect())+.5;
    return frame_prepass_at(vec2f(uvx,.5/rows)).x;
}
// How much the mountain hides a light from scene point p: the light's ray
// crosses the silhouette's depth at q (scene), softened over `soft` rows.
fn frame_shadow(q:vec2f,soft:f32)->f32 {
    let s=source_of_scene(q);
    return smoothstep(-soft,soft,s.y-frame_silhouette_at(s.x));
}

// ---- The frame --------------------------------------------------------------------
fn frame_shade(sf:FrameSurface,lights:FrameLights)->vec3f {
    let v=normalize(EYE-sf.p);
    let n=sf.n;
    let nv=max(dot(n,v),1e-3);
    let r=reflect(-v,n);
    var tt=sf.t-r*dot(sf.t,r);
    tt=select(normalize(tt),vec3f(0,1,0),dot(tt,tt)<1e-6);
    let bb=cross(r,tt);
    let fresnel=sf.f0+(1.0-sf.f0)*pow(1.0-nv,5.0);
    let w=sf.spread;
    var light=vec3f(0);
    // The ambient, brighter overhead and darker below.
    light+=FRAME_AMBIENT*(.35+.65*smoothstep(-.4,.9,r.y));
    // The mountain's shadow and occlusion near its base.
    var hidden_key=0.0;var hidden_lava=0.0;var near=0.0;
    if lights.mountain>0.0 {
        let p=sf.p;
        let depth=FRAME_MOUNTAIN_DEPTH-p.z;
        let to_key=FRAME_KEY_AT-p;
        hidden_key=frame_shadow(p.xy+to_key.xy*depth/to_key.z,6.0);
        let to=lights.lava_at-p;
        let f=clamp(depth/max(to.z,1.0),0.0,1.0);
        hidden_lava=frame_shadow(p.xy+to.xy*f,4.0+.2*lights.lava_size*f);
        let s=source_of_scene(p.xy);
        for(var i=0;i<4;i++) {
            let dx=array<f32,4>(-14.0,-5.0,5.0,14.0)[i];
            near+=.25*smoothstep(-8.0,8.0,s.y-frame_silhouette_at(s.x+dx));
        }
        hidden_key*=lights.mountain;hidden_lava*=lights.mountain;near*=lights.mountain;
    }
    light*=1.0-.6*near;
    // The room's lights; the mountain hides the key and the spark.
    let unhidden=(1.0-hidden_key)*(1.0-.5*near);
    light+=FRAME_KEY*unhidden*frame_lamp(sf.p,FRAME_KEY_AT,FRAME_KEY_SIZE,n,r,tt,bb,w,1.0);
    light+=FRAME_SPARK*unhidden*frame_lamp(sf.p,FRAME_SPARK_AT,FRAME_SPARK_SIZE,n,r,tt,bb,w,0.0);
    light+=FRAME_FILL*frame_lamp(sf.p,FRAME_FILL_AT,FRAME_FILL_SIZE,n,r,tt,bb,w,1.0);
    // The lava.
    if any(lights.lava>vec3f(0)) {
        light+=lights.lava*(1.0-hidden_lava)*frame_lamp(sf.p,lights.lava_at,lights.lava_size,n,r,tt,bb,w,1.0);
    }
    // The window, behind the opening.
    if any(lights.window>vec3f(0)) {
        light+=lights.window*(1.0-.5*FRAME_HAZE)*frame_window_seen(sf.p,r,tt,bb,w);
    }
    return light*fresnel*sf.occlusion*lights.level;
}

// The frame round the window, from its measured rectangles.
fn frame_layer_at(px:FieldPixel,lights:FrameLights)->FieldLayer {
    let s=px.source;
    let outer=frame_param(FRAME_RECTANGLES);
    let crease=frame_param(FRAME_RECTANGLES+1);
    if frame_param(FRAME_RECTANGLES+2).x<.5 {return no_layer();}
    let d=min(min(s.x-outer.x,outer.z-s.x),min(s.y-outer.y,outer.w-s.y));
    let cover=clamp(d/px.pixel+.5,0.0,1.0)*(1.0-window_cover(s,px.pixel));
    if cover<=0.0 {return no_layer();}
    let wr=FIELD_WINDOW;
    let away=vec4f(wr.x-s.x,s.x-wr.z,wr.y-s.y,s.y-wr.w);
    let c=vec4f(wr.x-crease.x,crease.z-wr.z,wr.y-crease.y,crease.w-wr.w);
    let e=vec4f(wr.x-outer.x,outer.z-wr.z,wr.y-outer.y,outer.w-wr.w);
    let t=away/max(e,vec4f(1));
    // The side the point belongs to, and the next, across a mitre.
    var first=0;var second=1;
    if t[1]>t[0] {first=1;second=0;}
    for(var k=2;k<4;k++) {
        if t[k]>t[first] {second=first;first=k;}
        else if t[k]>t[second] {second=k;}
    }
    var sf=frame_side_surface(first,s,max(away[first],0.0),c[first],e[first],px.pixel);
    // Across a mitre, blend the two sides over the pixel, with a fine seam.
    let gap=(t[first]-t[second])*e[first];
    if gap<1.5*px.pixel+.5 && t[second]>0.0 {
        let other=frame_side_surface(second,s,max(away[second],0.0),c[second],e[second],px.pixel);
        let m=.5*(1.0-smoothstep(0.0,px.pixel,gap));
        sf.n=normalize(mix(sf.n,other.n,m));
        sf.t=normalize(mix(sf.t,other.t,m)-sf.n*dot(mix(sf.t,other.t,m),sf.n));
        sf.spread=mix(sf.spread,other.spread,m);
        sf.f0=mix(sf.f0,other.f0,m);
        sf.p.z=mix(sf.p.z,other.p.z,m);
        sf.occlusion*=1.0-.45*(1.0-smoothstep(0.0,max(.3,px.pixel),gap));
    }
    sf=frame_brush(sf,px.pixel);
    return field_layer(frame_shade(sf,lights),cover,LAYER_FRAME_Z);
}

// The ornaments round the frame: cast metal in relief.
fn frame_ornament_at(px:FieldPixel,lights:FrameLights)->FieldLayer {
    let s=px.source;
    let shape=frame_page(s,0.0);
    let cover=clamp(shape.w/px.pixel+.5,0.0,1.0);
    if cover<=0.0 {return no_layer();}
    let detail=frame_page(s,1.0);
    // The gradient is per source column and row (y down); the scene's x is
    // 5/6 of a column and its y up.
    let n=normalize(vec3f(-shape.y/COLUMN,shape.z,1));
    let t=normalize(vec3f(1,0,0)-n*n.x);
    let cavity=detail.x;
    var sf=FrameSurface(vec3f(scene_of_source(s),shape.x),n,t,FRAME_CAST,FRAME_METAL,1.0);
    // Darker and rougher in the recesses, polished on what stands out.
    sf.occlusion=clamp(1.0+1.1*cavity,.3,1.0);
    sf.spread=FRAME_CAST*mix(1.6,.75,smoothstep(-.8,.8,cavity));
    sf.f0=FRAME_METAL*mix(.8,1.12,smoothstep(-.6,.6,cavity));
    return field_layer(frame_shade(sf,lights),cover,LAYER_FRAME_Z);
}

fn frame_layers(px:FieldPixel)->FrameLayers {
    let s=px.source;
    if !in_frame(s) || window_cover(s,px.pixel)>=1.0 {
        return FrameLayers(no_layer(),no_layer(),no_layer());
    }
    let lights=frame_lights();
    var frame=frame_layer_at(px,lights);
    if frame.alpha<1.0 {
        let ornament=frame_ornament_at(px,lights);
        if ornament.alpha>0.0 {
            frame=FieldLayer(frame.color+ornament.color*(1.0-frame.alpha),
                frame.alpha+ornament.alpha*(1.0-frame.alpha),LAYER_FRAME_Z);
        }
    }
    return FrameLayers(no_layer(),frame,no_layer());
}

// The prepass's first row holds, for each column, the top of the mountain's
// silhouette (source rows), as the relief marched with the code's camera
// and projection shows it (the stand-in's mountain), for the frame's
// shadows; the rest is empty.
fn frame_prepass(px:FieldPixel)->vec4f {
    let rows=ceil(u.screen.y*.5);
    if px.uv.y*rows>1.0 || field_phase()<.5 {return vec4f(400,0,0,0);}
    let dir=terrain_dir(px.source.x);
    let origin=camera_map();
    var z0=NEAR;var z1=FAR;
    for(var a=0;a<2;a++) {
        if abs(dir[a])>1e-6 {
            let p=-origin[a]/dir[a];let q=(MAP_SIZE-origin[a])/dir[a];
            z0=max(z0,min(p,q));z1=min(z1,max(p,q));
        } else if origin[a]<0.0 || origin[a]>MAP_SIZE {return vec4f(400,0,0,0);}
    }
    var top=400.0;
    var z=z0;
    loop {
        if z>=z1 {break;}
        let h=relief_height(origin+dir*z);
        if h>=.5 {top=min(top,terrain_row(z,h));}
        z+=1.0;
    }
    return vec4f(top,0,0,0);
}

// The fluid runs, drawing nothing yet: its state counts its steps, modulo
// 1000, in w, which the ignored test part5_fluid_is_the_same_however_it_is_
// reached reads back.
fn frame_fluid_init(cell:FluidCell)->vec4f {return vec4f(0);}
fn frame_fluid(cell:FluidCell)->vec4f {
    let before=fluid_load(cell.texel);
    return vec4f(0,0,0,f32((u32(round(before.w))+1u)%1000u));
}
