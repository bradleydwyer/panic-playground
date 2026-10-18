# Panic playground

Playing with a modern rendering of Future Crew's 1992 demo **Panic**.

[![Watch the modern Panic intro, scroller and rings on YouTube](https://img.youtube.com/vi/p4w5MObv9Ho/maxresdefault.jpg)](https://youtu.be/p4w5MObv9Ho)

[▶ Watch the modern intro, scroller and rings on YouTube](https://youtu.be/p4w5MObv9Ho)

So far this covers the opening through the PANIC logo, the square transition,
experiments with the following double scroller, and part 6's interference
rings as a pool of liquid chrome under neon. The idea is to keep the
sequence and music, and experiment with the rendering. There are chunky glass
letters that refract the stars behind them, a mirrored plate that reflects the
warp, motion blur, a starfield that ramps into warp, a heated metal logo with
slowly moving surface reflections, and lighting that pulses and breathes with
the music.

This builds on the [Rust port of Panic](https://github.com/bradleydwyer/panic).
It's a work in progress, with modern treatments for the intro, scroller and part 6. The original
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
fades in from 80.36; part 5 would begin at 81.27, where the preview ends.
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
machines of 1992; faces; things of the 1990s and what replaced them; social
media then and now; and AI. The music still picks which symbol lights and
when, so each set follows the original's order and pairs, and the original
symbols play before and after. The new symbols were drawn for this remake;
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
