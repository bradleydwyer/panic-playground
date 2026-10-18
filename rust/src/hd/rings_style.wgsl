// Entry points of every part-6 style module. rings_render.rs builds each
// style from the shared controls, rings_common.wgsl, wall.wgsl, the style's
// own file and this one, so each style needs only define
//   fn style_color(px:RingsPixel)->vec3f    linear HDR radiance; the shared
//                                           bloom and tonemap follow
//   fn style_prepass(px:RingsPixel)->vec4f  a half-resolution pass run first
//                                           when its StyleInfo asks for one
//                                           (read it with prepass_at)
@fragment fn rings_style(v:Vertex)->@location(0) vec4f {
    return vec4f(max(style_color(rings_pixel(v)),vec3f(0)),1);
}
@fragment fn rings_prepass(v:Vertex)->@location(0) vec4f {
    return style_prepass(rings_pixel(v));
}
