// The water parts 3 and 4 share (authored throughout): its reflectance,
// its swell and the drops' rings. Each part places the plane, finds where
// a ray meets it and mirrors its own light (dots_light.wgsl and
// plasma_light.wgsl); here is what the water itself does. `water` is a
// part's water controls (plasma::WaterTuning::controls): the mirror's
// gain, the swell's height, its clock (seconds) and its size, tuned live
// on the preview's number keys.
//
// Its index of refraction (Fresnel's equations, unpolarised: 0.02 head
// on, rising steeply toward grazing). The swell: four long directional
// waves (swell_wave), their periods with the square root of the
// wavelength as deep water's are, the longest's WATER_PERIOD seconds (the
// scale chosen by eye). The slopes too small to draw: their spread, in the
// plane of incidence, which draws the mirror image out into streaks.
const WATER_ETA=1.333;
const WATER_PERIOD=7.5;
const WATER_LONGEST=280.0;
const WATER_FINE=.025;
// The drops (their times and places authored, plasma.rs, WaterTuning::
// drops; their rings physical). Each sets off a ring packet of capillary-
// gravity waves, w^2 = g k + (sigma / rho) k^3, at an authored scale of
// 6.5 mm a unit (the slab 3.2 m wide): g 1509 units/s^2 and sigma / rho,
// water's, 265 units^3/s^2. By stationary phase (the Cauchy-Poisson
// problem), at radius r and age t the waves are those whose group
// velocity is r / t, on the gravity branch, so the longer waves lead;
// inside the slowest group velocity's radius (27.3 units/s, at k 0.94)
// there are none, and the packet's inner edge, a caustic, is taken at
// Airy's scale (from w's third derivative there, DROP_W3). The drop's
// disturbance is a Gaussian DROP_SOURCE across, which damps the short
// waves. Besides thinning as they spread, the rings fade (DROP_FADE,
// authored: the films on still water); DROP_STRENGTH sets their slope,
// about 0.04 at the packet's peak a second after the drop (authored).
// Beyond DROP_REACH units a second the packet is negligible.
const DROP_G=1509.0;
const DROP_TENSION=265.0;
const DROP_K_TURN=.9385;
const DROP_CG_MIN=27.316;
const DROP_W3=19.66;
const DROP_SOURCE=4.0;
const DROP_STRENGTH=10.0;
const DROP_FADE=3.0;
// plasma::DROP_LIFE and MAX_DROPS.
const DROP_LIFE=4.0;
const MAX_DROPS=6u;
const DROP_REACH=75.0;

// Fresnel's reflectance for unpolarised light from air onto water, at the
// cosine c of the angle of incidence.
fn water_fresnel(c:f32)->f32 {
    let ci=clamp(c,0.0,1.0);
    let ct=sqrt(max(1.0-(1.0-ci*ci)/(WATER_ETA*WATER_ETA),0.0));
    let s=(ci-WATER_ETA*ct)/(ci+WATER_ETA*ct);
    let p=(WATER_ETA*ci-ct)/(WATER_ETA*ci+ct);
    return .5*(s*s+p*p);
}
// One wave of the swell: its slope (the height's gradient, across and
// back) at point q of the water at the swell's clock t (water.z). Its
// height is steepness / k times sin(k u.q - w t + phase), its period deep
// water's for its wavelength. The tuned size scales the wavelengths (the
// periods following, as deep water's do) and the height the slopes.
fn swell_wave(q:vec2f,water:vec4f,dir:vec2f,wavelength:f32,steepness:f32,phase:f32)->vec2f {
    let u=normalize(dir);
    let t=water.z;
    let size=water.w;
    let k=(2.0*PI/wavelength)/size;
    let w=(2.0*PI/(WATER_PERIOD*sqrt(wavelength/WATER_LONGEST)))/sqrt(size);
    return u*(steepness*water.y)*cos(k*dot(u,q)-w*t+phase);
}
fn drop_omega(k:f32)->f32 {return sqrt(DROP_G*k+DROP_TENSION*k*k*k);}
fn drop_group(k:f32)->f32 {return (DROP_G+3.0*DROP_TENSION*k*k)/(2.0*drop_omega(k));}
// The slope a drop's rings give at floor point q; `footprint` is a
// pixel's size there (units), below which rings are not drawn.
fn drop_slope(q:vec2f,drop:vec4f,footprint:f32)->vec2f {
    let t=drop.z;
    if drop.w<=0.0 || t<=0.0 || t>=DROP_LIFE {return vec2f(0);}
    let rel=q-drop.xy;
    let r=length(rel);
    let speed=r/max(t,1e-4);
    if speed<=DROP_CG_MIN || speed>DROP_REACH {return vec2f(0);}
    // The wavenumber whose group velocity is r / t on the gravity branch,
    // where it falls as k rises: by bisection in log k.
    var lo=.01;
    var hi=DROP_K_TURN;
    for(var i=0;i<14;i++) {
        let m=sqrt(lo*hi);
        if drop_group(m)>speed {lo=m;} else {hi=m;}
    }
    let k=sqrt(lo*hi);
    let w=drop_omega(k);
    let cg=drop_group(k);
    let w2=(6.0*DROP_TENSION*k-2.0*cg*cg)/(2.0*w);
    let airy=pow(DROP_W3,2.0/3.0)*pow(t,-1.0/3.0);
    let curve=sqrt(w2*w2+airy*airy);
    let source=exp(-k*k*DROP_SOURCE*DROP_SOURCE*.25);
    let amplitude=DROP_STRENGTH*source*sqrt(k/(r*t*curve))*k;
    let life=smoothstep(0.0,.3,t)*exp(-t/DROP_FADE)*(1.0-smoothstep(DROP_LIFE-.8,DROP_LIFE,t));
    let drawn=smoothstep(3.0,6.0,2.0*PI/k/footprint);
    let inner=smoothstep(DROP_CG_MIN,DROP_CG_MIN*1.06,speed);
    return rel/r*amplitude*life*drawn*inner*sin(k*r-w*t);
}
// The water's slope at point q of it (across, and back from the camera)
// at the swell's clock: the swell, four waves, mostly coming from far off
// toward the camera (in part 4 from the sheets). (A ring of ripples from
// part 4's slab was tried and left out: the water near the slab is hidden
// behind it, and further out the ring's slopes run across the line of
// sight and hardly move the mirror image.)
fn water_slope(q:vec2f,water:vec4f)->vec2f {
    var g=swell_wave(q,water,vec2f(.3,-1),280.0,.026,0.0);
    g+=swell_wave(q,water,vec2f(-.7,-.7),175.0,.018,1.7);
    g+=swell_wave(q,water,vec2f(1,-.35),115.0,.012,4.1);
    g+=swell_wave(q,water,vec2f(-.25,1),78.0,.008,2.6);
    return g;
}
