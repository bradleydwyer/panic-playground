// The modern part 4: light on stone. Authored throughout, on the port's
// timing, positions and motion (plasma_common.wgsl), and on the slab that
// plasma_stone.rs fits to the logo.
//
// The original shows its two halves apart, split by the line compare: the
// plasma above, the logo slab below. Here they share one dark space, and
// the plasma is the light the stone is seen by:
// - The slab is a block of stone of part 3's box's width, standing where
//   the logo shows it, seen through the camera the logo's perspective
//   implies, its faces carved as the logo shades them (FUTURE in the top,
//   CREW in the front, the cracks) with rounded edges, polished where it is
//   flat and rough in the cut. It is ray-marched as a distance field: the
//   block, less the carving.
// - The plasma is two sheets of light standing on the floor behind the
//   slab, both from one line on it, leaning over the slab, the back one a
//   little less than the front, so it stands further back as they rise:
//   the original's two interlaced
//   copies of its field (the even rasters and the odd, which the strides
//   move apart), one a sheet each, sliding past each other as the strides
//   move them. On each, at each point, the colour the original's copy shows
//   there, but continuous: each copy's displacement is a smooth function of
//   height (its rasters' offsets smoothed down the screen and over a few
//   frames, then taken between samples by Catmull-Rom), the field a seamless
//   helix, and the palette's sine, its rotation and its level continuous
//   and smoothed over time, so the sheets glide at any display rate. Each
//   sheet's radiance is its colour taken back through the shared tonemap,
//   at half power, so together they give the light the original's two
//   copies do and its brightest bands read as light. The front sheet is
//   sharp; the back one, through it, is a little out of focus; both glow
//   softly over themselves and past their bottom edge.
// - The sheets light the stone: from behind and above, raking across the
//   top face, so the carving catches it at its rims, and the polished faces
//   mirror them, in the plasma's moving colours. Their light is gathered
//   over each sheet in cells (each a rectangle whose exact form factor is taken,
//   its radiance the mean of its part of the sheet), shadowed by the
//   carving toward where it comes from; the reflection reads each sheet's
//   light at the level its spread matches. A dim warm key light from the
//   upper left, where the logo's painted light comes from, keeps the stone
//   visible while the plasma is dark, lighting it no more than the plasma
//   will, and gives way as it rises.
// - The floor (the original has none: below its plasma it is black): part
//   3's water, level, lying as part 3 left it, draining from 186 to 200 s,
//   uncovering the slab, and away from under it as the plasma fades out
//   (plasma.rs, DRAIN), through the same camera,
//   running back to the sheets, which stand in it below the plasma's last
//   raster, the plasma carried on down them to it.
//   By default it is water: a mirror of the sheets and their glow
//   (Fresnel's equations for water, as it is: seen near grazing, it
//   mirrors 12 to 53% of their light),
//   its normals from a calm swell of four long waves, so the mirror image
//   wavers and draws out into streaks; its body scatters back a little of
//   the sheets' light, less in the slab's shade. On the preview's D key it
//   is polished dark stone instead, faintly mottled, with a little
//   roughness: it mirrors the sheets and their glow, softer the further the
//   reflected ray goes and fading with it, more at grazing angles
//   (Fresnel), and the sheets' light falls on it, less what the slab keeps
//   from it. Either would mirror the slab too, but from this camera the
//   ground in front of the slab, where that would show, is below the
//   frame. It shows only by the sheets' light, so it comes up and fades
//   with the plasma and is dark while the slab arrives and slides.
// - A mist hanging at the sheets scatters a little of their light, so it
//   spills softly below their edge.
// - The arrival: part 3's flare overexposes the camera, and the exposure
//   settles over the 64 frames of the original's fade from white, so the
//   carving (the darkest) comes out of the white first; part 3's view hands
//   over to this one in its first moments.
// - The music: each order's first row swells the sheets' light a little.
// - A faint dither keeps the 8-bit output from banding.
// - The slide: the slab sinks with the original's line compare; the floor
//   and the sheets, dark by then, go with it, as if the camera rose. Where
//   part 7 follows, the slab instead breaks up where it rests into part
//   7's dust (slab_breakup.wgsl): this view draws it into targets of part
//   7's, how much of it still stands in alpha and its surface (its normal
//   and albedo) in a second, and part 7's view lights it by the lighthouse
//   and flies the grains that have come away.
// Module: the shared controls, plasma_common.wgsl, slab_breakup.wgsl, this
// file and water.wgsl.

