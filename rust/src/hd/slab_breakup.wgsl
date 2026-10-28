// Part 4's slab breaking up into part 7's dust (authored; slab_breakup.rs
// says how). Part 4's modern module leaves out the grains that have come
// away; part 7's flies them up into its dust (greetings_light.wgsl). Both
// read the slab, at rest, from u.plasma, which renderer.rs fills while it
// breaks up, u.plasma.breakup then holding 1 and the grid: cells across
// both faces, down the top face (from its near edge), down the front face
// (from its top edge). A grain is a cell of the grid, its site strayed a
// little within it, so the cells are a Voronoi tiling of each face.
//
// When a grain comes away (seconds from the slide's first frame): the
// edge runs from the top face's far edge forward and on down the front
// face over BREAKUP_SPAN, ragged by BREAKUP_RAGGED at about BREAKUP_SCALE
// (part 4's units; the block is 490 wide), each grain a little earlier or
// later; and how far a site strays in its cell. By BREAKUP_END
// (slab_breakup::BREAK_UP) every grain has settled.
const BREAKUP_SPAN=1.6;
const BREAKUP_RAGGED=.3;
const BREAKUP_SCALE=40.0;
const BREAKUP_JITTER=.06;
const BREAKUP_STRAY=.7;
const BREAKUP_END=4.3;
// A cell with no mote (slab_breakup::NO_MOTE).
const BREAKUP_NO_MOTE=0xffffffffu;
// clip::PIXEL_ASPECT: a source column is 5/6 of a row on the 4:3 display.
const BREAKUP_COLUMN=5.0/6.0;

