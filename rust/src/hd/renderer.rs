use super::clip::{Clip, HEIGHT, ScrollerLighting, ScrollerPreview, WIDTH, plate_transform};
use super::dots::{DotsBlock, DotsPreview, PART3_PHASE};
use super::dots_render::DotsRenderer;
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
    // blue.
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
    // The back sheet: bottom edge's centre, height; half width.
    sheet_back: vec4f,
    sheet_back_size: vec4f,
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
    fields: [wgpu::Texture; 2],
    field_keys: [Option<(u8, usize)>; 2],
    reference: wgpu::Texture,
    reference_index: Option<usize>,
    /// Scroller glyph atlas and tables; 1 × 1 until a scroller clip is shown.
    glyph_atlas: wgpu::Texture,
    scroll_data: wgpu::Texture,
    glyph_key: Option<(usize, usize)>,
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
    bind_group: wgpu::BindGroup,
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
        let bind_group = bind_inputs(
            &device,
            &layout,
            &uniform,
            [
                &fields[0],
                &fields[1],
                &reference,
                &glyph_atlas,
                &scroll_data,
                &glyph_field,
            ],
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
                field_keys: [None, None],
                reference,
                reference_index: None,
                glyph_atlas,
                scroll_data,
                glyph_key: None,
                glyph_field,
                glyph_field_pipeline,
                field_inputs,
                field_bind_group,
                glyphs_active: false,
                inputs: layout,
                sampler,
                bind_group,
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
            },
            format,
        ))
    }

    pub fn prepare(&mut self, clip: &Clip, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud,
        } = view;
        self.hud_visible = hud.is_some();
        self.modern_view = modern;
        self.part6 = false;
        self.part5 = false;
        self.part3 = false;
        self.part4 = false;
        self.part7 = false;
        if let Some(greetings) = &clip.greetings {
            if self.greetings.as_ref().is_none_or(|r| !r.holds(greetings)) {
                self.greetings = Some(GreetingsRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    self.output_format,
                    greetings,
                ));
            }
            if greetings.frame_at(time).is_some() {
                self.prepare_greetings(clip, greetings, view);
                return;
            }
        }
        if let Some(dots) = &clip.dots
            && self.dots.as_ref().is_none_or(|r| !r.holds(dots))
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
            if self.plasma.as_ref().is_none_or(|r| !r.holds(plasma)) {
                self.plasma = Some(PlasmaRenderer::new(
                    &self.device,
                    &self.queue,
                    &self.uniform,
                    self.output_format,
                    plasma,
                ));
            }
            if plasma.frame_at(time).is_some() {
                self.prepare_plasma(clip, plasma, view);
                return;
            }
        }
        if let Some(dots) = &clip.dots
            && dots.frame_at(time).is_some()
        {
            self.prepare_dots(clip, dots, view);
            return;
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
                self.prepare_field(clip, field, view);
                return;
            }
        }
        if let Some(rings) = &clip.rings {
            if self.rings.as_ref().is_none_or(|r| !r.holds(rings)) {
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
                self.prepare_rings(clip, rings, j, view);
                return;
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
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
        }
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
        for (slot, i) in [index, next].into_iter().enumerate() {
            let source = &clip.frames[i];
            // Faithful HD draws smooth scroller letters over the original
            // backdrop; the modern scroller needs no frame fields at all.
            let backdrop = source.state.phase >= 8 && !modern && slot == 0;
            if (source.state.phase < 6 || backdrop)
                && self.field_keys[slot] != Some(source.shape_key())
            {
                let field = if backdrop {
                    source.backdrop()
                } else {
                    source.field()
                };
                self.queue.write_texture(
                    self.fields[slot].as_image_copy(),
                    bytemuck::cast_slice(&field),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(WIDTH as u32 * 8),
                        rows_per_image: Some(HEIGHT as u32),
                    },
                    wgpu::Extent3d {
                        width: WIDTH as u32,
                        height: HEIGHT as u32,
                        depth_or_array_layers: 1,
                    },
                );
                self.field_keys[slot] = Some(source.shape_key());
            }
        }
        if self.reference_index != Some(index) {
            self.queue.write_texture(
                self.reference.as_image_copy(),
                &frame.rgba(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH as u32 * 4),
                    rows_per_image: Some(HEIGHT as u32),
                },
                wgpu::Extent3d {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    depth_or_array_layers: 1,
                },
            );
            self.reference_index = Some(index);
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
        let (plate_center, plate_rotation) = plate_transform(sequence_frame);
        let (mut plate_open_center, plate_open_rotation) =
            plate_transform(sequence_frame - SHUTTER_FRAMES);
        plate_open_center[3] = SHUTTER_FRAMES;
        let mut transition = [0.0; 4];
        let mut tile_order = [[0.0; 4]; 40];
        let mut next_palette = [[0.0; 4]; 64];
        if let Some(cut) = &clip.transition {
            transition = [
                (time * 70.0 - cut.start_tick as f64) as f32,
                1.0,
                (cut.end_tick - cut.start_tick) as f32,
                // Formerly the trial style; only the laser cut remains.
                0.0,
            ];
            for (i, &rank) in cut.order.iter().enumerate() {
                tile_order[i / 4][i % 4] = rank as f32;
            }
            next_palette = cut.palette;
        }
        let audio = clip.audio_at(time);
        let mut scroller = [0.0; 4];
        let mut scroll_light = [0.0; 4];
        let mut glyphs = [0.0; 4];
        let mut backdrop = [0.0; 4];
        let mut backdrop_motion = [0.0; 4];
        let mut waves = [0.0; 4];
        if let Some(scroll) = &clip.scroller {
            self.upload_glyphs(scroll);
            if let Some([upper, lower, overlay]) = clip.scroll_at(time) {
                glyphs = [upper as f32, lower as f32, overlay as f32, 0.0];
            }
            scroller = [
                1.0,
                (time - scroll.start_tick as f64 / 70.0) as f32,
                // Formerly the preset; glass is now the only scroller style.
                0.0,
                (1.0 - ((time * 70.0 - scroll.end_tick as f64) / 12.0).max(0.0)).clamp(0.0, 1.0)
                    as f32,
            ];
            // The same causal cues as the intro; studio mode ignores them.
            let [level, bass, treble] = audio.scroller;
            scroll_light = [
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
            for (i, color) in next_palette.iter_mut().enumerate() {
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
            backdrop = [
                now[0],
                now[1],
                scroll.backdrop_style as u32 as f32,
                scroll.wall_shape,
            ];
            backdrop_motion = [
                now[0] - open[0],
                now[1] - open[1],
                scroll.wall_strength,
                0.0,
            ];
            let reveal = clip.transition.as_ref().map_or(0, |cut| cut.start_tick);
            let [depth, light] = scroll.waves_at(time, reveal);
            let swing = scroll.wave_swing(time, reveal);
            waves = [depth, (WALL_SHUTTER_TICKS / 70.0) as f32, light, swing];
        }
        let u = Uniforms {
            screen: [size.0 as f32, size.1 as f32, time as f32, comparison as f32],
            state: [
                frame.state.phase as f32,
                sequence_frame,
                frame.state.door as f32,
                mix,
            ],
            misc: [
                4.0 / 3.0,
                if self.srgb { 1.0 } else { 0.0 },
                clip.logo_at as f32,
                if modern { 1.0 } else { 0.0 },
            ],
            flights: clip.flights,
            plate_center,
            plate_rotation,
            transition,
            tile_order,
            next_palette,
            scroller,
            scroll_light,
            audio: audio.intro,
            plate_open_center,
            plate_open_rotation,
            hud: hud.map_or([0.0; 4], |[fps, ms]| [1.0, fps, ms, 0.0]),
            glyphs,
            backdrop,
            backdrop_motion,
            waves,
            ..Uniforms::zeroed()
        };
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Part 6: the shared controls for this moment, and the original frame
    /// when a comparison shows it, replayed on demand
    /// ([`RingsPreview::original`]) rather than stored.
    fn prepare_rings(&mut self, clip: &Clip, rings: &RingsPreview, j: usize, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud: _,
        } = view;
        self.part6 = true;
        self.rings_style = rings.style;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
            if let Some(renderer) = &mut self.rings {
                renderer.resize(&self.device, rings.style, size);
            }
        }
        let index = rings.start + j;
        if comparison != 0 && self.reference_index != Some(index) {
            self.queue.write_texture(
                self.reference.as_image_copy(),
                &rings.original_rgba(j),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH as u32 * 4),
                    rows_per_image: Some(HEIGHT as u32),
                },
                wgpu::Extent3d {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    depth_or_array_layers: 1,
                },
            );
            self.reference_index = Some(index);
        }
        let controls = rings.controls(time);
        let audio = clip.audio_at(time);
        let mut u = Uniforms::zeroed();
        u.screen = [size.0 as f32, size.1 as f32, time as f32, comparison as f32];
        u.state = [PART6_PHASE, j as f32, 0.0, (time * 70.0).fract() as f32];
        u.misc = [
            4.0 / 3.0,
            if self.srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ];
        u.audio = audio.intro;
        if let Some(scroll) = &clip.scroller {
            let [level, bass, treble] = audio.scroller;
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
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Make part 5's renderer, unless it holds `field` already.
    fn hold_field(&mut self, field: &FieldPreview) {
        if self.field.as_ref().is_none_or(|r| !r.holds(field)) {
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
    fn prepare_dots(&mut self, clip: &Clip, dots: &DotsPreview, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud: _,
        } = view;
        self.part3 = true;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
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
                self.modern_renderer
                    .resize(&self.device, &self.uniform, &self.reference, size);
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
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
        }
        self.dots.as_mut().unwrap().resize(&self.device, size);
        let index = dots.start + j;
        if comparison != 0 && self.reference_index != Some(index) {
            self.queue.write_texture(
                self.reference.as_image_copy(),
                &dots.original_rgba(j),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH as u32 * 4),
                    rows_per_image: Some(HEIGHT as u32),
                },
                wgpu::Extent3d {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    depth_or_array_layers: 1,
                },
            );
            self.reference_index = Some(index);
        }
        let audio = clip.audio_at(time);
        let mut u = Uniforms::zeroed();
        u.screen = [size.0 as f32, size.1 as f32, time as f32, comparison as f32];
        u.state = [PART3_PHASE, j as f32, 0.0, (time * 70.0).fract() as f32];
        u.misc = [
            4.0 / 3.0,
            if self.srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ];
        u.audio = audio.intro;
        u.field = field_block;
        u.dots = block;
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Part 4: the shared controls for this moment, with the music's cues;
    /// in its first moments, part 3's controls too, for its modern view to
    /// hand over from; and the original frame when a comparison shows it,
    /// rebuilt on demand ([`PlasmaPreview::original_full`]).
    fn prepare_plasma(&mut self, clip: &Clip, plasma: &PlasmaPreview, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud: _,
        } = view;
        self.part4 = true;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
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
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
            self.plasma.as_mut().unwrap().resize(&self.device, size);
            if let (true, Some(dots)) = (block.camera[2] > 0.0, &clip.dots) {
                let aspect = size.0 as f64 / size.1.max(1) as f64;
                dots_controls = dots_block(clip, dots, time, aspect);
                self.dots.as_mut().unwrap().resize(&self.device, size);
                self.part4_handover = true;
            }
        } else {
            block.camera[2] = 0.0;
        }
        let index = plasma.start + j;
        if comparison != 0 {
            self.plasma
                .as_mut()
                .unwrap()
                .show_original(&self.queue, plasma, j);
            if self.reference_index != Some(index) {
                self.queue.write_texture(
                    self.reference.as_image_copy(),
                    &plasma.original_rgba(j),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(WIDTH as u32 * 4),
                        rows_per_image: Some(HEIGHT as u32),
                    },
                    wgpu::Extent3d {
                        width: WIDTH as u32,
                        height: HEIGHT as u32,
                        depth_or_array_layers: 1,
                    },
                );
                self.reference_index = Some(index);
            }
        }
        let audio = clip.audio_at(time);
        let mut u = Uniforms::zeroed();
        u.screen = [size.0 as f32, size.1 as f32, time as f32, comparison as f32];
        u.state = [PART4_PHASE, j as f32, 0.0, (time * 70.0).fract() as f32];
        u.misc = [
            4.0 / 3.0,
            if self.srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ];
        u.audio = audio.intro;
        u.dots = dots_controls;
        u.plasma = block;
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Part 7: the shared controls for this moment, and the original frame
    /// when a comparison shows it, rebuilt on demand
    /// ([`GreetingsPreview::original_rgba`]).
    fn prepare_greetings(&mut self, clip: &Clip, greetings: &GreetingsPreview, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud: _,
        } = view;
        self.part7 = true;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
        let j = greetings.frame_at(time).unwrap_or(0);
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
            self.greetings.as_mut().unwrap().resize(&self.device, size);
        }
        let index = greetings.start + j;
        if comparison != 0 {
            let renderer = self.greetings.as_mut().unwrap();
            renderer.show_original(&self.queue, greetings, j);
            if self.reference_index != Some(index) {
                self.queue.write_texture(
                    self.reference.as_image_copy(),
                    &greetings.original_rgba(j),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(WIDTH as u32 * 4),
                        rows_per_image: Some(HEIGHT as u32),
                    },
                    wgpu::Extent3d {
                        width: WIDTH as u32,
                        height: HEIGHT as u32,
                        depth_or_array_layers: 1,
                    },
                );
                self.reference_index = Some(index);
            }
        }
        let mut u = Uniforms::zeroed();
        u.screen = [size.0 as f32, size.1 as f32, time as f32, comparison as f32];
        u.state = [PART7_PHASE, j as f32, 0.0, (time * 70.0).fract() as f32];
        u.misc = [
            4.0 / 3.0,
            if self.srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ];
        u.audio = clip.audio_at(time).intro;
        u.greetings = greetings.controls(time);
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Part 5: the shared controls for this moment, its palette and zoom
    /// buffers, the fluid brought to this moment, and the original frame
    /// when a comparison shows it, replayed on demand
    /// ([`FieldPreview::original`]).
    fn prepare_field(&mut self, clip: &Clip, field: &FieldPreview, view: View) {
        let View {
            time,
            size,
            comparison,
            modern,
            hud: _,
        } = view;
        self.part5 = true;
        self.glyphs_active = false;
        self.scroller = false;
        self.tile_transition = false;
        let j = field.frame_at(time).unwrap_or(0);
        let renderer = self.field.as_mut().unwrap();
        if modern {
            self.modern_renderer
                .resize(&self.device, &self.uniform, &self.reference, size);
            renderer.resize(&self.device, size);
        }
        renderer.update(&self.queue, field, j);
        // Before part 5, when its view draws part 6's fade, the original is
        // part 6's frame.
        let before = clip
            .rings
            .as_ref()
            .filter(|_| field.frame_at(time).is_none())
            .and_then(|r| Some((r, r.frame_at(time)?)));
        let index = before.map_or(field.start + j, |(r, k)| r.start + k);
        if comparison != 0 && self.reference_index != Some(index) {
            let original = match before {
                Some((rings, k)) => rings.original_rgba(k),
                None => field.original_rgba(j),
            };
            self.queue.write_texture(
                self.reference.as_image_copy(),
                &original,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH as u32 * 4),
                    rows_per_image: Some(HEIGHT as u32),
                },
                wgpu::Extent3d {
                    width: WIDTH as u32,
                    height: HEIGHT as u32,
                    depth_or_array_layers: 1,
                },
            );
            self.reference_index = Some(index);
        }
        let mut block = field.controls(time);
        if modern {
            block.fluid = renderer.advance_fluid(&self.device, &self.queue, field, time);
            renderer.advance_zoom(&self.device, &self.queue, &block, size);
        }
        let audio = clip.audio_at(time);
        let mut u = Uniforms::zeroed();
        u.screen = [size.0 as f32, size.1 as f32, time as f32, comparison as f32];
        u.state = [PART5_PHASE, j as f32, 0.0, (time * 70.0).fract() as f32];
        u.misc = [
            4.0 / 3.0,
            if self.srgb { 1.0 } else { 0.0 },
            clip.logo_at as f32,
            if modern { 1.0 } else { 0.0 },
        ];
        u.audio = audio.intro;
        u.field = block;
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    /// Upload a scroller clip's glyph atlas and tables once, on first use.
    fn upload_glyphs(&mut self, scroll: &ScrollerPreview) {
        let glyphs = &scroll.glyphs;
        // The clip owns these buffers for its lifetime; identify it by them.
        let key = (glyphs.atlas.as_ptr() as usize, glyphs.atlas.len());
        if self.glyph_key == Some(key) {
            return;
        }
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
        self.bind_group = bind_inputs(
            &self.device,
            &self.inputs,
            &self.uniform,
            [
                &self.fields[0],
                &self.fields[1],
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
        self.glyph_key = Some(key);
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
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
                &self.bind_group,
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
            pass.set_bind_group(0, &self.bind_group, &[]);
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
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.background);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.stars);
        pass.draw(0..6, 0..512);
        pass.set_pipeline(&self.foreground);
        pass.draw(0..3, 0..1);
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

    pub fn capture(
        &mut self,
        clip: &Clip,
        view: View,
        path: &Path,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        let size = view.size;
        let pixels = self.render_rgba(clip, view, format)?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let mut png = png::Encoder::new(BufWriter::new(file), size.0, size.1);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        let mut writer = png.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Everything part 3 puts in `u.dots` at a clip time, with the music's
/// cues: the low band at the moment and each tick before, for the trails;
/// and each order's first row's strength.
fn dots_block(clip: &Clip, dots: &DotsPreview, time: f64, aspect: f64) -> DotsBlock {
    let mut block = dots.controls(time, aspect);
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

/// Shared scene inputs: controls, two shape fields, the reference frame, a
/// filtering sampler, the scroller glyph atlas, its tables and the per-frame
/// scroller fields.
fn bind_inputs(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    textures: [&wgpu::Texture; 6],
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let views = textures.map(|t| t.create_view(&Default::default()));
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
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[[f32; 4]; 2]>(),
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
                    "dots_trail" | "shade_trail" | "swept" => &[(1, 2)],
                    "dots_haze" => &[(1, 2), (1, 3), (1, 4), (1, 5)],
                    "dots_style" => &[(1, 2), (2, 1), (2, 2), (2, 3)],
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
        let expected = fields![part, frame, sweep, twist, twist_late, events, events_late];
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
    /// written by their trace and read by the caustics, the blocks' geometry
    /// and the strips' transmission for the dust and the air, and the dust,
    /// the geometry and the air for the picture.
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
                    "photon_vertex" | "photon_fragment" => &[(1, 8)],
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
        ];
        // A field missing from this list leaves bytes unaccounted for.
        let last = size_of::<GreetingsBlock>();
        assert_eq!(fields.last().unwrap().1 + last, size_of::<Uniforms>());
        fields
    }
}
