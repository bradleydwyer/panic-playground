// The modern part 8's pictures: what each tube of the wall shows, as a
// function of the point on its face (ending_set.wgsl builds the wall of
// monitors, the camera and the light around them). Authored throughout, on
// the port's timing, pictures and palettes (ending_common.wgsl,
// hd/ending.rs). The look of the tubes is modelled on the owner's reference,
// a filmed wall of surveillance monitors: a model of how that footage looks,
// not of anything in the original demo.
//
// The original holds part 7's monster until the music reaches 35:00, then
// turns colour 0 white behind it, shows a white frame, squeezes the picture
// toward row 100 over 17 frames while lifting every colour toward white,
// keeps row 100 alone as a line, erases it from both ends to one pixel,
// pulses that pixel's colour twice and stops. It reads as a monitor
// switching off; here every tube of the wall does:
// - The hold, its look coming on in the reference's order (ec.look): the
//   picture first takes the reference's pale mint grade, high-key, its
//   blacks lifted to green, soft, its highlights glowing, with faint red and
//   green fringes; then the tube's aperture grille fades in, its stripes
//   continuous from the first and only coming up in strength; then, as the
//   screen's corners come into view, the raster's barrel bow and the darker
//   corners. Faint scanlines stay under the grille.
// - The grille is modelled as it looks filmed: continuous red, green and
//   blue phosphor stripes at a fixed pitch on the inside of the curved front
//   glass, lit by the picture, seen through that glass by a virtual camera
//   with a coarse sensor, a few dozen rows across the frame (hd/ending.rs,
//   SENSOR_LINES), whatever the size drawn. The glass's curvature bends the
//   stripes and makes their spacing vary smoothly across the face, mirrored
//   about its middle, a little more while the camera is close. Where the
//   stripes are well above the sensor's resolution they show as fine
//   vertical lines; nearer it they fade; beyond it the beat of their pitch
//   against the sensor's shows as broad soft vertical bands, bowed with the
//   glass, their edges in soft colour where the red and blue stripes' beats
//   sit a little off the green's, worked out per set from the scale (not
//   from aliasing in the drawing), so they widen, narrow and drift smoothly
//   as the camera draws back. The sensor is coarse enough that the bands
//   are there from when the grille fades in, close up, and pass again and
//   again as the camera draws back, until the switch-off.
// - The switch-off follows the original's stages and timing. The white
//   behind the monster and the white frame flash the face, and their light
//   lingers on the phosphor. The picture squeezes with the original's
//   compression, its colours lifted as the original lifts them and its light
//   concentrated as the beam's lines crowd together, so it brightens and
//   the band blooms. The line keeps the original's row and its ends as they
//   are erased, brightening as it shortens. The dot is a glowing point of
//   phosphor following the original's pulse, with a slower afterglow that
//   gathers while it pulses and decays to black at the end. The line and
//   the dot keep a least size on the frame, so they read on small tubes;
//   the dot keeps its peak while widened up to a limit and then its light,
//   so a far field of dots stays points of light in the dark rather than a
//   haze. Where a tube is small, the squeezed picture and the line are
//   taken over the pixel, from the filtered copy and from several columns,
//   so they do not sparkle. All of it in the same mint grade.
// - The phosphor's persistence: what the beam lit a few frames before still
//   glows, fading, so the collapse leaves a short trail. Everything is a
//   function of the moment (hd/ending.rs works out the past states).
// Nothing flickers or rolls; nothing is noise.
// Module: the shared controls, ending_common.wgsl, this file and
// ending_set.wgsl.

