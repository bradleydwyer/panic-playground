// Faithful HD for part 4 (M, --faithful): the original's look at display
// resolution, in the original's 4:3 frame.
// - The plasma: each of the 400 rasters reads video memory from its own
//   start address, as the scene's strides set it, at a continuous column,
//   so the field's colours are smooth along a raster while the rasters
//   keep their own offsets (the original's interlaced look). The start
//   addresses move between frames where they move a little; the palette
//   turns and fades between frames.
// - The logo: its pixels' colours, interpolated, in the palette each frame
//   shows (the fade from white between frames); the line compare at a
//   continuous raster, so the slide is smooth.
// - Part 7's first frame, smoothly upscaled.
// Not reproduced: the field's four-pixel columns and the original's whole
// rasters.
// Module: the shared controls, plasma_common.wgsl and this file.

fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308)),x,u.misc.y>.5);
}

@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    plasma_begin();
    let s=source_of_scene(scene_of_uv(v.uv));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !in_frame(s) {return vec4f(0,0,0,1);}
    if compare {
        // The original at its own raster: 400 lines, or part 7's 200.
        let rows=select(400.0,200.0,part7_shown());
        let ip=clamp(vec2i(vec2f(s.x,s.y*rows/200.0)),vec2i(0),vec2i(319,i32(rows)-1));
        return vec4f(encode(srgb_decode(textureLoad(original,ip,0).rgb)),1);
    }
    if part7_shown() {
        return vec4f(encode(srgb_decode(textureSampleLevel(part7,linear,s/vec2f(320,200),0.0).rgb)),1);
    }
    // The raster (continuous) and the line compare: the logo starts on the
    // raster after it.
    let raster=s.y*2.0;
    let compare_line=pc.logo.x;
    var c=vec3f(0);
    if raster>=compare_line+1.0 {
        c=logo_colour(vec2f(s.x,raster-compare_line-1.0));
    } else {
        let r=min(u32(raster),399u);
        let address=raster_at(r)+s.x/4.0;
        if field_written() {
            let p=field_phase(address);
            if p>=0.0 {c=dac_linear(bank_colour(p));}
        }
    }
    return vec4f(encode(c),1);
}
