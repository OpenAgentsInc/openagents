//! Procedural lab content: mesh variants with LOD chains and clusters, a
//! district of static instances, materials, textures, and a walking crowd.
//!
//! Everything is generated from a seed, so every run and both platforms see the
//! same scene. The simplifier and cluster builder are small pure-Rust stand-ins
//! so the lab runs inside Wasm without a C++ toolchain; the production asset
//! compiler would use meshoptimizer (decision 0007).

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const LODS: usize = 4;
pub const CLUSTER_TRIANGLES: usize = 64;
pub const MATERIALS: usize = 64;
pub const TEXTURE_SIZE: u32 = 128;
pub const TEXTURE_MIPS: u32 = 8;
pub const TILE_SIZE: f32 = 50.0;

/// One vertex, 32 bytes. Bound as a vertex buffer and as storage for vertex pulling.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// One LOD of one mesh variant, as the GPU sees it. 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MeshLod {
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: u32,
    pub first_cluster: u32,
    pub cluster_count: u32,
    pub _pad: [u32; 3],
}

/// Bounds of one mesh variant (all LODs), 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MeshBounds {
    pub center: [f32; 3],
    pub radius: f32,
}

/// A cluster of up to 64 triangles with culling bounds, 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Cluster {
    pub first_index: u32,
    pub triangle_count: u32,
    pub base_vertex: u32,
    pub _pad: u32,
    pub center: [f32; 3],
    pub radius: f32,
    pub cone_axis: [f32; 3],
    /// Cosine cutoff; a value above 1 disables backface cluster culling.
    pub cone_cutoff: f32,
}

/// A static instance, 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Instance {
    pub position: [f32; 3],
    pub scale: f32,
    pub yaw: f32,
    pub height_scale: f32,
    pub mesh: u32,
    pub material: u32,
}

/// A material, 32 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Material {
    pub color: [f32; 3],
    pub roughness: f32,
    pub layer: u32,
    pub uv_scale: f32,
    pub _pad: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Tile,
    Building,
    Tower,
    Rock,
    Crate,
    Pillar,
    Tree,
    Wall,
}

pub const KINDS: [Kind; 8] = [
    Kind::Tile,
    Kind::Building,
    Kind::Tower,
    Kind::Rock,
    Kind::Crate,
    Kind::Pillar,
    Kind::Tree,
    Kind::Wall,
];

