#![allow(clippy::unwrap_used)]

use server_core as sc;

#[test]
fn wizard_spawn_is_pushed_out_of_destructible_aabb() {
    let mut s = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut s);

    // Attempt to spawn a wizard at the center of the ruins AABB
    let inst = s.destruct_instances[0].clone();
    let min = glam::Vec3::from(inst.world_min);
    let max = glam::Vec3::from(inst.world_max);
    let center = (min + max) * 0.5;
    let wid = s.spawn_wizard_npc(center);

    let w = s.ecs.get(wid).unwrap();
    // Assert position is outside the AABB on X or Z (by at least 0.5m margin)
    let p = w.tr.pos;
    let outside_x = p.x < min.x - 0.49 || p.x > max.x + 0.49;
    let outside_z = p.z < min.z - 0.49 || p.z > max.z + 0.49;
    assert!(
        outside_x || outside_z,
        "wizard should be pushed out of destructible AABB"
    );
}
