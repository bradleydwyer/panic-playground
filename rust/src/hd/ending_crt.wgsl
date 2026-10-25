// The modern part 8's pictures: what each tube of the bank shows, as a
// function of the point on its face (ending_set.wgsl builds the bank, the
// camera and the light around them). Authored throughout, on the port's
// timing, pictures and palettes (ending_common.wgsl, hd/ending.rs).
//
// The original holds part 7's monster until the music reaches 35:00, then
// turns colour 0 white behind it, shows a white frame, squeezes the picture
// toward row 100 over 17 frames while lifting every colour toward white,
// keeps row 100 alone as a line, erases it from both ends to one pixel,
// pulses that pixel's colour twice and stops. It reads as a monitor
// switching off; here each tube of the bank does:
// - The hold. As the camera draws back the picture becomes an old tube's:
//   the raster bowed with the glass, the original's 200 lines as scanlines
//   whose spot widens on bright lines (fading out once a line is smaller
//   than about two of the frame's pixels), a little halation and
//   convergence error toward the sides, a vignette, and the face a shade
//   above black.
// - The switch-off follows the original's stages and timing. The white
//   behind the monster and the white frame flash the face, and their light
//   lingers on the phosphor. The picture squeezes with the original's
//   compression, its colours lifted as the original lifts them and its light
//   concentrated as the beam's lines crowd together, so it brightens and
//   the band blooms. The line keeps the original's row and its ends as they
//   are erased, brightening as it shortens. The dot is a glowing point of
//   phosphor following the original's pulse, with a slower afterglow that
//   gathers while it pulses and decays to black at the end.
// - The phosphor's persistence: what the beam lit a few frames before still
//   glows, fading, so the collapse leaves a short trail. Everything is a
//   function of the moment (hd/ending.rs works out the past states).
// Nothing flickers: every value moves smoothly between the original's frames.
// Module: the shared controls, ending_common.wgsl, this file and
// ending_set.wgsl.

// ---- The look (authored) -------------------------------------------------------
// The raster: how far it bows with the glass (each axis pulled in by the
// other's square), and its corners' radius (rows).
const WARP=.08;
const CORNER=11.0;
// The scanlines: how far the beam's line shape replaces the smooth picture,
// fading out between these sizes of a frame's pixel (rows), and the spot's
// spread (rows) on a dark and a bright line.
const SCANLINES=.6;
const SCANLINES_GONE=vec2f(.3,.5);
const SPOT_DARK=.3;
const SPOT_BRIGHT=.45;
// Halation (of the blurred picture), convergence error at the face's sides
// (source columns, red outward, blue inward), the vignette, and the face's
// own glow above black (display light).
const HALATION=.05;
const CONVERGENCE=.4;
const VIGNETTE=.22;
const GLASS=vec3f(.0016,.0019,.0021);
// As part 7 ends (greetings_monster.wgsl): the brightest the monster shows
// through the tonemap (its inverse, as part 7's). Light the scanlines put
// above it goes on into HDR.
const MONSTER_WHITE=.9;
const EXCESS=1.5;
// The white: the face's light on the white frame, and its afterglow.
const FLASH=5.0;
const FLASH_GLOW=1.2;
// The squeeze: the band's light rises as the compression to this power;
// the rows a screen row gathers are sampled this many times.
const BAND_POWER=.35;
const BAND_TAPS=6;
// The line: its spot (rows), its light, rising as it shortens by this
// power of the shortening, at most LINE_MOST; its halo's share and reach
// (rows).
const LINE_SPOT=.6;
const LINE_GAIN=5.0;
const LINE_POWER=.5;
const LINE_MOST=4.0;
const LINE_HALO=.035;
const LINE_HALO_REACH=8.0;
// The dot: the phosphor's colour, the spot's spread and light, its halo's
// light and reach (rows); the afterglow's spread, light and halo, a little
// greener (a white phosphor's longest-lived part).
const DOT_WHITE=vec3f(1.0,1.0,1.0);
const DOT_SPOT=1.1;
const DOT_CORE=30.0;
const DOT_HALO=.9;
const DOT_REACH=9.0;
const AFTER_TINT=vec3f(.85,1.0,.95);
const AFTER_SPOT=1.5;
const AFTER_CORE=6.0;
const AFTER_HALO=.25;
// The phosphor's persistence (70 Hz frames) and the trail's taps' ages
// (ending::PERSISTENCE and TRAIL_AGES); the trail's light, and how fast it
// falls away beyond the oldest tap (rows).
const PERSISTENCE=1.6;
const TRAIL_AGES=array<f32,5>(0.0,1.0,2.0,3.0,4.5);
const TRAIL=.12;
const TRAIL_FALLOFF=4.0;

