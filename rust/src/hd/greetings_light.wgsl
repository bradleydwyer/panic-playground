// The modern part 7: a lighthouse in dusty air, its letters cut crystal.
// Authored throughout, on the port's timing, positions and motion
// (greetings_common.wgsl), its letters' outlines from greetings_type.rs.
//
// The original turns 64 colours over a field of stars that never move, so
// a broad band of light sweeps round (160, 85) once every 64 frames; five
// strips of letters rise through it, each letter row one of 16 stored views
// of a turning column. Here the sweep is one broad cream lighthouse beam in
// a room of dusty air, its direction the original's band's peak, turning
// four times slower (authored):
// - The lamp stands a little behind the strips, where it projects on the
//   sweep's centre, a bright core with a halo, and the beam leaves its
//   aperture, of finite size, tilted a little toward the camera, so it
//   passes through the outer strips; broad and soft across, as the
//   original's band is broad.
// - The air is a participating medium marched in 3D at half resolution:
//   lit by the beam, the light falling off with the square of the
//   distance, and scattered toward the camera by a Henyey-Greenstein phase
//   function, so the beam brightens as it turns toward the lens. The lamp's
//   glow lights the haze everywhere, faintly. The air moves in slow,
//   swirling currents (the curl of sums of waves, authored), its thicker and
//   thinner regions carried with it. The beam's motion is integrated over
//   the shutter along each march.
// - The dust is motes in the air, seeded from the original's stars (each
//   star's screen position, a few motes at depths along it, around and in
//   front of the strips), each carried along its own path by the currents
//   (the original's stars never move): lit by the beam, forward scattering
//   strongly, and faintly by the lamp's glow; the near ones soft discs out
//   of focus, the far ones fine points.
// - Each letter is a block of clear cut crystal, like a lighthouse's lens,
//   the letter cut deep into its four faces as a V-groove, frosted, like an
//   engraved crystal award (the original's reading of the 16 views: the same
//   letter on each face of a turning column). The blocks are rigid, each
//   turned at its centre by the original's sine there (5.625 degrees a step
//   of the view index, continuous): the original turns each row by its own
//   step, up to 53 degrees across one letter, which as a solid would shear
//   the letter and warp its faces; rigid blocks keep the letters whole and
//   the faces flat, so they read and refract cleanly, and the stack still
//   ripples as the original's does. The cut glows softly by the lamp all the
//   time, so the words read in the dark; the clear glass is dark, reflecting
//   the lamp in glints, and refracting what lies behind with dispersion
//   (three wavelengths), the lamp seen through it split into colours.
// - When the beam passes through a block, its light is traced through the
//   crystal, each ray at one of three wavelengths, refracted by the faces
//   and the cut's facets (internally reflected where it must be), and where
//   it lands is gathered in a map of what the strips let through as the
//   lamp sees it: the block's shadow, and bright, coloured caustics shaped
//   by the cut, which light the dust and the air beyond. The cut and the
//   block's edges flare.
// - The end: the beam swings to the lens (authored motion, from a little
//   before the original's feedback fade), its forward scattering flooding
//   the picture, which the fade then blends toward white at the original's
//   timing and levels; the monster (MONSTER.U, upscaled) develops out of the
//   white at the original's levels.
// Module: the shared controls, greetings_common.wgsl and this file.

// ---- The look (authored) -------------------------------------------------------
// The camera's distance from the strips' plane (greetings::CAMERA_DISTANCE;
// source rows), the lamp's depth behind it, the beam's tilt toward the
// camera (radians), its half angle, the aperture's radius (rows), the
// penumbra's width at the aperture and its growth per row.
const CAMERA_DISTANCE=580.0;
const LAMP_DEPTH=22.0;
const BEAM_TILT=.4;
const APERTURE=2.6;
// The beam's breadth: across it, at a distance along, its light is
// exp(-BEAM_SOFTNESS q^2) where q is the distance across over APERTURE +
// along * BEAM_SPREAD (the tangent of about 29 degrees), ending smoothly by
// q = BEAM_END. The original lights 40 of its 64 colours at once, a very
// broad band, so the beam is broad and soft: about 32 degrees across at
// half its light, faintly out to some 60.
const BEAM_SPREAD=.55;
const BEAM_SOFTNESS=2.0;
const BEAM_END=1.7;
// The beam turns this many times slower than the original's colour cycle
// (authored: one turn every 256 frames, about 3.66 s, near one order of
// the music), its direction the original's band's at the fade to white.
const BEAM_SLOWING=4.0;
// The beam's power, and the distance within which its inverse-square
// falloff levels off.
const BEAM_POWER=1.0;
const FALLOFF=70.0;
// The air: its scattering, the phase function's asymmetry, its thinnest
// and thickest (relative), the noise levels between which it thickens, and
// its scale (rows a tile of the noise). The lamp's own glow scatters in it
// everywhere, this much (so the room reads as dusty air, not a void).
const SCATTER=.045;
const PHASE_G=.62;
const THINNEST=.4;
const THICKEST=1.8;
const THICKENS=vec2f(.15,.85);
const DENSITY_SCALE=170.0;
const AMBIENT_AIR=.00035;
// The air's flow (authored; the original's stars never move): a slow mean
// drift (rows a second), and lazy swirls, the curl of two sums of waves
// (so the flow neither gathers nor thins the air), large and slow, then
// smaller and quicker over the places the large ones have moved: each
// wave's direction and its period (seconds) and phase, and each sum's
// wavelength and reach (rows).
const DRIFT=vec3f(1.2,.5,-.8);
const SWIRL_LARGE=vec2f(260.0,26.0);
const SWIRL_SMALL=vec2f(90.0,7.0);
// The march: samples within the cone, the near and far depths it covers
// (scene z), and the shutter (70 Hz frames) its samples spread over.
const MARCH_SAMPLES=28u;
// The samples for the light the blocks throw out of the beam.
const SPRAY_SAMPLES=12u;
const AIR_NEAR=-420.0;
const AIR_FAR=240.0;
const SHUTTER=1.0;
// What the blocks let through, seen from the lamp: a map of directions
// round the beam's axis, as a spotlight's shadow map is, out to this half
// angle's tangent (the beam, its penumbra and its turn over the shutter);
// the blocks' depth either side of the strips' plane; the photons' grid
// (greetings_render::PHOTON_GRID); how far along the beam each photon's
// landing is taken (rows); and the brightest a caustic is let be.
const MAP_SPREAD=1.2;
const SLAB=11.5;
const PHOTON_GRID=vec2u(600u,600u);
// Each photon's splat: a tent SPLAT cells of the grid either way, weighted
// so a grid of them sums to one.
const SPLAT=3.0;
const LANDING=110.0;
const CAUSTIC_MAX=8.0;
// Where the blocks gather light (the photons landing more thickly than
// they left), it is shown this much brighter (authored: the true spray is
// spread so thin over the room's dust as to vanish).
const CAUSTIC_GAIN=8.0;
// The lamp seen directly: its core and halo (rows) and their gains; the
// flare of its aperture when the beam turns toward the camera.
const CORE=1.6;
const HALO=14.0;
const CORE_GAIN=10.0;
const HALO_GAIN=.25;
const FLARE=40.0;
// The dust: a mote's radius in focus (pixels at 1080 rows), the circle of
// confusion's scale (pixels at 1080 rows for a mote at the camera's
// distance, mirrored), its largest, its brightness in the beam, its phase
// function's asymmetry, and the faint glow of the original's band on the
// motes outside the beam.
const MOTE_RADIUS=.65;
const BOKEH=10.0;
const BOKEH_MAX=20.0;
const MOTE_GAIN=30.0;
const MOTE_G=.75;
const MOTE_BAND=.008;
// The motes' light from the lamp's glow alone, everywhere (in the beam's
// units times MOTE_GAIN).
const MOTE_AMBIENT=.1;
// The blocks: half a side (greetings_type::FACE / 2), the edges' rounding,
// the slots (greetings::SLOT_ROWS, SLOTS), and the turn per step of the
// sine's view index (90 degrees over 16, read as a turn of the column).
const HALF=7.5;
const ROUNDING=.7;
const SLOT_ROWS=18.0;
const SLOTS=16u;
const TWIST_STEP=.09817477;
const MARCH_STEPS=64u;
const LIPSCHITZ=.7;
// The letters' atlas (greetings_type::CELL, ATLAS_COLUMNS, ATLAS_SIZE).
const CELL=128.0;
const ATLAS_COLUMNS=8u;
const ATLAS_SIZE=vec2f(1024.0,512.0);
// The cut: the letter's square as a share of the face's side, the
// V-groove's slope (depth per row in from the outline) and deepest (rows).
const LETTER_SCALE=.8;
const CUT_SLOPE=2.0;
const CUT_DEPTH=2.4;
// The crystal: its refractive index at the three wavelengths (red, green,
// blue; a dense flint's dispersion, a little exaggerated), its absorption
// per row (a faint teal, from the original's letters), the reflectance at
// normal incidence, the polish (GGX alpha), and how far behind a block the
// refracted view is taken (rows).
const IOR=vec3f(1.6,1.63,1.68);
const ABSORPTION=vec3f(.012,.0035,.0045);
const F0=.05;
const POLISH=.025;
const BEHIND=160.0;
// The cut's frosting (how much of it scatters rather than refracts), its
// glow by the lamp at rest and with the beam through the block, the lamp's
// glints, the edges' and the cut's flare in the beam.
const FROST=.55;
// How much of the beam the frosted cut scatters away (so the letters cast
// their shapes into the beam).
const FROST_SCATTER=.85;
const CUT_REST=.42;
const CUT_BEAM=2.2;
const GLINT=24.0;
const EDGE_REST=.06;
const EDGE_FLARE=1.6;
const CUT_FLARE=1.2;
// What the polish mirrors: the picture behind the strips, and toward the
// camera the same picture mirrored about the strips' plane, as if it
// surrounded them (authored), dimmer; at this depth.
const FRONT_MIRROR=.5;
const MIRROR_DEPTH=220.0;
// The swing to the lens: it starts this many seconds before the original's
// fade to white (authored motion; the fade keeps its timing and levels),
// and the brightest the monster shows through the tonemap once developed
// (so it keeps its colours, near enough).
const SWING_LEAD=.45;
const MONSTER_WHITE=.9;

