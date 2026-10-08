//! The bake: sampling, shading, and the passes that turn traced rays into
//! bake products.
//!
//! Each vertex gathers the light arriving over its hemisphere, as the
//! load-time bake in `verse_pbr::pbr::textured_bake` does: open sky where a
//! ray escapes, and where it meets a surface, that surface's diffuse
//! reflection of the sun (through its own shadow ray) and of the indirect
//! light falling on it. The first pass assumes that indirect light is a
//! fixed share of the open sky, which is the load-time bake's single
//! bounce. Every later pass reads it from the previous pass's vertices,
//! interpolated across the triangle the ray met, so `bounces` passes carry
//! light over that many surfaces. The probe grid gathers the last pass's
//! light over the whole sphere into order-one spherical harmonics.
//!
//! Every vertex and probe samples a fixed pattern of directions turned by an
//! angle drawn from the bake's seed and the item's index, so the result
//! depends only on the scene, the light, and the settings. The baker hands
//! the backend batches of rays and reads the answers back in order, so the
//! backend's thread count never changes a value.

use std::f32::consts::{PI, TAU};
use std::time::Instant;

use glam::Vec3;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use verse_pbr::pbr::bake::{SOLID, dilate};
use verse_pbr::pbr::textured_bake::{
    BIAS, BakeLight, FAR_BIAS, FOLIAGE_FLOOR, HIT_AMBIENT, MAX_AMBIENT, SUN_REACH, encode,
};

use crate::backend::{Backend, MISS, NEAREST, Ray, RayHit, SHADOW, for_chunks};
use crate::products::{ProbeLayer, Products};
use crate::scene::{Scene, Triangle, hex};

/// Changes whenever the bake's rules do, so a product keyed by an older
/// [`bake_key`] stops matching.
pub const BAKER_VERSION: u32 = 1;
/// Order-zero and order-one spherical-harmonic basis constants, and the
/// cosine lobe's convolution weights (Ramamoorthi and Hanrahan 2001).
pub(crate) const Y0: f32 = 0.282_095;
pub(crate) const Y1: f32 = 0.488_603;
pub(crate) const A0: f32 = PI;
pub(crate) const A1: f32 = 2.0 * PI / 3.0;
/// The golden angle, which spreads a spiral's points evenly.
pub(crate) const GOLDEN: f32 = 2.399_963_2;
/// The most rays one batch hands a backend.
pub(crate) const BATCH_RAYS: usize = 1 << 20;
/// Items one accumulation worker takes at a time.
pub(crate) const ACCUMULATE_CHUNK: usize = 256;

/// The light a bake gathers: a sun and an open sky whose irradiance runs
/// from `ground` on surfaces facing down to `sky` on surfaces facing up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Light {
    /// Direction toward the sun that lights bounced light.
    pub sun_dir: [f32; 3],
    /// Sun illuminance on a surface facing it, lux.
    pub sun_illuminance: f32,
    /// Open-sky irradiance on surfaces facing up and down, lux.
    pub sky: f32,
    pub ground: f32,
}

impl Light {
    fn bake_light(&self) -> BakeLight {
        BakeLight {
            sun_dir: Vec3::from(self.sun_dir).normalize_or(Vec3::Y),
            sun_illuminance: self.sun_illuminance,
            sky: self.sky,
            ground: self.ground,
        }
    }
}

/// How finely a bake samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Rays over each vertex's hemisphere; a leaf card's vertex traces this
    /// many on each side.
    pub vertex_rays: u32,
    /// Rays over each probe's sphere.
    pub probe_rays: u32,
    /// Surfaces light is carried over, at least one.
    pub bounces: u32,
    /// Distance beyond which a gathering ray counts as open sky, m.
    pub reach: f32,
    /// The probe grid's corners and spacing, m.
    pub probe_min: [f32; 3],
    pub probe_max: [f32; 3],
    pub probe_cell: f32,
    /// Directions toward the sun to bake per-vertex sun visibility for,
    /// such as the hours of a day.
    pub suns: Vec<[f32; 3]>,
    /// Shadow rays per vertex and sun direction, spread over the sun's disk.
    pub sun_rays: u32,
    /// The sun disk's angular radius, radians.
    pub sun_radius: f32,
    /// What turns each item's sampling pattern.
    pub seed: u64,
}

