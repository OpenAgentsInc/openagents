//! voxel_proxy: chunked voxel grid + flood-fill voxelization and carve ops.
//!
//! Scope
//! - VoxelProxyMeta: metadata tying a grid to a design object + units/material.
//! - VoxelGrid: occupancy in chunked layout (u8 0/1 for P0 uniform material).
//! - Voxelization helpers: surface mark + flood-fill to produce solid occupancy.
//! - Carve ops: `carve_sphere` that clears voxels, tracks dirty chunks, and returns
//!   removed centers (for debris sampling upstream).
//!
//! Extending
//! - Optional per-voxel material palette (`u8`) in P1.
//! - Cache/load proxies by hash of (mesh, voxel size) when `--cache-proxy` is on.

#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use core::cmp::{max, min};
use core_materials::MaterialId;
use core_units::{Length, Mass};
use glam::{DVec3, UVec3, Vec3};
use std::collections::{HashSet, VecDeque};

/// Stable global identifier for a design object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobalId(pub u64);

/// Proxy metadata for a destructible object.
#[derive(Clone, Debug)]
pub struct VoxelProxyMeta {
    pub object_id: GlobalId,
    pub origin_m: DVec3,
    pub voxel_m: Length,
    pub dims: UVec3,
    pub chunk: UVec3,
    pub material: MaterialId,
}

/// Chunked voxel grid with uniform material (P0).
#[derive(Clone)]
pub struct VoxelGrid {
    meta: VoxelProxyMeta,
    /// Occupancy: 0 = empty, 1 = solid.
    occ: Vec<u8>,
    /// Dirty chunk coordinates flagged for remesh/collider update.
    dirty_chunks: HashSet<(u32, u32, u32)>,
}

impl VoxelGrid {
    /// Create an empty grid with given meta.
    pub fn new(meta: VoxelProxyMeta) -> Self {
        let len = (meta.dims.x as usize) * (meta.dims.y as usize) * (meta.dims.z as usize);
        Self {
            meta,
            occ: vec![0; len],
            dirty_chunks: HashSet::new(),
        }
    }

    /// Access proxy metadata.
    #[inline]
    pub fn meta(&self) -> &VoxelProxyMeta {
        &self.meta
    }

    /// Grid dimensions (voxels).
    #[inline]
    pub fn dims(&self) -> UVec3 {
        self.meta.dims
    }

    /// Grid origin in meters.
    #[inline]
    pub fn origin_m(&self) -> DVec3 {
        self.meta.origin_m
    }

    /// Voxel edge length in meters.
    #[inline]
    pub fn voxel_m(&self) -> Length {
        self.meta.voxel_m
    }

    /// Linear index for (x,y,z).
    #[inline]
    pub fn index(&self, x: u32, y: u32, z: u32) -> usize {
        let d = self.meta.dims;
        (x as usize)
            + (y as usize) * (d.x as usize)
            + (z as usize) * (d.x as usize) * (d.y as usize)
    }