// ---- Bindings -----------------------------------------------------------------------
// The letters' faces (greetings_type.rs): signed distance (rows of the
// face), unused, its gradient across and up.
@group(0) @binding(8) var glyphs:texture_2d<f32>;
// The dust motes (greetings::motes): across, up, depth (scene); the star's
// colour index plus its size (0 to 1).
@group(0) @binding(9) var<storage,read> motes:array<vec4f>;
// The air's density, a tiling, smooth noise (greetings_render::noise), and
// a sampler that repeats it.
@group(0) @binding(10) var density_noise:texture_3d<f32>;
@group(0) @binding(11) var repeat:sampler;
// The passes' images: the dust in front of the blocks and all of it; the
// blocks' geometry (coverage, strip * 16 + slot, the ray's distance to
// them, and to them where they are solid, else 1e9); the strips'
// transmission seen from the lamp (1 plus what the photons add); the lit
// air in front of the blocks (its depth in a) and all of it (half size).
@group(1) @binding(1) var dust_front:texture_2d<f32>;
@group(1) @binding(2) var geometry:texture_2d<f32>;
@group(1) @binding(3) var transmission:texture_2d<f32>;
@group(1) @binding(4) var dust_full:texture_2d<f32>;
@group(1) @binding(5) var air_front:texture_2d<f32>;
@group(1) @binding(6) var air_full:texture_2d<f32>;
// The photons: per photon its landing in the map (uv), the light it
// brings there, and the light it carried (0 where it met no block), in the
// beam's units at its axis. Written by trace_photons, read by
// photon_vertex.
@group(1) @binding(7) var<storage,read_write> photons_out:array<vec4f>;
@group(1) @binding(8) var<storage,read> photons_in:array<vec4f>;