// ---- The look (authored) -------------------------------------------------------
// The sheet's power (its radiance is the original's colour, linear, times
// this), and the swell of light on each order's first row (and its decay,
// seconds).
const SHEET_POWER=.75;
// The brightest the sheet shows through the tonemap (its luminance then is
// about 3).
const SHEET_WHITE=.95;
const HIT_SWELL=.18;
// The shutter, as a fraction of a 70 Hz frame: about 1/120 s.
const SHUTTER=.6;
// The back sheet's softness: the radius of its circle of confusion, in
// source rows.
const BACK_DEFOCUS=4.0;
// The sheets' own glow: how much of their light, spread over about a
// fiftieth of their width, lies over them and spills past their bottom
// edge, and how far it spills (in the sheets' height).
const GLOW=.22;
const GLOW_LEVEL=3.5;
const GLOW_SPILL=.06;
// Dither against banding in the 8-bit output: the light's relative jitter.
const DITHER=.008;
// plasma::LAYER_SAMPLES.
const LAYER_SAMPLES=200u;
const HIT_DECAY=.45;
// The key light: toward it (camera space), its colour and strength; a cool
// fill from the camera's side, and the ambient light. They light the slab
// alone (a spot on it): the floor shows only by the sheets' light.
const KEY_DIR=vec3f(-.5,-.55,-.67);
const KEY=vec3f(1.0,.93,.84)*.75;
// How far the key dims as the plasma comes up (at its full level).
const KEY_DUCK=.8;
// The key's share of KEY while the plasma is dark (authored): the light the
// full plasma itself gives the slab's top face (measured on the frames, its
// mean over the part's full level: 0.38 of what KEY gives it), so the
// stone is no brighter in the dark than by the plasma's light. At KEY it
// was 1.5 times as bright as under the full plasma, and the top face's far
// end, the face the key meets most squarely and all that shows above the
// water then, was the brightest thing in the frame.
const KEY_DARK=.38;
const FILL=vec3f(.55,.68,.9)*.2;
const AMBIENT=.035;
// The stone: its reflectance at normal incidence, polished on its faces
// and rough in the carving and the cracks (GGX alpha), as a stonemason
// finishes a slab with its letters cut, and the soft shadows' sharpness.
const SHEEN=.05;
const POLISHED=.035;
const ROUGH=.55;
// The stone's grain (authored): its scale (units), how much it varies the
// albedo and how much it bumps the surface.
const GRAIN=2.2;
const GRAIN_ALBEDO=.22;
const GRAIN_BUMP=.12;
const SHADOW_SHARPNESS=10.0;
// The haze: its scattering per unit and how far behind the sheet's plane a
// ray is followed.
const HAZE=.0012;
const HAZE_DEPTH=45.0;
const HAZE_STEPS=6u;
// The arrival: the exposure over 1 at which the stone's colour has given
// way to white.
const ARRIVAL_WASH=8.0;
// The ray march: steps, and the distance field's safety factor (the
// carving's slopes are steeper than one).
const MARCH_STEPS=96u;
const LIPSCHITZ=.6;
// The floor (authored): polished dark stone, the slab's base. Its albedo
// (linear), its reflectance at normal incidence, its polish (GGX alpha)
// and how much the polish varies over it, the size of its mottling
// (units), and over what length of the reflected ray its mirror image
// fades (units). It shows only by the sheets' light.
const FLOOR_ALBEDO=vec3f(.060,.058,.056);
const FLOOR_F0=.045;
const FLOOR_ROUGH=.10;
const FLOOR_ROUGH_VARY=.6;
const FLOOR_MOTTLE=.35;
// How much the mottling dims and brightens its mirror image.
const FLOOR_SHEEN_MOTTLE=.25;
const FLOOR_GRAIN=36.0;
const FLOOR_FADE=900.0;
// The water (authored; the default ground, the polished floor on the
// preview's D key): the same plane, a near-mirror, as water.wgsl has it
// (shared with part 3, whose objects fly over the same water). The light
// its body scatters back (of what enters it). How much more it mirrors
// than water does (pc.water.x, plasma::WaterTuning::part4_mirror_gain: by
// default 1, water as it is), the swell's height, clock and size
// (pc.water.yzw) are tuned live on the preview's number keys. (A gain of 3
// was authored for the old ground, the slab's base, which this camera met
// at 45 to 60 degrees, where water reflects only 3 to 6%; the level water
// it meets at 69 to 84 degrees, where water's own 12 to 53% is already
// more than that gain gave, 8 to 18%, and the gain raised it to 32 to
// 90%, near a perfect mirror toward the sheets' foot.)
const WATER_BODY=vec3f(.010,.020,.024);
// The slab's shadow on the floor: the strips each of its sides is taken
// in.
const SHADOW_STRIPS=3u;
// Each sheet's cells for the light (its 512 x 256 half of the texture's
// level 6 is 8 x 4 texels, one a cell) and for the haze (level 7, 4 x 2).
const SHEET_SIZE=vec2f(512.0,256.0);
const CELLS=vec2u(8u,4u);
const CELL_LEVEL=6.0;
const HAZE_CELLS=vec2u(4u,2u);
const HAZE_LEVEL=7.0;

// ---- Bindings -----------------------------------------------------------------------
// The slab's relief (plasma_stone.rs): top face, across and from its near
// edge to its far; front face, across and from its top edge down. Red: the
// carving's depth; green: the albedo's multiplier; blue: carved; alpha:
// crack.
@group(0) @binding(9) var relief_top:texture_2d<f32>;
@group(0) @binding(10) var relief_front:texture_2d<f32>;
// The water the top face's carving keeps once the water around the slab has
// drained below it (plasma_stone.rs, Stone::fill_pools): its thickness over
// the stone along the face's normal, across and from the near edge.
@group(0) @binding(12) var pools:texture_2d<f32>;
// Each layer's displacement, smoothed down the screen (plasma.rs,
// PlasmaPreview::layers): frame j's layer L, sample k, at (2 j + L) *
// LAYER_SAMPLES + k.
@group(0) @binding(11) var<storage,read> layers:array<f32>;
// The sheets' light, 512 x 512 with its mips (written by plasma_sheet and
// plasma_downsample): the front sheet in the upper half and the back sheet
// in the lower, each across and from its top edge down; and part 3's
// modern view for the handover (HDR).
@group(1) @binding(1) var sheet_light:texture_2d<f32>;
@group(1) @binding(2) var part3_view:texture_2d<f32>;
// A mip level's source, for plasma_downsample.
@group(1) @binding(3) var sheet_level:texture_2d<f32>;

