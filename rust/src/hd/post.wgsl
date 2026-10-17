// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl.
@group(0) @binding(1) var scene:texture_2d<f32>;
@group(0) @binding(2) var bloom:texture_2d<f32>;
@group(0) @binding(3) var reference:texture_2d<f32>;
@group(0) @binding(4) var linear:sampler;
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
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
    // Bloom breathes with the music and swells briefly on hits (authored).
    // The intro's bloom follows the music; the scroller has its own lighting
    // modes, and its studio mode should stay steady.
    let glow=select(.14*(.80+.25*u.audio.x+.40*u.audio.y),.14,u.state.x>=8.0);
    var c=textureSampleLevel(scene,linear,v.uv,0).rgb + textureSampleLevel(bloom,linear,v.uv,0).rgb*glow;
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
