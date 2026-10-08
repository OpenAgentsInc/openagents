//! The GPU layout of a textured scene: merged cells, shared meshes drawn as
//! instances, a compact vertex, and baked light in a texture.
//!
//! A scene that places a model many times used to merge a world-space copy
//! of the model into its cell for every placement, so a tree placed 300
//! times cost 300 trees of vertex memory. [`Prepared`] uploads each
//! repeated mesh once, in mesh space, and draws its placements as
//! instances: a run of [`Instance`] records, one per placement, through
//! the same indexed draw. Placements a zone edits, and meshes placed once,
//! still merge into cells ([`TexturedScene::instanced`]).
//!
//! The ideas follow Unreal Engine's instanced and hierarchical instanced
//! static meshes, reimplemented here rather than copied: instances are
//! grouped by mesh and, within a mesh, into spatial runs (Unreal's
//! cluster-tree leaves, here the scene's [`super::textured::CELL`] cells),
//! and each run is culled and given its level of detail as a whole on the
//! CPU, so no compute shader or storage buffer is needed. Runs of one mesh
//! that are adjacent in its record buffer and visible together draw in one
//! call ([`draws`]).
//!
//! A vertex is 28 bytes ([`GpuVertex`]): a full-precision position, an
//! octahedral normal in two 16-bit components (Cigolle et al., "A Survey of
//! Efficient Representations for Independent Unit Vectors", 2014), a
//! full-precision texture coordinate, since the images repeat far outside
//! 0 to 1, and the vertex color. Unreal's static meshes drop their
//! normals to 8 bits a component and their texture coordinates to half
//! floats; the coordinates stay 32-bit here so tiled ground and walls
//! sample exactly as before.
//!
//! Baked light is per placed vertex, so it cannot ride in a shared mesh.
//! It lives in a light texture instead, one RGBA8 texel per vertex of
//! [`TexturedScene::merge`] in that order, which is the bake's own; the
//! vertex shader reads its texel at the instance's `light` base plus the
//! vertex index. A merged cell's vertices are numbered as the merge numbers
//! them, so their records' base is zero. The texture is [`LIGHT_WIDTH`]
//! texels wide with at most [`LIGHT_ROWS`] rows a layer, within OpenGL ES
//! 3.0's 2048-texel guarantee, so phones and WebGL2 read it the same way.

use std::collections::{BTreeMap, HashMap};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use super::textured::{self, Batch, Detail, Merged, Pass, TexturedScene, TexturedVertex};

/// One vertex as the GPU holds it: 28 bytes, where [`TexturedVertex`] is 40.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GpuVertex {
    /// Position in meters: world space in a merged cell, mesh space in a
    /// shared mesh.
    pub pos: [f32; 3],
    /// Unit normal, octahedrally encoded ([`octahedral`]).
    pub normal: [i16; 2],
    pub uv: [f32; 2],
    /// Linear RGBA multiplier.
    pub color: [u8; 4],
}

impl GpuVertex {
    /// `vertex` without its light, which the light texture holds.
    #[must_use]
    pub fn pack(vertex: &TexturedVertex) -> Self {
        Self {
            pos: vertex.pos,
            normal: octahedral(Vec3::from(vertex.normal)),
            uv: vertex.uv,
            color: vertex.color,
        }
    }
}

/// The octahedral encoding of a unit vector in two signed 16-bit
/// components: the vector projected onto the octahedron |x| + |y| + |z| = 1,
/// its lower half folded over the upper. The shader's `unoctahedral`
/// inverts it to within about 0.04 degrees.
#[must_use]
pub fn octahedral(n: Vec3) -> [i16; 2] {
    let n = n.normalize_or(Vec3::Y);
    let l1 = n.x.abs() + n.y.abs() + n.z.abs();
    let (mut x, mut y) = (n.x / l1, n.y / l1);
    if n.z < 0.0 {
        let sign = |v: f32| if v >= 0.0 { 1.0 } else { -1.0 };
        (x, y) = ((1.0 - y.abs()) * sign(x), (1.0 - x.abs()) * sign(y));
    }
    [x, y].map(|c| (c.clamp(-1.0, 1.0) * 32767.0).round() as i16)
}

/// The unit vector [`octahedral`] encoded, as the shader decodes it.
#[must_use]
pub fn unoctahedral(e: [i16; 2]) -> Vec3 {
    let [x, y] = e.map(|c| (f32::from(c) / 32767.0).max(-1.0));
    let z = 1.0 - x.abs() - y.abs();
    let t = (-z).max(0.0);
    let fold = |v: f32| if v >= 0.0 { v - t } else { v + t };
    Vec3::new(fold(x), fold(y), z).normalize_or(Vec3::Y)
}

/// One instance's record, read per instance by the vertex shader: the rows
/// of its mesh-to-world transform and where its baked light starts in the
/// light texture, plus previous transforms for temporal reprojection. 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Instance {
    /// The top three rows of the transform; the fourth is `0 0 0 1`.
    pub rows: [[f32; 4]; 3],
    /// The preceding frame's transform for the same stable instance ID.
    pub previous: [[f32; 4]; 3],
    pub color: [f32; 4],
    pub light: u32,
    pub ambient: u32,
    pub pad: [u32; 2],
}

impl Instance {
    /// The record a merged cell draws with: world space, and its vertices'
    /// light at their own indices.
    pub const MERGED: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        previous: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        color: [1.0; 4],
        light: 0,
        ambient: 0,
        pad: [0; 2],
    };

    /// An instance at `transform` whose light starts at `light`.
    #[must_use]
    pub fn new(transform: Mat4, light: u32) -> Self {
        let t = transform.transpose();
        Self {
            rows: [
                t.x_axis.to_array(),
                t.y_axis.to_array(),
                t.z_axis.to_array(),
            ],
            light,
            previous: [
                t.x_axis.to_array(),
                t.y_axis.to_array(),
                t.z_axis.to_array(),
            ],
            ..Self::MERGED
        }
    }
}

