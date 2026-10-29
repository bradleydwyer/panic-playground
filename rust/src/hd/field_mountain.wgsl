// Part 5's mountain: the mountain author's file (with field_mountain.rs).
// field_render.rs builds the modern module from the shared controls,
// field_common.wgsl, field_frame.wgsl, field_zoom.wgsl, this file and
// field_style.wgsl. Prefix this file's own names with mountain_. It defines:
//   fn mountain_layer(px:FieldPixel)->FieldLayer   the mountain, blurred
//       along its motion through the exposure
//   fn mountain_prepass(px:FieldPixel)->vec4f      the mountain drawn sharp
//       at the middle of the exposure, at full resolution (MOUNTAIN.prepass
//       and full_prepass), read with mountain_prepass_at
//
// A volcano made from the Mandelbrot fit of DEP.U. What comes from where:
// - Derived: the camera through the orbit and the sink, the code's own
//   (field_mountain.rs's MountainView, a pinhole matched to the code's
//   projection), and the set, the volcano's caldera, where DEP.U has its
//   plateau.
// - Interpretation: reading the projection as 3D, a height unit as
//   MOUNTAIN_K map units.
// - Authored: the volcano's shape (field_mountain.rs's `volcano`, baked
//   into the relief), the join from the zoom, the lava, the lake's
//   convection, the rock, the lighting and the exposure. Nothing of the
//   lava's look or motion is from the original, which doesn't move its
//   terrain's colours.
//
// The method (authored). The relief is baked with maximum mipmaps: each
// level holds the highest height of its blocks. A ray skips any block it
// passes over, going up a level as it leaves one and down where it dips
// below a block's top, to blocks two texels a side, where it marches the
// bilinear surface and bisects the crossing. The prepass shades each
// pixel once, at the middle of the exposure; the layer marches the pixel's
// ray again for its depth, follows that point through the exposure with the
// camera at its opening and close, and averages the prepass along that
// motion.

// Map units a height unit (field_mountain.rs's MOUNTAIN_K).
const MOUNTAIN_K=0.29;
// The lake's surface and the rim's crest (field_mountain.rs's
// VOLCANO_LAKE and VOLCANO_RIM), and the potential at the flanks' foot
// (VOLCANO_FOOT).
const MOUNTAIN_LAKE=140.0;
const MOUNTAIN_RIM=150.0;
const MOUNTAIN_FOOT=.3;
// Gullies a turn of the external angle (VOLCANO_GULLIES).
const MOUNTAIN_GULLIES=160.0;
const MOUNTAIN_NONE=1e9;

// ---- The camera --------------------------------------------------------------------
// A pinhole (field_mountain.rs's MountainView): eye, view, up and right,
// focal lengths across and up (scene rows a unit of offset over depth), the
// scene point the view passes through, the frame's plane's depth along the
// view, how far the join has come, and the exposure (seconds).
struct MountainCam { eye:vec3f, fwd:vec3f, up:vec3f, right:vec3f, focal:vec2f, centre:vec2f, window:f32, join:f32, shutter:f32 }
fn mountain_cam_at(k:i32)->MountainCam {
    let a=fc.mountain_view[4*k];let b=fc.mountain_view[4*k+1];
    let c=fc.mountain_view[4*k+2];let d=fc.mountain_view[4*k+3];
    let f=normalize(b.xyz);
    let up=normalize(c.xyz-f*dot(c.xyz,f));
    return MountainCam(a.xyz,f,up,cross(f,up),vec2f(b.w,c.w),d.xy,d.z,d.w,a.w);
}
// The camera t of the way through the exposure: 0 its opening, 1 its close,
// the moment shown.
fn mountain_cam(t:f32)->MountainCam {
    let a=mountain_cam_at(0);let b=mountain_cam_at(1);
    let f=normalize(mix(a.fwd,b.fwd,t));
    let u0=mix(a.up,b.up,t);
    let up=normalize(u0-f*dot(u0,f));
    return MountainCam(mix(a.eye,b.eye,t),f,up,cross(f,up),mix(a.focal,b.focal,t),mix(a.centre,b.centre,t),
        mix(a.window,b.window,t),mix(a.join,b.join,t),b.shutter);
}
// The world ray through scene point s (rows, y up, centred).
fn mountain_ray(c:MountainCam,s:vec2f)->vec3f {
    return normalize(c.fwd+c.right*(s.x-c.centre.x)/c.focal.x+c.up*(s.y-c.centre.y)/c.focal.y);
}
// Where world point p shows: scene x and y, and its depth along the view.
fn mountain_project(c:MountainCam,p:vec3f)->vec3f {
    let d=p-c.eye;
    let z=max(dot(d,c.fwd),1e-4);
    return vec3f(c.centre+c.focal*vec2f(dot(d,c.right),dot(d,c.up))/z,z);
}
// The screen's uv for a scene point.
fn mountain_uv(s:vec2f)->vec2f {return vec2f(s.x/(200.0*screen_aspect())+.5,.5-s.y/200.0);}
fn mountain_world(m:vec2f,h:f32)->vec3f {return vec3f(m,h*MOUNTAIN_K);}