// ---- The beam -----------------------------------------------------------------------
// The stars' colour, (v, 8v/9, 7v/9) as the scanout shows it, in linear
// light at unit red.
fn star_colour()->vec3f {return srgb_decode(vec3f(1.0,8.0/9.0,7.0/9.0));}
// The star colours' intensity scales their 6-bit DAC values, which the
// display shows at about their 2.2 power: so does all the light here.
fn intensity()->f32 {return pow(clamp(gc.sweep.y,0.0,1.0),2.2);}
const CAMERA=vec3f(0.0,0.0,-CAMERA_DISTANCE);
fn project(p:vec3f)->vec2f {return p.xy*CAMERA_DISTANCE/(CAMERA_DISTANCE+p.z);}
// The lamp in camera space, placed so it projects on the sweep's centre.
fn lamp()->vec3f {
    return vec3f(0.0,(100.0-SWEEP_CENTRE.y)*(CAMERA_DISTANCE+LAMP_DEPTH)/CAMERA_DISTANCE,LAMP_DEPTH);
}
// How far the beam has swung to the lens (authored motion): it begins
// SWING_LEAD seconds before the original's fade to white and is done by the
// fade's third step; the fade itself keeps the original's timing and levels.
fn swing()->f32 {
    let white=gc.events.w;
    var lead=0.0;
    if white>=0.0 {lead=smoothstep(white-SWING_LEAD,white+.05,greetings_seconds());}
    let w=clamp(gc.sweep.z*2.5,0.0,1.0);
    return max(lead,w*w*(3.0-2.0*w));
}
// The beam's direction `back` frames before the moment: on the screen, the
// direction of the original's band's peak round the sweep's centre (the
// pulse peaks where colour index plus phase is 31.5; the index follows the
// fitted turn), turning BEAM_SLOWING times slower about the moment the fade
// to white begins (so there its direction is the original's), tilted
// toward the camera, and all the way to it as it swings to the lens.
fn beam_axis(back:f32)->vec3f {
    let anchor=gc.twist_late.z;
    let phase=anchor+(gc.sweep.x-back-anchor)/BEAM_SLOWING;
    let angle=(LAW_OFFSET-31.5+phase)*PI/32.0;
    let tilt=mix(BEAM_TILT,PI*.5,swing());
    // Source y runs down, the scene's up.
    return vec3f(cos(angle)*cos(tilt),-sin(angle)*cos(tilt),-sin(tilt));
}
// The beam's profile at x (0 outside the cone): brighter on its axis and
// soft at its edge, 1 on the axis; and its falloff with the distance.
fn beam_profile(x:vec3f,axis:vec3f)->f32 {
    let v=x-lamp();
    let along=dot(v,axis);
    if along<=0.0 {return 0.0;}
    let q=length(v-axis*along)/(APERTURE+along*BEAM_SPREAD);
    return exp(-BEAM_SOFTNESS*q*q)*(1.0-smoothstep(BEAM_END*.6,BEAM_END,q));
}
fn beam_falloff(x:vec3f)->f32 {
    let r2=dot(x-lamp(),x-lamp());
    return BEAM_POWER*FALLOFF*FALLOFF/(FALLOFF*FALLOFF+r2);
}
// The beam's light at x, the blocks ignored.
fn beam_at(x:vec3f,axis:vec3f)->f32 {return beam_profile(x,axis)*beam_falloff(x);}
// Henyey-Greenstein, for light travelling along `light` scattered toward
// `view` (both unit vectors), normalised to 1 for an even scatter.
fn phase(light:vec3f,view:vec3f,g:f32)->f32 {
    let c=dot(light,view);
    return (1.0-g*g)/pow(1.0+g*g-2.0*g*c,1.5);
}
// The map's frame: the beam's axis at the moment and two directions
// across it.
struct MapFrame { axis:vec3f, across:vec3f, up:vec3f }
fn map_frame()->MapFrame {
    let axis=beam_axis(0.0);
    var helper=vec3f(0,0,1);
    if abs(axis.z)>.9 {helper=vec3f(0,1,0);}
    let across=normalize(cross(helper,axis));
    return MapFrame(axis,across,cross(axis,across));
}
// The map's uv of the direction from the lamp to x (outside [0, 1] where x
// is off the map or behind the lamp).
fn map_uv(x:vec3f)->vec2f {
    let f=map_frame();
    let w=x-lamp();
    let along=dot(w,f.axis);
    if along<=1e-3 {return vec2f(-1);}
    return vec2f(dot(w,f.across),-dot(w,f.up))/(along*2.0*MAP_SPREAD)+.5;
}
// The direction from the lamp for a map uv.
fn map_direction(uv:vec2f)->vec3f {
    let f=map_frame();
    let t=(uv-.5)*2.0*MAP_SPREAD;
    return normalize(f.axis+f.across*t.x-f.up*t.y);
}
// The light the blocks move, toward x, by wavelength, in the beam's units
// at its axis (0 where they move none; less in their shadows, more in their
// caustics); only beyond the blocks. Softened over the aperture's penumbra
// (the aperture's radius over the blocks' distance, less over x's), two
// taps turned by `blur` (which the callers vary from sample to sample).
fn moved(x:vec3f,blur:f32)->vec3f {
    let l=lamp();
    if x.z>-SLAB || l.z-x.z<1.0 {return vec3f(0);}
    let uv=map_uv(x);
    if any(uv<vec2f(0)) || any(uv>vec2f(1)) {return vec3f(0);}
    let w=x-l;
    let blocks=length(w)*l.z/(l.z-x.z);
    let spread=APERTURE*max(1.0/max(blocks,1.0)-1.0/length(w),0.0)/(2.0*MAP_SPREAD);
    let o=vec2f(cos(blur),sin(blur))*spread;
    return (textureSampleLevel(transmission,linear,uv+o,0.0).rgb
        +textureSampleLevel(transmission,linear,uv-o,0.0).rgb)*.5;
}
// The beam's light at x through the blocks, by wavelength: its profile
// (along `axis`, the shutter's instant) and what the blocks move, never
// below nothing nor above CAUSTIC_MAX, times its falloff.
fn light_at(x:vec3f,axis:vec3f,blur:f32)->vec3f {
    let m=moved(x,blur);
    let lit=vec3f(beam_profile(x,axis))+min(m,vec3f(0))+max(m,vec3f(0))*CAUSTIC_GAIN;
    return clamp(lit,vec3f(0),vec3f(CAUSTIC_MAX))*beam_falloff(x);
}

// ---- The air ----------------------------------------------------------------------------
// The wind's displacement at a moment (authored), and the air's density at
// x: thin haze, thicker and thinner in slow, large regions carried by it.
// The swirls: the curl of a sum of four waves (each a direction k and an
// axis a: the field a sin(k.x + w t + p)'s curl is (k x a) cos(...)), of
// this wavelength and reach, `slow` scaling their periods.
fn swirl(x:vec3f,t:f32,wavelength:f32,reach:f32,slow:f32)->vec3f {
    let ks=array<vec3f,4>(vec3f(.62,.55,-.56),vec3f(-.71,.31,.63),vec3f(.18,-.88,.44),vec3f(-.35,-.42,-.84));
    // k x a, normalised, for the axes (.3,-.8,.5), (.6,.6,.5), (-.9,-.1,.4)
    // and (.2,.7,-.7).
    let curls=array<vec3f,4>(vec3f(-0.207,-0.573,-0.793),vec3f(-0.227,0.748,-0.624),vec3f(-0.313,-0.475,-0.822),vec3f(0.894,-0.418,-0.163));
    let periods=vec4f(31.0,23.0,37.0,27.0)*slow;
    let phases=vec4f(.3,2.1,4.4,5.6);
    var v=vec3f(0);
    for(var i=0;i<4;i++) {
        let k=ks[i]*(2.0*PI/wavelength);
        let w=2.0*PI/periods[i];
        v+=curls[i]*cos(dot(k,x)+w*t+phases[i]);
    }
    return v*reach*.5;
}
// Where the air that was at x when part 7 began has been carried by the
// moment: the drift, the large swirls, and the small over them.
fn flow(x:vec3f)->vec3f {
    let t=greetings_seconds();
    let y=x+DRIFT*t+swirl(x,t,SWIRL_LARGE.x,SWIRL_LARGE.y,1.0);
    return y+swirl(y,t,SWIRL_SMALL.x,SWIRL_SMALL.y,.4);
}
// Where the air at x came from, near enough: the drift and the large
// swirls undone once (the small ones are finer than the air's thickening).
fn unflow(x:vec3f)->vec3f {
    let t=greetings_seconds();
    return x-DRIFT*t-swirl(x,t,SWIRL_LARGE.x,SWIRL_LARGE.y,1.0);
}
fn density(x:vec3f)->f32 {
    let q=unflow(x)/DENSITY_SCALE;
    let n=textureSampleLevel(density_noise,repeat,q,0.0).r;
    return mix(THINNEST,THICKEST,smoothstep(THICKENS.x,THICKENS.y,n));
}
// A fixed pattern per pixel (interleaved gradient noise), so the samples
// spread without changing from frame to frame.
fn pattern(p:vec2f)->f32 {return fract(52.9829189*fract(dot(p,vec2f(.06711056,.00583715))));}
// Where the ray from the camera along d is inside the beam's cone (its
// edge's outer bound), between t0 and t1, or (1, 0). The cone's apex is
// behind the lamp so its radius there is the aperture's.
fn cone_span(d:vec3f,axis:vec3f,t0:f32,t1:f32)->vec2f {
    return cone_span_at(d,axis,t0,t1,BEAM_SPREAD*BEAM_END,APERTURE*BEAM_END);
}
// The same for a cone of this slope (its radius per row along) and radius
// at the lamp.
fn cone_span_at(d:vec3f,axis:vec3f,t0:f32,t1:f32,slope:f32,reach:f32)->vec2f {
    let apex=lamp()-axis*(reach/slope);
    let c2=1.0/(1.0+slope*slope);
    let co=CAMERA-apex;
    let da=dot(d,axis);
    let ca=dot(co,axis);
    let a=da*da-c2;
    let b=2.0*(da*ca-c2*dot(d,co));
    let c=ca*ca-c2*dot(co,co);
    // Inside means within the cone's angle and in front of its apex.
    let disc=b*b-4.0*a*c;
    if abs(a)<1e-6 {
        return vec2f(t0,t1);
    }
    if disc<0.0 {
        if a>0.0 && ca+da*t0>0.0 {return vec2f(t0,t1);}
        return vec2f(1,0);
    }
    let h=sqrt(disc);
    var r0=(-b-h)/(2.0*a);
    var r1=(-b+h)/(2.0*a);
    if r0>r1 {let s=r0;r0=r1;r1=s;}
    if a<0.0 {
        // Inside between the roots (if that part is in front of the apex).
        let m=(r0+r1)*.5;
        if ca+da*m<=0.0 {return vec2f(1,0);}
        return vec2f(max(r0,t0),min(r1,t1));
    }
    // Inside before r0 or after r1: the side in front of the apex.
    if ca+da*(r1+1.0)>0.0 {return vec2f(max(r1,t0),t1);}
    return vec2f(t0,min(r0,t1));
}