impl Instance {
    /// A rigid body's current and previous transforms and ambient override.
    pub fn dynamic(instance: &textured::DynamicInstance) -> Self {
        let mut record = Self::new(instance.current, u32::MAX);
        let p = instance.previous.transpose();
        record.previous = [
            p.x_axis.to_array(),
            p.y_axis.to_array(),
            p.z_axis.to_array(),
        ];
        record.color = instance.color;
        record.ambient = u32::from_le_bytes(instance.light);
        record.pad = [instance.id as u32, (instance.id >> 32) as u32];
        record
    }
}

/// Retains each renderer's preceding transform, independently of simulation ticks.
#[derive(Default)]
pub(crate) struct RenderedInstances {
    previous: HashMap<u64, [[f32; 4]; 3]>,
    current: HashMap<u64, [[f32; 4]; 3]>,
}

impl RenderedInstances {
    pub(crate) fn update(&mut self, records: &mut [Instance]) {
        self.current.clear();
        for record in records {
            let id = u64::from(record.pad[0]) | (u64::from(record.pad[1]) << 32);
            record.previous = self.previous.get(&id).copied().unwrap_or(record.rows);
            self.current.insert(id, record.rows);
        }
        std::mem::swap(&mut self.current, &mut self.previous);
    }

    pub(crate) fn clear(&mut self) {
        self.previous.clear();
        self.current.clear();
    }
}

/// Uploads each rigid mesh once and records each primitive's index range.
#[must_use]
pub fn rigid_meshes(scene: &TexturedScene) -> (Prepared, Vec<Vec<Batch>>) {
    let mut prepared = Prepared::default();
    let meshes = scene
        .meshes
        .iter()
        .map(|mesh| {
            mesh.primitives
                .iter()
                .map(|primitive| {
                    let base = prepared.vertices.len() as u32;
                    let first = prepared.indices.len() as u32;
                    prepared
                        .vertices
                        .extend(primitive.vertices.iter().map(GpuVertex::pack));
                    prepared
                        .indices
                        .extend(primitive.indices.iter().map(|i| i + base));
                    Batch {
                        material: primitive.material,
                        first,
                        count: primitive.indices.len() as u32,
                        min: textured::bounds(&primitive.vertices).0,
                        max: textured::bounds(&primitive.vertices).1,
                        level: textured::Level::Always,
                        run: None,
                    }
                })
                .collect()
        })
        .collect();
    // The dynamic instances supply ambient directly; keep a valid empty light map.
    (prepared, meshes)
}

/// Groups records by shared mesh, so repeated pieces draw together.
#[must_use]
pub fn rigid_frame(
    meshes: &[Vec<Batch>],
    instances: &[textured::DynamicInstance],
) -> (Vec<Instance>, Vec<Batch>) {
    let mut order: Vec<usize> = (0..instances.len()).collect();
    order.sort_by_key(|&i| (instances[i].mesh, instances[i].id));
    let mut records = Vec::with_capacity(order.len());
    let mut batches = Vec::new();
    let mut from = 0;
    while from < order.len() {
        let mesh = instances[order[from]].mesh;
        let mut to = from + 1;
        while to < order.len() && instances[order[to]].mesh == mesh {
            to += 1;
        }
        let first = records.len() as u32;
        records.extend(
            order[from..to]
                .iter()
                .map(|&i| Instance::dynamic(&instances[i])),
        );
        if let Some(parts) = meshes.get(mesh) {
            batches.extend(parts.iter().map(|part| {
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                for &index in &order[from..to] {
                    let (lo, hi) = transformed_bounds(part.min, part.max, instances[index].current);
                    min = min.min(lo);
                    max = max.max(hi);
                }
                Batch {
                    min,
                    max,
                    run: Some(Run {
                        first,
                        count: (to - from) as u32,
                    }),
                    ..*part
                }
            }));
        }
        from = to;
    }
    (records, batches)
}

fn transformed_bounds(min: Vec3, max: Vec3, transform: Mat4) -> (Vec3, Vec3) {
    let center = transform.transform_point3((min + max) * 0.5);
    let half = (max - min) * 0.5;
    let extent = (transform.x_axis.truncate() * half.x).abs()
        + (transform.y_axis.truncate() * half.y).abs()
        + (transform.z_axis.truncate() * half.z).abs();
    (center - extent, center + extent)
}

/// A run of instance records that draws one shared mesh's index range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub first: u32,
    pub count: u32,
}

/// Light texture width in texels; `photo.wgsl` shifts by its logarithm.
pub const LIGHT_WIDTH: u32 = 1 << 11;
/// Most light texture rows in one layer.
pub const LIGHT_ROWS: u32 = 2048;
/// Most light texture layers: OpenGL ES 3.0's guaranteed array size.
pub const LIGHT_LAYERS: u32 = 256;

/// The light texture's rows a layer and layers for `texels` texels: at
/// least two layers, since a one-layer array texture is a plain 2D
/// texture on OpenGL ES, and as few rows as fit them.
#[must_use]
pub fn light_extent(texels: usize) -> (u32, u32) {
    let rows = (texels.max(1) as u64).div_ceil(u64::from(LIGHT_WIDTH));
    let layers = rows.div_ceil(u64::from(LIGHT_ROWS)).max(2);
    let per_layer = rows.div_ceil(layers).max(1);
    (per_layer as u32, layers.min(u64::from(LIGHT_LAYERS)) as u32)
}

/// Bytes of a light texture for `texels` texels.
#[must_use]
pub fn light_bytes(texels: usize) -> u64 {
    let (rows, layers) = light_extent(texels);
    u64::from(LIGHT_WIDTH) * u64::from(rows) * u64::from(layers) * 4
}