/// Deterministic xorshift generator.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn vertex(&mut self, p: Vec3, n: Vec3, uv: [f32; 2]) -> u32 {
        self.vertices.push(Vertex { position: p.into(), normal: n.normalize_or_zero().into(), uv });
        self.vertices.len() as u32 - 1
    }

    /// A subdivided quad from `origin` spanning `u` and `v`.
    fn grid(&mut self, origin: Vec3, u: Vec3, v: Vec3, steps: u32, uv_scale: f32) {
        let n = u.cross(v).normalize();
        let base = self.vertices.len() as u32;
        for j in 0..=steps {
            for i in 0..=steps {
                let (fu, fv) = (i as f32 / steps as f32, j as f32 / steps as f32);
                let p = origin + u * fu + v * fv;
                self.vertex(p, n, [fu * u.length() * uv_scale, fv * v.length() * uv_scale]);
            }
        }
        let row = steps + 1;
        for j in 0..steps {
            for i in 0..steps {
                let a = base + j * row + i;
                self.indices.extend_from_slice(&[a, a + 1, a + row + 1, a, a + row + 1, a + row]);
            }
        }
    }

    fn boxed(&mut self, min: Vec3, max: Vec3, steps: u32, uv_scale: f32) {
        let d = max - min;
        let (x, y, z) = (Vec3::X * d.x, Vec3::Y * d.y, Vec3::Z * d.z);
        self.grid(min, z, y, steps, uv_scale); // -X
        self.grid(min + x + z, -z, y, steps, uv_scale); // +X
        self.grid(min + z, x, y, steps, uv_scale); // +Z
        self.grid(min + x, -x, y, steps, uv_scale); // -Z
        self.grid(min + y, z, x, steps, uv_scale); // +Y
        self.grid(min, x, z, steps, uv_scale); // -Y
    }

    /// A capped cylinder or cone frustum along +Y.
    fn cylinder(&mut self, base: Vec3, r0: f32, r1: f32, h: f32, segments: u32, rings: u32) {
        let start = self.vertices.len() as u32;
        let slope = (r0 - r1) / h;
        for j in 0..=rings {
            let t = j as f32 / rings as f32;
            let r = r0 + (r1 - r0) * t;
            for i in 0..=segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                let n = Vec3::new(c, slope, s);
                self.vertex(base + Vec3::new(c * r, t * h, s * r), n, [i as f32 / segments as f32 * 4.0, t * h * 0.5]);
            }
        }
        let row = segments + 1;
        for j in 0..rings {
            for i in 0..segments {
                let a = start + j * row + i;
                self.indices.extend_from_slice(&[a, a + row + 1, a + 1, a, a + row, a + row + 1]);
            }
        }
        if r1 > 0.0 {
            let center = self.vertex(base + Vec3::Y * h, Vec3::Y, [0.5, 0.5]);
            let ring = self.vertices.len() as u32;
            for i in 0..segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                self.vertex(base + Vec3::new(c * r1, h, s * r1), Vec3::Y, [0.5 + c * 0.5, 0.5 + s * 0.5]);
            }
            for i in 0..segments {
                self.indices.extend_from_slice(&[center, ring + (i + 1) % segments, ring + i]);
            }
        }
    }

    fn sphere(&mut self, center: Vec3, radius: f32, subdivisions: u32, noise_seed: u32, roughness: f32) {
        // Lat-long sphere keeps triangles in spatial order for clustering.
        let rings = 4 * (1 << subdivisions.min(6)) / 2;
        let segments = rings * 2;
        let start = self.vertices.len() as u32;
        for j in 0..=rings {
            let phi = j as f32 / rings as f32 * std::f32::consts::PI;
            for i in 0..=segments {
                let theta = i as f32 / segments as f32 * std::f32::consts::TAU;
                let dir = Vec3::new(phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin());
                let wrap = if i == segments { 0 } else { i };
                let key = Vec3::new(phi.sin() * (wrap as f32 / segments as f32 * std::f32::consts::TAU).cos(), phi.cos(), phi.sin() * (wrap as f32 / segments as f32 * std::f32::consts::TAU).sin());
                let bump = 1.0 + roughness * (fbm(key * 2.3, noise_seed) - 0.5);
                self.vertex(center + dir * radius * bump, dir, [i as f32 / segments as f32 * 3.0, j as f32 / rings as f32 * 1.5]);
            }
        }
        let row = segments + 1;
        for j in 0..rings {
            for i in 0..segments {
                let a = start + j * row + i;
                self.indices.extend_from_slice(&[a, a + 1, a + row + 1, a, a + row + 1, a + row]);
            }
        }
        recompute_normals(self, start, center);
    }

    pub fn bounds(&self) -> (Vec3, f32) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &self.vertices {
            min = min.min(v.position.into());
            max = max.max(v.position.into());
        }
        let center = (min + max) * 0.5;
        let radius = self.vertices.iter().map(|v| Vec3::from(v.position).distance(center)).fold(0.0, f32::max);
        (center, radius)
    }
}

fn recompute_normals(mesh: &mut MeshData, start: u32, center: Vec3) {
    let mut acc = vec![Vec3::ZERO; mesh.vertices.len()];
    for t in mesh.indices.as_chunks::<3>().0 {
        if t[0] < start {
            continue;
        }
        let p: [Vec3; 3] = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize].position.into());
        let n = (p[1] - p[0]).cross(p[2] - p[0]);
        for &i in t {
            acc[i as usize] += n;
        }
    }
    for (i, v) in mesh.vertices.iter_mut().enumerate().skip(start as usize) {
        let mut n = acc[i].normalize_or_zero();
        if n.dot(Vec3::from(v.position) - center) < 0.0 {
            n = -n;
        }
        if n != Vec3::ZERO {
            v.normal = n.into();
        }
    }
}

