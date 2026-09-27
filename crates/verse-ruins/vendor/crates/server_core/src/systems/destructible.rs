//! Destructible systems: core voxel carve/mesh/collider helpers and ECS glue.

use crate::ServerState;
use crate::destructible::{config::DestructibleConfig, queue::ChunkQueue};
use crate::ecs::schedule::Ctx;
use ecs_core::components::{CarveRequest, ChunkDirty, ChunkMesh, MeshCpu};
use glam::UVec3;
use net_core::snapshot::ChunkMeshDelta;
use voxel_proxy::VoxelGrid;

/// Apply a carve request to the grid and enqueue dirty chunks.
pub fn voxel_carve(
    grid: &mut VoxelGrid,
    req: &CarveRequest,
    cfg: &DestructibleConfig,
    dirty: &mut ChunkDirty,
) -> usize {
    let _out = crate::destructible::carve_and_spawn_debris(
        grid,
        req.center_m,
        core_units::Length::meters(req.radius_m),
        cfg.seed ^ (req.seed),
        req.impact_id as u64,
        cfg.max_debris,
    );
    let enq = grid.pop_dirty_chunks(usize::MAX);
    let n = enq.len();
    dirty.0.extend(enq);
    n
}

/// Greedy-mesh up to `budget` chunks from `dirty` into `out_mesh`.
pub fn greedy_mesh_budget(
    grid: &VoxelGrid,
    dirty: &mut ChunkDirty,
    out_mesh: &mut ChunkMesh,
    budget: usize,
) -> usize {
    if budget == 0 || dirty.0.is_empty() {
        return 0;
    }
    let mut q = ChunkQueue::new();
    q.enqueue_many(std::mem::take(&mut dirty.0));
    let chunks = q.pop_budget(budget);
    // put the remainder back
    dirty.0.extend(q.pop_budget(usize::MAX));
    let mut processed = 0usize;
    for c in chunks {
        let mb = voxel_mesh::greedy_mesh_chunk(grid, c);
        if mb.indices.is_empty() {
            out_mesh.map.remove(&(c.x, c.y, c.z));
            continue;
        }
        let mc = MeshCpu {
            positions: mb.positions.clone(),
            normals: mb.normals.clone(),
            indices: mb.indices.clone(),
        };
        if mc.validate().is_ok() {
            out_mesh.map.insert((c.x, c.y, c.z), mc);
            processed += 1;
        }
    }
    processed
}

/// Rebuild up to `budget` coarse colliders for the provided chunk list and refresh the static index.
pub fn collider_rebuild_budget(
    grid: &VoxelGrid,
    chunks: &[UVec3],
    store: &mut Vec<collision_static::chunks::StaticChunk>,
    static_index: &mut Option<collision_static::StaticIndex>,
    budget: usize,
) -> usize {
    use collision_static::chunks::{build_chunk_collider, rebuild_static_index, swap_in_updates};
    if budget == 0 || chunks.is_empty() {
        return 0;
    }
    let take = budget.min(chunks.len());
    let mut updates = Vec::new();
    for c in chunks.iter().copied().take(take) {
        if let Some(sc) = build_chunk_collider(grid, c) {
            updates.push(sc);
        }
    }
    if !updates.is_empty() {
        swap_in_updates(store, updates);
        *static_index = Some(rebuild_static_index(store));
    }
    take
}

/// Drain per-tick CarveRequests from `ctx` and apply them to registered proxies.
/// Carves for not-yet-registered proxies are retained for the next tick.
pub fn destructible_apply_carves(srv: &mut ServerState, ctx: &mut Ctx) {
    if ctx.carves.is_empty() {
        return;
    }
    let mut keep: Vec<CarveRequest> = Vec::new();
    for mut req in ctx.carves.drain(..) {
        let did = crate::destructible::state::DestructibleId(req.did);
        if let Some(proxy) = srv.destruct_registry.proxies.get_mut(&did) {
            // Convert world-space center to object-space if a transform is present,
            // and scale carve radius defensively by object_from_world scale (average if non-uniform).
            let ws = glam::Vec3::new(
                req.center_m.x as f32,
                req.center_m.y as f32,
                req.center_m.z as f32,
            );
            let os = (proxy.object_from_world * ws.extend(1.0)).truncate();
            let sx = proxy.object_from_world.x_axis.truncate().length();
            let sy = proxy.object_from_world.y_axis.truncate().length();
            let sz = proxy.object_from_world.z_axis.truncate().length();
            let avg = (sx + sy + sz) / 3.0;
            if (sx - sy).abs() > 1e-2 || (sx - sz).abs() > 1e-2 {
                log::warn!(
                    "destructible: non-uniform scale detected (sx={:.3},sy={:.3},sz={:.3}); using avg={:.3} for carve radius",
                    sx,
                    sy,
                    sz,
                    avg
                );
            }
            req.center_m = os.as_dvec3();
            req.radius_m *= avg as f64;
            let _ = voxel_carve(
                &mut proxy.grid,
                &req,
                &srv.destruct_registry.cfg,
                &mut proxy.dirty,
            );
            metrics::counter!("destruct.carves_applied_total").increment(1);
        } else {
            keep.push(req);
        }
    }
    ctx.carves = keep;
}

