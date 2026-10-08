//! The layered bake: the sky, each sun direction, and the lamps lit
//! separately, so a zone mixes them at run time
//! ([`verse_pbr::pbr::baked_layers`]).
//!
//! Light is linear, so one set of traced rays serves every layer. Each
//! gathering pass traces a vertex's hemisphere once and, where a ray meets a
//! surface, shades that surface once per layer: the sky's reflection of the
//! open sky, each sun's direct light through a shadow ray toward that sun,
//! and the lamps' light. As in [`crate::bake`], the first pass assumes the
//! sky light on hit surfaces, and every later pass reads each layer's light
//! from the previous pass's vertices, so `bounces` passes carry each layer
//! over that many surfaces.
//!
//! Lamps are the scene's emissive triangles, gathered into clusters: triangles
//! whose centers lie in neighboring cells [`LAMP_CLUSTER`] meters across
//! join one lamp. Each cluster is a point light at its
//! center whose intensity is a quarter of its emitted flux over π, the mean
//! projected area of a closed convex emitter (Cauchy's surface area
//! formula). A vertex takes each cluster's direct light through shadow rays
//! toward points around the center, out to the distance where the light
//! falls under [`LAMP_MIN_LUX`]; the light then bounces like the others.
//! Clusters stand in for shapes, so a lamp's light near its glass is
//! approximate.

use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};
use std::time::Instant;

use glam::Vec3;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use verse_pbr::pbr::bake::{SOLID, dilate};
use verse_pbr::pbr::baked_layers::{
    Layers, Reference, SunLayer, encode_lamp, encode_sun, hex as layer_hex,
};
use verse_pbr::pbr::textured_bake::{
    BIAS, FAR_BIAS, FOLIAGE_FLOOR, HIT_AMBIENT, MAX_AMBIENT, encode,
};

use crate::backend::{MISS, NEAREST, Ray, RayHit, SHADOW, for_chunks};
use crate::bake::{
    A0, A1, ACCUMULATE_CHUNK, BATCH_RAYS, Baker, Light, PROBE_STREAM, Sample, Settings,
    VERTEX_STREAM, Y0, Y1, elapsed_ms, facing, hemisphere, sphere, turn,
};
use crate::scene::{Scene, Triangle};

/// Changes whenever the layered bake's rules do.
pub const LAYERS_VERSION: u32 = 1;
/// The most sun directions one layered bake takes.
pub const MAX_SUNS: usize = 8;
/// The size of the cells emissive triangles cluster in, m; triangles in
/// neighboring cells join one lamp.
pub const LAMP_CLUSTER: f32 = 0.5;
/// The irradiance below which a lamp's light is dropped, lux.
pub const LAMP_MIN_LUX: f32 = 0.05;
/// The farthest any lamp reaches, m.
pub const LAMP_MAX_RANGE: f32 = 30.0;
/// Shadow rays per vertex and lamp.
pub const LAMP_SHADOW_RAYS: u32 = 2;
/// Spacing of the grid that finds the lamps near a vertex, m.
const LAMP_GRID: f32 = 4.0;
/// A stream that turns the lamps' shadow rays.
const LAMP_STREAM: u64 = 4;

/// One lamp: a cluster of emissive triangles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    /// The area-weighted center of its triangles, m.
    pub center: Vec3,
    /// How far its triangles reach from the center, m.
    pub radius: f32,
    /// Luminous intensity per channel, cd, in every direction.
    pub intensity: Vec3,
    /// Where its light falls under [`LAMP_MIN_LUX`], m.
    pub range: f32,
}

