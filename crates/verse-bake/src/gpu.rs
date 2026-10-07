//! The GPU backend: the scene's triangles in one hardware acceleration
//! structure, traced by a compute shader through `wgpu`'s experimental ray
//! queries.
//!
//! Every triangle is built opaque, so the shader needs only a ray query's
//! committed nearest hit, which every backend that supports ray queries
//! answers. Partial occluders, such as leaf cards and glass, are handled the
//! way the CPU walk treats them: the shader looks up the nearest triangle's
//! opacity, and if light passes it, continues the query just past that hit,
//! multiplying the transmittance, until a solid triangle, open sky, or
//! [`LAYERS`] crossings. Written from the ray query model in the Vulkan and
//! WGSL specifications; no vendor SDK code is used or linked.
//!
//! The backend runs on Vulkan adapters only, such as `coderos-4080`'s, and
//! refuses Metal: see [`GpuBackend::new`].

use wgpu::util::DeviceExt;

use crate::backend::{Backend, MISS, Ray, RayHit};
use crate::scene::Triangle;

/// The most crossings the shader follows along one ray.
pub const LAYERS: u32 = 64;
/// Rays per dispatch: the most workgroups a dispatch may name, each of 64.
const DISPATCH_RAYS: usize = 65_535 * 64;

const SHADER: &str = r"
enable wgpu_ray_query;

struct Ray {
    origin: vec3<f32>,
    reach: f32,
    dir: vec3<f32>,
    kind: u32,
}

struct Hit {
    transmittance: f32,
    distance: f32,
    triangle: u32,
    pad: u32,
}

@group(0) @binding(0) var scene: acceleration_structure;
@group(0) @binding(1) var<storage, read> opacity: array<f32>;
@group(0) @binding(2) var<storage, read> rays: array<Ray>;
@group(0) @binding(3) var<storage, read_write> hits: array<Hit>;
@group(0) @binding(4) var<uniform> count: vec4<u32>;

// The CPU walk's constants: crate::backend::{T_MIN, DARK, SHADOW, MISS} and
// verse_pbr::pbr::bake::SOLID.
const T_MIN: f32 = 1e-4;
const DARK: f32 = 1e-3;
const SOLID: f32 = 0.999;
const SHADOW: u32 = 1u;
const MISS: u32 = 0xffffffffu;
const LAYERS: u32 = 64u;

@compute @workgroup_size(64)
fn trace(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= count.x) {
        return;
    }
    let ray = rays[i];
    var transmittance = 1.0;
    var distance = 0.0;
    var triangle = MISS;
    var start = T_MIN;
    for (var layer = 0u; layer < LAYERS; layer++) {
        if (start >= ray.reach) {
            break;
        }
        var query: ray_query;
        rayQueryInitialize(&query, scene,
            RayDesc(RAY_FLAG_FORCE_OPAQUE, 0xffu, start, ray.reach, ray.origin, ray.dir));
        // Every triangle is opaque, so one step finishes the traversal.
        _ = rayQueryProceed(&query);
        let hit = rayQueryGetCommittedIntersection(&query);
        if (hit.kind == RAY_QUERY_INTERSECTION_NONE) {
            break;
        }
        if (triangle == MISS) {
            triangle = hit.primitive_index;
            distance = hit.t;
        }
        let o = opacity[hit.primitive_index];
        if (o >= SOLID) {
            transmittance = 0.0;
            break;
        }
        transmittance *= 1.0 - o;
        if (ray.kind == SHADOW && transmittance < DARK) {
            transmittance = 0.0;
            break;
        }
        start = hit.t + max(hit.t * 1e-6, T_MIN);
    }
    if (ray.kind == SHADOW) {
        triangle = MISS;
        distance = 0.0;
    }
    hits[i] = Hit(transmittance, distance, triangle, 0u);
}
";

/// The scene on a GPU that supports ray queries.
pub struct GpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
    /// `None` for a scene with no triangles, whose rays all miss.
    traced: Option<Traced>,
}

struct Traced {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    tlas: wgpu::Tlas,
    opacity: wgpu::Buffer,
    // Keeps the bottom level and its vertices alive with the top level.
    _blas: wgpu::Blas,
    _vertices: wgpu::Buffer,
}