/// What the renderer uploads for a textured scene.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prepared {
    /// The merged cells' world-space vertices, then each shared mesh's
    /// mesh-space vertices.
    pub vertices: Vec<GpuVertex>,
    /// Indices into `vertices`: the merged cells', as
    /// [`TexturedScene::merge`] writes them, then each shared mesh's.
    pub indices: Vec<u32>,
    /// What draws: merged cells, then runs of instances.
    pub items: Vec<Batch>,
    /// Instance records; the first is [`Instance::MERGED`].
    pub instances: Vec<Instance>,
    /// Each light texel, in [`TexturedScene::merge`]'s vertex order.
    pub lights: Vec<[u8; 4]>,
    pub fallback_groups: std::collections::BTreeSet<u16>,
}

/// A merge cell: pass, material, cell, and level.
type Key = (Pass, usize, i32, i32, Detail);

/// Counts of a scene's GPU layout ([`TexturedScene::gpu_bytes`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// Vertices and indices of the merged cells.
    pub merged_vertices: u64,
    pub merged_indices: u64,
    /// Vertices and indices of the shared meshes; a mesh placed both
    /// mirrored and not keeps two index lists.
    pub mesh_vertices: u64,
    pub mesh_indices: u64,
    /// Instance records, the merged cells' one included.
    pub instances: u64,
    /// Vertices of the whole merge: the light texels.
    pub texels: u64,
    /// Indices of the whole merge.
    pub merge_indices: u64,
}

impl Layout {
    /// Counts `scene`'s layout without building it.
    #[must_use]
    pub fn of(scene: &TexturedScene) -> Self {
        let instanced = scene.instanced();
        let mut layout = Self {
            instances: 1,
            ..Self::default()
        };
        let mut merged: BTreeMap<Key, (u64, u64)> = BTreeMap::new();
        let mut shared: BTreeMap<Key, (u64, u64)> = BTreeMap::new();
        let mut meshes = std::collections::BTreeSet::new();
        let mut lists = std::collections::BTreeSet::new();
        for (placement, &instanced) in scene.placements.iter().zip(&instanced) {
            let Some(mesh) = scene.meshes.get(placement.mesh) else {
                continue;
            };
            let cell = scene.placement_cell(placement);
            let mirrored = placement.transform.determinant() < 0.0;
            for (index, p) in mesh.primitives.iter().enumerate() {
                let Some(material) = scene.materials.get(p.material) else {
                    continue;
                };
                let key = (
                    material.alpha.pass(),
                    p.material,
                    cell.0,
                    cell.1,
                    placement.detail,
                );
                let cells = if instanced { &mut shared } else { &mut merged };
                let (indices, vertices) = cells.entry(key).or_default();
                *indices += (p.indices.len() / 3 * 3) as u64;
                *vertices += p.vertices.len() as u64;
                if instanced && p.indices.len() >= 3 {
                    layout.instances += 1;
                    if meshes.insert((placement.mesh, index)) {
                        layout.mesh_vertices += p.vertices.len() as u64;
                    }
                    if lists.insert((placement.mesh, index, mirrored)) {
                        layout.mesh_indices += (p.indices.len() / 3 * 3) as u64;
                    }
                }
            }
        }
        for (indices, vertices) in merged.values().filter(|(i, _)| *i > 0) {
            layout.merged_indices += indices;
            layout.merged_vertices += vertices;
        }
        layout.texels = layout.merged_vertices;
        layout.merge_indices = layout.merged_indices;
        for (indices, vertices) in shared.values().filter(|(i, _)| *i > 0) {
            layout.merge_indices += indices;
            layout.texels += vertices;
        }
        layout
    }

    /// Bytes on the GPU: vertices, indices, instance records, and the light
    /// texture.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        let vertex = std::mem::size_of::<GpuVertex>() as u64;
        let instance = std::mem::size_of::<Instance>() as u64;
        (self.merged_vertices + self.mesh_vertices) * vertex
            + (self.merged_indices + self.mesh_indices) * 4
            + self.instances * instance
            + light_bytes(self.texels as usize)
    }

    /// Bytes [`TexturedScene::merge`] builds on the CPU for the light
    /// bake, as [`TexturedVertex`]es and 32-bit indices.
    #[must_use]
    pub fn merge_bytes(&self) -> u64 {
        self.texels * std::mem::size_of::<TexturedVertex>() as u64 + self.merge_indices * 4
    }
}

