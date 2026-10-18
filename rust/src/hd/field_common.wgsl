// Part 5 (Field): bindings, coordinates and helpers shared by every part-5
// module: faithful HD (field_faithful.wgsl) and the modern view, which is
// the three authors' files (field_frame.wgsl, field_zoom.wgsl and
// field_mountain.wgsl) composited by field_style.wgsl. See rust/README.md,
// "Adding to part 5".
//
// Where the values come from (hd/field.rs has the details):
// - Read from the port's Field scene: the window, the zoom's scale, speed
//   and swaps and its two field buffers, the terrain level, the camera, the
//   projection's q, the window clear and the palettes.
// - Decoded locally from the data at load time: the frame picture, the
//   prepared terrain (DEP.U and MAP.U as `prepare` leaves them) and the sine
//   table behind the camera.
// - Observed at load time: the phases' times (u.field.events).
// - Interpretation: the Mandelbrot fit of DEP.U and the relief baked from it,
//   the camera's formulas without their integer truncation, interpolation
//   between the original's 70 Hz frames, the per-pixel march that stands in
//   for the original's 160 rays, and smooth resampling of its rasters.
// - Authored: everything the three authors' files add.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl;
// part 5's share is u.field, a FieldBlock. Read it through `fc` and the
// helpers below, which the fluid step shares with the fragments.

// The frame's palette: 256 x 1, the 6-bit DAC values expanded to 8 bits as
// the scanout does (sRGB-encoded), linearly filtered between entries.
@group(0) @binding(1) var palette:texture_2d<f32>;
// The frame picture Field::new makes from PSTART: 320 x 200 indices / 255.
@group(0) @binding(2) var picture:texture_2d<f32>;
// The zoom's field buffers, 256 x 200 values / 255 (1..127, WELLPAL
// indices): the one the window shows, and the next frame's.
@group(0) @binding(3) var field_buffer_now:texture_2d<f32>;
@group(0) @binding(4) var field_buffer_next:texture_2d<f32>;
// The prepared terrain, 200 x 200 (u map y, v map x): r the colour index,
// g the height, each / 255.
@group(0) @binding(5) var terrain:texture_2d<f32>;
// The highest height in each COARSE_CELL-unit cell, with a margin for
// bilinear sampling, / 255 (u cell y, v cell x): r the original terrain's,
// g the relief's.
@group(0) @binding(6) var coarse:texture_2d<f32>;
// The relief baked from the Mandelbrot fit (u map y, v map x): height (the
// prepared scale, 0..244), its gradient per map unit along x and y, and the
// distance outside the set in map units.
@group(0) @binding(7) var relief:texture_2d<f32>;
// The original frame, for faithful HD's comparison modes.
@group(0) @binding(8) var reference:texture_2d<f32>;
// Linear, clamped at the edges.
@group(0) @binding(9) var linear:sampler;
// Each author's half-resolution prepass (AuthorInfo::prepass); 1 x 1 black
// without one, and while the prepasses render.
@group(0) @binding(10) var prepass_frame:texture_2d<f32>;
@group(0) @binding(11) var prepass_zoom:texture_2d<f32>;
@group(0) @binding(12) var prepass_mountain:texture_2d<f32>;
// The fluid (field_render.rs): its latest state, which the fluid step reads
// as the state before it; the one before that, for drawing only; the step's
// own controls; and the state the step writes.
@group(1) @binding(1) var fluid_state:texture_2d<f32>;
@group(1) @binding(2) var fluid_previous:texture_2d<f32>;
@group(1) @binding(3) var<uniform> fluid_step:FluidStep;
@group(1) @binding(4) var fluid_out:texture_storage_2d<rgba16float,write>;

// One fluid step's controls: the part-5 block at the step's start, and the
// step (number, fluid seconds at its start, seconds per step, 1 for the
// initial state).
struct FluidStep { block:FieldBlock, step:vec4f }

