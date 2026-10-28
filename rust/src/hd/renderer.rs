use super::clip::{Clip, HEIGHT, ScrollerLighting, ScrollerPreview, WIDTH, plate_transform};
use super::dots::{DotsBlock, DotsPreview, PART3_PHASE};
use super::dots_render::DotsRenderer;
use super::ending::{EndingBlock, EndingPreview, PART8_PHASE};
use super::ending_render::EndingRenderer;
use super::field::{FRAME_TAKES_PART6_FADE, FieldBlock, FieldPreview, PART5_PHASE};
use super::field_render::FieldRenderer;
use super::greetings::{GreetingsBlock, GreetingsPreview, PART7_PHASE};
use super::greetings_render::GreetingsRenderer;
use super::modern::Modern;
use super::plasma::{PART4_PHASE, PlasmaBlock, PlasmaPreview};
use super::plasma_render::PlasmaRenderer;
use super::rings::{PART6_PHASE, RingsPreview, RingsStyle};
use super::rings_render::RingsRenderer;
use bytemuck::{Pod, Zeroable};
use std::{fs::File, io::BufWriter, path::Path};
use wgpu::util::DeviceExt;

/// Shared controls for every shader. [`CONTROLS_WGSL`] declares the same
/// layout once for WGSL; keep the two in step (the tests check them).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    screen: [f32; 4],
    state: [f32; 4],
    misc: [f32; 4],
    flights: [[f32; 4]; 10],
    plate_center: [f32; 4],
    plate_rotation: [f32; 4],
    transition: [f32; 4],
    tile_order: [[f32; 4]; 40],
    next_palette: [[f32; 4]; 64],
    scroller: [f32; 4],
    scroll_light: [f32; 4],
    /// Authored music-reactive intro lighting, measured from the clip's PCM
    /// (see `clip::audio_cues`): energy, onset, low band, accent, each 0..=1.
    audio: [f32; 4],
    /// Plate pose when the modern renderer's virtual shutter opened; w of the
    /// centre carries the exposure length in sequence frames.
    plate_open_center: [f32; 4],
    plate_open_rotation: [f32; 4],
    /// On-screen counter: shown, frames per second, milliseconds per frame.
    hud: [f32; 4],
    /// Scroller text positions read from the original's scroll counters and
    /// interpolated between its frames: upper and lower row text pixel at
    /// screen x = 0, overlay step, and an unused slot (formerly the archived
    /// ring layout).
    glyphs: [f32; 4],
    /// Scroller background: oscillator `xx / 128` and `yy / 128` interpolated
    /// between ticks, backdrop style (0 chamber, 1 wall, 2 chrome) and the
    /// authored bump shape.
    backdrop: [f32; 4],
    /// Bump wall: oscillator movement over its exposure (source pixels, from
    /// shutter open to now) and the authored wall strength.
    backdrop_motion: [f32; 4],
    /// Authored wall waves (`ScrollerPreview::waves_at`): depth (0 when off),
    /// the exposure in seconds, the side light's strength, and the swing
    /// (`ScrollerPreview::wave_swing`).
    waves: [f32; 4],
    // Part 6 (`rings::RingsPreview::controls`); all zero before it. The WGSL
    // declaration below documents each field.
    rings: [f32; 4],
    ring_centres: [f32; 4],
    ring_open: [f32; 4],
    ring_bands: [f32; 4],
    ring_fades: [f32; 4],
    symbol: [f32; 4],
    symbol_layers: [f32; 4],
    symbol_atlas: [f32; 4],
    authored_atlas: [f32; 4],
    neon_map: [f32; 4],
    /// Spare values for the selected part-6 style (`StyleInfo::controls`).
    rings_style: [[f32; 4]; 4],
    /// Part 5 (`field::FieldPreview::controls`); all zero before it.
    field: FieldBlock,
    /// Part 3 (`dots::DotsPreview::controls`); all zero before it.
    dots: DotsBlock,
    /// Part 4 (`plasma::PlasmaPreview::controls`); all zero before it.
    plasma: PlasmaBlock,
    /// Part 7 (`greetings::GreetingsPreview::controls`); all zero before it.
    greetings: GreetingsBlock,
    /// Part 8 (`ending::EndingPreview::controls`); all zero before it.
    ending: EndingBlock,
}

/// The only WGSL declaration of [`Uniforms`] and its binding. [`wgsl`]
/// prepends it to every shader module, so no module can declare a stale or
/// partial copy that silently shifts later fields.
const CONTROLS_WGSL: &str = "\
// Part 5's share of the controls (hd/field.rs, FieldBlock), in u.field and
// in each fluid step's own block.
struct FieldBlock {
    // Shown (1), seconds since part 5 began (negative before), phase (0
    // zoom, 1 terrain), part-5 frame (continuous).
    part: vec4f,
    // Zoom scale in the shown buffer's pixels per window pixel (below 1 while
    // the next level is due), 1 when the next frame swaps, level (log2 of
    // the magnification since the first zoom frame), the shown buffer's swaps.
    zoom: vec4f,
    // Speed / 1024, rows of the next level built, running, shown.
    zoom_state: vec4f,
    // Camera: map x and y, height, angle (1024ths of a turn, unwrapped).
    camera: vec4f,
    // Sine and cosine of the angle (amplitude 256), orbit radius, distance
    // from the map centre.
    view: vec4f,
    // Terrain level, projection q (0 flat to 248), terrain frame, terrain.
    mountain: vec4f,
    // Window cleared, seconds since, PSTART level, terrain pixels shown.
    window: vec4f,
    // Mandelbrot fit: c at map (100, 100), map units per unit of c along map
    // x, aspect; base, slope, plateau IoU, slopes' rms.
    mandel: vec4f,
    mandel_fit: vec4f,
    // Relief: texels a side, map units a texel, highest height, plateau DEP.
    relief: vec4f,
    // Part-5 seconds: full speed, braking, terrain, cleared; risen, sinking,
    // last terrain pixel, end.
    events: vec4f,
    events_late: vec4f,
    // Fluid: steps done, fraction to the latest, seconds a step, seconds
    // since its start; the source rectangle its grid covers.
    fluid: vec4f,
    fluid_region: vec4f,
    // Spare values from each author's controls hook.
    frame_style: array<vec4f,4>,
    zoom_style: array<vec4f,4>,
    mountain_style: array<vec4f,4>,
    // Part 3's view of part 5's last frame: the magnification about the
    // screen's centre (0 or 1 for none, as in part 5), unused.
    part3_view: vec4f,
}
// Part 3's controls (hd/dots.rs, DotsBlock), in u.dots.
struct DotsBlock {
    // Shown (1), seconds since part 3 began, phase (0 hold, 1 zoom, 2 shade
    // bobs, 3 dots, 4 part 4 waiting, 5 part 4's slab), part-3 frame.
    part: vec4f,
    // Magnification: the original's, the modern view's, the modern's rate
    // (d ln M / dt per second), part 5's frame still shown in modern (1).
    zoom: vec4f,
    // Shade iterations drawn (continuous), the stop, shown, the shown
    // frame's iterations.
    shades: vec4f,
    // Dot frame (continuous; -1 before), the last, the rotation's stop,
    // shown.
    dots: vec4f,
    // Focus distance; the object's centre in camera space.
    focus: vec4f,
    // Seconds since the rotation stopped (-1 before), until part 4's slab,
    // slab shown, seconds since it.
    end: vec4f,
    // Part-3 seconds: zoom, zoom end, shades, shade stop; shades end, dots,
    // rotation stop, part 4.
    events: vec4f,
    events_late: vec4f,
    // Seconds since the last four orders' first rows (newest first, -100
    // none) and their strengths.
    hits: vec4f,
    hit_strength: vec4f,
    // The PCM's low band now and at each of the 39 ticks before.
    low: array<vec4f,10>,
    // The port's eight echo levels (6-bit blue), echo k 4k frames old.
    echo_levels: array<vec4f,2>,
    // The water: mirror gain, swell height, swell clock, swell size; where
    // it lies: its tilt and the camera's height above it, unused; the
    // drops: place across and back, age, 1 (0 none); the lights in it:
    // ripples, glow and plankton, of the authored, the plankton's life.
    water: vec4f,
    water_plane: vec4f,
    drops: array<vec4f,6>,
    water_lights: vec4f,
}
// Part 4's controls (hd/plasma.rs, PlasmaBlock), in u.plasma.
struct PlasmaBlock {
    // Shown (1), seconds since its slab frame, phase (1 slab from white, 2
    // plasma, 3 slide, 4 part 7's first frame), part-4 frame.
    part: vec4f,
    // The frame shown, the next, the fraction toward it, blend (1).
    frame: vec4f,
    // The palette's rotation and level; green's and blue's offsets.
    palette: vec4f,
    // Line compare, the slab's slide (source rows), the fade from white
    // (0 to 1), part 7's frame shown.
    logo: vec4f,
    // Part-4 seconds: plasma, fade in, full, fade out; dark, slide, part 7.
    events: vec4f,
    events_late: vec4f,
    // Focal length (rows), axis column, part 3's weight, exposure.
    camera: vec4f,
    // The slab: near top edge's centre, rounding; tilt's sine and cosine,
    // width, depth; height, deepest carving, stone red and green; stone
    // blue, the ground (0 water, 1 polished floor).
    slab_origin: vec4f,
    slab_axes: vec4f,
    slab_size: vec4f,
    stone: vec4f,
    // The front sheet of light: bottom edge's centre, height; half width,
    // up.
    sheet: vec4f,
    sheet_size: vec4f,
    // Seconds since the last four orders' first rows (newest first, -100
    // none) and their strengths.
    hits: vec4f,
    hit_strength: vec4f,
    // The modern view's palette, smoothed over time: rotation per frame,
    // rotation, level.
    motion: vec4f,
    // The back sheet: bottom edge's centre, height; half width, up.
    sheet_back: vec4f,
    sheet_back_size: vec4f,
    // The water: mirror gain, swell height, swell clock, swell size; the
    // drops: place across and back, age, 1 (0 none); the ground: its point
    // below the slab's near top edge, slide included, and its tilt.
    water: vec4f,
    drops: array<vec4f,6>,
    ground: vec4f,
}
// Part 7's controls (hd/greetings.rs, GreetingsBlock), in u.greetings.
struct GreetingsBlock {
    // Shown (1), seconds since its first frame, stage (0 first frame, 1
    // words, 2 white, 3 monster, 4 part 8's first frame), part-7 frame.
    part: vec4f,
    // The frame shown, the next, the fraction toward it, strips moving (1).
    frame: vec4f,
    // Star colours' phase and intensity (0 to 1); white fade, monster level.
    sweep: vec4f,
    // The strips' sine positions 0 to 3; 4, the first slot's top row.
    twist: vec4f,
    twist_late: vec4f,
    // Part-7 seconds: lit, words, words' end, white; monster, part 8.
    events: vec4f,
    events_late: vec4f,
    // The modern view's beam: its phase (the original's colour steps, 64 a
    // turn), its tilt toward the camera (radians), its brightness, and how
    // fast its phase moves against the original's.
    beam: vec4f,
    // The beam's width (times the shader's spread).
    beam_late: vec4f,
}
// Part 8's controls (hd/ending.rs, EndingBlock), in u.ending.
struct EndingBlock {
    // Shown (1), seconds since its first frame, stage, part-8 frame.
    part: vec4f,
    // The frame shown, the next, the fraction toward it, its picture.
    frame: vec4f,
    // The CRT's presence, colour 0's level, the white frame, unused.
    tube: vec4f,
    // Compression, picture shown, line shown, fade to black.
    beam: vec4f,
    // The line's lit columns; its row's first and last with colour.
    ends: vec4f,
    // The dot's glow and afterglow, the white's afterglow; line colour.
    glow: vec4f,
    colour: vec4f,
    // The trail's taps: band rows, band light, line light, line ends.
    trail_rows: vec4f,
    trail_band: vec4f,
    trail_line: vec4f,
    trail_left: vec4f,
    trail_right: vec4f,
    // Part-8 seconds: white, collapse, line, dot; end, last.
    events: vec4f,
    events_late: vec4f,
    // The camera: its position and tan(half its field of view); its aim and
    // its draw back over a sixtieth of a second.
    eye: vec4f,
    aim: vec4f,
    // The look's ramps: grade, grille, the screen's edge, the bezel's light.
    look: vec4f,
    // The grille as filmed: triad pitch (rows), the sensor's lines; unused.
    moire: vec4f,
}
struct Controls {
    screen: vec4f, // drawable width/height, audio-clock seconds, comparison mode
    state: vec4f,  // reference phase, sequence frame, door, temporal shape interpolation
    misc: vec4f,   // composition aspect, sRGB output, logo reveal time, modern style
    flights: array<vec4f,10>, plate_center: vec4f, plate_rotation: vec4f,
    // next_palette: the laser cut's destination colours; with the scroller,
    // its 8x8 background image (linear, unfiltered; see clip.rs).
    transition: vec4f, tile_order: array<vec4f,40>, next_palette: array<vec4f,64>,
    scroller: vec4f,     // shown, seconds since the text began, unused, exit fade
    scroll_light: vec4f, // relative level, musical (1) or studio (0), bass, treble
    // The intro's authored music response, measured from the clip's PCM,
    // each 0..1: energy, onset (decaying pulse), low band, accent.
    audio: vec4f,
    plate_open_center: vec4f, plate_open_rotation: vec4f,
    hud: vec4f,    // on-screen counter: shown, frames per second, milliseconds per frame
    // Scroller text positions from the original's scroll counters: upper and
    // lower row text pixel at screen x = 0, overlay step, unused.
    glyphs: vec4f,
    // Scroller background: oscillator xx/128 and yy/128 (source pixels,
    // interpolated between ticks), backdrop (0 chamber, 1 tinted wall, 2 chrome
    // wall) and the authored bump shape (0 rounded square .. 1 hemisphere).
    backdrop: vec4f,
    // Bump wall: oscillator movement over its exposure (source pixels), the
    // authored wall strength, unused.
    backdrop_motion: vec4f,
    // Authored wall waves: depth (0 when off), exposure in seconds, side
    // light strength, swing (1 at the depth, -1 turned over; see clip.rs).
    waves: vec4f,
    // Part 6 (Rings), all zero before it; rings_common.wgsl reads them.
    // During part 6 the scroller's fields are reused for the bump wall:
    // backdrop holds the oscillator as part 2 left it and the wall shape,
    // backdrop_motion.z the wall strength, next_palette the tile image.
    // Shown (1), seconds since part 6 began, style (0 pool, the only one),
    // phase (0 waiting, 1 rings, 2 picture).
    rings: vec4f,
    // Ring centres in source pixels, layer 0 xy then layer 1 xy: now, and
    // at the shutter's opening.
    ring_centres: vec4f,
    ring_open: vec4f,
    // Band width, first edge radius, the model's match with M.DAT, unused.
    ring_bands: vec4f,
    // Ring intensity 0..1, PSTART level 0..1, the most the rings have been
    // lit so far (0..1), unused.
    ring_fades: vec4f,
    // The original's lit mask (-1 none), its previous mask, seconds since the
    // lit window began (or since the last ended), seconds until it ends (-1
    // none lit); the windows are taken a frame ahead, so the third is down to
    // -1/70 then.
    symbol: vec4f,
    // The symbol atlas layers that draw the lit and previous symbols (-1
    // none): the original's mask, 0..15, or in the middle of part 6 an
    // authored set's drawing, 16 + 16 k + mask for set k; unused, unused.
    symbol_layers: vec4f,
    // The atlases of the original symbols (layers 0..15) and of the authored
    // sets (16 and on), and the rectangle both neon light maps cover: origin
    // and size in source pixels.
    symbol_atlas: vec4f,
    authored_atlas: vec4f,
    neon_map: vec4f,
    // Spare values for the selected style, from its StyleInfo::controls.
    rings_style: array<vec4f,4>,
    // Part 5 (Field), all zero before it; field_common.wgsl reads it.
    field: FieldBlock,
    // Part 3 (Dots), all zero before it; dots_common.wgsl reads it.
    dots: DotsBlock,
    // Part 4 (Plasma), all zero before it; plasma_common.wgsl reads it.
    plasma: PlasmaBlock,
    // Part 7 (Greetings), all zero before it; greetings_common.wgsl reads it.
    greetings: GreetingsBlock,
    // Part 8 (the ending), all zero before it; ending_common.wgsl reads it.
    ending: EndingBlock,
}
@group(0) @binding(0) var<uniform> u: Controls;
";