impl Prepared {
    /// The GPU layout of `scene`: the placements that draw merged in their
    /// cells, as [`TexturedScene::merge`] begins, and each instanced mesh
    /// once with a run of records per cell, level, and material.
    ///
    /// # Errors
    ///
    /// Returns the scene's validation error.
    pub fn of_scene(scene: &TexturedScene) -> Result<Self, String> {
        scene.validate()?;
        let instanced = scene.instanced();
        let mut merged = Merged::default();
        scene.merge_into(&mut merged, |i| !instanced[i]);
        let mut out = Self {
            vertices: merged.vertices.iter().map(GpuVertex::pack).collect(),
            lights: merged.vertices.iter().map(|v| v.light).collect(),
            indices: merged.indices,
            items: merged.batches,
            instances: vec![Instance::MERGED],
            fallback_groups: scene.edits.group_fallbacks(),
        };
        drop(merged.vertices);
        // The instanced placements' cells, as the merge's second half
        // numbers their vertices: each cell's index and vertex counts and
        // box, and where each placement's primitive starts in its cell.
        struct Part {
            placement: usize,
            primitive: usize,
            key: Key,
            offset: u64,
        }
        let mut cells: BTreeMap<Key, (u64, u64, Vec3, Vec3)> = BTreeMap::new();
        let mut parts = Vec::new();
        for (index, placement) in scene.placements.iter().enumerate() {
            if !instanced[index] {
                continue;
            }
            let t = placement.transform;
            let cell = scene.placement_cell(placement);
            for (primitive, p) in scene.meshes[placement.mesh].primitives.iter().enumerate() {
                let key = (
                    scene.materials[p.material].alpha.pass(),
                    p.material,
                    cell.0,
                    cell.1,
                    placement.detail,
                );
                let entry = cells.entry(key).or_insert((
                    0,
                    0,
                    Vec3::splat(f32::INFINITY),
                    Vec3::splat(f32::NEG_INFINITY),
                ));
                if p.indices.len() >= 3 {
                    parts.push(Part {
                        placement: index,
                        primitive,
                        key,
                        offset: entry.1,
                    });
                }
                entry.0 += (p.indices.len() / 3 * 3) as u64;
                entry.1 += p.vertices.len() as u64;
                for v in &p.vertices {
                    let w = t.transform_point3(Vec3::from(v.pos));
                    entry.2 = entry.2.min(w);
                    entry.3 = entry.3.max(w);
                }
            }
        }
        // Each cell's first light texel, after the merged cells'.
        let mut bases: BTreeMap<Key, u64> = BTreeMap::new();
        let mut next = out.lights.len() as u64;
        for (key, (indices, vertices, ..)) in &cells {
            if *indices > 0 {
                bases.insert(*key, next);
                next += vertices;
            }
        }
        out.lights.resize(next as usize, textured::UNBAKED);
        // Each shared mesh's vertices once, and its indices once per
        // winding; a mirroring transform reverses its triangles.
        let mut meshes: BTreeMap<(usize, usize), u32> = BTreeMap::new();
        let mut lists: BTreeMap<(usize, usize, bool), (u32, u32)> = BTreeMap::new();
        // Each list's instances: cell, level, placement, and light base.
        let mut runs: BTreeMap<(usize, usize, usize, bool), Vec<(Key, usize, u32)>> =
            BTreeMap::new();
        for part in &parts {
            let placement = &scene.placements[part.placement];
            let p = &scene.meshes[placement.mesh].primitives[part.primitive];
            let base = bases[&part.key] + part.offset;
            for (i, v) in p.vertices.iter().enumerate() {
                out.lights[base as usize + i] = v.light;
            }
            let start = *meshes
                .entry((placement.mesh, part.primitive))
                .or_insert_with(|| {
                    let start = out.vertices.len() as u32;
                    out.vertices.extend(p.vertices.iter().map(GpuVertex::pack));
                    start
                });
            let mirrored = placement.transform.determinant() < 0.0;
            lists
                .entry((placement.mesh, part.primitive, mirrored))
                .or_insert_with(|| {
                    let first = out.indices.len() as u32;
                    for t in p.indices.chunks_exact(3) {
                        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i + start);
                        out.indices
                            .extend(if mirrored { [a, c, b] } else { [a, b, c] });
                    }
                    (first, out.indices.len() as u32 - first)
                });
            runs.entry((p.material, placement.mesh, part.primitive, mirrored))
                .or_default()
                .push((part.key, part.placement, (base as u32).wrapping_sub(start)));
        }
        // Runs: one per list, cell, and level, its records in a row, so
        // neighboring runs of one list draw together when both show.
        for ((material, mesh, primitive, mirrored), mut members) in runs {
            let (first, count) = lists[&(mesh, primitive, mirrored)];
            members.sort_by_key(|(key, placement, _)| (key.2, key.3, key.4, *placement));
            let mut i = 0;
            while i < members.len() {
                let key = members[i].0;
                let start = out.instances.len() as u32;
                while i < members.len() && members[i].0 == key {
                    let (_, placement, light) = members[i];
                    out.instances
                        .push(Instance::new(scene.placements[placement].transform, light));
                    i += 1;
                }
                let (_, _, min, max) = cells[&key];
                out.items.push(Batch {
                    material,
                    first,
                    count,
                    min,
                    max,
                    level: scene.level(key.4, (key.2, key.3)),
                    run: Some(Run {
                        first: start,
                        count: out.instances.len() as u32 - start,
                    }),
                });
            }
        }
        Ok(out)
    }

    /// A merge drawn as it is, every cell once: a figure's mesh.
    #[must_use]
    pub fn of_merged(merged: &Merged) -> Self {
        Self {
            vertices: merged.vertices.iter().map(GpuVertex::pack).collect(),
            indices: merged.indices.clone(),
            items: merged.batches.clone(),
            instances: vec![Instance::MERGED],
            lights: merged.vertices.iter().map(|v| v.light).collect(),
            fallback_groups: std::collections::BTreeSet::new(),
        }
    }

    /// Bytes on the GPU, as [`Layout::bytes`] counts them.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        (self.vertices.len() * std::mem::size_of::<GpuVertex>()
            + self.indices.len() * 4
            + self.instances.len() * std::mem::size_of::<Instance>()) as u64
            + light_bytes(self.lights.len())
    }

    /// What a frame through `view_proj` from `eye` draws when fog is total
    /// at `far` meters, each cell and run at the level it draws at from
    /// there, as the renderer counts it.
    #[must_use]
    pub fn frame_cost(&self, view_proj: Mat4, eye: Vec3, far: f32) -> textured::FrameCost {
        let order: Vec<usize> = (0..self.items.len())
            .filter(|&i| {
                let b = &self.items[i];
                b.level
                    .drawn_with_fallback(b.level.near(eye, None), &self.fallback_groups)
                    && textured::drawn(b.min, b.max, view_proj, eye, far)
            })
            .collect();
        let calls = draws(&self.items, &order);
        textured::FrameCost {
            cells: order.len() as u64,
            draws: calls.len() as u64,
            triangles: calls
                .iter()
                .map(|d| u64::from(d.count / 3) * u64::from(d.instances.count))
                .sum(),
        }
    }
}

/// One indexed draw call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Draw {
    /// The item whose material and pass it draws in.
    pub item: usize,
    pub first: u32,
    pub count: u32,
    /// The instance records it draws.
    pub instances: Run,
}

