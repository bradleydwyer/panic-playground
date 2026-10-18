// Faithful-plus (authored, close to the original): the original's two-grey
// XOR moiré, from the band model and palette exactly as faithful HD shows
// them, but antialiased and blurred over a 1/100 s shutter so it doesn't
// strobe; the lit symbol as yellow neon tubes along its strokes'
// centrelines (rings_common.wgsl's neon_tube: a glowing gas column in a
// clear glass tube, dark at the electrodes, with a haze round them), the
// print seen bent through the glass,
// just above the rings, lighting them and a dark backing behind them in a
// wide pool, as a sign lights its wall; a faint print grain on
// the bands; HDR bloom from the shared post chain. The reference style: it
// shows the shared plumbing, timing and helpers at work.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl.
// The tubes' height above the printed rings, in rows (authored), and a
// second, much greater height whose light stands in for the wide pool a sign
// throws on the wall behind it.
const PLUS_NEON_HEIGHT=5.0;
const PLUS_NEON_POOL=40.0;
// Shutter samples for the rings' motion blur.
const PLUS_SHUTTER=4;
// The dark backing behind the print, across the whole screen, as albedo, and
// the print grain's depth.
const PLUS_BACKING=.05;
const PLUS_GRAIN=.06;
fn plus_print(px:RingsPixel)->vec3f {
    let s=px.source;
    let window=ring_window(s,px.pixel*1.5);
    var c=vec3f(0);
    for(var i=0;i<PLUS_SHUTTER;i++) {
        let t=(f32(i)+.5)/f32(PLUS_SHUTTER);
        let a=ring_cover(0u,s,RING_FAITHFUL,t,px.pixel);
        let b=ring_cover(1u,s,RING_FAITHFUL,t,px.pixel);
        c+=ring_xor(a,b);
    }
    c/=f32(PLUS_SHUTTER);
    // Grain fixed to the print, not the moving bands: about a fifth of a
    // source pixel.
    let grain=hash2(floor(s*5.0))+hash2(floor(s*5.0)+vec2f(.5,7.3))-1.0;
    return c*(1.0+PLUS_GRAIN*grain)*window;
}
// What lies behind the tubes at source point s: the print, lit by them.
fn plus_behind(px:RingsPixel,s:vec2f)->vec3f {
    var at=px;
    at.source=s;
    at.scene=scene_of_source(s);
    let print=plus_print(at);
    // The tubes light what is behind them: the bands, and the dark backing
    // everywhere, near them and in a wide pool.
    let albedo=print+vec3f(PLUS_BACKING);
    let p=vec3f(at.scene,0);
    let up=vec3f(0,0,1);
    let light=neon_irradiance(p,up,PLUS_NEON_HEIGHT)*.25+neon_irradiance(p,up,PLUS_NEON_POOL)*.7;
    return tonemap_inverse(print)+albedo*light;
}
fn style_color(px:RingsPixel)->vec3f {
    if rings_phase()<.5 {return vec3f(0);}
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let behind=plus_behind(px,px.source);
    let tubes=neon_glass(px.source,px.pixel);
    // Through the glass, the print a little to the side.
    var seen=behind;
    if tubes.glass>0.0 {seen=plus_behind(px,px.source+tubes.bend);}
    return neon_over(behind,seen,tubes)+neon_haze(px.source);
}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
