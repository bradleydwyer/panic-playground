# Panic playground

Playing with a modern rendering of Future Crew's 1992 demo **Panic**.

[![Watch the modern Panic intro, scroller and rings on YouTube](https://img.youtube.com/vi/p4w5MObv9Ho/maxresdefault.jpg)](https://youtu.be/p4w5MObv9Ho)

[▶ Watch the modern intro, scroller and rings on YouTube](https://youtu.be/p4w5MObv9Ho)

So far this covers the opening through the PANIC logo, the square transition,
experiments with the following double scroller, part 6's interference
rings as a pool of liquid chrome under neon, part 5's deep Mandelbrot
zoom and molten Mandelbrot mountain, part 3's shade bobs and morphing
dots as light painting, and part 4's plasma as the light on a carved stone
slab. The idea is to keep the
sequence and music, and experiment with the rendering. There are chunky glass
letters that refract the stars behind them, a mirrored plate that reflects the
warp, motion blur, a starfield that ramps into warp, a heated metal logo with
slowly moving surface reflections, and lighting that pulses and breathes with
the music.

This builds on the [Rust port of Panic](https://github.com/bradleydwyer/panic).
It's a work in progress, with modern treatments for the intro, scroller, part 6, part 5, part 3 and part 4. The original
player is included as a reference. The new letterforms and block motion are
authored approximations, not recovered original 3D models.

No original demo data, music, screenshots or videos are stored in this repository.
The preview above is hosted on YouTube. You import
the data locally from your own copy of `PANIC.EXE`.

## Run it

You need Rust 1.88 or newer, CMake, a C/C++ compiler and a GPU supported by wgpu.
The preview has been tested on Apple Silicon with Metal; other platforms
haven't been checked yet.

First follow the [data setup instructions](rust/README.md#setup). The importer
needs Python, Deark and the packages in `requirements-import.txt`. It checks the
release and every generated file against known hashes, and keeps its output
under the ignored `artifacts/` directory.

Then, from the repository root:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --modern
```

If you've already imported the data for the original port, pass its location
with `--data /path/to/local-data`, or set `PANIC_DATA`.

For a split comparison, with the original on the left:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --modern --split
```

To continue through the next transition, where the logo screen is cut away
in squares:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --transition --seek 17.8
```

Bright cutting points trace each square, leave a small attachment at the top,
then sever it. The intact pieces drop directly away from the camera, as though
we're looking down through a floor. Smoke, sparks and glowing cuts shift from
cool blue on the left to orange on the right. The squares follow the original
activation order and wipe timing, with uninterrupted music. `--transition`
stops on the next backdrop; the scroller experiments below continue from it.

Omit `--seek 17.8` to watch the whole intro. Add `--split` to compare with the
original. `--transition` selects modern mode automatically; `--laser` still
works as an alias. Earlier trials of a curling burn and of falling, burning
sheets were removed, but remain in the Git history (commits `4c43ab2`,
`00a63d5` and `0fe8b55`).

To continue into the glass scroller, with studio lighting over a chrome bump wall:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --scroller --seek 27
```

| Flag | Experiment |
| --- | --- |
| `--scroller` | Glass lettering with reflective edges on a shallow arc, studio lighting and the chrome wall |
| `--scroller-lighting musical` | Bass pulses the coloured lights; percussion catches white highlights |
| `--scroller-lighting studio` | Steady lighting; selected by default |
| `--scroller-backdrop chrome` | A dim wall of chrome bumps on the original tile grid, moving with it; selected by default |
| `--scroller-backdrop wall` | The same bumps tinted blue steel by the tile image |
| `--scroller-backdrop chamber` | Dark chamber with the original tile wall faintly visible |
| `--wall-shape 0.5` | Bump shape, from 0 (a cushion like the original tile) to 1 (a hemisphere); 0.5 by default |
| `--wall-strength 0.6` | Presence of the bump wall, from 0 (dark) to 1; 0.6 by default |
| `--wall-waves 0.5` | Waves across the bump wall, lit from the side so the crests cast shadows; on by default, their height swinging from the depth to its opposite and back. The value is the depth, from 0.1 to 1, 0.5 by default; 0 turns them off |
| `--wave-light 0.5` | Strength of the waves' side light and shadows, from 0 to 1; 0.5 by default |
| `--faithful` | Faithful HD: the original look, with smooth letters and dots |

The scroller includes the laser transition and continues to the end of
part 2, around 42 seconds. Omit `--seek` for the complete intro; use `--seek 18`
to watch the tiles reveal the new scene. Add `--split` for the original on the
left. In the app, **L** switches lighting, **B** cycles the backdrop,
**[ / ]** change the bump shape and **- / =** the wall's strength. **W** turns
the wall's waves on or off, **, / .** change their depth and **; / '** their
light, without restarting the music.

Glass was chosen from three presets. The other two,
coated titanium ribbons and straight sculpted extrusions, were removed from the
code but remain in the Git history: commit `5e717ff` introduced them, and
`ad08523` is the last commit that still has them.

The letters are drawn from the original font, text and sine tables, read from
your local data when the preview starts, at the proportions of the original
4:3 display. Both rows and the sine text move continuously at the original's
speeds and stay in step with its timeline. Both rows run round one shallow
arc, a slice of a large cylinder, so they read flat in the centre and turn
away as they reach and pass the sides of the screen. The vertical sine
scroller runs off the top and bottom of the screen, passing in front of the
lower row and behind the upper row, where it remains visible through the glass.

Depth, extrusion, the arc, materials and lighting are authored experiments.
The glass glows just inside each letter's bevel and, over the bump wall, lets
through a blurred view of the wall's light behind it. Glass transmission,
reflections, haze and contact shadows are analytic approximations, not a full
optical simulation. Letters and the sine text fade in as part 2 begins, and
the scene fades out with its end.

A ring layout, with one whole ring of letters per row, was tried and
archived in favour of the rows on their arc. It was removed from the code
but remains in the Git history: commit `eedbbc8` introduced a single ring,
`424a4df` replaced it with two, and `21e1f7a` is the last commit that still
has them.

The tile wall behind the letters moves as in the original, which slides it on
a slow looping path by changing the palette. Its position comes from the
original's oscillator, interpolated between frames, so it glides smoothly at
any display rate. The wall already sways when the laser cut reveals it,
before the original's oscillator starts: an authored lead-in that runs the
oscillator backwards and joins the original's motion seamlessly when the
text begins. The chrome bumps, their lighting and their reflections of
the letters are authored. They are drawn as a dim, slightly out-of-focus
background, blurred along the wall's motion, so the fast sway doesn't strobe.

The wall's waves are a further experiment, on by default; **W** or
`--wall-waves 0` turns them off. The original's wall is flat, so its waves
are authored, not recovered. A slow swell rolls across the bumps, lit by a
grazing light from the left, so the crests cast shadows across the troughs
and the bumps on the lit slopes catch it. Every 7.36 seconds its height
swings from the set depth through flat to the opposite, the crests sinking
into troughs, and back. The light is the
wall's own: the letters are lit as before. The waves ease in as the laser cut
reveals the wall and settle before part 2 fades. They depend only on the
time, so seeking and captures show the same waves.

### Part 6: Rings

`--rings` carries on past the scroller through part 6, the interference rings
and flashing symbols, to the picture that opens part 5, with the music
unbroken:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --rings --seek 42
```

| Option | Effect |
| --- | --- |
| `--rings` | Continue through part 6, as a liquid-chrome pool under neon |
| `--original-symbols` | Part 6 with the original symbols throughout |

Part 6 starts at 42.23 seconds, black until its first symbol at 43.41. The
rings fade in from 50.33 seconds and out from 72.99, and the PSTART picture
fades in from 80.36; part 5 begins at 81.27, where `--rings` ends and
`--field` carries on.
The symbols become neon: glowing gas in clear glass tubes, bent into
separate runs with gaps where lines cross, as signs are made. The two ring
layers become two sets of waves interfering on a pool of liquid chrome that
fills the screen, under a cool and a warm lamp. Until the rings fade in
there is only the neon on black. The pool comes up with them, its waves
mirroring the neon far out across it, and when they fade out the lamps go,
leaving the waves lit by the neon alone. In the app, **M** shows faithful
HD, the original's look at display resolution. The
[renderer notes](rust/README.md#part-6-rings) say what is read from the port,
what is interpreted and what is authored, and how to add a style.

In the middle of the part the neon shows new symbols. Each of the seven
patterns of the tune from 50.79 to 76.64 seconds has its own set of 16
drawings in place of the original's: water; the elements and alchemy;
machines of 1992; classic emoji; things of the 1990s and what replaced them;
social media then and now; and AI. Every drawing is a different subject, and
each looks unlike the one the music shows next, so the flashes keep time as
the original's do. The music still picks which symbol lights and when, so
each set follows the original's order and pairs, and the original symbols
play before and after. They are bent into neon signs as the
originals are: each tube is a single run, loops open where the tube would
turn back into the sign, and where strokes meet or cross, one stops short of
the other with an air gap. The new symbols were drawn for this remake;
they are not recovered from the demo, and unlike its data they are kept in
the repository, in `rust/src/hd/rings_symbols.txt`. Some are neon tracings
of company logos: Claude, ChatGPT, Apple, Bitcoin, YouTube, Instagram, X,
WhatsApp, Facebook, Reddit, LinkedIn and TikTok. Those logos are trademarks
of their owners; their use here implies no affiliation or endorsement.
Faithful HD keeps the original symbols, and `--original-symbols`, or **N**
in the app, shows them throughout.

Four other styles were tried alongside the pool and removed from the code:
faithful-plus, the original's two greys with the neon symbols; edge-lit
glass plates; the rings projected onto the scroller's bump wall; and
spherical waves in haze. They remain in the Git history: commit `a786132`
introduced faithful-plus and the style framework, `43a1b78` glass, `64877d8`
the bump wall and `3e1c181` the haze, and `a9f0e30` is the last commit that
still has them.

### Part 5: Field

`--field` carries on through part 5, to the frame part 3 starts from, with
the music unbroken:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --field --seek 81
```

Part 5 starts at 81.27 seconds on the PSTART picture part 6 faded in. In its
window an endless zoom dives into a blue field, a level twice as fine every
0.90 seconds at full speed, until it brakes and stops at 96.96 after about
81,000 times. Then a mountain of fire colours rises in front of the frame,
the window goes dark at 98.11, and the camera circles the mountain until it
sinks from 121.83; the frame holds with its dark window until 130.03. All of
that is read from the port and checked against it frame by frame. As the
research found, the original's height map looks like a Mandelbrot mountain,
the set as its plateau and its slopes following the set's potential; the
preview fits that when it starts, which is an interpretation, and keeps
nothing fitted.

The modern look keeps that timing and changes the rest:

- The picture's frame and ornaments become dark gunmetal, shaped from the
  picture itself, with real 3D fluid smoke billowing round them, lit blue by
  the window and then orange by the lava.
- The window's zoom becomes a dive into the Mandelbrot set from a view
  3.3e-50 high, past rings of lace and spiral stars, onto a minibrot (a
  small copy of the set) that lands upright, filling the window, as the zoom
  stops. It is computed by perturbation from a high-precision reference
  orbit, in f32 on the GPU, a level per doubling as the original builds its
  own buffers.
- The mountain becomes the Mandelbrot set itself as molten rock: a
  convecting lava lake on the plateau spilling over its rim, and lava that
  runs down the set's field lines, crusting as it cools. The mountain
  computes the set where it is near, so its edge stays sharp however close
  the camera passes.

The target, the lava, the metal, the smoke and all the lighting are
authored. Faithful HD (**M**) shows the original's look at display
resolution: the zoom from the port's own buffers, smoothly, and the
mountain marched pixel by pixel through the original height map with the
port's camera. The [renderer notes](rust/README.md#part-5-field) say what is
read, what is interpreted and what is authored, and how the three parts plug
in.

### Part 3: Dots

`--dots` carries on through part 3 and part 4's wait to the first frame
part 4 draws, with the music unbroken:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --dots --seek 130
```

Part 3 starts at 130.03 seconds holding part 5's last frame, then zooms into
its dark window from 131.99. From 134.37 the shade bobs, 20 discs an
iteration whose overlaps light up a blue ribbon, writhe for 4.7 seconds.
From 139.11, 512 dots morph through eight objects, a sphere, a vase, a cube,
a grid, two lattices, a pyramid and a box, each with eight echoes trailing
it, until the box lands at 177.10. Part 4 waits until 179.93 and draws its
logo slab in white where the box landed, at 179.96, where `--dots` ends. All
of that is read from the port and checked against it frame by frame.

The modern look is light painting, a long exposure in a dark, hazy space:

- The camera flies through part 5's window, carrying on until the frame,
  still drawn by part 5's own modern view, has left the screen.
- The shade bobs become a fibre-optic brush: 20 glowing tips sweeping the
  path, exposed as long as the original keeps its discs, so the overlaps
  burn from blue to white as the original's palette does, each tip drawing
  a fine fibre of its own.
- The dots become 512 lights exposed continuously along their paths for as
  long as the original's echoes last, then fading in a faint afterglow,
  through a lens focused on the object, so near and far lights open into
  discs shaped by a seven-bladed diaphragm. A haze and drifting fog glow
  round them, they breathe with the bass, and each order of the tune sends
  a plane of light sweeping through the object.
- The landed box's trails collapse into its lights, which flare up in the
  second before part 4's white slab takes over.

The lights, the brush, the lens, the haze and the music's effects are
authored; the positions, motion and timing are the port's. Faithful HD
(**M**) shows the original's discs and dots at display resolution. The
[renderer notes](rust/README.md#part-3-dots) say what is read, what is
interpreted and what is authored.

### Part 4: Plasma

`--plasma` carries on through part 4 to part 7's first frame, with the
music unbroken:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --plasma --seek 177
```

Part 4 draws its logo slab, a stone block carved FUTURE CREW, at 179.96
seconds, white, and fades it to its colours over 64 frames, to 180.86.
Above it is a plasma: a fixed field of colour indices, a palette that
turns back and forth through them, and 400 rasters that each start where
a table of strides puts them, so the even and odd rasters show two copies
of the field pushed apart by waves running up and down the screen. It
stays black until the music passes 2A00, fades in by 191.06, fades out
from 2E15 (219.33 to 222.87), and from 2F00, at 224.23, the slab slides
down off the screen. Part 7 starts at 225.43 on a black frame, where
`--plasma` ends. All of that is read from the port and checked against it
frame by frame.

The modern look puts the two halves, which the original keeps apart with
its line compare, in one dark space, and makes the plasma the light the
stone is seen by:

- The slab becomes a block of stone in the original's place. The logo's
  silhouette fits a right-angled block seen in perspective; with part 3's
  box's width its depth comes out within 3% of the box's, so the box part 3
  lands is this block, made solid. Its faces are carved as the logo shades
  them, FUTURE in the top, CREW in the front and the cracks, the faces
  polished and the cut rough, as a mason finishes a slab.
- The plasma becomes a sheet of light standing behind the slab, showing
  the original's field and palette at every point, its two interlaced
  copies mixed as the eye mixes them, and its fades as the light's.
- The sheet lights the stone: its moving colours rake across the carving
  from behind and above and slide across the polished top as reflections,
  shadowed by the cut. A dim key light from where the logo's painted light
  comes from keeps the stone lit while the plasma is dark, and gives way
  as it rises.
- Part 3's flare overexposes the camera, and the exposure settles over the
  original's 64 frames of fade from white, so the carving comes out of the
  white first. The slab sinks with the original's line compare.

The stone, its carving and finish, the sheet, the lighting and the arrival
are authored; the positions, motion and timing are the port's. Faithful HD
(**M**) shows the original's rasters at display resolution, the field
smooth along each raster. The [renderer notes](rust/README.md#part-4-plasma)
say what is read, what is interpreted and what is authored.

### Keys

| Key | Action |
| --- | --- |
| Space | Pause or resume |
| Left / Right | Step one second; hold to scrub through time, faster the longer held |
| Home / R | Replay |
| End | Pause on the finished logo |
| M | Switch modern 3D / faithful HD |
| Tab | Toggle the original rendering |
| C | Toggle the split comparison |
| F | Toggle fullscreen |
| S | Save a PNG under `artifacts/` |
| P | Show or hide the frame-rate counter |
| V | Toggle vsync |
| Escape | Quit |

Add `--fps` to show a frame-rate counter, and `--no-vsync` to see the uncapped
rate.

The logo appears around 14.57 seconds. The preview's audio ends around 19.49
seconds, while the logo keeps animating. End, then Space, jumps straight to it.

[The renderer notes](rust/README.md#experimental-hd-intro) cover the timing,
rendering modes, 4K image capture and current limitations. The same document
also explains how to run the original full demo player.

## Development

The default tests use synthetic data:

```sh
python3 -m unittest discover -s tests -v
cargo test --release --locked --no-default-features
cargo test --release --locked --features hires
```

Tests using original demo data are opt-in; see [the test notes](rust/README.md#tests).

To catch unintended changes to the look while working on the shaders, render
a baseline set of frames first, then check each new build against it:

```sh
python3 tools/look_check.py render baseline
# edit, then rebuild panic-hd
python3 tools/look_check.py check baseline
```

`check` renders the same shots with `target/release/panic-hd` and compares
them pixel by pixel. If more than a few stray pixels change (by default more
than 0.1% of a frame, however faintly), or a shot no longer renders, it says
which shots and exits with an error. Heatmaps and a before, after and
difference sheet go under `artifacts/look-check/`. The
[look check notes](rust/README.md#look-check) cover the options.

Before committing, stage the changes and run:

```sh
python3 tools/check_publication.py --research
```

That checks the staged files and Git history against the source-only publication
scope. Keep imported data, captures and recordings under `artifacts/`.

## Credits and rights

Panic is by Future Crew. This is an unofficial experiment and isn't affiliated
with them.

The [MIT License](LICENSE) covers the standalone resource tools, their synthetic
tests and documentation. It does **not** cover `rust/` or the full import and
unpacking helpers; their rights status remains unresolved. The existing scope
is retained from the Rust port. See [NOTICE](NOTICE), [rust/NOTICE](rust/NOTICE)
and [the publication notes](docs/publication.md).

The bump wall, which the scroller and part 6 share, uses a ray / rounded box
intersection adapted from Inigo Quilez's
[Rounded Box - intersection](https://www.shadertoy.com/view/WlSXRW),
which he published under the MIT License. His notice is in
[rust/NOTICE](rust/NOTICE) and beside the code; it covers his code, not the
rest of `rust/`.

Importing the original files yourself doesn't grant any extra rights to use or
share them.
