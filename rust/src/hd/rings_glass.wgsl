// Glass (authored): the owner's pick. Each ring layer is a clear glass plate
// whose set bands are raised rods: layer 0's plate in front, layer 1's a
// little behind it. Each plate is lit through its own edge by its own lamp,
// cool for layer 0 and warm for layer 1, so its rods glow from inside in that
// colour, brightest along their walls (GLASS_LIGHTING 0; 1 is clear glass
// with no colour, 2 lights both plates blue from the left and warm from the
// right). The back plate is seen through the
// front one, bent by its rods. The lit symbols are neon tubes in front of
// both plates, reflected in the rods and shining through them onto a dark
// backing.
// Module: the shared controls, rings_common.wgsl, wall.wgsl, this file and
// rings_style.wgsl. Prefix this file's own names with glass_.
//
// Where it comes from:
// - From the port, through the shared helpers: the bands' geometry, both
//   layers' centres (each plate slides exactly on its layer's, with the
//   shared 1/100 s shutter), the ring window, the ring intensity and its
//   fades, the lit symbol and its windows, and the PSTART picture.
// - Authored: the plates, rods, lamps and depths, the light in and through
//   the glass, the reflections, the plates' edges and the neon's colour.
// - The original's palette draws the two layers grey 48 and grey 32, and
//   black where both are set or neither is. Here a layer's rods glow on
//   their own; where rods of both plates cross, an authored rule turns
//   their light aside (as crossed prisms would; not traced), so crossings
//   stay dark glass and the moire reads as the original's.

// ---- Tunables ---------------------------------------------------------------
// Depths in rows (scene units), below the tubes, which lie in the screen
// plane z = 0 at the original symbols' positions.
const GLASS_DEPTH=14.0;     // The front plate's face.
const GLASS_GAP=3.5;        // From the front plate's face to the back one's.
const GLASS_FLOOR=14.0;     // From the back plate's face to the dark backing.
const GLASS_ROD=1.8;        // A rod's height above its plate.
const GLASS_IOR=1.5;
// How the plates are lit:
// 0 layers: each plate through its own edge, the front one (layer 0) cool
//   and the back one (layer 1) warm, as the owner first asked.
// 1 clear: clear glass only, no colour: both plates lit white, the front
//   one brighter, as the original's grey 48 to its grey 32.
// 2 sides: clear glass lit blue only from the left side and warm only
//   from the right, both plates alike (the front one brighter); each lamp's
//   light fades across the plate and lights the rods' walls facing it.
const GLASS_LIGHTING=0;
// The layers' edge lamps: colour and level, and the way their light runs
// through each plate (scene axes): cool in from the upper left, warm from
// the lower right, as the scroller's key and warm lamps.
const GLASS_COOL=vec3f(.10,.42,1.0)*1.3;
const GLASS_WARM=vec3f(1.0,.30,.06)*1.1;
const GLASS_COOL_RUN=vec2f(.857,-.514);
const GLASS_WARM_RUN=vec2f(-.894,.447);
// Clear: the white lamps for the front and back plates.
const GLASS_WHITE_FRONT=vec3f(1.0)*1.0;
const GLASS_WHITE_BACK=vec3f(1.0)*.55;
// Sides: the lamps at the left and right, each plate's share (front, back)
// and how much of a lamp's light is left at the far side of a plate.
const GLASS_SIDE_BLUE=vec3f(.10,.42,1.0)*1.4;
const GLASS_SIDE_WARM=vec3f(1.0,.30,.06)*1.2;
const GLASS_SIDE_PLATES=vec2f(1.0,.6);
const GLASS_SIDE_FAR=.08;
// A rod's glow from a lamp: the body, and the brighter line just inside
// each wall, where the light leaves it (strongest on the walls facing one
// way: for an edge lamp away from it, for a side lamp towards it).
const GLASS_BODY=.40;
const GLASS_RIM=1.6;
// For clear glass (lighting 1 and 2): a fainter body, so the rods look
// clear, and more light at the walls; the share of a rim's light on walls
// facing the other way (layers: .2).
const GLASS_CLEAR_BODY=.18;
const GLASS_CLEAR_RIM=2.2;
const GLASS_CLEAR_SPREAD=.6;
const GLASS_SIDE_SPREAD=.05;
// Where rods of both plates cross, the share of their light that still
// reaches the eye (the original's XOR draws both-set as black).
const GLASS_CROSSING=.05;
// The neutral softbox the rods reflect (follows the lamps).
const GLASS_SOFTBOX=1.4;
// The neon: its reflections in the glass, the share of its light the rods
// gather and let out, and the backing's albedo.
const GLASS_GLINT=1.0;
const GLASS_GATHER=.10;
const GLASS_BACKING=.07;
// With the lamps off (symbols on black), how much the neon still catches the
// glass: 0 keeps it black, as the original; 1 shows the glass by its light.
const GLASS_DARK_CATCH=.6;

