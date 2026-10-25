//! GPU side of part 4: its tables, the slab's relief, the plasma's sheet of
//! light and its levels, and the pipelines. Faithful HD draws straight into
//! the output from the tables; the modern view draws the sheet's light,
//! averages it down level by level, then marches the slab and composites
//! into the modern renderer's HDR target, which then takes the shared bloom
//! and tonemap. In its first moments it mixes in part 3's modern view,
//! which renderer.rs draws into a target of this renderer's.
use super::plasma::{PlasmaPreview, RASTERS};
use super::plasma_stone::Relief;
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("plasma_common.wgsl");
const FAITHFUL: &str = include_str!("plasma_faithful.wgsl");
const LIGHT: &str = include_str!("plasma_light.wgsl");

/// The sheet's light: its texture's size and levels, as plasma_light.wgsl
/// has them (SHEET_SIZE; its cells read levels 6 and 7).
pub const SHEET_SIZE: (u32, u32) = (512, 256);
pub const SHEET_LEVELS: u32 = 8;

/// Faithful HD's module: the shared controls, plasma_common.wgsl and
/// plasma_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, plasma_common.wgsl and
/// plasma_light.wgsl.
pub(super) fn light_source() -> String {
    wgsl_parts(&[COMMON, LIGHT])
}

pub(super) struct PlasmaRenderer {
    /// Identifies the uploaded preview by its palettes.
    key: (usize, usize),
    data: BindGroup,
    original: Texture,
    /// The original frame uploaded to `original`.
    original_frame: Option<usize>,
    sheet_targets: Vec<TextureView>,
    sheet_levels: Vec<BindGroup>,
    light_layout: BindGroupLayout,
    sheet_view: TextureView,
    /// Part 3's modern view for the handover, and the composite's group 1,
    /// for one drawable size.
    targets: Option<(u32, u32, TextureView, BindGroup)>,
    faithful: RenderPipeline,
    sheet: RenderPipeline,
    downsample: RenderPipeline,
    style: RenderPipeline,
}

fn texture(
    device: &Device,
    label: &str,
    (width, height): (u32, u32),
    format: TextureFormat,
    usage: TextureUsages,
    levels: u32,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
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
        1,
    );
    upload(queue, &t, bytes, size, texel);
    t.create_view(&Default::default())
}

