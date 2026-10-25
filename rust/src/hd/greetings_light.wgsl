// The modern part 7: a lighthouse in the dust. Authored throughout, on the
// port's timing, positions and motion (greetings_common.wgsl), its letters
// derived from the user's data (greetings_type.rs).
//
// The original turns 64 colours over a field of stars that never move, so
// a broad band of light sweeps round (160, 85) once every 64 frames; five
// strips of letters rise through it, each letter row one of 16 stored views
// of a turning column. Here:
// - The stars are dust: each stored star a few motes in depth behind the
//   strips, lit as the original lights it (its colour index through the
//   cycle's pulse), so the original's broad band still sweeps the dust.
// - The cycle is also one hard beam from a lamp at the sweep's centre,
//   behind the strips: the top of the pulse, turning once every 64 frames,
//   in the stars' cream. The lamp is a bulb glowing all round with the beam
//   leaving it; the dust in the beam glows, and the beam's light streams
//   past the cubes in shafts (screen-space scattering of the lamp's glow
//   and the lit dust, the cubes standing in its way).
// - Each letter is a cube of polished black with rounded edges, the letter
//   inset on each of its four faces as a panel of frosted light with a
//   border of the black round it, coloured as the original shades its
//   letters (white crowns, teal edges) and always readable. The lamp lights
//   the cubes all round and the beam more: their edges and the faces
//   turned toward it catch it in highlights and glints as they turn, the
//   beam behind a cube brightens its panels, and the polish mirrors the
//   dust, the shafts and the lamp. The cubes turn as the original's sine
//   turns each row of a letter (5.625 degrees a step of its view index,
//   continuous), and the gaps between them stay open (the original's
//   padding rows; its smear, from a quirk, is faithful HD's).
// - The shutter: the beam turns about 400 degrees a second, so its light
//   and the dust's are taken over SHUTTER of a 70 Hz frame up to the moment;
//   the panels over CUBE_SHUTTER as the cubes turn and rise.
// - The end: the beam swings into the lens (authored motion, from a little
//   before the original's feedback fade), which then blends every colour
//   toward white at the original's timing and levels; the monster
//   (MONSTER.U, upscaled) develops out of the white at the original's
//   levels.
// Module: the shared controls, greetings_common.wgsl and this file.

// ---- The look (authored) -------------------------------------------------------
// The camera's distance from the strips' plane, the dust's mean depth and
// the lamp's (greetings::CAMERA_DISTANCE, DUST_DEPTH; source rows).
const CAMERA_DISTANCE=580.0;
const DUST_DEPTH=220.0;
const LAMP_DEPTH=190.0;
// The shutter (70 Hz frames) and its taps.
const SHUTTER=1.0;
const SHUTTER_TAPS=6u;
// The beam: the top of the original's pulse (which lights 225 degrees),
// between these levels: full within about 8 degrees of its peak, gone by
// about 23.
const BEAM_EDGE=vec2f(.82,.95);
// The dust: a mote's radius (pixels at 1080 rows), its gain under the
// original's band and in the beam, and its colour.
const MOTE_RADIUS=.85;
const DUST_GAIN=.32;
const BEAM_DUST=1.0;
// The lamp: a bulb glowing all round, its core and halo (rows) and their
// gains, and the beam's own glow where it leaves the bulb, its reach (rows)
// and gain; how much brighter the lamp grows as the beam turns to the lens.
// Of the bulb's glow, the share the shafts scatter. Then the shafts'
// samples, decay, length and gain, and the lit dust's share in them.
const CORE=2.6;
const HALO=22.0;
const CORE_GAIN=9.0;
const HALO_GAIN=.12;
const BEAM_REACH=45.0;
const BEAM_GLOW=2.2;
const LAMP_SWELL=3.0;
const BULB_SCATTER=.1;
const SHAFT_SAMPLES=80u;
const SHAFT_DECAY=.986;
const SHAFT_LENGTH=.85;
const SHAFT_GAIN=1.0;
// How much of the shafts' light falls in front of the cubes.
const FRONT_HAZE=.15;
const DUST_SCATTER=1.4;
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
// shafts and the lamp), and toward the camera the same picture mirrored
// about the strips' plane, as if it surrounded them (authored), dimmer;
// the mirror's strength, and the rim where the beam passes behind.
const MIRROR=2.5;
const FRONT_MIRROR=.5;
const RIM=.6;
// The cubes' motion over their shutter (70 Hz frames; shorter than the
// beam's, so the letters stay sharp where they turn fastest): taps along it.
const CUBE_SHUTTER=.5;
const CUBE_TAPS=3u;
// The swing into the lens: it starts this many seconds before the original's
// fade to white (authored motion; the fade keeps its timing and levels),
// the flood of light it brings, and the brightest the monster shows through
// the tonemap once developed (so it keeps its colours, near enough).
const SWING_LEAD=.4;
const FLOOD=6.0;
const MONSTER_WHITE=.9;

