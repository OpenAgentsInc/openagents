//! NPC ECS systems: perception, AI movement, collisions, melee, death.
//!
//! These helpers operate over simple slices so they are easy to test and do
//! not require a full ECS runtime.

use ecs_core::components::{Health, Npc};
use glam::Vec3;

/// For each NPC, pick the nearest wizard position (XZ) and move toward it.
pub fn npc_ai_seek(npc: &mut [Npc], npc_pos: &mut [Vec3], wizards: &[Vec3], dt: f32) {
    if wizards.is_empty() {
        return;
    }
    for (i, n) in npc.iter_mut().enumerate() {
        let p = npc_pos[i];
        // nearest wizard in XZ
        let mut best_d2 = f32::INFINITY;
        let mut best = wizards[0];
        for &w in wizards {
            let dx = w.x - p.x;
            let dz = w.z - p.z;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best = w;
            }
        }
        let to = Vec3::new(best.x - p.x, 0.0, best.z - p.z);
        let contact = n.radius + 0.7 + 0.35; // npc + wizard radius + pad
        let dist = to.length();
        if dist > contact + 0.02 {
            let step = (n.speed_mps * dt).min(dist - contact);
            if step > 1e-4 {
                npc_pos[i] = p + to.normalize() * step;
            }
        }
        n.attack_cooldown_s = (n.attack_cooldown_s - dt).max(0.0);
    }
}

/// Simple push-back to resolve inter-NPC and NPC-wizard overlaps on XZ plane.
pub fn resolve_collisions(npc: &[Npc], npc_pos: &mut [Vec3], wizards: &[Vec3]) {
    // npc-npc
    for i in 0..npc_pos.len() {
        for j in (i + 1)..npc_pos.len() {
            let (pi, pj) = (npc_pos[i], npc_pos[j]);
            let mut dx = pj.x - pi.x;
            let mut dz = pj.z - pi.z;
            let d2 = dx * dx + dz * dz;
            let min_d = npc[i].radius + npc[j].radius;
            if d2 < min_d * min_d {
                let d = d2.sqrt().max(1e-4);
                dx /= d;
                dz /= d;
                let overlap = min_d - d;
                npc_pos[i].x -= dx * overlap * 0.5;
                npc_pos[i].z -= dz * overlap * 0.5;
                npc_pos[j].x += dx * overlap * 0.5;
                npc_pos[j].z += dz * overlap * 0.5;
            }
        }
    }
    // npc-wizard
    let wiz_r = 0.7f32;
    for i in 0..npc_pos.len() {
        for &w in wizards {
            let mut dx = npc_pos[i].x - w.x;
            let mut dz = npc_pos[i].z - w.z;
            let d2 = dx * dx + dz * dz;
            let min_d = npc[i].radius + wiz_r;
            if d2 < min_d * min_d {
                let d = d2.sqrt().max(1e-4);
                dx /= d;
                dz /= d;
                let overlap = min_d - d;
                npc_pos[i].x += dx * overlap;
                npc_pos[i].z += dz * overlap;
            }
        }
    }
}

/// Apply melee damage when within reach; returns (wizard_index, damage) for hits.
pub fn melee_apply(
    npc: &mut [Npc],
    npc_pos: &[Vec3],
    wizards: &mut [Health],
    wizard_pos: &[Vec3],
    dt: f32,
) -> Vec<(usize, i32)> {
    let wiz_r = 0.7f32;
    let pad = 0.35f32;
    let attack_cd = 1.5f32;
    let mut hits = Vec::new();
    for i in 0..npc.len() {
        let n = npc[i];
        let p = npc_pos[i];
        // choose nearest wizard again for simplicity
        let mut best = 0usize;
        let mut best_d2 = f32::INFINITY;
        for (widx, w) in wizard_pos.iter().enumerate() {
            let dx = w.x - p.x;
            let dz = w.z - p.z;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best = widx;
            }
        }
        let to = Vec3::new(wizard_pos[best].x - p.x, 0.0, wizard_pos[best].z - p.z);
        let contact = n.radius + wiz_r + pad;
        let dist = to.length();
        if dist <= contact + 0.02 && npc[i].attack_cooldown_s <= 0.0 {
            let before = wizards[best].hp;
            let after = (before - n.damage).max(0);
            wizards[best].hp = after;
            hits.push((best, n.damage));
            // reset cooldown
            npc[i].attack_cooldown_s = attack_cd;
        }
    }
    // cooldown decay
    for n in npc.iter_mut() {
        n.attack_cooldown_s = (n.attack_cooldown_s - dt).max(0.0);
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seeks_and_hits() {
        let mut npc = vec![Npc::default()];
        let mut pos = vec![Vec3::new(-2.0, 0.0, 0.0)];
        let wpos = vec![Vec3::ZERO];
        npc_ai_seek(&mut npc, &mut pos, &wpos, 0.5);
        // should have moved toward origin on +X axis
        assert!(pos[0].x > -2.0);
        let mut wh = vec![Health { hp: 30, max: 30 }];
        // place NPC next to wizard and swing
        pos[0] = Vec3::new(1.0, 0.0, 0.0);
        let hits = melee_apply(&mut npc, &pos, &mut wh, &wpos, 0.1);
        assert_eq!(hits.len(), 1);
        assert!(wh[0].hp < 30);
    }
}