// ---- The blocks ------------------------------------------------------------------------
fn strip_x(s:u32)->f32 {return (f32(8u+64u*s)+14.0-160.0)*COLUMN;}
// Slot k's letter in strip s (frame shown), or 255.
fn letter_in(s:u32,k:u32)->u32 {
    let w=letters[frame_now()*6u+1u+s];
    return (w[k/4u]>>(8u*(k%4u)))&255u;
}
fn first_top()->f32 {return gc.twist_late.y;}
fn position_of(s:u32)->f32 {return select(gc.twist[s],gc.twist_late.x,s==4u);}
// Slot k's block in strip s: its centre's source row, its centre, and its
// turn (the original's sine at its centre row).
fn centre_row(k:f32)->f32 {return first_top()+SLOT_ROWS*k+HALF;}
fn block_centre(s:u32,k:f32)->vec3f {return vec3f(strip_x(s),100.0-centre_row(k),0.0);}
fn block_turn(s:u32,k:f32)->f32 {
    return TWIST_STEP*(40.0*sin(PI*(position_of(s)+centre_row(k))/200.0)+60.0);
}
// A point or direction in slot k's block's frame (across, up, back), and
// back again.
fn to_block(s:u32,k:f32,p:vec3f)->vec3f {
    let q=p-block_centre(s,k);
    let a=block_turn(s,k);
    return vec3f(cos(a)*q.x+sin(a)*q.z,q.y,-sin(a)*q.x+cos(a)*q.z);
}
fn turn_to_block(s:u32,k:f32,v:vec3f)->vec3f {
    let a=block_turn(s,k);
    return vec3f(cos(a)*v.x+sin(a)*v.z,v.y,-sin(a)*v.x+cos(a)*v.z);
}
fn turn_from_block(s:u32,k:f32,v:vec3f)->vec3f {
    let a=block_turn(s,k);
    return vec3f(cos(a)*v.x-sin(a)*v.z,v.y,sin(a)*v.x+cos(a)*v.z);
}
fn box_distance(l:vec3f)->f32 {
    let q=abs(l)-vec3f(HALF-ROUNDING);
    return length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-ROUNDING;
}
// The face a block-frame point lies on (its outward normal, a side face)
// and the point's place on it: across as seen from outside, and up.
struct Face { normal:vec3f, across:vec3f, at:vec2f }
fn face_of(l:vec3f)->Face {
    var n=vec3f(0,0,sign(l.z));
    if abs(l.x)>=abs(l.z) {n=vec3f(sign(l.x),0,0);}
    let r=vec3f(-n.z,0,n.x);
    return Face(n,r,vec2f(dot(l,r),l.y));
}
// The letter at a place on a face: its signed distance (rows, negative
// inside) and the distance's gradient.
fn letter_at(g:u32,at:vec2f)->vec3f {
    let t=vec2f(at.x,-at.y)/(2.0*HALF*LETTER_SCALE)+.5;
    if g==255u || any(t<vec2f(.5/CELL)) || any(t>vec2f(1.0-.5/CELL)) {return vec3f(4,0,0);}
    let cell=vec2f(f32(g%ATLAS_COLUMNS),f32(g/ATLAS_COLUMNS))*CELL;
    let face=textureSampleLevel(glyphs,linear,(cell+t*CELL)/ATLAS_SIZE,0.0);
    return vec3f(face.r*LETTER_SCALE,face.ba);
}
// The cut's depth at a place on a face, from the letter's distance.
fn cut_depth(d:f32)->f32 {return clamp(-d*CUT_SLOPE,0.0,CUT_DEPTH);}
// Slot k's block, cut: the rounded block, less the V-groove on the side
// face the point is nearest.
fn block_distance(s:u32,k:f32,p:vec3f)->f32 {
    let l=to_block(s,k,p);
    let b=box_distance(l);
    if b>2.0 {return b;}
    let f=face_of(l);
    let depth=cut_depth(letter_at(letter_in(s,u32(k)),f.at).x);
    // The groove's walls are steeper than one: a safer step there.
    let cut=(dot(l,f.normal)-(HALF-depth))/sqrt(1.0+CUT_SLOPE*CUT_SLOPE);
    return max(b,cut);
}
struct Near { distance:f32, slot:f32 }
// The nearest block in strip s (by its uncut box), and the distance to it
// cut.
fn stack_near(s:u32,p:vec3f)->Near {
    let k0=floor((100.0-p.y-first_top())/SLOT_ROWS);
    var best=Near(1e5,-1.0);
    for(var dk=-1.0;dk<=1.0;dk+=1.0) {
        let k=k0+dk;
        if k<0.0 || k>=f32(SLOTS) || letter_in(s,u32(k))==255u {continue;}
        let d=box_distance(to_block(s,k,p));
        if d<best.distance {best=Near(d,k);}
    }
    if best.slot>=0.0 {best.distance=block_distance(s,best.slot,p);}
    return best;
}
fn stack_distance(s:u32,p:vec3f)->f32 {return stack_near(s,p).distance;}
fn block_normal(s:u32,k:f32,p:vec3f)->vec3f {
    let e=vec2f(.02,-.02);
    return normalize(e.xyy*block_distance(s,k,p+e.xyy)+e.yyx*block_distance(s,k,p+e.yyx)
        +e.yxy*block_distance(s,k,p+e.yxy)+e.xxx*block_distance(s,k,p+e.xxx));
}
// The ray's span inside strip s's column (radius: a block's half
// diagonal), or (1, 0).
fn column_span(s:u32,o:vec3f,d:vec3f)->vec2f {
    let r=HALF*1.4143+.2;
    let ox=o.x-strip_x(s);
    let a=d.x*d.x+d.z*d.z;
    let b=2.0*(ox*d.x+o.z*d.z);
    let c=ox*ox+o.z*o.z-r*r;
    let disc=b*b-4.0*a*c;
    if disc<=0.0 || a<1e-9 {return vec2f(1,0);}
    let h=sqrt(disc);
    return vec2f((-b-h)/(2.0*a),(-b+h)/(2.0*a));
}
// The first block the ray from o along d meets between t0 and t1: (t,
// strip, slot), or t = -1.
fn first_block(o:vec3f,d:vec3f,t0:f32,t1:f32)->vec3f {
    var best=vec3f(-1,0,0);
    for(var s=0u;s<STRIPS;s++) {
        let span=column_span(s,o,d);
        let a=max(span.x,t0);
        var b=min(span.y,t1);
        if best.x>=0.0 {b=min(b,best.x);}
        if a>=b {continue;}
        var t=a;
        for(var i=0u;i<40u;i++) {
            let near=stack_near(s,o+d*t);
            if near.distance<.01 {best=vec3f(t,f32(s),near.slot);break;}
            t+=max(near.distance,.02);
            if t>b {break;}
        }
    }
    return best;
}
// The normal (block frame, outward) of slot k's block's surface near the
// block-frame point l: a side face's, tilted by the cut's facet where the
// letter is cut (the groove's walls slope CUT_SLOPE rows deep a row in,
// down to CUT_DEPTH); the top's or the bottom's.
fn face_normal(s:u32,k:f32,l:vec3f)->vec3f {
    let a=abs(l);
    if a.y>=max(a.x,a.z) {return vec3f(0,sign(l.y),0);}
    let f=face_of(l);
    let letter=letter_at(letter_in(s,u32(k)),f.at);
    let depth=-letter.x*CUT_SLOPE;
    if depth<=0.0 || depth>=CUT_DEPTH {return f.normal;}
    let du=f.across+f.normal*CUT_SLOPE*letter.y;
    let dv=vec3f(0,1,0)+f.normal*CUT_SLOPE*letter.z;
    let n=normalize(cross(du,dv));
    return select(-n,n,dot(n,f.normal)>0.0);
}
// How much of a block-frame point on slot k's block's side faces lies in
// the cut (0 to 1, soft over a tenth of a row).
fn frost_at(s:u32,k:f32,l:vec3f)->f32 {
    let a=abs(l);
    if a.y>=max(a.x,a.z) {return 0.0;}
    let f=face_of(l);
    return 1.0-smoothstep(-.1,.1,letter_at(letter_in(s,u32(k)),f.at).x);
}
// From a block-frame point inside the block along d (block frame): how far
// to its faces (its rounding and the cut's depth ignored), and which face.
struct Exit { distance:f32, axis:vec3f }
fn box_exit(l:vec3f,d:vec3f)->Exit {
    let t=(sign(d)*HALF-l)/d;
    if t.x<=min(t.y,t.z) {return Exit(max(t.x,0.0),vec3f(sign(d.x),0,0));}
    if t.y<=t.z {return Exit(max(t.y,0.0),vec3f(0,sign(d.y),0));}
    return Exit(max(t.z,0.0),vec3f(0,0,sign(d.z)));
}
// Light inside slot k's block from the block-frame point l along d (block
// frame), at refractive index eta: across the block to a face, out through
// it (its facet's normal there), or reflected inside where it must be and
// on, up to three times. Where it leaves (block frame), its direction out
// (zero if it never does) and the path inside.
struct Passage { at:vec3f, out:vec3f, path:f32 }
fn pass_through(s:u32,k:f32,l:vec3f,d:vec3f,eta:f32)->Passage {
    var at=l;
    var dir=d;
    var path=0.0;
    for(var bounce=0u;bounce<4u;bounce++) {
        let e=box_exit(at,dir);
        at+=dir*e.distance;
        path+=e.distance;
        let n=face_normal(s,k,at);
        let o=refract(dir,-n,eta);
        if dot(o,o)>0.0 {return Passage(at,o,path);}
        dir=reflect(dir,n);
        at-=e.axis*.01;
    }
    return Passage(at,vec3f(0),path);
}