// ---- The camera and the slab ---------------------------------------------------------
fn focal()->f32 {return pc.camera.x;}
// The camera-space ray through source point s.
fn ray_of_source(s:vec2f)->vec3f {
    return normalize(vec3f((s.x-pc.camera.y)/(1.2*focal()),(s.y-100.0)/focal(),1.0));
}
fn project(p:vec3f)->vec2f {
    return vec2f(pc.camera.y+1.2*focal()*p.x/p.z,100.0+focal()*p.y/p.z);
}
// The block's axes in camera space: across, back (from the top face's near
// edge to its far) and down (the front face), and world up.
fn slab_back()->vec3f {return vec3f(0,-pc.slab_axes.x,pc.slab_axes.y);}
fn slab_down()->vec3f {return vec3f(0,pc.slab_axes.y,pc.slab_axes.x);}
fn slab_size()->vec3f {return vec3f(pc.slab_axes.z,pc.slab_axes.w,pc.slab_size.x);}
// Block coordinates: across from its centre line, back from its near top
// edge, down from its top face.
fn slab_local(p:vec3f)->vec3f {
    let q=p-pc.slab_origin.xyz;
    return vec3f(q.x,dot(q,slab_back()),dot(q,slab_down()));
}
fn relief_at(l:vec3f,top:bool)->vec4f {
    let size=slab_size();
    if top {return textureSampleLevel(relief_top,linear,vec2f(l.x/size.x+.5,l.y/size.y),0.0);}
    return textureSampleLevel(relief_front,linear,vec2f(l.x/size.x+.5,l.z/size.z),0.0);
}
// The stone's distance field: the rounded block, less what is carved from
// its top and front faces. With `pooled`, the water its top keeps is part
// of it, for the camera's rays.
fn slab_distance(p:vec3f)->f32 {return slab_distance_with(p,false);}
fn slab_distance_with(p:vec3f,pooled:bool)->f32 {
    let l=slab_local(p);
    let size=slab_size();
    let r=pc.slab_origin.w;
    let q=abs(l-vec3f(0,size.y,size.z)*.5)-(size*.5-r);
    let block=length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-r;
    var depth=relief_at(l,true).r;
    if pooled {depth-=pool_at(l,depth);}
    let top=(depth-l.z)*LIPSCHITZ;
    let front=(relief_at(l,false).r-l.y)*LIPSCHITZ;
    return max(block,max(top,front));
}
// The pools' water at block point l, over stone carved `depth` deep there:
// its thickness, where its surface lies above the water around the slab
// (eased over a little, so a pool comes out of it as the level passes).
// None while the slab breaks up, or over the polished floor (D).
fn pool_at(l:vec3f,depth:f32)->f32 {
    let size=slab_size();
    if breakup_on() || pc.stone.y>.5 || l.y<0.0 || l.y>size.y || abs(l.x)>size.x*.5 {return 0.0;}
    let thick=textureSampleLevel(pools,linear,vec2f(l.x/size.x+.5,l.y/size.y),0.0).r;
    if thick<=0.0 {return 0.0;}
    let surface=pc.slab_origin.xyz+l.x*vec3f(1,0,0)+l.y*slab_back()+(depth-thick)*slab_down();
    return thick*smoothstep(0.0,.3,dot(surface-floor_origin(),floor_up()));
}
fn slab_normal(p:vec3f)->vec3f {
    let e=vec2f(.5,-.5);
    return normalize(e.xyy*slab_distance(p+e.xyy)+e.yyx*slab_distance(p+e.yyx)
        +e.yxy*slab_distance(p+e.yxy)+e.xxx*slab_distance(p+e.xxx));
}
// Where the ray from the camera (or from o) enters and leaves the block's
// bounds (a little enlarged), or (1, 0) when it misses.
fn slab_bounds(d:vec3f)->vec2f {return slab_bounds_from(vec3f(0),d);}
fn slab_bounds_from(origin:vec3f,d:vec3f)->vec2f {
    let size=slab_size()+2.0;
    let o=origin-pc.slab_origin.xyz;
    let ro=vec3f(o.x,dot(o,slab_back()),dot(o,slab_down()))+vec3f(0,1,1);
    let rd=vec3f(d.x,dot(d,slab_back()),dot(d,slab_down()));
    let lo=vec3f(-size.x*.5,0,0);
    let hi=vec3f(size.x*.5,size.y,size.z);
    let a=(lo-ro)/rd;
    let b=(hi-ro)/rd;
    let near=max(max(min(a.x,b.x),min(a.y,b.y)),min(a.z,b.z));
    let far=min(min(max(a.x,b.x),max(a.y,b.y)),max(a.z,b.z));
    return vec2f(max(near,0.0),far);
}
// The march: the distance along the ray from the camera (or from o) to
// the stone, or -1.
fn march(d:vec3f)->f32 {return march_from(vec3f(0),d);}
fn march_from(o:vec3f,d:vec3f)->f32 {return march_with(o,d,false);}
fn march_with(o:vec3f,d:vec3f,pooled:bool)->f32 {
    let bounds=slab_bounds_from(o,d);
    if bounds.x>=bounds.y {return -1.0;}
    var t=bounds.x;
    for(var i=0u;i<MARCH_STEPS;i++) {
        let h=slab_distance_with(o+d*t,pooled);
        if h<.0006*(t+length(o)) {return t;}
        t+=h;
        if t>bounds.y {break;}
    }
    return -1.0;
}
fn soft_shadow(p:vec3f,l:vec3f,reach:f32)->f32 {
    var lit=1.0;
    var t=.6;
    for(var i=0u;i<24u;i++) {
        let h=slab_distance(p+l*t);
        lit=min(lit,SHADOW_SHARPNESS*h/t);
        t+=clamp(h,.4,6.0);
        if lit<.02 || t>reach {break;}
    }
    return clamp(lit,0.0,1.0);
}
fn occlusion(p:vec3f,n:vec3f)->f32 {
    var o=0.0;
    for(var i=1u;i<=5u;i++) {
        let h=f32(i)*1.6;
        o+=(h-slab_distance(p+n*h))/h;
    }
    return clamp(1.0-.25*o,0.0,1.0);
}

