// The wall of monitors the modern part 8 scrolls out from, and the light
// their screens throw on it. Authored throughout: the sets, their shape,
// material and small differences, the camera's move (hd/ending.rs,
// `camera`) and the light. The pictures are ending_crt.wgsl's.
//
// - The camera starts where the centre set's face fills the frame's height
//   as part 7's picture does, straight on, with the glass flat, the face the
//   picture's own rectangle and the sets unlit, so part 8 starts on part 7's
//   last frame. It holds close while the picture becomes an old green
//   surveillance monitor's, then draws back, eased in, ever faster: the face
//   becomes a tube's, its glass bulging into bowed sides and deep corners,
//   set deep into a thick chamfered bezel, and more and more sets come into
//   view in every direction, a field of about 17 across the frame by the
//   switch-off and hundreds by the end; the wall has no edge.
// - Every set shows the whole picture, centred and upright in its screen.
//   The sets differ only a little, fixed per set: brightness and a warmer
//   or cooler green. All of them switch off together on the original's
//   timing. Their dots are drawn from the nearest sets wherever a ray goes,
//   so each stays a round point of light however small its set.
// - There is no room. The sets are lit only by the screens: each by its own
//   (in twelve patches, from the original picture's mean colours), by its
//   eight neighbours' and their dots, and by the wall's light at large, on
//   the chamfers, rims, lips, tunnels and buttons, all in the screens'
//   green. As the screens go dark, so does the wall. The glass shows the lit
//   rim in its curve, a faint glare and the phosphor's scattered green.
// - Where the sets are small, what is too small to see goes smoothly: the
//   controls sink into the bezel and the glass stops showing the rim.
// Module: the shared controls, ending_common.wgsl, ending_crt.wgsl and this
// file.

