// Part 6 (Rings): bindings, coordinates and helpers shared by every part-6
// module: faithful HD (rings_faithful.wgsl) and the style, the pool
// (rings_pool.wgsl; four more were tried and removed, and remain in the Git
// history). A style module is the shared controls, this file,
// wall.wgsl, the style's file and rings_style.wgsl; see rust/README.md,
// "Adding a part-6 style".
//
// Where the values come from (hd/rings.rs has the details):
// - Read from the port's Rings scene: the ring window, the layers' window
//   offsets (and so their centres), the ring intensity, the lit symbol and
//   its frames, the PSTART fade level and the part's phases.
// - Decoded locally from the data at load time: the 16 symbol masks, M.DAT's
//   band geometry and the PSTART picture.
// - Authored: the 7 new symbol sets (rings_symbols.txt) that the neon shows
//   in the middle of the part in place of the original masks; their runs are
//   made into neon tubes as signs are (rings::SymbolFields::authored).
// - Interpretation: the analytic band model fitted to M.DAT (98% of its
//   pixels), smooth outlines and centrelines of the masks, interpolation
//   between the original's 70 Hz frames and the symbols' averaged flicker.
// - Authored: the neon tubes, their light and everything a style adds.
// The uniform block `u: Controls` (binding 0) is prepended by renderer::wgsl.
@group(0) @binding(1) var symbol_atlas:texture_2d_array<f32>;
@group(0) @binding(2) var neon_light:texture_2d_array<f32>;
// The original frame, for faithful HD's comparison modes.
@group(0) @binding(3) var reference:texture_2d<f32>;
// Linear, with linear mipmap filtering; clamps at the edges.
@group(0) @binding(4) var linear:sampler;
// PSTART at its full palette: 320 x 200, sRGB-encoded 6-bit DAC values.
@group(0) @binding(5) var picture:texture_2d<f32>;
// The style's own half-resolution prepass (StyleInfo::prepass); 1 x 1 black
// without one, and while the prepass itself renders.
@group(0) @binding(6) var prepass:texture_2d<f32>;
// The authored symbol sets (layers 16 and on, one texture layer each from
// 0): only the tubes' fields, the centreline and free-end distances, and a
// light map texel per 2 x 2 pixels (rings::SymbolDetail::Tubes).
@group(0) @binding(7) var authored_atlas:texture_2d_array<f32>;
@group(0) @binding(8) var authored_light:texture_2d_array<f32>;

const PI=3.14159265;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const COLUMN=5.0/6.0;
// rings::RING_WINDOW, read from the code: the 256 x 184 pixels at screen
// (32, 8) that Rings::step copies the rings into. Outside it the original
// is black, apart from the symbols.
const RING_WINDOW=vec4f(32,8,288,192);
// rings::SYMBOL_SCALE (atlas texels per source pixel), rings::SYMBOL_LIMIT
// (where the atlas distances clamp, in rows) and rings::NEON_RADIUS (the
// authored tube radius the light map was built with, in rows).
const SYMBOL_SCALE=2.0;
const SYMBOL_LIMIT=12.0;
const NEON_RADIUS=3.8;
// rings::ORIGINAL_SYMBOLS (layers 0 to 15 are the original's masks, the rest
// the authored sets), rings::NEON_STROKE (the strokes' half-width, rows) and
// the log2 of rings::TUBE_LIGHT_SCALE (the authored light map's coarser
// texels, as mip levels).
const ORIGINAL_SYMBOLS=16;
const NEON_STROKE=2.2;
const TUBE_LIGHT_LOD=1.0;
// The default camera, as the scroller's: the eye 550 rows in front of the
// screen plane z = 0, where the 4:3 frame is 200 rows high. Styles may use
// their own.
const EYE_Z=550.0;
const EYE=vec3f(0,0,EYE_Z);

struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn fullscreen(@builtin(vertex_index) index:u32)->Vertex {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[index];
    return Vertex(vec4f(p,0,1),vec2f(p.x*.5+.5,.5-p.y*.5));
}