// ---- The plasma's light ---------------------------------------------------------------
// The palette's colour at phase p, its sine without the bank's truncation
// (an interpretation), 6-bit DAC values: 31 + 31 sin(2 pi i / 224), at the
// level.
// The modern view takes the palette's rotation and level smoothed over
// time (plasma.rs, PALETTE_SMOOTH), so they glide.
fn sine_colour(p:f32)->vec3f {
    let r=pc.motion.y;
    let i=vec3f(r,r+pc.palette.z,r+pc.palette.w)+p;
    return (31.0+31.0*sin(2.0*PI*i/PALETTE_PERIOD))*clamp(pc.motion.z,0.0,63.0)/63.0;
}
// The same over the shutter, which stays open SHUTTER of a 70 Hz frame up to
// the moment: three instants along the palette's turn, in linear light.
fn shutter_colour(p:f32)->vec3f {
    var c=vec3f(0);
    for(var k=0;k<3;k++) {
        c+=dac_linear(sine_colour(p-pc.motion.x*SHUTTER*f32(k)*.5));
    }
    return c/3.0;
}
// Layer L's smoothed displacement at continuous sample k (its raster 2 k +
// L), between samples by Catmull-Rom and between frames linearly; held
// past its first and last rasters.
fn layer_sample(j:u32,layer:u32,k:i32)->f32 {
    let at=u32(clamp(k,0,i32(LAYER_SAMPLES)-1));
    return layers[(2u*j+layer)*LAYER_SAMPLES+at];
}
fn layer_offset(layer:u32,k:f32)->f32 {
    let i=i32(floor(k));
    let f=k-floor(k);
    let w=vec4f(((-f+2.0)*f-1.0)*f,(3.0*f-5.0)*f*f+2.0,((-3.0*f+4.0)*f+1.0)*f,(f-1.0)*f*f)*.5;
    var d=vec2f(0);
    for(var t=0u;t<2u;t++) {
        let j=select(frame_now(),frame_next(),t==1u);
        d[t]=dot(w,vec4f(layer_sample(j,layer,i-1),layer_sample(j,layer,i),
            layer_sample(j,layer,i+1),layer_sample(j,layer,i+2)));
    }
    return mix(d.x,d.y,frame_mix());
}
// Layer L's palette phase at a continuous raster position (raster r's
// centre at r) and source column x. Raster r starts at row r of the field
// (200 bytes a row) displaced by D(r) bytes along it, so at column D(r) +
// x / 4, running on into the next row past column 200. The row is
// continuous and D is the layer's smoothed displacement, so the layer is
// one continuous field. The field is taken as a seamless helix: its sine
// a whole turn over the row (the code's 5.1 a column falls 4 of 1024
// short) and its rise of one a row spread along the row, so running on
// into the next row leaves no seam (an interpretation; faithful HD keeps
// the code's).
fn layer_phase(layer:u32,raster:f32,x:f32)->f32 {
    let m=layer_offset(layer,(raster-f32(layer))*.5)+x/4.0;
    let phase=sine_at(m*1024.0/FIELD_ROW)/10.0+200.0+raster+m/FIELD_ROW;
    return phase-PALETTE_PERIOD*floor(phase/PALETTE_PERIOD);
}
// The swell on the orders' first rows.
fn swell()->f32 {
    var s=0.0;
    for(var h=0u;h<4u;h++) {
        let t=pc.hits[h];
        if t>=0.0 {s+=pc.hit_strength[h]*exp(-t/HIT_DECAY);}
    }
    return 1.0+HIT_SWELL*s;
}
// Sheet L's radiance at source point s: its layer's colour taken through
// the shared tonemap backwards by its luminance, so that alone it would
// show the original's colour (and its bloom keeps the hue), at half power:
// the two sheets together give the light the original's two layers do.
fn layer_light(layer:u32,s:vec2f)->vec3f {
    if !field_written() || pc.palette.y<=0.0 {return vec3f(0);}
    let c=shutter_colour(layer_phase(layer,s.y*2.0-.5,s.x));
    let y=dot(c,vec3f(.2126,.7152,.0722));
    if y<=0.0 {return vec3f(0);}
    let k=tonemap_inverse(vec3f(min(y,SHEET_WHITE))).x/y;
    return c*k*.5*SHEET_POWER*swell();
}
// The back sheet as the lens sees it, a little out of focus: its light
// over a circle of confusion BACK_DEFOCUS rows across, from its texture
// at the level that matches (a centre and six round it).
fn back_light(d:vec3f)->vec3f {
    let hit=sheet_hit(1u,vec3f(0),d);
    if hit.z<=0.0 && hit.y>=0.0 {return vec3f(0);}
    // The circle's radius in the sheet's texels, across and up.
    let rows=BACK_DEFOCUS*hit.z/(focal()*sheet_base(1u).w);
    let radius=vec2f(rows*sheet_base(1u).w/(2.0*sheet_half(1u)*COLUMN),rows);
    let level=clamp(log2(max(rows*SHEET_SIZE.y,1.0)),0.0,6.0);
    var c=vec3f(0);
    var weight=0.0;
    for(var k=0;k<7;k++) {
        let a=f32(k)*PI/3.0+.5;
        let o=select(vec2f(cos(a),sin(a)),vec2f(0),k==6)*radius*.8;
        let st=hit.xy+o;
        // Past its bottom edge the sheet gives nothing: a soft edge.
        let inside=select(0.0,1.0,st.y>=0.0);
        c+=sheet_sample(1u,clamp(st,vec2f(0),vec2f(1)),level)*inside;
        weight+=1.0;
    }
    return c/weight;
}
// The sheets: the front one's (0) and the back one's (1) bottom edge's
// centre and height, half width, a point at (across, up) in 0..1, and
// their direction up and normal (facing the camera and the slab). Both
// stand on the floor on one line, the back one leaning a little less
// (plasma_stone.rs).
fn sheet_base(i:u32)->vec4f {return select(pc.sheet,pc.sheet_back,i==1u);}
fn sheet_half(i:u32)->f32 {return select(pc.sheet_size.x,pc.sheet_back_size.x,i==1u);}
fn sheet_up(i:u32)->vec3f {return select(pc.sheet_size.yzw,pc.sheet_back_size.yzw,i==1u);}
fn sheet_point(i:u32,st:vec2f)->vec3f {
    let b=sheet_base(i);
    return b.xyz+vec3f((st.x*2.0-1.0)*sheet_half(i),0,0)+sheet_up(i)*st.y*b.w;
}
fn sheet_normal(i:u32)->vec3f {return normalize(cross(vec3f(1,0,0),sheet_up(i)));}
// Where a ray from o along d meets sheet i's plane: (across, up), 0 to 1
// on the sheet, and the distance, or a distance of -1 from behind.
fn sheet_meet(i:u32,o:vec3f,d:vec3f)->vec3f {
    let n=sheet_normal(i);
    let denom=dot(d,n);
    if denom>=0.0 {return vec3f(0,0,-1);}
    let b=sheet_base(i);
    let t=dot(b.xyz-o,n)/denom;
    if t<=0.0 {return vec3f(0,0,-1);}
    let q=o+d*t-b.xyz;
    return vec3f(q.x/sheet_half(i)*.5+.5,dot(q,sheet_up(i))/b.w,t);
}
// The same on the sheet only: a distance of -1 off it.
fn sheet_hit(i:u32,o:vec3f,d:vec3f)->vec3f {
    let hit=sheet_meet(i,o,d);
    if any(hit.xy<vec2f(0)) || any(hit.xy>vec2f(1)) {return vec3f(hit.xy,-1);}
    return hit;
}
// The sheets' soft glow along a ray from o along d, over them and
// spilling a little past their bottom edges.
fn glow_from(o:vec3f,d:vec3f)->vec3f {
    if !field_written() || pc.palette.y<=0.0 {return vec3f(0);}
    var c=vec3f(0);
    for(var i=0u;i<2u;i++) {
        let hit=sheet_meet(i,o,d);
        if hit.z<=0.0 {continue;}
        let fade=exp(-max(-hit.y,0.0)/GLOW_SPILL);
        c+=sheet_sample(i,clamp(hit.xy,vec2f(0),vec2f(1)),GLOW_LEVEL)*fade;
    }
    return c*GLOW;
}
fn glow(d:vec3f)->vec3f {return glow_from(vec3f(0),d);}
// Sheet i's light at level `level`, kept inside its half of the texture.
fn sheet_sample(i:u32,st:vec2f,level:f32)->vec3f {
    let half=.5/f32(512u>>u32(clamp(level,0.0,7.0)));
    let v=clamp((f32(i)+1.0-st.y)*.5,f32(i)*.5+half,f32(i)*.5+.5-half);
    return textureSampleLevel(sheet_light,linear,vec2f(st.x,v),level).rgb;
}
// The exact form factor of a quad (the projected solid angle over pi, as
// seen from p with normal n), Lambert's formula over its edges; the quad
// counter-clockwise as p sees it (quad_factor_either: either way round).
fn quad_factor_either(p:vec3f,n:vec3f,a:vec3f,b:vec3f,c:vec3f,d:vec3f)->f32 {
    let v=array<vec3f,4>(normalize(a-p),normalize(b-p),normalize(c-p),normalize(d-p));
    var f=0.0;
    for(var i=0u;i<4u;i++) {
        let v0=v[i];
        let v1=v[(i+1u)%4u];
        let x=clamp(dot(v0,v1),-.9999,.9999);
        f+=acos(x)*dot(n,normalize(cross(v0,v1)));
    }
    return abs(f)/(2.0*PI);
}
fn quad_factor(p:vec3f,n:vec3f,a:vec3f,b:vec3f,c:vec3f,d:vec3f)->f32 {
    let v=array<vec3f,4>(normalize(a-p),normalize(b-p),normalize(c-p),normalize(d-p));
    var f=0.0;
    for(var i=0u;i<4u;i++) {
        let v0=v[i];
        let v1=v[(i+1u)%4u];
        let x=clamp(dot(v0,v1),-.9999,.9999);
        f+=acos(x)*dot(n,normalize(cross(v0,v1)));
    }
    return max(f/(2.0*PI),0.0);
}
struct Gathered { light:vec3f, toward:vec3f }
// The sheet's light falling on a point with normal n: each cell's form
// factor times its mean radiance (irradiance over pi), and the direction it
// mostly comes from.
fn gather(p:vec3f,n:vec3f)->Gathered {
    var light=vec3f(0);
    var toward=vec3f(0);
    for(var s=0u;s<2u;s++) {
        // The back sheet, further off, in coarser cells.
        let cells=select(CELLS,HAZE_CELLS,s==1u);
        let level=select(CELL_LEVEL,HAZE_LEVEL,s==1u);
        let cell=1.0/vec2f(cells);
        for(var j=0u;j<cells.y;j++) {
            for(var i=0u;i<cells.x;i++) {
                let st=vec2f(f32(i),f32(j))*cell;
                let a=sheet_point(s,st);
                let b=sheet_point(s,st+vec2f(cell.x,0));
                let c=sheet_point(s,st+cell);
                let d=sheet_point(s,st+vec2f(0,cell.y));
                // Counter-clockwise as the lit side sees it.
                let f=quad_factor(p,n,a,d,c,b);
                let l=sheet_sample(s,st+cell*.5,level);
                light+=f*l;
                toward+=f*dot(l,vec3f(.3,.5,.2))*normalize((a+c)*.5-p);
            }
        }
    }
    return Gathered(light,normalize(toward+n*1e-4));
}
// The solid angle of triangle abc as seen from the origin (Van Oosterom
// and Strackee).
fn solid_angle(a:vec3f,b:vec3f,c:vec3f)->f32 {
    let la=length(a);let lb=length(b);let lc=length(c);
    let num=abs(dot(a,cross(b,c)));
    let den=la*lb*lc+dot(a,b)*lc+dot(a,c)*lb+dot(b,c)*la;
    return 2.0*atan2(num,den);
}
// The sheet's light scattered toward the camera by a mist that hangs at the
// sheet, thinning away from it (authored), along the ray up to `reach`:
// at each step, each cell's radiance times the solid angle it fills.
fn haze(d:vec3f,reach:f32)->vec3f {
    let n=sheet_normal(0u);
    let denom=dot(d,n);
    if denom>=0.0 {return vec3f(0);}
    let plane=dot(pc.sheet.xyz,n)/denom;
    let begin=max(plane-3.0*HAZE_DEPTH,1.0);
    let end=min(reach,plane+HAZE_DEPTH);
    if end<=begin {return vec3f(0);}
    let span=(end-begin)/f32(HAZE_STEPS);
    let cell=1.0/vec2f(HAZE_CELLS);
    // Each cell's radiance, both lights, the same at every step: taken once.
    var radiance:array<vec3f,8>;
    for(var j=0u;j<HAZE_CELLS.y;j++) {
        for(var i=0u;i<HAZE_CELLS.x;i++) {
            let st=vec2f(f32(i),f32(j))*cell;
            radiance[j*HAZE_CELLS.x+i]=sheet_sample(0u,st+cell*.5,HAZE_LEVEL)
                +sheet_sample(1u,st+cell*.5,HAZE_LEVEL);
        }
    }
    var c=vec3f(0);
    for(var k=0u;k<HAZE_STEPS;k++) {
        let t=begin+(f32(k)+.5)*span;
        let q=d*t;
        let density=exp(-abs(t-plane)*abs(denom)/HAZE_DEPTH);
        // The two sheets stand close: both lights, at the front one's
        // place.
        var light=vec3f(0);
        for(var j=0u;j<HAZE_CELLS.y;j++) {
            for(var i=0u;i<HAZE_CELLS.x;i++) {
                let st=vec2f(f32(i),f32(j))*cell;
                let a=sheet_point(0u,st)-q;
                let b=sheet_point(0u,st+vec2f(cell.x,0))-q;
                let cc=sheet_point(0u,st+cell)-q;
                let e=sheet_point(0u,st+vec2f(0,cell.y))-q;
                let omega=solid_angle(a,b,cc)+solid_angle(a,cc,e);
                light+=omega*radiance[j*HAZE_CELLS.x+i];
            }
        }
        c+=light*density;
    }
    return c*HAZE*span/(4.0*PI);
}

