# Panic playground

Playing with a modern rendering of Future Crew's 1992 demo **Panic**.

[![Watch the modern Panic intro on YouTube](https://img.youtube.com/vi/kydoV3qX6J4/maxresdefault.jpg)](https://youtu.be/kydoV3qX6J4)

[▶ Watch the intro on YouTube](https://youtu.be/kydoV3qX6J4)

So far this covers the opening through the PANIC logo, the square transition,
and experiments with the following double scroller. The idea is to keep the
sequence and music, and experiment with the rendering. There are chunky 3D
letters, metal reflections, a starfield that ramps into warp, and a heated
metal logo with slowly moving surface reflections.

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

To try the next transition, where squares eat away the logo screen:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --modern --transition
```

The squares curl as their edges burn inward, with moving reflections, embers
and wisps of smoke. They use the original activation order and wipe timing.

To try the falling version:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --tear
```

Here the squares peel loose like thin sheets, fall under gravity and rapidly
burn towards their centres. Each piece lasts about half a second, with the
original activation sequence and uninterrupted music. Both versions stop on
the next backdrop unless a scroller experiment is selected.

There's also a laser-cut trial. Bright cutting points trace each square, leave
a small attachment at the top, then sever it. The intact pieces drop directly
away from the camera, as though we're looking down through a floor. Smoke,
sparks and glowing cuts shift from cool blue on the left to orange on the right:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --laser --seek 17.8
```

Omit `--seek 17.8` to watch the whole intro. Add `--split` to compare with the
original. `--laser` and `--tear` both select modern mode automatically.

To continue into the scroller experiments:

```sh
cargo run --release --locked --features hires --bin panic-hd -- --scroller ribbons --seek 27
```

| Flag | Experiment |
| --- | --- |
| `--scroller ribbons` | Coated titanium lettering with ends curving into depth |
| `--scroller sculpted` | Straight extruded lettering for comparison |
| `--scroller glass` | Curved tinted-glass treatment with reflective edges |
| `--scroller-lighting musical` | Bass pulses the coloured lights; percussion catches white highlights (default) |
| `--scroller-lighting studio` | Steady lights for inspecting shape and material |

The scroller modes include the laser transition and continue to the end of
part 2, around 42 seconds. Omit `--seek` for the complete intro; use `--seek 18`
to watch the tiles reveal the new scene. Add `--split` for the original on the
left. In the app, **1 / 2 / 3** switch presets and **L** switches lighting without
restarting the music. Existing `--transition` and `--tear` can also be combined
with `--scroller` to compare the earlier transitions.

The large lettering and sine overlay use smoothed contours from locally
rendered reference frames. Depth, ribbon bending, materials and lighting are
authored experiments. Glass transmission, reflections, haze and contact shadows
are analytic approximations, not a full optical simulation.

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
| Escape | Quit |

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