// The neon's colour in this style. The shared tubes are amber (NEON_GAS),
// which the light on the glass blends into the warm plate's orange; lemon,
// a little warmer than the original's average yellow (SYMBOL_YELLOW),
// keeps them apart. Warm white is the third alternative shown.
const GLASS_GAS_AMBER=NEON_GAS;
const GLASS_GAS_LEMON=vec3f(1.0,.86,.10);
const GLASS_GAS_WHITE=vec3f(1.0,.80,.50);
const GLASS_GAS=GLASS_GAS_LEMON;
// The plates' polished edges at the ring window: the lamp light that
// crossed the plate leaves through them, brightest at the lamp's own edge
// (cool on the left, warm on the right).
const GLASS_EDGE=.6;
// The shared haze round the tubes, scaled.
const GLASS_HAZE=.7;

// ---- Helpers ----------------------------------------------------------------
// Steady with studio lighting; with musical lighting (L), the reactive level.
fn glass_music(studio:f32,reactive:f32)->f32 {return mix(studio,reactive,musical_lighting());}
// The lamps' level through the original's fades: its palette steps the
// greys linearly in DAC units (ring_intensity, read from the port), so the
// lamps follow the light grey 48 gives on the way.
fn glass_fade()->f32 {return dac_linear(48.0*ring_intensity())/dac_linear(48.0);}