// ---- Shading ------------------------------------------------------------------------------
fn ggx(n:vec3f,v:vec3f,l:vec3f,alpha:f32)->f32 {
    let h=normalize(v+l);
    let nh=max(dot(n,h),0.0);
    let a2=alpha*alpha;
    let den=nh*nh*(a2-1.0)+1.0;
    return a2/(PI*den*den);
}
fn fresnel(cosine:f32)->f32 {return SHEEN+(1.0-SHEEN)*pow(1.0-clamp(cosine,0.0,1.0),5.0);}
// Value noise and a few octaves of it, over the faces (authored grain).
fn hash3(p:vec3f)->f32 {return fract(sin(dot(p,vec3f(127.1,311.7,74.7)))*43758.5453);}
fn noise(p:vec3f)->f32 {
    let i=floor(p);
    let f=p-i;
    let w=f*f*(3.0-2.0*f);
    var n=0.0;
    for(var k=0u;k<8u;k++) {
        let o=vec3f(f32(k&1u),f32((k>>1u)&1u),f32(k>>2u));
        let c=select(1.0-w,w,o>vec3f(.5));
        n+=hash3(i+o)*c.x*c.y*c.z;
    }
    return n;
}
fn grain(l:vec3f)->f32 {
    let q=l/GRAIN;
    return .55*noise(q)+.3*noise(q*2.7+7.1)+.15*noise(q*7.3+3.3);
}
// The stone's surface at p: its normal (the grain's bump in it), albedo
// and roughness.
struct StoneSurface { n:vec3f, albedo:vec3f, roughness:f32 }
fn stone_surface(p:vec3f)->StoneSurface {
    var n=slab_normal(p);
    let l=slab_local(p);
    let size=slab_size();
    // Which face's relief colours the point: the top's above the front's
    // carving, the front's below.
    let top=l.z<size.z*.5 && l.z<l.y+3.0;
    let r=relief_at(l,top);
    // The grain: mottling and a fine bump, from the noise's gradient.
    let g=grain(l);
    let e=.35;
    let gradient=vec3f(grain(l+vec3f(e,0,0)),grain(l+vec3f(0,e,0)),grain(l+vec3f(0,0,e)))-g;
    let bump=gradient.x*vec3f(1,0,0)+gradient.y*slab_back()+gradient.z*slab_down();
    // The cut is rough; the polished faces keep only the grain's colour.
    let cut=clamp(max(r.b*1.6,r.a),0.0,1.0);
    let roughness=mix(POLISHED,ROUGH,cut);
    n=normalize(n-GRAIN_BUMP*cut/e*(bump-dot(bump,n)*n));
    let albedo=vec3f(pc.slab_size.zw,pc.stone.x)*r.g*(1.0-.22*r.b)*(1.0-.3*r.a)
        *(1.0+GRAIN_ALBEDO*(g-.5)*2.0);
    return StoneSurface(n,albedo,roughness);
}
fn shade_stone(p:vec3f,d:vec3f)->vec3f {
    let surface=stone_surface(p);
    let n=surface.n;
    let albedo=surface.albedo;
    let roughness=surface.roughness;
    let v=-d;
    let ao=occlusion(p,n);
    let level=select(0.0,clamp(pc.palette.y/63.0,0.0,1.0),field_written());
    let key_light=KEY*mix(KEY_DARK,1.0-KEY_DUCK,level);
    let key=normalize(KEY_DIR);
    let shadow=soft_shadow(p+n*.3,key,120.0);
    let nk=max(dot(n,key),0.0);
    var c=albedo*(key_light*nk*shadow+FILL*max(dot(n,v),0.0)+AMBIENT*ao)*ao;
    c+=key_light*shadow*fresnel(dot(n,v))*ggx(n,v,key,max(roughness,.15))*nk;
    // The plasma's light: diffuse, shadowed toward where it comes from, and
    // the sheen reflecting the sheet.
    if level>0.0 {
        let gathered=gather(p,n);
        let blocked=soft_shadow(p+n*.3,gathered.toward,140.0);
        c+=albedo*gathered.light*blocked*ao;
        // The polished faces mirror the sheet, the cut blurs it; shadowed
        // where the carving stands in the way.
        let rd=reflect(d,n);
        let texel=2.0*pc.sheet_size.x/SHEET_SIZE.x;
        var mirrored=vec3f(0);
        for(var i=0u;i<2u;i++) {
            let hit=sheet_hit(i,p+n*.3,rd);
            if hit.z>0.0 {
                // The lobe's footprint on the sheet, in its texels.
                let lod=clamp(log2(max(2.0*roughness*hit.z/texel,1.0)),0.0,7.0);
                let edge=smoothstep(0.0,.04,min(min(hit.x,1.0-hit.x),min(hit.y,1.0-hit.y)));
                mirrored+=sheet_sample(i,hit.xy,lod)*edge;
            }
        }
        if any(mirrored>vec3f(0)) {
            c+=mirrored*fresnel(dot(n,v))*soft_shadow(p+n*.3,rd,80.0);
        }
    }
    return c;
}