const PI=3.14159265;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// field::FIELD_WINDOW, read from the code: the 128 x 100 window at screen
// (96, 46) that the zoom draws into and the terrain step clears.
const FIELD_WINDOW=vec4f(96,46,224,146);
// field::CLEAR_INDEX: the window's colour once cleared.
const CLEAR_INDEX=129;
// field::MAP_SIZE (the terrain map's side, map units) and
// field::COARSE_CELL (map units per coarse cell).
const MAP_SIZE=200.0;
const COARSE_CELL=8.0;
// field::SET_ITERATIONS, POTENTIAL_ITERATIONS and BAILOUT: the fit's set
// and potential.
const SET_ITERATIONS=100;
const POTENTIAL_ITERATIONS=600;
const BAILOUT=1e8;
// The rays' depths, read from `ray`: from 64 to 512 map units.
const NEAR=64.0;
const FAR=512.0;
// The column the camera looks along. `ray` casts a ray every second column
// with (column - 160) and draws it on that column and the next; read as
// continuous, a ray's two columns centre on 161 more than its offset.
const COLUMN_CENTRE=161.0;
// The default camera, as the scroller's and part 6's: the eye 550 rows in
// front of the screen plane z = 0, where the 4:3 frame is 200 rows high.
const EYE_Z=550.0;
const EYE=vec3f(0,0,EYE_Z);
// Default layer depths for the compositor (scene z, towards the viewer): the
// frame in the screen plane, the window's content behind it and the
// mountain in front, as the original draws them. Authored.
const LAYER_FRAME_Z=0.0;
const LAYER_WINDOW_Z=-60.0;
const LAYER_MOUNTAIN_Z=100.0;

// The part-5 block for this invocation: the fragment entries copy u.field,
// the fluid step its own block.
var<private> fc:FieldBlock;
fn field_begin() {fc=u.field;}

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates -------------------------------------------------------------
// The same three frames of reference as part 6's, all continuous:
// - screen: xy from -1 (bottom) to 1 (top), x scaled by the drawable's
//   aspect; the 4:3 frame spans x in [-4/3, 4/3].
// - source: the original's 320 x 200 pixels, y down; pixel (i, j) covers
//   [i, i + 1) x [j, j + 1). Continues beyond the frame on wide screens.
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre, z towards the viewer: x = (source x - 160) * 5/6, y = 100 -
//   source y. Isotropic on the display, so use it for 3D.
// And the terrain's map: map point (x, y), 0 to 200 each, is DEP.U and
// MAP.U's row x and column y; cell (i, j) covers [i, i + 1) x [j, j + 1).
struct FieldPixel {
    uv:vec2f,      // 0..1 across the target, y down
    xy:vec2f,      // screen
    source:vec2f,  // source, where the default camera's ray meets z = 0
    scene:vec2f,   // scene, on z = 0
    pixel:f32,     // one target pixel in source rows (any resolution)
}
fn screen_aspect()->f32 {return u.screen.x/u.screen.y;}
fn source_of_scene(q:vec2f)->vec2f {return vec2f(160.0+q.x/COLUMN,100.0-q.y);}
fn scene_of_source(s:vec2f)->vec2f {return vec2f((s.x-160.0)*COLUMN,100.0-s.y);}
// The default camera's ray through a screen point (unit length).
fn camera_ray(xy:vec2f)->vec3f {return normalize(vec3f(xy*100.0,-EYE_Z));}
// Fragments only: the footprint takes a derivative.
fn field_pixel(v:Vertex)->FieldPixel {
    let xy=(v.uv-.5)*vec2f(2.0*screen_aspect(),-2.0);
    let scene=xy*100.0;
    let source=source_of_scene(scene);
    return FieldPixel(v.uv,xy,source,scene,max(fwidth(source.y),1e-4));
}
// Inside the original's 320 x 200 frame.
fn in_frame(s:vec2f)->bool {return all(s>=vec2f(0)) && all(s<=vec2f(320,200));}

