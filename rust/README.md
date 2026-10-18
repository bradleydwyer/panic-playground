# Rust player

The port implements all eight scenes, played in the original order (parts 1, 2,
6, 5, 3, 4, 7, 8), plus the STMIK sequencer and stereo sample mixer. It reads
the original assets and the unpacked programs' data tables but never executes
DOS code. You generate that data yourself from your own `PANIC.EXE`; none of it
is included here.

This directory is not covered by the repository's MIT License. See
[NOTICE](NOTICE).

## Setup

You need Rust 1.88 or newer. The desktop build compiles SDL2 0.38.0 from
bundled source, so it also needs CMake and a C/C++ compiler. On Linux, install
the development packages SDL needs for your display and audio system. The
headless build is pure Rust and needs neither SDL nor CMake.

The importer needs Python, Unicorn 2.1.4, Capstone 5.0.7 and Deark. It was
tested with Deark 1.7.3-1, revision
`8997059c4f32af79978fb6c83be7c1c26c259afa`. To build that revision and import
your data, run this from the repository root (you'll need Git, make and a C
compiler):

```sh
git clone https://github.com/jsummers/deark.git artifacts/tools/deark
git -C artifacts/tools/deark checkout 8997059c4f32af79978fb6c83be7c1c26c259afa
make -C artifacts/tools/deark
python3 -m venv .venv
.venv/bin/python -m pip install -r requirements-import.txt
.venv/bin/python tools/import_panic.py /path/to/PANIC.EXE \
  --deark artifacts/tools/deark/deark --out artifacts/local-data
```

On Windows, use `.venv/Scripts/python.exe` and a Windows build of Deark.
Everything under `artifacts/` is ignored by Git.

Your `PANIC.EXE` must have SHA-256
`a7ceeae290a7132880cd6ba616c2b0bb4001b2f7c83694f04052314fa26df6e7`.
The importer checks the hash of every file it produces before finishing, and
copies the demo's own notice to `files/README.TXT`. The output includes
original program code, so keep it out of Git. Importing it doesn't give you any
extra rights to use or share it.

## Run

```sh
cargo run --release --locked -- --data artifacts/local-data
cargo run --release --locked --no-default-features -- \
  --headless --data artifacts/local-data
```

Escape quits, Space pauses and F toggles fullscreen. Useful options:

- `--seek 180` fast-forwards to the three-minute mark before playing.
- `--mute` silences the audio but keeps the music driving scene timing.
- `--fallback` uses the original no-music scene schedules.
- `--seconds 10` stops after ten seconds.
- `--out artifacts/export` saves output there. In headless mode that's a WAV,
  a snapshot every ten seconds and a timeline CSV; the desktop player saves a
  screenshot of each scene and a run summary.
- `--fullscreen` starts in fullscreen.

Instead of `--data` you can set `PANIC_DATA`. With neither, the player looks
for a `data/` directory in the current directory or next to the executable.
The output of `panic_resources.py extract` isn't enough to run the player; use
the full importer.

## Experimental HD intro and scroller

