// All artwork fields are derived locally. No original logo pixels are embedded.
struct Controls {
    screen: vec4f, // drawable width/height, audio-clock seconds, comparison mode
    state: vec4f,  // reference phase/tick/door, temporal shape interpolation
    misc: vec4f,   // composition aspect, sRGB output, logo reveal time, modern style
}
@group(0) @binding(0) var<uniform> u: Controls;
@group(0) @binding(1) var current: texture_2d<f32>;
@group(0) @binding(2) var following: texture_2d<f32>;
@group(0) @binding(3) var reference: texture_2d<f32>;
@group(0) @binding(4) var linear: sampler;

struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*0.5+0.5,0.5-p.y*0.5));
}
fn art_uv(uv:vec2f)->vec2f {
    let screen_aspect=u.screen.x/u.screen.y;
    let fit=vec2f(max(screen_aspect/u.misc.x,1.0),max(u.misc.x/screen_aspect,1.0));
    return (uv-0.5)*fit+0.5;
}
fn output(rgb:vec3f)->vec3f {
    let c=clamp(rgb,vec3f(0),vec3f(1));
    let linear_color=select(c/12.92,pow((c+0.055)/1.055,vec3f(2.4)),c>vec3f(0.04045));
    return select(c,linear_color,u.misc.y>0.5);
}
fn field(p:vec2f)->vec4f {
    return mix(textureSampleLevel(current,linear,p,0),textureSampleLevel(following,linear,p,0),u.state.w);
}
fn hash(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
fn noise(p:vec2f)->f32 {
    let i=floor(p);let f=fract(p);let s=f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2f(1,0)),s.x),mix(hash(i+vec2f(0,1)),hash(i+1.0),s.x),s.y);
}
fn metal_background(p:vec2f)->vec3f {
    let distance=abs(p.y-0.5);
    var grey=0.80-0.32*smoothstep(0.08,0.5,distance);
    grey+=0.006*sin(p.y*2400.0);
    let rail=abs(distance-0.219);
    let aa=max(fwidth(p.y),0.0001);
    var c=vec3f(grey);
    c=mix(c,vec3f(0.12,0.12,0.32),1.0-smoothstep(0.0015,0.0015+aa,rail));
    c+=vec3f(0.13)*(1.0-smoothstep(0.0008,0.0008+aa,abs(distance-0.225)));
    return c;
}
// The palette maths in Intro::doors, evaluated independently of the scanout.
// Sampling an edge pixel here would stretch passing stars into horizontal lines.
fn door_light(uv:vec2f)->f32 {
    if u.state.z>=200.0 {return 0.0;}
    let row=floor(clamp(uv.y*200.0,0.0,199.0));
    let i=floor(abs(row-99.5));
    if i<floor(u.state.z/2.0) {return 0.0;}
    let rim=floor(u.state.z*8.0/14.0);
    if i<rim {return 26.0/63.0;}
    if i==rim {return 40.0/63.0;}
    let gradient=i-rim-1.0;
    return (45.0-floor((floor(gradient/5.0)+(gradient%2.0))/2.0))/63.0;
}
@fragment fn background(v:Vertex)->@location(0) vec4f {
    let p=art_uv(v.uv);
    var c=vec3f(0);
    if u.state.x>=4.0 {
        c=metal_background(p);
        if u.misc.w<0.5 {c=textureSampleLevel(reference,linear,vec2f(0.0015625,clamp(p.y,0.0,1.0)),0).rgb;}
    }
    if u.state.x==3.0 {
        c=vec3f(door_light(p));
    }
    return vec4f(output(c),1);
}

struct StarVertex {
    @builtin(position) position:vec4f,
    @location(0) corner:vec2f,
    @location(1) intensity:f32,
    @location(2) uv:vec2f,
}
@vertex fn star_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->StarVertex {
    let corners=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let seed=f32(instance)+1.0;
    let z=1.0-fract(hash(vec2f(seed,7))*0.98+u.screen.z*0.15);
    let xy=vec2f(hash(vec2f(seed,13)),hash(vec2f(seed,29)))*2.0-1.0;
    let pos=xy/max(z,0.015)*vec2f(0.75,1.0);
    let size=(0.7+pow(1.0-z,3.0)*1.8)*u.screen.y/1080.0;
    let offset=corners[vertex]*size*2.0/u.screen.xy;
    let screen=pos+offset;
    return StarVertex(vec4f(screen,0,1),corners[vertex],pow(1.0-z,1.6),vec2f(screen.x*0.5+0.5,0.5-screen.y*0.5));
}
@fragment fn star_fragment(v:StarVertex)->@location(0) vec4f {
    if u.state.x>=4.0 {discard;}
    let p=art_uv(v.uv);
    if u.state.x==3.0 {
        let cover=door_light(p);
        if cover>0.1 {discard;}
    }
    let radius=length(v.corner);
    let alpha=exp(-radius*radius*3.0)*v.intensity;
    return vec4f(output(vec3f(0.56,0.65,1.0)),alpha);
}

@fragment fn foreground(v:Vertex)->@location(0) vec4f {
    let p=art_uv(v.uv);
    let inside=all(p>=vec2f(0)) && all(p<=vec2f(1));
    let compare=u.screen.w>0.5 && (u.screen.w<1.5 || v.uv.x<0.5);
    if compare || u.state.x>=6.0 {
        if !inside {return vec4f(0,0,0,1);}
        let ip=clamp(vec2i(p*vec2f(320,200)),vec2i(0),vec2i(319,199));
        return vec4f(output(textureLoad(reference,ip,0).rgb),1);
    }
    if u.screen.w>1.5 && abs(v.uv.x-0.5)<1.0/u.screen.x {return vec4f(0.5,0.6,0.8,1);}
    if !inside || u.state.x<1.0 || u.state.x==2.0 {discard;}
    let f=field(p);let d=f.r;
    let aa=max(fwidth(d)*0.65,0.035);
    let alpha=smoothstep(-aa,aa,d);
    if alpha<0.001 {discard;}
    let dx=vec2f(1.0/320.0,0);let dy=vec2f(0,1.0/200.0);
    let grad=vec2f(field(p+dx).r-field(p-dx).r,field(p+dy).r-field(p-dy).r);
    let bevel=1.0-smoothstep(0.0,select(1.25,3.5,u.misc.w>0.5),d);
    let normal=normalize(vec3f(-grad*bevel*1.3,1.0));
    let light=normalize(vec3f(-0.45,-0.6,1.0));
    let diffuse=max(dot(normal,light),0.0);
    if u.state.x<4.0 {
        // The prerecording supplies the pose and flat face brightness. The
        // continuous field supplies native-resolution edge coverage and bevels.
        let grey=clamp(f.b*0.8+0.14,0.0,1.0)*(0.65+diffuse*0.35);
        return vec4f(output(vec3f(grey)),alpha);
    }
    let original=textureSampleLevel(reference,linear,p,0).rgb;
    let edge_light=0.88+0.12*diffuse;
    return vec4f(output(original*edge_light),alpha);
}
