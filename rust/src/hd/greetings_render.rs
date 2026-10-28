//! GPU side of part 7: its tables, the letters' faces, the dust, the air's
//! density, and the pipelines. Faithful HD draws straight into the output
//! from the tables. The modern view traces the beam's photons through the
//! crystal blocks and gathers them into a map of what the strips let
//! through as the lamp sees it, marches the blocks (their coverage and
//! depth), draws the dust and marches the lit air at half size (each in
//! front of the blocks and in all), and composites, the crystal shaded
//! there, into the modern renderer's HDR target, which then takes the
//! shared bloom and tonemap.
use super::greetings::{GreetingsPreview, ROWS, SLOTS, STRIPS};
use super::greetings_type::ATLAS_SIZE;
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("greetings_common.wgsl");
const FAITHFUL: &str = include_str!("greetings_faithful.wgsl");
const LIGHT: &str = include_str!("greetings_light.wgsl");
const MONSTER: &str = include_str!("greetings_monster.wgsl");

/// Faithful HD's module: the shared controls, greetings_common.wgsl and
/// greetings_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, greetings_common.wgsl,
/// slab_breakup.wgsl and greetings_light.wgsl.
pub(super) fn light_source() -> String {
    wgsl_parts(&[COMMON, super::slab_breakup::WGSL, LIGHT])
}

/// The modern view's monster, a module of its own: the shared controls,
/// greetings_common.wgsl and greetings_monster.wgsl.
pub(super) fn monster_source() -> String {
    wgsl_parts(&[COMMON, MONSTER])
}

/// What the blocks let through, seen from the lamp: the map's size (a map
/// of directions round the beam's axis; greetings_light.wgsl's MAP_SPREAD).
pub const TRANSMISSION_SIZE: (u32, u32) = (1024, 1024);
/// The air's density noise: its size (texels a side) and lattices.
pub const NOISE_SIZE: u32 = 64;
/// The photons: one lamp ray for each cell of this grid over the map, at
/// each of three wavelengths (greetings_light.wgsl's PHOTON_GRID).
pub const PHOTON_GRID: (u32, u32) = (600, 600);
const PHOTONS: u32 = PHOTON_GRID.0 * PHOTON_GRID.1 * 3;
/// The photons a workgroup of count_splats and list_splats takes
/// (greetings_light.wgsl's SPLAT_BLOCK: its 256 threads, SPLAT_RUN each).
const SPLAT_BLOCK: u32 = 2048;

/// How the strips' transmission map is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Caustics {
    /// Only cleared, as the trace would leave it where the photons move no
    /// light ([`GreetingsPreview::photons_can_move_light`]).
    Cleared,
    /// The photons traced, and of their splats those that move light drawn,
    /// listed in order (greetings_light.wgsl's count_splats and
    /// list_splats).
    Listed,
    /// The photons traced and every splat drawn, in order, as the list
    /// stands in for (tests).
    #[cfg(test)]
    All,
}

/// The images the modern view's passes hand on, for one drawable size.
struct Targets {
    size: (u32, u32),
    geometry: TextureView,
    /// Part 4's view of its slab while it breaks up into the dust: the
    /// picture and the stone's surface (only while it does).
    slab: Option<[TextureView; 2]>,
    dust_front: TextureView,
    dust_full: TextureView,
    air_front: TextureView,
    air_full: TextureView,
    scene_inputs: BindGroup,
    style_inputs: BindGroup,
}

pub(super) struct GreetingsRenderer {
    data: BindGroup,
    original: Texture,
    /// The original frame uploaded to `original`.
    original_frame: Option<usize>,
    motes: u32,
    /// The grains part 4's slab breaks up into ([`super::slab_breakup`]).
    grains: u32,
    /// Bound for part 4's slab's images while it isn't breaking up.
    no_slab: TextureView,
    #[cfg(test)]
    transmission_map: Texture,
    transmission: TextureView,
    photons_out: BindGroup,
    photons_in: BindGroup,
    /// The splats the caustics draw, in order, and the draw's arguments
    /// (greetings_light.wgsl's count_splats and list_splats).
    splats_listed: BindGroup,
    splat_draw: Buffer,
    #[cfg(test)]
    splats: Buffer,
    scene_layout: BindGroupLayout,
    style_layout: BindGroupLayout,
    targets: Option<Targets>,
    faithful: RenderPipeline,
    trace: ComputePipeline,
    count_splats: ComputePipeline,
    list_splats: ComputePipeline,
    caustics: RenderPipeline,
    blocks: RenderPipeline,
    dust: RenderPipeline,
    air: RenderPipeline,
    style: RenderPipeline,
    monster: RenderPipeline,
    /// Whether the moment shows the monster (the modern view then draws
    /// only it).
    pub shows_monster: bool,
    /// Whether the moment's photons can move any light
    /// ([`GreetingsPreview::photons_can_move_light`]); where they can't, the
    /// trace and the caustics are skipped and their map only cleared.
    pub traces_photons: bool,
    /// Whether part 4's slab is breaking up into the dust
    /// ([`GreetingsRenderer::break_slab`]).
    breaks_slab: bool,
}

