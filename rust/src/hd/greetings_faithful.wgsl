// Faithful HD for part 7 (M, --faithful): the original's look at display
// resolution, in the original's 4:3 frame.
// - Every pixel's colour index is the original's (greetings_common.wgsl's
//   index_at: the sky, and each strip row from the source the scene last
//   copied to it, the bottom row's smear and the sky copied from beside the
//   strip included); the palette turns between frames as the star colours
//   cycle and the fades run.
// - The stars are drawn as round dots, one to a stored pixel; the letters'
//   colours are interpolated within and between their rows.
// - The monster is MONSTER.U, upscaled smoothly (Catmull-Rom).
// Not reproduced: the original's square pixels.
// Module: the shared controls, greetings_common.wgsl and this file.

// The dots' radius in source rows (authored: a little under half a pixel,
// so neighbours stay apart).
const DOT_RADIUS=.42;

fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(srgb_encode(x),x,u.misc.y>.5);
}

// Pixel (x, y)'s colour, a star's ground (colour 0) for a star.
fn ground_colour(j:u32,p:vec2i)->vec3f {
    let q=clamp(p,vec2i(0),vec2i(319,199));
    let i=index_at(j,u32(q.x),u32(q.y));
    return entry(select(i,0u,i>=STAR_BASE),true);
}

// The monster's colour at pixel p, clamped to the image.
fn monster_colour(p:vec2i)->vec3f {
    let q=clamp(p,vec2i(0),vec2i(319,199));
    return entry(textureLoad(monster,q,0).r,true);
}
fn catmull(t:f32)->vec4f {
    let t2=t*t;let t3=t2*t;
    return vec4f(-.5*t3+t2-.5*t,1.5*t3-2.5*t2+1.0,-1.5*t3+2.0*t2+.5*t,.5*t3-.5*t2);
}

@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let s=source_of_scene(scene_of_uv(v.uv));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !in_frame(s) {return vec4f(0,0,0,1);}
    if compare {
        let ip=clamp(vec2i(s),vec2i(0),vec2i(319,199));
        return vec4f(encode(srgb_decode(textureLoad(original,ip,0).rgb)),1);
    }
    let j=frame_now();
    if shows_monster() {
        let q=s-.5;
        let k=vec2i(floor(q));
        let wx=catmull(q.x-floor(q.x));
        let wy=catmull(q.y-floor(q.y));
        var c=vec3f(0);
        for(var y=0;y<4;y++) {
            for(var x=0;x<4;x++) {
                c+=monster_colour(k+vec2i(x-1,y-1))*wx[x]*wy[y];
            }
        }
        return vec4f(encode(max(c,vec3f(0))),1);
    }
    // The letters and the ground: bilinear between pixel centres (the
    // ground is colour 0, black until the fade to white lifts it).
    let q=s-.5;
    let k=vec2i(floor(q));
    let f=q-floor(q);
    var c=mix(mix(ground_colour(j,k),ground_colour(j,k+vec2i(1,0)),f.x),
        mix(ground_colour(j,k+vec2i(0,1)),ground_colour(j,k+vec2i(1,1)),f.x),f.y);
    // The stars: a round dot on each star pixel's centre.
    let cell=vec2i(floor(s));
    let aa=100.0/u.screen.y;
    for(var dy=-1;dy<=1;dy++) {
        for(var dx=-1;dx<=1;dx++) {
            let p=cell+vec2i(dx,dy);
            if any(p<vec2i(0)) || p.x>319 || p.y>199 {continue;}
            let i=index_at(j,u32(p.x),u32(p.y));
            if i<STAR_BASE {continue;}
            let d=(s-(vec2f(p)+.5))*vec2f(COLUMN,1.0);
            let cover=1.0-smoothstep(DOT_RADIUS-aa,DOT_RADIUS+aa,length(d));
            c=mix(c,entry(i,true),cover);
        }
    }
    return vec4f(encode(c),1);
}
