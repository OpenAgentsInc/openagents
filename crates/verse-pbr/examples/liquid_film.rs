//! A film demo of the particle liquid (#11660): water poured onto a
//! bottle-shaped part runs down its sides and pools in a tray. The CPU
//! reference (`physics::fluid::pbf`, with surface tension, adhesion and a
//! no-slip surface) simulates it; `verse_pbr::fluid::render` draws it as a
//! screen-space surface over a simple lit scene.
//!
//! ```sh
//! cargo run --release -p verse-pbr --example liquid_film -- OUT_DIR [SECONDS] [WIDTH HEIGHT]
//! ```
//!
//! Writes `OUT_DIR/frame-NNNN.png` at 30 frames a second, and
//! `OUT_DIR/liquid-pour.mp4` when `ffmpeg` is on the path.

use std::f64::consts::TAU;
use std::path::PathBuf;

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec3};
use physics::fluid::{Fluid, Settings, Tank};
use verse_pbr::fluid::render::{LiquidView, ScreenSpaceLiquid};
use wgpu::util::DeviceExt;

const RADIUS: f64 = 0.004;
const FPS: u32 = 30;

/// The bottle's outline: radius at each height, m.
const PROFILE: [(f64, f64); 9] = [
    (0.0, 0.045),
    (0.005, 0.05),
    (0.15, 0.05),
    (0.17, 0.046),
    (0.2, 0.026),
    (0.215, 0.017),
    (0.25, 0.016),
    (0.255, 0.019),
    (0.262, 0.019),
];
const TRAY: f64 = 0.14;

fn profile_radius(y: f64) -> Option<f64> {
    PROFILE.windows(2).find_map(|w| {
        let ((y0, r0), (y1, r1)) = (w[0], w[1]);
        (y >= y0 && y <= y1).then(|| r0 + (r1 - r0) * (y - y0) / (y1 - y0))
    })
}

/// Boundary particles about `d` apart over the bottle's side and cap.
fn bottle_points(d: f64) -> Vec<DVec3> {
    let mut points = Vec::new();
    // Walk the outline by arc length.
    let mut s = 0.0;
    let mut path = Vec::new();
    for w in PROFILE.windows(2) {
        let ((y0, r0), (y1, r1)) = (w[0], w[1]);
        let len = ((y1 - y0).powi(2) + (r1 - r0).powi(2)).sqrt();
        path.push((s, len, (y0, r0), (y1, r1)));
        s += len;
    }
    let total = s;
    let rings = (total / d).ceil() as usize;
    for k in 0..=rings {
        let at = total * k as f64 / rings as f64;
        let &(start, len, (y0, r0), (y1, r1)) = path
            .iter()
            .find(|p| at <= p.0 + p.1 + 1e-12)
            .unwrap_or(path.last().unwrap());
        let t = ((at - start) / len).clamp(0.0, 1.0);
        let (y, r) = (y0 + (y1 - y0) * t, r0 + (r1 - r0) * t);
        ring(&mut points, y, r, d);
    }
    // The cap and the base, as filled discs.
    for y in [PROFILE[PROFILE.len() - 1].0, 0.0] {
        let r = if y == 0.0 {
            PROFILE[0].1
        } else {
            PROFILE[PROFILE.len() - 1].1
        };
        let mut rr = r - d;
        while rr > d * 0.5 {
            ring(&mut points, y, rr, d);
            rr -= d;
        }
        points.push(DVec3::new(0.0, y, 0.0));
    }
    points
}

