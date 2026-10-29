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
`--scroller`, `--rings`, `--field`, `--dots`, `--plasma`, `--greetings` and
`--ending` continues through parts 2, 6, 5, 3, 4, 7 and 8, to the demo's end;
`--full` is the same as `--ending`, the whole demo. Build and run it from the
repository root after importing your data:

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
black space.

The plate carries the PANIC title, its letters raised in the same polished
gunmetal. It is out in space from the start, too far away to see while the
letters fly, and brakes into the recorded sequence's approach as the doors'
phase begins, then follows its two turns with a continuous quaternion tumble
and forward motion. It is sized to the screen it lands in, past the sides and
under the doors' lips, which are drawn in front of it, so it isn't the
original plate's size. As it lands, space leaves its reflections and the
arrival flash strikes the letters, which come up as the heated logo; the plate
stays as the logo screen's polished backing, under dimmed studio lights. This
is an authored approximation: the original Rust decoder plays stored animation
frames of a plain block, not 3D rotation equations. These are reconstructed
forms, not recovered original models.

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
settle, and the quick fade of the stars behind the landed plate are all
authored.
The soundtrack, cues and scene boundaries use the existing port unchanged.
The plate lands at about 14.57 seconds; the preview holds there and ends its
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
| Left / Right | Step one second; hold to scrub through time, faster the longer held |
| Shift+Left / Shift+Right | Pause and step one 70 Hz frame, the original's finest; hold to keep stepping. The time is printed |
| Cmd+Left / Cmd+Right | Back to the start of the section (or the one before, within a second of its start); on to the next. The sections are the window title's labels: each part, and each phase within one, such as part 3's zoom, shade bobs and each dot object. Ctrl works where there is no Cmd key |
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
| N | Part 6: the new symbols in its middle, or the original's throughout |
| D | Part 4: still water under the slab (default), or polished dark stone |
| 1 / 2 | Parts 3 and 4: the water's mirror, ×0.8 or ×1.25 (part 3's 1, real water, to 8, default 3; part 4's about ×0.33 to ×3.05 of real water, default ×1) |
| 3 / 4 | Parts 3 and 4: the swell's height, ×0.8 or ×1.25 (0, a flat mirror, to about 15; default 3.8; part 3's is 0.35 of it) |
| 5 / 6 | Parts 3 and 4: the swell's speed, ×0.8 or ×1.25 (about 0.26 to 15; default 3.8) |
| 7 / 8 | Parts 3 and 4: the swell's size, ×0.8 or ×1.25 (about 0.26 to 15; default 1) |
| 9 / 0 | Parts 3 and 4: drops ×0.8 or ×1.25 as often (none, or one every 17 s to 0.9 s; default every 1.2 s) |
| Shift + 1 / 2 | Parts 3 and 4: the water's tilt a degree less or more (−15 to 30; default 3) |
| Shift + - / = | Parts 3 and 4: the water ×1.1 nearer or further below the camera at part 3's end (40 to 700; default 100) |
| Shift + 3 / 4 | Part 3: the plankton's glow shorter or longer, ×0.8 or ×1.25 |
| Shift + 5 / 6 | Part 3: the plankton dimmer or brighter, ×0.8 or ×1.25 |
| Shift + 7 / 8 | Part 3: the glow under the water dimmer or brighter, ×0.8 or ×1.25 |
| Shift + 9 / 0 | Part 3: the ripples weaker or stronger, ×0.8 or ×1.25 |
| J / K | Part 7: the beam's starting turn 16 frames shorter (faster) or longer (slower); default 128; from 237 s it shortens linearly to 32 by part 8 |
| U / I | Part 7: the beam's tilt toward the camera, 3° less or more (0 to 90°, straight at the camera; default 51°) |
| G / H | Part 7: the beam's starting brightness, ×0.8 or ×1.25 (0.05 to 4; default 0.25); from 237 s it rises linearly to 1 by part 8 |
| Z / X | Part 7: the beam's width (its focus), 0.1 narrower or wider (0, a thin shaft, to 2; default 1) |
| O | Part 7: the lamp in the frame's middle (default), or on the sweep's centre 15 rows above |
| T / Y | Part 8: the grille's stripes ×1.1 coarser or finer, its moiré later or earlier (default a triad every 2 rows) |
| Q / E | Part 8: the pull-back ÷1.1 slower or ×1.1 faster (default about 3 sets across at the switch-off) |
| F | Toggle fullscreen |
| S | Save a new PNG beside `--out`, named for the moment shown (`intro-hd-83.250s.png`, then `-2`, `-3` and so on); no file is replaced |
| P | Show or hide the frame-rate counter |
| V | Toggle vsync |
| Escape | Quit |

### Part 6: Rings

```sh
cargo run --release --locked --features hires --bin panic-hd -- --rings --seek 42
cargo run --release --locked --features hires --bin panic-hd -- --rings --faithful --seek 50
cargo run --release --locked --features hires --bin panic-hd -- --original-symbols --seek 50
```

`--rings` continues past part 2 through part 6, the interference rings and
flashing symbols, to the first frame of part 5, with the music unbroken,
as the pool described below. `--faithful` (or **M**) shows faithful HD, and `--split` (or **C**) the original on the left,
as in the earlier parts. Without `--field` (below), the clip ends on the
PSTART picture as part 5 begins, at 81.27 seconds, and holds there.

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

#### New symbols

In the middle of the part the neon shows authored symbols instead of the
original's. Read from the port, the symbol windows come 32 to each pattern
of the tune: windows 0 to 31 are order 06, from the first symbol at 43.41
seconds, 32 to 63 order 07, and so on to window 320 at 80.33. The seven
patterns from window 64 to 287 each show their own set of 16 drawings, and
the original's mask number picks the drawing with the same number, so the
music's order and pairs are kept:

| Set | Windows | From | Theme |
| --- | --- | --- | --- |
| 1 | 64 to 95 | 50.79 s | Water |
| 2 | 96 to 127 | 54.50 s | The elements and alchemy |
| 3 | 128 to 159 | 58.16 s | Machines of 1992 |
| 4 | 160 to 191 | 61.87 s | Classic emoji |
| 5 | 192 to 223 | 65.56 s | Then and now: brick phone to smartphone, and so on |
| 6 | 224 to 255 | 69.24 s | Social media then and now |
| 7 | 256 to 287 | 72.93 s | AI |

Patterns 2, 4 and 7 never light one pair of masks (12 and 13 in the second;
6 and 7 in the others), and those sets leave them out. The windows before
64, and those from 288 (76.64 seconds) to the end of the part, keep the
original symbols. Faithful HD always shows the original's, and
`--original-symbols` (or **N**) shows them throughout. The ignored data
test checks the windows' patterns and times, and that each set draws every
mask its pattern lights.

The drawings are authored for this remake, not recovered from the demo, so
unlike the original's data they are kept in Git, in
`src/hd/rings_symbols.txt`: one line per drawing of its set, mask, label and
runs, polylines in a 100-unit box on a 0 to 1000 grid. The file's header
describes the format. It is exported from the drawings' sources in
`tools/symbols/` (`pattern_1.py` to `pattern_7.py`, drawn with `neon.py`;
`render.py` previews them) by `tools/symbols/export_symbols.py`, which needs
numpy and Pillow for the kit; a Python test checks that the two agree.
Some are neon tracings of company logos (Claude, ChatGPT, Apple and Bitcoin
in set 5; YouTube, Instagram, X, WhatsApp, Facebook, Reddit, LinkedIn and
TikTok in set 6). Those logos are trademarks of their owners; their use here
implies no affiliation or endorsement.

