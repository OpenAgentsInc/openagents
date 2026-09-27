#![allow(clippy::unwrap_used)]

use glam::vec3;
use server_core as sc;

#[test]
fn dk_spawn_not_on_top_of_zombie() {
    let mut s = sc::ServerState::new();
    let z = s.spawn_undead(vec3(5.0, 0.6, 0.0), 0.9, 10);
    let dk = s.spawn_death_knight(vec3(5.0, 0.6, 0.0));

    let zpos = s.ecs.get(z).unwrap().tr.pos;
    let dkpos = s.ecs.get(dk).unwrap().tr.pos;
    let min_dist = 0.9 + 1.0 + 0.1; // zombie radius + dk radius + pad
    assert!(
        dkpos.distance(zpos) >= min_dist,
        "spawns must be pushed apart (dk={}, z={})",
        dkpos,
        zpos
    );
}
