// Part 8 (the ending): bindings, coordinates and helpers shared by faithful
// HD (ending_faithful.wgsl) and the modern view (ending_crt.wgsl).
// ending_render.rs builds each module from the shared controls, this file
// and its own.
//
// Where the values come from (hd/ending.rs has the details):
// - Read from the port's Ending scene: every frame's indices and palette,
//   the step each frame shows since the music reached 35:00 and so the
//   stage, the compression each step shows and the line's ends as each step
//   erases them; MONSTER.U as part 8 inherits it in video memory.
// - Interpretation: the stages as a television switched off; interpolation
//   between the original's 70 Hz frames.
// - Authored: everything ending_crt.wgsl adds.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl;
// part 8's share is u.ending, an EndingBlock, read through `ec`.

// The distinct pictures the frames show, 320 x 200 indices each, four to a
// word (the lowest first): picture i's pixel (x, y) is byte
// i * 64000 + y * 320 + x.
@group(0) @binding(1) var<storage,read> images:array<u32>;
// The palette each frame shows, 256 x frames, the 6-bit DAC values expanded
// to 8 bits as the scanout does (sRGB-encoded).
@group(0) @binding(2) var palettes:texture_2d<f32>;
// MONSTER.U's indices as part 8 inherits them, 320 x 200.
@group(0) @binding(3) var monster:texture_2d<u32>;
// The original frame for the comparison modes, 320 x 200 (sRGB-encoded).
@group(0) @binding(4) var original:texture_2d<f32>;
// Linear, clamped at the edges.
@group(0) @binding(5) var linear:sampler;

// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// ending::LINE_ROW and DOT_COLUMN (read from the code).
const LINE_ROW=100.0;
const DOT_COLUMN=160.0;

// The part-8 block for this invocation.
var<private> ec:EndingBlock;
fn ending_begin() {ec=u.ending;}

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates (as in part 7) -----------------------------------------------
// - source: the original's 320 x 200 pixels, y down.
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre: x = (source x - 160) * 5/6, y = 100 - source y.
fn screen_aspect()->f32 {return u.screen.x/u.screen.y;}
fn source_of_scene(q:vec2f)->vec2f {return vec2f(160.0+q.x/COLUMN,100.0-q.y);}
fn scene_of_uv(uv:vec2f)->vec2f {return (uv-.5)*vec2f(2.0*screen_aspect(),-2.0)*100.0;}
fn in_frame(s:vec2f)->bool {return all(s>=vec2f(0)) && all(s<=vec2f(320,200));}

// ---- Timing ----------------------------------------------------------------------
fn frame_now()->u32 {return u32(ec.frame.x);}
fn frame_next()->u32 {return u32(ec.frame.y);}
fn frame_mix()->f32 {return ec.frame.z;}

// ---- Colour ----------------------------------------------------------------------
fn srgb_decode(c:vec3f)->vec3f {return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));}
fn srgb_encode(c:vec3f)->vec3f {
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308));
}
// Palette entry `index` as the shown frame shows it, toward the next's
// when `blend`, linear (as part 7's).
fn entry(index:u32,blend:bool)->vec3f {
    let a=srgb_decode(textureLoad(palettes,vec2u(index,frame_now()),0).rgb);
    let b=srgb_decode(textureLoad(palettes,vec2u(index,frame_next()),0).rgb);
    return mix(a,b,select(0.0,frame_mix(),blend));
}
fn catmull(t:f32)->vec4f {
    let t2=t*t;let t3=t2*t;
    return vec4f(-.5*t3+t2-.5*t,1.5*t3-2.5*t2+1.0,-1.5*t3+2.0*t2+.5*t,.5*t3-.5*t2);
}