The drawings' runs are made into neon tubes directly, in a box 155 rows
high on the screen's centre (`AUTHORED_BOX`), round on the 4:3 display.
That stands them about as tall as the originals, whose masks are 43 to 157
rows tall, 123 at the median; the drawings come to 43 to 152, 130 at the
median, and every one stays inside the 4:3 frame with its tubes' glass,
which a default test checks. The originals' thinning, tracing and redrawing
don't apply, but the same neon rules do (authored, after the owner's
direction): each tube is a single run that never closes or joins another.
Runs that meet end to end become one bent tube; where three or more ends
meet, the two that continue each other most nearly straight do. An end
whose glass touches another tube stops short of it with the originals' gap,
both tubes' glass and a stroke's half-width of air (9.8 rows to the other's
centreline). Where tubes still cross or touch, the shorter breaks with that
gap either side (a run's later pass, where it crosses itself). A loop opens
at the middle of its lowest stretch, its new ends that gap apart, or keeps
half of itself if it is too small for that. A run of one point is a dot.
What the cuts leave is kept however short, so pins, crossbars and links
remain as short tubes; only a run too short to keep the gap at both ends
could go, and none of the drawings has one. A run under half a row long is
a dot, as the drawing kit draws dots. The
atlas measures the distances to the tubes and their free ends, and the light
map is made from them, as for the originals; default tests check that no
loop is left, no glass touches, and nothing longer than two gaps is lost.

Every drawing is a different subject, none repeated across the sets, and
each looks unlike the one the music shows next: how alike they are decides
how well the flashes keep time. The ignored test
`consecutive_symbols_contrast` prints the overlap of their glass on screen
(intersection over union) for every flip the music plays, set by set,
against the original symbols', which average about 0.11, and checks that
each set's mean is under 0.15 and every flip under 0.3. The sets come to
0.11 to 0.13, the most alike flip to 0.25. The emoji in set 4 are our own
line drawings of the emoji ideas, not any vendor's emoji artwork.

The neon alone draws them, so their 112 layers keep only the two fields the
tubes use, the centreline and free-end distances, over their own rectangle,
and a light map at half the resolution each way (one texel per 2 by 2
pixels, `SymbolDetail::Tubes`). They take 86 MB, against 30 MB for the
original's 16 layers, and are built in about 0.02 seconds. Below about 1.2
pixels their light map is blurrier than asked, which shows only where the
pool's reflections soften.

Faithful HD shows the original's look at display resolution: the analytic
rings in the original window with its two greys and XOR, antialiased; the
symbols as smooth strokes at half strength in the original's yellow, which
is the average of its flicker in linear light (the random blue component
averaged too); and the PSTART picture smoothly upscaled, fading in DAC units
as its palette does. It takes 0.15 to 0.5 ms a frame at 2560 by 1440 on an
Apple M5 Max.

Part 6's style, the pool, is an authored experiment. It keeps the
original's ring centres, bands, fades and symbol timing through the shared
helpers; everything seen is authored.

The lit symbol is a yellow neon tube along its stroke's centreline, from the
shared tubes in `rings_common.wgsl`, and the shared HDR bloom makes it glow.
Following the owner's direction and photographs of neon signs, each tube is a
clear glass tube bolder than the original stroke (radius 3.8 rows to the
stroke's 2.2, `NEON_RADIUS`), with a column of glowing gas down its middle: white hot along the centre once tonemapped,
where a line of sight crosses the most gas, and a saturated amber yellow at
its edge, warmer than the original's lemon. The glass carries a little of
that light, gathers a bright line along each wall and catches a lamp's
highlight, and what lies behind shows through it, bent. As in a real sign,
each tube is a single run that bends but never branches: where the original's
strokes meet or cross, the two branches that continue each other most nearly
straight stay one run, and the others stop short of it with an air gap of
about a stroke's half-width, glowing to their rounded ends (their electrodes
bend away behind the sign; `NEON_ELECTRODE` shows them instead). Fragments
shorter than a stroke's width go. What is too short to keep is measured
against the original strokes (`NEON_STROKE`), not the glass, so the thick
tubes keep the drawing's small strokes; only the shortest, where a crossing
tube's wider gap takes all of it, go. A haze of scattered light follows the tubes' shape. The
neon's light is an approximation: a prefiltered map of the tubes, not traced
light.

The pool shows the rings as liquid chrome seen from above. The two ring
layers are two wave sources at the original's ring centres, with its band
spacing; the second layer's waves are inverted, so the surface lies level and
dark where the original is black, and rises or sinks where only one layer is
set. The waves fill the whole screen, widescreen included, rather than the
original's 256 by 184 window. A low lamp hangs over each source, cool over
the first and warm over the second, so each source's waves catch its colour
and their crests shade the waves beyond. While the rings are lit, the neon
symbols hang high above the surface, about a quarter of the screen, so the
waves' flanks catch them far out across the pool; as the rings fade out they
sink to just above it, so in the dark their reflections keep near them.
Each wave mirrors the tubes themselves, traced to where its reflection meets
them, in perspective, with a soft glow round them from the chrome's sheen;
both are twice as strong as the chrome alone would give, an authored boost
so the neon holds its own against the lamps. Until the rings first fade in
there is only the neon on black. They come up as the original's black and
white: its two greys and XOR, a flat print filling the screen under the
neon, lit by it. As the tune first says "panic" (read from P.P: order 0A,
row 0, at 58.16 s, first plays instrument 44, a voice sample, about 0.34 s
long), the waves rise out of the print, lamps and reflections with them
(authored; `PANIC_FADE`). When the rings fade out the lamps go
out, but the waves stay, seen only by the neon they reflect.
(`POOL_DARK_WAVES` at 0 lets them settle into a still black mirror instead.)
It is exposed over a whole 60 Hz frame, its samples spread across each pixel
as well, so the fringes don't flicker and the reflections stay sharp. It
takes about 2 ms a frame at 2560 × 1440.

Four other styles were tried alongside the pool and removed from the code.
Faithful-plus kept the original's two-grey moiré, antialiased, under the
neon; glass made each ring layer an edge-lit glass plate, one cool and one
warm; room projected the layers onto part 2's bump wall in the dark; and
volume made them spherical waves in haze, with a drifting camera. They
remain in the Git history: commit `a786132` introduced faithful-plus and
the style framework, `43a1b78` glass, `64877d8` room and `3e1c181` volume,
and `a9f0e30` is the last commit that still has them, with `--rings-style`
and the keys **1** to **5** to choose between them.

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
style's own names with its name (`pool_` for the pool); the naga test
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
  `ring_intensity()`, `ring_peak()` (the most the rings have been lit so
  far: 0 before the fade-in, 1 after it, through the fade-out),
  `picture_level()`, and the symbol windows
  (`neon_mask()`, `neon_previous()`, `neon_on()`, `neon_previous_on()`,
  `neon_since()`, `neon_until()`). The symbol switches are the original's
  own cues, on its tracker rows. Before 43.41 seconds the original is black.
  `neon_mask()` and `neon_previous()` give the symbol layers the neon draws:
  an authored set's drawings in the middle of the part, the original's masks
  elsewhere. `original_mask()` and `original_previous()` always give the
  original's masks, for faithful HD.
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
- Symbols, by layer: `symbol_field` (the outline's signed distance, the
  distance to the centreline, the stroke's half-width and the distance to
  the stroke's nearest free end; for the authored sets the outline and
  half-width are derived from the centreline distance), `tube_distance`; the tubes as `NeonGlass` samples
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
with `tools/look_check.py`, whose part-6 shots show it through each phase.

### Part 5: Field

```sh
cargo run --release --locked --features hires --bin panic-hd -- --field --seek 81
cargo run --release --locked --features hires --bin panic-hd -- --field --faithful --seek 96
```

`--field` continues past part 6 through part 5, the zoom and the mountain,
to the first frame of part 3, with the music unbroken. It implies `--rings`.
`--faithful` (or **M**) shows faithful HD, `--original` (or **Tab**) the
original and `--split` (or **C**) both, as in the earlier parts. The clip
ends at 130.03 seconds, when part 3 takes part 5's last frame, and holds
there; `--dots` carries on through part 3.

Read from the port, and checked against the engine on every frame by an
ignored test, part 5 runs like this. It begins at 81.27 seconds on the
PSTART picture part 6 faded in: a bevelled frame round a 128 by 100 window at
(96, 46). In the window is an endless zoom into a blue field. Each frame
resamples a 256 by 200 byte buffer round its centre, at 1 to 2 buffer pixels
per window pixel, and lowers that scale by the speed; when it passes 1 the
buffers swap to the next level, which `expand` builds two rows a frame as a
smoothed enlargement of the centre, twice as fine, and `perturb` adds noise
to the new level's centre. WELLPAL makes the field dark at 64 and bright at
both ends. The speed rises by 16 a frame to 1024 by 82.17 seconds, one level
every 63 frames (0.90 seconds): the frame that swaps lowers the scale a
second time. It dips to 1008 at 95.13, when the music's position passes
0x1400, and from 96.06, when the position is 0x400 past where the part
began, it falls by 16 a frame, so the zoom stops at 96.96 after 17 swaps,
a magnification of about 81,000.

The terrain then begins: a voxel height field of DEP.U's heights and MAP.U's
colours as `prepare` makes them, in MAP.PAL's fire colours, drawn by 160 rays
two columns wide from depth 64 to 512 in front of the picture. Its level
rises by one a frame to 64 by 97.86 seconds. The level takes the camera down
from 3 r + 470 to 3 r + 150 height units, r the orbit radius, and the
projection from flat to perspective (its table is built with q = 8 times the
level, up to 248). The window still shows the zoom's last frame behind the
rising mountain until 98.11, when it is cleared to colour 129, near black.
The camera circles the map's centre a turn every 1.46 seconds, looking
inwards, its distance swinging from 125 to 234 map units and back every 1.83
seconds. From 121.83 the level falls by two thirds of a unit a tracker row,
the camera rises and the projection flattens; the last terrain pixel is at
127.69, and the frame holds with its dark window until part 3 takes it.

The preview keeps part 5 as compact state per frame, read from the scene's
counters through `Field::hd_state`: the zoom's scale, speed, swaps and the
rows of the next level built, the terrain level, the projection table's q,
the camera and the window clear, with the three palettes the part shows. The
zoom buffers and the original frames for `--original` and `--split` are
replayed on demand rather than stored (about 570 MB): the scene, video memory
and the shown buffer are kept every 64 frames (about 45 MB), and the port's
own `Field::step` runs forward from the nearest checkpoint. The ignored test
checks every replayed frame and palette against the engine's, that the shown
buffer at the drawn scale is exactly the window (by the port's own `zoom`),
the still and cleared window, the terrain's palette, the camera and the PCM,
and prints the times above.

Some of this is interpretation:

- The zoom's scale is interpolated linearly between the original's frames,
  in the shown buffer's pixels. Into a swap it carries on below 1, where the
  next level takes over at twice the scale, so the magnification grows
  without a step. At each frame's own time the window is the buffer round
  its centre at the drawn scale, which agrees with the original's
  fixed-point sampling at every pixel the test checks.
- The camera follows the code's formulas at continuous time, without their
  integer truncation. The code truncates the orbit's radius, then its
  products, which moves the original's camera by up to 3 map units from the
  path; the HD camera keeps to it.
- Faithful HD marches each pixel's ray through the original terrain, as the
  original's rays do in whole steps two columns wide: from depth 64 to 512
  with the code's camera and projection, a pixel shows the nearest point
  whose height reaches its row. Heights and colours are bilinear between the
  map's cells. With each frame's own integer camera, its silhouette is within
  a row of the original's in 81% of columns, 1.3 rows away on average; with
  the smooth camera, 1.8 rows on average.
- As the research found, DEP.U looks like a Mandelbrot mountain: its plateau
  is the Mandelbrot set and its slopes follow base + slope sqrt(G(c)), G the
  set's potential. The preview fits that when the data loads
  (`MandelFit::fit`): the set's place, scale and aspect start from the
  plateau's moments against the set's own, and Nelder-Mead maximises the
  plateau's intersection over union with the set; the base and slope are a
  least-squares line through the slopes. On the imported data the plateau
  matches the set at an IoU of 0.987, with an aspect close to the VGA's 1.2,
  and the slopes have an rms error of 4 of 250. A default test recovers an
  invented mountain's parameters the same way. The fit is baked into a
  relief, a texture of the height `prepare` would make of the fitted DEP
  value with its gradient and the distance outside the set, for the modern
  view. Nothing fitted is stored. How DEP.U was made isn't known, and
  MAP.U's speckle, on a rounded plate, is of unknown origin.

Faithful HD shows the original's look at display resolution: the picture
smoothly upscaled through the current palette, with the window's edge kept
sharp; the zoom from the port's field buffers, smoothly resampled at the
interpolated scale in the original's palette, still once it stops and colour
129 once cleared; and the original terrain, marched per pixel, through the
palette, never over the top row, as in the original. At 2560 × 1440 on an
Apple M5 Max a frame takes about 0.9 ms during the zoom and 2.6 to 2.9 ms
during the terrain, as `--bench` measures it, CPU and waiting for the GPU
included (it gives part 6's faithful HD 0.7 ms).

The modern view is three authors' work, each in their own files; see
"Adding to part 5" below: the metal frame and its smoke, the window's deep
zoom and the mountain.

The frame is dark polished gunmetal. It is drawn exactly where the PSTART
picture has it, as nothing is displaced, and is lit by what is round it.
Its shape is made from the user's own picture when the renderer loads
(`frame_shape` in `field_frame.rs`, in about 25 ms), and nothing derived from
it is stored:

- Measured from the picture: on each side, the frame's outer edge and the
  crease between its outer face and its inner bevel, read from the runs of
  colour outward from the window's edge. On the imported data the outer
  edge runs from (67, 17) to (253, 175), and the crease is 15.5 pixels out
  from the window on every side. Also measured: the bands of lighter
  shading across the middle of each side.
- Interpretation: the ornaments' relief. The picture shades them as rounded
  metal, lighter where they stand out, so their height is the picture's
  lightness, smoothed so that its 16 tones don't show as terraces, falling
  to nothing at outlines smoothed from its pixels. Their normals come from
  that height. The bevels are clean planes between the measured lines.
- Authored: the bevels' profile (a steep foot at the window, the inner
  bevel at 55 degrees, a rounded crest, the outer face at 24 degrees and a
  rounded lip); the material (each side brushed along its length, with its
  highlights drawn out along the brushing, polished bands, and cast
  ornaments that are darker in their recesses); and the lights. The lights
  are a dim key from the upper left, as the picture is shaded, with a small
  bright light beside it for the catch-lights on edges, a dim cool fill from
  the lower right and a dim ambient. Then come the window's light, which
  the inner bevel reflects through the opening, and the lava's, each from
  the other author's values or a stand-in until they give them (below).
  The mountain's silhouette over each column, as the relief marched with
  the code's camera shows it (the frame's prepass), shadows the frame from
  the lava and the key and darkens it near the mountain's base.

Part 6's PSTART fade brings the lit frame in (`FRAME_TAKES_PART6_FADE`),
its lights rising as the picture's level did, so the hand-over into part 5
is seamless. Part 6 is unchanged before 80.36 seconds. After the last
terrain pixel the window is dark, and the frame holds, lit only by the
room's dim lights, until part 3 takes it. At 2560 × 1440 on an Apple M5 Max,
the frame costs about 0.2 ms a frame at 85, 98.2, 105 and 129 seconds, as
`--bench` measures it against the same build drawing no frame. That is less
than the flat picture it replaces cost.

Round the frame is smoke, a real fluid simulation: stable fluids (Stam
1999) on a 3D grid, with MacCormack advection, buoyancy, vorticity
confinement and a multigrid pressure solve. It is lifted from a separate
prototype, kept locally, whose kernels it adapts to the real scene
(`field_frame_smoke.rs` and its `field_frame_smoke/*.wgsl` kernels). The
velocity grid is 208 × 124 × 64 cells over the 16:9 screen, with margins.
Density and temperature are at twice that each way, stored as f16. It
steps 60 times a second from part 6's fade, after an unseen pre-roll of 8
seconds. Its light is computed each frame toward the window and the lava,
and drawn in two layers, behind the frame and in front of it.

- Driven by the scene's own state, through the part-5 block at each
  step: the zoom, the mountain's rise, level, camera and orbit, the
  window's clear, the sinking and the last terrain pixel, as the preview
  observes them, so the no-music timeline works too. The lights are the
  other authors' (or the frame's stand-ins for them).