// ---- The march ---------------------------------------------------------------------
fn mountain_texel()->f32 {return MAP_SIZE/relief_size();}
// The highest height in block `cell` of mipmap `level` (1 and up).
fn mountain_block(cell:vec2i,level:i32)->f32 {
    let n=i32(relief_size())>>u32(level);
    return textureLoad(relief,clamp(cell,vec2i(0),vec2i(n-1)).yx,level).r;
}
// The relief's height at u, in texels from the first texel's centre.
fn mountain_height_u(u:vec2f)->f32 {return textureSampleLevel(relief,linear,(u.yx+.5)/relief_size(),0.0).r;}
// What a ray meets: whether it hits, at what distance, how much of the
// pixel the mountain covers (antialiased by how close a miss came, in
// pixels) and, for a miss, the distance of its closest approach.
struct MountainHit { hit:bool, t:f32, cover:f32, near:f32 }
// The world ray from o along unit d; `foot` is a pixel's size in world
// units at unit distance.
fn mountain_trace(o:vec3f,d:vec3f,foot:f32)->MountainHit {
    var out=MountainHit(false,MOUNTAIN_NONE,0.0,MOUNTAIN_NONE);
    let texel=mountain_texel();
    let n=relief_size();
    // In texels and height units.
    let O=vec3f(o.xy/texel-.5,o.z/MOUNTAIN_K);
    let D=vec3f(d.xy/texel,d.z/MOUNTAIN_K);
    let top_level=i32(textureNumLevels(relief))-1;
    let top=mountain_block(vec2i(0),top_level);
    var t0=0.0;var t1=1e7;
    for(var a=0;a<2;a++) {
        if abs(D[a])>1e-9 {
            let p=-O[a]/D[a];let q=(n-1.0-O[a])/D[a];
            t0=max(t0,min(p,q));t1=min(t1,max(p,q));
        } else if O[a]<0.0 || O[a]>n-1.0 {return out;}
    }
    if D.z<-1e-9 {
        t1=min(t1,O.z/(-D.z));
        if O.z>top {t0=max(t0,(O.z-top)/(-D.z));}
    } else if O.z>top {return out;}
    if t0>=t1 {return out;}
    var t=t0;
    var level=top_level;
    var best=1e9;
    for(var i=0;i<224;i++) {
        if t>=t1 {break;}
        let p=O+D*t;
        let s=f32(1<<u32(level));
        let cell=vec2i(floor(p.xy/s));
        let lo=vec2f(cell)*s;
        var te=t1;
        if abs(D.x)>1e-9 {te=min(te,(select(lo.x,lo.x+s,D.x>0.0)-O.x)/D.x);}
        if abs(D.y)>1e-9 {te=min(te,(select(lo.y,lo.y+s,D.y>0.0)-O.y)/D.y);}
        te=max(te,t);
        let hm=mountain_block(cell,level);
        let ha=O.z+D.z*t;let hb=O.z+D.z*te;
        if min(ha,hb)>hm {
            if level<=2 {
                let tm=select(t,te,hb<ha);
                if (min(ha,hb)-hm)*MOUNTAIN_K/(foot*max(tm,1e-3))<best {
                    best=(min(ha,hb)-hm)*MOUNTAIN_K/(foot*max(tm,1e-3));
                    out.near=tm;
                }
            }
            t=te+1e-4*(1.0+te);
            level=min(level+1,top_level);
            continue;
        }
        var ts=t;
        if D.z<0.0 && ha>hm {ts=(O.z-hm)/(-D.z);}
        if level>1 {t=ts;level-=1;continue;}
        // A block two texels a side: march its bilinear surface.
        var ta=ts;
        var found=false;
        for(var k=1;k<=4;k++) {
            let tb=mix(ts,te,f32(k)*.25);
            let gb=O.z+D.z*tb-mountain_height_u((O+D*tb).xy);
            let margin=gb*MOUNTAIN_K/(foot*max(tb,1e-3));
            if margin<best {best=margin;out.near=tb;}
            if gb<=0.0 {
                var a=ta;var b=tb;
                for(var j=0;j<6;j++) {
                    let tm=.5*(a+b);
                    if O.z+D.z*tm-mountain_height_u((O+D*tm).xy)<=0.0 {b=tm;} else {a=tm;}
                }
                out.hit=true;out.t=b;found=true;
                break;
            }
            ta=tb;
        }
        if found {break;}
        t=te+1e-4*(1.0+te);
        level=min(level+1,top_level);
    }
    out.cover=select(clamp(.5-best,0.0,1.0),1.0,out.hit);
    return out;
}