impl GpuBackend {
    /// Builds the acceleration structure over `triangles` on the first
    /// Vulkan adapter that supports ray queries.
    ///
    /// # Errors
    ///
    /// Returns why no ray-query adapter is available, or why building
    /// failed.
    pub fn new(triangles: &[Triangle]) -> Result<Self, String> {
        // Vulkan only. Metal's translation of ray queries is incomplete in
        // this wgpu release: a query never reports its traversal finished,
        // and a shader that waits for it hung a Mac's GPU until the window
        // server's watchdog restarted the machine.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        let info = adapter.get_info();
        let name = format!("gpu:{:?}:{}", info.backend, info.name);
        if !adapter
            .features()
            .contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY)
        {
            return Err(format!("{name} does not support ray queries"));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("verse-bake"),
            required_features: wgpu::Features::EXPERIMENTAL_RAY_QUERY,
            required_limits: adapter.limits(),
            // SAFETY: the backend uses only ray queries against opaque
            // geometry it builds itself, the feature's documented path.
            experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
            ..Default::default()
        }))
        .map_err(|e| format!("{name}: {e}"))?;
        if info.backend != wgpu::Backend::Vulkan {
            return Err(format!("{name} is not a Vulkan adapter"));
        }
        let traced = if triangles.is_empty() {
            None
        } else {
            Some(Self::build(&device, &queue, triangles))
        };
        Ok(Self {
            device,
            queue,
            adapter: name,
            traced,
        })
    }

    fn build(device: &wgpu::Device, queue: &wgpu::Queue, triangles: &[Triangle]) -> Traced {
        let positions: Vec<[f32; 3]> = triangles
            .iter()
            .flat_map(|t| t.corners.map(|c| c.to_array()))
            .collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verse-bake vertices"),
            contents: bytemuck::cast_slice(&positions),
            usage: wgpu::BufferUsages::BLAS_INPUT,
        });
        let opacity: Vec<f32> = triangles.iter().map(|t| t.opacity).collect();
        let opacity = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verse-bake opacity"),
            contents: bytemuck::cast_slice(&opacity),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let size = wgpu::BlasTriangleGeometrySizeDescriptor {
            vertex_format: wgpu::VertexFormat::Float32x3,
            vertex_count: positions.len() as u32,
            index_format: None,
            index_count: None,
            flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
        };
        let blas = device.create_blas(
            &wgpu::CreateBlasDescriptor {
                label: Some("verse-bake scene"),
                flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                update_mode: wgpu::AccelerationStructureUpdateMode::Build,
            },
            wgpu::BlasGeometrySizeDescriptors::Triangles {
                descriptors: vec![size.clone()],
            },
        );
        let mut tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("verse-bake instances"),
            max_instances: 1,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });
        tlas[0] = Some(wgpu::TlasInstance::new(
            &blas,
            [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            0,
            0xff,
        ));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("verse-bake build"),
        });
        encoder.build_acceleration_structures(
            std::iter::once(&wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![
                    wgpu::BlasTriangleGeometry {
                        size: &size,
                        vertex_buffer: &vertices,
                        first_vertex: 0,
                        vertex_stride: 12,
                        index_buffer: None,
                        first_index: None,
                        transform_buffer: None,
                        transform_buffer_offset: None,
                    },
                ]),
            }),
            std::iter::once(&tlas),
        );
        queue.submit([encoder.finish()]);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse-bake trace"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse-bake trace"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::AccelerationStructure {
                        vertex_return: false,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, true),
                storage(3, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("verse-bake trace"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("verse-bake trace"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("trace"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Traced {
            pipeline,
            layout,
            tlas,
            opacity,
            _blas: blas,
            _vertices: vertices,
        }
    }

    fn dispatch(&self, traced: &Traced, rays: &[Ray]) -> Result<Vec<RayHit>, String> {
        let device = &self.device;
        let ray_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verse-bake rays"),
            contents: bytemuck::cast_slice(rays),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let size = (rays.len() * std::mem::size_of::<RayHit>()) as u64;
        let hits = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse-bake hits"),
            size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse-bake readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let count = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("verse-bake count"),
            contents: bytemuck::cast_slice(&[rays.len() as u32, 0, 0, 0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse-bake trace"),
            layout: &traced.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: traced.tlas.as_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: traced.opacity.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ray_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: hits.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: count.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("verse-bake trace"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("verse-bake trace"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&traced.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(rays.len().div_ceil(64) as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&hits, 0, &readback, 0, size);
        self.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| drop(send.send(r)));
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(600)),
            })
            .map_err(|e| format!("{}: {e}", self.adapter))?;
        receive
            .recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{}: {e}", self.adapter))?;
        let bytes = readback.slice(..).get_mapped_range();
        Ok(bytemuck::cast_slice::<u8, RayHit>(&bytes).to_vec())
    }
}

impl Backend for GpuBackend {
    fn name(&self) -> String {
        self.adapter.clone()
    }

    fn trace(&mut self, rays: &[Ray]) -> Result<Vec<RayHit>, String> {
        if rays.is_empty() {
            return Ok(Vec::new());
        }
        let Some(traced) = &self.traced else {
            return Ok(vec![RayHit::CLEAR; rays.len()]);
        };
        let mut out = Vec::with_capacity(rays.len());
        for batch in rays.chunks(DISPATCH_RAYS) {
            out.extend(self.dispatch(traced, batch)?);
        }
        debug_assert!(out.iter().all(|h| h.triangle == MISS || h.distance >= 0.0));
        Ok(out)
    }
}
