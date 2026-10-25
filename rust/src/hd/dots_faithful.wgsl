// Faithful HD for part 3 (M, --faithful): the original's look at display
// resolution, in the original's 4:3 frame.
// - The hold and the zoom: part 5's own faithful HD (field_faithful.wgsl),
//   drawn by field_render.rs at the original's magnification about the
//   screen's centre (FieldBlock::part3_view), so the frame is resampled
//   from the picture at every scale rather than enlarged from a raster.
// - The shade bobs: each kept iteration's 20 discs where the port's path
//   table puts them, as smooth circles in source pixels, their overlap
//   counted and coloured through the shade palette (interpolated between
//   its entries). Between frames the newest iteration fades in and the one
//   erased fades out.
// - The dots: the eight echoes 4 frames apart, each the port's own
//   camera-space points (linear between its frames) projected as the code
//   projects them, as round dots of a source pixel's area, at the port's
//   echo levels; where echoes overlap the brighter shows, as in the palette.
// - Part 4's first frame: its slab, smoothly upscaled.
// Not reproduced: the original's whole-pixel positions and its asynchronous
// page flips.
// Module: the shared controls, dots_common.wgsl and this file.

// The marks: the shade count (added) and the dots' echo level (the
// brightest kept), each a full-resolution r16float target.
@group(1) @binding(1) var marks_count:texture_2d<f32>;
@group(1) @binding(2) var marks_level:texture_2d<f32>;
// Part 5's faithful HD at the original's magnification, in the output's
// format (dots_render.rs), during the hold and the zoom.
@group(1) @binding(3) var underlay:texture_2d<f32>;

// The dots' radius, in rows: a round dot of one source pixel's area.
const DOT_RADIUS=.515;

struct Mark {
    @builtin(position) position:vec4f,
    // The pixel's offset from the mark's centre: source pixels for a shade
    // disc, scene rows for a dot.
    @location(0) local:vec2f,
    @location(1) value:f32,
}
struct Marks { @location(0) count:vec4f, @location(1) level:vec4f }
fn no_mark()->Mark {return Mark(vec4f(-2,-2,0,1),vec2f(0),0.0);}
fn quad_corner(vi:u32)->vec2f {
    return array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(1,1),vec2f(-1,-1),vec2f(1,1),vec2f(-1,1))[vi];
}

// Whether the frame with `drawn` iterations drawn keeps iteration g's discs:
// the last 16 drawn, if g added any.
fn shade_kept(g:i32,drawn:f32)->bool {
    let x=f32(g);
    return g>=0 && x>=drawn-SHADE_KEPT && x<=drawn-1.0 && x<shade_stop();
}
// Instance slot * 20 + disc: slot s is iteration drawn - 16 + s, so the 17
// slots hold the shown frame's iterations and the next frame's newest.
@vertex fn faithful_shade(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Mark {
    dots_begin();
    if !shades_shown() {return no_mark();}
    let drawn=dc.shades.w;
    let f=dc.shades.x-drawn;
    let g=i32(drawn)-16+i32(ii/SHADE_DISCS);
    let w=select(0.0,1.0-f,shade_kept(g,drawn))+select(0.0,f,shade_kept(g,drawn+1.0));
    if w<=0.0 {return no_mark();}
    let centre=shade_disc(g,ii%SHADE_DISCS);
    let reach=SHADE_RADIUS+2.0*pixel_rows()/COLUMN;
    let local=quad_corner(vi)*reach;
    return Mark(clip_of_scene(scene_of_source(centre+local)),local,w);
}
@fragment fn faithful_shade_mark(m:Mark)->Marks {
    let aa=max(fwidth(m.local.y),1e-4);
    let cover=clamp((SHADE_RADIUS-length(m.local))/aa+.5,0.0,1.0);
    return Marks(vec4f(m.value*cover,0,0,1),vec4f(0));
}
// Instance echo * 512 + point.
@vertex fn faithful_dot(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Mark {
    dots_begin();
    let k=ii/POINTS;
    let i=ii%POINTS;
    let tau=min(dot_position(),last_dot_frame())-ECHO_SPACING*f32(k);
    if !dots_shown() || tau<0.0 {return no_mark();}
    let p=port_point_at(i,tau);
    // The code plots column 160 + x and row 100 + y, x and y truncated
    // towards zero; its tables send anything else off the page.
    let xy=FOCAL*p.xy/p.z;
    if xy.x<=-161.0 || xy.x>=160.0 || xy.y<=-101.0 || xy.y>=100.0 {return no_mark();}
    let centre=scene_of_source(vec2f(160.5,100.5)+xy);
    let reach=DOT_RADIUS+2.0*pixel_rows();
    let local=quad_corner(vi)*reach;
    let level=dc.echo_levels[k/4u][k%4u];
    return Mark(clip_of_scene(centre+local),local,level);
}
@fragment fn faithful_dot_mark(m:Mark)->Marks {
    let cover=clamp((DOT_RADIUS-length(m.local))/pixel_rows()+.5,0.0,1.0);
    return Marks(vec4f(0),vec4f(m.value*cover,0,0,1));
}

fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308)),x,u.misc.y>.5);
}
// The shade palette at a continuous count, linear between its entries in
// DAC units, then linear light.
fn shade_color(count:f32)->vec3f {
    let at=(clamp(count,0.0,255.0)+.5)/256.0;
    return srgb_decode(textureSampleLevel(shade_palette,linear,vec2f(at,.5),0.0).rgb);
}
@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    dots_begin();
    let pixel=vec2i(v.position.xy);
    let s=source_of_scene(scene_of_uv(v.uv));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !in_frame(s) {return vec4f(0,0,0,1);}
    if compare {
        let ip=clamp(vec2i(s),vec2i(0),vec2i(319,199));
        return vec4f(encode(srgb_decode(textureLoad(reference,ip,0).rgb)),1);
    }
    let phase=dots_phase();
    // Part 5's faithful HD, already in the output's encoding.
    if phase<1.5 {return vec4f(textureLoad(underlay,pixel,0).rgb,1);}
    var c=vec3f(0);
    if phase<2.5 {
        c=shade_color(textureLoad(marks_count,pixel,0).r);
    } else if phase<4.5 {
        c=vec3f(0,0,dac_linear(textureLoad(marks_level,pixel,0).r));
    } else {
        let at=s/vec2f(320,200);
        c=srgb_decode(textureSampleLevel(slab,linear,at,0.0).rgb);
    }
    return vec4f(encode(c),1);
}
