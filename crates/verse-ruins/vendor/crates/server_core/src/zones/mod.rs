//! Zone boot/apply helpers. Server-authoritative spawn logic lives here.
//!
//! Policy: Platform and renderer must not spawn gameplay. Only zones do.

use crate::ServerState;

/// Boot a server for the given zone slug by applying its initial logic.
/// Returns `true` if any zone-specific content was spawned.
pub fn boot_with_zone(srv: &mut ServerState, slug: &str) -> bool {
    match slug {
        // Demo content zone. Keep spawns deterministic and minimal.
        "wizard_woods" => {
            // Spawn a few NPC rings and the demo destructible ruins as before.
            srv.ring_spawn(8, 15.0, 20);
            srv.ring_spawn(12, 30.0, 25);
            srv.ring_spawn(15, 45.0, 30);
            let wiz_count = 4usize;
            let wiz_r = 8.0f32;
            for i in 0..wiz_count {
                let a = (i as f32) / (wiz_count as f32) * std::f32::consts::TAU;
                let p = glam::vec3(wiz_r * a.cos(), 0.6, wiz_r * a.sin());
                let _ = srv.spawn_wizard_npc(p);
            }
            let _ = srv.spawn_nivita_unique(glam::vec3(0.0, 0.6, 0.0));
            let _dk = srv.spawn_death_knight(glam::vec3(60.0, 0.6, 0.0));
            crate::scene_build::add_demo_ruins_destructible(srv);
            true
        }
        _ => false,
    }
}
