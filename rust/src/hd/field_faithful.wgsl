// Faithful HD for part 5 (M, --faithful): the original's look at display
// resolution. The frame is the PSTART picture Field::new makes, smoothly
// upscaled through the current palette, its window's edge kept sharp. The
// window shows the zoom from the port's field buffers, smoothly resampled at
// the interpolated scale in the original's palette, still once it stops,
// then colour 129 once cleared. The terrain is the original's prepared
// DEP.U and MAP.U, marched per pixel with the port's camera and projection
// (terrain_march, from field_common.wgsl) and coloured through the palette;
// as in the original, it never covers the top row. Not reproduced: the
// original's two-column rays, whole-unit steps and integer camera.
// Module: the shared controls, field_common.wgsl and this file.
fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308)),x,u.misc.y>.5);
}
fn faithful_field(px:FieldPixel)->vec3f {
    let s=px.source;
    var c=picture_frame(s,picture_level());
    let w=window_cover(s,px.pixel);
    if w>0.0 {c=mix(c,window_content(s),w);}
    if field_phase()>.5 && s.y>=1.0 && in_frame(s) {
        let hit=terrain_march(s,px.pixel,false);
        if hit.cover>0.0 {c=mix(c,terrain_color(hit.map),hit.cover);}
    }
    return c;
}
@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    field_begin();
    // Before any branch on the pixel: it takes derivatives.
    let px=field_pixel(v);
    let hd=faithful_field(px);
    let p=px.source/vec2f(320,200);
    let inside=all(p>=vec2f(0)) && all(p<=vec2f(1));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !inside {return vec4f(0,0,0,1);}
    if compare {
        let ip=clamp(vec2i(p*vec2f(320,200)),vec2i(0),vec2i(319,199));
        return vec4f(encode(srgb_decode(textureLoad(reference,ip,0).rgb)),1);
    }
    return vec4f(encode(hd),1);
}
