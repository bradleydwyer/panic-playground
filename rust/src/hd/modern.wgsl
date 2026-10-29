// New geometric models, evaluated in 3D. This shader does not sample the old
// artwork for geometry. Letterforms and the continuous plate tumble are new;
// flights fit the locally decoded sequence. Doors use the Rust timeline.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl.
// Only the model and probe passes bind these; the backdrop holds space, stars
// and door panels without the models, so glass can refract what is behind it.
@group(0) @binding(1) var backdrop:texture_2d<f32>;
@group(0) @binding(2) var probe_map:texture_2d<f32>;
@group(0) @binding(3) var tile_map:texture_2d<f32>;
@group(0) @binding(4) var linear:sampler;
// Position within the authored exposure: 0 opens the shutter, 1 is the frame
// being shown. Model functions read it, so every sample sees one instant.
var<private> shutter:f32=1.0;
const CAMERA=vec3f(0,0,4.2);
const FOCAL=3.8;
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
fn box2(p:vec2f,b:vec2f,r:f32)->f32 {
    let q=abs(p)-b+r;
    return length(max(q,vec2f(0)))+min(max(q.x,q.y),0.0)-r;
}
// Authored outlines (not recovered source geometry), one table so new letters
// only add vertices. Each entry names its first vertex and vertex count.
const OUTLINE_VERTICES=array<vec2f,27>(
    // R leg: the shoulder is embedded in the bowl, with a horizontal foot.
    // Independent square-capped strokes protrude at the junction and below
    // the baseline, so the leg is a polygon of its own.
    vec2f(-.21,.07),vec2f(.03,.07),vec2f(.43,-.47),vec2f(.20,-.47),
    // W: one continuous outline gives the diagonal joins their own mitres and
    // flat tips. Unioning four capped strokes creates spikes at every join.
    vec2f(-.75,.47),vec2f(-.54,.47),vec2f(-.31,-.19),
    vec2f(-.09,.37),vec2f(.09,.37),vec2f(.31,-.19),
    vec2f(.54,.47),vec2f(.75,.47),vec2f(.435,-.47),
    vec2f(.235,-.47),vec2f(0,.07),vec2f(-.235,-.47),vec2f(-.435,-.47),
    // N of the PANIC logo, in the logo's 1/100 units.
    vec2f(-25,-37),vec2f(-12,-37),vec2f(13,13),vec2f(13,-37),vec2f(25,-37),
    vec2f(25,37),vec2f(12,37),vec2f(-13,-13),vec2f(-13,37),vec2f(-25,37)
);
const R_LEG=vec2u(0,4);
const W_OUTLINE=vec2u(4,13);
const N_OUTLINE=vec2u(17,10);
// Signed distance to a closed polygon from the table: negative inside.
fn polygon(p:vec2f,range:vec2u)->f32 {
    var nearest=1e6;var inside=false;
    for(var i=0u;i<range.y;i++) {
        let a=OUTLINE_VERTICES[range.x+i];let b=OUTLINE_VERTICES[range.x+(i+1u)%range.y];
        let edge=b-a;let delta=p-a;
        let closest=delta-edge*clamp(dot(delta,edge)/dot(edge,edge),0.0,1.0);
        nearest=min(nearest,dot(closest,closest));
        if (a.y>p.y)!=(b.y>p.y) {
            if p.x<(b.x-a.x)*(p.y-a.y)/(b.y-a.y)+a.x {inside=!inside;}
        }
    }
    return select(sqrt(nearest),-sqrt(nearest),inside);
}
fn panic_outline(point:vec2f)->f32 {
    var p=point*100.0;p.y=-p.y;p.x+=p.y*.24;
    let pp=p-vec2f(-112,0);
    var letter_p=min(box2(pp-vec2f(0,-17),vec2f(24,20),4.0),box2(pp-vec2f(-18,10),vec2f(6,27),.6));
    letter_p=max(letter_p,-box2(pp-vec2f(0,-17),vec2f(12,8),2.0));
    let pa=p-vec2f(-55,0);
    var letter_a=box2(pa,vec2f(24,37),4.0);
    letter_a=max(letter_a,-box2(pa-vec2f(0,-17),vec2f(12,8),2.0));
    letter_a=max(letter_a,-box2(pa-vec2f(0,27),vec2f(12,24),1.0));
    let pn=p-vec2f(3,0);
    let letter_n=polygon(pn,N_OUTLINE);
    let letter_i=box2(p-vec2f(49,0),vec2f(7,37),.8);
    let pc=p-vec2f(93,0);
    var letter_c=max(box2(pc,vec2f(26,37),5.0),-box2(pc,vec2f(14,25),2.0));
    letter_c=max(letter_c,-box2(pc-vec2f(24,0),vec2f(20,17),.8));
    return min(min(letter_p,letter_a),min(letter_n,min(letter_i,letter_c)))/100.0;
}
fn letter_outline(p:vec2f,id:u32)->f32 {
    // FUTURE CREW, in the reference order. Height is .94 world units.
    let glyph=array<u32,10>(0,1,2,1,3,4,5,3,4,6)[min(id,9u)];
    if glyph==0u || glyph==4u {
        var d=box2(p+vec2f(.205,0),vec2f(.095,.47),.008);
        d=min(d,box2(p-vec2f(0,.375),vec2f(.30,.095),.008));
        d=min(d,box2(p-vec2f(-.025,.025),vec2f(.275,.085),.008));
        if glyph==4u {d=min(d,box2(p+vec2f(0,.375),vec2f(.30,.095),.008));}
        return d;
    }
    if glyph==1u {
        return max(box2(p,vec2f(.44,.47),.075),-box2(p-vec2f(0,.17),vec2f(.24,.47),.035));
    }
    if glyph==2u {return min(box2(p-vec2f(0,.375),vec2f(.39,.095),.008),box2(p,vec2f(.105,.47),.008));}
    if glyph==3u {
        var d=min(box2(p+vec2f(.325,0),vec2f(.105,.47),.008),box2(p-vec2f(0,.225),vec2f(.43,.245),.06));
        d=max(d,-box2(p-vec2f(.01,.225),vec2f(.20,.09),.018));
        d=min(d,polygon(p,R_LEG));
        return d;
    }
    if glyph==5u {
        var d=max(box2(p,vec2f(.40,.47),.075),-box2(p,vec2f(.20,.28),.035));
        return max(d,-box2(p-vec2f(.35,0),vec2f(.30,.19),.012));
    }
    return polygon(p,W_OUTLINE);
}
fn rotate_x(p:vec3f,a:f32)->vec3f {return vec3f(p.x,cos(a)*p.y-sin(a)*p.z,sin(a)*p.y+cos(a)*p.z);}
fn rotate_y(p:vec3f,a:f32)->vec3f {return vec3f(cos(a)*p.x+sin(a)*p.z,p.y,-sin(a)*p.x+cos(a)*p.z);}
fn rotate_z(p:vec3f,a:f32)->vec3f {return vec3f(cos(a)*p.x-sin(a)*p.y,sin(a)*p.x+cos(a)*p.y,p.z);}
// The PANIC letters (10) ride on the tumbling plate (11) and stay as the logo
// when it lands: both share one frame, the logo's slight tilt, turned by the
// plate's tumble while in space. The plate is centred behind the letters, its
// face 0.15 behind theirs, and sized to what the screen shows of it once
// landed: just past the screen's sides, whatever its shape, and just past the
// doors' lips, which hide its edges. Authored, like the tumble; the original's
// plate is a smaller, plain raster block.
const PLATE=vec3f(0,0,-.12);
fn plate_size()->vec2f {
    let depth=CAMERA.z-PLATE.z;
    return vec2f(u.screen.x/u.screen.y*1.03*(depth+.15)/FOCAL+.05,.52);
}
// The plate and its letters turn with the tumble.
fn tumbling(id:u32)->bool {return id>=10u && in_space();}
fn pose(id:u32)->vec4f {
    if id>=10u {return vec4f(.045,-.045,0,1);}
    return vec4f(0,0,0,1);
}
fn exposure_frame()->f32 {return u.state.y-u.plate_open_center.w*(1.0-shutter);}
fn plate_quaternion()->vec4f {
    var open=u.plate_open_rotation;
    if dot(open,u.plate_rotation)<0.0 {open=-open;}
    return normalize(mix(open,u.plate_rotation,shutter));
}
fn center(id:u32)->vec3f {
    if id>=10u {
        var c=vec3f(0);
        if tumbling(id) {c=mix(u.plate_open_center.xyz,u.plate_center.xyz,shutter);}
        if id==11u {c+=world(PLATE,id);}
        return c;
    }
    let motion=u.flights[id];
    let frame=exposure_frame();
    // Begin farther into the starfield, meeting the fitted path before the
    // first cue. Camera crossing times and the original cue spacing stay fixed.
    let distance=(motion.x-frame)*motion.y;
    let retreat=1.0+.65*(1.0-smoothstep(0.0,48.0,frame));
    return vec3f(motion.z,motion.w,4.2-distance*retreat);
}
fn quaternion_rotate(p:vec3f,q:vec4f)->vec3f {
    return p+2.0*cross(q.xyz,cross(q.xyz,p)+q.w*p);
}
fn local_direction(p:vec3f,id:u32)->vec3f {
    var q=p;
    if tumbling(id) {let r=plate_quaternion();q=quaternion_rotate(q,vec4f(-r.xyz,r.w));}
    let a=pose(id);
    return rotate_z(rotate_y(rotate_x(q,-a.x),-a.y),-a.z)/a.w;
}
fn local(p:vec3f,id:u32)->vec3f {return local_direction(p-center(id),id);}
fn world(p:vec3f,id:u32)->vec3f {
    let a=pose(id);
    let w=rotate_x(rotate_y(rotate_z(p,a.z),a.y),a.x)*a.w;
    if tumbling(id) {return quaternion_rotate(w,plate_quaternion());}
    return w;
}
// Every model is a straight extrusion: 2D outline distance, half depth and
// bevel radius.
fn profile(p:vec2f,id:u32)->vec3f {
    if id==10u {return vec3f(panic_outline(p),.11,.022);}
    if id==11u {return vec3f(box2(p,plate_size(),.01),.08,.008);}
    return vec3f(letter_outline(p,min(id,9u)),LETTER_DEPTH,.024);
}
fn extrude(profile:vec3f,z:f32)->f32 {
    let q=vec2f(profile.x+profile.z,abs(z)-profile.y+profile.z);
    return min(max(q.x,q.y),0.0)+length(max(q,vec2f(0)))-profile.z;
}
fn shape(p:vec3f,id:u32)->f32 {return extrude(profile(p.xy,id),p.z);}
fn normal(p:vec3f,id:u32)->vec3f {
    let e=vec2f(.00035,0);
    return normalize(vec3f(shape(p+e.xyy,id)-shape(p-e.xyy,id),shape(p+e.yxy,id)-shape(p-e.yxy,id),shape(p+e.yyx,id)-shape(p-e.yyx,id)));
}
// The plate and its letters are out in space from the start, too far away to
// see until the doors' phase, and come up from black with the rest of space;
// once landed, they stay as the logo screen.
fn plate_opacity()->f32 {
    if in_space() {return smoothstep(0.0,1.5,u.screen.z);}
    return 1.0;
}
// The letters and their plate shade each other where they meet. `letters`
// says whether the letters are within the samples' reach of the plate.
fn occluder(p:vec3f,id:u32,letters:bool)->f32 {
    let d=shape(p,id);
    if id==11u && letters {return min(d,shape(p+PLATE,10u));}
    if id==10u {return min(d,shape(p-PLATE,11u));}
    return d;
}
fn ambient_occlusion(p:vec3f,n:vec3f,id:u32)->f32 {
    var occlusion=0.0;var weight=1.0;
    var letters=false;
    if id==11u && all(abs(p.xy+PLATE.xy)<object_extent(10u).xy+.15) {letters=shape(p+PLATE,10u)<.15;}
    for(var i=1;i<=5;i++) {
        let distance=f32(i)*.025;
        occlusion+=(distance-occluder(p+n*distance,id,letters))*weight;
        weight*=.6;
    }
    return clamp(1.0-occlusion*5.0,.25,1.0);
}
fn softbox(p:vec2f,center:vec2f,size:vec2f,softness:f32)->f32 {
    return 1.0-smoothstep(-softness,softness,box2(p-center,size,.08));
}
fn studio(p:vec3f,r:vec3f)->vec3f {
    var c=mix(vec3f(.035,.045,.065),vec3f(.20,.27,.38),r.y*.5+.5);
    if r.z>0.0 {
        let hit=p.xy+r.xy*((3.0-p.z)/max(r.z,.05));
        c+=vec3f(3.8,4.2,4.7)*softbox(hit,vec2f(-.35,1.15),vec2f(3.4,.95),.3);
        c+=vec3f(1.5,2.0,3.0)*softbox(hit,vec2f(-2.4,-.1),vec2f(.22,2.6),.20);
        c+=vec3f(3.5,2.8,1.8)*softbox(hit,vec2f(2.5,.25),vec2f(.4,2.1),.20);
        c+=vec3f(.9,1.1,1.5)*softbox(hit,vec2f(.5,-1.5),vec2f(3.0,.14),.12);
    }
    return c;
}
// Authored surroundings for the models while they are out in space. Directions
// that land on screen read the rendered backdrop, so reflections and refraction
// show the same stars, warp streaks and doors. Elsewhere a procedural sky of
// the same character fills in: it is not a copy of the visible starfield.
fn in_space()->bool {return u.state.x<4.0;}
// `blur` is the angular radius of the reflection lobe; wider lobes keep the
// same light per star, so a metal plate shows soft streaks, not glitter.
fn space_light(r:vec3f,blur:f32)->vec3f {
    let time=u.screen.z;
    let warp=smoothstep(12.0,14.2,time);
    let s=r.xy/(1.0+abs(r.z));
    let haze=noise(s*5.0+vec2f(time*.008,0))*.7+noise(s*11.0)*.3;
    let cloud=pow(max(0.0,1.0-abs(s.y+s.x*.28)*2.4),4.0)*haze;
    // The blue haze, a little brighter than the backdrop's so it reads in
    // reflections, and a cool glow towards the poles of travel during warp.
    var c=vec3f(.002,.004,.008)+vec3f(.014,.032,.07)*cloud;
    c+=vec3f(.06,.13,.30)*warp*pow(abs(r.z),10.0);
    // Stars in log-polar coordinates about the axis of travel, like the
    // visible field seen from both ends: ahead they stream outwards, behind
    // they recede into the centre, and the warp ramp stretches them radially.
    let ahead=r.z<0.0;
    let w=r.xy/max(abs(r.z),.02);
    let radius=max(length(w),1e-4);
    let cells=vec2f(560.0,60.0);
    let flow=select(1.0,-1.0,ahead);
    let g=vec2f((atan2(w.y,w.x)/6.283185+.5)*cells.x,log(radius)*cells.y+flow*star_travel(time)*2.2);
    // Radians per cell across and along, from the gnomonic mapping.
    let angular=radius/(1.0+radius*radius);
    let scale=vec2f(6.283185/cells.x,1.0/cells.y)*angular;
    let sharp=1.1/(u.screen.y*FOCAL*.5);
    let sigma=max(sharp,blur);
    let flux=sharp*sharp/(sigma*sigma);
    let tail=warp*.12;
    let reach=min(i32(ceil(tail/scale.y)),16);
    for(var k=0;k<=reach;k++) {
        // Tails trail away from the direction of motion in g.y.
        let cell=vec2f(floor(g.x),floor(g.y)-f32(k)*flow);
        // About 4000 stars per steradian, like the visible field, whatever
        // the cell size.
        if hash(cell)>min(4000.0*scale.x*scale.y,.6) {continue;}
        let star=cell+.15+.7*vec2f(hash(cell+17.3),hash(cell+41.9));
        let d=(g-star)*scale*vec2f(1.0,flow);
        let along=clamp(d.y,0.0,tail);
        let core=exp(-.5*(d.x*d.x+(d.y-along)*(d.y-along))/(sigma*sigma));
        let fade=select(1.0,pow(1.0-along/max(tail,1e-5),.65)*(.35+.65*warp),tail>0.0);
        let temperature=hash(cell+7.7);
        let color=mix(vec3f(.48,.66,1.0),vec3f(1.0,.95,.86),smoothstep(0.0,.75,temperature));
        c+=color*(.18+pow(hash(cell+3.1),3.4)*4.8)*core*fade*flux;
    }
    return c;
}
fn view_ray(screen:vec2f)->vec3f {
    let uv=(screen-.5)*vec2f(u.screen.x/u.screen.y,1)*2.0;
    return normalize(vec3f(uv.x,-uv.y,-FOCAL));
}
fn screen_of(direction:vec3f)->vec2f {
    let uv=vec2f(direction.x,-direction.y)*FOCAL/max(-direction.z,1e-4);
    return uv/(vec2f(u.screen.x/u.screen.y,1)*2.0)+.5;
}
fn scene_light(r:vec3f,blur:f32)->vec3f {
    if r.z>-.02 {return space_light(r,blur);}
    let uv=screen_of(r);
    let border=min(min(uv.x,1.0-uv.x),min(uv.y,1.0-uv.y));
    if border<=0.0 {return space_light(r,blur);}
    var seen=textureSampleLevel(backdrop,linear,uv,0).rgb;
    if blur>0.0 {
        // A rotated four-tap footprint about `blur` wide, so single-pixel
        // stars don't alias into sparkle when bent through the glass.
        let px=.5*blur*u.screen.y*FOCAL*.5/u.screen.xy;
        seen=.25*(textureSampleLevel(backdrop,linear,uv+px*vec2f(1,.33),0).rgb
                 +textureSampleLevel(backdrop,linear,uv+px*vec2f(-.33,1),0).rgb
                 +textureSampleLevel(backdrop,linear,uv+px*vec2f(-1,-.33),0).rgb
                 +textureSampleLevel(backdrop,linear,uv+px*vec2f(.33,-1),0).rgb);
    }
    // Well inside the frame the backdrop is all there is; skip the sky.
    let weight=smoothstep(0.0,.03,border)*smoothstep(-.02,-.12,r.z);
    if weight>=1.0 {return seen;}
    return mix(space_light(r,blur),seen,weight);
}
// `mirror` is the reflection without the cast-metal ripple: stars and
// streaks are so fine that the ripple would break them into glitter.
// `dim` keeps the studio panels dimmed as they are in space; `open` adds
// open space. The landed metal keeps the dimmed panels, so its light doesn't
// jump, but space no longer shows in it.
fn environment(p:vec3f,r:vec3f,mirror:vec3f,dim:bool,open:bool)->vec3f {
    let lights=studio(p,r);
    if !dim {return lights;}
    // The studio panels stay behind the camera, dimmed so the warp shows
    // in the plate; ahead lies open space.
    let dimmed=lights*mix(.2,.55,smoothstep(-.3,.3,r.z));
    if !open {return dimmed;}
    return dimmed+scene_light(mirror,.0012);
}
// Glass needs crisp lines of light rather than the broad panels that suit
// the metal: a dim sheen and thin strips behind the camera, by direction, so
// bevels catch them as continuous highlights. Authored lighting.
fn strips(r:vec3f)->vec3f {
    if r.z<=-.2 {return vec3f(0);}
    let line=r.xy/max(r.z,.2);
    // A broad soft panel above gives the faces a sheen that slides as the
    // letters cross the screen.
    var c=mix(vec3f(.004,.006,.010),vec3f(.035,.045,.065),smoothstep(-.4,.9,r.y));
    c+=vec3f(.9,1.05,1.3)*smoothstep(-.05,.35,line.y)*(1.0-smoothstep(.6,1.4,abs(line.x)));
    c+=vec3f(7.0,7.6,8.4)*(1.0-smoothstep(.02,.05,abs(line.y-.62)))*(1.0-smoothstep(.8,1.0,abs(line.x+.1)));
    c+=vec3f(2.4,3.4,5.2)*(1.0-smoothstep(.015,.04,abs(line.x+.75)))*(1.0-smoothstep(.6,.8,abs(line.y)));
    c+=vec3f(6.0,4.6,2.8)*(1.0-smoothstep(.015,.04,abs(line.x-.8)))*(1.0-smoothstep(.5,.7,abs(line.y-.1)));
    return c*smoothstep(-.2,.4,r.z);
}
fn glass_environment(r:vec3f,blur:f32)->vec3f {return strips(r)+scene_light(r,blur);}
fn direct_light(p:vec3f,n:vec3f,v:vec3f,position:vec3f,radiance:vec3f,f0:vec3f)->vec3f {
    return direct_light_rough(p,n,v,position,radiance,f0,.22);
}
fn direct_light_rough(p:vec3f,n:vec3f,v:vec3f,position:vec3f,radiance:vec3f,f0:vec3f,roughness:f32)->vec3f {
    let l=normalize(position-p);let h=normalize(l+v);
    let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),.001);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
    let a=roughness*roughness;let a2=a*a;
    let d=a2/(3.141593*pow(nh*nh*(a2-1.0)+1.0,2.0));
    let k=pow(roughness+1.0,2.0)/8.0;
    let geometry=nv/(nv*(1.0-k)+k)*nl/(nl*(1.0-k)+k);
    let fresnel=f0+(1.0-f0)*pow(1.0-vh,5.0);
    return min(d*geometry*fresnel/(4.0*nv*max(nl,.001)),vec3f(8))*radiance*nl;
}
fn cast_surface(p:vec2f)->f32 {
    return noise(p*vec2f(9,13))*.002+noise(p*vec2f(31,43))*.0006+noise(p*vec2f(190,270))*.00005;
}
// A shallow, flowing height field changes the reflected light without moving
// the letter outlines. This is an authored material animation, not source art.
fn surface_height(p:vec2f,id:u32)->f32 {
    // In space the letters are cast like their plate; they melt on landing.
    if id!=10u || in_space() {return cast_surface(p);}
    let t=max(u.screen.z-u.misc.z,0.0);
    let warm=smoothstep(-.25,.75,p.x+p.y*.38);
    let drift=vec2f(
        sin(p.y*5.8-t*.65)+.45*sin(p.x*4.7+p.y*2.3+t*.43),
        cos(p.x*5.1+t*.55)+.40*sin(p.y*7.0-p.x*2.1-t*.37)
    );
    let flow=p+drift*(.018+.014*warm)+vec2f(.014,-.006)*t;
    // Broad, slow waves carry the highlights; smaller cast-metal detail flows
    // across them. Extra movement at the hot end suggests a softer surface.
    let waves=.010*sin(p.x*8.0+p.y*4.0-t*1.1)
             +.006*sin(p.y*13.0-p.x*3.5+t*.8)
             +.002*sin(p.x*22.0+p.y*16.0-t*.6);
    return cast_surface(flow)+waves*(.75+.25*warm);
}
fn shade(p:vec3f,geometric_normal:vec3f,ray:vec3f,id:u32)->vec3f {
    let e=vec2f(.0008,0);
    let bump=vec2f(surface_height(p.xy+e.xy,id)-surface_height(p.xy-e.xy,id),surface_height(p.xy+e.yx,id)-surface_height(p.xy-e.yx,id))/(2.0*e.x);
    var weight=.35;
    if id==10u {weight*=smoothstep(0.0,.055,-panic_outline(p.xy));}
    let n=normalize(geometric_normal-vec3f(bump,0)*abs(geometric_normal.z)*weight);
    let grain=noise(p.xy*vec2f(22,95));
    // The letters only heat up once they have landed.
    var heat=0.0;
    if id==10u && !in_space() {
        let temper=p.x+p.y*.38+(noise(p.xy*vec2f(5,8))-.5)*.10;
        heat=smoothstep(-.25,.75,temper);
    }
    let oxide=smoothstep(.03,.15,heat)*(1.0-smoothstep(.20,.4,heat));
    var f0=mix(vec3f(.73,.82,.91),vec3f(.48,.24,.10),smoothstep(.02,.5,heat));
    f0=mix(f0,vec3f(.16,.12,.24),oxide*.6);
    // The plate and, until they land, the letters on it are one dark,
    // polished gunmetal. The landing's flash then shows the letters
    // cool to hot. Authored.
    if id==11u || in_space() {f0=vec3f(.24,.26,.30);}
    var wp=world(p,id);let wn=normalize(world(n,id));let view=normalize(-world(ray,id));
    // The plate is lit where it lies about the letters.
    if id==11u {wp+=world(PLATE,id);}
    let reflection=reflect(-view,wn);
    let fresnel=f0+(1.0-f0)*pow(1.0-max(dot(wn,view),0.0),5.0);
    var c=environment(wp,reflection,reflect(-view,normalize(world(geometric_normal,id))),in_space()||id==11u,in_space())*fresnel;
    // The key light breathes with the music's energy and lifts on hits; the
    // warm fill follows the bass. At typical playing levels they average close
    // to the former fixed strengths. An authored response, not original behaviour.
    let music=u.audio;
    c+=direct_light(wp,wn,view,vec3f(-2.0,3.0,3.0),vec3f(2.0,2.4,3.0)*(.82+.30*music.x+.25*music.y),f0);
    c+=direct_light(wp,wn,view,vec3f(3.0,-1.0,1.5),vec3f(1.0,.65,.25)*(.72+.50*music.z+.15*music.y),f0);
    c*=ambient_occlusion(p,n,id);
    c*=.94+.06*grain;
    if id==10u && !in_space() {
        // Incandescence, not gold paint: warm emission also feeds the HDR bloom.
        let flow=.75+.25*noise(p.xy*vec2f(9,15)+vec2f(0,u.screen.z*.05));
        let edge=1.0-smoothstep(.0,.055,-panic_outline(p.xy));
        // Hits briefly push the hot end towards yellow; the glow swells with
        // the bass. A strong downbeat re-strikes part of the arrival flash.
        let hot=heat*flow*(.95+.06*music.x+.07*music.y);
        let incandescent=mix(vec3f(1,.075,.002),vec3f(1,.72,.26),smoothstep(.5,1.0,hot));
        c+=incandescent*pow(hot,1.8)*(2.4+edge*2.3)*(.78+.25*music.z+.30*music.y);
        let flash=pow(1.0-clamp((u.screen.z-u.misc.z)/.9,0.0,1.0),3.0);
        c+=vec3f(3.0)*flash+vec3f(1.6,1.45,1.25)*pow(music.w,3.0);
    }
    return c;
}
// Door travel comes from the Rust timeline: Intro::step lowers `door` (passed
// as state.z) by 2 rows on every 70 Hz tick from sequence frame 108, so between
// ticks it is interpolated towards the next value with the tick phase, which is
// the fractional part of the half-frame sequence clock. On the logo it rests at
// the final value. Never infer coverage from a scanout pixel: stars can cross it.
fn door_amount()->f32 {
    if u.state.x<4.0 && u.state.y>=108.0 {return u.state.z-2.0*fract(u.state.y*2.0);}
    return u.state.z;
}
// Distances from the centre, as fractions of the height, of the back edge of
// the door's inner face (x) and of its lip (y). These follow the palette maths
// of Intro::doors: rows within door/2 stay open and the rim row sits at
// door*8/14. Reading the dark strip between them as the inner face seen in
// perspective, with the rim as the front lip, is an interpretation. The rim's
// end position (row 43, ~0.22) is where the original's logo screen and the
// faithful renderer draw the blue rail, so the modern rail is moved to the lip
// rather than stretching the door travel: the original framing of the logo is
// kept and the doors simply become the logo screen's frame without a jump.
fn door_edges()->vec2f {
    let amount=door_amount();
    return vec2f(amount/400.0,(amount*8.0/14.0+.5)/200.0);
}
// How far the doors' lips and front faces, from the lip's bead outwards, lie
// in front of the metal at a point on the screen. The plate passes in front
// of their inner faces and lands just behind the lips, which hide its edges.
fn door_front(uv:vec2f)->f32 {
    if u.state.x<3.0 {return 0.0;}
    let aa=.75/u.screen.y;
    let edge=door_edges().y-max(.006,3.0/u.screen.y);
    return smoothstep(edge-aa,edge+aa,abs(uv.y-.5));
}
// Authored: from the logo cue, the stars behind the landed plate fade out and
// the doors take the logo screen's light over a few tenths of a second,
// instead of cutting.
fn logo_fade()->f32 {
    if u.state.x<4.0 {return 0.0;}
    return smoothstep(0.0,.32,u.screen.z-u.misc.z);
}
fn panel_mask(uv:vec2f)->f32 {
    if u.state.x<3.0 {return 0.0;}
    let aa=.75/u.screen.y;
    let doors=smoothstep(door_edges().x-aa,door_edges().x+aa,abs(uv.y-.5));
    return max(doors,logo_fade());
}
// Authored light leaking through the gap from the warp-speed starfield.
fn door_spill(x:f32,gap:f32)->vec3f {
    let warp=smoothstep(12.0,14.2,u.screen.z);
    let narrow=clamp((.5-gap)/.31,0.0,1.0);
    let flicker=.7+.6*noise(vec2f(x*6.0,u.screen.z*9.0));
    return vec3f(.30,.52,1.0)*warp*(.30+.70*narrow*narrow)*flicker*(1.0-logo_fade());
}
// Authored bloom pulse when the doors reach their stop, with a faint contact
// ripple running outwards along the lip.
fn settle_pulse(x:f32)->f32 {
    let t=u.screen.z-u.misc.z;
    let decay=exp(-max(t,0.0)/.12)*smoothstep(-.05,0.0,t);
    let ripple=exp(-pow((abs(x)-max(t,0.0)*5.0)/.14,2.0));
    return decay*(.55+.9*ripple);
}
fn front_face(band:f32,uv:vec2f,lip:f32)->vec3f {
    var c=vec3f(.055,.061,.070)*(1.0-.45*smoothstep(.12,.5,band));
    // The brushed grain belongs to the moving door, not to the screen.
    c*=.99+.01*noise(vec2f(uv.x*70.0,(band-lip)*3000.0));
    return c;
}
// Authored door shading across the inner face, a rounded bead on the leading
// edge and a crease into the front face. Normals live in the (outwards, towards
// the camera) plane; `up` flips the key light between top and bottom doors.
fn door_panel(uv:vec2f,x:f32)->vec3f {
    let band=abs(uv.y-.5);
    let up=select(-1.0,1.0,uv.y<.5);
    let edges=door_edges();
    let px=1.0/u.screen.y;
    let bead=max(.006,3.0*px);
    let spill=door_spill(x,edges.x);
    let fade=logo_fade();
    let key=normalize(vec2f(.55*up,.8));
    let light=normalize(vec2f(-1.0,-.35));
    let s=(band-edges.y)/bead;
    var c=front_face(band,uv,edges.y);
    if s<1.0 {
        var n=vec2f(-1,0);
        if s>-1.0 {n=vec2f(s,sqrt(max(1.0-s*s,0.0)));}
        let h=normalize(key+vec2f(0,1));let hs=normalize(light+vec2f(0,1));
        // Ambient plus, on the logo screen, light bounced off the plate and
        // the hot end of the title.
        let warm=exp(-pow((x-.62)*1.6,2.0));
        var lit=vec3f(.010,.011,.014)+fade*(vec3f(.020,.022,.026)+vec3f(.030,.009,.002)*warm);
        lit+=vec3f(.075,.080,.088)*max(dot(n,key),0.0);
        lit+=vec3f(.50,.55,.62)*pow(max(dot(n,h),0.0),60.0)*step(-1.0,s);
        lit+=spill*(.10*max(dot(n,light),0.0)+.75*pow(max(dot(n,hs),0.0),30.0)*step(-1.0,s));
        if s<-1.0 {
            // Inner face: deeper parts see more of the gap and less of the key.
            // Passing streaks sweep outwards across it as moving bands of light.
            let depth=clamp((edges.y-bead-band)/max(edges.y-bead-edges.x,px),0.0,1.0);
            lit*=1.0-.35*depth*step(0.0,-dot(n,key));
            let streaks=.45+1.1*noise(vec2f(abs(x)*26.0-u.screen.z*22.0,depth*2.5+up*7.0));
            lit+=spill*.10*depth*depth*streaks;
            lit*=.88+.12*noise(vec2f(x*40.0,depth*6.0));
        }
        c=lit;
    }
    // Contact shadow in the crease between the bead and the front face, and
    // the rail's blue light along the bead, fed by the spill and the pulse.
    let crease=band-(edges.y+bead);
    let aa=px*.75;
    c*=1.0-.8*exp(-max(crease,0.0)/max(.005,3.0*px))*smoothstep(-aa,aa,crease);
    let rail=exp(-abs(band-(edges.y-.35*bead))/max(.0013,1.0*px));
    c+=(vec3f(.05,.16,.30)+spill*.35)*rail;
    let pulse=settle_pulse(x);
    let glow=sqrt(max(1.0-s*s,0.0))+rail;
    c+=vec3f(.55,.85,1.5)*2.4*pulse*glow;
    return c;
}
@fragment fn background(v:Vertex)->@location(0) vec4f {
    let xy=(v.uv-.5)*vec2f(u.screen.x/u.screen.y,1);
    let haze=noise(xy*4.0+vec2f(u.screen.z*.008,0))*.7+noise(xy*9.0)*.3;
    let cloud=pow(max(0.0,1.0-abs(xy.y+xy.x*.28)*2.0),4.0)*haze;
    var space=vec3f(.0015,.003,.006)+vec3f(.004,.009,.018)*cloud;
    space*=smoothstep(0.0,1.5,u.screen.z);
    if u.state.x<3.0 {return vec4f(space,1);}
    let aa=.75/u.screen.y;
    let doors=smoothstep(door_edges().x-aa,door_edges().x+aa,abs(v.uv.y-.5));
    // Once landed, the plate fills the opening between the doors.
    if doors<=0.0 {return vec4f(space,1);}
    return vec4f(mix(space,door_panel(v.uv,xy.x),doors),1);
}