// ---- Noise (authored) ----------------------------------------------------------------
fn mountain_hashu(p:vec2i,seed:u32)->u32 {
    var h=u32(p.x)*0x8da6b343u^u32(p.y)*0xd8163841u^seed*0xcb1ab31fu;
    h^=h>>16u;h*=0x7feb352du;h^=h>>15u;h*=0x846ca68bu;h^=h>>16u;
    return h;
}
fn mountain_hash(p:vec2i,seed:u32)->f32 {return f32(mountain_hashu(p,seed)&0xffffffu)/16777216.0;}
fn mountain_hash2(p:vec2i,seed:u32)->vec2f {
    let h=mountain_hashu(p,seed);
    return vec2f(f32(h&0xffffu),f32(h>>16u))/65536.0;
}
// Gradient noise, about -1 to 1.
fn mountain_gnoise(p:vec2f,seed:u32)->f32 {
    let i=vec2i(floor(p));let f=p-floor(p);
    let u=f*f*f*(f*(f*6.0-15.0)+10.0);
    let a=dot(mountain_hash2(i,seed)*2.0-1.0,f);
    let b=dot(mountain_hash2(i+vec2i(1,0),seed)*2.0-1.0,f-vec2f(1,0));
    let c=dot(mountain_hash2(i+vec2i(0,1),seed)*2.0-1.0,f-vec2f(0,1));
    let d=dot(mountain_hash2(i+vec2i(1,1),seed)*2.0-1.0,f-vec2f(1,1));
    return 1.9*mix(mix(a,b,u.x),mix(c,d,u.x),u.y);
}
// Octaves of it, 0 to 1 about .5, fading those finer than `fine` cells.
fn mountain_fbm(p:vec2f,seed:u32,octaves:i32,fine:f32)->f32 {
    var sum=0.0;var amp=.5;var q=p;var norm=0.0;var size=1.0;
    for(var o=0;o<octaves;o++) {
        let keep=1.0-smoothstep(.25,.6,fine/size);
        sum+=amp*keep*mountain_gnoise(q,seed+u32(o)*7u);
        norm+=amp;amp*=.5;q=q*2.0+vec2f(0.0,17.3);size*=.5;
    }
    return .5+.5*sum/norm;
}
// Plates of crust: the Voronoi cells of a jittered grid. The distance from
// p to the edge between the nearest two centres, in cells, the nearest
// cell's id and centre.
struct MountainPlate { edge:f32, id:f32, centre:vec2f }
fn mountain_plates(p:vec2f,seed:u32)->MountainPlate {
    let i=vec2i(floor(p));
    var d1=9.0;var d2=9.0;var c1=vec2f(0);var c2=vec2f(1);var id=0.0;
    for(var j=-1;j<=1;j++) {
        for(var k=-1;k<=1;k++) {
            let o=i+vec2i(k,j);
            let hu=mountain_hashu(o,seed);
            let cpt=vec2f(o)+.1+.8*vec2f(f32(hu&0xffffu),f32(hu>>16u))/65536.0;
            let e=p-cpt;
            let d=dot(e,e);
            if d<d1 {d2=d1;c2=c1;d1=d;c1=cpt;id=f32((hu*0x9e3779b9u)>>8u)/16777216.0;}
            else if d<d2 {d2=d;c2=cpt;}
        }
    }
    let n=normalize(c2-c1+vec2f(1e-6,0.0));
    return MountainPlate(max(dot(.5*(c1+c2)-p,n),0.0),id,c1);
}
// How much of a footprint f wide, centred d from a line, a band w either
// side of it covers.
fn mountain_band(d:f32,w:f32,f:f32)->f32 {
    let r=.5*max(f,1e-4);
    return clamp((min(d+r,w)-max(d-r,-w))/(2.0*r),0.0,1.0);
}