// ---- The sets (authored; scene units, source rows on the 4:3 display) -------
// The picture's half size; the glass's half size once rounded, its
// outline's exponent then and at first (nearly the picture's rectangle);
// how far the glass bulges; how deep it is set into the bezel; the bezel's
// opening at its front and that outline's exponent; a set's half size at
// its front, its depth, its edges' rounding, the chamfer round its front,
// the lip where the tunnel meets the front; the gap between sets and the
// pitch they stand at (twice the half size and the gap).
const PICTURE=vec2f(133.33333,100.0);
const GLASS_HALF=vec2f(129.0,96.0);
const GLASS_POWER=3.6;
const FLAT_POWER=40.0;
const BULGE=22.0;
const TUNNEL=40.0;
const OPENING=vec2f(152.0,118.0);
const OPENING_POWER=4.5;
const SET=vec2f(184.0,146.0);
const SET_DEPTH=300.0;
const SET_ROUND=5.0;
const CHAMFER=12.0;
const LIP=6.0;
const GAP=8.0;
const PITCH=vec2f(376.0,300.0);
// The controls under each face: a row of four buttons (their centre,
// spacing, half size, how far they stand out) and a knob (centre, radius,
// height).
const BUTTONS=vec2f(64.0,-129.0);
const BUTTON_SPACING=17.0;
const BUTTON=vec2f(5.5,3.5);
const BUTTON_OUT=3.0;
const KNOB=vec2f(-128.0,-129.0);
const KNOB_RADIUS=5.5;
const KNOB_OUT=4.0;
// The materials: the bezels' dark plastic, the tunnels' and the controls'
// (linear albedo, GGX roughness); the glass's reflectance head on.
const BEZEL=vec3f(.075,.075,.072);
const BEZEL_ROUGH=.38;
const RIM=vec3f(.1,.1,.095);
const RIM_ROUGH=.45;
const CONTROLS=vec3f(.14,.14,.135);
const CONTROLS_ROUGH=.3;
const GLASS_F0=.04;
// The screens' light on the sets: its scale (the exposure the dark sets are
// seen at), the wall's light at large, the dot's light and its afterglow's,
// the faint sheen of the wall's light on the glass (strength, direction,
// tightness).
const LIGHT=26.0;
const WALL_FILL=.05;
const DOT_LIGHT=150.0;
const AFTER_LIGHT=60.0;
const SHEEN=.1;
const SHEEN_FROM=vec3f(-.3,.8,.52);
const SHEEN_POWER=8.0;
// The sets' differences, small and fixed per set: brightness from LOOK_DIM
// to 1, and how far the green turns warmer or cooler (on the phosphor's
// exponents). Every set shows the whole picture, centred and upright in its
// screen.
const LOOK_DIM=.88;
const LOOK_WARMTH=.12;
// Where the sets are small, what is too small to see goes: the controls
// sink into the bezel and the glass stops showing the rim, between these
// sizes (source rows a frame's pixel spans).
const CONTROLS_FADE=vec2f(2.5,5.0);
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
// The glass's outline and bulge as the CRT comes on (`a`, 0 to 1).
fn glass_half(a:f32)->vec2f {return mix(PICTURE,GLASS_HALF,a);}
fn glass_power(a:f32)->f32 {return exp(mix(log(FLAT_POWER),log(GLASS_POWER),a));}
fn bulge(q:vec2f,a:f32)->f32 {let w=q/glass_half(a);return BULGE*a*(1.0-.5*dot(w,w));}
// A superellipse's outline: about the distance (units) outside it.
fn outline(q:vec2f,half:vec2f,power:f32)->f32 {
    let w=min(abs(q)/half,vec2f(4.0));
    return (pow(pow(w.x,power)+pow(w.y,power),1.0/power)-1.0)*min(half.x,half.y);
}
fn round_box(p:vec3f,b:vec3f,r:f32)->f32 {
    let q=abs(p)-b+r;
    return length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-r;
}
// How much the controls stand out (1 near, 0 where the sets are small).
var<private> standing:f32;
// The controls under the face: the buttons and the knob.
fn controls(p:vec3f)->f32 {
    let along=p.x-BUTTONS.x;
    let i=clamp(floor(along/BUTTON_SPACING)+.5,-1.5,1.5);
    let q=vec3f(along-i*BUTTON_SPACING,p.y-BUTTONS.y,p.z-TUNNEL);
    let button=round_box(q,vec3f(BUTTON,BUTTON_OUT*standing),1.2*standing);
    let r=vec2f(length(p.xy-KNOB)-KNOB_RADIUS,abs(p.z-TUNNEL)-KNOB_OUT*standing);
    let knob=min(max(r.x,r.y),0.0)+length(max(r,vec2f(0)))-standing;
    return min(button,knob);
}
// One set, its face's centre at the origin: a body with a chamfered front
// at z = TUNNEL, a tunnel down to the glass whose outline flares from the
// glass's at z = 0 to the opening's at the front, its lip rounded, and the
// controls under the face.
fn set_shape(p:vec3f,a:f32)->f32 {
    var body=round_box(p-vec3f(0,0,TUNNEL-SET_DEPTH*.5),vec3f(SET,SET_DEPTH*.5),SET_ROUND);
    let rim=max(abs(p.x)-SET.x,abs(p.y)-SET.y);
    body=max(body,(rim+CHAMFER+p.z-TUNNEL)*.70710678);
    let s=clamp(p.z/TUNNEL,0.0,1.0);
    let f=s*s;
    let hole=outline(p.xy,mix(glass_half(a),OPENING,f),mix(glass_power(a),OPENING_POWER,f));
    let h=clamp(.5-.5*(-hole-body)/LIP,0.0,1.0);
    let carved=mix(-hole,body,h)+LIP*h*(1.0-h);
    return min(carved,controls(p));
}
// The set a point belongs to: its place in the wall, across and up.
fn set_of(q:vec2f)->vec2f {return round(q/PITCH);}
fn wall(p:vec3f,a:f32)->f32 {
    return set_shape(p-vec3f(set_of(p.xy)*PITCH,0.0),a);
}
fn wall_normal(p:vec3f,a:f32)->vec3f {
    let e=.05;
    return normalize(vec3f(
        wall(p+vec3f(e,0,0),a)-wall(p-vec3f(e,0,0),a),
        wall(p+vec3f(0,e,0),a)-wall(p-vec3f(0,e,0),a),
        wall(p+vec3f(0,0,e),a)-wall(p-vec3f(0,0,e),a)));
}
// The ray's span within the wall's depth (from the controls' fronts to the
// sets' backs), or none.
fn wall_span(o:vec3f,d:vec3f)->vec2f {
    let front=TUNNEL+BUTTON_OUT+KNOB_OUT+1.0;
    let back=TUNNEL-SET_DEPTH;
    let t0=(front-o.z)/d.z;
    let t1=(back-o.z)/d.z;
    return vec2f(max(min(t0,t1),0.0),max(t0,t1));
}
fn march(o:vec3f,d:vec3f,span:vec2f,a:f32)->f32 {
    var t=span.x;
    for(var i=0u;i<STEPS;i++) {
        if t>span.y {break;}
        let h=wall(o+d*t,a);
        if h<.0004*t+.002 {return t;}
        t+=h*STEP;
    }
    return -1.0;
}
// Where the ray (d normalised) meets the glass of the set it crosses, as
// the distance along it and the set; -1 if it meets none.
fn glass_hit(o:vec3f,d:vec3f,a:f32)->vec3f {
    let middle=(BULGE*a*.5-o.z)/d.z;
    if middle<=0.0 {return vec3f(-1.0,0,0);}
    let tv=set_of((o+d*middle).xy);
    let local=o-vec3f(tv*PITCH,0.0);
    let half=glass_half(a);
    var t=(BULGE*a-local.z)/d.z;
    for(var i=0;i<4;i++) {
        let p=local+d*t;
        let f=p.z-bulge(p.xy,a);
        let slope=d.z+BULGE*a*dot(p.xy/(half*half),d.xy);
        t-=f/slope;
    }
    let p=local+d*t;
    if t<=0.0 || outline(p.xy,half,glass_power(a))>0.0 {return vec3f(-1.0,0,0);}
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
struct Look {brightness:f32, exponents:vec3f}
fn look_of(tv:vec2f)->Look {
    if all(tv==vec2f(0)) {return Look(1.0,PHOSPHOR);}
    let h=hash4(tv,1u);
    let warmth=h.y*2.0-1.0;
    return Look(mix(LOOK_DIM,1.0,h.x),PHOSPHOR+vec3f(-LOOK_WARMTH*warmth,0.0,LOOK_WARMTH*warmth));
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
    return graded(mean/12.0*screen_gain()+screen_flash(),PHOSPHOR,ec.tube.x)*LIGHT;
}
// The light the screens throw on the point p of set `own`: its own screen
// in twelve patches, each neighbour's as one, the dots, and the wall's
// light at large.
fn lit(p:vec3f,n:vec3f,v:vec3f,albedo:vec3f,rough:f32,own:vec2f)->vec3f {
    let gain=screen_gain();
    let flash=screen_flash();
    let a=ec.tube.x;
    let z=BULGE*a*.5;
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
                    let light=graded(cell_light(c)*gain+flash,look.exponents,a)*scale;
                    out+=glow_from(p,n,v,albedo,rough,at+vec3f(q*.9,0),light,size.x*size.y);
                }
            } else {
                let light=graded(mean*gain+flash,look.exponents,a)*scale;
                out+=glow_from(p,n,v,albedo,rough,at,light,4.0*PICTURE.x*PICTURE.y);
            }
            let spot=graded(vec3f(dot_light),look.exponents,a)*scale;
            out+=glow_from(p,n,v,albedo,rough,at+vec3f(0,0,1),spot,1.0);
        }
    }
    out+=albedo*WALL_FILL*wall_light()*(.6+.4*max(n.z,0.0));
    return out*a;
}

