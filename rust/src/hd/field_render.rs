//! GPU side of part 5: its textures, bind groups, pipelines and the fluid
//! hook. Faithful HD draws straight to the output; the modern view draws
//! into the modern renderer's HDR target, which then takes the shared bloom
//! and tonemap.
//!
//! The fluid hook runs the frame author's `frame_fluid` as a compute pass at
//! a fixed step ([`FLUID`]): state n is the fluid at its start plus n steps,
//! each step reading the state before it and the part-5 block at its own
//! start. A moment shows the state after it and the one before, blended, so
//! the fluid is always one step ahead. Every step depends only on the one
//! before, so playing on, seeking and exporting a single frame all reach the
//! same state; seeking back starts again from the nearest state kept every
//! `FLUID.checkpoint` steps, or from the start.
use super::field::{
    BUFFER_SIZE, COARSE_CELLS, FLUID, FieldBlock, FieldPreview, FluidStart, MAP_SIZE, authors,
    frame_shape,
};
use super::renderer::wgsl_parts;
use bytemuck::{Pod, Zeroable};
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

/// One fluid step's controls, as field_common.wgsl's `FluidStep`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct FluidStep {
    pub block: FieldBlock,
    /// Step number, fluid seconds at its start, seconds per step, 1 for the
    /// initial state.
    pub step: [f32; 4],
}

/// Bytes between the steps' controls in their buffer (a multiple of the
/// 256-byte dynamic offset alignment), and the steps one submission holds.
const STEP_STRIDE: u64 = 512;
const STEP_SLOTS: usize = 256;

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

