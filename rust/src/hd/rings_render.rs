//! GPU side of part 6: its textures, bind group and pipelines. Faithful HD
//! draws straight to the output; a style draws into the modern renderer's
//! HDR target, which then takes the shared bloom and tonemap.
use super::renderer::wgsl_parts;
use super::rings::{RingsPreview, RingsStyle, StyleInfo, SymbolFields};
use wgpu::*;

const COMMON: &str = include_str!("rings_common.wgsl");
const WALL: &str = include_str!("wall.wgsl");
const ENTRIES: &str = include_str!("rings_style.wgsl");
const FAITHFUL: &str = include_str!("rings_faithful.wgsl");

/// Faithful HD's module: the shared controls, the common part-6 code and
/// rings_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// A style's module: the shared controls, the common part-6 code, the bump
/// wall, the style's own file and the shared entry points.
pub(super) fn style_source(style: &StyleInfo) -> String {
    wgsl_parts(&[COMMON, WALL, style.source, ENTRIES])
}

struct StylePipelines {
    color: RenderPipeline,
    prepass: Option<RenderPipeline>,
}

/// Everything a part-6 bind group holds but the prepass texture.
struct Inputs {
    layout: BindGroupLayout,
    sampler: Sampler,
    uniform: Buffer,
    atlas: TextureView,
    light: TextureView,
    authored: TextureView,
    authored_light: TextureView,
    reference: TextureView,
    picture: TextureView,
}

impl Inputs {
    fn bind(&self, device: &Device, prepass: &TextureView) -> BindGroup {
        let view = |binding, view| BindGroupEntry {
            binding,
            resource: BindingResource::TextureView(view),
        };
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 6 inputs"),
            layout: &self.layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                view(1, &self.atlas),
                view(2, &self.light),
                view(3, &self.reference),
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&self.sampler),
                },
                view(5, &self.picture),
                view(6, prepass),
                view(7, &self.authored),
                view(8, &self.authored_light),
            ],
        })
    }
}

pub(super) struct RingsRenderer {
    /// Identifies the uploaded preview by its atlas buffer.
    key: (usize, usize),
    inputs: Inputs,
    /// Inputs with a blank prepass texture, and with the style's prepass
    /// (bind group, target and its size).
    plain: BindGroup,
    prepassed: Option<(BindGroup, TextureView, (u32, u32))>,
    faithful: RenderPipeline,
    styles: Vec<StylePipelines>,
}