impl Draw {
    pub(crate) fn indirect(self) -> wgpu::util::DrawIndexedIndirectArgs {
        wgpu::util::DrawIndexedIndirectArgs {
            index_count: self.count,
            instance_count: self.instances.count,
            first_index: self.first,
            base_vertex: 0,
            first_instance: self.instances.first,
        }
    }
}

/// Stationary objects use camera reprojection; only changed poses need object motion.
pub(crate) fn moving_draws(items: &[Batch], records: &[Instance]) -> Vec<Draw> {
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let Some(run) = item.run else { continue };
        let mut first = run.first;
        let end = run.first + run.count;
        while first < end {
            if records[first as usize].rows == records[first as usize].previous {
                first += 1;
                continue;
            }
            let mut next = first + 1;
            while next < end && records[next as usize].rows != records[next as usize].previous {
                next += 1;
            }
            append_draw(
                items,
                &mut out,
                i,
                Run {
                    first,
                    count: next - first,
                },
            );
            first = next;
        }
    }
    out
}

fn append_draw(items: &[Batch], out: &mut Vec<Draw>, i: usize, run: Run) {
    let item = &items[i];
    if let (Some(last), Some(_)) = (out.last_mut(), item.run)
        && items[last.item].run.is_some()
        && last.first == item.first
        && last.count == item.count
        && items[last.item].material == item.material
        && last.instances.first + last.instances.count == run.first
    {
        last.instances.count += run.count;
        return;
    }
    out.push(Draw {
        item: i,
        first: item.first,
        count: item.count,
        instances: run,
    });
}

/// The draws of `items` in `order`: one per item, except that consecutive
/// runs of one shared mesh's indices whose records follow on from each
/// other draw together.
#[must_use]
pub fn draws(items: &[Batch], order: &[usize]) -> Vec<Draw> {
    let mut out: Vec<Draw> = Vec::with_capacity(order.len());
    for &i in order {
        let item = &items[i];
        let run = item.run.unwrap_or(Run { first: 0, count: 1 });
        append_draw(items, &mut out, i, run);
    }
    out
}

/// Adjacent indirect commands that share one material, with their draw costs.
#[derive(Clone, Copy)]
pub(crate) struct IndirectRun {
    pub material: usize,
    pub first: u32,
    pub count: u32,
    pub triangles: u64,
    pub instances: u64,
}

/// One conservative world-space box per indirect command, including merged runs.
pub(crate) fn indirect_cells(items: &[Batch], order: &[usize]) -> Vec<Batch> {
    let mut calls = Vec::new();
    let mut cells: Vec<Batch> = Vec::new();
    for &i in order {
        let item = &items[i];
        let before = calls.len();
        append_draw(
            items,
            &mut calls,
            i,
            item.run.unwrap_or(Run { first: 0, count: 1 }),
        );
        if calls.len() == before {
            let cell = cells.last_mut().expect("a merged indirect cell");
            cell.min = cell.min.min(item.min);
            cell.max = cell.max.max(item.max);
            cell.run = Some(calls.last().expect("a merged indirect draw").instances);
        } else {
            cells.push(*item);
        }
    }
    cells
}

/// Contiguous visible commands, preserving material boundaries and GPU offsets.
pub(crate) fn visible_indirect_runs(
    cells: &[Batch],
    runs: &[IndirectRun],
    keep: &dyn Fn(&Batch) -> bool,
) -> Vec<IndirectRun> {
    let mut visible = Vec::new();
    for run in runs {
        let end = run.first + run.count;
        let mut first = run.first;
        while first < end {
            if !keep(&cells[first as usize]) {
                first += 1;
                continue;
            }
            let mut next = first;
            let mut triangles = 0;
            let mut instances = 0;
            while next < end && keep(&cells[next as usize]) {
                let cell = &cells[next as usize];
                let copies = u64::from(cell.run.map_or(1, |r| r.count));
                instances += copies;
                triangles += u64::from(cell.count / 3) * copies;
                next += 1;
            }
            visible.push(IndirectRun {
                material: run.material,
                first,
                count: next - first,
                triangles,
                instances,
            });
            first = next;
        }
    }
    visible
}

/// Preserves indexed draw ranges and instance offsets in a compact command stream.
pub(crate) fn indirect_draws(
    items: &[Batch],
    order: &[usize],
) -> (Vec<wgpu::util::DrawIndexedIndirectArgs>, Vec<IndirectRun>) {
    let draws = draws(items, order);
    let mut commands = Vec::with_capacity(draws.len());
    let mut runs: Vec<IndirectRun> = Vec::new();
    for draw in draws {
        let material = items[draw.item].material;
        if runs.last().is_none_or(|run| run.material != material) {
            runs.push(IndirectRun {
                material,
                first: commands.len() as u32,
                count: 0,
                triangles: 0,
                instances: 0,
            });
        }
        let run = runs.last_mut().expect("an indirect material run");
        run.count += 1;
        run.triangles += u64::from(draw.count / 3) * u64::from(draw.instances.count);
        run.instances += u64::from(draw.instances.count);
        commands.push(draw.indirect());
    }
    (commands, runs)
}

#[cfg(test)]
mod tests {
    use super::super::textured::{
        AlphaMode, Level, Primitive, TexturedMaterial, TexturedMesh, TexturedScene, TexturedVertex,
    };
    use super::*;

