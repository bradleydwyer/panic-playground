// The modern part 7: a lighthouse in dusty air. Authored throughout, on the
// port's timing, positions and motion (greetings_common.wgsl), its letters
// derived from the user's data (greetings_type.rs).
//
// The original turns 64 colours over a field of stars that never move, so
// a broad band of light sweeps round (160, 85) once every 64 frames; five
// strips of letters rise through it, each letter row one of 16 stored views
// of a turning column. Here the sweep is one hard cream spotlight in a room
// of dusty air, turning once every 64 frames, its direction the original's
// band's peak:
// - The lamp stands a little behind the strips, where it projects on the
//   sweep's centre, a bright core with a halo, and the beam leaves its
//   aperture, of finite size, tilted a little toward the camera, so it
//   passes through the outer strips.
// - The air is a participating medium marched in 3D at half resolution: a
//   thin haze with slow, low-frequency thicker and thinner regions drifting
//   on an authored wind, lit only inside the cone, the cone's light falling
//   off with the square of the distance, softer toward its edge (a
//   penumbra that grows with distance, as from an aperture), and scattered
//   toward the camera by a Henyey-Greenstein phase function, so the beam
//   brightens as it turns toward the lens. The cubes shadow it through a
//   map of what the strips let through, seen from the lamp (a colour
//   transmission, so letters could tint or shape the light), so the shafts
//   and shadow wedges are real 3D. The beam's motion is integrated over
//   the shutter along each march.
// - The dust is motes in the air, seeded from the original's stars (each
//   star's screen position, a few motes at depths along it, around and in
//   front of the strips) and drifting slowly on the same authored wind
//   (the original's stars never move): lit by the beam, forward scattering
//   strongly, and all but invisible outside it; the near ones soft discs out
//   of focus, the far ones fine points.
// - Each letter is a cube of polished black with rounded edges, the letter
//   inset on each of its four faces as a panel of frosted light with a
//   border of the black round it, coloured as the original shades its
//   letters (white crowns, teal edges) and always readable. The lamp lights
//   the cubes all round and the beam more; the polish mirrors the dust, the
//   lit air and the lamp. The cubes turn as the original's sine turns each
//   row of a letter (5.625 degrees a step of its view index, continuous),
//   and the gaps between them stay open (faithful HD keeps the smear).
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
const BEAM_HALF=.15;
const APERTURE=2.6;
const PENUMBRA=1.2;
const PENUMBRA_GROWTH=.08;
// The beam's power, the distance within which its inverse-square falloff
// levels off, and how much brighter its axis is than its edge.
const BEAM_POWER=1.0;
const FALLOFF=70.0;
const CORE_BIAS=.45;
// The air: its scattering, the phase function's asymmetry, its thinnest
// and thickest (relative), the noise levels between which it thickens, its
// scale (rows a tile of the noise) and the wind (rows a second; the same
// carries the dust).
const SCATTER=.07;
const PHASE_G=.62;
const THINNEST=.3;
const THICKEST=2.0;
const THICKENS=vec2f(.3,.75);
const DENSITY_SCALE=170.0;
const WIND=vec3f(3.2,1.1,-2.4);
// The march: samples within the cone, the near and far depths it covers
// (scene z), and the shutter (70 Hz frames) its samples spread over.
const MARCH_SAMPLES=36u;
const AIR_NEAR=-420.0;
const AIR_FAR=240.0;
const SHUTTER=1.0;
// The strips' transmission seen from the lamp: the half extent of the map
// on the strips' plane (rows; greetings_render::TRANSMISSION_EXTENT) and
// the cubes' depth either side of it.
const TRANSMISSION_EXTENT=vec2f(240.0,180.0);
const SLAB=11.5;
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
// The air's swirl round the wind (rows), and its period (seconds).
const SWIRL=4.0;
const SWIRL_PERIOD=9.0;
// The cubes: half a side (greetings_type::FACE / 2), the edges' rounding,
// the slots (greetings::SLOT_ROWS, SLOTS), and the turn per step of the
// sine's view index (90 degrees over 16, read as a turn of the column).
const HALF=7.5;
const ROUNDING=1.15;
const SLOT_ROWS=18.0;
const SLOTS=16u;
const TWIST_STEP=.09817477;
const MARCH_STEPS=64u;
const LIPSCHITZ=.8;
// The letters' atlas (greetings_type::CELL, ATLAS_COLUMNS, ATLAS_SIZE).
const CELL=128.0;
const ATLAS_COLUMNS=8u;
const ATLAS_SIZE=vec2f(1024.0,512.0);
// The panels: inset in each face (the letter's square is this share of
// the face's side, leaving a border of polished black), their glow at rest
// and with the beam behind, the bevel's bump.
const PANEL_SCALE=.8;
const PANEL_REST=.55;
const PANEL_BEAM=1.3;
const BUMP=.35;
// The black: its albedo; the polish: reflectance at normal incidence,
// roughness (GGX alpha) of the black and of the panels, and the glints'
// strength; a broader sheen, so the faces catch the light as they turn
// toward it. The lamp lights the cubes all round with BULB_LIGHT and in the
// beam with BEAM_LIGHT more.
const BLACK=.02;
const SHEEN=.04;
const BLACK_ROUGH=.07;
const PANEL_ROUGH=.16;
const GLINT=30.0;
const SHEEN_ROUGH=.35;
const SHEEN_GLINT=6.0;
const BULB_LIGHT=.6;
const BEAM_LIGHT=2.0;
// What the polish mirrors: the picture behind the strips (the dust, the
// lit air and the lamp), and toward the camera the same picture mirrored
// about the strips' plane, as if it surrounded them (authored), dimmer;
// the mirror's strength, the depth it is taken at, and the rim where the
// beam passes behind.
const MIRROR=2.5;
const FRONT_MIRROR=.5;
const MIRROR_DEPTH=220.0;
const RIM=.6;
// The cubes' motion over their shutter (70 Hz frames; shorter than the
// beam's, so the letters stay sharp where they turn fastest): taps along it.
const CUBE_SHUTTER=.5;
const CUBE_TAPS=3u;
// The swing to the lens: it starts this many seconds before the original's
// fade to white (authored motion; the fade keeps its timing and levels),
// and the brightest the monster shows through the tonemap once developed
// (so it keeps its colours, near enough).
const SWING_LEAD=.45;
const MONSTER_WHITE=.9;

