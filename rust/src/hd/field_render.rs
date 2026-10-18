//! GPU side of part 5: its textures, bind groups, pipelines and the fluid
//! hook. Faithful HD draws straight to the output; the modern view draws
//! into the modern renderer's HDR target, which then takes the shared bloom
//! and tonemap.
//!
//! The fluid hook is the frame author's [`FrameFluid`] (field_frame_smoke.rs):
//! a smoke simulation at a fixed step ([`FLUID`]), deterministic per step,
//! with its own GPU state, kept states for seeking back and lighting. This
//! file brings it to each moment shown and binds its two light layers for
//! the fragments.
use super::field::{
    BUFFER_SIZE, COARSE_CELLS, FieldPreview, FrameFluid, MAP_SIZE, authors, frame_shape,
};
use super::renderer::wgsl_parts;
use wgpu::*;

const COMMON: &str = include_str!("field_common.wgsl");
const ENTRIES: &str = include_str!("field_style.wgsl");
const FAITHFUL: &str = include_str!("field_faithful.wgsl");

/// Faithful HD's module: the shared controls, the common part-5 code and
/// field_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, the common part-5 code, the
/// three authors' files and the shared entry points.
pub(super) fn style_source() -> String {
    let [frame, zoom, mountain] = authors();
    wgsl_parts(&[COMMON, frame.source, zoom.source, mountain.source, ENTRIES])
}

/// Everything group 0 holds but the prepass textures.
struct Inputs {
    layout: BindGroupLayout,
    uniform: Buffer,
    sampler: Sampler,
    views: [TextureView; 9],
    blank: TextureView,
}

impl Inputs {
    fn bind(&self, device: &Device, prepasses: [&TextureView; 3]) -> BindGroup {
        let mut entries = vec![
            BindGroupEntry {
                binding: 0,
                resource: self.uniform.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 9,
                resource: BindingResource::Sampler(&self.sampler),
            },
        ];
        for (binding, view) in [1, 2, 3, 4, 5, 6, 7, 8, 13].into_iter().zip(&self.views) {
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::TextureView(view),
            });
        }
        for (binding, view) in [10, 11, 12].into_iter().zip(prepasses) {
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::TextureView(view),
            });
        }
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 5 inputs"),
            layout: &self.layout,
            entries: &entries,
        })
    }
}

/// Group 0 with the authors' prepass targets, the targets (one per author
/// with a prepass) and their size.
type Prepassed = (BindGroup, [Option<TextureView>; 3], (u32, u32));