// ---- Timing ------------------------------------------------------------------
// Seconds since part 5 began (81.27 s into the show; negative before, when
// part 6's fade or the fluid's early steps read the block); its phase, 0
// zoom, 1 terrain; and the part-5 frame, continuous.
fn field_seconds()->f32 {return fc.part.y;}
fn field_phase()->f32 {return fc.part.z;}
fn field_frame()->f32 {return fc.part.w;}
// The zoom. The original resamples its 256 x 200 buffer at a scale of 1 to
// 2 buffer pixels per window pixel, lowering it each frame by the speed
// (from 16 to 1024 in 65536ths, at full speed one level every 64 frames);
// when it passes 1 the buffers swap to the next level, twice as fine. The
// scale is in the shown buffer's pixels, interpolated between frames: below
// 1 when the next frame swaps (field_zoom_ahead), where the next level shows at
// twice it. The level is log2 of the magnification since the first zoom
// frame, continuous; field_zoom_phase its fraction, 0 as a level begins.
fn field_zoom_scale()->f32 {return fc.zoom.x;}
fn field_zoom_ahead()->bool {return fc.zoom.y>.5;}
fn field_zoom_level()->f32 {return fc.zoom.z;}
fn field_zoom_phase()->f32 {return fract(field_zoom_level());}
fn field_zoom_swaps()->f32 {return fc.zoom.w;}
// Speed / 1024; rows of the next level built (0..100); running (1 from the
// first zoom frame until it stops); shown (0 on part 5's first frame, which
// still shows PSTART's own window).
fn field_zoom_speed()->f32 {return fc.zoom_state.x;}
fn field_zoom_rows()->f32 {return fc.zoom_state.y;}
fn field_zoom_running()->bool {return fc.zoom_state.z>.5;}
fn field_zoom_shown()->bool {return fc.zoom_state.w>.5;}
// The terrain's camera, by the code's formulas without their integer
// truncation: map position, height (height units, the code's), angle in
// radians (unwrapped), the map directions of its view and its right (about
// unit length), the orbit radius (the code's 70 + sin / 12) and the distance
// from the map centre. Before the terrain, the first terrain step's.
fn camera_map()->vec2f {return fc.camera.xy;}
fn camera_height()->f32 {return fc.camera.z;}
fn camera_angle()->f32 {return fc.camera.w*2.0*PI/1024.0;}
fn camera_forward()->vec2f {return fc.view.xy/256.0;}
fn camera_right()->vec2f {return vec2f(fc.view.y,-fc.view.x)/256.0;}
fn orbit_radius()->f32 {return fc.view.z;}
fn camera_distance()->f32 {return fc.view.w;}
// The terrain level: 0 to 64 as it rises (1 a frame), falling as it sinks.
// The camera's height is 3 radius + 470 - 5 level.
fn terrain_level()->f32 {return fc.mountain.x;}
// The projection's q, 0 (flat) to 248 (perspective), and its table: rows
// per 1024 height units at depth z. The code builds it from the level of
// the step before (8 level, until that passes 255).
fn projection_q()->f32 {return fc.mountain.y;}
fn projection(z:f32)->f32 {
    let q=projection_q();
    return q*256.0/max(z,1.0)+512.0-2.0*q;
}
// The continuous terrain step, from 0.
fn terrain_frame()->f32 {return fc.mountain.z;}
// Source row (y down) at which height h shows at depth z, by `ray`'s
// projection: rows from the bottom, (h - height) projection(z) / 1024 + 200.
fn terrain_row(z:f32,h:f32)->f32 {return (camera_height()-h)*projection(z)/1024.0;}
// The map direction of source column `column`'s ray, per unit of depth, and
// its map point at depth z: `ray`'s endpoints, the camera's position plus z
// along its view and (column - 160) z / 256 along its right, read as
// continuous.
fn terrain_dir(column:f32)->vec2f {return camera_forward()+(column-COLUMN_CENTRE)/256.0*camera_right();}
fn map_point(column:f32,z:f32)->vec2f {return camera_map()+terrain_dir(column)*z;}
// The window: cleared to colour 129 (0 or 1, on its frame), seconds since
// (-1 before); the PSTART level (1 in part 5; part 6's fade before it); and
// whether the original shows terrain pixels (0 or 1, interpolated).
fn window_cleared()->bool {return fc.window.x>.5;}
fn cleared_seconds()->f32 {return fc.window.y;}
fn picture_level()->f32 {return fc.window.z;}
fn terrain_shown()->f32 {return fc.window.w;}
// Part-5 seconds of its events, observed from the frames when the preview
// loads (with the imported data: 0.90, 14.79, 15.69 and 16.84; 16.59, 40.56,
// 46.42 and 48.76): the zoom at full speed, braking (its last fall, after a
// dip of 16 at 95.13 s), the terrain's first frame and the window cleared;
// risen to level 64, sinking, the last terrain pixel and the end, when part
// 3 takes the frame.
fn time_full_speed()->f32 {return fc.events.x;}
fn time_braking()->f32 {return fc.events.y;}
fn time_terrain()->f32 {return fc.events.z;}
fn time_cleared()->f32 {return fc.events.w;}
fn time_risen()->f32 {return fc.events_late.x;}
fn time_sinking()->f32 {return fc.events_late.y;}
fn time_last_terrain()->f32 {return fc.events_late.z;}
fn time_end()->f32 {return fc.events_late.w;}
// Authored music cues measured from the clip's PCM, each 0..1, as the
// earlier parts use them (clip::audio_cues); not tracker events. They are
// the displayed moment's, so the fluid step must not use them.
fn cue_energy()->f32 {return u.audio.x;}
fn cue_onset()->f32 {return u.audio.y;}
fn cue_low()->f32 {return u.audio.z;}
fn cue_accent()->f32 {return u.audio.w;}

