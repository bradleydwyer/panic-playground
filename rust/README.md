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

The `panic-hd` preview covers the opening through the PANIC logo. Build and run
it from the repository root after importing your data:

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
Use `--scroller` to continue into the selected glass scroller with studio lighting.
Original/comparison mode shows the actual reference wipe, and faithful HD
uses the original pixels during this transition.

### Scroller experiments

Glass with studio lighting is the selected treatment; bare `--scroller` selects both.

```sh
cargo run --release --locked --features hires --bin panic-hd -- --scroller --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller ribbons --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller sculpted --seek 27
cargo run --release --locked --features hires --bin panic-hd -- --scroller glass --seek 27
```

`ribbons` bends the ends of coated titanium lettering into depth. `sculpted`
keeps the same material on straight extrusions. `glass` uses the ribbon shape
with approximate tinted transmission and reflective edges. The vertical sine
overlay passes in front of the lower row and behind the upper row. It follows
the ribbon curvature, with its depth crossover in the gap between rows, above
a rippling dark chamber. The three modes share the reference frames, music
and scene boundaries. The existing laser
transition reveals the chamber; the preview ends after part 2's fade.

`--scroller-lighting musical` uses relative loudness and bass/treble
onsets from the original PCM. Bass pulses the blue/orange lights, their metal
reflections and the small lettering; treble adds short white highlights.
These are authored audio cues, not recovered tracker events. The levels adapt
to loud passages so the response retains contrast. `--scroller-lighting studio` (default)
holds the lights steady for material comparisons. Press **1 / 2 / 3** to change the preset and
**L** to toggle lighting while playing or paused. `--split` compares the
modern view with the locally rendered original. Omit `--seek` for the full
intro, or use `--seek 18` to start at the transition.

The GPU extrudes signed distance fields derived from the original indexed
frames: low seven bits distinguish the large font from the background; bit 7
contains the sine overlay. Its sparse VGA samples are joined into strokes.
When a sine row crosses the reconstructed raster split, the modern contour
reads that row directly from the video planes before the lower-half offset.
This removes a periodic horizontal tear while preserving actual glyph gaps;
the faithful scanout and original comparison remain unchanged.
Contour smoothing deliberately rounds raster steps for the authored bevels;
it does not recover vector artwork. Adjacent fields are interpolated at display
time. The modern background filters the reconstruction's rapid palette cycling
into a slow colour drift, then interpolates at display time. The original view
keeps the unfiltered palette. This ambient filtering is independent of the
musical accents and is precomputed so seeking and exports remain deterministic.
Materials, ribbon bending, depth and light placement are authored. Contact
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
| F | Toggle fullscreen |
| S | Save a PNG to `--out`; existing files are preserved |
| P | Show or hide the frame-rate counter |
| V | Toggle vsync |
| Escape | Quit |

`--fps` starts with the frame-rate counter in the top-left corner, showing
frames per second and milliseconds per frame. With vsync the rate can't exceed
the display's refresh rate; `--no-vsync`, or V, shows how fast frames can
actually be drawn. Captures never include the counter.

Without `--modern`, the faithful HD mode retains the original painted artwork,
using interpolated contours and shading. Its detail is limited by the source
raster; it is not a replacement 3D model. `--original` displays the reference
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

The default HD tests use synthetic data. Separate ignored tests check the
intro's reference pixels and PCM against the existing engine, sampled fly-by
projection heights, and the transition's complete reference frames, audio and
tile schedule:

```sh
cargo test --release --locked --features hires
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked \
  --features hires reference_timing_audio_and_flyby_projections -- --ignored
PANIC_DATA=/absolute/path/to/imported-data cargo test --release --locked --features hires transition_matches_engine_frames_audio_and_schedule -- --ignored
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

## Timing and platform notes

The player runs on a fixed 70 Hz scene timer with 16 kHz stereo audio mixed in
512-byte blocks. Scene arithmetic and sequencing follow the original, but real
VGA and PIT timing isn't emulated, so frame timing won't match every DOS
machine exactly. A full headless run takes 17,442 timer steps (249.171429
seconds).

It's been tested on Apple Silicon Macs. It should build elsewhere, but display
and audio on other platforms haven't been checked yet.
