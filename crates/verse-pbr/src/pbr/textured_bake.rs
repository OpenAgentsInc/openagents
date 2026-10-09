//! Baked ambient light for static textured scenes: per-vertex sky
//! visibility with one bounce of sunlight, and a coarse probe grid that
//! characters sample.
//!
//! The bake traces the scene's merged triangles through the [`Bvh`] in
//! [`super::bake`]. Each vertex gathers the radiance arriving over its
//! hemisphere: the open sky where rays escape, and where they hit a surface,
//! that surface's diffuse reflection of the sun (shadowed by its own ray to
//! the sun) and of an assumed share of the sky. Dividing by what the open sky
//! alone would deliver gives a per-channel multiplier of the frame's ambient
//! irradiance, which [`TexturedVertex::light`] carries to the shader. A wall
//! beside sunlit grass so picks up green, and a floor under a roof darkens.
//!
//! Alpha-tested cards and glass are partial occluders: a ray crossing one
//! keeps the fraction of light the card's sampled texels let through, so a
//! canopy dims the ground under it without turning its own leaves black.
//!
//! A far level of detail ([`super::textured::Detail::Far`]) stands where its
//! near level does, so it is baked but never occludes: rays meet the near
//! level's surfaces instead. Its vertices, which lie on or just inside the
//! near surface, start their rays [`FAR_BIAS`] off it so they escape it.
//!
//! The probe grid gathers the same radiance over the whole sphere into
//! order-one spherical harmonics, as [`super::bake::bake_probes`] does for
//! Lagrange. [`AmbientProbes::shade`] fills a posed figure's light channel
//! from it, so characters darken when they walk under cover.
//!
//! [`BakeJob`] runs the bake at zone load on a worker thread, with the
//! vertices split across the machine's cores. Browsers have no threads, so
//! on `wasm32` the job advances a bounded number of vertices and probes per
//! frame at lower quality instead. A finished bake depends only on the
//! scene, the light, and the settings, which [`bake_key`] digests so a cache
//! can skip it.

use std::f32::consts::PI;
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};

use glam::Vec3;

use super::bake::{self, Bvh, Occluder, SOLID, Trace};
use super::textured::{
    AlphaMode, BakedVertices, Level, TexturedMaterial, TexturedScene, TexturedVertex,
    srgb_to_linear,
};
use super::{Key, ProbeGrid};

/// Changes whenever the bake's rules do, so a cached result keyed by
/// [`bake_key`] stops matching.
pub const BAKE_VERSION: u32 = 2;
/// The largest diffuse multiplier the light channel encodes.
pub const MAX_AMBIENT: f32 = 4.0;
/// The share of its open-sky ambient a hit surface is assumed to receive
/// when it reflects light. The bake follows one bounce, so a hit surface's
/// own occlusion is not known.
pub const HIT_AMBIENT: f32 = 0.6;
/// The most light an alpha-tested triangle stops. Its sampled texels miss
/// the gaps between leaves, so even a fully covered card passes some light.
const MASK_MAX_OPACITY: f32 = 0.8;
/// The lowest diffuse multiplier of an alpha-tested vertex, so leaf cards
/// deep in a canopy, lit through their neighbors, never turn black.
pub const FOLIAGE_FLOOR: f32 = 0.3;
/// How far ray origins sit off their surface, m.
pub const BIAS: f32 = 0.02;
/// How far ray origins sit off a far level of detail's surface, m: past
/// where simplifying moved it from the near level's.
pub const FAR_BIAS: f32 = 0.25;
/// How far a ray toward the sun looks for an occluder, m.
pub const SUN_REACH: f32 = 1.0e3;
/// Vertices or probes one [`BakeJob::poll`] advances on a thread-less
/// target.
#[cfg(target_arch = "wasm32")]
const FRAME_BUDGET: usize = 192;
/// Vertices or probes one worker claims at a time.
#[cfg(not(target_arch = "wasm32"))]
const CHUNK: usize = 512;
/// Barycentric points sampled across each triangle for its albedo and
/// coverage: the centroid, toward each corner, and each edge's middle.
const SAMPLES: [[f32; 3]; 7] = [
    [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
    [2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0],
    [1.0 / 6.0, 2.0 / 3.0, 1.0 / 6.0],
    [1.0 / 6.0, 1.0 / 6.0, 2.0 / 3.0],
    [0.5, 0.5, 0.0],
    [0.0, 0.5, 0.5],
    [0.5, 0.0, 0.5],
];
/// Order-zero and order-one spherical-harmonic basis constants, and the
/// cosine lobe's convolution weights (Ramamoorthi and Hanrahan 2001).
const Y0: f32 = 0.282_095;
const Y1: f32 = 0.488_603;
const A0: f32 = PI;
const A1: f32 = 2.0 * PI / 3.0;

/// The light a bake gathers: a sun and an open sky whose irradiance runs
/// linearly from `ground` on surfaces facing down to `sky` on surfaces facing
/// up, as [`Key::probes`] describes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeLight {
    /// Unit direction toward the sun.
    pub sun_dir: Vec3,
    /// Sun illuminance on a surface facing it, lux.
    pub sun_illuminance: f32,
    /// Open-sky irradiance on surfaces facing straight up and straight
    /// down, lux.
    pub sky: f32,
    pub ground: f32,
}

impl BakeLight {
    /// The light of a studio key: its shadowed key light is the sun. The
    /// unshadowed rim light is direct light and is not baked.
    #[must_use]
    pub fn from_key(key: &Key) -> Self {
        Self {
            sun_dir: key.dir.normalize_or(Vec3::Y),
            sun_illuminance: key.illuminance,
            sky: key.sky,
            ground: key.ground,
        }
    }

    /// The order-zero and order-one coefficients of the open-sky
    /// irradiance, `e0 + e1 n.y`.
    fn coefficients(&self) -> (f32, f32) {
        (
            (self.sky + self.ground) * 0.5,
            (self.sky - self.ground) * 0.5,
        )
    }

    /// Open-sky radiance arriving from direction `d`: the linear radiance
    /// whose cosine integral is the irradiance above, cd/m².
    #[must_use]
    pub fn sky_radiance(&self, d: Vec3) -> f32 {
        let (e0, e1) = self.coefficients();
        ((e0 + 1.5 * e1 * d.y) / PI).max(0.0)
    }

    /// Open-sky irradiance on a surface facing `n`, lux.
    #[must_use]
    pub fn ambient(&self, n: Vec3) -> f32 {
        let (e0, e1) = self.coefficients();
        (e0 + e1 * n.y).max(0.0)
    }
}

/// How finely a bake samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeSettings {
    /// Rays over each vertex's hemisphere.
    pub vertex_rays: usize,
    /// Rays over each probe's sphere.
    pub probe_rays: usize,
    /// Distance beyond which a vertex or probe ray counts as open sky, m.
    pub reach: f32,
    /// The probe grid's corners and spacing, m.
    pub probe_min: Vec3,
    pub probe_max: Vec3,
    pub probe_cell: f32,
}

