//! Server scene build helpers for destructibles.
//!
//! For v0, we provide a utility to transform a local-space AABB by a model
//! matrix and compute a tight world-space AABB (8-corner transform).

pub fn world_aabb_from_local(
    local_min: [f32; 3],
    local_max: [f32; 3],
    model: [[f32; 4]; 4],
) -> ([f32; 3], [f32; 3]) {
    let m = glam::Mat4::from_cols_array_2d(&model);
    let lm = glam::Vec3::from(local_min);
    let lmax = glam::Vec3::from(local_max);
    let corners = [
        glam::vec3(lm.x, lm.y, lm.z),
        glam::vec3(lmax.x, lm.y, lm.z),
        glam::vec3(lm.x, lmax.y, lm.z),
        glam::vec3(lmax.x, lmax.y, lm.z),
        glam::vec3(lm.x, lm.y, lmax.z),
        glam::vec3(lmax.x, lm.y, lmax.z),
        glam::vec3(lm.x, lmax.y, lmax.z),
        glam::vec3(lmax.x, lmax.y, lmax.z),
    ];
    let mut wmin = glam::Vec3::splat(f32::INFINITY);
    let mut wmax = glam::Vec3::splat(f32::NEG_INFINITY);
    for c in &corners {
        let wc = m.transform_point3(*c);
        wmin = wmin.min(wc);
        wmax = wmax.max(wc);
    }
    ([wmin.x, wmin.y, wmin.z], [wmax.x, wmax.y, wmax.z])
}

/// World-space AABB for a destructible instance (server-side view).
#[derive(Debug, Clone, PartialEq)]
pub struct DestructibleWorldAabb {
    pub did: u64,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
}

/// Build destructible instance world AABBs from scene declarations.
pub fn build_destructible_instances(
    decls: &[data_runtime::scene::destructibles::DestructibleDecl],
) -> Vec<DestructibleWorldAabb> {
    let mut out = Vec::with_capacity(decls.len());
    for (i, d) in decls.iter().enumerate() {
        let t = &d.transform;
        let yaw = glam::Quat::from_rotation_y(t.yaw_deg.to_radians());
        let model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::from(t.scale),
            yaw,
            glam::Vec3::from(t.translation),
        );
        let (wmin, wmax) =
            world_aabb_from_local(d.local_min, d.local_max, model.to_cols_array_2d());
        out.push(DestructibleWorldAabb {
            did: i as u64,
            world_min: wmin,
            world_max: wmax,
        });
    }
    out
}

/// Demo helper: register a simple voxel proxy for the wizard ruins, so impacts carve.
/// This synthesizes a box-like volume; replace with real baked proxy when available.
pub fn add_demo_ruins_destructible(srv: &mut crate::ServerState) {
    use crate::destructible::state::{DestructibleId, DestructibleProxy, WorldAabb};
    use core_units::Length;
    use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};

    // Place a single ruins just in front of the spawn so the player does not
    // start inside it. Shift +Z by ~12m; keep on ground (y=0).
    let shift_z = 12.0f32;
    let world_min = glam::vec3(-8.0, 0.0, -8.0 + shift_z);
    let world_max = glam::vec3(8.0, 6.0, 8.0 + shift_z);

    // Build a small grid and fill a thick shell
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: glam::DVec3::new(world_min.x as f64, world_min.y as f64, world_min.z as f64),
        voxel_m: Length::meters(0.5),
        dims: glam::UVec3::new(32, 16, 32),
        chunk: glam::UVec3::new(8, 8, 8),
        material: core_materials::find_material_id("stone")
            .unwrap_or(core_materials::MaterialId(0)),
    };
    let mut grid = VoxelGrid::new(meta);
    let d = grid.dims();
    for z in 0..d.z {
        for y in 0..d.y {
            for x in 0..d.x {
                // Build vertical side walls only; omit the bottom floor to avoid
                // z-fighting with the terrain plane.
                let bottom = false;
                let side_walls = x < 2 || z < 2 || x > d.x - 3 || z > d.z - 3;
                if bottom || side_walls {
                    grid.set(x, y, z, true);
                }
            }
        }
    }
    let did = DestructibleId(1);
    let aabb = WorldAabb {
        min: world_min,
        max: world_max,
    };
    let proxy = DestructibleProxy::new(did, grid, aabb);
    srv.destruct_registry.insert_proxy(proxy);
    srv.destruct_instances.push(DestructibleWorldAabb {
        did: did.0,
        world_min: [world_min.x, world_min.y, world_min.z],
        world_max: [world_max.x, world_max.y, world_max.z],
    });
    srv.destruct_bootstrap_instances_outstanding = true;
}

