//! voxel_mesh: greedy meshing for voxel grids (CPU-only).
//!
//! Scope
//! - Convert a `voxel_proxy::VoxelGrid` occupancy to triangle mesh buffers by extracting
//!   faces on solid→empty boundaries.
//! - Greedy merge co-planar faces per slice to minimize quad count.
//!
//! Extending
//! - Per-chunk meshing for dirty sets (integrate with queue from voxel_proxy).
//! - Material IDs per quad for mixed-material P1.

#![forbid(unsafe_code)]

use glam::{UVec3, Vec3};
use voxel_proxy::VoxelGrid;

/// Simple mesh buffers (positions, normals, indices).
#[derive(Default, Clone)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl MeshBuffers {
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }
}

/// Generate a greedy-meshed surface for the entire grid.
pub fn greedy_mesh_all(grid: &VoxelGrid) -> MeshBuffers {
    let d = grid.meta().dims;
    let vm = grid.meta().voxel_m.0 as f32;
    let origin = grid.meta().origin_m.as_vec3();
    let mut mesh = MeshBuffers::default();
    // Iterate three axes
    for axis in 0..3 {
        let (w, h, ld, _step_w, _step_h, _step_ld) = plane_dims(axis, d);
        // For each layer along the axis
        for layer in 0..ld {
            // Build face mask for this slice for positive normal
            let mut mask = vec![false; (w * h) as usize];
            let mut mask_neg = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let (x, y, z) = to_xyz(axis, i, j, layer);
                    // Neighbor checks; treat out-of-bounds neighbor as empty.
                    let here = if inside(d, x, y, z) {
                        grid.is_solid(x, y, z)
                    } else {
                        false
                    };
                    let there_neg = match axis {
                        0 => {
                            if x == 0 {
                                false
                            } else {
                                grid.is_solid(x - 1, y, z)
                            }
                        }
                        1 => {
                            if y == 0 {
                                false
                            } else {
                                grid.is_solid(x, y - 1, z)
                            }
                        }
                        _ => {
                            if z == 0 {
                                false
                            } else {
                                grid.is_solid(x, y, z - 1)
                            }
                        }
                    };
                    let there_pos = match axis {
                        0 => {
                            if x + 1 >= d.x {
                                false
                            } else {
                                grid.is_solid(x + 1, y, z)
                            }
                        }
                        1 => {
                            if y + 1 >= d.y {
                                false
                            } else {
                                grid.is_solid(x, y + 1, z)
                            }
                        }
                        _ => {
                            if z + 1 >= d.z {
                                false
                            } else {
                                grid.is_solid(x, y, z + 1)
                            }
                        }
                    };
                    let m_idx = (i + j * w) as usize;
                    // Solid→empty in +axis direction => positive face
                    // Solid→empty in -axis direction => negative face
                    mask[m_idx] = here && !there_pos;
                    mask_neg[m_idx] = here && !there_neg;
                }
            }
            // Emit quads for both directions
            greedy_emit(&mut mesh, &mask, axis, layer, w, h, vm, origin, true);
            greedy_emit(&mut mesh, &mask_neg, axis, layer, w, h, vm, origin, false);
        }
    }
    mesh
}