// Integrate the speed ramp instead of multiplying time by a changing speed:
// star positions stay continuous when the authored warp cue begins at 12 s.
fn star_travel(time:f32)->f32 {
    let ramp=2.2;
    let x=clamp((time-12.0)/ramp,0.0,1.0);
    let integral=ramp*(x*x*x-.5*x*x*x*x)+max(time-12.0-ramp,0.0);
    return time*1.3+integral*15.0;
}
struct StarVertex {
    @builtin(position) position:vec4f,
    @location(0) pixel:vec2f,
    @location(1) color:vec3f,
    @location(2) power:f32,
    @location(3) uv:vec2f,
    @location(4) shape:vec3f, // core sigma in pixels, streak length, warp amount
}
@vertex fn star_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) index:u32)->StarVertex {
    let corners=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let seed=f32(index)+1.0;
    let xy=(vec2f(hash(vec2f(seed,1)),hash(vec2f(seed,2)))*2.0-1.0)*13.0;
    let warp=smoothstep(12.0,14.2,u.screen.z);
    let travel=star_travel(u.screen.z);
    let z=1.0+(1.0-fract(hash(vec2f(seed,3))+travel/18.0))*18.0;
    let aspect=u.screen.x/u.screen.y;
    let pos=xy/z*vec2f(1.0/aspect,1.0)*2.0;
    let exposure=mix(.003,.12,warp);
    let distance=travel-star_travel(max(u.screen.z-exposure,0.0));
    let previous=xy/(z+distance)*vec2f(1.0/aspect,1.0)*2.0;
    let motion=(pos-previous)*u.screen.xy*.5;
    let direction=normalize(pos*u.screen.xy+vec2f(.00001));
    let perpendicular=vec2f(-direction.y,direction.x);
    let scale=u.screen.y/1080.0;
    let sigma=max(.38,(.34+.25*hash(vec2f(seed,5)))*(.8+1.0/sqrt(z))*scale);
    let tail=min(length(motion)*warp,360.0*scale);
    let extent=sigma*3.5;
    let corner=corners[vertex];
    let pixel=vec2f(corner.x*(tail*.5+extent)-tail*.5,corner.y*extent);
    let screen=pos+(direction*pixel.x+perpendicular*pixel.y)*2.0/u.screen.xy;
    let temperature=hash(vec2f(seed,7));
    var color=mix(vec3f(.48,.66,1.0),vec3f(1.0,.95,.86),smoothstep(0.0,.75,temperature));
    color=mix(color,vec3f(1.0,.60,.30),smoothstep(.94,1.0,temperature));
    color=mix(color,vec3f(.65,.80,1.0),warp*.28);
    // A broad luminosity distribution: many faint stars, a few bright ones.
    let luminosity=.18+pow(hash(vec2f(seed,9)),3.4)*4.8;
    let shimmer=1.0+.08*sin(u.screen.z*(.7+hash(vec2f(seed,10))*1.6)+seed);
    let power=luminosity*(.85+1.5/z)*shimmer*(1.0+.25*warp);
    let fade=smoothstep(1.0,2.0,z)*(1.0-smoothstep(17.0,19.0,z))*smoothstep(0.0,1.2,u.screen.z);
    return StarVertex(vec4f(screen,0,1),pixel,color,power*fade,vec2f(screen.x*.5+.5,.5-screen.y*.5),vec3f(sigma,tail,warp));
}
@fragment fn star_fragment(v:StarVertex)->@location(0) vec4f {
    let visibility=1.0-panel_mask(v.uv);
    if visibility<=0.0 {discard;}
    let sigma=v.shape.x;let tail=v.shape.y;let warp=v.shape.z;
    // Evaluate the head in pixel space so stretching a streak never stretches
    // the round core into the old thick oval. The trail has its own thin profile.
    let radius2=dot(v.pixel,v.pixel)/(sigma*sigma);
    let core=exp(-radius2*.5);
    let halo=exp(-radius2*.10)*.025;
    let across=exp(-pow(v.pixel.y/(sigma*.75),2.0)*.5);
    let ends=smoothstep(-tail-sigma,-tail+sigma,v.pixel.x)*(1.0-smoothstep(-sigma,sigma,v.pixel.x));
    let taper=pow(clamp((v.pixel.x+tail)/max(tail,.001),0.0,1.0),.65);
    let trail=across*ends*taper*warp*.65;
    return vec4f(v.color*(core+halo+trail)*v.power*visibility,1);
}