const HDR: TextureFormat = TextureFormat::Rgba16Float;
const FLOAT: TextureSampleType = TextureSampleType::Float { filterable: true };
const ADD: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
};

fn texture(
    device: &Device,
    label: &str,
    (width, height): (u32, u32),
    format: TextureFormat,
    usage: TextureUsages,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn upload(queue: &Queue, t: &Texture, bytes: &[u8], (width, height): (u32, u32), texel: u32) {
    queue.write_texture(
        t.as_image_copy(),
        bytes,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * texel),
            rows_per_image: Some(height),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

fn image(
    device: &Device,
    queue: &Queue,
    label: &str,
    size: (u32, u32),
    format: TextureFormat,
    bytes: &[u8],
    texel: u32,
) -> TextureView {
    let t = texture(
        device,
        label,
        size,
        format,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    );
    upload(queue, &t, bytes, size, texel);
    t.create_view(&Default::default())
}

fn entry(binding: u32, visibility: ShaderStages, ty: BindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    }
}

fn sampled(sample_type: TextureSampleType) -> BindingType {
    BindingType::Texture {
        sample_type,
        view_dimension: TextureViewDimension::D2,
        multisampled: false,
    }
}

fn resource(binding: u32, b: &Buffer) -> BindGroupEntry<'_> {
    BindGroupEntry {
        binding,
        resource: b.as_entire_binding(),
    }
}

fn view(binding: u32, v: &TextureView) -> BindGroupEntry<'_> {
    BindGroupEntry {
        binding,
        resource: BindingResource::TextureView(v),
    }
}

/// The letters' table the shaders read: per frame the first slot's top,
/// then each strip's 16 slots, four to a word.
fn letter_words(preview: &GreetingsPreview) -> Vec<u32> {
    let mut words = Vec::with_capacity(preview.slots.len() * 24);
    for (base, table) in &preview.slots {
        words.extend([*base as u32, 0, 0, 0]);
        for strip in table.iter().take(STRIPS) {
            for k in 0..SLOTS / 4 {
                words.push(u32::from_le_bytes([
                    strip[4 * k],
                    strip[4 * k + 1],
                    strip[4 * k + 2],
                    strip[4 * k + 3],
                ]));
            }
        }
    }
    words
}