// ---- Bindings -----------------------------------------------------------------------
// The letters' faces (greetings_type.rs): signed distance (rows), relief,
// its slopes across and up.
@group(0) @binding(8) var glyphs:texture_2d<f32>;
// The dust motes (greetings::motes): across, up, depth (scene); the star's
// colour index plus its size (0 to 1).
@group(0) @binding(9) var<storage,read> motes:array<vec4f>;
// The air's density, a tiling, smooth noise (greetings_render::noise), and
// a sampler that repeats it.
@group(0) @binding(10) var density_noise:texture_3d<f32>;
@group(0) @binding(11) var repeat:sampler;
// The passes' images: the dust; the cubes' depth along each ray (camera
// rows; 1e9 where none); the strips' transmission seen from the lamp; the
// cubes (colour and coverage); the lit air (half size; its depth in a);
// where the cubes' polish mirrors the picture (its uv, and the weight).
@group(1) @binding(1) var dust_image:texture_2d<f32>;
@group(1) @binding(2) var depth_image:texture_2d<f32>;
@group(1) @binding(3) var transmission:texture_2d<f32>;
@group(1) @binding(4) var cubes_image:texture_2d<f32>;
@group(1) @binding(5) var air_image:texture_2d<f32>;
@group(1) @binding(6) var mirror_image:texture_2d<f32>;

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
// fitted turn), tilted toward the camera, and all the way to it as it
// swings to the lens.
fn beam_axis(back:f32)->vec3f {
    let phase=gc.sweep.x-back;
    let angle=(LAW_OFFSET-31.5+phase)*PI/32.0;
    let tilt=mix(BEAM_TILT,PI*.5,swing());
    // Source y runs down, the scene's up.
    return vec3f(cos(angle)*cos(tilt),-sin(angle)*cos(tilt),-sin(tilt));
}
// The beam's light at x (0 outside the cone): the cone's profile across,
// brighter on its axis and soft at its edge, and the falloff along it.
fn beam_at(x:vec3f,axis:vec3f)->f32 {
    let v=x-lamp();
    let along=dot(v,axis);
    if along<=0.0 {return 0.0;}
    let across=length(v-axis*along);
    let radius=APERTURE+along*tan(BEAM_HALF);
    let soft=PENUMBRA+along*PENUMBRA_GROWTH;
    let edge=1.0-smoothstep(radius-soft,radius+soft,across);
    let core=1.0-CORE_BIAS+CORE_BIAS*exp(-2.0*(across/radius)*(across/radius));
    return edge*core*BEAM_POWER*FALLOFF*FALLOFF/(FALLOFF*FALLOFF+along*along);
}
// Henyey-Greenstein, for light travelling along `light` scattered toward
// `view` (both unit vectors), normalised to 1 for an even scatter.
fn phase(light:vec3f,view:vec3f,g:f32)->f32 {
    let c=dot(light,view);
    return (1.0-g*g)/pow(1.0+g*g-2.0*g*c,1.5);
}
// What the strips let through toward x, from the lamp: x's direction from
// the lamp picks the map's texel (where that ray crosses the strips'
// plane); the light is cut there only if x lies beyond the first thing the
// ray meets. Softened over the aperture's penumbra, four taps turned by
// `blur`.
fn transmitted(x:vec3f,blur:f32)->vec3f {
    let l=lamp();
    let v=x-l;
    if v.z>=-1e-3 {return vec3f(1);}
    let at=l.xy+v.xy*(l.z/(-v.z));
    let uv=vec2f(at.x,-at.y)/(2.0*TRANSMISSION_EXTENT)+.5;
    if any(uv<vec2f(0)) || any(uv>vec2f(1)) {return vec3f(1);}
    let s=length(v);
    // The aperture's penumbra: an aperture of radius APERTURE, the
    // occluders near the plane, x past them.
    let spread=APERTURE*max(-x.z,0.0)/max(l.z-x.z,1.0)/(2.0*TRANSMISSION_EXTENT);
    let o=vec2f(cos(blur),sin(blur))*spread;
    let taps=array<vec2f,4>(o,-o,vec2f(-o.y,o.x),vec2f(o.y,-o.x));
    var t=vec3f(0);
    for(var i=0;i<4;i++) {
        let m=textureSampleLevel(transmission,linear,uv+taps[i],0.0);
        t+=select(vec3f(1),m.rgb,s>m.a);
    }
    return t*.25;
}