impl Settings {
    /// The baker's default quality for a probe box from `min` to `max`,
    /// with the sun visibility baked toward `light`'s sun.
    #[must_use]
    pub fn new(min: Vec3, max: Vec3, cell: f32, light: &Light) -> Self {
        Self {
            vertex_rays: 128,
            probe_rays: 256,
            bounces: 2,
            reach: 60.0,
            probe_min: min.to_array(),
            probe_max: max.to_array(),
            probe_cell: cell.max(0.25),
            suns: vec![light.sun_dir],
            sun_rays: 4,
            sun_radius: 0.03,
            seed: 0x5eed_0b4c,
        }
    }

    /// Probe counts along each axis, as the load-time bake counts them.
    #[must_use]
    pub fn dims(&self) -> [u32; 3] {
        let size = (Vec3::from(self.probe_max) - Vec3::from(self.probe_min)) / self.probe_cell;
        [size.x, size.y, size.z].map(|d| (d.max(0.0).ceil() as u32 + 1).clamp(2, 64))
    }
}

/// SHA-256 over the scene's digest, the light, the settings, and
/// [`BAKER_VERSION`]. Equal keys mean an equal bake request; products from
/// either backend may be stored under it.
#[must_use]
pub fn bake_key(scene: &Scene, light: &Light, settings: &Settings) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"openagents.verse-bake.key.v1\0");
    hash.update(BAKER_VERSION.to_le_bytes());
    hash.update(scene.digest);
    // Field order is fixed and floats print exactly, so the JSON is stable.
    hash.update(serde_json::to_vec(light).unwrap_or_default());
    hash.update(serde_json::to_vec(settings).unwrap_or_default());
    hash.finalize().into()
}

/// How much one bake traced and how long its phases took.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// Gathering rays that report their nearest triangle.
    pub nearest_rays: u64,
    /// Shadow rays toward the sun.
    pub shadow_rays: u64,
    /// Wall time per pass of the vertex gather, then the sun layers and the
    /// probes, ms.
    pub pass_ms: Vec<u64>,
    pub sun_ms: u64,
    pub probe_ms: u64,
}

/// A bake of `scene` under `light` and `settings` on `backend`.
///
/// # Errors
///
/// Returns the backend's failure.
pub fn bake(
    scene: &Scene,
    light: &Light,
    settings: &Settings,
    backend: &mut dyn Backend,
    threads: usize,
) -> Result<(Products, Stats), String> {
    let mut baker = Baker {
        scene,
        light: light.bake_light(),
        settings,
        backend,
        threads: threads.max(1),
        stats: Stats::default(),
    };
    let mut indirect: Option<Vec<Vec3>> = None;
    let mut gathered = Vec::new();
    for _ in 0..settings.bounces.max(1) {
        let start = Instant::now();
        gathered = baker.gather_vertices(indirect.as_deref())?;
        indirect = Some(
            gathered
                .iter()
                .zip(&scene.vertices)
                .map(|(g, v)| g.multiplier * baker.light.ambient(Vec3::from(v.normal)))
                .collect(),
        );
        baker.stats.pass_ms.push(elapsed_ms(start));
    }
    let start = Instant::now();
    let suns = settings
        .suns
        .iter()
        .map(|&s| baker.sun_layer(Vec3::from(s).normalize_or(Vec3::Y)))
        .collect::<Result<Vec<_>, _>>()?;
    baker.stats.sun_ms = elapsed_ms(start);
    let start = Instant::now();
    let probes = baker.probes(indirect.as_deref())?;
    baker.stats.probe_ms = elapsed_ms(start);
    let products = Products {
        bake_key: hex(&bake_key(scene, light, settings)),
        scene: hex(&scene.digest),
        vertex_light: gathered
            .iter()
            .map(|g| encode(g.multiplier, g.open))
            .collect(),
        vertex_ambient: gathered.iter().map(|g| g.multiplier.to_array()).collect(),
        vertex_open: gathered.iter().map(|g| g.open).collect(),
        suns: settings.suns.clone(),
        vertex_sun: suns,
        probes,
        extra: Vec::new(),
    };
    Ok((products, baker.stats))
}

