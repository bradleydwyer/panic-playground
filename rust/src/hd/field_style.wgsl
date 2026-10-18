// Entry points of part 5's modern module. field_render.rs builds it from the
// shared controls, field_common.wgsl, the three authors' files
// (field_frame.wgsl, field_zoom.wgsl, field_mountain.wgsl) and this one;
// see rust/README.md, "Adding to part 5". field_style composites the
// authors' layers by depth: the window's content, the smoke behind the
// frame, the frame, the smoke in front and the mountain, over black, in
// linear HDR; the shared bloom and tonemap follow.
@fragment fn field_style(v:Vertex)->@location(0) vec4f {
    field_begin();
    let px=field_pixel(v);
    let f=frame_layers(px);
    let c=field_composite(array<FieldLayer,5>(zoom_layer(px),f.behind,f.frame,f.front,mountain_layer(px)));
    return vec4f(max(c,vec3f(0)),1);
}
@fragment fn field_frame_prepass(v:Vertex)->@location(0) vec4f {
    field_begin();
    return frame_prepass(field_pixel(v));
}
@fragment fn field_zoom_prepass(v:Vertex)->@location(0) vec4f {
    field_begin();
    return zoom_prepass(field_pixel(v));
}
@fragment fn field_mountain_prepass(v:Vertex)->@location(0) vec4f {
    field_begin();
    return mountain_prepass(field_pixel(v));
}
// One fluid step over the grid, or its first state.
@compute @workgroup_size(8,8) fn field_fluid(@builtin(global_invocation_id) id:vec3u) {
    fc=fluid_step.block;
    let size=vec2u(textureDimensions(fluid_out));
    if any(id.xy>=size) {return;}
    let cell=fluid_cell(vec2i(id.xy));
    var value:vec4f;
    if fluid_step.step.w>.5 {value=frame_fluid_init(cell);} else {value=frame_fluid(cell);}
    textureStore(fluid_out,id.xy,value);
}