impl RingsRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        reference: &Texture,
        output: TextureFormat,
        preview: &RingsPreview,
    ) -> Self {
        let (atlas, light) = upload_fields(device, queue, &preview.symbols, "Part 6 symbols");
        let (authored, authored_light) =
            upload_fields(device, queue, &preview.authored, "Part 6 authored symbols");
        let flat = |label, (width, height), format, data: &[u8]| {
            let texture = device.create_texture(&TextureDescriptor {
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
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                texture.as_image_copy(),
                data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(data.len() as u32 / height),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            texture.create_view(&Default::default())
        };
        let picture = flat(
            "PSTART picture",
            (320, 200),
            TextureFormat::Rgba8Unorm,
            &preview.picture,
        );
        let blank = flat("No prepass", (1, 1), TextureFormat::Rgba16Float, &[0; 8]);
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 6 fields"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        });
        let texture = |binding, view_dimension| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 6 inputs"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture(1, TextureViewDimension::D2Array),
                texture(2, TextureViewDimension::D2Array),
                texture(3, TextureViewDimension::D2),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                texture(5, TextureViewDimension::D2),
                texture(6, TextureViewDimension::D2),
                texture(7, TextureViewDimension::D2Array),
                texture(8, TextureViewDimension::D2Array),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Part 6"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |module: &ShaderModule, fragment: &str, format| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&pipeline_layout),
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
        let faithful_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 6 faithful HD"),
            source: ShaderSource::Wgsl(faithful_source().into()),
        });
        let faithful = pipeline(&faithful_module, "faithful", output);
        let hdr = TextureFormat::Rgba16Float;
        let styles = RingsStyle::ALL
            .iter()
            .map(|style| {
                let info = style.info();
                let module = device.create_shader_module(ShaderModuleDescriptor {
                    label: Some(info.name),
                    source: ShaderSource::Wgsl(style_source(info).into()),
                });
                StylePipelines {
                    color: pipeline(&module, "rings_style", hdr),
                    prepass: info
                        .prepass
                        .then(|| pipeline(&module, "rings_prepass", hdr)),
                }
            })
            .collect();
        let inputs = Inputs {
            layout,
            sampler,
            uniform: uniform.clone(),
            atlas,
            light,
            authored,
            authored_light,
            reference: reference.create_view(&Default::default()),
            picture,
        };
        Self {
            key: key(preview),
            plain: inputs.bind(device, &blank),
            inputs,
            prepassed: None,
            faithful,
            styles,
        }
    }

    /// Whether this renderer holds `preview`'s data.
    pub fn holds(&self, preview: &RingsPreview) -> bool {
        self.key == key(preview)
    }

    /// Make the style's half-resolution prepass target for this drawable
    /// size, when the style has a prepass.
    pub fn resize(&mut self, device: &Device, style: RingsStyle, size: (u32, u32)) {
        if !style.info().prepass {
            return;
        }
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        if self.prepassed.as_ref().is_some_and(|p| p.2 == half) {
            return;
        }
        let target = device
            .create_texture(&TextureDescriptor {
                label: Some("Part 6 style prepass"),
                size: Extent3d {
                    width: half.0,
                    height: half.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        self.prepassed = Some((self.inputs.bind(device, &target), target, half));
    }

    fn pass(
        &self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        pipeline: &RenderPipeline,
        inputs: &BindGroup,
        label: &str,
    ) {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
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
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, inputs, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        self.pass(
            encoder,
            output,
            &self.faithful,
            &self.plain,
            "Part 6 faithful HD",
        );
    }

    /// Draw a style into the HDR target, after its prepass if it has one.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView, style: RingsStyle) {
        let pipelines = &self.styles[style as usize];
        let mut inputs = &self.plain;
        if let (Some(prepass), Some((bound, target, _))) = (&pipelines.prepass, &self.prepassed) {
            self.pass(
                encoder,
                target,
                prepass,
                &self.plain,
                "Part 6 style prepass",
            );
            inputs = bound;
        }
        self.pass(encoder, hdr, &pipelines.color, inputs, style.name());
    }
}

/// Upload a set of symbol fields: the atlas, with as many channels as its
/// detail keeps, and the light map with its mip chain, one array layer per
/// mask each.
fn upload_fields(
    device: &Device,
    queue: &Queue,
    fields: &SymbolFields,
    label: &str,
) -> (TextureView, TextureView) {
    let layers = fields.layers as u32;
    let (tw, th) = fields.texels();
    let channels = fields.detail.channels() as u32;
    let array = |texture: &Texture| {
        texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        })
    };
    let atlas = device.create_texture(&TextureDescriptor {
        label: Some(&format!("{label}: fields")),
        size: Extent3d {
            width: tw,
            height: th,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: if channels == 4 {
            TextureFormat::Rgba16Float
        } else {
            TextureFormat::Rg16Float
        },
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        atlas.as_image_copy(),
        bytemuck::cast_slice(&fields.atlas),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(tw * channels * 2),
            rows_per_image: Some(th),
        },
        Extent3d {
            width: tw,
            height: th,
            depth_or_array_layers: layers,
        },
    );
    let [lw, lh] = fields.detail.light_size();
    let light = device.create_texture(&TextureDescriptor {
        label: Some(&format!("{label}: neon light map")),
        size: Extent3d {
            width: lw,
            height: lh,
            depth_or_array_layers: layers,
        },
        mip_level_count: fields.light.len() as u32,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R16Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, texels) in fields.light.iter().enumerate() {
        let (w, h) = ((lw >> level).max(1), (lh >> level).max(1));
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &light,
                mip_level: level as u32,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(texels),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 2),
                rows_per_image: Some(h),
            },
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: layers,
            },
        );
    }
    (array(&atlas), array(&light))
}

fn key(preview: &RingsPreview) -> (usize, usize) {
    let atlas = &preview.symbols.atlas;
    (atlas.as_ptr() as usize, atlas.len())
}
