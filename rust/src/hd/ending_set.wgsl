// The wall of monitors the modern part 8 scrolls out from, and the light
// their screens throw on it. Authored throughout, after the owner's
// reference (a filmed wall of surveillance monitors), not recovered from
// the original: the sets, their shape and material, their small
// differences, the camera's move (hd/ending.rs, `camera`) and the light.
// The pictures are ending_crt.wgsl's.
//
// - The camera starts where the centre set's picture (inset in its glass)
//   fills the frame's height as part 7's picture does, straight on, the
//   glass flat and its outline the face's rectangle, the glass round the
//   picture still black and the sets unlit, so part 8 starts on part 7's
//   last frame. It holds close while the picture takes the
//   reference's look, then draws back, eased in, ever faster: as the
//   screen's corners come into view its outline rounds into the
//   reference's (big corners, gently bowed sides), the glass bulges and a
//   thin lip of light shows round its edge; then the bezel takes the
//   screens' light; then more and more sets come into view in every
//   direction, a field of about 17 across the frame by the switch-off and
//   hundreds by the end; the wall has no edge.
// - Each set is a box of its own, butted against its neighbours with a thin
//   dark seam, its screen about 70 percent of its front across and down,
//   set deep in a thick, glossy, near-black bezel: a flat frame round the
//   front rising into a rounded cushion that rolls down into a deep recess
//   to the glass, its inner bevel lit by the screen.
// - Every set shows the whole picture, centred and upright in its screen,
//   inset at PICTURE_SCALE of the face (ending_crt.wgsl), the glass round it
//   dark and softly lit, its grille and lip carrying on across it.
//   They differ only a little, fixed per set: brightness and a warmer or
//   cooler tint. All of them switch off together on the original's timing.
//   Their dots are drawn from the nearest sets wherever a ray goes, so each
//   stays a round point of light however small its set.
// - There is no room. The sets are lit by the screens: each by its own (in
//   twelve patches, from the original picture's mean colours), by its eight
//   neighbours' and their dots, and by the wall's light at large; small
//   sharp glints on the cushions' rounded edges come from a light above and
//   to the left that is the wall's light too, so as the screens go dark, so
//   does the wall. The glass shows the lit bevel in its curve and a faint
//   glare.
// - Where the sets are small, what is too small to see goes smoothly: the
//   glints widen and dim, and the glass stops showing the bevel.
// Module: the shared controls, ending_common.wgsl, ending_crt.wgsl and this
// file.

// ---- The sets (authored; scene units, source rows on the 4:3 display) -------
// The picture's half size; the screen's (the glass's outline) once its edge
// has come on, its corners' radius and how far its sides bow out at their
// middles; how far the glass bulges. The bezel's cross-section, out from the
// screen's outline: a wall up from the glass to CUSHION_Z, then the
// cushion, a round of radius CUSHION_R rolling over its crest and down to
// the flat frame at FRAME_Z, with a fillet between them. A set's half size
// at its front, its depth and its edges' rounding; the gap between sets
// (the seam) and the pitch they stand at (twice the half size and the gap).
const PICTURE=vec2f(133.33333,100.0);
const SCREEN=vec2f(130.0,97.0);
const SCREEN_ROUND=38.0;
const SCREEN_BOW=5.0;
const BULGE=14.0;
const CUSHION_Z=10.0;
const CUSHION_R=20.0;
const FRAME_Z=22.0;
const FILLET=6.0;
const SET=vec2f(184.0,146.0);
const SET_DEPTH=300.0;
const SET_ROUND=7.0;
const GAP=8.0;
const PITCH=vec2f(376.0,300.0);
// The material: glossy, near-black plastic with a green cast (linear
// albedo, GGX roughness); the glass's reflectance head on.
const BEZEL=vec3f(.026,.036,.03);
const BEZEL_ROUGH=.22;
const GLASS_F0=.04;
// The glints: the light's direction (above and to the left), its strength
// against the wall's light in the hold, and the roughness they show at; its
// broad sheen on the bezels (roughness, strength).
const KEY_FROM=vec3f(-.55,.7,.45);
const KEY=1.4;
const KEY_ROUGH=.07;
const SHEEN_ROUGH=.4;
const KEY_SHEEN=.35;
// The lip of light round the glass's edge: its light against the screen's,
// its width and how far inside the edge it runs (rows), and its least width
// on the frame (pixels).
const LIP=.9;
const LIP_WIDTH=.8;
const LIP_IN=1.6;
const LIP_LEAST=.7;
// The screens' light on the sets: its scale (the exposure the dark sets are
// seen at), the wall's light at large, the dot's light and its afterglow's,
// the faint sheen of the wall's light on the glass (strength, direction,
// tightness).
const LIGHT=26.0;
const WALL_FILL=.05;
const DOT_LIGHT=150.0;
const AFTER_LIGHT=60.0;
const SHEEN=.035;
const SHEEN_FROM=vec3f(-.3,.8,.52);
const SHEEN_POWER=8.0;
// The sets' differences, small and fixed per set: brightness from LOOK_DIM
// to 1, and how far the tint turns warmer or cooler. Every set shows the
// whole picture, centred and upright in its screen.
const LOOK_DIM=.88;
const LOOK_TINT=.05;
// Where the sets are small the glass stops showing the bevel, between these
// sizes (source rows a frame's pixel spans).
const REFLECT_FADE=vec2f(1.5,3.0);
// The march.
const STEPS=120u;
const STEP=.75;