// ---- Bindings -----------------------------------------------------------------------
// The letters' faces (greetings_type.rs): signed distance (rows), relief,
// its slopes across and up.
@group(0) @binding(8) var glyphs:texture_2d<f32>;
// The dust motes (greetings::motes): across, up, depth; index * 4 plus
// brightness.
@group(0) @binding(9) var<storage,read> motes:array<vec4f>;
// The passes' images: the dust; the shafts' source; the cubes (colour and
// coverage); the shafts (half size); where the cubes' polish mirrors the
// picture (its uv, and the mirror's weight).
@group(1) @binding(1) var dust_image:texture_2d<f32>;
@group(1) @binding(2) var scatter_image:texture_2d<f32>;
@group(1) @binding(3) var cubes_image:texture_2d<f32>;
@group(1) @binding(4) var shafts_image:texture_2d<f32>;
@group(1) @binding(5) var mirror_image:texture_2d<f32>;

// ---- The beam -----------------------------------------------------------------------
// The stars' colour, (v, 8v/9, 7v/9) as the scanout shows it, in linear
// light at unit red.
fn star_colour()->vec3f {return srgb_decode(vec3f(1.0,8.0/9.0,7.0/9.0));}
// The star colours' intensity scales their 6-bit DAC values, which the
// display shows at about their 2.2 power: so does all the light here.
fn intensity()->f32 {return pow(clamp(gc.sweep.y,0.0,1.0),2.2);}
// The lamp in camera space, placed so it projects on the sweep's centre,
// where the beam's directions turn round.
fn lamp()->vec3f {
    return vec3f(0.0,(100.0-SWEEP_CENTRE.y)*(CAMERA_DISTANCE+LAMP_DEPTH)/CAMERA_DISTANCE,LAMP_DEPTH);
}
fn project(p:vec3f)->vec2f {return p.xy*CAMERA_DISTANCE/(CAMERA_DISTANCE+p.z);}
// The lamp's glow at r rows from it on the screen: the bulb all round, and
// the beam's glow where it leaves the bulb, `toward` the beam's strength in
// that direction.
fn bulb_glow(r:f32)->f32 {
    return (exp(-(r/CORE)*(r/CORE))*CORE_GAIN+exp(-r/HALO)*HALO_GAIN)*(1.0+LAMP_SWELL*swing());
}
fn beam_glow(r:f32,toward:f32)->f32 {return exp(-r/BEAM_REACH)*BEAM_GLOW*toward;}
// The direction round the sweep's centre (source pixels, y down) of a
// scene point.
fn sweep_angle(q:vec2f)->f32 {
    let s=source_of_scene(q)-SWEEP_CENTRE;
    return atan2(s.y,s.x);
}
// Over the shutter: the original's band at a colour index, and the beam in
// a direction.
fn band(index:f32)->f32 {
    var l=0.0;
    for(var k=0u;k<SHUTTER_TAPS;k++) {
        l+=star_light(index,gc.sweep.x-SHUTTER*f32(k)/f32(SHUTTER_TAPS-1u));
    }
    return l/f32(SHUTTER_TAPS);
}
// As the beam swings toward the lens over the fade to white, its cone,
// seen ever more end on, opens round the lamp until it lights every
// direction.
fn beam(angle:f32)->f32 {
    let index=index_of_angle(angle);
    let edge=mix(BEAM_EDGE,vec2f(-.3,0.0),swing());
    var l=0.0;
    for(var k=0u;k<SHUTTER_TAPS;k++) {
        l+=smoothstep(edge.x,edge.y,star_light(index,gc.sweep.x-SHUTTER*f32(k)/f32(SHUTTER_TAPS-1u)));
    }
    return l/f32(SHUTTER_TAPS);
}
// How far the beam has swung into the lens (authored motion): it begins
// SWING_LEAD seconds before the original's fade to white and is all but
// done by the fade's third step, so the turn shows before the white covers
// it; the fade itself keeps the original's timing and levels.
fn swing()->f32 {
    let white=gc.events.w;
    var lead=0.0;
    if white>=0.0 {lead=smoothstep(white-SWING_LEAD,white+.05,greetings_seconds());}
    let w=clamp(gc.sweep.z*2.5,0.0,1.0);
    return max(lead,w*w*(3.0-2.0*w));
}

