// Part 5's metal frame and the smoke behind and round it: the frame
// author's file (with field_frame.rs). field_render.rs builds the modern
// module from the shared controls, field_common.wgsl, this file,
// field_zoom.wgsl, field_mountain.wgsl and field_style.wgsl. Prefix this
// file's own names with frame_. It defines:
//   fn frame_layers(px:FieldPixel)->FrameLayers   smoke behind the frame,
//       the frame, smoke in front; each a FieldLayer (premultiplied linear
//       HDR light, opacity, scene z)
//   fn frame_fluid_init(cell:FluidCell)->vec4f     the fluid's first state
//   fn frame_fluid(cell:FluidCell)->vec4f          one fixed step: the cell's
//       next state from the state before (fluid_load, fluid_sample) and the
//       step's block (fc, the usual helpers), deterministic
//   fn frame_prepass(px:FieldPixel)->vec4f         the half-resolution
//       prepass, when FRAME.prepass is set (read it with frame_prepass_at)
// The fluid step is a compute shader: use textureSampleLevel or
// textureLoad, not textureSample, fwidth or the music cues, and not
// fluid_blend, which is for drawing.
//
// Stand-in: the PSTART frame as the original shows it, smoothly upscaled,
// opaque round the window, in the frame's plane; no smoke. The fluid runs,
// drawing nothing: its state counts its steps, modulo 1000, in w, which the
// ignored test part5_fluid_is_the_same_however_it_is_reached reads back.
fn frame_layers(px:FieldPixel)->FrameLayers {
    let s=px.source;
    var frame=no_layer();
    if in_frame(s) {
        let open=1.0-window_cover(s,px.pixel);
        frame=field_layer(tonemap_inverse(picture_frame(s,picture_level())),open,LAYER_FRAME_Z);
    }
    return FrameLayers(no_layer(),frame,no_layer());
}
fn frame_fluid_init(cell:FluidCell)->vec4f {return vec4f(0);}
fn frame_fluid(cell:FluidCell)->vec4f {
    let before=fluid_load(cell.texel);
    return vec4f(0,0,0,f32((u32(round(before.w))+1u)%1000u));
}
fn frame_prepass(px:FieldPixel)->vec4f {return vec4f(0);}
