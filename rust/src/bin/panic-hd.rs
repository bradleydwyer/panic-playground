use panic_demo::{
    data::Data,
    engine::SAMPLE_RATE,
    hd::{
        clip::Clip,
        renderer::{Renderer, View},
    },
};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::{Event, WindowEvent},
    keyboard::Keycode,
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
    transition: bool,
    fps: bool,
    vsync: bool,
    seconds: Option<f64>,
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
            transition: false,
            fps: false,
            vsync: true,
            seconds: None,
        };
        let mut args = std::env::args().skip(1);
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
                "--fps" => o.fps = true,
                "--no-vsync" => o.vsync = false,
                // --laser is kept as an alias from the transition trials.
                "--transition" | "--laser" => {
                    o.transition = true;
                    o.modern = true;
                }
                "--logo" => {
                    o.seek = f64::MAX;
                    o.paused = true;
                }
                "-h" | "--help" => {
                    println!(
                        "Panic HD — opening through the logo\n\nUsage: panic-hd [options]\n  --data DIR       Locally imported data (or PANIC_DATA)\n  --size WxH       PNG size / window aspect; default 1920x1080\n  --seek SECONDS   Start at a point in the intro\n  --logo           Start paused on the completed logo\n  --paused         Start paused\n  --modern         3D letter fly-bys, HDR lighting and heated metal\n  --transition     Continue through the laser-cut tile transition (modern)\n  --laser          Same as --transition\n  --original       Show the original rendering\n  --split          Original on the left, HD on the right\n  --mute           Silent playback, with music timing\n  --fullscreen     Start fullscreen\n  --headless       Export one native-resolution PNG, without a window\n  --out FILE       Capture destination; default artifacts/intro-hd.png\n  --seconds N      Close desktop preview after N seconds\n  --fps            Show the frame rate (P toggles it)\n  --no-vsync       Don't wait for the display refresh (V toggles it)\n\nSpace: pause · Left/Right: seek 1s · Home/R: replay · End: logo\nTab: original/HD · C: split comparison · M: faithful/modern · F: fullscreen · S: save PNG · P: frame rate · V: vsync · Esc: quit\nThe preview holds on the logo. --transition continues to the next backdrop; its scroller is not played."
                    );
                    std::process::exit(0);
                }
                _ => return Err(format!("Unknown option {arg}; use --help")),
            }
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
    let clip = if o.transition {
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
        "Space pause · arrows seek · Home replay · End logo · Tab original/HD · C compare · M faithful/modern · F fullscreen · S capture"
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
    let mut paused = o.paused;
    let mut anchor = o.seek;
    let mut queued = set_audio(&device, &clip, anchor, o.mute, paused)?;
    let mut time = anchor;
    let mut events = sdl.event_pump()?;
    let mut last_title = Instant::now() - Duration::from_secs(1);
    let runtime = Instant::now();
    let mut presented = 0_u64;
    let mut last_frame = Instant::now();
    // Frames per second and milliseconds per frame, averaged over ~0.25 s.
    let mut rate = [0.0_f32; 2];
    let mut rate_frames = 0_u32;
    let mut rate_start = Instant::now();
    'preview: loop {
        let now = Instant::now();
        let delta = now.duration_since(last_frame).as_secs_f64();
        last_frame = now;
        if o.seconds
            .is_some_and(|s| runtime.elapsed().as_secs_f64() >= s)
        {
            break;
        }
        if !paused {
            let remaining = device.size() as usize / 2;
            let audio_time =
                anchor + queued.saturating_sub(remaining) as f64 / (SAMPLE_RATE * 2) as f64;
            // Audio remains the clock throughout the sequence. Once the hold's
            // audio finishes, let the logo material keep moving until paused.
            time = if remaining == 0 {
                time.max(audio_time) + delta
            } else {
                audio_time
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
                    repeat: false,
                    ..
                } => match key {
                    Keycode::Space => {
                        paused = !paused;
                        if paused {
                            device.pause();
                        } else {
                            device.resume();
                        }
                    }
                    Keycode::Left => seek = Some((time - 1.0).max(0.0)),
                    Keycode::Right => seek = Some((time + 1.0).min(clip.duration())),
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
        if let Some(t) = seek {
            time = t;
            anchor = t;
            queued = set_audio(&device, &clip, t, o.mute, paused)?;
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
            window
                .set_title(&format!(
                    "Panic · {} · {time:.1}s · {w}×{h} · {mode}{}",
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
