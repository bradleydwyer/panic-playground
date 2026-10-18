// What part 5's window shows: the zoom author's file (with field_zoom.rs).
// field_render.rs builds the modern module from the shared controls,
// field_common.wgsl, field_frame.wgsl, this file, field_mountain.wgsl and
// field_style.wgsl. The level kernel that builds the levels is a module of
// its own, in field_zoom.rs (KERNEL_WGSL).
//
// A deep zoom into the Mandelbrot set, onto a minibrot of period 3341 and
// size 1.8e-55 (the target is authored; field_zoom.rs holds it). It moves
// with the port's own zoom: the depth is field_zoom_level(), the original's
// log2 magnification read from its scale and swaps, so each doubling, the
// brake and the stop fall on the original's frames. The window holds the
// landed view behind the rising mountain until it is cleared, then is dark.
//
// The method follows the original's own: Field resamples a level buffer at 1
// to 2 buffer pixels per window pixel and builds the next level, twice as
// fine, a few rows a frame, swapping at each 2x. Here each level k is the
// exact view at depth k (field_zoom.rs builds it in bands with its kernel:
// perturbation from one reference orbit, f32 floatexp deltas, bilinear
// approximation and rebasing), and the window resamples the level of the
// current doubling, shading each sample here. Prefix this file's own names
// with zoom_.
//
// Group 2 (field_zoom.rs, ZoomLevels): the level the window shows, one texel
// per (band phase, log2 distance estimate in texels, class), and its view.
// (Binding 0 is the shared controls' in this repo's files, so these start
// at 1.)
@group(2) @binding(1) var zoom_level:texture_2d<f32>;
struct ZoomView {
    // the level's depth k (doublings from the start), texels per window
    // height at its own depth, its width and height in texels
    level:vec4f,
    // 1 when a complete level is bound
    state:vec4f,
}
@group(2) @binding(2) var<uniform> zoom_view:ZoomView;

// ---- The look (authored; the owner's to judge) --------------------------------------
// Distances are in pixels of the window at 4K (1080 rows), so the look is the
// same at every resolution. Colours are linear HDR radiance (the shared bloom
// starts at 1, then the ACES tonemap).
// Overall brightness, below 1 so the window doesn't overpower the dark
// gunmetal frame round it.
const ZOOM_GAIN=0.45;
// The exterior's shade follows the distance estimate on a log scale: 0 at
// ZOOM_SHADE_FAR pixels from the set, 1 at ZOOM_SHADE_NEAR, raised to
// ZOOM_SHADE_GAMMA. A log scale keeps dense lace (most of it closer to the
// set than a pixel) from saturating white.
const ZOOM_SHADE_FAR=30.0;
const ZOOM_SHADE_NEAR=1e-4;
const ZOOM_SHADE_GAMMA=1.5;
// The palette along the shade: navy, a deep blue (the original window's
// colour), azure, cyan, and a white-cyan above 1, which blooms.
const ZOOM_NAVY=vec3f(.0035,.0090,.0420);
const ZOOM_BLUE=vec3f(.0100,.0330,.2400);
const ZOOM_AZURE=vec3f(.0200,.0900,.6500);
const ZOOM_CYAN=vec3f(.0900,.3200,1.1000);
const ZOOM_WHITE=vec3f(.4500,.8000,1.8000);
const ZOOM_STOPS=vec4f(.30,.58,.82,1.0);
// The glowing boundary line: its colour, width and strength, divided by its
// own density within ZOOM_CORE_SPREAD pixels (times ZOOM_CORE_DENSITY), so
// isolated filaments glow fully and dense lace stays textured.
const ZOOM_CORE=vec3f(.35,.70,1.70);
const ZOOM_CORE_WIDTH=0.6;
const ZOOM_CORE_GAIN=0.9;
const ZOOM_CORE_SPREAD=5.0;
const ZOOM_CORE_DENSITY=6.0;
// Faint smooth iteration bands far from the set (their period is the
// kernel's ZOOM_BAND_PERIOD, below), from 6 to 46 pixels away.
const ZOOM_BAND=vec3f(.006,.020,.110);
const ZOOM_BAND_GAIN=0.5;
// The minibrot's inside, near black; samples the kernel left at its step cap
// lie on the boundary and take the cyan.
const ZOOM_INSIDE=vec3f(.0015,.0025,.0100);
// Samples per window pixel, each way.
const ZOOM_SAMPLES=2;

fn zoom_palette(x:f32)->vec3f {
    if x<ZOOM_STOPS.x {return mix(ZOOM_NAVY,ZOOM_BLUE,x/ZOOM_STOPS.x);}
    if x<ZOOM_STOPS.y {return mix(ZOOM_BLUE,ZOOM_AZURE,(x-ZOOM_STOPS.x)/(ZOOM_STOPS.y-ZOOM_STOPS.x));}
    if x<ZOOM_STOPS.z {return mix(ZOOM_AZURE,ZOOM_CYAN,(x-ZOOM_STOPS.y)/(ZOOM_STOPS.z-ZOOM_STOPS.y));}
    return mix(ZOOM_CYAN,ZOOM_WHITE,clamp((x-ZOOM_STOPS.z)/(ZOOM_STOPS.w-ZOOM_STOPS.z),0.0,1.0));
}