pub(crate) fn elapsed_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// A vertex's gathered light: a multiplier of the open sky's irradiance per
/// channel, and the cosine-weighted fraction of its hemisphere open to the
/// sky.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gathered {
    multiplier: Vec3,
    open: f32,
}

/// The rays of one gathering item.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sample {
    pub(crate) origin: Vec3,
    /// The surface normal the rays are weighted against.
    pub(crate) normal: Vec3,
}

pub(crate) struct Baker<'a> {
    pub(crate) scene: &'a Scene,
    pub(crate) light: BakeLight,
    pub(crate) settings: &'a Settings,
    pub(crate) backend: &'a mut dyn Backend,
    pub(crate) threads: usize,
    pub(crate) stats: Stats,
}

/// splitmix64.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// An angle in `[0, 2π)` drawn from the seed, a stream, and an index.
pub(crate) fn turn(seed: u64, stream: u64, index: u64) -> f32 {
    let bits = mix(seed ^ mix(stream.wrapping_mul(0x1000_0000_01b3) ^ index));
    (bits >> 40) as f32 / (1u64 << 24) as f32 * TAU
}

/// `count` directions over the hemisphere around `n`, evenly spread in
/// solid angle and turned by `angle`, each with its cosine to `n`.
pub(crate) fn hemisphere(n: Vec3, count: u32, angle: f32, out: &mut Vec<(Vec3, f32)>) {
    let (t, b) = n.any_orthonormal_pair();
    for i in 0..count {
        let z = 1.0 - (i as f32 + 0.5) / count as f32;
        let r = (1.0 - z * z).max(0.0).sqrt();
        let phi = GOLDEN * i as f32 + angle;
        out.push(((t * phi.cos() + b * phi.sin()) * r + n * z, z));
    }
}

/// `count` directions over the whole sphere, turned about the vertical by
/// `angle`.
pub(crate) fn sphere(count: u32, angle: f32) -> impl Iterator<Item = Vec3> {
    (0..count).map(move |i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / count as f32;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let phi = GOLDEN * i as f32 + angle;
        Vec3::new(r * phi.cos(), y, r * phi.sin())
    })
}

/// `count` directions spread over a disk of angular radius `radius` around
/// `s`, turned by `angle`.
pub(crate) fn sun_disk(s: Vec3, radius: f32, count: u32, angle: f32) -> impl Iterator<Item = Vec3> {
    let (t, b) = s.any_orthonormal_pair();
    let spread = radius.tan();
    (0..count).map(move |i| {
        if count == 1 {
            return s;
        }
        let r = spread * ((i as f32 + 0.5) / count as f32).sqrt();
        let phi = GOLDEN * i as f32 + angle;
        (s + (t * phi.cos() + b * phi.sin()) * r).normalize()
    })
}

/// Streams that turn each kind of item's pattern differently.
pub(crate) const VERTEX_STREAM: u64 = 1;
pub(crate) const SUN_STREAM: u64 = 2;
pub(crate) const PROBE_STREAM: u64 = 3;

