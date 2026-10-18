// What part 5's window shows: the zoom author's file (with field_zoom.rs).
// field_render.rs builds the modern module from the shared controls,
// field_common.wgsl, field_frame.wgsl, this file, field_mountain.wgsl and
// field_style.wgsl. Prefix this file's own names with zoom_. It defines:
//   fn zoom_layer(px:FieldPixel)->FieldLayer   the window's content, behind
//       the frame (LAYER_WINDOW_Z by default); the frame hides it round the
//       window, so it may reach past the window's edge
//   fn zoom_prepass(px:FieldPixel)->vec4f      the half-resolution prepass,
//       when ZOOM.prepass is set (read it with zoom_prepass_at)
// The zoom runs from part 5's start to 96.96 s and stays still until the
// window is cleared at 98.11 s; the window must end dark and empty, as part
// 3 zooms into it.
//
// Stand-in: the window as the original shows it, smoothly: PSTART's own
// window on the first frame, then the zoom from the port's field buffers in
// the original's palette, still once it stops, then colour 129.
fn zoom_layer(px:FieldPixel)->FieldLayer {
    let s=px.source;
    if window_cover(s,px.pixel)<=0.0 {return no_layer();}
    return field_layer(tonemap_inverse(window_content(s)),1.0,LAYER_WINDOW_Z);
}
fn zoom_prepass(px:FieldPixel)->vec4f {return vec4f(0);}