    #[test]
    fn indirect_commands_keep_instance_offsets_ranges_and_actual_draw_costs() {
        let batch = |first, count, material, instance, copies| Batch {
            first,
            count,
            material,
            min: Vec3::ZERO,
            max: Vec3::ONE,
            level: Level::Always,
            run: Some(Run {
                first: instance,
                count: copies,
            }),
        };
        let batches = [
            batch(0, 3, 1, 0, 2),
            batch(0, 3, 1, 2, 3),
            batch(3, 6, 1, 5, 1),
            batch(9, 9, 2, 6, 4),
        ];
        let (commands, runs) = indirect_draws(&batches, &[0, 1, 2, 3]);
        assert_eq!(
            std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>(),
            20
        );
        assert_eq!(commands.len(), 3, "adjacent copies of one mesh still merge");
        assert_eq!(commands[0].instance_count, 5);
        assert_eq!(commands[1].first_index, 3);
        assert_eq!(commands[1].first_instance, 5);
        assert_eq!(commands[2].first_instance, 6);
        assert!(commands.iter().all(|c| c.base_vertex == 0));
        assert_eq!(runs.len(), 2);
        assert_eq!((runs[0].material, runs[0].first, runs[0].count), (1, 0, 2));
        assert_eq!((runs[0].triangles, runs[0].instances), (7, 6));
        assert_eq!(
            (runs[1].first, runs[1].count, runs[1].triangles),
            (2, 1, 12)
        );
        assert!(indirect_draws(&batches, &[]).0.is_empty());
    }

    #[test]
    fn indirect_culling_keeps_merged_bounds_and_nonzero_command_offsets() {
        let batch = |first, material, instance, x: f32| Batch {
            first,
            count: 6,
            material,
            min: Vec3::new(x, 0.0, 0.0),
            max: Vec3::new(x + 1.0, 1.0, 1.0),
            level: Level::Always,
            run: Some(Run {
                first: instance,
                count: 2,
            }),
        };
        let batches = [
            batch(0, 0, 0, -20.0),
            batch(0, 0, 2, 0.0),
            batch(6, 0, 4, 20.0),
            batch(12, 0, 6, 0.0),
            batch(18, 1, 8, 0.0),
            batch(24, 1, 10, 20.0),
        ];
        let order = [0, 1, 2, 3, 4, 5];
        let (commands, runs) = indirect_draws(&batches, &order);
        let cells = indirect_cells(&batches, &order);
        assert_eq!(cells.len(), commands.len());
        assert_eq!((cells[0].min.x, cells[0].max.x), (-20.0, 1.0));
        assert_eq!(cells[0].run.unwrap().count, 4);
        let visible = visible_indirect_runs(&cells, &runs, &|b| {
            textured::in_slab(b.min, b.max, Mat4::IDENTITY)
        });
        assert_eq!(
            visible
                .iter()
                .map(|r| (r.material, r.first, r.count))
                .collect::<Vec<_>>(),
            [(0, 0, 1), (0, 2, 1), (1, 3, 1)]
        );
        assert_eq!(visible.iter().map(|r| r.triangles).sum::<u64>(), 16);
        assert_eq!(commands[visible[1].first as usize].first_instance, 6);
        assert_eq!(commands[visible[2].first as usize].first_instance, 8);
    }

