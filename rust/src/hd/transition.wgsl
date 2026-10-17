// Authored laser-cut tile transition. Only the activation order/timing and
// the locally rendered source image come from the reconstruction; the cut,
// release, fall, light, sparks and smoke are authored effects.
struct Controls {
    screen:vec4f, state:vec4f, misc:vec4f,
    flights:array<vec4f,10>, plate_center:vec4f, plate_rotation:vec4f,
    transition:vec4f, tile_order:array<vec4f,40>, next_palette:array<vec4f,64>,
}
@group(0) @binding(0) var<uniform> u:Controls;
@group(0) @binding(1) var scene:texture_2d<f32>;
@group(0) @binding(4) var linear:sampler;
const GRID=vec2f(16,10);
const PI=3.14159265;
fn hash(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
fn noise(p:vec2f)->f32 {
    let i=floor(p);let f=fract(p);let s=f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2f(1,0)),s.x),mix(hash(i+vec2f(0,1)),hash(i+1.0),s.x),s.y);
}
fn order(id:u32)->f32 {return u.tile_order[id/4u][id%4u];}
fn cell_of(id:u32)->vec2f {return vec2f(f32(id%16u),f32(id/16u));}
fn warmth(uv:vec2f)->f32 {return smoothstep(.36,.77,uv.x+(.5-uv.y)*.12);}
const LASER_CUT=.21;
const LASER_RELEASE=.32;
const LASER_LIFE=1.10;
const LASER_PATH=.9875;
fn tile_lifetime()->f32 {return LASER_LIFE*7.0;}
fn heat_color(warm:f32)->vec3f {
    return mix(vec3f(1.2,2.1,3.8),vec3f(7.0,1.25,.045),warm);
}
fn project(p:vec3f)->vec4f {
    // A real perspective depth range for plates falling away from the camera.
    let distance=5.5-p.z;let near=.08;let far=60.0;
    let z=(far*distance-far*near)/(far-near);
    return vec4f(p.x/(u.screen.x/u.screen.y),p.y,z/5.5,distance/5.5);
}
fn screen_uv(p:vec3f)->vec2f {let c=project(p);return vec2f(c.x,-c.y)/c.w*.5+.5;}
fn rotate(p:vec3f,a:vec2f)->vec3f {
    let r=vec3f(p.x,cos(a.x)*p.y-sin(a.x)*p.z,sin(a.x)*p.y+cos(a.x)*p.z);
    return vec3f(cos(a.y)*r.x+sin(a.y)*r.z,r.y,-sin(a.y)*r.x+cos(a.y)*r.z);
}
// A moving cutting point leaves a small tab on the top edge. The plate
// sags at that tab, then falls along -Z: gravity points away from the camera,
// as though the image were a floor seen from above. This motion is authored.
fn tab_x(cell:vec2f)->f32 {return (hash(cell+51.0)-.5)*1.10;}
fn laser_start(cell:vec2f)->f32 {return (tab_x(cell)+1.0)/8.0+(1.0-LASER_PATH)*.5;}
fn laser_head(cell:vec2f,seconds:f32)->vec2f {
    if seconds>LASER_CUT {return vec2f(tab_x(cell),-1);}
    return perimeter(fract(laser_start(cell)+LASER_PATH*clamp(seconds/LASER_CUT,0.0,1.0)));
}
fn laser_power(seconds:f32)->f32 {
    if seconds>=0.0 && seconds<=LASER_CUT {return smoothstep(0.0,.012,seconds);}
    return exp(-pow((seconds-LASER_RELEASE)/.018,2.0))*1.5;
}
fn laser_surface(q:vec2f,cell:vec2f,seconds:f32)->vec3f {
    let aspect=u.screen.x/u.screen.y;
    let half_extent=vec2f(aspect/GRID.x,1.0/GRID.y);
    let flat=vec3f(q*half_extent*vec2f(1,-1),0);
    let origin=vec3f(((cell+.5)/GRID-.5)*vec2f(aspect*2.0,-2.0),0);
    if seconds<=LASER_CUT {return flat+origin;}
    let sag=smoothstep(LASER_CUT,LASER_RELEASE,seconds);
    let free=max(seconds-LASER_RELEASE,0.0);
    let pivot=vec3f(tab_x(cell)*half_extent.x,half_extent.y,0);
    let spin=hash(cell+61.0)-.5;
    let hang=vec2f(.26*sag,spin*sag*.10);
    let center=rotate(-pivot,hang)+pivot;
    var point=rotate(rotate(flat,hang),vec2f(free*(.25+hash(cell+64.0)*.40),free*spin*.60));
    let roll=spin*free*.35;
    point=vec3f(point.x*cos(roll)-point.y*sin(roll),point.x*sin(roll)+point.y*cos(roll),point.z)+center;
    // Once the tab releases, the centre has no lateral velocity. Perspective
    // alone draws the accelerating plates towards the vanishing point.
    point.z-=.5*100.0*free*free;
    return point+origin;
}
fn edge_parameter(q:vec2f)->f32 {
    if abs(q.y)>=abs(q.x) {
        if q.y<0.0 {return (q.x+1.0)/8.0;}
        return .5+(1.0-q.x)/8.0;
    }
    if q.x>0.0 {return .25+(q.y+1.0)/8.0;}
    return .75+(1.0-q.y)/8.0;
}
fn surface(q:vec2f,cell:vec2f,progress:f32)->vec3f {
    return laser_surface(q,cell,progress*LASER_LIFE);
}
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
fn base_backdrop(uv:vec2f)->vec3f {
    let xy=(uv-.5)*vec2f(u.screen.x/u.screen.y,1);
    let brush=noise(uv*vec2f(28,2800))*.65+noise(uv*vec2f(150,11000))*.35;
    var color=vec3f(.012,.020,.030)*(1.0+.12*brush);
    color+=vec3f(.012,.018,.024)*exp(-pow((xy.y+.18)*2.0,2.0));
    return color;
}
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}
@fragment fn backdrop(v:Vertex)->@location(0) vec4f {
    if u.transition.y<.5 || u.transition.x<0.0 {
        return textureSampleLevel(scene,linear,v.uv,0);
    }
    var color=base_backdrop(v.uv);
    var illumination=vec3f(0);
    let current=vec2i(floor(v.uv*GRID));
    // Local irradiance follows the moving cut front. This is an analytic
    // lighting approximation, not a baked reflection map.
    for(var y=-1;y<=1;y++) {for(var x=-1;x<=1;x++) {
        let cell=current+vec2i(x,y);
        if any(cell<vec2i(0)) || any(cell>=vec2i(16,10)) {continue;}
        let id=u32(cell.y*16+cell.x);
        let age=(u.transition.x-order(id))/10.0;
        if age<=0.0 || age>6.0 {continue;}
        let seconds=age/7.0;
        let head=laser_head(vec2f(cell),seconds);
        let local=v.uv*GRID-(vec2f(cell)+.5);
        let d=length((local-head*.5)*vec2f(1.1,1));
        let heat=heat_color(warmth((vec2f(cell)+.5)/GRID));
        illumination+=heat*exp(-d*7.0)*laser_power(seconds)*.20;
        let seam=abs(max(abs(local.x),abs(local.y))-.5);
        let glow=smoothstep(0.0,LASER_CUT,seconds)*exp(-max(seconds-LASER_CUT,0.0)*5.0);
        illumination+=heat*exp(-seam*6.5)*glow*.12;
    }}
    color+=illumination;
    // Moving glints in the brushed underlayer, illuminated only by nearby cuts.
    let shimmer=noise(v.uv*vec2f(65,180)+vec2f(u.screen.z*.12,0));
    color+=illumination*pow(shimmer,7.0)*1.8;
    return vec4f(color,1);
}
struct TileVertex {
    @builtin(position) position:vec4f,
    @location(0) source_uv:vec2f,
    @location(1) world:vec3f,
    @location(2) normal:vec3f,
    @location(3) q:vec2f,
    @location(4) @interpolate(flat) side:u32,
    @location(5) progress:f32,
    @location(6) @interpolate(flat) id:u32,
}
@vertex fn tile_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) id:u32)->TileVertex {
    var out:TileVertex;
    out.position=vec4f(2,2,2,1);
    let age=(u.transition.x-order(id))/10.0;
    if age>=tile_lifetime() {return out;}
    let p=clamp(age/tile_lifetime(),0.0,1.0);
    let corners=array<vec2f,6>(vec2f(0,0),vec2f(1,0),vec2f(0,1),vec2f(0,1),vec2f(1,0),vec2f(1,1));
    let corner=corners[vertex%6u];
    let cell=cell_of(id);
    var q=vec2f(0);var along=vec2f(0);
    if vertex<2400u {
        let quad=vertex/6u;
        q=(vec2f(f32(quad%20u),f32(quad/20u))+corner)*.1-1.0;
    } else {
        if p<=0.0 {return out;}
        let wall=vertex-2400u;let edge=wall/120u;
        let t=(f32((wall/6u)%20u)+corner.x)*.1-1.0;
        if edge==0u {q=vec2f(t,-1);along=vec2f(1,0);}
        if edge==1u {q=vec2f(1,t);along=vec2f(0,1);}
        if edge==2u {q=vec2f(-t,1);along=vec2f(-1,0);}
        if edge==3u {q=vec2f(-1,-t);along=vec2f(0,-1);}
        out.side=1u;
    }
    var point=surface(q,cell,p);
    let dx=surface(q+vec2f(.002,0),cell,p)-surface(q-vec2f(.002,0),cell,p);
    let dy=surface(q+vec2f(0,.002),cell,p)-surface(q-vec2f(0,.002),cell,p);
    var normal=normalize(cross(dy,dx));
    if out.side==1u {
        let tangent=surface(q+along*.002,cell,p)-surface(q-along*.002,cell,p);
        normal=normalize(cross(tangent,vec3f(0,0,-1)));
        point.z-=corner.y*.008;
    }
    out.position=project(point);
    out.world=point;out.normal=normal;out.q=q;out.progress=p;out.id=id;
    out.source_uv=(cell+.5+q*.5)/GRID;
    return out;
}
fn laser_material(v:TileVertex,original:vec3f)->vec3f {
    let seconds=v.progress*LASER_LIFE;
    let cell=cell_of(v.id);
    let n0=normalize(v.normal);
    let eye=normalize(vec3f(0,0,5.5)-v.world);
    let back=dot(n0,eye)<0.0;
    let n=select(n0,-n0,back);
    let light=normalize(vec3f(-.6,1.1,1.8));
    let key=pow(max(dot(n,normalize(light+eye)),0.0),66.0);
    let reflected=reflect(-eye,n);
    let strip=exp(-pow((reflected.y-.45)*9.0,2.0));
    let warm=warmth(v.source_uv);
    let metal_tint=mix(vec3f(.55,.72,.90),vec3f(.80,.48,.24),warm*.6);
    let tipped=smoothstep(LASER_CUT,LASER_RELEASE,seconds);
    var metal=original*mix(1.0,.40+.65*max(dot(n,light),0.0),tipped);
    metal+=metal_tint*(key*1.1+strip*.40)*tipped;
    if back {metal=vec3f(.018,.023,.030)+metal_tint*(key*.24+strip*.055);}
    if v.side==1u {metal=metal_tint*(.045+key*.25);}
    let position=fract(edge_parameter(v.q)-laser_start(cell)+1.0);
    let travel=LASER_PATH*clamp(seconds/LASER_CUT,0.0,1.0);
    let severed=seconds>=LASER_RELEASE;
    let cut=select(0.0,1.0,(position<=travel && seconds>0.0) || severed);
    var cut_at=position/LASER_PATH*LASER_CUT;
    if position>LASER_PATH {cut_at=LASER_RELEASE;}
    let cooling=exp(-max(seconds-cut_at,0.0)*4.0);
    let edge=max(abs(v.q.x),abs(v.q.y));
    let distance=max(1.0-edge,0.0);
    let kerf=exp(-pow(distance/.014,2.0))*cut;
    metal=mix(metal,vec3f(.008,.006,.005),kerf*.88);
    let rim=exp(-pow(distance/.020,2.0))*cut*cooling;
    let heat=heat_color(warm);
    metal+=heat*rim*1.8;
    metal+=heat*.12*exp(-distance*24.0)*cut*cooling;
    let delta=(v.q-laser_head(cell,seconds))*vec2f((u.screen.x/u.screen.y)*GRID.y/GRID.x,1);
    let d2=dot(delta,delta);
    let power=laser_power(seconds);
    let core=mix(vec3f(22,30,38),vec3f(38,25,16),warm);
    metal+=(core*exp(-d2/.0007)+heat*.35*exp(-d2/.012))*power;
    // A fine flare makes the moving white-blue cutting point readable.
    metal+=heat*.18*exp(-abs(delta.x)*14.0-abs(delta.y)*100.0)*power;
    let haze=1.0-exp(-max(-v.world.z,0.0)*.18);
    // Fade fully into the distant plane before lifetime culling. A depth-only
    // fall stays in frame, unlike the earlier downward-screen trajectory.
    let fade=max(haze,smoothstep(.88,LASER_LIFE-.02,seconds));
    return mix(metal,base_backdrop(screen_uv(v.world)),fade);
}
@fragment fn tile_fragment(v:TileVertex)->@location(0) vec4f {
    let original=textureSampleLevel(scene,linear,v.source_uv,0).rgb;
    if v.progress<=0.0 {return vec4f(original,1);}
    return vec4f(laser_material(v,original),1);
}
struct Particle {
    @builtin(position) position:vec4f,
    @location(0) local:vec2f,
    @location(1) color:vec3f,
    @location(2) shape:vec4f,
}
fn perimeter(t:f32)->vec2f {
    let side=u32(t*4.0);let s=fract(t*4.0)*2.0-1.0;
    if side==0u {return vec2f(s,-1);}
    if side==1u {return vec2f(1,s);}
    if side==2u {return vec2f(-s,1);}
    return vec2f(-1,-s);
}
// Twenty sparks per tile: fourteen along the cut, six where the tab releases.
// Seeds keep the earlier 48-slot numbering, so the pattern is unchanged.
const SPARKS=20u;
@vertex fn spark_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->Particle {
    var out:Particle;out.position=vec4f(2,2,2,1);
    let id=instance/SPARKS;let slot=instance%SPARKS;let n=f32(slot);let cell=cell_of(id);
    let seed=vec2f(f32(id*48u+slot)+1.0,n+7.0);
    var birth_seconds=(n+hash(seed))/14.0*LASER_CUT;
    if n>=14.0 {birth_seconds=LASER_RELEASE+(n-14.0)*.003;}
    let seconds=(u.transition.x-order(id))/70.0-birth_seconds;
    let lifetime=.11+hash(seed+3.0)*.19;
    if seconds<0.0 || seconds>lifetime {return out;}
    let q=laser_head(cell,birth_seconds);
    let start=laser_surface(q,cell,birth_seconds)+vec3f(0,0,.004);
    let direction=normalize(vec3f(q.x,-q.y,.4+hash(seed+4.0)));
    let velocity=direction*(.20+hash(seed+9.0)*.60);
    let drag=(1.0-exp(-seconds*3.0))/3.0;
    var point=start+velocity*drag;
    point.z-=seconds*seconds*10.0;
    let clip=project(point);let previous=screen_uv(point-velocity/100.0);
    let delta=(screen_uv(point)-previous)*u.screen.xy;
    let tangent=normalize(delta+vec2f(.00001,0));let perpendicular=vec2f(-tangent.y,tangent.x);
    let radius=(.35+pow(hash(seed+8.0),4.0)*1.4)*u.screen.y/1080.0;
    let tail=clamp(length(delta),radius,20.0*u.screen.y/1080.0);
    let corners=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let corner=corners[vertex];
    let pixel=vec2f(mix(-tail-radius*2.5,radius*2.5,corner.x*.5+.5),corner.y*radius*2.5);
    let offset=(tangent*pixel.x+perpendicular*pixel.y)*vec2f(2,-2)/u.screen.xy;
    out.position=clip+vec4f(offset*clip.w,0,0);
    out.local=pixel/radius;
    let fade=pow(1.0-seconds/lifetime,1.7)*smoothstep(0.0,.012,seconds);
    let warm=warmth((cell+.5)/GRID);
    out.color=heat_color(warm)*mix(1.5,.35,seconds/lifetime)*fade;
    out.shape=vec4f(tail/radius,hash(seed+15.0),0,0);
    return out;
}
@fragment fn spark_fragment(v:Particle)->@location(0) vec4f {
    let p=v.local;let core=exp(-dot(p,p)*1.7);
    let trail=exp(-p.y*p.y*3.0)*smoothstep(-v.shape.x,0.0,p.x)*(1.0-smoothstep(0.0,.6,p.x));
    let halo=exp(-dot(p,p)*.25)*.09;
    return vec4f(v.color*(core*1.4+trail*.45+halo),1);
}
@vertex fn vapor_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->Particle {
    var out:Particle;out.position=vec4f(2,2,2,1);
    let id=instance/6u;let seed=vec2f(f32(instance)+4.0,17);
    let cell=cell_of(id);let birth=(f32(instance%6u)+.5)/6.0;
    let birth_seconds=birth*LASER_CUT;
    let seconds=(u.transition.x-order(id))/70.0-birth_seconds;
    if seconds<0.0 || seconds>.60 {return out;}
    var point=laser_surface(laser_head(cell,birth_seconds),cell,birth_seconds);
    let angle=hash(seed+2.0)*PI*2.0+seconds*2.0;
    // In the overhead view the hot plume rises towards the camera.
    point+=vec3f(vec2f(cos(angle),sin(angle))*seconds*.09,seconds*.32+.04);
    let corners=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let corner=corners[vertex];
    let clip=project(point);let radius=(.018+seconds*.16)*vec2f(.8,1.35);
    out.position=clip+vec4f(corner*radius*vec2f(u.screen.y/u.screen.x,1),0,0);
    out.local=corner;
    let warm=warmth((cell+.5)/GRID);
    out.color=mix(vec3f(.18,.38,.65),vec3f(.72,.27,.075),warm);
    out.shape=vec4f(seconds,hash(seed+9.0)*50.0,smoothstep(0.0,.10,seconds)*(1.0-smoothstep(.20,.60,seconds)),0);
    return out;
}
@fragment fn vapor_fragment(v:Particle)->@location(0) vec4f {
    let p=v.local;let t=v.shape.x;
    let flow=vec2f(sin(p.y*4.0+t*5.0)*.3,-t*2.4);
    let wisps=noise(p*3.0+flow+v.shape.y)*.65+noise(p*7.0+flow*1.7+v.shape.y)*.35;
    let density=exp(-dot(p*vec2f(1.15,.85),p*vec2f(1.15,.85))*3.5)*smoothstep(.25,.8,wisps);
    return vec4f(v.color,density*v.shape.z*.32);
}