fn object_extent(id:u32)->vec3f {
    if id==10u {return vec3f(1.49,.42,.15);}
    if id==11u {return vec3f(plate_size()+.03,.11);}
    return vec3f(.81,.52,.27);
}
// `coarse` is for coverage only: it accepts a hit within about a pixel and
// gives up sooner, since the exact surface point isn't needed.
fn trace(origin:vec3f,ray:vec3f,id:u32,coarse:bool)->vec4f {
    let extent=object_extent(id);
    let inv=1.0/ray;let slab0=(-extent-origin)*inv;let slab1=(extent-origin)*inv;
    let entry=min(slab0,slab1);let leave=max(slab0,slab1);
    let near=max(max(entry.x,entry.y),entry.z);let far=min(min(leave.x,leave.y),leave.z);
    // Misses return a lower bound on the angle between the ray and the shape
    // in x, so the model pass can tell which pixels nothing can reach.
    if near>far || far<0.0 {
        let along=-dot(origin,ray);
        let radius=length(extent);
        if along<=0.0 {return vec4f(select(1.0,0.0,length(origin)<radius),0,0,-1);}
        return vec4f(max(length(origin+ray*along)-radius,0.0)/along,0,0,-1);
    }
    var travel=max(near,0.0);var p=origin+ray*travel;
    var clearance=1.0;
    let epsilon=max(.00006,travel/(u.screen.y*3.8)*select(.25,1.0,coarse));
    let steps=select(100,40,coarse);
    // Beside a wall the 3D distance stays tiny while the ray runs parallel
    // to it. Within the slab, the outline distance over the ray's sideways
    // speed is still a safe step, so silhouettes no longer cost every step.
    let sideways=max(length(ray.xy),1e-3);
    for(var i=0;i<steps;i++) {
        p=origin+ray*travel;
        let outline=profile(p.xy,id);
        let distance=extrude(outline,p.z);
        if distance<epsilon {return vec4f(p,travel);}
        // Between samples the distance can fall to a fraction of this.
        clearance=min(clearance,distance*.1/max(travel,1e-3));
        var step=distance*.8;
        if outline.x>0.0 && abs(p.z)<outline.y-outline.z {step=max(step,outline.x/sideways*.9);}
        travel+=step;
        if travel>far {break;}
    }
    return vec4f(clearance,0,0,-1);
}
fn hit_object(id:u32,direction:vec3f)->vec4f {
    return trace(local(CAMERA,id),normalize(local_direction(direction,id)),id,false);
}
fn object_range()->vec2u {
    // After the last laser-cut tile has gone (the renderer's 78-tick window),
    // nothing shows the logo again: the next scene replaces it.
    if u.transition.y>.5 && u.transition.x>u.transition.z+78.0 {return vec2u(0,0);}
    if u.state.x==1.0 {return vec2u(0,12);}
    return vec2u(10,12);
}
// Objects whose swept box reaches the current tile, as bits from the first
// object of the phase; everything else is skipped.
var<private> relevant:u32=0xffffffffu;
fn is_relevant(id:u32)->bool {return ((relevant>>(id-object_range().x))&1u)!=0u;}
fn letter_present(id:u32)->bool {return id>=10u || exposure_frame()<u.flights[id].x;}
fn letter_fade()->f32 {return smoothstep(0.0,30.0,u.state.y);}