// ---- Colour --------------------------------------------------------------------
fn srgb_decode(c:vec3f)->vec3f {return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));}
// A palette entry as the scanout shows it: sRGB-encoded, then linear.
fn palette_srgb(i:i32)->vec3f {return textureLoad(palette,vec2i(clamp(i,0,255),0),0).rgb;}
fn palette_entry(i:i32)->vec3f {return srgb_decode(palette_srgb(i));}
// The palette at a continuous index, linear between neighbouring entries in
// DAC units, then linear light: a smooth ramp through the original's.
fn palette_color(index:f32)->vec3f {
    return srgb_decode(textureSampleLevel(palette,linear,vec2f((clamp(index,0.0,255.0)+.5)/256.0,.5),0.0).rgb);
}
// The HDR value the shared tonemap (post.wgsl's ACES fit) turns into the
// display-linear colour c, for c below 1: shows original colours as they
// were. Bloom still adds to it.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}
fn hash2(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}
// Catmull-Rom weights of the four taps round a point t of the way past the
// second.
fn cr_weights(t:f32)->vec4f {
    return vec4f(t*(-.5+t*(1.0-.5*t)),1.0+t*t*(-2.5+1.5*t),t*(.5+t*(2.0-1.5*t)),t*t*(-.5+.5*t));
}

// ---- The window and the picture ---------------------------------------------------
fn window_rect()->vec4f {return FIELD_WINDOW;}
fn in_window(s:vec2f)->bool {return all(s>=FIELD_WINDOW.xy) && all(s<FIELD_WINDOW.zw);}
// The window, 1 inside, antialiased over `blur` source pixels.
fn window_cover(s:vec2f,blur:f32)->f32 {
    let inside=min(min(s.x-FIELD_WINDOW.x,FIELD_WINDOW.z-s.x),min(s.y-FIELD_WINDOW.y,FIELD_WINDOW.w-s.y));
    return clamp(inside/max(blur,1e-4)+.5,0.0,1.0);
}
// Source point s in the window's own pixels, 0..128 by 0..100.
fn window_point(s:vec2f)->vec2f {return s-FIELD_WINDOW.xy;}
fn picture_index(p:vec2i)->i32 {
    return i32(round(textureLoad(picture,clamp(p,vec2i(0),vec2i(319,199)),0).r*255.0));
}
// Keep a tap on s's side of the window's edge: inside for a point inside,
// and for a point outside, on the nearest outside row or column.
fn keep_side(p:vec2i,s:vec2f,inside:bool)->vec2i {
    let lo=vec2i(FIELD_WINDOW.xy);
    let hi=vec2i(FIELD_WINDOW.zw)-1;
    if inside {return clamp(p,lo,hi);}
    if any(p<lo) || any(p>hi) {return p;}
    var q=p;
    if s.x<FIELD_WINDOW.x {q.x=lo.x-1;}
    else if s.x>=FIELD_WINDOW.z {q.x=hi.x+1;}
    else if s.y<FIELD_WINDOW.y {q.y=lo.y-1;}
    else {q.y=hi.y+1;}
    return q;
}
// The frame picture at source point s through the current palette, at
// palette level `level` (0..1, in DAC units as part 6's fade), in linear
// light. Catmull-Rom between its pixels in sRGB, as the DAC values are,
// clamped; the window's edge is kept sharp, so the frame never smears into
// the window or the window into the frame (an authored smoothing). Black
// outside the 320 x 200 frame.
fn picture_color(s:vec2f,level:f32)->vec3f {
    if !in_frame(s) {return vec3f(0);}
    let q=s-.5;let i=vec2i(floor(q));let t=q-floor(q);
    var wx=cr_weights(t.x);var wy=cr_weights(t.y);
    let inside=in_window(s);
    var c=vec3f(0);
    for(var y=0;y<4;y++) {
        for(var x=0;x<4;x++) {
            let p=keep_side(clamp(i+vec2i(x-1,y-1),vec2i(0),vec2i(319,199)),s,inside);
            c+=palette_srgb(picture_index(p))*wx[x]*wy[y];
        }
    }
    return srgb_decode(clamp(c*level,vec3f(0),vec3f(1)));
}