// ---- Bindings ----------------------------------------------------------------------
// Per frame, the original picture's mean colour (linear) in each of 4 x 3
// cells, left to right and top to bottom (ending::cell_means).
@group(0) @binding(9) var<storage,read> cells:array<vec4f>;

const PI=3.14159265;

// ---- The sets' shape ------------------------------------------------------------------
// The screen's half size as its edge comes on (`e`, 0 to 1).
fn screen_half(e:f32)->vec2f {return mix(PICTURE,SCREEN,e);}
// The screen's outline: the signed distance (rows, negative inside) from a
// rounded rectangle whose sides bow out a little at their middles.
fn outline(q:vec2f,e:f32)->f32 {
    let half=screen_half(e);
    let w=clamp(q/half,vec2f(-1),vec2f(1));
    let bowed=half+SCREEN_BOW*e*(vec2f(1)-w.yx*w.yx);
    let r=SCREEN_ROUND*e;
    let b=abs(q)-bowed+r;
    return length(max(b,vec2f(0)))+min(max(b.x,b.y),0.0)-r;
}
fn bulge(q:vec2f,e:f32)->f32 {let w=q/screen_half(e);return BULGE*e*(1.0-.5*dot(w,w));}
fn round_box(p:vec3f,b:vec3f,r:f32)->f32 {
    let q=abs(p)-b+r;
    return length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-r;
}
fn smin(a:f32,b:f32,k:f32)->f32 {
    let h=clamp(.5+.5*(b-a)/k,0.0,1.0);
    return mix(b,a,h)-k*h*(1.0-h);
}
fn smax(a:f32,b:f32,k:f32)->f32 {return -smin(-a,-b,k);}
// A quadrant's signed distance: the region where q.x <= 0 and q.y <= 0.
fn quadrant(q:vec2f)->f32 {return length(max(q,vec2f(0)))+min(max(q.x,q.y),0.0);}
// The bezel's cross-section, `d` out from the screen's outline at height z:
// the wall from the glass, the cushion rolling over it and the flat frame.
fn bezel(d:f32,z:f32)->f32 {
    let wall=quadrant(vec2f(-d,z-CUSHION_Z));
    let cushion=length(vec2f(d-CUSHION_R,z-CUSHION_Z))-CUSHION_R;
    let frame=quadrant(vec2f(CUSHION_R-d,z-FRAME_Z));
    return smin(min(wall,cushion),frame,FILLET);
}
// One set, its screen's centre at the origin: the bezel round the screen,
// cut to the set's box with rounded edges.
fn set_shape(p:vec3f,e:f32)->f32 {
    let front=bezel(outline(p.xy,e),p.z);
    let top=CUSHION_Z+CUSHION_R+4.0;
    let body=round_box(p-vec3f(0,0,top-SET_DEPTH*.5),vec3f(SET,SET_DEPTH*.5),SET_ROUND);
    return smax(front,body,SET_ROUND);
}
// The set a point belongs to: its place in the wall, across and up.
fn set_of(q:vec2f)->vec2f {return round(q/PITCH);}
fn wall(p:vec3f,e:f32)->f32 {
    return set_shape(p-vec3f(set_of(p.xy)*PITCH,0.0),e);
}
fn wall_normal(p:vec3f,e:f32)->vec3f {
    let k=.05;
    return normalize(vec3f(
        wall(p+vec3f(k,0,0),e)-wall(p-vec3f(k,0,0),e),
        wall(p+vec3f(0,k,0),e)-wall(p-vec3f(0,k,0),e),
        wall(p+vec3f(0,0,k),e)-wall(p-vec3f(0,0,k),e)));
}
// The ray's span within the wall's depth (from the cushions' crests to the
// sets' backs), or none.
fn wall_span(o:vec3f,d:vec3f)->vec2f {
    let front=CUSHION_Z+CUSHION_R+1.0;
    let back=CUSHION_Z+CUSHION_R-SET_DEPTH;
    let t0=(front-o.z)/d.z;
    let t1=(back-o.z)/d.z;
    return vec2f(max(min(t0,t1),0.0),max(t0,t1));
}
fn march(o:vec3f,d:vec3f,span:vec2f,e:f32)->f32 {
    var t=span.x;
    for(var i=0u;i<STEPS;i++) {
        if t>span.y {break;}
        let h=wall(o+d*t,e);
        if h<.0004*t+.002 {return t;}
        t+=h*STEP;
    }
    return -1.0;
}
// Where the ray (d normalised) meets the glass of the set it crosses, as
// the distance along it and the set; -1 if it meets none.
fn glass_hit(o:vec3f,d:vec3f,e:f32)->vec3f {
    let middle=(BULGE*e*.5-o.z)/d.z;
    if middle<=0.0 {return vec3f(-1.0,0,0);}
    let tv=set_of((o+d*middle).xy);
    let local=o-vec3f(tv*PITCH,0.0);
    let half=screen_half(e);
    var t=(BULGE*e-local.z)/d.z;
    for(var i=0;i<4;i++) {
        let p=local+d*t;
        let f=p.z-bulge(p.xy,e);
        let slope=d.z+BULGE*e*dot(p.xy/(half*half),d.xy);
        t-=f/slope;
    }
    let p=local+d*t;
    if t<=0.0 || outline(p.xy,e)>0.0 {return vec3f(-1.0,0,0);}
    return vec3f(t,tv);
}