// Path from a point on or inside a letter to where it leaves the block, as
// the outward normal there and the distance travelled.
const LETTER_DEPTH=.24;
fn cross_letter(start:vec3f,direction:vec3f,id:u32)->vec4f {
    var slab=1e6;
    if abs(direction.z)>1e-5 {slab=max((sign(direction.z)*LETTER_DEPTH-start.z)/direction.z,0.0);}
    let speed=length(direction.xy);
    if speed<1e-4 {return vec4f(0,0,sign(direction.z),slab);}
    var t=0.0;
    var entered=false;
    for(var i=0;i<64;i++) {
        let o=letter_outline(start.xy+direction.xy*t,id);
        if o< -.0004 {entered=true;}
        if (entered && o>=0.0) || t>=slab {
            if t<slab {t=max(t-o/speed,0.0);}
            break;
        }
        t+=max(abs(o),.0015)/speed;
    }
    if t>=slab {return vec4f(0,0,sign(direction.z),slab);}
    let p=start.xy+direction.xy*t;
    let e=vec2f(.0005,0);
    let gradient=vec2f(letter_outline(p+e.xy,id)-letter_outline(p-e.xy,id),letter_outline(p+e.yx,id)-letter_outline(p-e.yx,id));
    return vec4f(normalize(vec3f(gradient,0)+vec3f(0,0,1e-6)),t);
}

