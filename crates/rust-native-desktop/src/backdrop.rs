//! A live picture behind the views.
//!
//! An application may give its window a [`Backdrop`]: something it draws
//! with the window's own `wgpu` device into a texture, frame by frame. The
//! window then keeps the views and the backdrop apart:
//!
//! - The views are painted in software, as always, but into a clear frame
//!   ([`Frame::transparent`]), and only when they change. The frame goes to
//!   a texture once per change.
//! - Each backdrop frame is drawn smaller than the window ([`Look::scale`]),
//!   softened with a small blur, and covered by the window's background
//!   color at [`Look::dim`], so it stays behind everything.
//! - One pass lays the views over it. Wherever a view draws, its own
//!   colors win exactly as in a plain window: a card, a button, a white
//!   code square, or text keeps its full contrast.
//!
//! The backdrop decides its own frame rate with [`Backdrop::next_frame`];
//! the window sleeps between frames, and asks for none while it is hidden.

use crate::Waker;
use crate::canvas::Frame;
use rust_native::style::Color;
use std::time::Instant;

/// The format a backdrop draws into: 8-bit sRGB with alpha.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// The window's device, which a backdrop draws with.
pub struct Gpu<'a> {
    pub adapter: &'a wgpu::Adapter,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}

/// A picture the window draws behind its views.
pub trait Backdrop {
    /// A registered surface to draw into, or the whole window for a backdrop.
    fn surface(&self) -> Option<&str> {
        None
    }

    /// The current destination in logical points and pixels per point.
    fn viewport(&mut self, rect: crate::Rect, scale: f32) {
        let _ = (rect, scale);
    }

    /// Override the backdrop treatment for an interactive layer.
    fn look(&self) -> Option<Look> {
        None
    }
    /// Called once with a waker before the first frame.
    fn start(&mut self, waker: Waker) {
        let _ = waker;
    }

    /// The window became visible (`true`) or hidden: minimized, covered, on
    /// another space, or behind a locked screen.
    fn shown(&mut self, visible: bool, now: Instant);

    /// When the next frame is due, or `None` while the picture is still.
    /// The window asks whenever it wakes, hidden or not, so a backdrop may
    /// do its own housekeeping here; a hidden window draws no frame.
    fn next_frame(&mut self, now: Instant) -> Option<Instant>;

    /// Records one frame into `target`, a [`FORMAT`] texture `size` pixels.
    ///
    /// # Errors
    ///
    /// A message when the backdrop cannot draw; the window then drops it and
    /// shows its plain background.
    fn draw(
        &mut self,
        gpu: &Gpu<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        now: Instant,
    ) -> Result<(), String>;
}

/// How the backdrop sits under the views.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// How much of the window's background color covers the backdrop, from
    /// 0 (none) to 1 (the backdrop is hidden).
    pub dim: f32,
    /// Backdrop pixels per window pixel, from 0.25 to 1.
    pub scale: f32,
    /// The blur's reach, in backdrop pixels.
    pub blur: f32,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            dim: 0.55,
            scale: 0.5,
            blur: 1.5,
        }
    }
}

impl Look {
    fn clamped(self) -> Look {
        let finite = |value: f32, fallback: f32| if value.is_finite() { value } else { fallback };
        Look {
            dim: finite(self.dim, 0.55).clamp(0.0, 1.0),
            scale: finite(self.scale, 0.5).clamp(0.25, 1.0),
            blur: finite(self.blur, 1.5).clamp(0.0, 8.0),
        }
    }

    /// The backdrop's size for a window `width` by `height` pixels.
    pub fn backdrop_size(self, width: u32, height: u32) -> (u32, u32) {
        let scale = self.clamped().scale;
        let side = |pixels: u32| ((pixels as f32 * scale).round() as u32).max(1);
        (side(width), side(height))
    }
}