- Derived from the user's picture: the obstacles. The frame and its window
  are a solid plate up to the bevels' tops. The ornaments are smoothed into
  solid ribbons, as the cells are coarser than the filigree, and their
  softened coverage shadows the smoke.
- Interpretation: the lava's heat rises in a band below the mountain's
  outline over each column. The outline is the fitted relief marched with
  the code's camera and projection, as the mountain's stand-in is drawn.
- Authored: the emitters, the heat, the air's movement, the lights' placing
  and strength, and the solver's parameters. The smoke is kept mostly
  behind and beside the frame, secondary to it and the mountain. Its heat
  lies behind the picture plane, or near it beside the frame, and a draft
  toward the back keeps the window and the mountain clear. Smoke in front
  of the frame draws behind the mountain.

`SMOKE` in `field_frame_smoke.rs` picks the smoke: `Volume` (the default,
above), `SoftVolume` (density and temperature at the velocity grid's own
resolution, softer) or `Flat` (the prototype's 2D fallback, 512 × 288,
which reads as ink in water). At 2560 × 1440 on an Apple M5 Max, `Volume`
costs about 5 ms a frame during the zoom and 6 to 7 ms during the terrain
and after it, as `--bench` measures it against the frame without smoke. A
step takes about 4.4 ms, and the light the rest. With the other stand-ins,
a whole frame takes about 6 ms during the zoom and 9 ms during the terrain,
the shared bloom included. The mountain's own cost replaces its
stand-in's, about 2 ms of that. Timings on a busy machine run higher.

The simulation is a pure function of the step, so it reaches the same
state however a moment is reached, which an ignored test checks by a hash
of the whole state. It is chaotic, so changing a kernel or a parameter
changes every later frame. Bit-exactness holds on one machine, GPU and
driver. Seeking and memory, for `Volume`:

- Playing through part 6, the pre-roll runs ahead over the half minute
  before the fade, a step every three or four frames, so the fade's first
  frame runs one step. A seek into part 6 catches up two steps a frame. A
  seek past the fade runs whatever is left there, about 2 seconds for all
  480 steps.
- A fresh process, as a one-frame export is, simulates from the fade's
  pre-roll: about 9 seconds to reach 105 seconds and 15 to reach the end.
- A state is kept every 3 seconds as it is first reached: 76 MB each, up
  to 17 of them (1.3 GB), on the GPU. The simulation's own working set is
  about 0.4 GB.
- Seeking back runs at most 179 steps from a kept state, about 0.8 seconds
  at worst; onto a kept state it is immediate.
- Scrubbing stays exact. Forward through time not yet simulated, a quarter
  second a frame costs about 75 ms a displayed frame. Backward, a second a
  frame costs 0.2 to 0.9 seconds a frame. Playing on runs a step a frame.

The renderer asks the GPU for its full number of storage buffers per
shader stage, as the smoke's solver binds more than wgpu's default of 8.

The modern window is a deep zoom into the Mandelbrot set, from a view
3.3e-50 high to one 4.0e-55 high. It lands on a minibrot of period 3341,
in the set's usual orientation and filling 78% of the window's height. It
stays deep the whole way, so the window never shows the whole set: rings
of lace round the copy close in on it, doubling by doubling, until it
fills the window.

- Read from the port: the timing. The depth is the port's own
  magnification, `field_zoom_level()`: log2 of 2^swaps over the scale,
  interpolated between the original's frames as faithful HD has it. So the
  16.3 doublings, the 63-frame rhythm, the dip at 95.13, the brake and the
  stop at 96.96 fall on the original's frames; the zoom counts no frames
  or seconds of its own. The window then holds the landed view behind the
  rising mountain until the port clears it at 98.11, and is dark from then
  on. Through part 6's fade and part 5's first frame, where the original
  still shows PSTART's window, it shows the zoom's first view.
- Authored: the target, its framing, the method and the look. The target is
  the end of a chain of nested minibrots out of the seahorse valley
  (periods 32, 250, 1030 and 3341), found by a local search; nothing about
  it comes from the demo. It is a constant in `field_zoom.rs`, the nucleus
  and the window's centre as decimal strings, read when the renderer
  loads. The window is centred on the copy's body and turned by a
  constant 128.2 degrees so that the copy lands in the set's usual
  orientation.

The method parallels the original's. Field shows one level buffer,
resampled at 1 to 2 buffer pixels per window pixel, while it builds the
next one, twice as fine, two rows a frame, and swaps at each doubling. Here
level k is the exact view at depth k, at 2 texels per window pixel at its
own depth (`LEVEL_SCALE`). A compute kernel builds it in bands while the
doubling before it plays, due by 0.8 of the way (`BUILD_BY`), and the
window resamples the level of the current doubling, 1 to 2 times
magnified, with four samples a pixel. Unlike the original's, a level is
computed afresh, not enlarged from the one before. Three levels are kept on
the GPU: the one shown, the next and one for stepping back. The first
level, which part 6's fade first shows, is built the same way ahead of it,
over the second that ends half a second before the fade (`PREBUILD_SPAN`,
`PREBUILD_LEAD`), after the smoke's pre-roll, so that the fade's first
frame doesn't build it all at once.

A level is a pure function of its depth and size, so playing, seeking,
scrubbing and a one-frame export show the same image. An ignored test
checks that byte for byte against a fresh renderer, across a doubling and
at five moments reached by seeking. A level that isn't complete when it is
needed is finished at once, rather than shown partly built or blank. That
is a one-off hitch when seeking: at 2560 x 1440 a level takes about 18 to
40 ms, at 3840 x 2160 40 to 80 ms (the deeper, the longer), and a seek can
need two. A second ignored test checks that the first level built ahead,
in playing through part 6 or after seeks into and back before the second
it is built in, is the same, byte for byte, as one built at the fade.

Each texel is computed by perturbation from one reference orbit, at the
minibrot's nucleus. That orbit is exactly periodic (Z_3341 = 0 = Z_0), so
its 3341 points serve every iteration and no texel needs a second
reference. The orbit is computed in 376-bit fixed point when the renderer
loads (`field_zoom.rs`'s own, no crate; 0.5 ms), with bilinear
approximation (BLA) tables over it and their radii for each level (3 ms
more). The GPU works in f32 only:

- The deltas from the reference are plain f32 down to 2^-45 and floatexp
  below that: an f32 mantissa pair with a shared exponent.
- BLA skips runs of iterations, with a tolerance of 2^-12, which the
  prototype measured moving the smooth iteration count by about 0.001.
- Zhuoran's rebasing keeps the deltas small without another reference.
- A periodicity test finds the minibrot's inside, and the distance
  estimate shades the outside.
- A texel takes at most 1500 steps. Those that reach the cap, 0.5% at the
  stop, lie on the boundary and are drawn as it.

A local prototype, outside the repository, checked the kernel's method
against a direct 376-bit render and mpmath at the deepest view.

The look is cool blues in linear HDR, into the shared bloom and tonemap.
The outside is shaded by the distance estimate on a log scale, so that
dense lace doesn't saturate: navy far from the set, through the original
window's deep blue and azure to cyan. A white-cyan boundary line blooms.
It is divided by its own density nearby, so lone filaments glow and dense
lace stays textured. Faint smooth iteration bands lie far out, and the
inside is near black. Distances are in pixels of a 4K window, so the look
is the same at every size, and the whole is held at 0.45 (`ZOOM_GAIN`) so
that the window doesn't overpower the dark frame. The look's constants are
together at the top of `field_zoom.wgsl`, and the method's at the top of
`field_zoom.rs`.

The window's light for the frame and smoke (`fc.zoom_style[0]`) is
authored from measurements:

- It is the window's mean colour times an intensity of 2.
- At each whole depth, it is scaled by the window's mean luminance there
  against depth 0, raised to 0.75 so that the smoke swings less than the
  window does. That takes it from 1 to 1.6 times over the zoom, linear
  between depths.
- It rises with part 6's fade and goes out over 0.12 seconds when the
  window is cleared.

The colour and luminances were measured from captures of the window's
middle with the current look; they should be measured again if the look
changes.

At 2560 x 1440 on an Apple M5 Max a frame takes about 5.7 to 6.0 ms during
the zoom, against 5.3 to 5.7 ms with the stand-in it replaced (at 82, 88
and 94 seconds, medians of `--bench 150`): 0.3 to 0.5 ms more, for the
shading and the level built in the background. After the stop, while the
mountain rises and no level is built, frames take 10 to 11 ms, as with the
stand-in. At 3840 x 2160 the zoom costs 0.8 to 1.3 ms a frame more than
the stand-in. The levels take 8 bytes a texel: three of 1583 x 1484 at 2560
x 1440 are 56 MB, three of 2374 x 2225 at 3840 x 2160 127 MB; the tables
are a few MB.

The modern mountain is a molten Mandelbrot: a convecting lava lake on the
plateau, the set itself, spilling over its rim in bright falls that run down
the slopes in channels and crust over as they go, on a mountain of dark
cooled crust.

- Derived: the motion and the shape. The level, the camera, the flat to
  perspective morph of the projection, the orbit and the sink come through
  the shared helpers, as faithful HD has them; within each exposure the
  camera moves on by the code's formulas (`field_mountain.rs` passes the
  rates: a terrain step and a level a frame while it rises, q eight times
  the level). The surface is the fitted one: the baked relief, and near the
  set the fitted formula, base + slope sqrt(G(c)), evaluated per pixel.
  The lava's paths are the fitted slopes' fall lines.
