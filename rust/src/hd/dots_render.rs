//! GPU side of part 3: its tables, the samples the modern view's lights
//! take along their trails, its targets and pipelines. Faithful HD draws
//! the shade discs and the dots' echoes as marks, then composites them in
//! the original's palettes into the output; the modern view computes its
//! samples, paints the light, lights the haze and composites into the
//! modern renderer's HDR target, which then takes the shared bloom and
//! tonemap. During the hold and the zoom both draw part 5's own view first,
//! into an underlay of their own (renderer.rs), at the zoom's
//! magnification (`FieldBlock::part3_view`).
use super::dots::{DotsPreview, POINTS, SHADE_DISCS};
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("dots_common.wgsl");
const FAITHFUL: &str = include_str!("dots_faithful.wgsl");
const LIGHT: &str = include_str!("dots_light.wgsl");

/// Samples along each light's trail and its afterglow and each shade
/// tip's, as dots_light.wgsl has them (TRAIL_SAMPLES + AFTER_SAMPLES and
/// SHADE_SAMPLES).
pub const TRAIL_SAMPLES: u32 = 64;
pub const AFTER_SAMPLES: u32 = 40;
pub const SHADE_SAMPLES: u32 = 64;
/// Iterations the faithful shade marks cover: the shown frame's 16 and the
/// next frame's newest.
const SHADE_SLOTS: u32 = 17;
/// The fog's light grid and noise, as dots_light.wgsl has them
/// (FOG_COLUMNS, FOG_ROWS, FOG_SLICES and NOISE_SIZE).
pub const FOG_GRID: [u32; 3] = [96, 54, 48];
pub const NOISE_SIZE: u32 = 128;

/// Faithful HD's module: the shared controls, dots_common.wgsl and
/// dots_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, dots_common.wgsl and
/// dots_light.wgsl.
pub(super) fn light_source() -> String {
    wgsl_parts(&[COMMON, LIGHT])
}

/// Samples in the buffer: each light's trail, each shade tip's, then the
/// lights the haze sees.
fn sample_count() -> u32 {
    let points = POINTS as u32;
    let discs = SHADE_DISCS as u32;
    points * (TRAIL_SAMPLES + AFTER_SAMPLES + 1) + discs * (SHADE_SAMPLES + 1) + points + discs
}

/// Bytes of one sample (two vec4f).
const SAMPLE_BYTES: u64 = 32;

/// The targets for one drawable size.
struct Targets {
    size: (u32, u32),
    /// Part 5's modern view during the hold and the zoom (HDR), and its
    /// faithful HD (the output's format).
    underlay_hdr: TextureView,
    underlay_out: TextureView,
    /// The light painted (rgb) and the shade count (a), and the haze at half
    /// resolution.
    painted: TextureView,
    haze: TextureView,
    /// Faithful HD's marks: the shade count and the echo level.
    count: TextureView,
    level: TextureView,
    /// Group 2 of the modern composite and group 1 of faithful HD's.
    light_images: BindGroup,
    faithful_images: BindGroup,
}

pub(super) struct DotsRenderer {
    data: BindGroup,
    samples_write: BindGroup,
    samples_read: BindGroup,
    images_layout: BindGroupLayout,
    output: TextureFormat,
    targets: Option<Targets>,
    samples: ComputePipeline,
    fog_bind: BindGroup,
    fog_light: ComputePipeline,
    trails: RenderPipeline,
    shades: RenderPipeline,
    haze: RenderPipeline,
    style: RenderPipeline,
    marks_shade: RenderPipeline,
    marks_dot: RenderPipeline,
    faithful: RenderPipeline,
}

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

fn image(
    device: &Device,
    queue: &Queue,
    label: &str,
    size: (u32, u32),
    rgba: &[u8],
) -> TextureView {
    let t = texture(
        device,
        label,
        size,
        TextureFormat::Rgba8Unorm,
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    );
    queue.write_texture(
        t.as_image_copy(),
        rgba,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.0 * 4),
            rows_per_image: Some(size.1),
        },
        Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
    );
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

fn sampled() -> BindingType {
    BindingType::Texture {
        sample_type: TextureSampleType::Float { filterable: true },
        view_dimension: TextureViewDimension::D2,
        multisampled: false,
    }
}