/// One pixel of the window, as the compositing pass makes it: the backdrop
/// under the window's background at `dim`, then the views' premultiplied
/// pixel over that. Every value is 8-bit sRGB-encoded, as the software
/// painter blends. Tests and captures composite with it; the shader does
/// the same arithmetic.
pub fn composite_pixel(backdrop: [u8; 3], background: Color, dim: f32, view: [u8; 4]) -> [u8; 3] {
    let dim = dim.clamp(0.0, 1.0);
    let alpha = f32::from(view[3]) / 255.0;
    let under = [background.red, background.green, background.blue];
    std::array::from_fn(|i| {
        let base = f32::from(backdrop[i]) * (1.0 - dim) + f32::from(under[i]) * dim;
        (f32::from(view[i]) + base * (1.0 - alpha))
            .round()
            .clamp(0.0, 255.0) as u8
    })
}

/// Lays `views`, a premultiplied frame, over `backdrop` (any size, scaled
/// to fit with the nearest pixel) as the window would, without the blur.
pub fn composite(views: &Frame, backdrop: &Frame, background: Color, dim: f32) -> Frame {
    let mut out = Frame::new(views.width, views.height, background);
    for y in 0..views.height {
        for x in 0..views.width {
            let bx = (x * backdrop.width / views.width.max(1)).min(backdrop.width - 1);
            let by = (y * backdrop.height / views.height.max(1)).min(backdrop.height - 1);
            let at = (y * views.width + x) * 4;
            let view = [
                views.pixels[at],
                views.pixels[at + 1],
                views.pixels[at + 2],
                views.pixels[at + 3],
            ];
            let [r, g, b] = composite_pixel(backdrop.pixel(bx, by), background, dim, view);
            out.pixels[at..at + 3].copy_from_slice(&[r, g, b]);
        }
    }
    out
}

/// The compositing pass's uniforms: the background color, one backdrop
/// texel in texture coordinates, the dim, and the blur's reach.
fn uniforms(background: Color, texel: [f32; 2], look: Look, region: [f32; 4]) -> Vec<u8> {
    let channel = |value: u8| f32::from(value) / 255.0;
    [
        channel(background.red),
        channel(background.green),
        channel(background.blue),
        1.0,
        texel[0],
        texel[1],
        look.dim,
        look.blur,
        region[0],
        region[1],
        region[2],
        region[3],
    ]
    .iter()
    .flat_map(|value| value.to_le_bytes())
    .collect()
}

/// The textures and the pass that put the views over the backdrop.
pub(crate) struct Compositor {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniforms: wgpu::Buffer,
    /// The backdrop's texture, its sRGB view for drawing into, and the
    /// encoded view the pass samples.
    backdrop: Option<(wgpu::Texture, wgpu::TextureView, wgpu::TextureView)>,
    views: Option<wgpu::Texture>,
    group: Option<wgpu::BindGroup>,
    window: (u32, u32),
    backdrop_size: (u32, u32),
    region: [f32; 4],
}