impl GreetingsRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        output: TextureFormat,
        preview: &GreetingsPreview,
    ) -> Self {
        let frames = preview.frames.len();
        let buffer = |label, contents: &[u8]| {
            device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: BufferUsages::STORAGE,
            })
        };
        let planes: Vec<u32> = preview
            .planes
            .chunks(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let planes = buffer("Part 7 TRUG planes", bytemuck::cast_slice(&planes));
        assert_eq!(preview.shown.len(), frames * STRIPS * ROWS);
        let shown: Vec<u32> = preview
            .shown
            .chunks(2)
            .map(|p| p[0] as u32 | (p[1] as u32) << 16)
            .collect();
        let shown = buffer("Part 7 strip rows", bytemuck::cast_slice(&shown));
        let letters = buffer(
            "Part 7 letters",
            bytemuck::cast_slice(&letter_words(preview)),
        );
        let motes = buffer("Part 7 dust", bytemuck::cast_slice(&preview.motes));
        // Each grain's mote; one entry at least, for the binding.
        let mut cell_motes = preview.breakup.motes.clone();
        cell_motes.push(super::slab_breakup::NO_MOTE);
        let cell_motes = buffer(
            "Part 7 dust from part 4's slab",
            bytemuck::cast_slice(&cell_motes),
        );
        let indices: Vec<u8> = (0..=255).collect();
        let palettes: Vec<u8> = (0..frames)
            .flat_map(|j| super::field::rgba(&indices, preview.palette(j)))
            .collect();
        let palettes = image(
            device,
            queue,
            "Part 7 palettes",
            (256, frames as u32),
            TextureFormat::Rgba8Unorm,
            &palettes,
            4,
        );
        let monster = image(
            device,
            queue,
            "Part 7 monster",
            (320, 200),
            TextureFormat::R8Uint,
            &preview.tables.monster[..64000],
            1,
        );
        let glyphs = image(
            device,
            queue,
            "Part 7 letters' faces",
            ATLAS_SIZE,
            HDR,
            bytemuck::cast_slice(&preview.letterforms.texels),
            8,
        );
        let original = texture(
            device,
            "Part 7 original",
            (320, 200),
            TextureFormat::Rgba8Unorm,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        );
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 7"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let repeat = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 7 repeating"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let noise_texture = device.create_texture(&TextureDescriptor {
            label: Some("Part 7 air's density"),
            size: Extent3d {
                width: NOISE_SIZE,
                height: NOISE_SIZE,
                depth_or_array_layers: NOISE_SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            noise_texture.as_image_copy(),
            &noise(),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(NOISE_SIZE),
                rows_per_image: Some(NOISE_SIZE),
            },
            Extent3d {
                width: NOISE_SIZE,
                height: NOISE_SIZE,
                depth_or_array_layers: NOISE_SIZE,
            },
        );
        let noise_view = noise_texture.create_view(&Default::default());
        // (Tests read the map back.)
        let readable = if cfg!(test) {
            TextureUsages::COPY_SRC
        } else {
            TextureUsages::empty()
        };
        let transmission_map = texture(
            device,
            "Part 7 strips' transmission",
            TRANSMISSION_SIZE,
            HDR,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | readable,
        );
        let transmission = transmission_map.create_view(&Default::default());
        let all = ShaderStages::VERTEX | ShaderStages::FRAGMENT | ShaderStages::COMPUTE;
        let storage = BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        // The data every pass reads: its layout and group.
        let original_view = original.create_view(&Default::default());
        let data_for = |visibility: ShaderStages| {
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Part 7 data"),
                entries: &[
                    entry(
                        0,
                        visibility,
                        BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                    ),
                    entry(1, visibility, storage),
                    entry(2, visibility, storage),
                    entry(3, visibility, storage),
                    entry(4, visibility, sampled(FLOAT)),
                    entry(5, visibility, sampled(TextureSampleType::Uint)),
                    entry(6, visibility, sampled(FLOAT)),
                    entry(
                        7,
                        visibility,
                        BindingType::Sampler(SamplerBindingType::Filtering),
                    ),
                    entry(8, visibility, sampled(FLOAT)),
                    entry(9, visibility, storage),
                    entry(
                        10,
                        visibility,
                        BindingType::Texture {
                            sample_type: FLOAT,
                            view_dimension: TextureViewDimension::D3,
                            multisampled: false,
                        },
                    ),
                    entry(
                        11,
                        visibility,
                        BindingType::Sampler(SamplerBindingType::Filtering),
                    ),
                    entry(12, visibility, storage),
                ],
            });
            let group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Part 7 data"),
                layout: &layout,
                entries: &[
                    resource(0, uniform),
                    resource(1, &planes),
                    resource(2, &shown),
                    resource(3, &letters),
                    view(4, &palettes),
                    view(5, &monster),
                    view(6, &original_view),
                    BindGroupEntry {
                        binding: 7,
                        resource: BindingResource::Sampler(&sampler),
                    },
                    view(8, &glyphs),
                    resource(9, &motes),
                    view(10, &noise_view),
                    BindGroupEntry {
                        binding: 11,
                        resource: BindingResource::Sampler(&repeat),
                    },
                    resource(12, &cell_motes),
                ],
            });
            (layout, group)
        };
        let (data_layout, data) = data_for(all);
        let unfiltered = sampled(TextureSampleType::Float { filterable: false });
        let scene_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 scene's inputs"),
            entries: &[
                entry(2, all, unfiltered),
                entry(3, all, sampled(FLOAT)),
                entry(13, all, sampled(FLOAT)),
                entry(14, all, sampled(FLOAT)),
            ],
        });
        let fragment = ShaderStages::FRAGMENT;
        let style_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 picture's inputs"),
            entries: &[
                entry(1, fragment, sampled(FLOAT)),
                entry(2, fragment, unfiltered),
                entry(3, fragment, sampled(FLOAT)),
                entry(4, fragment, sampled(FLOAT)),
                entry(5, fragment, sampled(FLOAT)),
                entry(6, fragment, sampled(FLOAT)),
                entry(13, fragment, sampled(FLOAT)),
                entry(14, fragment, sampled(FLOAT)),
            ],
        });
        let photons = device.create_buffer(&BufferDescriptor {
            label: Some("Part 7 photons"),
            size: PHOTONS as u64 * 16,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let photons_out_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 photons traced"),
            entries: &[entry(
                7,
                ShaderStages::COMPUTE,
                BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            )],
        });
        let photons_in_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 photons gathered"),
            entries: &[
                entry(8, ShaderStages::VERTEX, storage),
                entry(12, ShaderStages::VERTEX, storage),
            ],
        });
        // The splats: a count for each block of photons, the list (at most
        // two for each photon) and the draw's four arguments. (Tests write
        // the list and the arguments to draw every splat.)
        let written = if cfg!(test) {
            BufferUsages::COPY_DST
        } else {
            BufferUsages::empty()
        };
        let splat_buffer = |label, size: u64, usage| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size,
                usage: BufferUsages::STORAGE | usage,
                mapped_at_creation: false,
            })
        };
        let block_splats = splat_buffer(
            "Part 7 splats a block",
            PHOTONS.div_ceil(SPLAT_BLOCK) as u64 * 4,
            BufferUsages::empty(),
        );
        let splats = splat_buffer("Part 7 splats", PHOTONS as u64 * 2 * 4, written);
        let splat_draw = splat_buffer("Part 7 splats drawn", 16, BufferUsages::INDIRECT | written);
        let read_write = BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let splats_listed_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 splats listed"),
            entries: &[
                entry(8, ShaderStages::COMPUTE, storage),
                entry(9, ShaderStages::COMPUTE, read_write),
                entry(10, ShaderStages::COMPUTE, read_write),
                entry(11, ShaderStages::COMPUTE, read_write),
            ],
        });
        let splats_listed = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 splats listed"),
            layout: &splats_listed_layout,
            entries: &[
                resource(8, &photons),
                resource(9, &block_splats),
                resource(10, &splats),
                resource(11, &splat_draw),
            ],
        });
        let photons_out = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 photons traced"),
            layout: &photons_out_layout,
            entries: &[resource(7, &photons)],
        });
        let photons_in = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 photons gathered"),
            layout: &photons_in_layout,
            entries: &[resource(8, &photons), resource(12, &splats)],
        });
        let layout = |groups: &[&BindGroupLayout]| {
            let groups: Vec<Option<&BindGroupLayout>> = groups.iter().map(|&g| Some(g)).collect();
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Part 7"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            })
        };
        let light_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 7 modern"),
            source: ShaderSource::Wgsl(light_source().into()),
        });
        let faithful_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 7 faithful HD"),
            source: ShaderSource::Wgsl(faithful_source().into()),
        });
        let pipeline = |label: &str,
                        layout: &PipelineLayout,
                        module: &ShaderModule,
                        vertex: &str,
                        fragment: &str,
                        targets: &[Option<ColorTargetState>]| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: VertexState {
                    module,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets,
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let target = |format, blend| {
            Some(ColorTargetState {
                format,
                blend,
                write_mask: ColorWrites::ALL,
            })
        };
        let faithful = pipeline(
            "Part 7 faithful HD",
            &layout(&[&data_layout]),
            &faithful_module,
            "fullscreen",
            "faithful",
            &[target(output, None)],
        );
        let scene = layout(&[&data_layout, &scene_layout]);
        let trace = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Part 7 photons"),
            layout: Some(&layout(&[&data_layout, &photons_out_layout])),
            module: &light_module,
            entry_point: Some("trace_photons"),
            compilation_options: Default::default(),
            cache: None,
        });
        let compute = |label, entry_point| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout(&[&data_layout, &splats_listed_layout])),
                module: &light_module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let count_splats = compute("Part 7 splats counted", "count_splats");
        let list_splats = compute("Part 7 splats listed", "list_splats");
        let caustics = pipeline(
            "Part 7 caustics",
            &layout(&[&data_layout, &photons_in_layout]),
            &light_module,
            "photon_vertex",
            "photon_fragment",
            &[target(HDR, Some(ADD))],
        );
        let blocks = pipeline(
            "Part 7 blocks",
            &layout(&[&data_layout]),
            &light_module,
            "fullscreen",
            "blocks",
            &[target(TextureFormat::Rgba32Float, None)],
        );
        let dust = pipeline(
            "Part 7 dust",
            &scene,
            &light_module,
            "dust_vertex",
            "dust_fragment",
            &[target(HDR, Some(ADD)), target(HDR, Some(ADD))],
        );
        let air = pipeline(
            "Part 7 air",
            &scene,
            &light_module,
            "fullscreen",
            "air",
            &[target(HDR, None), target(HDR, None)],
        );
        let style = pipeline(
            "Part 7 modern",
            &layout(&[&data_layout, &style_layout]),
            &light_module,
            "fullscreen",
            "style",
            &[target(HDR, None)],
        );
        let monster_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 7 modern monster"),
            source: ShaderSource::Wgsl(monster_source().into()),
        });
        let monster = pipeline(
            "Part 7 modern monster",
            &layout(&[&data_layout]),
            &monster_module,
            "fullscreen",
            "monster_style",
            &[target(HDR, None)],
        );
        Self {
            data,
            original,
            original_frame: None,
            motes: preview.motes.len() as u32,
            grains: preview.breakup.cells(),
            #[cfg(test)]
            transmission_map,
            transmission,
            photons_out,
            photons_in,
            splats_listed,
            splat_draw,
            #[cfg(test)]
            splats,
            scene_layout,
            style_layout,
            targets: None,
            faithful,
            trace,
            count_splats,
            list_splats,
            caustics,
            blocks,
            dust,
            air,
            style,
            monster,
            shows_monster: false,
            traces_photons: true,
            breaks_slab: false,
            no_slab: texture(
                device,
                "Part 7 no slab",
                (1, 1),
                HDR,
                TextureUsages::TEXTURE_BINDING,
            )
            .create_view(&Default::default()),
        }
    }

    /// Upload the original frame `j` for the comparison modes.
    pub fn show_original(&mut self, queue: &Queue, preview: &GreetingsPreview, j: usize) {
        if self.original_frame == Some(j) {
            return;
        }
        upload(
            queue,
            &self.original,
            &preview.original_rgba(j),
            (320, 200),
            4,
        );
        self.original_frame = Some(j);
    }

    /// Make the passes' images for this drawable size.
    pub fn resize(&mut self, device: &Device, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
        let make = |label, size, format| {
            texture(device, label, size, format, usage).create_view(&Default::default())
        };
        let geometry = make("Part 7 blocks", size, TextureFormat::Rgba32Float);
        let dust_front = make("Part 7 dust in front", size, HDR);
        let dust_full = make("Part 7 dust", size, HDR);
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        let air_front = make("Part 7 lit air in front", half, HDR);
        let air_full = make("Part 7 lit air", half, HDR);
        let (scene_inputs, style_inputs) =
            self.inputs(device, [&geometry, &dust_front, &dust_full, &air_front, &air_full], None);
        self.targets = Some(Targets {
            size,
            geometry,
            slab: None,
            dust_front,
            dust_full,
            air_front,
            air_full,
            scene_inputs,
            style_inputs,
        });
        self.breaks_slab = false;
    }

    /// The scene's and the picture's inputs: the blocks, the dust in front
    /// and all of it, the air in front and all of it, and part 4's slab's
    /// picture and surface (or the 1 x 1 stand-in).
    fn inputs(
        &self,
        device: &Device,
        [geometry, dust_front, dust_full, air_front, air_full]: [&TextureView; 5],
        slab: Option<&[TextureView; 2]>,
    ) -> (BindGroup, BindGroup) {
        let [picture, surface] = slab.map_or([&self.no_slab; 2], |[p, s]| [p, s]);
        let scene_inputs = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 scene's inputs"),
            layout: &self.scene_layout,
            entries: &[
                view(2, geometry),
                view(3, &self.transmission),
                view(13, picture),
                view(14, surface),
            ],
        });
        let style_inputs = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 picture's inputs"),
            layout: &self.style_layout,
            entries: &[
                view(1, dust_front),
                view(2, geometry),
                view(3, &self.transmission),
                view(4, dust_full),
                view(5, air_front),
                view(6, air_full),
                view(13, picture),
                view(14, surface),
            ],
        });
        (scene_inputs, style_inputs)
    }

    /// Whether the targets for a drawable size are made.
    pub fn has_targets(&self) -> bool {
        self.targets.is_some()
    }

    /// Whether part 4's slab is breaking up into the dust (renderer.rs fills
    /// u.plasma and has part 4's view draw the slab into
    /// [`GreetingsRenderer::slab`]): the dust then draws a grain for each
    /// cell of the slab, each mote still on its way from it. Its images are
    /// made for the drawable size while it does, and dropped after. Call
    /// [`GreetingsRenderer::resize`] first.
    pub fn break_slab(&mut self, device: &Device, breaks: bool) {
        self.breaks_slab = breaks;
        let t = self.targets.as_ref().unwrap();
        if breaks == t.slab.is_some() {
            return;
        }
        let slab = breaks.then(|| {
            let usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
            ["Part 7 part 4's slab", "Part 7 part 4's slab's surface"]
                .map(|label| texture(device, label, t.size, HDR, usage).create_view(&Default::default()))
        });
        let (scene_inputs, style_inputs) = self.inputs(
            device,
            [&t.geometry, &t.dust_front, &t.dust_full, &t.air_front, &t.air_full],
            slab.as_ref(),
        );
        let t = self.targets.as_mut().unwrap();
        (t.slab, t.scene_inputs, t.style_inputs) = (slab, scene_inputs, style_inputs);
    }

    /// Where part 4's view draws its slab while it breaks up (HDR): the
    /// picture, how much of the slab still stands in alpha, and the stone's
    /// surface. Call [`GreetingsRenderer::break_slab`] first.
    pub fn slab(&self) -> [&TextureView; 2] {
        let [picture, surface] = self.targets.as_ref().unwrap().slab.as_ref().unwrap();
        [picture, surface]
    }

    fn pass<'a>(
        encoder: &'a mut CommandEncoder,
        label: &str,
        targets: &[&TextureView],
    ) -> RenderPass<'a> {
        let attachments: Vec<Option<RenderPassColorAttachment>> = targets
            .iter()
            .map(|view| {
                Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })
            })
            .collect();
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some(label),
            color_attachments: &attachments,
            ..Default::default()
        })
    }

    /// The modern view into the HDR target (or the monster alone, when the
    /// moment shows it): the beam's photons traced
    /// through the blocks and gathered into the transmission map; the
    /// blocks' coverage and depth; the dust; the lit air; then the picture.
    /// Call [`GreetingsRenderer::resize`] first.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        if self.shows_monster {
            let mut pass = Self::pass(encoder, "Part 7 modern monster", &[hdr]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.monster);
            pass.draw(0..3, 0..1);
            return;
        }
        let t = self.targets.as_ref().unwrap();
        let caustics = if self.traces_photons {
            Caustics::Listed
        } else {
            Caustics::Cleared
        };
        self.draw_caustics(encoder, caustics);
        {
            let mut pass = Self::pass(encoder, "Part 7 blocks", &[&t.geometry]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.blocks);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 dust", &[&t.dust_front, &t.dust_full]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &t.scene_inputs, &[]);
            pass.set_pipeline(&self.dust);
            let count = if self.breaks_slab {
                self.grains
            } else {
                self.motes
            };
            pass.draw(0..6, 0..count);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 air", &[&t.air_front, &t.air_full]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &t.scene_inputs, &[]);
            pass.set_pipeline(&self.air);
            pass.draw(0..3, 0..1);
        }
        let mut pass = Self::pass(encoder, "Part 7 modern", &[hdr]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &t.style_inputs, &[]);
        pass.set_pipeline(&self.style);
        pass.draw(0..3, 0..1);
    }

    /// The transmission map, cleared, then (but for [`Caustics::Cleared`])
    /// the beam's photons traced through the blocks and their splats
    /// gathered into it.
    fn draw_caustics(&self, encoder: &mut CommandEncoder, caustics: Caustics) {
        let photons = PHOTONS.div_ceil(64);
        let traced = ("Part 7 photons", &self.trace, &self.photons_out, photons);
        let (listed, blocks) = (&self.splats_listed, PHOTONS.div_ceil(SPLAT_BLOCK));
        let passes = match caustics {
            Caustics::Cleared => vec![],
            Caustics::Listed => vec![
                traced,
                ("Part 7 splats counted", &self.count_splats, listed, blocks),
                ("Part 7 splats listed", &self.list_splats, listed, blocks),
            ],
            #[cfg(test)]
            Caustics::All => vec![traced],
        };
        for (label, pipeline, group, groups) in passes {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, group, &[]);
            pass.dispatch_workgroups(groups, 1, 1);
        }
        let mut pass = Self::pass(encoder, "Part 7 caustics", &[&self.transmission]);
        if caustics == Caustics::Cleared {
            return;
        }
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &self.photons_in, &[]);
        pass.set_pipeline(&self.caustics);
        match caustics {
            Caustics::Listed => pass.draw_indirect(&self.splat_draw, 0),
            #[cfg(test)]
            Caustics::All => pass.draw(0..6, 0..PHOTONS * 2),
            Caustics::Cleared => unreachable!(),
        }
    }

    /// The transmission map at the prepared moment, made `caustics`' way
    /// whatever the renderer would do there, read back (RGBA16F texels,
    /// row by row).
    #[cfg(test)]
    pub(super) fn caustics_map(
        &self,
        device: &Device,
        queue: &Queue,
        caustics: Caustics,
    ) -> Vec<u8> {
        if caustics == Caustics::All {
            // Every splat, in order: the list is every instance.
            let every: Vec<u32> = (0..PHOTONS * 2).collect();
            queue.write_buffer(&self.splats, 0, bytemuck::cast_slice(&every));
        }
        let (width, height) = TRANSMISSION_SIZE;
        let stride = width * 8;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Part 7 transmission readback"),
            size: stride as u64 * height as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        self.draw_caustics(&mut encoder, caustics);
        encoder.copy_texture_to_buffer(
            self.transmission_map.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        device.poll(PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let map = buffer.slice(..).get_mapped_range().unwrap().to_vec();
        buffer.unmap();
        map
    }

    /// Faithful HD into the output.
    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        let mut pass = Self::pass(encoder, "Part 7 faithful HD", &[output]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_pipeline(&self.faithful);
        pass.draw(0..3, 0..1);
    }
}

/// The air's density (authored): a smooth noise that tiles in all three
/// directions, two octaves of value noise on 4- and 8-cell lattices, cubic
/// between their points, NOISE_SIZE texels a side, 0 to 255.
pub fn noise() -> Vec<u8> {
    let n = NOISE_SIZE as usize;
    let hash = |x: usize, y: usize, z: usize, cells: usize, seed: u64| -> f64 {
        let (x, y, z) = (x % cells, y % cells, z % cells);
        let mut h =
            (x as u64 * 73_856_093) ^ (y as u64 * 19_349_663) ^ (z as u64 * 83_492_791) ^ seed;
        h = h.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        h ^= h >> 29;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 32;
        (h >> 11) as f64 / (1u64 << 53) as f64
    };
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let octave = |x: usize, y: usize, z: usize, cells: usize, seed: u64| -> f64 {
        let scale = cells as f64 / n as f64;
        let (fx, fy, fz) = (x as f64 * scale, y as f64 * scale, z as f64 * scale);
        let (ix, iy, iz) = (fx as usize, fy as usize, fz as usize);
        let (tx, ty, tz) = (
            smooth(fx - ix as f64),
            smooth(fy - iy as f64),
            smooth(fz - iz as f64),
        );
        let mut v = 0.0;
        for k in 0..8 {
            let (dx, dy, dz) = (k & 1, (k >> 1) & 1, k >> 2);
            let w = if dx == 1 { tx } else { 1.0 - tx }
                * if dy == 1 { ty } else { 1.0 - ty }
                * if dz == 1 { tz } else { 1.0 - tz };
            v += w * hash(ix + dx, iy + dy, iz + dz, cells, seed);
        }
        v
    };
    let mut out = Vec::with_capacity(n * n * n);
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let v = (octave(x, y, z, 4, 7) * 2.0 + octave(x, y, z, 8, 11)) / 3.0;
                out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::greetings::{
        CAMERA_DISTANCE, ROWS, SLOT_ROWS, SLOTS, STAR_BASE, STRIP_WIDTH, STRIP_X, STRIPS,
        SWEEP_CENTRE,
    };
    use super::super::greetings_type::{ATLAS_COLUMNS, ATLAS_SIZE, CELL, FACE};

    /// The constants the part-7 shaders repeat from hd/greetings.rs,
    /// greetings_type.rs and the scene.
    #[test]
    fn part7_shaders_repeat_the_rust_constants() {
        let common = include_str!("greetings_common.wgsl");
        let (cx, cy) = SWEEP_CENTRE;
        for constant in [
            format!("const STRIPS={STRIPS}u;"),
            format!("const STRIP_WIDTH={STRIP_WIDTH}u;"),
            format!("const ROWS={ROWS}u;"),
            format!("const STAR_BASE={STAR_BASE}u;"),
            format!("const SWEEP_CENTRE=vec2f({cx:.1},{cy:.1});"),
            "const LAW_OFFSET=17.8;".to_owned(),
            "const COLUMN=5.0/6.0;".to_owned(),
        ] {
            assert!(
                common.contains(&constant),
                "greetings_common.wgsl lacks {constant}"
            );
        }
        let light = include_str!("greetings_light.wgsl");
        let (w, h) = ATLAS_SIZE;
        for constant in [
            format!("const CAMERA_DISTANCE={CAMERA_DISTANCE:.1};"),
            format!("const HALF={};", FACE / 2.0),
            format!("const SLOT_ROWS={SLOT_ROWS}.0;"),
            format!("const SLOTS={SLOTS}u;"),
            format!("const CELL={CELL}.0;"),
            format!("const ATLAS_COLUMNS={ATLAS_COLUMNS}u;"),
            format!("const ATLAS_SIZE=vec2f({w}.0,{h}.0);"),
        ] {
            assert!(
                light.contains(&constant),
                "greetings_light.wgsl lacks {constant}"
            );
        }
        // The strips the shaders find at 8 + 64 s.
        for (s, &x) in STRIP_X.iter().enumerate() {
            assert_eq!(x, 8 + 64 * s);
        }
        assert!((ATLAS_COLUMNS * (ATLAS_SIZE.1 as usize / CELL)) >= 26);
        assert_eq!(ATLAS_COLUMNS * CELL, ATLAS_SIZE.0 as usize);
        let (gx, gy) = super::PHOTON_GRID;
        assert!(light.contains(&format!("const PHOTON_GRID=vec2u({gx}u,{gy}u);")));
        // The splats' blocks: a workgroup of 256 threads, SPLAT_RUN photons
        // each.
        let block = super::SPLAT_BLOCK;
        assert!(light.contains(&format!("const SPLAT_BLOCK={block}u;")));
        assert!(light.contains(&format!("const SPLAT_RUN={}u;", block / 256)));
        assert_eq!(
            light.matches("@compute @workgroup_size(256) fn ").count(),
            2,
            "count_splats and list_splats"
        );
        // The map is square, as its spread is the same both ways.
        let (w, h) = super::TRANSMISSION_SIZE;
        assert_eq!(w, h);
        // The lamp stands on the row the controls give (beam_late.y), not
        // on the sweep's centre: placed at LAMP_DEPTH so the camera's
        // projection takes it to that row, 100 less it above the middle.
        let at = light.find("fn lamp(").unwrap();
        let lamp = &light[at..at + light[at..].find("\n}\n").unwrap() + 2];
        assert_eq!(
            lamp.split_whitespace().collect::<String>(),
            "fnlamp()->vec3f{returnvec3f(0.0,(100.0-gc.beam_late.y)\
             *(CAMERA_DISTANCE+LAMP_DEPTH)/CAMERA_DISTANCE,LAMP_DEPTH);}"
        );
        assert!(light.contains(
            "fn project(p:vec3f)->vec2f {return p.xy*CAMERA_DISTANCE/(CAMERA_DISTANCE+p.z);}"
        ));
    }

    /// Part 7's caustics are every photon's: at every frame of the part, and
    /// halfway to the next, the strips' transmission map the renderer makes,
    /// the trace skipped where the photons move no light or the splats that
    /// move light listed and drawn, is byte for byte the one that drawing
    /// every photon's splats makes; and both ways are taken, the list
    /// through the words, where the caustics show.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part7_caustics_are_every_photons_however_they_are_drawn() {
        use super::super::clip::{Clip, ScrollerLighting};
        use super::super::renderer::{Renderer, View};
        use super::Caustics;
        let data = crate::test_support::data();
        let clip = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let greetings = clip.greetings.as_ref().unwrap();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (mut renderer, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
        let (mut cleared, mut listed, mut lit) = (0, 0, 0);
        let mut empty = None;
        for j in 0..greetings.frames.len() {
            for half in [0.0, 0.5] {
                let time = greetings.seconds(j) + half / crate::engine::FRAME_RATE as f64;
                renderer.prepare(
                    &clip,
                    View {
                        time,
                        size: (64, 36),
                        comparison: 0,
                        modern: true,
                        hud: None,
                    },
                );
                let Some(part7) = renderer.part7().filter(|r| !r.shows_monster) else {
                    continue;
                };
                let made = if part7.traces_photons {
                    listed += 1;
                    Caustics::Listed
                } else {
                    cleared += 1;
                    Caustics::Cleared
                };
                let (device, queue) = (&renderer.device, &renderer.queue);
                let every = part7.caustics_map(device, queue, Caustics::All);
                assert!(
                    part7.caustics_map(device, queue, made) == every,
                    "{time:.4} s (frame {j}, {made:?})"
                );
                let empty = empty
                    .get_or_insert_with(|| part7.caustics_map(device, queue, Caustics::Cleared));
                lit += (every != *empty) as usize;
            }
        }
        println!("{cleared} moments cleared, {listed} listed, {lit} with caustics");
        assert!(
            cleared > 100 && listed > 1000,
            "{cleared} cleared, {listed} listed"
        );
        assert!(lit > 500, "{lit} moments with caustics");
    }

    /// The air's density tiles: its opposite faces meet without a seam, and
    /// it varies smoothly between neighbours.
    #[test]
    fn the_airs_density_tiles_smoothly() {
        let n = super::NOISE_SIZE as usize;
        let v = super::noise();
        assert_eq!(v.len(), n * n * n);
        let at = |x: usize, y: usize, z: usize| v[(z % n) * n * n + (y % n) * n + x % n] as i32;
        let mut largest = 0;
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    for (dx, dy, dz) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
                        largest = largest.max((at(x, y, z) - at(x + dx, y + dy, z + dz)).abs());
                    }
                }
            }
        }
        assert!(largest <= 32, "steps of {largest} between neighbours");
        let (lo, hi) = (v.iter().min().unwrap(), v.iter().max().unwrap());
        assert!(*lo < 80 && *hi > 175, "range {lo} to {hi}");
    }
}
