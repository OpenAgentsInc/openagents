//! chunks: helpers to build and manage coarse colliders per voxel chunk.
//!
//! P0 goal is a fast, coarse approximation for debris-vs-world preview: one OBB per
//! dirty chunk AABB. This keeps broadphase simple and avoids per-triangle cost.
//!
//! Extending later: finer collision derived from meshed surfaces.

use crate::{Aabb, OBB, ShapeRef, StaticCollider, StaticIndex};
use glam::{Mat3, UVec3, Vec3};
use voxel_proxy::VoxelGrid;

/// A collider tied to a chunk coordinate.
#[derive(Clone, Debug)]
pub struct StaticChunk {
    pub coord: UVec3,
    pub collider: StaticCollider,
}

/// Compute world-space AABB for a given chunk.
pub fn chunk_world_aabb(grid: &VoxelGrid, coord: UVec3) -> Aabb {
    let (xr, yr, zr) = grid.chunk_bounds_voxels(coord);
    let vox = grid.voxel_m().0 as f32;
    let o = grid.origin_m().as_vec3();
    let min = o + Vec3::new(
        xr.start as f32 * vox,
        yr.start as f32 * vox,
        zr.start as f32 * vox,
    );
    let max = o + Vec3::new(
        xr.end as f32 * vox,
        yr.end as f32 * vox,
        zr.end as f32 * vox,
    );
    Aabb { min, max }
}

/// Build a coarse OBB collider for a chunk if any voxel in the chunk is solid.
pub fn build_chunk_collider(grid: &VoxelGrid, coord: UVec3) -> Option<StaticChunk> {
    let (xr, yr, zr) = grid.chunk_bounds_voxels(coord);
    let mut any = false;
    for z in zr.clone() {
        for y in yr.clone() {
            for x in xr.clone() {
                if grid.is_solid(x, y, z) {
                    any = true;
                    break;
                }
            }
            if any {
                break;
            }
        }
        if any {
            break;
        }
    }
    if !any {
        return None;
    }
    let aabb = chunk_world_aabb(grid, coord);
    let center = (aabb.min + aabb.max) * 0.5;
    let half_extents = (aabb.max - aabb.min) * 0.5;
    let rot3x3 = Mat3::IDENTITY;
    let collider = StaticCollider {
        aabb,
        shape: ShapeRef::Box(OBB {
            center,
            half_extents,
            rot3x3,
        }),
    };
    Some(StaticChunk { coord, collider })
}

/// Replace stored chunk colliders by coord with the provided updates (one per coord).
pub fn swap_in_updates(store: &mut Vec<StaticChunk>, updates: Vec<StaticChunk>) {
    use std::collections::HashMap;
    let mut map: HashMap<(u32, u32, u32), usize> = HashMap::new();
    for (i, c) in store.iter().enumerate() {
        map.insert((c.coord.x, c.coord.y, c.coord.z), i);
    }
    for up in updates {
        let key = (up.coord.x, up.coord.y, up.coord.z);
        if let Some(idx) = map.get(&key).copied() {
            store[idx] = up;
        } else {
            map.insert(key, store.len());
            store.push(up);
        }
    }
}

/// Flatten chunk colliders into a `StaticIndex` for queries.
pub fn rebuild_static_index(store: &[StaticChunk]) -> StaticIndex {
    StaticIndex {
        colliders: store.iter().map(|c| c.collider).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_materials::find_material_id;
    use core_units::Length;
    use glam::{DVec3, UVec3};
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_grid(d: UVec3, c: UVec3) -> VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: DVec3::ZERO,
            voxel_m: Length::meters(1.0),
            dims: d,
            chunk: c,
            material: find_material_id("stone").unwrap(),
        };
        VoxelGrid::new(meta)
    }

    #[test]
    fn empty_chunk_yields_none() {
        let g = mk_grid(UVec3::new(32, 16, 16), UVec3::new(16, 16, 16));
        assert!(build_chunk_collider(&g, UVec3::new(1, 0, 0)).is_none());
    }

    #[test]
    fn solid_voxel_in_chunk_yields_aabb_collider() {
        let mut g = mk_grid(UVec3::new(32, 16, 16), UVec3::new(16, 16, 16));
        // Put a single solid in chunk (1,0,0)
        g.set(17, 0, 0, true);
        let c = build_chunk_collider(&g, UVec3::new(1, 0, 0)).expect("has collider");
        let aabb = c.collider.aabb;
        assert!(aabb.max.x > aabb.min.x);
        assert!(aabb.max.y > aabb.min.y);
        assert!(aabb.max.z > aabb.min.z);
    }

    #[test]
    fn swap_and_rebuild_index() {
        let mut store: Vec<StaticChunk> = Vec::new();
        let aabb0 = Aabb {
            min: Vec3::splat(0.0),
            max: Vec3::splat(1.0),
        };
        let obb = OBB {
            center: Vec3::splat(0.5),
            half_extents: Vec3::splat(0.5),
            rot3x3: Mat3::IDENTITY,
        };
        swap_in_updates(
            &mut store,
            vec![StaticChunk {
                coord: UVec3::new(0, 0, 0),
                collider: StaticCollider {
                    aabb: aabb0,
                    shape: ShapeRef::Box(obb),
                },
            }],
        );
        swap_in_updates(
            &mut store,
            vec![StaticChunk {
                coord: UVec3::new(1, 0, 0),
                collider: StaticCollider {
                    aabb: aabb0,
                    shape: ShapeRef::Box(obb),
                },
            }],
        );
        // Replace first
        swap_in_updates(
            &mut store,
            vec![StaticChunk {
                coord: UVec3::new(0, 0, 0),
                collider: StaticCollider {
                    aabb: Aabb {
                        min: Vec3::splat(1.0),
                        max: Vec3::splat(2.0),
                    },
                    shape: ShapeRef::Box(obb),
                },
            }],
        );
        let idx = rebuild_static_index(&store);
        assert_eq!(idx.colliders.len(), 2);
    }
}