// The point on the frame's side of the window's edge nearest source point
// s: s itself outside the window, and inside it, just past its nearest edge.
fn picture_frame_side(s:vec2f)->vec2f {
    if !in_window(s) {return s;}
    let d=vec4f(s.x-FIELD_WINDOW.x,FIELD_WINDOW.z-s.x,s.y-FIELD_WINDOW.y,FIELD_WINDOW.w-s.y);
    let m=min(min(d.x,d.y),min(d.z,d.w));
    if m==d.x {return vec2f(FIELD_WINDOW.x-1e-3,s.y);}
    if m==d.y {return vec2f(FIELD_WINDOW.z,s.y);}
    if m==d.z {return vec2f(s.x,FIELD_WINDOW.y-1e-3);}
    return vec2f(s.x,FIELD_WINDOW.w);
}
// The frame round the window at source point s: the picture as seen from
// the frame's side, for antialiasing its edge against the window's content.
fn picture_frame(s:vec2f,level:f32)->vec3f {return picture_color(picture_frame_side(s),level);}

// ---- The zoom -------------------------------------------------------------------
// A zoom buffer's value (1..127) at buffer point b (columns 0..256, rows
// 0..200, continuous; pixel centres at half units), Catmull-Rom between its
// pixels: `next` for the next frame's buffer.
fn field_zoom_buffer_value(b:vec2f,next:bool)->f32 {
    let q=b-.5;let i=vec2i(floor(q));let t=q-floor(q);
    var wx=cr_weights(t.x);var wy=cr_weights(t.y);
    var v=0.0;
    for(var y=0;y<4;y++) {
        for(var x=0;x<4;x++) {
            let p=clamp(i+vec2i(x-1,y-1),vec2i(0),vec2i(255,199));
            var raw:f32;
            if next {raw=textureLoad(field_buffer_next,p,0).r;} else {raw=textureLoad(field_buffer_now,p,0).r;}
            v+=raw*255.0*wx[x]*wy[y];
        }
    }
    return v;
}
// The zoom's buffer point for source point s: the window's centre (64, 50)
// shows the buffer's centre (128, 100), as `zoom` draws it, at the scale
// round it; and whether that is the next frame's buffer, at twice the scale.
fn field_zoom_point(s:vec2f)->vec3f {
    var scale=field_zoom_scale();
    var next=0.0;
    if field_zoom_ahead() && scale<1.0 {scale*=2.0;next=1.0;}
    return vec3f(vec2f(128,100)+(window_point(s)-vec2f(64,50))*scale,next);
}
// The field's value at source point s, smoothly resampled.
fn field_zoom_value(s:vec2f)->f32 {
    let b=field_zoom_point(s);
    return field_zoom_buffer_value(b.xy,b.z>.5);
}
// The field in the original's palette (WELLPAL: dark at 64, bright at both
// extremes), linear between its entries.
fn field_zoom_color(s:vec2f)->vec3f {return palette_color(clamp(field_zoom_value(s),1.0,127.0));}
// What the original's window shows at source point s, smoothly: PSTART's own
// window on part 5's first frame, the zoom, still once it stops, then colour
// 129 once the window is cleared. Linear light.
fn window_content(s:vec2f)->vec3f {
    if window_cleared() {return palette_entry(CLEAR_INDEX);}
    if field_zoom_shown() {return field_zoom_color(s);}
    return picture_color(clamp(s,FIELD_WINDOW.xy,FIELD_WINDOW.zw-1e-3),picture_level());
}