fn ring(points: &mut Vec<DVec3>, y: f64, r: f64, d: f64) {
    let n = ((TAU * r) / d).ceil().max(3.0) as usize;
    for i in 0..n {
        let a = TAU * i as f64 / n as f64;
        points.push(DVec3::new(r * a.cos(), y, r * a.sin()));
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

fn bottle_mesh(out: &mut Vec<Vertex>) {
    let segments = 96;
    let color = [0.12, 0.26, 0.2, 1.0];
    let mut rows = Vec::new();
    for w in PROFILE.windows(2) {
        rows.push(w[0]);
    }
    rows.push(PROFILE[PROFILE.len() - 1]);
    // Surface normals from the outline slope.
    for w in rows.windows(2) {
        let ((y0, r0), (y1, r1)) = (w[0], w[1]);
        let slope = Vec3::new((y1 - y0) as f32, -((r1 - r0) as f32), 0.0).normalize();
        for i in 0..segments {
            let a0 = TAU * i as f64 / segments as f64;
            let a1 = TAU * (i + 1) as f64 / segments as f64;
            let p = |y: f64, r: f64, a: f64| [(r * a.cos()) as f32, y as f32, (r * a.sin()) as f32];
            let n = |a: f64| {
                let (c, s) = (a.cos() as f32, a.sin() as f32);
                [slope.x * c, slope.y, slope.x * s]
            };
            let quad = [
                (p(y0, r0, a0), n(a0)),
                (p(y1, r1, a0), n(a0)),
                (p(y1, r1, a1), n(a1)),
                (p(y0, r0, a0), n(a0)),
                (p(y1, r1, a1), n(a1)),
                (p(y0, r0, a1), n(a1)),
            ];
            for (pos, normal) in quad {
                out.push(Vertex { pos, normal, color });
            }
        }
    }
    let (y, r) = PROFILE[PROFILE.len() - 1];
    for i in 0..segments {
        let a0 = TAU * i as f64 / segments as f64;
        let a1 = TAU * (i + 1) as f64 / segments as f64;
        for (x, z) in [
            (0.0, 0.0),
            (r * a1.cos(), r * a1.sin()),
            (r * a0.cos(), r * a0.sin()),
        ] {
            out.push(Vertex {
                pos: [x as f32, y as f32, z as f32],
                normal: [0.0, 1.0, 0.0],
                color,
            });
        }
    }
}

fn quad(out: &mut Vec<Vertex>, corners: [Vec3; 4], normal: Vec3, color: [f32; 4]) {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(Vertex {
            pos: corners[i].to_array(),
            normal: normal.to_array(),
            color,
        });
    }
}

fn boxed(out: &mut Vec<Vertex>, lo: Vec3, hi: Vec3, color: [f32; 4]) {
    let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    let (a, b) = (lo, hi);
    quad(
        out,
        [
            c(a.x, b.y, a.z),
            c(a.x, b.y, b.z),
            c(b.x, b.y, b.z),
            c(b.x, b.y, a.z),
        ],
        Vec3::Y,
        color,
    );
    quad(
        out,
        [
            c(a.x, a.y, b.z),
            c(b.x, a.y, b.z),
            c(b.x, b.y, b.z),
            c(a.x, b.y, b.z),
        ],
        Vec3::Z,
        color,
    );
    quad(
        out,
        [
            c(b.x, a.y, a.z),
            c(a.x, a.y, a.z),
            c(a.x, b.y, a.z),
            c(b.x, b.y, a.z),
        ],
        -Vec3::Z,
        color,
    );
    quad(
        out,
        [
            c(b.x, a.y, b.z),
            c(b.x, a.y, a.z),
            c(b.x, b.y, a.z),
            c(b.x, b.y, b.z),
        ],
        Vec3::X,
        color,
    );
    quad(
        out,
        [
            c(a.x, a.y, a.z),
            c(a.x, a.y, b.z),
            c(a.x, b.y, b.z),
            c(a.x, b.y, a.z),
        ],
        -Vec3::X,
        color,
    );
}

const SCENE: &str = r"
struct U { view_proj: mat4x4<f32>, view: mat4x4<f32>, sun: vec4<f32> };
@group(0) @binding(0) var<uniform> u: U;
struct V { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>, @location(3) dist: f32 };
@vertex fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>) -> V {
    var o: V;
    o.clip = u.view_proj * vec4<f32>(pos, 1.0);
    o.world = pos;
    o.normal = normal;
    o.color = color;
    o.dist = -(u.view * vec4<f32>(pos, 1.0)).z;
    return o;
}
struct Out { @location(0) color: vec4<f32>, @location(1) depth: f32 };
@fragment fn fs(v: V) -> Out {
    var base = v.color.rgb;
    if v.color.a < 0.5 {
        let c = floor(v.world.xz / 0.05);
        let check = (i32(c.x) + i32(c.y)) & 1;
        base = select(vec3<f32>(0.3, 0.29, 0.27), vec3<f32>(0.22, 0.21, 0.2), check == 1);
    }
    let n = normalize(v.normal);
    let l = normalize(u.sun.xyz);
    let diffuse = max(dot(n, l), 0.0);
    let sky = 0.35 + 0.25 * n.y;
    var o: Out;
    o.color = vec4<f32>(base * (diffuse * 0.9 + sky), 1.0);
    o.depth = v.dist;
    return o;
}
";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .expect("usage: liquid_film OUT_DIR [SECONDS] [W H]"),
    );
    let seconds: f64 = args.get(2).map_or(5.0, |s| s.parse().unwrap());
    let width: u32 = args.get(3).map_or(1920, |s| s.parse().unwrap());
    let height: u32 = args.get(4).map_or(1080, |s| s.parse().unwrap());
    std::fs::create_dir_all(&out).unwrap();

    // The liquid: a tray around a bottle, and a stream from above.
    let settings = Settings {
        radius: RADIUS,
        surface_tension: 0.15,
        adhesion: 6.0,
        boundary_viscosity: 0.08,
        viscosity: 0.03,
        ..Settings::default()
    };
    let tank = Tank {
        min: DVec3::new(-TRAY, 0.0, -TRAY),
        max: DVec3::new(TRAY, 0.06, TRAY),
    };
    let mut fluid = Fluid::new(settings, Some(tank));
    fluid.add_boundary(bottle_points(2.0 * RADIUS));
    let d = 2.0 * RADIUS;
    let nozzle = DVec3::new(0.026, 0.31, 0.012);
    let speed = 1.2;
    let stream = 0.016;
    let mut emitted = 0.0;
    let pour_until = seconds.min(2.5);

    let (device, queue, adapter) = verse_pbr::fluid::headless().expect("a GPU adapter");
    eprintln!("{adapter}: {width}x{height}");
    let color_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let make = |format, usage, label| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let attach = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    let background = make(wgpu::TextureFormat::Rgba16Float, attach, "background");
    let scene_depth = make(wgpu::TextureFormat::R32Float, attach, "scene depth");
    let zbuffer = make(wgpu::TextureFormat::Depth32Float, attach, "scene z");
    let output = make(
        color_format,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        "output",
    );
    let views =
        [&background, &scene_depth, &zbuffer, &output].map(|t| t.create_view(&Default::default()));
    let liquid = ScreenSpaceLiquid::new(&device, width, height, color_format);

    let mut vertices = Vec::new();
    quad(
        &mut vertices,
        [
            Vec3::new(-3.0, 0.0, -3.0),
            Vec3::new(-3.0, 0.0, 3.0),
            Vec3::new(3.0, 0.0, 3.0),
            Vec3::new(3.0, 0.0, -3.0),
        ],
        Vec3::Y,
        [0.0; 4],
    );
    let rim = [0.28, 0.22, 0.17, 1.0];
    let (t, h, w) = (TRAY as f32, 0.06_f32, 0.012_f32);
    boxed(
        &mut vertices,
        Vec3::new(-t - w, 0.0, -t - w),
        Vec3::new(t + w, h, -t),
        rim,
    );
    boxed(
        &mut vertices,
        Vec3::new(-t - w, 0.0, t),
        Vec3::new(t + w, h, t + w),
        rim,
    );
    boxed(
        &mut vertices,
        Vec3::new(-t - w, 0.0, -t),
        Vec3::new(-t, h, t),
        rim,
    );
    boxed(
        &mut vertices,
        Vec3::new(t, 0.0, -t),
        Vec3::new(t + w, h, t),
        rim,
    );
    bottle_mesh(&mut vertices);
    // The spout the water comes from.
    let n = nozzle.as_vec3();
    boxed(
        &mut vertices,
        n + Vec3::new(-0.018, 0.012, -0.018),
        n + Vec3::new(0.018, 0.09, 0.018),
        [0.2, 0.22, 0.25, 1.0],
    );
    let mesh = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scene"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let eye = Vec3::new(0.40, 0.33, 0.58);
    let target = Vec3::new(0.0, 0.15, 0.0);
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let proj = Mat4::perspective_rh(36f32.to_radians(), width as f32 / height as f32, 0.02, 20.0);
    let sun = Vec3::new(0.5, 0.9, 0.35).normalize();
    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct SceneUniform {
        view_proj: [[f32; 4]; 4],
        view: [[f32; 4]; 4],
        sun: [f32; 4],
    }
    let scene_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scene"),
        contents: bytemuck::bytes_of(&SceneUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            sun: sun.extend(0.0).to_array(),
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scene"),
        source: wgpu::ShaderSource::Wgsl(SCENE.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("scene"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[
                Some(wgpu::TextureFormat::Rgba16Float.into()),
                Some(wgpu::TextureFormat::R32Float.into()),
            ],
        }),
        primitive: Default::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let scene_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: scene_uniform.as_entire_binding(),
        }],
    });

    let row = (width * 4).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame"),
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let steps_per_frame = (1.0 / (f64::from(FPS) * settings.dt)).round() as u32;
    let frames = (seconds * f64::from(FPS)).round() as u32;
    let started = std::time::Instant::now();
    for frame in 0..frames {
        for _ in 0..steps_per_frame {
            let t = fluid.tick as f64 * settings.dt;
            if t < pour_until {
                // One disc of particles each time the last has moved a
                // spacing out of the nozzle.
                emitted += speed * settings.dt;
                while emitted >= d {
                    emitted -= d;
                    let n = (stream / d).ceil() as i32;
                    for i in -n..=n {
                        for k in -n..=n {
                            let off = DVec3::new(f64::from(i) * d, 0.0, f64::from(k) * d);
                            if off.length() <= stream {
                                fluid.push(
                                    nozzle + off - DVec3::Y * emitted,
                                    DVec3::new(0.0, -speed, 0.0),
                                );
                            }
                        }
                    }
                }
            }
            fluid.step(None);
        }
        let particles: Vec<[f32; 4]> = fluid
            .positions()
            .iter()
            .map(|p| [p.x as f32, p.y as f32, p.z as f32, 0.0])
            .collect();
        let particles = if particles.is_empty() {
            vec![[0.0, -10.0, 0.0, 0.0]]
        } else {
            particles
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("particles"),
            contents: bytemuck::cast_slice(&particles),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &views[0],
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.6,
                                g: 0.68,
                                b: 0.76,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &views[1],
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 1.0e5,
                                g: 0.0,
                                b: 0.0,
                                a: 0.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &views[2],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &scene_group, &[]);
            pass.set_vertex_buffer(0, mesh.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        liquid.draw(
            &device,
            &mut encoder,
            &LiquidView::water(view, proj, (RADIUS * 1.6) as f32),
            buffer.slice(..),
            particles.len() as u32,
            &views[0],
            &views[1],
            &views[3],
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().unwrap().unwrap();
        let pixels: Vec<u8> = {
            let bytes = readback.slice(..).get_mapped_range().unwrap();
            bytes
                .chunks(row as usize)
                .flat_map(|r| r[..width as usize * 4].to_vec())
                .collect()
        };
        readback.unmap();
        let path = out.join(format!("frame-{frame:04}.png"));
        let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let mut png = png::Encoder::new(file, width, height);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        eprintln!(
            "frame {frame}/{frames}: {} particles, {:.0} s",
            fluid.len(),
            started.elapsed().as_secs_f64()
        );
    }
    let video = out.join("liquid-pour.mp4");
    let encoded = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-framerate",
            &FPS.to_string(),
            "-i",
        ])
        .arg(out.join("frame-%04d.png"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18"])
        .arg(&video)
        .status();
    if matches!(encoded, Ok(s) if s.success()) {
        eprintln!("wrote {}", video.display());
    }
}
