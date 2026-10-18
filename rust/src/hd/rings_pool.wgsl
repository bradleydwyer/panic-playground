// Pool (stub, to be replaced): a liquid-chrome interference pool, the two
// ring layers as waves on its surface, with the symbols as neon lights above
// it. For now the plain reference rendering with a silver-blue label tint.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with pool_.
fn style_color(px:RingsPixel)->vec3f {return rings_reference(px)*vec3f(.8,.9,1.0);}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