impl Baker<'_> {
    pub(crate) fn trace(&mut self, rays: &[Ray]) -> Result<Vec<RayHit>, String> {
        for ray in rays {
            if ray.kind == SHADOW {
                self.stats.shadow_rays += 1;
            } else {
                self.stats.nearest_rays += 1;
            }
        }
        let hits = self.backend.trace(rays)?;
        if hits.len() == rays.len() {
            Ok(hits)
        } else {
            Err(format!(
                "{} answered {} of {} rays",
                self.backend.name(),
                hits.len(),
                rays.len()
            ))
        }
    }

    /// The samples of vertex `i`: one side, or both for a leaf card.
    pub(crate) fn vertex_samples(&self, i: usize) -> ([Sample; 2], usize) {
        let v = &self.scene.vertices[i];
        let p = Vec3::from(v.pos);
        let n = Vec3::from(v.normal).normalize_or(Vec3::Y);
        let bias = if self.scene.far[i] { FAR_BIAS } else { BIAS };
        let front = Sample {
            origin: p + n * bias,
            normal: n,
        };
        let back = Sample {
            origin: p - n * bias,
            normal: -n,
        };
        ([front, back], if self.scene.foliage[i] { 2 } else { 1 })
    }

    /// One gathering pass over every vertex, reading the indirect light on
    /// hit surfaces from `indirect` or, on the first pass, assuming it.
    fn gather_vertices(&mut self, indirect: Option<&[Vec3]>) -> Result<Vec<Gathered>, String> {
        let count = self.scene.vertices.len();
        let rays_per = self.settings.vertex_rays.max(1) as usize;
        let mut out = Vec::with_capacity(count);
        let mut start = 0;
        while start < count {
            let end = (start + (BATCH_RAYS / (2 * rays_per)).max(1)).min(count);
            let mut items = Vec::with_capacity(end - start);
            let mut directions = Vec::new();
            for i in start..end {
                let (sides, used) = self.vertex_samples(i);
                let angle = turn(self.settings.seed, VERTEX_STREAM, i as u64);
                for side in &sides[..used] {
                    hemisphere(side.normal, rays_per as u32, angle, &mut directions);
                }
                items.push((sides, used));
            }
            let samples: Vec<Sample> = items
                .iter()
                .flat_map(|(sides, used)| sides[..*used].iter().copied())
                .collect();
            let radiance = self.gather(&samples, &directions, rays_per, indirect)?;
            let mut at = 0;
            for (_, used) in &items {
                let sides = &radiance[at..at + used];
                at += used;
                let gathered = if *used == 2 {
                    // Leaf cards show both faces and pass light through:
                    // average the two sides and keep a floor.
                    Gathered {
                        multiplier: ((sides[0].multiplier + sides[1].multiplier) * 0.5)
                            .max(Vec3::splat(FOLIAGE_FLOOR)),
                        open: (sides[0].open + sides[1].open) * 0.5,
                    }
                } else {
                    sides[0]
                };
                out.push(gathered);
            }
            start = end;
        }
        Ok(out)
    }

    /// Traces `rays_per` of `directions` from each of `samples` and
    /// returns each sample's gathered light.
    fn gather(
        &mut self,
        samples: &[Sample],
        directions: &[(Vec3, f32)],
        rays_per: usize,
        indirect: Option<&[Vec3]>,
    ) -> Result<Vec<Gathered>, String> {
        let reach = self.settings.reach;
        let rays: Vec<Ray> = samples
            .iter()
            .enumerate()
            .flat_map(|(k, s)| {
                directions[k * rays_per..(k + 1) * rays_per]
                    .iter()
                    .map(move |&(d, _)| Ray::new(s.origin, d, reach, NEAREST))
            })
            .collect();
        let hits = self.trace(&rays)?;
        let sun = self.sun_on_hits(&rays, &hits)?;
        let light = self.light;
        let triangles = &self.scene.triangles;
        let mut out = vec![
            Gathered {
                multiplier: Vec3::ONE,
                open: 1.0
            };
            samples.len()
        ];
        let index: Vec<usize> = (0..samples.len()).collect();
        for_chunks(self.threads, &index, &mut out, ACCUMULATE_CHUNK, |&k| {
            let (mut lit, mut reference) = (Vec3::ZERO, 0.0f32);
            let (mut open, mut total) = (0.0f32, 0.0f32);
            for j in k * rays_per..(k + 1) * rays_per {
                let (d, c) = directions[j];
                let sky = light.sky_radiance(d);
                let incoming = incoming(&light, triangles, &rays[j], &hits[j], sun[j], indirect);
                lit += incoming * c;
                reference += sky * c;
                open += hits[j].transmittance * c;
                total += c;
            }
            if reference <= 0.0 || total <= 0.0 {
                Gathered {
                    multiplier: Vec3::ONE,
                    open: 1.0,
                }
            } else {
                Gathered {
                    multiplier: (lit / reference).clamp(Vec3::ZERO, Vec3::splat(MAX_AMBIENT)),
                    open: open / total,
                }
            }
        });
        Ok(out)
    }

    /// The sun's irradiance on the surface each ray met, through a shadow
    /// ray from the hit point; zero where the surface faces away or the
    /// ray met nothing.
    fn sun_on_hits(&mut self, rays: &[Ray], hits: &[RayHit]) -> Result<Vec<f32>, String> {
        self.sun_on_hits_toward(rays, hits, self.light.sun_dir)
    }

    /// [`Self::sun_on_hits`] for a sun toward `s`.
    pub(crate) fn sun_on_hits_toward(
        &mut self,
        rays: &[Ray],
        hits: &[RayHit],
        s: Vec3,
    ) -> Result<Vec<f32>, String> {
        let mut shadow = Vec::new();
        let mut owner = Vec::new();
        let mut cosines = Vec::new();
        for (j, (ray, hit)) in rays.iter().zip(hits).enumerate() {
            if hit.triangle == MISS {
                continue;
            }
            let Some(tri) = self.scene.triangles.get(hit.triangle as usize) else {
                continue;
            };
            let d = Vec3::from(ray.dir);
            let n = facing(tri, d);
            let cos = n.dot(s);
            if cos <= 0.0 {
                continue;
            }
            let point = Vec3::from(ray.origin) + d * hit.distance;
            shadow.push(Ray::new(point + n * BIAS, s, SUN_REACH, SHADOW));
            owner.push(j);
            cosines.push(cos);
        }
        let passed = self.trace(&shadow)?;
        let mut sun = vec![0.0; rays.len()];
        for ((j, cos), hit) in owner.into_iter().zip(cosines).zip(passed) {
            sun[j] = self.light.sun_illuminance * cos * hit.transmittance;
        }
        Ok(sun)
    }

    /// Per-vertex visibility of the sun toward `s`, 0 to 1: the share of the
    /// sun's disk that reaches the vertex. A face turned away reads zero; a
    /// leaf card is seen from whichever side faces the sun.
    pub(crate) fn sun_layer(&mut self, s: Vec3) -> Result<Vec<f32>, String> {
        let count = self.scene.vertices.len();
        let per = self.settings.sun_rays.max(1) as usize;
        let mut out = vec![0.0f32; count];
        let mut start = 0;
        while start < count {
            let end = (start + (BATCH_RAYS / per).max(1)).min(count);
            let mut rays = Vec::new();
            let mut owner = Vec::new();
            for i in start..end {
                let v = &self.scene.vertices[i];
                let mut n = Vec3::from(v.normal).normalize_or(Vec3::Y);
                if self.scene.foliage[i] && n.dot(s) < 0.0 {
                    n = -n;
                }
                if n.dot(s) <= 0.0 {
                    continue;
                }
                let bias = if self.scene.far[i] { FAR_BIAS } else { BIAS };
                let origin = Vec3::from(v.pos) + n * bias;
                let angle = turn(self.settings.seed, SUN_STREAM, i as u64);
                for d in sun_disk(s, self.settings.sun_radius, per as u32, angle) {
                    rays.push(Ray::new(origin, d, SUN_REACH, SHADOW));
                }
                owner.push(i);
            }
            let hits = self.trace(&rays)?;
            for (k, &i) in owner.iter().enumerate() {
                let sum: f32 = hits[k * per..(k + 1) * per]
                    .iter()
                    .map(|h| h.transmittance)
                    .sum();
                out[i] = sum / per as f32;
            }
            start = end;
        }
        Ok(out)
    }

    /// The probe grid: each probe's irradiance over the sphere in order-one
    /// spherical harmonics, with probes buried in geometry filled from their
    /// neighbors.
    fn probes(&mut self, indirect: Option<&[Vec3]>) -> Result<ProbeLayer, String> {
        let dims = self.settings.dims();
        let count = (dims[0] * dims[1] * dims[2]) as usize;
        let per = self.settings.probe_rays.max(1) as usize;
        let origin = Vec3::from(self.settings.probe_min);
        let cell = self.settings.probe_cell;
        let mut data = vec![[0.0f32; 12]; count];
        let mut valid = vec![false; count];
        let mut start = 0;
        while start < count {
            let end = (start + (BATCH_RAYS / per).max(1)).min(count);
            let mut rays = Vec::with_capacity((end - start) * per);
            for index in start..end {
                let [dx, dy, _] = dims.map(|d| d as usize);
                let at = Vec3::new(
                    (index % dx) as f32,
                    ((index / dx) % dy) as f32,
                    (index / (dx * dy)) as f32,
                );
                let p = origin + at * cell;
                let angle = turn(self.settings.seed, PROBE_STREAM, index as u64);
                for d in sphere(per as u32, angle) {
                    rays.push(Ray::new(p, d, self.settings.reach, NEAREST));
                }
            }
            let hits = self.trace(&rays)?;
            let sun = self.sun_on_hits(&rays, &hits)?;
            let weight = 4.0 * PI / per as f32;
            for index in start..end {
                let (mut l0, mut l1) = (Vec3::ZERO, [Vec3::ZERO; 3]);
                let mut backfaces = 0;
                for j in (index - start) * per..(index - start + 1) * per {
                    let d = Vec3::from(rays[j].dir);
                    if let Some(tri) = self.scene.triangles.get(hits[j].triangle as usize)
                        && tri.opacity >= SOLID
                        && tri.normal.dot(d) > 0.0
                    {
                        backfaces += 1;
                    }
                    let radiance = incoming(
                        &self.light,
                        &self.scene.triangles,
                        &rays[j],
                        &hits[j],
                        sun[j],
                        indirect,
                    );
                    l0 += radiance * (Y0 * weight);
                    for (k, axis) in l1.iter_mut().enumerate() {
                        *axis += radiance * (Y1 * d[k] * weight);
                    }
                }
                let probe = &mut data[index];
                for c in 0..3 {
                    probe[c * 4] = A0 * Y0 * l0[c];
                    for k in 0..3 {
                        probe[c * 4 + 1 + k] = A1 * Y1 * l1[k][c];
                    }
                }
                valid[index] = backfaces * 3 < per;
            }
            start = end;
        }
        dilate(&mut data, &valid, dims);
        Ok(ProbeLayer {
            origin: self.settings.probe_min,
            cell,
            dims,
            data,
        })
    }
}