// ---- The photons -----------------------------------------------------------------------
// One ray from the lamp for each cell of the photon grid over the map, at
// each of the three wavelengths (photon n's wavelength is n mod 3): traced
// through the blocks it meets, refracting at each face and facet,
// internally reflected where it must be (twice at most, else lost), and
// where it is LANDING rows along the beam is put back on the map as the
// lamp's straight ray there. Rays meeting no block are left alone.
@compute @workgroup_size(64) fn trace_photons(@builtin(global_invocation_id) id:vec3u) {
    greetings_begin();
    let n=id.x;
    let count=PHOTON_GRID.x*PHOTON_GRID.y*3u;
    if n>=count {return;}
    photons_out[n]=vec4f(0);
    if stage()<STAGE_WORDS-.5 || stage()>STAGE_WHITE+.5 || intensity()<=0.0 {return;}
    let cell=n/3u;
    let c=n%3u;
    let uv=(vec2f(f32(cell%PHOTON_GRID.x),f32(cell/PHOTON_GRID.x))+.5)/vec2f(PHOTON_GRID);
    let l=lamp();
    let d=map_direction(uv);
    if d.z>=0.0 {return;}
    let enter=(l.z-SLAB)/(-d.z);
    let leave=(l.z+SLAB)/(-d.z);
    let hit=first_block(l,d,enter,leave);
    if hit.x<0.0 {return;}
    let s=u32(hit.y);
    let k=hit.z;
    let iors=IOR;
    let eta=iors[c];
    // The beam's profile in this direction: the light the photon carries.
    let axis=map_frame().axis;
    let carried=beam_profile(l+d*(LANDING/max(dot(d,axis),.05)),axis);
    if carried<=0.0 {return;}
    let at=to_block(s,k,l+d*hit.x);
    let n_in=face_normal(s,k,at);
    // The frosted cut scatters what meets it: little of it goes on.
    var frosted=frost_at(s,k,at);
    let d_in=turn_to_block(s,k,d);
    let inside=refract(d_in,n_in,1.0/eta);
    var weight=1.0-(F0+(1.0-F0)*pow(1.0-clamp(dot(-d_in,n_in),0.0,1.0),5.0));
    let passage=pass_through(s,k,at-n_in*.01,inside,eta);
    if dot(passage.out,passage.out)==0.0 {
        // Trapped: lost here.
        photons_out[n]=vec4f(-1,-1,0,carried);
        return;
    }
    let out=turn_from_block(s,k,passage.out);
    let p=turn_from_block(s,k,passage.at)+block_centre(s,k);
    let absorption=ABSORPTION;
    frosted=max(frosted,frost_at(s,k,passage.at));
    weight*=exp(-passage.path*absorption[c])*.96*(1.0-FROST_SCATTER*frosted);
    if out.z>=-1e-3 {
        // Out behind the strips, where the map does not reach: lost here.
        photons_out[n]=vec4f(-1,-1,0,carried);
        return;
    }
    let along=dot(p-l,axis);
    let landing=p+out*(max(LANDING-along,1.0)/max(dot(out,axis),.05));
    photons_out[n]=vec4f(map_uv(landing),weight*carried,carried);
}
// Each photon that met a block moves its light: it takes what it carried
// away where its straight ray would have gone and puts what reached through
// where it landed (a tent, see SPLAT), in its wavelength's channel; the
// map is cleared to 0 and added to the beam's profile.
struct PhotonVertex {
    @builtin(position) position:vec4f,
    @location(0) offset:vec2f,
    @location(1) @interpolate(flat) colour:vec3f,
}
@vertex fn photon_vertex(@builtin(vertex_index) corner:u32,@builtin(instance_index) i:u32)->PhotonVertex {
    var out:PhotonVertex;
    out.position=vec4f(2,2,2,1);
    let n=i/2u;
    let landing=(i%2u)==1u;
    let ph=photons_in[n];
    if ph.w<=0.0 || (landing && ph.z<=0.0) {return out;}
    let cell=n/3u;
    let home=(vec2f(f32(cell%PHOTON_GRID.x),f32(cell/PHOTON_GRID.x))+.5)/vec2f(PHOTON_GRID);
    let centre=select(home,ph.xy,landing);
    let channel=array<vec3f,3>(vec3f(1,0,0),vec3f(0,1,0),vec3f(0,0,1))[n%3u];
    let c=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[corner];
    let uv=centre+c*SPLAT/vec2f(PHOTON_GRID);
    out.position=vec4f(uv.x*2.0-1.0,1.0-uv.y*2.0,0,1);
    out.offset=c;
    out.colour=channel*select(-ph.w,ph.z,landing)/(SPLAT*SPLAT);
    return out;
}
@fragment fn photon_fragment(v:PhotonVertex)->@location(0) vec4f {
    let w=max(1.0-abs(v.offset.x),0.0)*max(1.0-abs(v.offset.y),0.0);
    return vec4f(v.colour*w,0);
}