// One plate's rod at source point s: coverage of the set band (antialiased
// over the pixel and blurred over the shutter by the centre's movement
// across the band), the position across the rod (-1 at its inner edge, 1 at
// the outer), the radial direction outwards and the face's height gradient,
// both in scene axes, and the face's curvature (rows per pixel squared).
struct GlassRod { cover:f32, across:f32, out:vec2f, grad:vec2f, curve:f32 }
fn glass_rod(layer:u32,s:vec2f,pixel:f32)->GlassRod {
    let d=s-ring_centre(layer,.5);
    let r=max(length(d),1e-4);
    let dir=d/r;
    let motion=abs(dot(ring_centre(layer,0.0)-ring_centre(layer,1.0),dir));
    let blur=sqrt(pixel*pixel*1.2+motion*motion);
    let cover=clamp(band_distance(r)/max(blur,1e-3)+.5,0.0,1.0);
    // 0..1 across a set band, 1..2 across the clear band after it.
    let t=fract((r-ring_first())/(2.0*ring_width()))*2.0;
    let a=select(select(-1.0,1.0,t<1.5),2.0*t-1.0,t<1.0);
    // Profile h = GLASS_ROD (1 - a^4): a flat top and rounded shoulders.
    let w=ring_width();
    let dhdr=-8.0*GLASS_ROD*a*a*a/w;
    let toward=vec2f(dir.x/COLUMN,-dir.y);
    return GlassRod(cover,a,normalize(toward),dhdr*toward*cover,48.0*GLASS_ROD*a*a/(w*w)*cover);
}
fn glass_normal(rod:GlassRod)->vec3f {return normalize(vec3f(-rod.grad,1));}
// A rod's glow from a lamp whose light leaves it most through the walls
// facing `toward` (scene axes): the body, and the rims with `spread` of
// their light on the walls facing away.
fn glass_shine(rod:GlassRod,toward:vec2f,body:f32,rims:f32,spread:f32)->f32 {
    let x=abs(rod.across);
    let wall=sign(rod.across)*rod.out;
    let facing=max(dot(wall,toward),0.0);
    let rim=exp(-pow((x-.82)/.10,2.0));
    let edge=1.0-.5*smoothstep(.93,1.0,x);
    return (body*(.75+.5*facing*x)+rims*rim*(spread+(1.0-spread)*facing))*edge;
}
fn glass_glow(rod:GlassRod,toward:vec2f)->f32 {
    if GLASS_LIGHTING==1 {return glass_shine(rod,toward,GLASS_CLEAR_BODY,GLASS_CLEAR_RIM,GLASS_CLEAR_SPREAD);}
    if GLASS_LIGHTING==2 {return glass_shine(rod,toward,GLASS_CLEAR_BODY,GLASS_CLEAR_RIM,GLASS_SIDE_SPREAD);}
    return glass_shine(rod,toward,GLASS_BODY,GLASS_RIM,.2);
}
fn glass_fresnel(c:f32)->f32 {return .04+.96*pow(1.0-clamp(c,0.0,1.0),5.0);}
// The neutral softbox and a dim top light, as reflected along r.
// Off the axis, so the flat glass doesn't show it: only the rods'
// shoulders turned towards it catch it, as a line along each rod.
fn glass_softbox(r:vec3f)->vec3f {
    let soft=(r.xy-vec2f(-.45,.55))*vec2f(1.0,1.4);
    var c=vec3f(.9,.95,1.0)*exp(-dot(soft,soft)*9.0)*smoothstep(0.0,.3,r.z);
    c+=vec3f(.30,.32,.35)*exp(-pow((r.y-.85)*5.0,2.0))*smoothstep(0.0,.3,r.z);
    return c;
}
// ---- The neon, in this style's colour ----------------------------------------
// Local copies of rings_common.wgsl's neon_tube and neon_glass with the gas
// colour as a parameter, so the tubes can be tinted here; otherwise the
// same. The light-map helpers return NEON_GAS times a level, so their
// results are recoloured exactly.
fn glass_recolour(c:vec3f)->vec3f {return c.r/NEON_GAS.r*GLASS_GAS;}
fn glass_tube(d:f32,end:f32,across:vec2f,footprint:f32,gas_color:vec3f)->NeonGlass {
    let x=min(d/NEON_RADIUS,1.0);
    let glass=clamp((NEON_RADIUS-d)/max(footprint,1e-3)+.5,0.0,1.0);
    let lit=select(smoothstep(.25,1.0,end/NEON_RADIUS/NEON_ELECTRODE),1.0,NEON_ELECTRODE<=0.0)
        *mix(.55,1.0,smoothstep(0.0,1.5,end/NEON_RADIUS));
    let c=x/NEON_COLUMN;
    let chord=sqrt(max(1.0-c*c,0.0));
    let column=clamp((1.0-c)*NEON_RADIUS*NEON_COLUMN/max(footprint,1e-3)+.5,0.0,1.0);
    let gas=gas_color*(NEON_WALL*mix(exp(-(c-1.0)*5.0)*.35,1.0,column)+NEON_HOT*pow(chord,6.0));
    let electrode=vec3f(.03,.027,.024)*column;
    let wall=smoothstep(.8,.92,x)*(1.0-smoothstep(.95,1.0,x));
    var light=mix(electrode,gas,lit)+gas_color*(NEON_GLASS_GLOW+.5*wall)*mix(.35,1.0,lit);
    let normal=vec3f(across*x,sqrt(1.0-x*x));
    let half_way=normalize(normalize(vec3f(-.45,.6,.65))+vec3f(0,0,1));
    let shown=step(1e-3,dot(across,across));
    light+=vec3f(1,.97,.9)*pow(max(dot(normal,half_way),0.0),80.0)*1.2*shown;
    let bend=vec2f(across.x*COLUMN,-across.y)*x*NEON_RADIUS*.6;
    return NeonGlass(light*glass,glass*mix(1.0,.35,max(column*lit,0.0)),bend);
}
fn glass_tube_at(mask:i32,s:vec2f,footprint:f32)->NeonGlass {
    let f=symbol_field(mask,s);
    if f.y>NEON_RADIUS+footprint {return NeonGlass(vec3f(0),0.0,vec2f(0));}
    let e=.5;
    let g=vec2f(symbol_field(mask,s+vec2f(e,0)).y-symbol_field(mask,s-vec2f(e,0)).y,
        symbol_field(mask,s+vec2f(0,e)).y-symbol_field(mask,s-vec2f(0,e)).y);
    let across=vec2f(g.x/COLUMN,-g.y);
    let n=select(vec2f(0),across/max(length(across),1e-5),length(across)>1e-4);
    return glass_tube(f.y,f.w,n,footprint,GLASS_GAS);
}
fn glass_tubes(s:vec2f,footprint:f32)->NeonGlass {
    var n=NeonGlass(vec3f(0),0.0,vec2f(0));
    for(var k=0;k<2;k++) {
        let mask=select(neon_mask(),neon_previous(),k==1);
        let on=select(neon_on(),neon_previous_on(),k==1);
        if on>0.0 {
            let t=glass_tube_at(mask,s,footprint);
            n.light+=t.light*on;
            n.bend+=t.bend*t.glass*on;
            n.glass=max(n.glass,t.glass*on);
        }
    }
    return n;
}
// The neon as a reflection from scene point p along r shows it: the tubes
// themselves where the reflection is sharp, their light map, blurred over
// the reflection's footprint in the tubes' plane, where a curved shoulder
// spreads it. `curve` is the surface's curvature (rows per pixel squared).
fn glass_glint(p:vec3f,r:vec3f,pixel:f32,curve:f32)->vec3f {
    let hit=neon_plane_hit(p,r,0.0);
    if hit.w<0.0 {return vec3f(0);}
    let footprint=pixel*(1.0+2.0*hit.w*curve);
    let soft=glass_recolour(neon_reflection(p,r,0.0,max(footprint,.6)/hit.w));
    if footprint>2.0 {return soft;}
    let sharp=glass_tubes(source_of_scene(hit.xy),footprint).light;
    return mix(sharp,soft,smoothstep(.8,2.0,footprint));
}
fn glass_neon_light(p:vec3f,n:vec3f)->vec3f {return glass_recolour(neon_irradiance(p,n,0.0));}
// Where the lit tubes' light comes from at scene point p, below them: the
// unit direction towards their centroid within about the distance to them,
// found from the light map's slope as neon_irradiance finds it.
fn glass_neon_toward(p:vec3f)->vec3f {
    let reach=-p.z+NEON_RADIUS;
    let sigma=.75*reach;
    let s=source_of_scene(p.xy);
    let e=.5*sigma;
    var f=0.0;
    var g=vec2f(0);
    for(var k=0;k<2;k++) {
        let mask=select(neon_mask(),neon_previous(),k==1);
        let on=select(neon_on(),neon_previous_on(),k==1);
        if on>0.0 {
            f+=neon_map(mask,s,sigma)*on;
            g+=vec2f(neon_map(mask,s+vec2f(e,0),sigma)-neon_map(mask,s-vec2f(e,0),sigma),
                neon_map(mask,s+vec2f(0,e),sigma)-neon_map(mask,s-vec2f(0,e),sigma))/(2.0*e)*on;
        }
    }
    if f<1e-5 {return vec3f(0,0,1);}
    let offset=clamp(g*sigma*sigma/f,vec2f(-3.0*sigma),vec2f(3.0*sigma));
    return normalize(vec3f(offset.x*COLUMN,-offset.y,reach));
}
// A plate's point (scene, on the plate `below` rows under the tubes) as the
// source point whose pattern it carries: each plate's pattern is scaled with
// its depth so that it projects exactly onto the original's.
fn glass_plate_source(q:vec2f,below:f32)->vec2f {return source_of_scene(q*EYE_Z/(EYE_Z+below));}
// The neon's light after crossing a plate's rods at scene point q on that
// plate (`below` rows under the tubes), which it reaches `depth` rows
// further down: clear rods bend it rather than block it, so beneath each
// rod there are dark edges and brighter lines inside them, spreading out
// with the depth (authored, not traced).
fn glass_lens(layer:u32,q:vec2f,below:f32,depth:f32)->f32 {
    let s=glass_plate_source(q,below);
    let rod=glass_rod(layer,s,.5+depth*.12);
    let x=abs(rod.across);
    let focus=1.0+.9*exp(-pow((x-.55)/.13,2.0))-.8*smoothstep(.72,.98,x);
    return mix(1.0,mix(focus,.6,smoothstep(4.0,16.0,depth)),rod.cover*ring_window(s,1.0));
}