// ---- Bindings --------------------------------------------------------------------
// Each frame's palette in linear light, 256 entries a frame.
@group(0) @binding(6) var<storage,read> colours:array<vec4f>;
// The monster on the hold's palette, linear, blurred (ending::halation).
@group(0) @binding(7) var halation:texture_2d<f32>;
// Each column's mean colour under the hold's palette (ending::profile).
@group(0) @binding(8) var<storage,read> profile:array<vec4f>;

// ---- Part 7's look --------------------------------------------------------------
// greetings_common.wgsl's inverse of the shared tonemap: part 7 shows the
// monster through it, and so does this view.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}

// ---- The picture on the tube ------------------------------------------------------
// Palette entry `index` between the shown frame and the next, linear.
fn colour(index:u32)->vec3f {
    let a=colours[frame_now()*256u+index].rgb;
    let b=colours[frame_next()*256u+index].rgb;
    return mix(a,b,frame_mix());
}
// Source row m at column x (pixel centres at i + .5), Catmull-Rom across;
// colour 0 above and below the picture.
fn row_colour(m:i32,x:f32)->vec3f {
    if m<0 || m>199 {return colour(0u);}
    let q=x-.5;
    let k=i32(floor(q));
    let w=catmull(q-floor(q));
    var c=vec3f(0);
    for(var i=0;i<4;i++) {
        c+=colour(textureLoad(monster,vec2i(clamp(k+i-1,0,319),m),0).r)*w[i];
    }
    return c;
}
// The three guns' colours at their own columns: convergence error.
fn row_rgb(m:i32,x:f32,shift:f32)->vec3f {
    return vec3f(row_colour(m,x+shift).r,row_colour(m,x).g,row_colour(m,x-shift).b);
}
// The hold: the four rows round the point, weighted as the smooth picture
// weights them or as the beam's lines light them, each line a spot spread
// as it is bright.
fn scanned(s:vec2f,shift:f32,amount:f32)->vec3f {
    let q=s-.5;
    let ky=i32(floor(q.y));
    let t=q.y-floor(q.y);
    let smoothed=catmull(t);
    var c=vec3f(0);
    for(var i=0;i<4;i++) {
        let r=row_rgb(ky+i-1,s.x,shift);
        let d=t-f32(i-1);
        let sigma=mix(SPOT_DARK,SPOT_BRIGHT,sqrt(clamp(max(r.r,max(r.g,r.b)),0.0,1.0)));
        let line=exp(-d*d/(2.0*sigma*sigma))/(sigma*2.5066283);
        c+=r*mix(smoothed[i],line,amount);
    }
    return max(c,vec3f(0));
}
// The squeeze: screen row y shows source rows about row 100 + (y - row 100)
// times the compression; its lines crowd within the beam's spot, so a
// screen row gathers the source rows the spot spans.
fn squeezed(s:vec2f,shift:f32,compression:f32)->vec3f {
    let centre=LINE_ROW+.5;
    let m=centre+(s.y-centre)*compression;
    let reach=max(SPOT_BRIGHT*compression,.5);
    var c=vec3f(0);
    var total=0.0;
    for(var i=0;i<BAND_TAPS;i++) {
        let o=((f32(i)+.5)/f32(BAND_TAPS)*2.0-1.0)*1.5*reach;
        let w=exp(-o*o/(2.0*reach*reach));
        c+=row_rgb(i32(floor(m+o)),s.x,shift)*w;
        total+=w;
    }
    return max(c/total,vec3f(0));
}
// The line's light at a point: its row's colours between its ends, in the
// beam's spot, brighter as it shortens.
fn line_gain(left:f32,right:f32)->f32 {
    let full=ec.ends.w-ec.ends.z+1.0;
    return LINE_GAIN*min(pow(full/max(right-left+1.0,1.0),LINE_POWER),LINE_MOST);
}
fn between(x:f32,left:f32,right:f32)->f32 {
    return smoothstep(left-.5,left+.5,x)*(1.0-smoothstep(right+.5,right+1.5,x));
}
// The phosphor's persistence at distance d along one way out from the
// beam: `reach` is how far the lit shape reached at each age (now, then
// each of the trail's taps), never less as it gets older, and `light` how
// bright it was there (0 where nothing was drawn). Inside what is lit now
// there is no trail; beyond it, the age at which the shape last reached d
// is found between the taps, and its light decays with that age.
fn persisted(d:f32,reach:array<f32,5>,light:array<f32,5>)->f32 {
    var r=reach;
    var l=light;
    var found=false;
    var last_reach=0.0;
    var last_glow=0.0;
    for(var i=0;i<5;i++) {
        if l[i]<=0.0 {
            if found {break;}
            continue;
        }
        let glow=l[i]*exp(-TRAIL_AGES[i]/PERSISTENCE);
        if d<=r[i] {
            if !found {return select(glow,0.0,i==0);}
            let f=clamp((d-last_reach)/max(r[i]-last_reach,1e-4),0.0,1.0);
            return mix(last_glow,glow,f);
        }
        found=true;
        last_reach=r[i];
        last_glow=glow;
    }
    if !found {return 0.0;}
    return last_glow*exp(-(d-last_reach)/TRAIL_FALLOFF);
}

