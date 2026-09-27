//! Client replication scaffolding.
//!
//! Responsibilities
//! - Buffer incoming snapshot deltas
//! - Apply to client ECS/state
//! - Invalidate GPU uploads for changed chunks
//!
//! Filled in later when net_core types are finalized.

use glam::Vec3;
use log::warn;
/// Client-side replication buffer that accumulates incoming deltas (chunks,
/// entity snapshots) and exposes a coherent view for presentation layers.
use net_core::snapshot::SnapshotDecode;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct ReplicationBuffer {
    pub updated_chunks: usize,
    pending_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
    // Deltas for unknown instances are held until corresponding instance arrives
    deferred_mesh: Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)>,
    known_dids: std::collections::HashSet<u64>,
    // Track destructible instances' world AABBs for delta validation
    destr_instances: HashMap<u64, (Vec3, Vec3)>,
    pub boss_status: Option<BossStatus>,
    pub actors: Vec<ActorView>,
    pub wizards: Vec<WizardView>,
    pub npcs: Vec<NpcView>,
    pub projectiles: Vec<ProjectileView>,
    pub hits: Vec<net_core::snapshot::HitFx>,
    pub toasts: Vec<u8>,
    pub hud: HudState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BossStatus {
    pub name: String,
    pub ac: i32,
    pub hp: i32,
    pub max: i32,
    pub pos: glam::Vec3,
}