// ---- The set, per pixel --------------------------------------------------------------
// The set at map point m, exactly: inside, or outside with the potential G,
// its rate per map unit and the external angle (turns), by the product
// formula over the orbit as field_mountain.rs's `orbit` (principal
// arguments, the first 16 terms).
struct MountainSet { inside:bool, g:f32, rate:f32, angle:f32 }
fn mountain_atan2(y:f32,x:f32)->f32 {
    let ax=abs(x);let ay=abs(y);
    let a=min(ax,ay)/max(max(ax,ay),1e-30);
    let s=a*a;
    var r=((-.0464964749*s+.15931422)*s-.327622764)*s*a+a;
    if ay>ax {r=1.57079637-r;}
    if x<0.0 {r=3.14159274-r;}
    return select(r,-r,y<0.0);
}
fn mountain_set(m:vec2f)->MountainSet {
    var e=MountainSet(true,0.0,0.0,0.0);
    let c=mandel_c(m);
    let q=(c.x-.25)*(c.x-.25)+c.y*c.y;
    if q*(q+c.x-.25)<=.25*c.y*c.y || (c.x+1.0)*(c.x+1.0)+c.y*c.y<=.0625 {return e;}
    var z=c;
    var w=vec2f(1,0);
    var power=2.0;
    var angle=atan2(c.y,c.x);
    var weight=.5;
    var kept=vec2f(1e9);
    for(var n=1;n<=SET_ITERATIONS+24;n++) {
        let m2=dot(z,z);
        if m2>BAILOUT {
            let wz=vec2f(w.x*z.x+w.y*z.y,w.y*z.x-w.x*z.y)/(m2*power);
            let s=mandel_scale();
            e.inside=false;
            e.g=.5*log(m2)/power;
            e.rate=length(vec2f(-wz.y/s,wz.x/(s*mandel_aspect())));
            e.angle=fract(angle/(2.0*PI));
            return e;
        }
        if n>SET_ITERATIONS && m2<4.0 {return e;}
        let z2=vec2f(z.x*z.x-z.y*z.y,2.0*z.x*z.y);
        if n<=16 {
            let t=vec2f(1.0,0.0)+vec2f(c.x*z2.x+c.y*z2.y,c.y*z2.x-c.x*z2.y)/max(dot(z2,z2),1e-30);
            angle+=weight*mountain_atan2(t.y,t.x);
            weight*=.5;
        }
        w=2.0*vec2f(z.x*w.x-z.y*w.y,z.x*w.y+z.y*w.x)+vec2f(1,0);
        z=z2+c;
        power*=2.0;
        if m2<4.0 {
            let back=z-kept;
            if dot(back,back)<1e-10 {return e;}
            if (n&(n-1))==0 {kept=z;}
        }
    }
    return e;
}
// The gully a flank point lies in (field_mountain.rs's `gully`): its
// distance across from the floor in gullies, and its number.
fn mountain_gully(angle:f32,x:f32)->vec2f {
    let a=2.0*PI*angle;
    let wander=.3*sin(9.0*a+7.0*x+1.3)+.2*sin(3.0*a-11.0*x+.4);
    let u=MOUNTAIN_GULLIES*angle+wander;
    let k=round(u);
    return vec2f(u-k,k);
}
// field_mountain.rs's gully_hash.
fn mountain_gully_hash(k:f32,seed:u32)->f32 {
    let n=i32(MOUNTAIN_GULLIES);
    var h=u32(((i32(k)%n)+n)%n)*0x9E3779B1u^seed*0x85EBCA77u;
    h^=h>>15u;h*=0x2C1B3C6Du;h^=h>>12u;h*=0x297A2D39u;h^=h>>15u;
    return f32(h>>8u)/16777216.0;
}

// ---- Light ----------------------------------------------------------------------------
// The lava's light at temperature tc, in degrees C, as linear HDR radiance
// (authored, after a blackbody's): deep red about 700, orange about 950 and
// yellow-white about 1150, where it is 12, for the shared bloom; its
// brightness by Wien's approximation with B 14,080 K, about 0.6 of the
// physical value at 600 nm, so that dull red to white fits the tonemap.
fn mountain_glow(tc:f32)->vec3f {
    let k=max(tc,0.0)+273.15;
    let r=12.0*exp(-14080.0*(1.0/k-1.0/1423.15));
    let x=clamp((tc-600.0)/550.0,0.0,1.3);
    let y=clamp((tc-900.0)/250.0,0.0,1.3);
    return r*vec3f(1.0,.35*x*sqrt(x),.04*y*y);
}
// When the lake's lava spills over the rim, in part-5 seconds: as the join
// brings the camera round (authored); the flows' fronts run down from it.
fn mountain_spill_time()->f32 {return time_terrain()+.9;}
// The cool key, from above and behind the camera's left, dim (authored):
// the lava and the glow of the plume over the crater are the main light.
fn mountain_key_dir(c:MountainCam)->vec3f {
    let f=normalize(vec3f(c.fwd.xy,0.0)+vec3f(1e-4,0,0));
    let r=normalize(vec3f(c.right.xy,0.0)+vec3f(0,1e-4,0));
    return normalize(-.5*r-.45*f+vec3f(0,0,.8));
}
const MOUNTAIN_KEY=vec3f(.35,.40,.52);
// The plume over the crater, lit from below by the lake: an orange sky
// over the volcano, strongest overhead (authored).
const MOUNTAIN_SKY=vec3f(1.1,.36,.10);
// GGX specular for a light (Smith-Schlick visibility), times n.l.
fn mountain_ggx(n:vec3f,v:vec3f,l:vec3f,rough:f32)->f32 {
    let h=normalize(v+l);
    let nh=max(dot(n,h),0.0);let nl=max(dot(n,l),0.0);let nv=max(dot(n,v),1e-3);
    let a=max(rough*rough,1e-3);let a2=a*a;
    let d=a2/(PI*pow(nh*nh*(a2-1.0)+1.0,2.0));
    let k=.5*a;
    let g=nl/(nl*(1.0-k)+k)*nv/(nv*(1.0-k)+k);
    return d*g/(4.0*nv);
}
// The relief's horizon over m in six directions, to darken hollows.
fn mountain_occlusion(m:vec2f,h:f32,n:vec3f)->f32 {
    var occ=0.0;
    for(var k=0;k<6;k++) {
        let a=f32(k)*1.0472+.39;
        let d=vec2f(cos(a),sin(a));
        let r=select(1.2,3.6,(k&1)==1);
        let hs=relief_height(m+d*r);
        let tangent=-dot(n.xy,d)/max(n.z,.25);
        occ+=clamp((hs-h)*MOUNTAIN_K/r-tangent,0.0,1.2);
    }
    return clamp(1.0-occ/4.5,0.0,1.0);
}

