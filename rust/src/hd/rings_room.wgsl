// Room (authored): lights out in the scroller's room. The chrome bump wall
// of part 2 (wall.wgsl, as part 2 left it, seen with the scroller's camera)
// stays in the dark after the scroller's fade. The lit symbol hangs a few
// rows in front of it as yellow neon tubes, which light the bumps turned to
// them and show in their chrome. Two gobo projectors above the camera, one a
// little to the left and one to the right, cast the two ring layers onto the
// wall: layer 0 in cool blue, layer 1 in warm orange. The bands light the
// bumps' upper faces and the bumps shade and shadow themselves below; each
// tube casts two shadows, one in each beam.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with room_.
//
// From the original, through the shared helpers: the timing (black until
// part 6 leaves its wait at about 43.41 s, the PSTART fade from 80.36 s),
// the ring centres, band geometry and intensity fade, and the lit symbol
// and its windows. From the port, through wall.wgsl: the tile grid's
// layout and the wall's position as part 2 left it. Everything else is
// authored: the room, the projectors, their colours, positions and focus,
// the tubes' height, the materials and the light. Nothing here is a
// recovered model.
//
// The projectors are aligned so that on the wall's flat backing, as the
// camera sees it, each draws its layer's bands exactly where the original
// draws them: their gobo is the original's frame mapped onto the wall. Only
// what stands off the backing moves: the bumps take the pattern displaced
// towards each projector by their height times the tangent of its angle
// (with the default bump shape, about a band pixel and a half at a bump's
// top), and the tubes' shadows fall below and beside them.
//
// Overlaps: two lamps add, so they can't give the original's XOR, where
// both layers together are dark. Here the overlap is an authored
// subtraction (ROOM_OVERLAP): where both gobos pass light, each beam keeps
// only a small share, so the original's moiré keeps its dark lobes and
// reads as interference, coloured by which layer lights each part.

// ---- Tunables (all authored) -------------------------------------------------
// Wall samples per pixel, spread over a small defocus disc (screen units; a
// tile is .067 wide) and over the rings' 1/100 s shutter.
const ROOM_SAMPLES=6;
const ROOM_DEFOCUS=.004;
// The tubes' plane, in rows in front of the wall's backing (with the default
// bump shape the bumps stand about 1.7 rows proud of it, 3.4 as hemispheres),
// and a far greater height whose light stands in for the wide, soft pool a
// sign throws on its wall.
const ROOM_TUBE_GAP=9.0;
const ROOM_POOL_GAP=45.0;
// Neon on the wall: the near light, the wide pool, and the chrome's
// reflection of the tubes (sharp and broad lobes, with their roughness in
// radians).
const ROOM_NEON_NEAR=.45;
const ROOM_NEON_POOL=.25;
const ROOM_NEON_MIRROR=vec2f(.55,.25);
const ROOM_NEON_ROUGH=vec2f(.06,.35);
// The projectors: the direction from the wall's centre towards each (layer
// 0 cool, up and to the left; layer 1 warm, up and to the right; both about
// 41 degrees off the wall's normal), how far away they stand (rows), and
// their colours, close to the scroller's cool and warm light (scroller.wgsl,
// heat), and their brightness. The lamps keep the original's order of
// brightness: grey 48 (layer 0) gives about 2.4 times the light of grey 32
// (layer 1), and the cool lamp gives about twice the warm one's luminance,
// so the moiré's light and dark parts fall much as they do there, and the
// colour adds which layer lights each part.
const ROOM_COOL_FROM=vec3f(-.40,.52,.75);
const ROOM_WARM_FROM=vec3f(.40,.52,.75);
const ROOM_THROW=900.0;
const ROOM_COOL=vec3f(.22,.55,1.0);
const ROOM_WARM=vec3f(1.0,.30,.06);
const ROOM_LAMP=vec2f(3.0,1.9);
// The projectors' focus: the bands' edges blurred over this many band
// pixels on the backing. Softer bands beat less against the 8-pixel bump
// grid (the third moiré, see room_beam); sharper ones keep the moiré's
// small checks.
const ROOM_FOCUS=.9;
// How much the beams fall off towards the gate's corners, and the softness
// of the gate's edge (source pixels): the original's ring window.
const ROOM_FALLOFF=.35;
const ROOM_GATE=3.0;
// The width of the tubes' shadows' penumbra, in rows.
const ROOM_PENUMBRA=.8;
// Where both gobos pass light, the share of each beam that remains: 1 adds
// them as lamps do (blue and orange mix to a pale third tone, which washes
// out the moiré), 0 is the original's XOR (both dark). The small remainder
// leaves a dim lilac there.
const ROOM_OVERLAP=.08;
// The satin chrome: the share of the light it scatters broadly, its broad
// gloss and its sharp glint (Blinn-Phong exponents and weights), and how
// much darker the backing between the bumps is.
const ROOM_SATIN=.65;
const ROOM_GLOSS=vec2f(16.0,.35);
const ROOM_GLINT=vec2f(220.0,.18);
const ROOM_BACKING=.7;
// How far the broad share wraps past the terminator (0: Lambert).
const ROOM_WRAP=.4;
// Musical lighting (L) only: the projector lamps flare a little on the bass.
const ROOM_BEAT=.35;

