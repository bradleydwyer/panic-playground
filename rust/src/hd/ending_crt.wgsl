// The modern part 8's pictures: what each tube of the wall shows, as a
// function of the point on its face (ending_set.wgsl builds the wall of
// monitors, the camera and the light around them). Authored throughout, on
// the port's timing, pictures and palettes (ending_common.wgsl,
// hd/ending.rs).
//
// The original holds part 7's monster until the music reaches 35:00, then
// turns colour 0 white behind it, shows a white frame, squeezes the picture
// toward row 100 over 17 frames while lifting every colour toward white,
// keeps row 100 alone as a line, erases it from both ends to one pixel,
// pulses that pixel's colour twice and stops. It reads as a monitor
// switching off; here every tube of the wall does:
// - The hold. Early, while the centre tube still fills the frame, the
//   picture becomes an old surveillance monitor's, strongly: the original's
//   200 lines as scanlines with dark gaps between them (each line's spot
//   wider as it is brighter), an aperture grille of red, green and blue
//   phosphor stripes, the colour bleeding a little along the lines, bloom
//   and halation on bright parts, a bowed raster, darker corners, and the
//   whole graded toward a green-yellow phosphor, green-black in the
//   shadows. Where a tube is too small for its 200 lines or its stripes,
//   they give way to coarser ones (2, 4, 8 ... rows a line; 2, 4 ...
//   columns a triad) so it still reads as a CRT, never as a clean picture,
//   and never aliasing; far pictures are taken from a filtered copy.
// - The switch-off follows the original's stages and timing. The white
//   behind the monster and the white frame flash the face, and their light
//   lingers on the phosphor. The picture squeezes with the original's
//   compression, its colours lifted as the original lifts them and its light
//   concentrated as the beam's lines crowd together, so it brightens and
//   the band blooms. The line keeps the original's row and its ends as they
//   are erased, brightening as it shortens. The dot is a glowing point of
//   phosphor following the original's pulse, with a slower afterglow that
//   gathers while it pulses and decays to black at the end. The line and
//   the dot keep a least size on the frame, so they read on small tubes.
//   All of it glows in the same green phosphor.
// - The phosphor's persistence: what the beam lit a few frames before still
//   glows, fading, so the collapse leaves a short trail. Everything is a
//   function of the moment (hd/ending.rs works out the past states).
// Nothing flickers or rolls; nothing is noise.
// Module: the shared controls, ending_common.wgsl, this file and
// ending_set.wgsl.

// ---- The look (authored) -------------------------------------------------------
// The raster: how far it bows with the glass (each axis pulled in by the
// other's square), and its corners' radius (rows).
const WARP=.13;
const CORNER=14.0;
// The scanlines: the beam's spot (rows) on a dark and a bright line, so
// dark gaps open between the lines; the least frame pixels between lines,
// coarser lines taking over below it; how deep the coarse lines' gaps go.
const SPOT_DARK=.22;
const SPOT_BRIGHT=.44;
const LINE_PIXELS=3.2;
const COARSE_DEPTH=.75;
// The aperture grille: a triad of stripes to each source column at first,
// its depth, and the least frame pixels a triad (coarser below it).
const TRIAD=1.0;
const GRILLE=.85;
const TRIAD_PIXELS=4.5;
// The colour bleeding along the lines: how far back (columns) and how much
// of the colour comes from there.
const BLEED=1.6;
const BLEED_AMOUNT=.5;
// Halation (of the blurred picture), convergence error at the face's sides
// (source columns, red outward, blue inward), the vignette and the corners'
// darkening, and the face's own glow above black (display light, green).
const HALATION=.14;
const CONVERGENCE=.5;
const VIGNETTE=.45;
const CORNERS=.35;
const GLASS=vec3f(.0010,.0026,.0013);
// The green phosphor: how much of the picture's own colour gives way to it,
// the exponents that shade each channel's ramp (red and blue fall off
// faster than green, so mids go yellow-green and shadows green), and the
// pale green the highlights, the flash, the lines and the dots glow.
const GREEN=.82;
const PHOSPHOR=vec3f(1.22,1.0,2.4);
const PHOSPHOR_HIGH=vec3f(.62,1.0,.36);
// As part 7 ends (greetings_monster.wgsl): the brightest the monster shows
// through the tonemap (its inverse, as part 7's). Light the scanlines put
// above it goes on into HDR.
const MONSTER_WHITE=.9;
const EXCESS=1.6;
// The white: the face's light on the white frame, and its afterglow.
const FLASH=3.0;
const FLASH_GLOW=1.2;
// The squeeze: the band's light rises as the compression to this power;
// the rows a screen row gathers are sampled this many times.
const BAND_POWER=.35;
const BAND_TAPS=6;
// The line: its spot (rows, and at least LINE_LEAST frame pixels), its
// light, rising as it shortens by this power of the shortening, at most
// LINE_MOST; its halo's share and reach (rows).
const LINE_SPOT=.6;
const LINE_LEAST=.8;
const LINE_GAIN=5.0;
const LINE_POWER=.5;
const LINE_MOST=4.0;
const LINE_HALO=.035;
const LINE_HALO_REACH=8.0;
// The dot: the spot's spread (rows, and at least DOT_LEAST frame pixels)
// and light, its halo's light and reach (rows, and at least DOT_LEAST_REACH
// pixels); the afterglow's spread, light, halo and a greener tint (a
// phosphor's longest-lived part).
const DOT_SPOT=1.1;
const DOT_LEAST=1.3;
const DOT_CORE=30.0;
const DOT_HALO=.9;
const DOT_REACH=9.0;
const DOT_LEAST_REACH=9.0;
const AFTER_TINT=vec3f(.8,1.0,.85);
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
// The monster in linear light with its mipmaps (ending::pictures): on the
// hold's palette, and on the white behind it.
@group(0) @binding(10) var pictures:texture_2d_array<f32>;
// Linear between texels and between mipmaps.
@group(0) @binding(11) var trilinear:sampler;

