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
// - The plasma is a sheet of light standing behind the slab and leaning a
//   little over it: on it, at each point, the colour the original shows
//   there. Its rasters' two layers (the even ones and the odd, which the
//   strides move apart) are mixed as the eye mixes them; the field and the
//   palette's sine are evaluated at continuous positions, over a short
//   shutter along the palette's turn, and the palette's level is the
//   light's. Its radiance is the original's colour taken back through the
//   shared tonemap, so it shows about the original's colours.
// - The sheet lights the stone: from behind and above, raking across the
//   top face, so the carving catches it at its rims, and the polished faces
//   mirror it, in the plasma's moving colours. Its light is gathered over
//   the sheet in cells (each a rectangle whose exact form factor is taken,
//   its radiance the mean of its part of the sheet), shadowed by the
//   carving toward where it comes from; the reflection reads the sheet's
//   light at the level its spread matches. A dim warm key light from the
//   upper left, where the logo's painted light comes from, keeps the stone
//   visible while the plasma is dark and gives way as it rises.
// - A mist hanging at the sheet scatters a little of its light, so it
//   spills softly below the sheet's edge.
// - The arrival: part 3's flare overexposes the camera, and the exposure
//   settles over the 64 frames of the original's fade from white, so the
//   carving (the darkest) comes out of the white first; part 3's view hands
//   over to this one in its first moments.
// - The music: each order's first row swells the sheet's light a little.
// - The slide: the slab sinks with the original's line compare.
// Module: the shared controls, plasma_common.wgsl and this file.

