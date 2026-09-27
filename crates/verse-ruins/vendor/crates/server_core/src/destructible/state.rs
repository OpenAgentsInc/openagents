//! Destructible registry: multi-proxy voxel state owned by the server ECS.
//!
//! Scope
//! - Holds per-proxy voxel grids, dirty chunk sets, mesh caches, and colliders.
//! - Collects `CarveRequest` from gameplay systems and applies them during tick.
//! - Produces compact `ChunkMeshDelta` records for replication.

use ecs_core::components::{ChunkDirty, ChunkMesh};
use glam::{Mat4, Vec3};
use net_core::snapshot::ChunkMeshDelta;
use std::collections::HashMap;
use voxel_proxy::VoxelGrid;

use super::config::DestructibleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DestructibleId(pub u64);

/// Simple axis-aligned world AABB for broad-phase tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldAabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// Per-instance proxy state. Heavy voxel data lives here; ECS entities hold only lightweight refs.
pub struct DestructibleProxy {
    pub did: DestructibleId,
    pub grid: VoxelGrid,
    pub dirty: ChunkDirty,
    pub meshes: ChunkMesh,
    pub colliders: Vec<collision_static::chunks::StaticChunk>,
    pub static_index: Option<collision_static::StaticIndex>,
    pub world_from_object: Mat4,
    pub object_from_world: Mat4,
    pub world_aabb: WorldAabb,
}

impl DestructibleProxy {
    pub fn new(did: DestructibleId, grid: VoxelGrid, world_aabb: WorldAabb) -> Self {
        // Object-space coordinates for meshes are in the grid's local space with
        // origin at `meta.origin_m`. Build transforms so we can emit world-space
        // positions in replication consistently with the announced AABB.
        let o = grid.origin_m();
        let trans = glam::Vec3::new(o.x as f32, o.y as f32, o.z as f32);
        let world_from_object = Mat4::from_translation(trans);
        let object_from_world = world_from_object.inverse();
        let mut proxy = Self {
            did,
            grid,
            dirty: ChunkDirty::default(),
            meshes: ChunkMesh::default(),
            colliders: Vec::new(),
            static_index: None,
            world_from_object,
            object_from_world,
            world_aabb,
        };
        // Mark all chunks dirty on creation so we produce initial meshes/deltas
        let dims = proxy.grid.dims();
        let cs = proxy.grid.meta().chunk;
        let nx = dims.x.div_ceil(cs.x);
        let ny = dims.y.div_ceil(cs.y);
        let nz = dims.z.div_ceil(cs.z);
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    proxy.dirty.0.push(glam::UVec3::new(x, y, z));
                }
            }
        }
        proxy
    }
}

/// Server-owned registry for destructible proxies and per-tick mesh deltas.
#[derive(Default)]
pub struct DestructibleRegistry {
    pub proxies: HashMap<DestructibleId, DestructibleProxy>,
    pub pending_mesh_deltas: Vec<ChunkMeshDelta>,
    pub cfg: DestructibleConfig,
    /// Chunks touched (meshed or removed) this tick; used to prioritize collider refresh.
    pub touched_this_tick: Vec<(DestructibleId, glam::UVec3)>,
}

impl std::fmt::Debug for DestructibleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DestructibleRegistry")
            .field("proxies", &self.proxies.len())
            .field("pending_mesh_deltas", &self.pending_mesh_deltas.len())
            .finish()
    }
}

impl DestructibleRegistry {
    /// Register a proxy into the registry.
    pub fn insert_proxy(&mut self, proxy: DestructibleProxy) {
        self.proxies.insert(proxy.did, proxy);
    }
}