    #[test]
    fn motion_splits_mixed_runs_and_omits_exact_stationary_poses() {
        let batch = Batch {
            first: 12,
            count: 9,
            material: 0,
            min: Vec3::ZERO,
            max: Vec3::ONE,
            level: Level::Always,
            run: Some(Run { first: 0, count: 5 }),
        };
        let mut records = vec![Instance::MERGED; 5];
        for i in [1, 2, 4] {
            records[i].rows[0][3] = 1.0;
        }
        let draws = moving_draws(&[batch], &records);
        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].instances, Run { first: 1, count: 2 });
        assert_eq!(draws[1].instances, Run { first: 4, count: 1 });
        assert_eq!(draws[0].indirect().first_instance, 1);
        assert_eq!(draws[1].indirect().first_instance, 4);
        assert_eq!(draws[0].indirect().first_index, 12);
        for record in &mut records {
            record.previous = record.rows;
        }
        assert!(moving_draws(&[batch], &records).is_empty());
        records[3].rows[1][0] = f32::EPSILON;
        assert_eq!(
            moving_draws(&[batch], &records)[0].instances.first,
            3,
            "rotation is not rounded away"
        );
    }

    #[test]
    fn affine_bounds_match_transformed_corners_with_rotation_and_scale() {
        let min = Vec3::new(-2.0, -1.0, 0.5);
        let max = Vec3::new(3.0, 4.0, 2.5);
        for i in 0..100 {
            let transform = Mat4::from_scale_rotation_translation(
                Vec3::new(0.5, 2.0, 1.5),
                glam::Quat::from_euler(glam::EulerRot::XYZ, i as f32 * 0.02, 0.7, -0.3),
                Vec3::new(8.0, -3.0, 20.0),
            );
            let (lo, hi) = transformed_bounds(min, max, transform);
            let mut expected_lo = Vec3::splat(f32::INFINITY);
            let mut expected_hi = Vec3::splat(f32::NEG_INFINITY);
            for corner in 0..8 {
                let p = Vec3::new(
                    if corner & 1 == 0 { min.x } else { max.x },
                    if corner & 2 == 0 { min.y } else { max.y },
                    if corner & 4 == 0 { min.z } else { max.z },
                );
                let p = transform.transform_point3(p);
                expected_lo = expected_lo.min(p);
                expected_hi = expected_hi.max(p);
            }
            assert!(lo.distance(expected_lo) < 1e-5);
            assert!(hi.distance(expected_hi) < 1e-5);
        }
    }

    #[test]
    fn octahedral_normals_round_trip_within_a_twentieth_of_a_degree() {
        let mut worst = 0.0f32;
        for i in 0..40 {
            for j in 0..80 {
                let theta = std::f32::consts::PI * (i as f32 + 0.5) / 40.0;
                let phi = std::f32::consts::TAU * j as f32 / 80.0;
                let n = Vec3::new(
                    theta.sin() * phi.cos(),
                    theta.sin() * phi.sin(),
                    theta.cos(),
                );
                let back = unoctahedral(octahedral(n));
                worst = worst.max(n.dot(back).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        for n in [
            Vec3::X,
            Vec3::NEG_X,
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::Z,
            Vec3::NEG_Z,
        ] {
            assert!(unoctahedral(octahedral(n)).distance(n) < 1e-4, "{n}");
        }
        assert!(worst < 0.05, "{worst} degrees");
        assert_eq!(std::mem::size_of::<GpuVertex>(), 28);
        assert_eq!(std::mem::size_of::<Instance>(), 128);
    }

    fn tree() -> TexturedMesh {
        let v = |x: f32, y: f32, z: f32| TexturedVertex::new(Vec3::new(x, y, z), Vec3::Y, [x, z]);
        TexturedMesh {
            primitives: vec![Primitive {
                vertices: vec![v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 2.0, 0.0)],
                indices: vec![0, 1, 2],
                material: 0,
            }],
        }
    }

    #[test]
    fn seven_hundred_rigid_chunks_share_uploaded_vertices_and_one_draw() {
        let mut scene = TexturedScene::default();
        scene.add_mesh(tree());
        let (prepared, meshes) = rigid_meshes(&scene);
        let instances: Vec<_> = (0..700)
            .map(|id| textured::DynamicInstance {
                id,
                mesh: 0,
                current: Mat4::from_translation(Vec3::X * id as f32),
                previous: Mat4::from_translation(Vec3::X * (id as f32 - 0.25)),
                color: [0.8, 0.7, 0.6, 1.0],
                light: [100, 110, 120, 200],
                settled: false,
            })
            .collect();
        let (records, batches) = rigid_frame(&meshes, &instances);
        assert_eq!(prepared.vertices.len(), 3, "one local source mesh");
        assert_eq!(prepared.indices, [0, 1, 2]);
        assert_eq!(records.len(), 700);
        assert_eq!(batches.len(), 1);
        assert_eq!(
            batches[0].run,
            Some(Run {
                first: 0,
                count: 700
            })
        );
        let calls = draws(&batches, &[0]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].instances.count, 700);
        assert_eq!(records[17].rows[0][3], 17.0);
        assert_eq!(records[17].previous[0][3], 16.75);
        assert_eq!(records[17].color, instances[17].color);
        assert_eq!(records[17].light, u32::MAX);
        assert_eq!(records[17].ambient.to_le_bytes(), instances[17].light);
        assert_eq!(batches[0].min.x, 0.0);
        assert_eq!(batches[0].max.x, 700.0);
        assert_eq!(std::mem::offset_of!(Instance, previous), 48);
        assert_eq!(std::mem::offset_of!(Instance, color), 96);
        assert_eq!(std::mem::offset_of!(Instance, light), 112);
    }

    #[test]
    fn rigid_records_sort_by_mesh_and_stable_id_without_losing_previous_pose() {
        let mut scene = TexturedScene::default();
        scene.add_mesh(tree());
        scene.add_mesh(tree());
        let (_, meshes) = rigid_meshes(&scene);
        let instance = |id, mesh| textured::DynamicInstance {
            id,
            mesh,
            current: Mat4::IDENTITY,
            previous: Mat4::from_translation(Vec3::X * id as f32),
            color: [1.0; 4],
            light: [0; 4],
            settled: false,
        };
        let (records, batches) =
            rigid_frame(&meshes, &[instance(9, 1), instance(5, 0), instance(3, 0)]);
        assert_eq!(
            records.iter().map(|r| r.previous[0][3]).collect::<Vec<_>>(),
            [3.0, 5.0, 9.0]
        );
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].run.unwrap().count, 2);
        assert_eq!(batches[1].run.unwrap().first, 2);
    }

    #[test]
    fn rigid_motion_uses_last_rendered_pose_across_multiple_or_no_ticks() {
        let record = |id: u64, x: f32, tick_previous: f32| {
            Instance::dynamic(&textured::DynamicInstance {
                id,
                mesh: 0,
                current: Mat4::from_translation(Vec3::X * x),
                previous: Mat4::from_translation(Vec3::X * tick_previous),
                color: [1.0; 4],
                light: [0; 4],
                settled: false,
            })
        };
        let mut history = RenderedInstances::default();
        let mut first = [record(9, 1.0, 0.0), record(u64::MAX, 20.0, 19.0)];
        history.update(&mut first);
        assert_eq!(
            first[0].previous, first[0].rows,
            "new object has no history"
        );

        // Three simulation ticks pass before the next render. Record order changes.
        let mut next = [record(u64::MAX, 23.0, 22.0), record(9, 4.0, 3.0)];
        history.update(&mut next);
        assert_eq!(next[0].previous[0][3], 20.0);
        assert_eq!(next[1].previous[0][3], 1.0);

        let mut repeated = [record(9, 4.0, 3.0)];
        history.update(&mut repeated);
        assert_eq!(repeated[0].previous[0][3], 4.0, "no tick means no motion");
        let mut returned = [record(u64::MAX, 30.0, 29.0)];
        history.update(&mut returned);
        assert_eq!(returned[0].previous, returned[0].rows, "absent IDs retire");
        history.clear();
        returned[0].rows[0][3] = 31.0;
        history.update(&mut returned);
        assert_eq!(
            returned[0].previous, returned[0].rows,
            "world reset retires history"
        );

        let mut other_renderer = RenderedInstances::default();
        let mut independent = [record(9, 100.0, 99.0)];
        other_renderer.update(&mut independent);
        assert_eq!(independent[0].previous, independent[0].rows);
    }

    #[test]
    fn rigid_validation_rejects_bad_frames_and_rechecks_mutated_sources() {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial::default());
        scene.add_mesh(tree());
        let mut figure = textured::InstancedFigure {
            scene: std::sync::Arc::new(scene),
            instances: std::sync::Arc::new(vec![textured::DynamicInstance {
                id: 1,
                mesh: 0,
                current: Mat4::IDENTITY,
                previous: Mat4::IDENTITY,
                color: [1.0; 4],
                light: [0; 4],
                settled: false,
            }]),
        };
        figure.validate().unwrap();
        let valid = figure.instances[0];
        std::sync::Arc::make_mut(&mut figure.instances).push(valid);
        assert!(figure.validate().is_err(), "duplicate stable IDs");
        std::sync::Arc::make_mut(&mut figure.instances).pop();
        std::sync::Arc::make_mut(&mut figure.instances)[0].previous =
            Mat4::from_translation(Vec3::splat(f32::NAN));
        assert!(figure.validate().is_err(), "nonfinite prior transforms");
        std::sync::Arc::make_mut(&mut figure.instances)[0] = valid;
        let before = figure.scene.clone();
        std::sync::Arc::make_mut(&mut figure.scene).meshes[0].primitives[0].indices[0] = 100;
        assert!(!std::sync::Arc::ptr_eq(&before, &figure.scene));
        assert!(
            figure.validate().is_err(),
            "changed immutable source receives validation"
        );
    }

    /// Four placements of one mesh, two instanced in one cell, one
    /// instanced in another, and one merged.
    fn forest() -> TexturedScene {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial::default());
        let mesh = scene.add_mesh(tree());
        let at = |x: f32, z: f32| Mat4::from_translation(Vec3::new(x, 0.0, z));
        scene.place_instanced(mesh, at(1.0, 1.0), Detail::Always);
        scene.place_instanced(mesh, at(2.0, 2.0), Detail::Always);
        scene.place(mesh, at(3.0, 3.0));
        scene.place_instanced(mesh, at(20.0, 1.0), Detail::Always);
        scene
    }

    #[test]
    fn repeated_meshes_upload_once_and_draw_as_runs_per_cell() {
        let scene = forest();
        assert_eq!(scene.instanced(), [true, true, false, true]);
        let prepared = Prepared::of_scene(&scene).unwrap();
        // The merged placement's 3 vertices, and the shared mesh's 3.
        assert_eq!(prepared.vertices.len(), 6);
        assert_eq!(prepared.indices.len(), 6);
        // One merged cell and a run in each of two cells.
        assert_eq!(prepared.items.len(), 3);
        assert_eq!(prepared.items[0].run, None);
        assert_eq!(prepared.items[1].run, Some(Run { first: 1, count: 2 }));
        assert_eq!(prepared.items[2].run, Some(Run { first: 3, count: 1 }));
        // The light texels are the merge's vertices, in its order.
        let merged = scene.merge().unwrap();
        assert_eq!(prepared.lights.len(), merged.vertices.len());
        // Each instance, transformed, lands on its merged vertices, and its
        // light base finds their texels.
        for item in &prepared.items[1..] {
            let run = item.run.unwrap();
            for record in &prepared.instances[run.first as usize..][..run.count as usize] {
                for &index in &prepared.indices[item.first as usize..][..item.count as usize] {
                    let v = prepared.vertices[index as usize];
                    let p = Vec3::from(v.pos).extend(1.0);
                    let world = Vec3::new(
                        glam::Vec4::from(record.rows[0]).dot(p),
                        glam::Vec4::from(record.rows[1]).dot(p),
                        glam::Vec4::from(record.rows[2]).dot(p),
                    );
                    let texel = record.light.wrapping_add(index) as usize;
                    assert_eq!(Vec3::from(merged.vertices[texel].pos), world);
                }
            }
        }
        // Both runs of the mesh draw in one call when both show, and the
        // merged cell in its own.
        let all: Vec<usize> = (0..prepared.items.len()).collect();
        let calls = draws(&prepared.items, &all);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].instances, Run { first: 1, count: 3 });
        // Only placements a zone may edit have index ranges.
        let ranges = scene.index_ranges();
        assert!(ranges[0].is_empty() && ranges[1].is_empty() && ranges[3].is_empty());
        assert_eq!(ranges[2].len(), 1);
        assert_eq!(
            scene.range_indices(2, &ranges[2][0]),
            prepared.indices[..3].to_vec()
        );
        let layout = Layout::of(&scene);
        assert_eq!(layout.bytes(), prepared.bytes());
        assert_eq!(layout.texels, merged.vertices.len() as u64);
    }

    #[test]
    fn a_mesh_placed_once_merges_and_a_mirrored_instance_keeps_its_front_faces() {
        let mut scene = TexturedScene::default();
        scene.add_material(TexturedMaterial {
            alpha: AlphaMode::Opaque,
            ..TexturedMaterial::default()
        });
        let lone = scene.add_mesh(tree());
        let pair = scene.add_mesh(tree());
        scene.place_instanced(lone, Mat4::IDENTITY, Detail::Always);
        scene.place_instanced(pair, Mat4::IDENTITY, Detail::Always);
        scene.place_instanced(
            pair,
            Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
            Detail::Always,
        );
        assert_eq!(scene.instanced(), [false, true, true]);
        let prepared = Prepared::of_scene(&scene).unwrap();
        // Two index lists for the pair, one reversed.
        let runs: Vec<&Batch> = prepared.items.iter().filter(|b| b.run.is_some()).collect();
        assert_eq!(runs.len(), 2);
        let list = |b: &Batch| prepared.indices[b.first as usize..][..b.count as usize].to_vec();
        let (a, b) = (list(runs[0]), list(runs[1]));
        assert_eq!(a, [b[0], b[2], b[1]]);
        assert_eq!(runs[0].level, Level::Always);
    }

    #[test]
    fn light_textures_stay_within_es_limits() {
        assert_eq!(light_extent(1), (1, 2));
        assert_eq!(light_extent(5_000_000), (1221, 2));
        let (rows, layers) = light_extent(40_000_000);
        assert!(rows <= LIGHT_ROWS && layers <= LIGHT_LAYERS);
        assert!(u64::from(rows * layers) * u64::from(LIGHT_WIDTH) >= 40_000_000);
    }
}
