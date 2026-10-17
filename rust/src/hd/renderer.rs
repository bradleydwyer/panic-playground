use super::clip::{Clip, HEIGHT, ScrollerLighting, ScrollerPreview, WIDTH, plate_transform};
use super::modern::Modern;
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
    /// screen x = 0, overlay step, and the authored layout (0 rows, 1 ring).
    glyphs: [f32; 4],
    /// Scroller background: oscillator `xx / 128` and `yy / 128` interpolated
    /// between ticks, backdrop style (0 chamber, 1 wall, 2 chrome) and the
    /// authored bump shape.
    backdrop: [f32; 4],
    /// Bump wall: oscillator movement over its exposure (source pixels, from
    /// shutter open to now) and the authored wall strength.
    backdrop_motion: [f32; 4],
}

/// The only WGSL declaration of [`Uniforms`] and its binding. [`wgsl`]
/// prepends it to every shader module, so no module can declare a stale or
/// partial copy that silently shifts later fields.
const CONTROLS_WGSL: &str = "\
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
    // lower row text pixel at screen x = 0, overlay step, layout (0 rows, 1 ring).
    glyphs: vec4f,
    // Scroller background: oscillator xx/128 and yy/128 (source pixels,
    // interpolated between ticks), backdrop (0 chamber, 1 tinted wall, 2 chrome
    // wall) and the authored bump shape (0 rounded square .. 1 hemisphere).
    backdrop: vec4f,
    // Bump wall: oscillator movement over its exposure (source pixels), the
    // authored wall strength, unused.
    backdrop_motion: vec4f,
}
@group(0) @binding(0) var<uniform> u: Controls;
";

/// A shader module's complete source: the shared controls, then its body.
pub(super) fn wgsl(body: &str) -> String {
    [CONTROLS_WGSL, body].concat()
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
    modern_renderer: Modern,
    modern_view: bool,
    tile_transition: bool,
    scroller: bool,
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
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Panic HD GPU"),
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
            source: wgpu::ShaderSource::Wgsl(wgsl(include_str!("scroller.wgsl")).into()),
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
                modern_renderer,
                modern_view: false,
                tile_transition: false,
                scroller: false,
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
        if let Some(scroll) = &clip.scroller {
            self.upload_glyphs(scroll);
            if let Some([upper, lower, overlay]) = clip.scroll_at(time) {
                // Faithful HD keeps the original's two rows.
                let layout = if modern { scroll.layout as u32 } else { 0 };
                glyphs = [upper as f32, lower as f32, overlay as f32, layout as f32];
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
            // The oscillator is constant outside the scroll, so blending with
            // the following tick is always safe.
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
        };
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

    /// Every shader module body in this directory. Reading the directory
    /// means a new module is checked without being listed here.
    fn modules() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/hd");
        let mut modules: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|e| e == "wgsl"))
            .map(|path| {
                let body = std::fs::read_to_string(&path).unwrap();
                (path.file_name().unwrap().to_string_lossy().into(), body)
            })
            .collect();
        modules.sort();
        modules
    }

    #[test]
    fn every_shader_validates_with_the_one_controls_declaration() {
        let modules = modules();
        let names: Vec<_> = modules.iter().map(|m| m.0.as_str()).collect();
        for expected in [
            "intro.wgsl",
            "modern.wgsl",
            "post.wgsl",
            "scroller.wgsl",
            "transition.wgsl",
        ] {
            assert!(names.contains(&expected), "{expected} not found");
        }
        let expected: Vec<(&str, usize)> = rust_fields();
        for (name, body) in &modules {
            assert!(
                !body.contains("struct Controls") && !body.contains("@binding(0)"),
                "{name} must not declare its own controls; renderer::wgsl prepends them"
            );
            let source = wgsl(body);
            let module = naga::front::wgsl::parse_str(&source)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&source)));
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
        ];
        // A field missing from this list leaves bytes unaccounted for.
        let last = size_of::<[f32; 4]>();
        assert_eq!(fields.last().unwrap().1 + last, size_of::<Uniforms>());
        fields
    }
}