// ---- Coordinates -------------------------------------------------------------
// Three frames of reference, all continuous:
// - screen: xy from -1 (bottom) to 1 (top), x scaled by the drawable's
//   aspect; the 4:3 frame spans x in [-4/3, 4/3].
// - source: the original's 320 x 200 pixels, y down; pixel (i, j) covers
//   [i, i + 1) x [j, j + 1). Continues beyond the frame on wide screens.
// - scene: source rows on the 4:3 display, y up, origin at the frame's
//   centre, z towards the viewer: x = (source x - 160) * 5/6, y = 100 -
//   source y. Isotropic on the display, so use it for 3D.
struct RingsPixel {
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
fn rings_pixel(v:Vertex)->RingsPixel {
    let xy=(v.uv-.5)*vec2f(2.0*screen_aspect(),-2.0);
    let scene=xy*100.0;
    let source=source_of_scene(scene);
    return RingsPixel(v.uv,xy,source,scene,max(fwidth(source.y),1e-4));
}

// ---- Timing and fades --------------------------------------------------------
// Seconds since part 6 began (42.23 s into the show); its phase: 0 waiting
// for 06:00 (black), 1 rings and symbols, 2 the PSTART fade.
fn rings_seconds()->f32 {return u.rings.y;}
fn rings_phase()->f32 {return u.rings.w;}
// The rings' palette intensity, 0..1: off until 50.33 s, up over 64 frames
// by 51.23, down from 72.99 to 73.89 s. Interpolated between frames.
fn ring_intensity()->f32 {return u.ring_fades.x;}
// The most the rings have been lit so far, 0..1: 0 before the fade-in, up
// with it, then 1 to the end of the part, through the fade-out. For a style
// whose scene stays once the rings have shown it.
fn ring_peak()->f32 {return u.ring_fades.z;}
// The PSTART palette level, 0..1 (63/64 at most), from 80.36 s.
fn picture_level()->f32 {return u.ring_fades.y;}
// Authored music cues measured from the clip's PCM, each 0..1, as the intro
// and the scroller use them (clip::audio_cues): energy, onset, low band and
// accent; the scroller's relative level, bass and treble onsets. They are not
// tracker events. musical_lighting() is 1 when the viewer chose musical
// lighting (L, --scroller-lighting), 0 for studio. The original's own cues
// are the symbol switches below, on its tracker rows.
fn cue_energy()->f32 {return u.audio.x;}
fn cue_onset()->f32 {return u.audio.y;}
fn cue_low()->f32 {return u.audio.z;}
fn cue_accent()->f32 {return u.audio.w;}
fn cue_level()->f32 {return u.scroll_light.x;}
fn cue_bass()->f32 {return u.scroll_light.z;}
fn cue_treble()->f32 {return u.scroll_light.w;}
fn musical_lighting()->f32 {return u.scroll_light.y;}

// ---- Rings -------------------------------------------------------------------
// Two layers of the same concentric bands. Layer 0 is plane 0 (index bit 0,
// grey 48, its phase -28 per frame at 9/10 of the orbit), layer 1 plane 1
// (bit 1, grey 32, +35 per frame): opposite turns of about 2.09 and 1.67 s.
// Their centres come from the frames' window offsets and the band model
// fitted to M.DAT; each band is u.ring_bands.x (7) pixels wide and the first
// edge, round a clear centre, at u.ring_bands.y (about 6.7).
// The bands are circles in source pixels, so the original shows them 1.2
// times taller than wide. RING_FAITHFUL measures that way; RING_ROUND
// measures on the display, so the same bands look round (authored).
const RING_FAITHFUL=vec2f(1,1);
const RING_ROUND=vec2f(COLUMN,1);
// A layer's centre in source pixels: shutter 0 now, 1 at the shutter's
// opening, about 1/100 s earlier, for motion blur.
fn ring_centre(layer:u32,shutter:f32)->vec2f {
    let now=select(u.ring_centres.xy,u.ring_centres.zw,layer==1u);
    let open=select(u.ring_open.xy,u.ring_open.zw,layer==1u);
    return mix(now,open,shutter);
}
fn ring_width()->f32 {return u.ring_bands.x;}
fn ring_first()->f32 {return u.ring_bands.y;}
// Distance from a layer's centre, in band pixels (see the metrics above).
fn ring_radius(layer:u32,s:vec2f,metric:vec2f,shutter:f32)->f32 {
    return length((s-ring_centre(layer,shutter))*metric);
}
// Signed distance from radius r to the nearest band edge, in band pixels:
// positive inside a set band (one the original draws), negative in a clear
// one, half the band width at a band's middle.
fn band_distance(r:f32)->f32 {
    if r<ring_first() {return r-ring_first();}
    let w=ring_width();
    let q=r-ring_first()-.5*w;
    return .5*w-abs(q-2.0*w*floor(q/(2.0*w)+.5));
}
fn ring_band(layer:u32,s:vec2f,metric:vec2f,shutter:f32)->f32 {
    return band_distance(ring_radius(layer,s,metric,shutter));
}
// Coverage of a layer's set bands, antialiased over `blur` band pixels (a
// pixel's footprint, say).
fn ring_cover(layer:u32,s:vec2f,metric:vec2f,shutter:f32,blur:f32)->f32 {
    return clamp(ring_band(layer,s,metric,shutter)/max(blur,1e-4)+.5,0.0,1.0);
}
// Position across the bands: 0 at the first edge, 1 per period (a set and a
// clear band), so fract() < .5 is inside a set band.
fn ring_phase(layer:u32,s:vec2f,metric:vec2f,shutter:f32)->f32 {
    return (ring_radius(layer,s,metric,shutter)-ring_first())/(2.0*ring_width());
}
// Unit direction away from a layer's centre, in source axes (y down).
fn ring_dir(layer:u32,s:vec2f,metric:vec2f,shutter:f32)->vec2f {
    let d=(s-ring_centre(layer,shutter))*metric;
    return d/max(length(d),1e-4);
}
// The original's ring window, 1 inside, antialiased over `blur` pixels.
fn ring_window(s:vec2f,blur:f32)->f32 {
    let inside=min(min(s.x-RING_WINDOW.x,RING_WINDOW.z-s.x),min(s.y-RING_WINDOW.y,RING_WINDOW.w-s.y));
    return clamp(inside/max(blur,1e-4)+.5,0.0,1.0);
}
// A 6-bit DAC value (0..63, continuous) in linear light, as the scanout
// expands it (v * 4 + v / 16 of 255, then sRGB).
fn dac_linear(v:f32)->f32 {
    let c=clamp(v*(4.0+1.0/16.0)/255.0,0.0,1.0);
    return select(c/12.92,pow((c+.055)/1.055,2.4),c>.04045);
}
// The original's palette for the ring bits, in linear light: grey 48 where
// only layer 0 is set, grey 32 where only layer 1 is, black where both or
// neither are, scaled by the intensity in DAC units as the palette is. a, b:
// each layer's coverage, treated as independent.
fn ring_xor(a:f32,b:f32)->vec3f {
    let i=ring_intensity();
    return vec3f(a*(1.0-b)*dac_linear(48.0*i)+b*(1.0-a)*dac_linear(32.0*i));
}

// ---- Symbols -----------------------------------------------------------------
// The 16 masks Rings::new decodes from M.SUM are atlas layers 0 to 15. The
// lit one changes on the original's tracker rows, about every 8 frames: the
// selector toggles every second row and the page every second toggle. The
// original lights it in yellow (63, 63, random 0..31) on even frames only.
// Layers 16 and on are the authored sets, 16 each (set k's drawing of mask n
// at 16 + 16 k + n), which the neon shows in place of the originals in the
// middle of the part; rings::window_layer picks the layer. Faithful HD
// always draws the original masks.
// Atlas layout: u.symbol_atlas and u.authored_atlas hold their origins and
// sizes in source pixels.
// Returns the outline's signed distance (positive inside the stroke), the
// distance to the stroke's centreline, the stroke's half-width there and the
// distance to the nearest free end of a stroke's centreline, all in source
// rows (scene units). Beyond the atlas, or with no layer (-1), the distances
// read as at least SYMBOL_LIMIT. The authored sets keep only the centreline
// and free-end distances; their strokes are taken as NEON_STROKE round the
// runs, so the half-width reads as that and the outline as that less the
// centreline distance.
fn symbol_field(layer:i32,s:vec2f)->vec4f {
    let far=vec4f(-SYMBOL_LIMIT,SYMBOL_LIMIT,0,SYMBOL_LIMIT);
    if layer<0 {return far;}
    if layer<ORIGINAL_SYMBOLS {
        let t=(s-u.symbol_atlas.xy)/u.symbol_atlas.zw;
        if any(t<vec2f(0)) || any(t>vec2f(1)) {return far;}
        return textureSampleLevel(symbol_atlas,linear,t,layer,0.0);
    }
    let t=(s-u.authored_atlas.xy)/u.authored_atlas.zw;
    if any(t<vec2f(0)) || any(t>vec2f(1)) {return far;}
    let f=textureSampleLevel(authored_atlas,linear,t,layer-ORIGINAL_SYMBOLS,0.0).rg;
    return vec4f(max(NEON_STROKE-f.x,-SYMBOL_LIMIT),f.x,select(0.0,NEON_STROKE,f.x<SYMBOL_LIMIT),f.y);
}
// The original's average symbol tint in linear light: (63, 63, b) with b
// uniformly random in 0..31, the mean of b's linear light (read from the
// palette code; the average is an interpretation of its flicker).
const SYMBOL_YELLOW=vec3f(1,1,.0683);
// The atlas layers the neon draws (-1 for none): the lit symbol's and the
// one lit before it. In the middle of the part they are an authored set's
// drawings of the original's masks; elsewhere, or with --original-symbols,
// the original masks themselves.
fn neon_mask()->i32 {return i32(u.symbol_layers.x);}
fn neon_previous()->i32 {return i32(u.symbol_layers.y);}
// The original's lit mask and the one lit before it (-1 for none), which
// are also their atlas layers: for faithful HD, which always shows them.
fn original_mask()->i32 {return i32(u.symbol.x);}
fn original_previous()->i32 {return i32(u.symbol.y);}
// Seconds since the lit window began (or since the last one ended, with
// none lit), and until it ends (-1 with none lit). The windows are taken a
// frame ahead, so the first is negative, down to -1/70, through the frame
// before a switch.
fn neon_since()->f32 {return u.symbol.z;}
fn neon_until()->f32 {return u.symbol.w;}
// How lit each symbol is, 0..1: the new one comes on over the frame before
// its first lit frame, as the old one goes off (an authored cross-fade; the
// original switches between frames), so at each frame's own time the HD
// views show that frame's symbol. Steady in between: they show the
// original's 35 Hz flicker as its average.
fn neon_on()->f32 {
    if u.symbol.x<0.0 {return 0.0;}
    return clamp(u.symbol.z*70.0+1.0,0.0,1.0);
}
fn neon_previous_on()->f32 {
    if u.symbol.y<0.0 {return 0.0;}
    return 1.0-clamp(u.symbol.z*70.0+1.0,0.0,1.0);
}
// Authored neon tubes, in an amber yellow warmer than the original's lemon
// (the owner's direction: lit neon in glass tubes, after photographs of neon
// signs; nothing from them is reproduced). Each stroke is a glass tube of
// NEON_RADIUS (rows) around its centreline, with a column of glowing gas
// filling the middle NEON_COLUMN of it: white hot down the middle once
// tonemapped, where a line of sight crosses the most gas, and saturated at
// its edge. The glass around the column is clear, carries a little of its
// light and gathers a bright line along each wall, and a lamp above left
// catches it. Each tube is a single run: where strokes meet or cross, one
// passes and the others stop short with a gap, and no run closes a loop
// (rings::SymbolFields); a run glows to its rounded ends, dimming a little
// where it turns into the backing board, its electrodes behind the sign.
// NEON_ELECTRODE, in tube radii, shows dark electrodes at the ends instead.
// Round the tubes, their light scattered in the air: a haze that follows
// their shape.
const NEON_GAS=vec3f(1.0,.52,.05);
const NEON_ELECTRODE=0.0;
const NEON_COLUMN=.55;   // The gas column's radius, as a share of the glass's.
const NEON_WALL=1.3;    // The column's radius at its edge, times NEON_GAS.
const NEON_HOT=7.0;     // Added along its centre line.
// The glass between column and wall: its own glow and the share of the
// scene behind it that it lets through, tinted.
const NEON_GLASS_GLOW=.12;
const NEON_GLASS_TINT=vec3f(.80,.72,.62);
// The light the lit tubes cast, as the radiance of an area light covering
// the glass (about the mean across it).
const NEON_LIGHT=NEON_GAS*1.8;
// The haze's strength at two widths, about 2 and 8 rows.
const NEON_HAZE=vec2f(.30,.16);
// One sample of the tubes: the light they add, how much glass covers the
// pixel, and where (an offset in source pixels) the scene behind is seen
// through that glass. Compose with neon_over.
struct NeonGlass { light:vec3f, glass:f32, bend:vec2f }
// A tube seen at distance d (rows) from its centreline, `end` rows from the
// nearest free end of its stroke, with a pixel footprint in rows. `across`
// is the unit direction, in scene axes, from the centreline towards this
// point, for the glass's shading as a cylinder; vec2f(0) leaves its
// highlight and refraction out.
fn neon_tube(d:f32,end:f32,across:vec2f,footprint:f32)->NeonGlass {
    let x=min(d/NEON_RADIUS,1.0);
    let glass=clamp((NEON_RADIUS-d)/max(footprint,1e-3)+.5,0.0,1.0);
    // The last stretch dims a little as the tube turns into the backing.
    let lit=select(smoothstep(.25,1.0,end/NEON_RADIUS/NEON_ELECTRODE),1.0,NEON_ELECTRODE<=0.0)
        *mix(.55,1.0,smoothstep(0.0,1.5,end/NEON_RADIUS));
    // The gas column, with a soft edge.
    let c=x/NEON_COLUMN;
    let chord=sqrt(max(1.0-c*c,0.0));
    let column=clamp((1.0-c)*NEON_RADIUS*NEON_COLUMN/max(footprint,1e-3)+.5,0.0,1.0);
    let gas=NEON_GAS*(NEON_WALL*mix(exp(-(c-1.0)*5.0)*.35,1.0,column)+NEON_HOT*pow(chord,6.0));
    // The electrode: dark metal where the column would be.
    let electrode=vec3f(.03,.027,.024)*column;
    // The glass: its glow, and a line along each wall.
    let wall=smoothstep(.8,.92,x)*(1.0-smoothstep(.95,1.0,x));
    var light=mix(electrode,gas,lit)+NEON_GAS*(NEON_GLASS_GLOW+.5*wall)*mix(.35,1.0,lit);
    // A highlight from a lamp above left, on the glass as a cylinder.
    let normal=vec3f(across*x,sqrt(1.0-x*x));
    let half_way=normalize(normalize(vec3f(-.45,.6,.65))+vec3f(0,0,1));
    let shown=step(1e-3,dot(across,across));
    light+=vec3f(1,.97,.9)*pow(max(dot(normal,half_way),0.0),80.0)*1.2*shown;
    // Through the glass the scene shifts outwards, more towards the wall.
    let bend=vec2f(across.x*COLUMN,-across.y)*x*NEON_RADIUS*.6;
    return NeonGlass(light*glass,glass*mix(1.0,.35,max(column*lit,0.0)),bend);
}
// One layer's tube at source point s.
fn neon_tube_at(layer:i32,s:vec2f,footprint:f32)->NeonGlass {
    let f=symbol_field(layer,s);
    if f.y>NEON_RADIUS+footprint {return NeonGlass(vec3f(0),0.0,vec2f(0));}
    let e=.5;
    let g=vec2f(symbol_field(layer,s+vec2f(e,0)).y-symbol_field(layer,s-vec2f(e,0)).y,
        symbol_field(layer,s+vec2f(0,e)).y-symbol_field(layer,s-vec2f(0,e)).y);
    // Source axes to scene axes: columns are 5/6 of a row; y turns up.
    let across=vec2f(g.x/COLUMN,-g.y);
    let n=select(vec2f(0),across/max(length(across),1e-5),length(across)>1e-4);
    return neon_tube(f.y,f.w,n,footprint);
}
// Tube distance (rows) to a layer's centreline at source point s.
fn tube_distance(layer:i32,s:vec2f)->f32 {return symbol_field(layer,s).y;}
// The lit tubes at source point s, current and previous symbol together.
fn neon_glass(s:vec2f,footprint:f32)->NeonGlass {
    var n=NeonGlass(vec3f(0),0.0,vec2f(0));
    for(var k=0;k<2;k++) {
        let layer=select(neon_mask(),neon_previous(),k==1);
        let on=select(neon_on(),neon_previous_on(),k==1);
        if on>0.0 {
            let t=neon_tube_at(layer,s,footprint);
            n.light+=t.light*on;
            n.bend+=t.bend*t.glass*on;
            n.glass=max(n.glass,t.glass*on);
        }
    }
    return n;
}
// The tubes over the scene: `behind` is the scene at the pixel, `seen` the
// scene at source point s + bend, through the glass.
fn neon_over(behind:vec3f,seen:vec3f,n:NeonGlass)->vec3f {
    return mix(behind,seen*NEON_GLASS_TINT,n.glass)+n.light;
}
// The light the lit tubes add at source point s, for a style that draws
// nothing behind them through the glass.
fn neon_emission(s:vec2f,footprint:f32)->vec3f {return neon_glass(s,footprint).light;}
// The haze round the lit tubes at source point s, as radiance.
fn neon_haze(s:vec2f)->vec3f {
    var h=0.0;
    for(var k=0;k<2;k++) {
        let layer=select(neon_mask(),neon_previous(),k==1);
        let on=select(neon_on(),neon_previous_on(),k==1);
        if on>0.0 {h+=(neon_map(layer,s,2.0)*NEON_HAZE.x+neon_map(layer,s,8.0)*NEON_HAZE.y)*on;}
    }
    return NEON_GAS*h;
}
// Neon light map of one layer: the share of the area around source point s
// covered by its tubes, blurred over about `sigma` source pixels (the mip
// chain of rings::SymbolFields::light; trilinear, so roughly Gaussian).
// The authored sets' maps start a level coarser, so below about 1.2 pixels
// they are blurrier than asked.
fn neon_map(layer:i32,s:vec2f,sigma:f32)->f32 {
    if layer<0 {return 0.0;}
    let t=(s-u.neon_map.xy)/u.neon_map.zw;
    let lod=log2(max(sigma/.6,1.0));
    if layer<ORIGINAL_SYMBOLS {return textureSampleLevel(neon_light,linear,t,layer,lod).r;}
    return textureSampleLevel(authored_light,linear,t,layer-ORIGINAL_SYMBOLS,max(lod-TUBE_LIGHT_LOD,0.0)).r;
}
// Light from one layer's tubes, lying in the plane z = height (scene), at
// scene point p with unit normal n. Approximate: the tubes are an area light
// of radiance NEON_LIGHT; the irradiance from the plane is pi times the
// share of it the point sees within about the distance to it (the light map
// blurred that far), arriving from the tubes' centroid there (found from
// the map's slope).
fn neon_light_one(layer:i32,p:vec3f,n:vec3f,height:f32)->vec3f {
    let dz=height-p.z;
    let reach=abs(dz)+NEON_RADIUS;
    let sigma=.75*reach;
    let s=source_of_scene(p.xy);
    let f=neon_map(layer,s,sigma);
    if f<1e-5 {return vec3f(0);}
    let e=.5*sigma;
    let g=vec2f(neon_map(layer,s+vec2f(e,0),sigma)-neon_map(layer,s-vec2f(e,0),sigma),
        neon_map(layer,s+vec2f(0,e),sigma)-neon_map(layer,s-vec2f(0,e),sigma))/(2.0*e);
    // The centroid's offset in source pixels, then in scene axes.
    let offset=clamp(g*sigma*sigma/f,vec2f(-3.0*sigma),vec2f(3.0*sigma));
    let l=normalize(vec3f(offset.x*COLUMN,-offset.y,select(-reach,reach,dz>=0.0)));
    return NEON_LIGHT*PI*f*max(dot(n,l),0.0);
}
// The lit symbols' light on a surface: scene point p, unit normal n, tubes in
// the plane z = height.
fn neon_irradiance(p:vec3f,n:vec3f,height:f32)->vec3f {
    var c=vec3f(0);
    if neon_on()>0.0 {c+=neon_light_one(neon_mask(),p,n,height)*neon_on();}
    if neon_previous_on()>0.0 {c+=neon_light_one(neon_previous(),p,n,height)*neon_previous_on();}
    return c;
}
// Light arriving at scene point p from every direction (no surface), for
// haze and volumes: the same estimate without the cosine.
fn neon_fluence(p:vec3f,height:f32)->vec3f {
    let reach=abs(height-p.z)+NEON_RADIUS;
    let s=source_of_scene(p.xy);
    let f=neon_map(neon_mask(),s,.75*reach)*neon_on()+neon_map(neon_previous(),s,.75*reach)*neon_previous_on();
    return NEON_LIGHT*PI*f;
}
// Where a ray from scene point p along unit r meets the tubes' plane z =
// height: the scene point and the distance, or w = -1 if it never does.
fn neon_plane_hit(p:vec3f,r:vec3f,height:f32)->vec4f {
    let t=(height-p.z)/r.z;
    if abs(r.z)<1e-5 || t<=0.0 {return vec4f(0,0,0,-1);}
    return vec4f(p+r*t,t);
}
// The lit tubes as a reflection shows them: the radiance arriving at scene
// point p along unit direction r, from tubes in the plane z = height, blurred
// by a rough surface (roughness: the spread in radians, about). For a sharp
// mirror, neon_emission at neon_plane_hit's point is crisper.
fn neon_reflection(p:vec3f,r:vec3f,height:f32,roughness:f32)->vec3f {
    let hit=neon_plane_hit(p,r,height);
    if hit.w<0.0 {return vec3f(0);}
    let s=source_of_scene(hit.xy);
    let sigma=max(roughness*hit.w,.6);
    let f=neon_map(neon_mask(),s,sigma)*neon_on()+neon_map(neon_previous(),s,sigma)*neon_previous_on();
    return NEON_LIGHT*f;
}

// ---- PSTART ------------------------------------------------------------------
// The picture at source point s, at palette level `level` (0..1) applied in
// DAC units as the fade does, in linear light. Catmull-Rom between its
// pixels, in sRGB as the DAC values are, clamped: an authored smoothing of
// the raster. Black outside the frame.
fn picture_color(s:vec2f,level:f32)->vec3f {
    if any(s<vec2f(0)) || any(s>vec2f(320,200)) {return vec3f(0);}
    let q=s-.5;let i=floor(q);let t=q-i;
    let w0=t*(-.5+t*(1.0-.5*t));let w1=1.0+t*t*(-2.5+1.5*t);
    let w2=t*(.5+t*(2.0-1.5*t));let w3=t*t*(-.5+.5*t);
    var c=vec3f(0);
    for(var y=0;y<4;y++) {
        let wy=select(select(select(w3.y,w2.y,y==2),w1.y,y==1),w0.y,y==0);
        for(var x=0;x<4;x++) {
            let wx=select(select(select(w3.x,w2.x,x==2),w1.x,x==1),w0.x,x==0);
            let at=clamp(vec2i(i)+vec2i(x-1,y-1),vec2i(0),vec2i(319,199));
            c+=textureLoad(picture,at,0).rgb*wx*wy;
        }
    }
    c=clamp(c*level,vec3f(0),vec3f(1));
    return select(c/12.92,pow((c+.055)/1.055,vec3f(2.4)),c>vec3f(.04045));
}

// ---- Output ------------------------------------------------------------------
// The HDR value the shared tonemap (post.wgsl's ACES fit) turns into the
// display-linear colour c, for c below 1: lets a style show original
// colours as they were, before adding its own light. Bloom still adds to it.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}
// The style's half-resolution prepass at a target uv.
fn prepass_at(uv:vec2f)->vec4f {return textureSampleLevel(prepass,linear,uv,0.0);}
fn hash2(p:vec2f)->f32 {return fract(sin(dot(p,vec2f(127.1,311.7)))*43758.5453);}

// A plain modern rendering of the original, for the stubs and as a starting
// point: the XOR moiré in the original window, antialiased, and the lit
// symbols as neon tubes, in HDR (authored).
fn rings_reference(px:RingsPixel)->vec3f {
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let s=px.source;
    let blur=px.pixel;
    let window=ring_window(s,blur);
    let a=ring_cover(0u,s,RING_FAITHFUL,0.0,blur)*window;
    let b=ring_cover(1u,s,RING_FAITHFUL,0.0,blur)*window;
    return tonemap_inverse(ring_xor(a,b))+neon_emission(s,px.pixel)+neon_haze(s);
}
