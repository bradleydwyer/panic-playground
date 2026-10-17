// New geometric models, evaluated in 3D. This shader does not sample the old
// artwork for geometry. Letterforms and the continuous plate tumble are new;
// flights fit the locally decoded sequence. Doors use the Rust timeline.
struct Controls { screen:vec4f, state:vec4f, misc:vec4f, flights:array<vec4f,10>, plate_center:vec4f, plate_rotation:vec4f }
@group(0) @binding(0) var<uniform> u:Controls;
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
fn pose(id:u32)->vec4f {
    if id==10u {return vec4f(.045,-.045,0,1);}
    return vec4f(0,0,0,1);
}
fn center(id:u32)->vec3f {
    if id==11u {return u.plate_center.xyz;}
    if id==10u {return vec3f(0);}
    let motion=u.flights[id];
    // Begin farther into the starfield, meeting the fitted path before the
    // first cue. Camera crossing times and the original cue spacing stay fixed.
    let distance=(motion.x-u.state.y)*motion.y;
    let retreat=1.0+.65*(1.0-smoothstep(0.0,48.0,u.state.y));
    return vec3f(motion.z,motion.w,4.2-distance*retreat);
}
fn quaternion_rotate(p:vec3f,q:vec4f)->vec3f {
    return p+2.0*cross(q.xyz,cross(q.xyz,p)+q.w*p);
}
fn local_direction(p:vec3f,id:u32)->vec3f {
    if id==11u {return quaternion_rotate(p,vec4f(-u.plate_rotation.xyz,u.plate_rotation.w));}
    let a=pose(id);
    return rotate_z(rotate_y(rotate_x(p,-a.x),-a.y),-a.z)/a.w;
}
fn local(p:vec3f,id:u32)->vec3f {return local_direction(p-center(id),id);}
fn world(p:vec3f,id:u32)->vec3f {
    if id==11u {return quaternion_rotate(p,u.plate_rotation);}
    let a=pose(id);
    return rotate_x(rotate_y(rotate_z(p,a.z),a.y),a.x)*a.w;
}
fn shape(p:vec3f,id:u32)->f32 {
    var outline=letter_outline(p.xy,min(id,9u));var depth=.24;var bevel=.010;
    if id==10u {outline=panic_outline(p.xy);depth=.11;bevel=.022;}
    if id==11u {outline=box2(p.xy,vec2f(1.45,.47),.01);depth=.08;bevel=.008;}
    let q=vec2f(outline+bevel,abs(p.z)-depth+bevel);
    return min(max(q.x,q.y),0.0)+length(max(q,vec2f(0)))-bevel;
}
fn normal(p:vec3f,id:u32)->vec3f {
    let e=vec2f(.00035,0);
    return normalize(vec3f(shape(p+e.xyy,id)-shape(p-e.xyy,id),shape(p+e.yxy,id)-shape(p-e.yxy,id),shape(p+e.yyx,id)-shape(p-e.yyx,id)));
}
fn ambient_occlusion(p:vec3f,n:vec3f,id:u32)->f32 {
    var occlusion=0.0;var weight=1.0;
    for(var i=1;i<=5;i++) {
        let distance=f32(i)*.025;
        occlusion+=(distance-shape(p+n*distance,id))*weight;
        weight*=.6;
    }
    return clamp(1.0-occlusion*5.0,.25,1.0);
}
fn softbox(p:vec2f,center:vec2f,size:vec2f,softness:f32)->f32 {
    return 1.0-smoothstep(-softness,softness,box2(p-center,size,.08));
}
fn environment(p:vec3f,r:vec3f)->vec3f {
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
fn direct_light(p:vec3f,n:vec3f,v:vec3f,position:vec3f,radiance:vec3f,f0:vec3f)->vec3f {
    let l=normalize(position-p);let h=normalize(l+v);
    let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),.001);let nh=max(dot(n,h),0.0);let vh=max(dot(v,h),0.0);
    let roughness=.22;let a=roughness*roughness;let a2=a*a;
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
    if id!=10u {return cast_surface(p);}
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
    var heat=0.0;
    if id==10u {
        let temper=p.x+p.y*.38+(noise(p.xy*vec2f(5,8))-.5)*.10;
        heat=smoothstep(-.25,.75,temper);
    }
    let oxide=smoothstep(.03,.15,heat)*(1.0-smoothstep(.20,.4,heat));
    var f0=mix(vec3f(.73,.82,.91),vec3f(.48,.24,.10),smoothstep(.02,.5,heat));
    f0=mix(f0,vec3f(.16,.12,.24),oxide*.6);
    let wp=world(p,id);let wn=normalize(world(n,id));let view=normalize(-world(ray,id));
    let reflection=reflect(-view,wn);
    let fresnel=f0+(1.0-f0)*pow(1.0-max(dot(wn,view),0.0),5.0);
    var c=environment(wp,reflection)*fresnel;
    c+=direct_light(wp,wn,view,vec3f(-2.0,3.0,3.0),vec3f(2.0,2.4,3.0),f0);
    c+=direct_light(wp,wn,view,vec3f(3.0,-1.0,1.5),vec3f(1.0,.65,.25),f0);
    c*=ambient_occlusion(p,n,id);
    c*=.94+.06*grain;
    if id==10u {
        // Incandescence, not gold paint: warm emission also feeds the HDR bloom.
        let flow=.75+.25*noise(p.xy*vec2f(9,15)+vec2f(0,u.screen.z*.05));
        let edge=1.0-smoothstep(.0,.055,-panic_outline(p.xy));
        let hot=heat*flow;
        let incandescent=mix(vec3f(1,.075,.002),vec3f(1,.72,.26),smoothstep(.5,1.0,hot));
        c+=incandescent*pow(hot,1.8)*(2.4+edge*2.3);
        let flash=pow(1.0-clamp((u.screen.z-u.misc.z)/.9,0.0,1.0),3.0);
        c+=vec3f(3.0)*flash;
    }
    return c;
}
fn panel_mask(uv:vec2f)->f32 {
    if u.state.x>=4.0 {return 1.0;}
    if u.state.x<3.0 {return 0.0;}
    // Intro::step closes the doors by 2 rows per 70 Hz tick from frame 108.
    // Never infer panel coverage from a scanout pixel: stars can cross it.
    let amount=clamp(200.0-4.0*max(u.state.y-108.0,0.0),0.0,200.0);
    let edge=amount/400.0;
    let aa=1.0/u.screen.y;
    return smoothstep(edge-aa,edge+aa,abs(uv.y-.5));
}
@fragment fn background(v:Vertex)->@location(0) vec4f {
    let xy=(v.uv-.5)*vec2f(u.screen.x/u.screen.y,1);
    let haze=noise(xy*4.0+vec2f(u.screen.z*.008,0))*.7+noise(xy*9.0)*.3;
    let cloud=pow(max(0.0,1.0-abs(xy.y+xy.x*.28)*2.0),4.0)*haze;
    var space=vec3f(.0015,.003,.006)+vec3f(.004,.009,.018)*cloud;
    space*=smoothstep(0.0,1.5,u.screen.z);
    let band=abs(v.uv.y-.5);
    var panel=vec3f(.055,.061,.070)*(1.0-.45*smoothstep(.12,.5,band));
    panel+=vec3f(.018)*exp(-pow((v.uv.y-.36)*5.0,2.0));
    panel*=.99+.01*noise(vec2f(v.uv.x*70.0,v.uv.y*3000.0));
    let rail=abs(band-.245);
    panel=mix(panel,vec3f(.006,.008,.012),1.0-smoothstep(.001,.003,rail));
    panel+=vec3f(.06,.18,.32)*exp(-rail*1700.0);
    // A soft mounting shadow below the 3D title on the recessed panel.
    if u.state.x>=4.0 {
        let uv=(v.uv-.5)*vec2f(u.screen.x/u.screen.y,1.0)*2.0;
        let q=vec2f(uv.x, -uv.y)*1.10-vec2f(.025,-.04);
        let distance=panic_outline(q);
        panel*=1.0-.65*exp(-max(distance,0.0)*28.0);
        panel+=vec3f(.055,.012,.001)*exp(-dot((uv-vec2f(.70,0))*vec2f(1.1,2.0),(uv-vec2f(.70,0))*vec2f(1.1,2.0)));
    }
    return vec4f(mix(space,panel,panel_mask(v.uv)),1);
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
    if u.state.x>=4.0 {discard;}
    let visibility=1.0-panel_mask(v.uv);
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

fn trace(origin:vec3f,ray:vec3f,id:u32)->vec4f {
    var extent=vec3f(.81,.52,.27);
    if id==10u {extent=vec3f(1.49,.42,.15);}
    if id==11u {extent=vec3f(1.48,.50,.11);}
    let inv=1.0/ray;let slab0=(-extent-origin)*inv;let slab1=(extent-origin)*inv;
    let entry=min(slab0,slab1);let leave=max(slab0,slab1);
    let near=max(max(entry.x,entry.y),entry.z);let far=min(min(leave.x,leave.y),leave.z);
    if near>far || far<0.0 {return vec4f(0,0,0,-1);}
    var travel=max(near,0.0);var p=origin+ray*travel;
    let epsilon=max(.00006,travel/(u.screen.y*3.8)*.25);
    for(var i=0;i<100;i++) {
        p=origin+ray*travel;
        let distance=shape(p,id);
        if distance<epsilon {return vec4f(p,travel);}
        travel+=distance*.8;
        if travel>far {break;}
    }
    return vec4f(0,0,0,-1);
}
@fragment fn model(v:Vertex)->@location(0) vec4f {
    if u.state.x<1.0 || u.state.x==2.0 {discard;}
    let uv=(v.uv-.5)*vec2f(u.screen.x/u.screen.y,1)*2.0;
    let camera=vec3f(0,0,4.2);
    let direction=normalize(vec3f(uv.x,-uv.y,-3.8));
    var first=0u;var last=10u;
    if u.state.x>=4.0 {first=10u;last=11u;}
    if u.state.x==3.0 {first=11u;last=12u;}
    // Sort actual ray hits, so stars and more distant letters remain visible
    // through the translucent blocks even when their projections overlap.
    var distances:array<f32,10>;
    var surfaces:array<vec4f,10>;
    var count=0u;
    for(var id=first;id<last;id++) {
        if id<10u && u.state.y>=u.flights[id].x {continue;}
        let origin=local(camera,id);
        let ray=normalize(local_direction(direction,id));
        let hit=trace(origin,ray,id);
        if hit.w<0.0 {continue;}
        let n=normal(hit.xyz,id);
        let color=shade(hit.xyz,n,ray,id);
        if id==10u {return vec4f(color,1);}
        if id==11u {return vec4f(color,smoothstep(0.0,30.0,u.state.y));}
        let rim=pow(1.0-max(dot(n,-ray),0.0),3.0);
        let opacity=(.78+.16*rim)*smoothstep(0.0,30.0,u.state.y);
        let distance=hit.w*pose(id).w;
        var slot=count;
        loop {
            if slot==0u {break;}
            if distances[slot-1u]<=distance {break;}
            distances[slot]=distances[slot-1u];
            surfaces[slot]=surfaces[slot-1u];
            slot-=1u;
        }
        distances[slot]=distance;
        surfaces[slot]=vec4f(color,opacity);
        count+=1u;
    }
    var color=vec3f(0);var alpha=0.0;
    for(var i=0u;i<count;i++) {
        let layer=surfaces[i];
        let coverage=(1.0-alpha)*layer.a;
        color+=layer.rgb*coverage;
        alpha+=coverage;
    }
    if alpha<.0001 {discard;}
    return vec4f(color/alpha,alpha);
}