fn hash3(p: [i32; 3], seed: u32) -> f32 {
    let mut h = (p[0] as u32).wrapping_mul(0x8da6_b343) ^ (p[1] as u32).wrapping_mul(0xd816_3841) ^ (p[2] as u32).wrapping_mul(0xcb1a_b31f) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn value_noise(p: Vec3, seed: u32) -> f32 {
    let f = p.floor();
    let t = p - f;
    let s = t * t * (Vec3::splat(3.0) - 2.0 * t);
    let i = [f.x as i32, f.y as i32, f.z as i32];
    let mut out = 0.0;
    for corner in 0..8 {
        let o = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let w = (if o[0] == 1 { s.x } else { 1.0 - s.x }) * (if o[1] == 1 { s.y } else { 1.0 - s.y }) * (if o[2] == 1 { s.z } else { 1.0 - s.z });
        out += w * hash3([i[0] + o[0], i[1] + o[1], i[2] + o[2]], seed);
    }
    out
}

fn fbm(p: Vec3, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq) = (0.0, 0.5, 1.0);
    for octave in 0..4 {
        sum += amp * value_noise(p * freq, seed + octave);
        amp *= 0.5;
        freq *= 2.0;
    }
    sum
}

/// Builds LOD0 for one variant of a kind.
pub fn build_mesh(kind: Kind, variant: u32) -> MeshData {
    let mut m = MeshData::default();
    let mut rng = Rng::new(variant as u64 * 977 + kind as u64 * 131 + 7);
    match kind {
        Kind::Tile => m.grid(Vec3::ZERO, Vec3::Z * TILE_SIZE, Vec3::X * TILE_SIZE, 8, 0.25),
        Kind::Building => {
            let (w, d, h) = (rng.range(6.0, 12.0), rng.range(6.0, 12.0), rng.range(6.0, 16.0));
            m.boxed(Vec3::new(-w / 2.0, 0.0, -d / 2.0), Vec3::new(w / 2.0, h, d / 2.0), 6 + variant % 4, 0.25);
            m.boxed(Vec3::new(-w / 2.0 - 0.3, h, -d / 2.0 - 0.3), Vec3::new(w / 2.0 + 0.3, h + 0.6, d / 2.0 + 0.3), 2, 0.5);
        }
        Kind::Tower => {
            let r = rng.range(2.0, 4.0);
            let h = rng.range(12.0, 24.0);
            m.cylinder(Vec3::ZERO, r, r, h, 32, 12);
            m.cylinder(Vec3::Y * h, r * 1.2, 0.0, r * 2.0, 32, 4);
        }
        Kind::Rock => m.sphere(Vec3::Y * 1.2, 1.6, 5, variant * 31 + 3, 0.45),
        Kind::Crate => m.boxed(Vec3::splat(-0.5) + Vec3::Y * 0.5, Vec3::splat(0.5) + Vec3::Y * 0.5, 2, 1.0),
        Kind::Pillar => {
            m.cylinder(Vec3::ZERO, 0.45, 0.4, 5.0, 16, 8);
            m.boxed(Vec3::new(-0.6, 5.0, -0.6), Vec3::new(0.6, 5.4, 0.6), 1, 1.0);
        }
        Kind::Tree => {
            m.cylinder(Vec3::ZERO, 0.25, 0.2, 2.0, 10, 2);
            for k in 0..3 {
                let y = 1.6 + k as f32 * 1.4;
                m.cylinder(Vec3::Y * y, 2.2 - k as f32 * 0.5, 0.05, 2.4, 14, 3);
            }
        }
        Kind::Wall => m.boxed(Vec3::new(-6.0, 0.0, -0.4), Vec3::new(6.0, 3.5, 0.4), 8, 0.4),
    }
    m
}