// ---- The air ----------------------------------------------------------------------------
// The wind's displacement at a moment (authored), and the air's density at
// x: thin haze, thicker and thinner in slow, large regions carried by it.
fn wind_offset()->vec3f {return WIND*greetings_seconds();}
fn density(x:vec3f)->f32 {
    let q=(x-wind_offset())/DENSITY_SCALE;
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
    let slope=tan(BEAM_HALF)+PENUMBRA_GROWTH*1.2;
    let reach=APERTURE+PENUMBRA*1.2;
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

// ---- The transmission map ------------------------------------------------------------------
// What the strips let through, seen from the lamp: for each point of the
// strips' plane, the lamp's ray through it across the cubes' slab, 0 where a
// cube stands in its way, softened at their edges, and in alpha how far from
// the lamp the ray first meets a cube (far where it meets none). White
// elsewhere.
@fragment fn strips_transmission(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let at=(vec2f(v.uv.x,1.0-v.uv.y)-.5)*2.0*TRANSMISSION_EXTENT;
    let l=lamp();
    let d=normalize(vec3f(at,0.0)-l);
    var lit=1.0;
    var first=1e4;
    if stage()>=STAGE_WORDS-.5 && stage()<STAGE_MONSTER-.5 && d.z<0.0 {
        let enter=(l.z-SLAB)/(-d.z);
        let leave=(l.z+SLAB)/(-d.z);
        for(var s=0u;s<STRIPS;s++) {
            let span=column_span(s,l,d);
            let a=max(span.x,enter);
            let b=min(span.y,leave);
            if a>=b {continue;}
            var t=a;
            for(var i=0u;i<24u;i++) {
                let h=stack_distance(s,l+d*t);
                let here=clamp(h/.35,0.0,1.0);
                if here<1.0 {first=min(first,t);}
                lit=min(lit,here);
                if lit<=0.0 {break;}
                t+=max(h,.15);
                if t>b {break;}
            }
        }
    }
    return vec4f(vec3f(lit),first);
}

// ---- The lit air ------------------------------------------------------------------------------
// The air's light along each ray, at half size: marched within the cone up
// to the cubes, each sample at its own instant of the shutter (the beam
// turns about 400 degrees a second). Its depth goes in alpha for the
// picture's upsampling.
@fragment fn air(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let pixel=vec2i(v.position.xy);
    let full=min(pixel*2,vec2i(u.screen.xy)-1);
    let depth=textureLoad(depth_image,full,0).r;
    let q=scene_of_uv(v.uv);
    let d=normalize(vec3f(q,CAMERA_DISTANCE));
    let near=(AIR_NEAR+CAMERA_DISTANCE)/d.z;
    let far=min((AIR_FAR+CAMERA_DISTANCE)/d.z,depth);
    let jitter=pattern(v.position.xy);
    // The span over the shutter: the union of the cone at its two ends.
    let a0=cone_span(d,beam_axis(0.0),near,far);
    let a1=cone_span(d,beam_axis(SHUTTER),near,far);
    var span=vec2f(min(a0.x,a1.x),max(a0.y,a1.y));
    if a0.x>=a0.y {span=a1;}
    if a1.x>=a1.y {span=a0;}
    var light=0.0;
    if span.x<span.y && intensity()>0.0 {
        let step=(span.y-span.x)/f32(MARCH_SAMPLES);
        for(var k=0u;k<MARCH_SAMPLES;k++) {
            let f=fract(jitter+f32(k)*.618034);
            let t=span.x+(f32(k)+f)*step;
            let x=CAMERA+d*t;
            let axis=beam_axis(SHUTTER*fract(jitter*7.31+f32(k)*.38197));
            let b=beam_at(x,axis);
            if b<=0.0 {continue;}
            let towards=normalize(x-lamp());
            let lit=b*phase(towards,-d,PHASE_G)*density(x)*transmitted(x,jitter*6.283+f32(k)).r;
            light+=lit*step;
        }
    }
    return vec4f(star_colour()*light*SCATTER*intensity(),depth);
}

// ---- The dust ------------------------------------------------------------------------
// A mote's place at the moment: carried by the wind, and swirled a little
// round it (authored; slow, and the same for neighbouring motes).
fn mote_place(m:vec4f)->vec3f {
    let x=m.xyz;
    let t=greetings_seconds();
    let w=2.0*PI/SWIRL_PERIOD;
    let swirl=vec3f(sin(w*t+x.y*.011+x.z*.007),sin(w*.83*t+x.x*.009+2.1),sin(w*.71*t+x.x*.006+x.y*.008+4.2));
    return x+wind_offset()+swirl*SWIRL;
}
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
    // Lit by the beam over the shutter, forward scattering; outside it, a
    // trace of the original's band.
    var lit=0.0;
    for(var k=0u;k<3u;k++) {
        lit+=beam_at(x,beam_axis(SHUTTER*f32(k)*.5));
    }
    lit*=phase(normalize(x-lamp()),to_camera/distance,MOTE_G)*transmitted(x,f32(n)).r/3.0;
    lit=lit*MOTE_GAIN+MOTE_BAND*star_light(index,gc.sweep.x);
    lit*=(.3+2.2*size*size)*intensity();
    // In focus at the strips' plane; the near ones soft discs, the far ones
    // points; a disc's light spread over its area.
    let px=u.screen.y/1080.0;
    let coc=min(BOKEH*abs(1.0-CAMERA_DISTANCE/distance),BOKEH_MAX)*px;
    let core=MOTE_RADIUS*(.7+1.6*size)*px;
    let radius=max(core,coc);
    lit*=(core*core)/(radius*radius);
    if lit<1e-4 {return out;}
    let q=project(x);
    let c=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[corner];
    let extent=radius*1.6+1.0;
    let uv=uv_of_scene(q);
    let ndc=vec2f(uv.x*2.0-1.0,1.0-uv.y*2.0)+c*extent*2.0/u.screen.xy;
    out.position=vec4f(ndc,0,1);
    out.offset=c*extent;
    out.colour=star_colour()*lit;
    out.radius=radius;
    out.distance=distance;
    return out;
}
@fragment fn dust_fragment(v:DustVertex)->@location(0) vec4f {
    // Hidden behind a cube.
    if textureLoad(depth_image,vec2i(v.position.xy),0).r<v.distance {discard;}
    let r=length(v.offset)/v.radius;
    // A soft disc, its edge a little brighter, as out of focus light is.
    let disc=(1.0-smoothstep(.75,1.05,r))*(.85+.25*smoothstep(.4,.95,r));
    return vec4f(v.colour*disc,1);
}