- Interpretation: lighting the projection as 3D, a height unit as 0.29 map
  units (at full perspective a height unit shows 62 / z rows and a map unit
  across 213 / z), and reading MAP.U's speckle back out of `prepare`'s
  colours as (4 colour - height) / 3. The level's rate while it sinks is
  estimated, as its mean fall since the sink began.
- Authored: the lava, its flow and the lake's convection, the crust, the
  lighting, the detail method, the exposure and the light handed to the
  frame. The original colours its terrain from MAP.U through MAP.PAL's fire
  ramp and moves none of it; nothing of the lava's look or motion is from
  it. The lava is the main light, with a dim cool key from behind the
  camera, shadowed and occluded by the relief. The rays start at depth 4
  rather than the original's 64 (read from `ray`; 4 is where the code's
  projection table starts), so that close passes show the nearer slopes
  where the original smears the terrain at depth 64 down the columns below
  it.

The lava is drawn as temperatures. Its light, `mountain_glow`, follows a
blackbody-like ramp: deep red about 700 °C, orange about 950 and
yellow-white about 1150, where it is 12 in linear HDR, far brighter than the
frame or the smoke, into the shared bloom and tonemap. Its brightness
follows Wien's approximation, exp(-B / T), with B about 0.6 of the physical
value at 600 nm so that dull red to white fits the tonemap, and its green
over its red rises faster than Planck's law gives, standing for a camera's
saturation. The lava leaves the lake at about 1170 °C and cools as it
travels; its skin cools within a few seconds to dark glassy crust, through
which cracks show the hot lava below.