// ---- The lit air ------------------------------------------------------------------------------
// The air's light along each ray, at half size: marched within the cone,
// each sample at its own instant of the shutter (the beam turns about 400
// degrees a second), the light in front of the blocks kept apart (its depth
// goes in alpha for the picture's upsampling) from all of it (seen through
// the crystal).
struct AirOut { @location(0) front:vec4f, @location(1) full:vec4f }
@fragment fn air(v:Vertex)->AirOut {
    greetings_begin();
    let pixel=vec2i(v.position.xy);
    let full=min(pixel*2,vec2i(u.screen.xy)-1);
    let depth=textureLoad(geometry,full,0).w;
    let q=scene_of_uv(v.uv);
    let d=normalize(vec3f(q,CAMERA_DISTANCE));
    let near=(AIR_NEAR+CAMERA_DISTANCE)/d.z;
    let far=(AIR_FAR+CAMERA_DISTANCE)/d.z;
    let jitter=pattern(v.position.xy);
    // The span over the shutter: the union of the cone at its two ends.
    let a0=cone_span(d,beam_axis(0.0),near,far);
    let a1=cone_span(d,beam_axis(SHUTTER),near,far);
    var span=vec2f(min(a0.x,a1.x),max(a0.y,a1.y));
    if a0.x>=a0.y {span=a1;}
    if a1.x>=a1.y {span=a0;}
    var front=vec3f(0);
    var all=vec3f(0);
    if span.x<span.y && intensity()>0.0 {
        let step=(span.y-span.x)/f32(MARCH_SAMPLES);
        for(var k=0u;k<MARCH_SAMPLES;k++) {
            let f=fract(jitter+f32(k)*.618034);
            let t=span.x+(f32(k)+f)*step;
            let x=CAMERA+d*t;
            let axis=beam_axis(SHUTTER*fract(jitter*7.31+f32(k)*.38197));
            let b=light_at(x,axis,jitter*6.283+f32(k));
            if max(b.r,max(b.g,b.b))<=0.0 {continue;}
            let towards=normalize(x-lamp());
            let lit=b*phase(towards,-d,PHASE_G)*density(x)*step;
            all+=lit;
            if t<depth {front+=lit;}
        }
    }
    // The light the blocks throw out of the cone, over the map's wider cone
    // but outside the beam's.
    let axis=beam_axis(0.0);
    let wide=cone_span_at(d,axis,near,far,MAP_SPREAD,APERTURE);
    if wide.x<wide.y && intensity()>0.0 {
        let step=(wide.y-wide.x)/f32(SPRAY_SAMPLES);
        for(var k=0u;k<SPRAY_SAMPLES;k++) {
            let t=wide.x+(f32(k)+fract(jitter*3.17+f32(k)*.618034))*step;
            let x=CAMERA+d*t;
            if beam_profile(x,axis)>0.0 {continue;}
            let m=max(moved(x,jitter*6.283+f32(k)),vec3f(0));
            if max(m.r,max(m.g,m.b))<=0.0 {continue;}
            let lit=min(m*CAUSTIC_GAIN,vec3f(CAUSTIC_MAX))*beam_falloff(x)*phase(normalize(x-lamp()),-d,PHASE_G)*density(x)*step;
            all+=lit;
            if t<depth {front+=lit;}
        }
    }
    // The lamp's glow in the air everywhere: from a point lamp falling off
    // as the beam does, along the ray, in closed form, thickened or
    // thinned as the air is where the ray passes nearest the lamp.
    let to=lamp()-CAMERA;
    let s0=dot(to,d);
    let h=length(to-d*s0);
    let w=sqrt(FALLOFF*FALLOFF+h*h);
    let glow=FALLOFF*FALLOFF/w*AMBIENT_AIR*BEAM_POWER*mix(1.0,density(CAMERA+d*max(s0,near)),.5);
    let glow_front=glow*(atan((min(depth,far)-s0)/w)-atan((near-s0)/w));
    let glow_all=glow*(atan((far-s0)/w)-atan((near-s0)/w));
    let k=star_colour()*SCATTER*intensity();
    let g=star_colour()*intensity();
    return AirOut(vec4f(front*k+g*max(glow_front,0.0),depth),vec4f(all*k+g*glow_all,1));
}

