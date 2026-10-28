# Part 4's ground

These notes cover the ground in modern part 4 (light on stone), where its two
sheets of light stand now, the slab's break-up into part 7's dust and the
slab's right end. They belong to the Rust
intro experiment, under the terms in [rust/NOTICE](../rust/NOTICE). The rest
of the part is described under Part 4 in [rust/README.md](../rust/README.md).

The ground, both its looks (water, the default, and a polished stone floor on
the preview's **D** key) and the sheets' new placement are all authored. None
of it is recovered from the original, which has no ground: below its plasma
and beside its logo the screen is black. Faithful HD and the original view
are unchanged. The water itself (its reflectance, swell and drops,
`water.wgsl`) is shared with part 3, whose objects fly over it, with the
tuning keys below; part 3's notes in [rust/README.md](../rust/README.md)
say where it lies there.

## Where it lies

The ground is now part 3's water, carried on: it lies level, tilted as part
3's lies (3 degrees by default, rising further off), as far below the camera
as part 3 left it at the slab frame, and from 186 to 200 seconds it drains,
eased, to 222 units below the camera, uncovering the slab; over the plasma's
fade out (219.33 to 222.87) it drains away, eased, to 40 units below the
slab's lowest edge, gone from under the slab by part 4's end
(`dots_water.rs`, `plasma.rs`'s `DRAIN` and `DRAINED`). The sheets stand in it, on the
water straight below the line where the slab's base plane meets row 113,
the plasma carried on down them below its last raster to the water
(`PlasmaPreview::sheet_rasters`). Part 4's mirror gain is 1, water as it
is, by default: this level water meets the camera near grazing, where it
mirrors enough. What follows describes the ground as it was first laid, at
the slab's base, which the sheets' placement still starts from.

The camera and the slab come from the fit to the logo, which is measured when
the preview loads (`plasma_stone.rs`). The focal length is 262.8 rows, the
block's tilt 31.3 degrees and its near top edge 504 units away. The block is
490 by 193.9 by 100. Its height of 100 is part 3's box's, an interpretation,
and its base is below the frame: the front face's bottom edge projects to
row 210.

- **The ground** is the plane the slab stands on: its base, 100 units below
  its top face and level with it, seen through the same camera. Its horizon
  is at row -60, above the frame. Seen straight back from the camera's axis,
  it reaches the bottom of the plasma's last raster (row 113, read from the
  code) 366.5 units behind the slab's near edge, 172.6 behind its far edge.
- **The sheets before:** their bottom edges were near the height of the top
  face. The front one stood 30 units behind the far edge and the back one 70
  further. On a ground at the slab's base they would have floated 95 and 29
  units above it. The ground below them projects to rows 121 to 141: under
  the plasma's edge, hidden by the slab in the middle and visible beside it.
- **The sheets now** stand on the ground, both on the line where it meets row
  113, so their bottom edges still project to the plasma's last raster. Their
  top edges are the same points as before (row -60), so on the screen each
  covers what it did, and what it shows there is unchanged: each point of a
  sheet takes the colour the original shows where it projects. Only their
  slope changed:
  - The front one leans 32.1 degrees toward the camera from upright. Before,
    both leaned 22.9 degrees and were parallel.
  - The back one leans 25.9 degrees, so the two open from the line like a
    book's pages, 70 units apart at the top as before and closing to nothing
    at the ground.
  - Both are wider (a half width of 881 units, before 709 and 829), so they
    still cover a 21:9 drawable at their new foot.
- **What that changes:** the sheets' lower parts are further from the slab,
  so its light and its polished top's mirror image change a little. On 1440p
  stills at full level, the top face's mean colour moved by up to about 5
  levels in 255, and by up to about 11 toward its far edge, where the mirror
  image changes most. That image now shows the sheets a little higher up:
  at the near edge about row 46 of the plasma (before about 64), at the far
  edge about row 86 (before about 108).

## Ruled out

- **Bringing the sheets down to the ground where they stood:** their planes
  meet it at rows 118 and 132, so beside the slab the plasma would have to
  reach up to 19 rows below its last raster, where the original shows
  nothing.
- **Parallel sheets, both standing on the ground:** the back one's foot would
  project 11 rows above the front one's. The plasma's bottom band would then
  show only one of its two copies.
- **Moving the sheets back whole, keeping their lean:** the top's mirror image
  would move further (to about rows 19 to 76).

## The water

- **The mirror:** the reflected ray finds both sheets and their glow, weighted
  by Fresnel's equations for unpolarised light on water (index 1.333: 2% head
  on, rising steeply toward grazing). This camera looks down 28 to 52 degrees
  onto the visible water, where water reflects only about 2 to 7%, which reads
  darker than the polished floor. So the mirror image carries an authored
  gain of 3 (`WATER_MIRROR`; 1 is water as it is). It is not faded with
  distance.