// ---- The terrain ------------------------------------------------------------------
// A height at map point m: the original's prepared terrain (bilinear between
// its cells' centres) or the relief; 0 beyond the map.
fn terrain_height(m:vec2f,from_relief:bool)->f32 {
    if any(m<vec2f(0)) || any(m>vec2f(MAP_SIZE)) {return 0.0;}
    let t=m.yx/MAP_SIZE;
    if from_relief {return textureSampleLevel(relief,linear,t,0.0).r;}
    return textureSampleLevel(terrain,linear,t,0.0).g*255.0;
}
// The original terrain's colour at map point m: its colour index (bilinear)
// through the palette, MAP.PAL's ramp during the terrain.
fn terrain_color(m:vec2f)->vec3f {
    return palette_color(textureSampleLevel(terrain,linear,m.yx/MAP_SIZE,0.0).r*255.0);
}
fn coarse_height(cell:vec2i,from_relief:bool)->f32 {
    let t=textureLoad(coarse,clamp(cell.yx,vec2i(0),vec2i(24)),0);
    return select(t.r,t.g,from_relief)*255.0;
}
// What a pixel's ray meets: how much of the pixel the terrain covers (0..1,
// antialiased along its top edge), the map point and depth of the nearest
// surface the pixel's centre sees (or, uncovered, of the point nearest to
// covering it) and its height.
struct TerrainHit { cover:f32, map:vec2f, z:f32, height:f32 }
// March source point s's ray through a height field (the original terrain,
// or with from_relief the fitted relief) with the port's camera and
// projection: the ray of column s.x, from depth NEAR to FAR, meets the
// terrain at the nearest depth z whose height h shows at or above row s.y
// (terrain_row(z, h) <= s.y; heights under half a unit are not drawn, as
// `ray` skips 0). The original does this once per 2 columns, in whole map
// steps; this marches each pixel a quarter map unit at a time through the
// cells whose coarse height could reach it. `footprint` is a pixel in
// source rows.
fn terrain_march(s:vec2f,footprint:f32,from_relief:bool)->TerrainHit {
    var out=TerrainHit(0.0,vec2f(0),0.0,0.0);
    let y=s.y;
    let dir=terrain_dir(s.x);
    let origin=camera_map();
    // Nothing on the map shows above the highest height at the far end.
    if y<terrain_row(FAR,255.0)-footprint {return out;}
    // The depths where the ray is over the map.
    var z0=NEAR;var z1=FAR;
    if abs(dir.x)>1e-6 {
        let a=-origin.x/dir.x;let b=(MAP_SIZE-origin.x)/dir.x;
        z0=max(z0,min(a,b));z1=min(z1,max(a,b));
    } else if origin.x<0.0 || origin.x>MAP_SIZE {return out;}
    if abs(dir.y)>1e-6 {
        let a=-origin.y/dir.y;let b=(MAP_SIZE-origin.y)/dir.y;
        z0=max(z0,min(a,b));z1=min(z1,max(a,b));
    } else if origin.y<0.0 || origin.y>MAP_SIZE {return out;}
    if z0>=z1 {return out;}
    let fine=.25/max(length(dir),1e-3);
    // The most the terrain covers the pixel by so far, in pixels, and where.
    var best=-1e9;
    var hit=false;
    var z=z0;
    var previous=z0;
    var previous_margin=-1e9;
    var cells=0;
    loop {
        if z>=z1 || cells>96 || best>=.5 {break;}
        cells++;
        let m=origin+dir*z;
        let cell=vec2i(floor(m/COARSE_CELL));
        let lo=vec2f(cell)*COARSE_CELL;
        var exit=z1;
        if abs(dir.x)>1e-6 {exit=min(exit,(select(lo.x,lo.x+COARSE_CELL,dir.x>0.0)-origin.x)/dir.x);}
        if abs(dir.y)>1e-6 {exit=min(exit,(select(lo.y,lo.y+COARSE_CELL,dir.y>0.0)-origin.y)/dir.y);}
        exit=max(exit,z+1e-3);
        // This cell can only matter if its highest point, at its far end,
        // could cover more of the pixel than anything so far.
        let top=coarse_height(cell,from_relief);
        let reach=(y-terrain_row(exit,top))/footprint;
        if top<.5 || reach<=max(best,-.5) {
            z=exit;previous=z;previous_margin=-1e9;
            continue;
        }
        loop {
            if z>=exit || best>=.5 {break;}
            let p=origin+dir*z;
            let h=terrain_height(p,from_relief);
            var margin=-1e9;
            if h>=.5 {margin=(y-terrain_row(z,h))/footprint;}
            if margin>best {
                best=margin;
                if !hit {out.map=p;out.z=z;out.height=h;}
            }
            if !hit && margin>=0.0 {
                hit=true;
                // The crossing between the last sample and this one.
                var a=previous;var b=z;
                if previous_margin<0.0 && previous<z {
                    for(var k=0;k<6;k++) {
                        let c=.5*(a+b);
                        let hc=terrain_height(origin+dir*c,from_relief);
                        if hc>=.5 && y>=terrain_row(c,hc) {b=c;} else {a=c;}
                    }
                }
                out.z=b;out.map=origin+dir*b;out.height=terrain_height(out.map,from_relief);
            }
            previous=z;previous_margin=margin;
            z+=fine;
        }
        z=max(z,exit);
    }
    out.cover=clamp(.5+best,0.0,1.0);
    return out;
}

