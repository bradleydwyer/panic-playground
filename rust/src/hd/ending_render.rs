//! GPU side of part 8: its pictures, palettes and halation image, and the
//! two pipelines. Faithful HD draws straight into the output; the modern
//! view draws the television into the modern renderer's HDR target, which
//! then takes the shared bloom and tonemap, as part 7's does.
use super::clip::vga_linear;
use super::ending::{EndingPreview, halation};
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("ending_common.wgsl");
const FAITHFUL: &str = include_str!("ending_faithful.wgsl");
const CRT: &str = include_str!("ending_crt.wgsl");

/// Faithful HD's module: the shared controls, ending_common.wgsl and
/// ending_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, ending_common.wgsl and
/// ending_crt.wgsl.
pub(super) fn crt_source() -> String {
    wgsl_parts(&[COMMON, CRT])
}

pub(super) struct EndingRenderer {
    /// Identifies the uploaded preview by its palettes.
    key: (usize, usize),
    data: BindGroup,
    original: Texture,
    /// The original frame uploaded to `original`.
    original_frame: Option<usize>,
    faithful: RenderPipeline,
    crt: RenderPipeline,
}

const HDR: TextureFormat = TextureFormat::Rgba16Float;

fn image(
    device: &Device,
    queue: &Queue,
    label: &str,
    (width, height): (u32, u32),
    format: TextureFormat,
    bytes: &[u8],
    texel: u32,
) -> Texture {
    device.create_texture_with_data(
        queue,
        &TextureDescriptor {
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
        },
        util::TextureDataOrder::LayerMajor,
        &bytes[..(width * height * texel) as usize],
    )
}

impl EndingRenderer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        uniform: &Buffer,
        output: TextureFormat,
        preview: &EndingPreview,
    ) -> Self {
        let frames = preview.frames.len();
        let buffer = |label, contents: &[u8]| {
            device.create_buffer_init(&util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: BufferUsages::STORAGE,
            })
        };
        let words: Vec<u32> = preview
            .images
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<u8>>()
            .chunks(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let images = buffer("Part 8 pictures", bytemuck::cast_slice(&words));
        let colours: Vec<[f32; 4]> = (0..frames)
            .flat_map(|j| {
                preview
                    .palette(j)
                    .chunks(3)
                    .map(|p| [vga_linear(p[0]), vga_linear(p[1]), vga_linear(p[2]), 1.0])
                    .collect::<Vec<_>>()
            })
            .collect();
        let colours = buffer("Part 8 palettes, linear", bytemuck::cast_slice(&colours));
        let profile = buffer("Part 8 columns", bytemuck::cast_slice(&preview.profile));
        let indices: Vec<u8> = (0..=255).collect();
        let palettes: Vec<u8> = (0..frames)
            .flat_map(|j| super::field::rgba(&indices, preview.palette(j)))
            .collect();
        let palettes = image(
            device,
            queue,
            "Part 8 palettes",
            (256, frames as u32),
            TextureFormat::Rgba8Unorm,
            &palettes,
            4,
        );
        let monster = image(
            device,
            queue,
            "Part 8 monster",
            (320, 200),
            TextureFormat::R8Uint,
            &preview.monster,
            1,
        );
        let glow = image(
            device,
            queue,
            "Part 8 halation",
            (320, 200),
            HDR,
            bytemuck::cast_slice(&halation(&preview.monster, preview.palette(0))),
            8,
        );
        let original = device.create_texture(&TextureDescriptor {
            label: Some("Part 8 original"),
            size: Extent3d {
                width: 320,
                height: 200,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 8"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        let all = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
        let entry = |binding, ty| BindGroupLayoutEntry {
            binding,
            visibility: all,
            ty,
            count: None,
        };
        let storage = BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let sampled = |sample_type| BindingType::Texture {
            sample_type,
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        };
        let float = TextureSampleType::Float { filterable: true };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 8 data"),
            entries: &[
                entry(
                    0,
                    BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                entry(1, storage),
                entry(2, sampled(float)),
                entry(3, sampled(TextureSampleType::Uint)),
                entry(4, sampled(float)),
                entry(5, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(6, storage),
                entry(7, sampled(float)),
                entry(8, storage),
            ],
        });
        let view = |t: &Texture| t.create_view(&Default::default());
        let (palettes, monster, original_view, glow) = (
            view(&palettes),
            view(&monster),
            view(&original),
            view(&glow),
        );
        let data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 8 data"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: images.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&palettes),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&monster),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&original_view),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::Sampler(&sampler),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: colours.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::TextureView(&glow),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: profile.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Part 8"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |label: &str, source: String, fragment: &str, format| {
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(label),
                source: ShaderSource::Wgsl(source.into()),
            });
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &module,
                    entry_point: Some("fullscreen"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: &module,
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
        Self {
            key: key(preview),
            faithful: pipeline("Part 8 faithful HD", faithful_source(), "faithful", output),
            crt: pipeline("Part 8 modern", crt_source(), "crt", HDR),
            data,
            original,
            original_frame: None,
        }
    }

    /// Whether this renderer holds `preview`'s data.
    pub fn holds(&self, preview: &EndingPreview) -> bool {
        self.key == key(preview)
    }

    /// Upload the original frame `j` for the comparison modes.
    pub fn show_original(&mut self, queue: &Queue, preview: &EndingPreview, j: usize) {
        if self.original_frame == Some(j) {
            return;
        }
        queue.write_texture(
            self.original.as_image_copy(),
            &preview.original_rgba(j),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(320 * 4),
                rows_per_image: Some(200),
            },
            Extent3d {
                width: 320,
                height: 200,
                depth_or_array_layers: 1,
            },
        );
        self.original_frame = Some(j);
    }

    fn draw(&self, encoder: &mut CommandEncoder, target: &TextureView, pipeline: &RenderPipeline) {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Part 8"),
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
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_pipeline(pipeline);
        pass.draw(0..3, 0..1);
    }

    /// The modern view into the HDR target.
    pub fn draw_crt(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        self.draw(encoder, hdr, &self.crt);
    }

    /// Faithful HD into the output.
    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        self.draw(encoder, output, &self.faithful);
    }
}