impl BakeSettings {
    /// Settings for probes spaced `cell` meters apart from `min` to `max`.
    /// Browsers bake on the main thread, so they trace fewer rays.
    #[must_use]
    pub fn new(min: Vec3, max: Vec3, cell: f32) -> Self {
        let (vertex_rays, probe_rays) = if cfg!(target_arch = "wasm32") {
            (10, 24)
        } else {
            (24, 64)
        };
        Self {
            vertex_rays,
            probe_rays,
            reach: 60.0,
            probe_min: min,
            probe_max: max,
            probe_cell: cell.max(0.25),
        }
    }

    /// Probe counts along each axis.
    fn dims(&self) -> [u32; 3] {
        let size = (self.probe_max - self.probe_min) / self.probe_cell;
        [size.x, size.y, size.z].map(|d| (d.max(0.0).ceil() as u32 + 1).clamp(2, 64))
    }
}

/// A digest of everything a bake depends on: the content identity of the
/// scene's source (such as a pack's SHA-256), the scene's placements and mesh
/// sizes, the light, the settings, and [`BAKE_VERSION`]. Equal keys mean an
/// equal bake, so a cache may store the result under it.
#[must_use]
pub fn bake_key(
    source: &str,
    scene: &TexturedScene,
    light: &BakeLight,
    settings: &BakeSettings,
) -> u64 {
    // FNV-1a, 64-bit.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(&BAKE_VERSION.to_le_bytes());
    eat(source.as_bytes());
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    eat(&floats(&light.sun_dir.to_array()));
    eat(&floats(&[light.sun_illuminance, light.sky, light.ground]));
    eat(&(settings.vertex_rays as u64).to_le_bytes());
    eat(&(settings.probe_rays as u64).to_le_bytes());
    eat(&floats(&[settings.reach, settings.probe_cell]));
    eat(&floats(&settings.probe_min.to_array()));
    eat(&floats(&settings.probe_max.to_array()));
    for mesh in &scene.meshes {
        for p in &mesh.primitives {
            eat(&(p.vertices.len() as u64).to_le_bytes());
            eat(&(p.indices.len() as u64).to_le_bytes());
            eat(&(p.material as u64).to_le_bytes());
        }
    }
    for placement in &scene.placements {
        eat(&(placement.mesh as u64).to_le_bytes());
        eat(&floats(&placement.transform.to_cols_array()));
        eat(format!("{:?}", placement.detail).as_bytes());
    }
    eat(&floats(&scene.switches));
    hash
}

/// Encodes a diffuse multiplier and an open sky fraction as a
/// [`TexturedVertex::light`]: each color byte is `255 × sqrt(m / 4)`, so dark
/// values keep their precision, and alpha never reaches zero, which marks an
/// unbaked vertex.
#[must_use]
pub fn encode(multiplier: Vec3, open: f32) -> [u8; 4] {
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    let m = multiplier.to_array().map(|c| {
        let c = if c.is_finite() { c } else { 1.0 };
        byte((c / MAX_AMBIENT).max(0.0).sqrt())
    });
    let open = if open.is_finite() { open } else { 1.0 };
    [m[0], m[1], m[2], byte(open).max(1)]
}

/// The diffuse multiplier and open sky fraction a light channel holds, as
/// the textured shader decodes them; an unbaked vertex reads as `(1, 1)`.
#[must_use]
pub fn decode(light: [u8; 4]) -> (Vec3, f32) {
    if light[3] == 0 {
        return (Vec3::ONE, 1.0);
    }
    let unit = |b: u8| f32::from(b) / 255.0;
    let m = Vec3::new(unit(light[0]), unit(light[1]), unit(light[2]));
    (m * m * MAX_AMBIENT, unit(light[3]))
}

/// A finished bake.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneBake {
    /// The [`bake_key`] it was made under.
    pub key: u64,
    /// The scene's merged vertices with their light channel filled, in
    /// [`TexturedScene::merge`]'s order.
    pub vertices: Vec<TexturedVertex>,
    /// What characters sample.
    pub probes: AmbientProbes,
}

/// The probe grid of a bake and the light it was baked under.
#[derive(Clone, Debug, PartialEq)]
pub struct AmbientProbes {
    /// Irradiance in lux, sky and one bounce of sunlight together.
    pub grid: ProbeGrid,
    pub light: BakeLight,
}

impl AmbientProbes {
    /// Baked irradiance at `p` on a surface facing `n`, lux per channel,
    /// sampled as the lit shader samples a grid: half a cell along the
    /// normal, with trilinear weights.
    #[must_use]
    pub fn irradiance(&self, p: Vec3, n: Vec3) -> Vec3 {
        let g = &self.grid;
        if g.data.is_empty() || g.cell <= 0.0 {
            return Vec3::splat(self.light.ambient(n));
        }
        let q = (p + n * g.cell * 0.5 - g.origin) / g.cell;
        let max = Vec3::new(
            (g.dims[0] - 1) as f32,
            (g.dims[1] - 1) as f32,
            (g.dims[2] - 1) as f32,
        );
        let q = q.clamp(Vec3::ZERO, max);
        let base = q.floor().min(max - 1.0).max(Vec3::ZERO);
        let t = q - base;
        let mut sum = Vec3::ZERO;
        for corner in 0..8u32 {
            let offset = Vec3::new(
                (corner & 1) as f32,
                ((corner >> 1) & 1) as f32,
                ((corner >> 2) & 1) as f32,
            );
            let at = base + offset;
            let weight = (Vec3::ONE - offset + (offset * 2.0 - 1.0) * t).element_product();
            if weight <= 0.0 {
                continue;
            }
            let index = at.x as usize
                + at.y as usize * g.dims[0] as usize
                + at.z as usize * (g.dims[0] * g.dims[1]) as usize;
            let Some(probe) = g.data.get(index) else {
                continue;
            };
            let channel = |c: usize| {
                probe[c * 4]
                    + probe[c * 4 + 1] * n.x
                    + probe[c * 4 + 2] * n.y
                    + probe[c * 4 + 3] * n.z
            };
            sum += Vec3::new(channel(0), channel(1), channel(2)) * weight;
        }
        sum.max(Vec3::ZERO)
    }

    /// The diffuse multiplier at `p` facing `n`: baked irradiance over the
    /// open sky's.
    #[must_use]
    pub fn multiplier(&self, p: Vec3, n: Vec3) -> Vec3 {
        let open = self.light.ambient(n);
        if open <= 0.0 {
            return Vec3::ONE;
        }
        (self.irradiance(p, n) / open).clamp(Vec3::ZERO, Vec3::splat(MAX_AMBIENT))
    }

    /// Fills the light channel of posed, world-space vertices, such as a
    /// figure's, from the grid.
    pub fn shade(&self, vertices: &mut [TexturedVertex]) {
        for v in vertices {
            let n = Vec3::from(v.normal).normalize_or(Vec3::Y);
            let m = self.multiplier(Vec3::from(v.pos), n);
            // The grid holds no separate sky fraction; the mean multiplier
            // stands in for it as the reflection occlusion.
            let open = ((m.x + m.y + m.z) / 3.0).clamp(0.0, 1.0);
            v.light = encode(m, open);
        }
    }
}

