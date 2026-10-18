// Volume (stub, to be replaced): volumetric spherical-wave interference in
// haze, lit by the neon symbols. For now the plain reference rendering with
// a violet label tint. A volume may want the half-resolution prepass: set
// prepass in VOLUME (hd/rings.rs) and fill style_prepass.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with volume_.
fn style_color(px:RingsPixel)->vec3f {return rings_reference(px)*vec3f(1.0,.7,1.25);}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