// ---- The look (authored) -------------------------------------------------------
// The sheet's power (its radiance is the original's colour, linear, times
// this), and the swell of light on each order's first row (and its decay,
// seconds).
const SHEET_POWER=1.0;
// The brightest the sheet shows through the tonemap (its luminance then is
// about 3).
const SHEET_WHITE=.95;
const HIT_SWELL=.18;
// The shutter, as a fraction of a 70 Hz frame: about 1/120 s.
const SHUTTER=.6;
const HIT_DECAY=.45;
// The key light: toward it (camera space), its colour and strength; a cool
// fill from the camera's side, and the ambient light.
const KEY_DIR=vec3f(-.5,-.55,-.67);
const KEY=vec3f(1.0,.93,.84)*.75;
// How far the key dims as the plasma comes up (at its full level).
const KEY_DUCK=.8;
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
// The sheet's cells for the light (its 512 x 256 texture's level 6 is 8 x
// 4 texels, one a cell) and for the haze (level 7, 4 x 2).
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
// The sheet's light, 512 x 256 with its mips (written by plasma_sheet and
// plasma_downsample), across and from its top edge down; and part 3's
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
// its top and front faces.
fn slab_distance(p:vec3f)->f32 {
    let l=slab_local(p);
    let size=slab_size();
    let r=pc.slab_origin.w;
    let q=abs(l-vec3f(0,size.y,size.z)*.5)-(size*.5-r);
    let block=length(max(q,vec3f(0)))+min(max(q.x,max(q.y,q.z)),0.0)-r;
    let top=(relief_at(l,true).r-l.z)*LIPSCHITZ;
    let front=(relief_at(l,false).r-l.y)*LIPSCHITZ;
    return max(block,max(top,front));
}
fn slab_normal(p:vec3f)->vec3f {
    let e=vec2f(.5,-.5);
    return normalize(e.xyy*slab_distance(p+e.xyy)+e.yyx*slab_distance(p+e.yyx)
        +e.yxy*slab_distance(p+e.yxy)+e.xxx*slab_distance(p+e.xxx));
}
// Where the ray enters and leaves the block's bounds (a little enlarged),
// or (1, 0) when it misses.
fn slab_bounds(d:vec3f)->vec2f {
    let size=slab_size()+2.0;
    let o=-pc.slab_origin.xyz;
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
// The march: the distance along the ray to the stone, or -1.
fn march(d:vec3f)->f32 {
    let bounds=slab_bounds(d);
    if bounds.x>=bounds.y {return -1.0;}
    var t=bounds.x;
    for(var i=0u;i<MARCH_STEPS;i++) {
        let h=slab_distance(d*t);
        if h<.0006*t {return t;}
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
fn sine_colour(p:f32)->vec3f {
    let i=rotations()+p;
    return (31.0+31.0*sin(2.0*PI*i/PALETTE_PERIOD))*clamp(pc.palette.y,0.0,63.0)/63.0;
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
// Layer L's palette phase at a continuous raster position (raster r's
// centre at r) and source column x. Its rasters are 2 k + L, 0 to 225;
// raster r starts at row r of the field (200 bytes a row) displaced by
// D(r) bytes along it, so at column D(r) + x / 4, running on into the next
// row (and its phase one higher) past column 200. Between the layer's
// rasters the row is continuous and D is interpolated (not the address:
// half way along a row is half way across the field); past them D holds.
fn layer_phase(layer:u32,raster:f32,x:f32)->f32 {
    let last=f32((PLASMA_RASTERS-1u-layer)/2u);
    let k=clamp((raster-f32(layer))*.5,0.0,last);
    let k0=min(floor(k),last-1.0);
    let r0=2u*u32(k0)+layer;
    let r1=r0+2u;
    let d0=raster_at(r0)-FIELD_START-FIELD_ROW*f32(r0);
    let d1=raster_at(r1)-FIELD_START-FIELD_ROW*f32(r1);
    let m=mix(d0,d1,clamp(k-k0,0.0,1.0))+x/4.0;
    let wraps=floor(m/FIELD_ROW);
    let phase=sine_at((m-wraps*FIELD_ROW)*5.1)/10.0+200.0+raster+wraps;
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
// The sheet's radiance at source point s: the two layers' colours mixed
// as the eye mixes the original's alternate rasters, and taken through the
// shared tonemap backwards, so the sheet shows about the original's
// colours (but for the bloom) and gives the light that would.
fn plasma_light(s:vec2f)->vec3f {
    if !field_written() || pc.palette.y<=0.0 {return vec3f(0);}
    var c=vec3f(0);
    for(var layer=0u;layer<2u;layer++) {
        c+=shutter_colour(layer_phase(layer,s.y*2.0-.5,s.x));
    }
    // Inverted by luminance, so the light keeps the colour's hue (and its
    // bloom does too).
    let shown=c*.5;
    let y=dot(shown,vec3f(.2126,.7152,.0722));
    if y<=0.0 {return vec3f(0);}
    let k=tonemap_inverse(vec3f(min(y,SHEET_WHITE))).x/y;
    return shown*k*SHEET_POWER*swell();
}
// The sheet: its bottom edge's centre, across, up, its size.
fn sheet_up()->vec3f {return pc.sheet_size.yzw;}
fn sheet_point(st:vec2f)->vec3f {
    return pc.sheet.xyz+vec3f((st.x*2.0-1.0)*pc.sheet_size.x,0,0)+sheet_up()*st.y*pc.sheet.w;
}
// The sheet faces the camera and the slab.
fn sheet_normal()->vec3f {return normalize(cross(vec3f(1,0,0),sheet_up()));}
// Where a ray from o along d meets the sheet: (across, up) in 0..1 and the
// distance, or a distance of -1.
fn sheet_hit(o:vec3f,d:vec3f)->vec3f {
    let n=sheet_normal();
    let denom=dot(d,n);
    if denom>=0.0 {return vec3f(0,0,-1);}
    let t=dot(pc.sheet.xyz-o,n)/denom;
    if t<=0.0 {return vec3f(0,0,-1);}
    let q=o+d*t-pc.sheet.xyz;
    let st=vec2f(q.x/pc.sheet_size.x*.5+.5,dot(q,sheet_up())/pc.sheet.w);
    if any(st<vec2f(0)) || any(st>vec2f(1)) {return vec3f(st,-1);}
    return vec3f(st,t);
}
fn sheet_sample(st:vec2f,level:f32)->vec3f {
    return textureSampleLevel(sheet_light,linear,vec2f(st.x,1.0-st.y),level).rgb;
}
// The exact form factor of a quad (the projected solid angle over pi, as
// seen from p with normal n), Lambert's formula over its edges.
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
    let cell=1.0/vec2f(CELLS);
    for(var j=0u;j<CELLS.y;j++) {
        for(var i=0u;i<CELLS.x;i++) {
            let st=vec2f(f32(i),f32(j))*cell;
            let a=sheet_point(st);
            let b=sheet_point(st+vec2f(cell.x,0));
            let c=sheet_point(st+cell);
            let d=sheet_point(st+vec2f(0,cell.y));
            // Counter-clockwise as the lit side sees it.
            let f=quad_factor(p,n,a,d,c,b);
            let l=sheet_sample(st+cell*.5,CELL_LEVEL);
            light+=f*l;
            toward+=f*dot(l,vec3f(.3,.5,.2))*normalize((a+c)*.5-p);
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
    let n=sheet_normal();
    let denom=dot(d,n);
    if denom>=0.0 {return vec3f(0);}
    let plane=dot(pc.sheet.xyz,n)/denom;
    let begin=max(plane-3.0*HAZE_DEPTH,1.0);
    let end=min(reach,plane+HAZE_DEPTH);
    if end<=begin {return vec3f(0);}
    let span=(end-begin)/f32(HAZE_STEPS);
    let cell=1.0/vec2f(HAZE_CELLS);
    var c=vec3f(0);
    for(var k=0u;k<HAZE_STEPS;k++) {
        let t=begin+(f32(k)+.5)*span;
        let q=d*t;
        let density=exp(-abs(t-plane)*abs(denom)/HAZE_DEPTH);
        var light=vec3f(0);
        for(var j=0u;j<HAZE_CELLS.y;j++) {
            for(var i=0u;i<HAZE_CELLS.x;i++) {
                let st=vec2f(f32(i),f32(j))*cell;
                let a=sheet_point(st)-q;
                let b=sheet_point(st+vec2f(cell.x,0))-q;
                let cc=sheet_point(st+cell)-q;
                let e=sheet_point(st+vec2f(0,cell.y))-q;
                let omega=solid_angle(a,b,cc)+solid_angle(a,cc,e);
                light+=omega*sheet_sample(st+cell*.5,HAZE_LEVEL);
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
fn shade_stone(p:vec3f,d:vec3f)->vec3f {
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
    let v=-d;
    let ao=occlusion(p,n);
    let level=select(0.0,clamp(pc.palette.y/63.0,0.0,1.0),field_written());
    let key_light=KEY*(1.0-KEY_DUCK*level);
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
        let hit=sheet_hit(p+n*.3,rd);
        if hit.z>0.0 {
            // The lobe's footprint on the sheet, in its texels.
            let texel=2.0*pc.sheet_size.x/SHEET_SIZE.x;
            let lod=clamp(log2(max(2.0*roughness*hit.z/texel,1.0)),0.0,7.0);
            let edge=smoothstep(0.0,.04,min(min(hit.x,1.0-hit.x),min(hit.y,1.0-hit.y)));
            let clear=soft_shadow(p+n*.3,rd,80.0);
            c+=sheet_sample(hit.xy,lod)*fresnel(dot(n,v))*clear*edge;
        }
    }
    return c;
}

// ---- Entry points ------------------------------------------------------------------------
// The sheet's light, over the sheet (across, and from its top edge down).
@fragment fn plasma_sheet(v:Vertex)->@location(0) vec4f {
    plasma_begin();
    let p=sheet_point(vec2f(v.uv.x,1.0-v.uv.y));
    return vec4f(plasma_light(project(p)),1);
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
@fragment fn plasma_style(v:Vertex)->@location(0) vec4f {
    plasma_begin();
    let pixel=vec2i(v.position.xy);
    let s=source_of_scene(scene_of_uv(v.uv));
    var c=vec3f(0);
    if !part7_shown() {
        let d=ray_of_source(s);
        let t=march(d);
        var reach=pc.sheet.z+400.0;
        if t>0.0 {
            c=shade_stone(d*t,d);
            reach=t;
        } else {
            let hit=sheet_hit(vec3f(0),d);
            if hit.z>0.0 {
                c=plasma_light(s);
                reach=hit.z;
            }
        }
        if field_written() && pc.palette.y>0.0 {c+=haze(d,reach);}
        // Overexposed, the stone goes white as the film does, not orange:
        // its colour gives way to its luminance while the exposure is high.
        let over=clamp((pc.camera.w-1.0)/ARRIVAL_WASH,0.0,1.0);
        c=mix(c,vec3f(dot(c,vec3f(.2126,.7152,.0722))),over)*pc.camera.w;
    }
    if pc.camera.z>0.0 {c=mix(c,textureLoad(part3_view,pixel,0).rgb,pc.camera.z);}
    return vec4f(max(c,vec3f(0)),1);
}