pub(super) struct FieldRenderer {
    /// Identifies the uploaded preview by its heights buffer.
    key: (usize, usize),
    palette: Texture,
    palette_key: Option<usize>,
    buffers: [Texture; 2],
    buffers_key: Option<usize>,
    inputs: Inputs,
    /// Group 0 with blank prepass textures, and with the authors' prepass
    /// targets.
    plain: BindGroup,
    prepassed: Option<Prepassed>,
    faithful: RenderPipeline,
    style: RenderPipeline,
    prepasses: [Option<RenderPipeline>; 3],
    fluid: FrameFluid,
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

fn write(queue: &Queue, texture: &Texture, data: &[u8], (width, height): (u32, u32)) {
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
}

impl FieldRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        reference: &Texture,
        output: TextureFormat,
        preview: &FieldPreview,
    ) -> Self {
        let sampled = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        let made = |label, size: (usize, usize), format, data: &[u8]| {
            let size = (size.0 as u32, size.1 as u32);
            let t = texture(device, label, size, format, sampled);
            write(queue, &t, data, size);
            t
        };
        let palette = texture(
            device,
            "Part 5 palette",
            (256, 1),
            TextureFormat::Rgba8Unorm,
            sampled,
        );
        let picture = made(
            "Part 5 picture",
            (320, 200),
            TextureFormat::R8Unorm,
            &preview.picture,
        );
        let buffers = ["Zoom buffer shown", "Zoom buffer next"].map(|label| {
            texture(
                device,
                label,
                (BUFFER_SIZE[0] as u32, BUFFER_SIZE[1] as u32),
                TextureFormat::R8Unorm,
                sampled,
            )
        });
        let terrain: Vec<u8> = preview
            .colors
            .iter()
            .zip(&preview.heights)
            .flat_map(|(&c, &h)| [c, h])
            .collect();
        let terrain = made(
            "Part 5 terrain",
            (MAP_SIZE, MAP_SIZE),
            TextureFormat::Rg8Unorm,
            &terrain,
        );
        let coarse: Vec<u8> = preview
            .coarse
            .iter()
            .zip(&preview.relief.coarse)
            .flat_map(|(&a, &b)| [a, b])
            .collect();
        let coarse = made(
            "Part 5 coarse heights",
            (COARSE_CELLS, COARSE_CELLS),
            TextureFormat::Rg8Unorm,
            &coarse,
        );
        let size = preview.relief.size;
        let relief = made(
            "Part 5 Mandelbrot relief",
            (size, size),
            TextureFormat::Rgba16Float,
            bytemuck::cast_slice(&preview.relief.texels),
        );
        // The frame author's shape, baked from the picture (field_frame.rs).
        let frame = frame_shape(preview);
        let shape = made(
            "Part 5 frame shape",
            frame.size,
            TextureFormat::Rgba16Float,
            bytemuck::cast_slice(&frame.texels),
        );
        let blank = made("No prepass", (1, 1), TextureFormat::Rgba16Float, &[0; 8])
            .create_view(&Default::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 5 fields"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let visible = ShaderStages::FRAGMENT | ShaderStages::COMPUTE;
        let sampled_entry = |binding| BindGroupLayoutEntry {
            binding,
            visibility: visible,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let mut entries = vec![
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | visible,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 9,
                visibility: visible,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ];
        entries.extend([1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13].map(sampled_entry));
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 5 inputs"),
            entries: &entries,
        });
        let fragment_fluid = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 5 fluid, drawn"),
            entries: &[1, 2].map(|binding| BindGroupLayoutEntry {
                visibility: ShaderStages::FRAGMENT,
                ..sampled_entry(binding)
            }),
        });
        let inputs = Inputs {
            layout,
            uniform: uniform.clone(),
            sampler,
            views: [
                &palette,
                &picture,
                &buffers[0],
                &buffers[1],
                &terrain,
                &coarse,
                &relief,
                reference,
                &shape,
            ]
            .map(|t| t.create_view(&Default::default())),
            blank,
        };
        let plain = inputs.bind(device, [&inputs.blank; 3]);
        let layout_of = |groups: &[&BindGroupLayout]| {
            let groups: Vec<Option<&BindGroupLayout>> = groups.iter().map(|&g| Some(g)).collect();
            device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Part 5"),
                bind_group_layouts: &groups,
                immediate_size: 0,
            })
        };
        let drawn = layout_of(&[&inputs.layout, &fragment_fluid]);
        let pipeline = |layout: &PipelineLayout, module: &ShaderModule, fragment: &str, format| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(fragment),
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
        let faithful_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 5 faithful HD"),
            source: ShaderSource::Wgsl(faithful_source().into()),
        });
        let faithful = pipeline(
            &layout_of(&[&inputs.layout]),
            &faithful_module,
            "faithful",
            output,
        );
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Part 5 modern"),
            source: ShaderSource::Wgsl(style_source().into()),
        });
        let hdr = TextureFormat::Rgba16Float;
        let style = pipeline(&drawn, &module, "field_style", hdr);
        let prepasses = authors().map(|author| {
            author.prepass.then(|| {
                pipeline(
                    &drawn,
                    &module,
                    &format!("field_{}_prepass", author.name),
                    hdr,
                )
            })
        });
        let fluid = FrameFluid::new(device, queue, preview, &frame, &relief, &fragment_fluid);
        Self {
            key: key(preview),
            palette,
            palette_key: None,
            buffers,
            buffers_key: None,
            plain,
            inputs,
            prepassed: None,
            faithful,
            style,
            prepasses,
            fluid,
        }
    }

    /// Whether this renderer holds `preview`'s data.
    pub fn holds(&self, preview: &FieldPreview) -> bool {
        self.key == key(preview)
    }

    /// Make the authors' half-resolution prepass targets for this drawable
    /// size, when any author has a prepass.
    pub fn resize(&mut self, device: &Device, size: (u32, u32)) {
        if self.prepasses.iter().all(Option::is_none) {
            return;
        }
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        if self.prepassed.as_ref().is_some_and(|p| p.2 == half) {
            return;
        }
        let targets = self.prepasses.each_ref().map(|p| {
            p.as_ref().map(|_| {
                texture(
                    device,
                    "Part 5 prepass",
                    half,
                    TextureFormat::Rgba16Float,
                    TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                )
                .create_view(&Default::default())
            })
        });
        let views = [0, 1, 2].map(|k| targets[k].as_ref().unwrap_or(&self.inputs.blank));
        self.prepassed = Some((self.inputs.bind(device, views), targets, half));
    }

    /// Upload frame `j`'s palette and zoom buffers, when they change.
    pub fn update(&mut self, queue: &Queue, preview: &FieldPreview, j: usize) {
        let palette = preview.frames[j].palette;
        if self.palette_key != Some(palette) {
            let rgba =
                super::field::rgba(&(0..=255).collect::<Vec<u8>>(), &preview.palettes[palette]);
            write(queue, &self.palette, &rgba, (256, 1));
            self.palette_key = Some(palette);
        }
        let shown = j.clamp(1, preview.last_zoom.max(1));
        if self.buffers_key != Some(shown) {
            let size = (BUFFER_SIZE[0] as u32, BUFFER_SIZE[1] as u32);
            for (texture, data) in self.buffers.iter().zip(preview.zoom_buffers(shown)) {
                write(queue, texture, &data, size);
            }
            self.buffers_key = Some(shown);
        }
    }

    /// Bring the fluid to the moment `time` shows: the state one step past
    /// it, lit. Returns `u.field.fluid`.
    pub fn advance_fluid(
        &mut self,
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        time: f64,
    ) -> [f32; 4] {
        self.fluid.advance(device, queue, preview, time)
    }

    /// A hash of the fluid's latest state, and its number.
    #[cfg(test)]
    pub(super) fn read_fluid(&self, device: &Device, queue: &Queue) -> (u64, usize) {
        self.fluid.read(device, queue)
    }

    /// The fragments' fluid bind group: the smoke's light behind the frame
    /// and in front of it.
    fn fluid_shown(&self) -> &BindGroup {
        self.fluid.drawn()
    }

    fn pass(
        &self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        pipeline: &RenderPipeline,
        inputs: &BindGroup,
        fluid: bool,
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
        if fluid {
            pass.set_bind_group(1, self.fluid_shown(), &[]);
        }
        pass.draw(0..3, 0..1);
    }

    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        self.pass(
            encoder,
            output,
            &self.faithful,
            &self.plain,
            false,
            "Part 5 faithful HD",
        );
    }

    /// Draw the modern view into the HDR target, after the authors'
    /// prepasses, if any.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        let mut inputs = &self.plain;
        if let Some((bound, targets, _)) = &self.prepassed {
            for (pipeline, target) in self.prepasses.iter().zip(targets) {
                if let (Some(pipeline), Some(target)) = (pipeline, target) {
                    self.pass(
                        encoder,
                        target,
                        pipeline,
                        &self.plain,
                        true,
                        "Part 5 prepass",
                    );
                }
            }
            inputs = bound;
        }
        self.pass(encoder, hdr, &self.style, inputs, true, "Part 5 modern");
    }
}

fn key(preview: &FieldPreview) -> (usize, usize) {
    (preview.heights.as_ptr() as usize, preview.heights.len())
}
