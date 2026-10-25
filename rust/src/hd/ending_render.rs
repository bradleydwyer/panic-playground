//! GPU side of part 8: its pictures, palettes, halation image and the
//! screens' light, and the two pipelines. Faithful HD draws straight into
//! the output; the modern view draws the wall of monitors into the modern
//! renderer's HDR target, which then takes the shared bloom and tonemap, as
//! part 7's does.
use super::clip::vga_linear;
use super::ending::{EndingPreview, halation, picture_levels};
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("ending_common.wgsl");
const FAITHFUL: &str = include_str!("ending_faithful.wgsl");
const CRT: &str = include_str!("ending_crt.wgsl");
const SET: &str = include_str!("ending_set.wgsl");

/// Faithful HD's module: the shared controls, ending_common.wgsl and
/// ending_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, ending_common.wgsl,
/// ending_crt.wgsl (the pictures) and ending_set.wgsl (the wall).
pub(super) fn crt_source() -> String {
    wgsl_parts(&[COMMON, CRT, SET])
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
        let cells = buffer(
            "Part 8 screens' light",
            bytemuck::cast_slice(&preview.cells),
        );
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
        // The far tubes' pictures: on each palette the picture is shown on,
        // from the hold to the line, with their mipmaps, and each frame's.
        let (palettes_shown, layer_of) = picture_layers(preview);
        let layers: Vec<_> = palettes_shown
            .iter()
            .map(|&j| picture_levels(&preview.monster, preview.palette(j)))
            .collect();
        let layer_of = buffer("Part 8 frames' pictures", bytemuck::cast_slice(&layer_of));
        let pictures = device.create_texture(&TextureDescriptor {
            label: Some("Part 8 far pictures"),
            size: Extent3d {
                width: 320,
                height: 200,
                depth_or_array_layers: layers.len() as u32,
            },
            mip_level_count: layers[0].len() as u32,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: HDR,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, levels) in layers.iter().enumerate() {
            for (mip, (w, h, texels)) in levels.iter().enumerate() {
                queue.write_texture(
                    TexelCopyTextureInfo {
                        texture: &pictures,
                        mip_level: mip as u32,
                        origin: Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as u32,
                        },
                        aspect: TextureAspect::All,
                    },
                    bytemuck::cast_slice(texels),
                    TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(w * 8),
                        rows_per_image: Some(*h),
                    },
                    Extent3d {
                        width: *w,
                        height: *h,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        let trilinear = device.create_sampler(&SamplerDescriptor {
            label: Some("Part 8 far pictures"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..Default::default()
        });
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
                entry(9, storage),
                entry(
                    10,
                    BindingType::Texture {
                        sample_type: float,
                        view_dimension: TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                ),
                entry(11, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(12, storage),
            ],
        });
        let view = |t: &Texture| t.create_view(&Default::default());
        let (palettes, monster, original_view, glow) = (
            view(&palettes),
            view(&monster),
            view(&original),
            view(&glow),
        );
        let pictures_view = pictures.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });
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
                BindGroupEntry {
                    binding: 9,
                    resource: cells.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 10,
                    resource: BindingResource::TextureView(&pictures_view),
                },
                BindGroupEntry {
                    binding: 11,
                    resource: BindingResource::Sampler(&trilinear),
                },
                BindGroupEntry {
                    binding: 12,
                    resource: layer_of.as_entire_binding(),
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
            crt: pipeline("Part 8 modern", crt_source(), "television", HDR),
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

/// The frames whose palettes the far pictures are drawn on, one for each
/// distinct palette shown up to the line (the picture is not drawn after
/// it), and each frame's layer among them (from the line on, the last).
fn picture_layers(preview: &EndingPreview) -> (Vec<usize>, Vec<u32>) {
    let mut shown: Vec<usize> = Vec::new();
    let mut layer_of = Vec::with_capacity(preview.frames.len());
    for (j, frame) in preview.frames.iter().enumerate() {
        if frame.step <= 21 {
            let found = shown
                .iter()
                .position(|&k| preview.palette(k) == preview.palette(j));
            if found.is_none() {
                shown.push(j);
            }
        }
        let layer = shown
            .iter()
            .position(|&k| preview.palette(k) == preview.palette(j))
            .unwrap_or(shown.len() - 1);
        layer_of.push(layer as u32);
    }
    (shown, layer_of)
}

fn key(preview: &EndingPreview) -> (usize, usize) {
    (preview.palettes.as_ptr() as usize, preview.palettes.len())
}

#[cfg(test)]
mod tests {
    use super::super::clip::{Clip, ScrollerLighting};
    use super::super::ending::{
        DOT_COLUMN, EndingBlock, EndingEvents, EndingFrame, EndingPreview, LINE_ROW, PERSISTENCE,
        SET_PITCH, SPEED, Stage, TRAIL_AGES, TRIAD_PITCH,
    };
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
            events_late,
            eye,
            aim,
            look,
            moire
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
            include_str!("greetings_monster.wgsl"),
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
        // The wall the camera draws back from (ending::SET_PITCH): a set and
        // a gap to each pitch.
        let bank = include_str!("ending_set.wgsl");
        let (set, gap) = ((184.0, 146.0), 8.0);
        assert_eq!(2.0 * set.1 + gap, SET_PITCH);
        for constant in [
            format!("const SET=vec2f({:.1},{:.1});", set.0, set.1),
            format!("const GAP={gap:.1};"),
            format!(
                "const PITCH=vec2f({:.1},{:.1});",
                2.0 * set.0 + gap,
                2.0 * set.1 + gap
            ),
            format!("const PICTURE=vec2f({:.5},100.0);", 400.0 / 3.0),
        ] {
            assert!(bank.contains(&constant), "ending_set.wgsl lacks {constant}");
        }
        // Every set shows the whole picture, centred and upright in its
        // screen: no set moves, turns, sizes, rolls or skews its picture.
        let compact_bank = compact(bank);
        assert!(compact_bank.contains("fnplaced(local:vec2f)->vec2f{returnlocal/PICTURE;}"));
        assert!(compact_bank.contains("structLook{brightness:f32,exponents:vec3f}"));
    }

    /// The far pictures: one layer for each palette shown up to the line,
    /// each frame drawing on its own palette's, the frames after the line on
    /// the last.
    #[test]
    fn far_pictures_take_each_palette_up_to_the_line() {
        let steps = [0, 0, 0, 2, 3, 4, 5, 5, 20, 21, 22, 68, 69];
        let palette = |step: usize| -> Vec<u8> {
            let mut p = vec![10u8; 768];
            match step {
                2 | 3 => p[..3].fill(63),
                4..=20 => p[3..].fill(10 + step as u8),
                21.. => p[3..].fill(60),
                _ => {}
            }
            if step >= 68 {
                p[3..6].fill(step as u8 - 40);
            }
            p
        };
        let preview = EndingPreview {
            start: 0,
            frames: steps
                .iter()
                .map(|&step| EndingFrame {
                    step,
                    stage: Stage::of(step),
                    image: 0,
                })
                .collect(),
            palettes: steps.iter().flat_map(|&s| palette(s)).collect(),
            images: vec![vec![0; 64000]],
            monster: vec![0; 64000],
            music: Vec::new(),
            events: EndingEvents::default(),
            content: (0, 319),
            line_colour: [0.0; 3],
            profile: Vec::new(),
            phosphor: Vec::new(),
            cells: Vec::new(),
            pull_speed: SPEED,
            triad_pitch: TRIAD_PITCH,
        };
        let (shown, layer_of) = super::picture_layers(&preview);
        assert_eq!(shown, [0, 3, 5, 6, 8, 9]);
        assert_eq!(layer_of, [0, 0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 5, 5]);
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
                // The afterglow as the pulse ends: by then the wall is a far
                // field of dots, dim on a frame this small soon after.
                moments.push(e.end + 0.05);
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