// Authored glass for the FUTURE CREW letters. Light refracts in, crosses the
// block (with up to two internal reflections), and refracts out with a little
// dispersion; the body absorbs red first, giving a cool tint that deepens
// along longer paths.
const GLASS_IOR=vec3f(1.498,1.505,1.512);
const GLASS_ABSORPTION=vec3f(1.9,.75,.3);
fn glass(p:vec3f,ray:vec3f,id:u32,behind:vec3f,layered:bool)->vec3f {
    // On the flat cap the distance field's gradient is exactly ±z.
    var geometric=vec3f(0,0,sign(p.z));
    let outline=letter_outline(p.xy,id);
    if outline>-.0245 {geometric=normal(p,id);}
    // Cast-glass ripples bend the light without moving the outline.
    let e=vec2f(.0008,0);
    let bump=vec2f(cast_surface(p.xy+e.xy)-cast_surface(p.xy-e.xy),cast_surface(p.xy+e.yx)-cast_surface(p.xy-e.yx))/(2.0*e.x);
    let n=normalize(geometric-vec3f(bump,0)*abs(geometric.z)*.5);
    // Refraction gets only a trace of the ripple: stronger bending scatters
    // the sparse stars into sparkle.
    let inward=normalize(geometric-vec3f(bump,0)*abs(geometric.z)*.12);
    let wp=world(p,id);let wn=normalize(world(n,id));let wray=normalize(world(ray,id));
    let cosine=max(dot(n,-ray),0.0);
    let f0=.04;
    let fresnel=f0+(1.0-f0)*pow(1.0-cosine,5.0);
    var color=glass_environment(reflect(wray,wn),0.0)*fresnel;
    color+=direct_light_rough(wp,wn,-wray,vec3f(-2.0,3.0,3.0),vec3f(2.0,2.4,3.0),vec3f(f0),.07);
    color+=direct_light_rough(wp,wn,-wray,vec3f(3.0,-1.0,1.5),vec3f(1.0,.65,.25),vec3f(f0),.07);
    // The letters are straight extrusions, so the back face is a plane and
    // only the 2D outline needs marching (bevels are ignored inside).
    var direction=refract(ray,inward,1.0/GLASS_IOR.y);
    var q=p;
    var travelled=0.0;
    var exit_normal=geometric;
    var out=direction;
    for(var bounce=0;bounce<3;bounce++) {
        let leg=cross_letter(q,direction,id);
        q+=direction*leg.w;
        travelled+=leg.w;
        exit_normal=leg.xyz;
        out=refract(direction,-exit_normal,GLASS_IOR.y);
        if dot(out,out)>.0001 {break;}
        // Total internal reflection: another leg inside the block.
        direction=reflect(direction,exit_normal);
        out=direction;
    }
    let tint=exp(-GLASS_ABSORPTION*travelled);
    var transmitted=behind;
    if !layered {
        // Light crossing the block softens with distance travelled, which
        // also keeps the sparse stars from aliasing.
        let blur=(1.0+2.5*travelled)/(u.screen.y*FOCAL*.5);
        for(var c=0;c<3;c++) {
            var bent=refract(direction,-exit_normal,GLASS_IOR[c]);
            if dot(bent,bent)<.0001 {bent=out;}
            transmitted[c]=glass_environment(normalize(world(bent,id)),blur)[c];
        }
    }
    // Light trapped by internal reflection leaks out near the edges and
    // bevels, so the blocks read against black space; brighter towards the
    // top strip. A little scattering lets thick paths glow faintly.
    // Distance to the nearest edge line: on a face one coordinate is zero.
    let edge=length(vec2f(max(-outline,0.0),max(LETTER_DEPTH-abs(p.z),0.0)));
    var rim=0.0;
    if edge<.1 {
        let e2=vec2f(.0015,0);
        let rim_normal=normalize(vec2f(letter_outline(p.xy+e2.xy,id)-letter_outline(p.xy-e2.xy,id),letter_outline(p.xy+e2.yx,id)-letter_outline(p.xy-e2.yx,id))+1e-6);
        rim=exp(-edge/.016)*(.25+.75*smoothstep(-.2,1.0,dot(rim_normal,vec2f(-.35,.94))));
    }
    let scatter=(1.0-exp(-1.4*travelled))*vec3f(.006,.016,.03);
    return color+(transmitted*tint+scatter+rim*vec3f(.20,.42,.75))*(1.0-fresnel);
}

