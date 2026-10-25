// Part 4 (Plasma): bindings, coordinates and helpers shared by faithful HD
// (plasma_faithful.wgsl) and the modern view (plasma_light.wgsl).
// plasma_render.rs builds each module from the shared controls, this file
// and its own.
//
// Where the values come from (hd/plasma.rs has the details):
// - Read from the port's Plasma scene: the phases, every raster's start
//   address (the stride table and its offsets, the line compare), the index
//   field, the palette's rotation and level, the bank, the logo and its
//   palette, part 7's first frame.
// - Interpretation: interpolation between the original's 70 Hz frames; the
//   field and the palette's sine at continuous positions; the even and odd
//   rasters as two layers (modern).
// - Authored: everything plasma_light.wgsl adds.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl;
// part 4's share is u.plasma, a PlasmaBlock, read through `pc`.

// Every frame's 400 raster start addresses, two to a word (the lower
// first): frame j's raster r is word 200 j + r / 2.
@group(0) @binding(1) var<storage,read> rasters:array<u32>;
// The scene's sine table, 1024 a turn, amplitude 1023.
@group(0) @binding(2) var<storage,read> sine:array<f32>;
// The palette each frame shows, 256 x frames, the 6-bit DAC values
// expanded to 8 bits as the scanout does (sRGB-encoded).
@group(0) @binding(3) var palettes:texture_2d<f32>;
// The bank, 1024 x 64: entry j of level l, times 4 / 255 (so 6-bit values
// are exact).
@group(0) @binding(4) var bank:texture_2d<f32>;
// The logo's indices, 320 x 180.
@group(0) @binding(5) var logo:texture_2d<u32>;
// Part 7's first frame, 320 x 200 (sRGB-encoded).
@group(0) @binding(6) var part7:texture_2d<f32>;
// The original frame for the comparison modes, 320 x 400 (part 7's frame
// fills the upper half).
@group(0) @binding(7) var original:texture_2d<f32>;
// Linear, clamped at the edges.
@group(0) @binding(8) var linear:sampler;

const PI=3.14159265;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// plasma::FIELD_START, FIELD_ROW, FIELD_ROWS, PLASMA_RASTERS,
// PALETTE_PERIOD and LOGO_ROWS, read from the code.
const FIELD_START=14400.0;
const FIELD_ROW=200.0;
const FIELD_ROWS=223.0;
const PLASMA_RASTERS=226u;
const PALETTE_PERIOD=224.0;
const LOGO_ROWS=180.0;

// The part-4 block for this invocation.
var<private> pc:PlasmaBlock;
fn plasma_begin() {pc=u.plasma;}

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates -------------------------------------------------------------
// As in parts 6, 5 and 3:
// - screen: xy from -1 (bottom) to 1 (top), x scaled by the drawable's
//   aspect; the 4:3 frame spans x in [-4/3, 4/3].
// - source: the original's 320 x 200 pixels, y down (part 4 shows 400
//   rasters: raster r covers rows r / 2 to (r + 1) / 2).
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre: x = (source x - 160) * 5/6, y = 100 - source y.
fn screen_aspect()->f32 {return u.screen.x/u.screen.y;}
fn source_of_scene(q:vec2f)->vec2f {return vec2f(160.0+q.x/COLUMN,100.0-q.y);}
fn scene_of_uv(uv:vec2f)->vec2f {return (uv-.5)*vec2f(2.0*screen_aspect(),-2.0)*100.0;}
fn in_frame(s:vec2f)->bool {return all(s>=vec2f(0)) && all(s<=vec2f(320,200));}

// ---- Timing --------------------------------------------------------------------
fn plasma_seconds()->f32 {return pc.part.y;}
fn plasma_phase()->f32 {return pc.part.z;}
fn frame_now()->u32 {return u32(pc.frame.x);}
fn frame_next()->u32 {return u32(pc.frame.y);}
fn frame_mix()->f32 {return pc.frame.z;}
fn part7_shown()->bool {return pc.logo.w>.5;}
fn field_written()->bool {return plasma_phase()>1.5;}

