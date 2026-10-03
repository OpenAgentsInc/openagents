//! Portable world state imported from the private WoW compatibility bridge.
//!
//! This crate owns no transport, credentials, GPU, or realm authority. Positions
//! use Verse meters and Y-up axes. The adapter preserves authoritative IDs and
//! refuses malformed snapshots before presentation consumes them.
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Convert WoW yards and Z-up axes into Verse meters and Y-up axes.
pub fn position_from_wow(p: [f32; 3]) -> Vec3 {
    Vec3::new(-p[1], p[2], -p[0]) * 0.9144
}

/// An authoritative unit projected into presentation state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: u64,
    pub entry: u32,
    pub name: String,
    pub position: Vec3,
    pub health: u32,
    pub max_health: u32,
}

/// A bounded compatibility snapshot, without account or transport information.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct World {
    pub map: u32,
    pub revision: u64,
    pub entities: BTreeMap<u64, Entity>,
}

#[derive(Deserialize)]
struct BridgeUnit {
    guid: String,
    #[serde(default)]
    entry: u32,
    #[serde(default)]
    name: String,
    position: [f32; 3],
    health: Option<u32>,
    max_health: Option<u32>,
}

#[derive(Deserialize)]
struct BridgeSnapshot {
    map: u32,
    nearby: Vec<BridgeUnit>,
}

impl World {
    /// Atomically install the bridge's nearby-unit observation.
    pub fn apply_bridge_snapshot(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 1024 * 1024 {
            return Err("World snapshot exceeds 1 MiB".into());
        }
        let input: BridgeSnapshot = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if input.nearby.len() > 256 {
            return Err("World snapshot exceeds 256 entities".into());
        }
        let mut entities = BTreeMap::new();
        for unit in input.nearby {
            let id = unit.guid.parse().map_err(|_| "Invalid entity GUID")?;
            let position = position_from_wow(unit.position);
            if id == 0 || !position.is_finite() || unit.name.len() > 128 {
                return Err("Invalid world entity".into());
            }
            let health = unit.health.unwrap_or(1);
            let max_health = unit.max_health.unwrap_or(health).max(1);
            if health > max_health || entities.contains_key(&id) {
                return Err("Invalid health or duplicate entity GUID".into());
            }
            entities.insert(
                id,
                Entity {
                    id,
                    entry: unit.entry,
                    name: unit.name,
                    position,
                    health,
                    max_health,
                },
            );
        }
        self.map = input.map;
        self.entities = entities;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_updates_are_atomic_and_preserve_full_guid() {
        let mut world = World::default();
        world.apply_bridge_snapshot(br#"{"map":289,"nearby":[{"guid":"18446744073709551615","entry":900001,"name":"Claude","position":[1,2,3],"health":20,"max_health":30}]}"#).unwrap();
        assert_eq!(
            world.entities[&u64::MAX].position,
            Vec3::new(-2.0, 3.0, -1.0) * 0.9144
        );
        let revision = world.revision;
        assert!(
            world
                .apply_bridge_snapshot(br#"{"map":0,"nearby":[{"guid":"0","position":[0,0,0]}]}"#)
                .is_err()
        );
        assert_eq!(world.map, 289);
        assert_eq!(world.revision, revision);
    }
}

pub mod assets;