The `panic-hd` preview covers the opening through the PANIC logo, and with
`--scroller` and `--rings` continues through parts 2 and 6. Build and run it
from the repository root after importing your data:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --modern
```

The modern mode renders extruded geometric models in perspective, with bevels,
reflections, surface texture, ambient occlusion, HDR lighting and bloom.
The logo moves from cool metal to incandescent metal on the right. Slow surface
ripples and flowing texture shift its reflections like semi-liquid metal, with
more movement at the hot end. Its starfield has 3,200 fine, round stars with
depth, colour and luminosity variation. From 12 seconds, an authored warp ramp
accelerates them into long, narrow radial streaks as the music builds.

The modern lighting also follows the soundtrack. While the clip is prepared,
the generated PCM is measured once per 70 Hz frame: overall energy, onsets,
bass and strong accents. The direct lights on the metal, the logo's glow and
bloom breathe with the energy and bass and pulse on hits, and the big downbeat
just after the logo lands re-strikes part of its white flash. This is an
authored effect; the music, its timing and its PCM are unchanged, and seeking
gives the same lighting as uninterrupted playback.

The opening still spells FUTURE CREW as individual letters fly past the camera.
Their perspective motion is fitted locally to the reference frames, with a
farther starting position, gradual fade-in and thicker block forms. The
letters are tinted glass: light refracts through each block, with internal
reflection and slight dispersion, and the rendered starfield shows through
them. Light trapped inside leaks out along the edges, so they read against
black space. The plate also fades in, then uses a continuous quaternion tumble
and forward motion, following the recorded sequence's two turns and approach.
This is an authored approximation: the original Rust decoder plays stored
animation frames, not 3D rotation equations. These are reconstructed forms,
not recovered original models.

The models also reflect the scene. The space backdrop, stars and doors are
rendered first, and on-screen reflections and refraction sample it. Elsewhere
a procedural sky of the same character fills in, with stars that stream
radially and stretch with the warp, so the tumbling plate reflects the warp
too. Motion blur uses a virtual exposure of about 1/100 s: the letters and
plate are traced at several moments within it, in proportion to how far they
move on screen. Silhouettes and bevels are supersampled where a per-pixel
coverage probe finds them. The glass, sky, exposure and sampling are authored;
the original has none of them.

The closing panels follow the Rust door timing directly, interpolated between
70 Hz ticks. Their geometry follows the palette maths of `Intro::doors`: the
open rows end at door/2 and the brighter rim row sits at door×8/14. The modern
renderer reads that dark strip as each door's inner face seen in perspective
and the rim as its front lip; this is an interpretation. The lip stops where
the original's rim ends, about 0.22 of the height from the centre, and becomes
the logo screen's blue rail. The bevelled lip, contact shadow, cool light
spilling from the warp through the narrowing gap, the bloom pulse as the doors
settle, and the quick fade of the stars into the recessed logo panel are all
authored.
The soundtrack, cues and scene boundaries use the existing port unchanged.
The logo appears at about 14.57 seconds; the preview holds there and ends its
audio after another 4.91 seconds. The logo material keeps animating after the
audio ends; Space pauses it too. End then Space lets you watch the logo directly.
Later demo scenes are outside this preview.

Add `--transition` to continue through the square wipe into part 2. It reads
the 16 by 10 activation order from your local data and follows `Scroller`'s
200-tick hold and 169-tick wipe at 70 Hz. That places the wipe at about
18.34–20.76 seconds, with uninterrupted original music. The preview then holds
for one second, ending before the scrolling text begins. `--transition`
selects modern mode automatically and supports `--split`; `--laser` is kept as
an alias. `--seek 17.8` starts just before the wipe.

The modern view draws the wipe as a laser cut. Cutting points trace each tile's
perimeter over 0.21 seconds, leaving a narrow tab on the top edge. The plate
tips slightly around that attachment until it releases at 0.32 seconds, then
accelerates directly away from the camera, like a floor giving way beneath an
overhead view. Its centre has no lateral drift after release. Perspective makes
the intact pieces shrink; depth haze blends them into the backdrop before they
are removed at 1.10 seconds. Glowing edges, sparks and illuminated smoke shift
from cool blue on the left to hot orange on the right. Sparks come from the
moving cutting point and final severing flash; smoke rises towards the camera.
Nearby cuts light the brushed metal underneath. Attachment positions vary
deterministically between tiles. The pieces keep their physical size and
vanish during the existing final hold. Several cuts overlap to retain the
original activation cadence. The logo's surface keeps moving during the wipe.

Earlier trials, a curling burn and a version with falling, burning sheets,
have been removed from the code. They remain in the Git history: commits
`4c43ab2`, `00a63d5` and `0fe8b55` introduced them.

The geometry, gravity, laser treatment and particle motion are authored effects.
The lighting uses analytic approximations; this isn't a physical simulation.
The original destination is a plain light screen at this point;
the modern view gives it a dark brushed-metal finish.
Use `--scroller` to continue into the glass scroller.
Original/comparison mode shows the actual reference wipe, and faithful HD
uses the original pixels during this transition.

### Scroller experiment

```sh
cargo run --release --locked --features hires --bin panic-hd -- --scroller --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller --scroller-backdrop wall --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller --wall-waves 0 --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller --faithful --seek 27
```

By default the scroller uses glass lettering with studio lighting, on the
arc described below, over the chrome bump wall (shape 0.5, strength 0.6)
with its waves; the
glass has approximate tinted transmission and reflective edges. The vertical
sine overlay passes in front of the lower row and behind the upper row. It
follows the arc, with its depth crossover in the gap between rows. The
existing laser transition reveals the wall; the preview ends after part 2's
fade. `--scroller glass` is still accepted.

Over the bump wall (`chrome` or `wall`), the glass lets through a blurred
view of the wall's light behind each letter; the `chamber` shows through it
refracted but unblurred. Either is tinted blue towards the left and orange
towards the right. Seen through a letter, its own soft shadow on the backdrop
is lighter, as that light came through the glass. Lamp light gathered by
each face leaves in a band just inside its bevel, so the letters glow along
their edges while the bevel line stays crisp, and the sine text's light
spreads through the glass close to it. Part 2's whole scene then has an
exposure of 1.2 before the shared tonemap. These are authored looks, set by
constants in `scroller.wgsl` (`GLASS_LIGHT`, `GLASS_RIM`, `GLASS_EDGE_ROWS`,
`FILAMENT_IN_GLASS`, `GLASS_SHADOW` and `SCROLLER_EXPOSURE`), not a model of
the original.

Glass was chosen from three presets. The other two were `ribbons`, coated
titanium lettering with curved row ends, and `sculpted`, the same material on
straight extrusions. They have been removed from the code but remain in the
Git history: commit `5e717ff` introduced them, and `ad08523` is the last
commit with them.

`--scroller-backdrop` selects the background: `chrome` (default), `wall` or
`chamber`. Press **B** to cycle them while playing. The original's background
is one 8 by 8 pixel tile repeated across the screen. Every tile uses the same
64 palette entries, and each frame `palette()` loads them from an 8 by 8 window
of a tile image, so the whole wall slides as the window moves. Two oscillators
(`xx`, `yy`) move the window about ±144 pixels across every 5.5 seconds and
±100 pixels down every 4.6 seconds. Alternate frames add half a tile to cancel
the hardware scroll, so the wall doesn't travel with the text. The preview
records the oscillator and image for each frame and interpolates the position
at display time, so the wall moves in fractions of a pixel. An ignored test
checks that they reproduce every background pixel of the original's scroll
frames. Until the first `palette()` call, when the text scroll begins at about
22.2 seconds, the original still shows the intro's flat grey there and its
oscillator stands still. The HD view shows the tile wall instead, already
moving: an authored lead-in, not original behaviour. For every frame before
that call, the original oscillator is run backwards from its starting state
(`xx` 0, `yy` 12800, speeds 384 and 0), one exact inverse of its update per
frame. The laser cut, from about 18.3 seconds, reveals a wall that is already
swaying, and the motion arrives at the original's first position with its
speed, then continues with the original's own values: no jump or kink. The
ignored test checks that every frame's acceleration is the oscillator's 4/128
pixel per frame, across the start too. `chamber` shows the moving
tiles dimly inside a dark, rippling chamber.

`chrome` and `wall` are authored treatments: chrome bumps on the original tile
grid, 40 by 25 tiles in the 4:3 frame, continued across the wide screen.
`wall` tints them with the tile image; `chrome` leaves them plain. `--wall-shape`
(0 to 1, default 0.5; **[** and **]** in steps of 0.1) runs from a low cushion
that fills the tile, like the original's shaded square, to a round hemisphere.
Each sample intersects the bumps it can reach analytically, with a ray /
rounded box test adapted from Inigo Quilez's MIT-licensed code (see
[NOTICE](NOTICE)). A dense, bright
grid moving this fast reads as op art, so the wall is treated as a defocused
background: eight samples per pixel spread over a small disc and over a
1/100 s exposure blur it along the oscillator's own motion. The metal is rough,
with broad, dim reflections of a studio surround and the letters' lamps, and
its relief is pulled towards a flat tile's level. It is kept dim overall, with
a gentle vignette towards the corners, so the letters stay the brightest thing
on screen.
Softened reflections of the letters themselves come from a short march through
the letters' shapes, once per pixel, over the depth the rows' arc occupies in
front of the wall. The letters' soft contact
shadows fall on the wall as they do in the chamber. Through the glass letters
the wall shows only its light at low frequency, averaged over a few bump
normals, never its tiles, so the letters don't look textured. `--wall-strength` (0 to 1,
default 0.6; **-** and **=** in steps of 0.1) sets how present the wall is.
All of this is authored, not recovered from the original.

The `chrome` and `wall` backdrops have waves. They are on by default; **W**
turns them on and off while playing, and `--wall-waves 0` starts without
them. The original's wall is flat, an image moved by palette changes, so the
waves and their light are authored, not recovered. A swell about 15 tiles
from crest to crest rolls across the wall every 5.5 seconds, varied along its
crests by a slower, longer one. Their height swings too, from the set depth
through flat to its opposite, the crests sinking into troughs, and back again
every 7.36 seconds: about two of the tune's patterns at their pace in part 6,
but not timed to them. The waves are anchored to the screen, and the wall's
sway carries the bumps through them. They ease in from the laser cut's first
opening to the start of the text, and settle over the 1.5 seconds before part
2's fade. They depend only on the time, so seeking, pausing and captures
agree.

With the waves on, a grazing light of the wall's own lights it from the left,
the scene's cool side, a little above the horizontal and 18 degrees over the
wall. The crests throw shadows across the troughs, the bumps on the slopes
facing the light catch it, and each bump shades its neighbour. As the light
strengthens it takes over from the studio light, so the wall's overall level
stays about the same and the shadows read. It lights only the wall: the
letters' lamps and the studio are unchanged, and without the waves nothing
changes at all.

Two controls adjust them while playing, each in steps of 0.1, and both
appear in the window title:

- Depth, how far the swell's height swings each way: `--wall-waves 0.5` sets
  it, from 0.1 (barely there) to 1 (far too deep), 0.5 by default; **,** and
  **.** step it. At 0.5, at the ends of its swing, the swell rises and falls
  1.3 tile widths, about six times the bumps' own height.
- Light, the side light's strength and so its shadows': `--wave-light 0.5`
  sets it, from 0 (the studio alone) to 1 (stark), 0.5 by default; **;**
  and **'** step it.

Each bump rests rigidly on the waves at its own tile's centre, tilted with
their slope there, and stays on its tile; every pixel and sample sees the same
bump at that moment, so the intersection stays analytic. Each sample finds
the waves at its own time in the 1/100 s exposure. The backing between the
bumps is the waves' tangent plane at the pixel, and that plane, widened by how
far the waves curve away from it, bounds the few tiles a ray must test. The
crests' shadows come from a short march along the light across the waves'
own surface, once per pixel and softened over the light's breadth. The bumps'
shadows come from a ray towards the light through the bumps within a tile or
so, on every other sample, each aimed at a different part of the light, so
their edges are soft too. The side light follows the waves' slope more than
each bump's, so the bumps don't sparkle. With the waves on, the wall costs
about 1.5 to 2.5 ms more per frame at 2560 × 1440 on an Apple M5 Max.

`--scroller-lighting musical` uses relative loudness and bass/treble
onsets from the original PCM. Bass pulses the blue/orange lights, their metal
reflections and the small lettering; treble adds short white highlights.
These are authored audio cues, not recovered tracker events. The levels adapt
to loud passages so the response retains contrast. They come from the same
causal analysis of the PCM as the intro's music lighting, computed once per
clip: the bass cue reads the band below about 150 Hz, the treble cue the band
above about 1.8 kHz. `--scroller-lighting studio` (default) holds the lights
steady. Press **L** to toggle lighting while playing
or paused. `--split` compares the
modern view with the locally rendered original. Omit `--seek` for the full
intro, or use `--seek 18` to start at the transition.

The letter shapes come from the original data, not from screen frames. When
the clip loads, `Scroller::hd_font` reads the text, the large font (each
character's 26 four-pixel columns through the descriptor table, exactly as
`font()` blits them), the sine overlay's x table and its row codes. Each
character's letter pixels become a distance field at four samples per source
pixel, in an atlas built in memory. A small blur before the outline is traced
rounds single-pixel stair steps; this is an interpretation of the raster, not
recovered vector artwork. Distances are measured on the original 4:3 display,
where a source pixel is 5/6 as wide as it is tall, so the letters keep their
original proportions: the O is as tall as it is wide.

Positions are read from the scroll counters, not the pixels
(`Scroller::hd_scroll`). Both rows keep a strip of text columns at fixed VGA
addresses and move the display window across it, four pixels per 70 Hz frame:
the upper row shows the text from its start moving left, and the lower row the
same text from its end moving right. The text holds the phrase twice, and
part 2 ends when the upper row reaches the middle of the text (or at a music
position), so each row shows one copy; both read normally. The lower row is the upper row run
backwards in time, not a mirror image. The sine text is a column of 12-dot
rows, one every four lines, moving up a line per frame; each row's x position
comes from the table, which `overlay()` only ever reads at even words. The
preview interpolates these positions between frames, so each frame's position
is shown exactly when that frame appears and the text glides in between. An
ignored test rebuilds every pixel of both rows and every overlay dot of every
part-2 frame from these tables and positions and compares them with the
reference. It skips two one-frame artefacts of the original's raster tricks
that the preview doesn't reproduce: the overlay row drawn at the raster split
with the wrong offset, and one column at the lower row's right edge when the
window wraps.

Both rows follow one authored arc of constant curvature: a vertical slice of
a cylinder whose front touches the screen plane at the centre. The letters
wrap round it at their original size, measured along the surface, so the
centre reads like the original and the rows turn away steadily towards the
sides. The radius is 1.55 times the screen's aspect ratio (2.76 units at
16:9, where the picture is 2 units tall), which puts the cylinder's edge-on
silhouette just past the left and right edges. Beyond the 4:3 picture the
rows show the text that's about to enter or has just left; letters fade as
they turn edge-on. The sine text's depth follows the same arc, and the
contact shadows and reflections are projected from it. The sine text also
runs past the top and bottom of the 4:3 picture to the screen edges, where
the original stops at its 200 lines. The overlay's dots are joined into
blocky strokes, and its displacement follows a smooth curve through the table
with the rounding to whole pixels smoothed out. All of this is authored.
Each frame, the GPU renders both distance fields once into a texture; the
marched letters then sample it.

An authored ring layout, with one whole ring of letters per row seen from
between them, was tried as an alternative to the arc and archived. It was
removed from the code, with the second ray-march layer only it needed, but
remains in the Git history: commit `eedbbc8` introduced a single ring,
`424a4df` replaced it with two rings fading at a seam at the back, and
`21e1f7a` is the last commit that still has them (`--scroller-layout ring`
or **G**). `--scroller-layout rows` is still accepted.

Letters and the sine text fade in over part 2's first 0.8
seconds; the text itself arrives later, as in the original, so this only
softens the first sine row at the screen's bottom edge. At the end the
whole scene fades with the original's palette fade. On the arc, letters
enter and leave beyond the screen's sides, fading as they turn edge-on.

`--faithful` (or **M**) shows faithful HD: the original backdrop, with the
original letters and overlay dots drawn over it at display resolution and
moving continuously. The letters keep the original greys from the font
pixels, interpolated between them. The backdrop is the original picture with
its letters replaced by the tile pattern eight pixels away, and its dots
removed. Only the smoothing is new.

The tile wall's oscillator and image are precomputed per frame, so seeking
and exports remain deterministic.
Materials, the arc, depth and light placement are authored. Contact
shadows and reflections are approximations; glass is not a full multi-bounce
optical simulation. All fields are generated locally in memory.

| Key | Action |
| --- | --- |
| Space | Pause or resume |
| Left / Right | Seek one second |
| Home / R | Replay from the start |
| End | Pause on the finished logo |
| M | Switch modern 3D / faithful HD |
| Tab | Toggle the original rendering |
| C | Compare original on the left with the selected HD mode on the right |
| L | Scroller lighting: studio or musical |
| B | Cycle the scroller backdrop: chamber, wall, chrome |
| [ / ] | Bump shape for the wall backdrops |
| - / = (keypad - / +) | Strength of the wall backdrops |
| W | Waves on the wall backdrops, on or off |
| , / . | Depth of the waves |
| ; / ' | Strength of the waves' side light and shadows |
| 1 to 5 | Part 6 style: faithful-plus, glass, pool, room, volume |
| F | Toggle fullscreen |
| S | Save a PNG to `--out`; existing files are preserved |
| P | Show or hide the frame-rate counter |
| V | Toggle vsync |
| Escape | Quit |

### Part 6: Rings

```sh
cargo run --release --locked --features hires --bin panic-hd -- --rings --seek 42
cargo run --release --locked --features hires --bin panic-hd -- --rings --faithful --seek 50
cargo run --release --locked --features hires --bin panic-hd -- --rings-style glass --seek 57
```

`--rings` continues past part 2 through part 6, the interference rings and
flashing symbols, to the first frame of part 5, with the music unbroken.
`--rings-style` chooses its style and implies `--rings`; **1** to **5** switch
styles while playing, and the title shows the one in use. `--faithful` (or
**M**) shows faithful HD, and `--split` (or **C**) the original on the left,
as in the earlier parts. Part 5 itself is outside the preview: the clip ends
on the PSTART picture as part 5 begins, at 81.27 seconds, and holds there.

Read from the port, and checked against the engine on every frame by an
ignored test, part 6 runs like this (the engine's `SEQUENCE` plays it after
part 2). It begins at 42.23 seconds, the moment part 2's palette fade ends,
on a black screen that waits for the music to reach order 6, row 0. The
first symbol appears at 43.41 seconds. The symbols are 16 hand-drawn masks
decoded from M.SUM into planes 2 and 3; the lit one changes on the tracker's
rows, about every 8 frames, and the original lights it in yellow on even
frames only, a 35 Hz flicker. The rings are two layers of the same concentric
bands from M.DAT, copied each frame into a 256 by 184 window, circling in
opposite directions about every 2.09 and 1.67 seconds. Their palette shows
grey 48 where only the first layer is set, grey 32 where only the second is,
and black where both or neither are. They fade in over 64 frames from 50.33
seconds (the fade counter starts at -100 at 07:1F, so the 08:00 downbeat
lands mid-fade), fade out from 72.99 to 73.89, and the symbols carry on alone
until the PSTART picture fades in over 64 frames from 80.36.

The preview keeps part 6 as compact state per frame, read from the scene's
counters through `Rings::hd_state` and `Rings::hd_offsets`, rather than its
pixels, which would add about 177 MB. The original frames for `--original`
and `--split` are replayed on demand: the scene and video memory are kept
every 64 frames (about 20 MB), and the port's own `Rings::step` runs forward
from the nearest checkpoint, one step per frame during playback. The ignored
test checks that every replayed frame equals the engine's.

Some of this is interpretation. The rings use an analytic model of M.DAT's
bands, fitted when the data loads: 7-pixel bands round a centre found from
the image's symmetry, with the first edge at about 6.7 pixels. It matches
98.2% of M.DAT's pixels and 98.1% of the ring pixels part 6 shows; the
misses lie on band edges. The bands are circles in source pixels, so, as in
the original, they look 1.2 times taller than wide at 4:3. The ring centres
are exact at each 70 Hz frame and follow a Catmull-Rom curve between them.
The original shows each frame's rings one frame late (its pages are double
buffered), and about 6% of its frames, at each page advance, show a page
drawn one or two seconds earlier. The preview always shows the current
rings, so it skips that artefact; the ignored test lists those frames. The
symbols' outlines are smoothed as the scroller's letters are, for faithful
HD. All of this is built in memory when the preview starts; nothing derived
from the data is stored.

For the neon, the symbols are redrawn rather than traced: an authored
recreation that keeps where every stroke, crossing and end lies in the
original but not its raster wobble. Each stroke's centreline comes from
thinning the smoothed stroke, with short spurs removed, and is split into
the separate runs described below. A closed run that is nearly a circle
becomes a true circle. Any other run is reduced to its key points (wobbles
under about a row go). A stretch between sharp turns that strays from a
straight line by less than 2 rows, or a twentieth of its length, is drawn
dead straight; a curved one that a circle fits closely becomes an exact arc,
and freehand curves become smooth splines through the key points; and each
sharp turn becomes a short bend, as glass tube is bent. A short overshoot past a corner at a run's end
is dropped. The atlas measures distances exactly to these curves.

Faithful HD shows the original's look at display resolution: the analytic
rings in the original window with its two greys and XOR, antialiased; the
symbols as smooth strokes at half strength in the original's yellow, which
is the average of its flicker in linear light (the random blue component
averaged too); and the PSTART picture smoothly upscaled, fading in DAC units
as its palette does.

The styles are authored experiments. Faithful-plus, the default, keeps the
original's moiré exactly, antialiased and blurred over a 1/100 s shutter so
it doesn't strobe, under a faint print grain. The lit symbol is a yellow neon
tube along its stroke's centreline, just above the rings, lighting them, and
the shared HDR bloom makes it glow. Following the owner's direction and
photographs of neon signs, each tube is a clear glass tube about as wide as
the original stroke, with a column of glowing gas down its middle: white
hot along the centre once tonemapped, where a line of sight crosses the most
gas, and a saturated amber yellow at its edge, warmer than the original's
lemon. The glass carries a little of that light, gathers a bright line along
each wall and catches a lamp's highlight, and the rings show through it,
bent. As in a real sign, each tube is a single run that bends but never
branches: where the original's strokes meet or cross, the two branches that
continue each other most nearly straight stay one run, and the others stop
short of it with a gap of about a tube's radius, glowing to their rounded
ends (their electrodes bend away behind the sign; `NEON_ELECTRODE` shows
them instead). Fragments shorter than a tube's width go. A haze of
scattered light follows the tubes' shape.
The tubes light the rings and a dark backing behind them, close by and in a
wide pool. The neon light is an approximation: a prefiltered map of the
tubes, not traced light. At 2560 by 1440 on an Apple M5 Max, with other work
sharing the GPU, faithful-plus took 0.6 to 1.25 ms a frame over repeated
runs, including bloom and tonemapping, and faithful HD 0.15 to 0.5 ms.

The other styles keep the original's ring centres, bands, fades and symbol
timing through the shared helpers; everything seen is authored.

Glass (**2**) turns each ring layer into a clear glass plate whose set bands
are raised rods, with layer 0's plate in front of layer 1's. Each plate is
lit through its own edge, cool for the front plate and warm for the back
one, so its rods glow from inside, brightest along their walls; the back
plate is seen through the front one, bent by its rods. Where rods of both
plates cross, an authored rule turns their light aside, so crossings stay
dark as in the original and the moiré reads the same way. The symbols are
neon tubes in front of the plates, lemon yellow here rather than the shared
amber so they stay apart from the warm plate; the rods reflect them, and
their light passes through the plates onto a dark backing. While the lamps
are off, the neon still catches the glass faintly. `GLASS_LIGHTING` in
`rings_glass.wgsl` switches the plates to clear glass with no colour, or to
both plates lit blue from the left and warm from the right. The plates fill
the whole screen, widescreen included, rather than the original's 256 by 184
window, and the tubes' own glass is clear, catching the lamps and showing
the rings bent through it. It took 1.2 to 2.6 ms a frame.

Pool (**3**) shows the rings as liquid chrome seen from above. The two ring
layers are two wave sources at the original's ring centres, with its band
spacing; the second layer's waves are inverted, so the surface lies level and
dark where the original is black, and rises or sinks where only one layer is
set. The waves fill the whole screen, widescreen included, rather than the
original's 256 by 184 window. A low lamp hangs over each source, cool over
the first and warm over the second, so each source's waves catch its colour
and their crests shade the waves beyond. The lamps come up and go out with
the original's ring fades, but the waves stay: before and after the fades
they are seen only by the neon symbols hovering just above, whose
reflections they bend. (`POOL_DARK_WAVES` at 0 lets them settle into a still
black mirror between the fades instead.) It is exposed over a whole 60 Hz
frame so the fringes don't flicker. It takes about 1.8 ms a frame at
2560 × 1440.

Room (**4**) keeps part 2's chrome bump wall, seen with the scroller's camera,
and turns the lights out: the wall fades with the scroller and stays black
until the first symbol. The lit symbol hangs a few rows in front of the wall
as neon tubes, which light the bumps turned to them and show in their
chrome. Two gobo projectors above the camera cast the ring layers onto the
wall, layer 0 in cool blue and layer 1 in warm orange, aligned so that the
wall's flat backing shows the original's bands exactly; the bumps take the
pattern displaced towards each projector, shade and shadow themselves, and
each tube casts a shadow in each beam. Lamps add, so the original's dark
overlaps come from an authored subtraction rather than from the light. The
bump shape and wall strength keys apply, and musical lighting (**L**) lets
the lamps flare on the bass. It took 4.1 to 4.5 ms a frame on a quiet GPU.

Volume (**5**) turns the two ring layers into spherical waves in haze. Each
layer's centre is an emitter in the screen plane, and shells at the
original's band radii hold the haze, so the plane cuts them in the
original's rings and the moiré extends into depth; layer 0's shells are
cool, layer 1's warm, and where both are set the haze is dark, as the
original's XOR palette is. Each band's haze is lens-shaped, thin at its edges
so the bands stay sharp from any angle, and deepens round the emitters into
nested spheres; a side light from the upper left shades and shadows it, and
the neon symbols light the shells near them. The camera keeps the original's
view while the rings fade in and out, and drifts up to 24 degrees off the
axis while they are at full intensity, an authored move that depends only on
the clip time. The haze is ray-marched at half resolution in closed form,
without noise, in about 2 ms a frame.

### Adding a part-6 style

Each style is one WGSL file, `src/hd/rings_<name>.wgsl`, which defines two
functions:

```wgsl
fn style_color(px:RingsPixel)->vec3f    // linear HDR radiance for this pixel
fn style_prepass(px:RingsPixel)->vec4f  // optional half-resolution pass
```

The renderer builds the style's module from the shared controls,
`rings_common.wgsl`, `wall.wgsl`, the style's file and `rings_style.wgsl`,
whose entry points call these two functions. The style draws into the modern
renderer's HDR target; the shared bloom and ACES tonemap follow. Prefix the
style's own names with its name (`glass_`, `pool_` and so on); the naga test
validates every style's module. Edit only the style's file and, in
`src/hd/rings.rs`, the style's own `StyleInfo` constant and controls function.
Set `prepass: true` there to have `style_prepass` drawn first, at half the
resolution each way, into a texture that `prepass_at(uv)` reads. Return
values for `u.rings_style` (four vec4s, zero by default) from the controls
function, if the style needs anything computed on the CPU at each moment.

`px` gives the pixel in three spaces: `uv` (0 to 1, y down); `xy`, the screen
(y from -1 to 1, x scaled by the aspect); and `source`, the original's 320 by
200 pixels, continuing beyond the frame on wide screens. `scene` is the same
point in source rows on the 4:3 display, y up, centred: an isotropic space
for 3D, with the default camera (`EYE`, `camera_ray`) as the scroller's,
which is 100 rows to the scroller's world unit. `pixel` is one target pixel
in source rows.

The helpers in `rings_common.wgsl` include:

- Timing: `rings_seconds()`, `rings_phase()` (0 waiting, 1 rings, 2 picture),
  `ring_intensity()`, `picture_level()`, and the symbol windows
  (`neon_mask()`, `neon_previous()`, `neon_on()`, `neon_previous_on()`,
  `neon_since()`, `neon_until()`). The symbol switches are the original's
  own cues, on its tracker rows. Before 43.41 seconds the original is black.
- Music cues measured from the PCM, as the intro and scroller use them:
  `cue_energy()`, `cue_onset()`, `cue_low()`, `cue_accent()`, `cue_level()`,
  `cue_bass()`, `cue_treble()`, and `musical_lighting()` for the viewer's
  studio or musical choice (**L**).
- Rings, per layer (0 is the grey-48 layer, 1 the grey-32 one):
  `ring_centre`, `ring_radius`, `ring_band` (signed distance to the nearest
  band edge), `ring_cover` (antialiased coverage), `ring_phase`, `ring_dir`,
  `ring_window` and `ring_xor` (the original's greys). Pass `RING_FAITHFUL`
  for the original's tall rings or `RING_ROUND` for round ones, and a
  shutter position from 0 (now) to 1 (1/100 s earlier) for motion blur.
- Symbols: `symbol_field` (the outline's signed distance, the distance to
  the centreline, the stroke's half-width and the distance to the stroke's
  nearest free end), `tube_distance`; the tubes as `NeonGlass` samples
  (the light they add, how much glass covers the pixel and where the scene
  behind shows through it) from `neon_tube`, `neon_tube_at` and
  `neon_glass`, composed over a scene with `neon_over`; `neon_emission`
  (just their light) and `neon_haze` (the glow in the air round them),
  `neon_irradiance` (their light on a surface with a normal),
  `neon_reflection` (their light along a ray, blurred by roughness),
  `neon_fluence` (light in a volume) and `neon_plane_hit`, with the tubes in
  a plane `z = height` of the style's choosing. `SYMBOL_YELLOW` is the
  original's average tint.
- The PSTART picture (`picture_color`), `tonemap_inverse` to place original
  colours exactly before adding light, `rings_reference` (a plain modern
  rendering to start from) and `prepass_at`.
- From `wall.wgsl`, the scroller's bump wall: `wall_trace` and its
  helpers. During part 6, `u.backdrop` holds the oscillator as part 2 left it
  and the wall's shape, `u.backdrop_motion.z` its strength and
  `u.next_palette` the tile image; the waves are off.

`style_color` returns linear light before tonemapping. After 80.36 seconds
it should show the PSTART picture, which part 5 opens on. Check a style
against the others with `tools/look_check.py`, which has a shot of each.

`--fps` starts with the frame-rate counter in the top-left corner, showing
frames per second and milliseconds per frame. With vsync the rate can't exceed
the display's refresh rate; `--no-vsync`, or V, shows how fast frames can
actually be drawn. Captures never include the counter.

Without `--modern`, the faithful HD mode retains the original painted artwork,
using interpolated contours and shading. Its detail is limited by the source
raster; it is not a replacement 3D model. `--faithful` selects it even with
`--scroller` or `--transition`. `--original` displays the reference
pixels, including the original starfield. All modes share the same timeline.

The desktop renderer uses the window's native drawable resolution, including
Retina scaling. `--size` sets PNG resolution and the starting window's aspect;
the initial window is bounded to fit a 1280 by 720 logical area. For a 4K image:

```sh
./target/release/panic-hd --modern --headless --logo --size 3840x2160 \
  --out artifacts/intro-logo-4k.png