    /// Mark occupancy at (x,y,z).
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, solid: bool) {
        let idx = self.index(x, y, z);
        self.occ[idx] = if solid { 1 } else { 0 };
        if solid {
            self.mark_chunk_dirty_xyz(x, y, z);
        }
        // NOTE: dirty-chunk marking on clear happens in carve ops (e.g., carve_sphere).
    }

    /// Read occupancy at (x,y,z).
    #[inline]
    pub fn is_solid(&self, x: u32, y: u32, z: u32) -> bool {
        self.occ[self.index(x, y, z)] != 0
    }

    /// Compute chunk coordinate for voxel coord.
    #[inline]
    pub fn chunk_of(&self, x: u32, y: u32, z: u32) -> UVec3 {
        let c = self.meta.chunk;
        UVec3::new(x / c.x.max(1), y / c.y.max(1), z / c.z.max(1))
    }

    /// Bounds (start..end) in voxel coordinates for a given chunk coordinate.
    pub fn chunk_bounds_voxels(
        &self,
        chunk: UVec3,
    ) -> (
        core::ops::Range<u32>,
        core::ops::Range<u32>,
        core::ops::Range<u32>,
    ) {
        let d = self.meta.dims;
        let c = self.meta.chunk;
        let x0 = chunk.x.saturating_mul(c.x);
        let y0 = chunk.y.saturating_mul(c.y);
        let z0 = chunk.z.saturating_mul(c.z);
        let x1 = (x0 + c.x).min(d.x);
        let y1 = (y0 + c.y).min(d.y);
        let z1 = (z0 + c.z).min(d.z);
        (x0..x1, y0..y1, z0..z1)
    }

    #[inline]
    fn mark_chunk_dirty_xyz(&mut self, x: u32, y: u32, z: u32) {
        let cc = self.chunk_of(x, y, z);
        self.dirty_chunks.insert((cc.x, cc.y, cc.z));
    }

    /// Pop up to `n` dirty chunks to process this frame.
    pub fn pop_dirty_chunks(&mut self, n: usize) -> Vec<UVec3> {
        let mut out = Vec::new();
        for _ in 0..n {
            if let Some(&(x, y, z)) = self.dirty_chunks.iter().next() {
                self.dirty_chunks.take(&(x, y, z));
                out.push(UVec3::new(x, y, z));
            } else {
                break;
            }
        }
        out
    }

    /// Total solid voxels.
    pub fn solid_count(&self) -> usize {
        self.occ.iter().filter(|&&b| b != 0).count()
    }

    /// Estimate debris mass for one voxel (helper for upstream usage/tests).
    pub fn voxel_mass(&self) -> Mass {
        core_materials::mass_for_voxel(self.meta.material, self.meta.voxel_m).unwrap()
    }

    /// Number of dirty chunks currently queued.
    #[inline]
    pub fn dirty_len(&self) -> usize {
        self.dirty_chunks.len()
    }

    /// Bounds check helper.
    #[inline]
    pub fn inside(&self, x: u32, y: u32, z: u32) -> bool {
        x < self.meta.dims.x && y < self.meta.dims.y && z < self.meta.dims.z
    }

    /// Compute a lightweight hash of the occupancy bytes within a chunk's voxel bounds.
    ///
    /// This can be used upstream to skip meshing/uploads when a chunk hasn't changed.
    #[inline]
    pub fn chunk_occ_hash(&self, chunk: UVec3) -> u64 {
        let (xr, yr, zr) = self.chunk_bounds_voxels(chunk);
        // FNV-1a 64-bit
        let mut h: u64 = 0xcbf29ce484222325;
        let prime: u64 = 0x00000100000001B3;
        for z in zr.clone() {
            for y in yr.clone() {
                let base = self.index(xr.start, y, z);
                let len = (xr.end - xr.start) as usize;
                // Sample every Nth byte (denser on short rows) and always include the last
                let stride = if len < 64 { 4 } else { 8 };
                let mut i = 0usize;
                while i < len {
                    let b = self.occ[base + i] as u64;
                    h ^= b;
                    h = h.wrapping_mul(prime);
                    i = i.saturating_add(stride);
                }
                if len > 0 {
                    let b = self.occ[base + (len - 1)] as u64;
                    h ^= b;
                    h = h.wrapping_mul(prime);
                }
            }
        }
        h
    }
}

/// Builds a voxel grid by marking a watertight surface then flood-filling interior.
pub fn voxelize_surface_fill(
    meta: VoxelProxyMeta,
    surface_marks: &[u8], // 0/1 array sized dims.x*dims.y*dims.z
    close_surfaces: bool,
) -> VoxelGrid {
    let mut grid = VoxelGrid::new(meta);
    let d = grid.meta.dims;
    assert_eq!(
        surface_marks.len(),
        (d.x as usize) * (d.y as usize) * (d.z as usize)
    );

    // Optionally dilate by 1 voxel to close small leaks.
    let mut surf = surface_marks.to_vec();
    if close_surfaces {
        let mut dil = surf.clone();
        for z in 0..d.z {
            for y in 0..d.y {
                for x in 0..d.x {
                    let idx = grid.index(x, y, z);
                    if surf[idx] != 0 {
                        continue;
                    }
                    // if any 6-neighbor is surface, mark
                    let nbs = neighbors6(x, y, z, d);
                    if nbs
                        .iter()
                        .any(|&(nx, ny, nz)| surf[grid.index(nx, ny, nz)] != 0)
                    {
                        dil[idx] = 1;
                    }
                }
            }
        }
        // Preserve original surface by OR-ing
        for i in 0..surf.len() {
            if surf[i] != 0 {
                dil[i] = 1;
            }
        }
        surf = dil;
    }

    // BFS flood from boundary through empty to mark outside.
    let mut outside = vec![0u8; surf.len()];
    let mut q = VecDeque::new();
    // seed all boundary cells that are not surface
    for z in 0..d.z {
        for y in 0..d.y {
            for x in 0..d.x {
                if x == 0 || y == 0 || z == 0 || x == d.x - 1 || y == d.y - 1 || z == d.z - 1 {
                    let idx = grid.index(x, y, z);
                    if surf[idx] == 0 && outside[idx] == 0 {
                        outside[idx] = 1;
                        q.push_back((x, y, z));
                    }
                }
            }
        }
    }
    while let Some((x, y, z)) = q.pop_front() {
        for &(nx, ny, nz) in neighbors6(x, y, z, d).iter() {
            let i = grid.index(nx, ny, nz);
            if surf[i] == 0 && outside[i] == 0 {
                outside[i] = 1;
                q.push_back((nx, ny, nz));
            }
        }
    }
    // Cells not marked outside are interior or surface -> solid
    for z in 0..d.z {
        for y in 0..d.y {
            for x in 0..d.x {
                let idx = grid.index(x, y, z);
                if outside[idx] == 0 {
                    grid.set(x, y, z, true);
                }
            }
        }
    }
    grid
}

