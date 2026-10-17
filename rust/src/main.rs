use panic_demo::{
    data::Data,
    engine::{Engine, FRAME_RATE, SAMPLE_RATE},
};
use std::{
    env, fs,
    io::{BufWriter, Seek, Write},
    path::{Path, PathBuf},
};

struct Options {
    data: PathBuf,
    headless: bool,
    out: Option<PathBuf>,
    seconds: Option<f64>,
    seek: f64,
    mute: bool,
    fallback: bool,
    fullscreen: bool,
}
fn options() -> Result<Options, String> {
    let mut o = Options {
        data: env::var_os("PANIC_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".")),
        headless: false,
        out: None,
        seconds: None,
        seek: 0.0,
        mute: false,
        fallback: false,
        fullscreen: false,
    };
    let mut a = env::args().skip(1);
    while let Some(arg) = a.next() {
        match arg.as_str() {
            "--data" => o.data = PathBuf::from(a.next().ok_or("--data requires a path")?),
            "--headless" => o.headless = true,
            "--out" => o.out = Some(PathBuf::from(a.next().ok_or("--out requires a directory")?)),
            "--seconds" => {
                o.seconds = Some(
                    a.next()
                        .ok_or("--seconds requires a number")?
                        .parse()
                        .map_err(|_| "Invalid seconds")?,
                )
            }
            "--seek" => {
                o.seek = a
                    .next()
                    .ok_or("--seek requires a number")?
                    .parse()
                    .map_err(|_| "Invalid seek time")?
            }
            "--mute" => o.mute = true,
            "--fallback" => o.fallback = true,
            "--fullscreen" => o.fullscreen = true,
            "-h" | "--help" => {
                println!(
                    "Panic — Future Crew (1992), portable Rust reconstruction\n\nUsage: panic-demo [options]\n  --data PATH     Research root or portable data directory (default: ./data or .)\n  --headless      Render without a window or audio device\n  --out DIR       Headless export: WAV, 10-second PPM snapshots, timeline.csv\n  --seconds N     Stop after N seconds (after seeking)\n  --seek N        Fast-forward N seconds before playback\n  --mute          Keep music synchronization, silence the speakers\n  --fallback      Use the original no-music scene schedules\n  --fullscreen    Start fullscreen\n\nDesktop controls: Escape quits; Space pauses; F toggles fullscreen.\nUse --release for smooth playback."
                );
                std::process::exit(0);
            }
            _ => return Err(format!("Unknown argument {arg}; use --help")),
        }
    }
    if !o.seek.is_finite() || o.seek < 0.0 || o.seconds.is_some_and(|s| !s.is_finite() || s <= 0.0)
    {
        return Err("Times must be finite and positive (seek may be zero)".into());
    }
    if o.data == Path::new(".") {
        let executable = env::current_exe().map_err(|e| e.to_string())?;
        let parent = executable
            .parent()
            .ok_or("Executable has no parent directory")?;
        for candidate in [
            PathBuf::from("data"),
            parent.join("data"),
            parent.join("../Resources/data"),
        ] {
            if candidate.join("files/P.P").is_file() {
                o.data = candidate;
                break;
            }
        }
    }
    Ok(o)
}
struct Wav {
    file: BufWriter<fs::File>,
    bytes: u32,
}
impl Wav {
    fn new(path: &Path) -> Result<Self, String> {
        let mut w = Self {
            file: BufWriter::new(fs::File::create(path).map_err(|e| e.to_string())?),
            bytes: 0,
        };
        w.file.write_all(&[0; 44]).map_err(|e| e.to_string())?;
        Ok(w)
    }
    fn write(&mut self, pcm: &[i16]) -> Result<(), String> {
        for &s in pcm {
            self.file
                .write_all(&s.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        self.bytes += pcm.len() as u32 * 2;
        Ok(())
    }
    fn finish(mut self) -> Result<(), String> {
        let mut h = Vec::from(*b"RIFF");
        h.extend((self.bytes + 36).to_le_bytes());
        h.extend(b"WAVEfmt ");
        h.extend(16u32.to_le_bytes());
        h.extend(1u16.to_le_bytes());
        h.extend(2u16.to_le_bytes());
        h.extend((SAMPLE_RATE as u32).to_le_bytes());
        h.extend((SAMPLE_RATE as u32 * 4).to_le_bytes());
        h.extend(4u16.to_le_bytes());
        h.extend(16u16.to_le_bytes());
        h.extend(b"data");
        h.extend(self.bytes.to_le_bytes());
        self.file.rewind().map_err(|e| e.to_string())?;
        self.file.write_all(&h).map_err(|e| e.to_string())?;
        self.file.flush().map_err(|e| e.to_string())
    }
}
fn ppm(path: &Path, engine: &Engine) -> Result<(), String> {
    let v = &engine.video;
    let mut f = BufWriter::new(fs::File::create(path).map_err(|e| e.to_string())?);
    write!(f, "P6\n{} {}\n255\n", v.width, v.height).map_err(|e| e.to_string())?;
    f.write_all(&v.rgb()).map_err(|e| e.to_string())?;
    f.flush().map_err(|e| e.to_string())
}
fn headless(mut engine: Engine, o: &Options) -> Result<(), String> {
    let end = o
        .seconds
        .map(|s| engine.frames + (s * FRAME_RATE as f64).ceil() as u64);
    let mut wav = None;
    let mut timeline = None;
    if let Some(dir) = &o.out {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        wav = Some(Wav::new(&dir.join("panic.wav"))?);
        let mut csv =
            BufWriter::new(fs::File::create(dir.join("timeline.csv")).map_err(|e| e.to_string())?);
        writeln!(
            csv,
            "frame,seconds,part,music_order,music_row,music_tick,width,height"
        )
        .map_err(|e| e.to_string())?;
        timeline = Some(csv);
    }
    let start = std::time::Instant::now();
    let mut part = 0;
    while !engine.finished && end.is_none_or(|e| engine.frames < e) {
        let pcm = engine.step()?;
        if let Some(w) = &mut wav {
            w.write(&pcm)?;
        }
        let changed = engine.part() != part;
        if changed {
            part = engine.part();
            println!(
                "{:8.3}s  part {}  music {:04x}",
                engine.seconds(),
                part,
                engine.music.position()
            );
        }
        if let Some(t) = &mut timeline {
            writeln!(
                t,
                "{},{:.6},{},{},{},{},{},{}",
                engine.frames,
                engine.seconds(),
                engine.part(),
                engine.music.position() >> 8,
                engine.music.position() & 255,
                engine.music.tick(),
                engine.video.width,
                engine.video.height
            )
            .map_err(|e| e.to_string())?;
        }
        if (changed || engine.frames.is_multiple_of(FRAME_RATE * 10) || engine.finished)
            && let Some(dir) = &o.out
        {
            ppm(
                &dir.join(format!(
                    "frame-{:06}-part{}.ppm",
                    engine.frames,
                    engine.part()
                )),
                &engine,
            )?;
        }
    }
    if let Some(w) = wav {
        w.finish()?;
    }
    if let Some(mut t) = timeline {
        t.flush().map_err(|e| e.to_string())?;
    }
    if let Some(dir) = &o.out {
        ppm(&dir.join("last.ppm"), &engine)?;
    }
    println!(
        "{} at {:.3}s ({} frames), music {:04x}/tick {}; rendered in {:.2}s",
        if engine.finished {
            "Completed"
        } else {
            "Stopped"
        },
        engine.seconds(),
        engine.frames,
        engine.music.position(),
        engine.music.tick(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(feature = "desktop")]
fn desktop(mut engine: Engine, o: &Options) -> Result<(), String> {
    use sdl2::{
        audio::AudioSpecDesired, event::Event, keyboard::Keycode, pixels::PixelFormatEnum,
        rect::Rect, video::FullscreenType,
    };
    use std::{
        collections::VecDeque,
        time::{Duration, Instant},
    };
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let audio = sdl.audio()?;
    let mut builder = video.window("Panic — Future Crew / Rust", 960, 720);
    builder.position_centered().resizable().allow_highdpi();
    let window = builder.build().map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    if o.fullscreen {
        canvas
            .window_mut()
            .set_fullscreen(FullscreenType::Desktop)?;
    }
    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 320, 400)
        .map_err(|e| e.to_string())?;
    let device = audio.open_queue::<i16, _>(
        None,
        &AudioSpecDesired {
            freq: Some(SAMPLE_RATE as i32),
            channels: Some(2),
            samples: Some(512),
        },
    )?;
    if device.spec().freq != SAMPLE_RATE as i32 || device.spec().channels != 2 {
        return Err("Audio device did not accept 16 kHz stereo".into());
    }
    let mut events = sdl.event_pump()?;
    let origin = engine.audio_frames();
    let end = o
        .seconds
        .map(|s| origin + (s * SAMPLE_RATE as f64).ceil() as u64);
    let mut submitted = 0u64;
    let mut paused = false;
    let mut full = o.fullscreen;
    let mut started = false;
    let mut queue = VecDeque::new();
    let mut shown = (
        engine.video.rgb(),
        engine.video.width,
        engine.video.height,
        engine.part(),
    );
    let mut previous_part = 0;
    let mut paused_at = Instant::now();
    let wall_start = Instant::now();
    let mut presentations = 0u64;
    if let Some(dir) = &o.out {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    'running: loop {
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    repeat: false,
                    ..
                } => {
                    paused = !paused;
                    if paused {
                        device.pause();
                        paused_at = Instant::now();
                    } else {
                        device.resume();
                    }
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F),
                    repeat: false,
                    ..
                } => {
                    full = !full;
                    canvas.window_mut().set_fullscreen(if full {
                        FullscreenType::Desktop
                    } else {
                        FullscreenType::Off
                    })?;
                }
                _ => {}
            }
        }
        let played = submitted.saturating_sub(device.size() as u64 / 4);
        if !paused {
            while !engine.finished
                && engine.audio_frames() - origin < played + SAMPLE_RATE * 3 / FRAME_RATE
                && end.is_none_or(|e| engine.audio_frames() < e)
            {
                let start = engine.audio_frames() - origin;
                let mut pcm = engine.step()?;
                let count = pcm.len() as u64 / 2;
                queue.push_back((
                    start,
                    engine.video.rgb(),
                    engine.video.width,
                    engine.video.height,
                    engine.part(),
                ));
                if o.mute {
                    pcm.fill(0);
                }
                device.queue_audio(&pcm)?;
                submitted += count;
            }
            if !started {
                device.resume();
                started = true;
            }
            while queue.front().is_some_and(|f| f.0 <= played) {
                let (_, rgb, w, h, p) = queue.pop_front().unwrap();
                shown = (rgb, w, h, p);
            }
            if (engine.finished || end.is_some_and(|e| engine.audio_frames() >= e))
                && played >= submitted
            {
                break;
            }
        }
        let (rgb, w, h, part) = &shown;
        let changed = *part != previous_part;
        if changed {
            println!("part {}", part);
            previous_part = *part;
        }
        texture
            .update(Rect::new(0, 0, *w as u32, *h as u32), rgb, *w * 3)
            .map_err(|e| e.to_string())?;
        let (cw, ch) = canvas.output_size()?;
        let (dw, dh) = if cw * 3 > ch * 4 {
            (ch * 4 / 3, ch)
        } else {
            (cw, cw * 3 / 4)
        };
        canvas.set_draw_color(sdl2::pixels::Color::BLACK);
        canvas.clear();
        canvas.copy(
            &texture,
            Rect::new(0, 0, *w as u32, *h as u32),
            Rect::new(((cw - dw) / 2) as i32, ((ch - dh) / 2) as i32, dw, dh),
        )?;
        if changed && let Some(dir) = &o.out {
            let readback = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
            let mut file = BufWriter::new(
                fs::File::create(dir.join(format!("desktop-part{part}.ppm")))
                    .map_err(|e| e.to_string())?,
            );
            write!(file, "P6\n{cw} {ch}\n255\n").map_err(|e| e.to_string())?;
            file.write_all(&readback).map_err(|e| e.to_string())?;
            file.flush().map_err(|e| e.to_string())?;
        }
        canvas.present();
        presentations += 1;
        if paused && paused_at.elapsed() > Duration::from_millis(50) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    device.pause();
    if let Some(dir) = &o.out {
        fs::write(dir.join("desktop-run.json"),format!("{{\n  \"renderer\": \"{}\",\n  \"audio_driver\": \"{}\",\n  \"sample_rate\": {},\n  \"channels\": {},\n  \"queued_frames\": {},\n  \"remaining_frames\": {},\n  \"presentations\": {},\n  \"elapsed_seconds\": {:.6},\n  \"demo_seconds\": {:.6},\n  \"part\": {},\n  \"complete\": {}\n}}\n",canvas.info().name,audio.current_audio_driver(),device.spec().freq,device.spec().channels,submitted,device.size()/4,presentations,wall_start.elapsed().as_secs_f64(),engine.seconds(),engine.part(),engine.finished)).map_err(|e|e.to_string())?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let o = options()?;
    let mut engine = Engine::new(Data::new(&o.data)?, o.fallback)?;
    let seek = (o.seek * FRAME_RATE as f64).floor() as u64;
    while engine.frames < seek && !engine.finished {
        engine.step()?;
    }
    if o.headless {
        return headless(engine, &o);
    }
    #[cfg(feature = "desktop")]
    {
        desktop(engine, &o)
    }
    #[cfg(not(feature = "desktop"))]
    {
        Err("Desktop support is disabled; use --headless or rebuild with default features".into())
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("panic-demo: {e}");
        std::process::exit(1);
    }
}
