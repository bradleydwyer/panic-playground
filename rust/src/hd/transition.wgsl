// Authored 3D material treatment. Only the activation order/timing and the
// locally rendered source image come from the reconstruction.
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
fn falling()->bool {return u.transition.w>.5 && u.transition.w<1.5;}
fn laser()->bool {return u.transition.w>1.5;}
const LASER_CUT=.21;
const LASER_RELEASE=.32;
const LASER_LIFE=1.10;
const LASER_PATH=.9875;
fn tile_lifetime()->f32 {
    if laser() {return LASER_LIFE*7.0;}
    return select(1.0,3.6,falling());
}
fn heat_color(warm:f32)->vec3f {
    if falling() {return mix(vec3f(5.0,.95,.065),vec3f(8.0,1.65,.10),warm);}
    return mix(vec3f(1.2,2.1,3.8),vec3f(7.0,1.25,.045),warm);
}
fn project(p:vec3f)->vec4f {
    // Use a real perspective depth range for curled, overlapping sheets.
    let distance=5.5-p.z;let near=.08;let far=20.0;
    let z=(far*distance-far*near)/(far-near);
    return vec4f(p.x/(u.screen.x/u.screen.y),p.y,z/5.5,distance/5.5);
}
fn screen_uv(p:vec3f)->vec2f {let c=project(p);return vec2f(c.x,-c.y)/c.w*.5+.5;}
fn rotate(p:vec3f,a:vec2f)->vec3f {
    let r=vec3f(p.x,cos(a.x)*p.y-sin(a.x)*p.z,sin(a.x)*p.y+cos(a.x)*p.z);
    return vec3f(cos(a.y)*r.x+sin(a.y)*r.z,r.y,-sin(a.y)*r.x+cos(a.y)*r.z);
}
fn remaining(progress:f32)->f32 {
    if laser() {return 1.0;}
    return 1.0-progress;
}
// Move the tessellated boundary itself, so the thin cut walls remain joined
// to the ragged burn front. A fragment-only cut would leave those walls behind.
fn cut_coordinates(q:vec2f,cell:vec2f,progress:f32)->vec2f {
    if laser() {return q;}
    let life=smoothstep(0.0,.12,progress);
    let flow=noise(q*9.0+cell*3.0+vec2f(u.screen.z*2.0,-u.screen.z*1.5));
    let erosion=(.018+.060*flow)*life*pow(max(abs(q.x),abs(q.y)),3.0);
    return q*(1.0-erosion);
}
fn sheet(q:vec2f,cell:vec2f,progress:f32)->vec3f {
    let aspect=u.screen.x/u.screen.y;
    let envelope=smoothstep(0.0,.20,progress);
    let extent=.5*remaining(progress);
    let angle=hash(cell+12.0)*PI*2.0;
    let axis=vec2f(cos(angle),sin(angle));
    let flat=cut_coordinates(q,cell,progress)*extent/GRID*vec2f(aspect*2.0,-2.0);
    let along=dot(flat,axis);
    // Cylindrical, asymmetric curl: a thin sheet rolls from one edge, rather
    // than inflating all four corners like a rigid dish. No recovered physics.
    let bend=(.60+hash(cell+27.0)*1.2)*envelope;
    let curvature=max(bend/(.12*max(remaining(progress),.04)),.0001);
    let offset=.10*remaining(progress);
    let theta=(along+offset)*curvature;
    let rolled=sin(theta)/curvature-offset;
    var point=vec3f(flat+axis*(rolled-along),(1.0-cos(theta))/curvature);
    point.z+=sin(q.x*3.0+q.y*2.0+progress*6.0+hash(cell)*8.0)*.008*envelope;
    return point;
}
// A moving cutting point leaves a small tab on the top edge. The plate
// hangs from that tab, tips, then releases into an authored ballistic fall.
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
    var point=rotate(flat-pivot,vec2f(-.42*sag-free*(.7+hash(cell+64.0)),spin*(sag*.12+free*1.5)));
    let roll=spin*(sag*.16+free*.8);
    point=vec3f(point.x*cos(roll)-point.y*sin(roll),point.x*sin(roll)+point.y*cos(roll),point.z)+pivot;
    point.x+=(hash(cell+71.0)-.5)*free*.16;
    point.y-=.5*13.5*free*free;
    point.z-=free*.12;
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
    if laser() {return laser_surface(q,cell,progress*LASER_LIFE);}
    let aspect=u.screen.x/u.screen.y;
    let center=(cell+.5)/GRID;
    var origin=vec3f((center-.5)*vec2f(aspect*2.0,-2.0),0);
    var point=sheet(q,cell,progress);
    if falling() {
        let seconds=progress*tile_lifetime()/7.0;
        let release=smoothstep(0.0,.18,progress);
        let spin=(vec2f(hash(cell+3.0),hash(cell+7.0))-.5)*progress*2.7;
        point=rotate(point,spin*release);
        // Gravity dominates the motion. A little sideways drag and flutter
        // make the heat-thinned sheet fall like burning paper, not a rigid tile.
        let wind=(hash(cell+31.0)-.5)*.20;
        origin.x+=wind*seconds+sin(seconds*9.0+hash(cell)*6.0)*seconds*.035;
        origin.y-=.5*8.5*seconds*seconds;
        origin.z=.035*release+sin(seconds*7.0)*.018;
    } else {
        point=rotate(point,(vec2f(hash(cell+3.0),hash(cell+7.0))-.5)*sin(progress*PI)*.65);
        origin.z=sin(progress*PI)*.035-progress*progress*.10;
    }
    return origin+point;
}
// Embers and vapor start on the moving burn front, then carry their own
// momentum. Their emission origins follow the burn progress at birth.
fn emission(q:vec2f,cell:vec2f,birth:f32)->vec3f {
    return surface(q,cell,birth);
}
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}
@fragment fn backdrop(v:Vertex)->@location(0) vec4f {
    if u.transition.y<.5 || u.transition.x<0.0 {
        return textureSampleLevel(scene,linear,v.uv,0);
    }
    let xy=(v.uv-.5)*vec2f(u.screen.x/u.screen.y,1);
    let brush=noise(v.uv*vec2f(28,2800))*.65+noise(v.uv*vec2f(150,11000))*.35;
    var color=vec3f(.012,.020,.030)*(1.0+.12*brush);
    color+=vec3f(.012,.018,.024)*exp(-pow((xy.y+.18)*2.0,2.0));
    var illumination=vec3f(0);var occlusion=0.0;
    let current=vec2i(floor(v.uv*GRID));
    // Local irradiance and soft contact shadows follow the moving cut front.
    // This is an analytic lighting approximation, not a baked reflection map.
    for(var y=-5;y<=1;y++) {for(var x=-1;x<=1;x++) {
        if !falling() && y< -1 {continue;}
        let cell=current+vec2i(x,y);
        if any(cell<vec2i(0)) || any(cell>=vec2i(16,10)) {continue;}
        let id=u32(cell.y*16+cell.x);
        let age=(u.transition.x-order(id))/10.0;
        if age<=0.0 || age>6.0 {continue;}
        if laser() {
            let seconds=age/7.0;
            let head=laser_head(vec2f(cell),seconds);
            let local=v.uv*GRID-(vec2f(cell)+.5);
            let d=length((local-head*.5)*vec2f(1.1,1));
            illumination+=vec3f(.18,.38,.65)*exp(-d*9.0)*laser_power(seconds);
            continue;
        }
        let progress=clamp(age/tile_lifetime(),0.0,1.0);
        var q=v.uv*GRID-(vec2f(cell)+.5);
        if falling() {
            let seconds=progress*tile_lifetime()/7.0;
            q.y-=.5*8.5*seconds*seconds*GRID.y*.5;
        }
        let half_size=.5*(1.0-progress);
        let d=abs(max(abs(q.x),abs(q.y))-half_size);
        let life=smoothstep(0.0,.12,age)*exp(-max(age-1.0,0.0)*1.5);
        let flow=noise(q*4.0+vec2f(u.screen.z*1.6,-u.screen.z*2.0)+vec2f(cell));
        let pool=exp(-d*5.5)*life*(.75+.25*flow)*select(1.0,pow(1.0-progress,.65),falling());
        illumination+=heat_color(warmth((vec2f(cell)+.5)/GRID))*pool*.075;
        let shadow_q=q+vec2f(.10,-.16)*sin(progress*PI);
        let sd=max(abs(shadow_q.x),abs(shadow_q.y))-half_size;
        occlusion+=exp(-abs(sd)*12.0)*(1.0-progress)*life*.38;
    }}
    color*=1.0-clamp(occlusion,0.0,.8);
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
        point.z-=corner.y*select(.0025,.008,laser());
    }
    out.position=project(point);
    out.world=point;out.normal=normal;out.q=q;out.progress=p;out.id=id;
    out.source_uv=(cell+.5+cut_coordinates(q,cell,p)*.5*remaining(p))/GRID;
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
    metal+=mix(vec3f(4.2,.48,.025),vec3f(7.0,1.0,.045),warm)*rim;
    metal+=vec3f(.45,.08,.01)*exp(-distance*24.0)*cut*cooling;
    let delta=(v.q-laser_head(cell,seconds))*vec2f((u.screen.x/u.screen.y)*GRID.y/GRID.x,1);
    let d2=dot(delta,delta);
    let power=laser_power(seconds);
    metal+=(vec3f(22,30,38)*exp(-d2/.0007)+vec3f(.35,.85,1.5)*exp(-d2/.012))*power;
    // A fine flare makes the moving white-blue cutting point readable.
    metal+=vec3f(.25,.60,1.0)*exp(-abs(delta.x)*14.0-abs(delta.y)*100.0)*power;
    return metal;
}
@fragment fn tile_fragment(v:TileVertex)->@location(0) vec4f {
    let original=textureSampleLevel(scene,linear,v.source_uv,0).rgb;
    if v.progress<=0.0 {return vec4f(original,1);}
    if laser() {return vec4f(laser_material(v,original),1);}
    let life=smoothstep(0.0,.12,v.progress);
    var n=normalize(v.normal);let eye=normalize(vec3f(0,0,5.5)-v.world);
    let back=dot(n,eye)<0.0;
    if back {n=-n;}
    let light=normalize(vec3f(-.7,1.2,1.6));
    let reflected=reflect(-eye,n);
    let key=pow(max(dot(n,normalize(light+eye)),0.0),52.0);
    let strip=exp(-pow((reflected.y-.45)*7.0,2.0));
    let rim=pow(1.0-max(dot(n,eye),0.0),3.0);
    let warm=warmth(v.source_uv);
    let f0=mix(vec3f(.68,.79,.93),vec3f(.80,.44,.20),warm*.7);
    var metal=original*(.30+.75*max(dot(n,light),0.0));
    metal+=f0*(key*2.2+strip*.85+rim*.40);
    if back {metal=metal*.25+vec3f(.014,.010,.007);}
    let edge=max(abs(v.q.x),abs(v.q.y));
    let flow=noise(v.q*9.0+vec2f(u.screen.z*2.0,-u.screen.z*1.5));
    // The dark scorched band sits just inside the incandescent, irregular lip.
    let charred=smoothstep(.81,.965,edge)*life;
    metal=mix(metal,vec3f(.014,.008,.005)+metal*.06,charred);
    let hot=exp(-pow((edge-.986)*48.0,2.0))*(.55+.45*flow)*life;
    if v.side==1u {metal=f0*.045;}
    metal+=heat_color(warm)*hot*1.8;
    metal+=vec3f(3.0,2.0,.9)*pow(hot,4.0);
    return vec4f(mix(original,metal,life),1);
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
@vertex fn spark_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) instance:u32)->Particle {
    var out:Particle;out.position=vec4f(2,2,2,1);
    let id=instance/48u;let n=f32(instance%48u);let cell=cell_of(id);
    let seed=vec2f(f32(instance)+1.0,n+7.0);
    let birth=(n+hash(seed)) / 48.0;
    var birth_seconds=birth*tile_lifetime()/7.0;
    if laser() {
        if n>=20.0 {return out;}
        birth_seconds=(n+hash(seed))/14.0*LASER_CUT;
        if n>=14.0 {birth_seconds=LASER_RELEASE+(n-14.0)*.003;}
    }
    var seconds=(u.transition.x-order(id)-birth*10.0*tile_lifetime())/70.0;
    if laser() {seconds=(u.transition.x-order(id))/70.0-birth_seconds;}
    let lifetime=select(.23+hash(seed+3.0)*.43,.11+hash(seed+3.0)*.19,laser());
    if seconds<0.0 || seconds>lifetime {return out;}
    var q=perimeter(hash(seed+13.0));
    var start=emission(q,cell,birth);
    if laser() {
        q=laser_head(cell,birth_seconds);
        start=laser_surface(q,cell,birth_seconds)+vec3f(0,0,.004);
    }
    let direction=normalize(vec3f(q.x,-q.y,.4+hash(seed+4.0)));
    var velocity=direction*(.20+hash(seed+9.0)*.60);
    if falling() {velocity.y-=8.5*birth*tile_lifetime()/7.0*.45;}
    let drag=(1.0-exp(-seconds*3.0))/3.0;
    var point=start+velocity*drag;
    point.y-=seconds*seconds*1.5;
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
    out.color=mix(heat_color(warm),vec3f(2.5,.25,.01),warm*seconds/lifetime)*fade;
    if laser() {out.color=mix(vec3f(6.0,2.0,.40),vec3f(2.0,.20,.01),seconds/lifetime)*fade;}
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
    var birth_seconds=birth*tile_lifetime()/7.0;
    if laser() {birth_seconds=birth*LASER_CUT;}
    var seconds=(u.transition.x-order(id)-birth*10.0*tile_lifetime())/70.0;
    if laser() {seconds=(u.transition.x-order(id))/70.0-birth_seconds;}
    if seconds<0.0 || seconds>select(.60,.28,laser()) {return out;}
    let q=perimeter(hash(seed));
    var point=emission(q,cell,birth);
    if laser() {point=laser_surface(laser_head(cell,birth_seconds),cell,birth_seconds);}

    point+=vec3f(sin(seconds*5.0+hash(seed+2.0)*6.0)*seconds*.12,seconds*.18,seconds*.1+.04);
    let corners=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let corner=corners[vertex];
    let clip=project(point);let radius=(.018+seconds*.16)*vec2f(.8,1.35);
    out.position=clip+vec4f(corner*radius*vec2f(u.screen.y/u.screen.x,1),0,0);
    out.local=corner;
    let warm=warmth((cell+.5)/GRID);
    out.color=mix(vec3f(.10,.15,.20),vec3f(.20,.11,.055),warm);
    out.shape=vec4f(seconds,hash(seed+9.0)*50.0,smoothstep(0.0,.10,seconds)*(1.0-smoothstep(.20,.60,seconds)),0);
    return out;
}
@fragment fn vapor_fragment(v:Particle)->@location(0) vec4f {
    let p=v.local;let t=v.shape.x;
    let flow=vec2f(sin(p.y*4.0+t*5.0)*.3,-t*2.4);
    let wisps=noise(p*3.0+flow+v.shape.y)*.65+noise(p*7.0+flow*1.7+v.shape.y)*.35;
    let density=exp(-dot(p*vec2f(1.15,.85),p*vec2f(1.15,.85))*3.5)*smoothstep(.25,.8,wisps);
    return vec4f(v.color,density*v.shape.z*select(.20,.085,laser()));
}