// ---- The sets' differences ----------------------------------------------------------
// Four numbers from 0 to 1, fixed per set.
fn scramble(x:u32)->u32 {
    var v=x;
    v^=v>>16u;v*=0x7feb352du;v^=v>>15u;v*=0x846ca68bu;v^=v>>16u;
    return v;
}
fn hash4(tv:vec2f,salt:u32)->vec4f {
    let i=vec2u(vec2i(tv)+vec2i(32768));
    let h0=scramble(i.x*1973u+i.y*9277u+salt*26699u);
    let h1=scramble(h0^0x9e3779b9u);
    let h2=scramble(h1^0x85ebca6bu);
    let h3=scramble(h2^0xc2b2ae35u);
    return vec4f(vec4u(h0,h1,h2,h3)>>vec4u(8u))/16777216.0;
}
// A set's brightness and tint; the centre set at the mean brightness and
// tint.
struct Look {brightness:f32, tint:vec3f}
fn look_of(tv:vec2f)->Look {
    if all(tv==vec2f(0)) {return Look(1.0,vec3f(1));}
    let h=hash4(tv,1u);
    let warmth=(h.y*2.0-1.0)*LOOK_TINT;
    return Look(mix(LOOK_DIM,1.0,h.x),vec3f(1.0+warmth,1.0,1.0-warmth));
}
// The point of the picture a set shows at `local` on its face, in units of
// the picture's half size: the same for every set.
fn placed(local:vec2f)->vec2f {return local/PICTURE;}