// ---- Geometry ----------------------------------------------------------------
// The scroller's eye, in its world units (100 rows each, wall.wgsl).
const ROOM_EYE=vec3f(0,0,5.5);
// The wall's backing, in scene rows.
fn room_backing()->f32 {return WALL_Z*100.0;}
// The tubes' plane, and the scale that carries a scene point into the space
// where the neon helpers expect it: the tubes' plane seen from the eye at
// their screen positions. A uniform scale about the screen centre, so
// angles and directions are unchanged.
fn room_tube_z()->f32 {return room_backing()+ROOM_TUBE_GAP;}
fn room_tube_scale(z:f32)->f32 {return EYE_Z/(EYE_Z-z);}
// The source pixel at which the camera sees a point of the backing (scene).
fn room_backing_source(q:vec2f)->vec2f {return source_of_scene(q/wall_scale());}
// A projector's position (scene rows).
fn room_projector(layer:u32)->vec3f {
    let toward=select(ROOM_COOL_FROM,ROOM_WARM_FROM,layer==1u);
    return vec3f(0,0,room_backing())+normalize(toward)*ROOM_THROW;
}

// ---- Materials ---------------------------------------------------------------
// The wall's metal, as scroller.wgsl's wall_metal: plain silver for the
// chrome wall (the default), or tinted by the original tile image.
fn room_metal(at:vec3f)->vec3f {
    let silver=vec3f(.56,.60,.66);
    if u.backdrop.z>1.5 {return silver;}
    let image=mix(tile_image(vec2f(at.x*8.0-.5,at.y*8.0+.5)),
        tile_image(floor(at.xy)*8.0+vec2f(3.5,4.5)),clamp(u.backdrop.w,0.0,1.0)*.6)*4.6;
    return mix(silver*.25,image,.85);
}
// Satin chrome lit from unit direction l, seen from v: the radiance per
// unit irradiance across the beam.
fn room_brdf(n:vec3f,l:vec3f,v:vec3f,metal:vec3f)->vec3f {
    let nl=max(dot(n,l),0.0);
    let h=normalize(l+v);
    let nh=max(dot(n,h),0.0);
    let gloss=(ROOM_GLOSS.x+8.0)/8.0*pow(nh,ROOM_GLOSS.x)*ROOM_GLOSS.y;
    let glint=(ROOM_GLINT.x+8.0)/8.0*pow(nh,ROOM_GLINT.x)*ROOM_GLINT.y;
    // The broad share wraps a little past the terminator, so the bumps'
    // shading stays below the bands' contrast.
    let satin=ROOM_SATIN*max(dot(n,l)+ROOM_WRAP,0.0)/(1.0+ROOM_WRAP);
    return metal*(satin+nl*(gloss+glint))/PI;
}