// ---- The ground (authored) ----------------------------------------------------------
// The original has no ground; this one is part 3's water as it lies
// through part 3's dots (dots_water.rs), level below the camera (or as
// tilted, pc.ground.w), seen through the camera the logo implies, running
// on under the sheets, which hang over it, to the horizon. It slides with
// the slab.
fn floor_up()->vec3f {return vec3f(0.0,-cos(pc.ground.w),-sin(pc.ground.w));}
// Back along it, away from the camera.
fn ground_back()->vec3f {return vec3f(0.0,-sin(pc.ground.w),cos(pc.ground.w));}
// The ground straight below the slab's near top edge's centre.
fn floor_origin()->vec3f {return pc.ground.xyz;}
// Ground coordinates: across, and back from below the near top edge.
fn floor_local(p:vec3f)->vec2f {
    let q=p-floor_origin();
    return vec2f(q.x,dot(q,ground_back()));
}
// Where a ray from o along d meets the ground, or -1.
fn floor_hit(o:vec3f,d:vec3f)->f32 {
    let n=floor_up();
    let denom=dot(d,n);
    if denom>=0.0 {return -1.0;}
    let t=dot(floor_origin()-o,n)/denom;
    if t<=0.0 {return -1.0;}
    return t;
}
// The light both sheets send from o along d (0 off them), at the level of
// their light that matches a spread `spread` per unit of distance, its
// edges soft but for the bottom ones, which stand on the slab's base plane.
fn sheets_toward(o:vec3f,d:vec3f,spread:f32)->vec3f {
    let texel=2.0*pc.sheet_size.x/SHEET_SIZE.x;
    var c=vec3f(0);
    for(var i=0u;i<2u;i++) {
        let hit=sheet_meet(i,o,d);
        if hit.z<=0.0 || hit.y<0.0 || hit.y>1.0 || hit.x<0.0 || hit.x>1.0 {continue;}
        let lod=clamp(log2(max(spread*hit.z/texel,1.0)),0.0,7.0);
        let edge=smoothstep(0.0,.04,min(min(hit.x,1.0-hit.x),1.0-hit.y));
        c+=sheet_sample(i,hit.xy,lod)*edge;
    }
    return c;
}
// The sheets' light the slab keeps from a point of the floor: over each
// side of the block the point sees (its silhouette from the floor), in a
// few strips, each strip's form factor times the light the sheets send
// through it.
fn slab_blocks(p:vec3f,n:vec3f)->vec3f {
    let size=slab_size();
    let l=slab_local(p);
    let o=pc.slab_origin.xyz;
    let across=vec3f(1,0,0);
    let top=0.0;
    let base=size.z;
    var c=vec3f(0);
    // The sides: x = -w/2 and w/2 (back from 0 to the depth), and the
    // front and back faces (across): each when the point is outside it.
    for(var f=0u;f<4u;f++) {
        var a0:vec3f;var a1:vec3f;
        if f<2u {
            let x=select(-.5,.5,f==1u)*size.x;
            if (f==0u && l.x>=x) || (f==1u && l.x<=x) {continue;}
            a0=o+across*x;
            a1=a0+slab_back()*size.y;
        } else {
            let y=select(0.0,size.y,f==3u);
            if (f==2u && l.y>=y) || (f==3u && l.y<=y) {continue;}
            a0=o+slab_back()*y-across*size.x*.5;
            a1=a0+across*size.x;
        }
        for(var k=0u;k<SHADOW_STRIPS;k++) {
            let u0=mix(a0,a1,f32(k)/f32(SHADOW_STRIPS));
            let u1=mix(a0,a1,f32(k+1u)/f32(SHADOW_STRIPS));
            let ff=quad_factor_either(p,n,u0+slab_down()*top,u1+slab_down()*top,
                u1+slab_down()*base,u0+slab_down()*base);
            let centre=(u0+u1)*.5+slab_down()*(top+base)*.5;
            let toward=normalize(centre-p);
            c+=ff*sheets_toward(p,toward,1.0);
        }
    }
    return c;
}
fn shade_floor(p:vec3f,d:vec3f)->vec3f {
    // It shows only by the sheets' light.
    if !field_written() || pc.palette.y<=0.0 {return vec3f(0);}
    let n=floor_up();
    let v=-d;
    let l=floor_local(p);
    // Its mottling and the polish varying with it.
    let q=vec3f(l,0)/FLOOR_GRAIN;
    let m=.6*noise(q)+.4*noise(q*2.9+5.3);
    let polish=.5*noise(q*.37+1.7)+.5*noise(q*.9+8.1);
    let albedo=FLOOR_ALBEDO*(1.0+FLOOR_MOTTLE*(m-.5)*2.0);
    let roughness=FLOOR_ROUGH*(1.0+FLOOR_ROUGH_VARY*(polish-.5)*2.0);
    // The sheets' light falling on it, less what the slab keeps from it.
    let gathered=gather(p,n);
    var c=albedo*max(gathered.light-slab_blocks(p,n),vec3f(0));
    // Its mirror image: the sheets and their glow, or the slab where it
    // stands in the way (from this camera that falls outside the frame:
    // the floor in front of the slab, which would mirror it, is below
    // it), softer the further the reflected ray goes, and fading.
    let o=p+n*.3;
    let rd=reflect(d,n);
    var mirrored=vec3f(0);
    let t=march_from(o,rd);
    var reach=1e4;
    if t>0.0 {
        mirrored=shade_stone(o+rd*t,rd);
        reach=t;
    } else {
        mirrored=sheets_toward(o,rd,2.0*roughness)+glow_from(o,rd);
        let hit=sheet_meet(0u,o,rd);
        if hit.z>0.0 {reach=hit.z;}
    }
    let f=FLOOR_F0+(1.0-FLOOR_F0)*pow(1.0-clamp(dot(n,v),0.0,1.0),5.0);
    c+=mirrored*f*exp(-reach/FLOOR_FADE)*(1.0+FLOOR_SHEEN_MOTTLE*(m-.5)*2.0);
    return c;
}