// ---- The dust ------------------------------------------------------------------------
struct DustVertex {
    @builtin(position) position:vec4f,
    @location(0) offset:vec2f,
    @location(1) @interpolate(flat) colour:vec3f,
    @location(2) @interpolate(flat) radius:f32,
}
@vertex fn dust_vertex(@builtin(vertex_index) corner:u32,@builtin(instance_index) n:u32)->DustVertex {
    greetings_begin();
    let m=motes[n];
    let index=floor(m.w/4.0);
    let gain=m.w-index*4.0;
    let q=project(m.xyz);
    let lit=intensity()*gain*(DUST_GAIN*band(index)+BEAM_DUST*beam(sweep_angle(q)));
    let radius=MOTE_RADIUS*u.screen.y/1080.0*sqrt((CAMERA_DISTANCE+DUST_DEPTH)/(CAMERA_DISTANCE+m.z));
    var out:DustVertex;
    if lit<1e-4 || stage()>STAGE_WHITE+.5 {
        out.position=vec4f(2,2,2,1);
        return out;
    }
    let c=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[corner];
    let extent=radius*2.0+1.0;
    let uv=uv_of_scene(q);
    let ndc=vec2f(uv.x*2.0-1.0,1.0-uv.y*2.0)+c*extent*2.0/u.screen.xy;
    out.position=vec4f(ndc,0,1);
    out.offset=c*extent;
    out.colour=star_colour()*lit;
    out.radius=radius;
    return out;
}
@fragment fn dust_fragment(v:DustVertex)->@location(0) vec4f {
    let d=length(v.offset)/v.radius;
    return vec4f(v.colour*exp(-d*d),1);
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
    if disc<=0.0 {return vec2f(1,0);}
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
// A point on strip s's cubes, seen along d from o, `footprint` rows a
// pixel there: its colour, and where its polish mirrors the picture (uv)
// with the mirror's weight.
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
    let at=beam(sweep_angle(p.xy));
    // The lamp lights the cube all round, and the beam more.
    let light=BULB_LIGHT*(1.0+LAMP_SWELL*swing())+BEAM_LIGHT*at;
    let nl=max(dot(n,L),0.0);
    let rough=mix(BLACK_ROUGH,PANEL_ROUGH,panel);
    let spec=(ggx(n,h,rough)*GLINT+ggx(n,h,SHEEN_ROUGH)*SHEEN_GLINT)*fresnel(dot(h,v))*nl*light;
    let diffuse=BLACK*nl*light*(1.0-panel);
    // Behind the cube, the beam shining through its panels.
    let centre=vec2f(strip_x(s),100.0-(first_top()+SLOT_ROWS*k+HALF));
    let behind=beam(sweep_angle(centre));
    let rim=pow(1.0-max(dot(n,v),0.0),3.0)*RIM*at*(1.0-panel);
    let ramp=letter_ramp(relief);
    let glow=(ramp*PANEL_REST+mix(ramp,star_colour(),.5)*PANEL_BEAM*behind)*panel;
    // Where the polish mirrors the picture (see MIRROR).
    let rd=reflect(d,n);
    let toward=vec3f(rd.xy,abs(rd.z)+.05);
    let q=p+toward*((DUST_DEPTH-p.z)/toward.z);
    let uvq=uv_of_scene(project(q));
    let weight=fresnel(dot(n,v))*MIRROR*select(FRONT_MIRROR,1.0,rd.z>.05)*(1.0-.5*panel);
    var mirrored=vec3f(0);
    if all(uvq>vec2f(0)) && all(uvq<vec2f(1)) {
        mirrored=textureLoad(dust_image,vec2i(uvq*u.screen.xy),0).rgb;
    }
    let colour=((spec+rim+diffuse)*star_colour()+glow)*intensity()+mirrored*weight;
    return Shade(colour,vec3f(uvq,weight));
}