// ---- The scene --------------------------------------------------------------
// A plate's polished edge at source point s, for a lamp on the left
// (side -1) or right (1): a thin line `at` rows inside the ring window
// (the back plate's lies just outside the front one's).
fn glass_edge(s:vec2f,blur:f32,side:f32,at:f32)->f32 {
    let inside=min(min(s.x-RING_WINDOW.x,RING_WINDOW.z-s.x),min(s.y-RING_WINDOW.y,RING_WINDOW.w-s.y));
    let line=exp(-pow((inside-at)/max(.4,blur),2.0));
    let across=clamp((s.x-RING_WINDOW.x)/(RING_WINDOW.z-RING_WINDOW.x),0.0,1.0);
    let near=select(1.0-across,across,side>0.0);
    return line*(.06+.94*near*near)*GLASS_EDGE;
}
// A plate's light (layer 0 the front plate, 1 the back) at source point s
// with its rod there: the rod's glow, at `cover`, and the plate's edge,
// `at` rows inside the window, for GLASS_LIGHTING.
fn glass_lamps(layer:u32,rod:GlassRod,s:vec2f,blur:f32,at:f32,bass:f32,treble:f32,cover:f32)->vec3f {
    let front=layer==0u;
    if GLASS_LIGHTING==2 {
        let u=clamp((s.x-RING_WINDOW.x)/(RING_WINDOW.z-RING_WINDOW.x),0.0,1.0);
        let fade=mix(GLASS_SIDE_FAR,1.0,1.0-smoothstep(0.0,1.0,u));
        let rise=mix(GLASS_SIDE_FAR,1.0,smoothstep(0.0,1.0,u));
        let share=select(GLASS_SIDE_PLATES.y,GLASS_SIDE_PLATES.x,front);
        let blue=GLASS_SIDE_BLUE*bass*share;
        let warm=GLASS_SIDE_WARM*treble*share;
        return (blue*fade*glass_glow(rod,vec2f(-1,0))+warm*rise*glass_glow(rod,vec2f(1,0)))*cover
            +blue*glass_edge(s,blur,-1.0,at)+warm*glass_edge(s,blur,1.0,at);
    }
    var color=select(GLASS_WARM,GLASS_COOL,front);
    if GLASS_LIGHTING==1 {color=select(GLASS_WHITE_BACK,GLASS_WHITE_FRONT,front);}
    let run=select(GLASS_WARM_RUN,GLASS_COOL_RUN,front);
    return color*select(treble,bass,front)*(glass_glow(rod,run)*cover+glass_edge(s,blur,select(1.0,-1.0,front),at));
}
// A rod's rims alone, where light gathered in it leaves.
fn glass_rims(rod:GlassRod)->f32 {return exp(-pow((abs(rod.across)-.82)/.12,2.0));}
// The plates and the backing at source point s (the pixel's, or bent through
// a tube), lamps at level `lit` and the neon's light on the glass at `caught`.
fn glass_scene(px:RingsPixel,s:vec2f,lit:f32,caught:f32)->vec3f {
    let ray=camera_ray(px.xy);
    let up=vec3f(0,0,1);
    let blur=px.pixel;
    // Lamp levels. With musical lighting the cool (or left, or front)
    // lamp follows the bass and the warm (right, back) one the treble (the
    // shared cues measured from the PCM).
    let bass=lit*glass_music(1.0,.45+cue_level()*.35+cue_bass()*cue_bass()*1.8);
    let treble=lit*glass_music(1.0,.45+cue_level()*.35+pow(cue_treble(),1.5)*1.6);
    // The front plate: its rod, the surface's reflection and the light it
    // lets through.
    let window0=ring_window(s,blur*1.5);
    let p0=vec3f(scene_of_source(s)*(EYE_Z+GLASS_DEPTH)/EYE_Z,-GLASS_DEPTH);
    let f=glass_rod(0u,s,blur);
    let n0=normalize(mix(up,glass_normal(f),window0));
    let f0=glass_fresnel(dot(n0,-ray));
    let r0=reflect(ray,n0);
    var c=f0*(glass_softbox(r0)*GLASS_SOFTBOX*lit+glass_glint(p0,r0,blur,f.curve)*GLASS_GLINT*caught)*window0;
    // Through the rod and out of the plate's flat back; total internal
    // reflection there shows nothing behind.
    let inside=refract(ray,n0,1.0/GLASS_IOR);
    let out=refract(inside,up,GLASS_IOR);
    let clear=dot(out,out)>.5;
    let through=select(0.0,1.0-f0,clear);
    let below=select(ray,out,clear);
    let shift=(below.xy/-below.z-ray.xy/-ray.z)*GLASS_GAP;
    let s1=s+vec2f(shift.x/COLUMN,-shift.y);
    // The back plate, as seen through the front one.
    let window1=ring_window(s1,blur*1.5);
    let p1=vec3f(scene_of_source(s1)*(EYE_Z+GLASS_DEPTH+GLASS_GAP)/EYE_Z,-GLASS_DEPTH-GLASS_GAP);
    let b=glass_rod(1u,s1,blur);
    let n1=normalize(mix(up,glass_normal(b),window1));
    let f1=glass_fresnel(dot(n1,-below));
    let r1=reflect(below,n1);
    let a0=f.cover*window0;
    let a1=b.cover*window1;
    // Each plate's glow, cut where rods of the other cross it, and its edge.
    c+=glass_lamps(0u,f,s,blur,.5,bass,treble,a0*mix(1.0,GLASS_CROSSING,a1));
    let glow1=glass_lamps(1u,b,s1,blur,-.45,bass,treble,a1*mix(1.0,GLASS_CROSSING,a0));
    // The neon's light gathered in the rods, let out along their rims; the
    // back plate's comes through the front one's rods.
    c+=glass_neon_light(p0,up)*GLASS_GATHER*glass_rims(f)*a0*caught;
    var back=glow1+f1*(glass_softbox(r1)*GLASS_SOFTBOX*lit+glass_glint(p1,r1,blur,b.curve)*GLASS_GLINT*caught)*window1;
    // The dark backing, lit by the neon through both plates.
    let p2=vec3f(p1.xy,-GLASS_DEPTH-GLASS_GAP-GLASS_FLOOR);
    let neon2=glass_neon_light(p2,up)*caught;
    let neon1=glass_neon_light(p1,up)*caught;
    var backing=vec3f(0);
    if max(neon1.r,neon2.r)>1e-4 {
        let l=glass_neon_toward(p2);
        let lens1=glass_lens(0u,p1.xy+l.xy*(GLASS_GAP/l.z),GLASS_DEPTH,GLASS_GAP);
        back+=neon1*GLASS_GATHER*glass_rims(b)*a1*lens1;
        backing=neon2*GLASS_BACKING*glass_lens(1u,p2.xy+l.xy*(GLASS_FLOOR/l.z),GLASS_DEPTH+GLASS_GAP,GLASS_FLOOR)
            *glass_lens(0u,p2.xy+l.xy*((GLASS_FLOOR+GLASS_GAP)/l.z),GLASS_DEPTH,GLASS_FLOOR+GLASS_GAP);
    }
    back+=backing*(1.0-f1)*window1;
    c+=back*through;
    return c;
}

// Black while part 6 waits for 06:00, as the original; from 80.36 s the
// PSTART picture exactly as faithful HD shows it, so part 5 can take over at
// 81.27 s. In between the lamps follow the ring intensity, and the neon
// stays on the glass at GLASS_DARK_CATCH while they are off.
fn style_color(px:RingsPixel)->vec3f {
    if rings_phase()<.5 {return vec3f(0);}
    if rings_phase()>1.5 {return tonemap_inverse(picture_color(px.source,picture_level()));}
    let lit=glass_fade();
    let caught=mix(GLASS_DARK_CATCH,1.0,ring_intensity());
    let behind=glass_scene(px,px.source,lit,caught);
    let tubes=glass_tubes(px.source,px.pixel);
    // Through the tubes' glass, the plates a little to the side.
    var seen=behind;
    if tubes.glass>0.0 {seen=glass_scene(px,px.source+tubes.bend,lit,caught);}
    return neon_over(behind,seen,tubes)+glass_recolour(neon_haze(px.source))*GLASS_HAZE;
}
fn style_prepass(px:RingsPixel)->vec4f {return vec4f(0);}