// The light a tube shows at `plain`, a point on its face in units of the
// picture's half size (so the 4:3 picture spans -1 to 1 on each axis), with
// the CRT's presence `a` and a frame's pixel `pixel` rows across there:
// HDR, before the fade to black.
fn screen(plain:vec2f,a:f32,pixel:f32)->vec3f {
    let half=vec2f(160.0*COLUMN,100.0);
    let n=plain*(1.0+WARP*a*plain.yx*plain.yx);
    let p=n*half;
    let radius=CORNER*a;
    let box=abs(p)-half+radius;
    let edge=length(max(box,vec2f(0)))+min(max(box.x,box.y),0.0)-radius;
    let face=1.0-smoothstep(-.5*pixel,.5*pixel,edge);
    let s=vec2f(160.0+p.x/COLUMN,100.0-p.y);
    let shift=CONVERGENCE*a*n.x;
    let compression=ec.beam.x;
    let centre=LINE_ROW+.5;
    var light=vec3f(0);
    if ec.beam.y>0.0 {
        var c=vec3f(0);
        var gain=1.0;
        if compression<=1.0 {
            let fine=1.0-smoothstep(SCANLINES_GONE.x,SCANLINES_GONE.y,pixel);
            c=scanned(s,shift,SCANLINES*a*fine);
            c+=HALATION*a*textureSampleLevel(halation,linear,s/vec2f(320,200),0.0).rgb;
            c+=GLASS*a;
        } else {
            c=squeezed(s,shift,compression);
            gain=pow(compression,BAND_POWER);
        }
        light=tonemap_inverse(min(c,vec3f(MONSTER_WHITE)))+max(c-MONSTER_WHITE,vec3f(0))*EXCESS;
        light*=gain*ec.beam.y;
    }
    // The white frame, and the white's afterglow on the face once both the
    // white behind the monster and the white frame have gone.
    let cover=ec.tube.z;
    light=mix(light,vec3f(FLASH),cover);
    light+=vec3f(FLASH_GLOW)*max(ec.glow.z-max(cover,ec.tube.y),0.0);
    let dy=s.y-centre;
    // The line.
    if ec.beam.z>0.0 {
        let gain=line_gain(ec.ends.x,ec.ends.y);
        let spot=exp(-dy*dy/(2.0*LINE_SPOT*LINE_SPOT));
        let row=row_rgb(i32(LINE_ROW),s.x,shift);
        light+=row*gain*between(s.x,ec.ends.x,ec.ends.y)*spot*ec.beam.z;
        let dx=max(max(ec.ends.x-.5-s.x,s.x-ec.ends.y-.5),0.0)*COLUMN;
        let halo=exp(-length(vec2f(dx,dy))/LINE_HALO_REACH);
        light+=ec.colour.rgb*gain*LINE_HALO*halo*ec.beam.z*step(ec.ends.x,ec.ends.y);
    }
    // The trail: what the band and the line lit a few frames before still
    // glows, fading. The band's, once the picture squeezes, coloured as the
    // picture's columns are; the line's in its row's colours.
    if compression>1.0 {
        var reach=array<f32,5>(100.0/compression,0.0,0.0,0.0,0.0);
        var band=array<f32,5>(ec.beam.y*pow(compression,BAND_POWER),0.0,0.0,0.0,0.0);
        for(var i=0;i<4;i++) {
            // The picture held and the white leave none: the white's
            // afterglow is the face's own.
            let h=ec.trail_rows[i];
            reach[i+1]=h;
            band[i+1]=select(0.0,ec.trail_band[i]*pow(100.0/h,BAND_POWER),h<99.9);
        }
        let column=profile[u32(clamp(s.x,0.0,319.0))].rgb;
        light+=column*TRAIL*persisted(abs(dy),reach,band);
    }
    // Its ends close in on the dot's column from both sides, so each side's
    // reach is measured from there.
    let middle=DOT_COLUMN+.5;
    let leftward=s.x<middle;
    let ends=select(ec.trail_right+1.0-middle,middle-ec.trail_left,leftward);
    var reach_line=array<f32,5>(select(ec.ends.y+1.0-middle,middle-ec.ends.x,leftward),0.0,0.0,0.0,0.0);
    var lit=array<f32,5>(ec.beam.z*line_gain(ec.ends.x,ec.ends.y),0.0,0.0,0.0,0.0);
    var any_line=ec.beam.z;
    for(var i=0;i<4;i++) {
        reach_line[i+1]=ends[i];
        lit[i+1]=ec.trail_line[i]*line_gain(ec.trail_left[i],ec.trail_right[i]);
        any_line+=ec.trail_line[i];
    }
    if any_line>0.0 {
        let spot=exp(-dy*dy/(2.0*LINE_SPOT*LINE_SPOT));
        let after=persisted(abs(s.x-middle),reach_line,lit);
        light+=row_rgb(i32(LINE_ROW),s.x,shift)*TRAIL*after*spot;
    }
    // The dot: a point of phosphor, glowing with the original's pulse, and
    // its slower afterglow.
    let r=length((s-vec2f(DOT_COLUMN+.5,centre))*vec2f(COLUMN,1.0));
    let fast=ec.glow.x;
    let slow=ec.glow.y;
    light+=DOT_WHITE*fast*(DOT_CORE*exp(-r*r/(2.0*DOT_SPOT*DOT_SPOT))+DOT_HALO*exp(-r/DOT_REACH));
    light+=AFTER_TINT*slow*(AFTER_CORE*exp(-r*r/(2.0*AFTER_SPOT*AFTER_SPOT))+AFTER_HALO*exp(-r/DOT_REACH));
    // The raster's edge and the vignette.
    let vignette=1.0-VIGNETTE*a*(1.0-(1.0-.5*n.x*n.x)*(1.0-.5*n.y*n.y));
    return max(light*face*vignette,vec3f(0));
}
