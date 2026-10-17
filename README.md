# Panic playground

Playing with a modern rendering of Future Crew's 1992 demo **Panic**.

[![Watch the modern Panic intro on YouTube](https://img.youtube.com/vi/kydoV3qX6J4/maxresdefault.jpg)](https://youtu.be/kydoV3qX6J4)

[▶ Watch the intro on YouTube](https://youtu.be/kydoV3qX6J4)

So far this covers the opening through the PANIC logo, the square transition,
and experiments with the following double scroller. The idea is to keep the
sequence and music, and experiment with the rendering. There are chunky glass
letters that refract the stars behind them, a mirrored plate that reflects the
warp, motion blur, a starfield that ramps into warp, a heated metal logo with
slowly moving surface reflections, and lighting that pulses and breathes with
the music.

This builds on the [Rust port of Panic](https://github.com/bradleydwyer/panic).
It's a work in progress, with modern treatments for the intro and scroller. The original
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

To continue into the selected glass scroller with studio lighting:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --scroller --seek 27
```

| Flag | Experiment |
| --- | --- |
| `--scroller ribbons` | Coated titanium lettering with ends curving into depth |
| `--scroller sculpted` | Straight extruded lettering for comparison |
| `--scroller` or `--scroller glass` | Selected curved glass treatment with reflective edges |
| `--scroller-lighting musical` | Bass pulses the coloured lights; percussion catches white highlights |
| `--scroller-lighting studio` | Steady lighting, selected by default |
| `--scroller-layout ring` | Experimental: one tilted ring of letters instead of two rows |
| `--faithful` | Faithful HD: the original look, with smooth letters and dots |

The scroller modes include the laser transition and continue to the end of
part 2, around 42 seconds. Omit `--seek` for the complete intro; use `--seek 18`
to watch the tiles reveal the new scene. Add `--split` for the original on the
left. In the app, **1 / 2 / 3** switch presets, **L** switches lighting and
**G** switches between rows and the ring without restarting the music.

The letters are drawn from the original font, text and sine tables, read from
your local data when the preview starts, at the proportions of the original
4:3 display. Both rows and the sine text move continuously at the original's
speeds and stay in step with its timeline. On a wide screen the rows carry on
past the edges of the 4:3 picture and curve away into the chamber. The vertical
sine scroller passes in front of the lower row and behind the upper row, where
it remains visible through the glass.

Depth, extrusion, the curve, materials and lighting are authored experiments.
Glass transmission, reflections, haze and contact shadows are analytic
approximations, not a full optical simulation.

The ring is an experiment. Its front matches the upper row exactly; the same
letters then travel round behind it and back across the far side, seen from
behind. The original's lower row shows something a ring can't: the second copy
of the text, read normally, revealed from its end. So the ring drops that copy,
and the two rows remain the default.

| Key | Action |
| --- | --- |
| Space | Pause or resume |
| Left / Right | Seek one second |
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

Importing the original files yourself doesn't grant any extra rights to use or
share them.