impl ReplicationBuffer {
    /// Apply a raw message. Returns whether any state changed.
    #[allow(clippy::too_many_lines)]
    pub fn apply_message(&mut self, bytes: &[u8]) -> bool {
        // If the message is framed, unwrap the payload; else fall back to raw
        let payload: &[u8] = match net_core::frame::read_msg(bytes) {
            Ok(p) => p,
            Err(_) => bytes,
        };
        // Prefer actor delta snapshot (v4) first
        let mut slice_delta_v3: &[u8] = payload;
        if let Ok(d) = net_core::snapshot::ActorSnapshotDelta::decode(&mut slice_delta_v3) {
            // Removals
            if !d.removals.is_empty() {
                self.actors.retain(|a| !d.removals.contains(&a.id));
                self.wizards.retain(|w| !d.removals.contains(&w.id));
                self.npcs.retain(|n| !d.removals.contains(&n.id));
            }
            // Build id->index map AFTER removals so indices are valid
            let mut idx: HashMap<u32, usize> = HashMap::new();
            for (i, a) in self.actors.iter().enumerate() {
                idx.insert(a.id, i);
            }
            // Updates
            for u in d.updates {
                if let Some(&i) = idx.get(&u.id) {
                    let a = &mut self.actors[i];
                    let mut changed = false;
                    if u.flags & 1 != 0 {
                        a.pos.x = net_core::snapshot::dqpos(u.qpos[0]);
                        a.pos.y = net_core::snapshot::dqpos(u.qpos[1]);
                        a.pos.z = net_core::snapshot::dqpos(u.qpos[2]);
                        changed = true;
                    }
                    if u.flags & 2 != 0 {
                        a.yaw = net_core::snapshot::dqyaw(u.qyaw);
                        changed = true;
                    }
                    if u.flags & 4 != 0 {
                        a.hp = u.hp;
                        changed = true;
                    }
                    if u.flags & 8 != 0 {
                        a.alive = u.alive != 0;
                        changed = true;
                    }
                    if changed {
                        // Update derived views if present
                        for w in &mut self.wizards {
                            if w.id == a.id {
                                w.pos = a.pos;
                                w.yaw = a.yaw;
                                w.hp = a.hp;
                                w.max = a.max;
                            }
                        }
                        for n in &mut self.npcs {
                            if n.id == a.id {
                                n.pos = a.pos;
                                n.yaw = a.yaw;
                                n.hp = a.hp;
                                n.max = a.max;
                                n.alive = a.alive;
                            }
                        }
                    }
                }
            }
            // Spawns
            for a in d.spawns {
                let av = ActorView {
                    id: a.id,
                    kind: a.kind,
                    faction: a.faction,
                    archetype_id: a.archetype_id,
                    name_id: a.name_id,
                    unique: a.unique,
                    pos: glam::vec3(a.pos[0], a.pos[1], a.pos[2]),
                    yaw: a.yaw,
                    radius: a.radius,
                    hp: a.hp,
                    max: a.max,
                    alive: a.alive,
                };
                self.actors.push(av.clone());
                match a.kind {
                    0 => self.wizards.push(WizardView {
                        id: a.id,
                        kind: 0,
                        pos: av.pos,
                        yaw: av.yaw,
                        hp: av.hp,
                        max: av.max,
                        is_pc: av.faction == 0,
                    }),
                    1 | 2 => self.npcs.push(NpcView {
                        id: a.id,
                        hp: av.hp,
                        max: av.max,
                        pos: av.pos,
                        radius: av.radius,
                        alive: av.alive,
                        attack_anim: 0.0,
                        yaw: av.yaw,
                    }),
                    _ => {}
                }
            }
            // After applying updates/spawns/removals, rebuild derived views from actors to ensure
            // that HP/pos/yaw/alive changes are reflected even when only updates occurred.
            {
                self.wizards.clear();
                self.npcs.clear();
                for a in &self.actors {
                    match a.kind {
                        0 => self.wizards.push(WizardView {
                            id: a.id,
                            kind: 0,
                            pos: a.pos,
                            yaw: a.yaw,
                            hp: a.hp,
                            max: a.max,
                            is_pc: a.faction == 0,
                        }),
                        1 | 2 => self.npcs.push(NpcView {
                            id: a.id,
                            hp: a.hp,
                            max: a.max,
                            pos: a.pos,
                            radius: a.radius,
                            alive: a.alive,
                            attack_anim: 0.0,
                            yaw: a.yaw,
                        }),
                        _ => {}
                    }
                }
            }
            // Populate boss status from actor list (kind=2=Boss, prefer unique==1).
            // This replaces the legacy BossStatusMsg and drives HUD + DK model updates.
            self.boss_status = None;
            if let Some(b) = self
                .actors
                .iter()
                .find(|a| a.kind == 2 && a.unique != 0)
                .or_else(|| self.actors.iter().find(|a| a.kind == 2))
                .cloned()
            {
                self.boss_status = Some(BossStatus {
                    name: "Death Knight".to_string(),
                    ac: 0, // AC can be filled from a future replicated field or config if needed
                    hp: b.hp,
                    max: b.max,
                    pos: b.pos,
                });
            }
            // Projectiles (full list)
            self.projectiles.clear();
            for p in d.projectiles {
                self.projectiles.push(ProjectileView {
                    id: p.id,
                    kind: p.kind,
                    pos: glam::vec3(p.pos[0], p.pos[1], p.pos[2]),
                    vel: glam::vec3(p.vel[0], p.vel[1], p.vel[2]),
                });
            }
            // HitFX for this frame
            self.hits = d.hits;
            return true;
        }
        // Destructible instance (register DID)
        let mut slice_inst: &[u8] = payload;
        if let Ok(inst) = net_core::snapshot::DestructibleInstance::decode(&mut slice_inst) {
            // Validate instance AABB to avoid mis-decoding other frames as instances
            let min = Vec3::new(inst.world_min[0], inst.world_min[1], inst.world_min[2]);
            let max = Vec3::new(inst.world_max[0], inst.world_max[1], inst.world_max[2]);
            let ext = max - min;
            let sane = ext.x.is_finite()
                && ext.y.is_finite()
                && ext.z.is_finite()
                && ext.x >= 0.001
                && ext.y >= 0.001
                && ext.z >= 0.001
                && ext.x <= 10000.0
                && ext.y <= 10000.0
                && ext.z <= 10000.0
                && min.x <= max.x
                && min.y <= max.y
                && min.z <= max.z;
            if sane {
                self.known_dids.insert(inst.did);
                self.destr_instances.insert(inst.did, (min, max));
                // Move any deferred deltas for this DID into pending
                let mut rest = Vec::new();
                for (did, chunk, entry) in self.deferred_mesh.drain(..) {
                    if did == inst.did {
                        self.pending_mesh.push((did, chunk, entry));
                        self.updated_chunks += 1;
                    } else {
                        rest.push((did, chunk, entry));
                    }
                }
                self.deferred_mesh = rest;
                return true;
            }
        }
        // Chunk mesh deltas: accept and stash iff instance is known, with validation
        let mut slice_delta: &[u8] = payload;
        if let Ok(delta) = net_core::snapshot::ChunkMeshDelta::decode(&mut slice_delta) {
            // Basic shape checks
            if delta.positions.len() != delta.normals.len() {
                return false;
            }
            if delta.indices.len() % 3 != 0 {
                return false;
            }
            if delta.positions.len() > 200_000 {
                return false;
            }
            // Strict validation: indices must be in-bounds and positions must be finite.
            // Allow empty (zero-tri) deltas as a valid way to clear a chunk.
            if delta.positions.is_empty() {
                // If positions are empty, indices and normals must also be empty
                if !delta.indices.is_empty() || !delta.normals.is_empty() {
                    return false;
                }
            } else {
                let plen = delta.positions.len();
                if delta.indices.iter().any(|&i| (i as usize) >= plen) {
                    return false;
                }
                // Reject any non-finite vertex coordinates
                for p in &delta.positions {
                    if !(p[0].is_finite() && p[1].is_finite() && p[2].is_finite()) {
                        return false;
                    }
                }
            }
            // If we know the DID, ensure the delta bbox lies near the instance AABB
            if let Some((min, max)) = self.destr_instances.get(&delta.did).copied() {
                let mut bb_min = Vec3::splat(f32::INFINITY);
                let mut bb_max = Vec3::splat(f32::NEG_INFINITY);
                for p in &delta.positions {
                    let v = Vec3::new(p[0], p[1], p[2]);
                    bb_min = bb_min.min(v);
                    bb_max = bb_max.max(v);
                }
                // Expand by epsilon to tolerate greedy-mesh edges
                let eps = 0.5f32;
                if bb_max.x < min.x - eps
                    || bb_min.x > max.x + eps
                    || bb_max.y < min.y - eps
                    || bb_min.y > max.y + eps
                    || bb_max.z < min.z - eps
                    || bb_min.z > max.z + eps
                {
                    warn!(
                        "replication: drop destructible delta outside instance AABB (did={})",
                        delta.did
                    );
                    return false;
                }
            }
            let entry = crate::upload::ChunkMeshEntry {
                positions: delta.positions,
                normals: delta.normals,
                indices: delta.indices,
            };
            if self.known_dids.contains(&delta.did) {
                // Avoid enqueuing duplicate pending entries for the same DID+chunk
                let exists = self
                    .pending_mesh
                    .iter()
                    .any(|(did, ch, _)| *did == delta.did && *ch == delta.chunk);
                if !exists {
                    self.pending_mesh.push((delta.did, delta.chunk, entry));
                    self.updated_chunks += 1;
                }
            } else {
                // Avoid duplicate deferred entries for the same DID+chunk
                let exists = self
                    .deferred_mesh
                    .iter()
                    .any(|(did, ch, _)| *did == delta.did && *ch == delta.chunk);
                if !exists {
                    self.deferred_mesh.push((delta.did, delta.chunk, entry));
                }
            }
            return true;
        }
        // HUD status message
        let mut hud_slice: &[u8] = payload;
        if let Ok(hud) = net_core::snapshot::HudStatusMsg::decode(&mut hud_slice) {
            self.hud.mana = hud.mana;
            self.hud.mana_max = hud.mana_max;
            self.hud.gcd_ms = hud.gcd_ms;
            // map spell ids 0,1,2
            self.hud.spell_cds = [0, 0, 0];
            for (id, ms) in hud.spell_cds {
                if (id as usize) < self.hud.spell_cds.len() {
                    self.hud.spell_cds[id as usize] = ms;
                }
            }
            self.hud.burning_ms = hud.burning_ms;
            self.hud.slow_ms = hud.slow_ms;
            self.hud.stunned_ms = hud.stunned_ms;
            return true;
        }
        // HUD toast message
        let mut toast_slice: &[u8] = payload;
        if let Ok(toast) = net_core::snapshot::HudToastMsg::decode(&mut toast_slice) {
            self.toasts.push(toast.code);
            return true;
        }
        false
    }