// ---- The cubes -------------------------------------------------------------------------
fn strip_x(s:u32)->f32 {return (f32(8u+64u*s)+14.0-160.0)*COLUMN;}
// Slot k's letter in strip s (frame shown), or 255.
fn letter_in(s:u32,k:u32)->u32 {
    let w=letters[frame_now()*6u+1u+s];
    return (w[k/4u]>>(8u*(k%4u)))&255u;
}
fn first_top()->f32 {return gc.twist_late.y;}
fn position_of(s:u32)->f32 {return select(gc.twist[s],gc.twist_late.x,s==4u);}
// The turn at source row y in strip s.
fn twist(s:u32,row:f32)->f32 {
    return TWIST_STEP*(40.0*sin(PI*(position_of(s)+row)/200.0)+60.0);
}
// A point in slot k's cube's frame: across, up from its centre, toward the
// back; the turn is the row's, so the letter twists as the original's does.
fn cube_local(s:u32,k:f32,p:vec3f)->vec3f {return cube_local_at(s,k,p,0.0);}
// The same with the cube's pose `tau` frames earlier, when it was tau rows
// lower and its sine position tau further on (both fall one a frame).
fn cube_local_at(s:u32,k:f32,p:vec3f,tau:f32)->vec3f {
    let x=p.x-strip_x(s);
    let row=100.0-p.y;
    let a=TWIST_STEP*(40.0*sin(PI*(position_of(s)+tau+row)/200.0)+60.0);
    let centre=100.0-(first_top()+tau+SLOT_ROWS*k+HALF);
    return vec3f(cos(a)*x+sin(a)*p.z,p.y-centre,-sin(a)*x+cos(a)*p.z);
}
fn box_distance(l:vec3f)->f32 {
    let q=abs(l)-vec3f(HALF-ROUNDING);
    return length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-ROUNDING;
}
struct Near { distance:f32, slot:f32 }
fn stack_near(s:u32,p:vec3f)->Near {
    let k0=floor((100.0-p.y-first_top())/SLOT_ROWS);
    var best=Near(1e5,-1.0);
    for(var dk=-1.0;dk<=1.0;dk+=1.0) {
        let k=k0+dk;
        if k<0.0 || k>=f32(SLOTS) || letter_in(s,u32(k))==255u {continue;}
        let d=box_distance(cube_local(s,k,p))*LIPSCHITZ;
        if d<best.distance {best=Near(d,k);}
    }
    return best;
}
fn stack_distance(s:u32,p:vec3f)->f32 {return stack_near(s,p).distance;}
fn stack_normal(s:u32,p:vec3f)->vec3f {
    let e=vec2f(.03,-.03);
    return normalize(e.xyy*stack_distance(s,p+e.xyy)+e.yyx*stack_distance(s,p+e.yyx)
        +e.yxy*stack_distance(s,p+e.yxy)+e.xxx*stack_distance(s,p+e.xxx));
}
// The ray's span inside strip s's column (radius: a cube's half diagonal),
// or (1, 0).
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
// The original's letter colours 1 to 6 (white to teal), along the relief.
fn letter_ramp(relief:f32)->vec3f {
    let ramp=array<vec3f,6>(vec3f(60,60,60),vec3f(44,52,52),vec3f(32,44,44),
        vec3f(20,36,36),vec3f(12,32,32),vec3f(4,24,24));
    let x=clamp(1.0-relief,0.0,1.0)*5.0;
    let i=min(u32(x),4u);
    return srgb_decode(mix(ramp[i],ramp[i+1u],x-f32(i))/63.0);
}
fn ggx(n:vec3f,h:vec3f,alpha:f32)->f32 {
    let nh=max(dot(n,h),0.0);
    let a2=alpha*alpha;
    let den=nh*nh*(a2-1.0)+1.0;
    return a2/(PI*den*den);
}
fn fresnel(cosine:f32)->f32 {return SHEEN+(1.0-SHEEN)*pow(1.0-clamp(cosine,0.0,1.0),5.0);}
// The letter panel on the face of slot k's cube that point p lies on, with
// the cube's pose tau frames earlier: coverage, relief and its slopes
// across and up, and the face's across in the cube's frame (x, z).
struct Panel { cover:f32, relief:f32, slope:vec2f, across:vec2f }
fn panel_at(s:u32,k:f32,p:vec3f,tau:f32,footprint:f32)->Panel {
    let l=cube_local_at(s,k,p,tau);
    var nl=vec3f(0,0,sign(l.z));
    if abs(l.x)>=abs(l.z) {nl=vec3f(sign(l.x),0,0);}
    let r=vec3f(-nl.z,0,nl.x);
    var out=Panel(0.0,0.0,vec2f(0),r.xz);
    if abs(l.y)>=max(abs(l.x),abs(l.z))+.3 {return out;}
    let t=vec2f(dot(l,r),-l.y)/(2.0*HALF*PANEL_SCALE)+.5;
    if any(t<vec2f(.5/CELL)) || any(t>vec2f(1.0-.5/CELL)) {return out;}
    let g=letter_in(s,u32(k));
    let cell=vec2f(f32(g%ATLAS_COLUMNS),f32(g/ATLAS_COLUMNS))*CELL;
    let face=textureSampleLevel(glyphs,linear,(cell+t*CELL)/ATLAS_SIZE,0.0);
    // The atlas's distances are in rows of the face; the panel is smaller.
    let d=face.r*PANEL_SCALE;
    let aa=max(footprint*.7,.02);
    out.cover=1.0-smoothstep(-aa,aa,d);
    out.relief=face.g;
    out.slope=face.ba/PANEL_SCALE;
    return out;
}
struct Shade { colour:vec3f, mirror:vec3f }
// A point on strip s's cubes, seen along d, `footprint` rows a pixel
// there: its colour, and where its polish mirrors the picture (uv) with the
// mirror's weight.
fn shade_cube(s:u32,p:vec3f,d:vec3f,footprint:f32)->Shade {
    let near=stack_near(s,p);
    let k=max(near.slot,0.0);
    var n=stack_normal(s,p);
    // The panel over the shutter: its coverage and relief averaged as the
    // cube turns and rises; its bevel from the moment itself.
    let now=panel_at(s,k,p,0.0,footprint);
    var panel=0.0;
    var relief=0.0;
    let span=CUBE_SHUTTER*gc.frame.w;
    for(var i=0u;i<CUBE_TAPS;i++) {
        let tap=panel_at(s,k,p,span*f32(i)/f32(CUBE_TAPS-1u),footprint);
        panel+=tap.cover;
        relief+=tap.relief*tap.cover;
    }
    relief/=max(panel,1e-4);
    panel/=f32(CUBE_TAPS);
    // The panel's bevel tilts the normal; across is the face's turned to
    // the world.
    let a=twist(s,100.0-p.y);
    let rw=vec3f(cos(a)*now.across.x-sin(a)*now.across.y,0,sin(a)*now.across.x+cos(a)*now.across.y);
    n=normalize(n-BUMP*now.cover*(now.slope.x*rw+now.slope.y*vec3f(0,1,0)));
    let v=-d;
    let L=normalize(lamp()-p);
    let h=normalize(L+v);
    let axis=beam_axis(0.0);
    let at=min(beam_at(p,axis),2.0);
    // The lamp lights the cube all round, and the beam more.
    let light=BULB_LIGHT+BEAM_LIGHT*at;
    let nl=max(dot(n,L),0.0);
    let rough=mix(BLACK_ROUGH,PANEL_ROUGH,panel);
    let spec=(ggx(n,h,rough)*GLINT+ggx(n,h,SHEEN_ROUGH)*SHEEN_GLINT)*fresnel(dot(h,v))*nl*light;
    let diffuse=BLACK*nl*light*(1.0-panel);
    // The beam on the cube's far side, shining through its panels.
    let centre=vec3f(strip_x(s),100.0-(first_top()+SLOT_ROWS*k+HALF),0.0);
    let behind=min(beam_at(centre,axis),1.5);
    let rim=pow(1.0-max(dot(n,v),0.0),3.0)*RIM*at*(1.0-panel);
    let ramp=letter_ramp(relief);
    let glow=(ramp*PANEL_REST+mix(ramp,star_colour(),.5)*PANEL_BEAM*behind)*panel;
    // Where the polish mirrors the picture (see MIRROR).
    let rd=reflect(d,n);
    let toward=vec3f(rd.xy,abs(rd.z)+.05);
    let q=p+toward*((MIRROR_DEPTH-p.z)/toward.z);
    let uvq=uv_of_scene(project(q));
    let weight=fresnel(dot(n,v))*MIRROR*select(FRONT_MIRROR,1.0,rd.z>.05)*(1.0-.5*panel);
    let colour=((spec+rim+diffuse)*star_colour()+glow)*intensity();
    return Shade(colour,vec3f(uvq,weight));
}

