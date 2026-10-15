//! Particle liquid on the GPU (#11660): a wgpu compute port of the CPU
//! position-based fluid in `physics::fluid::pbf`, and its screen-space
//! drawing ([`render`]).
//!
//! The GPU solver runs the same scene the CPU reference builds (its tank,
//! wall particles, mass and kernel), in `f32`, with a counting-sort grid
//! rebuilt every step. Summation order follows the GPU's atomics, so runs
//! are not bit-identical; they are checked against the CPU statistically
//! (`tests::the_gpu_dam_break_matches_the_cpu_reference`). Gameplay stays
//! on the deterministic CPU solver; this path is for visuals and film.
//! Surface tension, adhesion and rigid coupling stay CPU-only for now.

pub mod render;
#[cfg(test)]
mod tests;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use physics::fluid::Fluid;
use wgpu::util::DeviceExt;

const WG: u32 = 128;
const BLOCK: u32 = 256;

pub const SHADER: &str = include_str!("pbf.wgsl");

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Params {
    n: u32,
    ncells: u32,
    nblocks: u32,
    flip: u32,
    dims: [u32; 3],
    iterations: u32,
    origin: [f32; 3],
    h: f32,
    tank_min: [f32; 3],
    radius: f32,
    tank_max: [f32; 3],
    dt: f32,
    gravity: [f32; 3],
    mass: f32,
    rho0: f32,
    eps: f32,
    poly6: f32,
    spiky: f32,
    viscosity: f32,
    tensile: f32,
    w_dq: f32,
    mq: f32,
}

/// The kernels, in dispatch order.
const ENTRIES: [&str; 11] = [
    "predict",
    "clear",
    "count",
    "scan_blocks",
    "scan_totals",
    "scan_add",
    "scatter",
    "lambda",
    "delta",
    "velocity",
    "xsph",
];

/// A liquid stepping on the GPU.
pub struct GpuFluid {
    n: u32,
    ncells: u32,
    nblocks: u32,
    iterations: u32,
    pipelines: Vec<wgpu::ComputePipeline>,
    /// Bind groups for the A-to-B and B-to-A density iterations.
    groups: [wgpu::BindGroup; 2],
    /// Positions, velocities, predicted positions, raw velocities.
    pub state: wgpu::Buffer,
    /// Multipliers then densities, one `f32` per particle each.
    pub scalars: wgpu::Buffer,
    /// Seconds per step.
    pub dt: f32,
}

/// A glass tank of liquid a zone shows (the Water Lab's): a column of
/// water standing at one end that the renderer lets go, a dam break that
/// runs and sloshes. Each new `round` stands the column up again, at the
/// other end on odd rounds. The physical renderer steps it (on the GPU
/// where the device computes, else on a CPU thread) and draws it as a
/// screen-space surface ([`render`]) on the tiers that copy the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiquidTank {
    /// The inside of the tank, world m.
    pub min: Vec3,
    pub max: Vec3,
    /// Particle radius, m.
    pub radius: f32,
    /// The column's length along x, as a fraction of the tank's, and its
    /// height, m.
    pub column: f32,
    pub height: f32,
    pub round: u32,
}

impl LiquidTank {
    /// The scene at the start of this round, on the CPU.
    #[must_use]
    pub fn fluid(&self) -> Fluid {
        use glam::DVec3;
        // Steps of 1/120 s with four density iterations: about one particle
        // radius a step at the dam break's front, and cheap enough to
        // step a thousand particles on a CPU core.
        let settings = physics::fluid::Settings {
            radius: f64::from(self.radius),
            dt: 1.0 / 120.0,
            iterations: 4,
            ..physics::fluid::Settings::default()
        };
        let (min, max) = (self.min.as_dvec3(), self.max.as_dvec3());
        let mut fluid = Fluid::new(settings, Some(physics::fluid::Tank { min, max }));
        let length = (max.x - min.x) * f64::from(self.column);
        let (x0, x1) = if self.round % 2 == 0 {
            (min.x, min.x + length)
        } else {
            (max.x - length, max.x)
        };
        fluid.add_block(
            DVec3::new(x0, min.y, min.z),
            DVec3::new(x1, min.y + f64::from(self.height), max.z),
            DVec3::ZERO,
        );
        fluid
    }
}

/// What a readback returns.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub positions: Vec<Vec3>,
    pub velocities: Vec<Vec3>,
    pub densities: Vec<f32>,
}

