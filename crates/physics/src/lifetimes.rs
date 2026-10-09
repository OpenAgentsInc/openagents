//! Generation-fenced ownership of kinematic bodies and corpse collision.
use crate::{
    body::{Body, BodyKind},
    queries::{ColliderKey, Life, Mesh, MeshCollider, Usage},
};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Hull {
    UprightCapsule { radius: f64, height: f64 },
    Box { half: DVec3 },
}
impl Hull {
    fn validate(self) -> Result<(), String> {
        match self {
            Self::UprightCapsule { radius, height }
                if radius.is_finite()
                    && (0.01..=100.).contains(&radius)
                    && height.is_finite()
                    && height >= radius * 2.
                    && height <= 1000. =>
            {
                Ok(())
            }
            Self::Box { half }
                if half.is_finite()
                    && half.min_element() >= 1e-6
                    && half.max_element() <= 1000. =>
            {
                Ok(())
            }
            _ => Err("Invalid life-bound collision hull".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Phase {
    Alive,
    Corpse { until: f64 },
    Removed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub life: Life,
    pub body: Body,
    pub hull: Hull,
    pub phase: Phase,
    pub actor: bool,
}
impl Record {
    pub fn key(&self) -> ColliderKey {
        ColliderKey {
            life: self.life,
            shape: 0,
        }
    }
    pub fn damage_enabled(&self) -> bool {
        self.actor && self.phase == Phase::Alive
    }
    pub fn selection_enabled(&self) -> bool {
        self.actor && self.phase == Phase::Alive
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bodies {
    pub instance: u64,
    entries: BTreeMap<u64, Record>,
}
fn point(p: DVec3) -> Result<(), String> {
    if !p.is_finite() || p.abs().max_element() > 1_000_000. {
        return Err("Invalid life-bound body position".into());
    }
    Ok(())
}
impl Bodies {
    pub fn new(instance: u64) -> Self {
        Self {
            instance,
            entries: BTreeMap::new(),
        }
    }
    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.entries.values()
    }
    pub fn get(&self, life: Life) -> Option<&Record> {
        self.entries.get(&life.entity).filter(|r| r.life == life)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.entries.len() > 1024 {
            return Err("Life-bound body budget exceeded".into());
        }
        for (entity, r) in &self.entries {
            if *entity == 0
                || *entity != r.life.entity
                || r.life.instance != self.instance
                || r.body.kind != BodyKind::Kinematic
                || r.body.removed != (r.phase == Phase::Removed)
            {
                return Err("Invalid life-bound body ownership".into());
            }
            point(r.body.pos)?;
            point(r.body.prev_pos)?;
            r.hull.validate()?;
            if !r.body.vel.is_finite()
                || r.body.vel.length() > 1000.
                || r.body.orientation != glam::DQuat::IDENTITY
                || r.body.prev_orientation != glam::DQuat::IDENTITY
                || r.body.mass != 1.
                || r.body.inertia != DVec3::ONE
                || r.body.force != DVec3::ZERO
                || r.body.torque != DVec3::ZERO
                || r.body.omega != DVec3::ZERO
            {
                return Err("Invalid kinematic body checkpoint".into());
            }
            if let Phase::Corpse { until } = r.phase {
                if !until.is_finite() || until < 0. || !matches!(r.hull, Hull::Box { .. }) {
                    return Err("Invalid corpse lifetime".into());
                }
            }
        }
        Ok(())
    }
    pub fn spawn(&mut self, life: Life, center: DVec3, hull: Hull) -> Result<(), String> {
        point(center)?;
        hull.validate()?;
        if life.entity == 0
            || life.instance != self.instance
            || self
                .entries
                .get(&life.entity)
                .is_some_and(|r| life.generation <= r.life.generation)
        {
            return Err("Stale or foreign body life".into());
        }
        if !self.entries.contains_key(&life.entity) && self.entries.len() >= 1024 {
            return Err("Life-bound body budget exceeded".into());
        }
        let mut body = Body::new(1., DVec3::ONE, center);
        body.kind = BodyKind::Kinematic;
        self.entries.insert(
            life.entity,
            Record {
                life,
                body,
                hull,
                phase: Phase::Alive,
                actor: true,
            },
        );
        Ok(())
    }
    /// Updates a collision-only prop without granting actor damage or selection.
    pub fn upsert_prop(&mut self, life: Life, min: DVec3, max: DVec3) -> Result<(), String> {
        point(min)?;
        point(max)?;
        let hull = Hull::Box {
            half: (max - min) * 0.5,
        };
        hull.validate()?;
        if self.entries.get(&life.entity).is_some_and(|r| r.actor) {
            return Err("Prop cannot replace an actor body".into());
        }
        if let Some(r) = self.get(life) {
            if r.actor || r.phase != Phase::Alive {
                return Err("Prop cannot replace an actor or removed life".into());
            }
            self.place(life, (min + max) * 0.5, 0.)?;
            self.entries.get_mut(&life.entity).unwrap().hull = hull;
        } else {
            self.spawn(life, (min + max) * 0.5, hull)?;
            self.entries.get_mut(&life.entity).unwrap().actor = false;
        }
        Ok(())
    }
    pub fn place(&mut self, life: Life, center: DVec3, dt: f64) -> Result<(), String> {
        point(center)?;
        if !dt.is_finite() || !(0. ..=0.1).contains(&dt) {
            return Err("Invalid body placement step".into());
        }
        let r = self
            .entries
            .get_mut(&life.entity)
            .filter(|r| r.life == life && r.phase == Phase::Alive)
            .ok_or("Inactive body life")?;
        let velocity = if dt > 0. {
            (center - r.body.pos) / dt
        } else {
            DVec3::ZERO
        };
        if velocity.length() > 1000. {
            return Err("Body placement velocity exceeded".into());
        }
        r.body.prev_pos = r.body.pos;
        r.body.pos = center;
        r.body.vel = velocity;
        Ok(())
    }
    pub fn corpse(&mut self, life: Life, min: DVec3, max: DVec3, until: f64) -> Result<(), String> {
        point(min)?;
        point(max)?;
        let hull = Hull::Box {
            half: (max - min) * 0.5,
        };
        hull.validate()?;
        if !until.is_finite() || until < 0. {
            return Err("Invalid corpse expiry".into());
        }
        let r = self
            .entries
            .get_mut(&life.entity)
            .filter(|r| r.life == life && r.phase == Phase::Alive)
            .ok_or("Inactive body life")?;
        r.hull = hull;
        r.body.prev_pos = r.body.pos;
        r.body.pos = (min + max) * 0.5;
        r.body.vel = DVec3::ZERO;
        r.phase = Phase::Corpse { until };
        Ok(())
    }
    pub fn remove(&mut self, life: Life) -> bool {
        let Some(r) = self
            .entries
            .get_mut(&life.entity)
            .filter(|r| r.life == life && r.phase != Phase::Removed)
        else {
            return false;
        };
        r.phase = Phase::Removed;
        r.body.removed = true;
        r.body.vel = DVec3::ZERO;
        true
    }
    pub fn expire(&mut self, now: f64) -> Result<Vec<Life>, String> {
        if !now.is_finite() || now < 0. {
            return Err("Invalid corpse clock".into());
        }
        let due: Vec<_> = self
            .entries
            .values()
            .filter(|r| matches!(r.phase, Phase::Corpse {until} if now >= until))
            .map(|r| r.life)
            .collect();
        for life in &due {
            self.remove(*life);
        }
        Ok(due)
    }
    /// Retains the generation fence after an actor leaves the owning world.
    pub fn retire_actor(&mut self, life: Life) -> Result<(), String> {
        self.entries
            .get(&life.entity)
            .filter(|r| r.life == life && r.actor)
            .ok_or("Retired actor body is stale or missing")?;
        self.remove(life);
        self.entries.get_mut(&life.entity).unwrap().actor = false;
        Ok(())
    }
    pub fn corpse_colliders(&self) -> Result<Vec<MeshCollider>, String> {
        self.entries
            .values()
            .filter(|r| matches!(r.phase, Phase::Corpse { .. }))
            .map(|r| {
                let Hull::Box { half } = r.hull else {
                    return Err("Corpse hull is not a box".into());
                };
                Ok(MeshCollider {
                    key: crate::walkable::blocker_key(r.life),
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(r.body.pos - half, r.body.pos + half)?,
                })
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn life(generation: u64) -> Life {
        Life {
            instance: 7,
            entity: 2,
            generation,
        }
    }
    fn hull() -> Hull {
        Hull::UprightCapsule {
            radius: 0.35,
            height: 1.8,
        }
    }
    #[test]
    fn actor_retirement_keeps_generation_fences_without_live_collision() {
        let mut bodies = Bodies::new(7);
        bodies.spawn(life(0), DVec3::Y, hull()).unwrap();
        assert!(bodies.retire_actor(life(1)).is_err());
        assert!(bodies.get(life(0)).unwrap().damage_enabled());
        bodies.retire_actor(life(0)).unwrap();
        let record = bodies.get(life(0)).unwrap();
        assert!(!record.actor);
        assert_eq!(record.phase, Phase::Removed);
        assert!(!record.selection_enabled());
        assert!(bodies.spawn(life(0), DVec3::Y, hull()).is_err());
        bodies.validate().unwrap();
    }
    #[test]
    fn death_masks_expiry_and_replacement_are_fenced() {
        let mut bodies = Bodies::new(7);
        bodies.spawn(life(0), DVec3::Y, hull()).unwrap();
        bodies
            .corpse(life(0), DVec3::ZERO, DVec3::ONE, 60.)
            .unwrap();
        assert!(!bodies.get(life(0)).unwrap().damage_enabled());
        assert!(!bodies.get(life(0)).unwrap().selection_enabled());
        assert_eq!(bodies.corpse_colliders().unwrap().len(), 1);
        assert!(bodies.expire(59.9).unwrap().is_empty());
        assert_eq!(bodies.expire(60.).unwrap(), vec![life(0)]);
        assert!(bodies.corpse_colliders().unwrap().is_empty());
        assert!(bodies.spawn(life(0), DVec3::Y, hull()).is_err());
        bodies.spawn(life(1), DVec3::Y, hull()).unwrap();
        assert!(!bodies.remove(life(0)));
        assert!(bodies.place(life(0), DVec3::ZERO, 0.01).is_err());
        assert!(bodies.get(life(1)).unwrap().damage_enabled());
        bodies.validate().unwrap();
    }
    #[test]
    fn checkpoint_and_invalid_updates_preserve_ownership() {
        let mut b = Bodies::new(7);
        b.spawn(life(0), DVec3::Y, hull()).unwrap();
        b.place(life(0), DVec3::new(1., 1., 0.), 0.1).unwrap();
        let bytes = serde_json::to_vec(&b).unwrap();
        assert!(b.place(life(0), DVec3::NAN, 0.1).is_err());
        assert!(
            b.spawn(
                Life {
                    instance: 8,
                    ..life(1)
                },
                DVec3::Y,
                hull()
            )
            .is_err()
        );
        assert_eq!(bytes, serde_json::to_vec(&b).unwrap());
        let restored: Bodies = serde_json::from_slice(&bytes).unwrap();
        restored.validate().unwrap();
        assert_eq!(
            restored.get(life(0)).unwrap().body.vel,
            DVec3::new(10., 0., 0.)
        );
    }
}
