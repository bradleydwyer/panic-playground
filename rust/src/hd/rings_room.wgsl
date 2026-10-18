// Room (stub, to be replaced): the scroller's chrome bump wall in the dark,
// lit by the neon symbols, with the rings projected onto it in blue and
// orange. For now the plain reference rendering with a warm label tint, over
// the bump wall (wall.wgsl, as part 2 left it) lit faintly by the neon, to
// show both are callable here.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with room_.
fn style_color(px:RingsPixel)->vec3f {
    // The scroller's world: 100 rows to a unit, the eye 5.5 units out.
    let ray=normalize(vec3f(px.xy,-5.5));
    let hit=wall_trace(vec3f(0,0,5.5),ray,false);
    let p=(vec3f(0,0,5.5)+ray*hit.t)*100.0;
    let wall=vec3f(.5,.52,.56)*neon_irradiance(p,hit.n,0.0)*.4;
    return rings_reference(px)*vec3f(1.25,.85,.6)+wall;
}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