/// Mesh a budgeted number of dirty chunks per proxy, updating CPU mesh maps and
/// enqueueing `ChunkMeshDelta` for replication. Deterministic order across runs.
pub fn destructible_remesh_budgeted(srv: &mut ServerState) {
    let budget = srv.destruct_registry.cfg.max_chunk_remesh.max(0);
    if budget == 0 {
        return;
    }
    for (did, proxy) in srv.destruct_registry.proxies.iter_mut() {
        if proxy.dirty.0.is_empty() {
            continue;
        }
        // Deterministic worklist
        let mut q = ChunkQueue::new();
        q.enqueue_many(std::mem::take(&mut proxy.dirty.0));
        let chunks = q.pop_budget(budget);
        // Put unprocessed back
        proxy.dirty.0.extend(q.pop_budget(usize::MAX));
        for c in chunks {
            let mb = voxel_mesh::greedy_mesh_chunk(&proxy.grid, c);
            let key = (c.x, c.y, c.z);
            if mb.indices.is_empty() {
                proxy.meshes.map.remove(&key);
                srv.destruct_registry
                    .pending_mesh_deltas
                    .push(ChunkMeshDelta {
                        did: did.0,
                        chunk: key,
                        positions: Vec::new(),
                        normals: Vec::new(),
                        indices: Vec::new(),
                    });
                srv.destruct_registry.touched_this_tick.push((*did, c));
                continue;
            }
            let mc = MeshCpu {
                positions: mb.positions.clone(),
                normals: mb.normals.clone(),
                indices: mb.indices.clone(),
            };
            if mc.validate().is_ok() {
                proxy.meshes.map.insert(key, mc.clone());
                // Clamp overly large meshes for safety
                let too_large = mc.positions.len() > 200_000 || mc.indices.len() > 600_000;
                if too_large {
                    log::warn!(
                        "destructible: skipping oversize chunk mesh (pos={}, idx={})",
                        mc.positions.len(),
                        mc.indices.len()
                    );
                } else {
                    // Transform object-space positions (and normals) to world-space for clients.
                    // This avoids requiring a per-DID model matrix on the renderer side.
                    let xf = proxy.world_from_object;
                    let nxf = xf.inverse().transpose();
                    let mut wpos = Vec::with_capacity(mc.positions.len());
                    let mut wnorm = Vec::with_capacity(mc.normals.len());
                    for (i, p) in mc.positions.iter().enumerate() {
                        let wp = (xf * glam::Vec4::new(p[0], p[1], p[2], 1.0)).truncate();
                        wpos.push([wp.x, wp.y, wp.z]);
                        if let Some(n) = mc.normals.get(i) {
                            let wn = (nxf * glam::Vec4::new(n[0], n[1], n[2], 0.0))
                                .truncate()
                                .normalize_or_zero();
                            wnorm.push([wn.x, wn.y, wn.z]);
                        }
                    }
                    // Ensure normals vector matches positions length for decode validators
                    if wnorm.len() < wpos.len() {
                        wnorm.resize(wpos.len(), [0.0, 1.0, 0.0]);
                    }
                    srv.destruct_registry
                        .pending_mesh_deltas
                        .push(ChunkMeshDelta {
                            did: did.0,
                            chunk: key,
                            positions: wpos,
                            normals: wnorm,
                            indices: mc.indices,
                        });
                }
                srv.destruct_registry.touched_this_tick.push((*did, c));
            }
        }
    }
}

/// Refresh colliders for a budgeted number of chunks per proxy.
pub fn destructible_refresh_colliders(srv: &mut ServerState) {
    let mut budget = srv.destruct_registry.cfg.collider_budget_per_tick.max(0);
    if budget == 0 {
        return;
    }
    let mut rest: Vec<(crate::destructible::state::DestructibleId, UVec3)> = Vec::new();
    for (did, c) in srv.destruct_registry.touched_this_tick.drain(..) {
        if budget == 0 {
            rest.push((did, c));
            continue;
        }
        if let Some(proxy) = srv.destruct_registry.proxies.get_mut(&did) {
            let _ = collider_rebuild_budget(
                &proxy.grid,
                std::slice::from_ref(&c),
                &mut proxy.colliders,
                &mut proxy.static_index,
                1,
            );
            budget = budget.saturating_sub(1);
        }
    }
    srv.destruct_registry.touched_this_tick = rest;
}