- **The swell:** four long directional waves, mostly coming from the sheets
  toward the camera, with wavelengths of 280, 175, 115 and 78 units and
  steepnesses of 0.026 down to 0.008. Their periods go with the square root
  of the wavelength, as deep water's do; the scale is chosen by eye, with the
  longest wave taking 7.5 s. The mirror image comes from the waves' normals.
  Each is a smooth function of the clip's time, so there is no noise and the
  water is the same however it is reached.
- **The streaks:** the slopes too small to draw are taken as a spread in the
  plane of incidence (0.025), over five mirrored rays with Gaussian weights.
  That draws the mirror image out into the vertical streaks of water.
- **Its body** scatters a little of the sheets' light back: of what enters,
  less what the slab keeps from it, so the water is a little darker in the
  slab's shade.
- **Flicker:** over five consecutive 60 fps frames at 205 s, the water's
  frame-to-frame changes stay within the plasma's own, since it mirrors the
  plasma. Its second differences peak at 35 levels, against the plasma's 45.
- **Drops** (their times and places authored, their rings physical): at
  the authored rate one every 3 to 6 seconds, and by default (chosen by eye)
  about four times as often, while the plasma is lit, from 2 s after it begins
  to light until 4 s before it is dark. The intervals and places come from
  a fixed hash of each drop's number, so seeking always shows the same
  drops. Each lands in view beside the slab, never behind it, and sets off
  a ring packet of capillary-gravity waves, `w^2 = g k + (sigma / rho) k^3`.
  The scale is authored at 6.5 mm a unit (the slab 3.2 m wide), with
  water's gravity and surface tension. By stationary phase (the
  Cauchy-Poisson problem), at radius r and age t the rings are the waves
  whose group velocity is r / t, on the gravity branch, so the longer ones
  lead. Inside the slowest group velocity's radius (27.3 units a second)
  there are none; that inner edge, a caustic, is taken at Airy's scale.
  The drop's disturbance is a Gaussian 4 units across, which damps the
  short waves. The rings thin as they spread and also fade over 3 s
  (authored), each drop lasting 4 s. Their slopes join the swell's in the
  normal. Rings finer than a few pixels are not drawn, and over
  consecutive frames they add no flicker of their own.
- **Tuning keys** (the preview's, each step x0.8 or x1.25 against the
  authored swell and rate; the defaults were chosen by eye with them):
  - **1/2:** the mirror's gain, from 1 (water as it is) to 8; 3 by default.
  - **3/4:** the swell's height (its slopes), down to exactly 0, a flat
    mirror, and up to about x15; x3.8 by default.
  - **5/6:** the swell's speed, about x0.26 to x15; x3.8 by default. The
    swell's clock is integrated on the CPU, so a change of speed does not
    move the waves.
  - **7/8:** the swell's size (its wavelengths, the periods following as
    deep water's do), about x0.26 to x15; x1 by default. Its waves re-form
    at the new size.
  - **9/0:** the drops' rate, down to exactly 0, none, and up to about x4.8
    (a drop every 0.9 s on average), where six drops at once, all the
    shader holds, would start to cut drops short; x3.8 (a drop every 1.2 s)
    by default. A change re-times the drops.
  - The window's title shows the current values.
- **Left out** (when the ground was the slab's base): the waterline was the slab's base, which is below the frame and
  hidden behind the slab. A ring of ripples from the slab was tried and left
  out. The water near the slab is hidden behind it, and further out the
  ring's slopes run across the line of sight, so they hardly move the mirror
  image; at 2.5 times the steepness it still did not show.

## The polished floor (D)

- **Material:** polished dark stone. Its albedo is 0.06 (linear) and its
  reflectance at normal incidence 0.045. Its polish (GGX alpha 0.10) varies by
  up to 60% with a mottling 36 units across. The same mottling varies its
  albedo by up to 35% and its mirror image by up to 25%.
- **The mirror image:** the reflected ray finds both sheets and their glow. It
  reads their light at a coarser level the further it goes, so the image is
  sharp at the sheets' foot and softer further off. It fades over 900 units
  of the ray and is weighted by Schlick's Fresnel, so it is strongest at
  grazing angles, toward the sheets. The sheets' bottom edges stand on the
  floor and are not faded, so the image meets them.
- **The sheets' light on the floor:** it is gathered over the same cells as on
  the stone, by their exact form factors. From that is taken what the slab
  keeps from each point: each side of the block the point sees, in three
  strips, each strip's form factor times the light the sheets send through
  it. The sheets lean over the floor, so it is lit almost evenly; on a floor
  this dark the mirror image dominates.

## In both

- **The slab in the mirror:** the reflected ray is marched against the slab
  too, but from this camera the slab's image cannot appear. The camera looks
  down about 31 degrees at a block whose base is below the frame, and every
  point of the ground in view, beside the slab or behind it, mirrors rays that
  travel away from it. Only the ground in front of the slab would mirror it,
  and that is below the frame.
- **Lighting:** the ground shows only by the sheets' light. The warm key light
  is taken as a spot on the slab and does not reach the ground.
- **The arrival:** the ground is dark while the slab comes out of part 3's
  flare and while the plasma is dark.
- **The fades:** the ground emerges as the plasma's level rises (187.51 to
  191.06) and fades with it (219.33 to 222.87).