impl GpuFluid {
    /// Upload `fluid`'s particles, tank and wall particles. The fluid must
    /// have a tank; its coupled bodies are not carried over.
    pub fn new(device: &wgpu::Device, fluid: &Fluid) -> Self {
        let s = fluid.settings;
        let tank = fluid.tank().expect("the GPU liquid runs in a tank");
        let h = fluid.kernel_radius();
        let n = fluid.len() as u32;
        let margin = h * 2.0;
        let origin = tank.min - glam::DVec3::splat(margin);
        let extent = tank.max - tank.min + glam::DVec3::splat(2.0 * margin);
        let dims = (extent / h).ceil().as_uvec3().max(glam::UVec3::ONE);
        let ncells = dims.x * dims.y * dims.z;
        let nblocks = ncells.div_ceil(BLOCK);
        let iterations = s.iterations.max(1).next_multiple_of(2);
        let cell = |p: glam::DVec3| {
            let c = ((p - origin) / h)
                .floor()
                .as_ivec3()
                .clamp(glam::IVec3::ZERO, dims.as_ivec3() - glam::IVec3::ONE);
            c.x as u32 + dims.x * (c.y as u32 + dims.y * c.z as u32)
        };
        // Wall particles sorted by cell, with each cell's start.
        let mut walls: Vec<(u32, [f32; 4])> = fluid
            .boundary()
            .iter()
            .zip(fluid.boundary_volumes())
            .map(|(p, psi)| (cell(*p), [p.x as f32, p.y as f32, p.z as f32, *psi as f32]))
            .collect();
        walls.sort_by_key(|w| w.0);
        let mut bstarts = vec![0u32; ncells as usize + 1];
        for w in &walls {
            bstarts[w.0 as usize + 1] += 1;
        }
        for c in 0..ncells as usize {
            bstarts[c + 1] += bstarts[c];
        }
        let mut boundary: Vec<[f32; 4]> = walls.into_iter().map(|w| w.1).collect();
        if boundary.is_empty() {
            boundary.push([1e9; 4]);
        }

        let mut state = vec![[0.0f32; 4]; 5 * n as usize];
        for (i, (p, v)) in fluid.positions().iter().zip(fluid.velocities()).enumerate() {
            state[i] = [p.x as f32, p.y as f32, p.z as f32, 0.0];
            state[n as usize + i] = [v.x as f32, v.y as f32, v.z as f32, 0.0];
        }
        let ints_len = 3 * n + ncells + 1 + nblocks;
        let mut ints = vec![0u32; ints_len as usize];
        ints.extend(&bstarts);

        let r = s.radius;
        let kernel_h = h;
        let poly6 = 315.0 / (64.0 * std::f64::consts::PI * kernel_h.powi(9));
        let w = |r2: f64| {
            let d = (kernel_h * kernel_h - r2).max(0.0);
            poly6 * d * d * d
        };
        let params = |flip: u32| Params {
            n,
            ncells,
            nblocks,
            flip,
            dims: dims.to_array(),
            iterations,
            origin: origin.as_vec3().to_array(),
            h: h as f32,
            tank_min: tank.min.as_vec3().to_array(),
            radius: r as f32,
            tank_max: tank.max.as_vec3().to_array(),
            dt: s.dt as f32,
            gravity: s.gravity.as_vec3().to_array(),
            mass: fluid.particle_mass() as f32,
            rho0: s.rest_density as f32,
            eps: fluid.softening() as f32,
            poly6: poly6 as f32,
            spiky: (-45.0 / (std::f64::consts::PI * kernel_h.powi(6))) as f32,
            viscosity: s.viscosity as f32,
            tensile: fluid.tensile_strength() as f32,
            w_dq: w((0.2 * kernel_h).powi(2)) as f32,
            mq: (fluid.particle_mass() / s.rest_density) as f32,
        };
        let storage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("liquid state"),
            contents: bytemuck::cast_slice(&state),
            usage: storage | wgpu::BufferUsages::VERTEX,
        });
        let ints_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("liquid grid"),
            contents: bytemuck::cast_slice(&ints),
            usage: storage,
        });
        let counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("liquid cell counts"),
            size: u64::from(ncells.max(1)) * 4,
            usage: storage,
            mapped_at_creation: false,
        });
        let scalars = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("liquid lambda and density"),
            size: u64::from(2 * n.max(1)) * 4,
            usage: storage,
            mapped_at_creation: false,
        });
        let boundary_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("liquid walls"),
            contents: bytemuck::cast_slice(&boundary),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let uniforms = [0, 1].map(|flip| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("liquid params"),
                contents: bytemuck::bytes_of(&params(flip)),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let rw = wgpu::BufferBindingType::Storage { read_only: false };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("liquid"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, rw),
                entry(2, rw),
                entry(3, rw),
                entry(4, rw),
                entry(5, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let groups = [0, 1].map(|flip| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("liquid"),
                layout: &layout,
                entries: &[
                    (0, &uniforms[flip]),
                    (1, &state_buffer),
                    (2, &ints_buffer),
                    (3, &counts),
                    (4, &scalars),
                    (5, &boundary_buffer),
                ]
                .map(|(binding, buffer)| wgpu::BindGroupEntry {
                    binding,
                    resource: buffer.as_entire_binding(),
                }),
            })
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("liquid"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("liquid"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = ENTRIES
            .iter()
            .map(|entry| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                })
            })
            .collect();
        Self {
            n,
            ncells,
            nblocks,
            iterations,
            pipelines,
            groups,
            state: state_buffer,
            scalars,
            dt: s.dt as f32,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.n as usize
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Record `steps` steps.
    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, steps: u32) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("liquid step"),
            timestamp_writes: None,
        });
        let particles = self.n.div_ceil(WG);
        let cells = self.ncells.div_ceil(WG);
        let p = &self.pipelines;
        for _ in 0..steps {
            pass.set_bind_group(0, &self.groups[0], &[]);
            for (pipeline, groups) in [
                (&p[0], particles),
                (&p[1], cells),
                (&p[2], particles),
                (&p[3], self.nblocks),
                (&p[4], 1),
                (&p[5], self.nblocks),
                (&p[6], particles),
            ] {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(groups, 1, 1);
            }
            for iteration in 0..self.iterations {
                pass.set_bind_group(0, &self.groups[(iteration % 2) as usize], &[]);
                pass.set_pipeline(&p[7]);
                pass.dispatch_workgroups(particles, 1, 1);
                pass.set_pipeline(&p[8]);
                pass.dispatch_workgroups(particles, 1, 1);
            }
            pass.set_bind_group(0, &self.groups[0], &[]);
            pass.set_pipeline(&p[9]);
            pass.dispatch_workgroups(particles, 1, 1);
            pass.set_pipeline(&p[10]);
            pass.dispatch_workgroups(particles, 1, 1);
        }
    }

    /// Step `steps` times and wait for the GPU.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn step(&self, device: &wgpu::Device, queue: &wgpu::Queue, steps: u32) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("liquid"),
        });
        self.encode(&mut encoder, steps);
        queue.submit([encoder.finish()]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Copy positions, velocities and densities back.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn read(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Snapshot {
        let n = u64::from(self.n);
        let state = read_buffer(device, queue, &self.state, 0, 2 * n * 16);
        let scalars = read_buffer(device, queue, &self.scalars, n * 4, n * 4);
        let vectors: &[[f32; 4]] = bytemuck::cast_slice(&state);
        Snapshot {
            positions: vectors[..self.n as usize]
                .iter()
                .map(|v| Vec3::new(v[0], v[1], v[2]))
                .collect(),
            velocities: vectors[self.n as usize..]
                .iter()
                .map(|v| Vec3::new(v[0], v[1], v[2]))
                .collect(),
            densities: bytemuck::cast_slice(&scalars).to_vec(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    offset: u64,
    size: u64,
) -> Vec<u8> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("liquid readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("liquid readback"),
    });
    encoder.copy_buffer_to_buffer(buffer, offset, &staging, 0, size);
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    match rx.recv() {
        Ok(Ok(())) => {}
        _ => return vec![0; size as usize],
    }
    let bytes = staging
        .slice(..)
        .get_mapped_range()
        .map(|b| b.to_vec())
        .unwrap_or_default();
    staging.unmap();
    bytes
}

/// A headless device with the limits the liquid needs, or `None` without a
/// GPU.
#[cfg(not(target_arch = "wasm32"))]
pub fn headless() -> Option<(wgpu::Device, wgpu::Queue, String)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let name = adapter.get_info().name;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .ok()?;
    Some((device, queue, name))
}