/// The scene's emissive triangles gathered into lamps.
#[must_use]
pub fn lamps(scene: &Scene) -> Vec<Lamp> {
    let mut cells: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, e) in scene.emitters.iter().enumerate() {
        let c = (e.corners[0] + e.corners[1] + e.corners[2]) / 3.0;
        if !c.is_finite() {
            continue;
        }
        let at = (c / LAMP_CLUSTER).floor();
        cells
            .entry((at.x as i32, at.y as i32, at.z as i32))
            .or_default()
            .push(i);
    }
    // Neighboring cells join, so a lamp a cell boundary crosses stays one.
    let keys: Vec<(i32, i32, i32)> = cells.keys().copied().collect();
    let index: BTreeMap<(i32, i32, i32), usize> =
        keys.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    let mut parent: Vec<usize> = (0..keys.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, &(x, y, z)) in keys.iter().enumerate() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(&j) = index.get(&(x + dx, y + dy, z + dz)) {
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        parent[a.max(b)] = a.min(b);
                    }
                }
            }
        }
    }
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, key) in keys.iter().enumerate() {
        let r = root(&mut parent, i);
        clusters.entry(r).or_default().extend(&cells[key]);
    }
    clusters
        .values()
        .filter_map(|members| {
            let (mut area, mut center, mut flux) = (0.0f32, Vec3::ZERO, Vec3::ZERO);
            for &i in members {
                let e = &scene.emitters[i];
                let [a, b, c] = e.corners;
                let da = (b - a).cross(c - a).length() * 0.5;
                area += da;
                center += (a + b + c) / 3.0 * da;
                // Luminous flux of a Lambertian emitter, one side: π L A.
                flux += e.luminance * (PI * da);
            }
            if area <= 0.0 || flux.max_element() <= 0.0 {
                return None;
            }
            let center = center / area;
            let radius = members
                .iter()
                .flat_map(|&i| scene.emitters[i].corners)
                .map(|p| p.distance(center))
                .fold(0.0f32, f32::max);
            // A closed convex emitter shows a quarter of its area from any
            // side, so it emits a quarter of its one-sided flux per π sr.
            let intensity = flux / (4.0 * PI);
            let range = (intensity.max_element() / LAMP_MIN_LUX)
                .sqrt()
                .min(LAMP_MAX_RANGE);
            Some(Lamp {
                center,
                radius,
                intensity,
                range,
            })
        })
        .collect()
}

/// A lamp's light falls with the inverse square of distance, windowed to
/// zero at its range (Karis 2013).
fn window(d: f32, range: f32) -> f32 {
    let x = (d / range).powi(4);
    (1.0 - x).clamp(0.0, 1.0).powi(2)
}

/// The lamps whose light may reach each grid cell.
struct LampGrid {
    min: Vec3,
    dims: [usize; 2],
    cells: Vec<Vec<u32>>,
}

impl LampGrid {
    fn new(lamps: &[Lamp]) -> Self {
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for l in lamps {
            min = min.min(l.center - Vec3::splat(l.range));
            max = max.max(l.center + Vec3::splat(l.range));
        }
        if lamps.is_empty() {
            return Self {
                min: Vec3::ZERO,
                dims: [0, 0],
                cells: Vec::new(),
            };
        }
        let dims = [
            ((max.x - min.x) / LAMP_GRID).ceil() as usize + 1,
            ((max.z - min.z) / LAMP_GRID).ceil() as usize + 1,
        ];
        let mut cells = vec![Vec::new(); dims[0] * dims[1]];
        for (k, l) in lamps.iter().enumerate() {
            let lo = ((l.center - Vec3::splat(l.range) - min) / LAMP_GRID).floor();
            let hi = ((l.center + Vec3::splat(l.range) - min) / LAMP_GRID).floor();
            for z in lo.z.max(0.0) as usize..=(hi.z as usize).min(dims[1] - 1) {
                for x in lo.x.max(0.0) as usize..=(hi.x as usize).min(dims[0] - 1) {
                    cells[z * dims[0] + x].push(k as u32);
                }
            }
        }
        Self { min, dims, cells }
    }

    fn near(&self, p: Vec3) -> &[u32] {
        let q = ((p - self.min) / LAMP_GRID).floor();
        if q.x < 0.0 || q.z < 0.0 {
            return &[];
        }
        let (x, z) = (q.x as usize, q.z as usize);
        if x >= self.dims[0] || z >= self.dims[1] {
            return &[];
        }
        &self.cells[z * self.dims[0] + x]
    }
}

/// How much one layered bake traced and how long its phases took.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerStats {
    /// Lamps found among the emissive triangles.
    pub lamps: usize,
    /// Vertices any lamp reaches.
    pub lit_by_lamps: usize,
    pub nearest_rays: u64,
    pub shadow_rays: u64,
    pub lamp_ms: u64,
    pub pass_ms: Vec<u64>,
    pub sun_ms: u64,
    pub probe_ms: u64,
}

/// The key of a layered bake: [`crate::bake_key`]'s inputs and
/// [`LAYERS_VERSION`].
#[must_use]
pub fn layers_key(scene: &Scene, light: &Light, settings: &Settings) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"openagents.verse-bake.layers.v1\0");
    hash.update(LAYERS_VERSION.to_le_bytes());
    hash.update(crate::bake_key(scene, light, settings));
    hash.finalize().into()
}