fn key(preview: &EndingPreview) -> (usize, usize) {
    (preview.palettes.as_ptr() as usize, preview.palettes.len())
}

#[cfg(test)]
mod tests {
    use super::super::clip::{Clip, ScrollerLighting};
    use super::super::ending::{DOT_COLUMN, EndingBlock, LINE_ROW, PERSISTENCE, TRAIL_AGES};
    use super::super::renderer::{Renderer, View};
    use std::mem::{offset_of, size_of};

    fn parsed(name: &str, source: &str) -> naga::Module {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::default(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
        module
    }

    #[test]
    fn part8_block_matches_its_wgsl_declaration() {
        macro_rules! fields {
            ($($field:ident),* $(,)?) => {
                vec![$((stringify!($field).to_owned(), offset_of!(EndingBlock, $field))),*]
            };
        }
        let expected = fields![
            part,
            frame,
            tube,
            beam,
            ends,
            glow,
            colour,
            trail_rows,
            trail_band,
            trail_line,
            trail_left,
            trail_right,
            events,
            events_late
        ];
        assert_eq!(
            expected.last().unwrap().1 + size_of::<[f32; 4]>(),
            size_of::<EndingBlock>()
        );
        for (name, source) in [
            ("part 8 modern", super::crt_source()),
            ("part 8 faithful HD", super::faithful_source()),
        ] {
            let module = parsed(name, &source);
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some("EndingBlock"))
                .unwrap();
            let naga::TypeInner::Struct { members, span } = &ty.inner else {
                panic!("EndingBlock is not a struct");
            };
            let members: Vec<(String, usize)> = members
                .iter()
                .map(|m| (m.name.clone().unwrap(), m.offset as usize))
                .collect();
            assert_eq!(members, expected, "{name}");
            assert_eq!(*span as usize, size_of::<EndingBlock>(), "{name}");
        }
    }