The slopes' lava runs in channels drawn in the set's external angle, arg
Phi(c) / 2 pi from the product formula for Böttcher's coordinate (its first
16 terms, with principal arguments), which is constant along the fall lines
(the field lines of G): 48 cells a turn, each with a channel of its own
width and speed or none, meandering a little, and between them 192 thinner
rivulets a turn, quicker to crust over. The lava falls at a rate that
depends on the height alone, 9 height units a second at the foot rising
exponentially to 42 at the rim, as the fitted slopes steepen towards the set
(their median steepness about triples from the foot to the rim, sampled on
the imported data's fit). So the time it has taken from the rim is in
closed form, and anything drawn in that time less the clock moves exactly
with the flow, down the fall lines, without stretching or repeating. On the
steep walls under the rim it runs young, fast and open, in bright streaks;
on the gentler ground below it slows and crusts over. Rapids speed it up and
slow it by 45% every few seconds of travel: the plates of crust it carries
as rafts down a channel's middle tear apart, opening glowing cracks, where
it speeds up, and buckle into ropes bowed downstream where it slows. A
channel's middle is its hottest and fastest; towards its stiff, cooled
levees the crust closes up and the open lava is sheared into streaks.
Surges come and go along each channel, lower down stretches roof over into
tubes, and between the channels the old flows' crust keeps a few glowing
cracks near the active channels and where the original's colour is hot.
The lake spills a quarter of a second before the mountain has risen, and
its lava front runs down each channel at that channel's own speed, so the
slopes light up from the rim down; below the front the channels hold older,
crusted lava.

The lake convects: hot lava wells up along a wavering seam down the set's
real axis, through the main cardioid and the period-2 bulb, and spreads away
on either side at 4 map units a second, so its plates of crust are as old as
their distance from the seam: thin and glowing, with wide cracks, by the
seam, dark and thick towards the rim, where they founder in a bright margin.
Cracks pulse open and shut, and now and then gas bursts through, throwing
open a disc of bright lava that crusts over within a second. The lake is
molten as the mountain rises out of the zoom's set, so that the set reads
against a dark rim, and crusts over in the second after it has risen.

Everything the lava does is a function of the clip's time alone
(`field_seconds()`), so seeking, captures and video agree. Seen at the
orbit's distance, features on the falls under the rim travel up to about a
tenth of the screen's height a second, half that halfway down and a fifth of
it at the foot, and the lake's plates about their own width a second. Each pattern fades to its mean where it would be
finer than a pixel, measured along its own axes across the pixel's
footprint on the surface, which is longer where the view grazes it; under
the exposure's motion the footprint is widened along the motion only, so
that the exposure's samples blend into a streak across the screen while
detail along the flow, such as the crust's edges and the surges, stays
sharp.

Near the set the detail is computed on demand rather than taken from the
bake. Rays march the relief, skipping space by its coarse cells, and in a
band round the set, where its texels can't hold the boundary, the fitted
formula itself: the potential and its gradient by direct iteration in f32,
stepped by a bound from the potential (the distance estimate d = G / (2
|grad G|) is about a lower bound on the distance to the set for small G, by
Koebe's quarter theorem, and within it the positive harmonic G is at least G
(d - r) / (d + r) at distance r, by Harnack's inequality) or by a Newton
step towards the potential the ray's height needs, kept between that bound
and four times it, no finer than half a pixel. A comparison only needs to
know whether G is below that potential, and an orbit still short of the
bailout after n iterations has G below about 11 / 2^n, so the march stops
iterating early; shading decides the plateau with the full 100 iterations.
So the plateau's edge, the filaments' ridges and the cusp stay sharp to the
pixel, up close as far off, with normals from the potential's gradient; a
ray that grazes the crests for more than 20 steps goes on through the
relief. A half-resolution prepass marches every ray at the middle of the
exposure and keeps where it first came within a pixel and a half of the
surface or entered the band, and its hit and coverage; each full-resolution
pixel marches on from there. The exposure is 0.7 of a 70 Hz frame, as part
6's rings use: where the surface moves across the screen in it, up to four
samples are taken along the motion, each what the prepass saw where its ray
points at the middle of the exposure (its depth interpolated where the
texels round it see one surface), shaded with the middle's surface carried
over to first order (G, and the angle by Cauchy-Riemann) and its detail
smoothed along the motion over the samples' spacing.

At 2560 × 1440 on an Apple M5 Max the whole part-5 frame, with the frame, its
smoke and the window's stand-in, takes about 10.4 to 11.8 ms during the
terrain (at 98.2, 105, 115 and 122.5 seconds, medians of `--bench 150`, the
best of three interleaved runs on a machine in use), within noise of the
mountain's first version, and about 8 ms with no mountain. Before the
frame's smoke was in, the mountain's first version took 5.1 to 6.0 ms a
frame with the frame's and window's stand-ins; at 105 seconds its prepass
took about 1.0 ms, the middle of the exposure 2.3 ms and the rest of the
exposure 1.4 ms. The lava's shading costs more than that version's, and
taking four samples across the exposure rather than six pays for it.

### Adding to part 5

Part 5's modern view has three authors, each with a WGSL file and a Rust
file of their own in `src/hd/`:

| Author | Files | Draws |
| --- | --- | --- |
| Frame | `field_frame.wgsl`, `field_frame.rs`, and the smoke's `field_frame_smoke.rs` and `field_frame_smoke/*.wgsl` | The metal frame and the smoke behind and round it, with the fluid hook |
| Zoom | `field_zoom.wgsl`, `field_zoom.rs` | What the window shows, with the level hook |
| Mountain | `field_mountain.wgsl`, `field_mountain.rs` | The mountain |

The renderer builds one module from the shared controls,
`field_common.wgsl`, the three authors' files and `field_style.wgsl`, whose
entry points call the authors' functions:

```wgsl
fn frame_layers(px:FieldPixel)->FrameLayers   // smoke behind the frame, the frame, smoke in front
fn zoom_layer(px:FieldPixel)->FieldLayer      // the window's content
fn mountain_layer(px:FieldPixel)->FieldLayer  // the mountain
fn frame_fluid_init(cell:FluidCell)->vec4f    // the fluid's first state
fn frame_fluid(cell:FluidCell)->vec4f         // one step of the fluid
fn frame_prepass(px:FieldPixel)->vec4f        // likewise zoom_prepass and mountain_prepass
```

A `FieldLayer` is light premultiplied by its opacity (linear HDR; the shared
bloom and ACES tonemap follow), the opacity, and a depth as scene z, towards
the viewer: the frame's plane is z = 0 (`LAYER_FRAME_Z`), the window's
content is behind it (`LAYER_WINDOW_Z`) and the mountain in front
(`LAYER_MOUNTAIN_Z`), as the original draws them. `field_style` puts the
five layers over black farthest first, so the mountain covers the frame and
smoke can lie behind it, round it and in front. The zoom's layer may reach
past the window's edge, where the frame hides it. Prefix your own names with
`frame_`, `zoom_` or `mountain_`: the shared code leaves those prefixes to
you but for the prepass readers, and a default test checks it. The naga
test validates the module and checks that each entry point uses only the
bindings its pipeline has; the zoom's levels are group 2 of the drawn
pipelines. Edit only your two files.

Each Rust file holds its author's `AuthorInfo`: set `prepass: true` to have
`<name>_prepass` drawn first, at half the resolution each way, into a
texture `<name>_prepass_at(uv)` reads. Its controls function returns four
spare vec4s at each moment, read as `fc.frame_style`, `fc.zoom_style` or
`fc.mountain_style`. `field_frame.rs` also holds the fluid's settings
(`FLUID`), `FRAME_TAKES_PART6_FADE` (set) and `frame_shape`. That is the
frame's shape, baked from the picture when the renderer loads, which
`field_render.rs` binds at group 0, binding 13, for `field_frame.wgsl`;
its layout is documented on `FrameShape`. `field_mountain.rs` holds the
relief's resolution (`RELIEF_SIZE`, 1024 by default, about 5 texels a map
unit).

The frame reflects light from the other two authors. Each value counts as
given once any of its four components is non-zero, so w can mark a dark
light as given; until then the frame draws a stand-in:

- `fc.zoom_style[0]`: the window's light, rgb, its average colour times its
  intensity, as linear HDR radiance. The zoom now gives it, with w 1
  throughout part 5. The stand-in, a cool blue that goes out when the
  window is cleared, remains in the frame's code.
- `fc.mountain_style[0]`: the lava's light, x and y in source pixels, scene
  z, and its spread (the glow's radius in rows).
- `fc.mountain_style[1]`: the lava's radiance, rgb, linear HDR; this one
  says whether the lava is given. The stand-in is an orange glow over the
  plateau, in front of the frame, rising and sinking with the terrain.

The mountain's controls carry the lava's light for the frame and smoke to
use: `fc.mountain_style[0]` is the centre of the lava's light as the frame
sees it, a little above the lake, its source x and y (320 by 200 pixels), a
scene z in front of the frame (100, `LAYER_MOUNTAIN_Z`, with the lake 180
map units from the camera, nearer the viewer as it comes closer) and a
spread (the glow's radius in source rows, 36 map units across: the lake and
the falls below its rim). `fc.mountain_style[1]` is its colour times its
strength in linear HDR, with the luminance in w: an orange like the lava's
about 1050 °C, as bright as the frame's stand-in for it (luminance 2.4),
which the frame's reflections and the smoke's light and heat were tuned to.
Of that, 40% is the slopes' and 60% the lake's, which follows how much of
the lake the camera sees: its area on the screen, as the code projects it,
against its area at the orbit's mean, within 0.3 to 1.25 of it. So the light
is brighter as the camera looks down on the lake or passes close by and
dimmer as it sees the lake nearly edge on, between about 0.6 and 1.15 of the
stand-in's, and fades to 0 as the lake leaves the bottom of the screen; it
is 0 with no mountain. Both follow the camera smoothly. The frame reads them
unchanged: with them its ornaments and bevels nearest the mountain catch
copper highlights and the smoke behind glows orange from below.
`fc.mountain_style[2]` is for the mountain's own exposure.

`px` gives the pixel in the same spaces as part 6's: `uv`, `xy` (the
screen), `source` (the original's 320 by 200 pixels, continuing beyond the
frame on wide screens), `scene` (source rows on the 4:3 display, y up,
centred, with the default camera `EYE` and `camera_ray`) and `pixel` (one
target pixel in source rows). The terrain's map is 200 by 200 units: map
point (x, y) is DEP.U and MAP.U's row x and column y.

The helpers in `field_common.wgsl` read the part-5 block through `fc`, which
the fragment entries copy from `u.field` and the fluid step from its own
controls, so they work in both. They include:

- Timing: `field_seconds()`, `field_phase()` (0 zoom, 1 terrain),
  `field_frame()`, and the phases' times, as part-5 seconds:
  `time_full_speed()`, `time_braking()`, `time_terrain()`,
  `time_cleared()`, `time_risen()`, `time_sinking()`,
  `time_last_terrain()` and `time_end()`.
- The zoom: `field_zoom_scale()` (buffer pixels per window pixel, below 1
  while the next level is due), `field_zoom_ahead()`,
  `field_zoom_level()` (log2 of the magnification since the first zoom
  frame, continuous), `field_zoom_phase()` (its fraction),
  `field_zoom_swaps()`, `field_zoom_speed()`, `field_zoom_rows()`,
  `field_zoom_running()` and `field_zoom_shown()`; the field itself,
  smoothly resampled, with `field_zoom_value(s)`, `field_zoom_color(s)`,
  `field_zoom_point(s)` and `field_zoom_buffer_value(b, next)`.
- The camera and projection, as the code has them: `camera_map()`,
  `camera_height()`, `camera_angle()`, `camera_forward()`,
  `camera_right()`, `orbit_radius()`, `camera_distance()`,
  `terrain_level()`, `projection_q()`, `projection(z)`,
  `terrain_frame()`, `terrain_row(z, h)` (the source row at which height h
  shows at depth z), `terrain_dir(column)` and `map_point(column, z)`; and
  `terrain_march(s, footprint, from_relief)`, the per-pixel march, through
  the original terrain or the relief, with `terrain_height`,
  `terrain_color` and `terrain_shown()`.
- The window: `window_rect()`, `in_window(s)`, `window_cover(s, blur)`,
  `window_point(s)`, `window_cleared()`, `cleared_seconds()`,
  `window_content(s)` (the original's window, smoothly) and
  `picture_level()`.
- The original's pictures: `picture_color(s, level)`,
  `picture_frame(s, level)` (the frame round the window, for its edge),
  `palette_entry(i)` and `palette_color(index)`.
- The Mandelbrot fit: `mandel_c(m)`, `mandel_potential(c, limit)`,
  `mandel_dep(m)` and `mandel_height(m)` by direct iteration in f32,
  `height_of_dep`, and the baked relief, `relief_at(m)` (height, gradient
  and distance outside the set) and `relief_height(m)`; the fit's own
  values are `mandel_centre()`, `mandel_scale()`, `mandel_aspect()`,
  `mandel_base()`, `mandel_slope()` and `mandel_plateau()`.
- Compositing: `FieldLayer`, `field_layer(c, a, z)`, `no_layer()`,
  `FrameLayers`, `field_composite`, `depth_z(xy, t)` and
  `tonemap_inverse`.
- The fluid: `fluid_load`, `fluid_sample`, `fluid_blend` (fragments only),
  `fluid_uv`, `fluid_source`, `fluid_cell`, `fluid_steps()`,
  `fluid_fraction()`, `fluid_dt()`, `fluid_seconds()` and
  `fluid_region()`; in the step, `fluid_step_number()` and
  `fluid_step_seconds()`.
- Music cues measured from the PCM, as in the earlier parts: `cue_energy()`,
  `cue_onset()`, `cue_low()` and `cue_accent()` (fragments only).

The fluid hook is the frame author's `FrameFluid` (`field_frame_smoke.rs`),
which `field_render.rs` creates with the frame's shape and the relief, and
brings to each moment shown. It owns its GPU state, its steps (one every 1
/ `FLUID.rate` seconds from `FLUID.start`, part 6's PSTART fade), the
states it keeps every `FLUID.checkpoint` steps for seeking back, and its
lighting. Its two light layers, `FLUID.size` texels over the source
rectangle `FLUID.region` (rows from the top), are bound for the fragments
as `fluid_state` (the smoke behind the frame) and `fluid_previous` (in
front of it); `fluid_uv` maps a source point onto them. `u.field.fluid`
holds the state shown, its fraction past the one before, the step's length
and the seconds since the fluid's start. The groundwork's single-texture
step, `frame_fluid` and `frame_fluid_init` with `field_style.wgsl`'s
`field_fluid`, remains but is unused, as do the helpers for it
(`fluid_load`, `fluid_blend`, `fluid_cell` and the step's own).

The zoom hook is the zoom author's `ZoomLevels` (`field_zoom.rs`), which
`field_render.rs` creates beside the fluid, with the zoom's stop as the
preview measures it, and brings to each moment shown (`advance_zoom`, after
the fluid); in the modern view's frames before part 5 it builds the first
level ahead (`prebuild_zoom`). It owns the reference orbit, the BLA tables
and radii, the level kernel and `LEVEL_SLOTS` level textures, and decides
which level is shown and how much of the next to build. The kernel,
`KERNEL_WGSL`, is a compute module of its own. It is kept in the Rust file
because every `.wgsl` file in `src/hd` goes into the renderers' modules with
the shared controls. Group 2 of the modern module's fragments is the zoom's:

- Binding 1, `zoom_level`: the shown level, rgba16float. Each texel holds
  its band phase, log2 of its distance estimate in texels, and its class:
  outside, inside, or at the step cap.
- Binding 2, `zoom_view`: the level's depth, its texels per window height,
  its size, and whether a complete level is bound.

Binding 0 is left unused: the shader test reserves `@binding(0)` for the
shared controls. Once the window is cleared no level is bound, and the
window draws what faithful HD does: colour 129, near black.

Two hand-overs frame the part. Part 6 ends on the PSTART picture, and its
look is part 6's own. Setting `FRAME_TAKES_PART6_FADE`, as the frame now
does, lets part 5's modern view draw part 6's fade instead, from 80.36
seconds, so the frame author can bring the lit metal frame in there:
`picture_level()` rises from 0 to 1 over it and `field_seconds()` is
negative. The fade is part 5's only with `--field`; `--rings` alone still
ends on part 6's own picture, as the look check's part-6 shots show it. Part 5 must end on the frame with a
dark, empty window, which part 3 zooms into: the window is cleared at
98.11 seconds, and from then on the zoom's layer should stay dark. Check the
look with `tools/look_check.py`, whose part-5 shots show each phase.

Part 3 takes part 5's last frame from part 5's own renderer, in both views,
while it holds the frame and zooms into its window. Its one hook is
`FieldBlock::part3_view`: `field_pixel` divides each pixel's screen position
by its x, the magnification about the screen's centre, before anything is
drawn, so the frame, its smoke and its window are resampled at every scale
rather than enlarged from a picture. It is 0 throughout part 5, which then
draws exactly as before, and `FieldPreview::controls` never sets it, so the
smoke's steps never see it. Anything that maps a source point back to the
screen without `field_pixel` (the frame's mountain shadow does) would not
follow the zoom; by then the mountain has gone.

### Part 3: Dots

```sh
cargo run --release --locked --features hires --bin panic-hd -- --dots --seek 130
cargo run --release --locked --features hires --bin panic-hd -- --dots --faithful --seek 139
```

`--dots` continues past part 5 through part 3, and part 4's wait, to the
first frame part 4 draws, with the music unbroken. It implies `--field`.
**M**, **Tab** and **C** work as in the earlier parts. The clip ends at
179.96 seconds, on part 4's logo slab in white, and holds there.

Read from the port, and checked against the engine on every frame by an
ignored test, part 3 runs like this:

- From 130.03 seconds it holds part 5's last frame, its window dark, until
  the music reaches 1E00.
- From 131.99 it zooms about the screen's centre: each frame resamples that
  picture at a scale that falls by 256 a frame from 65535, to 23256 at
  134.36, 2.82 times, when the window fills the 4:3 screen and all is black.
- From 134.37, the shade bobs. Each iteration adds 20 discs, 16 by 16
  pixels, along a path of two pairs of sines, and erases the ones added 16
  iterations before, so the overlap counts, through a palette that is blue
  up to 11, whitens from 13 and is white from 25, draw a ribbon. The port
  takes one iteration a frame. Once the music passes 1E3C, at 138.89, no
  more are added, and the last are erased by 139.10.
- From 139.11, 2,660 dot frames. 512 points take eight generated objects
  in turn: a sphere lattice, a vase (a solid of revolution), cube edges, a
  grid of lines, a 16 by 8 by 4 lattice, an 8 by 8 by 8 lattice, pyramid
  edges and a 490 by 200 by 100 box. Each stage lasts 300 frames (the
  sphere 500, the box 360), and in its last 129 its points move in a
  straight line to the next object's. The object turns by 3, 4 and 5
  1024ths of a turn a frame about its three axes and is carried along a
  92-point spline. Each frame projects the points to column 160 + 256 X / Z
  and row 100 + 256 Y / Z and sets one bit of one of four pages there, so
  a page shows eight echoes 4 frames apart, which its palette shows at
  blue 60, 45, 40, 35, 30, 25, 20 and 15 of 63, the newest brightest. The
  rotation stops from 176.27, leaving the box tilted by 70 degrees, and the
  spline brings it to rest by 177.10.
- Part 4 begins at 177.11 and waits, the box held, until the music reaches
  29:00 at 179.93. At 179.96 it draws its logo slab at 400 lines, white,
  where the box landed.

The ignored test checks every replayed frame and palette against the
engine's and the PCM, that each dot frame's newest bit is set exactly where
column 160 + x and row 100 + y put its points and nowhere else (the code's
tables send anything off the screen past the page, and no point wraps back
on), and that the camera evaluated in floating point is within 7.9 camera
units of the port's points.

The preview keeps part 3 as compact state per frame (`Dots::hd_state`): the
phase, the zoom's scale, the shade iterations and their stop, the stage, its
counters and the angles, with the camera-space points each dot frame drew
(8 MB) and the scene's tables (`Dots::hd_tables`). The original frames are
replayed on demand from checkpoints every 64 frames (about 50 MB), and part
4's two pictures are kept.

The port's pacing differs from the original's. In a DOSBox-X capture of the
original the shade bobs run about 40 iterations a displayed frame and are
over in about 0.13 seconds, and the dots start about 4.6 seconds earlier
against the music. The preview follows the port, which gives the shade bobs
4.7 seconds; their timing is a choice, not recovered.

Some of this is interpretation:

- Between the original's frames the zoom's scale is linear, the shade path
  is evaluated at continuous iterations with a floating-point sine (the code
  tabulates whole indices and truncates), and the dots' rotation, morph and
  spline are evaluated at continuous frames in floating point, without the
  code's Q7 coefficients and truncation. Faithful HD keeps the port's own
  points, linear between its frames.
- The zoom read as a dolly at constant speed (its scale, the distance to
  the picture, falls linearly), and the landed box as anticipating the logo
  slab, which part 4 draws in nearly the same place.

Faithful HD shows the original's look at display resolution, in the 4:3
frame. The hold and the zoom are part 5's own faithful HD at the original's
magnification. The shade bobs are each kept iteration's discs where the
port's path table puts them, as smooth circles, their overlap counted and
coloured through the shade palette; between frames the newest iteration
fades in and the oldest out. The dots are the eight echoes, each the port's
points, projected as the code projects them, as round dots of a source
pixel's area at the port's echo levels, the brighter showing where they
overlap. Part 4's slab is smoothly upscaled.

The modern view is light painting: a long exposure in a dark, faintly hazy
space, as a camera on a tripod would take it. All of it is authored, on the
port's timing, positions and motion.

- The zoom flies through the window. Part 5's own modern view is drawn at
  the zoom's magnification, by the original's law, and carries on past the
  original's last scale until the frame has left the drawable (on a 16:9
  screen about 0.3 seconds more, to 3.8 times), blurred along the zoom over
  a 1/80 second exposure and going out of focus as it nears.
- The shade bobs are a fibre-optic brush. Each of the 20 discs is a tip
  that sweeps the path continuously, its glow exposed for the 16 iterations
  the original keeps, so the overlap count becomes the exposure. The
  film's response is the original's palette: through the shared tonemap a
  count shows the palette's colour, blue rising, burning to white where the
  overlaps pile up, and past the palette's white the light keeps rising
  into the bloom. Each tip also draws its own fine fibre, bright at its
  head.
- The dots are 512 lights, each exposed continuously along its path for the
  32 frames the original's echoes span, brightest at the head and fading at
  the rate the echo levels fall, then a faint afterglow for 2 seconds more,
  the long exposure's memory. The exposure along each piece of path is
  exact for a disc moving at constant speed, so faster lights leave fainter
  trails and still ones burn brighter, as in a photograph. The lens is
  focused on the object's centre: a light out of focus is a disc of light,
  and its trail a swept disc. Where the discs are large and bright, near
  each head, they take the shape of a seven-bladed diaphragm with rounded
  corners, clipped to a cat's eye by the barrel toward the edges of the
  picture; that too is exact, as a line meets a convex shape in one span.
- The air holds a faint uniform haze and drifting fog, both lit by every
  light. The light from all 532 lights (the dots and, before them, the
  shade bobs' tips) is gathered each frame in a grid of 96 by 54 cells
  across the picture and 48 deep, spaced evenly in log depth: each light's
  power over its distance squared, softened over the cell's size and dimmed
  with the distance, a little for the fog and more for the haze. Each
  pixel, at half resolution, marches 48 steps through the grid, which
  gives the haze as single scattering in a uniform medium, and where the
  fog has wisps, the fog's. The fog is tileable noise twisted by a second
  noise, each drifting slowly, so it is a function of the clip's time.
  Where the grid holds no light the march takes no noise.
- The music: the lights breathe with the PCM's low band at the moment of
  each part of the exposure, and each order's first row, read from the
  music's position, sends a plane of light through the object along a
  direction of its own, stronger where the PCM shows an onset. The waves
  light the lights and their trails, not the air.
- The water: part 4's (`water.wgsl`, shared) lies beneath the objects:
  its reflectance, raised by the mirror's gain to 1 - (1 - F)^gain (about
  gain times F where F is small, as in part 4, and never past a perfect
  mirror near grazing), its swell at 0.35 of part 4's height (this level
  view meets it near grazing, where the same slopes turn the mirror image
  much further) and its drops, from 2 s after the dots begin until the
  rotation stops, below the objects. The trails and the shade tips are
  painted again at their mirror images, at the depth those lie, so as far
  out of focus; where each pixel's ray meets the water the composite takes
  that image, moved as the slopes turn the reflected ray and spread over
  the slopes too small to draw. The air's march goes on past the water
  through its mirror image (the grid's cells under the water gather the
  light at their mirror images). The objects fly along the camera's axis,
  about 500 to 2,300 units out, and the water lies just below them, tilted
  3 degrees so it rises further off, rising through the part, eased, from
  400 units below the camera to 100 at part 4's slab frame, where part 4's
  ground takes it on (`dots_water.rs`), so they fly ever deeper through
  it. A light under it is seen bent at the surface by Snell's law, through
  the swell, the drops and the ripples, fading over 110 units of water and
  blurring as the water scatters its light, which lights the water from
  below. The lights moving through the water set off ripples along their
  paths, as a drop sets off rings, as hard as they move and less the
  deeper they run, and stir its plankton, which glows white blue along the
  path behind them and then where the rings pass, fading with its
  lifetime. Shift with the number keys tunes the water's tilt and depth,
  the plankton's lifetime and brightness, the glow under the water and the
  ripples (all authored, chosen by eye).
- At the end the rotation stops, the box's trails collapse into its lights,
  which burn brighter, and in the second before part 4 draws they flare and
  swell. Part 4's slab takes over at its own colours through the tonemap,
  with a flash that fades, so the clip ends on the original's white slab.

Everything is a function of the clip's time, so seeking, captures and video
agree; an ignored test checks frames drawn after others against a new
renderer's, byte for byte. Each frame a compute pass takes 105 samples
along each light's path and 65 along each tip's and another gathers the
air's light, the lights are drawn as 53,000 swept discs and again at their
mirror images, the haze and fog
are marched at half resolution, and a composite takes part 5's view during
the zoom. The look's constants are at the top of `dots_light.wgsl`; the
fog's noise is made once, on the GPU, when the renderer loads.

At 2560 × 1440 on an Apple M5 Max, as `--bench` measures it (150 frames,
on a machine in use), the shade bobs and the dots take 2.4 to 4.3 ms a
frame, the flare the most, before the water; its mirror image adds about
0.5 to 0.9 ms (at 151 s, 3.3 to 3.8 ms; at 176 s, 3.5 to 4.3). The hold and the zoom, which draw part 5's
frame and smoke beneath, take 7 to 9 ms, about what part 5's own last
seconds take. Faithful HD takes 0.3 to 0.8 ms. Gathering the air's light in
the grid and marching it, rather than summing each light's closed-form
glow at every pixel, cut the dots' frames from about 8.5 ms to 3.6 ms.

### Part 4: Plasma

```sh
cargo run --release --locked --features hires --bin panic-hd -- --plasma --seek 177
cargo run --release --locked --features hires --bin panic-hd -- --plasma --faithful --seek 186
```

`--plasma` continues past part 3 through part 4 to part 7's first frame,
with the music unbroken. It implies `--dots`. **M**, **Tab** and **C** work
as in the earlier parts. Part 3's preview ends on part 4's first drawn
frame, its slab in white at 179.96 seconds; part 4's preview begins on the
same frame. The clip ends at 225.43, on part 7's first frame, which is
black, and holds there.

Read from the port, and checked against the engine on every frame by an
ignored test, part 4 runs like this:

- From 177.11 it waits for 29:00 (part 3's preview shows that). At 179.94
  it clears video memory and copies in its logo, 320 by 180 pixels shown
  at 400 lines from raster 237, and from 179.96 fades the logo's colours
  from white over 64 frames, to 180.86.
- Then it writes its index field: 200 columns by 223 rows of 200 bytes,
  each column a sine band, `(sine(5.1 x) / 10 + 200) mod 224`, rising one
  colour a row. Colours 32 to 255 are one period of a sine, 0 to 62, at a
  level from 0 to 63, the green channel 30 entries and the blue 40 behind
  the red, all three turned by one rotation that swings back and forth
  over 1,050 entries, at up to 9 a frame. The palette a frame shows was
  filled the frame before, from the rotation and level of the frame before
  that.
- The first 225 rasters each move the start address on by twice the sum
  of two entries of a stride table, plus 200: two reads 2 entries further
  and 2 back each frame, so two waves run in opposite directions down the
  rasters. The table alternates nearly opposite values, so the even and
  odd rasters are two copies of the field, pushed apart by up to 90 bytes,
  each displaced by its own smooth offset that grows down the screen from
  a fixed top raster. Raster 226 jumps 510 bytes, past the field, so
  rasters 226 to 236 are black.
- The level stays 0 until the music passes 2A00, then rises a step every
  four frames, from 187.51 to 191.06; once the music passes 2E14 it falls,
  from 219.33 to 222.87.
- From 2F00, at 224.23, the line compare moves up a source row and then
  down a row a frame, so the slab slides off the screen by 225.41. Part 7
  is made in the next step and its first frame, at 225.43, is black.

The preview keeps part 4 as compact state per frame (`Plasma::hd_state`):
the counters each frame's step rendered with and the palette it showed,
with the scene's tables (`Plasma::hd_tables`) and part 7's first frame.
Each original frame is rebuilt on demand from that state, with the start
addresses `plasma::raster_starts` gives, the function the scene's own
render uses; the ignored test checks every frame's pixels and palette,
that the rotation and level rebuild the plasma's colours from the bank,
and the PCM.

Some of this is interpretation:

- Between the original's frames the start addresses (where they move a
  little), the rotation, the level and the line compare are linear, and
  the field and the palette's sine are evaluated at continuous positions.
- The even and odd rasters read as two layers of the plasma, the stride
  table as two waves running through a stack of lines.
- The logo read as a pinhole view of a right-angled block on square
  display pixels (`plasma_stone.rs`). Measured when the preview loads, the
  silhouette's side edges are straight to 0.3 columns, and the vanishing
  rows of the top face's edges and the front face's give the camera's
  focal length and the block's tilt: 262.8 rows and 31.3 degrees. Taking
  part 3's box's width (490), the near edge is 504 units away, and the
  depth that puts the far edge on the logo's first row is 193.9, within 3%
  of the box's 200. The logo shades its letters as cut in, dark on the side
  the light comes from, so where each face, unwarped from the view, is
  darker than its stone it is carved; thin dark lines are cracks.

Faithful HD shows the original's look at display resolution, in the 4:3
frame. Each of the 400 rasters reads the field from its own start address
at a continuous column, so the colours are smooth along a raster and the
rasters keep their offsets, the original's interlaced look; the palette
turns and fades between frames. The logo is its pixels' colours,
interpolated, in the palette each frame shows, with the line compare at a
continuous raster, so the fade and the slide are smooth. With **Tab** or
**C** the original is shown at its own 400 lines.

The modern view is light on stone. The original keeps its two halves
apart; here they share one dark space, and the plasma is the light the
stone is seen by. All of it is authored, on the port's timing, positions
and motion, and on the block fitted to the logo.

- The slab is a block of part 3's box's width, the logo's depth and the
  box's height, in the logo's place, seen through the camera the logo's
  perspective implies, ray-marched as a distance field: the block with
  rounded edges, less the carving. Each face's carving is unwarped from the
  logo when the preview loads, 1024 texels across: where the face is darker
  than the threshold between its two tones it is cut, 5 units deep a bevel
  of 8 in from the edge, its outline smoothed; thin dark lines are cracks
  1.4 deep; and the stone's colour and mottling are the logo's, flattened
  of its painted light. The faces are polished and the cut and the cracks
  rough, with a fine grain.
- The plasma is two sheets of light standing on the floor behind the
  slab, both from the line where the floor meets the plasma's last raster,
  leaning over the slab, the back one less, so it stands further back as
  they rise, wide enough for a 21:9 drawable: the original's two
  interlaced copies of the field, the even rasters' and the odd ones', a
  sheet each, sliding past each other as the strides move them. On each,
  at each point, the colour the original's copy shows there, made
  continuous. Each copy's displacement along the field's rows (a raster's
  start address less 200 bytes a raster) is smoothed down the screen, a
  Gaussian 3 of the copy's rasters wide, and over a frame either side, and
  taken between its samples by Catmull-Rom, so it is a smooth function of
  height with no line structure (the address itself is not interpolated:
  half way along a row is half way across the field). The field is taken
  as a seamless helix, its sine a whole turn over the 200-byte row (the
  code's 5.1 a column falls 4 of 1024 short) and its rise of one a row
  spread along the row, so running on into the next row leaves no seam.
  The palette's sine is taken without the bank's truncation, its rotation
  and level smoothed over time (1.5 and 3 frames), over a shutter of about
  1/120 s along its turn, and everything is interpolated between the
  original's frames, so the sheets glide at any display rate. Each sheet's
  colour is taken back through the shared tonemap by its luminance, at
  half power, so the two together give the light the original's two copies
  do and their brightest bands bloom. The front sheet is sharp; the back
  one, through it, is drawn from its light's texture over a circle of
  confusion 4 source rows across; both glow softly over themselves and a
  little past their bottom edges. A faint dither keeps the output from
  banding.
- The sheets light the stone. Their light is drawn each frame at 512 by
  256 over each sheet and averaged down to 4 by 2; each pixel of stone
  takes the exact form factor of each of 32 cells of the front sheet and 8
  of the back one times the cell's mean radiance, shadowed by the carving
  toward where the light mostly comes from, and the polished faces mirror
  both sheets, sharply where polished and blurred in the cut, at the level
  of each sheet's light that matches the reflection's spread. A dim, warm key light from the upper
  left, where the logo's painted light comes from, keeps the stone lit
  while the plasma is dark and dims to a fifth as the plasma rises.
- A mist hanging at the sheets scatters a little of their light, so it
  spills softly below their edges.
- The ground, which the original lacks, is part 3's water reaching back
  to the sheets, which stand in it, the plasma carried on down them to it:
  calm water mirroring them, with an occasional drop, or with **D**
  polished dark stone. It lies as part 3 left it, drains from 186 to
  200 s, uncovering the slab, and over the plasma's fade out (219.33 to
  222.87) drains away to 40 units below the slab's lowest edge. The number
  keys tune the water (part 3's too). It shows only by their light, so only while the plasma is lit
  ([docs/part4-plasma.md](../docs/part4-plasma.md)).
- The arrival: on the slab frame part 3's own modern view, its box flared,
  is still drawn, and gives way to this one over 0.35 s. The stone comes in
  overexposed, white rather than its colour, and the exposure settles over
  the original's 64 frames, so the carving, the darkest, comes out of the
  white first.
- Each order's first row swells the sheet's light a little.
- From the slide's first frame, with part 7 loaded, the slab breaks up
  where it rests into part 7's dust instead of sliding away, every mote
  rising from it and settled by 228.54 s
  ([docs/part4-plasma.md](../docs/part4-plasma.md#the-break-up-into-part-7s-dust)).
  Without part 7 (`--plasma`) the slab sinks along the camera's vertical
  so its far edge, the top of its silhouette, follows the original's line
  compare, the floor and the dark sheets with it.

Part 3's view during the handover is part 3's renderer at a moment past
its last frame, which it already handles (its slab frame is held and the
flare's light decays): part 3's code is unchanged. Everything is a
function of the clip's time, so seeking, captures and video agree; an
ignored test checks frames drawn after others against a new renderer's,
byte for byte. The look's constants are at the top of `plasma_light.wgsl`
and `plasma_stone.rs`.

At 2560 × 1440 on an Apple M5 Max, as `--bench` measures it (150 frames,
on a machine in use), the plasma's seconds take about 5 ms a frame (median
4.5 to 5.0, slowest 6 to 14), the slab alone 1.7 ms, the slide 0.9 ms and
the handover, which also draws part 3's view, 5 ms. Faithful HD takes 0.3
to 0.4 ms.

### Part 7: Greetings

```sh
cargo run --release --locked --features hires --bin panic-hd -- --greetings --seek 224
cargo run --release --locked --features hires --bin panic-hd -- --greetings --faithful --seek 228
```

`--greetings` continues past part 4 through part 7 to part 8's first
frame, with the music unbroken. It implies `--plasma`. **M**, **Tab** and
**C** work as in the earlier parts. Part 4's preview ends on part 7's first
frame, which is black, at 225.43 seconds; part 7's preview begins on the
same frame. The clip ends at 242.30, on part 8's first frame, the monster,
and holds there.

Read from the port, and checked against the engine on every frame by an
ignored test, part 7 runs like this:

- It loads the four TRUG maps into the four planes: the first 16,000
  bytes of each are the 320 by 200 sky, and glyph `g` starts at 0x3e90 +
  0x700 g, 16 rows of 112 bytes, each row 16 views of the letter, 7 bytes
  (28 pixels) each. The sky's stars are colours 192 to 255; they never
  move. Each frame turns those 64 colours one step through a pulse,
  `max(0, 3 min(j, 63 - j) - 33)` as (v, 8v/9, 7v/9), scaled by an
  intensity that rises over the first 64 frames (to 226.37).
- Five strips, 28 pixels wide at x = 8, 72, 136, 200 and 264, rise one row a
  frame. Each strip has a queue of 201 row descriptors: a glyph row's
  address, copied with a view chosen per row by a sine (601 entries,
  `(trunc(40 sin(pi i / 200) + 60) & 15) * 7`, read from position + row,
  the positions falling one a frame); or 0xf490, which copies the sky 28
  pixels to the right; or above that, which leaves the row as it was. A
  letter takes 18 rows, 15 of glyph and 3 of padding. The padding rows use
  the first strip's text for every strip, so while it shows `[` they are
  left alone and each letter's bottom row smears down over the gap.
- The words are GREETINGS, GO TO, RENAISSANCE, TRITON and SONIC, entering
  one after another from 228.79; the last leaves the top at 236.73.
- The loop ends at its 1,100-frame limit, at 241.16 (its music cue, 341E,
  is never reached; a debugger trace of the original shows the same). A
  feedback fade turns every colour toward white in 15 steps; then
  MONSTER.U comes out of the white over 65 steps, from 241.37. Part 8 takes
  over at 242.30 with the monster on screen.

The preview keeps part 7 as compact state per frame
(`Greetings::hd_state` and `hd_drawn`): every strip row's source in video
memory, the palette each frame shows, the star colours' phase and
intensity, the stage and the fade's levels, with the TRUG planes and
MONSTER.U. Each original frame is rebuilt on demand from that; the
ignored test checks every frame's pixels and palette, that the phase and
intensity rebuild the star colours, and the PCM.

Some of this is interpretation:

- The 16 views read as a square column of side about 18 pixels turning 90
  degrees, the same letter on every face: the faces' widths across the
  views follow an orthographic turn. So each letter is a cube, and the
  sine's view index is its turn, 5.625 degrees a step, here continuous.
- The stars' colour indices run once round (160, 85), less 64 / 2 pi
  times their angle plus 17.8, scattered by about 12 (a fit). Under the
  real palette their brightness centroid turns 5.65 degrees a frame round
  that centre, so the cycle reads as a light turning once every 64 frames.
- The feedback fade reads as the picture overexposing.

Faithful HD shows the original's look at display resolution, in the 4:3
frame: every pixel's colour index is the original's (the smear and the
copied sky included), the stars drawn as round dots, the letters' colours
interpolated, the palette turning between frames; the monster is
upscaled smoothly. With **Tab** or **C** the original is shown.

The modern view is crystal letters in a lighthouse's beam, in dusty air.
All of it is authored, on the port's timing, positions and motion:

- The sweep is one broad cream lighthouse beam. Its direction on the
  screen is the peak of the original's band round the sweep's centre, but
  it turns at its own speed (authored, chosen by eye): one turn every 128
  frames (about 1.83 s, half the original's speed), quickening from 237 s,
  its turn shortening linearly to 32 frames (twice the original's speed)
  by part 8's first frame. Its phase integrates that speed, matched to the
  original's as the fade to white begins, so there its direction is the
  original's. It is tilted 51
  degrees toward the camera (chosen by eye), so it reads as coming our way,
  the lens catching only the faint outer edge of its cone on each turn:
  only the swing into the lens at the end points at the camera. It shines
  at a quarter of its full light, and from 237 s brightens linearly to its
  full light by part 8's first frame (both chosen by eye). **J**/**K**,
  **U**/**I**, **G**/**H** and **Z**/**X** change the turn's length, the
  tilt, the beam's starting brightness and its width while the preview
  plays. The lamp stands a little behind the strips, where it projects on
  the frame's middle, 15 rows below the sweep's centre (chosen by eye;
  **O** puts it on the sweep's centre): a bright core with a halo, the
  beam leaving an aperture of finite size. The original lights 40 of its 64
  colours at once, a very broad band, so the beam is broad and soft: across
  it its light falls as exp(-2 q^2), q its distance across over the
  aperture plus 0.55 rows a row along, about 32 degrees wide at half its
  light and fading out by some 43 degrees from its axis.
- The air is a participating medium, marched in 3D at half resolution
  within the cone (28 samples a pixel), lit by the beam: its light falls
  off with the square of the distance, and it scatters toward the camera by
  a Henyey-Greenstein phase function (g = 0.62), so the beam brightens as
  it turns toward the lens. Each sample is at its own instant over a
  shutter of one 70 Hz frame, so the beam sweeps rather than steps. The
  lamp's own glow also lights the haze everywhere, faintly (the light of a
  point lamp in uniform air along each ray, in closed form), so the room
  reads as dusty air with a beam turning in it, not a void.
- The air moves (authored; the original's stars never move): a slow drift
  of about a row and a half a second, and lazy swirls, the curl of a sum of
  four waves 260 rows long reaching 26 rows, with periods of 23 to 37
  seconds, and over the places they have carried the air, the same 90
  rows long reaching 7, quicker; being curls, they neither gather nor thin
  the air. The air's thicker and thinner regions (a smooth, tiling noise)
  are carried by the same flow.
- The part begins before its first frame, while part 4's slab breaks up
  into its dust (part 4 above), its light coming up 1.19 s early; from
  228.54 s the frames are as before.
- The dust is motes in the air, seeded from the original's stars: each
  star's screen position, six motes at depths along its ray from in front
  of the strips to behind them, filling the air evenly, so at rest they
  would project where the original shows its stars. The air's flow
  carries each along its own path. Each is lit by the beam, forward
  scattering strongly (g = 0.75), and faintly by the lamp's glow
  everywhere; the camera is focused on the strips, so near motes are soft
  discs and far ones fine points, of varied sizes.
- Each letter is a block of clear cut crystal, like a lighthouse's lens,
  the letter cut into its four faces as a V-groove (walls sloping two rows
  deep a row in, down to 2.4), frosted, as in an engraved crystal award:
  the original's reading of its 16 views, the same letter on each face of
  a turning column. The blocks are rigid, each turned at its centre by the
  original's sine there (5.625 degrees a step of its view index,
  continuous). The original turns each row by its own step, up to 53
  degrees across one letter; as a solid that would shear the letter and
  warp its faces, so rigid blocks keep the letters whole and the faces
  flat, which reads and refracts cleanly, and the stack still ripples as
  the original's does. The cut glows softly by the lamp's light carried in
  the glass all the time, its walls brighter than its floor, so the words
  read in the dark; the clear glass is dark, reflecting the lamp in glints
  and the room in its faces, and refracting what lies behind with
  dispersion at three wavelengths (a dense flint's, a little exaggerated),
  out through the faces or reflected inside where it must be, so the lamp
  seen through a block splits into colours. Its edges and its cut flare
  when the beam passes through. The original's white-to-teal tints the
  glass and the cut faintly. The gaps between the blocks stay open;
  faithful HD keeps the smear.
- The beam is traced through the blocks: a photon for each of 600 by 600
  directions round its axis, at each of three wavelengths, refracted at
  each face and at the cut's facets (from the letter's distance field),
  internally reflected where it must be, the frosted cut scattering most of
  what meets it. Each photon that meets a block takes its light away where
  its straight ray would have gone and puts it where it lands, in a map of
  directions from the lamp, as a spotlight's shadow map is, in colour. The
  air and the dust read their light from the beam's profile plus that map,
  softened over the aperture's penumbra: the blocks' shadows and the
  letters' shadows in the beam, and coloured caustics, in it and sprayed
  out of it (a second, sparser march outside the cone gathers those).
  Where the photons gather, the light is shown 8 times brighter (authored:
  the true spray is too thin to see). Only the splats that move light are
  drawn, listed in their order, so the map adds up the same light in the
  same order as drawing them all; where the photons can move none (outside
  the words and the white, with the stars dark, or with no letter in the
  frame shown: before the words rise and after they have gone) the trace
  is skipped and the map only cleared. An ignored test checks the map,
  made either way, against every photon's splats, byte for byte, at every
  frame of the part and halfway to the next.
- The letterforms are authored outlines, drawn for this remake in the
  original's spirit (greetings_letters.txt and tools/letters/; they are not
  the original's glyphs): each letter's signed distance is computed from
  its contours when the preview loads, its box filling 80% of a face. The
  original's letters, derived from the TRUG maps, remain in
  greetings_type.rs for comparison; faithful HD reads TRUG as before.
- The end: the beam swings to the lens from 0.45 s before the original's
  feedback fade (authored motion), its forward scattering and the lamp's
  flare flooding the picture, which the fade then blends toward white at
  the original's timing and levels; the monster, upscaled, develops out of
  the white at the original's levels.

Nothing follows the music, as in the original. Everything is a function of
the clip's time; an ignored test checks frames drawn after others against
a new renderer's, byte for byte. The look's constants are at the top of
`greetings_light.wgsl`, the letterforms' in `greetings_type.rs`. The
monster's frames are drawn by a module of their own,
`greetings_monster.wgsl`.

At 2560 × 1440 on an Apple M5 Max, as `--bench` measures it from 226 s
(on a machine in use), modern part 7 takes about 3.6 ms a frame (median;
about 5.5 before the photons were skipped and listed), the photons, the
air's marches, the crystal and the shutter included; faithful HD under
1 ms.

### Part 8: the ending

```sh
cargo run --release --locked --features hires --bin panic-hd -- --ending --seek 241
cargo run --release --locked --features hires --bin panic-hd -- --ending --faithful --seek 246
```

`--ending` continues past part 7 through part 8 to the demo's last frame,
with the music unbroken: the monster held, then a monitor switched off. The
modern view is an endless wall of old green surveillance monitors the
camera scrolls out from, all switched off together. [Part 8's
notes](../docs/part8-ending.md) say what is read from the port, what was
observed, what is interpretation and what is authored.

### Options and tests

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
frame, part 6's state, replayed frames and PCM against every part-6 frame,
part 5's against every part-5 frame, part 3's against every part-3
frame, part 4's against every part-4 frame, part 7's against every
part-7 frame and part 8's against every part-8 frame; one bakes the frame's
shape from PSTART and checks what it measures.
Two more need a GPU: one checks that part 5's smoke reaches the same state
however a moment is reached (it re-simulates from the start at each of
nine moments, a minute and a half), the others that part 3's, part 4's,
part 7's and part 8's frames are the same however they are reached (and
that part 8 starts on part 7's last frame):

```sh
cargo test --release --locked --features hires
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked \
  --features hires reference_timing_audio_and_flyby_projections -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires transition_matches_engine_frames_audio_and_schedule -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires scroller_glyphs_and_positions_rebuild_every_reference_frame -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part6_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part5_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires frame_is_measured_from_pstart -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part5_fluid_is_the_same_however_it_is_reached -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part3_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires the_clip_ends_on_part4s_slab -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part3_frames_are_the_same_however_reached -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part4_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires the_clip_ends_on_part7s_first_frame -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part4s_sections_start_where_its_labels_change -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part4_frames_are_the_same_however_reached -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part7_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part7s_letters_are_the_texts_in_order -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part7s_sections_start_where_its_labels_change -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part7_frames_are_the_same_however_reached -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part8_state_matches_every_engine_frame -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part8s_sections_start_where_its_labels_change -- --ignored --nocapture
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires part8_continues_part7_and_its_frames_are_the_same_however_reached -- --ignored
```

`--bench N` draws N frames from `--seek`, a sixtieth of a second apart,
without a window, and prints the time a frame takes, CPU and waiting for the
GPU included:

```sh
./target/release/panic-hd --field --faithful --bench 120 --seek 105 --size 2560x1440
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
ignores. Its 58 shots cover the modern intro letters, the plate and doors, the
logo, the laser cut, the scroller with each backdrop and lighting mode, parts 6,
5 and 3 through their phases, faithful HD and the original view, at 960 × 540. Their
times are authored test points, not cues from the original. `list` shows
them, and `--only` selects shots or groups by name, with wildcards
(`--only scroller,laser` or `--only 'intro-7*'`).

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
Rendering takes about 0.2 to 1.2 seconds a shot. Comparing takes under a
second for each changed 960 × 540 shot, and shots are compared in parallel.

## Timing and platform notes

The player runs on a fixed 70 Hz scene timer with 16 kHz stereo audio mixed in
512-byte blocks. Scene arithmetic and sequencing follow the original, but real
VGA and PIT timing isn't emulated, so frame timing won't match every DOS
machine exactly. A full headless run takes 17,442 timer steps (249.171429
seconds).

It's been tested on Apple Silicon Macs. It should build elsewhere, but display
and audio on other platforms haven't been checked yet.