/// A shader module's complete source: the shared controls, then its body.
pub(super) fn wgsl(body: &str) -> String {
    [CONTROLS_WGSL, body].concat()
}

/// A module built from several files, after the shared controls. WGSL
/// declarations may come in any order, so a file can use another's.
pub(super) fn wgsl_parts(parts: &[&str]) -> String {
    let mut source = CONTROLS_WGSL.to_owned();
    for part in parts {
        source.push_str(part);
        source.push('\n');
    }
    source
}

/// The scroller's module: scroller.wgsl and the bump wall it shares with
/// part 6.
pub(super) fn scroller_source() -> String {
    wgsl_parts(&[include_str!("scroller.wgsl"), include_str!("wall.wgsl")])
}

/// Authored exposure for the bump wall's motion blur, in 70 Hz ticks: about
/// 1/100 s, matching the intro's shutter.
const WALL_SHUTTER_TICKS: f64 = 0.7;

/// Authored exposure for the modern renderer's motion blur, in sequence
/// frames (35 per second): about 1/100 s, a little under a 180° shutter at
/// 60 Hz. The original has no motion blur; this only smooths the new 3D motion.
const SHUTTER_FRAMES: f32 = 0.35;

#[derive(Clone, Copy)]
pub struct View {
    pub time: f64,
    pub size: (u32, u32),
    pub comparison: u32,
    pub modern: bool,
    /// Frames per second and milliseconds per frame for the on-screen
    /// counter, or `None` to hide it (captures never show it).
    pub hud: Option<[f32; 2]>,
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter_name: String,
    uniform: wgpu::Buffer,
    /// Faithful HD's two shape fields, the current shape and the next.
    /// `field_wants` is what each binding needs: what it would hold with a
    /// texture of its own that keeps its shape while the shape's key stays,
    /// as the key and the frame it was built from. The frame matters: the
    /// two 70 Hz frames of a sequence frame share a key, yet with the local
    /// data their light channels differ at the few hundred pixels that change
    /// between them (observed in the letters' and the doors' phases). It is
    /// kept in the modern view too, so that a switch shows the same.
    /// `field_holds` is what each texture holds; `fields_swapped` binds
    /// texture 1 as the current shape, so that when the next shape becomes
    /// current the bindings swap rather than build it again.
    fields: [wgpu::Texture; 2],
    field_wants: [Option<FieldSource>; 2],
    field_holds: [Option<FieldSource>; 2],
    fields_swapped: bool,
    reference: wgpu::Texture,
    reference_index: Option<usize>,
    /// The clip ([`Clip::id`]) that the part renderers, the glyphs and the
    /// uploads above were made for; a different clip starts them again.
    clip: Option<u64>,
    /// Scroller glyph atlas and tables; 1 × 1 until a scroller clip is shown.
    glyph_atlas: wgpu::Texture,
    scroll_data: wgpu::Texture,
    glyphs_uploaded: bool,
    /// Both scroller distance fields for the displayed moment, rendered once
    /// per frame in world space, so marched rays sample rather than evaluate.
    /// Also 1 × 1 until a scroller clip is shown.
    glyph_field: wgpu::Texture,
    glyph_field_pipeline: wgpu::RenderPipeline,
    field_inputs: wgpu::BindGroupLayout,
    field_bind_group: wgpu::BindGroup,
    glyphs_active: bool,
    inputs: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The scene inputs with texture 0 as the current shape, then texture 1.
    bind_groups: [wgpu::BindGroup; 2],
    background: wgpu::RenderPipeline,
    stars: wgpu::RenderPipeline,
    foreground: wgpu::RenderPipeline,
    hud: wgpu::RenderPipeline,
    hud_visible: bool,
    srgb: bool,
    output_format: wgpu::TextureFormat,
    modern_renderer: Modern,
    modern_view: bool,
    tile_transition: bool,
    scroller: bool,
    /// Part 6's textures and pipelines, made when a clip with part 6 is first
    /// prepared; and whether the prepared moment is in part 6, in which style.
    rings: Option<RingsRenderer>,
    part6: bool,
    rings_style: RingsStyle,
    /// Part 5's, likewise, and whether the prepared moment is drawn by it.
    field: Option<FieldRenderer>,
    part5: bool,
    /// Part 3's, likewise, and whether the moment also draws part 5's view
    /// beneath it (the hold and the zoom).
    dots: Option<DotsRenderer>,
    part3: bool,
    part3_underlay: bool,
    /// Part 4's, likewise, and whether the moment also draws part 3's
    /// modern view for the handover.
    plasma: Option<PlasmaRenderer>,
    part4: bool,
    part4_handover: bool,
    /// Part 7's, likewise.
    greetings: Option<GreetingsRenderer>,
    part7: bool,
    /// Part 8's, likewise.
    ending: Option<EndingRenderer>,
    part8: bool,
}

