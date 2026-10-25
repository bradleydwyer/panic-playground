// The bank of nine monitors the modern part 8 draws back to reveal, and the
// light their screens throw on them. Authored throughout: the sets, their
// shape, material and small differences, the camera's move (hd/ending.rs,
// `camera`) and the light. The pictures are ending_crt.wgsl's.
//
// - The camera starts where the centre set's face fills the frame's height
//   as part 7's picture does, straight on, with the glass flat, the face the
//   picture's own rectangle and the sets unlit, so part 8 starts on part 7's
//   last frame. As it draws back the face becomes a tube's: the glass
//   bulges, and its outline rounds into bowed sides and deep corners, set
//   back into a thick dark bezel; the bank of nine comes into view, whole
//   and centred in the dark by the end of the hold. From the white it eases
//   in a little, so the centre set's line and dot read.
// - The nine sets stand in three rows of three, their bezels nearly
//   touching. Each shows the monster with small differences of its own
//   (brightness, warmth, the picture's place and turn), as real sets do;
//   the centre set has none. All switch off together on the original's
//   timing.
// - There is no room. The sets are lit only by their screens: each by its
//   own (in twelve patches, from the original picture's mean colours) and
//   by its neighbours', on the bezels' rounded edges, their rims and the
//   tunnels down to the glass. As the screens go dark, so does the bank. The
//   glass shows the lit rim in its curve and a faint sheen of the bank's
//   own light.
// Module: the shared controls, ending_common.wgsl, ending_crt.wgsl and this
// file.

// ---- The bank (authored; scene units, source rows on the 4:3 display) -------
// The picture's half size; the glass's half size once rounded, its
// outline's exponent then and at first (nearly the picture's rectangle);
// how far the glass bulges; how deep it is set into the bezel; the bezel's
// opening at its front and that outline's exponent; a set's half size at
// its front, its depth and its edges' rounding; the gap between sets, and
// the pitch they stand at (twice the half size and the gap).
const PICTURE=vec2f(133.33333,100.0);
const GLASS_HALF=vec2f(131.0,98.0);
const GLASS_POWER=4.0;
const FLAT_POWER=40.0;
const BULGE=16.0;
const TUNNEL=34.0;
const OPENING=vec2f(152.0,118.0);
const OPENING_POWER=5.0;
const SET=vec2f(182.0,142.0);
const SET_DEPTH=260.0;
const SET_ROUND=24.0;
const LIP=7.0;
const GAP=5.0;
const PITCH=vec2f(369.0,289.0);
// The materials: the bezels' dark plastic and the tunnels' (linear albedo,
// GGX roughness); the glass's reflectance head on.
const BEZEL=vec3f(.06,.058,.055);
const BEZEL_ROUGH=.42;
const RIM=vec3f(.09,.087,.083);
const RIM_ROUGH=.45;
const GLASS_F0=.04;
// The screens' light on the sets: its scale (the exposure the dark sets are
// seen at), the dot's light and its afterglow's, the share of the bank's
// light the glass's phosphor scatters back (its grey), the faint sheen of
// the bank's light on the glass (its strength, direction and tightness),
// and the tubes' hint of warmth.
const LIGHT=22.0;
const SCATTER=vec3f(.0016,.0017,.0018);
const DOT_LIGHT=150.0;
const AFTER_LIGHT=60.0;
const SHEEN=.1;
const SHEEN_FROM=vec3f(-.3,.8,.52);
const SHEEN_POWER=8.0;
const WARMTH=vec3f(1.03,1.0,.95);
// Each set's differences, left to right and top to bottom (index 4 is the
// centre): brightness and warmth (red up and blue down by it); the
// picture's offset across and down (rows), its turn (degrees) and size.
const LOOK=array<vec4f,9>(
    vec4f(.92,.03,0,0),vec4f(1.0,-.01,0,0),vec4f(.95,.02,0,0),
    vec4f(.97,-.02,0,0),vec4f(1.0,0.0,0,0),vec4f(.9,.015,0,0),
    vec4f(1.02,.02,0,0),vec4f(.94,-.03,0,0),vec4f(.97,0.0,0,0));