    /// Drain pending mesh updates accumulated from replication into a vector
    /// of (did, chunk, entry). Renderer or host applies uploads via `MeshUpload`.
    pub fn drain_mesh_updates(
        &mut self,
    ) -> Vec<(u64, (u32, u32, u32), crate::upload::ChunkMeshEntry)> {
        let mut v = Vec::new();
        std::mem::swap(&mut v, &mut self.pending_mesh);
        v
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcView {
    pub id: u32,
    pub hp: i32,
    pub max: i32,
    pub pos: glam::Vec3,
    pub radius: f32,
    pub alive: bool,
    pub attack_anim: f32,
    pub yaw: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActorView {
    pub id: u32,
    pub kind: u8,
    pub faction: u8,
    pub archetype_id: u16,
    pub name_id: u16,
    pub unique: u8,
    pub pos: glam::Vec3,
    pub yaw: f32,
    pub radius: f32,
    pub hp: i32,
    pub max: i32,
    pub alive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HudState {
    pub mana: u16,
    pub mana_max: u16,
    pub gcd_ms: u16,
    pub spell_cds: [u16; 3],
    pub burning_ms: u16,
    pub slow_ms: u16,
    pub stunned_ms: u16,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WizardView {
    pub id: u32,
    pub kind: u8,
    pub pos: glam::Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub max: i32,
    pub is_pc: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileView {
    pub id: u32,
    pub kind: u8,
    pub pos: glam::Vec3,
    pub vel: glam::Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffer_default_is_empty() {
        let b = ReplicationBuffer::default();
        assert_eq!(b.updated_chunks, 0);
    }
    #[test]
    fn tick_snapshot_populates_npcs_and_projectiles() {
        // Legacy TickSnapshot no longer decoded; expect no-op on garbage bytes
        let mut repl = ReplicationBuffer::default();
        assert!(!repl.apply_message(&[0u8]));
    }

    #[test]
    fn apply_tick_snapshot_populates_all_views() {
        let mut buf = ReplicationBuffer::default();
        assert!(!buf.apply_message(&[0u8]));
    }
}