// ---- The lake ---------------------------------------------------------------------------
// A convecting lava lake in the caldera (authored). Hot lava wells up along
// a wavering seam down the set's real axis and spreads away on either side
// at MOUNTAIN_LAKE_SPEED map units a second, so its plates of crust are as
// old as their distance from the seam: thin and glowing, with wide cracks,
// by the seam; dark and thick towards the rim, where they founder in a
// bright margin. Cracks pulse open and shut. As the volcano rises out of
// the zoom the lake is molten, from black, and crusts over after.
const MOUNTAIN_LAKE_SPEED=4.0;
const MOUNTAIN_LAKE_PLATE=6.5;
fn mountain_lake(c:MountainCam,m:vec2f,v:vec3f,fp:f32)->vec3f {
    let t=field_seconds();
    let ignite=smoothstep(.02,.35,c.join);
    let crusting=smoothstep(time_terrain()+.8,time_terrain()+1.8,t);
    let along=m.y;
    let seam=100.0-mandel_centre().y*mandel_scale()+3.5*sin(.16*along+.13*t)+2.0*sin(.37*along-.21*t+1.3);
    let y=m.x-seam;
    let side=select(0u,1u,y<0.0);
    let dist=abs(y);
    let age=dist/MOUNTAIN_LAKE_SPEED;
    let qp=vec2f(along,dist-MOUNTAIN_LAKE_SPEED*t)/MOUNTAIN_LAKE_PLATE;
    // Plates of many sizes: cells, some split by a finer crack.
    let warp=vec2f(mountain_gnoise(qp*.6,57u),mountain_gnoise(qp*.6+vec2f(5.2,1.3),58u))*.35;
    let pl=mountain_plates(qp+warp,51u+side);
    let fq=fp/MOUNTAIN_LAKE_PLATE/max(abs(v.z),.2);
    let pulse=smoothstep(.6,1.0,sin(6.2832*(.21*t+3.0*pl.id)));
    let cw=.008+.06*exp(-age/.6)+.02*pulse;
    let crack=mix(mountain_band(pl.edge,cw,fq),min(4.0*cw,1.0),smoothstep(.35,1.1,fq));
    let seam_w=.55*(.7+.6*mountain_gnoise(vec2f(along*.4,.4*t),52u));
    let churn=mountain_gnoise(m*.35+vec2f(.25*t,-.18*t),54u);
    let open_seam=mountain_band(dist,seam_w,fp/max(abs(v.z),.2));
    // The margin, where the plates founder against the rim: within about
    // two map units of it, by the relief's maximum mipmap four levels up
    // (blocks 1.6 map units a side), which holds the rim's height there.
    let rim=textureSampleLevel(relief,linear,m.yx/MAP_SIZE,4.0).r;
    let edge=smoothstep(MOUNTAIN_LAKE+2.0,MOUNTAIN_RIM-1.0,rim)*(.6+.4*churn);
    let t_plate=mix(1010.0+40.0*churn,mix(1000.0,470.0,smoothstep(.1,1.8,age))-60.0*pl.id,crusting);
    let t_crack=1130.0-150.0*smoothstep(.5,6.0,age)-60.0*pl.id;
    var e=mix(mountain_glow(t_plate),mountain_glow(t_crack),crack);
    // Finer cracks splitting the plates, duller.
    let small=mountain_plates(qp*2.3+warp*2.0+vec2f(3.1,7.7),53u);
    let fs=2.3*fq;
    let fine=mix(mountain_band(small.edge,.03,fs),.12,smoothstep(.35,1.1,fs))*step(.45,small.id)*crusting*(1.0-crack);
    e+=mountain_glow(840.0-120.0*smoothstep(1.0,5.0,age))*fine*.6;
    e=mix(e,mountain_glow(1160.0+20.0*churn),open_seam);
    e=mix(e,mountain_glow(1120.0+30.0*churn),edge);
    // The plates' glossy skin catches the key.
    let skin=crusting*(1.0-crack)*(1.0-open_seam)*(1.0-edge);
    let dome=(qp-pl.centre)*.35*skin;
    let nb=normalize(vec3f(-dome.yx,1.0));
    let fresnel=.04+.96*pow(1.0-max(dot(nb,v),0.0),5.0);
    e+=MOUNTAIN_KEY*8.0*mountain_ggx(nb,v,mountain_key_dir(c),.3)*fresnel*skin;
    return e*ignite;
}