const PLACE=array<vec4f,9>(
    vec4f(1.2,-.8,.4,1.01),vec4f(-.6,.9,-.3,.99),vec4f(.4,.5,.6,1.015),
    vec4f(-1.0,-.4,-.5,.995),vec4f(0,0,0,1),vec4f(.8,.7,.3,1.005),
    vec4f(-.5,1.1,-.2,1.01),vec4f(1.1,-.6,.5,.985),vec4f(-.9,-1.0,-.4,1.0));
// The march.
const STEPS=110u;
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
// One set, its face's centre at the origin: a rounded body, its front at
// z = TUNNEL, with a tunnel down to the glass whose outline flares from
// the glass's at z = 0 to the opening's at the front.
fn set_shape(p:vec3f,a:f32)->f32 {
    let body=round_box(p-vec3f(0,0,TUNNEL-SET_DEPTH*.5),vec3f(SET,SET_DEPTH*.5),SET_ROUND);
    let s=clamp(p.z/TUNNEL,0.0,1.0);
    let f=s*s;
    let hole=outline(p.xy,mix(glass_half(a),OPENING,f),mix(glass_power(a),OPENING_POWER,f));
    // The opening's lip rounded where the tunnel meets the front.
    let k=LIP;
    let h=clamp(.5-.5*(-hole-body)/k,0.0,1.0);
    return mix(-hole,body,h)+k*h*(1.0-h);
}
// The set a point belongs to, -1 to 1 across and up.
fn set_of(q:vec2f)->vec2f {return clamp(round(q/PITCH),vec2f(-1),vec2f(1));}
fn bank(p:vec3f,a:f32)->f32 {
    return set_shape(p-vec3f(set_of(p.xy)*PITCH,0.0),a);
}
fn bank_normal(p:vec3f,a:f32)->vec3f {
    let e=.05;
    return normalize(vec3f(
        bank(p+vec3f(e,0,0),a)-bank(p-vec3f(e,0,0),a),
        bank(p+vec3f(0,e,0),a)-bank(p-vec3f(0,e,0),a),
        bank(p+vec3f(0,0,e),a)-bank(p-vec3f(0,0,e),a)));
}
// The ray's span inside the bank's box, or none.
fn bank_span(o:vec3f,d:vec3f)->vec2f {
    let lo=vec3f(-1.5*PITCH,TUNNEL-SET_DEPTH)-1.0;
    let hi=vec3f(1.5*PITCH,TUNNEL)+1.0;
    let inv=1.0/d;
    let t0=(lo-o)*inv;
    let t1=(hi-o)*inv;
    let near=min(t0,t1);
    let far=max(t0,t1);
    return vec2f(max(max(near.x,near.y),max(near.z,0.0)),min(far.x,min(far.y,far.z)));
}
fn march(o:vec3f,d:vec3f,span:vec2f,a:f32)->f32 {
    var t=span.x;
    for(var i=0u;i<STEPS;i++) {
        if t>span.y {break;}
        let h=bank(o+d*t,a);
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

// ---- The screens' light -------------------------------------------------------------
fn index_of(tv:vec2f)->u32 {return u32(tv.x+1.0)+3u*u32(1.0-tv.y);}
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
fn set_tint(k:u32)->vec3f {
    let look=LOOK[k];
    return look.x*vec3f(1.0+look.y,1.0,1.0-look.y)*mix(vec3f(1),WARMTH,ec.tube.x);
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
// The light every screen throws on the point p of set `own`: its own
// screen in twelve patches, each neighbour's as one, and the dots.
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
            let tv=vec2f(f32(i),f32(j));
            let at=vec3f(tv*PITCH,z);
            let tint=set_tint(index_of(tv))*LIGHT;
            if all(tv==own) {
                let size=vec2f(2.0*PICTURE.x/4.0,2.0*PICTURE.y/3.0);
                for(var c=0u;c<12u;c++) {
                    let cell=vec2f(f32(c%4u),f32(c/4u));
                    let q=vec2f(-PICTURE.x,PICTURE.y)+(cell+.5)*size*vec2f(1,-1);
                    let light=(cell_light(c)*gain+flash)*tint;
                    out+=glow_from(p,n,v,albedo,rough,at+vec3f(q*.9,0),light,size.x*size.y);
                }
            } else {
                let light=(mean*gain+flash)*tint;
                out+=glow_from(p,n,v,albedo,rough,at,light,4.0*PICTURE.x*PICTURE.y);
            }
            out+=glow_from(p,n,v,albedo,rough,at+vec3f(0,0,1),vec3f(dot_light)*tint,1.0);
        }
    }
    return out*a;
}