/// What a bake reads from a scene: its merged vertices, which of them are
/// leaf cards or far levels of detail, and the triangles that occlude, with
/// the albedo and opacity sampled from each one's material.
#[derive(Clone, Debug, PartialEq)]
pub struct BakeGeometry {
    /// The scene's merged vertices, in [`TexturedScene::merge`]'s order.
    pub vertices: Vec<TexturedVertex>,
    /// Whether each vertex belongs to an alpha-tested material.
    pub foliage: Vec<bool>,
    /// Whether each vertex belongs to a far level of detail.
    pub far: Vec<bool>,
    /// Every triangle of a near or single level, in merged index order.
    pub occluders: Vec<Occluder>,
    /// The merged vertices at each occluder's corners.
    pub corners: Vec<[u32; 3]>,
    /// Each occluder's triangle in the merged indices: its first index
    /// over three, which [`super::textured::IndexEdits`] address.
    pub triangles: Vec<u32>,
    /// Every triangle of a near or single level whose material emits.
    pub emitters: Vec<Emitter>,
}

/// A triangle that gives light: a lamp's glass, a flame, or embers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    /// World-space corners, m.
    pub corners: [Vec3; 3],
    /// Emitted luminance per channel, cd/m²: the material's emission times
    /// the triangle's sampled base color.
    pub luminance: Vec3,
}

impl BakeGeometry {
    /// Merges `scene` and samples each triangle's material.
    ///
    /// # Errors
    ///
    /// Returns the scene's validation error.
    pub fn new(scene: &TexturedScene) -> Result<Self, String> {
        let merged = scene.merge()?;
        let mut foliage = vec![false; merged.vertices.len()];
        let mut far = vec![false; merged.vertices.len()];
        let mut occluders = Vec::with_capacity(merged.indices.len() / 3);
        let mut corners_of = Vec::with_capacity(merged.indices.len() / 3);
        let mut triangles = Vec::with_capacity(merged.indices.len() / 3);
        let mut emitters = Vec::new();
        for batch in &merged.batches {
            let material = &scene.materials[batch.material];
            let masked = matches!(material.alpha, AlphaMode::Mask { .. });
            let distant = matches!(
                batch.level,
                Level::Far { .. } | Level::Group { level: 1..=3, .. }
            );
            let range = batch.first as usize..(batch.first + batch.count) as usize;
            let first_triangle = batch.first / 3;
            for (t, triangle) in merged.indices[range].chunks_exact(3).enumerate() {
                let corners =
                    [triangle[0], triangle[1], triangle[2]].map(|i| &merged.vertices[i as usize]);
                if masked {
                    for &i in triangle {
                        foliage[i as usize] = true;
                    }
                }
                if distant {
                    for &i in triangle {
                        far[i as usize] = true;
                    }
                    continue;
                }
                let (albedo, opacity) = surface(scene, material, corners);
                if material.emissive > 0.0 {
                    emitters.push(Emitter {
                        corners: corners.map(|v| Vec3::from(v.pos)),
                        luminance: albedo * material.emissive,
                    });
                }
                occluders.push(Occluder {
                    corners: corners.map(|v| Vec3::from(v.pos)),
                    normal: corners.iter().map(|v| Vec3::from(v.normal)).sum::<Vec3>(),
                    albedo,
                    opacity,
                });
                corners_of.push([triangle[0], triangle[1], triangle[2]]);
                triangles.push(first_triangle + t as u32);
            }
        }
        Ok(Self {
            vertices: merged.vertices,
            foliage,
            far,
            occluders,
            corners: corners_of,
            triangles,
            emitters,
        })
    }
}

/// A bake in progress: the scene's merged vertices and its hierarchy, with
/// the vertices and probes done so far.
pub struct SceneBaker {
    key: u64,
    light: BakeLight,
    settings: BakeSettings,
    bvh: Bvh,
    vertices: Vec<TexturedVertex>,
    /// Whether each vertex belongs to an alpha-tested material.
    foliage: Vec<bool>,
    /// Whether each vertex belongs to a far level of detail.
    far: Vec<bool>,
    hemisphere: Vec<Vec3>,
    sphere: Vec<Vec3>,
    dims: [u32; 3],
    next_vertex: usize,
    probes: Vec<[f32; 12]>,
    valid: Vec<bool>,
}

impl SceneBaker {
    /// Merges `scene` and builds its hierarchy, each triangle carrying the
    /// albedo and opacity sampled from its material.
    ///
    /// # Errors
    ///
    /// Returns the scene's validation error.
    pub fn new(
        scene: &TexturedScene,
        light: BakeLight,
        settings: BakeSettings,
        key: u64,
    ) -> Result<Self, String> {
        Ok(Self::from_geometry(
            BakeGeometry::new(scene)?,
            light,
            settings,
            key,
        ))
    }

    /// A bake of geometry already read from a scene.
    #[must_use]
    pub fn from_geometry(
        geometry: BakeGeometry,
        light: BakeLight,
        settings: BakeSettings,
        key: u64,
    ) -> Self {
        let BakeGeometry {
            vertices,
            foliage,
            far,
            occluders,
            ..
        } = geometry;
        let rays = settings.vertex_rays.max(1);
        let probe_rays = settings.probe_rays.max(1);
        let dims = settings.dims();
        let count = (dims[0] * dims[1] * dims[2]) as usize;
        Self {
            key,
            light: BakeLight {
                sun_dir: light.sun_dir.normalize_or(Vec3::Y),
                ..light
            },
            settings,
            bvh: Bvh::from_occluders(occluders),
            vertices,
            foliage,
            far,
            // About half of a sphere's directions face any one normal.
            hemisphere: bake::sphere_directions(rays * 2),
            sphere: bake::sphere_directions(probe_rays),
            dims,
            next_vertex: 0,
            probes: Vec::with_capacity(count),
            valid: Vec::with_capacity(count),
        }
    }

    /// How many merged vertices the bake lights.
    pub(crate) fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Where merged vertex `i` stands, m.
    pub(crate) fn vertex_position(&self, i: usize) -> Vec3 {
        Vec3::from(self.vertices[i].pos)
    }

    /// Where probe `index` of the grid stands, in x-fastest order, m.
    pub(crate) fn probe_point(&self, index: usize) -> Vec3 {
        let [dx, dy, _] = self.dims.map(|d| d as usize);
        let cell = Vec3::new(
            (index % dx) as f32,
            ((index / dx) % dy) as f32,
            (index / (dx * dy)) as f32,
        );
        self.settings.probe_min + cell * self.settings.probe_cell
    }

    pub(crate) fn probe_count(&self) -> usize {
        (self.dims[0] * self.dims[1] * self.dims[2]) as usize
    }

    /// Whether every vertex and probe is baked.
    #[must_use]
    pub fn done(&self) -> bool {
        self.next_vertex >= self.vertices.len() && self.probes.len() >= self.probe_count()
    }

    /// Bakes up to `budget` more vertices or probes, and reports whether the
    /// bake is done.
    pub fn step(&mut self, budget: usize) -> bool {
        let mut left = budget.max(1);
        while left > 0 && self.next_vertex < self.vertices.len() {
            let i = self.next_vertex;
            let light = self.bake_vertex(i, None);
            self.vertices[i].light = light;
            self.next_vertex += 1;
            left -= 1;
        }
        while left > 0 && self.probes.len() < self.probe_count() {
            let (probe, valid) = self.bake_probe(self.probes.len(), None);
            self.probes.push(probe);
            self.valid.push(valid);
            left -= 1;
        }
        self.done()
    }