// ---- The flanks ----------------------------------------------------------------------------
// Dark basalt, lit dimly by the key and the plume's glow, with the lake's
// light on the crater's inner wall, and lava in some of the gullies: it
// spills over the rim at mountain_spill_time and its fronts run down, open
// and bright near the rim, cooling as it travels and crusting over lower
// down, its surface streaming down the gully (authored).
const MOUNTAIN_FLOWING=.1;
fn mountain_flank(c:MountainCam,m:vec2f,h:f32,n:vec3f,v:vec3f,fp:f32,e:MountainSet,wall:f32)->vec3f {
    let t=field_seconds();
    let x=clamp(e.g/MOUNTAIN_FOOT,0.0,1.0);
    // Rock: dark basalt, its tone and roughness varying.
    let grain=mountain_fbm(m*.9,71u,4,fp*.9);
    let ash=smoothstep(.55,.75,mountain_fbm(m*.12+vec2f(3.0,7.0),75u,3,fp*.12));
    let albedo=mix(mix(vec3f(.030,.029,.029),vec3f(.070,.066,.062),grain),vec3f(.13,.12,.11),.5*ash);
    // Rough rock: the normal bent by two octaves of noise, a map unit and
    // a third of one across, faded where finer than the pixel.
    var nb=n;
    for(var o=0;o<2;o++) {
        let size=select(1.1,.35,o==1);
        let keep=1.0-smoothstep(.3,.8,fp/size);
        let q=m/size;
        let e0=mountain_gnoise(q,81u+u32(o));
        let gx=mountain_gnoise(q+vec2f(.1,0.0),81u+u32(o))-e0;
        let gy=mountain_gnoise(q+vec2f(0.0,.1),81u+u32(o))-e0;
        nb=nb-vec3f(gx,gy,0.0)*(.55*keep);
    }
    let nr=normalize(nb);
    let ao=mountain_occlusion(m,h,n);
    let l=mountain_key_dir(c);
    var light=MOUNTAIN_KEY*max(dot(nr,l),0.0);
    // The plume's glow, from above the lake: brighter on the upper flanks
    // and on faces turned up towards it.
    let above=vec3f(100.0,100.0,MOUNTAIN_RIM*MOUNTAIN_K+40.0)-mountain_world(m,h);
    let dl=normalize(above);
    let fall=1.0/(1.0+dot(above,above)/(70.0*70.0));
    light+=MOUNTAIN_SKY*fall*(.2+.8*max(dot(nr,dl),0.0))*smoothstep(.1,.6,c.join);
    light+=vec3f(.02,.022,.03);
    // The lake lights the crater's inner wall, facing it across the rim.
    let inner=exp(-wall/1.4)*smoothstep(.02,.35,c.join);
    light+=mountain_glow(1080.0)*.18*inner;
    var col=albedo*light*ao;
    // Lava in the gullies that carry it.
    let gl=mountain_gully(e.angle,x);
    let across=abs(gl.x);
    let k=gl.y;
    let flowing=step(mountain_gully_hash(k,3u),MOUNTAIN_FLOWING);
    let reach=.35+.6*mountain_gully_hash(k,4u);
    let width=.08+.1*mountain_gully_hash(k,5u);
    // The gully's footprint across, in gullies: its spacing on the ground.
    let per_unit=MOUNTAIN_GULLIES*e.rate/(2.0*PI);
    let fa=fp*per_unit/max(length(n.xy)+abs(v.z),.3);
    let since=t-mountain_spill_time()-.3*mountain_gully_hash(k,6u);
    let front=clamp(since*(.35+.25*mountain_gully_hash(k,7u)),0.0,reach);
    let run=step(x,front)*flowing;
    if run>0.0 && wall>.6 {
        // The flow swells and narrows down the gully, and meanders in it.
        let swell=mountain_gnoise(vec2f(x*9.0,k*1.7),73u);
        let w=width*(1.0-.4*x/reach)*(.7+.45*swell);
        let off=.25*mountain_gnoise(vec2f(x*4.0,k*2.3),74u)+.08*mountain_gnoise(vec2f(x*13.0,k*3.1),76u);
        let a=abs(gl.x-off);
        // A rounded front.
        let tip=sqrt(max(1.0-pow(clamp((x-front+.03)/.03,0.0,1.0),2.0),0.0));
        let lava=mix(mountain_band(a,w*tip,fa),2.0*w*tip,smoothstep(.4,1.2,fa))*run;
        // Cooling down the flow; its skin crusts into rafts drawn out along
        // it, which part over the hot core and close at the edges.
        let cool=x/reach;
        let speed=.35+.5*(1.0-x);
        let across=a/max(w,1e-3);
        let rafts=mountain_fbm(vec2f(across*2.2+k,x*9.0-t*speed),72u,3,fp*9.0*e.rate/MOUNTAIN_FOOT);
        let core=1.0-smoothstep(.0,.6,across);
        let crust=clamp(smoothstep(.0,.55,cool)*.8+.25+.9*(rafts-.5)-.75*core*(1.0-.6*cool),0.0,1.0);
        let skin=smoothstep(.45,.75,crust);
        let tc=mix(1080.0,880.0,cool)-140.0*across;
        col=mix(col,mix(mountain_glow(tc),albedo*light*ao*.8+mountain_glow(tc-380.0),skin),lava);
        // Its light on the rock beside it.
        let near=exp(-max(a-w,0.0)/.1)*run*(1.0-lava);
        col+=albedo*mountain_glow(mix(1000.0,820.0,cool))*.3*near;
    }
    return col;
}