// The texel of the shown level at window point w (window heights from its
// centre, y up), magnified z times.
fn zoom_texel(w:vec2f,z:f32)->vec4f {
    let size=zoom_view.level.zw;
    let t=vec2f(w.x,-w.y)/z*zoom_view.level.y+.5*size;
    return textureLoad(zoom_level,clamp(vec2i(floor(t)),vec2i(0),vec2i(size)-1),0);
}
// log2 of one texel in 4K-window pixels, at magnification z.
fn zoom_texel_log2(z:f32)->f32 {return log2(1080.0/(zoom_view.level.y*z));}
// One sample: colour and the unnormalised core.
fn zoom_sample(t:vec4f,lt:f32)->vec4f {
    if t.z>1.5 {return vec4f(ZOOM_CYAN,0.0);}
    if t.z>.5 {return vec4f(ZOOM_INSIDE,0.0);}
    let lg=t.y+lt;                           // log2 distance, 4K pixels
    let d=exp2(lg);
    let span=log2(ZOOM_SHADE_FAR)-log2(ZOOM_SHADE_NEAR);
    let sh=pow(clamp((log2(ZOOM_SHADE_FAR)-lg)/span,0.0,1.0),ZOOM_SHADE_GAMMA);
    let far=clamp((d-6.0)/40.0,0.0,1.0);
    let band=.5+.5*cos(2.0*PI*t.x);
    let c=zoom_palette(sh)+ZOOM_BAND*(ZOOM_BAND_GAIN*band*far);
    let x=d/ZOOM_CORE_WIDTH;
    return vec4f(c,exp(-x*x));
}
fn zoom_core_at(t:vec4f,lt:f32)->f32 {
    if t.z>.5 {return 0.0;}
    let x=exp2(t.y+lt)/ZOOM_CORE_WIDTH;
    return exp(-x*x);
}

// The deep view at window point w (window heights, y up), over one window
// pixel of `pixel` window heights: linear HDR.
fn zoom_view_at(w:vec2f,pixel:f32)->vec3f {
    // The port's magnification (log2), held once the zoom stops; the shown
    // level is the doubling it is in, magnified the rest of the way.
    let z=exp2(clamp(field_zoom_level()-zoom_view.level.x,0.0,1.0));
    let lt=zoom_texel_log2(z);
    var c=vec3f(0);
    var core=0.0;
    for(var j=0;j<ZOOM_SAMPLES;j++) {
        for(var i=0;i<ZOOM_SAMPLES;i++) {
            let o=(vec2f(f32(i),f32(j))+.5)/f32(ZOOM_SAMPLES)-.5;
            let s=zoom_sample(zoom_texel(w+o*pixel,z),lt);
            c+=s.rgb;core+=s.a;
        }
    }
    let n=f32(ZOOM_SAMPLES*ZOOM_SAMPLES);
    c/=n;core/=n;
    // The core's local density, from a 3 x 3 spread round the pixel.
    let spread=ZOOM_CORE_SPREAD/1080.0;
    var dens=0.0;
    for(var j=-1;j<=1;j++) {
        for(var i=-1;i<=1;i++) {
            dens+=zoom_core_at(zoom_texel(w+vec2f(f32(i),f32(j))*spread,z),lt);
        }
    }
    dens/=9.0;
    return (c+ZOOM_CORE*(ZOOM_CORE_GAIN*core/(1.0+ZOOM_CORE_DENSITY*dens)))*ZOOM_GAIN;
}

fn zoom_layer(px:FieldPixel)->FieldLayer {
    let s=px.source;
    let cover=window_cover(s,px.pixel);
    if cover<=0.0 {return no_layer();}
    // Dark and empty once the original clears the window, as part 3 zooms
    // into it; and the stand-in should no level be bound.
    if window_cleared() || zoom_view.state.x<.5 {
        return field_layer(tonemap_inverse(window_content(s)),1.0,LAYER_WINDOW_Z);
    }
    // Window heights from its centre (source (160, 96)), y up; a target pixel
    // is px.pixel source rows, a hundredth of the window's height each.
    let w=(scene_of_source(s)-vec2f(0,4))/100.0;
    var c=zoom_view_at(w,px.pixel/100.0);
    // Part 6's fade brings the window in with the frame.
    c*=pow(clamp(picture_level(),0.0,1.0),2.2);
    return field_layer(c,cover,LAYER_WINDOW_Z);
}
fn zoom_prepass(px:FieldPixel)->vec4f {return vec4f(0);}