// ---- Part 7's look --------------------------------------------------------------
// greetings_common.wgsl's inverse of the shared tonemap: part 7 shows the
// monster through it, and so does this view.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}

// ---- The green phosphor ------------------------------------------------------------
fn luminance(c:vec3f)->f32 {return dot(c,vec3f(.2126,.7152,.0722));}
// Light c as the green phosphor shows it, its ramp shaded by `exponents`:
// its brightness kept, its colour the phosphor's (yellow-green in the mids,
// green in the shadows, pale green in the highlights and beyond).
fn phosphor(c:vec3f,exponents:vec3f)->vec3f {
    let l=max(luminance(c),0.0);
    let ramp=pow(vec3f(clamp(l,1e-4,1.0)),exponents-1.0);
    let tint=mix(ramp,PHOSPHOR_HIGH,smoothstep(.5,1.0,l));
    return l*tint/max(luminance(tint),1e-4);
}
// The phosphor's share of the picture at the CRT's presence `a`.
fn graded(c:vec3f,exponents:vec3f,a:f32)->vec3f {
    return mix(c,phosphor(c,exponents),GREEN*a);
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
// The three guns' colours at their own columns (convergence error), the
// colour bleeding in from BLEED columns back by `bleed`.
fn row_rgb(m:i32,x:f32,shift:f32,bleed:f32)->vec3f {
    let sharp=vec3f(row_colour(m,x+shift).r,row_colour(m,x).g,row_colour(m,x-shift).b);
    if bleed<=0.0 {return sharp;}
    let back=row_colour(m,x-BLEED);
    let l=luminance(sharp);
    return l+mix(sharp-l,back-luminance(back),bleed);
}
// The four rows round the point: as the smooth picture weights them, and
// as the beam's lines light them, each line a spot spread as it is bright.
struct Scanned {even:vec3f, lines:vec3f}
fn scanned(s:vec2f,shift:f32,bleed:f32)->Scanned {
    let q=s-.5;
    let ky=i32(floor(q.y));
    let t=q.y-floor(q.y);
    let smoothed=catmull(t);
    var out=Scanned(vec3f(0),vec3f(0));
    for(var i=0;i<4;i++) {
        let r=row_rgb(ky+i-1,s.x,shift,bleed);
        let d=t-f32(i-1);
        let sigma=mix(SPOT_DARK,SPOT_BRIGHT,sqrt(clamp(max(r.r,max(r.g,r.b)),0.0,1.0)));
        out.lines+=r*exp(-d*d/(2.0*sigma*sigma))/(sigma*2.5066283);
        out.even+=r*smoothed[i];
    }
    out.lines=max(out.lines,vec3f(0));
    out.even=max(out.even,vec3f(0));
    return out;
}
// Lines `period` rows apart, at row centres, as a brightness of mean 1.
fn lines_of(y:f32,period:f32,depth:f32)->f32 {
    return 1.0+depth*cos(6.2831853*(y/period-.5));
}
// The coarse line structure where the beam's 200 lines would come closer
// than LINE_PIXELS frame pixels: 2, 4, 8 ... rows a line, one level
// fading into the next.
fn coarse_lines(y:f32,pixel:f32,depth:f32)->f32 {
    let level=max(log2(LINE_PIXELS*pixel),0.0);
    let k=floor(level);
    let f=smoothstep(0.0,1.0,level-k);
    return mix(lines_of(y,exp2(k),depth),lines_of(y,exp2(k+1.0),depth),f);
}
// The aperture grille: red, green and blue stripes, each of mean 1, a
// triad to TRIAD columns, or 2, 4 ... times that where a triad would be
// under TRIAD_PIXELS frame pixels.
fn stripes(u:f32)->vec3f {
    let t=6.2831853*u;
    let w=.5+.5*cos(vec3f(t-1.0471976,t-3.1415927,t-5.2359878));
    return w*w/.375;
}
fn grille(x:f32,pixel:f32)->vec3f {
    let level=max(log2(TRIAD_PIXELS*pixel/COLUMN/TRIAD),0.0);
    let k=floor(level);
    let f=smoothstep(0.0,1.0,level-k);
    return mix(stripes(x/(TRIAD*exp2(k))),stripes(x/(TRIAD*exp2(k+1.0))),f);
}
// The picture far off: the filtered copy, on the hold's palette or on the
// white behind the monster.
fn far_picture(s:vec2f,pixel:f32)->vec3f {
    let uv=s/vec2f(320,200);
    let lod=max(log2(pixel/COLUMN),0.0);
    let held=textureSampleLevel(pictures,trilinear,uv,0,lod).rgb;
    let white=textureSampleLevel(pictures,trilinear,uv,1,lod).rgb;
    return mix(held,white,clamp(ec.tube.y,0.0,1.0));
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
        c+=row_rgb(i32(floor(m+o)),s.x,shift,0.0)*w;
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
// the CRT's presence `a`, a frame's pixel `pixel` rows across there, and
// the exponents of its green: HDR, before the fade to black.
fn screen(plain:vec2f,a:f32,pixel:f32,exponents:vec3f)->vec3f {
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
    var picture=vec3f(0);
    var light=vec3f(0);
    if ec.beam.y>0.0 {
        var c=vec3f(0);
        var gain=1.0;
        if compression<=1.0 {
            // Near, the beam's 200 lines; farther, coarser lines over the
            // smooth picture; far, over the filtered copy (as the CRT comes
            // on: until then, part 7's picture as it is).
            let fine=1.0-smoothstep(.8,1.0,LINE_PIXELS*pixel);
            let far=smoothstep(.6,1.0,pixel)*a;
            var even=vec3f(0);
            var near=vec3f(0);
            if far<1.0 {
                let rows=scanned(s,shift,BLEED_AMOUNT*a);
                even=rows.even;
                near=mix(rows.even,rows.lines,a);
            }
            if far>0.0 {
                even=mix(even,far_picture(s,pixel),far);
            }
            let depth=COARSE_DEPTH*a*(1.0-.5*clamp(luminance(even),0.0,1.0));
            let coarse=even*coarse_lines(s.y,pixel,depth);
            c=mix(coarse,near,fine);
            c+=HALATION*a*textureSampleLevel(halation,linear,s/vec2f(320,200),0.0).rgb;
            // Green first, then the grille's stripes, so its triads show.
            c=graded(c,exponents,a)*mix(vec3f(1),grille(s.x,pixel),GRILLE*a);
            c+=GLASS*a;
        } else {
            c=graded(squeezed(s,shift,compression),exponents,a);
            gain=pow(compression,BAND_POWER);
        }
        picture=tonemap_inverse(min(c,vec3f(MONSTER_WHITE)))+max(c-MONSTER_WHITE,vec3f(0))*EXCESS;
        picture*=gain*ec.beam.y;
    }
    // The white frame, and the white's afterglow on the face once both the
    // white behind the monster and the white frame have gone.
    let cover=ec.tube.z;
    picture*=1.0-cover;
    light+=vec3f(FLASH)*cover;
    light+=vec3f(FLASH_GLOW)*max(ec.glow.z-max(cover,ec.tube.y),0.0);
    let dy=s.y-centre;
    let line_spot=max(LINE_SPOT,LINE_LEAST*pixel);
    // The line.
    if ec.beam.z>0.0 {
        let gain=line_gain(ec.ends.x,ec.ends.y);
        let spot=exp(-dy*dy/(2.0*line_spot*line_spot));
        let row=row_rgb(i32(LINE_ROW),s.x,shift,0.0);
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
        let spot=exp(-dy*dy/(2.0*line_spot*line_spot));
        let after=persisted(abs(s.x-middle),reach_line,lit);
        light+=row_rgb(i32(LINE_ROW),s.x,shift,0.0)*TRAIL*after*spot;
    }
    // The dot: a point of phosphor, glowing with the original's pulse, and
    // its slower afterglow; never smaller than a few of the frame's pixels.
    let r=length((s-vec2f(DOT_COLUMN+.5,centre))*vec2f(COLUMN,1.0));
    let fast=ec.glow.x;
    let slow=ec.glow.y;
    let spot=max(DOT_SPOT,DOT_LEAST*pixel);
    let reach=max(DOT_REACH,DOT_LEAST_REACH*pixel);
    light+=fast*(DOT_CORE*exp(-r*r/(2.0*spot*spot))+DOT_HALO*exp(-r/reach));
    let after_spot=max(AFTER_SPOT,DOT_LEAST*1.3*pixel);
    light+=AFTER_TINT*slow*(AFTER_CORE*exp(-r*r/(2.0*after_spot*after_spot))+AFTER_HALO*exp(-r/reach));
    // The flash, the line, the trails and the dot in the green phosphor;
    // the raster's edge, the vignette and the darker corners.
    let vignette=(1.0-VIGNETTE*a*(1.0-(1.0-.5*n.x*n.x)*(1.0-.5*n.y*n.y)))
        *(1.0-CORNERS*a*n.x*n.x*n.y*n.y);
    return max((picture+graded(light,exponents,a))*face*vignette,vec3f(0));
}