fn render(screen:vec2f)->vec3f {
    let direction=view_ray(screen);
    let background=textureSampleLevel(backdrop,linear,screen,0).rgb;
    let range=object_range();
    // Sort the actual ray hits, then composite from the back, so distant
    // letters and stars stay visible through the nearer glass.
    var distances:array<f32,12>;
    var hits:array<vec4f,12>;
    var ids:array<u32,12>;
    var count=0u;
    for(var id=range.x;id<range.y;id++) {
        if !is_relevant(id) || !letter_present(id) {continue;}
        let hit=hit_object(id,direction);
        if hit.w<0.0 {continue;}
        var slot=count;
        loop {
            if slot==0u {break;}
            if distances[slot-1u]<=hit.w {break;}
            distances[slot]=distances[slot-1u];
            hits[slot]=hits[slot-1u];
            ids[slot]=ids[slot-1u];
            slot-=1u;
        }
        distances[slot]=hit.w;hits[slot]=hit;ids[slot]=id;
        count+=1u;
    }
    // The letters and the metal are opaque: nothing behind the nearest of
    // them shows once it is wholly there.
    var last=i32(count)-1;
    for(var i=0;i<i32(count);i++) {
        if ids[i]>=10u && plate_opacity()>=1.0 {last=i;break;}
    }
    var color=background;
    for(var i=last;i>=0;i--) {
        let id=ids[i];
        let ray=normalize(local_direction(direction,id));
        let p=hits[i].xyz;
        if id>=10u {
            let lit=shade(p,normal(p,id),ray,id);
            let opacity=plate_opacity();
            color=select(mix(color,lit,opacity),lit,opacity>=1.0);
            continue;
        }
        let layer=glass(p,ray,id,color,u32(i)+1u<count);
        color=mix(color,layer,letter_fade());
    }
    if count>0u {color=mix(color,background,door_front(screen));}
    return color;
}