```

Use `--seek 5.2` instead of `--logo` to capture a fly-by. Headless HD rendering
still requires a GPU and the `hires` build dependencies. `--data` and `PANIC_DATA`
override the preview's default `artifacts/local-data` path. All original data and
generated images stay local, outside Git.

The default HD tests use synthetic data. They also parse and validate every
WGSL shader with naga and check that the one WGSL declaration of the shared
uniform block, which `renderer.rs` prepends to each shader, matches the Rust
`Uniforms` layout field by field. Separate ignored tests check the
intro's reference pixels and PCM against the existing engine, sampled fly-by
projection heights, the transition's complete reference frames, audio and
tile schedule, the scroller's letters and positions against every part-2
frame, and part 6's state, replayed frames and PCM against every part-6 frame:

```sh
cargo test --release --locked --features hires
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked \
  --features hires reference_timing_audio_and_flyby_projections -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires transition_matches_engine_frames_audio_and_schedule -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires scroller_glyphs_and_positions_rebuild_every_reference_frame -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part6_state_matches_every_engine_frame -- --ignored --nocapture
```

The fitted trajectories approximate the reference projection; they do not prove
the original camera or model parameters. No claim of complete reconstruction is
made. This preview has been exercised on Apple Silicon with Metal; other GPU
backends remain unverified.

## Tests

The normal suite needs no original files:

```sh
cargo test --release --locked --no-default-features
```

Tests that compare against original data or recorded reference output are
marked `#[ignore]`. They read their inputs at run time rather than embedding
them. To run them, point `PANIC_DATA` at your imported data and
`PANIC_FIXTURES` at a directory containing `tests/fixtures/` and
`testdata/567/` reference files:

```sh
PANIC_DATA=/path/to/imported-data PANIC_FIXTURES=/path/to/reference-rust \
  cargo test --release --locked --no-default-features -- --ignored
```

The importer doesn't create those reference files, and they aren't distributed
here. Don't commit any binary fixtures, including audio.

### Look check

`tools/look_check.py` renders a fixed list of shots with `panic-hd --headless`
and compares them with an earlier set, to catch unintended changes to the look
while the shaders change. It needs only the Python standard library and your
imported data, and writes everything under `artifacts/look-check/`, which Git
ignores. Its 35 shots cover the modern intro letters, the plate and doors, the
logo, the laser cut, the scroller with each backdrop and lighting mode, part 6
through its phases and in each style, faithful HD and the original view, at
960 × 540. Their times are authored test
points, not cues from the original. `list` shows them, and `--only` selects
shots or groups by name, with wildcards (`--only scroller,laser` or
`--only 'intro-7*'`).

```sh
python3 tools/look_check.py render baseline --bin /path/to/older/panic-hd
python3 tools/look_check.py check baseline   # renders "current", then compares
python3 tools/look_check.py compare baseline current
```

Each set records the binary and its SHA-256, `git describe` for the checkout
the tool ran in, the GPU, each shot's flags and the size. Data comes from
`--data`, `PANIC_DATA` or `artifacts/local-data`. `--size` and
`--extra='--wall-strength 0.8'` apply to every shot; `check` reuses the base
set's size and extra flags unless you pass them. A set can be named or given
as a directory path, but `render --force` and `check` only ever replace a
directory that holds a set. A shot that needs a flag missing from the binary's
`--help` is skipped, so older builds still give a partial baseline. A shot
skipped in the new set counts as a failure, because the build under test has
lost it.