fn volume() -> BindingType {
    BindingType::Texture {
        sample_type: TextureSampleType::Float { filterable: true },
        view_dimension: TextureViewDimension::D3,
        multisampled: false,
    }
}

fn written_volume() -> BindingType {
    BindingType::StorageTexture {
        access: StorageTextureAccess::WriteOnly,
        format: TextureFormat::Rgba16Float,
        view_dimension: TextureViewDimension::D3,
    }
}

fn volume_texture(device: &Device, label: &str, [w, h, d]: [u32; 3]) -> TextureView {
    device
        .create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: d,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn storage(read_only: bool) -> BindingType {
    BindingType::Buffer {
        ty: BufferBindingType::Storage { read_only },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

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
const MAX: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Max,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Max,
    },
};

impl DotsRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        reference: &Texture,
        output: TextureFormat,
        preview: &DotsPreview,
    ) -> Self {
        let buffer = |label, contents: &[u8]| {
            device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: BufferUsages::STORAGE,
            })
        };
        let dot_frames = buffer(
            "Part 3 dot frames",
            bytemuck::cast_slice(&preview.dot_frames),
        );
        let objects: Vec<[f32; 4]> = preview
            .tables
            .objects
            .iter()
            .flat_map(|o| {
                o.iter()
                    .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, 0.0])
            })
            .collect();
        let objects = buffer("Part 3 objects", bytemuck::cast_slice(&objects));
        let points: Vec<[u32; 2]> = preview
            .points
            .iter()
            .map(|p| {
                [
                    (p[0] as u16 as u32) | ((p[1] as u16 as u32) << 16),
                    p[2] as i32 as u32,
                ]
            })
            .collect();
        let points = buffer("Part 3 port points", bytemuck::cast_slice(&points));
        let path: Vec<[f32; 2]> = preview.tables.path[..1024]
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32])
            .collect();
        let path = buffer("Part 3 shade path", bytemuck::cast_slice(&path));
        let indices: Vec<u8> = (0..=255).collect();
        let palette = image(
            device,
            queue,
            "Part 3 shade palette",
            (256, 1),
            &super::field::rgba(&indices, &preview.tables.shade_palette),
        );
        let rows = (preview.slab.0.len() / 320) as u32;
        let slab = image(
            device,
            queue,
            "Part 4's first frame",
            (320, rows),
            &super::field::rgba(&preview.slab.0, &preview.slab.1),
        );
        let reference = reference.create_view(&Default::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 3"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let all = ShaderStages::VERTEX | ShaderStages::FRAGMENT | ShaderStages::COMPUTE;
        let data_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 data"),
            entries: &[
                entry(
                    0,
                    all,
                    BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(1, all, storage(true)),
                entry(2, all, storage(true)),
                entry(3, all, storage(true)),
                entry(4, all, storage(true)),
                entry(5, all, sampled()),
                entry(6, all, sampled()),
                entry(7, all, sampled()),
                entry(8, all, BindingType::Sampler(SamplerBindingType::Filtering)),
            ],
        });
        let data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 3 data"),
            layout: &data_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: dot_frames.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: objects.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: points.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: path.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::TextureView(&palette),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: BindingResource::TextureView(&reference),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::TextureView(&slab),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });
        let samples = device.create_buffer(&BufferDescriptor {
            label: Some("Part 3 samples"),
            size: sample_count() as u64 * SAMPLE_BYTES,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let write_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 samples, written"),
            entries: &[entry(1, ShaderStages::COMPUTE, storage(false))],
        });
        let read_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 samples and fog, read"),
            entries: &[
                entry(
                    2,
                    ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    storage(true),
                ),
                entry(3, ShaderStages::FRAGMENT, volume()),
                entry(4, ShaderStages::FRAGMENT, volume()),
                entry(
                    5,
                    ShaderStages::FRAGMENT,
                    BindingType::Sampler(SamplerBindingType::Filtering),
                ),
            ],
        });
        let fog_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 fog light, written"),
            entries: &[
                entry(2, ShaderStages::COMPUTE, storage(true)),
                entry(6, ShaderStages::COMPUTE, written_volume()),
            ],
        });
        let noise_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 fog noise, written"),
            entries: &[entry(7, ShaderStages::COMPUTE, written_volume())],
        });
        let fog_light = volume_texture(device, "Part 3 fog light", FOG_GRID);
        let fog_noise = volume_texture(device, "Part 3 fog noise", [NOISE_SIZE; 3]);
        let repeat = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 3 fog noise"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let samples_write = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 3 samples, written"),
            layout: &write_layout,
            entries: &[BindGroupEntry {
                binding: 1,
                resource: samples.as_entire_binding(),
            }],
        });
        let samples_read = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 3 samples and fog, read"),
            layout: &read_layout,
            entries: &[
                BindGroupEntry {
                    binding: 2,
                    resource: samples.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&fog_light),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&fog_noise),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::Sampler(&repeat),
                },
            ],
        });
        let fog_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 3 fog light, written"),
            layout: &fog_layout,
            entries: &[
                BindGroupEntry {
                    binding: 2,
                    resource: samples.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: BindingResource::TextureView(&fog_light),
                },
            ],
        });
        let noise_bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 3 fog noise, written"),
            layout: &noise_layout,
            entries: &[BindGroupEntry {
                binding: 7,
                resource: BindingResource::TextureView(&fog_noise),
            }],
        });
        let images_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 3 images"),
            entries: &[1, 2, 3].map(|b| entry(b, ShaderStages::FRAGMENT, sampled())),
        });
        let layout = |groups: &[&BindGroupLayout]| {
            let groups: Vec<Option<&BindGroupLayout>> = groups.iter().map(|&g| Some(g)).collect();
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Part 3"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            })
        };
        let light_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 3 modern"),
            source: ShaderSource::Wgsl(light_source().into()),
        });
        let faithful_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 3 faithful HD"),
            source: ShaderSource::Wgsl(faithful_source().into()),
        });
        let hdr = TextureFormat::Rgba16Float;
        let marks = TextureFormat::R16Float;
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
        let painting = layout(&[&data_layout, &read_layout]);
        let trails = pipeline(
            "Part 3 trails",
            &painting,
            &light_module,
            "dots_trail",
            "swept",
            &[target(hdr, Some(ADD))],
        );
        let shades = pipeline(
            "Part 3 shade bobs",
            &painting,
            &light_module,
            "shade_trail",
            "swept",
            &[target(hdr, Some(ADD))],
        );
        let haze = pipeline(
            "Part 3 haze",
            &painting,
            &light_module,
            "fullscreen",
            "dots_haze",
            &[target(hdr, None)],
        );
        let style = pipeline(
            "Part 3 modern",
            &layout(&[&data_layout, &read_layout, &images_layout]),
            &light_module,
            "fullscreen",
            "dots_style",
            &[target(hdr, None)],
        );
        let compute = |label, groups: &[&BindGroupLayout], entry_point| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout(groups)),
                module: &light_module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let samples_pipeline = compute(
            "Part 3 samples",
            &[&data_layout, &write_layout],
            "dots_samples",
        );
        let fog_light_pipeline = compute(
            "Part 3 fog light",
            &[&data_layout, &fog_layout],
            "dots_fog_light",
        );
        // The fog's noise, made once.
        let noise_pipeline = compute(
            "Part 3 fog noise",
            &[&data_layout, &noise_layout],
            "dots_fog_noise",
        );
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&noise_pipeline);
            pass.set_bind_group(0, &data, &[]);
            pass.set_bind_group(1, &noise_bind, &[]);
            pass.dispatch_workgroups(NOISE_SIZE / 4, NOISE_SIZE / 4, NOISE_SIZE / 4);
        }
        queue.submit([encoder.finish()]);
        let marking = layout(&[&data_layout]);
        let marks_shade = pipeline(
            "Part 3 faithful shade discs",
            &marking,
            &faithful_module,
            "faithful_shade",
            "faithful_shade_mark",
            &[target(marks, Some(ADD)), target(marks, Some(MAX))],
        );
        let marks_dot = pipeline(
            "Part 3 faithful dots",
            &marking,
            &faithful_module,
            "faithful_dot",
            "faithful_dot_mark",
            &[target(marks, Some(ADD)), target(marks, Some(MAX))],
        );
        let faithful = pipeline(
            "Part 3 faithful HD",
            &layout(&[&data_layout, &images_layout]),
            &faithful_module,
            "fullscreen",
            "faithful",
            &[target(output, None)],
        );
        Self {
            data,
            samples_write,
            samples_read,
            images_layout,
            output,
            targets: None,
            samples: samples_pipeline,
            fog_bind,
            fog_light: fog_light_pipeline,
            trails,
            shades,
            haze,
            style,
            marks_shade,
            marks_dot,
            faithful,
        }
    }

    /// Make the targets for this drawable size.
    pub fn resize(&mut self, device: &Device, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let drawn = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
        let view = |label, size, format| {
            texture(device, label, size, format, drawn).create_view(&Default::default())
        };
        let hdr = TextureFormat::Rgba16Float;
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        let underlay_hdr = view("Part 3 underlay", size, hdr);
        let underlay_out = view("Part 3 faithful underlay", size, self.output);
        let painted = view("Part 3 light painted", size, hdr);
        let haze = view("Part 3 haze", half, hdr);
        let count = view("Part 3 shade count", size, TextureFormat::R16Float);
        let level = view("Part 3 echo level", size, TextureFormat::R16Float);
        let images = |views: [&TextureView; 3]| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Part 3 images"),
                layout: &self.images_layout,
                entries: &[1, 2, 3].map(|b| BindGroupEntry {
                    binding: b,
                    resource: BindingResource::TextureView(views[b as usize - 1]),
                }),
            })
        };
        let light_images = images([&underlay_hdr, &painted, &haze]);
        let faithful_images = images([&count, &level, &underlay_out]);
        self.targets = Some(Targets {
            size,
            underlay_hdr,
            underlay_out,
            painted,
            haze,
            count,
            level,
            light_images,
            faithful_images,
        });
    }

    /// Where part 5's view goes during the hold and the zoom: its modern
    /// view (HDR) and its faithful HD (the output's format). Call
    /// [`DotsRenderer::resize`] first.
    pub fn underlay_hdr(&self) -> &TextureView {
        &self.targets.as_ref().unwrap().underlay_hdr
    }
    pub fn underlay_out(&self) -> &TextureView {
        &self.targets.as_ref().unwrap().underlay_out
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
                        load: LoadOp::Clear(Color::TRANSPARENT),
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

    /// The modern view into the HDR target, after part 5's underlay if the
    /// moment shows it.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        let targets = self.targets.as_ref().unwrap();
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Part 3 samples"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.samples);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &self.samples_write, &[]);
            pass.dispatch_workgroups(sample_count().div_ceil(64), 1, 1);
            pass.set_pipeline(&self.fog_light);
            pass.set_bind_group(1, &self.fog_bind, &[]);
            let [w, h, d] = FOG_GRID;
            pass.dispatch_workgroups(w.div_ceil(4), h.div_ceil(4), d.div_ceil(4));
        }
        {
            let mut pass = Self::pass(encoder, "Part 3 light painted", &[&targets.painted]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &self.samples_read, &[]);
            pass.set_pipeline(&self.trails);
            pass.draw(0..6, 0..POINTS as u32 * (TRAIL_SAMPLES + AFTER_SAMPLES));
            pass.set_pipeline(&self.shades);
            pass.draw(0..6, 0..2 * SHADE_DISCS as u32 * SHADE_SAMPLES);
        }
        {
            let mut pass = Self::pass(encoder, "Part 3 haze", &[&targets.haze]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &self.samples_read, &[]);
            pass.set_pipeline(&self.haze);
            pass.draw(0..3, 0..1);
        }
        let mut pass = Self::pass(encoder, "Part 3 modern", &[hdr]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &self.samples_read, &[]);
        pass.set_bind_group(2, &targets.light_images, &[]);
        pass.set_pipeline(&self.style);
        pass.draw(0..3, 0..1);
    }

    /// Faithful HD into the output, after part 5's underlay if the moment
    /// shows it.
    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        let targets = self.targets.as_ref().unwrap();
        {
            let mut pass = Self::pass(encoder, "Part 3 marks", &[&targets.count, &targets.level]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.marks_shade);
            pass.draw(0..6, 0..SHADE_SLOTS * SHADE_DISCS as u32);
            pass.set_pipeline(&self.marks_dot);
            pass.draw(0..6, 0..8 * POINTS as u32);
        }
        let mut pass = Self::pass(encoder, "Part 3 faithful HD", &[output]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &targets.faithful_images, &[]);
        pass.set_pipeline(&self.faithful);
        pass.draw(0..3, 0..1);
    }
}
