// Faithful HD for part 6 (M, --faithful): the original's look at display
// resolution. The rings are the analytic bands fitted to M.DAT (an
// interpretation matching 98% of its pixels), circles in source pixels so
// 1.2 times taller than wide on the 4:3 display as in the original, in the
// original's window, with its two greys and XOR, antialiased. The symbols are
// always the original's masks (original_mask(), never the authored sets the
// neon shows), their smoothed stroke outlines at half strength in the
// original's average tint: the original lights them on alternate frames only, and this shows
// the average of that 35 Hz flicker in linear light. The PSTART picture is
// smoothly upscaled and faded in DAC units, as its palette is. Not
// reproduced: the original's one-frame stale ring pages after a page flip.
// Module: the shared controls, rings_common.wgsl and this file.
fn encode(c:vec3f)->vec3f {
    // Linear light out; sRGB-encode it when the target doesn't.
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308)),x,u.misc.y>.5);
}
fn faithful_rings(px:RingsPixel)->vec3f {
    let s=px.source;
    if rings_phase()>1.5 {return picture_color(s,picture_level());}
    let window=ring_window(s,px.pixel);
    // Antialias over the pixel's footprint across the bands.
    let a=ring_cover(0u,s,RING_FAITHFUL,0.0,fwidth(ring_radius(0u,s,RING_FAITHFUL,0.0)))*window;
    let b=ring_cover(1u,s,RING_FAITHFUL,0.0,fwidth(ring_radius(1u,s,RING_FAITHFUL,0.0)))*window;
    var c=ring_xor(a,b);
    for(var k=0;k<2;k++) {
        let mask=select(original_mask(),original_previous(),k==1);
        let on=select(neon_on(),neon_previous_on(),k==1);
        if on>0.0 {
            let d=symbol_field(mask,s).x;
            let cover=clamp(d/max(fwidth(d),1e-3)*.75+.5,0.0,1.0)*on;
            c=mix(c,.5*c+.5*SYMBOL_YELLOW,cover);
        }
    }
    return c;
}
@fragment fn faithful(v:Vertex)->@location(0) vec4f {
    let px=rings_pixel(v);
    // Before any branch on the pixel: it takes derivatives.
    let hd=faithful_rings(px);
    let p=px.source/vec2f(320,200);
    let inside=all(p>=vec2f(0)) && all(p<=vec2f(1));
    let compare=u.screen.w>.5 && (u.screen.w<1.5 || v.uv.x<.5);
    if u.screen.w>1.5 && abs(v.uv.x-.5)<1.0/u.screen.x {return vec4f(encode(vec3f(.25,.4,.65)),1);}
    if !inside {return vec4f(0,0,0,1);}
    if compare {
        let ip=clamp(vec2i(p*vec2f(320,200)),vec2i(0),vec2i(319,199));
        let c=textureLoad(reference,ip,0).rgb;
        return vec4f(encode(select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045))),1);
    }
    return vec4f(encode(hd),1);
}
