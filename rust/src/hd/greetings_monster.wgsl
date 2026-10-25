// Modern part 7's last frames: MONSTER.U developing out of the white, in a
// module of its own (the shared controls, greetings_common.wgsl and this
// file), so that changes to the lighthouse (greetings_light.wgsl) cannot
// change how these frames, which part 8 begins from, are compiled; and the
// lighthouse's passes are not drawn for them.
// - Read from the port: the image, and the palette each frame shows (the
//   original's level of the monster: trunc(m*L/64)+trunc(63*(64-L)/64)).
// - Authored: the upscaling (Catmull-Rom), and showing it through the
//   shared tonemap backwards, its whites held a little below the top once
//   the picture is up (so it keeps its colours, near enough).

// The brightest the monster shows through the tonemap once developed.
const MONSTER_WHITE=.9;

fn monster_colour(p:vec2i)->vec3f {
    let q=clamp(p,vec2i(0),vec2i(319,199));
    return entry(textureLoad(monster,q,0).r,true);
}
fn catmull(t:f32)->vec4f {
    let t2=t*t;let t3=t2*t;
    return vec4f(-.5*t3+t2-.5*t,1.5*t3-2.5*t2+1.0,-1.5*t3+2.0*t2+.5*t,.5*t3-.5*t2);
}
// MONSTER.U at the frame's palette (the original's level of it), upscaled;
// outside the 4:3 frame, its colour 0 at that level.
fn monster_picture(s:vec2f)->vec3f {
    if !in_frame(s) {return entry(0u,true);}
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
    return clamp(c,vec3f(0),vec3f(1));
}
@fragment fn monster_style(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let q=scene_of_uv(v.uv);
    // Developed out of the white: shown as the original shows it, through
    // the tonemap backwards, the whites held a little below its top once
    // the picture is up.
    let top=mix(.999,MONSTER_WHITE,clamp(gc.sweep.w,0.0,1.0));
    return vec4f(tonemap_inverse(min(monster_picture(source_of_scene(q)),vec3f(top))),1);
}
