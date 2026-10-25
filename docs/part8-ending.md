# Part 8: the ending

Notes on part 8 in the HD preview (`panic-hd --ending`): what is read from
the port, what was observed, what is interpretation and what the modern view
authors. The preview's other parts are described in
[rust/README.md](../rust/README.md#experimental-hd-intro-and-scroller).

```sh
cargo run --release --locked --features hires --bin panic-hd -- --ending --seek 241
cargo run --release --locked --features hires --bin panic-hd -- --ending --faithful --seek 246
```

`--ending` continues past part 7 through part 8 to the demo's last frame,
with the music unbroken. It implies `--greetings`. **M**, **Tab** and **C**
work as in the earlier parts. Part 7's preview ends on part 8's first
frame, the monster, at 242.30 seconds; part 8's preview begins on the same
frame, and part 7's renderer draws it, so the hand-over is exact in both
views. The clip ends at 249.17, on the demo's last frame, and holds there.

Read from the port, and checked against the engine on every frame by an
ignored test, part 8 runs like this (steps count from the first that reads
the music at 35:00):

- It keeps the first 16,000 bytes of each plane, MONSTER.U as part 7 left
  it, and its palette, and clears the rest of video memory. It holds the
  picture until the music reaches 35:00: the first step that reads it
  comes at 246.36 and shows the monster again.
- Step 2 turns colour 0 white, so the black behind the monster (and its
  eye and mouth) turn white; step 3 shows the cleared memory, all white.
- Steps 4 to 7 squeeze the picture with the CRTC: start 16000, stride 2 to
  5 rows, and a line compare that starts the picture again from the top
  memory below the middle, so every second to fifth row shows, centred.
  Steps 8 to 20 copy every 7th, 8th, ... 44th row round row 100 instead
  (the compressions 7, 8, 9, 10, 12, 14, 16, 19, 22, 26, 31, 37 and 44).
  Each step also lifts every colour but 0 toward white,
  `c' = s + c (63 - s) / 63`, by the step before's compression.
- Step 21 copies the top row over rows 98, 99, 101 and 102, leaving source
  row 100 as a line. Steps 22 to 67 write the top row's last four pixels
  (held in the latches) over four columns at each end of the line, 3
  columns further in each step, from columns 20 and 300; the monster's row
  100 holds colour only in columns 44 to 268, so the line starts to
  shorten at step 29.
- Step 68 sets one pixel, (160, 100), to colour 1, white. Steps 69 to 128
  set colour 1 to `31 cos(pi i / 20) + 32`, i = 0 to 59: white, dark, white,
  dark, a pulse 40 steps long. Then the scene waits 70 more steps (the
  original's one-second sleep) and the demo ends.

The preview keeps each frame's step, its palette and the distinct pictures
the frames show (`Ending::hd_frame`), and reads the compression each step
shows and the line's ends as each step erases them from the scene
(`ending::hd_compression`, `hd_line_ends`); a default test checks those
against the scene stepped on an invented picture.

Observed in a DOSBox capture of the original: the white behind the
monster shows for two or three frames, no whole white frame appears, and
the first squeeze shown keeps every third row. Why is not established; a
guess is the timing of the CRTC and palette writes against the retrace,
which the port groups at each retrace wait. The rest of the stages and
their timing match the port's: the dot is set 66 frames after the white in
both, and the exit to DOS comes 194 frames after it, where the port's demo
ends 196 frames after it.

Interpretation: the stages read as a monitor switching off. Nothing in
the code names them.

Faithful HD shows the original's look at display resolution, in the 4:3
frame: every pixel's colour index is the original's on every frame, upscaled
smoothly as part 7's monster is, the palette moving between frames as the
original's changes (the lift toward white, the dot's pulse).

The modern view is an endless wall of old green surveillance monitors,
switched off. All of it is authored, on the port's pictures, palettes, stages
and timing:

- The close-up. Part 8's first frame is part 7's last, which part 7 draws.
  Each set's picture is drawn smaller than its face, at 0.7 of it, centred,
  the rest of the raster lit but empty. The camera starts where the centre
  set's picture fills the frame's height as part 7's picture does (the
  empty raster, still black, at its sides) and holds close, barely moving
  for the first second, while over 0.64 seconds the picture becomes an old
  monitor's, strongly: the original's 200 lines as scanlines with dark gaps
  between them (each line's spot wider as it is brighter), an aperture
  grille of red, green and blue stripes (a triad to each source column), the
  colour bleeding a little along the lines, bloom and halation on bright
  parts, convergence error, a bowed raster and darker corners, and
  everything graded toward a green-yellow phosphor (yellow-green mids,
  green-black shadows, pale green highlights), keeping enough of the
  picture's own colour for the monster to be recognised. The empty raster
  round the picture glows faintly, its scanlines and stripes carrying on
  across it, so it reads as dark, lit glass.
- The scroll out. The camera then draws back and never stops, eased in on
  the logarithm of its distance: that speed rises from rest over the first
  220 frames (a smootherstep) and keeps rising gently to the end, so the
  zoom out starts softly out of the centre screen and keeps accelerating,
  with its momentum through the switch-off. It shows the empty glass round
  the picture first, then the bezel, then the wall: about 1.5 sets across
  the frame at 244.4 s, 3.5 at 245.2 s, 9 at 245.9 s and 16 across (some
  190 sets in view) at the switch-off at 246.36 s; the dots appear with
  some 57 across and the pulse ends with about 185. The face becomes a
  tube's, its glass bulging into bowed sides and deep corners, set deep into
  a thick chamfered bezel with a row of buttons and a knob below it, and
  more and more sets come into view in every direction; the wall has no
  edge (the sets are ray-marched, one every 376 by 300 rows). Where a tube
  is too small for its 200 lines or its stripes, coarser lines (2, 4, 8 ...
  rows a line) and stripes take over, the stripes easing back so the lines
  lead, and far pictures are taken from a filtered copy on each frame's
  palette, so it still reads as a CRT, never aliasing. Where the sets are
  small, the controls sink into the bezel and the glass stops showing the
  rim, smoothly.
- The sets. Every set shows the whole picture, centred and upright in its
  screen, with the same margin of empty glass round it. They differ only a
  little, fixed per set: a little brighter or dimmer (88 to 100 percent) and
  a little warmer or cooler green. (The previous version also shifted,
  turned and sized each set's picture and gave some a rolled, skewed,
  tinted or dimmed picture; these read as mistakes and are gone.)
- The light. There is no room: the screens are the only light. Each set is
  lit in green by its own screen (twelve patches, from the original
  picture's mean colours), its eight neighbours' and their dots, and the
  wall's light at large, on its chamfers, rims, tunnels and controls; the
  glass shows the lit rim in its curve, a faint glare and a little
  scattered light. As the screens go dark, so does the wall.
- The switch-off, on every set at once, on the original's stages and
  timing, within each set's smaller picture, centred on its screen. The
  white behind the monster and the white frame flash the pictures in pale
  green and light the wall. The pictures squeeze with the
  original's compression and palettes, so they lift toward white, their
  light rising with the compression to the power 0.35 as the beam's lines
  crowd together. The lines keep the original's row and erased ends,
  brightening as they shorten. The dots are glowing points of green
  phosphor following the original's colour 1, so they pulse as the
  original does, with a slower afterglow that decays through the last
  second: an endless field of dots. The line and the dot keep a least size
  on the frame (as a 1440-line frame shows them; other sizes keep that
  light), so they read on the small tubes; widened, a dot keeps its peak up
  to 30 times its area and from there keeps its light, so the far field
  stays points of light in the dark. Each dot is drawn from the sets
  nearest the ray, so it stays round however small its set, and where the
  sets are small the squeezed pictures and the lines are taken over the
  pixel so they do not sparkle. Everything fades to black over the last
  third of a second. What the beam lit a few frames before still glows,
  fading.

Nothing flickers, rolls or is noise: every value moves smoothly between the
original's frames, and the sets' differences are fixed. Everything is a
function of the clip's time; an ignored test checks that part 8's first
frame is part 7's last, byte for byte, that its own drawing a moment later is
within two steps of a channel of it, that frames drawn after others match a
new renderer's and that the end is black. The pictures' constants are at the
top of `rust/src/hd/ending_crt.wgsl`, the wall's in
`rust/src/hd/ending_set.wgsl`, the timing and the camera in
`rust/src/hd/ending.rs`.

At 2560 × 1440 on an Apple M5 Max, as `--bench` measures it (120 frames, on
a machine in use), modern part 8 takes about 3 to 4 ms a frame, the widest
wall included; faithful HD under 2 ms.
