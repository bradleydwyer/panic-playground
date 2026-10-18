// Glass (stub, to be replaced): the owner's pick. Two glass ring layers, one
// lit by cool light and the other by warm, with the symbols as neon lights
// that change the reflections in the glass. For now the plain reference
// rendering with a cool label tint, so the style can be selected.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with glass_.
fn style_color(px:RingsPixel)->vec3f {return rings_reference(px)*vec3f(.55,.8,1.25);}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