/// What one pass gathered for every vertex.
struct Pass {
    /// The sky's multiplier and open fraction.
    sky: Vec<Vec3>,
    open: Vec<f32>,
    /// Each sun's bounce multiplier, vertex-major: `suns` entries a vertex.
    suns: Vec<Vec3>,
    /// The lamps' bounced irradiance, lux.
    lamp: Vec<Vec3>,
}

/// Each layer's irradiance on every vertex that the next pass reads.
struct Around {
    sky: Vec<Vec3>,
    suns: Vec<Vec3>,
    /// Direct and bounced lamp light together.
    lamp: Vec<Vec3>,
}

/// One gathering sample's sums.
#[derive(Clone, Copy)]
struct Sums {
    sky: Vec3,
    suns: [Vec3; MAX_SUNS],
    lamp: Vec3,
    open: f32,
}

/// A layered bake of `scene` under `light` (the reference the multipliers
/// are relative to) for each of `settings.suns`, on `backend`. `digest` is
/// [`verse_pbr::pbr::baked_layers::scene_digest`] of the scene's source.
///
/// # Errors
///
/// Returns the backend's failure, or a message when there are more than
/// [`MAX_SUNS`] suns.
pub fn bake_layers(
    scene: &Scene,
    digest: [u8; 32],
    light: &Light,
    settings: &Settings,
    backend: &mut dyn crate::Backend,
    threads: usize,
) -> Result<(Layers, LayerStats), String> {
    if settings.suns.len() > MAX_SUNS {
        return Err(format!("a layered bake takes at most {MAX_SUNS} suns"));
    }
    let mut baker = Baker {
        scene,
        light: verse_pbr::pbr::textured_bake::BakeLight {
            sun_dir: Vec3::from(light.sun_dir).normalize_or(Vec3::Y),
            sun_illuminance: light.sun_illuminance,
            sky: light.sky,
            ground: light.ground,
        },
        settings,
        backend,
        threads: threads.max(1),
        stats: crate::Stats::default(),
    };
    let mut stats = LayerStats::default();
    let suns: Vec<Vec3> = settings
        .suns
        .iter()
        .map(|&s| Vec3::from(s).normalize_or(Vec3::Y))
        .collect();
    let start = Instant::now();
    let lamps = lamps(scene);
    stats.lamps = lamps.len();
    let direct = baker.lamp_direct(&lamps)?;
    stats.lamp_ms = elapsed_ms(start);
    let n = scene.vertices.len();
    let mut around: Option<Around> = None;
    let mut pass = None;
    for _ in 0..settings.bounces.max(1) {
        let start = Instant::now();
        let gathered = baker.gather_layers(&suns, around.as_ref(), &direct)?;
        let ambient: Vec<f32> = scene
            .vertices
            .iter()
            .map(|v| baker.light.ambient(Vec3::from(v.normal)))
            .collect();
        around = Some(Around {
            sky: gathered
                .sky
                .iter()
                .zip(&ambient)
                .map(|(m, a)| *m * *a)
                .collect(),
            suns: gathered
                .suns
                .iter()
                .enumerate()
                .map(|(i, m)| *m * ambient[i / suns.len().max(1)])
                .collect(),
            lamp: gathered
                .lamp
                .iter()
                .zip(&direct)
                .map(|(b, d)| *b + *d)
                .collect(),
        });
        pass = Some(gathered);
        stats.pass_ms.push(elapsed_ms(start));
    }
    let pass = pass.ok_or("a layered bake needs a pass")?;
    let start = Instant::now();
    let visibility = suns
        .iter()
        .map(|&s| baker.sun_layer(s))
        .collect::<Result<Vec<_>, _>>()?;
    stats.sun_ms = elapsed_ms(start);
    let start = Instant::now();
    let around = around.ok_or("a layered bake needs a pass")?;
    let (sky_probes, sun_probes) = baker.probe_layers(&suns, &around)?;
    stats.probe_ms = elapsed_ms(start);
    stats.nearest_rays = baker.stats.nearest_rays;
    stats.shadow_rays = baker.stats.shadow_rays;
    let k = suns.len();
    let mut lamp_texels = Vec::new();
    for i in 0..n {
        let texel = encode_lamp(direct[i] + pass.lamp[i]);
        if texel[3] > 0 {
            lamp_texels.push((i as u32, texel));
        }
    }
    stats.lit_by_lamps = lamp_texels.len();
    let layers = Layers {
        bake_key: hex(&layers_key(scene, light, settings)),
        scene: layer_hex(&digest),
        reference: Reference {
            sun: light.sun_illuminance,
            sky: light.sky,
            ground: light.ground,
        },
        sky: pass
            .sky
            .iter()
            .zip(&pass.open)
            .map(|(m, o)| encode(*m, *o))
            .collect(),
        sky_probes,
        suns: suns
            .iter()
            .enumerate()
            .zip(sun_probes)
            .map(|((j, s), probes)| SunLayer {
                dir: s.to_array(),
                vertices: (0..n)
                    .map(|i| encode_sun(pass.suns[i * k + j], visibility[j][i]))
                    .collect(),
                probes,
            })
            .collect(),
        lamps: lamp_texels,
        probe_origin: settings.probe_min,
        probe_cell: settings.probe_cell,
        probe_dims: settings.dims(),
    };
    Ok((layers, stats))
}