// ---- The look (authored) -------------------------------------------------------
// The ramps (hd/ending.rs, ec.look): the grade, the grille, the screen's
// edge, the bezel's light, each 0 to 1.
// The picture's size within the screen's face (ending::PICTURE_SCALE), the
// glass round it dark but softly lit: the picture's light spilling onto it
// (its share of the picture's mean, and how far it reaches, rows of the
// face), and its black against the picture's lifted black.
const PICTURE_SCALE=0.7;
const SPILL=.05;
const SPILL_REACH=10.0;
const GLASS_BLACK=.5;
// The raster: how far it bows (barrel; each axis pulled in by the other's
// square), and its corners' radius (rows).
const WARP=.1;
const CORNER=14.0;
// Faint scanlines: the beam's spot (rows) on a dark and a bright line, the
// share of the picture the lines take, the least frame pixels between lines
// (coarser lines below it) and how deep the coarse lines go.
const SPOT_DARK=.11;
const SPOT_BRIGHT=.26;
const SCANLINES=.08;
const LINE_PIXELS=3.2;
const COARSE_DEPTH=.06;
// The grille (a model of the reference's filmed tube, not of anything in
// the original): continuous vertical stripes of red, green and blue
// phosphor, a triad every few rows, never broken along their length; the
// stripes' depth. The triads' pitch and the sensor's rows come from
// hd/ending.rs (TRIAD_PITCH, tuned live, and SENSOR_LINES; triad_pitch and
// sensor_lines here). Their profile as harmonics of the pitch (a
// stripe a little narrower than its gap, its edges soft): each harmonic
// beats against the sensor on its own, which is what makes the bands cycle
// many times as the camera draws back. The colour split: how far the red
// and blue stripes sit from the green (cycles), the same everywhere; the
// colour that shows comes from that and the beats. The virtual camera: the
// share of a pixel's pitch its light is gathered over (small, as on an
// older interline sensor, so the bands keep coming back as the camera draws
// back, each pass a little weaker), its lens (how fast the contrast falls
// with frequency on the sensor, cycles a pixel: sharp, for the same) and
// the spread of the reconstruction between its pixels (pixels); the bands'
// strength against the physics (authored, less than it, so the picture
// shows through them close up).
const STRIPES=.75;
const HARMONICS=array<f32,6>(1.0,.23,-.2,-.15,.03,.04);
const SPLIT=.07;
const FILL=.1;
const LENS=12.0;
const BAND_GAIN=.6;
const RECONSTRUCT=.5;
// The curved front glass the stripes are seen through (modelled, CHIRP and
// GRILLE_BOW exaggerated so that they show; the same on every set, which
// is seen as if from straight in front at the camera's distance from the
// wall). The stripes lie evenly along the glass's curved inner surface, so
// seen head on they crowd toward the side edges, CHIRP denser there, and
// bow with it toward its top and bottom by GRILLE_BOW of their distance
// from the middle (seen as "(" and ")" at the sides). The glass is
// FACE_THICK rows thick at its middle and thicker toward its edges, its
// inner surface curving away from the camera with a radius of FACE_RADIUS
// rows, its index GLASS_INDEX: seen through it from close, the stripes away
// from the middle bend and crowd a little more, and that falls away as the
// camera draws back. Only the stripes are seen through it: the picture stays drawn
// where the ray meets the glass's front, as before.
const CHIRP=.2;
const GRILLE_BOW=.02;
const FACE_THICK=10.0;
const FACE_RADIUS=1200.0;
const GLASS_INDEX=1.5;
// The picture: halation of the blurred picture (its highlights glowing), the
// red and green fringes (source columns, red one way, green the other), the
// colour bleeding along the lines, the vignette and the corners' darkening.
const HALATION=.22;
const FRINGE=.55;
const BLEED=1.6;
const BLEED_AMOUNT=.3;
const VIGNETTE=.5;
const CORNERS=.45;
// The grade (read by eye and by sampling the reference's frames): blacks
// lifted (linear), mids raised (the tone's exponent), how much of the
// picture's own colour stays, and the tint from the shadows' deep green to
// the highlights' pale mint (red, green, blue against green), between these
// levels.
const LIFT=.006;
const HIGH_KEY=.68;
const OWN_COLOUR=.04;
const SHADOW_TINT=vec3f(.4,1.0,.48);
const HIGH_TINT=vec3f(.88,1.0,.88);
const TINT_LEVELS=vec2f(.05,.6);
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
// pixels); how many times its area the spot widened to its least size
// keeps its peak (beyond that it keeps its light, spread wider), and the
// halo, on a frame this many pixels high (on others the light is that
// one's); the afterglow's
// spread, light, halo and a greener tint (a phosphor's longest-lived part).
const DOT_SPOT=1.1;
const DOT_LEAST=1.3;
const DOT_CORE=30.0;
const DOT_HALO=.9;
const DOT_REACH=9.0;
const DOT_LEAST_REACH=2.5;
const DOT_BOOST=30.0;
const HALO_BOOST=6.0;
const REFERENCE_HEIGHT=1440.0;
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
// The monster in linear light with its mipmaps (ending::picture_levels),
// one layer for each palette the picture is shown on, from the hold to the
// line, and each frame's layer.
@group(0) @binding(10) var pictures:texture_2d_array<f32>;
// Linear between texels and between mipmaps.
@group(0) @binding(11) var trilinear:sampler;
@group(0) @binding(12) var<storage,read> layer_of:array<u32>;