// ---- Fountains ----------------------------------------------------------------------------
// Lava thrown up from the lake on the tune (authored): on each accent
// (fc.mountain_style[2], from field_mountain.rs's `hits`) a few fountains
// leap from vents along the lake's seam, on each beat ([3]) a spatter. Each
// blob flies a ballistic arc and cools as it goes, drawn as the streak it
// makes over the exposure, hidden by the mountain where it is behind it.
const MOUNTAIN_FOUNTAIN_G=70.0;
const MOUNTAIN_FOUNTAIN_HIGH=34.0;
const MOUNTAIN_FOUNTAINS=3;
const MOUNTAIN_BLOBS=26;
const MOUNTAIN_SPATTER=10;
fn mountain_hash1(k:u32,seed:u32)->f32 {return mountain_hash(vec2i(i32(k),i32(seed)),977u);}
// A vent on the lake: a point along the seam through the cardioid and the
// period-2 bulb.
fn mountain_vent(k:u32)->vec2f {
    let re=mix(-1.1,.12,mountain_hash1(k,1u));
    let im=.3*(mountain_hash1(k,2u)-.5)*(1.0-smoothstep(-.8,-1.1,re)*.7);
    let s=mandel_scale();
    return vec2f(100.0+(im-mandel_centre().y)*s,100.0+(re-mandel_centre().x)*s*mandel_aspect());
}
// Blob b of a burst seeded k, `age` seconds after it, its peak `high` map
// units: where it is, and its temperature, or w < 0 once it has fallen back.
fn mountain_blob(k:u32,b:u32,age:f32,high:f32,vent:vec2f)->vec4f {
    let h=high*mix(.35,1.0,mountain_hash1(k*61u+b,3u));
    let up=sqrt(2.0*MOUNTAIN_FOUNTAIN_G*h);
    let a=2.0*PI*mountain_hash1(k*61u+b,4u);
    let out=up*mix(.03,.22,mountain_hash1(k*61u+b,5u));
    let flight=2.0*up/MOUNTAIN_FOUNTAIN_G;
    let t=age-.08*mountain_hash1(k*61u+b,6u);
    if t<0.0 || t>flight {return vec4f(0,0,0,-1);}
    let p=vec3f(vent+out*t*vec2f(cos(a),sin(a)),MOUNTAIN_LAKE*MOUNTAIN_K+up*t-.5*MOUNTAIN_FOUNTAIN_G*t*t);
    return vec4f(p,1150.0-280.0*t/flight);
}
// The fountains' light at scene point s over the exposure: premultiplied
// light and coverage, behind the mountain where its ray hits it at `depth`.
fn mountain_fountains(s:vec2f,pixel:f32,depth:f32)->vec4f {
    var out=vec4f(0);
    let open=mountain_cam(0.0);let close=mountain_cam(1.0);
    let shutter=close.shutter;
    for(var burst=0;burst<4;burst++) {
        let accent=burst<2;
        let i=burst%2;
        let style=select(fc.mountain_style[3],fc.mountain_style[2],accent);
        let age=style[i];
        if age<0.0 {continue;}
        let k=u32(style[2+i]);
        let vents=select(1,MOUNTAIN_FOUNTAINS,accent);
        let blobs=select(MOUNTAIN_SPATTER,MOUNTAIN_BLOBS,accent);
        let high=select(.3,1.0,accent)*MOUNTAIN_FOUNTAIN_HIGH;
        for(var f=0;f<vents;f++) {
            let vent=mountain_vent(k*7u+u32(f));
            // Skip the vent where it is far from this pixel on the screen.
            let at=mountain_project(close,vec3f(vent,MOUNTAIN_LAKE*MOUNTAIN_K+.5*high));
            let reach=close.focal.y*high/max(at.z,1.0);
            if length(s-at.xy)>1.6*reach+4.0 {continue;}
            for(var b=0;b<blobs;b++) {
                let id=u32(f)*97u+u32(b);
                let p1=mountain_blob(k,id,age,high,vent);
                if p1.w<0.0 {continue;}
                let p0=mountain_blob(k,id,age-shutter,high,vent);
                let a=mountain_project(close,p1.xyz);
                let q=select(p1,p0,p0.w>0.0);
                let c=mountain_project(open,q.xyz);
                if a.z<1.0 || a.z>depth {continue;}
                let r=max(close.focal.y*mix(.35,1.0,pow(mountain_hash1(k*61u+id,7u),2.0))/a.z,.6*pixel);
                // Distance from the streak between where it was and is.
                let seg=a.xy-c.xy;
                let l2=max(dot(seg,seg),1e-6);
                let h=clamp(dot(s-c.xy,seg)/l2,0.0,1.0);
                let d=length(s-c.xy-seg*h);
                let cover=clamp((r-d)/pixel+.5,0.0,1.0)*r/(r+.5*sqrt(l2));
                if cover>0.0 {
                    out=vec4f(mountain_glow(p1.w)*cover+out.rgb*(1.0-cover),cover+out.a*(1.0-cover));
                }
            }
        }
    }
    return out*smoothstep(.5,.9,mountain_cam(1.0).join);
}

