struct Controls {
    screen:vec4f, state:vec4f, misc:vec4f,
    flights:array<vec4f,10>, plate_center:vec4f, plate_rotation:vec4f,
    transition:vec4f, tile_order:array<vec4f,40>, next_palette:array<vec4f,64>,
}
@group(0) @binding(0) var<uniform> u:Controls;
@group(0) @binding(1) var scene:texture_2d<f32>;
@group(0) @binding(2) var bloom:texture_2d<f32>;
@group(0) @binding(3) var reference:texture_2d<f32>;
@group(0) @binding(4) var linear:sampler;
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}
fn cut_hash(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
fn backdrop_color(p:vec2i)->vec3f {
    // Scroller::begin_transition repeats an 8 by 8 indexed pattern. Its
    // palette is supplied locally, never baked into this shader.
    let x=u32((p.x%8+8)%8);let y=u32(((p.y+6)%8+8)%8);
    return u.next_palette[y*8u+x].rgb;
}
fn next_backdrop(uv:vec2f)->vec3f {
    let q=uv*vec2f(320,200)-.5;
    let at=vec2i(floor(q));let f=fract(q);let s=f*f*(3.0-2.0*f);
    // The original has prepared its repeating pattern here, but its palette
    // is still flat during the pause before the scroller. Give that exposed
    // plane authored brushed-metal lighting without advancing the scene.
    let c=mix(mix(backdrop_color(at),backdrop_color(at+vec2i(1,0)),s.x),
              mix(backdrop_color(at+vec2i(0,1)),backdrop_color(at+vec2i(1,1)),s.x),s.y);
    let softbox=.65+.35*exp(-pow((uv.y-.28)*2.4,2.0));
    let tint=mix(vec3f(.78,.87,1.0),vec3f(1.0,.96,.90),uv.x*.35);
    let grain=.985+.015*cut_hash(floor(uv*vec2f(1400,6000)));
    return (.09+c*.085)*tint*softbox*grain;
}
@fragment fn tile_cut(v:Vertex)->@location(0) vec4f {
    let original=textureSampleLevel(scene,linear,v.uv,0).rgb;
    if u.transition.y<.5 || u.transition.x<=0.0 {return vec4f(original,1);}
    let grid=vec2f(16,10);
    let cell=vec2u(min(floor(v.uv*grid),grid-1.0));
    let index=cell.y*16u+cell.x;
    let rank=u.tile_order[index/4u][index%4u];
    let age=(u.transition.x-rank)/10.0;
    if age<=0.0 {return vec4f(original,1);}
    let local=fract(v.uv*grid)-.5;
    let progress=clamp(age,0.0,1.0);
    let half_size=.5*(1.0-progress);
    let d=half_size-max(abs(local.x),abs(local.y));
    let aa=max(grid.x/u.screen.x,grid.y/u.screen.y)*.7;
    let remains=1.0-smoothstep(.98,1.0,progress);
    let face=smoothstep(-aa,aa,d)*remains;
    let cut_life=smoothstep(0.0,.10,age)*(1.0-smoothstep(.95,1.08,age));
    let warm=smoothstep(.38,.76,v.uv.x+(.5-v.uv.y)*.12);
    let hot_edge=mix(vec3f(.28,.58,1.0),vec3f(4.2,.62,.025),warm);
    var under=next_backdrop(v.uv);
    // A small displaced silhouette supplies the lower cut wall. Its contact
    // shadow stays with the patch as the front recedes into the next plane.
    let wall_local=local-vec2f(.012,.023)*progress;
    let wall_d=half_size-max(abs(wall_local.x),abs(wall_local.y));
    let wall=smoothstep(-aa,aa,wall_d)*remains;
    let shadow=smoothstep(-.035,.015,wall_d)*remains;
    under*=1.0-shadow*.48;
    under=mix(under,vec3f(.018,.024,.032)+hot_edge*.045,wall);
    let sample_uv=v.uv+local/grid*(progress*progress*.075);
    var metal=textureSampleLevel(scene,linear,sample_uv,0).rgb;
    var edge_normal=vec2f(0,sign(local.y));
    if abs(local.x)>abs(local.y) {edge_normal=vec2f(sign(local.x),0);}
    let facing=max(dot(edge_normal,normalize(vec2f(-.5,-.8))),0.0);
    let bevel=(1.0-smoothstep(0.0,.035,d))*cut_life;
    metal*=1.0+bevel*(facing*.95-.55);
    metal+=vec3f(.12,.18,.25)*bevel*facing;
    var color=mix(under,metal,face);
    let rim=exp(-abs(d)/.0065)*cut_life;
    color+=hot_edge*rim;
    // Sparse beads just behind the moving cut cool quickly. They stay close
    // to the square edge instead of turning the wipe into flying debris.
    let speck=cut_hash(floor(v.uv*grid*34.0));
    let residue=smoothstep(.91,.99,speck)*exp(-abs(d+.022)/.019)
        *(1.0-smoothstep(-aa,aa,d))*cut_life;
    color+=hot_edge*residue*.24;
    return vec4f(color,1);
}
fn blur(uv:vec2f, direction:vec2f, threshold:bool)->vec3f {
    var sum=vec3f(0);var weight=0.0;
    for(var i=-10;i<=10;i++) {
        let w=exp(-f32(i*i)/32.0);
        var c=textureSampleLevel(scene,linear,uv+direction*f32(i),0).rgb;
        if threshold {c=max(c-vec3f(1.0),vec3f(0));}
        sum+=c*w;weight+=w;
    }
    return sum/weight;
}
@fragment fn blur_x(v:Vertex)->@location(0) vec4f {return vec4f(blur(v.uv,vec2f(3.0/u.screen.x,0),true),1);}
@fragment fn blur_y(v:Vertex)->@location(0) vec4f {return vec4f(blur(v.uv,vec2f(0,3.0/u.screen.y),false),1);}
@fragment fn composite(v:Vertex)->@location(0) vec4f {
    var c=textureSampleLevel(scene,linear,v.uv,0).rgb + textureSampleLevel(bloom,linear,v.uv,0).rgb*.14;
    // ACES approximation: preserve bright metal highlights without clipping.
    c=clamp((c*(2.51*c+.03))/(c*(2.43*c+.59)+.14),vec3f(0),vec3f(1));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if compare {
        let aspect=u.screen.x/u.screen.y;
        let p=(v.uv-.5)*vec2f(max(aspect/(4.0/3.0),1.0),max((4.0/3.0)/aspect,1.0))+.5;
        c=vec3f(0);
        if all(p>=vec2f(0)) && all(p<=vec2f(1)) {
            let rgb=textureLoad(reference,clamp(vec2i(p*vec2f(320,200)),vec2i(0),vec2i(319,199)),0).rgb;
            c=select(rgb/12.92,pow((rgb+.055)/1.055,vec3f(2.4)),rgb>vec3f(.04045));
        }
    }
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {c=vec3f(.25,.4,.65);}
    if u.misc.y<.5 {c=select(c*12.92,1.055*pow(c,vec3f(1.0/2.4))-.055,c>vec3f(.0031308));}
    return vec4f(c,1);
}