// ---- The screens' light -------------------------------------------------------------
fn cell_light(c:u32)->vec3f {
    let a=cells[frame_now()*12u+c].rgb;
    let b=cells[frame_next()*12u+c].rgb;
    return mix(a,b,frame_mix());
}
// How bright the screens are against the original's colours (as
// ending_crt.wgsl shows them): the squeezed band's gain or the line's, and
// the white frame's flash and the white's afterglow over the whole face.
fn screen_gain()->f32 {
    let compression=ec.beam.x;
    return ec.beam.y*select(1.0,pow(compression,BAND_POWER),compression>1.0)
        +ec.beam.z*line_gain(ec.ends.x,ec.ends.y);
}
fn screen_flash()->f32 {
    let cover=ec.tube.z;
    return (FLASH-1.0)*cover+FLASH_GLOW*max(ec.glow.z-max(cover,ec.tube.y),0.0);
}
// GGX's distribution.
fn ggx(nh:f32,alpha:f32)->f32 {
    let a2=alpha*alpha;
    let d=nh*nh*(a2-1.0)+1.0;
    return a2/(PI*d*d);
}
// A patch of screen at `centre` (facing +z, its light leaving forward) of
// radiance `light` and `area` lighting the point p (normal n, toward the
// camera v): diffuse and specular, the patch's size widening the
// highlight and softening the light close to it.
fn glow_from(p:vec3f,n:vec3f,v:vec3f,albedo:vec3f,rough:f32,centre:vec3f,light:vec3f,area:f32)->vec3f {
    let to=centre-p;
    let r2=dot(to,to);
    let l=to*inverseSqrt(r2);
    let ce=max(-l.z,0.0);
    let cr=max(dot(n,l),0.0);
    if ce*cr<=0.0 {return vec3f(0);}
    let e=light*area*ce/(r2+.3*area);
    let h=normalize(l+v);
    let alpha=clamp(rough*rough+.5*sqrt(area/r2),.02,1.0);
    let f=GLASS_F0+(1.0-GLASS_F0)*pow(1.0-max(dot(v,h),0.0),5.0);
    let spec=e*ggx(max(dot(n,h),0.0),alpha)*f/(4.0*max(dot(n,v),.2));
    return albedo/PI*e*cr+spec;
}
// The wall's light at large, as the screens show it: their mean colour.
fn wall_light()->vec3f {
    var mean=vec3f(0);
    for(var c=0u;c<12u;c++) {mean+=cell_light(c);}
    return graded_light(mean/12.0*screen_gain()+screen_flash(),vec3f(1),ec.look.x)*LIGHT;
}
// How lit the wall is against the hold: the screens' mean light now over
// their mean light on the first frame.
fn screens_lit()->f32 {
    var now=vec3f(0);
    var held=vec3f(0);
    for(var c=0u;c<12u;c++) {now+=cell_light(c);held+=cells[c].rgb;}
    return luminance(now*screen_gain()+12.0*screen_flash())/max(luminance(held),1e-4);
}
// The glint of the light above and to the left on the point with normal n
// seen from v, where a frame's pixel spans `detail` rows (small sets widen
// it, keeping its light).
fn glint(n:vec3f,v:vec3f,detail:f32)->f32 {
    let l=normalize(KEY_FROM);
    let h=normalize(l+v);
    let spread=.5*detail/CUSHION_R;
    let alpha=min(sqrt(KEY_ROUGH*KEY_ROUGH+spread*spread),1.0);
    let f=GLASS_F0+(1.0-GLASS_F0)*pow(1.0-max(dot(v,h),0.0),5.0);
    let nh=max(dot(n,h),0.0);
    let lobes=ggx(nh,alpha)+KEY_SHEEN*ggx(nh,SHEEN_ROUGH);
    return lobes*f*max(dot(n,l),0.0)/(4.0*max(dot(n,v),.2));
}
// The light the screens throw on the point p of set `own`: its own screen
// in twelve patches, each neighbour's as one, the dots, and the wall's
// light at large.
fn lit(p:vec3f,n:vec3f,v:vec3f,albedo:vec3f,rough:f32,own:vec2f)->vec3f {
    let gain=screen_gain();
    let flash=screen_flash();
    let a=ec.look.x;
    let z=BULGE*ec.look.z*.5;
    var mean=vec3f(0);
    for(var c=0u;c<12u;c++) {mean+=cell_light(c);}
    mean/=12.0;
    let dot_light=DOT_LIGHT*ec.glow.x+AFTER_LIGHT*ec.glow.y;
    var out=vec3f(0);
    for(var j=-1;j<=1;j++) {
        for(var i=-1;i<=1;i++) {
            let tv=own+vec2f(f32(i),f32(j));
            let look=look_of(tv);
            let at=vec3f(tv*PITCH,z);
            let scale=look.brightness*LIGHT;
            if i==0 && j==0 {
                let size=vec2f(2.0*PICTURE.x/4.0,2.0*PICTURE.y/3.0);
                for(var c=0u;c<12u;c++) {
                    let cell=vec2f(f32(c%4u),f32(c/4u));
                    let q=vec2f(-PICTURE.x,PICTURE.y)+(cell+.5)*size*vec2f(1,-1);
                    let light=graded_light(cell_light(c)*gain+flash,look.tint,a)*scale;
                    out+=glow_from(p,n,v,albedo,rough,at+vec3f(q*.9*PICTURE_SCALE,0),light,size.x*size.y);
                }
            } else {
                let light=graded_light(mean*gain+flash,look.tint,a)*scale;
                out+=glow_from(p,n,v,albedo,rough,at,light,4.0*PICTURE.x*PICTURE.y);
            }
            let spot=graded_light(vec3f(dot_light),look.tint,a)*scale;
            out+=glow_from(p,n,v,albedo,rough,at+vec3f(0,0,1),spot,1.0);
        }
    }
    out+=albedo*WALL_FILL*wall_light()*(.6+.4*max(n.z,0.0));
    return out*ec.look.w;
}

