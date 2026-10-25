use panic_demo::{
    data::Data,
    engine::SAMPLE_RATE,
    hd::{
        clip::{Clip, ScrollerBackdrop, ScrollerLighting},
        renderer::{Renderer, View},
    },
};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::{Event, WindowEvent},
    keyboard::{Keycode, Mod, Scancode},
    video::FullscreenType,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

struct Options {
    data: PathBuf,
    size: (u32, u32),
    seek: f64,
    headless: bool,
    out: PathBuf,
    mute: bool,
    paused: bool,
    fullscreen: bool,
    comparison: u32,
    modern: bool,
    faithful: bool,
    transition: bool,
    scroller: bool,
    /// Continue through part 6 (Rings) to part 5's first frame.
    rings: bool,
    /// Continue through part 5 (Field) to part 3's first frame.
    field: bool,
    /// Continue through part 3 (Dots) and part 4's wait to its first frame.
    dots: bool,
    /// Continue through part 4 (Plasma) to part 7's first frame.
    plasma: bool,
    /// Continue through part 7 (Greetings) to part 8's first frame.
    greetings: bool,
    /// Continue through part 8 (the ending) to the demo's last frame.
    ending: bool,
    /// Part 6's original symbols throughout, instead of the authored sets
    /// in its middle (N).
    original_symbols: bool,
    scroller_lighting: ScrollerLighting,
    scroller_backdrop: ScrollerBackdrop,
    wall_shape: f32,
    wall_strength: f32,
    /// The authored wall waves (W), their depth and their side light.
    waves: bool,
    wave_depth: f32,
    wave_light: f32,
    fps: bool,
    vsync: bool,
    seconds: Option<f64>,
    /// Headless: draw this many frames from the seek time, at 60 a second,
    /// and print the time each took.
    bench: Option<usize>,
}
impl Options {
    fn parse() -> Result<Self, String> {
        let mut o = Self {
            data: std::env::var_os("PANIC_DATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| "artifacts/local-data".into()),
            size: (1920, 1080),
            seek: 0.0,
            headless: false,
            out: "artifacts/intro-hd.png".into(),
            mute: false,
            paused: false,
            fullscreen: false,
            comparison: 0,
            modern: false,
            faithful: false,
            transition: false,
            scroller: false,
            rings: false,
            field: false,
            dots: false,
            plasma: false,
            greetings: false,
            ending: false,
            original_symbols: false,
            scroller_lighting: ScrollerLighting::Studio,
            scroller_backdrop: ScrollerBackdrop::Chrome,
            wall_shape: 0.5,
            wall_strength: 0.6,
            waves: true,
            wave_depth: 0.5,
            wave_light: 0.5,
            fps: false,
            vsync: true,
            seconds: None,
            bench: None,
        };
        let mut args = std::env::args().skip(1).peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--data" => o.data = args.next().ok_or("--data needs a directory")?.into(),
                "--size" => {
                    let s = args.next().ok_or("--size needs WIDTHxHEIGHT")?;
                    let (w, h) = s.split_once('x').ok_or("Use --size WIDTHxHEIGHT")?;
                    o.size = (
                        w.parse().map_err(|_| "Invalid width")?,
                        h.parse().map_err(|_| "Invalid height")?,
                    );
                }
                "--seek" => {
                    o.seek = args
                        .next()
                        .ok_or("--seek needs seconds")?
                        .parse()
                        .map_err(|_| "Invalid seek")?
                }
                "--seconds" => {
                    o.seconds = Some(
                        args.next()
                            .ok_or("--seconds needs a duration")?
                            .parse()
                            .map_err(|_| "Invalid duration")?,
                    )
                }
                "--out" => o.out = args.next().ok_or("--out needs a PNG path")?.into(),
                "--headless" => o.headless = true,
                "--mute" => o.mute = true,
                "--paused" => o.paused = true,
                "--fullscreen" => o.fullscreen = true,
                "--original" => o.comparison = 1,
                "--split" => o.comparison = 2,
                "--modern" => o.modern = true,
                "--faithful" => o.faithful = true,
                "--fps" => o.fps = true,
                "--no-vsync" => o.vsync = false,
                // --laser is kept as an alias from the transition trials.
                "--transition" | "--laser" => {
                    o.transition = true;
                    o.modern = true;
                }
                "--scroller" => {
                    // Glass is the only scroller look; `--scroller glass` is
                    // still accepted. The other presets remain in Git history.
                    if let Some(style) = args.next_if(|arg| !arg.starts_with('-'))
                        && style != "glass"
                    {
                        return Err(format!(
                            "The {style} scroller preset was removed; use --scroller"
                        ));
                    }
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 6 follows part 2, so it continues the whole show.
                "--rings" => {
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 5 follows part 6.
                "--field" => {
                    o.field = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 3 follows part 5.
                "--dots" => {
                    o.dots = true;
                    o.field = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 4 follows part 3.
                "--plasma" => {
                    o.plasma = true;
                    o.dots = true;
                    o.field = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 8 follows part 7, the last.
                "--ending" => {
                    o.ending = true;
                    o.greetings = true;
                    o.plasma = true;
                    o.dots = true;
                    o.field = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                // Part 7 follows part 4.
                "--greetings" => {
                    o.greetings = true;
                    o.plasma = true;
                    o.dots = true;
                    o.field = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                "--bench" => {
                    o.bench = Some(
                        args.next()
                            .ok_or("--bench needs a number of frames")?
                            .parse()
                            .ok()
                            .filter(|&n: &usize| n > 0)
                            .ok_or("Use --bench with a number of frames")?,
                    );
                    o.headless = true;
                }
                "--original-symbols" => {
                    o.original_symbols = true;
                    o.rings = true;
                    o.scroller = true;
                    o.modern = true;
                }
                "--scroller-lighting" => {
                    o.scroller_lighting = args
                        .next()
                        .ok_or("--scroller-lighting needs musical or studio")?
                        .parse()?;
                    o.scroller = true;
                    o.modern = true;
                }
                "--scroller-layout" => {
                    // The two rows are the only layout; `rows` is still
                    // accepted. The ring prototype remains in Git history.
                    let layout = args.next().ok_or("--scroller-layout needs rows")?;
                    if layout != "rows" {
                        return Err(format!(
                            "The {layout} scroller layout was removed; use --scroller"
                        ));
                    }
                    o.scroller = true;
                    o.modern = true;
                }
                "--scroller-backdrop" => {
                    o.scroller_backdrop = args
                        .next()
                        .ok_or("--scroller-backdrop needs chamber, wall or chrome")?
                        .parse()?;
                    o.scroller = true;
                    o.modern = true;
                }
                "--wall-shape" => {
                    o.wall_shape = args
                        .next()
                        .ok_or("--wall-shape needs a value from 0 to 1")?
                        .parse()
                        .ok()
                        .filter(|v: &f32| (0.0..=1.0).contains(v))
                        .ok_or("Use --wall-shape with a value from 0 to 1")?;
                }
                "--wall-strength" => {
                    o.wall_strength = args
                        .next()
                        .ok_or("--wall-strength needs a value from 0 to 1")?
                        .parse()
                        .ok()
                        .filter(|v: &f32| (0.0..=1.0).contains(v))
                        .ok_or("Use --wall-strength with a value from 0 to 1")?;
                }
                "--wall-waves" => {
                    // An optional depth follows; 0 turns the waves off.
                    o.waves = true;
                    if let Some(depth) = args.next_if(|arg| !arg.starts_with('-')) {
                        let depth: f32 = depth
                            .parse()
                            .ok()
                            .filter(|v: &f32| *v == 0.0 || (0.1..=1.0).contains(v))
                            .ok_or("Use --wall-waves with a depth from 0.1 to 1, or 0 for none")?;
                        if depth == 0.0 {
                            o.waves = false;
                        } else {
                            o.wave_depth = depth;
                        }
                    }
                    o.scroller = true;
                    o.modern = true;
                }
                "--wave-light" => {
                    o.wave_light = args
                        .next()
                        .ok_or("--wave-light needs a value from 0 to 1")?
                        .parse()
                        .ok()
                        .filter(|v: &f32| (0.0..=1.0).contains(v))
                        .ok_or("Use --wave-light with a value from 0 to 1")?;
                }
                "--logo" => {
                    o.seek = f64::MAX;
                    o.paused = true;
                }
                "-h" | "--help" => {
                    println!(
                        "Panic HD — intro and scroller experiments\n\nUsage: panic-hd [options]\n  --data DIR       Locally imported data (or PANIC_DATA)\n  --size WxH       PNG size / window aspect; default 1920x1080\n  --seek SECONDS   Start at a point in the intro\n  --logo           Start paused on the completed logo\n  --paused         Start paused\n  --modern         Glass letter fly-bys, HDR lighting and heated metal\n  --faithful       Start in faithful HD, even with --scroller or --transition\n  --transition     Continue through the laser-cut tile transition (modern)\n  --laser          Same as --transition\n  --scroller       Continue into the glass scroller: studio lighting, chrome bump wall\n  --scroller-lighting MODE  Studio (default) or musical lighting\n  --scroller-backdrop MODE  Chrome (default) or tinted (wall) bump wall, or chamber\n  --wall-shape N   Bump shape, 0 rounded square to 1 hemisphere; default 0.5\n  --wall-strength N  Bump wall presence, 0 to 1; default 0.6\n  --wall-waves [D] Authored waves across the bump wall (on by default), swinging\n                   from depth D to -D and back; 0.1 to 1, default 0.5; 0 for none\n  --wave-light N   Their grazing side light and shadows, 0 to 1; default 0.5\n  --rings          Continue through part 6 (Rings), as a liquid-chrome pool, to part 5's first frame\n  --field          Continue through part 5 (Field), the zoom and the mountain, to part 3's first frame\n  --dots           Continue through part 3 (Dots), as light painting, and part 4's wait to its first frame\n  --plasma         Continue through part 4 (Plasma), as light on stone, to part 7's first frame\n  --greetings      Continue through part 7 (Greetings), as a lighthouse in the dust, to part 8's first frame\n  --ending         Continue through part 8 (the ending), an old television switched off, to the demo's end\n  --original-symbols  Part 6 with the original symbols throughout, not the new sets in its middle\n  --original       Show the original rendering\n  --split          Original on the left, HD on the right\n  --mute           Silent playback, with music timing\n  --fullscreen     Start fullscreen\n  --headless       Export one native-resolution PNG, without a window\n  --out FILE       Capture destination; default artifacts/intro-hd.png\n  --bench N        Headless: draw N frames from --seek at 60 a second and print the time each takes\n  --seconds N      Close desktop preview after N seconds\n  --fps            Show the frame rate (P toggles it)\n  --no-vsync       Don't wait for the display refresh (V toggles it)\n\nSpace: pause · Left/Right: step 1s, hold to scrub · Cmd+Left/Right: previous/next section\nHome/R: replay · End: logo · Tab: original/HD · C: split comparison · M: faithful/modern\nF: fullscreen · S: save PNG · L: scroller lighting · B: backdrop · [/]: bump shape · -/=: wall strength\nW: wall waves · ,/.: wave depth · ;/': wave light · N: part 6's new or original symbols\nP: frame rate · V: vsync · Esc: quit\nThe preview holds on the logo. --transition continues to the backdrop. --scroller plays part 2 after the laser transition. --rings continues through part 6, --field through part 5, --dots through part 3, --plasma through part 4, --greetings through part 7, --ending through part 8."
                    );
                    std::process::exit(0);
                }
                _ => return Err(format!("Unknown option {arg}; use --help")),
            }
        }
        if o.faithful {
            o.modern = false;
        }
        if o.size.0 < 64 || o.size.1 < 64 || o.size.0 > 8192 || o.size.1 > 8192 {
            return Err("Dimensions must be between 64 and 8192".into());
        }
        if !o.seek.is_finite()
            || o.seek < 0.0
            || o.seconds.is_some_and(|s| !s.is_finite() || s <= 0.0)
        {
            return Err("Invalid time".into());
        }
        Ok(o)
    }
}

/// Holding Left or Right scrubs through time once held this long (seconds);
/// a tap still steps one second.
const SCRUB_DELAY: f64 = 0.25;
/// The scrub's speed in seconds of the show per second held: its start, how
/// fast it gains, and its most.
const SCRUB_SPEED: (f64, f64, f64) = (4.0, 8.0, 20.0);
/// The modifiers that make Left and Right step by section (Cmd, or Ctrl
/// where there is no Cmd key).
const SECTION_KEYS: Mod = Mod::LGUIMOD
    .union(Mod::RGUIMOD)
    .union(Mod::LCTRLMOD)
    .union(Mod::RCTRLMOD);

fn set_audio(
    device: &AudioQueue<i16>,
    clip: &Clip,
    time: f64,
    mute: bool,
    paused: bool,
) -> Result<usize, String> {
    device.pause();
    device.clear();
    let start = ((time * SAMPLE_RATE as f64) as usize * 2).min(clip.pcm.len());
    let samples = &clip.pcm[start..];
    if mute {
        device.queue_audio(&vec![0; samples.len()])?;
    } else {
        device.queue_audio(samples)?;
    }
    if !paused {
        device.resume();
    }
    Ok(samples.len())
}

/// With vsync the rate is capped at the display's refresh rate; without it
/// the counter shows how fast frames can actually be produced.
fn present_mode(vsync: bool) -> wgpu::PresentMode {
    if vsync {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::AutoNoVsync
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("panic-hd: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut o = Options::parse()?;
    let started = Instant::now();
    let data = Data::new(&o.data)?;
    let mut clip = if o.scroller {
        let mut clip = if o.ending {
            Clip::load_ending(&data, o.scroller_lighting)?
        } else if o.greetings {
            Clip::load_greetings(&data, o.scroller_lighting)?
        } else if o.plasma {
            Clip::load_plasma(&data, o.scroller_lighting)?
        } else if o.dots {
            Clip::load_dots(&data, o.scroller_lighting)?
        } else if o.field {
            Clip::load_field(&data, o.scroller_lighting)?
        } else if o.rings {
            Clip::load_rings(&data, o.scroller_lighting)?
        } else {
            Clip::load_scroller(&data, o.scroller_lighting)?
        };
        let scroll = clip.scroller.as_mut().unwrap();
        scroll.backdrop_style = o.scroller_backdrop;
        scroll.wall_shape = o.wall_shape;
        scroll.wall_strength = o.wall_strength;
        scroll.waves = o.waves;
        scroll.wave_depth = o.wave_depth;
        scroll.wave_light = o.wave_light;
        if let Some(rings) = &mut clip.rings {
            rings.original_symbols = o.original_symbols;
        }
        clip
    } else if o.transition {
        Clip::load_transition(&data)?
    } else {
        Clip::load(&data)?
    };
    o.seek = if o.seek == f64::MAX {
        clip.logo_ready_at // --logo: finished reveal, paused.
    } else {
        o.seek.min(clip.duration())
    };
    println!(
        "Intro: {:.3}s to logo, {:.3}s total; prepared in {:.2}s",
        clip.logo_at,
        clip.duration(),
        started.elapsed().as_secs_f32()
    );
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    if let (true, Some(frames)) = (o.headless, o.bench) {
        let (mut renderer, format) = pollster::block_on(Renderer::new(&instance, None))?;
        let view = |time| View {
            time,
            size: o.size,
            comparison: o.comparison,
            modern: o.modern,
            hud: None,
        };
        let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Benchmark target"),
            size: wgpu::Extent3d {
                width: o.size.0,
                height: o.size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = target.create_view(&Default::default());
        let mut draw = |time: f64| -> Result<f64, String> {
            let begun = Instant::now();
            renderer.prepare(&clip, view(time));
            let mut encoder = renderer.device.create_command_encoder(&Default::default());
            renderer.draw(&mut encoder, &target);
            renderer.queue.submit([encoder.finish()]);
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            Ok(begun.elapsed().as_secs_f64() * 1000.0)
        };
        // The first frame builds the pipelines and catches up any replay.
        let first = draw(o.seek)?;
        let mut times: Vec<f64> = (1..=frames)
            .map(|k| draw((o.seek + k as f64 / 60.0).min(clip.duration())))
            .collect::<Result<_, _>>()?;
        times.sort_by(f64::total_cmp);
        println!(
            "{frames} frames of {} × {} from {:.3}s: {:.2} ms a frame (median {:.2}, \
             slowest {:.2}; first {first:.1}), prepare and draw, waiting for the GPU",
            o.size.0,
            o.size.1,
            o.seek,
            times.iter().sum::<f64>() / frames as f64,
            times[frames / 2],
            times[frames - 1],
        );
        return Ok(());
    }
    if o.headless {
        let (mut renderer, format) = pollster::block_on(Renderer::new(&instance, None))?;
        renderer.capture(
            &clip,
            View {
                time: o.seek,
                size: o.size,
                comparison: o.comparison,
                modern: o.modern,
                hud: None,
            },
            &o.out,
            format,
        )?;
        println!(
            "Saved {} × {} at {:.3}s to {} using {}",
            o.size.0,
            o.size.1,
            o.seek,
            o.out.display(),
            renderer.adapter_name
        );
        return Ok(());
    }
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let audio = sdl.audio()?;
    let window_scale = (1280.0 / o.size.0 as f64)
        .min(720.0 / o.size.1 as f64)
        .min(1.0);
    let mut builder = video.window(
        "Panic HD — intro",
        (o.size.0 as f64 * window_scale) as u32,
        (o.size.1 as f64 * window_scale) as u32,
    );
    builder.position_centered().resizable().allow_highdpi();
    #[cfg(target_os = "macos")]
    builder.metal_view();
    let mut window = builder.build().map_err(|e| e.to_string())?;
    if o.fullscreen {
        window.set_fullscreen(FullscreenType::Desktop)?;
    }
    // SAFETY: SDL owns the native handles. `surface` is created after `window`
    // and dropped before it; SDL/video also remain alive throughout this scope.
    let surface = unsafe {
        instance.create_surface_unsafe(
            wgpu::SurfaceTargetUnsafe::from_display_and_window(&window, &window)
                .map_err(|e| e.to_string())?,
        )
    }
    .map_err(|e| e.to_string())?;
    let (mut renderer, format) = pollster::block_on(Renderer::new(&instance, Some(&surface)))?;
    println!("GPU: {}", renderer.adapter_name);
    println!(
        "Space pause · arrows step, hold to scrub · Cmd+arrows section · Home replay · End logo · Tab original/HD · C compare · M faithful/modern · F fullscreen · S capture · L scroller lighting · B backdrop · [/] bump shape · -/= wall strength · W wall waves · ,/. wave depth · ;/' wave light · N part 6 symbols"
    );
    let (width, height) = window.drawable_size();
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode: present_mode(o.vsync),
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        color_space: Default::default(),
    };
    surface.configure(&renderer.device, &config);
    let device = audio.open_queue::<i16, _>(
        None,
        &AudioSpecDesired {
            freq: Some(SAMPLE_RATE as i32),
            channels: Some(2),
            samples: Some(512),
        },
    )?;
    if device.spec().freq != SAMPLE_RATE as i32 || device.spec().channels != 2 {
        return Err("Audio device did not accept the reference 16 kHz stereo stream".into());
    }
    // Where Cmd+Left and Cmd+Right go: each change of the window title's
    // section label.
    let sections = clip.sections();
    let mut paused = o.paused;
    let mut anchor = o.seek;
    let mut queued = set_audio(&device, &clip, anchor, o.mute, true)?;
    let mut time = anchor;
    // The audio clock moves a device buffer at a time (512 samples, 32 ms),
    // so between its steps the picture runs on with the wall clock. `heard`
    // is the audio clock and when it last moved.
    let buffer = device.spec().samples as f64 / SAMPLE_RATE as f64;
    let mut heard = (anchor, Instant::now());
    // Set at the start and on a seek: the frame's catch-up (part 5's smoke
    // runs every step it skips) finishes on the GPU before the music starts.
    let mut settle = true;
    let mut events = sdl.event_pump()?;
    let mut last_title = Instant::now() - Duration::from_secs(1);
    let runtime = Instant::now();
    let mut presented = 0_u64;
    let mut last_frame = Instant::now();
    // Frames per second and milliseconds per frame, averaged over ~0.25 s.
    let mut rate = [0.0_f32; 2];
    let mut rate_frames = 0_u32;
    let mut rate_start = Instant::now();
    // Left or Right held down, and since when, and whether it has started to
    // scrub (with the music paused until it is let go).
    let mut hold: Option<(f64, Instant)> = None;
    let mut scrubbing = false;
    'preview: loop {
        let now = Instant::now();
        let delta = now.duration_since(last_frame).as_secs_f64();
        last_frame = now;
        if o.seconds
            .is_some_and(|s| runtime.elapsed().as_secs_f64() >= s)
        {
            break;
        }
        if !paused && !scrubbing && !settle {
            let remaining = device.size() as usize / 2;
            let audio_time =
                anchor + queued.saturating_sub(remaining) as f64 / (SAMPLE_RATE * 2) as f64;
            if audio_time != heard.0 {
                heard = (audio_time, now);
            }
            // Audio remains the clock throughout the sequence: the picture
            // runs on from its latest step for at most a buffer, and never
            // back. Once the hold's audio finishes, let the logo material
            // keep moving until paused.
            time = if remaining == 0 {
                time.max(audio_time) + delta
            } else {
                time.max(audio_time + now.duration_since(heard.1).as_secs_f64().min(buffer))
            };
        }
        let mut seek = None;
        let mut capture = false;
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'preview,
                Event::KeyDown {
                    keycode: Some(key),
                    keymod,
                    repeat: false,
                    ..
                } => match key {
                    // Back to this section's start, or to the one before when
                    // within a second of it; on to the next.
                    Keycode::Left if keymod.intersects(SECTION_KEYS) => {
                        let i = sections.partition_point(|&s| s <= time - 1.0);
                        seek = Some(sections[i.saturating_sub(1)]);
                    }
                    Keycode::Right if keymod.intersects(SECTION_KEYS) => {
                        let i = sections.partition_point(|&s| s <= time);
                        seek = sections.get(i).copied().or(seek);
                    }
                    Keycode::Space => {
                        paused = !paused;
                        if paused {
                            device.pause();
                        } else if !scrubbing {
                            device.resume();
                            heard.1 = now;
                        }
                    }
                    Keycode::Left => {
                        seek = Some((time - 1.0).max(0.0));
                        hold = Some((-1.0, now));
                    }
                    Keycode::Right => {
                        seek = Some((time + 1.0).min(clip.duration()));
                        hold = Some((1.0, now));
                    }
                    Keycode::Home | Keycode::R => {
                        seek = Some(0.0);
                        paused = false;
                    }
                    Keycode::End => {
                        seek = Some(clip.logo_ready_at);
                        paused = true;
                    }
                    Keycode::Tab => o.comparison = if o.comparison == 1 { 0 } else { 1 },
                    Keycode::C => o.comparison = if o.comparison == 2 { 0 } else { 2 },
                    Keycode::M => o.modern = !o.modern,
                    Keycode::L => {
                        if let Some(scroll) = &mut clip.scroller {
                            scroll.lighting = match scroll.lighting {
                                ScrollerLighting::Musical => ScrollerLighting::Studio,
                                ScrollerLighting::Studio => ScrollerLighting::Musical,
                            };
                        }
                    }
                    Keycode::B => {
                        if let Some(scroll) = &mut clip.scroller {
                            scroll.backdrop_style = scroll.backdrop_style.next();
                        }
                    }
                    Keycode::LeftBracket | Keycode::RightBracket => {
                        if let Some(scroll) = &mut clip.scroller {
                            let step = if key == Keycode::LeftBracket {
                                -0.1
                            } else {
                                0.1
                            };
                            scroll.wall_shape = ((scroll.wall_shape + step) * 10.0).round() / 10.0;
                            scroll.wall_shape = scroll.wall_shape.clamp(0.0, 1.0);
                            println!("Bump shape {:.1}", scroll.wall_shape);
                        }
                    }
                    // Plus is the main keyboard's raise key on layouts without
                    // a separate = key; KpEquals is on Mac extended keypads.
                    Keycode::Minus
                    | Keycode::KpMinus
                    | Keycode::Equals
                    | Keycode::Plus
                    | Keycode::KpPlus
                    | Keycode::KpEquals => {
                        if let Some(scroll) = &mut clip.scroller {
                            let lower = matches!(key, Keycode::Minus | Keycode::KpMinus);
                            let step = if lower { -0.1 } else { 0.1 };
                            scroll.wall_strength = (((scroll.wall_strength + step) * 10.0).round()
                                / 10.0)
                                .clamp(0.0, 1.0);
                            println!("Wall strength {:.1}", scroll.wall_strength);
                        }
                    }
                    Keycode::W => {
                        if let Some(scroll) = &mut clip.scroller {
                            scroll.waves = !scroll.waves;
                            println!("Wall waves {}", if scroll.waves { "on" } else { "off" });
                        }
                    }
                    Keycode::Comma | Keycode::Period => {
                        if let Some(scroll) = &mut clip.scroller {
                            let step = if key == Keycode::Comma { -0.1 } else { 0.1 };
                            scroll.wave_depth = (((scroll.wave_depth + step) * 10.0).round()
                                / 10.0)
                                .clamp(0.1, 1.0);
                            println!("Wave depth {:.1}", scroll.wave_depth);
                        }
                    }
                    Keycode::Semicolon | Keycode::Quote => {
                        if let Some(scroll) = &mut clip.scroller {
                            let step = if key == Keycode::Semicolon { -0.1 } else { 0.1 };
                            scroll.wave_light = (((scroll.wave_light + step) * 10.0).round()
                                / 10.0)
                                .clamp(0.0, 1.0);
                            println!("Wave light {:.1}", scroll.wave_light);
                        }
                    }
                    Keycode::N => {
                        if let Some(rings) = &mut clip.rings {
                            rings.original_symbols = !rings.original_symbols;
                            println!(
                                "Part 6 symbols: {}",
                                if rings.original_symbols {
                                    "original"
                                } else {
                                    "new sets in the middle"
                                }
                            );
                        }
                    }
                    Keycode::P => o.fps = !o.fps,
                    Keycode::V => {
                        o.vsync = !o.vsync;
                        config.present_mode = present_mode(o.vsync);
                        surface.configure(&renderer.device, &config);
                    }
                    Keycode::S => capture = true,
                    Keycode::F => {
                        o.fullscreen = !o.fullscreen;
                        window.set_fullscreen(if o.fullscreen {
                            FullscreenType::Desktop
                        } else {
                            FullscreenType::Off
                        })?;
                    }
                    _ => {}
                },
                Event::Window {
                    win_event: WindowEvent::Minimized,
                    ..
                } => {
                    device.pause();
                    paused = true;
                }
                _ => {}
            }
        }
        if let Some((direction, since)) = hold {
            let key = if direction < 0.0 {
                Scancode::Left
            } else {
                Scancode::Right
            };
            let held = now.duration_since(since).as_secs_f64();
            if !events.keyboard_state().is_scancode_pressed(key) {
                hold = None;
                if scrubbing {
                    // Let go: the music picks up from here.
                    scrubbing = false;
                    seek = Some(time);
                    println!("Scrubbed to {time:.1}s");
                }
            } else if held > SCRUB_DELAY {
                if !scrubbing {
                    scrubbing = true;
                    device.pause();
                }
                let (start, gain, most) = SCRUB_SPEED;
                let speed = (start + gain * (held - SCRUB_DELAY)).min(most);
                time = (time + direction * speed * delta).clamp(0.0, clip.duration());
            }
        }
        if let Some(t) = seek {
            time = t;
            device.pause();
            settle = true;
        }
        if capture {
            match renderer.capture(
                &clip,
                View {
                    time,
                    size: o.size,
                    comparison: o.comparison,
                    modern: o.modern,
                    hud: None,
                },
                &o.out,
                format,
            ) {
                Ok(()) => println!("Saved {}", o.out.display()),
                Err(e) => eprintln!("Capture: {e}"),
            }
        }
        let (w, h) = window.drawable_size();
        if w == 0 || h == 0 {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if (w, h) != (config.width, config.height) {
            config.width = w;
            config.height = h;
            surface.configure(&renderer.device, &config);
        }
        if last_title.elapsed() > Duration::from_millis(200) {
            let mode = match o.comparison {
                1 => "Original",
                2 => "Original | HD",
                _ => {
                    if o.modern {
                        "Modern 3D"
                    } else {
                        "Faithful HD"
                    }
                }
            };
            // Parts 8, 7, 4, 3, 5 and 6 show their own; the scroller's settings
            // before them.
            let part5 = if clip.ending_frame(time).is_some() {
                Some(" · part 8 (the ending)".to_owned())
            } else if clip.greetings_frame(time).is_some() {
                Some(" · part 7 (Greetings)".to_owned())
            } else if clip.plasma_frame(time).is_some() {
                Some(" · part 4 (Plasma)".to_owned())
            } else if clip.dots_frame(time).is_some() {
                Some(" · part 3 (Dots)".to_owned())
            } else {
                clip.field_frame(time)
                    .map(|_| " · part 5 (Field)".to_owned())
            };
            let style = clip
                .rings
                .as_ref()
                .filter(|_| part5.is_none() && clip.rings_frame(time).is_some())
                .map(|r| {
                    let symbols = match r.symbol_set_at(time) {
                        Some(title) => format!(" · symbols: {title}"),
                        None if r.original_symbols => " · original symbols".to_owned(),
                        None => String::new(),
                    };
                    format!(" · style {}{symbols}", r.style.name())
                });
            let preset = part5.or(style).or_else(|| {
                clip.scroller.as_ref().map(|s| {
                    format!(
                        " · {:?} · {:?} {:.1}/{:.1} · waves {} {:.1}/{:.1}",
                        s.lighting,
                        s.backdrop_style,
                        s.wall_shape,
                        s.wall_strength,
                        if s.waves { "on" } else { "off" },
                        s.wave_depth,
                        s.wave_light
                    )
                })
            });
            let preset = preset.unwrap_or_default();
            window
                .set_title(&format!(
                    "Panic · {}{preset} · {time:.1}s · {w}×{h} · {mode}{}",
                    clip.label(time),
                    if paused { " · Paused" } else { "" }
                ))
                .map_err(|e| e.to_string())?;
            last_title = Instant::now();
        }
        renderer.prepare(
            &clip,
            View {
                time,
                size: (w, h),
                comparison: o.comparison,
                modern: o.modern,
                hud: o.fps.then_some(rate),
            },
        );
        if settle {
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            anchor = time;
            queued = set_audio(&device, &clip, time, o.mute, paused || scrubbing)?;
            heard = (time, Instant::now());
            settle = false;
        }
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&renderer.device, &config);
                continue;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            status => return Err(format!("Display surface: {status:?}")),
        };
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        renderer.draw(
            &mut encoder,
            &frame.texture.create_view(&Default::default()),
        );
        renderer.queue.submit([encoder.finish()]);
        renderer.queue.present(frame);
        presented += 1;
        rate_frames += 1;
        let elapsed = rate_start.elapsed().as_secs_f32();
        if elapsed >= 0.25 {
            rate = [
                rate_frames as f32 / elapsed,
                elapsed * 1000.0 / rate_frames as f32,
            ];
            rate_frames = 0;
            rate_start = Instant::now();
        }
    }
    device.pause();
    println!(
        "Presented {presented} frames in {:.2}s ({:.1} fps average), {}×{}",
        runtime.elapsed().as_secs_f64(),
        presented as f64 / runtime.elapsed().as_secs_f64(),
        config.width,
        config.height
    );
    Ok(())
}