// Pixel-space bounds of an object's box at the current shutter position.
// Boxes reaching the camera plane cover everything.
fn screen_box(id:u32)->vec4f {
    let e=object_extent(id);
    let c=center(id);
    var low=vec2f(1e9);var high=vec2f(-1e9);
    for(var k=0u;k<8u;k++) {
        let corner=vec3f(select(-e.x,e.x,(k&1u)!=0u),select(-e.y,e.y,(k&2u)!=0u),select(-e.z,e.z,(k&4u)!=0u));
        let q=world(corner,id)+c-CAMERA;
        if -q.z<.05 {return vec4f(-1e9,-1e9,1e9,1e9);}
        let pixel=vec2f(q.x,-q.y)*FOCAL/(-q.z)*u.screen.y*.5+u.screen.xy*.5;
        low=min(low,pixel);high=max(high,pixel);
    }
    return vec4f(low,high);
}
const MOTION_SAMPLES=16u;
const EDGE_SAMPLES=6u;
const TILE=8.0;
// Per 8 × 8 tile: visibility samples needed for the motion of every object
// whose swept box reaches it (about one per 2 pixels of travel), or zero
// when none can. Doing this per tile keeps the box maths out of each pixel.
@fragment fn tiles(v:Vertex)->@location(0) vec4f {
    let low_corner=floor(v.position.xy)*TILE-2.0;
    let high_corner=low_corner+TILE+4.0;
    let range=object_range();
    var count=0u;
    var mask=0u;
    var reach=0.0;
    for(var id=range.x;id<range.y;id++) {
        shutter=0.0;
        let present_open=letter_present(id);
        let a=screen_box(id);
        shutter=1.0;
        let present_close=letter_present(id);
        let b=screen_box(id);
        if !present_open && !present_close {continue;}
        let low=min(a.xy,b.xy);let high=max(a.zw,b.zw);
        if any(high<low_corner) || any(low>high_corner) {continue;}
        mask|=1u<<(id-range.x);
        // Very long smears hide sampling noise, so they get fewer samples:
        // the full count up to about 128 pixels of travel, tapering to half
        // for a letter passing right by the camera.
        if a.x<-1e8 || b.x<-1e8 {count=max(count,MOTION_SAMPLES/2u);reach=60000.0;continue;}
        let travel=max(length(b.xy-a.xy),length(b.zw-a.zw));
        let most=u32(mix(f32(MOTION_SAMPLES),f32(MOTION_SAMPLES/2u),smoothstep(128.0,512.0,travel)));
        count=max(count,clamp(u32(ceil(travel/2.0)),1u,most));
        reach=max(reach,travel);
    }
    shutter=1.0;
    // Samples, object mask and the farthest any of them moves, in pixels.
    return vec4f(f32(count),f32(mask),min(reach,60000.0),0);
}
fn tile_at(pixel:vec2f)->vec4f {return textureLoad(tile_map,vec2i(pixel/TILE),0);}
const NOTHING=255u;
fn nearest_object(screen:vec2f)->vec2f {
    let direction=view_ray(screen);
    let range=object_range();
    var nearest=vec2f(f32(NOTHING),1e6);
    for(var id=range.x;id<range.y;id++) {
        if !is_relevant(id) || !letter_present(id) {continue;}
        let hit=trace(local(CAMERA,id),normalize(local_direction(direction,id)),id,true);
        if hit.w>=0.0 && hit.w<nearest.y {nearest=vec2f(f32(id),hit.w);}
    }
    return nearest;
}