// ---- The dots ------------------------------------------------------------------------
// The dots the sets' tubes shrink to, seen along the ray (o, d): each a point
// of phosphor at the middle of its face, on the glass's crown. They are
// drawn wherever the ray goes, from the three by three sets nearest it, so a
// dot never ends at its glass's edge however small its set.
fn dots(o:vec3f,d:vec3f,e:f32)->vec3f {
    let t=(BULGE*e-o.z)/d.z;
    if t<=0.0 {return vec3f(0);}
    let q=(o+d*t).xy;
    // In the picture's rows, which are PICTURE_SCALE of the face's.
    let pixel=t*2.0*ec.eye.w/u.screen.y/PICTURE_SCALE;
    let at=vec2f((DOT_COLUMN+.5-160.0)*COLUMN,100.0-(LINE_ROW+.5))*PICTURE_SCALE;
    let near=set_of(q);
    var out=vec3f(0);
    for(var j=-1;j<=1;j++) {
        for(var i=-1;i<=1;i++) {
            let tv=near+vec2f(f32(i),f32(j));
            let look=look_of(tv);
            let r=length(q-tv*PITCH-at)/PICTURE_SCALE;
            out+=graded_light(dot_glow(r,pixel),look.tint,ec.look.x)*look.brightness;
        }
    }
    return out*(1.0-GLASS_F0*e);
}