- **The slide:** the original moves only the logo, by the line compare.
  Where part 7 follows, the slab breaks up where it rests into part 7's
  dust instead (`slab_breakup.rs`; rust/README.md, part 4). Without it,
  the slab sinks along the camera's vertical as before, so its far edge
  follows the line compare, and the ground and the sheets move with it, as if
  the camera rose. A ground that stayed put would cut the slab off at its
  level before it reached the bottom of the frame. By then the sheets are dark
  and the ground unlit.
- **The hand-over:** with the break-up, part 7's view is drawn from the
  slide's cue on, its light coming up from black 1.19 s early; without
  it, part 7 still opens from black at 225.43.
- **Unchanged frames:** compared on 1440p stills, the frames from the slab
  frame to the plasma's first light, and those of the slide, differ from
  before only on the slab's right end (below), and the two grounds give the
  same frames there. The keys' defaults keep the water's frames as they
  were before the keys, but for a few dozen pixels a frame off by one
  level (the gain and the clock now read from the controls) and where a
  drop's rings are.

## The break-up into part 7's dust

All authored (`slab_breakup.rs`, `slab_breakup.wgsl`): the original slides
the slab down off the screen from 2F00 and opens part 7 on black.

- **Where part 7 follows,** from the slide's first frame, 224.24 s, the slab
  stays where it rests and breaks up into part 7's dust. Without part 7
  (`--plasma`) it slides away as before (In both, above).
- **The grains:** its top and front faces are a jittered grid of cells, one
  grain each, a Voronoi tiling of each face, with a cell for every one of
  part 7's 211,224 motes (a few over: those grains float up and fade).
- **The edge** along which grains come away runs from the top face's far
  edge forward and on down the front face over 1.6 s, ragged by a smooth
  noise, each grain a little earlier or later.
- **Drawing it:** part 4's view keeps drawing the slab at rest into two
  targets of part 7's: the picture, how much of the slab still stands in
  alpha (each cell gone once its grain has, softened over a pixel at the
  cells' edges), and the stone's surface, its normal and albedo. Part 7's
  renderer makes those targets only while the slab breaks up. Part 7's
  view draws its own scene from the cue, lights the stone's surface with
  its beam and lamp glow (the carving's shadows left out) and draws the
  slab over its scene, behind the dust and the lit air in front of it.
- **The flight:** each grain, from rest where it was, floats up, carried by
  part 7's air, and eases onto the path of its mote over 1.6 to 2.2 s, then
  is that mote, drawn as the mote is. On the way its light turns from the
  slab's (as part 4's key light and the lighthouse light it, flickering as
  it tumbles in the key light) to a mote's, and its shape from its cell to
  the mote's disc.
- **The matching:** motes and cells are matched by where they show as part 7
  begins, both in 64 columns of equal count, left to right, each from the
  bottom up, so each grain rises to a mote above it, or level with it, in
  the same part of the screen.
- **Part 7's light** comes up from the cue, 1.19 s before part 7's first
  frame: the original's fade from black played that much early, held at
  its full level after, and before its first frame its colours' phase, and
  with it the beam, turns on at the pace of its first second.
- **The end:** the last grains have settled by 4.3 s after the cue, 228.54 s,
  before the words rise; from then on part 7's frames are as before, byte
  for byte.

## The slab's right end

The logo's light comes from the upper left, so the painting puts the slab's
right end in shade and outlines its right edge dark. Before, the carving
(where a face, unwarped from the logo, is darker than its two tones'
threshold) read that shade as cut stone. The edge margin, which keeps the
carving 4 units off a face's edges, then cut it off in straight lines. That
left a square pocket at the top face's near right corner, another at its far
right corner, and a straight raised strip down the right end of the front
face. The crack finder read the dark outline as a crack along the top face's
right edge, a straight groove that left a thin raised rim. The columns past
the logo's edge, and the outline, gave the last few texels a different colour.
The left end, painted in light, has none of this.

Now, on each face, carving that reaches the right margin is taken as that
shade and left uncut, and the cracks fade out over the 4 units before the
margin. In the margin, the stone's colour is mirrored from just inside it.
The stone's own tone, by which the painted light is flattened out of its
colour, is still taken from the face as first read, so the rest of the face
is unchanged.

This changes the slab's right end in every part-4 frame that shows it,
including the arrival and the slide. On 1440p stills, the changed pixels lie
on the slab between columns 1868 and 2226 of 2560; nothing else changes.

The look's constants are at the top of `plasma_light.wgsl` (`WATER_*` and
`FLOOR_*`). The sheets' placement is in `plasma_stone.rs` (`place_sheets`), as
is the right end (`carve`, `without_regions_reaching`).
