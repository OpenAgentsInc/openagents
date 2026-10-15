//! A zone's glass tank of particle liquid (#11660), stepped on the water
//! clock and drawn as a screen-space surface ([`crate::fluid::render`])
//! over the scene after its water and blended passes, refracting the
//! opaque scene's copy and hidden behind its depth copy. Only the tiers
//! that copy the scene draw it.
//!
//! The liquid steps on the GPU ([`crate::fluid::GpuFluid`]) where the
//! device runs compute. Verse's scene device asks for the WebGL 2 limits
//! everywhere, with no compute or storage buffers, so in the app the CPU
//! solver (`physics::fluid`) steps it on a thread of its own (inline on
//! the web), and each frame uploads the latest positions it finished.

use glam::{Mat4, Vec3};
use physics::fluid::Fluid;

use crate::fluid::render::{LiquidView, ScreenSpaceLiquid};
use crate::fluid::{GpuFluid, LiquidTank};

/// The most solver steps one frame asks for; a longer frame lets the
/// liquid fall behind the clock rather than stall.
const MAX_STEPS: u32 = 8;

enum Solver {
    Gpu(GpuFluid),
    /// The CPU solver and the vertex buffer its positions go to.
    Cpu(Cpu, wgpu::Buffer),
}

enum Cpu {
    Inline(Box<Fluid>),
    #[cfg(not(target_arch = "wasm32"))]
    Worker(Worker),
}

/// The CPU solver on its own thread: it is sent a number of steps, takes
/// them, and sends the positions back. The thread ends when this drops.
#[cfg(not(target_arch = "wasm32"))]
struct Worker {
    steps: std::sync::mpsc::Sender<u32>,
    positions: std::sync::mpsc::Receiver<Vec<[f32; 4]>>,
    busy: bool,
    /// Steps owed while the thread was busy.
    pending: u32,
}

fn pack(fluid: &Fluid) -> Vec<[f32; 4]> {
    fluid
        .positions()
        .iter()
        .map(|p| [p.x as f32, p.y as f32, p.z as f32, 0.0])
        .collect()
}

impl Cpu {
    /// `tank`'s solver, on a thread where one starts.
    fn new(tank: &LiquidTank) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (steps, take) = std::sync::mpsc::channel::<u32>();
            let (give, positions) = std::sync::mpsc::channel();
            let mut fluid = tank.fluid();
            let spawned = std::thread::Builder::new()
                .name("verse tank liquid".into())
                .spawn(move || {
                    while let Ok(n) = take.recv() {
                        for _ in 0..n {
                            fluid.step(None);
                        }
                        if give.send(pack(&fluid)).is_err() {
                            break;
                        }
                    }
                });
            if spawned.is_ok() {
                return Self::Worker(Worker {
                    steps,
                    positions,
                    busy: false,
                    pending: 0,
                });
            }
        }
        Self::Inline(Box::new(tank.fluid()))
    }

    /// Asks for `steps` more steps, and returns positions newer than the
    /// last it returned, if there are any.
    fn advance(&mut self, steps: u32) -> Option<Vec<[f32; 4]>> {
        match self {
            Self::Inline(fluid) => {
                for _ in 0..steps {
                    fluid.step(None);
                }
                (steps > 0).then(|| pack(fluid))
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::Worker(worker) => {
                worker.pending = (worker.pending + steps).min(MAX_STEPS);
                let mut latest = None;
                if worker.busy
                    && let Ok(positions) = worker.positions.try_recv()
                {
                    worker.busy = false;
                    latest = Some(positions);
                }
                if !worker.busy && worker.pending > 0 && worker.steps.send(worker.pending).is_ok() {
                    worker.busy = true;
                    worker.pending = 0;
                }
                latest
            }
        }
    }
}

pub(super) struct TankLiquid {
    tank: LiquidTank,
    solver: Solver,
    dt: f32,
    count: u32,
    /// The water clock when last stepped, s, and the time owed since.
    clock: f32,
    owed: f32,
    screen: Option<([u32; 2], ScreenSpaceLiquid)>,
}

/// Whether `device` runs the GPU solver: compute with the storage buffers
/// its stages bind.
fn computes(device: &wgpu::Device) -> bool {
    let limits = device.limits();
    limits.max_storage_buffers_per_shader_stage >= 5
        && limits.max_compute_workgroup_size_x >= 256
        && limits.max_compute_invocations_per_workgroup >= 256
}

impl TankLiquid {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, tank: LiquidTank, time: f32) -> Self {
        let fluid = tank.fluid();
        let count = fluid.len() as u32;
        let dt = fluid.settings.dt as f32;
        let solver = if computes(device) {
            Solver::Gpu(GpuFluid::new(device, &fluid))
        } else {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("verse tank liquid"),
                size: u64::from(count.max(1)) * 16,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&pack(&fluid)));
            Solver::Cpu(Cpu::new(&tank), buffer)
        };
        Self {
            tank,
            solver,
            dt,
            count,
            clock: time,
            owed: 0.0,
            screen: None,
        }
    }

    /// Steps `slot`'s liquid to `time` (built again when the tank or its
    /// round changed) and draws it into `output`.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        slot: &mut Option<Self>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        tank: LiquidTank,
        time: f32,
        view_proj: Mat4,
        eye: Vec3,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        background: &wgpu::TextureView,
        scene_depth: &wgpu::TextureView,
        output: &wgpu::TextureView,
    ) {
        if slot.as_ref().is_none_or(|liquid| liquid.tank != tank) {
            let screen = slot.take().and_then(|liquid| liquid.screen);
            *slot = Some(Self {
                screen,
                ..Self::new(device, queue, tank, time)
            });
        }
        let Some(liquid) = slot else { return };
        if liquid.count == 0 {
            return;
        }
        liquid.owed += (time - liquid.clock).clamp(0.0, 0.25);
        liquid.clock = time;
        let steps = ((liquid.owed / liquid.dt).floor() as u32).min(MAX_STEPS);
        liquid.owed = if steps == MAX_STEPS {
            0.0
        } else {
            liquid.owed - steps as f32 * liquid.dt
        };
        let particles = match &mut liquid.solver {
            Solver::Gpu(gpu) => {
                if steps > 0 {
                    gpu.encode(encoder, steps);
                }
                &gpu.state
            }
            Solver::Cpu(cpu, buffer) => {
                if let Some(positions) = cpu.advance(steps) {
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&positions));
                }
                &*buffer
            }
        };
        if liquid.screen.as_ref().is_none_or(|(s, _)| *s != size) {
            liquid.screen = Some((
                size,
                ScreenSpaceLiquid::new(device, size[0], size[1], format),
            ));
        }
        let Some((_, screen)) = &liquid.screen else {
            return;
        };
        // The clip w row of a perspective is the view's forward axis, in
        // metres: the same depth the scene's copy holds.
        let forward = view_proj.row(3).truncate().normalize_or(Vec3::NEG_Z);
        let up = if forward.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let view = Mat4::look_to_rh(eye, forward, up);
        let proj = view_proj * view.inverse();
        let look = LiquidView {
            composite: true,
            ..LiquidView::water(view, proj, tank.radius * 1.6)
        };
        screen.draw(
            device,
            encoder,
            &look,
            particles.slice(..u64::from(liquid.count) * 16),
            liquid.count,
            background,
            scene_depth,
            output,
        );
    }
}