// ---- The picture ---------------------------------------------------------------------
@fragment fn television(v:Vertex)->@location(0) vec4f {
    ending_begin();
    var light=vec3f(0);
    for(var k=0;k<SAMPLES*SAMPLES;k++) {
        let o=(vec2f(f32(k%SAMPLES),f32(k/SAMPLES))+.5)/f32(SAMPLES)-.5;
        light+=shade(v.uv+o/u.screen.xy);
    }
    return vec4f(max(light/f32(SAMPLES*SAMPLES)*ec.beam.w,vec3f(0)),1);
}
// Samples a pixel takes across and down (1; more only to make a reference
// to compare against when checking for aliasing).
const SAMPLES=1;
// The light the frame shows at `uv` (0 to 1 across and down), HDR, before
// the fade to black.
fn shade(uv:vec2f)->vec3f {
    let e=ec.look.z;
    let b=ec.look.w;
    // The camera's ray: straight at the face, its 200 rows filling the
    // frame's height, at first.
    let eye=ec.eye.xyz;
    let forward=normalize(ec.aim.xyz-eye);
    let right=normalize(cross(forward,vec3f(0,1,0)));
    let up=cross(right,forward);
    let ndc=vec2f(uv.x*2.0-1.0,1.0-uv.y*2.0)*ec.eye.w;
    let d=normalize(forward+right*ndc.x*screen_aspect()+up*ndc.y);
    let toward=-d;
    // Rows a frame's pixel spans on the wall, straight ahead.
    let detail=(eye.z-FRAME_Z)*2.0*ec.eye.w/u.screen.y;
    var light=vec3f(0);
    let glass=glass_hit(eye,d,e);
    var span=wall_span(eye,d);
    if glass.x>0.0 {span.y=min(span.y,glass.x);}
    let t=select(-1.0,march(eye,d,span,e),span.x<span.y);
    if t>0.0 {
        // A bezel: its frame, its cushion, its inner bevel or its seam.
        let p=eye+d*t;
        let tv=set_of(p.xy);
        let n=wall_normal(p,e);
        let spread=.5*detail/CUSHION_R;
        let rough=sqrt(BEZEL_ROUGH*BEZEL_ROUGH+spread);
        light=lit(p,n,toward,BEZEL,rough,tv);
        light+=vec3f(KEY*b*screens_lit()*glint(n,toward,detail));
    } else if glass.x>0.0 {
        // The glass: the picture through it, the lip of light round its
        // edge, the lit bevel in its curve and a faint glare.
        let tv=glass.yz;
        let look=look_of(tv);
        let p=eye+d*glass.x;
        let local=p-vec3f(tv*PITCH,0.0);
        let half=screen_half(e);
        let n=normalize(vec3f(BULGE*e*local.xy/(half*half),1.0));
        let cv=max(dot(n,toward),0.0);
        let fresnel=GLASS_F0+(1.0-GLASS_F0)*pow(1.0-cv,5.0);
        let plain=placed(local.xy);
        let pixel=glass.x*2.0*ec.eye.w/u.screen.y/max(cv,.3);
        let o=outline(local.xy,e);
        let edge=1.0-smoothstep(-pixel,0.0,o);
        let flat=glass.x*2.0*ec.eye.w/u.screen.y;
        let shown=screen(plain,pixel,flat,look.tint)*look.brightness;
        light=shown*(1.0-fresnel*e)*edge;
        if e>0.0 {
            // The lip, lit as the screen is, keeping its light as a frame
            // REFERENCE_HEIGHT pixels high shows it when widened.
            let width=max(LIP_WIDTH,LIP_LEAST*pixel);
            let keep=max(LIP_WIDTH,LIP_LEAST*pixel*u.screen.y/REFERENCE_HEIGHT)/width;
            let off=o+max(LIP_IN,1.2*pixel);
            let line=exp(-off*off/(2.0*width*width))*keep;
            var mean=vec3f(0);
            for(var c=0u;c<12u;c++) {mean+=cell_light(c);}
            let screen_light=graded_light(mean/12.0*screen_gain()+screen_flash(),look.tint,ec.look.x);
            light+=LIP*e*line*screen_light*look.brightness;
        }
        let rim=1.0-smoothstep(REFLECT_FADE.x,REFLECT_FADE.y,pixel);
        if e>0.0 && rim>0.0 {
            let r=reflect(d,n);
            var reach=wall_span(p,r);
            reach.y=min(reach.y,reach.x+200.0);
            let back=select(-1.0,march(p+n*.05,r,reach,e),reach.x<reach.y);
            if back>0.0 {
                let q2=p+n*.05+r*back;
                let n2=wall_normal(q2,e);
                light+=rim*fresnel*lit(q2,n2,-r,BEZEL,BEZEL_ROUGH,set_of(q2.xy));
            }
        }
        if e>0.0 {
            let r=reflect(d,n);
            let sheen=pow(max(dot(r,normalize(SHEEN_FROM)),0.0),SHEEN_POWER);
            light+=SHEEN*e*fresnel/GLASS_F0*sheen*wall_light();
        }
    }
    if ec.glow.x+ec.glow.y>0.0 {light+=dots(eye,d,e);}
    return light;
}