impl Renderer {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<(Self, wgpu::TextureFormat), String> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: surface,
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("No compatible GPU: {e}"))?;
        let info = adapter.get_info();
        let format = surface
            .map(|s| {
                let caps = s.get_capabilities(&adapter);
                caps.formats
                    .iter()
                    .copied()
                    .find(|f| f.is_srgb())
                    .unwrap_or(caps.formats[0])
            })
            .unwrap_or(wgpu::TextureFormat::Rgba8UnormSrgb);
        // Part 5's smoke binds more storage buffers to a compute pass than
        // wgpu's default of 8 (field_frame_smoke.rs); ask for the adapter's.
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Panic HD GPU"),
                required_limits: wgpu::Limits {
                    max_storage_buffers_per_shader_stage: adapter
                        .limits()
                        .max_storage_buffers_per_shader_stage,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Intro controls"),
            contents: bytemuck::bytes_of(&Uniforms::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let texture = |label, format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let fields = [
            texture("Current shape", wgpu::TextureFormat::Rgba16Float),
            texture("Next shape", wgpu::TextureFormat::Rgba16Float),
        ];
        let reference = texture("Original comparison", wgpu::TextureFormat::Rgba8Unorm);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Continuous field sampling"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Native-resolution intro"),
            source: wgpu::ShaderSource::Wgsl(wgsl(include_str!("intro.wgsl")).into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Intro inputs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                texture_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(5),
                table_entry(),
                texture_entry(7),
            ],
        });
        let field_inputs = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scroller field inputs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(5),
                table_entry(),
            ],
        });
        let glyph_atlas = data_texture(&device, (1, 1), wgpu::TextureFormat::Rg16Float);
        let scroll_data = data_texture(&device, (1, 1), wgpu::TextureFormat::Rgba32Float);
        let glyph_field = field_texture(&device, (1, 1));
        let bind_groups = bind_inputs(
            &device,
            &layout,
            &uniform,
            &fields,
            [&reference, &glyph_atlas, &scroll_data, &glyph_field],
            &sampler,
        );
        let field_bind_group = bind_field(
            &device,
            &field_inputs,
            &uniform,
            [&glyph_atlas, &scroll_data],
            &sampler,
        );
        let scroller_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Scroller distance fields"),
            source: wgpu::ShaderSource::Wgsl(scroller_source().into()),
        });
        let field_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Scroller fields"),
            bind_group_layouts: &[Some(&field_inputs)],
            immediate_size: 0,
        });
        let glyph_field_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glyph_field"),
            layout: Some(&field_layout),
            vertex: wgpu::VertexState {
                module: &scroller_shader,
                entry_point: Some("fullscreen"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &scroller_shader,
                entry_point: Some("glyph_field"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Intro pipeline"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |vertex, fragment, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let background = pipeline("fullscreen", "background", None);
        let stars = pipeline(
            "star_vertex",
            "star_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let foreground = pipeline(
            "fullscreen",
            "foreground",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let hud = pipeline("fullscreen", "hud", Some(wgpu::BlendState::ALPHA_BLENDING));
        let modern_renderer = Modern::new(&device, &layout, &uniform, &reference, format);
        Ok((
            Self {
                device,
                queue,
                adapter_name: format!("{} ({:?})", info.name, info.backend),
                uniform,
                fields,
                field_wants: [None; 2],
                field_holds: [None; 2],
                fields_swapped: false,
                reference,
                reference_index: None,
                clip: None,
                glyph_atlas,
                scroll_data,
                glyphs_uploaded: false,
                glyph_field,
                glyph_field_pipeline,
                field_inputs,
                field_bind_group,
                glyphs_active: false,
                inputs: layout,
                sampler,
                bind_groups,
                background,
                stars,
                foreground,
                hud,
                hud_visible: false,
                srgb: format.is_srgb(),
                output_format: format,
                modern_renderer,
                modern_view: false,
                tile_transition: false,
                scroller: false,
                rings: None,
                part6: false,
                rings_style: RingsStyle::Pool,
                field: None,
                part5: false,
                dots: None,
                part3: false,
                part3_underlay: false,
                plasma: None,
                part4: false,
                part4_handover: false,
                greetings: None,
                part7: false,
                ending: None,
                part8: false,
            },
            format,
        ))
    }

    pub fn prepare(&mut self, clip: &Clip, view: View) {
        let u = self.controls(clip, view);
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Everything [`Renderer::prepare`] does for a moment but writing the
    /// shared controls, which it returns.
    fn controls(&mut self, clip: &Clip, view: View) -> Uniforms {
        let View {
            time,
            size,
            comparison,
            modern,
            hud,
        } = view;
        if self.clip != Some(clip.id()) {
            self.start_clip(clip);
        }
        self.hud_visible = hud.is_some();
        self.modern_view = modern;
        self.part6 = false;
        self.part5 = false;
        self.part3 = false;
        self.part4 = false;
        self.part7 = false;
        self.part8 = false;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
        }
        if let Some(ending) = &clip.ending {
            if self.ending.is_none() {
                self.ending = Some(EndingRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    self.output_format,
                    ending,
                ));
            }
            // Its first frame is part 7's last, which part 7 draws, so the
            // hand-over is exact.
            if ending.position(time) > 0.0 {
                return self.prepare_ending(clip, ending, view);
            }
        }
        if let Some(greetings) = &clip.greetings {
            if self.greetings.is_none() {
                self.greetings = Some(GreetingsRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    self.output_format,
                    greetings,
                ));
            }
            if greetings.frame_at(time).is_some() {
                return self.prepare_greetings(clip, greetings, view);
            }
        }
        if let Some(dots) = &clip.dots
            && self.dots.is_none()
        {
            self.dots = Some(DotsRenderer::new(
                &self.device,
                &self.queue,
                &self.uniform,
                &self.reference,
                self.output_format,
                dots,
            ));
        }
        if let Some(plasma) = &clip.plasma {
            if self.plasma.is_none() {
                self.plasma = Some(PlasmaRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    self.output_format,
                    plasma,
                ));
            }
            if plasma.frame_at(time).is_some() {
                return self.prepare_plasma(clip, plasma, view);
            }
        }
        if let Some(dots) = &clip.dots
            && dots.frame_at(time).is_some()
        {
            return self.prepare_dots(clip, dots, view);
        }
        if let Some(field) = &clip.field {
            self.hold_field(field);
            // Part 6's PSTART fade, when the frame author's modern view
            // takes it over (field::FRAME_TAKES_PART6_FADE).
            let fade = FRAME_TAKES_PART6_FADE
                && modern
                && clip.rings.as_ref().is_some_and(|r| {
                    r.frame_at(time)
                        .is_some_and(|j| r.frames[j].state.phase == 2)
                });
            if field.frame_at(time).is_some() || fade {
                return self.prepare_field(clip, field, view);
            }
            // Ahead of part 5, in either view: a few of its smoke's pre-roll
            // steps, so that they don't all run when the smoke starts; and
            // in the modern view, its zoom's first level, in bands.
            let renderer = self.field.as_mut().unwrap();
            renderer.preroll_fluid(&self.device, &self.queue, field, time);
            if modern {
                renderer.prebuild_zoom(&self.device, &self.queue, field, time, size);
            }
        }
        if let Some(rings) = &clip.rings {
            if self.rings.is_none() {
                self.rings = Some(RingsRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    &self.reference,
                    self.output_format,
                    rings,
                ));
            }
            if let Some(j) = rings.frame_at(time) {
                return self.prepare_rings(clip, rings, j, view);
            }
        }
        // The scroller scene is first seen through the laser cut.
        self.scroller = clip.scroller.is_some()
            && clip
                .transition
                .as_ref()
                .is_some_and(|cut| time * 70.0 >= cut.start_tick as f64);
        self.tile_transition = clip.transition.as_ref().is_some_and(|cut| {
            time * 70.0 >= cut.start_tick as f64 && time * 70.0 < cut.end_tick as f64 + 78.0
        });
        let index = clip.frame_index(time);
        let frame = &clip.frames[index];
        self.glyphs_active = clip.scroller.is_some() && frame.state.phase >= 8;
        let mut next = index;
        if frame.state.phase >= 8 {
            next = (index + 1).min(clip.frames.len() - 1);
        }
        if frame.state.phase == 1 || frame.state.phase == 3 {
            for i in index + 1..(index + 3).min(clip.frames.len()) {
                if clip.frames[i].state.phase != frame.state.phase {
                    break;
                }
                if clip.frames[i].shape_key() != frame.shape_key() {
                    next = i;
                    break;
                }
            }
        }
        // Only faithful HD draws the frame's shapes and the frame itself; the
        // modern view reads the frame only when a comparison shows it.
        self.upload_fields(clip, [index, next], modern);
        if !modern || comparison != 0 {
            self.upload_reference(index, || frame.rgba());
        }
        let mix = if frame.state.phase >= 8 {
            (time * 70.0).fract() as f32
        } else if next == index {
            0.0
        } else {
            ((frame.state.tick as f64 + (time * 70.0).fract()) / 2.0).clamp(0.0, 1.0) as f32
        };
        let sequence_frame = frame.state.frame as f32
            + frame.state.tick as f32 * 0.5
            + (time * 70.0).fract() as f32 * 0.5;
        let state = [
            frame.state.phase as f32,
            sequence_frame,
            frame.state.door as f32,
            mix,
        ];
        let mut u = base_uniforms(clip, view, self.srgb, state);
        u.flights = clip.flights;
        (u.plate_center, u.plate_rotation) = plate_transform(sequence_frame);
        (u.plate_open_center, u.plate_open_rotation) =
            plate_transform(sequence_frame - SHUTTER_FRAMES);
        u.plate_open_center[3] = SHUTTER_FRAMES;
        if let Some(cut) = &clip.transition {
            u.transition = [
                (time * 70.0 - cut.start_tick as f64) as f32,
                1.0,
                (cut.end_tick - cut.start_tick) as f32,
                // Formerly the trial style; only the laser cut remains.
                0.0,
            ];
            for (i, &rank) in cut.order.iter().enumerate() {
                u.tile_order[i / 4][i % 4] = rank as f32;
            }
            u.next_palette = cut.palette;
        }
        if let Some(scroll) = &clip.scroller {
            self.upload_glyphs(scroll);
            if let Some([upper, lower, overlay]) = clip.scroll_at(time) {
                u.glyphs = [upper as f32, lower as f32, overlay as f32, 0.0];
            }
            u.scroller = [
                1.0,
                (time - scroll.start_tick as f64 / 70.0) as f32,
                // Formerly the preset; glass is now the only scroller style.
                0.0,
                (1.0 - ((time * 70.0 - scroll.end_tick as f64) / 12.0).max(0.0)).clamp(0.0, 1.0)
                    as f32,
            ];
            // The same causal cues as the intro; studio mode ignores them.
            let [level, bass, treble] = clip.audio_at(time).scroller;
            u.scroll_light = [
                level,
                if scroll.lighting == ScrollerLighting::Musical {
                    1.0
                } else {
                    0.0
                },
                bass,
                treble,
            ];
            // The scroller replaces the transition's palette with its
            // background image; mix only blends during the exit fade.
            for (i, color) in u.next_palette.iter_mut().enumerate() {
                for (channel, c) in color[..3].iter_mut().enumerate() {
                    *c = scroll.backdrop[index][i][channel] * (1.0 - mix)
                        + scroll.backdrop[next][i][channel] * mix;
                }
                color[3] = 1.0;
            }
            // The oscillator, including its authored lead-in before the
            // scroll, moves by one update per tick, so blending with the
            // following tick is always safe and keeps the motion blur.
            let oscillator_at = |t: f64| {
                let i = clip.frame_index(t);
                let [a, b] = [
                    scroll.oscillator[i],
                    scroll.oscillator[(i + 1).min(clip.frames.len() - 1)],
                ];
                let f = (t.max(0.0) * 70.0).fract() as f32;
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
            };
            let now = oscillator_at(time);
            let open = oscillator_at(time - WALL_SHUTTER_TICKS / 70.0);
            u.backdrop = [
                now[0],
                now[1],
                scroll.backdrop_style as u32 as f32,
                scroll.wall_shape,
            ];
            u.backdrop_motion = [
                now[0] - open[0],
                now[1] - open[1],
                scroll.wall_strength,
                0.0,
            ];
            let reveal = clip.transition.as_ref().map_or(0, |cut| cut.start_tick);
            let [depth, light] = scroll.waves_at(time, reveal);
            let swing = scroll.wave_swing(time, reveal);
            u.waves = [depth, (WALL_SHUTTER_TICKS / 70.0) as f32, light, swing];
        }
        u
    }

    /// Forget what was made for another clip: the part renderers, the
    /// scroller's glyphs and the frames uploaded.
    fn start_clip(&mut self, clip: &Clip) {
        self.clip = Some(clip.id());
        self.rings = None;
        self.field = None;
        self.dots = None;
        self.plasma = None;
        self.greetings = None;
        self.ending = None;
        self.glyphs_uploaded = false;
        self.field_wants = [None; 2];
        self.field_holds = [None; 2];
        self.reference_index = None;
    }

    /// Faithful HD's shape fields: the shown frame's (in the scroller, its
    /// backdrop) as the current shape and the next frame's as the following
    /// one, where faithful HD reads them. Only faithful HD builds them; see
    /// `field_wants` for what each holds.
    fn upload_fields(&mut self, clip: &Clip, frames: [usize; 2], modern: bool) {
        let mut read = [false; 2];
        for (slot, i) in frames.into_iter().enumerate() {
            let frame = &clip.frames[i];
            let backdrop = slot == 0 && frame.state.phase >= 8 && !modern;
            if frame.state.phase < 6 || backdrop {
                let key = frame.shape_key();
                if self.field_wants[slot].is_none_or(|(k, _)| k != key) {
                    self.field_wants[slot] = Some((key, i));
                }
                read[slot] = true;
            }
        }
        if modern {
            return;
        }
        let wanted = [0, 1].map(|slot| self.field_wants[slot].filter(|_| read[slot]));
        let builds;
        (self.fields_swapped, builds) = plan_fields(wanted, self.field_holds, self.fields_swapped);
        // With no other shape coming up, both read the shown one.
        let mut built: Option<(FieldSource, Vec<_>)> = None;
        for (texture, source) in builds.into_iter().enumerate() {
            let Some(source) = source else {
                continue;
            };
            let frame = &clip.frames[source.1];
            let field = match built.take() {
                Some((done, field)) if done == source => field,
                _ if frame.state.phase >= 8 => frame.backdrop(),
                _ => frame.field(),
            };
            self.queue.write_texture(
                self.fields[texture].as_image_copy(),
                bytemuck::cast_slice(&field),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH as u32 * 8),
                    rows_per_image: Some(HEIGHT as u32),
                },
                SOURCE_EXTENT,
            );
            self.field_holds[texture] = Some(source);
            built = Some((source, field));
        }
    }

    /// Show original frame `index` in the comparison texture, its pixels
    /// made by `rgba` only when another frame is there.
    fn upload_reference(&mut self, index: usize, rgba: impl FnOnce() -> Vec<u8>) {
        if self.reference_index == Some(index) {
            return;
        }
        self.queue.write_texture(
            self.reference.as_image_copy(),
            &rgba(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH as u32 * 4),
                rows_per_image: Some(HEIGHT as u32),
            },
            SOURCE_EXTENT,
        );
        self.reference_index = Some(index);
    }

    /// Part 6: the shared controls for this moment, and the original frame
    /// when a comparison shows it, replayed on demand
    /// ([`RingsPreview::original`]) rather than stored.
    fn prepare_rings(
        &mut self,
        clip: &Clip,
        rings: &RingsPreview,
        j: usize,
        view: View,
    ) -> Uniforms {
        let time = view.time;
        self.part6 = true;
        self.rings_style = rings.style;
        if let (true, Some(renderer)) = (view.modern, &mut self.rings) {
            renderer.resize(&self.device, rings.style, view.size);
        }
        if view.comparison != 0 {
            self.upload_reference(rings.start + j, || rings.original_rgba(j));
        }
        let controls = rings.controls(time);
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART6_PHASE, j, time));
        if let Some(scroll) = &clip.scroller {
            let [level, bass, treble] = clip.audio_at(time).scroller;
            let musical = scroll.lighting == ScrollerLighting::Musical;
            u.scroll_light = [level, if musical { 1.0 } else { 0.0 }, bass, treble];
            // The bump wall as part 2 left it, for styles that use wall.wgsl:
            // the oscillator's last position, the tile image at full strength
            // (as during the text) and the wall's settings.
            let last = *scroll.oscillator.last().unwrap_or(&[0.0; 2]);
            u.backdrop = [
                last[0],
                last[1],
                scroll.backdrop_style as u32 as f32,
                scroll.wall_shape,
            ];
            u.backdrop_motion = [0.0, 0.0, scroll.wall_strength, 0.0];
            if let Some(image) = scroll.backdrop.get(scroll.end_tick.saturating_sub(1)) {
                u.next_palette = *image;
            }
        }
        u.waves = [0.0, (WALL_SHUTTER_TICKS / 70.0) as f32, 0.0, 0.0];
        u.rings = controls.rings;
        u.ring_centres = controls.ring_centres;
        u.ring_open = controls.ring_open;
        u.ring_bands = controls.ring_bands;
        u.ring_fades = controls.ring_fades;
        u.symbol = controls.symbol;
        u.symbol_layers = controls.symbol_layers;
        u.symbol_atlas = controls.symbol_atlas;
        u.authored_atlas = controls.authored_atlas;
        u.neon_map = controls.neon_map;
        u.rings_style = controls.rings_style;
        u
    }

    /// Make part 5's renderer, if there is none for this clip yet.
    fn hold_field(&mut self, field: &FieldPreview) {
        if self.field.is_none() {
            self.field = Some(FieldRenderer::new(
                &self.device,
                &self.queue,
                &self.uniform,
                &self.reference,
                self.output_format,
                field,
            ));
        }
    }

    /// Part 3: the shared controls for this moment, with the music's cues;
    /// during the hold and the zoom, part 5's view of its last frame at the
    /// zoom's magnification beneath it (its smoke brought to this moment as
    /// in part 5); and the original frame when a comparison shows it,
    /// replayed on demand ([`DotsPreview::original_rgba`]).
    fn prepare_dots(&mut self, clip: &Clip, dots: &DotsPreview, view: View) -> Uniforms {
        let View {
            time, size, modern, ..
        } = view;
        self.part3 = true;
        let j = dots.frame_at(time).unwrap_or(0);
        let aspect = size.0 as f64 / size.1.max(1) as f64;
        let block = dots_block(clip, dots, time, aspect);
        let phase = dots.frames[j].phase;
        let underlay = if modern {
            block.zoom[3] > 0.5 && phase <= super::dots::Phase::Shades
        } else {
            phase <= super::dots::Phase::Zoom
        };
        let mut field_block = FieldBlock::zeroed();
        self.part3_underlay = false;
        if let (true, Some(field)) = (underlay, &clip.field) {
            self.hold_field(field);
            let renderer = self.field.as_mut().unwrap();
            if modern {
                renderer.resize(&self.device, size);
            }
            renderer.update(&self.queue, field, field.frames.len() - 1);
            field_block = field.controls(time);
            if modern {
                field_block.fluid = renderer.advance_fluid(&self.device, &self.queue, field, time);
                renderer.advance_zoom(&self.device, &self.queue, &field_block, size);
            }
            let magnification = if modern { block.zoom[1] } else { block.zoom[0] };
            field_block.part3_view = [magnification, 0.0, 0.0, 0.0];
            self.part3_underlay = true;
        }
        let renderer = self.dots.as_mut().unwrap();
        renderer.resize(&self.device, size);
        renderer.set_water(&self.queue, &dots.ripples_at(time), &dots.under_at(time));
        if view.comparison != 0 {
            self.upload_reference(dots.start + j, || dots.original_rgba(j));
        }
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART3_PHASE, j, time));
        u.field = field_block;
        u.dots = block;
        u
    }

    /// Part 4: the shared controls for this moment, with the music's cues;
    /// in its first moments, part 3's controls too, for its modern view to
    /// hand over from; and the original frame when a comparison shows it,
    /// rebuilt on demand ([`PlasmaPreview::original_full`]).
    fn prepare_plasma(&mut self, clip: &Clip, plasma: &PlasmaPreview, view: View) -> Uniforms {
        let View {
            time, size, modern, ..
        } = view;
        self.part4 = true;
        let j = plasma.frame_at(time).unwrap_or(0);
        let mut block = plasma.controls(time);
        for (h, &since) in block.hits.iter().enumerate() {
            if since >= 0.0 {
                block.hit_strength[h] = hit_strength(clip, time - since as f64);
            }
        }
        let mut dots_controls = DotsBlock::zeroed();
        self.part4_handover = false;
        if modern {
            self.plasma.as_mut().unwrap().resize(&self.device, size);
            if let (true, Some(dots)) = (block.camera[2] > 0.0, &clip.dots) {
                let aspect = size.0 as f64 / size.1.max(1) as f64;
                dots_controls = dots_block(clip, dots, time, aspect);
                let renderer = self.dots.as_mut().unwrap();
                renderer.resize(&self.device, size);
                renderer.set_water(&self.queue, &dots.ripples_at(time), &dots.under_at(time));
                self.part4_handover = true;
            }
        } else {
            block.camera[2] = 0.0;
        }
        if view.comparison != 0 {
            self.plasma
                .as_mut()
                .unwrap()
                .show_original(&self.queue, plasma, j);
            self.upload_reference(plasma.start + j, || plasma.original_rgba(j));
        }
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART4_PHASE, j, time));
        u.dots = dots_controls;
        u.plasma = block;
        u
    }

    /// Part 7: the shared controls for this moment, and the original frame
    /// when a comparison shows it, rebuilt on demand
    /// ([`GreetingsPreview::original_rgba`]).
    fn prepare_greetings(
        &mut self,
        clip: &Clip,
        greetings: &GreetingsPreview,
        view: View,
    ) -> Uniforms {
        let time = view.time;
        self.part7 = true;
        let j = greetings.frame_at(time).unwrap_or(0);
        let renderer = self.greetings.as_mut().unwrap();
        renderer.shows_monster = greetings.frames[j].stage >= super::greetings::Stage::Monster;
        if view.modern {
            renderer.resize(&self.device, view.size);
        }
        if view.comparison != 0 {
            renderer.show_original(&self.queue, greetings, j);
            self.upload_reference(greetings.start + j, || greetings.original_rgba(j));
        }
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART7_PHASE, j, time));
        u.greetings = greetings.controls(time);
        self.greetings.as_mut().unwrap().traces_photons =
            greetings.photons_can_move_light(&u.greetings);
        u
    }

    /// Part 7's renderer, when the prepared moment is part 7's (tests).
    #[cfg(test)]
    pub(super) fn part7(&self) -> Option<&GreetingsRenderer> {
        self.greetings.as_ref().filter(|_| self.part7)
    }

    /// Part 8: the shared controls for this moment, and the original frame
    /// when a comparison shows it ([`EndingPreview::original_rgba`]).
    fn prepare_ending(&mut self, clip: &Clip, ending: &EndingPreview, view: View) -> Uniforms {
        let time = view.time;
        self.part8 = true;
        let j = ending.frame_at(time).unwrap_or(0);
        if view.comparison != 0 {
            self.ending
                .as_mut()
                .unwrap()
                .show_original(&self.queue, ending, j);
            self.upload_reference(ending.start + j, || ending.original_rgba(j));
        }
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART8_PHASE, j, time));
        u.ending = ending.controls(time);
        u
    }

    /// Part 5: the shared controls for this moment, its palette and zoom
    /// buffers, the fluid brought to this moment, and the original frame
    /// when a comparison shows it, replayed on demand
    /// ([`FieldPreview::original`]).
    fn prepare_field(&mut self, clip: &Clip, field: &FieldPreview, view: View) -> Uniforms {
        let View {
            time, size, modern, ..
        } = view;
        self.part5 = true;
        let j = field.frame_at(time).unwrap_or(0);
        let renderer = self.field.as_mut().unwrap();
        if modern {
            renderer.resize(&self.device, size);
        }
        renderer.update(&self.queue, field, j);
        if view.comparison != 0 {
            // Before part 5, when its view draws part 6's fade, the original
            // is part 6's frame.
            let before = clip
                .rings
                .as_ref()
                .filter(|_| field.frame_at(time).is_none())
                .and_then(|r| Some((r, r.frame_at(time)?)));
            let index = before.map_or(field.start + j, |(r, k)| r.start + k);
            self.upload_reference(index, || match before {
                Some((rings, k)) => rings.original_rgba(k),
                None => field.original_rgba(j),
            });
        }
        let mut block = field.controls(time);
        if modern {
            let renderer = self.field.as_mut().unwrap();
            block.fluid = renderer.advance_fluid(&self.device, &self.queue, field, time);
            renderer.advance_zoom(&self.device, &self.queue, &block, size);
        }
        let mut u = base_uniforms(clip, view, self.srgb, part_state(PART5_PHASE, j, time));
        u.field = block;
        u
    }

    /// Upload a scroller clip's glyph atlas and tables once, on first use.
    fn upload_glyphs(&mut self, scroll: &ScrollerPreview) {
        if self.glyphs_uploaded {
            return;
        }
        let glyphs = &scroll.glyphs;
        let upload = |texture: &wgpu::Texture, bytes: &[u8], (width, height): (u32, u32)| {
            self.queue.write_texture(
                texture.as_image_copy(),
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes.len() as u32 / height),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        };
        self.glyph_atlas = data_texture(
            &self.device,
            glyphs.atlas_size,
            wgpu::TextureFormat::Rg16Float,
        );
        self.scroll_data = data_texture(
            &self.device,
            glyphs.data_size,
            wgpu::TextureFormat::Rgba32Float,
        );
        self.glyph_field = field_texture(&self.device, GLYPH_FIELD_SIZE);
        upload(
            &self.glyph_atlas,
            bytemuck::cast_slice(&glyphs.atlas),
            glyphs.atlas_size,
        );
        upload(
            &self.scroll_data,
            bytemuck::cast_slice(&glyphs.data),
            glyphs.data_size,
        );
        self.bind_groups = bind_inputs(
            &self.device,
            &self.inputs,
            &self.uniform,
            &self.fields,
            [
                &self.reference,
                &self.glyph_atlas,
                &self.scroll_data,
                &self.glyph_field,
            ],
            &self.sampler,
        );
        self.field_bind_group = bind_field(
            &self.device,
            &self.field_inputs,
            &self.uniform,
            [&self.glyph_atlas, &self.scroll_data],
            &self.sampler,
        );
        self.glyphs_uploaded = true;
    }

    /// The scene inputs, with the shape fields in their current order.
    fn scene_inputs(&self) -> &wgpu::BindGroup {
        &self.bind_groups[usize::from(self.fields_swapped)]
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if let (true, Some(ending)) = (self.part8, &self.ending) {
            if self.modern_view {
                ending.draw_crt(encoder, self.modern_renderer.hdr_target());
                self.modern_renderer.post(encoder, view);
            } else {
                ending.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if let (true, Some(greetings)) = (self.part7, &self.greetings) {
            if self.modern_view {
                greetings.draw_style(encoder, self.modern_renderer.hdr_target());
                self.modern_renderer.post(encoder, view);
            } else {
                greetings.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if let (true, Some(plasma)) = (self.part4, &self.plasma) {
            if self.modern_view {
                if let (true, Some(dots)) = (self.part4_handover, &self.dots) {
                    dots.draw_style(encoder, plasma.handover());
                }
                plasma.draw_style(encoder, self.modern_renderer.hdr_target());
                self.modern_renderer.post(encoder, view);
            } else {
                plasma.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if let (true, Some(dots)) = (self.part3, &self.dots) {
            let field = self.field.as_ref().filter(|_| self.part3_underlay);
            if self.modern_view {
                if let Some(field) = field {
                    field.draw_style(encoder, dots.underlay_hdr());
                }
                dots.draw_style(encoder, self.modern_renderer.hdr_target());
                self.modern_renderer.post(encoder, view);
            } else {
                if let Some(field) = field {
                    field.draw_faithful(encoder, dots.underlay_out());
                }
                dots.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if let (true, Some(field)) = (self.part5, &self.field) {
            if self.modern_view {
                field.draw_style(encoder, self.modern_renderer.hdr_target());
                self.modern_renderer.post(encoder, view);
            } else {
                field.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if let (true, Some(rings)) = (self.part6, &self.rings) {
            if self.modern_view {
                rings.draw_style(encoder, self.modern_renderer.hdr_target(), self.rings_style);
                self.modern_renderer.post(encoder, view);
            } else {
                rings.draw_faithful(encoder, view);
            }
            self.draw_hud(encoder, view);
            return;
        }
        if self.glyphs_active {
            let field = self.glyph_field.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scroller distance fields"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &field,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, &self.field_bind_group, &[]);
            pass.set_pipeline(&self.glyph_field_pipeline);
            pass.draw(0..3, 0..1);
        }
        if self.modern_view {
            self.modern_renderer.draw(
                encoder,
                view,
                self.scene_inputs(),
                self.tile_transition,
                self.scroller,
            );
        } else {
            self.draw_faithful(encoder, view);
        }
        self.draw_hud(encoder, view);
    }

    fn draw_hud(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.hud_visible {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Frame-rate counter"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_bind_group(0, self.scene_inputs(), &[]);
            pass.set_pipeline(&self.hud);
            pass.draw(0..3, 0..1);
        }
    }

    fn draw_faithful(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("HD intro"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_bind_group(0, self.scene_inputs(), &[]);
        pass.set_pipeline(&self.background);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.stars);
        pass.draw(0..6, 0..512);
        pass.set_pipeline(&self.foreground);
        pass.draw(0..3, 0..1);
    }

    /// Part 5's renderer, once made; for its tests.
    #[cfg(test)]
    pub(super) fn field_renderer(&self) -> Option<&FieldRenderer> {
        self.field.as_ref()
    }

    /// Render one frame for offline capture, in tightly packed RGBA byte order.
    pub fn render_rgba(
        &mut self,
        clip: &Clip,
        view: View,
        format: wgpu::TextureFormat,
    ) -> Result<Vec<u8>, String> {
        let size = view.size;
        let limit = self.device.limits().max_texture_dimension_2d;
        if size.0 == 0 || size.1 == 0 || size.0 > limit || size.1 > limit {
            return Err(format!("Image dimensions must be 1..={limit}"));
        }
        self.prepare(clip, view);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HD capture"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let stride = (size.0 * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Capture readback"),
            size: stride as u64 * size.1 as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.draw(&mut encoder, &texture.create_view(&Default::default()));
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.1),
                },
            },
            wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mapped = buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity(size.0 as usize * size.1 as usize * 4);
        for row in mapped.chunks_exact(stride as usize) {
            pixels.extend_from_slice(&row[..size.0 as usize * 4]);
        }
        if matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for p in pixels.chunks_exact_mut(4) {
                p.swap(0, 2);
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }

    /// Render one frame and save it as a new PNG at `path`; a file already
    /// there is never replaced.
    pub fn capture(
        &mut self,
        clip: &Clip,
        view: View,
        path: &Path,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        let pixels = self.render_rgba(clip, view, format)?;
        write_png(path, view.size, &pixels).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Save tightly packed RGBA pixels as a new PNG, making its directory. A file
/// already at `path` is left as it is, and the error's kind is
/// `AlreadyExists`.
pub fn write_png(path: &Path, (width, height): (u32, u32), pixels: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::options().write(true).create_new(true).open(path)?;
    let mut png = png::Encoder::new(BufWriter::new(file), width, height);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    let mut writer = png.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    Ok(())
}

/// The controls every moment shares, before a part's own: the drawable size,
/// the clock and the comparison, `state` (phase, frame, door and the
/// interpolation between frames), the composition's aspect, the output's
/// encoding, the logo's time and the view, the intro's music cues and the
/// frame-rate counter.
fn base_uniforms(clip: &Clip, view: View, srgb: bool, state: [f32; 4]) -> Uniforms {
    let View {
        time,
        size,
        comparison,
        modern,
        hud,
    } = view;
    Uniforms {
        screen: [size.0 as f32, size.1 as f32, time as f32, comparison as f32],
        state,
        misc: [
            4.0 / 3.0,
            if srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ],
        audio: clip.audio_at(time).intro,
        hud: hud.map_or([0.0; 4], |[fps, ms]| [1.0, fps, ms, 0.0]),
        ..Uniforms::zeroed()
    }
}

/// A part's `u.state`: its phase, its frame `j` and the 70 Hz tick's fraction.
fn part_state(phase: f32, j: usize, time: f64) -> [f32; 4] {
    [phase, j as f32, 0.0, (time * 70.0).fract() as f32]
}

/// A shape field's key ([`super::clip::Frame::shape_key`]) and the frame it
/// was built from.
type FieldSource = ((u8, usize), usize);

/// For the shape fields each binding reads (`wanted`; the current shape,
/// then the next), given what each texture `holds`: whether texture 1 is
/// bound as the current shape, and what to build into each texture. The
/// order that holds more of them already wins, and on a tie the order bound
/// before; so when the next shape becomes the current one, the bindings swap
/// and only the new next shape is built.
fn plan_fields(
    wanted: [Option<FieldSource>; 2],
    holds: [Option<FieldSource>; 2],
    swapped: bool,
) -> (bool, [Option<FieldSource>; 2]) {
    let held =
        |texture: usize, slot: usize| wanted[slot].is_some() && holds[texture] == wanted[slot];
    let order = |current: usize| u8::from(held(current, 0)) + u8::from(held(current ^ 1, 1));
    let kept = usize::from(swapped);
    let swapped = swapped != (order(kept ^ 1) > order(kept));
    let current = usize::from(swapped);
    let mut builds = [None; 2];
    for (slot, texture) in [(0, current), (1, current ^ 1)] {
        if !held(texture, slot) {
            builds[texture] = wanted[slot];
        }
    }
    (swapped, builds)
}

/// A 320 × 200 source image.
const SOURCE_EXTENT: wgpu::Extent3d = wgpu::Extent3d {
    width: WIDTH as u32,
    height: HEIGHT as u32,
    depth_or_array_layers: 1,
};

/// Everything part 3 puts in `u.dots` at a clip time, with the music's
/// cues: the low band at the moment and each tick before, for the trails;
/// and each order's first row's strength.
fn dots_block(clip: &Clip, dots: &DotsPreview, time: f64, aspect: f64) -> DotsBlock {
    let mut block = dots.controls(time, aspect);
    // Part 4's water, as tuned, on part 4's clock (its slab frame is part
    // 3's last).
    let water = clip
        .plasma
        .as_ref()
        .map_or_else(Default::default, |p| p.water);
    block.water = water.controls(time - dots.seconds(dots.frames.len() - 1));
    block.drops = dots
        .water
        .drops(&water, &dots.events, time - dots.seconds(0));
    for k in 0..40 {
        block.low[k / 4][k % 4] = clip.audio_at(time - k as f64 / 70.0).intro[2];
    }
    for h in 0..4 {
        let since = block.hits[h] as f64;
        if since > -50.0 {
            block.hit_strength[h] = hit_strength(clip, time - since);
        }
    }
    block
}

/// An order's first row's strength (authored): a base, and more with the
/// onsets the PCM shows in the tenth of a second after it.
fn hit_strength(clip: &Clip, at: f64) -> f32 {
    let onset = (0..8)
        .map(|k| clip.audio_at(at + k as f64 / 70.0).intro[1])
        .fold(0.0_f32, f32::max);
    0.55 + 0.45 * onset
}

fn data_texture(
    device: &wgpu::Device,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Scroller glyphs and tables"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Texels of the per-frame scroller field texture; scroller.wgsl maps it
/// onto a fixed world-space extent.
const GLYPH_FIELD_SIZE: (u32, u32) = (2048, 1024);

/// Target for the per-frame scroller fields; 1 × 1 until a scroller clip is shown.
fn field_texture(device: &wgpu::Device, (width, height): (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Scroller distance fields"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Inputs for rendering the scroller fields: controls, sampler, atlas, tables.
fn bind_field(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    [atlas, data]: [&wgpu::Texture; 2],
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let (atlas, data) = (
        atlas.create_view(&Default::default()),
        data.create_view(&Default::default()),
    );
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Scroller field inputs"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&atlas),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&data),
            },
        ],
    })
}

/// Shared scene inputs: controls, the two shape fields as the current and
/// the next shape, the reference frame, a filtering sampler, the scroller
/// glyph atlas, its tables and the per-frame scroller fields; with field 0 as
/// the current shape, then field 1.
fn bind_inputs(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    [first, second]: &[wgpu::Texture; 2],
    [reference, atlas, data, glyph_field]: [&wgpu::Texture; 4],
    sampler: &wgpu::Sampler,
) -> [wgpu::BindGroup; 2] {
    [[first, second], [second, first]].map(|[current, next]| {
        let views = [current, next, reference, atlas, data, glyph_field]
            .map(|t| t.create_view(&Default::default()));
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ];
        for (view, binding) in views.iter().zip([1, 2, 3, 5, 6, 7]) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene inputs"),
            layout,
            entries: &entries,
        })
    })
}

/// The scroller tables are read texel by texel, never filtered.
fn table_entry() -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: 6,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Every shader file in this directory. Reading the directory means a
    /// new file is checked without being listed here.
    fn files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/hd");
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "wgsl"))
            .map(|path| {
                let body = std::fs::read_to_string(&path).unwrap();
                (path.file_name().unwrap().to_string_lossy().into(), body)
            })
            .collect();
        files.sort();
        files
    }

    /// Every module the renderers build, as they build it.
    fn modules() -> Vec<(String, String)> {
        let mut modules: Vec<(String, String)> = [
            ("intro", include_str!("intro.wgsl")),
            ("modern", include_str!("modern.wgsl")),
            ("post", include_str!("post.wgsl")),
            ("transition", include_str!("transition.wgsl")),
        ]
        .into_iter()
        .map(|(name, body)| (name.to_owned(), wgsl(body)))
        .collect();
        modules.push(("scroller".into(), scroller_source()));
        modules.push((
            "part 6 faithful HD".into(),
            super::super::rings_render::faithful_source(),
        ));
        for style in RingsStyle::ALL {
            modules.push((
                format!("part 6 style {}", style.name()),
                super::super::rings_render::style_source(style.info()),
            ));
        }
        modules.push((
            "part 5 faithful HD".into(),
            super::super::field_render::faithful_source(),
        ));
        modules.push((
            "part 5 modern".into(),
            super::super::field_render::style_source(),
        ));
        modules.push((
            "part 3 faithful HD".into(),
            super::super::dots_render::faithful_source(),
        ));
        modules.push((
            "part 3 modern".into(),
            super::super::dots_render::light_source(),
        ));
        modules.push((
            "part 4 faithful HD".into(),
            super::super::plasma_render::faithful_source(),
        ));
        modules.push((
            "part 4 modern".into(),
            super::super::plasma_render::light_source(),
        ));
        modules.push((
            "part 7 faithful HD".into(),
            super::super::greetings_render::faithful_source(),
        ));
        modules.push((
            "part 7 modern".into(),
            super::super::greetings_render::light_source(),
        ));
        modules.push((
            "part 7 modern monster".into(),
            super::super::greetings_render::monster_source(),
        ));
        modules.push((
            "part 8 faithful HD".into(),
            super::super::ending_render::faithful_source(),
        ));
        modules.push((
            "part 8 modern".into(),
            super::super::ending_render::crt_source(),
        ));
        modules
    }

    #[test]
    fn every_shader_validates_with_the_one_controls_declaration() {
        let files = files();
        let modules = modules();
        for (name, body) in &files {
            assert!(
                !body.contains("struct Controls") && !body.contains("@binding(0)"),
                "{name} must not declare its own controls; renderer::wgsl prepends them"
            );
            assert!(
                modules
                    .iter()
                    .any(|(_, source)| source.contains(body.as_str())),
                "{name} is in no module the renderers build"
            );
        }
        for expected in [
            "intro.wgsl",
            "modern.wgsl",
            "post.wgsl",
            "scroller.wgsl",
            "transition.wgsl",
            "wall.wgsl",
            "rings_common.wgsl",
            "rings_style.wgsl",
            "rings_faithful.wgsl",
            "rings_pool.wgsl",
            "field_common.wgsl",
            "field_style.wgsl",
            "field_faithful.wgsl",
            "field_frame.wgsl",
            "field_zoom.wgsl",
            "field_mountain.wgsl",
            "dots_common.wgsl",
            "dots_faithful.wgsl",
            "dots_light.wgsl",
            "plasma_common.wgsl",
            "plasma_faithful.wgsl",
            "plasma_light.wgsl",
            "greetings_common.wgsl",
            "greetings_faithful.wgsl",
            "greetings_light.wgsl",
            "greetings_monster.wgsl",
            "ending_common.wgsl",
            "ending_faithful.wgsl",
            "ending_crt.wgsl",
        ] {
            assert!(
                files.iter().any(|f| f.0 == expected),
                "{expected} not found"
            );
        }
        let expected: Vec<(&str, usize)> = rust_fields();
        for (name, source) in &modules {
            let module = naga::front::wgsl::parse_str(source)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            let controls = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some("Controls"))
                .expect("Controls struct");
            let naga::TypeInner::Struct { members, span } = &controls.1.inner else {
                panic!("Controls is not a struct");
            };
            assert_eq!(*span as usize, size_of::<Uniforms>(), "{name}: size");
            let layout: Vec<_> = members
                .iter()
                .map(|m| (m.name.as_deref().unwrap(), m.offset as usize))
                .collect();
            assert_eq!(layout, expected, "{name}: field names and offsets");
        }
    }

    /// The constants the part-6 shaders repeat from hd/rings.rs and clip.rs.
    #[test]
    fn part6_shaders_repeat_the_rust_constants() {
        use super::super::rings::{
            NEON_RADIUS, NEON_STROKE, ORIGINAL_SYMBOLS, RING_WINDOW, SYMBOL_LIMIT, SYMBOL_SCALE,
            TUBE_LIGHT_SCALE,
        };
        let common = include_str!("rings_common.wgsl");
        for constant in [
            format!("const SYMBOL_SCALE={SYMBOL_SCALE}.0;"),
            format!("const SYMBOL_LIMIT={SYMBOL_LIMIT:.1};"),
            format!("const NEON_RADIUS={NEON_RADIUS:?};"),
            format!("const ORIGINAL_SYMBOLS={ORIGINAL_SYMBOLS};"),
            format!("const NEON_STROKE={NEON_STROKE:?};"),
            format!(
                "const TUBE_LIGHT_LOD={:.1};",
                (TUBE_LIGHT_SCALE as f32).log2()
            ),
            format!(
                "const RING_WINDOW=vec4f({},{},{},{});",
                RING_WINDOW[0], RING_WINDOW[1], RING_WINDOW[2], RING_WINDOW[3]
            ),
            "const COLUMN=5.0/6.0;".to_owned(),
        ] {
            assert!(
                common.contains(&constant),
                "rings_common.wgsl lacks {constant}"
            );
        }
        assert_eq!(super::super::clip::PIXEL_ASPECT, 5.0 / 6.0);
        // The original's mean symbol blue in linear light.
        let mean = (0..32)
            .map(|b| super::super::clip::vga_linear(b as u8))
            .sum::<f32>()
            / 32.0;
        assert!(
            common.contains(
                &format!("const SYMBOL_YELLOW=vec3f(1,1,{:.4});", mean).replace("0.", ".")
            )
        );
    }

    /// Parse and validate one module.
    fn validated(name: &str, source: &str) -> (naga::Module, naga::valid::ModuleInfo) {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        (module, info)
    }

    /// Member names and offsets of a struct type in a module.
    fn wgsl_members(module: &naga::Module, name: &str) -> (Vec<(String, usize)>, usize) {
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no {name}"));
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            panic!("{name} is not a struct");
        };
        let members = members
            .iter()
            .map(|m| (m.name.clone().unwrap(), m.offset as usize))
            .collect();
        (members, *span as usize)
    }

    #[test]
    fn part5_block_matches_its_wgsl_declaration() {
        macro_rules! fields {
            ($($field:ident),* $(,)?) => {
                vec![$((stringify!($field).to_owned(), offset_of!(FieldBlock, $field))),*]
            };
        }
        let expected = fields![
            part,
            zoom,
            zoom_state,
            camera,
            view,
            mountain,
            window,
            mandel,
            mandel_fit,
            relief,
            events,
            events_late,
            fluid,
            fluid_region,
            frame_style,
            zoom_style,
            mountain_style,
            part3_view,
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[f32; 4]>(),
            size_of::<FieldBlock>()
        );
        let (module, _) = validated("part 5 modern", &super::super::field_render::style_source());
        let (members, span) = wgsl_members(&module, "FieldBlock");
        assert_eq!(members, expected);
        assert_eq!(span, size_of::<FieldBlock>());
    }

    /// The constants the part-5 shaders repeat from hd/field.rs and clip.rs.
    #[test]
    fn part5_shaders_repeat_the_rust_constants() {
        use super::super::field::{
            BAILOUT, CLEAR_INDEX, COARSE_CELL, FIELD_WINDOW, MAP_SIZE, POTENTIAL_ITERATIONS,
            SET_ITERATIONS,
        };
        let common = include_str!("field_common.wgsl");
        let [x0, y0, x1, y1] = FIELD_WINDOW;
        for constant in [
            format!("const FIELD_WINDOW=vec4f({x0},{y0},{x1},{y1});"),
            format!("const CLEAR_INDEX={CLEAR_INDEX};"),
            format!("const MAP_SIZE={MAP_SIZE}.0;"),
            format!("const COARSE_CELL={COARSE_CELL}.0;"),
            format!("const SET_ITERATIONS={SET_ITERATIONS};"),
            format!("const POTENTIAL_ITERATIONS={POTENTIAL_ITERATIONS};"),
            format!("const BAILOUT={BAILOUT:e};"),
            "const COLUMN=5.0/6.0;".to_owned(),
        ] {
            assert!(
                common.contains(&constant),
                "field_common.wgsl lacks {constant}"
            );
        }
    }

    /// Each part-5 entry point uses only the bindings its pipeline layout
    /// gives it (field_render.rs): faithful HD group 0; the modern view and
    /// the prepasses group 0, the fluid's latest and previous states and the
    /// zoom's group 2 (its shown level and view); the fluid step group 0, the
    /// state before it, its controls and its output.
    /// An author's helper that reaches for the wrong one fails here, not on
    /// the GPU.
    #[test]
    fn part5_entry_points_use_only_their_bindings() {
        let drawn = [1, 2];
        let step = [1, 3, 4];
        for (name, source) in [
            (
                "part 5 faithful HD",
                super::super::field_render::faithful_source(),
            ),
            ("part 5 modern", super::super::field_render::style_source()),
        ] {
            let (module, info) = validated(name, &source);
            for (index, entry) in module.entry_points.iter().enumerate() {
                let allowed: &[u32] = match entry.name.as_str() {
                    "fullscreen" | "faithful" => &[],
                    "field_fluid" => &step,
                    n if n.starts_with("field_") => &drawn,
                    n => panic!("unexpected entry point {n}"),
                };
                let uses = info.get_entry_point(index);
                for (handle, global) in module.global_variables.iter() {
                    let (Some(binding), false) = (&global.binding, uses[handle].is_empty()) else {
                        continue;
                    };
                    let zoom = binding.group == 2 && allowed == drawn;
                    assert!(
                        binding.group == 0
                            || zoom
                            || (binding.group == 1 && allowed.contains(&binding.binding)),
                        "{name}: {} uses {:?} (group {}, binding {})",
                        entry.name,
                        global.name,
                        binding.group,
                        binding.binding
                    );
                }
            }
        }
    }

    #[test]
    fn part3_blocks_match_their_wgsl_declarations() {
        use super::super::dots::DotFrame;
        macro_rules! fields {
            ($ty:ty; $($field:ident),* $(,)?) => {
                vec![$((stringify!($field).to_owned(), offset_of!($ty, $field))),*]
            };
        }
        let expected = fields![DotsBlock;
            part,
            zoom,
            shades,
            dots,
            focus,
            end,
            events,
            events_late,
            hits,
            hit_strength,
            low,
            echo_levels,
            water,
            water_plane,
            drops,
            water_lights,
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[f32; 4]>(),
            size_of::<DotsBlock>()
        );
        for (name, source) in [
            ("part 3 modern", super::super::dots_render::light_source()),
            (
                "part 3 faithful HD",
                super::super::dots_render::faithful_source(),
            ),
        ] {
            let (module, _) = validated(name, &source);
            let (members, span) = wgsl_members(&module, "DotsBlock");
            assert_eq!(members, expected);
            assert_eq!(span, size_of::<DotsBlock>());
            let (members, span) = wgsl_members(&module, "DotFrame");
            assert_eq!(members, fields![DotFrame; angles, translation]);
            assert_eq!(span, size_of::<DotFrame>());
        }
    }

    /// The constants the part-3 shaders repeat from hd/dots.rs,
    /// dots_render.rs and the scene.
    #[test]
    fn part3_shaders_repeat_the_rust_constants() {
        use super::super::dots::{ECHO_SPACING, ECHOES, POINTS, SHADE_DISCS, SHADE_KEPT};
        use super::super::dots_render::{
            AFTER_SAMPLES, FOG_GRID, NOISE_SIZE, SHADE_SAMPLES, TRAIL_SAMPLES,
        };
        let common = include_str!("dots_common.wgsl");
        let order = crate::scenes::dots::ORDER
            .map(|o| format!("{o}u"))
            .join(",");
        for constant in [
            format!("const POINTS={POINTS}u;"),
            format!("const SHADE_DISCS={SHADE_DISCS}u;"),
            format!("const SHADE_KEPT={SHADE_KEPT}.0;"),
            format!("const ECHOES={ECHOES}u;"),
            format!("const ECHO_SPACING={ECHO_SPACING}.0;"),
            format!("const ORDER=array<u32,8>({order});"),
        ] {
            assert!(
                common.contains(&constant),
                "dots_common.wgsl lacks {constant}"
            );
        }
        let light = include_str!("dots_light.wgsl");
        for constant in [
            format!("const TRAIL_SAMPLES={TRAIL_SAMPLES}u;"),
            format!("const AFTER_SAMPLES={AFTER_SAMPLES}u;"),
            format!("const SHADE_SAMPLES={SHADE_SAMPLES}u;"),
            format!("const FOG_COLUMNS={}u;", FOG_GRID[0]),
            format!("const FOG_ROWS={}u;", FOG_GRID[1]),
            format!("const FOG_SLICES={}u;", FOG_GRID[2]),
            format!("const NOISE_SIZE={NOISE_SIZE}u;"),
        ] {
            assert!(
                light.contains(&constant),
                "dots_light.wgsl lacks {constant}"
            );
        }
        assert_eq!(super::super::clip::PIXEL_ASPECT, 5.0 / 6.0);
        assert!(common.contains("const COLUMN=5.0/6.0;"));
        // The diaphragm's blade normals follow its turn.
        let number = |s: &str| s.parse::<f64>().unwrap();
        let turn = light.split("const BLADE_TURN=").nth(1).unwrap();
        let turn = number(&turn[..turn.find(';').unwrap()]);
        let normals = light
            .split("const BLADE_NORMALS=array<vec2f,7>(")
            .nth(1)
            .unwrap();
        let normals: Vec<f64> = normals[..normals.find(");").unwrap()]
            .split("vec2f(")
            .skip(1)
            .flat_map(|v| {
                let v = &v[..v.find(')').unwrap()];
                v.split(',').map(number).collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(normals.len(), 14);
        for k in 0..7 {
            let a = turn + k as f64 * std::f64::consts::TAU / 7.0;
            assert!((normals[2 * k] - a.cos()).abs() < 1e-6, "blade {k}");
            assert!((normals[2 * k + 1] - a.sin()).abs() < 1e-6, "blade {k}");
        }
    }

    /// Each part-3 entry point uses only the bindings its pipeline has
    /// (dots_render.rs): group 0 everywhere; group 1 binding 1 (the samples,
    /// written) for the samples step, binding 2 (read) for the painting,
    /// the haze and the composite, and in faithful HD bindings 1 to 3 (its
    /// images) for its composite; group 2 (the images) for the modern
    /// composite.
    #[test]
    fn part3_entry_points_use_only_their_bindings() {
        for (name, source) in [
            (
                "part 3 faithful HD",
                super::super::dots_render::faithful_source(),
            ),
            ("part 3 modern", super::super::dots_render::light_source()),
        ] {
            let (module, info) = validated(name, &source);
            for (index, entry) in module.entry_points.iter().enumerate() {
                let allowed: &[(u32, u32)] = match entry.name.as_str() {
                    "fullscreen"
                    | "faithful_shade"
                    | "faithful_dot"
                    | "faithful_shade_mark"
                    | "faithful_dot_mark" => &[],
                    "faithful" => &[(1, 1), (1, 2), (1, 3)],
                    "dots_samples" => &[(1, 1)],
                    "dots_fog_light" => &[(1, 2), (1, 6)],
                    "dots_fog_noise" => &[(1, 7)],
                    "dots_trail"
                    | "shade_trail"
                    | "dots_trail_mirrored"
                    | "shade_trail_mirrored"
                    | "swept" => &[(1, 2)],
                    "dots_haze" => &[(1, 2), (1, 3), (1, 4), (1, 5)],
                    "dots_water_field" => &[],
                    "dots_style" => &[(1, 2), (2, 1), (2, 2), (2, 3), (2, 4), (2, 5)],
                    n => panic!("unexpected entry point {n}"),
                };
                let uses = info.get_entry_point(index);
                for (handle, global) in module.global_variables.iter() {
                    let (Some(binding), false) = (&global.binding, uses[handle].is_empty()) else {
                        continue;
                    };
                    assert!(
                        binding.group == 0 || allowed.contains(&(binding.group, binding.binding)),
                        "{name}: {} uses {:?} (group {}, binding {})",
                        entry.name,
                        global.name,
                        binding.group,
                        binding.binding
                    );
                }
            }
        }
    }

    /// Part 3 is a function of the moment: a frame drawn after others is
    /// the same, byte for byte, as one drawn by a new renderer, in both
    /// views (after the hold and the zoom, which draw part 5's smoke, whose
    /// own test covers it).
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part3_frames_are_the_same_however_reached() {
        let data = crate::test_support::data();
        let clip = Clip::load_dots(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let view = |time, modern| View {
            time,
            size: (480, 270),
            comparison: 0,
            modern,
            hud: None,
        };
        let moments = [136.5, 141.8, 145.7, 151.43, 176.3, 179.6, 180.2];
        for modern in [true, false] {
            let (mut playing, format) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
            let mut played = Vec::new();
            for &t in &moments {
                // Draw a few frames before each, as playback would.
                for k in (1..4).rev() {
                    playing
                        .render_rgba(&clip, view(t - k as f64 / 60.0, modern), format)
                        .unwrap();
                }
                played.push(playing.render_rgba(&clip, view(t, modern), format).unwrap());
            }
            for (&t, played) in moments.iter().zip(&played).rev() {
                let (mut fresh, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
                let pixels = fresh.render_rgba(&clip, view(t, modern), format).unwrap();
                assert!(
                    &pixels == played,
                    "{t} s ({})",
                    if modern { "modern" } else { "faithful" }
                );
                assert!(pixels.iter().any(|&p| p > 8), "{t} s is black");
            }
        }
    }

    #[test]
    fn part4_block_matches_its_wgsl_declaration() {
        macro_rules! fields {
            ($($field:ident),* $(,)?) => {
                vec![$((stringify!($field).to_owned(), offset_of!(PlasmaBlock, $field))),*]
            };
        }
        let expected = fields![
            part,
            frame,
            palette,
            logo,
            events,
            events_late,
            camera,
            slab_origin,
            slab_axes,
            slab_size,
            stone,
            sheet,
            sheet_size,
            hits,
            hit_strength,
            motion,
            sheet_back,
            sheet_back_size,
            water,
            drops,
            ground,
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[f32; 4]>(),
            size_of::<PlasmaBlock>()
        );
        for (name, source) in [
            ("part 4 modern", super::super::plasma_render::light_source()),
            (
                "part 4 faithful HD",
                super::super::plasma_render::faithful_source(),
            ),
        ] {
            let (module, _) = validated(name, &source);
            let (members, span) = wgsl_members(&module, "PlasmaBlock");
            assert_eq!(members, expected, "{name}");
            assert_eq!(span, size_of::<PlasmaBlock>(), "{name}");
        }
    }

    /// The constants the part-4 shaders repeat from hd/plasma.rs,
    /// plasma_render.rs and the scene.
    #[test]
    fn part4_shaders_repeat_the_rust_constants() {
        use super::super::plasma::{
            FIELD_ROW, FIELD_ROWS, FIELD_START, LOGO_ROWS, PALETTE_PERIOD, PLASMA_RASTERS,
        };
        use super::super::plasma_render::{SHEET_LEVELS, SHEET_SIZE};
        let common = include_str!("plasma_common.wgsl");
        for constant in [
            format!("const FIELD_START={FIELD_START}.0;"),
            format!("const FIELD_ROW={FIELD_ROW}.0;"),
            format!("const FIELD_ROWS={FIELD_ROWS}.0;"),
            format!("const PLASMA_RASTERS={PLASMA_RASTERS}u;"),
            format!("const PALETTE_PERIOD={PALETTE_PERIOD}.0;"),
            format!("const LOGO_ROWS={LOGO_ROWS}.0;"),
            "const COLUMN=5.0/6.0;".to_owned(),
        ] {
            assert!(
                common.contains(&constant),
                "plasma_common.wgsl lacks {constant}"
            );
        }
        let light = include_str!("plasma_light.wgsl");
        assert!(light.contains(&format!(
            "const LAYER_SAMPLES={}u;",
            super::super::plasma::LAYER_SAMPLES
        )));
        let (w, h) = SHEET_SIZE;
        assert!(light.contains(&format!("const SHEET_SIZE=vec2f({w}.0,{h}.0);")));
        // The light's cells and the haze's are one texel of their levels.
        for (cells, level) in [("CELLS", "CELL_LEVEL"), ("HAZE_CELLS", "HAZE_LEVEL")] {
            let read = |name: &str| {
                let at = light.split(&format!("const {name}=")).nth(1).unwrap();
                at[..at.find(';').unwrap()].to_owned()
            };
            let level: u32 = read(level).trim_end_matches(".0").parse().unwrap();
            assert!(level < SHEET_LEVELS);
            assert_eq!(
                read(cells),
                format!("vec2u({}u,{}u)", w >> level, h >> level),
                "{cells}"
            );
        }
        // The field starts where the logo (320 x 180, a quarter of it in
        // each plane) ends.
        assert_eq!(FIELD_START, 320 * LOGO_ROWS / 4);
    }

    /// Each part-4 entry point uses only the bindings its pipeline has
    /// (plasma_render.rs): group 0 everywhere; group 1 binding 3 (a level
    /// of the sheet) for the levels, bindings 1 and 2 (the sheet and part
    /// 3's view) for the modern composite.
    #[test]
    fn part4_entry_points_use_only_their_bindings() {
        for (name, source) in [
            (
                "part 4 faithful HD",
                super::super::plasma_render::faithful_source(),
            ),
            ("part 4 modern", super::super::plasma_render::light_source()),
        ] {
            let (module, info) = validated(name, &source);
            for (index, entry) in module.entry_points.iter().enumerate() {
                let allowed: &[(u32, u32)] = match entry.name.as_str() {
                    "fullscreen" | "faithful" | "plasma_sheet" => &[],
                    "plasma_downsample" => &[(1, 3)],
                    "plasma_style" => &[(1, 1), (1, 2)],
                    n => panic!("unexpected entry point {n}"),
                };
                let uses = info.get_entry_point(index);
                for (handle, global) in module.global_variables.iter() {
                    let (Some(binding), false) = (&global.binding, uses[handle].is_empty()) else {
                        continue;
                    };
                    assert!(
                        binding.group == 0 || allowed.contains(&(binding.group, binding.binding)),
                        "{name}: {} uses {:?} (group {}, binding {})",
                        entry.name,
                        global.name,
                        binding.group,
                        binding.binding
                    );
                }
            }
        }
    }

    /// Part 4 is a function of the moment: a frame drawn after others is
    /// the same, byte for byte, as one drawn by a new renderer, in both
    /// views, the handover from part 3 included.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part4_frames_are_the_same_however_reached() {
        let data = crate::test_support::data();
        let clip = Clip::load_plasma(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let view = |time, modern| View {
            time,
            size: (480, 270),
            comparison: 0,
            modern,
            hud: None,
        };
        let slab = clip.plasma.as_ref().unwrap().seconds(0);
        let moments = [slab + 0.1, 180.6, 184.0, 189.0, 205.0, 221.0, 224.8];
        for modern in [true, false] {
            let (mut playing, format) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
            let mut played = Vec::new();
            for &t in &moments {
                for k in (1..4).rev() {
                    playing
                        .render_rgba(&clip, view(t - k as f64 / 60.0, modern), format)
                        .unwrap();
                }
                played.push(playing.render_rgba(&clip, view(t, modern), format).unwrap());
            }
            for (&t, played) in moments.iter().zip(&played).rev() {
                let (mut fresh, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
                let pixels = fresh.render_rgba(&clip, view(t, modern), format).unwrap();
                assert!(
                    &pixels == played,
                    "{t} s ({})",
                    if modern { "modern" } else { "faithful" }
                );
                assert!(pixels.iter().any(|&p| p > 8), "{t} s is black");
            }
        }
    }

    #[test]
    fn part7_block_matches_its_wgsl_declaration() {
        macro_rules! fields {
            ($($field:ident),* $(,)?) => {
                vec![$((stringify!($field).to_owned(), offset_of!(GreetingsBlock, $field))),*]
            };
        }
        let expected = fields![
            part,
            frame,
            sweep,
            twist,
            twist_late,
            events,
            events_late,
            beam,
            beam_late
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[f32; 4]>(),
            size_of::<GreetingsBlock>()
        );
        for (name, source) in [
            (
                "part 7 modern",
                super::super::greetings_render::light_source(),
            ),
            (
                "part 7 faithful HD",
                super::super::greetings_render::faithful_source(),
            ),
        ] {
            let (module, _) = validated(name, &source);
            let (members, span) = wgsl_members(&module, "GreetingsBlock");
            assert_eq!(members, expected, "{name}");
            assert_eq!(span, size_of::<GreetingsBlock>(), "{name}");
        }
    }

    /// Each part-7 entry point uses only the bindings its pipeline has
    /// (greetings_render.rs): group 0 everywhere; in group 1, the photons
    /// written by their trace, read with the splats' counts, list and draw
    /// as the splats are listed, and with the list by the caustics, the
    /// blocks' geometry and the strips' transmission for the dust and the
    /// air, and the dust, the geometry and the air for the picture.
    #[test]
    fn part7_entry_points_use_only_their_bindings() {
        for (name, source) in [
            (
                "part 7 faithful HD",
                super::super::greetings_render::faithful_source(),
            ),
            (
                "part 7 modern",
                super::super::greetings_render::light_source(),
            ),
        ] {
            let (module, info) = validated(name, &source);
            for (index, entry) in module.entry_points.iter().enumerate() {
                let allowed: &[(u32, u32)] = match entry.name.as_str() {
                    "fullscreen" | "faithful" | "blocks" => &[],
                    "trace_photons" => &[(1, 7)],
                    "count_splats" | "list_splats" => &[(1, 8), (1, 9), (1, 10), (1, 11)],
                    "photon_vertex" | "photon_fragment" => &[(1, 8), (1, 12)],
                    "dust_vertex" | "dust_fragment" | "air" => &[(1, 2), (1, 3)],
                    "style" => &[(1, 1), (1, 2), (1, 4), (1, 5), (1, 6)],
                    n => panic!("unexpected entry point {n}"),
                };
                let uses = info.get_entry_point(index);
                for (handle, global) in module.global_variables.iter() {
                    let (Some(binding), false) = (&global.binding, uses[handle].is_empty()) else {
                        continue;
                    };
                    assert!(
                        binding.group == 0 || allowed.contains(&(binding.group, binding.binding)),
                        "{name}: {} uses {:?} (group {}, binding {})",
                        entry.name,
                        global.name,
                        binding.group,
                        binding.binding
                    );
                }
            }
        }
    }

    /// Part 7 is a function of the moment: a frame drawn after others is
    /// the same, byte for byte, as one drawn by a new renderer, in both
    /// views.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part7_frames_are_the_same_however_reached() {
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let view = |time, modern| View {
            time,
            size: (480, 270),
            comparison: 0,
            modern,
            hud: None,
        };
        let moments = [226.5, 231.0, 233.1, 238.6, 241.25, 241.9, 242.29];
        for modern in [true, false] {
            let (mut playing, format) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
            let mut played = Vec::new();
            for &t in &moments {
                for k in (1..4).rev() {
                    playing
                        .render_rgba(&clip, view(t - k as f64 / 60.0, modern), format)
                        .unwrap();
                }
                played.push(playing.render_rgba(&clip, view(t, modern), format).unwrap());
            }
            for (&t, played) in moments.iter().zip(&played).rev() {
                let (mut fresh, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
                let pixels = fresh.render_rgba(&clip, view(t, modern), format).unwrap();
                assert!(
                    &pixels == played,
                    "{t} s ({})",
                    if modern { "modern" } else { "faithful" }
                );
                assert!(pixels.iter().any(|&p| p > 8), "{t} s is black");
            }
        }
    }

    /// The fluid hook reaches the same state whether the clip plays on,
    /// seeks back to a kept state or starts afresh at a moment, as video
    /// export does: each moment's state matches a new renderer's byte for
    /// byte (by a hash of the whole state), and is the state the moment
    /// needs, one step past it.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part5_fluid_is_the_same_however_it_is_reached() {
        use super::super::field::{FLUID, FluidStart};
        let data = crate::test_support::data();
        let clip = Clip::load_field(&data, ScrollerLighting::Studio).unwrap();
        let field = clip.field.as_ref().unwrap();
        let start = match FLUID.start {
            FluidStart::Part5 => field.seconds(0),
            FluidStart::PictureFade => field.picture_fade.unwrap(),
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || {
            pollster::block_on(Renderer::new(&instance, None))
                .unwrap()
                .0
        };
        let read = |r: &mut Renderer, time: f64| {
            r.prepare(
                &clip,
                View {
                    time,
                    size: (320, 200),
                    comparison: 0,
                    modern: true,
                    hud: None,
                },
            );
            r.field.as_ref().unwrap().read_fluid(&r.device, &r.queue)
        };
        let mut playing = renderer();
        let end = field.seconds(field.frames.len());
        for offset in [0.5, 5.0, 30.0, 3.0, 4.0, 47.0, 20.0, 0.01, 47.5] {
            let time = (start + offset).min(end);
            let state = read(&mut playing, time);
            assert!(state == read(&mut renderer(), time), "fluid at {time}");
            let steps = ((time - start) * FLUID.rate).floor() as usize + 1;
            assert_eq!(state.1, steps, "steps at {time}");
        }
    }

    /// The fluid's pre-roll runs a few steps a frame in the frames before
    /// its start, so that the start's frame runs one step, and makes the
    /// same states as when it all runs at the start: whether played at 60
    /// frames a second through part 6, caught up after seeks into it and
    /// back before it, or finished at the start, the state past the start
    /// matches a new renderer's byte for byte (by a hash of the whole
    /// state), as does the frame drawn; and seeking back before the start
    /// keeps a finished pre-roll.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part5_fluid_prerolls_ahead_of_its_start_the_same() {
        use super::super::field::{FLUID, FluidStart};
        let data = crate::test_support::data();
        let clip = Clip::load_field(&data, ScrollerLighting::Studio).unwrap();
        let field = clip.field.as_ref().unwrap();
        let start = match FLUID.start {
            FluidStart::Part5 => field.seconds(0),
            FluidStart::PictureFade => field.picture_fade.unwrap(),
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || pollster::block_on(Renderer::new(&instance, None)).unwrap();
        let view = |time| View {
            time,
            size: (320, 200),
            comparison: 0,
            modern: true,
            hud: None,
        };
        let read = |r: &mut Renderer, time: f64| {
            r.prepare(&clip, view(time));
            r.field.as_ref().unwrap().read_fluid(&r.device, &r.queue)
        };
        let prerolled = |r: &Renderer| r.field.as_ref().unwrap().fluid_prerolled();
        let play = |r: &mut Renderer, from: f64, to: f64| {
            let mut k = 0;
            while from + k as f64 / 60.0 < to {
                r.prepare(&clip, view(from + k as f64 / 60.0));
                k += 1;
            }
        };
        // Played through part 6 from before the pre-roll begins: it has
        // finished by the start, which runs one step.
        let (mut playing, format) = renderer();
        play(&mut playing, start - 35.0, start - 34.0);
        assert_eq!(prerolled(&playing).0, 0, "no pre-roll 35 s ahead");
        play(&mut playing, start - 34.0, start);
        let (done, length) = prerolled(&playing);
        assert_eq!(done, length, "pre-roll finished ahead of the start");
        let first = read(&mut playing, start);
        assert!(
            first == read(&mut renderer().0, start),
            "fluid at the start"
        );
        assert_eq!(first.1, 1);
        let time = start + 0.5;
        let pixels = playing.render_rgba(&clip, view(time), format).unwrap();
        let fresh = renderer().0.render_rgba(&clip, view(time), format).unwrap();
        assert!(pixels == fresh, "frame at {time}");
        // Seeking back before the start keeps the finished pre-roll.
        play(&mut playing, start - 30.0, start - 29.0);
        assert_eq!(prerolled(&playing), (length, length));
        let time = start + 4.0;
        assert!(
            read(&mut playing, time) == read(&mut renderer().0, time),
            "fluid at {time}"
        );
        // Seeks into the pre-roll, on and back before it, then into part 5
        // before it has finished: it carries on from the steps it has run.
        let (mut seeking, _) = renderer();
        play(&mut seeking, start - 20.0, start - 19.0);
        let partial = prerolled(&seeking).0;
        assert!(partial > 0 && partial < length, "{partial} steps");
        play(&mut seeking, start - 40.0, start - 39.0);
        assert_eq!(prerolled(&seeking).0, partial);
        play(&mut seeking, start - 1.0, start - 0.9);
        assert!(prerolled(&seeking).0 > partial);
        let time = start + 3.0;
        assert!(
            read(&mut seeking, time) == read(&mut renderer().0, time),
            "fluid at {time}"
        );
        assert_eq!(prerolled(&seeking), (length, length));
    }

    /// The scroller skips what the laser cut's flat plates hide by the cut's
    /// grid, while the plates are drawn, and only before the plates tip:
    /// their tile age reaches 7 × LASER_CUT at the end of the cut.
    #[test]
    fn scroller_mask_follows_the_laser_cut() {
        let (cut, scroller) = (
            include_str!("transition.wgsl"),
            include_str!("scroller.wgsl"),
        );
        let constant = |source: &str, name: &str| {
            let start = source.find(&format!("\nconst {name}=")).unwrap() + name.len() + 8;
            source[start..].split(';').next().unwrap().to_owned()
        };
        assert_eq!(constant(cut, "GRID"), constant(scroller, "CUT_GRID"));
        let flat: f32 = constant(scroller, "CUT_FLAT_AGE").parse().unwrap();
        assert!(flat < 7.0 * constant(cut, "LASER_CUT").parse::<f32>().unwrap());
        assert!(cut.contains("fn tile_lifetime()->f32 {return LASER_LIFE*7.0;}"));
        assert!(cut.contains("let age=(u.transition.x-order(id))/10.0;"));
        assert!(scroller.contains("u.transition.x>=u.transition.z+78.0"));
    }

    /// When the next shape becomes the current one, the two shape fields'
    /// bindings swap and only the new next shape is built; anything else is
    /// built from the frame a texture per binding would hold.
    #[test]
    fn shape_fields_swap_rather_than_build_again() {
        // Shape keys and the frames they were built from; `later` is `b`
        // reached a tick later, as when a tick is skipped.
        let [a, b, c, later] =
            [(10, 20), (11, 22), (12, 24), (11, 23)].map(|(k, i)| Some(((1, k), i)));
        for (wanted, held, swapped, plan) in [
            ([a, b], [None; 2], false, (false, [a, b])),
            ([b, c], [a, b], false, (true, [c, None])),
            ([c, a], [c, b], true, (false, [None, a])),
            ([later, c], [a, b], false, (false, [later, c])),
            // Held already, or not read: nothing is built.
            ([b, c], [c, b], true, (true, [None; 2])),
            ([b, None], [a, b], false, (true, [None; 2])),
            ([None; 2], [a, b], true, (true, [None; 2])),
            // No other shape coming: both bindings read the shown one.
            ([c, c], [c, b], true, (true, [None, c])),
        ] {
            let planned = plan_fields(wanted, held, swapped);
            assert_eq!(planned, plan, "{wanted:?} {held:?}");
        }
    }

    /// Every part's controls carry the shared values, the frame-rate
    /// counter's among them, not only the intro's and the scroller's.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn every_part_carries_the_shared_controls() {
        let data = crate::test_support::data();
        let clip = Clip::load_ending(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (mut renderer, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
        for (time, phase) in [
            (6.8, 1.0),
            (31.0, 8.0),
            (57.2, PART6_PHASE),
            (84.0, PART5_PHASE),
            (151.4, PART3_PHASE),
            (197.0, PART4_PHASE),
            (233.1, PART7_PHASE),
            (245.5, PART8_PHASE),
        ] {
            for (modern, comparison) in [(true, 0), (false, 2)] {
                let hud = Some([59.0, 16.9]);
                let view = View {
                    modern,
                    comparison,
                    hud,
                    ..view_at(time)
                };
                let u = renderer.controls(&clip, view);
                let shared = (u.state[0], u.hud, u.screen, u.misc, u.audio);
                let screen = [480.0, 270.0, time as f32, comparison as f32];
                let misc = [
                    4.0 / 3.0,
                    1.0,
                    clip.logo_at as f32,
                    f32::from(u8::from(modern)),
                ];
                let counter = [1.0, 59.0, 16.9, 0.0];
                let audio = clip.audio_at(time).intro;
                assert_eq!(shared, (phase, counter, screen, misc, audio), "{time} s");
            }
        }
        assert_eq!(renderer.controls(&clip, view_at(245.5)).hud, [0.0; 4]);
    }

    fn view_at(time: f64) -> View {
        let size = (480, 270);
        View {
            time,
            size,
            comparison: 0,
            modern: true,
            hud: None,
        }
    }

    /// A renderer starts again for each clip it is given, by the clip's
    /// number rather than by where its data happens to lie in memory, which
    /// a clip loaded after another is dropped may reuse: it draws a new
    /// clip as a new renderer would, here one with no scroller letters.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn a_renderer_starts_again_for_each_clip() {
        let data = crate::test_support::data();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || pollster::block_on(Renderer::new(&instance, None)).unwrap();
        let (mut playing, format) = renderer();
        let clip = Clip::load_scroller(&data, ScrollerLighting::Studio).unwrap();
        let lettered = playing.render_rgba(&clip, view_at(31.0), format).unwrap();
        let first = clip.id();
        drop(clip);
        let mut clip = Clip::load_scroller(&data, ScrollerLighting::Studio).unwrap();
        assert_ne!(clip.id(), first);
        for value in &mut clip.scroller.as_mut().unwrap().glyphs.atlas {
            *value = half::f16::from_f32(-13.0);
        }
        let blank = playing.render_rgba(&clip, view_at(31.0), format).unwrap();
        assert_eq!(playing.clip, Some(clip.id()));
        let fresh = renderer().0.render_rgba(&clip, view_at(31.0), format);
        assert!(blank == fresh.unwrap());
        assert!(blank != lettered, "the letters show");
    }

    /// The modern intro, laser cut and scroller draw the same whether a
    /// moment is played into or reached by a seek, also back from the
    /// scroller into the cut (whose scene is not drawn after the cut) and
    /// from one part of the cut to another (the scroller skips what its
    /// flat plates hide).
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn modern_intro_frames_are_the_same_however_reached() {
        let data = crate::test_support::data();
        let clip = Clip::load_scroller(&data, ScrollerLighting::Studio).unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = || pollster::block_on(Renderer::new(&instance, None)).unwrap();
        let (mut playing, format) = renderer();
        for t in [5.2, 27.0, 18.3, 31.0, 19.5, 8.0, 20.8, 18.1, 13.9, 23.0] {
            for k in (1..4).rev() {
                playing
                    .render_rgba(&clip, view_at(t - k as f64 / 60.0), format)
                    .unwrap();
            }
            let played = playing.render_rgba(&clip, view_at(t), format).unwrap();
            let fresh = renderer().0.render_rgba(&clip, view_at(t), format).unwrap();
            assert!(played == fresh, "{t} s");
            assert!(fresh.iter().any(|&p| p > 8), "{t} s is black");
        }
    }

    /// Rust field names and byte offsets, in declaration order.
    fn rust_fields() -> Vec<(&'static str, usize)> {
        macro_rules! fields {
            ($($field:ident),* $(,)?) => {
                vec![$((stringify!($field), offset_of!(Uniforms, $field))),*]
            };
        }
        let fields = fields![
            screen,
            state,
            misc,
            flights,
            plate_center,
            plate_rotation,
            transition,
            tile_order,
            next_palette,
            scroller,
            scroll_light,
            audio,
            plate_open_center,
            plate_open_rotation,
            hud,
            glyphs,
            backdrop,
            backdrop_motion,
            waves,
            rings,
            ring_centres,
            ring_open,
            ring_bands,
            ring_fades,
            symbol,
            symbol_layers,
            symbol_atlas,
            authored_atlas,
            neon_map,
            rings_style,
            field,
            dots,
            plasma,
            greetings,
            ending,
        ];
        // A field missing from this list leaves bytes unaccounted for.
        let last = size_of::<EndingBlock>();
        assert_eq!(fields.last().unwrap().1 + last, size_of::<Uniforms>());
        fields
    }
}