    /// Bakes everything on this thread and the machine's other cores, or
    /// returns `None` once `cancel` is set.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self, cancel: &AtomicBool) -> Option<SceneBake> {
        let threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .clamp(1, 8);
        let vertices = self.vertices.len();
        let lights = parallel(vertices, threads, cancel, |i| self.bake_vertex(i, None))?;
        for (v, light) in self.vertices.iter_mut().zip(lights) {
            v.light = light;
        }
        self.next_vertex = vertices;
        let probes = parallel(self.probe_count(), threads, cancel, |i| {
            self.bake_probe(i, None)
        })?;
        for (probe, valid) in probes {
            self.probes.push(probe);
            self.valid.push(valid);
        }
        Some(self.finish())
    }

    /// The finished bake. Probes buried in geometry take their neighbors'
    /// light, so it does not leak from inside solid parts.
    #[must_use]
    pub fn finish(self) -> SceneBake {
        let mut data = self.probes;
        let count = (self.dims[0] * self.dims[1] * self.dims[2]) as usize;
        data.resize(count, [0.0; 12]);
        let mut valid = self.valid;
        valid.resize(count, false);
        bake::dilate(&mut data, &valid, self.dims);
        SceneBake {
            key: self.key,
            vertices: self.vertices,
            probes: AmbientProbes {
                grid: ProbeGrid {
                    origin: self.settings.probe_min,
                    cell: self.settings.probe_cell,
                    dims: self.dims,
                    data,
                    // Never zero and never tagged as a studio key's grid.
                    version: (self.key >> 1) | 1,
                },
                light: self.light,
            },
        }
    }

    /// The diffuse multiplier and open sky fraction at `p` on a surface
    /// facing `n`.
    #[must_use]
    pub fn ambient_at(&self, p: Vec3, n: Vec3) -> (Vec3, f32) {
        self.ambient_off(p, n, BIAS, None)
    }

    /// [`Self::ambient_at`] with rays starting `bias` off the surface,
    /// through the occluders `skip` leaves standing
    /// ([`super::bake::Bvh::trace_masked`]).
    fn ambient_off(&self, p: Vec3, n: Vec3, bias: f32, skip: Option<&[bool]>) -> (Vec3, f32) {
        let Some(n) = n.try_normalize() else {
            return (Vec3::ONE, 1.0);
        };
        let origin = p + n * bias;
        let (mut lit, mut reference) = (Vec3::ZERO, 0.0f32);
        let (mut open, mut total) = (0.0f32, 0.0f32);
        for &d in &self.hemisphere {
            let c = d.dot(n);
            if c <= 0.0 {
                continue;
            }
            let sky = self.light.sky_radiance(d);
            let trace = self.bvh.trace_masked(origin, d, self.settings.reach, skip);
            lit += self.incoming(origin, d, &trace, sky, skip) * c;
            reference += sky * c;
            open += trace.transmittance * c;
            total += c;
        }
        if reference <= 0.0 || total <= 0.0 {
            return (Vec3::ONE, 1.0);
        }
        (lit / reference, open / total)
    }

    /// Vertex `i`'s light channel, through the occluders `skip` leaves
    /// standing.
    pub(crate) fn bake_vertex(&self, i: usize, skip: Option<&[bool]>) -> [u8; 4] {
        let v = &self.vertices[i];
        let p = Vec3::from(v.pos);
        let n = Vec3::from(v.normal);
        let bias = if self.far[i] { FAR_BIAS } else { BIAS };
        if self.foliage[i] {
            // Leaf cards show both faces and pass light through: average the
            // two sides and keep a floor.
            let (front, front_open) = self.ambient_off(p, n, bias, skip);
            let (back, back_open) = self.ambient_off(p, -n, bias, skip);
            let m = ((front + back) * 0.5).max(Vec3::splat(FOLIAGE_FLOOR));
            encode(m, (front_open + back_open) * 0.5)
        } else {
            let (m, open) = self.ambient_off(p, n, bias, skip);
            encode(m, open)
        }
    }

    /// Probe `index` of the grid, in x-fastest order, and whether it lies
    /// in open air rather than inside geometry.
    pub(crate) fn bake_probe(&self, index: usize, skip: Option<&[bool]>) -> ([f32; 12], bool) {
        let [dx, dy, _] = self.dims.map(|d| d as usize);
        let cell = Vec3::new(
            (index % dx) as f32,
            ((index / dx) % dy) as f32,
            (index / (dx * dy)) as f32,
        );
        let p = self.settings.probe_min + cell * self.settings.probe_cell;
        let weight = 4.0 * PI / self.sphere.len() as f32;
        let mut l0 = Vec3::ZERO;
        let mut l1 = [Vec3::ZERO; 3];
        let mut backfaces = 0;
        for &d in &self.sphere {
            let trace = self.bvh.trace_masked(p, d, self.settings.reach, skip);
            if let Some((hit, opacity)) = trace.nearest
                && opacity >= SOLID
                && hit.normal.dot(d) > 0.0
            {
                backfaces += 1;
            }
            let radiance = self.incoming(p, d, &trace, self.light.sky_radiance(d), skip);
            l0 += radiance * (Y0 * weight);
            for (k, axis) in l1.iter_mut().enumerate() {
                *axis += radiance * (Y1 * d[k] * weight);
            }
        }
        let mut probe = [0.0; 12];
        for c in 0..3 {
            probe[c * 4] = A0 * Y0 * l0[c];
            for k in 0..3 {
                probe[c * 4 + 1 + k] = A1 * Y1 * l1[k][c];
            }
        }
        (probe, backfaces * 3 < self.sphere.len())
    }

    /// Radiance arriving at `origin` along `d`: the sky through whatever
    /// the ray crosses, and the nearest surface's reflection in proportion
    /// to its opacity.
    fn incoming(
        &self,
        origin: Vec3,
        d: Vec3,
        trace: &Trace,
        sky: f32,
        skip: Option<&[bool]>,
    ) -> Vec3 {
        let mut radiance = Vec3::splat(sky * trace.transmittance);
        if let Some((hit, opacity)) = trace.nearest {
            let point = origin + d * hit.distance;
            radiance += self.reflected(point, d, hit.normal, hit.albedo, skip) * opacity.min(1.0);
        }
        radiance
    }

    /// Diffuse radiance leaving a hit surface back along the ray: the sun,
    /// shadowed by its own ray, and an assumed share of the open sky.
    fn reflected(
        &self,
        point: Vec3,
        d: Vec3,
        normal: Vec3,
        albedo: Vec3,
        skip: Option<&[bool]>,
    ) -> Vec3 {
        // Thin panels reflect from whichever face the ray meets.
        let n = if normal.dot(d) > 0.0 { -normal } else { normal };
        let s = self.light.sun_dir;
        let cos = n.dot(s);
        let sun = if cos > 0.0 {
            self.light.sun_illuminance
                * cos
                * self
                    .bvh
                    .transmittance_masked(point + n * BIAS, s, SUN_REACH, skip)
        } else {
            0.0
        };
        albedo * ((sun + self.light.ambient(n) * HIT_AMBIENT) / PI)
    }
}