// ---- Neon on the wall --------------------------------------------------------
// One mask's light on the wall near scene point p, from tubes `gap` rows in
// front of the backing: the direction it arrives from and its strength, in
// units of NEON_GAS (as rings_common.wgsl's neon_light_one, but without the
// surface's cosine, so that a pixel's samples can share it, and in the
// tubes' own plane, see room_tube_scale).
fn room_neon_light(mask:i32,p:vec3f,gap:f32)->vec4f {
    let z=room_backing()+gap;
    let k=room_tube_scale(z);
    let q=p*k;
    let reach=(z-p.z)*k+NEON_RADIUS;
    let sigma=.75*reach;
    let s=source_of_scene(q.xy);
    let f=neon_map(mask,s,sigma);
    if f<1e-5 {return vec4f(0,0,1,0);}
    let e=.5*sigma;
    let g=vec2f(neon_map(mask,s+vec2f(e,0),sigma)-neon_map(mask,s-vec2f(e,0),sigma),
        neon_map(mask,s+vec2f(0,e),sigma)-neon_map(mask,s-vec2f(0,e),sigma))/(2.0*e);
    let offset=clamp(g*sigma*sigma/f,vec2f(-3.0*sigma),vec2f(3.0*sigma));
    let l=normalize(vec3f(offset.x*COLUMN,-offset.y,reach));
    return vec4f(l,NEON_LIGHT.x*PI*f);
}
// The lit masks' light near the pixel: near and pool terms for the current
// and previous masks, each a direction and a strength.
struct RoomNeon { near:vec4f, pool:vec4f, near_old:vec4f, pool_old:vec4f }
fn room_neon(p:vec3f)->RoomNeon {
    var n=RoomNeon(vec4f(0,0,1,0),vec4f(0,0,1,0),vec4f(0,0,1,0),vec4f(0,0,1,0));
    let on=neon_on();let old=neon_previous_on();
    if on>0.0 {
        n.near=room_neon_light(neon_mask(),p,ROOM_TUBE_GAP)*vec4f(1,1,1,on*ROOM_NEON_NEAR);
        n.pool=room_neon_light(neon_mask(),p,ROOM_POOL_GAP)*vec4f(1,1,1,on*ROOM_NEON_POOL);
    }
    if old>0.0 {
        n.near_old=room_neon_light(neon_previous(),p,ROOM_TUBE_GAP)*vec4f(1,1,1,old*ROOM_NEON_NEAR);
        n.pool_old=room_neon_light(neon_previous(),p,ROOM_POOL_GAP)*vec4f(1,1,1,old*ROOM_NEON_POOL);
    }
    return n;
}
// One term's light on a surface with unit normal n.
fn room_neon_on(light:vec4f,n:vec3f)->f32 {
    if light.w<=0.0 {return 0.0;}
    return light.w*max(dot(n,light.xyz),0.0);
}
// The tubes as the chrome reflects them: the light arriving at scene point
// p along unit r, blurred by roughness (radians), skipping unlit masks.
fn room_neon_mirror(p:vec3f,r:vec3f,rough:f32)->f32 {
    let z=room_tube_z();
    let k=room_tube_scale(z);
    let hit=neon_plane_hit(p*k,r,z*k);
    if hit.w<0.0 {return 0.0;}
    let s=source_of_scene(hit.xy);
    let sigma=max(rough*hit.w,.6);
    var f=0.0;
    if neon_on()>0.0 {f+=neon_map(neon_mask(),s,sigma)*neon_on();}
    if neon_previous_on()>0.0 {f+=neon_map(neon_previous(),s,sigma)*neon_previous_on();}
    return f;
}

// ---- Projectors --------------------------------------------------------------
// A projector's beam towards scene point p, arriving from unit direction l:
// the share its gobo passes, and the share that arrives. The beam is
// followed on to the backing, where the camera sees the original's pattern,
// and the band there read (ring_cover, with the projector's focus and the
// 1/100 s shutter); the gate is the original's ring window. What arrives
// is that, a little less towards the gate's corners, less the lit tubes'
// shadow where the beam crosses their plane.
//
// The third moiré: the bumps sit on an 8-pixel grid and the bands repeat
// every 14 pixels, finer than two bumps, so a pattern sampled bump by bump
// would alias against the grid, to bands about 19 pixels apart. Here the
// beam is resolved across each bump rather than per bump, so the bands run
// on over the bumps' faces; both lamps come from above, so the bumps' own
// shading is the same for both colours and doesn't split a bump in two; the
// satin share wraps past the terminator (ROOM_WRAP), keeping that shading
// below the bands' contrast; and the projectors' slightly soft focus
// (ROOM_FOCUS) rounds the bands' square edges, whose harmonics beat with
// the grid soonest. What remains is a fine beading along the bands' edges.
fn room_beam(layer:u32,p:vec3f,l:vec3f,shutter:f32,blur:f32)->vec2f {
    let q=p-l*(p.z-room_backing())/l.z;
    let s=room_backing_source(q.xy);
    let gobo=ring_cover(layer,s,RING_FAITHFUL,shutter,blur)*ring_window(s,ROOM_GATE);
    if gobo<=0.0 {return vec2f(0);}
    // A mild hot spot: the beam falls off a little towards the gate's corners.
    let r=(s-vec2f(160,100))/vec2f(128,92);
    var lit=gobo*(1.0-ROOM_FALLOFF*smoothstep(.2,1.4,dot(r,r)));
    // The tubes' shadows.
    let z=room_tube_z();
    if p.z<z {
        let k=room_tube_scale(z);
        let t=p+l*(z-p.z)/l.z;
        let at=source_of_scene(t.xy*k);
        let edge=vec2f(NEON_RADIUS-ROOM_PENUMBRA*.5,NEON_RADIUS+ROOM_PENUMBRA*.5);
        if neon_on()>0.0 {lit*=1.0-neon_on()*(1.0-smoothstep(edge.x,edge.y,tube_distance(neon_mask(),at)));}
        if neon_previous_on()>0.0 {lit*=1.0-neon_previous_on()*(1.0-smoothstep(edge.x,edge.y,tube_distance(neon_previous(),at)));}
    }
    return vec2f(gobo,lit);
}