fn breakup_on()->bool {return u.plasma.breakup.x>.5;}
fn breakup_seconds()->f32 {return u.plasma.part.y-u.plasma.events_late.y;}
fn breakup_grid()->vec3i {return vec3i(u.plasma.breakup.yzw);}
// The slab at rest (plasma_light.wgsl's slab_back, slab_down): its near
// top edge's centre, its axes back and down, its width, depth and height.
fn breakup_origin()->vec3f {return u.plasma.slab_origin.xyz;}
fn breakup_back()->vec3f {return vec3f(0,-u.plasma.slab_axes.x,u.plasma.slab_axes.y);}
fn breakup_down()->vec3f {return vec3f(0,u.plasma.slab_axes.y,u.plasma.slab_axes.x);}
fn breakup_size()->vec3f {return vec3f(u.plasma.slab_axes.zw,u.plasma.slab_size.x);}
// Where a point in part 4's camera space shows, in scene coordinates.
fn breakup_scene(x:vec3f)->vec2f {
    let f=u.plasma.camera.x;
    let s=vec2f(u.plasma.camera.y+1.2*f*x.x/x.z,100.0+f*x.y/x.z);
    return vec2f((s.x-160.0)*BREAKUP_COLUMN,100.0-s.y);
}
// A cell (face: 0 top, 1 front; column; row) and its index in the grid
// (the top face's cells row by row, then the front face's).
fn breakup_cell(n:u32)->vec3i {
    let g=breakup_grid();
    let top=u32(g.x*g.y);
    if n<top {return vec3i(0,i32(n%u32(g.x)),i32(n/u32(g.x)));}
    let k=n-top;
    if k>=u32(g.x*g.z) {return vec3i(-1);}
    return vec3i(1,i32(k%u32(g.x)),i32(k/u32(g.x)));
}
fn breakup_hash(c:vec3i,k:u32)->f32 {
    var h=u32(c.y+4096)*73856093u^u32(c.z+4096)*19349663u^u32(c.x+1)*2654435761u^(k+1u)*83492791u;
    h=(h^(h>>16u))*0x7feb352du;
    h=(h^(h>>15u))*0x846ca68bu;
    return f32((h^(h>>16u))&0xffffffu)/16777216.0;
}
// A cell's site, in the face's cells (column, row, each 0 to its count).
fn breakup_site(c:vec3i)->vec2f {
    let j=vec2f(breakup_hash(c,0u),breakup_hash(c,1u))-.5;
    return vec2f(c.yz)+.5+BREAKUP_STRAY*j;
}
// A point of a face, from its place in the face's cells, in part 4's camera
// space (on the face's plane: the carving left out).
fn breakup_point(face:i32,p:vec2f)->vec3f {
    let g=vec2f(breakup_grid().xy);
    let size=breakup_size();
    let across=vec3f((p.x/g.x-.5)*size.x,0,0);
    if face==0 {return breakup_origin()+across+breakup_back()*(p.y/g.y*size.y);}
    return breakup_origin()+across+breakup_down()*(p.y/f32(breakup_grid().z)*size.z);
}
// A smooth noise in the block, for the edge's raggedness.
fn breakup_noise(x:vec3f)->f32 {
    let q=x/BREAKUP_SCALE;
    let i=vec3i(floor(q));
    let f=q-floor(q);
    let w=f*f*(3.0-2.0*f);
    var n=0.0;
    for(var k=0;k<8;k++) {
        let o=vec3i(k&1,(k>>1)&1,k>>2);
        let c=select(1.0-w,w,o>vec3i(0));
        n+=breakup_hash(i+o,7u)*c.x*c.y*c.z;
    }
    return n;
}
// When cell c (its site `site`) comes away.
fn breakup_release(c:vec3i,site:vec2f)->f32 {
    let g=vec3f(breakup_grid());
    let size=breakup_size();
    // How far along the edge's way: back from the top face's far edge, then
    // down the front face.
    var along:f32;
    var x:vec3f;
    if c.x==0 {
        let back=site.y/g.y*size.y;
        along=size.y-back;
        x=vec3f((site.x/g.x-.5)*size.x,back,0.0);
    } else {
        let down=site.y/g.z*size.z;
        along=size.y+down;
        x=vec3f((site.x/g.x-.5)*size.x,0.0,down);
    }
    let ragged=.65*breakup_noise(x)+.35*breakup_noise(x*2.3+17.0);
    return BREAKUP_SPAN*along/(size.y+size.z)+BREAKUP_RAGGED*(ragged-.5)*2.0
        +BREAKUP_JITTER*(breakup_hash(c,2u)-.5);
}
// How far from the camera the slab's top or front face is at scene point q
// (part 4's camera space, the carving left out; part 7 places the slab as
// far from its camera), or 1e9 off it.
fn breakup_distance(q:vec2f)->f32 {
    let f=u.plasma.camera.x;
    let s=vec2f(160.0+q.x/BREAKUP_COLUMN,100.0-q.y);
    let d=vec3f((s.x-u.plasma.camera.y)/(1.2*f),(s.y-100.0)/f,1.0);
    let o=breakup_origin();
    let size=breakup_size();
    var nearest=1e9;
    for(var face=0;face<2;face++) {
        let normal=select(-breakup_back(),-breakup_down(),face==0);
        let toward=dot(d,normal);
        if abs(toward)<1e-6 {continue;}
        let hit=d*(dot(o,normal)/toward);
        let l=hit-o;
        let along=select(dot(l,breakup_down())/size.z,dot(l,breakup_back())/size.y,face==0);
        if hit.z>0.0 && abs(l.x)<=size.x*.5 && along>=0.0 && along<=1.0 {
            nearest=min(nearest,length(hit));
        }
    }
    return nearest;
}
// How far point p (in the face's cells) lies inside cell c (site `site`),
// to the nearest edge its neighbours draw, in cells (negative outside); and
// that neighbour.
struct BreakupEdge { inside:f32, toward:vec2f, across:vec3i }
fn breakup_edge(c:vec3i,site:vec2f,p:vec2f)->BreakupEdge {
    var e=BreakupEdge(1e9,vec2f(1,0),c);
    for(var y=-1;y<=1;y++) {
        for(var x=-1;x<=1;x++) {
            if x==0 && y==0 {continue;}
            let k=c+vec3i(0,x,y);
            let other=breakup_site(k);
            let n=normalize(other-site);
            let d=dot((site+other)*.5-p,n);
            if d<e.inside {e=BreakupEdge(d,n,k);}
        }
    }
    return e;
}
// How much of the slab still stands where the ray along d (part 4's camera
// space) meets it, on face `face` (0 top, 1 front): the cell it
// falls in, if that grain has not come away, softened over a pixel toward
// the neighbour across the nearest edge.
fn breakup_standing(d:vec3f,face:i32)->f32 {
    // Where the ray meets the face's plane, in its cells.
    let normal=select(-breakup_back(),-breakup_down(),face==0);
    let o=breakup_origin();
    let hit=d*(dot(o,normal)/dot(d,normal));
    let q=hit-o;
    let g=vec3f(breakup_grid());
    let size=breakup_size();
    let along=select(dot(q,breakup_down())/size.z*g.z,dot(q,breakup_back())/size.y*g.y,face==0);
    let p=vec2f((q.x/size.x+.5)*g.x,along);
    let c=vec3i(face,i32(floor(p.x)),i32(floor(p.y)));
    var nearest=c;
    var best=1e9;
    for(var y=-1;y<=1;y++) {
        for(var x=-1;x<=1;x++) {
            let k=c+vec3i(0,x,y);
            let e=p-breakup_site(k);
            if dot(e,e)<best {best=dot(e,e);nearest=k;}
        }
    }
    let site=breakup_site(nearest);
    let edge=breakup_edge(nearest,site,p);
    // A pixel's size across the edge, in cells.
    let at=breakup_scene(hit);
    let step=breakup_scene(breakup_point(face,p+edge.toward))-at;
    let cells=200.0/u.screen.y/max(length(step),1e-6);
    let now=breakup_seconds();
    let mine=select(0.0,1.0,breakup_release(nearest,site)>now);
    let other=select(0.0,1.0,breakup_release(edge.across,breakup_site(edge.across))>now);
    return mix(other,mine,clamp(.5+edge.inside/cells,0.0,1.0));
}
