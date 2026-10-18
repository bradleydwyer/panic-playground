// Part 5's mountain: the mountain author's file (with field_mountain.rs).
// field_render.rs builds the modern module from the shared controls,
// field_common.wgsl, field_frame.wgsl, field_zoom.wgsl, this file and
// field_style.wgsl. Prefix this file's own names with mountain_. It defines:
//   fn mountain_layer(px:FieldPixel)->FieldLayer   the mountain, in front of
//       the frame (LAYER_MOUNTAIN_Z by default)
//   fn mountain_prepass(px:FieldPixel)->vec4f      the half-resolution
//       prepass, when MOUNTAIN.prepass is set (read it with
//       mountain_prepass_at)
//
// Stand-in: the relief fitted to DEP.U (relief_at) marched per pixel with
// the port's camera and projection (terrain_march), coloured by height along
// the palette's ramp (MAP.PAL during the terrain) and lit from one side of
// the map.
fn mountain_layer(px:FieldPixel)->FieldLayer {
    if field_phase()<.5 {return no_layer();}
    let hit=terrain_march(px.source,px.pixel,true);
    if hit.cover<=0.0 {return no_layer();}
    let r=relief_at(hit.map);
    // Map x, map y and up; a height unit about a quarter of a map unit.
    let n=normalize(vec3f(-.25*r.y,-.25*r.z,1.0));
    let light=normalize(vec3f(-.4,.3,.85));
    let base=palette_color(128.0+63.0*clamp(r.x/max(relief_highest(),1.0),0.0,1.0));
    let c=base*(.25+.75*max(dot(n,light),0.0));
    return field_layer(tonemap_inverse(c),hit.cover,LAYER_MOUNTAIN_Z);
}
fn mountain_prepass(px:FieldPixel)->vec4f {return vec4f(0);}
