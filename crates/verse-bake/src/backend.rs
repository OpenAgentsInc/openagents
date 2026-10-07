//! What a backend does: trace batches of rays against a scene's triangles.
//!
//! The baker keeps its sampling and shading on the CPU and hands each
//! backend only rays, so both backends answer the same questions in the same
//! order and differ only in how a ray meets a triangle. [`CpuBackend`] walks
//! the bake's [`Bvh`] on the machine's cores; the GPU backend
//! (`gpu::GpuBackend`, behind the `gpu` feature) asks the hardware's
//! acceleration structure through ray queries.

use std::sync::Mutex;

use glam::Vec3;
use verse_pbr::pbr::bake::{Bvh, Occluder};

use crate::scene::Triangle;

/// A ray that reports the nearest triangle it crosses and the light that
/// passes every triangle within its reach.
pub const NEAREST: u32 = 0;
/// A ray that reports only the light that passes, and stops once almost
/// none does.
pub const SHADOW: u32 = 1;
/// A ray that reports only the nearest triangle whose normal side faces
/// it; the GPU backend uses it to learn the hardware's winding.
pub const FRONT: u32 = 2;
/// [`RayHit::triangle`] of a ray that crossed nothing.
pub const MISS: u32 = u32::MAX;
/// The nearest distance a ray counts a crossing at, m, so a ray does not meet
/// the surface it starts on.
pub const T_MIN: f32 = 1e-4;
/// The transmittance below which a [`SHADOW`] ray stops and reports zero.
pub const DARK: f32 = 1e-3;

/// One ray, laid out as the GPU backend's shader reads it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Ray {
    pub origin: [f32; 3],
    /// How far the ray looks, m.
    pub reach: f32,
    /// Unit direction.
    pub dir: [f32; 3],
    /// [`NEAREST`] or [`SHADOW`].
    pub kind: u32,
}

impl Ray {
    #[must_use]
    pub fn new(origin: Vec3, dir: Vec3, reach: f32, kind: u32) -> Self {
        Self {
            origin: origin.to_array(),
            reach,
            dir: dir.to_array(),
            kind,
        }
    }
}

/// What one ray met.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RayHit {
    /// The fraction of light that passes every triangle within reach:
    /// zero past a solid one, otherwise the product of one minus each
    /// partial occluder's opacity.
    pub transmittance: f32,
    /// The distance to the nearest triangle, m; meaningful only for a
    /// [`NEAREST`] ray that met one.
    pub distance: f32,
    /// The nearest triangle's index in the scene's list, or [`MISS`]. A
    /// [`SHADOW`] ray leaves it unset.
    pub triangle: u32,
    pub pad: u32,
}

impl RayHit {
    /// A ray that crossed nothing.
    pub const CLEAR: Self = Self {
        transmittance: 1.0,
        distance: 0.0,
        triangle: MISS,
        pad: 0,
    };
}

/// Traces rays against one scene.
pub trait Backend {
    /// The backend's name in receipts, such as `cpu` or `gpu:<adapter>`.
    fn name(&self) -> String;

    /// What each of `rays` meets, in order.
    ///
    /// # Errors
    ///
    /// Returns a message when the device fails.
    fn trace(&mut self, rays: &[Ray]) -> Result<Vec<RayHit>, String>;
}

/// Rays one CPU worker takes at a time.
const CHUNK: usize = 2048;

/// The CPU backend: the bake's [`Bvh`], walked on `threads` cores. Each
/// ray's answer depends only on the ray, so the thread count never changes
/// a result.
pub struct CpuBackend {
    bvh: Bvh,
    threads: usize,
}

impl CpuBackend {
    /// A backend over `triangles` on `threads` workers, at least one.
    #[must_use]
    pub fn new(triangles: &[Triangle], threads: usize) -> Self {
        let bvh = Bvh::from_occluders(triangles.iter().map(|t| Occluder {
            corners: t.corners,
            normal: t.normal,
            albedo: t.albedo,
            opacity: t.opacity,
        }));
        Self {
            bvh,
            threads: threads.max(1),
        }
    }

    /// The machine's cores.
    #[must_use]
    pub fn available_threads() -> usize {
        std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
    }

    fn one(&self, ray: &Ray) -> RayHit {
        let origin = Vec3::from(ray.origin);
        let dir = Vec3::from(ray.dir);
        if ray.kind == SHADOW {
            return RayHit {
                transmittance: self.bvh.transmittance(origin, dir, ray.reach),
                ..RayHit::CLEAR
            };
        }
        let trace = self.bvh.trace_indexed(origin, dir, ray.reach);
        let (distance, triangle) = trace.nearest.unwrap_or((0.0, MISS));
        RayHit {
            transmittance: trace.transmittance,
            distance,
            triangle,
            pad: 0,
        }
    }
}

impl Backend for CpuBackend {
    fn name(&self) -> String {
        "cpu".into()
    }

    fn trace(&mut self, rays: &[Ray]) -> Result<Vec<RayHit>, String> {
        let mut out = vec![RayHit::CLEAR; rays.len()];
        let this = &*self;
        for_chunks(this.threads, rays, &mut out, CHUNK, |ray| this.one(ray));
        Ok(out)
    }
}

/// Fills `out[i]` with `work(&input[i])` on `threads` workers that claim
/// `chunk`-sized runs. Each output depends only on its input, so the
/// result is the same for any thread count.
pub fn for_chunks<I: Sync, O: Send>(
    threads: usize,
    input: &[I],
    out: &mut [O],
    chunk: usize,
    work: impl Fn(&I) -> O + Sync,
) {
    debug_assert_eq!(input.len(), out.len());
    let threads = threads.max(1).min(input.len().div_ceil(chunk).max(1));
    if threads == 1 {
        for (o, i) in out.iter_mut().zip(input) {
            *o = work(i);
        }
        return;
    }
    let runs = Mutex::new(input.chunks(chunk).zip(out.chunks_mut(chunk)));
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let next = runs.lock().map(|mut runs| runs.next());
                    let Ok(Some((inputs, outputs))) = next else {
                        break;
                    };
                    for (o, i) in outputs.iter_mut().zip(inputs) {
                        *o = work(i);
                    }
                }
            });
        }
    });
}