// ---- The wall ----------------------------------------------------------------
// One sample of the wall through screen point xy, at a moment in the
// shutter (0 now, 1 at its opening), with the bands' edges blurred over
// `blur` band pixels: radiance, before the wall's presence. `shadows`
// traces the bumps' shadows in the projectors' light (bumps_pass).
fn room_sample(xy:vec2f,shutter:f32,blur:f32,neon:RoomNeon,shadows:bool)->vec3f {
    let ray=normalize(vec3f(xy,-5.5));
    let hit=wall_trace(ROOM_EYE,ray,false);
    let world=ROOM_EYE+ray*hit.t;
    let p=world*100.0;
    let n=hit.n;
    let v=-ray;
    var metal=room_metal(hit.at);
    if !hit.bump {metal*=ROOM_BACKING;}
    var c=vec3f(0);
    // The projectors.
    // Each lamp fades as the original's palette does, in DAC units of its
    // grey, so the fade looks as it does there.
    let i=ring_intensity();
    let level=vec2f(dac_linear(48.0*i)/dac_linear(48.0),dac_linear(32.0*i)/dac_linear(32.0))
        *(1.0+ROOM_BEAT*cue_bass()*musical_lighting());
    if i>0.0 {
        var light=array<vec3f,2>(vec3f(0),vec3f(0));
        var cover=vec2f(0);
        for(var k=0u;k<2u;k++) {
            // The beam's cover counts even on a face turned away from it,
            // for the overlap rule below.
            let l=normalize(room_projector(k)-p);
            let beam=room_beam(k,p,l,shutter,blur);
            cover[k]=beam.x;
            if beam.y<=0.0 || dot(n,l)<= -ROOM_WRAP {continue;}
            var clear=1.0;
            if shadows {clear=bumps_pass(grid_point(world+n*.002),grid_direction(l));}
            light[k]=select(ROOM_COOL,ROOM_WARM,k==1u)*ROOM_LAMP[k]*room_brdf(n,l,v,metal)*clear*beam.y;
        }
        // Where both gobos pass light, only ROOM_OVERLAP of it remains, so a
        // tube's shadow in one beam doesn't uncover the other.
        let s=1.0-ROOM_OVERLAP;
        c+=light[0]*(1.0-s*cover[1])*level.x+light[1]*(1.0-s*cover[0])*level.y;
    }
    // The neon: its light on the bumps turned to the tubes, and the tubes
    // in their chrome.
    let lit=room_neon_on(neon.near,n)+room_neon_on(neon.pool,n)
        +room_neon_on(neon.near_old,n)+room_neon_on(neon.pool_old,n);
    var glow=NEON_GAS*metal*lit*ROOM_SATIN/PI;
    if neon_on()+neon_previous_on()>0.0 {
        let r=reflect(ray,n);
        let mirror=room_neon_mirror(p,r,ROOM_NEON_ROUGH.x)*ROOM_NEON_MIRROR.x
            +room_neon_mirror(p,r,ROOM_NEON_ROUGH.y)*ROOM_NEON_MIRROR.y;
        glow+=NEON_LIGHT*metal*mirror;
    }
    return c+glow;
}
// The wall's presence: the wall strength keys (- and =), 1 at the default.
fn room_presence()->f32 {return u.backdrop_motion.z/.6;}
// The wall at the pixel: ROOM_SAMPLES samples over the defocus disc and
// the shutter, stratified in an order that doesn't follow the disc.
fn room_wall(px:RingsPixel)->vec3f {
    let base=vec3f(px.scene*wall_scale(),room_backing());
    let neon=room_neon(base);
    var c=vec3f(0);
    let n=f32(ROOM_SAMPLES);
    for(var i=0;i<ROOM_SAMPLES;i++) {
        let f=f32(i);
        let disc=sqrt((f+.5)/n)*ROOM_DEFOCUS*vec2f(cos(f*2.39996),sin(f*2.39996));
        let shutter=fract((f*5.0+.5)/n);
        c+=room_sample(px.xy+disc,shutter,max(ROOM_FOCUS,px.pixel),neon,true);
    }
    return c/n*room_presence();
}
// The wall as seen through a tube's glass, bent: one sample, no shadows.
fn room_wall_seen(px:RingsPixel,bend:vec2f)->vec3f {
    let xy=px.xy+vec2f(bend.x*COLUMN,-bend.y)/100.0;
    let base=vec3f(xy*100.0*wall_scale(),room_backing());
    return room_sample(xy,.5,ROOM_FOCUS,room_neon(base),false)*room_presence();
}

fn style_color(px:RingsPixel)->vec3f {
    // Black while the original waits for 06:00; then the PSTART picture
    // exactly as the original fades it in, which part 5 opens on.
    if rings_phase()<.5 {return vec3f(0);}
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let behind=room_wall(px);
    let tubes=neon_glass(px.source,px.pixel);
    var seen=behind;
    if tubes.glass>0.0 {seen=room_wall_seen(px,tubes.bend);}
    return neon_over(behind,seen,tubes)+neon_haze(px.source);
}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