/// Generate a meshed surface for a single chunk by filtering the full mesh to triangles
/// whose centroids belong to the given chunk coordinate. This prioritizes correctness
/// and chunk ownership of faces; optimized per-slice greedy meshing can layer on later.
pub fn greedy_mesh_chunk(grid: &VoxelGrid, chunk: UVec3) -> MeshBuffers {
    // Per-chunk, slice-local greedy meshing. We only emit faces whose
    // adjacent solid voxel lies inside the given chunk to avoid duplication
    // across chunk boundaries.
    let (xr, yr, zr) = grid.chunk_bounds_voxels(chunk);
    let vm = grid.meta().voxel_m.0 as f32;
    let origin_world = grid.meta().origin_m.as_vec3();
    let offset = Vec3::new(xr.start as f32, yr.start as f32, zr.start as f32);
    let origin = origin_world + offset * vm;
    let dims = grid.meta().dims;

    let mut mesh = MeshBuffers::default();

    // Axis 0 (X): w = Y, h = Z, ld = X
    {
        let w = yr.end - yr.start;
        let h = zr.end - zr.start;
        let ld = xr.end - xr.start;
        // Positive normal (+X): own faces where (x,y,z) is solid inside chunk and (x-1,y,z) is empty
        for layer_rel in 0..ld {
            let x = xr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let y = yr.start + i;
                    let z = zr.start + j;
                    let here = grid.is_solid(x, y, z);
                    let there = if x > 0 {
                        grid.is_solid(x - 1, y, z)
                    } else {
                        false
                    };
                    mask[(i + j * w) as usize] = here && !there;
                }
            }
            greedy_emit(&mut mesh, &mask, 0, layer_rel, w, h, vm, origin, true);
        }
        // Negative normal (-X): own faces at plane x where left side (inside this chunk)
        // is empty and right side (x,y,z) is solid. This biases ownership of boundary
        // faces to the lower X chunk (left) while still emitting interior faces correctly.
        for layer_rel in 1..=ld {
            // layer_rel corresponds to current (x) and x-1 must be inside [xr.start..xr.end)
            let x = xr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let y = yr.start + i;
                    let z = zr.start + j;
                    // Left side 'here': inside this chunk -> treat as empty when x==xr.end.
                    let here = if x <= xr.end {
                        false
                    } else {
                        grid.is_solid(x - 1, y, z)
                    };
                    // Right side 'there': sample (x,y,z) if in-bounds
                    let there = if x < dims.x {
                        grid.is_solid(x, y, z)
                    } else {
                        false
                    };
                    mask[(i + j * w) as usize] = !here && there;
                }
            }
            greedy_emit(&mut mesh, &mask, 0, layer_rel, w, h, vm, origin, false);
        }
    }

    // Axis 1 (Y): w = X, h = Z, ld = Y
    {
        let w = xr.end - xr.start;
        let h = zr.end - zr.start;
        let ld = yr.end - yr.start;
        // +Y
        for layer_rel in 0..ld {
            let y = yr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let x = xr.start + i;
                    let z = zr.start + j;
                    let here = grid.is_solid(x, y, z);
                    let there = if y > 0 {
                        grid.is_solid(x, y - 1, z)
                    } else {
                        false
                    };
                    mask[(i + j * w) as usize] = here && !there;
                }
            }
            greedy_emit(&mut mesh, &mask, 1, layer_rel, w, h, vm, origin, true);
        }
        // -Y (y-1 must be inside chunk). Bias ownership to lower chunk along Y.
        for layer_rel in 1..=ld {
            let y = yr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let x = xr.start + i;
                    let z = zr.start + j;
                    let here = if y < dims.y && y < yr.end {
                        grid.is_solid(x, y, z)
                    } else {
                        false
                    };
                    let there = grid.is_solid(x, y - 1, z);
                    mask[(i + j * w) as usize] = !here && there;
                }
            }
            greedy_emit(&mut mesh, &mask, 1, layer_rel, w, h, vm, origin, false);
        }
    }

    // Axis 2 (Z): w = X, h = Y, ld = Z
    {
        let w = xr.end - xr.start;
        let h = yr.end - yr.start;
        let ld = zr.end - zr.start;
        // +Z
        for layer_rel in 0..ld {
            let z = zr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let x = xr.start + i;
                    let y = yr.start + j;
                    let here = grid.is_solid(x, y, z);
                    let there = if z > 0 {
                        grid.is_solid(x, y, z - 1)
                    } else {
                        false
                    };
                    mask[(i + j * w) as usize] = here && !there;
                }
            }
            greedy_emit(&mut mesh, &mask, 2, layer_rel, w, h, vm, origin, true);
        }
        // -Z (z-1 must be inside chunk). Bias ownership to lower chunk along Z.
        for layer_rel in 1..=ld {
            let z = zr.start + layer_rel;
            let mut mask = vec![false; (w * h) as usize];
            for j in 0..h {
                for i in 0..w {
                    let x = xr.start + i;
                    let y = yr.start + j;
                    let here = if z < dims.z && z < zr.end {
                        grid.is_solid(x, y, z)
                    } else {
                        false
                    };
                    let there = grid.is_solid(x, y, z - 1);
                    mask[(i + j * w) as usize] = !here && there;
                }
            }
            greedy_emit(&mut mesh, &mask, 2, layer_rel, w, h, vm, origin, false);
        }
    }

    mesh
}