/// Vertex-clustering simplification at a grid resolution relative to the bounds.
pub fn simplify(mesh: &MeshData, resolution: f32) -> MeshData {
    let (center, radius) = mesh.bounds();
    let cell = (radius * 2.0 / resolution).max(1e-4);
    let mut map = std::collections::HashMap::<(i32, i32, i32), u32>::new();
    let mut sums: Vec<(Vec3, Vec3, [f32; 2], f32)> = Vec::new();
    let mut remap = Vec::with_capacity(mesh.vertices.len());
    for v in &mesh.vertices {
        let p = (Vec3::from(v.position) - center) / cell;
        // Separate cells by normal direction so creases survive.
        let n = Vec3::from(v.normal);
        let key = (p.x.floor() as i32 * 8 + (n.x > 0.5) as i32 - (n.x < -0.5) as i32 * 2, p.y.floor() as i32 * 8 + (n.y > 0.5) as i32 - (n.y < -0.5) as i32 * 2, p.z.floor() as i32 * 8 + (n.z > 0.5) as i32 - (n.z < -0.5) as i32 * 2);
        let id = *map.entry(key).or_insert_with(|| {
            sums.push((Vec3::ZERO, Vec3::ZERO, [0.0; 2], 0.0));
            sums.len() as u32 - 1
        });
        let s = &mut sums[id as usize];
        s.0 += Vec3::from(v.position);
        s.1 += n;
        s.2[0] += v.uv[0];
        s.2[1] += v.uv[1];
        s.3 += 1.0;
        remap.push(id);
    }
    let vertices = sums
        .iter()
        .map(|(p, n, uv, k)| Vertex { position: (*p / *k).into(), normal: n.normalize_or_zero().into(), uv: [uv[0] / k, uv[1] / k] })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut indices = Vec::new();
    for t in mesh.indices.as_chunks::<3>().0 {
        let r = [remap[t[0] as usize], remap[t[1] as usize], remap[t[2] as usize]];
        if r[0] == r[1] || r[1] == r[2] || r[0] == r[2] {
            continue;
        }
        let mut key = r;
        key.sort();
        if seen.insert(key) {
            indices.extend_from_slice(&r);
        }
    }
    MeshData { vertices, indices }
}

/// Splits a mesh's triangles into clusters of up to 64 in index order.
pub fn clusters(mesh: &MeshData, first_index: u32, base_vertex: u32) -> Vec<Cluster> {
    let mut out = Vec::new();
    for (c, tris) in mesh.indices.chunks(CLUSTER_TRIANGLES * 3).enumerate() {
        let points: Vec<Vec3> = tris.iter().map(|&i| mesh.vertices[i as usize].position.into()).collect();
        let center = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        let radius = points.iter().map(|p| p.distance(center)).fold(0.0, f32::max);
        let normals: Vec<Vec3> = tris
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let p: [Vec3; 3] = [0, 1, 2].map(|k| mesh.vertices[t[k] as usize].position.into());
                (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero()
            })
            .collect();
        let axis = normals.iter().copied().sum::<Vec3>().normalize_or_zero();
        let min_dot = normals.iter().map(|n| n.dot(axis)).fold(1.0f32, f32::min);
        // Cone test is valid only when every face normal is within 90 degrees.
        let cutoff = if axis == Vec3::ZERO || min_dot <= 0.1 { 2.0 } else { (1.0 - min_dot * min_dot).max(0.0).sqrt() };
        out.push(Cluster {
            first_index: first_index + (c * CLUSTER_TRIANGLES * 3) as u32,
            triangle_count: (tris.len() / 3) as u32,
            base_vertex,
            _pad: 0,
            center: center.into(),
            radius,
            cone_axis: axis.into(),
            cone_cutoff: cutoff,
        });
    }
    out
}

/// All geometry, packed for upload.
pub struct Geometry {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub lods: Vec<MeshLod>,
    pub bounds: Vec<MeshBounds>,
    pub clusters: Vec<Cluster>,
    pub kinds: Vec<Kind>,
}