// ---- The dust ------------------------------------------------------------------------
// A mote's place at the moment: carried by the air's flow from where it
// was when part 7 began (authored), each along its own path.
fn mote_place(m:vec4f)->vec3f {return flow(m.xyz);}
struct DustVertex {
    @builtin(position) position:vec4f,
    @location(0) offset:vec2f,
    @location(1) @interpolate(flat) colour:vec3f,
    @location(2) @interpolate(flat) radius:f32,
    @location(3) @interpolate(flat) distance:f32,
}
@vertex fn dust_vertex(@builtin(vertex_index) corner:u32,@builtin(instance_index) n:u32)->DustVertex {
    greetings_begin();
    var out:DustVertex;
    out.position=vec4f(2,2,2,1);
    if stage()>STAGE_WHITE+.5 || intensity()<=0.0 {return out;}
    let m=motes[n];
    let index=floor(m.w);
    let size=m.w-index;
    let x=mote_place(m);
    let to_camera=CAMERA-x;
    let distance=length(to_camera);
    if x.z<=-CAMERA_DISTANCE+40.0 {return out;}
    // Lit by the beam over the shutter, forward scattering, through what
    // the blocks let through; outside it, a trace of the original's band.
    var lit=vec3f(0);
    for(var k=0u;k<3u;k++) {
        lit+=light_at(x,beam_axis(SHUTTER*f32(k)*.5),f32(n)+f32(k)*2.1);
    }
    let beam=lit*phase(normalize(x-lamp()),to_camera/distance,MOTE_G)/3.0*MOTE_GAIN;
    let glow=MOTE_AMBIENT*beam_falloff(x);
    var colour=star_colour()*(beam+glow+MOTE_BAND*star_light(index,gc.sweep.x));
    colour*=(.3+2.2*size*size)*intensity();
    // In focus at the strips' plane; the near ones soft discs, the far ones
    // points; a disc's light spread over its area.
    let px=u.screen.y/1080.0;
    let coc=min(BOKEH*abs(1.0-CAMERA_DISTANCE/distance),BOKEH_MAX)*px;
    let core=MOTE_RADIUS*(.7+1.6*size)*px;
    let radius=max(core,coc);
    colour*=(core*core)/(radius*radius);
    if max(colour.r,max(colour.g,colour.b))<1e-4 {return out;}
    let q=project(x);
    let c=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[corner];
    let extent=radius*1.6+1.0;
    let uv=uv_of_scene(q);
    let ndc=vec2f(uv.x*2.0-1.0,1.0-uv.y*2.0)+c*extent*2.0/u.screen.xy;
    out.position=vec4f(ndc,0,1);
    out.offset=c*extent;
    out.colour=colour;
    out.radius=radius;
    out.distance=distance;
    return out;
}
struct DustOut { @location(0) front:vec4f, @location(1) full:vec4f }
@fragment fn dust_fragment(v:DustVertex)->DustOut {
    let r=length(v.offset)/v.radius;
    // A soft disc, its edge a little brighter, as out of focus light is.
    let disc=(1.0-smoothstep(.75,1.05,r))*(.85+.25*smoothstep(.4,.95,r));
    let c=v.colour*disc;
    // In front of the blocks, or behind them (seen through the crystal).
    let hidden=textureLoad(geometry,vec2i(v.position.xy),0).w<v.distance;
    return DustOut(vec4f(select(c,vec3f(0),hidden),1),vec4f(c,1));
}

// ---- The blocks seen ------------------------------------------------------------------------
// The blocks along each ray: coverage (soft at their silhouettes), strip
// * 16 + slot, the distance to them, and the distance where they are solid
// (else 1e9), for the air and the dust to stop at.
@fragment fn blocks(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let q=scene_of_uv(v.uv);
    let d=normalize(vec3f(q,CAMERA_DISTANCE));
    let row_per_pixel=200.0/u.screen.y;
    if stage()<STAGE_WORDS-.5 || stage()>STAGE_WHITE+.5 {return vec4f(0,0,0,1e9);}
    for(var s=0u;s<STRIPS;s++) {
        let span=column_span(s,CAMERA,d);
        if span.x>=span.y {continue;}
        var t=max(span.x,0.0);
        var closest=1e5;
        var closest_t=t;
        var slot=-1.0;
        var hit=false;
        for(var i=0u;i<MARCH_STEPS;i++) {
            let near=stack_near(s,CAMERA+d*t);
            let footprint=row_per_pixel*t/CAMERA_DISTANCE;
            if near.distance/footprint<closest {closest=near.distance/footprint;closest_t=t;slot=near.slot;}
            if near.distance<.002*t*row_per_pixel {hit=true;break;}
            t+=max(near.distance,.01);
            if t>span.y {break;}
        }
        let c=select(clamp(1.0-closest,0.0,1.0),1.0,hit);
        if c>0.0 && slot>=0.0 {
            let at=select(closest_t,t,hit);
            return vec4f(c,f32(s)*16.0+slot,at,select(1e9,at,c>.5));
        }
    }
    return vec4f(0,0,0,1e9);
}

