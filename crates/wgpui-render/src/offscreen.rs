use crate::Renderer;
use std::fmt;
use std::sync::mpsc;
use wgpu::TexelCopyBufferLayout;
use wgpui_core::{Scene, Size};

const BYTES_PER_PIXEL: u32 = 4;

#[derive(Clone, Copy, Debug)]
pub struct OffscreenGlyphAtlas<'a> {
    pub data: &'a [u8],
    pub size: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct OffscreenRenderRequest<'a> {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub clear_color: wgpu::Color,
    pub atlas: Option<OffscreenGlyphAtlas<'a>>,
}

impl<'a> OffscreenRenderRequest<'a> {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scale_factor: 1.0,
            clear_color: wgpu::Color::BLACK,
            atlas: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OffscreenRenderOutput {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub format: wgpu::TextureFormat,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct OffscreenRendererOptions {
    pub backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    pub allow_fallback_adapter: bool,
    pub format: wgpu::TextureFormat,
}

impl Default for OffscreenRendererOptions {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::all(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            allow_fallback_adapter: false,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

#[derive(Debug)]
pub enum OffscreenRenderError {
    InvalidDimensions { width: u32, height: u32 },
    InvalidScaleFactor(f32),
    AdapterUnavailable,
    DeviceRequest(wgpu::RequestDeviceError),
    BufferMap(wgpu::BufferAsyncError),
    BufferReceive,
    BufferLayoutOverflow,
}

impl fmt::Display for OffscreenRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid offscreen render dimensions {width}x{height}")
            }
            Self::InvalidScaleFactor(scale_factor) => {
                write!(f, "invalid offscreen render scale factor {scale_factor}")
            }
            Self::AdapterUnavailable => write!(f, "no compatible offscreen adapter found"),
            Self::DeviceRequest(error) => write!(f, "failed to create offscreen device: {error}"),
            Self::BufferMap(error) => write!(f, "failed to map offscreen readback buffer: {error}"),
            Self::BufferReceive => {
                write!(f, "failed to receive offscreen buffer mapping result")
            }
            Self::BufferLayoutOverflow => {
                write!(f, "offscreen buffer layout overflowed supported limits")
            }
        }
    }
}

impl std::error::Error for OffscreenRenderError {}

pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
}

impl OffscreenRenderer {
    pub async fn new(options: OffscreenRendererOptions) -> Result<Self, OffscreenRenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: options.backends,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: options.power_preference,
                compatible_surface: None,
                force_fallback_adapter: options.allow_fallback_adapter,
            })
            .await
            .ok_or(OffscreenRenderError::AdapterUnavailable)?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .map_err(OffscreenRenderError::DeviceRequest)?;

        Ok(Self {
            device,
            queue,
            format: options.format,
        })
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub const fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub const fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn create_renderer(&self) -> Renderer {
        Renderer::new(&self.device, self.format)
    }

    pub fn render_scene(
        &self,
        renderer: &mut Renderer,
        scene: &Scene,
        request: OffscreenRenderRequest<'_>,
    ) -> Result<OffscreenRenderOutput, OffscreenRenderError> {
        validate_request(&request)?;

        renderer.resize(
            &self.queue,
            Size::new(request.width as f32, request.height as f32),
            request.scale_factor,
        );
        if let Some(atlas) = request.atlas {
            renderer.update_atlas(&self.queue, atlas.data, atlas.size);
        }
        renderer.prepare(&self.device, &self.queue, scene, request.scale_factor);

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Offscreen Render Target"),
            size: wgpu::Extent3d {
                width: request.width,
                height: request.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bytes_per_row = request
            .width
            .checked_mul(BYTES_PER_PIXEL)
            .ok_or(OffscreenRenderError::BufferLayoutOverflow)?;
        let padded_bytes_per_row = padded_bytes_per_row(bytes_per_row);
        let buffer_size = u64::from(padded_bytes_per_row)
            .checked_mul(u64::from(request.height))
            .ok_or(OffscreenRenderError::BufferLayoutOverflow)?;

        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Offscreen Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen_render_encoder"),
            });

        renderer.render_with_clear(&mut encoder, &view, request.clear_color);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(request.height),
                },
            },
            wgpu::Extent3d {
                width: request.width,
                height: request.height,
                depth_or_array_layers: 1,
            },
        );

        let submission_index = self.queue.submit(std::iter::once(encoder.finish()));
        let slice = readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submission_index));
        let map_result = receiver
            .recv()
            .map_err(|_| OffscreenRenderError::BufferReceive)?;
        map_result.map_err(OffscreenRenderError::BufferMap)?;

        let mapped = slice.get_mapped_range();
        let mut bytes = Vec::with_capacity((bytes_per_row as usize) * (request.height as usize));
        for row in mapped.chunks_exact(padded_bytes_per_row as usize) {
            bytes.extend_from_slice(&row[..bytes_per_row as usize]);
        }
        drop(mapped);
        readback_buffer.unmap();

        Ok(OffscreenRenderOutput {
            width: request.width,
            height: request.height,
            scale_factor: request.scale_factor,
            format: self.format,
            bytes,
        })
    }
}

fn validate_request(request: &OffscreenRenderRequest<'_>) -> Result<(), OffscreenRenderError> {
    if request.width == 0 || request.height == 0 {
        return Err(OffscreenRenderError::InvalidDimensions {
            width: request.width,
            height: request.height,
        });
    }
    if !request.scale_factor.is_finite() || request.scale_factor <= 0.0 {
        return Err(OffscreenRenderError::InvalidScaleFactor(
            request.scale_factor,
        ));
    }
    Ok(())
}

fn padded_bytes_per_row(bytes_per_row: u32) -> u32 {
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    bytes_per_row.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::{OffscreenRenderRequest, OffscreenRenderer, OffscreenRendererOptions};
    use crate::Renderer;
    use pollster::block_on;
    use wgpui_core::{Bounds, Hsla, Quad, Scene};

    #[test]
    fn offscreen_render_roundtrip_smoke() {
        let offscreen = match block_on(OffscreenRenderer::new(OffscreenRendererOptions {
            allow_fallback_adapter: true,
            ..Default::default()
        })) {
            Ok(offscreen) => offscreen,
            Err(_error) => return,
        };

        let mut renderer = Renderer::new(offscreen.device(), offscreen.format());
        let mut scene = Scene::new();
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, 16.0, 16.0))
                .with_background(Hsla::new(0.0, 0.0, 1.0, 1.0)),
        );

        let output = offscreen
            .render_scene(&mut renderer, &scene, OffscreenRenderRequest::new(16, 16))
            .expect("offscreen render should succeed");

        assert_eq!(output.width, 16);
        assert_eq!(output.height, 16);
        assert_eq!(output.bytes.len(), 16 * 16 * 4);
        assert!(output.bytes.iter().any(|byte| *byte != 0));
    }
}