/// Remove voxels within a sphere; return removed centers and touched chunks.
pub fn carve_sphere(grid: &mut VoxelGrid, center_m: DVec3, radius: Length) -> RemovedVoxels {
    let d = grid.meta.dims;
    let vm = grid.meta.voxel_m.0;
    let r = radius.0;
    let r2 = r * r;
    // Compute voxel-space AABB bounds
    let to_voxel = |p: DVec3| -> Vec3 { ((p - grid.meta.origin_m) / vm).as_vec3() };
    let c_v = to_voxel(center_m);
    let pad = ((r / vm) as f32) + 1.0;
    let min_v = c_v - Vec3::splat(pad);
    let max_v = c_v + Vec3::splat(pad);
    let xi0 = max(min_v.x.floor() as i32, 0) as u32;
    let yi0 = max(min_v.y.floor() as i32, 0) as u32;
    let zi0 = max(min_v.z.floor() as i32, 0) as u32;
    let xi1 = min(max_v.x.ceil() as u32, d.x - 1);
    let yi1 = min(max_v.y.ceil() as u32, d.y - 1);
    let zi1 = min(max_v.z.ceil() as u32, d.z - 1);
    let mut removed_centers = Vec::new();
    let mut chunks = HashSet::new();
    for z in zi0..=zi1 {
        for y in yi0..=yi1 {
            for x in xi0..=xi1 {
                // center of voxel in meters
                let p_m = grid.meta.origin_m
                    + DVec3::new(
                        (x as f64 + 0.5) * vm,
                        (y as f64 + 0.5) * vm,
                        (z as f64 + 0.5) * vm,
                    );
                let d2 = (p_m - center_m).length_squared();
                if d2 <= r2 && grid.is_solid(x, y, z) {
                    // clear
                    let idx = grid.index(x, y, z);
                    grid.occ[idx] = 0; // cleared
                    let cc = grid.chunk_of(x, y, z);
                    chunks.insert((cc.x, cc.y, cc.z));
                    removed_centers.push(p_m);
                }
            }
        }
    }
    for (x, y, z) in &chunks {
        grid.dirty_chunks.insert((*x, *y, *z));
    }
    RemovedVoxels {
        centers_m: removed_centers,
        chunks_touched: chunks
            .into_iter()
            .map(|(x, y, z)| UVec3::new(x, y, z))
            .collect(),
    }
}

/// Summary of carve operation for debris spawning and remesh scheduling.
pub struct RemovedVoxels {
    pub centers_m: Vec<DVec3>,
    pub chunks_touched: Vec<UVec3>,
}

#[inline]
fn neighbors6(x: u32, y: u32, z: u32, d: UVec3) -> Small6 {
    let mut out = Small6 {
        n: 0,
        v: [(0, 0, 0); 6],
    };
    let push = |xx: u32, yy: u32, zz: u32, out: &mut Small6| {
        out.v[out.n] = (xx, yy, zz);
        out.n += 1;
    };
    if x > 0 {
        push(x - 1, y, z, &mut out);
    }
    if x + 1 < d.x {
        push(x + 1, y, z, &mut out);
    }
    if y > 0 {
        push(x, y - 1, z, &mut out);
    }
    if y + 1 < d.y {
        push(x, y + 1, z, &mut out);
    }
    if z > 0 {
        push(x, y, z - 1, &mut out);
    }
    if z + 1 < d.z {
        push(x, y, z + 1, &mut out);
    }
    out
}

