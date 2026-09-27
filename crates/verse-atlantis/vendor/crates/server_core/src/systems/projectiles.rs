//! Authoritative projectile integration and simple collision helpers.

use ecs_core::components::{
    CarveRequest, CollisionShape, EntityId, Health, InputCommand, Projectile,
};
use glam::Vec3;
// telemetry hooks reserved; macros elided in CI builds
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Integrate projectiles forward by `dt` seconds; returns list of (index, p0, p1).
pub fn integrate(projectiles: &mut [Projectile], dt: f32) -> Vec<(usize, Vec3, Vec3)> {
    let t0 = Instant::now();
    let mut segs = Vec::with_capacity(projectiles.len());
    for (i, p) in projectiles.iter_mut().enumerate() {
        if p.life_s <= 0.0 {
            continue;
        }
        let p0 = p.pos;
        p.pos += p.vel * dt;
        p.life_s -= dt;
        let p1 = p.pos;
        segs.push((i, p0, p1));
    }
    let _ = t0;
    segs
}

#[inline]
fn segment_sphere_hit(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let d = p1 - p0;
    let m = p0 - center;
    let a = d.dot(d);
    if a <= 1e-6 {
        return m.length() <= radius;
    }
    let t = (-(m.dot(d)) / a).clamp(0.0, 1.0);
    let c = p0 + d * t;
    (c - center).length() <= radius
}