// ---- The water (authored) -------------------------------------------------------------
fn shade_water(p:vec3f,d:vec3f)->vec3f {
    // Its surface shows only by the sheets' light (and the stone's mirror
    // image); what lies under it, by any light.
    let sheets_lit=field_written() && pc.palette.y>0.0;
    let up=floor_up();
    let v=-d;
    let q=floor_local(p);
    // A pixel's size on the water, across and back (foreshortened), and
    // for the drops' rings, across its foreshortening.
    let pixel=length(p)*200.0/(focal()*u.screen.y);
    let swell=water_slope(q,pc.water,vec2f(pixel,pixel/max(dot(up,v),.01)));
    var g=swell.xy;
    let footprint=pixel/max(dot(up,v),.2);
    // (Unrolled: indexing the controls with a variable slows and changes
    // the whole shader's code.)
    g+=drop_slope(q,pc.drops[0],footprint)+drop_slope(q,pc.drops[1],footprint)
        +drop_slope(q,pc.drops[2],footprint)+drop_slope(q,pc.drops[3],footprint)
        +drop_slope(q,pc.drops[4],footprint)+drop_slope(q,pc.drops[5],footprint);
    let n=normalize(up-g.x*vec3f(1,0,0)-g.y*ground_back());
    let o=p+up*.3;
    // The mirror image, over the slopes too small to draw, spread in the
    // plane of incidence (Gaussian weights): the sheets, their glow, or the
    // slab where it stands in the way (its part above the water, mirrored
    // just below where it comes out).
    let away=normalize(d-dot(d,up)*up);
    let rd=reflect(d,n);
    let f=water_share(water_fresnel(dot(n,v)),pc.water.x);
    var mirrored=vec3f(0);
    let t=march_from(o,rd);
    if t>0.0 {
        mirrored=shade_stone(o+rd*t,rd)*f;
    } else if sheets_lit {
        var weight=0.0;
        for(var j=0;j<5;j++) {
            let x=f32(j-2)*.8;
            let nj=normalize(n+away*(WATER_FINE+swell.z)*x);
            let rj=reflect(d,nj);
            if dot(rj,up)<=0.0 {continue;}
            let wj=exp(-.5*x*x);
            mirrored+=wj*water_share(water_fresnel(dot(nj,v)),pc.water.x)*sheets_toward(o,rj,WATER_FINE+swell.z);
            weight+=wj;
        }
        mirrored=mirrored/max(weight,1e-4)+f*glow_from(o,rd);
    }
    // The light its body scatters back: of the sheets' light that enters,
    // less what the slab keeps from it, so a little darker in its shade.
    var lit=vec3f(0);
    if sheets_lit {lit=max(gather(p,up).light-slab_blocks(p,up),vec3f(0));}
    // What lies under the surface: the stone, seen along the ray refracted
    // down through the moving surface, fading over the water it is seen
    // through and the water its light comes down through (WATER_CLARITY).
    // Its radiance comes out of the water spread from the cone it filled
    // there (Snell's window) over the wider one above, so divided by the
    // index squared (the n-squared law for radiance), besides what the
    // surface reflects back (1 - f, below). The light falling on it is
    // taken as in air: the key comes in at 54 degrees, where water lets
    // through 96%.
    var beneath=vec3f(0);
    let into=refract(d,n,1.0/WATER_ETA);
    let below=p-up*.3;
    let ti=march_from(below,into);
    if ti>0.0 {
        let q=below+into*ti;
        let depth=max(dot(p-q,up),0.0);
        beneath=shade_stone(q,into)*exp(-(ti+depth)/WATER_CLARITY)/(WATER_ETA*WATER_ETA);
    }
    return mirrored+max(1.0-f,0.0)*(WATER_BODY*lit+beneath);
}
// ---- The pools (authored) ---------------------------------------------------------------
// The still water the carving keeps, as the water around the slab is
// shaded, level and unruffled: the sheets, their glow, or the stone
// mirrored, and the stone beneath seen through it.
fn shade_pool(p:vec3f,d:vec3f)->vec3f {
    let up=floor_up();
    let v=-d;
    let f=water_share(water_fresnel(dot(up,v)),pc.water.x);
    let o=p+up*.05;
    let rd=reflect(d,up);
    var mirrored=vec3f(0);
    let t=march_from(o,rd);
    if t>0.0 {
        mirrored=shade_stone(o+rd*t,rd)*f;
    } else if field_written() && pc.palette.y>0.0 {
        mirrored=f*(sheets_toward(o,rd,WATER_FINE)+glow_from(o,rd));
    }
    var beneath=vec3f(0);
    let into=refract(d,up,1.0/WATER_ETA);
    let below=p-up*.05;
    let ti=march_from(below,into);
    if ti>0.0 {
        beneath=shade_stone(below+into*ti,into)*exp(-ti/WATER_CLARITY)/(WATER_ETA*WATER_ETA);
    }
    return mirrored+max(1.0-f,0.0)*beneath;
}
// Whether the camera's ray meets the pools' water at p rather than the
// stone: closer to the water's surface than to the stone under it.
fn on_pool(p:vec3f)->bool {
    let l=slab_local(p);
    let depth=relief_at(l,true).r;
    let thick=pool_at(l,depth);
    return thick>0.0 && l.z<depth-.5*thick;
}

