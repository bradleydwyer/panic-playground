//! GPU side of part 7: its tables, the letters' faces, the dust, and the
//! pipelines. Faithful HD draws straight into the output from the tables;
//! the modern view draws the dust, marches the cubes (writing the shafts'
//! source beside them), gathers the shafts at half size and composites into
//! the modern renderer's HDR target, which then takes the shared bloom and
//! tonemap.
use super::greetings::{GreetingsPreview, ROWS, SLOTS, STRIPS};
use super::greetings_type::ATLAS_SIZE;
use super::renderer::wgsl_parts;
use wgpu::util::DeviceExt;
use wgpu::*;

const COMMON: &str = include_str!("greetings_common.wgsl");
const FAITHFUL: &str = include_str!("greetings_faithful.wgsl");
const LIGHT: &str = include_str!("greetings_light.wgsl");

/// Faithful HD's module: the shared controls, greetings_common.wgsl and
/// greetings_faithful.wgsl.
pub(super) fn faithful_source() -> String {
    wgsl_parts(&[COMMON, FAITHFUL])
}

/// The modern module: the shared controls, greetings_common.wgsl and
/// greetings_light.wgsl.
pub(super) fn light_source() -> String {
    wgsl_parts(&[COMMON, LIGHT])
}

/// The images the modern view's passes hand on, for one drawable size.
struct Targets {
    size: (u32, u32),
    dust: TextureView,
    cubes: TextureView,
    scatter: TextureView,
    shafts: TextureView,
    cubes_inputs: BindGroup,
    shafts_inputs: BindGroup,
    style_inputs: BindGroup,
}

pub(super) struct GreetingsRenderer {
    /// Identifies the uploaded preview by its palettes.
    key: (usize, usize),
    data: BindGroup,
    original: Texture,
    /// The original frame uploaded to `original`.
    original_frame: Option<usize>,
    motes: u32,
    cubes_layout: BindGroupLayout,
    shafts_layout: BindGroupLayout,
    style_layout: BindGroupLayout,
    targets: Option<Targets>,
    faithful: RenderPipeline,
    dust: RenderPipeline,
    cubes: RenderPipeline,
    shafts: RenderPipeline,
    style: RenderPipeline,
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
        let all = ShaderStages::VERTEX | ShaderStages::FRAGMENT;
        let storage = BindingType::Buffer {
            ty: BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let data_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 data"),
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
                entry(3, all, storage),
                entry(4, all, sampled(FLOAT)),
                entry(5, all, sampled(TextureSampleType::Uint)),
                entry(6, all, sampled(FLOAT)),
                entry(7, all, BindingType::Sampler(SamplerBindingType::Filtering)),
                entry(8, all, sampled(FLOAT)),
                entry(9, all, storage),
            ],
        });
        let original_view = original.create_view(&Default::default());
        let data = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 data"),
            layout: &data_layout,
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
            ],
        });
        let inputs = |label, bindings: &[u32]| {
            let entries: Vec<BindGroupLayoutEntry> = bindings
                .iter()
                .map(|&b| entry(b, ShaderStages::FRAGMENT, sampled(FLOAT)))
                .collect();
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &entries,
            })
        };
        let cubes_layout = inputs("Part 7 cubes' inputs", &[1]);
        let shafts_layout = inputs("Part 7 shafts' inputs", &[2]);
        let style_layout = inputs("Part 7 picture's inputs", &[1, 3, 4]);
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
        let dust = pipeline(
            "Part 7 dust",
            &layout(&[&data_layout]),
            &light_module,
            "dust_vertex",
            "dust_fragment",
            &[target(HDR, Some(ADD))],
        );
        let cubes = pipeline(
            "Part 7 cubes",
            &layout(&[&data_layout, &cubes_layout]),
            &light_module,
            "fullscreen",
            "cubes",
            &[target(HDR, None), target(HDR, None)],
        );
        let shafts = pipeline(
            "Part 7 shafts",
            &layout(&[&data_layout, &shafts_layout]),
            &light_module,
            "fullscreen",
            "shafts",
            &[target(HDR, None)],
        );
        let style = pipeline(
            "Part 7 modern",
            &layout(&[&data_layout, &style_layout]),
            &light_module,
            "fullscreen",
            "style",
            &[target(HDR, None)],
        );
        Self {
            key: key(preview),
            data,
            original,
            original_frame: None,
            motes: preview.motes.len() as u32,
            cubes_layout,
            shafts_layout,
            style_layout,
            targets: None,
            faithful,
            dust,
            cubes,
            shafts,
            style,
        }
    }

    /// Whether this renderer holds `preview`'s data.
    pub fn holds(&self, preview: &GreetingsPreview) -> bool {
        self.key == key(preview)
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
        let make =
            |label, size| texture(device, label, size, HDR, usage).create_view(&Default::default());
        let dust = make("Part 7 dust image", size);
        let cubes = make("Part 7 cubes image", size);
        let scatter = make("Part 7 shafts' source", size);
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        let shafts = make("Part 7 shafts", half);
        let group = |label, layout: &BindGroupLayout, entries: &[BindGroupEntry]| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some(label),
                layout,
                entries,
            })
        };
        let cubes_inputs = group(
            "Part 7 cubes' inputs",
            &self.cubes_layout,
            &[view(1, &dust)],
        );
        let shafts_inputs = group(
            "Part 7 shafts' inputs",
            &self.shafts_layout,
            &[view(2, &scatter)],
        );
        let style_inputs = group(
            "Part 7 picture's inputs",
            &self.style_layout,
            &[view(1, &dust), view(3, &cubes), view(4, &shafts)],
        );
        self.targets = Some(Targets {
            size,
            dust,
            cubes,
            scatter,
            shafts,
            cubes_inputs,
            shafts_inputs,
            style_inputs,
        });
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

    /// The modern view into the HDR target: the dust, the cubes and the
    /// shafts' source, the shafts, then the picture. Call
    /// [`GreetingsRenderer::resize`] first.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        let t = self.targets.as_ref().unwrap();
        {
            let mut pass = Self::pass(encoder, "Part 7 dust", &[&t.dust]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.dust);
            pass.draw(0..6, 0..self.motes);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 cubes", &[&t.cubes, &t.scatter]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &t.cubes_inputs, &[]);
            pass.set_pipeline(&self.cubes);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 shafts", &[&t.shafts]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &t.shafts_inputs, &[]);
            pass.set_pipeline(&self.shafts);
            pass.draw(0..3, 0..1);
        }
        let mut pass = Self::pass(encoder, "Part 7 modern", &[hdr]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_bind_group(1, &t.style_inputs, &[]);
        pass.set_pipeline(&self.style);
        pass.draw(0..3, 0..1);
    }

    /// Faithful HD into the output.
    pub fn draw_faithful(&self, encoder: &mut CommandEncoder, output: &TextureView) {
        let mut pass = Self::pass(encoder, "Part 7 faithful HD", &[output]);
        pass.set_bind_group(0, &self.data, &[]);
        pass.set_pipeline(&self.faithful);
        pass.draw(0..3, 0..1);
    }
}

fn key(preview: &GreetingsPreview) -> (usize, usize) {
    (preview.palettes.as_ptr() as usize, preview.palettes.len())
}

#[cfg(test)]
mod tests {
    use super::super::greetings::{
        CAMERA_DISTANCE, DUST_DEPTH, ROWS, SLOT_ROWS, SLOTS, STAR_BASE, STRIP_WIDTH, STRIP_X,
        STRIPS, SWEEP_CENTRE,
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
            format!("const DUST_DEPTH={DUST_DEPTH:.1};"),
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
    }
}
