// Faithful HD for part 8 (M, --faithful): the original's look at display
// resolution, in the original's 4:3 frame.
// - Every pixel's colour index is the original's on every frame (the
//   monster, the white, the squeezed picture, the line and the dot), the
//   palette moving between frames as the original's changes (the lift
//   toward white, the dot's pulse).
// - The pixels are upscaled smoothly (Catmull-Rom), as part 7's monster is.
//   Part 8's first frame is part 7's last, which part 7 draws.
// Not reproduced: the original's square pixels.
// Module: the shared controls, ending_common.wgsl and this file.

fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(srgb_encode(x),x,u.misc.y>.5);
}

// The shown frame's colour at pixel p, clamped to the picture.
fn pixel_colour(p:vec2i)->vec3f {
    let q=clamp(p,vec2i(0),vec2i(319,199));
    let i=u32(ec.frame.w)*64000u+u32(q.y)*320u+u32(q.x);
    return entry((images[i>>2u]>>(8u*(i&3u)))&255u,true);
}

@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    ending_begin();
    let s=source_of_scene(scene_of_uv(v.uv));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !in_frame(s) {return vec4f(0,0,0,1);}
    if compare {
        let ip=clamp(vec2i(s),vec2i(0),vec2i(319,199));
        return vec4f(encode(srgb_decode(textureLoad(original,ip,0).rgb)),1);
    }
    let q=s-.5;
    let k=vec2i(floor(q));
    let wx=catmull(q.x-floor(q.x));
    let wy=catmull(q.y-floor(q.y));
    var c=vec3f(0);
    for(var y=0;y<4;y++) {
        for(var x=0;x<4;x++) {
            c+=pixel_colour(k+vec2i(x-1,y-1))*wx[x]*wy[y];
        }
    }
    return vec4f(encode(max(c,vec3f(0))),1);
}