struct CubesOut {
    @location(0) colour:vec4f,
    @location(1) scatter:vec4f,
    @location(2) mirror:vec4f,
}
// The cubes, where their polish mirrors the picture, and the shafts'
// source: the lamp's glow and the lit dust where no cube stands in front
// of them.
@fragment fn cubes(v:Vertex)->CubesOut {
    greetings_begin();
    let pixel=vec2i(v.position.xy);
    let q=scene_of_uv(v.uv);
    let o=vec3f(0,0,-CAMERA_DISTANCE);
    let d=normalize(vec3f(q,CAMERA_DISTANCE));
    let row_per_pixel=200.0/u.screen.y;
    var colour=vec3f(0);
    var mirror=vec3f(0);
    var cover=0.0;
    if stage()>=STAGE_WORDS-.5 && stage()<STAGE_MONSTER-.5 {
        for(var s=0u;s<STRIPS;s++) {
            let span=column_span(s,o,d);
            if span.x>=span.y {continue;}
            var t=max(span.x,0.0);
            var closest=1e5;
            var closest_t=t;
            var hit=false;
            for(var i=0u;i<MARCH_STEPS;i++) {
                let h=stack_distance(s,o+d*t);
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
                let shade=shade_cube(s,o+d*at,d,row_per_pixel*at/CAMERA_DISTANCE);
                colour=shade.colour*c;
                mirror=vec3f(shade.mirror.xy,shade.mirror.z*c);
                cover=c;
                break;
            }
        }
    }
    let r=length(q-project(lamp()));
    let toward=beam(sweep_angle(q));
    let glow=(bulb_glow(r)*BULB_SCATTER+beam_glow(r,toward))*star_colour()*intensity();
    let dust=textureLoad(dust_image,pixel,0).rgb;
    let source=(glow+dust*toward*DUST_SCATTER)*(1.0-cover);
    return CubesOut(vec4f(colour,cover),vec4f(source,1),vec4f(mirror,1));
}

// ---- The shafts --------------------------------------------------------------------------
fn noise(p:vec2f)->f32 {return fract(52.9829189*fract(dot(p,vec2f(.06711056,.00583715))));}
@fragment fn shafts(v:Vertex)->@location(0) vec4f {
    greetings_begin();
    let centre=uv_of_scene(project(lamp()));
    let jitter=noise(v.position.xy);
    var c=vec3f(0);
    var w=1.0;
    var total=0.0;
    for(var i=0u;i<SHAFT_SAMPLES;i++) {
        let s=1.0-SHAFT_LENGTH*(f32(i)+jitter)/f32(SHAFT_SAMPLES);
        let uv=centre+(v.uv-centre)*s;
        c+=w*textureSampleLevel(scatter_image,linear,uv,0.0).rgb;
        total+=w;
        w*=SHAFT_DECAY;
    }
    return vec4f(c/total,1);
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
    let dust=textureLoad(dust_image,pixel,0).rgb;
    let shafts=textureSampleLevel(shafts_image,linear,v.uv,0.0).rgb;
    // The lamp, seen directly where no cube stands in front of it.
    let lq=project(lamp());
    let lamp_light=star_colour()*intensity();
    let glow=(bulb_glow(length(q-lq))+beam_glow(length(q-lq),beam(sweep_angle(q))))*lamp_light;
    // The cubes stand in front of the dust, the lamp and most of the beam's
    // light; their polish mirrors the shafts and the lamp too.
    var c=(dust+glow+shafts*SHAFT_GAIN)*(1.0-cubes.a)+cubes.rgb+shafts*SHAFT_GAIN*FRONT_HAZE*cubes.a;
    let mirror=textureLoad(mirror_image,pixel,0);
    if mirror.z>0.0 {
        let mq=scene_of_uv(mirror.xy);
        let mirrored=textureSampleLevel(shafts_image,linear,mirror.xy,0.0).rgb*SHAFT_GAIN
            +bulb_glow(length(mq-lq))*lamp_light;
        c+=mirrored*mirror.z;
    }
    // The beam into the lens: a flood of its light, then every colour
    // blended toward white as the feedback fade has it.
    let w=clamp(gc.sweep.z,0.0,1.0);
    if w>0.0 {
        let r=length(q-project(lamp()));
        c+=star_colour()*FLOOD*swing()*exp(-r/(40.0+500.0*w));
        c=tonemap_inverse(mix(tonemap(c),vec3f(1),w));
    }
    return vec4f(max(c,vec3f(0)),1);
}