fn relief(device: &Device, queue: &Queue, label: &str, r: &Relief) -> TextureView {
    image(
        device,
        queue,
        label,
        r.size,
        TextureFormat::Rgba16Float,
        bytemuck::cast_slice(&r.texels),
        8,
    )
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

const FLOAT: TextureSampleType = TextureSampleType::Float { filterable: true };

impl PlasmaRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        output: TextureFormat,
        preview: &PlasmaPreview,
    ) -> Self {
        let frames = preview.frames.len();
        let buffer = |label, contents: &[u8]| {
            device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: BufferUsages::STORAGE,
            })
        };
        let rasters: Vec<u32> = (0..frames)
            .flat_map(|j| {
                let starts = if j + 1 < frames {
                    preview.rasters(j)
                } else {
                    [0; RASTERS]
                };
                starts
                    .chunks(2)
                    .map(|p| p[0] as u32 | (p[1] as u32) << 16)
                    .collect::<Vec<_>>()
            })
            .collect();
        let rasters = buffer("Part 4 rasters", bytemuck::cast_slice(&rasters));
        let sine: Vec<f32> = preview.tables.sine.iter().map(|&s| s as f32).collect();
        let sine = buffer("Part 4 sine", bytemuck::cast_slice(&sine));
        let indices: Vec<u8> = (0..=255).collect();
        let palettes: Vec<u8> = (0..frames)
            .flat_map(|j| super::field::rgba(&indices, preview.palette(j)))
            .collect();
        let palettes = image(
            device,
            queue,
            "Part 4 palettes",
            (256, frames as u32),
            TextureFormat::Rgba8Unorm,
            &palettes,
            4,
        );
        let bank: Vec<u8> = preview.tables.bank.iter().map(|&v| (v & 63) * 4).collect();
        let bank = image(
            device,
            queue,
            "Part 4 bank",
            (1024, 64),
            TextureFormat::R8Unorm,
            &bank,
            1,
        );
        let logo = image(
            device,
            queue,
            "Part 4 logo",
            (320, 180),
            TextureFormat::R8Uint,
            &preview.tables.logo,
            1,
        );
        let part7 = image(
            device,
            queue,
            "Part 7's first frame",
            (320, 200),
            TextureFormat::Rgba8Unorm,
            &super::field::rgba(&preview.part7.0, &preview.part7.1),
            4,
        );
        let original = texture(
            device,
            "Part 4 original",
            (320, 400),
            TextureFormat::Rgba8Unorm,
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            1,
        );
        let top = relief(device, queue, "Part 4 slab top", &preview.stone.top);
        let front = relief(device, queue, "Part 4 slab front", &preview.stone.front);
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 4"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        });
        let all = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
        let storage = BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let data_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 4 data"),
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
                entry(1, all, storage),
                entry(2, all, storage),
                entry(3, all, sampled(FLOAT)),
                entry(4, all, sampled(FLOAT)),
                entry(5, all, sampled(TextureSampleType::Uint)),
                entry(6, all, sampled(FLOAT)),
                entry(7, all, sampled(FLOAT)),
                entry(8, all, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(9, all, sampled(FLOAT)),
                entry(10, all, sampled(FLOAT)),
            ],
        });
        let original_view = original.create_view(&Default::default());
        fn view(binding: u32, v: &TextureView) -> BindGroupEntry<'_> {
            BindGroupEntry {
                binding,
                resource: BindingResource::TextureView(v),
            }
        }
        let data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 4 data"),
            layout: &data_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: rasters.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: sine.as_entire_binding(),
                },
                view(3, &palettes),
                view(4, &bank),
                view(5, &logo),
                view(6, &part7),
                view(7, &original_view),
                BindGroupEntry {
                    binding: 8,
                    resource: BindingResource::Sampler(&sampler),
                },
                view(9, &top),
                view(10, &front),
            ],
        });
        let hdr = TextureFormat::Rgba16Float;
        let sheet = texture(
            device,
            "Part 4 sheet of light",
            SHEET_SIZE,
            hdr,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            SHEET_LEVELS,
        );
        let level_view = |level| {
            sheet.create_view(&TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let sheet_targets: Vec<TextureView> = (0..SHEET_LEVELS).map(level_view).collect();
        let level_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 4 sheet level"),
            entries: &[entry(3, ShaderStages::FRAGMENT, sampled(FLOAT))],
        });
        let sheet_levels = (1..SHEET_LEVELS)
            .map(|level| {
                device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Part 4 sheet level"),
                    layout: &level_layout,
                    entries: &[view(3, &sheet_targets[level as usize - 1])],
                })
            })
            .collect();
        let light_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 4 light"),
            entries: &[
                entry(1, ShaderStages::FRAGMENT, sampled(FLOAT)),
                entry(2, ShaderStages::FRAGMENT, sampled(FLOAT)),
            ],
        });
        let layout = |groups: &[&BindGroupLayout]| {
            let groups: Vec<Option<&BindGroupLayout>> = groups.iter().map(|&g| Some(g)).collect();
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Part 4"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            })
        };
        let light_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 4 modern"),
            source: ShaderSource::Wgsl(light_source().into()),
        });
        let faithful_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 4 faithful HD"),
            source: ShaderSource::Wgsl(faithful_source().into()),
        });
        let pipeline = |label: &str,
                        layout: &PipelineLayout,
                        module: &ShaderModule,
                        fragment: &str,
                        format: TextureFormat| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: VertexState {
                    module,
                    entry_point: Some("fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let faithful = pipeline(
            "Part 4 faithful HD",
            &layout(&[&data_layout]),
            &faithful_module,
            "faithful",
            output,
        );
        let sheet_pipeline = pipeline(
            "Part 4 sheet of light",
            &layout(&[&data_layout]),
            &light_module,
            "plasma_sheet",
            hdr,
        );
        let downsample = pipeline(
            "Part 4 sheet levels",
            &layout(&[&data_layout, &level_layout]),
            &light_module,
            "plasma_downsample",
            hdr,
        );
        let style = pipeline(
            "Part 4 modern",
            &layout(&[&data_layout, &light_layout]),
            &light_module,
            "plasma_style",
            hdr,
        );
        Self {
            key: key(preview),
            data,
            original,
            original_frame: None,
            sheet_view: sheet.create_view(&Default::default()),
            sheet_targets,
            sheet_levels,
            light_layout,
            targets: None,
            faithful,
            sheet: sheet_pipeline,
            downsample,
            style,
        }
    }

    /// Whether this renderer holds `preview`'s data.
    pub fn holds(&self, preview: &PlasmaPreview) -> bool {
        self.key == key(preview)
    }

    /// Upload the original frame `j` for the comparison modes.
    pub fn show_original(&mut self, queue: &Queue, preview: &PlasmaPreview, j: usize) {
        if self.original_frame == Some(j) {
            return;
        }
        let rgba = preview.original_full(j);
        let rows = (rgba.len() / (320 * 4)) as u32;
        upload(queue, &self.original, &rgba, (320, rows), 4);
        self.original_frame = Some(j);
    }

    /// Make the handover's target for this drawable size.
    pub fn resize(&mut self, device: &Device, size: (u32, u32)) {
        if self.targets.as_ref().is_some_and(|t| (t.0, t.1) == size) {
            return;
        }
        let part3 = texture(
            device,
            "Part 4 handover from part 3",
            size,
            TextureFormat::Rgba16Float,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            1,
        )
        .create_view(&Default::default());
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 4 light"),
            layout: &self.light_layout,
            entries: &[
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.sheet_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&part3),
                },
            ],
        });
        self.targets = Some((size.0, size.1, part3, group));
    }

    /// Where part 3's modern view goes during the handover (HDR). Call
    /// [`PlasmaRenderer::resize`] first.
    pub fn handover(&self) -> &TextureView {
        &self.targets.as_ref().unwrap().2
    }

    fn pass<'a>(
        encoder: &'a mut CommandEncoder,
        label: &str,
        target: &TextureView,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            ..Default::default()
        })
    }

    /// The modern view into the HDR target: the sheet's light and its
    /// levels, then the slab and the composite.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        {
            let mut pass = Self::pass(encoder, "Part 4 sheet of light", &self.sheet_targets[0]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.sheet);
            pass.draw(0..3, 0..1);
        }
        for (level, group) in self.sheet_levels.iter().enumerate() {
            let mut pass = Self::pass(
                encoder,
                "Part 4 sheet level",
                &self.sheet_targets[level + 1],
            );
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, group, &[]);
            pass.set_pipeline(&self.downsample);
            pass.draw(0..3, 0..1);
        }
        let mut pass = Self::pass(encoder, "Part 4 modern", hdr);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &self.targets.as_ref().unwrap().3, &[]);
        pass.set_pipeline(&self.style);
        pass.draw(0..3, 0..1);
    }

    /// Faithful HD into the output.
    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        let mut pass = Self::pass(encoder, "Part 4 faithful HD", output);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_pipeline(&self.faithful);
        pass.draw(0..3, 0..1);
    }
}

fn key(preview: &PlasmaPreview) -> (usize, usize) {
    (preview.palettes.as_ptr() as usize, preview.palettes.len())
}