impl Geometry {
    /// Builds `variants` meshes per kind (tiles always have one), each with
    /// `LODS` levels and clusters.
    pub fn build(variants: u32) -> Geometry {
        let mut g = Geometry { vertices: Vec::new(), indices: Vec::new(), lods: Vec::new(), bounds: Vec::new(), clusters: Vec::new(), kinds: Vec::new() };
        for kind in KINDS {
            let count = if kind == Kind::Tile { 1 } else { variants };
            for variant in 0..count {
                let lod0 = build_mesh(kind, variant);
                let (center, radius) = lod0.bounds();
                g.bounds.push(MeshBounds { center: center.into(), radius });
                g.kinds.push(kind);
                let mut levels = vec![lod0.clone()];
                for resolution in [40.0, 16.0, 7.0] {
                    let next = simplify(&lod0, resolution);
                    levels.push(if next.indices.len() >= 12 { next } else { levels.last().unwrap().clone() });
                }
                for level in levels {
                    let base_vertex = g.vertices.len() as u32;
                    let first_index = g.indices.len() as u32;
                    let cl = clusters(&level, first_index, base_vertex);
                    g.lods.push(MeshLod {
                        first_index,
                        index_count: level.indices.len() as u32,
                        base_vertex,
                        first_cluster: g.clusters.len() as u32,
                        cluster_count: cl.len() as u32,
                        _pad: [0; 3],
                    });
                    g.clusters.extend(cl);
                    g.vertices.extend(level.vertices);
                    g.indices.extend(level.indices);
                }
            }
        }
        g
    }

    pub fn mesh_count(&self) -> usize {
        self.bounds.len()
    }

    /// Indexes of the meshes of `kind`.
    pub fn meshes_of(&self, kind: Kind) -> Vec<u32> {
        self.kinds.iter().enumerate().filter(|(_, k)| **k == kind).map(|(i, _)| i as u32).collect()
    }
}

/// The world side length for `count` static instances.
pub fn world_side(count: u32) -> f32 {
    ((count.max(1000) as f32) * 60.0).sqrt()
}

/// Places `count` static instances plus ground tiles.
pub fn place_instances(geometry: &Geometry, count: u32, seed: u64) -> Vec<Instance> {
    let side = world_side(count);
    let half = side / 2.0;
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(count as usize + 1024);
    let tile = geometry.meshes_of(Kind::Tile)[0];
    let tiles = (side / TILE_SIZE).ceil() as i32;
    for j in 0..tiles {
        for i in 0..tiles {
            out.push(Instance {
                position: [-half + i as f32 * TILE_SIZE, 0.0, -half + j as f32 * TILE_SIZE],
                scale: 1.0,
                yaw: 0.0,
                height_scale: 1.0,
                mesh: tile,
                material: 0,
            });
        }
    }
    let pools: Vec<(Kind, Vec<u32>, f32)> = [
        (Kind::Building, 0.25),
        (Kind::Tower, 0.05),
        (Kind::Rock, 0.15),
        (Kind::Crate, 0.2),
        (Kind::Pillar, 0.1),
        (Kind::Tree, 0.2),
        (Kind::Wall, 0.05),
    ]
    .into_iter()
    .map(|(k, w)| (k, geometry.meshes_of(k), w))
    .collect();
    // Streets every 40 m stay clear so the camera has corridors.
    let block = 40.0;
    while out.len() < count as usize + (tiles * tiles) as usize {
        let x = rng.range(-half, half);
        let z = rng.range(-half, half);
        let street = |v: f32| ((v + half) % block) < 7.0;
        let mut pick = rng.f32();
        let mut chosen = &pools[0];
        for p in &pools {
            if pick < p.2 {
                chosen = p;
                break;
            }
            pick -= p.2;
        }
        let (kind, meshes, _) = chosen;
        if matches!(kind, Kind::Building | Kind::Tower | Kind::Wall) && (street(x) || street(z)) {
            continue;
        }
        let mesh = meshes[(rng.next_u32() as usize) % meshes.len()];
        let scale = match kind {
            Kind::Crate => rng.range(0.6, 1.4),
            Kind::Rock => rng.range(0.5, 2.2),
            _ => rng.range(0.8, 1.2),
        };
        out.push(Instance {
            position: [x, 0.0, z],
            scale,
            yaw: rng.range(0.0, std::f32::consts::TAU),
            height_scale: if matches!(kind, Kind::Building | Kind::Tower) { rng.range(0.7, 1.6) } else { 1.0 },
            mesh,
            material: 1 + rng.next_u32() % (MATERIALS as u32 - 1),
        });
    }
    out
}

