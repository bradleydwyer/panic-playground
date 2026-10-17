//! HDR rendering and bloom for the procedural 3D remake.
use wgpu::*;

struct Targets {
    size: (u32, u32),
    scene: TextureView,
    cut_scene: TextureView,
    cut_depth: TextureView,
    glow: [TextureView; 2],
    bindings: [BindGroup; 4],
}

pub struct Modern {
    background: RenderPipeline,
    stars: RenderPipeline,
    model: RenderPipeline,
    tile_cut: RenderPipeline,
    tile_surface: RenderPipeline,
    vapor: RenderPipeline,
    sparks: RenderPipeline,
    blur_x: RenderPipeline,
    blur_y: RenderPipeline,
    composite: RenderPipeline,
    post_layout: BindGroupLayout,
    sampler: Sampler,
    targets: Targets,
}

impl Modern {
    pub fn new(
        device: &Device,
        scene_layout: &BindGroupLayout,
        uniform: &Buffer,
        reference: &Texture,
        output: TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Extruded 3D lettering and HDR lighting"),
            source: ShaderSource::Wgsl(include_str!("modern.wgsl").into()),
        });
        let post = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("HDR bloom and tone mapping"),
            source: ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let cut = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Curled metal tiles, vapor and molten sparks"),
            source: ShaderSource::Wgsl(include_str!("transition.wgsl").into()),
        });
        let texture = |binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let post_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("HDR postprocess inputs"),
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
                texture(1),
                texture(2),
                texture(3),
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline = |module: &ShaderModule,
                        layout: &BindGroupLayout,
                        vertex,
                        fragment,
                        format,
                        blend,
                        depth: Option<bool>| {
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(fragment),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&layout),
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
                    targets: &[Some(ColorTargetState {
                        format,
                        blend,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: depth.map(|write| DepthStencilState {
                    format: TextureFormat::Depth32Float,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let hdr = TextureFormat::Rgba16Float;
        let additive = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent::REPLACE,
        };
        let background = pipeline(
            &shader,
            scene_layout,
            "fullscreen",
            "background",
            hdr,
            None,
            None,
        );
        let stars = pipeline(
            &shader,
            scene_layout,
            "star_vertex",
            "star_fragment",
            hdr,
            Some(additive),
            None,
        );
        let model = pipeline(
            &shader,
            scene_layout,
            "fullscreen",
            "model",
            hdr,
            Some(BlendState::ALPHA_BLENDING),
            None,
        );
        let blur_x = pipeline(&post, &post_layout, "fullscreen", "blur_x", hdr, None, None);
        let tile_cut = pipeline(
            &cut,
            &post_layout,
            "fullscreen",
            "backdrop",
            hdr,
            None,
            None,
        );
        let tile_surface = pipeline(
            &cut,
            &post_layout,
            "tile_vertex",
            "tile_fragment",
            hdr,
            None,
            Some(true),
        );
        let vapor = pipeline(
            &cut,
            &post_layout,
            "vapor_vertex",
            "vapor_fragment",
            hdr,
            Some(BlendState::ALPHA_BLENDING),
            Some(false),
        );
        let sparks = pipeline(
            &cut,
            &post_layout,
            "spark_vertex",
            "spark_fragment",
            hdr,
            Some(additive),
            Some(false),
        );
        let blur_y = pipeline(&post, &post_layout, "fullscreen", "blur_y", hdr, None, None);
        let composite = pipeline(
            &post,
            &post_layout,
            "fullscreen",
            "composite",
            output,
            None,
            None,
        );
        let sampler = device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let targets = Self::targets(device, &post_layout, &sampler, uniform, reference, (1, 1));
        Self {
            background,
            stars,
            model,
            tile_cut,
            tile_surface,
            vapor,
            sparks,
            blur_x,
            blur_y,
            composite,
            post_layout,
            sampler,
            targets,
        }
    }

    fn targets(
        device: &Device,
        layout: &BindGroupLayout,
        sampler: &Sampler,
        uniform: &Buffer,
        reference: &Texture,
        size: (u32, u32),
    ) -> Targets {
        let texture = |label, width, height| {
            device
                .create_texture(&TextureDescriptor {
                    label: Some(label),
                    size: Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba16Float,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let scene = texture("HDR scene", size.0, size.1);
        let cut_scene = texture("Heated tile transition", size.0, size.1);
        let cut_depth = device
            .create_texture(&TextureDescriptor {
                label: Some("Curled tile depth"),
                size: Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Depth32Float,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let glow = [
            texture("Bloom horizontal", size.0.div_ceil(2), size.1.div_ceil(2)),
            texture("Bloom vertical", size.0.div_ceil(2), size.1.div_ceil(2)),
        ];
        let reference = reference.create_view(&Default::default());
        let binding = |source, bloom| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Postprocess textures"),
                layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(source),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::TextureView(bloom),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&reference),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        // Do not bind a render target as an input to its own pass.
        let bindings = [
            binding(&scene, &scene),
            binding(&cut_scene, &cut_scene),
            binding(&glow[0], &glow[0]),
            binding(&cut_scene, &glow[1]),
        ];
        Targets {
            size,
            scene,
            cut_scene,
            cut_depth,
            glow,
            bindings,
        }
    }

    pub fn resize(
        &mut self,
        device: &Device,
        uniform: &Buffer,
        reference: &Texture,
        size: (u32, u32),
    ) {
        if self.targets.size != size {
            self.targets = Self::targets(
                device,
                &self.post_layout,
                &self.sampler,
                uniform,
                reference,
                size,
            );
        }
    }

    pub fn draw(
        &self,
        encoder: &mut CommandEncoder,
        output: &TextureView,
        scene_inputs: &BindGroup,
        transition: bool,
    ) {
        {
            let attachments = [Some(RenderPassColorAttachment {
                view: &self.targets.scene,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Modern 3D scene"),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_bind_group(0, scene_inputs, &[]);
            pass.set_pipeline(&self.background);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.stars);
            pass.draw(0..6, 0..3200);
            pass.set_pipeline(&self.model);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Exposed metal and moving reflected light"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.cut_scene,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.tile_cut);
            pass.set_bind_group(0, &self.targets.bindings[0], &[]);
            pass.draw(0..3, 0..1);
        }
        if transition {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Curled metal patches and cooling debris"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.targets.cut_scene,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.targets.cut_depth,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.targets.bindings[0], &[]);
            pass.set_pipeline(&self.tile_surface);
            // 20x20 front quads, plus four 20-segment cut walls per tile.
            pass.draw(0..2880, 0..160);
            pass.set_pipeline(&self.vapor);
            pass.draw(0..6, 0..960);
            pass.set_pipeline(&self.sparks);
            pass.draw(0..6, 0..7680);
        }
        for (pipeline, target, binding) in [
            (
                &self.blur_x,
                &self.targets.glow[0],
                &self.targets.bindings[1],
            ),
            (
                &self.blur_y,
                &self.targets.glow[1],
                &self.targets.bindings[2],
            ),
            (&self.composite, output, &self.targets.bindings[3]),
        ] {
            let attachments = [Some(RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("HDR postprocess"),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, binding, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
