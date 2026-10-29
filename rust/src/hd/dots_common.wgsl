// Part 3 (Dots): bindings, coordinates and helpers shared by faithful HD
// (dots_faithful.wgsl) and the modern view (dots_light.wgsl). dots_render.rs
// builds each module from the shared controls, this file and its own.
//
// Where the values come from (hd/dots.rs has the details):
// - Read from the port's Dots scene: the phases, the zoom's scale, the shade
//   iterations, their stop, the shade path and disc, the eight objects, the
//   stages and their morphs, the rotation, the camera spline, the points
//   each dot frame drew, the projection (column 160 + 256 X / Z, row 100 +
//   256 Y / Z) and the palettes.
// - Observed at load time: the phases' times and the orders' first rows.
// - Interpretation: interpolation between the original's 70 Hz frames; the
//   rotation, morphs and spline in floating point, without the code's Q7
//   coefficients and truncation; the shade path at continuous iterations.
// - Authored: everything dots_light.wgsl adds.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl;
// part 3's share is u.dots, a DotsBlock, read through `dc`.

// One dot frame (dots::DotFrame): the rotation (1024ths of a turn,
// unwrapped) and the stage; the translation and the morph (0..1).
struct DotFrame { angles:vec4f, translation:vec4f }
@group(0) @binding(1) var<storage,read> dot_frames:array<DotFrame>;
// The eight objects in storage order (P3D1 to P3D8), 512 points each,
// object-space xyz, before any morph.
@group(0) @binding(2) var<storage,read> objects:array<vec4f>;
// The camera-space points each dot frame drew, as the port computed them,
// 512 a frame: x | y << 16 (16-bit, signed), and z.
@group(0) @binding(3) var<storage,read> port_points:array<vec2u>;
// The shade path as the port tabulates it, in whole source pixels: the
// top-left corner of a disc, x and y, 1024 entries.
@group(0) @binding(4) var<storage,read> shade_path:array<vec2f>;
// The shade palette, 256 x 1: the 6-bit DAC values expanded to 8 bits as
// the scanout does (sRGB-encoded).
@group(0) @binding(5) var shade_palette:texture_2d<f32>;
// The original frame, for the comparison modes, 320 x 200.
@group(0) @binding(6) var reference:texture_2d<f32>;
// Part 4's first frame, its logo slab in white, 320 x 400 (sRGB-encoded).
@group(0) @binding(7) var slab:texture_2d<f32>;
// Linear, clamped at the edges.
@group(0) @binding(8) var linear:sampler;

const PI=3.14159265;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// dots::POINTS, SHADE_DISCS, SHADE_KEPT, ECHOES and ECHO_SPACING, read from
// the code.
const POINTS=512u;
const SHADE_DISCS=20u;
const SHADE_KEPT=16.0;
const ECHOES=8u;
const ECHO_SPACING=4.0;
// The order the stages show the objects in (dots.rs ORDER, read from the
// code).
const ORDER=array<u32,8>(0u,7u,1u,3u,4u,2u,5u,6u);
// The projection's focal length in source pixels: x = 256 X / Z, read from
// DRAW.
const FOCAL=256.0;
// The shade disc (dots.rs ROWS): 16 by 16 source pixels round the top-left
// corner plus 8, 188 pixels in all; a circle of radius 7.8 is its smooth
// stand-in (an interpretation).
const SHADE_RADIUS=7.8;

// The part-3 block for this invocation.
var<private> dc:DotsBlock;
fn dots_begin() {dc=u.dots;}

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates -------------------------------------------------------------
// As in parts 6 and 5:
// - screen: xy from -1 (bottom) to 1 (top), x scaled by the drawable's
//   aspect; the 4:3 frame spans x in [-4/3, 4/3].
// - source: the original's 320 x 200 pixels, y down.
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre: x = (source x - 160) * 5/6, y = 100 - source y.
// And the dots' camera space: the code's, x right, y down, z away, in which
// a point shows at source (160 + 256 x / z, 100 + 256 y / z).
fn screen_aspect()->f32 {return u.screen.x/u.screen.y;}
fn source_of_scene(q:vec2f)->vec2f {return vec2f(160.0+q.x/COLUMN,100.0-q.y);}
fn scene_of_source(s:vec2f)->vec2f {return vec2f((s.x-160.0)*COLUMN,100.0-s.y);}
fn scene_of_uv(uv:vec2f)->vec2f {return (uv-.5)*vec2f(2.0*screen_aspect(),-2.0)*100.0;}
fn clip_of_scene(q:vec2f)->vec4f {return vec4f(q.x/(100.0*screen_aspect()),q.y/100.0,0,1);}
// One target pixel in scene rows.
fn pixel_rows()->f32 {return 200.0/u.screen.y;}
fn in_frame(s:vec2f)->bool {return all(s>=vec2f(0)) && all(s<=vec2f(320,200));}
fn project(c:vec3f)->vec2f {return vec2f(160.0,100.0)+FOCAL*c.xy/max(c.z,1.0);}
// The camera-space direction a source point sees.
fn ray_of_source(s:vec2f)->vec3f {return normalize(vec3f(s-vec2f(160.0,100.0),FOCAL));}