// ---- The field ------------------------------------------------------------------
// Raster r's start address in frame j.
fn raster_start(j:u32,r:u32)->f32 {
    let word=rasters[j*200u+r/2u];
    return f32(select(word&0xffffu,word>>16u,(r&1u)==1u));
}
// Raster r's start, between the shown frame and the next when they are
// close (a stride that wraps, or the logo's line compare, jumps; those are
// shown as they come).
fn raster_at(r:u32)->f32 {
    let a=raster_start(frame_now(),r);
    let b=raster_start(frame_next(),r);
    return select(a,mix(a,b,frame_mix()),abs(b-a)<48.0);
}
// The scene's sine at a continuous index, linear between its entries.
fn sine_at(i:f32)->f32 {
    let k=floor(i);
    let a=sine[u32(k)&1023u];
    let b=sine[u32(k+1.0)&1023u];
    return mix(a,b,i-k);
}
// The palette phase (0 to 224, colour 32 on) the field holds at a
// continuous address, or -1 outside it. The scene writes column x of every
// row (x 0 to 199) as (sine(x * 5.1) / 10 + 200) mod 224, plus one a row;
// here at a continuous column, without the integer division.
fn field_phase(address:f32)->f32 {
    let m=address-FIELD_START;
    if m<0.0 || m>=FIELD_ROW*FIELD_ROWS {return -1.0;}
    let row=floor(m/FIELD_ROW);
    let column=m-row*FIELD_ROW;
    let phase=sine_at(column*5.1)/10.0+200.0+row;
    return phase-PALETTE_PERIOD*floor(phase/PALETTE_PERIOD);
}
// ---- Colour ---------------------------------------------------------------------------
fn srgb_decode(c:vec3f)->vec3f {return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));}
// 6-bit DAC values (continuous) as the scanout shows them, linear light.
fn dac_linear(v:vec3f)->vec3f {return srgb_decode((v*4.0+floor(v/16.0))/255.0);}
// The HDR value the shared tonemap (post.wgsl's ACES fit) turns into c,
// for c below 1.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}
// The palette's rotation of each channel (palette entries), and its level.
fn rotations()->vec3f {
    let r=pc.palette.x;
    return vec3f(r,r+pc.palette.z,r+pc.palette.w);
}
fn wrap224(x:vec3f)->vec3f {return x-PALETTE_PERIOD*floor(x/PALETTE_PERIOD);}
// The plasma's colour at palette phase p from the bank, as the scene fills
// colour 32 + p (6-bit DAC values), at the interpolated rotation and level.
fn bank_colour(p:f32)->vec3f {
    let x=wrap224(rotations()+p);
    let level=clamp(pc.palette.y,0.0,63.0);
    let y=(level+.5)/64.0;
    let r=textureSampleLevel(bank,linear,vec2f((x.r+.5)/1024.0,y),0.0).r;
    let g=textureSampleLevel(bank,linear,vec2f((x.g+.5)/1024.0,y),0.0).r;
    let b=textureSampleLevel(bank,linear,vec2f((x.b+.5)/1024.0,y),0.0).r;
    return vec3f(r,g,b)*255.0/4.0;
}
// Palette entry `index` as the shown frame shows it (and the next, when
// they blend), linear.
fn shown_entry(index:u32)->vec3f {
    let a=srgb_decode(textureLoad(palettes,vec2u(index,frame_now()),0).rgb);
    let b=srgb_decode(textureLoad(palettes,vec2u(index,frame_next()),0).rgb);
    return mix(a,b,select(0.0,frame_mix(),pc.frame.w>.5));
}
// The logo's colour at a continuous logo position (column, row), linear,
// between its pixels' colours.
fn logo_colour(p:vec2f)->vec3f {
    let q=p-.5;
    let k=floor(q);
    let f=q-k;
    var c=vec3f(0);
    for(var dy=0;dy<2;dy++) {
        for(var dx=0;dx<2;dx++) {
            let at=vec2i(k)+vec2i(dx,dy);
            var colour=vec3f(0);
            if at.x>=0 && at.y>=0 && at.x<320 && f32(at.y)<LOGO_ROWS {
                colour=shown_entry(textureLoad(logo,at,0).r);
            }
            let w=select(1.0-f.x,f.x,dx==1)*select(1.0-f.y,f.y,dy==1);
            c+=w*colour;
        }
    }
    return c;
}