    /// The constants the part-8 shaders repeat from hd/ending.rs, and the
    /// part-7 look the modern view's first frame must match.
    #[test]
    fn part8_shaders_repeat_the_rust_constants() {
        let common = include_str!("ending_common.wgsl");
        for constant in [
            format!("const LINE_ROW={LINE_ROW}.0;"),
            format!("const DOT_COLUMN={DOT_COLUMN}.0;"),
            "const COLUMN=5.0/6.0;".to_owned(),
        ] {
            assert!(
                common.contains(&constant),
                "ending_common.wgsl lacks {constant}"
            );
        }
        let crt = include_str!("ending_crt.wgsl");
        let part7 = [
            include_str!("greetings_common.wgsl"),
            include_str!("greetings_light.wgsl"),
        ]
        .concat();
        let compact = |s: &str| s.split_whitespace().collect::<String>();
        let part7 = compact(&part7);
        // Part 7 shows the monster through this inverse of the tonemap, its
        // whites at this level; the modern view starts from the same.
        let at = crt.find("fn tonemap_inverse(").unwrap();
        let body = &crt[at..at + crt[at..].find("\n}\n").unwrap() + 2];
        assert!(part7.contains(&compact(body)), "tonemap_inverse differs");
        assert!(part7.contains("constMONSTER_WHITE=.9;"));
        assert!(crt.contains("const MONSTER_WHITE=.9;"));
        let persistence = format!("const PERSISTENCE={PERSISTENCE:?};");
        let ages = TRAIL_AGES.map(|a| format!("{a:.1}")).join(",");
        let ages = format!("const TRAIL_AGES=array<f32,5>(0.0,{ages});");
        for constant in [persistence, ages] {
            assert!(crt.contains(&constant), "ending_crt.wgsl lacks {constant}");
        }
    }

    fn view(time: f64, modern: bool) -> View {
        View {
            time,
            size: (480, 270),
            comparison: 0,
            modern,
            hud: None,
        }
    }

    /// Part 8's first frame is part 7's last, byte for byte, in both views,
    /// and part 8's own drawing starts from it (a moment later, within two
    /// steps of a channel); part 8 is a function of the moment (a frame
    /// drawn after others is a new renderer's); and the demo ends on black.
    #[test]
    #[ignore = "requires locally supplied PANIC_DATA and a GPU; run with --ignored"]
    fn part8_continues_part7_and_its_frames_are_the_same_however_reached() {
        let data = crate::test_support::data();
        let part7 = Clip::load_greetings(&data, ScrollerLighting::Studio).unwrap();
        let clip = Clip::load_ending(&data, ScrollerLighting::Studio).unwrap();
        let ending = clip.ending.as_ref().unwrap();
        let first = ending.seconds(0);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        for modern in [true, false] {
            let (mut a, format) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
            let (mut b, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
            assert_eq!(
                part7.greetings_frame(first),
                Some(part7.greetings.as_ref().unwrap().frames.len() - 1)
            );
            let before = a.render_rgba(&part7, view(first, modern), format).unwrap();
            let after = b.render_rgba(&clip, view(first, modern), format).unwrap();
            assert!(before == after, "part 8's first frame (modern {modern})");
            let soon = b
                .render_rgba(&clip, view(first + 1.0 / 600.0, modern), format)
                .unwrap();
            let most = before.iter().zip(&soon).map(|(x, y)| x.abs_diff(*y)).max();
            assert!(most <= Some(2), "{most:?} a moment into part 8 ({modern})");
            let e = ending.events;
            let mut moments = vec![
                0.5,
                3.0,
                e.white,
                e.collapse + 0.05,
                e.line + 0.2,
                e.dot + 0.05,
            ];
            if modern {
                moments.push(e.end + 0.4);
            }
            let mut played = Vec::new();
            for &m in &moments {
                for k in (1..4).rev() {
                    a.render_rgba(&clip, view(first + m - k as f64 / 60.0, modern), format)
                        .unwrap();
                }
                played.push(
                    a.render_rgba(&clip, view(first + m, modern), format)
                        .unwrap(),
                );
            }
            for (&m, played) in moments.iter().zip(&played).rev() {
                let (mut fresh, _) = pollster::block_on(Renderer::new(&instance, None)).unwrap();
                let pixels = fresh
                    .render_rgba(&clip, view(first + m, modern), format)
                    .unwrap();
                assert!(&pixels == played, "{m} s into part 8 (modern {modern})");
                assert!(
                    pixels.chunks(4).any(|p| p[..3].iter().any(|&c| c > 8)),
                    "{m} s is black"
                );
            }
            let end = a
                .render_rgba(&clip, view(clip.duration(), modern), format)
                .unwrap();
            if modern {
                assert!(
                    end.chunks(4).all(|p| p[..3] == [0, 0, 0]),
                    "the end is black"
                );
            }
        }
    }
}
