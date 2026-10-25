//! GPU side of part 7: its tables, the letters' faces, the dust, the air's
//! density, and the pipelines. Faithful HD draws straight into the output
//! from the tables. The modern view maps what the strips let through as the
//! lamp sees it, marches the cubes (their colour, depth and where their
//! polish mirrors the picture), draws the dust in front of them, marches the
//! lit air at half size, and composites into the modern renderer's HDR
//! target, which then takes the shared bloom and tonemap.
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

/// The strips' transmission seen from the lamp: the map's size, and its
/// half extent on the strips' plane (source rows; greetings_light.wgsl's
/// TRANSMISSION_EXTENT).
pub const TRANSMISSION_SIZE: (u32, u32) = (1024, 768);
#[cfg_attr(not(test), allow(dead_code))]
pub const TRANSMISSION_EXTENT: (f64, f64) = (240.0, 180.0);
/// The air's density noise: its size (texels a side) and lattices.
pub const NOISE_SIZE: u32 = 64;

/// The images the modern view's passes hand on, for one drawable size.
struct Targets {
    size: (u32, u32),
    dust: TextureView,
    cubes: TextureView,
    depth: TextureView,
    mirror: TextureView,
    air: TextureView,
    scene_inputs: BindGroup,
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
    transmission: TextureView,
    scene_layout: BindGroupLayout,
    style_layout: BindGroupLayout,
    targets: Option<Targets>,
    faithful: RenderPipeline,
    strips_transmission: RenderPipeline,
    cubes: RenderPipeline,
    dust: RenderPipeline,
    air: RenderPipeline,
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
        let transmission = texture(
            device,
            "Part 7 strips' transmission",
            TRANSMISSION_SIZE,
            HDR,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        )
        .create_view(&Default::default());
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
                entry(
                    10,
                    all,
                    BindingType::Texture {
                        sample_type: FLOAT,
                        view_dimension: TextureViewDimension::D3,
                        multisampled: false,
                    },
                ),
                entry(11, all, BindingType::Sampler(SamplerBindingType::Filtering)),
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
                view(10, &noise_view),
                BindGroupEntry {
                    binding: 11,
                    resource: BindingResource::Sampler(&repeat),
                },
            ],
        });
        let depth_type = sampled(TextureSampleType::Float { filterable: false });
        let scene_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 scene's inputs"),
            entries: &[entry(2, all, depth_type), entry(3, all, sampled(FLOAT))],
        });
        let fragment = ShaderStages::FRAGMENT;
        let style_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Part 7 picture's inputs"),
            entries: &[
                entry(1, fragment, sampled(FLOAT)),
                entry(2, fragment, depth_type),
                entry(4, fragment, sampled(FLOAT)),
                entry(5, fragment, sampled(FLOAT)),
                entry(6, fragment, sampled(FLOAT)),
            ],
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
        let strips_transmission = pipeline(
            "Part 7 strips' transmission",
            &layout(&[&data_layout]),
            &light_module,
            "fullscreen",
            "strips_transmission",
            &[target(HDR, None)],
        );
        let cubes = pipeline(
            "Part 7 cubes",
            &layout(&[&data_layout]),
            &light_module,
            "fullscreen",
            "cubes",
            &[
                target(HDR, None),
                target(TextureFormat::R32Float, None),
                target(HDR, None),
            ],
        );
        let dust = pipeline(
            "Part 7 dust",
            &scene,
            &light_module,
            "dust_vertex",
            "dust_fragment",
            &[target(HDR, Some(ADD))],
        );
        let air = pipeline(
            "Part 7 air",
            &scene,
            &light_module,
            "fullscreen",
            "air",
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
            transmission,
            scene_layout,
            style_layout,
            targets: None,
            faithful,
            strips_transmission,
            cubes,
            dust,
            air,
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
        let make = |label, size, format| {
            texture(device, label, size, format, usage).create_view(&Default::default())
        };
        let dust = make("Part 7 dust image", size, HDR);
        let cubes = make("Part 7 cubes image", size, HDR);
        let depth = make("Part 7 cubes' depth", size, TextureFormat::R32Float);
        let mirror = make("Part 7 cubes' mirror", size, HDR);
        let half = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        let air = make("Part 7 lit air", half, HDR);
        let scene_inputs = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 scene's inputs"),
            layout: &self.scene_layout,
            entries: &[view(2, &depth), view(3, &self.transmission)],
        });
        let style_inputs = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Part 7 picture's inputs"),
            layout: &self.style_layout,
            entries: &[
                view(1, &dust),
                view(2, &depth),
                view(4, &cubes),
                view(5, &air),
                view(6, &mirror),
            ],
        });
        self.targets = Some(Targets {
            size,
            dust,
            cubes,
            depth,
            mirror,
            air,
            scene_inputs,
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

    /// The modern view into the HDR target: what the strips let through,
    /// seen from the lamp; the cubes, their depth and where they mirror the
    /// picture; the dust; the lit air; then the picture. Call
    /// [`GreetingsRenderer::resize`] first.
    pub fn draw_style(&self, encoder: &mut CommandEncoder, hdr: &TextureView) {
        let t = self.targets.as_ref().unwrap();
        {
            let mut pass = Self::pass(
                encoder,
                "Part 7 strips' transmission",
                &[&self.transmission],
            );
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.strips_transmission);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 cubes", &[&t.cubes, &t.depth, &t.mirror]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_pipeline(&self.cubes);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 dust", &[&t.dust]);
            pass.set_bind_group(0, &self.data, &[]);
            pass.set_bind_group(1, &t.scene_inputs, &[]);
            pass.set_pipeline(&self.dust);
            pass.draw(0..6, 0..self.motes);
        }
        {
            let mut pass = Self::pass(encoder, "Part 7 air", &[&t.air]);
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

fn key(preview: &GreetingsPreview) -> (usize, usize) {
    (preview.palettes.as_ptr() as usize, preview.palettes.len())
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
            format!(
                "const TRANSMISSION_EXTENT=vec2f({:.1},{:.1});",
                super::TRANSMISSION_EXTENT.0,
                super::TRANSMISSION_EXTENT.1
            ),
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
        // The map's texels are square on the plane.
        let (w, h) = super::TRANSMISSION_SIZE;
        let (ex, ey) = super::TRANSMISSION_EXTENT;
        assert!((ex / w as f64 - ey / h as f64).abs() < 1e-9);
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
