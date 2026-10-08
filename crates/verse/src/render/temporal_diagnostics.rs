//! Read-only capture of the live temporal textures, after the primary render.

use super::{Offscreen, extent};

const SOURCE: &str = "
@group(0) @binding(0) var marker: texture_2d<f32>;
@group(0) @binding(1) var history: texture_2d<f32>;
@group(0) @binding(2) var scene: texture_2d<f32>;
struct Masks {
    @location(0) marker: vec4<f32>,
    @location(1) history: vec4<f32>,
    @location(2) scene: vec4<f32>,
    @location(3) additive_fx: vec4<f32>,
};
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> Masks {
    let at = vec2<i32>(p.xy);
    let visibility = clamp(textureLoad(marker, at, 0).rg, vec2<f32>(0.0), vec2<f32>(1.0));
    let reactive = select(0.0, 1.0, textureLoad(history, at, 0).a < 0.0);
    let hdr = max(textureLoad(scene, at, 0).rgb, vec3<f32>(0.0));
    let displayed = pow(hdr / (vec3<f32>(1.0) + hdr), vec3<f32>(1.0 / 2.2));
    return Masks(vec4<f32>(vec3<f32>(visibility.r), 1.0), vec4<f32>(vec3<f32>(reactive), 1.0), vec4<f32>(displayed, 1.0), vec4<f32>(vec3<f32>(visibility.g), 1.0));
}";

const STAGE_SOURCE: &str = "
@group(0) @binding(0) var before_temporal: texture_2d<f32>;
@group(0) @binding(1) var history: texture_2d<f32>;
struct Stages {
    @location(0) before_temporal: vec4<f32>,
    @location(1) history: vec4<f32>,
};
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
fn displayed(rgb: vec3<f32>) -> vec4<f32> {
    let hdr = max(rgb, vec3<f32>(0.0));
    return vec4<f32>(pow(hdr / (vec3<f32>(1.0) + hdr), vec3<f32>(1.0 / 2.2)), 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> Stages {
    let at = vec2<i32>(p.xy);
    return Stages(displayed(textureLoad(before_temporal, at, 0).rgb), displayed(textureLoad(history, at, 0).rgb));
}";

const PLANES: usize = 6;

/// Tightly packed RGBA8 diagnostics from one already rendered frame.
/// These visualizations never replace the physical scene or its history.
pub struct TemporalDiagnosticPixels {
    /// Resolved reactive coverage: black is zero; partial MSAA samples are gray.
    pub marker: Vec<u8>,
    /// False when the saved marker texture belongs to an earlier frame.
    pub marker_current: bool,
    /// White where the saved history alpha is negative, black elsewhere.
    pub history_reactive: Vec<u8>,
    /// HDR scene after sharpening, displayed with `(rgb / (1 + rgb))^(1/2.2)`.
    pub hdr_scene: Vec<u8>,
    /// Additive sprite output after later particle coverage, resolved over visible samples.
    pub additive_fx: Vec<u8>,
    /// Current resolved HDR scene before temporal resolve, using the same display curve.
    pub hdr_before_temporal: Vec<u8>,
    /// Newly written temporal history RGB before sharpening, using the same curve.
    pub hdr_history: Vec<u8>,
    /// Actual current jittered world-to-clip matrix used by the temporal resolve.
    pub current: [[f32; 4]; 4],
    /// Actual previous jittered world-to-clip matrix used by the temporal resolve.
    pub previous: [[f32; 4]; 4],
}

impl Offscreen {
    /// Requests a pre-temporal snapshot during the next primary render.
    /// Disable it outside the selected diagnostic frames to release the snapshot.
    ///
    /// # Errors
    /// Returns a message if the physical targets cannot retain temporal history.
    pub fn set_temporal_diagnostic_snapshot(&mut self, enabled: bool) -> Result<(), String> {
        if enabled && !self.temporal_aa_available() {
            return Err("Temporal snapshots require a desktop Medium or High HDR renderer".into());
        }
        if let Some(targets) = &mut self.targets.photo {
            targets.set_temporal_diagnostic_snapshot(&self.device, enabled);
        }
        Ok(())
    }

    /// Reads the last enabled temporal frame without rendering it again.
    /// Two auxiliary draws read the actual textures into separate RGBA8 targets;
    /// neither advances cameras, history, exposure, or simulation. The pre-resolve
    /// scene must have been requested before that frame rendered.
    ///
    /// # Errors
    /// Returns a message without an enabled temporal frame or if readback fails.
    pub fn capture_temporal_diagnostics(&self) -> Result<TemporalDiagnosticPixels, String> {
        let views = self
            .targets
            .photo
            .as_ref()
            .and_then(crate::pbr::gpu::PhotoTargets::temporal_diagnostic_views)
            .ok_or("Temporal diagnostics require an enabled rendered HDR frame")?;
        let before_temporal = views
            .before_temporal
            .ok_or("The pre-temporal diagnostic snapshot was not requested before this frame")?;
        let device = &self.device;
        let textures: [wgpu::Texture; PLANES] = std::array::from_fn(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("verse temporal diagnostic output"),
                size: extent(self.width, self.height),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let outputs = textures
            .each_ref()
            .map(|t| t.create_view(&Default::default()));
        let pipeline = readback_pipeline(device, SOURCE, 4);
        let stage_pipeline = readback_pipeline(device, STAGE_SOURCE, 2);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse temporal diagnostic sources"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(views.marker),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(views.history),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(views.scene),
                },
            ],
        });
        let stage_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse temporal diagnostic color stages"),
            layout: &stage_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(before_temporal),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(views.history),
                },
            ],
        });
        let row = (self.width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let plane = u64::from(row) * u64::from(self.height);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse temporal diagnostic readback"),
            size: plane * PLANES as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        // The device requests the portable floor of four color attachments.
        // Keep the six diagnostic planes in passes of four and two.
        for (outputs, pipeline, group) in [
            (&outputs[..4], &pipeline, &group),
            (&outputs[4..], &stage_pipeline, &stage_group),
        ] {
            let attachments: Vec<_> = outputs
                .iter()
                .map(|view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })
                })
                .collect();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse temporal diagnostic readback"),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
        for (index, texture) in textures.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: index as u64 * plane,
                        bytes_per_row: Some(row),
                        rows_per_image: Some(self.height),
                    },
                },
                extent(self.width, self.height),
            );
        }
        let submission = self.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = send.send(result);
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .map_err(|error| format!("Temporal diagnostic readback did not finish: {error}"))?;
        receive
            .recv()
            .map_err(|error| format!("Temporal diagnostic mapping did not return: {error}"))?
            .map_err(|error| format!("Temporal diagnostic buffer could not be mapped: {error}"))?;
        let mapped = readback.get_mapped_range(..);
        let [
            marker,
            history_reactive,
            hdr_scene,
            additive_fx,
            hdr_before_temporal,
            hdr_history,
        ] = unpack(&mapped, row, self.width, self.height)?;
        drop(mapped);
        readback.unmap();
        Ok(TemporalDiagnosticPixels {
            marker,
            marker_current: views.marker_current,
            history_reactive,
            hdr_scene,
            additive_fx,
            hdr_before_temporal,
            hdr_history,
            current: views.current,
            previous: views.previous,
        })
    }
}