/// `work` for each index below `count`, in index order, spread over
/// `threads` workers that claim [`CHUNK`]-sized runs, or `None` once
/// `cancel` is set.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parallel<T: Send>(
    count: usize,
    threads: usize,
    cancel: &AtomicBool,
    work: impl Fn(usize) -> T + Sync,
) -> Option<Vec<T>> {
    let next = AtomicUsize::new(0);
    let mut parts: Vec<(usize, Vec<T>)> = Vec::new();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done: Vec<(usize, Vec<T>)> = Vec::new();
                    while !cancel.load(Ordering::Relaxed) {
                        let start = next.fetch_add(CHUNK, Ordering::Relaxed);
                        if start >= count {
                            break;
                        }
                        let end = (start + CHUNK).min(count);
                        done.push((start, (start..end).map(&work).collect()));
                    }
                    done
                })
            })
            .collect();
        for worker in workers {
            if let Ok(done) = worker.join() {
                parts.extend(done);
            }
        }
    });
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    parts.sort_by_key(|(start, _)| *start);
    let out: Vec<T> = parts.into_iter().flat_map(|(_, values)| values).collect();
    (out.len() == count).then_some(out)
}

/// The albedo and opacity of a triangle, sampled from its material's image,
/// factor, and vertex colors at [`SAMPLES`].
fn surface(
    scene: &TexturedScene,
    material: &TexturedMaterial,
    corners: [&TexturedVertex; 3],
) -> (Vec3, f32) {
    let table = srgb_to_linear();
    let image = material.image.and_then(|i| scene.images.get(i));
    let factor = Vec3::new(
        material.base_color[0],
        material.base_color[1],
        material.base_color[2],
    );
    let (mut kept, mut kept_color, mut all_color, mut alpha_sum) =
        (0usize, Vec3::ZERO, Vec3::ZERO, 0.0f32);
    for w in SAMPLES {
        let mix = |f: &dyn Fn(&TexturedVertex) -> f32| {
            w[0] * f(corners[0]) + w[1] * f(corners[1]) + w[2] * f(corners[2])
        };
        let u = mix(&|v| v.uv[0]);
        let v = mix(&|v| v.uv[1]);
        let tint = Vec3::new(
            mix(&|v| f32::from(v.color[0])),
            mix(&|v| f32::from(v.color[1])),
            mix(&|v| f32::from(v.color[2])),
        ) / 255.0;
        let tint_alpha = mix(&|v| f32::from(v.color[3])) / 255.0;
        let (texel, texel_alpha) = match image {
            Some(image) if image.width > 0 && image.height > 0 => {
                let x = ((u.rem_euclid(1.0) * image.width as f32) as u32).min(image.width - 1);
                let y = ((v.rem_euclid(1.0) * image.height as f32) as u32).min(image.height - 1);
                let o = (y as usize * image.width as usize + x as usize) * 4;
                match image.rgba.get(o..o + 4) {
                    Some(t) => (
                        Vec3::new(
                            table[t[0] as usize],
                            table[t[1] as usize],
                            table[t[2] as usize],
                        ),
                        f32::from(t[3]) / 255.0,
                    ),
                    None => (Vec3::ONE, 1.0),
                }
            }
            _ => (Vec3::ONE, 1.0),
        };
        let color = texel * factor * tint;
        let alpha = texel_alpha * material.base_color[3] * tint_alpha;
        all_color += color;
        alpha_sum += alpha;
        if material.alpha.keeps(alpha) {
            kept += 1;
            kept_color += color;
        }
    }
    let n = SAMPLES.len() as f32;
    let (albedo, opacity) = match material.alpha {
        AlphaMode::Opaque => (all_color / n, 1.0),
        AlphaMode::Mask { .. } => {
            let albedo = if kept > 0 {
                kept_color / kept as f32
            } else {
                all_color / n
            };
            (albedo, (kept as f32 / n).min(MASK_MAX_OPACITY))
        }
        AlphaMode::Blend => (all_color / n, (alpha_sum / n).clamp(0.0, 1.0)),
    };
    (albedo.clamp(Vec3::ZERO, Vec3::splat(0.95)), opacity)
}

/// Offline-baked layers a [`BakeJob`] may use in place of baking, and how
/// to combine them ([`super::baked_layers`]).
#[derive(Clone, Debug)]
pub struct LayerChoice {
    /// Exact reviewed alternatives, bound to the complete layer artifact.
    pub compatibility: Option<Arc<super::baked_layers::SceneCompatibility>>,
    pub layers: Arc<super::baked_layers::Layers>,
    /// The sun direction whose bounce joins the sky's, if any.
    pub sun: Option<usize>,
    /// How strongly, as [`super::baked_layers::Layers::sun_ratio`] gives.
    pub ratio: f32,
}

impl LayerChoice {
    /// The light and lamp texels and the probes of `scene`, when the layers
    /// were baked for that scene or its explicitly audited equivalent.
    ///
    /// # Errors
    ///
    /// Returns why the layers do not fit: the scene fails to merge, or its
    /// digest or vertex count differs from the layers'.
    pub fn apply(&self, scene: &TexturedScene) -> Result<Layered, String> {
        let merged = scene.merge()?;
        let digest = super::baked_layers::hex(&super::baked_layers::scene_digest(scene, &merged));
        if (digest != self.layers.scene
            && !self
                .compatibility
                .as_ref()
                .is_some_and(|record| record.accepts(&self.layers, &digest, None)))
            || merged.vertices.len() != self.layers.vertex_count()
        {
            return Err(format!(
                "the baked light layers are for scene {}, not {digest}",
                self.layers.scene
            ));
        }
        self.layers.validate()?;
        Ok(Layered {
            lights: self.layers.lights(self.sun, self.ratio),
            lamps: self.layers.lamp_texels(),
            probes: self.layers.probes(self.sun, self.ratio),
        })
    }
}

/// A scene's light from offline-baked layers.
#[derive(Clone, Debug, PartialEq)]
pub struct Layered {
    /// The light texture's texels, in [`TexturedScene::merge`]'s order.
    pub lights: Vec<[u8; 4]>,
    /// The lamp texture's texels, in the same order.
    pub lamps: Vec<[u8; 4]>,
    pub probes: AmbientProbes,
}

/// What a finished job produced.
enum Outcome {
    Baked(SceneBake),
    Layered(Layered),
}

enum JobState {
    /// Validated offline layers waiting for the normal delivery poll.
    #[cfg(any(test, target_arch = "wasm32"))]
    Ready(Option<Layered>),
    /// A worker thread bakes and sends the result.
    #[cfg(not(target_arch = "wasm32"))]
    Thread(std::sync::mpsc::Receiver<Option<Outcome>>),
    /// The caller's thread advances the bake each poll; the baker is built
    /// on the first.
    Stepped(Arc<TexturedScene>, Option<Box<SceneBaker>>),
    Done,
}

/// A scene's light bake, running from zone load. It delivers the baked
/// vertices to the scene's [`BakedVertices`] slot for the renderer, and the
/// probes to the zone through [`Self::poll`]. Given offline-baked layers
/// that fit the scene, it delivers their light instead of baking; given
/// layers for another scene, it bakes as before. Dropping it cancels the
/// bake.
pub struct BakeJob {
    slot: BakedVertices,
    light: BakeLight,
    settings: BakeSettings,
    key: u64,
    cancel: Arc<AtomicBool>,
    state: JobState,
    layered: bool,
}