/// The face of `tri` a ray along `d` meets: thin panels reflect from
/// whichever side the ray arrives on.
pub(crate) fn facing(tri: &Triangle, d: Vec3) -> Vec3 {
    if tri.normal.dot(d) > 0.0 {
        -tri.normal
    } else {
        tri.normal
    }
}

/// Radiance arriving along `ray`: the sky through whatever it crosses, and
/// the nearest surface's diffuse reflection in proportion to its opacity.
/// `sun` is the sun's irradiance on that surface; the indirect light on it
/// comes from `indirect`, interpolated over its corners, or is assumed.
fn incoming(
    light: &BakeLight,
    triangles: &[Triangle],
    ray: &Ray,
    hit: &RayHit,
    sun: f32,
    indirect: Option<&[Vec3]>,
) -> Vec3 {
    let d = Vec3::from(ray.dir);
    let mut radiance = Vec3::splat(light.sky_radiance(d) * hit.transmittance);
    let Some(tri) = triangles.get(hit.triangle as usize) else {
        return radiance;
    };
    let n = facing(tri, d);
    let around = match indirect {
        Some(indirect) => {
            let w = tri.weights(Vec3::from(ray.origin) + d * hit.distance);
            let [a, b, c] = tri.vertices.map(|v| indirect[v as usize]);
            a * w.x + b * w.y + c * w.z
        }
        None => Vec3::splat(light.ambient(n) * HIT_AMBIENT),
    };
    radiance += tri.albedo * ((Vec3::splat(sun) + around) / PI) * tri.opacity.min(1.0);
    radiance
}
