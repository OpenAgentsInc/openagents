//! The OpenAgents homepage field on Aethyr's shared GPU interface.
use af_gpu::*;
pub const SHADER: &str = include_str!("field.wgsl");
pub struct HomeField {
    gpu: Gpu,
    pipeline: RenderPipeline,
    uniforms: Buffer,
    group: BindGroup,
}
impl HomeField {
    pub fn new(gpu: Gpu) -> Self {
        let module = gpu.create_shader_module("home field", SHADER);
        let pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
            label: "home field",
            module: &module,
            vertex_entry: "vs",
            fragment_entry: Some("fs"),
            buffers: vec![],
            cull_mode: CullMode::None,
            depth: None,
            sample_count: 1,
            targets: vec![ColorTarget {
                format: gpu.surface_format(),
                blend: false,
            }],
        });
        let uniforms = gpu.create_buffer(&BufferDescriptor {
            label: "home uniforms",
            size: 16,
            usage: buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        });
        let layout = gpu.render_bind_group_layout(&pipeline, 0);
        let group = gpu.create_bind_group(
            "home uniforms",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer {
                    buffer: &uniforms,
                    offset: 0,
                    size: Some(16),
                },
            }],
        );
        Self {
            gpu,
            pipeline,
            uniforms,
            group,
        }
    }
    pub fn frame(&self, width: u32, height: u32, seconds: f32) {
        let mut data = Vec::with_capacity(16);
        for value in [width.max(1) as f32, height.max(1) as f32, seconds, 0.0] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        self.gpu.write_buffer(&self.uniforms, 0, &data);
        let surface = self.gpu.surface_view();
        let mut encoder = self.gpu.create_command_encoder();
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: "home field",
                color: vec![ColorAttachment {
                    view: &surface,
                    resolve_target: None,
                    load: LoadOp::Clear([0., 0., 0., 1.]),
                    store: true,
                }],
                depth: None,
                timestamps: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group);
            pass.draw(3, 1, 0, 0);
        }
        self.gpu.submit(vec![encoder.finish()]);
        self.gpu.present();
    }
}
// A still frame for reduced-capability hosts. This is the homepage shader's
// scalar field, evaluated once; it needs neither WebGL nor a second GPU API.
fn fract(v: f32) -> f32 {
    v - v.floor()
}
fn hash(x: f32, y: f32) -> f32 {
    fract((x * 127.1 + y * 311.7).sin() * 43758.5453)
}
fn noise(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (fract(x), fract(y));
    let (sx, sy) = (fx * fx * (3. - 2. * fx), fy * fy * (3. - 2. * fy));
    let a = hash(ix, iy) * (1. - sx) + hash(ix + 1., iy) * sx;
    let b = hash(ix, iy + 1.) * (1. - sx) + hash(ix + 1., iy + 1.) * sx;
    a * (1. - sy) + b * sy
}
fn fbm(mut x: f32, mut y: f32) -> f32 {
    let (mut value, mut amplitude) = (0., 0.5);
    for _ in 0..5 {
        value += amplitude * noise(x, y);
        x = x * 2.03 + 1.7;
        y = y * 2.03 + 9.2;
        amplitude *= 0.5;
    }
    value
}
pub fn still_frame(width: u32, height: u32) -> Vec<u8> {
    let (width, height) = (width.clamp(1, 512), height.clamp(1, 512));
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        for px in 0..width {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let (u, v) = (
                (x - 0.5 * width as f32) / height as f32,
                (y - 0.5 * height as f32) / height as f32,
            );
            let time = 20.;
            let t = time * 0.035;
            let (wx, wy) = (
                fbm(u * 1.6 + t, v * 1.6 - t),
                fbm(u * 1.6 - t + 4., v * 1.6 + t * 0.7 + 4.),
            );
            let n = fbm(u * 2.2 + 2.4 * wx, v * 2.2 + 2.4 * wy + t);
            let glow = (-2.2 * (u * u + v * v)).exp();
            let star = if hash((x * 0.5).floor(), (y * 0.5).floor()) >= 0.9975 {
                (0.5 + 0.5 * (time * 1.3 + hash(x.floor(), y.floor()) * 40.).sin())
                    * 0.5
                    * (1. - glow * 0.6)
            } else {
                0.
            };
            for (a, b) in [(0.05, 0.20), (0.055, 0.21), (0.075, 0.27)] {
                let c = (a + (b - a) * n * n) * glow * glow * (0.2 + 0.8 * n) + star;
                pixels.push((c.clamp(0., 1.) * 255.).round() as u8);
            }
            pixels.push(255);
        }
    }
    pixels
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::RefCell;
    thread_local! { static STILL: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) }; }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_home_still(width: u32, height: u32) -> *const u8 {
        STILL.with(|pixels| {
            let mut pixels = pixels.borrow_mut();
            *pixels = still_frame(width, height);
            pixels.as_ptr()
        })
    }
    thread_local! { static HOME: RefCell<Option<HomeField>> = const { RefCell::new(None) }; }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_home_start() {
        HOME.with(|home| *home.borrow_mut() = Some(HomeField::new(Gpu::from_host())));
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_home_frame(width: u32, height: u32, seconds: f32) {
        HOME.with(|home| {
            if let Some(home) = home.borrow().as_ref() {
                home.frame(width, height, seconds);
            }
        });
    }
    #[unsafe(no_mangle)]
    pub extern "C" fn af_home_stop() {
        HOME.with(|home| *home.borrow_mut() = None);
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn still_field_has_clouds_stars_and_bounded_opaque_pixels() {
        let pixels = super::still_frame(64, 128);
        assert_eq!(pixels.len(), 64 * 128 * 4);
        assert!(pixels.chunks_exact(4).all(|p| p[3] == 255));
        assert!(pixels.chunks_exact(4).any(|p| p[2] > 30));
        assert!(pixels.chunks_exact(4).any(|p| p[2] < 16));
        assert_eq!(super::still_frame(0, 0).len(), 4);
        assert_eq!(super::still_frame(u32::MAX, 1).len(), 512 * 4);
    }
    #[test]
    fn shader_keeps_the_public_homepage_visual() {
        let source = include_str!("../../../openagents-web/src/pages/coming_soon.rs");
        let shader = source
            .split("const shader = `")
            .nth(1)
            .unwrap()
            .split("`;")
            .next()
            .unwrap();
        assert_eq!(
            super::SHADER
                .lines()
                .skip(1)
                .collect::<Vec<_>>()
                .join("\n")
                .trim(),
            shader.trim()
        );
    }
    #[test]
    fn homepage_shader_validates() {
        let shader = naga::front::wgsl::parse_str(super::SHADER).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&shader)
        .unwrap();
    }
}