// The ground: the water, or the polished floor.
fn shade_ground(p:vec3f,d:vec3f)->vec3f {
    if pc.stone.y>.5 {return shade_floor(p,d);}
    return shade_water(p,d);
}

// ---- Entry points ------------------------------------------------------------------------
// The sheets' light, over each sheet (across, and from its top edge
// down): the front sheet in the upper half, the back one in the lower.
@fragment fn plasma_sheet(v:Vertex)->@location(0) vec4f {
    plasma_begin();
    let i=select(0u,1u,v.uv.y>=.5);
    let p=sheet_point(i,vec2f(v.uv.x,1.0-(v.uv.y*2.0-f32(i))));
    return vec4f(layer_light(i,project(p)),1);
}
// One mip level of the sheet's light from the one above it.
@fragment fn plasma_downsample(v:Vertex)->@location(0) vec4f {
    let at=vec2i(v.position.xy)*2;
    var c=vec3f(0);
    for(var y=0;y<2;y++) {
        for(var x=0;x<2;x++) {
            c+=textureLoad(sheet_level,at+vec2i(x,y),0).rgb;
        }
    }
    return vec4f(c*.25,1);
}
@fragment fn plasma_style(v:Vertex)->@location(0) vec4f {return picture(v).light;}
// While the slab breaks up, part 7's view takes it in two images: the
// picture, how much of the slab still stands in alpha; and the stone's
// surface (its normal, part 4's camera space, and its albedo over the
// stone's colour; 0 off it), which part 7 lights by its lamp and beam.
struct Breaking { @location(0) light:vec4f, @location(1) surface:vec4f }
@fragment fn plasma_breaking(v:Vertex)->Breaking {
    let p=picture(v);
    return Breaking(p.light,p.surface);
}
struct Picture { light:vec4f, surface:vec4f }
fn picture(v:Vertex)->Picture {
    plasma_begin();
    let pixel=vec2i(v.position.xy);
    let s=source_of_scene(scene_of_uv(v.uv));
    var c=vec3f(0);
    // While the slab breaks up, how much of it still stands here (else 1),
    // and its surface.
    var standing=select(1.0,0.0,breakup_on());
    var surface=vec4f(0);
    if !part7_shown() {
        let d=ray_of_source(s);
        let t=march_with(vec3f(0),d,true);
        var reach=pc.sheet.z+400.0;
        var on_sheets=false;
        // The ground, where it lies in front of the sheets, and of the stone
        // where the stone stands in the water (the water lies over what of
        // it is under the surface).
        let hit=sheet_hit(0u,vec3f(0),d);
        let at=floor_hit(vec3f(0),d);
        let ground=at>0.0 && (hit.z<=0.0 || at<hit.z) && (t<=0.0 || at<t);
        if t>0.0 && !ground {
            if on_pool(d*t) {c=shade_pool(d*t,d);} else {c=shade_stone(d*t,d);}
            reach=t;
            if breakup_on() {
                let l=slab_local(d*t);
                let top=l.z<slab_size().z*.5 && l.z<l.y+3.0;
                standing=breakup_standing(d,select(1,0,top));
                let stone=stone_surface(d*t);
                surface=vec4f(stone.n,stone.albedo.g/max(pc.slab_size.w,1e-6));
            }
        } else {
            // The two sheets of light, the front one sharp and the back one
            // through it, softer; below them, the ground.
            if ground {
                c=shade_ground(d*at,d);
                reach=at;
            } else if hit.z>0.0 {
                c=layer_light(0u,s)+back_light(d);
                reach=hit.z;
                on_sheets=true;
            }
            c+=glow(d);
        }
        // The mist shows where the sheets don't: over the stone and below
        // their edges (over them their own light is far brighter).
        if field_written() && pc.palette.y>0.0 && !on_sheets {c+=haze(d,reach);}
        // Overexposed, the stone goes white as the film does, not orange:
        // its colour gives way to its luminance while the exposure is high.
        let over=clamp((pc.camera.w-1.0)/ARRIVAL_WASH,0.0,1.0);
        c=mix(c,vec3f(dot(c,vec3f(.2126,.7152,.0722))),over)*pc.camera.w;
    }
    if pc.camera.z>0.0 {c=mix(c,textureLoad(part3_view,pixel,0).rgb,pc.camera.z);}
    c*=1.0+DITHER*(hash3(vec3f(vec2f(pixel),7.0))-.5);
    return Picture(vec4f(max(c,vec3f(0)),standing),surface);
}