pub fn materials() -> Vec<Material> {
    let mut rng = Rng::new(42);
    (0..MATERIALS as u32)
        .map(|m| {
            let base = if m == 0 { [0.32, 0.34, 0.30] } else { [rng.range(0.35, 0.95), rng.range(0.3, 0.85), rng.range(0.25, 0.8)] };
            Material { color: base, roughness: rng.range(0.3, 0.9), layer: m, uv_scale: if m == 0 { 0.25 } else { 1.0 }, _pad: [0; 2] }
        })
        .collect()
}

/// RGBA8 texels for every material layer and mip, layer-major then mip.
pub fn textures() -> Vec<Vec<Vec<u8>>> {
    (0..MATERIALS as u32)
        .map(|layer| {
            let size = TEXTURE_SIZE;
            let mut base = vec![0u8; (size * size * 4) as usize];
            let pattern = layer % 6;
            for y in 0..size {
                for x in 0..size {
                    let (fx, fy) = (x as f32 / size as f32, y as f32 / size as f32);
                    let n = fbm(Vec3::new(fx * 8.0, fy * 8.0, layer as f32), layer);
                    let v = match pattern {
                        0 => 0.75 + 0.25 * n,
                        1 => {
                            // Bricks.
                            let row = (fy * 8.0).floor();
                            let off = if row as i32 % 2 == 0 { 0.0 } else { 0.5 };
                            let mortar = (fy * 8.0).fract() < 0.1 || ((fx * 4.0 + off).fract() < 0.05);
                            if mortar { 0.45 } else { 0.8 + 0.2 * n }
                        }
                        2 => if ((fx * 8.0).floor() as i32 + (fy * 8.0).floor() as i32) % 2 == 0 { 0.9 } else { 0.65 },
                        3 => 0.7 + 0.3 * ((fx * 30.0 + n * 6.0).sin() * 0.5 + 0.5),
                        4 => 0.6 + 0.4 * n * n,
                        _ => if (fy * 10.0).fract() < 0.12 { 0.5 } else { 0.85 + 0.15 * n },
                    };
                    let b = (v.clamp(0.0, 1.0) * 255.0) as u8;
                    let i = ((y * size + x) * 4) as usize;
                    base[i..i + 4].copy_from_slice(&[b, b, b, 255]);
                }
            }
            let mut mips = vec![base];
            for level in 1..TEXTURE_MIPS {
                let prev = mips.last().unwrap();
                let ps = size >> (level - 1);
                let s = size >> level;
                let mut next = vec![0u8; (s * s * 4) as usize];
                for y in 0..s {
                    for x in 0..s {
                        for c in 0..4 {
                            let at = |xx: u32, yy: u32| prev[((yy * ps + xx) * 4 + c) as usize] as u32;
                            let sum = at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1);
                            next[((y * s + x) * 4 + c) as usize] = (sum / 4) as u8;
                        }
                    }
                }
                mips.push(next);
            }
            mips
        })
        .collect()
}

/// The crowd character: a rigid-skinned humanoid with 11 bones. Each vertex
/// stores its bone index in `uv[1]`'s integer part via a separate array.
pub struct Character {
    pub mesh: MeshData,
    pub bones: Vec<u32>,
    pub low: MeshData,
    pub low_bones: Vec<u32>,
}