fn pixel_of(world_point:vec3f)->vec2f {
    let q=world_point-CAMERA;
    return vec2f(q.x,-q.y)*FOCAL/max(-q.z,1e-4)*u.screen.y*.5+u.screen.xy*.5;
}
// How far, in pixels, an object's box moves on screen during the exposure.
fn box_travel(id:u32)->f32 {
    shutter=0.0;
    let a=screen_box(id);
    shutter=1.0;
    let b=screen_box(id);
    if a.x<-1e8 || b.x<-1e8 {return 1e9;}
    return max(length(b.xy-a.xy),length(b.zw-a.zw));
}
// Centre-ray coverage: object id + 1 (0 for none), hit distance, and a
// margin in pixels. A positive margin means the motion within the exposure
// can't bring a different surface under this pixel: the face it hits extends
// farther than its own point moves, and every other object stays farther
// away than its box moves.
@fragment fn probe(v:Vertex)->@location(0) vec4f {
    let tile=tile_at(v.position.xy);
    if tile.x==0.0 {return vec4f(0);}
    relevant=u32(tile.y);
    let direction=view_ray(v.uv);
    let range=object_range();
    let angle_to_pixels=FOCAL*u.screen.y*.5;
    var nearest=vec4f(0,1e6,0,0);
    var margin=1e9;
    var nearest_ray=vec3f(0);
    var nearest_point=vec3f(0);
    for(var id=range.x;id<range.y;id++) {
        if !is_relevant(id) {continue;}
        if !letter_present(id) {
            // Passing the camera during the exposure: it was here at the
            // start, so leave this pixel to the full coverage samples.
            shutter=0.0;
            if letter_present(id) {margin=-1.0;}
            shutter=1.0;
            continue;
        }
        let ray=normalize(local_direction(direction,id));
        let hit=trace(local(CAMERA,id),ray,id,false);
        if hit.w<0.0 {
            margin=min(margin,hit.x*angle_to_pixels-box_travel(id)*1.1-2.0);
            continue;
        }
        if hit.w>=nearest.y {continue;}
        nearest=vec4f(f32(id+1u),hit.w,0,0);
        nearest_ray=ray;
        nearest_point=hit.xyz;
    }
    if nearest.x>0.0 {
        let id=u32(nearest.x)-1u;
        // Hits on a flat cap stay on it for the outline distance, shortened
        // by how obliquely the cap is seen. Walls and bevels have none.
        let shape=profile(nearest_point.xy,id);
        var inside=0.0;
        if abs(nearest_point.z)>shape.y-shape.z-.002 {
            inside=max(-shape.x-shape.z,0.0)*abs(nearest_ray.z)/nearest.y*angle_to_pixels;
        }
        // The point's own path through the exposure, plus the change in
        // scale across that much face as the object approaches.
        shutter=0.0;
        let open=world(nearest_point,id)+center(id);
        shutter=1.0;
        let close=world(nearest_point,id)+center(id);
        let moved=length(pixel_of(close)-pixel_of(open));
        let scale=abs(1.0-(CAMERA.z-close.z)/max(CAMERA.z-open.z,1e-4));
        margin=min(margin,inside-(moved*1.2+inside*scale*1.2+2.0));
    }
    return vec4f(nearest.xy,margin,0);
}

@fragment fn model(v:Vertex)->@location(0) vec4f {
    let background=textureSampleLevel(backdrop,linear,v.uv,0).rgb;
    let range=object_range();
    if range.x==range.y {return vec4f(background,1);}
    let tile=tile_at(v.position.xy);
    var moving=u32(tile.x);
    if moving==0u {return vec4f(background,1);}
    relevant=u32(tile.y);
    let pixel=vec2i(v.position.xy);
    let limit=vec2i(textureDimensions(probe_map))-1;
    var probe:array<vec4f,9>;
    for(var i=0;i<9;i++) {
        probe[i]=textureLoad(probe_map,clamp(pixel+vec2i(i%3-1,i/3-1),vec2i(0),limit),0);
    }
    let centre=probe[4];
    var edge=false;
    var samples=EDGE_SAMPLES;
    for(var i=0;i<9;i++) {
        if abs(probe[i].x-centre.x)>.5 || abs(probe[i].y-centre.y)>.02*centre.y {edge=true;}
    }
    // Bevels show as curvature in depth; their thin highlights alias too.
    if centre.x>.5 && !edge {
        let footprint=centre.y*2.0/(u.screen.y*FOCAL);
        let curvature=max(abs(probe[3].y+probe[5].y-2.0*centre.y),abs(probe[1].y+probe[7].y-2.0*centre.y));
        edge=curvature>.05*footprint;
        samples=4u;
    }
    // Nothing within reach of the motion: coverage can't change during the
    // exposure, so one shade at mid-exposure (or the backdrop) is exact.
    var settled=false;
    if moving>1u && centre.z>0.0 {
        if centre.x<.5 {return vec4f(background,1);}
        settled=true;
    }
    // Interleaved gradient noise decorrelates neighbouring pixels' patterns.
    let jitter=fract(52.9829189*fract(dot(v.position.xy,vec2f(.06711056,.00583715))));
    // Shading jobs: screen position, shutter position and weight. One call
    // site keeps the shader small.
    var jobs:array<vec4f,6>;
    var job_count=1u;
    var sum=vec3f(0);
    jobs[0]=vec4f(v.uv,1,1);
    if settled {
        jobs[0].z=.5;
    } else if moving<=1u && edge {
        // Static silhouettes and folds: full shading at each sample.
        job_count=samples;
        for(var i=0u;i<samples;i++) {
            let r2=fract(vec2f(.7548776662,.5698402910)*f32(i+1u)+jitter);
            jobs[i]=vec4f((v.position.xy+(r2-.5)*1.2)/u.screen.xy,1,1.0/f32(samples));
        }
    } else if moving>1u {
        // Moving objects: many cheap visibility samples through the exposure,
        // then one full shade per object seen, at the mean of its samples and
        // weighted by its coverage. Colour changes far less within 1/100 s
        // than coverage does, and the mean keeps neighbouring pixels steady.
        var objects=array<u32,3>(NOTHING,NOTHING,NOTHING);
        var hits=array<u32,3>(0u,0u,0u);
        var mean:array<vec3f,3>;
        for(var i=0u;i<moving;i++) {
            let r2=fract(vec2f(.7548776662,.5698402910)*f32(i+1u)+jitter);
            shutter=(f32(i)+fract(jitter*7.31+f32(i)*.6180339887))/f32(moving);
            let screen=(v.position.xy+(r2-.5)*1.2)/u.screen.xy;
            let id=u32(nearest_object(screen).x);
            if id==NOTHING {sum+=textureSampleLevel(backdrop,linear,screen,0).rgb;continue;}
            var slot=2u;
            for(var k=0u;k<3u;k++) {
                if objects[k]==id || objects[k]==NOTHING {slot=k;break;}
            }
            if objects[slot]==NOTHING {objects[slot]=id;mean[slot]=vec3f(0);}
            mean[slot]+=vec3f(screen,shutter);
            hits[slot]+=1u;
        }
        sum/=f32(moving);
        job_count=0u;
        for(var k=0u;k<3u;k++) {
            if hits[k]==0u {continue;}
            // Fully covered: shade the pixel centre at mid-exposure, so fast
            // reflections stay steady rather than following each pixel's jitter.
            var at=mean[k]/f32(hits[k]);
            if hits[k]==moving {at=vec3f(v.uv,.5);}
            jobs[job_count]=vec4f(at,f32(hits[k])/f32(moving));
            job_count++;
        }
    }
    for(var j=0u;j<job_count;j++) {
        shutter=jobs[j].z;
        sum+=render(jobs[j].xy)*jobs[j].w;
    }
    shutter=1.0;
    return vec4f(sum,1);
}
