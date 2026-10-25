// Part 7 (Greetings): bindings, coordinates and helpers shared by faithful
// HD (greetings_faithful.wgsl) and the modern view (greetings_light.wgsl).
// greetings_render.rs builds each module from the shared controls, this file
// and its own.
//
// Where the values come from (hd/greetings.rs has the details):
// - Read from the port's Greetings scene: every strip row's source in video
//   memory on every frame (the descriptors and sine positions each step drew
//   with), the TRUG maps (sky and glyphs), the palettes, the stages, the star
//   colours' phase and intensity, the fade's levels, MONSTER.U.
// - Interpretation: the letters as cubes and the sine's view index as their
//   turn; the star colours as a light turning round (160, 85); interpolation
//   between the original's 70 Hz frames.
// - Authored: everything greetings_light.wgsl adds.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl;
// part 7's share is u.greetings, a GreetingsBlock, read through `gc`.

// The TRUG maps as video memory holds them: plane p's byte a is byte
// p * 65536 + a, four to a word (the lowest first).
@group(0) @binding(1) var<storage,read> planes:array<u32>;
// Every frame's strip rows' sources, two to a word (the lower first): frame
// j, strip s, row y is entry j * 1000 + s * 200 + y.
@group(0) @binding(2) var<storage,read> shown:array<u32>;
// The modern view's letters: per frame six words of four; the first holds
// the first slot's top row (as i32) in x; then a strip each, its 16 slots'
// letters four to a word (0 = A, 255 none).
@group(0) @binding(3) var<storage,read> letters:array<vec4u>;
// The palette each frame shows, 256 x frames, the 6-bit DAC values expanded
// to 8 bits as the scanout does (sRGB-encoded).
@group(0) @binding(4) var palettes:texture_2d<f32>;
// MONSTER.U's indices, 320 x 200.
@group(0) @binding(5) var monster:texture_2d<u32>;
// The original frame for the comparison modes, 320 x 200 (sRGB-encoded).
@group(0) @binding(6) var original:texture_2d<f32>;
// Linear, clamped at the edges.
@group(0) @binding(7) var linear:sampler;

const PI=3.14159265;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// greetings::STRIPS, STRIP_WIDTH and ROWS, read from the code; the strips'
// left columns are 8 + 64 s.
const STRIPS=5u;
const STRIP_WIDTH=28u;
const ROWS=200u;
// The stars' first colour (read from the code).
const STAR_BASE=192u;
// greetings::Stage.
const STAGE_SETUP=0.0;
const STAGE_WORDS=1.0;
const STAGE_WHITE=2.0;
const STAGE_MONSTER=3.0;
const STAGE_PART8=4.0;

// The part-7 block for this invocation.
var<private> gc:GreetingsBlock;
fn greetings_begin() {gc=u.greetings;}

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates -------------------------------------------------------------
// As in parts 6, 5, 3 and 4:
// - source: the original's 320 x 200 pixels, y down.
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre: x = (source x - 160) * 5/6, y = 100 - source y.
fn screen_aspect()->f32 {return u.screen.x/u.screen.y;}
fn source_of_scene(q:vec2f)->vec2f {return vec2f(160.0+q.x/COLUMN,100.0-q.y);}
fn scene_of_uv(uv:vec2f)->vec2f {return (uv-.5)*vec2f(2.0*screen_aspect(),-2.0)*100.0;}
fn uv_of_scene(q:vec2f)->vec2f {return q/(vec2f(2.0*screen_aspect(),-2.0)*100.0)+.5;}
fn in_frame(s:vec2f)->bool {return all(s>=vec2f(0)) && all(s<=vec2f(320,200));}

// ---- Timing --------------------------------------------------------------------
fn greetings_seconds()->f32 {return gc.part.y;}
fn stage()->f32 {return gc.part.z;}
fn frame_now()->u32 {return u32(gc.frame.x);}
fn frame_next()->u32 {return u32(gc.frame.y);}
fn frame_mix()->f32 {return gc.frame.z;}
fn shows_monster()->bool {return stage()>STAGE_MONSTER-.5;}

// ---- Video memory ------------------------------------------------------------------
fn plane_byte(p:u32,a:u32)->u32 {
    let i=p*65536u+(a&65535u);
    return (planes[i>>2u]>>(8u*(i&3u)))&255u;
}
// Strip s's row y's source address in frame j.
fn shown_at(j:u32,s:u32,y:u32)->u32 {
    let i=j*1000u+s*200u+y;
    return (shown[i>>1u]>>(16u*(i&1u)))&0xffffu;
}
// The colour index frame j shows at source pixel (x, y): the monster, or
// the sky with the strips' rows from their sources.
fn index_at(j:u32,x:u32,y:u32)->u32 {
    if shows_monster() {return textureLoad(monster,vec2u(x,y),0).r;}
    if x>=8u && y<ROWS {
        let s=(x-8u)/64u;
        let k=x-8u-s*64u;
        if s<STRIPS && k<STRIP_WIDTH {
            return plane_byte(k&3u,shown_at(j,s,y)+k/4u);
        }
    }
    return plane_byte(x&3u,y*80u+x/4u);
}

// ---- Colour ---------------------------------------------------------------------------
fn srgb_decode(c:vec3f)->vec3f {return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));}
fn srgb_encode(c:vec3f)->vec3f {
    let x=clamp(c,vec3f(0),vec3f(1));
    return select(x*12.92,1.055*pow(x,vec3f(1.0/2.4))-.055,x>vec3f(.0031308));
}
// Palette entry `index` as the shown frame shows it, toward the next's
// when `blend`, linear.
fn entry(index:u32,blend:bool)->vec3f {
    let a=srgb_decode(textureLoad(palettes,vec2u(index,frame_now()),0).rgb);
    let b=srgb_decode(textureLoad(palettes,vec2u(index,frame_next()),0).rgb);
    return mix(a,b,select(0.0,frame_mix(),blend));
}
// The shared tonemap (post.wgsl's ACES fit), and the HDR value it turns
// into c, for c below 1.
fn tonemap(c:vec3f)->vec3f {return clamp((c*(2.51*c+.03))/(c*(2.43*c+.59)+.14),vec3f(0),vec3f(1));}
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}

// ---- The star colours as a light ---------------------------------------------
// greetings::SWEEP_CENTRE and the fitted turn: a star's colour index is
// about LAW_OFFSET less 64 / 2 pi times its angle round the centre (source
// pixels, y down). The cycle lights index i with the pulse at (i + phase)
// mod 64: max(0, 3 min(j, 63 - j) - 33) / 60 (read from the code).
const SWEEP_CENTRE=vec2f(160.0,85.0);
const LAW_OFFSET=17.8;
fn pulse(j:f32)->f32 {
    let w=j-64.0*floor(j/64.0);
    return max(0.0,3.0*min(w,63.0-w)-33.0)/60.0;
}
// The pulse at a continuous colour index and phase.
fn star_light(index:f32,phase:f32)->f32 {return pulse(index+phase);}
// The index the fitted turn gives a direction (source pixels, y down).
fn index_of_angle(a:f32)->f32 {return -a*32.0/PI+LAW_OFFSET;}