/// The fluid's state textures (ping-pong: state n lives in texture n % 2),
/// their bind groups, the steps' controls and the kept states.
struct Fluid {
    pipeline: ComputePipeline,
    textures: [Texture; 2],
    /// Fragment bind groups: texture k latest, the other previous.
    draw: [BindGroup; 2],
    /// Compute bind groups: read texture k, write the other.
    step: [BindGroup; 2],
    steps: Buffer,
    kept: Texture,
    filled: Vec<bool>,
    /// The state the latest texture holds, once the first has been made.
    done: Option<usize>,
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
    fluid: Fluid,
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
        let shape = frame_shape(preview);
        let shape = made(
            "Part 5 frame shape",
            shape.size,
            TextureFormat::Rgba16Float,
            bytemuck::cast_slice(&shape.texels),
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
        let compute_fluid = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 5 fluid step"),
            entries: &[
                BindGroupLayoutEntry {
                    visibility: ShaderStages::COMPUTE,
                    ..sampled_entry(1)
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<FluidStep>() as u64,
                        ),
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba16Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
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
        let fluid_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Part 5 fluid step"),
            layout: Some(&layout_of(&[&inputs.layout, &compute_fluid])),
            module: &module,
            entry_point: Some("field_fluid"),
            compilation_options: Default::default(),
            cache: None,
        });
        let [width, height] = FLUID.size;
        let states = ["Part 5 fluid A", "Part 5 fluid B"].map(|label| {
            texture(
                device,
                label,
                (width, height),
                TextureFormat::Rgba16Float,
                TextureUsages::TEXTURE_BINDING
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC
                    | TextureUsages::COPY_DST,
            )
        });
        let views = states
            .each_ref()
            .map(|t| t.create_view(&Default::default()));
        let steps = device.create_buffer(&BufferDescriptor {
            label: Some("Part 5 fluid steps"),
            size: STEP_STRIDE * STEP_SLOTS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let draw = [0, 1].map(|k| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Part 5 fluid, drawn"),
                layout: &fragment_fluid,
                entries: &[
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&views[k]),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(&views[1 - k]),
                    },
                ],
            })
        });
        let step = [0, 1].map(|k| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Part 5 fluid step"),
                layout: &compute_fluid,
                entries: &[
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&views[k]),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &steps,
                            offset: 0,
                            size: std::num::NonZeroU64::new(
                                std::mem::size_of::<FluidStep>() as u64,
                            ),
                        }),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(&views[1 - k]),
                    },
                ],
            })
        });
        // The states kept for seeking back, one layer per checkpoint from the
        // fluid's start to the end of part 5.
        let span = preview.seconds(preview.frames.len()) - fluid_start(preview);
        let layers = (span * FLUID.rate) as usize / FLUID.checkpoint.max(1) + 2;
        let kept = device.create_texture(&TextureDescriptor {
            label: Some("Part 5 fluid checkpoints"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: layers as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
            view_formats: &[],
        });
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
            fluid: Fluid {
                pipeline: fluid_pipeline,
                textures: states,
                draw,
                step,
                steps,
                kept,
                filled: vec![false; layers],
                done: None,
            },
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
    /// it, with the one before. Returns `u.field.fluid`.
    pub fn advance_fluid(
        &mut self,
        device: &Device,
        queue: &Queue,
        preview: &FieldPreview,
        time: f64,
    ) -> [f32; 4] {
        let dt = 1.0 / FLUID.rate;
        let start = fluid_start(preview);
        let elapsed = time - start;
        if elapsed < 0.0 {
            return [0.0, 0.0, dt as f32, elapsed as f32];
        }
        let position = elapsed * FLUID.rate;
        let target = position.floor() as usize + 1;
        let every = FLUID.checkpoint.max(1);
        let fluid = &mut self.fluid;
        let mut encoder = device.create_command_encoder(&Default::default());
        // Seeking back: the latest kept state at or before the one before the
        // target, or the start.
        if fluid.done.is_none_or(|done| done > target) {
            let wanted = ((target - 1) / every).min(fluid.filled.len() - 1);
            fluid.done = None;
            if let Some(k) = (0..=wanted).rev().find(|&k| fluid.filled[k]) {
                let n = k * every;
                copy_layer(
                    &mut encoder,
                    (&fluid.kept, k as u32),
                    (&fluid.textures[n % 2], 0),
                );
                fluid.done = Some(n);
            }
        }
        let groups = FLUID.size.map(|s| s.div_ceil(8));
        let first = fluid.done.map_or(0, |done| done + 1);
        let mut slot = 0;
        for n in first..=target {
            if slot == STEP_SLOTS {
                queue.submit([encoder.finish()]);
                encoder = device.create_command_encoder(&Default::default());
                slot = 0;
            }
            // State n is made by the step from state n - 1, with the block at
            // that step's start; state 0 is the first.
            let from = n.saturating_sub(1);
            let t = from as f64 * dt;
            let mut block = preview.controls(start + t);
            block.fluid = [from as f32, 0.0, dt as f32, t as f32];
            let step = FluidStep {
                block,
                step: [
                    from as f32,
                    t as f32,
                    dt as f32,
                    if n == 0 { 1.0 } else { 0.0 },
                ],
            };
            let offset = slot as u64 * STEP_STRIDE;
            queue.write_buffer(&fluid.steps, offset, bytemuck::bytes_of(&step));
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Part 5 fluid step"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&fluid.pipeline);
                pass.set_bind_group(0, &self.plain, &[]);
                // Writes texture n % 2, reading the other.
                pass.set_bind_group(1, &fluid.step[(n + 1) % 2], &[offset as u32]);
                pass.dispatch_workgroups(groups[0], groups[1], 1);
            }
            slot += 1;
            fluid.done = Some(n);
            let k = n / every;
            if n % every == 0 && k < fluid.filled.len() && !fluid.filled[k] {
                copy_layer(
                    &mut encoder,
                    (&fluid.textures[n % 2], 0),
                    (&fluid.kept, k as u32),
                );
                fluid.filled[k] = true;
            }
        }
        queue.submit([encoder.finish()]);
        [
            target as f32,
            (position - (target - 1) as f64) as f32,
            dt as f32,
            elapsed as f32,
        ]
    }

    /// The latest fluid state and the one before, as Rgba16Float bytes.
    #[cfg(test)]
    pub(super) fn read_fluid(&self, device: &Device, queue: &Queue) -> [Vec<u8>; 2] {
        let [width, height] = FLUID.size;
        let row = (width * 8).div_ceil(256) * 256;
        let latest = self.fluid.done.unwrap_or(0) % 2;
        [latest, 1 - latest].map(|k| {
            let buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Part 5 fluid readback"),
                size: (row * height) as u64,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                self.fluid.textures[k].as_image_copy(),
                TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(row),
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
            buffer.slice(..).map_async(MapMode::Read, |_| {});
            device.poll(PollType::wait_indefinitely()).unwrap();
            let mapped = buffer.slice(..).get_mapped_range().unwrap();
            mapped
                .chunks(row as usize)
                .flat_map(|r| r[..width as usize * 8].to_vec())
                .collect()
        })
    }

    /// The fragments' fluid bind group: the latest state and the one before.
    fn fluid_shown(&self) -> &BindGroup {
        &self.fluid.draw[self.fluid.done.unwrap_or(0) % 2]
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

/// Copy one layer of a fluid state texture to another.
fn copy_layer(encoder: &mut CommandEncoder, from: (&Texture, u32), to: (&Texture, u32)) {
    fn at((texture, layer): (&Texture, u32)) -> TexelCopyTextureInfo<'_> {
        TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: TextureAspect::All,
        }
    }
    encoder.copy_texture_to_texture(
        at(from),
        at(to),
        Extent3d {
            width: FLUID.size[0],
            height: FLUID.size[1],
            depth_or_array_layers: 1,
        },
    );
}

/// When the fluid starts, in clip seconds.
fn fluid_start(preview: &FieldPreview) -> f64 {
    match FLUID.start {
        FluidStart::Part5 => preview.seconds(0),
        FluidStart::PictureFade => preview.picture_fade.unwrap_or(preview.seconds(0)),
    }
}

fn key(preview: &FieldPreview) -> (usize, usize) {
    (preview.heights.as_ptr() as usize, preview.heights.len())
}