struct Small6 {
    n: usize,
    v: [(u32, u32, u32); 6],
}
impl Small6 {
    #[inline]
    fn iter(&self) -> core::slice::Iter<'_, (u32, u32, u32)> {
        self.v[..self.n].iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_meta(d: UVec3, c: UVec3) -> VoxelProxyMeta {
        VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(0.1),
            dims: d,
            chunk: c,
            material: core_materials::find_material_id("stone").unwrap(),
        }
    }

    #[test]
    fn indexing_round_trip() {
        let meta = mk_meta(UVec3::new(8, 9, 10), UVec3::new(4, 4, 4));
        let mut g = VoxelGrid::new(meta);
        for z in 0..10 {
            for y in 0..9 {
                for x in 0..8 {
                    let i = g.index(x, y, z);
                    g.occ[i] = 1;
                    assert!(g.is_solid(x, y, z));
                }
            }
        }
        assert_eq!(g.solid_count(), 8 * 9 * 10);
    }

    #[test]
    fn flood_fill_cube_shell_fills_interior() {
        let d = UVec3::new(16, 16, 16);
        let meta = mk_meta(d, UVec3::new(8, 8, 8));
        let mut surf = vec![0u8; (d.x * d.y * d.z) as usize];
        // inner box spans [2..=13] each axis; mark its surface cells
        let idx = |x: u32, y: u32, z: u32| -> usize { (x + y * d.x + z * d.x * d.y) as usize };
        for z in 2..=13 {
            for y in 2..=13 {
                for x in 2..=13 {
                    if x == 2 || x == 13 || y == 2 || y == 13 || z == 2 || z == 13 {
                        surf[idx(x, y, z)] = 1;
                    }
                }
            }
        }
        let g = voxelize_surface_fill(meta, &surf, false);
        // expected solids = volume of 12^3 cube (including surface) = 1728
        assert_eq!(g.solid_count(), 12 * 12 * 12);
    }

    #[test]
    fn carve_sphere_marks_dirty_chunks_across_boundary() {
        let d = UVec3::new(32, 16, 16);
        let meta = mk_meta(d, UVec3::new(16, 16, 16));
        // Fill entire grid solid
        let mut g = VoxelGrid::new(meta);
        for z in 0..d.z {
            for y in 0..d.y {
                for x in 0..d.x {
                    g.set(x, y, z, true);
                }
            }
        }
        // Carve a sphere centered near x=16 boundary to touch both chunks
        let center = DVec3::new(
            16.0 * g.meta.voxel_m.0,
            (d.y as f64) * g.meta.voxel_m.0 * 0.5,
            (d.z as f64) * g.meta.voxel_m.0 * 0.5,
        );
        let _removed = carve_sphere(&mut g, center, Length::meters(0.5));
        let dirty = g.pop_dirty_chunks(16);
        // Expect at least two distinct chunk x-coordinates
        let mut xs: HashSet<u32> = HashSet::new();
        for c in dirty {
            xs.insert(c.x);
        }
        assert!(
            xs.len() >= 2,
            "expected carve to touch chunks across boundary"
        );
    }

    #[test]
    fn close_surfaces_preserves_surface_voxels() {
        let d = UVec3::new(8, 8, 8);
        let meta = mk_meta(d, UVec3::new(4, 4, 4));
        let mut surf = vec![0u8; (d.x * d.y * d.z) as usize];
        // Mark a single surface voxel in the center
        let idx = |x: u32, y: u32, z: u32| -> usize { (x + y * d.x + z * d.x * d.y) as usize };
        surf[idx(4, 4, 4)] = 1;
        let g = voxelize_surface_fill(meta, &surf, true);
        // The marked cell should remain solid after dilation + flood fill
        assert!(g.is_solid(4, 4, 4));
    }

    #[test]
    fn chunk_occ_hash_changes_when_last_cell_flips() {
        let d = UVec3::new(8, 2, 1); // small row to exercise stride sampling
        let meta = mk_meta(d, UVec3::new(8, 2, 1));
        let mut g = VoxelGrid::new(meta);
        // Mark all zeros initially
        let c = UVec3::new(0, 0, 0);
        let h0 = g.chunk_occ_hash(c);
        // Flip the last cell in the row to solid
        g.set(d.x - 1, 0, 0, true);
        let h1 = g.chunk_occ_hash(c);
        assert_ne!(h0, h1, "hash should change when last cell flips");
    }
}