fn hex(bytes: &[u8]) -> String {
    crate::scene::hex(bytes)
}

/// `values` at the point with barycentric weights `w` on `tri`.
fn interpolate(values: &[Vec3], tri: &Triangle, w: Vec3) -> Vec3 {
    let [a, b, c] = tri.vertices.map(|v| values[v as usize]);
    a * w.x + b * w.y + c * w.z
}

impl Baker<'_> {
    /// Every vertex's direct light from `lamps`, lux.
    fn lamp_direct(&mut self, lamps: &[Lamp]) -> Result<Vec<Vec3>, String> {
        let count = self.scene.vertices.len();
        let mut out = vec![Vec3::ZERO; count];
        if lamps.is_empty() {
            return Ok(out);
        }
        let grid = LampGrid::new(lamps);
        let per = LAMP_SHADOW_RAYS.max(1) as usize;
        let mut rays = Vec::new();
        // Each ray's vertex and its share of the unshadowed light.
        let mut owners: Vec<(u32, Vec3)> = Vec::new();
        let flush = |this: &mut Self,
                     rays: &mut Vec<Ray>,
                     owners: &mut Vec<(u32, Vec3)>,
                     out: &mut [Vec3]|
         -> Result<(), String> {
            let hits = this.trace(rays)?;
            for ((i, e), hit) in owners.iter().zip(hits) {
                out[*i as usize] += *e * hit.transmittance;
            }
            rays.clear();
            owners.clear();
            Ok(())
        };
        for i in 0..count {
            let v = &self.scene.vertices[i];
            let p = Vec3::from(v.pos);
            let n = Vec3::from(v.normal).normalize_or(Vec3::Y);
            let foliage = self.scene.foliage[i];
            let bias = if self.scene.far[i] { FAR_BIAS } else { BIAS };
            for (j, &k) in grid.near(p).iter().enumerate() {
                let lamp = &lamps[k as usize];
                let to = lamp.center - p;
                let d = to.length();
                if d >= lamp.range || d <= 1e-4 {
                    continue;
                }
                let l = to / d;
                let mut cos = n.dot(l);
                if foliage {
                    cos = cos.abs();
                }
                if cos <= 0.0 {
                    continue;
                }
                // Inside a lamp's own cluster the distance stops shrinking.
                let at = d.max(lamp.radius).max(0.1);
                let e = lamp.intensity * (cos * window(d, lamp.range) / (at * at));
                if e.max_element() < LAMP_MIN_LUX * 0.1 {
                    continue;
                }
                let side = if foliage && n.dot(l) < 0.0 { -n } else { n };
                let origin = p + side * bias;
                let reach = d - lamp.radius - 0.02;
                if reach <= 0.0 {
                    out[i] += e;
                    continue;
                }
                let angle = turn(self.settings.seed, LAMP_STREAM, (i * 31 + j) as u64);
                let (t, b) = l.any_orthonormal_pair();
                for r in 0..per {
                    let phi = angle + TAU * r as f32 / per as f32;
                    let offset = (t * phi.cos() + b * phi.sin()) * (lamp.radius * 0.5);
                    let target = lamp.center + offset;
                    let dir = (target - origin).normalize_or(l);
                    rays.push(Ray::new(origin, dir, reach, SHADOW));
                    owners.push((i as u32, e / per as f32));
                }
            }
            if rays.len() >= BATCH_RAYS {
                flush(self, &mut rays, &mut owners, &mut out)?;
            }
        }
        flush(self, &mut rays, &mut owners, &mut out)?;
        Ok(out)
    }

    /// One gathering pass over every vertex for every layer.
    fn gather_layers(
        &mut self,
        suns: &[Vec3],
        around: Option<&Around>,
        direct: &[Vec3],
    ) -> Result<Pass, String> {
        let count = self.scene.vertices.len();
        let k = suns.len();
        let rays_per = self.settings.vertex_rays.max(1) as usize;
        let mut out = Pass {
            sky: Vec::with_capacity(count),
            open: Vec::with_capacity(count),
            suns: Vec::with_capacity(count * k),
            lamp: Vec::with_capacity(count),
        };
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
            let gathered =
                self.gather_samples(&samples, &directions, rays_per, suns, around, direct)?;
            let mut at = 0;
            for (_, used) in &items {
                let sides = &gathered[at..at + used];
                at += used;
                let mean =
                    |f: &dyn Fn(&Sums) -> Vec3| sides.iter().map(f).sum::<Vec3>() / *used as f32;
                let mut sky = mean(&|s| s.sky);
                let open = sides.iter().map(|s| s.open).sum::<f32>() / *used as f32;
                if *used == 2 {
                    // Leaf cards show both faces and pass light through.
                    sky = sky.max(Vec3::splat(FOLIAGE_FLOOR));
                }
                out.sky.push(sky);
                out.open.push(open);
                for j in 0..k {
                    out.suns.push(mean(&|s| s.suns[j]));
                }
                out.lamp.push(mean(&|s| s.lamp));
            }
            start = end;
        }
        Ok(out)
    }

    /// Traces `rays_per` of `directions` from each of `samples` and returns
    /// each sample's layers.
    fn gather_samples(
        &mut self,
        samples: &[Sample],
        directions: &[(Vec3, f32)],
        rays_per: usize,
        suns: &[Vec3],
        around: Option<&Around>,
        direct: &[Vec3],
    ) -> Result<Vec<Sums>, String> {
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
        let sun_light = suns
            .iter()
            .map(|&s| self.sun_on_hits_toward(&rays, &hits, s))
            .collect::<Result<Vec<_>, _>>()?;
        let light = self.light;
        let triangles = &self.scene.triangles;
        let k = suns.len();
        let empty = Sums {
            sky: Vec3::ONE,
            suns: [Vec3::ZERO; MAX_SUNS],
            lamp: Vec3::ZERO,
            open: 1.0,
        };
        let mut out = vec![empty; samples.len()];
        let index: Vec<usize> = (0..samples.len()).collect();
        let solid_angle = TAU / rays_per as f32;
        for_chunks(self.threads, &index, &mut out, ACCUMULATE_CHUNK, |&s| {
            let mut sums = Sums {
                sky: Vec3::ZERO,
                suns: [Vec3::ZERO; MAX_SUNS],
                lamp: Vec3::ZERO,
                open: 0.0,
            };
            let (mut reference, mut total) = (0.0f32, 0.0f32);
            for j in s * rays_per..(s + 1) * rays_per {
                let (d, c) = directions[j];
                let hit: &RayHit = &hits[j];
                let sky = light.sky_radiance(d);
                reference += sky * c;
                total += c;
                sums.open += hit.transmittance * c;
                sums.sky += Vec3::splat(sky * hit.transmittance * c);
                if hit.triangle == MISS {
                    continue;
                }
                let Some(tri) = triangles.get(hit.triangle as usize) else {
                    continue;
                };
                let n = facing(tri, d);
                let point = Vec3::from(rays[j].origin) + d * hit.distance;
                let w = tri.weights(point);
                let reflect = tri.albedo * (tri.opacity.min(1.0) / PI) * c;
                let sky_around = match around {
                    Some(a) => interpolate(&a.sky, tri, w),
                    None => Vec3::splat(light.ambient(n) * HIT_AMBIENT),
                };
                sums.sky += reflect * sky_around;
                for q in 0..k {
                    let bounce = around.map_or(Vec3::ZERO, |a| {
                        let [x, y, z] = tri.vertices.map(|v| a.suns[v as usize * k + q]);
                        x * w.x + y * w.y + z * w.z
                    });
                    sums.suns[q] += reflect * (Vec3::splat(sun_light[q][j]) + bounce);
                }
                let lamp_around = match around {
                    Some(a) => interpolate(&a.lamp, tri, w),
                    None => interpolate(direct, tri, w),
                };
                sums.lamp += reflect * lamp_around;
            }
            if reference <= 0.0 || total <= 0.0 {
                return empty;
            }
            let clamp = |m: Vec3| m.clamp(Vec3::ZERO, Vec3::splat(MAX_AMBIENT));
            Sums {
                sky: clamp(sums.sky / reference),
                suns: std::array::from_fn(|q| clamp(sums.suns[q] / reference)),
                lamp: sums.lamp * solid_angle,
                open: sums.open / total,
            }
        });
        Ok(out)
    }

    /// The sky's and each sun's probe grids, from the last pass's light.
    fn probe_layers(
        &mut self,
        suns: &[Vec3],
        around: &Around,
    ) -> Result<(Vec<[f32; 12]>, Vec<Vec<[f32; 12]>>), String> {
        let dims = self.settings.dims();
        let count = (dims[0] * dims[1] * dims[2]) as usize;
        let per = self.settings.probe_rays.max(1) as usize;
        let origin = Vec3::from(self.settings.probe_min);
        let cell = self.settings.probe_cell;
        let k = suns.len();
        let mut sky_data = vec![[0.0f32; 12]; count];
        let mut sun_data = vec![vec![[0.0f32; 12]; count]; k];
        let mut valid = vec![false; count];
        let project = |radiance: &[(Vec3, Vec3)]| -> [f32; 12] {
            let weight = 4.0 * PI / radiance.len().max(1) as f32;
            let (mut l0, mut l1) = (Vec3::ZERO, [Vec3::ZERO; 3]);
            for &(d, r) in radiance {
                l0 += r * (Y0 * weight);
                for (a, axis) in l1.iter_mut().enumerate() {
                    *axis += r * (Y1 * d[a] * weight);
                }
            }
            let mut probe = [0.0f32; 12];
            for c in 0..3 {
                probe[c * 4] = A0 * Y0 * l0[c];
                for a in 0..3 {
                    probe[c * 4 + 1 + a] = A1 * Y1 * l1[a][c];
                }
            }
            probe
        };
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
            let sun_light = suns
                .iter()
                .map(|&s| self.sun_on_hits_toward(&rays, &hits, s))
                .collect::<Result<Vec<_>, _>>()?;
            for index in start..end {
                let mut sky = Vec::with_capacity(per);
                let mut lit: Vec<Vec<(Vec3, Vec3)>> = vec![Vec::with_capacity(per); k];
                let mut backfaces = 0;
                for j in (index - start) * per..(index - start + 1) * per {
                    let d = Vec3::from(rays[j].dir);
                    let hit = &hits[j];
                    let mut radiance = Vec3::splat(self.light.sky_radiance(d) * hit.transmittance);
                    let tri = self.scene.triangles.get(hit.triangle as usize);
                    if let Some(tri) = tri {
                        if tri.opacity >= SOLID && tri.normal.dot(d) > 0.0 {
                            backfaces += 1;
                        }
                        let point = Vec3::from(rays[j].origin) + d * hit.distance;
                        let w = tri.weights(point);
                        let reflect = tri.albedo * (tri.opacity.min(1.0) / PI);
                        radiance += reflect * interpolate(&around.sky, tri, w);
                        for (q, out) in lit.iter_mut().enumerate() {
                            let [x, y, z] = tri.vertices.map(|v| around.suns[v as usize * k + q]);
                            let bounce = x * w.x + y * w.y + z * w.z;
                            out.push((d, reflect * (Vec3::splat(sun_light[q][j]) + bounce)));
                        }
                    } else {
                        for out in &mut lit {
                            out.push((d, Vec3::ZERO));
                        }
                    }
                    sky.push((d, radiance));
                }
                sky_data[index] = project(&sky);
                for (q, radiance) in lit.iter().enumerate() {
                    sun_data[q][index] = project(radiance);
                }
                valid[index] = backfaces * 3 < per;
            }
            start = end;
        }
        dilate(&mut sky_data, &valid, dims);
        for data in &mut sun_data {
            dilate(data, &valid, dims);
        }
        Ok((sky_data, sun_data))
    }
}