For each shot the comparison reports the largest channel difference, the mean
RGB difference (both in 1/255 steps), and how many pixels differ at all and by
more than the tolerance. A shot is *identical*; *noise*, when no more than
0.1% of its pixels differ at all, no more than 0.01% by more than 2/255, and
the mean stays at or below 0.25/255; or *changed*. A faint change over a
broad area is therefore a change even when every step is within 2/255.
`--max-differing`, `--max-changed`, `--tolerance` and `--max-mean` adjust
those limits. Changed shots, shots the new set skipped or failed to render,
and size mismatches give exit status 1. Each shot that differs gets a heatmap,
noise included, and changed shots go on a sheet of before, after and
difference panels, in `artifacts/look-check/reports/`.

On an Apple M5 Max the captures were deterministic: two runs of one build, and
builds of the same source from two checkouts, matched byte for byte.
Reassociating the arithmetic in one scroller expression moved a single pixel
by 1/255 in each of two shots, which counts as noise. Raising the wall
strength from 0.6 to 0.61 moved pixels by at most 2/255 but over 19% to 92% of
each shot showing the bump wall, and those count as changed; at 18.6 seconds,
where the wall has barely appeared, only 30 pixels moved by 1/255, which
counts as noise. Sets from different GPUs or drivers aren't expected to match.
Rendering takes about 0.2 to 0.5 seconds a shot. Comparing takes under a
second for each changed 960 × 540 shot, and shots are compared in parallel.

## Timing and platform notes

The player runs on a fixed 70 Hz scene timer with 16 kHz stereo audio mixed in
512-byte blocks. Scene arithmetic and sequencing follow the original, but real
VGA and PIT timing isn't emulated, so frame timing won't match every DOS
machine exactly. A full headless run takes 17,442 timer steps (249.171429
seconds).

It's been tested on Apple Silicon Macs. It should build elsewhere, but display
and audio on other platforms haven't been checked yet.
