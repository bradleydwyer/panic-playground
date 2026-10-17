// Authored scroller experiments. The two distance fields are derived at run
// time from locally rendered part-2 frames. They supply contours and motion,
// not original 3D models. Lighting, depth, coating and ribbons are new.
struct Controls {
    screen:vec4f, state:vec4f, misc:vec4f,
    flights:array<vec4f,10>, plate_center:vec4f, plate_rotation:vec4f,
    transition:vec4f, tile_order:array<vec4f,40>, next_palette:array<vec4f,64>,
    scroller:vec4f, scroll_light:vec4f,
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
fn bass()->f32 {return u.scroll_light.z*u.scroll_light.y;}
fn attack()->f32 {return u.scroll_light.w*u.scroll_light.y;}
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
fn ribbon_z(x:f32)->f32 {
    if u.scroller.z>.5 && u.scroller.z<1.5 {return 0.0;}
    // A legible central track, with the ends turning into the chamber.
    let edge=max(abs(x)/aspect()-.48,0.0)/.52;
    return -1.5*edge*edge;
}
fn extrusion(d:f32,z:f32,half_depth:f32,bevel:f32)->f32 {
    let q=vec2f(d+bevel,abs(z)-half_depth+bevel);
    return min(max(q.x,q.y),0.0)+length(max(q,vec2f(0)))-bevel;
}
fn shapes(p:vec3f)->vec2f {
    let f=field(uv_at(p.xy));
    let distance=-(f-vec2f(0,.20))*.0086;
    let letter=extrusion(distance.x,p.z-ribbon_z(p.x)+.075,.12,.018);
    // The original sine displacement is already in the field. Depth makes
    // that same overlay weave in front of and behind the larger lettering.
    let wave_z=.13+.24*sin(p.y*3.8-clock()*1.3);
    let filament=extrusion(distance.y,p.z-wave_z,.018,.009);
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
        let f=field(uv_at(p.xy+vec2f(.045,-.065)));
        // Soft projected contact shadows and rough reflected silhouettes.
        // These are analytic approximations, not traced indirect lighting.
        let shadow=smoothstep(-9.0,4.0,f.x);
        color*=1.0-shadow*.65;
        let reflected=exp(-abs(field(uv_at(p.xy+vec2f(-.035,.12))).x)*.20);
        color+=heat(p.x)*reflected*.022;
        color+=heat(p.x)*exp(-abs(f.y)*.23)*lighting(.0376,.012+musical()*.025+bass()*.14);
    }
    let haze=noise(p.xy*2.5+vec2f(clock()*.055,-clock()*.035));
    color+=heat(p.x)*pow(haze,3.0)*.022*entrance;
    return color;
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
        return heat(p.x)*lighting(1.136,.28+energy*.65+bass()*2.0+attack()*.6)*body
            +vec3f(.10,.12,.14)*(1.0-rim);
    }
    let face=abs(n0.z);
    let grain=noise(p.xy*vec2f(260,900));
    let n=normalize(n0+vec3f((grain-.5)*.025,0,0)*face);
    let rough=mix(.15,.30,smoothstep(.6,.98,face));
    let f0=mix(vec3f(.61,.70,.78),vec3f(.75,.62,.49),warm(p.x)*.3);
    let sweep=clock()*.23*u.scroll_light.y;
    var color=f0*.07;
    color+=lamp(p,n,eye,vec3f(-2.5,2.1,3.8),vec3f(.8,1.3,2.2)*lighting(1.098,.35+energy*.50+bass()*1.90),f0,rough);
    color+=lamp(p,n,eye,vec3f(2.5,-1.3,3.2),vec3f(2.4,.9,.30)*lighting(1.168,.30+energy*.55+bass()*2.6),f0,rough);
    color+=lamp(p,n,eye,vec3f(sin(sweep)*2.0,1.4,3.0),vec3f(1.0,1.1,1.2)*lighting(1.0,.22+attack()*2.8),f0,.36);
    let r=reflect(ray,n);
    let softbox=exp(-pow((r.y-.32)*4.8,2.0));
    let edgebox=exp(-pow((r.x+.6)*11.0,2.0));
    // Reflected sources must react too: a steady softbox previously dominated
    // the metal while the small direct-light changes disappeared in tonemapping.
    color+=f0*(softbox*.65+edgebox*.35)*lighting(1.0,.30+energy*.35+bass()*.90+attack()*.45);
    let fresnel=pow(1.0-max(dot(n,eye),0.0),4.0);
    let coating=mix(vec3f(.30,.55,.80),vec3f(.75,.40,.18),.5+.5*sin(dot(n,eye)*13.0+p.x));
    color+=coating*fresnel*.24;
    // A travelling filament lights the metal as it passes through its depth.
    let small=field(uv_at(p.xy)).y;
    color+=heat(p.x)*exp(-abs(small)*.16)*lighting(.234,.08+energy*.15+bass()*.60);
    if u.scroller.z>1.5 {
        // Experimental tinted solid glass: refracted chamber plus absorption,
        // a reflective coating and lit cut edges. Not a full multi-bounce trace.
        let refracted=normalize(ray+n*.085);
        let transmitted=chamber(refracted)*mix(vec3f(.55,.83,.96),vec3f(.96,.65,.36),warm(p.x));
        color=mix(transmitted*1.35,color,.065+fresnel*.75);
        let behind=field(uv_at(p.xy+n.xy*.025)).y;
        color+=heat(p.x)*smoothstep(-.5,.8,behind)*.55;
        color+=heat(p.x)*pow(1.0-face,2.0)*lighting(.462,.20+energy*.25+bass()*.85+attack()*.7);
    }
    return color;
}
@fragment fn scroller(v:Vertex)->@location(0) vec4f {
    let xy=(v.uv-.5)*vec2f(2.0*aspect(),-2.0);
    let ray=normalize(vec3f(xy,-5.5));
    var color=chamber(ray);
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