/// Add a second demo ruins instance centered at `center`. The AABB matches the
/// default demo ruins size: 16m (X/Z) wide and 6m tall. `did` must be unique.
pub fn add_demo_ruins_destructible_at(
    srv: &mut crate::ServerState,
    center: glam::Vec3,
    did_val: u64,
) {
    use crate::destructible::state::{DestructibleId, DestructibleProxy, WorldAabb};
    use core_units::Length;
    use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};

    let half = glam::vec3(8.0, 3.0, 8.0);
    let world_min = glam::vec3(center.x - half.x, 0.0, center.z - half.z);
    let world_max = glam::vec3(center.x + half.x, 6.0, center.z + half.z);

    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: glam::DVec3::new(world_min.x as f64, world_min.y as f64, world_min.z as f64),
        voxel_m: Length::meters(0.5),
        dims: glam::UVec3::new(32, 16, 32),
        chunk: glam::UVec3::new(8, 8, 8),
        material: core_materials::find_material_id("stone")
            .unwrap_or(core_materials::MaterialId(0)),
    };
    let mut grid = VoxelGrid::new(meta);
    let d = grid.dims();
    for z in 0..d.z {
        for y in 0..d.y {
            for x in 0..d.x {
                let bottom = false;
                let side_walls = x < 2 || z < 2 || x > d.x - 3 || z > d.z - 3;
                if bottom || side_walls {
                    grid.set(x, y, z, true);
                }
            }
        }
    }
    let did = DestructibleId(did_val);
    let aabb = WorldAabb {
        min: world_min,
        max: world_max,
    };
    let proxy = DestructibleProxy::new(did, grid, aabb);
    srv.destruct_registry.insert_proxy(proxy);
    srv.destruct_instances.push(DestructibleWorldAabb {
        did: did.0,
        world_min: [world_min.x, world_min.y, world_min.z],
        world_max: [world_max.x, world_max.y, world_max.z],
    });
    srv.destruct_bootstrap_instances_outstanding = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aabb_transform_matches_expected() {
        let local_min = [-1.0, -2.0, -3.0];
        let local_max = [2.0, 3.0, 4.0];
        let t = glam::Mat4::from_translation(glam::vec3(10.0, 0.5, -2.0))
            * glam::Mat4::from_scale(glam::vec3(2.0, 1.0, 0.5));
        let (wmin, wmax) = world_aabb_from_local(local_min, local_max, t.to_cols_array_2d());
        // Rough bounds check
        assert!(wmin[0] < wmax[0] && wmin[1] < wmax[1] && wmin[2] < wmax[2]);
        // Spot check one corner
        let c = glam::vec3(local_max[0], local_min[1], local_max[2]);
        let wc = t.transform_point3(c);
        assert!(wc.x <= wmax[0] + 1e-4 && wc.y >= wmin[1] - 1e-4);
    }
    #[test]
    fn build_instances_produces_world_aabbs() {
        let decl = data_runtime::scene::destructibles::DestructibleDecl {
            mesh_id: 0,
            local_min: [-1.0, -1.0, -1.0],
            local_max: [1.0, 1.0, 1.0],
            transform: data_runtime::scene::destructibles::TransformDecl {
                translation: [5.0, 0.0, -2.0],
                yaw_deg: 45.0,
                scale: [2.0, 1.0, 1.0],
            },
        };
        let inst = build_destructible_instances(&[decl]);
        assert_eq!(inst.len(), 1);
        let a = &inst[0];
        // Bounds expand and translate reasonably
        assert!(a.world_max[0] > a.world_min[0]);
        assert!(a.world_max[2] > a.world_min[2]);
        // Center roughly near translation in XZ plane
        let cx = 0.5 * (a.world_min[0] + a.world_max[0]);
        let cz = 0.5 * (a.world_min[2] + a.world_max[2]);
        assert!((cx - 5.0).abs() < 3.0);
        assert!((cz + 2.0).abs() < 3.0);
    }
}