fn readback_pipeline(device: &wgpu::Device, source: &str, planes: usize) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("verse temporal diagnostics"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let formats: Vec<Option<wgpu::ColorTargetState>> = (0..planes)
        .map(|_| Some(wgpu::TextureFormat::Rgba8Unorm.into()))
        .collect();
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("verse temporal diagnostic readback"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &formats,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn unpack(bytes: &[u8], row: u32, width: u32, height: u32) -> Result<[Vec<u8>; PLANES], String> {
    let tight = width as usize * 4;
    let plane = row as usize * height as usize;
    if row == 0
        || width == 0
        || height == 0
        || (row as usize) < tight
        || bytes.len() != plane * PLANES
    {
        return Err("Temporal diagnostic buffer has an invalid row or plane layout".into());
    }
    Ok(std::array::from_fn(|index| {
        bytes[index * plane..(index + 1) * plane]
            .chunks_exact(row as usize)
            .flat_map(|line| line[..tight].iter().copied())
            .collect()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readback_preserves_plane_order_and_removes_row_padding() {
        let mut bytes = vec![255; PLANES * 2 * 16];
        for plane in 0..PLANES {
            for row in 0..2 {
                for pixel in 0..2 {
                    let start = plane * 32 + row * 16 + pixel * 4;
                    bytes[start..start + 4].copy_from_slice(&[
                        plane as u8,
                        row as u8,
                        pixel as u8,
                        128,
                    ]);
                }
            }
        }
        let decoded = unpack(&bytes, 16, 2, 2).unwrap();
        for (plane, values) in decoded.iter().enumerate() {
            assert_eq!(values.len(), 16);
            assert_eq!(values[0..4], [plane as u8, 0, 0, 128]);
            assert_eq!(values[12..16], [plane as u8, 1, 1, 128]);
            assert!(!values.contains(&255));
        }
        assert!(unpack(&bytes[..bytes.len() - 1], 16, 2, 2).is_err());
        assert!(unpack(&bytes, 4, 2, 2).is_err());
    }
}