// ---- The dots ------------------------------------------------------------------------
// The dots the sets' tubes shrink to, seen along the ray (o, d): each a point
// of phosphor at the middle of its face, on the glass's crown. They are
// drawn wherever the ray goes, from the three by three sets nearest it, so a
// dot never ends at its glass's edge however small its set.
fn dots(o:vec3f,d:vec3f,a:f32)->vec3f {
    let t=(BULGE*a-o.z)/d.z;
    if t<=0.0 {return vec3f(0);}
    let q=(o+d*t).xy;
    let pixel=t*2.0*ec.eye.w/u.screen.y;
    let at=vec2f((DOT_COLUMN+.5-160.0)*COLUMN,100.0-(LINE_ROW+.5));
    let near=set_of(q);
    var out=vec3f(0);
    for(var j=-1;j<=1;j++) {
        for(var i=-1;i<=1;i++) {
            let tv=near+vec2f(f32(i),f32(j));
            let look=look_of(tv);
            let r=length(q-tv*PITCH-at);
            out+=graded(dot_glow(r,pixel),look.exponents,a)*look.brightness;
        }
    }
    return out*(1.0-GLASS_F0*a);
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
    let a=ec.tube.x;
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
    let detail=(eye.z-TUNNEL)*2.0*ec.eye.w/u.screen.y;
    standing=1.0-smoothstep(CONTROLS_FADE.x,CONTROLS_FADE.y,detail);
    var light=vec3f(0);
    let glass=glass_hit(eye,d,a);
    var span=wall_span(eye,d);
    if glass.x>0.0 {span.y=min(span.y,glass.x);}
    let t=select(-1.0,march(eye,d,span,a),span.x<span.y);
    if t>0.0 {
        // A bezel, its chamfer, its rim, a tunnel or the controls.
        let p=eye+d*t;
        let tv=set_of(p.xy);
        let n=wall_normal(p,a);
        let local=p-vec3f(tv*PITCH,0.0);
        let inside=outline(local.xy,OPENING,OPENING_POWER)<1.0 && local.z<TUNNEL-.5;
        let knobs=controls(local)<.5;
        var albedo=select(BEZEL,RIM,inside);
        var rough=select(BEZEL_ROUGH,RIM_ROUGH,inside);
        if knobs {albedo=mix(albedo,CONTROLS,standing);rough=mix(rough,CONTROLS_ROUGH,standing);}
        light=lit(p,n,toward,albedo,rough,tv);
    } else if glass.x>0.0 {
        // The glass: the picture through it, the lit rim in its curve, a
        // faint glare and the phosphor's scattered light.
        let tv=glass.yz;
        let look=look_of(tv);
        let p=eye+d*glass.x;
        let local=p-vec3f(tv*PITCH,0.0);
        let half=glass_half(a);
        let n=normalize(vec3f(BULGE*a*local.xy/(half*half),1.0));
        let cv=max(dot(n,toward),0.0);
        let fresnel=GLASS_F0+(1.0-GLASS_F0)*pow(1.0-cv,5.0);
        let plain=placed(local.xy);
        let pixel=glass.x*2.0*ec.eye.w/u.screen.y/max(cv,.3);
        let edge=1.0-smoothstep(-pixel,0.0,outline(local.xy,half,glass_power(a)));
        let shown=screen(plain,a,pixel,look.exponents)*look.brightness;
        light=shown*(1.0-fresnel*a)*edge;
        let rim=1.0-smoothstep(REFLECT_FADE.x,REFLECT_FADE.y,pixel);
        if a>0.0 && rim>0.0 {
            let r=reflect(d,n);
            var reach=wall_span(p,r);
            reach.y=min(reach.y,reach.x+200.0);
            let back=select(-1.0,march(p+n*.05,r,reach,a),reach.x<reach.y);
            if back>0.0 {
                let q2=p+n*.05+r*back;
                let n2=wall_normal(q2,a);
                light+=rim*fresnel*lit(q2,n2,-r,RIM,RIM_ROUGH,set_of(q2.xy));
            }
        }
        if a>0.0 {
            let r=reflect(d,n);
            let sheen=pow(max(dot(r,normalize(SHEEN_FROM)),0.0),SHEEN_POWER);
            light+=SHEEN*a*fresnel/GLASS_F0*sheen*wall_light();
        }
    }
    if ec.glow.x+ec.glow.y>0.0 {light+=dots(eye,d,a);}
    return light;
}
