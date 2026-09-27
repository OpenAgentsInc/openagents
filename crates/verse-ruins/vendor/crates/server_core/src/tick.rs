//! Fixed-order tick orchestration for destructibles: carve -> mesh -> colliders (budgeted).

use crate::destructible::config::DestructibleConfig;
use crate::systems::destructible::{collider_rebuild_budget, greedy_mesh_budget, voxel_carve};
use crate::jobs::JobScheduler;
use ecs_core::components::{CarveRequest, ChunkDirty, ChunkMesh};
use glam::UVec3;
use voxel_proxy::VoxelGrid;

/// Run one destructible tick:
/// - Apply all pending carve requests (enqueue dirty chunks)
/// - Mesh up to `cfg.max_chunk_remesh` chunks
/// - Rebuild colliders for up to `cfg.collider_budget_per_tick` chunks
/// Returns (carves_applied, meshed_count, colliders_count).
pub fn tick_destructibles(
    grid: &mut VoxelGrid,
    cfg: &DestructibleConfig,
    pending_carves: &mut Vec<CarveRequest>,
    dirty: &mut ChunkDirty,
    meshes: &mut ChunkMesh,
    colliders: &mut Vec<collision_static::chunks::StaticChunk>,
    static_index: &mut Option<collision_static::StaticIndex>,
) -> (usize, usize, usize) {
    // 1) Apply all pending carves this tick (small counts expected in v0)
    let mut carves_applied = 0usize;
    if !pending_carves.is_empty() {
        let mut reqs = Vec::new();
        std::mem::swap(&mut reqs, pending_carves);
        for req in reqs {
            let _touched = voxel_carve(grid, &req, cfg, dirty);
            carves_applied += 1;
        }
    }
    // 2) Greedy mesh via scheduler (synchronous dispatch)
    let sched = JobScheduler::new();
    let meshed = sched.dispatch_mesh(cfg.max_chunk_remesh.max(0), |budget| {
        greedy_mesh_budget(grid, dirty, meshes, budget)
    });
    // 3) Collider budget: rebuild over a stable subset of chunk keys we have meshes for
    let mut keys: Vec<UVec3> = meshes
        .map
        .keys()
        .copied()
        .map(|(x, y, z)| UVec3::new(x, y, z))
        .collect();
    // Deterministic order: sort by xyz
    keys.sort_unstable_by_key(|c| (c.x, c.y, c.z));
    let coll = sched.dispatch_collider(cfg.collider_budget_per_tick.max(0), |budget| {
        collider_rebuild_budget(grid, &keys, colliders, static_index, budget)
    });
    (carves_applied, meshed, coll)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use core_units::Length;
    use ecs_core::components::CarveRequest;
    use voxel_proxy::{GlobalId, VoxelProxyMeta};

    fn mk_grid(d: UVec3, c: UVec3, vox_m: f64) -> VoxelGrid {
        let meta = VoxelProxyMeta {
            object_id: GlobalId(1),
            origin_m: glam::DVec3::ZERO,
            voxel_m: Length::meters(vox_m),
            dims: d,
            chunk: c,
            material: core_materials::find_material_id("stone").unwrap(),
        };
        VoxelGrid::new(meta)
    }

    #[test]
    fn orchestrator_runs_and_respects_budgets() {
        let mut grid = mk_grid(UVec3::new(32, 32, 32), UVec3::new(8, 8, 8), 0.25);
        let mut cfg = DestructibleConfig::default();
        cfg.max_chunk_remesh = 2;
        cfg.collider_budget_per_tick = 1;
        let mut pending = vec![CarveRequest { did: 1, center_m: glam::DVec3::new(2.0, 2.0, 2.0), radius_m: 0.6, seed: 7, impact_id: 1 }];
        let mut dirty = ChunkDirty::default();
        let mut meshes = ChunkMesh::default();
        let mut cols: Vec<collision_static::chunks::StaticChunk> = Vec::new();
        let mut idx = None;
        let (carves, meshed, colliders) = tick_destructibles(
            &mut grid,
            &cfg,
            &mut pending,
            &mut dirty,
            &mut meshes,
            &mut cols,
            &mut idx,
        );
        assert_eq!(carves, 1);
        assert!(meshed <= cfg.max_chunk_remesh);
        assert!(colliders <= cfg.collider_budget_per_tick);
        // After one pass there should be some mesh entries and at most one collider chunk
        assert!(!meshes.map.is_empty());
        assert!(cols.len() <= 1);
        assert!(idx.is_some());
    }

    #[test]
    fn orchestrator_multiple_ticks_progresses() {
        let mut grid = mk_grid(UVec3::new(32, 32, 32), UVec3::new(8, 8, 8), 0.25);
        let cfg = DestructibleConfig { max_chunk_remesh: 2, collider_budget_per_tick: 1, ..Default::default() };
        let mut pending = vec![CarveRequest { did: 1, center_m: glam::DVec3::new(4.0, 4.0, 4.0), radius_m: 0.8, seed: 1, impact_id: 1 }];
        let mut dirty = ChunkDirty::default();
        let mut meshes = ChunkMesh::default();
        let mut cols: Vec<collision_static::chunks::StaticChunk> = Vec::new();
        let mut idx = None;
        let _ = tick_destructibles(&mut grid, &cfg, &mut pending, &mut dirty, &mut meshes, &mut cols, &mut idx);
        let _ = tick_destructibles(&mut grid, &cfg, &mut pending, &mut dirty, &mut meshes, &mut cols, &mut idx);
        assert!(!meshes.map.is_empty());
        assert!(idx.is_some());
    }
}