// ---- Timing --------------------------------------------------------------------
fn dots_seconds()->f32 {return dc.part.y;}
fn dots_phase()->f32 {return dc.part.z;}
// The magnification the original shows during the zoom (1 before it).
fn zoom_original()->f32 {return dc.zoom.x;}
// The continuous dot frame (-1 before the dots; running on past the last,
// where the points stay, so the exposure catches up).
fn dot_position()->f32 {return dc.dots.x;}
fn last_dot_frame()->f32 {return dc.dots.y;}
fn dots_shown()->bool {return dc.dots.w>.5;}
fn shades_shown()->bool {return dc.shades.z>.5;}
fn shade_stop()->f32 {return dc.shades.y;}
fn slab_shown()->bool {return dc.end.z>.5;}

// ---- The dots --------------------------------------------------------------------
// The rotation's rows at angles a (1024ths of a turn), as COUNT_CONST's
// coefficients approximate them: camera = R object + translation.
struct Rotation { x:vec3f, y:vec3f, z:vec3f }
fn rotation(a:vec3f)->Rotation {
    let r=a*(2.0*PI/1024.0);
    let s=sin(r);let c=cos(r);
    return Rotation(
        vec3f(c.y*c.z,c.y*s.z,-s.y),
        vec3f(s.x*s.y*c.z-c.x*s.z,s.x*s.y*s.z+c.x*c.z,c.y*s.x),
        vec3f(c.x*s.y*c.z+s.x*s.z,c.x*s.y*s.z-s.x*c.z,c.y*c.x));
}
fn rotate(r:Rotation,p:vec3f)->vec3f {return vec3f(dot(r.x,p),dot(r.y,p),dot(r.z,p));}
// The dots' state at continuous dot frame tau: angles, translation, the
// stage and the morph, linear between frames. A stage's last frame has
// reached its morph's target, which is the next stage's object.
struct DotState { angles:vec3f, translation:vec3f, stage:u32, morph:f32 }
fn dot_state(tau:f32)->DotState {
    let last=u32(max(last_dot_frame(),0.0));
    let t=clamp(tau,0.0,f32(last));
    let k=min(u32(floor(t)),last);
    let f=select(t-f32(k),0.0,k==last);
    let a=dot_frames[k];
    let b=dot_frames[min(k+1u,last)];
    let same=b.angles.w==a.angles.w;
    return DotState(mix(a.angles.xyz,b.angles.xyz,f),mix(a.translation.xyz,b.translation.xyz,f),
        u32(a.angles.w),mix(a.translation.w,select(1.0,b.translation.w,same),f));
}
// Point i of a stage's object, the morph of the way to the next stage's.
fn object_point(stage:u32,i:u32,morph:f32)->vec3f {
    let a=objects[ORDER[min(stage,7u)]*POINTS+i].xyz;
    if stage>=7u {return a;}
    let b=objects[ORDER[stage+1u]*POINTS+i].xyz;
    return mix(a,b,morph);
}
// Point i in camera space at continuous dot frame tau.
fn camera_point(i:u32,tau:f32)->vec3f {
    let s=dot_state(tau);
    return rotate(rotation(s.angles),object_point(s.stage,i,s.morph))+s.translation;
}
// The port's own camera-space point i of dot frame n.
fn port_point(n:u32,i:u32)->vec3f {
    let p=port_points[n*POINTS+i];
    let x=f32(bitcast<i32>(p.x<<16u)>>16u);
    let y=f32(bitcast<i32>(p.x)>>16u);
    return vec3f(x,y,f32(bitcast<i32>(p.y)));
}
// The port's point i at continuous dot frame tau, linear between its frames.
fn port_point_at(i:u32,tau:f32)->vec3f {
    let last=u32(max(last_dot_frame(),0.0));
    let t=clamp(tau,0.0,f32(last));
    let k=min(u32(floor(t)),last);
    return mix(port_point(k,i),port_point(min(k+1u,last),i),t-f32(k));
}

// ---- The shade bobs ----------------------------------------------------------------
// The centre of disc t of iteration g as the port draws it: the path's
// x at 4 (g + 1) + 5 t and y at 3 (g + 1) + 5 t, plus 8 (source pixels).
fn shade_disc(g:i32,t:u32)->vec2f {
    let ix=u32(4*(g+1)+5*i32(t))&1023u;
    let iy=u32(3*(g+1)+5*i32(t))&1023u;
    return vec2f(shade_path[ix].x,shade_path[iy].y)+8.0;
}
// The same at a continuous iteration, from the path's formula with a
// floating-point sine (the port divides its table's values by 660, 350, 660
// and 700, truncating): an interpretation for smooth motion.
fn shade_tip(g:f32,t:u32)->vec2f {
    let ix=4.0*(g+1.0)+5.0*f32(t);
    let iy=3.0*(g+1.0)+5.0*f32(t);
    let w=2.0*PI/1024.0;
    let x=150.0-32767.0*sin(w*(5.0*ix+256.0))/660.0-32767.0*sin(w*9.0*ix)/350.0;
    let y=32767.0*sin(w*(4.0*iy+150.0))/660.0+32767.0*sin(w*7.0*iy)/700.0+90.0;
    return vec2f(x,y)+8.0;
}

// ---- Colour ---------------------------------------------------------------------------
fn srgb_decode(c:vec3f)->vec3f {return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));}
// A 6-bit DAC level as the scanout shows it, in linear light.
fn dac_linear(v:f32)->f32 {
    let e=(v*4.0+floor(v/16.0))/255.0;
    return srgb_decode(vec3f(e)).x;
}
// The HDR value the shared tonemap (post.wgsl's ACES fit) turns into c,
// for c below 1.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}
fn hash2(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