// ---- The layer -------------------------------------------------------------------------
// The mountain at scene point s with camera c, through a pixel `pixel` rows
// across: premultiplied light and coverage.
fn mountain_shade_at(c:MountainCam,s:vec2f,pixel:f32)->vec4f {
    let d=mountain_ray(c,s);
    let foot=pixel/c.focal.y;
    let hit=mountain_trace(c.eye,d,foot);
    if hit.cover<=0.0 {return vec4f(0);}
    let t=select(hit.near,hit.t,hit.hit);
    let p=c.eye+d*t;
    let texel=mountain_texel();
    let m=p.xy;
    let r=relief_at(m);
    let n=normalize(vec3f(-MOUNTAIN_K*r.yz,1.0));
    let v=-d;
    let fp=foot*t;
    var col:vec3f;
    // The lake where the set is, exactly; the rock and lava elsewhere.
    let e=mountain_set(m);
    if e.inside && r.x<MOUNTAIN_LAKE+2.0 {
        col=mountain_lake(c,m,v,fp);
    } else {
        col=mountain_flank(c,m,r.x,n,v,fp,e,select(r.w,64.0,e.inside));
    }
    // The volcano comes in over the zoom's minibrot as the join begins: the
    // lake at once, the rock round it more slowly, over the lace.
    let lake=e.inside && r.x<MOUNTAIN_LAKE+2.0;
    let a=hit.cover*select(smoothstep(.03,.3,c.join),smoothstep(0.0,.06,c.join),lake);
    return vec4f(col*a,a);
}
fn mountain_prepass(px:FieldPixel)->vec4f {
    if field_phase()<.5 {return vec4f(0);}
    return mountain_shade_at(mountain_cam(.5),px.scene,px.pixel);
}
fn mountain_layer(px:FieldPixel)->FieldLayer {
    if field_phase()<.5 {return no_layer();}
    let mid=mountain_cam(.5);
    let d=mountain_ray(mid,px.scene);
    let foot=px.pixel/mid.focal.y;
    let hit=mountain_trace(mid.eye,d,foot);
    // The point whose motion the pixel follows: what its ray meets, or comes
    // nearest, or the lake's distance.
    var t=select(hit.near,hit.t,hit.hit);
    if t>=MOUNTAIN_NONE {t=length(mid.eye-vec3f(100.0,100.0,MOUNTAIN_LAKE*MOUNTAIN_K));}
    let p=mid.eye+d*t;
    let a=mountain_project(mountain_cam(0.0),p).xy;
    let b=mountain_project(mountain_cam(1.0),p).xy;
    let motion=b-a;
    let pixels=length(motion)/px.pixel;
    // Taps along the motion, no more than two pixels apart where they can
    // be, at offsets jittered per pixel and per sixtieth of a second, so that
    // a long streak breaks into fine grain rather than copies.
    let count=i32(clamp(ceil(pixels/2.0),1.0,32.0));
    let jitter=select(.5,mountain_hash(vec2i(floor(px.uv*u.screen.xy)),91u+u32(max(field_seconds(),0.0)*60.0)),count>1);
    var sum=vec4f(0);
    for(var k=0;k<count;k++) {
        let f=(f32(k)+jitter)/f32(count)-.5;
        sum+=mountain_prepass_at(mountain_uv(px.scene+motion*f));
    }
    sum/=f32(count);
    // The lava thrown up over the lake.
    let behind=select(MOUNTAIN_NONE,dot(d*hit.t,mid.fwd),hit.hit);
    let lava=mountain_fountains(px.scene,px.pixel,behind);
    sum=vec4f(lava.rgb+sum.rgb*(1.0-lava.a),lava.a+sum.a*(1.0-lava.a));
    if sum.a<=0.0 {return no_layer();}
    // Beyond the frame's plane the mountain is behind the frame, seen through
    // the window; nearer, in front.
    let depth=dot(p-mid.eye,mid.fwd);
    let z=select(LAYER_FRAME_Z-30.0,LAYER_MOUNTAIN_Z,depth<mid.window || (!hit.hit && mid.join>.99));
    return FieldLayer(sum.rgb,sum.a,z);
}