#[inline]
fn segment_aabb_enter_t(p0: Vec3, p1: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let d = p1 - p0;
    let mut tmin = 0.0f32;
    let mut tmax = 1.0f32;
    for i in 0..3 {
        let s = p0[i];
        let dir = d[i];
        let minb = min[i];
        let maxb = max[i];
        if dir.abs() < 1e-6 {
            if s < minb || s > maxb {
                return None;
            }
        } else {
            let inv = 1.0 / dir;
            let mut t0 = (minb - s) * inv;
            let mut t1 = (maxb - s) * inv;
            if t0 > t1 {
                core::mem::swap(&mut t0, &mut t1);
            }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

/// Resolve collisions for segments against a set of shapes; apply damage and emit carve requests.
pub fn collide_and_damage(
    segs: &[(usize, Vec3, Vec3)],
    projectiles: &mut [Projectile],
    targets: &mut [(EntityId, CollisionShape, Health)],
    destructs: &[(ecs_core::components::DestructibleId, Vec3, Vec3)],
    out_carves: &mut Vec<CarveRequest>,
) {
    let t0 = Instant::now();
    let mut _hits_total: u64 = 0;
    let mut _hits_npc: u64 = 0;
    let _hits_player: u64 = 0; // reserved for future
    let mut _hits_destruct: u64 = 0;
    for (pi, p0, p1) in segs.iter().copied() {
        let pr = projectiles[pi];
        // Entities first
        for (_eid, shape, h) in targets.iter_mut() {
            match *shape {
                CollisionShape::Sphere { center, radius } => {
                    if segment_sphere_hit(p0, p1, center, radius) {
                        h.hp = (h.hp - pr.damage).max(0);
                        _hits_total += 1;
                        _hits_npc += 1;
                    }
                }
                CollisionShape::CapsuleY { .. } => {
                    // Not implemented in v0
                }
                CollisionShape::Aabb { .. } => {}
            }
        }
        // Destructibles: segment vs AABB, emit CarveRequest on entry
        for (did, min, max) in destructs {
            if let Some(t) = segment_aabb_enter_t(p0, p1, *min, *max) {
                let hit = p0 + (p1 - p0) * t.max(0.0);
                out_carves.push(CarveRequest {
                    did: did.0,
                    center_m: hit.as_dvec3(),
                    radius_m: pr.radius_m as f64,
                    seed: 0,
                    impact_id: 0,
                });
                _hits_total += 1;
                _hits_destruct += 1;
            }
        }
    }
    // Coarse batch counter to avoid label churn in CI/dev builds.
    let _ = t0;
}

/// Map an `InputCommand` to a projectile spec name used by the db.
fn action_name(cmd: &InputCommand) -> Option<&'static str> {
    match cmd {
        InputCommand::AtWillLMB => Some("AtWillLMB"),
        InputCommand::AtWillRMB => Some("AtWillRMB"),
        InputCommand::EncounterQ => Some("EncounterQ"),
        InputCommand::EncounterE => Some("EncounterE"),
        InputCommand::EncounterR => Some("EncounterR"),
        _ => None,
    }
}

/// Spawn a projectile from a player's transform and look direction given an input command.
pub fn spawn_from_command(
    cmd: &InputCommand,
    owner: EntityId,
    origin: Vec3,
    look_dir: Vec3,
    db: &data_runtime::specs::projectiles::ProjectileSpecDb,
) -> Option<Projectile> {
    let name = action_name(cmd)?;
    let spec = db.actions.get(name)?;
    let dir = look_dir.normalize_or_zero();
    if dir.length_squared() <= 1e-6 {
        return None;
    }
    Some(Projectile {
        radius_m: spec.radius_m,
        damage: spec.damage,
        life_s: spec.life_s,
        owner,
        pos: origin,
        vel: dir * spec.speed_mps,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }
    #[test]
    fn sphere_hit_applies_damage() {
        let owner = EntityId(1);
        let mut projectiles = vec![Projectile {
            radius_m: 0.2,
            damage: 10,
            life_s: 1.0,
            owner,
            pos: Vec3::new(-2.0, 0.0, 0.0),
            vel: Vec3::new(4.0, 0.0, 0.0),
        }];
        let segs = integrate(&mut projectiles, 0.5);
        let mut targets = vec![(
            EntityId(2),
            CollisionShape::Sphere {
                center: Vec3::ZERO,
                radius: 0.5,
            },
            Health { hp: 30, max: 30 },
        )];
        let mut carves = Vec::new();
        collide_and_damage(&segs, &mut projectiles, &mut targets, &[], &mut carves);
        assert_eq!(targets[0].2.hp, 20);
        assert!(carves.is_empty());
    }
    #[test]
    fn destructible_hit_emits_carve() {
        let owner = EntityId(1);
        let mut projectiles = vec![Projectile {
            radius_m: 0.6,
            damage: 1,
            life_s: 1.0,
            owner,
            pos: Vec3::new(-2.0, 0.0, 0.0),
            vel: Vec3::new(4.0, 0.0, 0.0),
        }];
        let segs = integrate(&mut projectiles, 0.25);
        let mut targets: Vec<(EntityId, CollisionShape, Health)> = Vec::new();
        let min = Vec3::new(-1.0, -1.0, -1.0);
        let max = Vec3::new(1.0, 1.0, 1.0);
        let did = ecs_core::components::DestructibleId(1);
        let mut carves = Vec::new();
        collide_and_damage(
            &segs,
            &mut projectiles,
            &mut targets,
            &[(did, min, max)],
            &mut carves,
        );
        assert_eq!(carves.len(), 1);
        assert!((carves[0].radius_m - 0.6).abs() < 1e-6);
        assert_eq!(carves[0].did, 1);
    }
    #[test]
    fn spawn_from_input_command_uses_spec() {
        let db = data_runtime::specs::projectiles::ProjectileSpecDb::load_default().unwrap();
        let owner = EntityId(7);
        let origin = Vec3::new(1.0, 2.0, 3.0);
        let dir = Vec3::Z;
        let p = spawn_from_command(
            &ecs_core::components::InputCommand::AtWillLMB,
            owner,
            origin,
            dir,
            &db,
        )
        .expect("spawn");
        assert!(p.vel.length() > 0.0 && (p.vel.normalize() - dir).length() < 1e-4);
        assert!(p.life_s > 0.0 && p.radius_m > 0.0);
    }
    #[test]
    fn integrate_is_deterministic_over_steps() {
        let owner = EntityId(9);
        let p = Projectile {
            radius_m: 0.1,
            damage: 1,
            life_s: 10.0,
            owner,
            pos: Vec3::new(0.0, 0.0, 0.0),
            vel: Vec3::new(4.0, 0.0, 0.0),
        };
        // Single step
        let mut a = vec![p];
        let _ = integrate(&mut a, 0.5);
        let single = a[0].pos;
        // Two half-steps
        let mut b = vec![p];
        let _ = integrate(&mut b, 0.25);
        let _ = integrate(&mut b, 0.25);
        let two = b[0].pos;
        assert!(approx(single.x, two.x) && approx(single.y, two.y) && approx(single.z, two.z));
    }
}