// ---- Part 7's look --------------------------------------------------------------
// greetings_common.wgsl's inverse of the shared tonemap: part 7 shows the
// monster through it, and so does this view.
fn tonemap_inverse(c:vec3f)->vec3f {
    let y=clamp(c,vec3f(0),vec3f(.999));
    let a=2.43*y-2.51;let b=.59*y-.03;let k=.14*y;
    return (-b-sqrt(max(b*b-4.0*a*k,vec3f(0))))/(2.0*a);
}

// ---- The grade ---------------------------------------------------------------------
fn luminance(c:vec3f)->f32 {return dot(c,vec3f(.2126,.7152,.0722));}
// Light c in the reference's mint: its brightness raised toward high-key by
// the exponent `key`, its colour the tint at its level (deep green in the
// shadows, pale mint in the highlights), keeping a little of its own, and a
// set's `tint`.
fn mint(c:vec3f,tint:vec3f,key:f32)->vec3f {
    let l=max(luminance(c),0.0);
    let level=pow(l,key);
    let t=mix(SHADOW_TINT,HIGH_TINT,smoothstep(TINT_LEVELS.x,TINT_LEVELS.y,level))*tint;
    let grey=level*t/luminance(t);
    let own=c*(level/max(l,1e-3));
    return mix(grey,own,OWN_COLOUR);
}
// The grade's share of the picture at its ramp `a`, and of light (the
// flash, the line, the dots, the light on the sets), its tint alone.
fn graded(c:vec3f,tint:vec3f,a:f32)->vec3f {
    return mix(c,mint(c,tint,HIGH_KEY),a);
}
fn graded_light(c:vec3f,tint:vec3f,a:f32)->vec3f {
    return mix(c,mint(c,tint,1.0),a);
}