struct CubesOut {
    @location(0) colour:vec4f,
    @location(1) depth:vec4f,
    @location(2) mirror:vec4f,
}
// The cubes, their depth along each ray, and where their polish mirrors the
// picture.
@fragment fn cubes(v:Vertex)->CubesOut {
    greetings_begin();
    let q=scene_of_uv(v.uv);
    let d=normalize(vec3f(q,CAMERA_DISTANCE));
    let row_per_pixel=200.0/u.screen.y;
    var colour=vec3f(0);
    var mirror=vec3f(0);
    var cover=0.0;
    var depth=1e9;
    if stage()>=STAGE_WORDS-.5 && stage()<STAGE_MONSTER-.5 {
        for(var s=0u;s<STRIPS;s++) {
            let span=column_span(s,CAMERA,d);
            if span.x>=span.y {continue;}
            var t=max(span.x,0.0);
            var closest=1e5;
            var closest_t=t;
            var hit=false;
            for(var i=0u;i<MARCH_STEPS;i++) {
                let h=stack_distance(s,CAMERA+d*t);
                let footprint=row_per_pixel*t/CAMERA_DISTANCE;
                if h/footprint<closest {closest=h/footprint;closest_t=t;}
                if h<.002*t*row_per_pixel {hit=true;break;}
                t+=max(h,.01);
                if t>span.y {break;}
            }
            // A soft silhouette: the nearest approach within a pixel.
            let c=select(clamp(1.0-closest,0.0,1.0),1.0,hit);
            if c>0.0 {
                let at=select(closest_t,t,hit);
                let shade=shade_cube(s,CAMERA+d*at,d,row_per_pixel*at/CAMERA_DISTANCE);
                colour=shade.colour*c;
                mirror=vec3f(shade.mirror.xy,shade.mirror.z*c);
                cover=c;
                // The air and dust stop at it where it is solid.
                if c>.5 {depth=at;}
                break;
            }
        }
    }
    return CubesOut(vec4f(colour,cover),vec4f(depth,0,0,1),vec4f(mirror,1));
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
// The lit air at a full-size pixel from the half-size image: the four
// nearest samples, each weighted by how near its depth is to the pixel's,
// so the air stops cleanly at the cubes' edges.
fn air_at(pixel:vec2i,depth:f32)->vec3f {
    let size=vec2i(textureDimensions(air_image));
    let h=(vec2f(pixel)+.5)*.5-.5;
    let base=vec2i(floor(h));
    let f=h-floor(h);
    var sum=vec3f(0);
    var weights=0.0;
    for(var y=0;y<2;y++) {
        for(var x=0;x<2;x++) {
            let at=clamp(base+vec2i(x,y),vec2i(0),size-1);
            let s=textureLoad(air_image,at,0);
            let w=select(1.0-f.x,f.x,x==1)*select(1.0-f.y,f.y,y==1)
                *exp(-abs(min(s.a,1e4)-min(depth,1e4))/8.0)+1e-5;
            sum+=s.rgb*w;
            weights+=w;
        }
    }
    return sum/weights;
}
// The lamp seen directly at r rows from it: the core and halo, and its
// aperture's flare as the beam turns toward the camera.
fn lamp_glow(r:f32)->f32 {
    let facing=max(-beam_axis(0.0).z,0.0);
    let flare=FLARE*pow(facing,8.0);
    return exp(-(r/CORE)*(r/CORE))*CORE_GAIN*(1.0+flare)+exp(-r/HALO)*HALO_GAIN*(1.0+flare*.5);
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
    let cubes=textureLoad(cubes_image,pixel,0);
    let depth=textureLoad(depth_image,pixel,0).r;
    let dust=textureLoad(dust_image,pixel,0).rgb;
    let air=air_at(pixel,depth);
    let lq=project(lamp());
    let lamp_light=star_colour()*intensity();
    // The lamp where no cube stands in front of it; the dust and the air,
    // both already stopped at the cubes.
    var c=cubes.rgb+dust+air+lamp_glow(length(q-lq))*lamp_light*(1.0-cubes.a);
    // The polish mirrors the dust, the lit air and the lamp.
    let mirror=textureLoad(mirror_image,pixel,0);
    if mirror.z>0.0 {
        let mq=scene_of_uv(mirror.xy);
        let at=clamp(vec2i(mirror.xy*u.screen.xy),vec2i(0),vec2i(u.screen.xy)-1);
        let half=clamp(vec2i(mirror.xy*u.screen.xy*.5),vec2i(0),vec2i(textureDimensions(air_image))-1);
        let mirrored=textureLoad(dust_image,at,0).rgb+textureLoad(air_image,half,0).rgb
            +lamp_glow(length(mq-lq))*lamp_light;
        c+=mirrored*mirror.z;
    }
    // The fade to white, at the original's levels.
    let w=clamp(gc.sweep.z,0.0,1.0);
    if w>0.0 {
        c=tonemap_inverse(mix(tonemap(c),vec3f(1),w));
    }
    return vec4f(max(c,vec3f(0)),1);
}