impl Compositor {
    pub(crate) fn new(device: &wgpu::Device, output: wgpu::TextureFormat) -> Compositor {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rust-native-desktop backdrop"),
            entries: &[
                texture_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rust-native-desktop backdrop"),
            source: wgpu::ShaderSource::Wgsl(include_str!("backdrop.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rust-native-desktop backdrop"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rust-native-desktop backdrop"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("rust-native-desktop backdrop"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rust-native-desktop backdrop"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Compositor {
            pipeline,
            layout,
            sampler,
            uniforms,
            backdrop: None,
            views: None,
            group: None,
            window: (0, 0),
            backdrop_size: (0, 0),
            region: [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Makes the textures fit a window `width` by `height` pixels. Returns
    /// true when they were remade, so the views must be uploaded again.
    pub(crate) fn fit(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        look: Look,
        region: crate::PxRect,
    ) -> bool {
        self.region = [
            region.x / width as f32,
            region.y / height as f32,
            region.w / width as f32,
            region.h / height as f32,
        ];
        let backdrop_size = look.backdrop_size(region.w.max(1.0) as u32, region.h.max(1.0) as u32);
        if self.group.is_some()
            && self.window == (width, height)
            && self.backdrop_size == backdrop_size
        {
            return false;
        }
        let texture =
            |label, (w, h): (u32, u32), format, usage, view_formats: &[wgpu::TextureFormat]| {
                device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats,
                })
            };
        let encoded = FORMAT.remove_srgb_suffix();
        let backdrop = texture(
            "rust-native-desktop backdrop",
            backdrop_size,
            FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[encoded],
        );
        let draw_view = backdrop.create_view(&wgpu::TextureViewDescriptor::default());
        let sample_view = backdrop.create_view(&wgpu::TextureViewDescriptor {
            format: Some(encoded),
            ..Default::default()
        });
        let views = texture(
            "rust-native-desktop views",
            (width, height),
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            &[],
        );
        let views_view = views.create_view(&wgpu::TextureViewDescriptor::default());
        self.group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rust-native-desktop backdrop"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniforms.as_entire_binding(),
                },
            ],
        }));
        self.backdrop = Some((backdrop, draw_view, sample_view));
        self.views = Some(views);
        self.window = (width, height);
        self.backdrop_size = backdrop_size;
        true
    }

    /// The view a backdrop draws into, and its size.
    pub(crate) fn target(&self) -> Option<(&wgpu::TextureView, (u32, u32))> {
        self.backdrop
            .as_ref()
            .map(|(_, draw, _)| (draw, self.backdrop_size))
    }

    /// Uploads the views' premultiplied frame, the window's size.
    pub(crate) fn upload_regions(
        &self,
        queue: &wgpu::Queue,
        frame: &Frame,
        regions: &[crate::PxRect],
    ) {
        let Some(views) = &self.views else {
            return;
        };
        if (frame.width as u32, frame.height as u32) != self.window {
            return;
        }
        for region in regions {
            let x = region.x as u32;
            let y = region.y as u32;
            let width = region.w as u32;
            let height = region.h as u32;
            if width == 0 || height == 0 {
                continue;
            }
            let offset = (y as usize * frame.width + x as usize) * 4;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: views,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.pixels[offset..],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.window.0 * 4),
                    rows_per_image: Some(self.window.1),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Records the pass that lays the views over the backdrop into `output`.
    pub(crate) fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        background: Color,
        look: Look,
    ) {
        let Some(group) = &self.group else {
            return;
        };
        let texel = [
            1.0 / self.backdrop_size.0 as f32,
            1.0 / self.backdrop_size.1 as f32,
        ];
        queue.write_buffer(
            &self.uniforms,
            0,
            &uniforms(background, texel, look.clamped(), self.region),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rust-native-desktop backdrop"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a graphics adapter; run explicitly for compositor changes"]
    fn partial_menu_uploads_preserve_the_gpu_foreground() {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        }))
        .expect("a graphics adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("a graphics device");
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let size = wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("menu upload regression"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("menu upload readback"),
            size: 256 * 256 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let background = Color::rgb(6, 6, 6);
        let mut compositor = Compositor::new(&device, format);
        let window = crate::PxRect {
            x: 0.0,
            y: 0.0,
            w: 256.0,
            h: 256.0,
        };
        let menu = crate::PxRect {
            x: 17.0,
            y: 23.0,
            w: 211.0,
            h: 177.0,
        };
        let mut previous = None;
        for step in 0..32 {
            let look = Look {
                dim: 1.0,
                blur: 0.0,
                scale: if step < 16 { 0.5 } else { 0.25 },
            };
            let replaced = compositor.fit(&device, 256, 256, look, window);
            let mut frame = Frame::transparent(256, 256);
            frame.fill(
                crate::PxRect {
                    x: 3.0,
                    y: 4.0,
                    w: 250.0,
                    h: 248.0,
                },
                12.0,
                Color::rgb(13, 13, 13),
            );
            let state = step % 5;
            if matches!(state, 1..=3) {
                frame.fill(menu, 12.0, Color::rgb(16, 16, 16));
                frame.fill(
                    crate::PxRect {
                        x: 25.0,
                        y: 31.0 + if state == 3 { 32.0 } else { 0.0 },
                        w: 195.0,
                        h: 30.0,
                    },
                    7.0,
                    Color {
                        alpha: 28,
                        ..Color::rgb(255, 255, 255)
                    },
                );
            }
            let unchanged = previous.as_ref() == Some(&frame.pixels);
            let regions = if replaced {
                vec![window]
            } else if unchanged {
                vec![]
            } else {
                vec![menu]
            };
            compositor.upload_regions(&queue, &frame, &regions);
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            compositor.encode(&queue, &mut encoder, &output_view, background, look);
            encoder.copy_texture_to_buffer(
                output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(1024),
                        rows_per_image: Some(256),
                    },
                },
                size,
            );
            queue.submit([encoder.finish()]);
            let slice = readback.slice(..);
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            slice.map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap();
            });
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(5)),
                })
                .expect("GPU completion");
            receiver.recv().unwrap().expect("mapped readback");
            let pixels = slice.get_mapped_range();
            for (index, (gpu, cpu)) in pixels
                .chunks_exact(4)
                .zip(frame.pixels.chunks_exact(4))
                .enumerate()
            {
                let expected = composite_pixel([0; 3], background, 1.0, cpu.try_into().unwrap());
                assert!(
                    gpu[..3]
                        .iter()
                        .zip(expected)
                        .all(|(actual, expected)| actual.abs_diff(expected) <= 1),
                    "GPU menu frame {step}, pixel {index}: {gpu:?}, expected {expected:?}"
                );
                assert_eq!(gpu[3], 255);
            }
            drop(pixels);
            readback.unmap();
            previous = Some(frame.pixels);
        }
    }

    #[test]
    fn an_interactive_layer_keeps_its_destination_and_unmodified_pixels() {
        let look = Look {
            dim: 0.0,
            blur: 0.0,
            scale: 1.0,
        };
        assert_eq!(look.backdrop_size(800, 600), (800, 600));
        assert_eq!(
            composite_pixel([35, 120, 200], Color::rgb(0, 0, 0), look.dim, [0; 4]),
            [35, 120, 200]
        );
        let bytes = uniforms(
            Color::rgb(0, 0, 0),
            [0.01, 0.01],
            look,
            [0.2, 0.1, 0.7, 0.8],
        );
        assert_eq!(bytes.len(), 48);
        let region: Vec<f32> = bytes[32..]
            .chunks_exact(4)
            .map(|value| f32::from_le_bytes(value.try_into().unwrap()))
            .collect();
        assert_eq!(region, [0.2, 0.1, 0.7, 0.8]);
    }

    #[test]
    fn an_opaque_view_pixel_wins_and_a_clear_one_shows_the_dimmed_backdrop() {
        let black = Color::rgb(0, 0, 0);
        // A white code square over a bright backdrop stays white; its black
        // module stays black.
        assert_eq!(
            composite_pixel([255, 255, 255], black, 0.6, [255, 255, 255, 255]),
            [255, 255, 255]
        );
        assert_eq!(
            composite_pixel([255, 255, 255], black, 0.6, [0, 0, 0, 255]),
            [0, 0, 0]
        );
        // Where no view draws, the backdrop shows at 40 percent.
        assert_eq!(
            composite_pixel([200, 100, 50], black, 0.6, [0, 0, 0, 0]),
            [80, 40, 20]
        );
    }

    #[test]
    fn the_compositing_shader_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("backdrop.wgsl")).expect("parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("validates");
    }

    #[test]
    fn the_backdrop_is_drawn_smaller_than_the_window() {
        let look = Look::default();
        assert_eq!(look.backdrop_size(1120, 1440), (560, 720));
        let silly = Look {
            dim: f32::NAN,
            scale: 9.0,
            blur: -1.0,
        };
        assert_eq!(silly.backdrop_size(100, 100), (100, 100));
        assert_eq!(silly.clamped().dim, 0.55);
    }
}