// ---- The Mandelbrot fit -------------------------------------------------------------
// DEP.U's heights as a Mandelbrot mountain (an interpretation, fitted when
// the data loads; see field::MandelFit): its plateau is the set and its
// slopes are base + slope sqrt(G(c)), G the potential, clamped from 0 to the
// plateau. Map point m is c = ((m.y - 100) / (scale aspect) + re, (m.x -
// 100) / scale + im): the real axis runs along DEP.U's rows.
fn mandel_centre()->vec2f {return fc.mandel.xy;}
fn mandel_scale()->f32 {return fc.mandel.z;}
fn mandel_aspect()->f32 {return fc.mandel.w;}
fn mandel_base()->f32 {return fc.mandel_fit.x;}
fn mandel_slope()->f32 {return fc.mandel_fit.y;}
fn mandel_plateau()->f32 {return fc.relief.w;}
fn mandel_c(m:vec2f)->vec2f {
    return vec2f((m.y-100.0)/(mandel_scale()*mandel_aspect()),(m.x-100.0)/mandel_scale())+mandel_centre();
}
// G(c) and the iterations it took, iterating directly in f32 (bailout |z|^2
// above BAILOUT); (0, -1) if c stays bounded for `limit` iterations.
fn mandel_potential(c:vec2f,limit:i32)->vec2f {
    var z=vec2f(0);
    var power=1.0;
    for(var n=1;n<=limit;n++) {
        z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
        power*=2.0;
        let m=dot(z,z);
        if m>BAILOUT {return vec2f(.5*log(m)/power,f32(n));}
    }
    return vec2f(0,-1);
}
// The fitted DEP value at map point m by direct iteration, as the fit
// defines it: the plateau for points bounded after SET_ITERATIONS. In f32,
// so near the set's edge it can differ from the relief, baked in f64.
fn mandel_dep(m:vec2f)->f32 {
    let c=mandel_c(m);
    var z=vec2f(0);
    for(var n=0;n<SET_ITERATIONS;n++) {
        z=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y)+c;
        if dot(z,z)>4.0 {
            let g=mandel_potential(c,POTENTIAL_ITERATIONS);
            return clamp(mandel_base()+mandel_slope()*sqrt(max(g.x,0.0)),0.0,mandel_plateau());
        }
    }
    return mandel_plateau();
}
// The height `prepare` makes of a DEP value: 2 (DEP - 128), at least 0.
fn height_of_dep(dep:f32)->f32 {return 2.0*max(dep-128.0,0.0);}
fn mandel_height(m:vec2f)->f32 {return height_of_dep(mandel_dep(m));}
// The baked relief at map point m (bilinear): height, its gradient per map
// unit along map x and y, and the distance outside the set (map units).
// Zero beyond the map; relief_size() texels a side.
fn relief_at(m:vec2f)->vec4f {
    if any(m<vec2f(0)) || any(m>vec2f(MAP_SIZE)) {return vec4f(0);}
    return textureSampleLevel(relief,linear,m.yx/MAP_SIZE,0.0);
}
fn relief_height(m:vec2f)->f32 {return relief_at(m).x;}
fn relief_size()->f32 {return fc.relief.x;}
fn relief_highest()->f32 {return fc.relief.z;}