impl BakeJob {
    /// Starts baking `scene` under `light`: on a worker thread where the
    /// target has threads, otherwise a little on each [`Self::poll`].
    #[must_use]
    pub fn start(
        scene: Arc<TexturedScene>,
        light: BakeLight,
        settings: BakeSettings,
        key: u64,
    ) -> Self {
        Self::start_layered(scene, light, settings, key, None)
    }

    /// [`Self::start`], using `layers` instead when they fit the scene.
    /// Targets without threads prepare matching layers once at load.
    #[must_use]
    pub fn start_layered(
        scene: Arc<TexturedScene>,
        light: BakeLight,
        settings: BakeSettings,
        key: u64,
        layers: Option<LayerChoice>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let slot = scene.baked.clone();
        let state = Self::spawn(&scene, light, settings, key, layers, &cancel)
            .unwrap_or(JobState::Stepped(scene, None));
        Self {
            slot,
            light,
            settings,
            key,
            cancel,
            state,
            layered: false,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn(
        scene: &Arc<TexturedScene>,
        light: BakeLight,
        settings: BakeSettings,
        key: u64,
        layers: Option<LayerChoice>,
        cancel: &Arc<AtomicBool>,
    ) -> Option<JobState> {
        let (send, receive) = std::sync::mpsc::channel();
        let scene = scene.clone();
        let cancel = cancel.clone();
        std::thread::Builder::new()
            .name("verse-light-bake".into())
            .spawn(move || {
                let layered = layers.and_then(|choice| {
                    choice
                        .apply(&scene)
                        .map_err(|error| eprintln!("verse: baking light at load: {error}"))
                        .ok()
                });
                let outcome = match layered {
                    Some(layered) => Some(Outcome::Layered(layered)),
                    None => SceneBaker::new(&scene, light, settings, key)
                        .map_err(|error| eprintln!("verse: light bake unavailable: {error}"))
                        .ok()
                        .and_then(|baker| baker.run(&cancel))
                        .map(Outcome::Baked),
                };
                let _ = send.send(outcome);
            })
            .ok()
            .map(|_| JobState::Thread(receive))
    }

    #[cfg(target_arch = "wasm32")]
    fn spawn(
        scene: &Arc<TexturedScene>,
        _: BakeLight,
        _: BakeSettings,
        _: u64,
        layers: Option<LayerChoice>,
        _: &Arc<AtomicBool>,
    ) -> Option<JobState> {
        Self::inline_layers(scene, layers)
    }

    #[cfg(any(test, target_arch = "wasm32"))]
    fn inline_layers(scene: &TexturedScene, layers: Option<LayerChoice>) -> Option<JobState> {
        let layered = layers?
            .apply(scene)
            .map_err(|error| eprintln!("verse: baking light at load: {error}"))
            .ok()?;
        Some(JobState::Ready(Some(layered)))
    }

    /// Whether the bake has finished or failed.
    #[must_use]
    pub fn finished(&self) -> bool {
        matches!(self.state, JobState::Done)
    }

    /// Whether the job delivered offline-baked layers rather than a bake.
    #[must_use]
    pub fn layered(&self) -> bool {
        self.layered
    }

    /// Advances or checks the bake. The first poll after it finishes
    /// delivers the vertices to the scene's slot and returns the probes.
    pub fn poll(&mut self) -> Option<AmbientProbes> {
        let outcome = match &mut self.state {
            #[cfg(any(test, target_arch = "wasm32"))]
            JobState::Ready(layered) => Some(layered.take().map(Outcome::Layered)),
            #[cfg(not(target_arch = "wasm32"))]
            JobState::Thread(receive) => match receive.try_recv() {
                Ok(outcome) => Some(outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
            },
            JobState::Stepped(scene, baker) => {
                if baker.is_none() {
                    match SceneBaker::new(&**scene, self.light, self.settings, self.key) {
                        Ok(built) => *baker = Some(Box::new(built)),
                        Err(error) => {
                            eprintln!("verse: light bake unavailable: {error}");
                            self.state = JobState::Done;
                            return None;
                        }
                    }
                    // Building the hierarchy is this frame's work.
                    None
                } else if baker.as_mut().is_some_and(|b| b.step(Self::budget())) {
                    baker.take().map(|b| Some(Outcome::Baked((*b).finish())))
                } else {
                    None
                }
            }
            JobState::Done => None,
        }?;
        self.state = JobState::Done;
        match outcome? {
            Outcome::Baked(bake) => {
                self.slot.deliver(bake.vertices);
                Some(bake.probes)
            }
            Outcome::Layered(layered) => {
                self.layered = true;
                self.slot.deliver_lights(layered.lights);
                self.slot.deliver_lamps(layered.lamps);
                Some(layered.probes)
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn budget() -> usize {
        FRAME_BUDGET
    }

    /// A target with threads steps only when its worker could not start, so
    /// it takes bigger steps.
    #[cfg(not(target_arch = "wasm32"))]
    fn budget() -> usize {
        4 * CHUNK
    }
}

impl Drop for BakeJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pbr::textured::{BaseColorImage, Primitive, TexturedMesh, UNBAKED};
    use glam::Mat4;

    const LIGHT: BakeLight = BakeLight {
        sun_dir: Vec3::Y,
        sun_illuminance: 4_000.0,
        sky: 1_200.0,
        ground: 450.0,
    };

    fn settings() -> BakeSettings {
        BakeSettings {
            vertex_rays: 128,
            probe_rays: 256,
            reach: 50.0,
            probe_min: Vec3::new(-2.0, 1.0, 0.0),
            probe_max: Vec3::new(10.0, 2.0, 0.0),
            probe_cell: 1.0,
        }
    }

    /// A quad of half extents `u` and `v` around `center`, facing `normal`.
    fn quad(center: Vec3, u: Vec3, v: Vec3, normal: Vec3, material: usize) -> TexturedMesh {
        let corner =
            |a: f32, b: f32, uv: [f32; 2]| TexturedVertex::new(center + u * a + v * b, normal, uv);
        TexturedMesh {
            primitives: vec![Primitive {
                vertices: vec![
                    corner(-1.0, -1.0, [0.0, 0.0]),
                    corner(1.0, -1.0, [1.0, 0.0]),
                    corner(1.0, 1.0, [1.0, 1.0]),
                    corner(-1.0, 1.0, [0.0, 1.0]),
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                material,
            }],
        }
    }

    fn material(scene: &mut TexturedScene, color: [f32; 3], alpha: AlphaMode) -> usize {
        scene.add_material(TexturedMaterial {
            base_color: [color[0], color[1], color[2], 1.0],
            alpha,
            double_sided: !matches!(alpha, AlphaMode::Opaque),
            ..TexturedMaterial::default()
        })
    }

    fn add(scene: &mut TexturedScene, mesh: TexturedMesh) {
        let mesh = scene.add_mesh(mesh);
        scene.place(mesh, Mat4::IDENTITY);
    }

    /// Open ground 40 m across, gray, facing up.
    fn ground(scene: &mut TexturedScene, color: [f32; 3]) {
        let m = material(scene, color, AlphaMode::Opaque);
        add(
            scene,
            quad(Vec3::ZERO, Vec3::X * 20.0, Vec3::Z * 20.0, Vec3::Y, m),
        );
    }

    fn baker(scene: &TexturedScene) -> SceneBaker {
        SceneBaker::new(scene, LIGHT, settings(), 1).unwrap()
    }

    #[test]
    fn open_ground_sees_the_whole_sky() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let (m, open) = baker(&scene).ambient_at(Vec3::new(1.0, 0.0, 2.0), Vec3::Y);
        assert!(open > 0.99, "{open}");
        for c in m.to_array() {
            assert!((c - 1.0).abs() < 0.02, "{m}");
        }
    }

    #[test]
    fn a_point_under_a_roof_is_occluded() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let roof = material(&mut scene, [0.5; 3], AlphaMode::Opaque);
        add(
            &mut scene,
            quad(
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::X * 4.0,
                Vec3::Z * 4.0,
                -Vec3::Y,
                roof,
            ),
        );
        let baker = baker(&scene);
        let (m, open) = baker.ambient_at(Vec3::ZERO, Vec3::Y);
        assert!(open < 0.3, "{open}");
        assert!(m.max_element() < 0.5, "{m}");
        // Out from under the roof the sky opens again.
        let (_, outside) = baker.ambient_at(Vec3::new(15.0, 0.0, 0.0), Vec3::Y);
        assert!(outside > 0.9, "{outside}");
    }

    #[test]
    fn a_leaf_card_occludes_partially_and_stays_lit() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        // Half the card's texels pass the cutoff.
        let image = scene.add_image(BaseColorImage {
            name: "leaves".into(),
            width: 2,
            height: 2,
            rgba: [255, 0, 0, 255]
                .iter()
                .flat_map(|&a| [60, 140, 40, a])
                .collect(),
        });
        let leaf = scene.add_material(TexturedMaterial {
            image: Some(image),
            alpha: AlphaMode::Mask { cutoff: 0.5 },
            double_sided: true,
            ..TexturedMaterial::default()
        });
        add(
            &mut scene,
            quad(
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::X * 4.0,
                Vec3::Z * 4.0,
                Vec3::Y,
                leaf,
            ),
        );
        let solid = {
            let mut scene = TexturedScene::default();
            ground(&mut scene, [0.3; 3]);
            let roof = material(&mut scene, [0.5; 3], AlphaMode::Opaque);
            add(
                &mut scene,
                quad(
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::X * 4.0,
                    Vec3::Z * 4.0,
                    -Vec3::Y,
                    roof,
                ),
            );
            baker(&scene).ambient_at(Vec3::ZERO, Vec3::Y).1
        };
        let mut baker = baker(&scene);
        let (_, open) = baker.ambient_at(Vec3::ZERO, Vec3::Y);
        assert!(open > solid + 0.15, "{open} against {solid}");
        assert!(open < 0.9, "{open}");
        // The card's own vertices are baked from both sides and never black.
        while !baker.step(64) {}
        let bake = baker.finish();
        let leaves: Vec<_> = bake
            .vertices
            .iter()
            .filter(|v| (v.pos[1] - 2.0).abs() < 1e-4)
            .collect();
        assert_eq!(leaves.len(), 4);
        for v in leaves {
            let (m, open) = decode(v.light);
            assert!(v.light[3] > 0);
            assert!(m.min_element() >= FOLIAGE_FLOOR - 0.02, "{m}");
            assert!(open > 0.0);
        }
    }

    #[test]
    fn a_white_wall_beside_sunlit_grass_turns_green() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.1, 0.5, 0.1]);
        let wall = material(&mut scene, [0.9; 3], AlphaMode::Opaque);
        add(
            &mut scene,
            quad(
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::X * 6.0,
                Vec3::Y * 3.0,
                Vec3::Z,
                wall,
            ),
        );
        let (m, _) = baker(&scene).ambient_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Z);
        assert!(m.y > m.x * 1.2 && m.y > m.z * 1.2, "{m}");
    }

    #[test]
    fn probes_darken_under_cover_and_characters_sample_them() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let roof = material(&mut scene, [0.5; 3], AlphaMode::Opaque);
        add(
            &mut scene,
            quad(
                Vec3::new(-2.0, 2.5, 0.0),
                Vec3::X * 4.0,
                Vec3::Z * 4.0,
                -Vec3::Y,
                roof,
            ),
        );
        let mut baker = baker(&scene);
        while !baker.step(16) {}
        let probes = baker.finish().probes;
        assert_eq!(probes.grid.dims, [13, 2, 2]);
        let covered = probes.multiplier(Vec3::new(-2.0, 1.0, 0.0), Vec3::Y);
        let open = probes.multiplier(Vec3::new(10.0, 1.0, 0.0), Vec3::Y);
        assert!(covered.x < open.x * 0.7, "{covered} against {open}");
        let mut figure = [
            TexturedVertex::new(Vec3::new(-2.0, 1.0, 0.0), Vec3::Y, [0.0; 2]),
            TexturedVertex::new(Vec3::new(10.0, 1.0, 0.0), Vec3::Y, [0.0; 2]),
        ];
        probes.shade(&mut figure);
        let under = decode(figure[0].light).0;
        let clear = decode(figure[1].light).0;
        assert!(figure.iter().all(|v| v.light[3] > 0));
        assert!(under.x < clear.x, "{under} against {clear}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_threaded_bake_matches_the_stepped_one() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let roof = material(&mut scene, [0.5; 3], AlphaMode::Opaque);
        add(
            &mut scene,
            quad(
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::X * 4.0,
                Vec3::Z * 4.0,
                -Vec3::Y,
                roof,
            ),
        );
        let threaded = baker(&scene).run(&AtomicBool::new(false)).unwrap();
        let mut stepped = baker(&scene);
        while !stepped.step(3) {}
        assert_eq!(threaded, stepped.finish());
        assert!(baker(&scene).run(&AtomicBool::new(true)).is_none());
    }

    #[test]
    fn a_job_delivers_vertices_to_the_scene_and_probes_to_the_zone() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let scene = Arc::new(scene);
        let key = bake_key("test", &scene, &LIGHT, &settings());
        let mut job = BakeJob::start(scene.clone(), LIGHT, settings(), key);
        let mut probes = None;
        // Ten seconds at most; a worker finishes this scene in far less.
        for _ in 0..10_000 {
            probes = job.poll();
            if probes.is_some() || job.finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(probes.is_some());
        let lights = scene.baked.take().unwrap();
        assert_eq!(lights.len(), scene.merge().unwrap().vertices.len());
        assert!(lights.iter().all(|light| light[3] > 0));
        assert!(scene.baked.take().is_none());
    }

    fn finish(job: &mut BakeJob) -> Option<AmbientProbes> {
        for _ in 0..10_000 {
            if let Some(probes) = job.poll() {
                return Some(probes);
            }
            if job.finished() {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        None
    }

    /// Layers for `scene` whose sky multiplier is 0.5 everywhere and whose
    /// one lamp reaches vertex 1.
    fn layers_for(scene: &TexturedScene) -> super::super::baked_layers::Layers {
        use super::super::baked_layers::{Layers, Reference, SunLayer, encode_lamp, hex};
        let merged = scene.merge().unwrap();
        let n = merged.vertices.len();
        Layers {
            bake_key: "offline".into(),
            scene: hex(&super::super::baked_layers::scene_digest(scene, &merged)),
            reference: Reference {
                sun: LIGHT.sun_illuminance,
                sky: LIGHT.sky,
                ground: LIGHT.ground,
            },
            sky: vec![encode(Vec3::splat(0.5), 0.8); n],
            sky_probes: vec![[100.0; 12]; 8],
            suns: vec![SunLayer {
                dir: [0.0, 1.0, 0.0],
                vertices: vec![[0, 0, 0, 255]; n],
                probes: vec![[0.0; 12]; 8],
            }],
            lamps: vec![(1, encode_lamp(Vec3::splat(20.0)))],
            probe_origin: [0.0; 3],
            probe_cell: 4.0,
            probe_dims: [2, 2, 2],
        }
    }

    #[test]
    fn reviewed_scene_pair_requires_exact_artifact_recipe_and_target() {
        use super::super::baked_layers::{CompatibleScene, SceneCompatibility, hex, scene_digest};
        use sha2::Digest as _;
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let mut layers = layers_for(&scene);
        let target = hex(&scene_digest(&scene, &scene.merge().unwrap()));
        layers.scene = "reviewed-source".into();
        let bytes = layers.encode();
        let record = SceneCompatibility {
            artifact_sha256: hex(&sha2::Sha256::digest(&bytes)),
            artifact_bytes: bytes.len() as u64,
            baked_scene: layers.scene.clone(),
            baked_key: layers.bake_key.clone(),
            vertices: layers.vertex_count(),
            targets: vec![CompatibleScene {
                scene: target.clone(),
                bake_key: Some("target-recipe".into()),
            }],
        };
        assert!(record.accepts(&layers, &target, Some("target-recipe")));
        assert!(!record.accepts(&layers, &target, Some("other-recipe")));
        assert!(!record.accepts(&layers, "unknown-scene", None));
        let mut altered = layers.clone();
        altered.sky[0][0] ^= 1;
        assert!(!record.accepts(&altered, &target, None));
        altered = layers.clone();
        altered.bake_key = "other-source-recipe".into();
        assert!(!record.accepts(&altered, &target, None));
        let choice = LayerChoice {
            compatibility: Some(Arc::new(record)),
            layers: Arc::new(layers),
            sun: Some(0),
            ratio: 1.0,
        };
        assert!(choice.apply(&scene).is_ok());
        let mut unreviewed = TexturedScene::default();
        ground(&mut unreviewed, [0.31; 3]);
        assert!(choice.apply(&unreviewed).is_err());
        let mut strict = choice;
        strict.compatibility = None;
        assert!(strict.apply(&scene).is_err());
    }

    #[test]
    fn a_job_delivers_layers_baked_for_its_scene_instead_of_baking() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let layers = Arc::new(layers_for(&scene));
        let scene = Arc::new(scene);
        let choice = LayerChoice {
            compatibility: None,
            layers: layers.clone(),
            sun: Some(0),
            ratio: 1.0,
        };
        let mut job = BakeJob::start_layered(scene.clone(), LIGHT, settings(), 1, Some(choice));
        let probes = finish(&mut job).unwrap();
        assert!(job.layered());
        assert_eq!(probes.grid.data[0][0], 100.0);
        assert_eq!(scene.baked.take().unwrap(), layers.sky);
        let lamps = scene.baked.take_lamps().unwrap();
        assert_eq!(lamps[1], layers.lamps[0].1);
        assert_eq!(lamps[0], [0; 4]);
    }

    #[test]
    fn threadless_jobs_deliver_matching_layers_once_without_stepping() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let layers = Arc::new(layers_for(&scene));
        let choice = LayerChoice {
            compatibility: None,
            layers: layers.clone(),
            sun: Some(0),
            ratio: 1.0,
        };
        let state = BakeJob::inline_layers(&scene, Some(choice.clone())).unwrap();
        assert!(matches!(state, JobState::Ready(_)));
        let mut job = BakeJob {
            slot: scene.baked.clone(),
            light: LIGHT,
            settings: settings(),
            key: 1,
            cancel: Arc::new(AtomicBool::new(false)),
            state,
            layered: false,
        };
        assert!(!job.finished());
        assert!(scene.baked.take().is_none());
        assert!(job.poll().is_some());
        assert!(job.finished() && job.layered());
        assert_eq!(scene.baked.take().unwrap(), layers.sky);
        assert_eq!(scene.baked.take_lamps().unwrap()[1], layers.lamps[0].1);
        assert!(job.poll().is_none());
        assert!(scene.baked.take().is_none());
        assert!(BakeJob::inline_layers(&scene, None).is_none());
        let mut other = TexturedScene::default();
        ground(&mut other, [0.8; 3]);
        assert!(BakeJob::inline_layers(&other, Some(choice)).is_none());
    }

    #[test]
    fn a_scene_without_its_bake_data_renders_with_the_load_time_bake() {
        let mut other = TexturedScene::default();
        ground(&mut other, [0.8; 3]);
        let stale = layers_for(&other);
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let scene = Arc::new(scene);
        for layers in [None, Some(stale)] {
            let choice = layers.map(|layers| LayerChoice {
                compatibility: None,
                layers: Arc::new(layers),
                sun: Some(0),
                ratio: 1.0,
            });
            let mut job = BakeJob::start_layered(scene.clone(), LIGHT, settings(), 2, choice);
            assert!(finish(&mut job).is_some());
            assert!(!job.layered());
            let lights = scene.baked.take().unwrap();
            assert_eq!(lights.len(), scene.merge().unwrap().vertices.len());
            // Baked at load: every vertex lit, not the stale layers' 0.5.
            assert!(lights.iter().all(|light| light[3] > 0));
            assert_ne!(lights[0], encode(Vec3::splat(0.5), 0.8));
            assert!(scene.baked.take_lamps().is_none());
        }
    }

    #[test]
    fn the_key_follows_the_sun_and_the_light_channel_round_trips() {
        let mut scene = TexturedScene::default();
        ground(&mut scene, [0.3; 3]);
        let a = bake_key("pack", &scene, &LIGHT, &settings());
        let moved = BakeLight {
            sun_dir: Vec3::new(0.3, 0.9, 0.1).normalize(),
            ..LIGHT
        };
        assert_eq!(a, bake_key("pack", &scene, &LIGHT, &settings()));
        assert_ne!(a, bake_key("pack", &scene, &moved, &settings()));
        assert_ne!(a, bake_key("other", &scene, &LIGHT, &settings()));
        assert_eq!(decode(UNBAKED), (Vec3::ONE, 1.0));
        let (m, open) = decode(encode(Vec3::new(0.25, 1.0, 2.5), 0.5));
        assert!(
            (m - Vec3::new(0.25, 1.0, 2.5)).abs().max_element() < 0.03,
            "{m}"
        );
        assert!((open - 0.5).abs() < 0.01);
        // A fully closed vertex still reads as baked.
        assert!(encode(Vec3::ZERO, 0.0)[3] > 0);
    }
}