// ---- The picture ----------------------------------------------------------------------------
fn monster_colour(p:vec2i)->vec3f {
    let q=clamp(p,vec2i(0),vec2i(319,199));
    return entry(textureLoad(monster,q,0).r,true);
}
fn catmull(t:f32)->vec4f {
    let t2=t*t;let t3=t2*t;
    return vec4f(-.5*t3+t2-.5*t,1.5*t3-2.5*t2+1.0,-1.5*t3+2.0*t2+.5*t,.5*t3-.5*t2);
}
// MONSTER.U at the frame's palette (the original's level of it), upscaled;
// outside the 4:3 frame, its colour 0 at that level.
fn monster_picture(s:vec2f)->vec3f {
    if !in_frame(s) {return entry(0u,true);}
    let q=s-.5;
    let k=vec2i(floor(q));
    let wx=catmull(q.x-floor(q.x));
    let wy=catmull(q.y-floor(q.y));
    var c=vec3f(0);
    for(var y=0;y<4;y++) {
        for(var x=0;x<4;x++) {
            c+=monster_colour(k+vec2i(x-1,y-1))*wx[x]*wy[y];
        }
    }
    return clamp(c,vec3f(0),vec3f(1));
}
// The lamp seen directly at r rows from it: the core and halo, and its
// aperture's flare as the beam turns toward the camera.
fn lamp_glow(r:f32)->f32 {
    let facing=max(-beam_axis(0.0).z,0.0);
    let flare=FLARE*pow(facing,8.0);
    return exp(-(r/CORE)*(r/CORE))*CORE_GAIN*(1.0+flare)+exp(-r/HALO)*HALO_GAIN*(1.0+flare*.5);
}
// The lit air in front of the blocks at a full-size pixel from the
// half-size image: the four nearest samples, each weighted by how near its
// depth is to the pixel's, so the air stops cleanly at the blocks' edges.
fn air_in_front(pixel:vec2i,depth:f32)->vec3f {
    let size=vec2i(textureDimensions(air_front));
    let h=(vec2f(pixel)+.5)*.5-.5;
    let base=vec2i(floor(h));
    let f=h-floor(h);
    var sum=vec3f(0);
    var weights=0.0;
    for(var y=0;y<2;y++) {
        for(var x=0;x<2;x++) {
            let at=clamp(base+vec2i(x,y),vec2i(0),size-1);
            let s=textureLoad(air_front,at,0);
            let w=select(1.0-f.x,f.x,x==1)*select(1.0-f.y,f.y,y==1)
                *exp(-abs(min(s.a,1e4)-min(depth,1e4))/8.0)+1e-5;
            sum+=s.rgb*w;
            weights+=w;
        }
    }
    return sum/weights;
}
// Everything behind the blocks seen along the ray from p along d: the dust,
// the lit air and the lamp, from the images (taken where the ray reaches
// BEHIND rows on) and the lamp's glow round the ray.
fn behind(p:vec3f,d:vec3f)->vec3f {
    var reach=BEHIND;
    if d.z<0.0 {reach=min(reach,(p.z+CAMERA_DISTANCE-60.0)/(-d.z));}
    let q=p+d*reach;
    let uv=uv_of_scene(project(q));
    var c=vec3f(0);
    if all(uv>vec2f(0)) && all(uv<vec2f(1)) {
        c=textureLoad(dust_full,vec2i(uv*u.screen.xy),0).rgb+textureSampleLevel(air_full,linear,uv,0.0).rgb;
    }
    // The lamp: how near the ray passes it, in rows.
    let to=lamp()-p;
    let along=dot(to,d);
    if along>0.0 {c+=lamp_glow(length(to-d*along))*star_colour()*intensity();}
    return c;
}
// A block seen at p along d, `footprint` rows a pixel there.
fn crystal(s:u32,k:f32,p:vec3f,d:vec3f,footprint:f32)->vec3f {
    let n=block_normal(s,k,p);
    let l=to_block(s,k,p);
    let v=-d;
    // The letter on this face: how much of the point is cut, and how deep.
    let f=face_of(l);
    let side=abs(l.y)<max(abs(l.x),abs(l.z))+.2;
    let letter=letter_at(letter_in(s,u32(k)),f.at);
    let aa=max(footprint*.8,.03);
    let cut=select(0.0,1.0-smoothstep(-aa,aa,letter.x),side);
    let depth=cut_depth(letter.x)/CUT_DEPTH;
    let cosine=clamp(dot(n,v),0.0,1.0);
    let fresnel=F0+(1.0-F0)*pow(1.0-cosine,5.0);
    // The beam through the block, and the lamp's light on it.
    let axis=beam_axis(0.0);
    let through=min(beam_at(block_centre(s,k),axis),2.0);
    let L=normalize(lamp()-p);
    let h=normalize(L+v);
    let nh=max(dot(n,h),0.0);
    let a2=POLISH*POLISH;
    let ggx=a2/(PI*(nh*nh*(a2-1.0)+1.0)*(nh*nh*(a2-1.0)+1.0));
    let glint=ggx*fresnel*max(dot(n,L),0.0)*GLINT*(.25+through);
    // The polish mirrors the picture (see FRONT_MIRROR).
    let rd=reflect(d,n);
    let toward=vec3f(rd.xy,abs(rd.z)+.05);
    let mq=p+toward*((MIRROR_DEPTH-p.z)/toward.z);
    let muv=uv_of_scene(project(mq));
    var mirrored=vec3f(0);
    if all(muv>vec2f(0)) && all(muv<vec2f(1)) {
        mirrored=textureLoad(dust_full,vec2i(muv*u.screen.xy),0).rgb+textureSampleLevel(air_full,linear,muv,0.0).rgb;
    }
    mirrored*=select(FRONT_MIRROR,1.0,rd.z>.05);
    // Through the crystal, a wavelength at a time: refracted in, across the
    // block (absorbing a little), out (or reflected inside, then out).
    var seen=vec3f(0);
    let iors=IOR;
    let absorption=ABSORPTION;
    let d_in=turn_to_block(s,k,d);
    let n_in=turn_to_block(s,k,n);
    for(var c=0u;c<3u;c++) {
        let eta=iors[c];
        let passage=pass_through(s,k,l-n_in*.01,refract(d_in,n_in,1.0/eta),eta);
        var out=passage.out;
        if dot(out,out)==0.0 {out=reflect(d_in,n_in);}
        let exit=turn_from_block(s,k,passage.at)+block_centre(s,k);
        let light=behind(exit,turn_from_block(s,k,normalize(out)));
        seen[c]=light[c]*exp(-passage.path*absorption[c]);
    }
    // The cut, frosted, glows by the lamp's light carried in the glass and
    // flares with the beam through it; its walls, steeper to the eye,
    // brighter than the groove's floor and the crease, as a cut catches the
    // light inside the glass. Tinted faintly as the original tints its
    // letters (white crowns, teal edges: the walls whiter, the floor
    // teal), and cooled from the lamp's cream toward white.
    let wall=clamp((1.0-dot(n,turn_from_block(s,k,f.normal)))*3.0,0.0,1.0);
    let teal=srgb_decode(vec3f(12.0,32.0,32.0)/63.0)/srgb_decode(vec3f(32.0/63.0));
    let tint=mix(mix(vec3f(1),teal,.22),vec3f(1),wall)*mix(star_colour(),vec3f(1),.6);
    let glow=tint*(CUT_REST+CUT_BEAM*through)*(.75+.5*wall);
    // The edges: light caught in the block leaves where its faces meet.
    let a=abs(l);
    let mid=max(min(a.x,a.y),min(max(a.x,a.y),a.z));
    let edge=smoothstep(HALF-1.4,HALF-.15,mid);
    let flare=edge*(EDGE_REST+EDGE_FLARE*through)+cut*wall*CUT_FLARE*through;
    let clear=(1.0-fresnel)*seen*(1.0-FROST*cut);
    // The glass's own light is the lamp's, cooled a little as the cut's is.
    let light=mix(star_colour(),vec3f(1),.5);
    return clear+(fresnel*mirrored)+((glint+flare)*light+glow*FROST*cut)*intensity();
}
@fragment fn style(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let pixel=vec2i(v.position.xy);
    let q=scene_of_uv(v.uv);
    if shows_monster() {
        // Developed out of the white: shown as the original shows it,
        // through the tonemap backwards, the whites held a little below
        // its top once the picture is up.
        let top=mix(.999,MONSTER_WHITE,clamp(gc.sweep.w,0.0,1.0));
        return vec4f(tonemap_inverse(min(monster_picture(source_of_scene(q)),vec3f(top))),1);
    }
    let g=textureLoad(geometry,pixel,0);
    let lq=project(lamp());
    let lamp_light=star_colour()*intensity();
    // Without the blocks: all the dust and the air, and the lamp.
    let open=textureLoad(dust_full,pixel,0).rgb+textureSampleLevel(air_full,linear,v.uv,0.0).rgb
        +lamp_glow(length(q-lq))*lamp_light;
    var c=open;
    if g.x>0.0 {
        let s=u32(g.y/16.0);
        let k=g.y-f32(s)*16.0;
        let d=normalize(vec3f(q,CAMERA_DISTANCE));
        let p=CAMERA+d*g.z;
        let footprint=200.0/u.screen.y*g.z/CAMERA_DISTANCE;
        let front=textureLoad(dust_front,pixel,0).rgb+air_in_front(pixel,g.w);
        c=mix(open,front+crystal(s,k,p,d,footprint),g.x);
    }
    // The fade to white, at the original's levels.
    let w=clamp(gc.sweep.z,0.0,1.0);
    if w>0.0 {
        c=tonemap_inverse(mix(tonemap(c),vec3f(1),w));
    }
    return vec4f(max(c,vec3f(0)),1);
}