/// Naive mesher for a single chunk: emits each boundary face (solid next to empty)
/// as two triangles. This is simple and robust for demos.
pub fn naive_mesh_chunk(grid: &VoxelGrid, chunk: UVec3) -> MeshBuffers {
    let (xr, yr, zr) = grid.chunk_bounds_voxels(chunk);
    let vm = grid.meta().voxel_m.0 as f32;
    let origin_world = grid.meta().origin_m.as_vec3();
    let offset = glam::Vec3::new(xr.start as f32, yr.start as f32, zr.start as f32);
    let origin = origin_world + offset * vm;
    let mut mesh = MeshBuffers::default();
    let dims = grid.meta().dims;
    let add_face = |mesh: &mut MeshBuffers,
                    p0: glam::Vec3,
                    p1: glam::Vec3,
                    p2: glam::Vec3,
                    p3: glam::Vec3,
                    n: glam::Vec3| {
        let base = mesh.positions.len() as u32;
        mesh.positions.extend_from_slice(&[
            [p0.x, p0.y, p0.z],
            [p1.x, p1.y, p1.z],
            [p2.x, p2.y, p2.z],
            [p3.x, p3.y, p3.z],
        ]);
        let nn = [n.x, n.y, n.z];
        mesh.normals.extend_from_slice(&[nn, nn, nn, nn]);
        // CCW
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    for z in zr.clone() {
        for y in yr.clone() {
            for x in xr.clone() {
                if !grid.is_solid(x, y, z) {
                    continue;
                }
                let wx = origin.x + (x - xr.start) as f32 * vm;
                let wy = origin.y + (y - yr.start) as f32 * vm;
                let wz = origin.z + (z - zr.start) as f32 * vm;
                let v000 = glam::Vec3::new(wx, wy, wz);
                let vx1 = v000 + glam::Vec3::new(vm, 0.0, 0.0);
                let vy1 = v000 + glam::Vec3::new(0.0, vm, 0.0);
                let vz1 = v000 + glam::Vec3::new(0.0, 0.0, vm);
                let v110 = v000 + glam::Vec3::new(vm, vm, 0.0);
                let v101 = v000 + glam::Vec3::new(vm, 0.0, vm);
                let v011 = v000 + glam::Vec3::new(0.0, vm, vm);
                let v111 = v000 + glam::Vec3::new(vm, vm, vm);
                // -X face if x==0 or neighbor empty
                if x == 0 || !grid.is_solid(x - 1, y, z) {
                    add_face(&mut mesh, v000, vz1, v011, vy1, glam::Vec3::NEG_X);
                }
                // +X face
                if x + 1 >= dims.x || !grid.is_solid(x + 1, y, z) {
                    add_face(&mut mesh, vx1, v110, v111, v101, glam::Vec3::X);
                }
                // -Y face
                if y == 0 || !grid.is_solid(x, y - 1, z) {
                    add_face(&mut mesh, v000, vx1, v101, vz1, glam::Vec3::NEG_Y);
                }
                // +Y face
                if y + 1 >= dims.y || !grid.is_solid(x, y + 1, z) {
                    add_face(&mut mesh, vy1, v011, v111, v110, glam::Vec3::Y);
                }
                // -Z face
                if z == 0 || !grid.is_solid(x, y, z - 1) {
                    add_face(&mut mesh, v000, vy1, v110, vx1, glam::Vec3::NEG_Z);
                }
                // +Z face
                if z + 1 >= dims.z || !grid.is_solid(x, y, z + 1) {
                    add_face(&mut mesh, vz1, v101, v111, v011, glam::Vec3::Z);
                }
            }
        }
    }
    mesh
}

fn plane_dims(axis: u32, d: UVec3) -> (u32, u32, u32, UVec3, UVec3, UVec3) {
    match axis {
        // (w,h,ld)
        0 => (
            d.y,
            d.z,
            d.x,
            UVec3::new(0, 1, 0),
            UVec3::new(0, 0, 1),
            UVec3::new(1, 0, 0),
        ),
        1 => (
            d.x,
            d.z,
            d.y,
            UVec3::new(1, 0, 0),
            UVec3::new(0, 0, 1),
            UVec3::new(0, 1, 0),
        ),
        _ => (
            d.x,
            d.y,
            d.z,
            UVec3::new(1, 0, 0),
            UVec3::new(0, 1, 0),
            UVec3::new(0, 0, 1),
        ),
    }
}

#[inline]
fn to_xyz(axis: u32, i: u32, j: u32, l: u32) -> (u32, u32, u32) {
    match axis {
        0 => (l, i, j),
        1 => (i, l, j),
        _ => (i, j, l),
    }
}

#[inline]
fn inside(d: UVec3, x: u32, y: u32, z: u32) -> bool {
    x < d.x && y < d.y && z < d.z
}

#[allow(clippy::too_many_arguments)]
fn greedy_emit(
    mesh: &mut MeshBuffers,
    mask: &[bool],
    axis: u32,
    layer: u32,
    w: u32,
    h: u32,
    vm: f32,
    origin: Vec3,
    pos_normal: bool,
) {
    let mut skipped = vec![false; mask.len()];
    let normal = match (axis, pos_normal) {
        (0, true) => Vec3::X,
        (0, false) => -Vec3::X,
        (1, true) => Vec3::Y,
        (1, false) => -Vec3::Y,
        (2, true) => Vec3::Z,
        (2, false) => -Vec3::Z,
        _ => Vec3::Z,
    };
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            let idx = (x + y * w) as usize;
            if mask[idx] && !skipped[idx] {
                // Find maximal width
                let mut w_run = 1;
                while x + w_run < w {
                    let ii = (x + w_run + y * w) as usize;
                    if mask[ii] && !skipped[ii] {
                        w_run += 1;
                    } else {
                        break;
                    }
                }
                // Find maximal height
                let mut h_run = 1;
                'outer: while y + h_run < h {
                    for xx in x..(x + w_run) {
                        let jj = (xx + (y + h_run) * w) as usize;
                        if !mask[jj] || skipped[jj] {
                            break 'outer;
                        }
                    }
                    h_run += 1;
                }
                // Mark visited
                for yy in y..(y + h_run) {
                    for xx in x..(x + w_run) {
                        skipped[(xx + yy * w) as usize] = true;
                    }
                }
                // Emit quad at rectangle [x..x+w_run), [y..y+h_run)
                emit_rect(
                    mesh, axis, layer, x, y, w_run, h_run, vm, origin, normal, pos_normal,
                );
                x += w_run; // advance past rect
            } else {
                x += 1;
            }
        }
        y += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_rect(
    mesh: &mut MeshBuffers,
    axis: u32,
    layer: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    vm: f32,
    origin: Vec3,
    normal: Vec3,
    pos_normal: bool,
) {
    // Compute the 4 corners in voxel space at face plane
    let (ax_u, ax_v, ax_w) = match axis {
        0 => (Vec3::Y, Vec3::Z, Vec3::X),
        1 => (Vec3::X, Vec3::Z, Vec3::Y),
        _ => (Vec3::X, Vec3::Y, Vec3::Z),
    };
    // layer is along ax_w; positive-normal faces live at the far side of the voxel cell
    // (layer + 1), while negative-normal faces live at the near side (layer).
    let base = ax_w * (layer as f32 + if pos_normal { 1.0 } else { 0.0 });
    let p0 = base + ax_u * (x as f32) + ax_v * (y as f32);
    let p1 = base + ax_u * ((x + w) as f32) + ax_v * (y as f32);
    let p2 = base + ax_u * ((x + w) as f32) + ax_v * ((y + h) as f32);
    let p3 = base + ax_u * (x as f32) + ax_v * ((y + h) as f32);
    let add = |mesh: &mut MeshBuffers, p: Vec3, n: Vec3| {
        let wm = origin + p * vm; // place on voxel edge lines in meters
        mesh.positions.push([wm.x, wm.y, wm.z]);
        mesh.normals.push([n.x, n.y, n.z]);
    };
    let i0 = mesh.positions.len() as u32;
    add(mesh, p0, normal);
    add(mesh, p1, normal);
    add(mesh, p2, normal);
    add(mesh, p3, normal);
    // Two triangles. For axis=Y the base UV coordinate system leads to opposite
    // winding; compensate by inverting the choice.
    let use_normal_order = if axis == 1 { !pos_normal } else { pos_normal };
    if use_normal_order {
        mesh.indices
            .extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
    } else {
        mesh.indices
            .extend_from_slice(&[i0, i0 + 2, i0 + 1, i0, i0 + 3, i0 + 2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_materials::find_material_id;
    use core_units::Length;
    use glam::{DVec3, UVec3};
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_full_grid(d: UVec3) -> voxel_proxy::VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: d,
            chunk: UVec3::new(16, 16, 16),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        for z in 0..d.z {
            for y in 0..d.y {
                for x in 0..d.x {
                    g.set(x, y, z, true);
                }
            }
        }
        g
    }

    #[test]
    fn solid_cube_produces_six_quads() {
        let g = mk_full_grid(UVec3::new(3, 3, 3));
        let m = greedy_mesh_all(&g);
        assert_eq!(m.quad_count(), 6);
    }

    #[test]
    fn rod_1x1x8_produces_six_quads() {
        let g = mk_full_grid(UVec3::new(1, 1, 8));
        // Already filled by mk_full_grid; just mesh
        let m = greedy_mesh_all(&g);
        assert_eq!(m.quad_count(), 6);
    }

    #[test]
    fn single_voxel_produces_36_indices() {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(1, 1, 1),
            chunk: UVec3::new(1, 1, 1),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(0, 0, 0, true);
        let m = greedy_mesh_all(&g);
        assert_eq!(m.indices.len(), 36);
    }

    #[test]
    fn chunk_mesher_filters_to_correct_chunk() {
        // 32x16x16 with chunk size 16^3: voxel at x=17,y=2,z=3
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(32, 16, 16),
            chunk: UVec3::new(16, 16, 16),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(17, 2, 3, true);
        let m0 = greedy_mesh_chunk(&g, UVec3::new(0, 0, 0));
        let m1 = greedy_mesh_chunk(&g, UVec3::new(1, 0, 0));
        assert_eq!(m0.indices.len(), 0);
        assert_eq!(m1.indices.len(), 36);
    }

    #[test]
    fn boundary_face_goes_to_lower_chunk_with_bias() {
        // Single voxel exactly at x=16 boundary between chunks (0,0,0) and (1,0,0)
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(32, 4, 4),
            chunk: UVec3::new(16, 16, 16),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(16, 1, 1, true);
        let left = greedy_mesh_chunk(&g, UVec3::new(0, 0, 0));
        let right = greedy_mesh_chunk(&g, UVec3::new(1, 0, 0));
        // Left chunk should own exactly the boundary face (2 triangles = 6 indices)
        assert_eq!(left.indices.len(), 6);
        // Right chunk should own the remaining 5 faces (10 triangles = 30 indices)
        assert_eq!(right.indices.len(), 30);
    }

    #[test]
    fn normals_match_triangle_winding() {
        // Single 1x1x1 solid at origin
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: UVec3::new(1, 1, 1),
            chunk: UVec3::new(1, 1, 1),
            material: find_material_id("stone").unwrap(),
        };
        let mut g = voxel_proxy::VoxelGrid::new(meta);
        g.set(0, 0, 0, true);
        let m = greedy_mesh_all(&g);
        assert_eq!(m.indices.len(), 36);
        // For each triangle, the geometric normal should align with the stored vertex normal
        for tri in m.indices.chunks_exact(3) {
            let p0 = glam::Vec3::from(m.positions[tri[0] as usize]);
            let p1 = glam::Vec3::from(m.positions[tri[1] as usize]);
            let p2 = glam::Vec3::from(m.positions[tri[2] as usize]);
            let face_n = (p1 - p0).cross(p2 - p0).normalize();
            let n0 = glam::Vec3::from(m.normals[tri[0] as usize]).normalize();
            let n1 = glam::Vec3::from(m.normals[tri[1] as usize]).normalize();
            let n2 = glam::Vec3::from(m.normals[tri[2] as usize]).normalize();
            assert!(
                face_n.dot(n0) > 0.5 && face_n.dot(n1) > 0.5 && face_n.dot(n2) > 0.5,
                "triangle normal not aligned with vertex normals: face_n={:?} n0={:?} n1={:?} n2={:?}",
                face_n,
                n0,
                n1,
                n2
            );
        }
    }
}