/// Bone indices: 0 pelvis, 1 spine, 2 head, 3/4 upper arms, 5/6 forearms,
/// 7/8 thighs, 9/10 shins. Pivots live in the crowd shader.
pub fn character() -> Character {
    let mut mesh = MeshData::default();
    let mut bones = Vec::new();
    let mut part = |mesh: &mut MeshData, bone: u32, f: &dyn Fn(&mut MeshData)| {
        let before = mesh.vertices.len();
        f(mesh);
        bones.resize(bones.len() + mesh.vertices.len() - before, bone);
    };
    part(&mut mesh, 0, &|m| m.cylinder(Vec3::Y * 0.9, 0.18, 0.2, 0.25, 24, 4));
    part(&mut mesh, 1, &|m| m.cylinder(Vec3::Y * 1.15, 0.2, 0.24, 0.4, 24, 8));
    part(&mut mesh, 2, &|m| m.sphere(Vec3::Y * 1.72, 0.13, 3, 9, 0.05));
    for (bone, x) in [(3u32, -0.3f32), (4, 0.3)] {
        part(&mut mesh, bone, &move |m| m.cylinder(Vec3::new(x, 1.25, 0.0), 0.06, 0.055, 0.3, 12, 6));
        part(&mut mesh, bone + 2, &move |m| m.cylinder(Vec3::new(x, 0.95, 0.0), 0.05, 0.045, 0.3, 12, 6));
    }
    for (bone, x) in [(7u32, -0.1f32), (8, 0.1)] {
        part(&mut mesh, bone, &move |m| m.cylinder(Vec3::new(x, 0.47, 0.0), 0.085, 0.09, 0.43, 14, 6));
        part(&mut mesh, bone + 2, &move |m| m.cylinder(Vec3::new(x, 0.02, 0.0), 0.06, 0.075, 0.45, 14, 6));
    }
    // Arms hang: their cylinders are built pointing up from the elbow and
    // wrist, which is close enough for the lab's silhouette.
    let low = simplify(&mesh, 18.0);
    // Map simplified vertices back to the nearest original vertex's bone.
    let low_bones = low
        .vertices
        .iter()
        .map(|v| {
            let p = Vec3::from(v.position);
            let mut best = (f32::MAX, 0);
            for (i, o) in mesh.vertices.iter().enumerate() {
                let d = p.distance_squared(o.position.into());
                if d < best.0 {
                    best = (d, bones[i]);
                }
            }
            best.1
        })
        .collect();
    Character { mesh, bones, low, low_bones }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_is_deterministic_and_bounded() {
        let a = Geometry::build(2);
        let b = Geometry::build(2);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.mesh_count(), 1 + 7 * 2);
        assert_eq!(a.lods.len(), a.mesh_count() * LODS);
        for lod in &a.lods {
            assert!(lod.index_count % 3 == 0 && lod.index_count > 0);
            assert!(lod.cluster_count as usize == (lod.index_count as usize / 3).div_ceil(CLUSTER_TRIANGLES));
            let max = a.indices[lod.first_index as usize..(lod.first_index + lod.index_count) as usize].iter().max().unwrap();
            assert!((lod.base_vertex + max) < a.vertices.len() as u32);
        }
    }

    #[test]
    fn lods_reduce_triangles() {
        let g = Geometry::build(1);
        let rock = g.meshes_of(Kind::Rock)[0] as usize;
        let counts: Vec<u32> = (0..LODS).map(|l| g.lods[rock * LODS + l].index_count / 3).collect();
        assert!(counts[0] > 10_000, "{counts:?}");
        assert!(counts[1] < counts[0] && counts[2] < counts[1] && counts[3] <= counts[2], "{counts:?}");
    }

    #[test]
    fn clusters_cover_every_triangle() {
        let g = Geometry::build(1);
        for lod in &g.lods {
            let cl = &g.clusters[lod.first_cluster as usize..(lod.first_cluster + lod.cluster_count) as usize];
            let tris: u32 = cl.iter().map(|c| c.triangle_count).sum();
            assert_eq!(tris, lod.index_count / 3);
        }
    }

    #[test]
    fn placement_fills_the_request() {
        let g = Geometry::build(2);
        let inst = place_instances(&g, 5000, 1);
        let tiles = inst.iter().filter(|i| i.mesh == g.meshes_of(Kind::Tile)[0]).count();
        assert_eq!(inst.len() - tiles, 5000);
        assert!(inst.iter().all(|i| (i.mesh as usize) < g.mesh_count()));
    }

    #[test]
    fn character_has_a_bone_per_vertex() {
        let c = character();
        assert_eq!(c.bones.len(), c.mesh.vertices.len());
        assert_eq!(c.low_bones.len(), c.low.vertices.len());
        assert!(c.low.indices.len() < c.mesh.indices.len());
        assert!(c.bones.iter().all(|&b| b < 11));
    }
}