// ---- Compositing ------------------------------------------------------------------
// One layer at a pixel: its light, premultiplied by its opacity (linear HDR
// radiance; the shared bloom and tonemap follow), its opacity (0..1) and its
// depth as scene z (towards the viewer: the frame's plane is z = 0, the
// window's content behind it, the mountain in front).
struct FieldLayer { color:vec3f, alpha:f32, z:f32 }
fn no_layer()->FieldLayer {return FieldLayer(vec3f(0),0.0,-1e9);}
// Light c at opacity a and depth z.
fn field_layer(c:vec3f,a:f32,z:f32)->FieldLayer {return FieldLayer(c*a,a,z);}
// What the frame author draws: smoke behind the frame, the frame itself and
// smoke in front of it, each a layer.
struct FrameLayers { behind:FieldLayer, frame:FieldLayer, front:FieldLayer }
// The layers over black, farthest first; layers at the same depth keep the
// order given (field_style.wgsl: the window, smoke behind, the frame, smoke
// in front, the mountain).
fn field_composite(layers:array<FieldLayer,5>)->vec3f {
    var l=layers;
    for(var i=1;i<5;i++) {
        var k=i;
        loop {
            if k==0 || l[k-1].z<=l[k].z {break;}
            let t=l[k];l[k]=l[k-1];l[k-1]=t;
            k--;
        }
    }
    var c=vec3f(0);
    for(var i=0;i<5;i++) {c=c*(1.0-clamp(l[i].alpha,0.0,1.0))+l[i].color;}
    return c;
}
// The scene z of the plane the default camera's ray through a pixel meets at
// distance t from the eye: for layers that know their distance instead.
fn depth_z(xy:vec2f,t:f32)->f32 {return EYE_Z+camera_ray(xy).z*t;}

// ---- The fluid -------------------------------------------------------------------
// The fluid's grid (field::FLUID) over the source rectangle fluid_region():
// one step a 1 / rate of a second, from its start. In the fluid step,
// fluid_load and fluid_sample read the state before the step; in a
// fragment, the latest state, and fluid_blend mixes the one before it in
// for display rates above the fluid's.
struct FluidCell { texel:vec2i, uv:vec2f, source:vec2f, size:vec2i }
fn fluid_region()->vec4f {return fc.fluid_region;}
fn fluid_steps()->f32 {return fc.fluid.x;}
fn fluid_fraction()->f32 {return fc.fluid.y;}
fn fluid_dt()->f32 {return fc.fluid.z;}
fn fluid_seconds()->f32 {return fc.fluid.w;}
fn fluid_uv(s:vec2f)->vec2f {let r=fluid_region();return (s-r.xy)/(r.zw-r.xy);}
fn fluid_source(uv:vec2f)->vec2f {let r=fluid_region();return r.xy+uv*(r.zw-r.xy);}
fn fluid_size()->vec2i {return vec2i(textureDimensions(fluid_state));}
fn fluid_load(texel:vec2i)->vec4f {return textureLoad(fluid_state,clamp(texel,vec2i(0),fluid_size()-1),0);}
fn fluid_sample(uv:vec2f)->vec4f {return textureSampleLevel(fluid_state,linear,uv,0.0);}
// Fragments only.
fn fluid_blend(uv:vec2f)->vec4f {
    return mix(textureSampleLevel(fluid_previous,linear,uv,0.0),fluid_sample(uv),fluid_fraction());
}
fn fluid_cell(texel:vec2i)->FluidCell {
    let size=fluid_size();
    let uv=(vec2f(texel)+.5)/vec2f(size);
    return FluidCell(texel,uv,fluid_source(uv),size);
}
// The step's own values: its number, seconds since the fluid's start at the
// step's start, and seconds per step. The fluid step only.
fn fluid_step_number()->f32 {return fluid_step.step.x;}
fn fluid_step_seconds()->f32 {return fluid_step.step.y;}

// ---- Prepasses -------------------------------------------------------------------
fn frame_prepass_at(uv:vec2f)->vec4f {return textureSampleLevel(prepass_frame,linear,uv,0.0);}
fn zoom_prepass_at(uv:vec2f)->vec4f {return textureSampleLevel(prepass_zoom,linear,uv,0.0);}
fn mountain_prepass_at(uv:vec2f)->vec4f {return textureSampleLevel(prepass_mountain,linear,uv,0.0);}