// ---- The picture ---------------------------------------------------------------------
@fragment fn television(v:Vertex)->@location(0) vec4f {
    ending_begin();
    let a=ec.tube.x;
    // The camera's ray: straight at the face, its 200 rows filling the
    // frame's height, at first.
    let eye=ec.eye.xyz;
    let forward=normalize(ec.aim.xyz-eye);
    let right=normalize(cross(forward,vec3f(0,1,0)));
    let up=cross(right,forward);
    let ndc=vec2f(v.uv.x*2.0-1.0,1.0-v.uv.y*2.0)*ec.eye.w;
    let d=normalize(forward+right*ndc.x*screen_aspect()+up*ndc.y);
    let toward=-d;
    var light=vec3f(0);
    let glass=glass_hit(eye,d,a);
    var span=bank_span(eye,d);
    if glass.x>0.0 {span.y=min(span.y,glass.x);}
    let t=select(-1.0,march(eye,d,span,a),span.x<span.y);
    if t>0.0 {
        // A bezel, its rim or a tunnel.
        let p=eye+d*t;
        let tv=set_of(p.xy);
        let n=bank_normal(p,a);
        let local=p-vec3f(tv*PITCH,0.0);
        let inside=outline(local.xy,OPENING,OPENING_POWER)<1.0 && local.z<TUNNEL-.5;
        light=lit(p,n,toward,select(BEZEL,RIM,inside),select(BEZEL_ROUGH,RIM_ROUGH,inside),tv);
    } else if glass.x>0.0 {
        // The glass: the picture through it, the lit rim in its curve and a
        // faint sheen of the bank's light.
        let tv=glass.yz;
        let k=index_of(tv);
        let p=eye+d*glass.x;
        let local=p-vec3f(tv*PITCH,0.0);
        let half=glass_half(a);
        let n=normalize(vec3f(BULGE*a*local.xy/(half*half),1.0));
        let cv=max(dot(n,toward),0.0);
        let fresnel=GLASS_F0+(1.0-GLASS_F0)*pow(1.0-cv,5.0);
        // The picture's place on this set's face.
        let place=PLACE[k];
        let turn=radians(place.z);
        let q=(local.xy-place.xy*vec2f(1,-1))/place.w;
        let turned=vec2f(cos(turn)*q.x+sin(turn)*q.y,-sin(turn)*q.x+cos(turn)*q.y);
        let pixel=glass.x*2.0*ec.eye.w/u.screen.y/max(cv,.3);
        let edge=1.0-smoothstep(-pixel,0.0,outline(local.xy,half,glass_power(a)));
        light=screen(turned/PICTURE,a,pixel)*set_tint(k)*(1.0-fresnel*a)*edge;
        if a>0.0 {
            let r=reflect(d,n);
            var reach=bank_span(p,r);
            reach.y=min(reach.y,reach.x+200.0);
            let back=select(-1.0,march(p+n*.05,r,reach,a),reach.x<reach.y);
            if back>0.0 {
                let q2=p+n*.05+r*back;
                let n2=bank_normal(q2,a);
                light+=fresnel*lit(q2,n2,-r,RIM,RIM_ROUGH,set_of(q2.xy));
            }
            var mean=vec3f(0);
            for(var c=0u;c<12u;c++) {mean+=cell_light(c);}
            let bank_light=(mean/12.0*screen_gain()+screen_flash())*LIGHT;
            // The sheen is a glare, near white: the bank's light's brightness.
            let sheen=pow(max(dot(r,normalize(SHEEN_FROM)),0.0),SHEEN_POWER);
            let glare=dot(bank_light,vec3f(.2126,.7152,.0722))*vec3f(.97,1.0,1.03);
            light+=SHEEN*a*fresnel/GLASS_F0*sheen*glare;
            light+=SCATTER*a*bank_light*edge;
        }
    }
    return vec4f(max(light*ec.beam.w,vec3f(0)),1);
}