// ---- The picture on the tube ------------------------------------------------------
// Palette entry `index` between the shown frame and the next, linear.
fn colour(index:u32)->vec3f {
    let a=colours[frame_now()*256u+index].rgb;
    let b=colours[frame_next()*256u+index].rgb;
    return mix(a,b,frame_mix());
}
// Source row m at column x (pixel centres at i + .5), Catmull-Rom across;
// nothing beyond the picture (the raster there is empty).
fn row_colour(m:i32,x:f32)->vec3f {
    if m<0 || m>199 {return vec3f(0);}
    let q=x-.5;
    let k=i32(floor(q));
    let w=catmull(q-floor(q));
    var c=vec3f(0);
    for(var i=0;i<4;i++) {
        let j=k+i-1;
        if j>=0 && j<=319 {c+=colour(textureLoad(monster,vec2i(j,m),0).r)*w[i];}
    }
    return c;
}
// The picture's mean colour now (linear), from its twelve cells
// (ending_set.wgsl's binding).
fn picture_mean()->vec3f {
    var m=vec3f(0);
    for(var c=0u;c<12u;c++) {
        m+=mix(cells[frame_now()*12u+c].rgb,cells[frame_next()*12u+c].rgb,frame_mix());
    }
    return m/12.0;
}
// How much of a frame's pixel (`pixel` rows) about source point s lies
// within the picture, across and down.
fn within(s:vec2f,pixel:f32)->vec2f {
    let d=min(s,vec2f(320,200)-s)*vec2f(COLUMN,1.0);
    return clamp(d/pixel+.5,vec2f(0),vec2f(1));
}
// The guns' colours at their own columns (red `shift` one way, green the
// other: the faint fringes), the colour bleeding in from BLEED columns back
// by `bleed`.
fn row_rgb(m:i32,x:f32,shift:f32,bleed:f32)->vec3f {
    let sharp=vec3f(row_colour(m,x+shift).r,row_colour(m,x-shift).g,row_colour(m,x).b);
    if bleed<=0.0 {return sharp;}
    let back=row_colour(m,x-BLEED);
    let l=luminance(sharp);
    return l+mix(sharp-l,back-luminance(back),bleed);
}
// The four rows round the point: as the smooth picture weights them, and
// as the beam's lines light them, each line a spot spread as it is bright.
struct Scanned {even:vec3f, lines:vec3f}
fn scanned(s:vec2f,shift:f32,bleed:f32,level:f32)->Scanned {
    let q=s-.5;
    let ky=i32(floor(q.y));
    let t=q.y-floor(q.y);
    let smoothed=catmull(t);
    var out=Scanned(vec3f(0),vec3f(0));
    for(var i=0;i<4;i++) {
        let r=row_rgb(ky+i-1,s.x,shift,bleed)+level;
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
// The triads' pitch (rows of the face) and the sensor's rows across the
// frame (hd/ending.rs).
fn triad_pitch()->f32 {return ec.moire.x;}
fn sensor_lines()->f32 {return ec.moire.y;}
// Where the camera, `distance` rows in front of a face on its axis, sees
// the stripes behind the point q of the glass's front (rows from the face's
// middle), to first order in the ray's angle: the ray bends toward the
// glass's normal as it enters and carries on through the glass, further
// out the thicker the glass, to the inner surface. The point there, and how
// many times as fast the stripes pass across there as across the front.
struct Seen {at:vec2f, crowd:f32}
fn through_glass(q:vec2f,distance:f32)->Seen {
    let k=1.0/(GLASS_INDEX*distance);
    let depth=FACE_THICK+dot(q,q)/(2.0*FACE_RADIUS);
    return Seen(q*(1.0+depth*k),1.0+depth*k+q.x*q.x/FACE_RADIUS*k);
}
// The grille as the virtual camera sees it at `face` (rows from the face's
// middle, on the glass's front), `across` sensor pixels from the set's
// middle, where a frame's pixel spans `pixel` rows of the face seen head
// on, while it fades in (`v`): a brightness for each channel, of mean 1.
// Every stripe is continuous; the fading in only brings them up in
// strength.
//
// The stripes are found where the camera sees them through the glass
// (through_glass), then laid along its curved inner surface (CHIRP,
// GRILLE_BOW): the curvature is all that bends them or changes their
// spacing, from the moment they fade in. Each harmonic k of their profile
// has frequency k f on the sensor (f triads a sensor pixel, higher toward
// the edges where the stripes crowd). Sampled, it shows as its beats
// against the sensor's pixels, at k f less a whole number n, phase
// k s / P - n across: for n = 0 the stripes themselves; otherwise bands,
// broad where the beat is near zero. Each beat is weighed by the sensor's
// aperture at k f and the reconstruction at the beat, and the two nearest n
// are summed (so near a whole number the bands are faint, as a pixel then
// gathers whole stripes), so the picture changes smoothly as f grows: the
// bands widen until they vanish, come back drifting the other way, narrow,
// and again, many times as the camera draws back, the pattern chirped from
// the middle to the mirrored edges. The colour split shifts the red and
// blue stripes, and so each beat k times as much.
fn grille(face:vec2f,across:f32,pixel:f32,v:f32)->vec3f {
    // Before it fades in it is 1 everywhere: sqrt(v) is 0 and the sum
    // finite.
    if v<=0.0 {return vec3f(1);}
    let half=160.0*COLUMN;
    let seen=through_glass(face,ec.eye.z);
    let bow=1.0+GRILLE_BOW*(seen.at.y/100.0)*(seen.at.y/100.0);
    let x=seen.at.x*bow;
    let h=x/half;
    // Along the glass, in triads; and the triads a sensor pixel here.
    let s=x*(1.0+CHIRP*h*h/3.0)/triad_pitch()+vec3f(SPLIT,0.0,-SPLIT);
    let f=pixel*seen.crowd*bow*u.screen.y/sensor_lines()/triad_pitch()*(1.0+CHIRP*h*h);
    let shown=sensor_lines()/u.screen.y;
    var sum=vec3f(0);
    for(var k=1;k<=6;k++) {
        let kf=f*f32(k);
        // The sensor's pixel (a box FILL of its pitch wide) and the lens.
        let q=3.1415927*kf*FILL;
        let a=HARMONICS[k-1]*sin(q)/q*exp(-(kf/LENS)*(kf/LENS));
        let n0=floor(kf);
        // As the camera draws back every beat at this point moves by k
        // times the triads from the middle times the draw back, over the
        // exposure (cycles); what moves fast blurs, and what moves half a
        // cycle or more a frame is left out, as sixty frames a second
        // cannot show it (it would seem to jump or run backward).
        let moved=f32(k)*abs(x)/triad_pitch()*ec.aim.w;
        let blur=exp(-2.5*moved*moved)*(1.0-smoothstep(.25,.5,moved));
        for(var j=0;j<2;j++) {
            let n=n0+f32(j);
            let b=kf-n;
            var w=a*exp(-19.739209*RECONSTRUCT*RECONSTRUCT*b*b)*select(BAND_GAIN,1.0,n==0.0);
            // Not finer than the frame drawn can show.
            w*=1.0-smoothstep(.3,.5,abs(b)*shown);
            let t=6.2831853*(f32(k)*s-n*across);
            sum+=w*blur*cos(t);
        }
    }
    return vec3f(1)+STRIPES*sqrt(v)*sum;
}
// The picture far off: the filtered copy on the shown frame's palette,
// toward the next's, at mip level `lod` (nothing beyond the picture is the
// caller's to weigh, by `within`).
fn far_picture(s:vec2f,lod:f32)->vec3f {
    let uv=clamp(s/vec2f(320,200),vec2f(0),vec2f(1));
    let a=textureSampleLevel(pictures,trilinear,uv,layer_of[frame_now()],lod).rgb;
    let b=textureSampleLevel(pictures,trilinear,uv,layer_of[frame_next()],lod).rgb;
    return mix(a,b,frame_mix());
}
// The squeeze: screen row y shows source rows about row 100 + (y - row 100)
// times the compression; its lines crowd within the beam's spot, so a
// screen row gathers the source rows the spot spans. Where a frame's pixel
// (`pixel` rows) spans many of them, it gathers them all, from the filtered
// copy.
fn squeezed(s:vec2f,shift:f32,compression:f32,pixel:f32)->vec3f {
    let centre=LINE_ROW+.5;
    let m=centre+(s.y-centre)*compression;
    let spot=max(SPOT_BRIGHT*compression,.5);
    let far=smoothstep(.6,1.0,pixel);
    let rows=pixel*compression;
    let reach=sqrt(spot*spot+far*rows*rows/12.0);
    let lod=max(log2(max(pixel/COLUMN,.5*reach)),0.0);
    var near=vec3f(0);
    var filtered=vec3f(0);
    var total=0.0;
    for(var i=0;i<BAND_TAPS;i++) {
        let o=((f32(i)+.5)/f32(BAND_TAPS)*2.0-1.0)*1.5*reach;
        let w=exp(-o*o/(2.0*reach*reach));
        let row=m+o;
        if far<1.0 {near+=row_rgb(i32(floor(row)),s.x,shift,0.0)*w;}
        if far>0.0 {
            let inside=within(vec2f(s.x,row),pixel).x*f32(row>=0.0 && row<200.0);
            filtered+=far_picture(vec2f(s.x,row),lod)*inside*w;
        }
        total+=w;
    }
    return max(mix(near,filtered,far)/total,vec3f(0));
}
// Source row m about column x as a frame's pixel (`pixel` rows) shows it:
// the columns it spans averaged once they are more than one.
fn row_seen(m:i32,x:f32,shift:f32,pixel:f32)->vec3f {
    let width=pixel/COLUMN;
    if width<=1.0 {return row_rgb(m,x,shift,0.0);}
    var c=vec3f(0);
    for(var i=0;i<4;i++) {c+=row_rgb(m,x+((f32(i)+.5)/4.0-.5)*width,shift,0.0);}
    return c/4.0;
}
// The line's light at a point: its row's colours between its ends, in the
// beam's spot, brighter as it shortens.
fn line_gain(left:f32,right:f32)->f32 {
    let full=ec.ends.w-ec.ends.z+1.0;
    return LINE_GAIN*min(pow(full/max(right-left+1.0,1.0),LINE_POWER),LINE_MOST);
}
// erf (Abramowitz and Stegun 7.1.26).
fn erf(x:f32)->f32 {
    let t=1.0/(1.0+.3275911*abs(x));
    let y=1.0-(((((1.061405429*t-1.453152027)*t)+1.421413741)*t-.284496736)*t+.254829592)*t*exp(-x*x);
    return sign(x)*y;
}
// A spot across the line, exp(-dy^2 / 2 sigma^2), averaged over a frame's
// pixel `pixel` rows high, so a thin line moving across the pixels keeps its
// light.
fn spot_across(dy:f32,sigma:f32,pixel:f32)->f32 {
    if pixel<.05*sigma {return exp(-dy*dy/(2.0*sigma*sigma));}
    let k=1.0/(sigma*1.4142136);
    return sigma*1.2533141/pixel*(erf((dy+.5*pixel)*k)-erf((dy-.5*pixel)*k));
}
// Whether column x lies on the line between its ends, its edges `width`
// columns soft.
fn between(x:f32,left:f32,right:f32,width:f32)->f32 {
    let w=.5*max(width,1.0);
    return smoothstep(left-w,left+w,x)*(1.0-smoothstep(right+1.0-w,right+1.0+w,x));
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

// A spot of `natural` rows widened to at least `least` frame pixels, where a
// pixel is `pixel` rows: its size, and its light's scale. As a frame
// REFERENCE_HEIGHT pixels high shows it, the spot keeps its peak until it
// has been widened to `boost` times its area, and from there keeps its
// light, so a far field of dots holds its brightness; at other sizes the
// spot keeps the light it has at that one, so the field looks the same.
fn widened(natural:f32,least:f32,pixel:f32,boost:f32)->vec2f {
    let size=max(natural,least*pixel);
    let reference=max(natural,least*pixel*u.screen.y/REFERENCE_HEIGHT)/natural;
    let light=min(reference*reference,boost);
    let k=size/natural;
    return vec2f(size,light/(k*k));
}
// The dot: a point of phosphor at `r` rows from its centre, glowing with the
// original's pulse, and its slower afterglow, where a frame's pixel is
// `pixel` rows across; never smaller than a few of the frame's pixels, so it
// reads as a point of light however small the tube. HDR, before the green.
fn dot_glow(r:f32,pixel:f32)->vec3f {
    let spot=widened(DOT_SPOT,DOT_LEAST,pixel,DOT_BOOST);
    let halo=widened(DOT_REACH,DOT_LEAST_REACH,pixel,HALO_BOOST);
    let after=widened(AFTER_SPOT,DOT_LEAST*1.3,pixel,DOT_BOOST);
    let core=DOT_CORE*spot.y*exp(-r*r/(2.0*spot.x*spot.x));
    let around=halo.y*exp(-r/halo.x);
    let fast=ec.glow.x*(core+DOT_HALO*around);
    let slow=ec.glow.y*(AFTER_CORE*after.y*exp(-r*r/(2.0*after.x*after.x))+AFTER_HALO*around);
    return vec3f(fast)+AFTER_TINT*slow;
}

// The light a tube shows at `plain`, a point on its face in units of the
// face's half size (the raster spans -1 to 1 on each axis, the picture
// PICTURE_SCALE of that in its middle), where a frame's pixel spans
// `face_pixel` rows of the face (`flat_pixel` seen head on), with its set's
// tint: HDR, before the fade to black. The look's ramps are ec.look's.
fn screen(plain:vec2f,face_pixel:f32,flat_pixel:f32,tint:vec3f)->vec3f {
    let g=ec.look.x;
    let v=ec.look.y;
    let e=ec.look.z;
    let half=vec2f(160.0*COLUMN,100.0);
    let n=plain*(1.0+WARP*e*plain.yx*plain.yx);
    let radius=CORNER*e;
    let box=abs(n*half)-half+radius;
    let edge=length(max(box,vec2f(0)))+min(max(box.x,box.y),0.0)-radius;
    let face=1.0-smoothstep(-.5*face_pixel,.5*face_pixel,edge);
    // The picture's own rows and columns, inset in the face, and a frame's
    // pixel in its rows.
    let p=n*half/PICTURE_SCALE;
    let pixel=face_pixel/PICTURE_SCALE;
    let s=vec2f(160.0+p.x/COLUMN,100.0-p.y);
    let inside=within(s,pixel);
    let picture_area=inside.x*inside.y;
    let shift=FRINGE*g;
    let compression=ec.beam.x;
    let centre=LINE_ROW+.5;
    var picture=vec3f(0);
    var light=vec3f(0);
    if ec.beam.y>0.0 {
        var c=vec3f(0);
        var gain=1.0;
        if compression<=1.0 {
            // Near, the smooth picture with faint scanlines; farther,
            // coarser lines; far, the filtered copy (as the grade comes on:
            // until then, part 7's picture as it is).
            let fine=1.0-smoothstep(.8,1.0,LINE_PIXELS*pixel);
            let far=smoothstep(.6,1.0,pixel)*g;
            var even=vec3f(0);
            var near=vec3f(0);
            if far<1.0 {
                let rows=scanned(s,shift,BLEED_AMOUNT*g,0.0);
                even=rows.even;
                near=mix(rows.even,rows.lines,SCANLINES*g);
            }
            if far>0.0 {
                let filtered=far_picture(s,max(log2(pixel/COLUMN),0.0))*picture_area;
                even=mix(even,filtered,far);
            }
            let depth=COARSE_DEPTH*g*(1.0-.5*clamp(luminance(even),0.0,1.0));
            let coarse=even*coarse_lines(s.y,pixel,depth);
            c=mix(coarse,near,fine);
            // The highlights' glow, the lifted black, and round the picture
            // its light spilling softly onto the glass.
            c+=HALATION*g*picture_area*textureSampleLevel(halation,linear,s/vec2f(320,200),0.0).rgb;
            c+=LIFT*g*mix(GLASS_BLACK,1.0,picture_area);
            let out=max(abs(p)-half,vec2f(0));
            c+=SPILL*g*picture_mean()*exp(-length(out)*PICTURE_SCALE/SPILL_REACH)*(1.0-picture_area);
            // Graded, then lit through the grille's stripes, across the
            // whole face.
            // Sensor pixels from the set's middle (so every set shows the
            // same pattern, mirrored about its middle).
            let across=plain.x*half.x/(flat_pixel*u.screen.y/sensor_lines());
            c=graded(c,tint,g)*grille(plain*half,across,flat_pixel,v);
        } else {
            c=graded(squeezed(s,shift,compression,pixel),tint,g);
            gain=pow(compression,BAND_POWER);
        }
        picture=tonemap_inverse(min(c,vec3f(MONSTER_WHITE)))+max(c-MONSTER_WHITE,vec3f(0))*EXCESS;
        picture*=gain*ec.beam.y;
    }
    // The white frame, and the white's afterglow on the face once both the
    // white behind the monster and the white frame have gone.
    let cover=ec.tube.z*picture_area;
    picture*=1.0-cover;
    light+=vec3f(FLASH)*cover;
    light+=vec3f(FLASH_GLOW)*max(ec.glow.z-max(ec.tube.z,ec.tube.y),0.0)*picture_area;
    let dy=s.y-centre;
    // The line's spot, and its light kept as a frame REFERENCE_HEIGHT
    // pixels high shows it (widened, the line keeps its peak there).
    let line_spot=max(LINE_SPOT,LINE_LEAST*pixel);
    let line_light=max(LINE_SPOT,LINE_LEAST*pixel*u.screen.y/REFERENCE_HEIGHT)/line_spot;
    // The line.
    if ec.beam.z>0.0 {
        let gain=line_gain(ec.ends.x,ec.ends.y);
        let spot=spot_across(dy,line_spot,pixel)*line_light;
        let row=row_seen(i32(LINE_ROW),s.x,shift,pixel);
        light+=row*gain*between(s.x,ec.ends.x,ec.ends.y,pixel/COLUMN)*spot*ec.beam.z;
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
        let column=profile[u32(clamp(s.x,0.0,319.0))].rgb*inside.x;
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
        let spot=spot_across(dy,line_spot,pixel)*line_light;
        let after=persisted(abs(s.x-middle),reach_line,lit);
        light+=row_seen(i32(LINE_ROW),s.x,shift,pixel)*TRAIL*after*spot;
    }
    // The flash, the line and the trails in the grade; the raster's edge,
    // the vignette and the darker corners. (The dot is ending_set.wgsl's,
    // from dot_glow.)
    let vignette=(1.0-VIGNETTE*e*(1.0-(1.0-.5*n.x*n.x)*(1.0-.5*n.y*n.y)))
        *(1.0-CORNERS*e*n.x*n.x*n.y*n.y);
    return max((picture+graded_light(light,tint,g))*face*vignette,vec3f(0));
}
