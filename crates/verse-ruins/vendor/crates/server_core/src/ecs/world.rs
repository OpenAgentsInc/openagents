use glam::Vec3;

use crate::actor::{ActorId, ActorKind, Faction, Health, Transform};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug)]
pub struct MoveSpeed {
    pub mps: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct AggroRadius {
    pub m: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct AttackRadius {
    pub m: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Melee {
    pub damage: i32,
    pub cooldown_s: f32,
    pub ready_in_s: f32,
}

/// Entity handle local to this world (opaque index).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Entity(u32);

#[derive(Clone, Debug)]
pub struct Components {
    pub id: ActorId,
    pub kind: ActorKind,
    pub faction: Faction,
    pub name: Option<String>,
    pub tr: Transform,
    pub hp: Health,
    pub move_speed: Option<MoveSpeed>,
    pub aggro: Option<AggroRadius>,
    pub attack: Option<AttackRadius>,
    pub melee: Option<Melee>,
    pub projectile: Option<Projectile>,
    pub velocity: Option<Velocity>,
    pub owner: Option<Owner>,
    pub homing: Option<Homing>,
    pub spellbook: Option<Spellbook>,
    pub pool: Option<ResourcePool>,
    pub cooldowns: Option<Cooldowns>,
    // Intents (authoritative inputs)
    pub intent_move: Option<IntentMove>,
    pub intent_aim: Option<IntentAim>,
    // Effects & lifecycle
    pub burning: Option<Burning>,
    pub slow: Option<Slow>,
    pub stunned: Option<Stunned>,
    pub despawn_after: Option<DespawnAfter>,
}

#[derive(Default, Debug)]
pub struct WorldEcs {
    next_ent: u32,
    next_id: u32,
    ents: Vec<Components>,
}

impl WorldEcs {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.ents.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ents.is_empty()
    }

    pub fn spawn(
        &mut self,
        kind: ActorKind,
        faction: Faction,
        tr: Transform,
        hp: Health,
    ) -> ActorId {
        let id = ActorId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let _e = Entity(self.next_ent);
        self.next_ent = self.next_ent.wrapping_add(1);
        self.ents.push(Components {
            id,
            kind,
            faction,
            name: None,
            tr,
            hp,
            move_speed: None,
            aggro: None,
            attack: None,
            melee: None,
            projectile: None,
            velocity: None,
            owner: None,
            homing: None,
            spellbook: None,
            pool: None,
            cooldowns: None,
            intent_move: None,
            intent_aim: None,
            burning: None,
            slow: None,
            stunned: None,
            despawn_after: None,
        });
        id
    }

    pub fn spawn_from_components(&mut self, mut c: Components) -> ActorId {
        // Assign id if not set
        if c.id.0 == 0 {
            c.id = ActorId(self.next_id);
            self.next_id = self.next_id.wrapping_add(1);
        }
        let _e = Entity(self.next_ent);
        self.next_ent = self.next_ent.wrapping_add(1);
        let id = c.id;
        if c.intent_move.is_none() {
            c.intent_move = None;
        }
        if c.intent_aim.is_none() {
            c.intent_aim = None;
        }
        self.ents.push(c);
        id
    }

    pub fn get(&self, id: ActorId) -> Option<&Components> {
        self.ents.iter().find(|c| c.id == id)
    }

    pub fn get_mut(&mut self, id: ActorId) -> Option<&mut Components> {
        self.ents.iter_mut().find(|c| c.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Components> {
        self.ents.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Components> {
        self.ents.iter_mut()
    }

    pub fn remove_dead(&mut self) {
        self.ents.retain(|c| c.hp.alive());
    }

    /// Helper: find nearest hostile actor to `pos` within optional max radius^2.
    pub fn nearest_hostile(
        &self,
        faction: Faction,
        pos: Vec3,
        max_r2: Option<f32>,
    ) -> Option<ActorId> {
        let mut best: Option<(f32, ActorId)> = None;
        for c in &self.ents {
            if !c.hp.alive() {
                continue;
            }
            if !hostile_default(faction, c.faction) {
                continue;
            }
            let dx = c.tr.pos.x - pos.x;
            let dz = c.tr.pos.z - pos.z;
            let d2 = dx * dx + dz * dz;
            if let Some(cap) = max_r2
                && d2 > cap
            {
                continue;
            }
            if best.map(|(b, _)| d2 < b).unwrap_or(true) {
                best = Some((d2, c.id));
            }
        }
        best.map(|(_, id)| id)
    }
}

impl Components {
    pub fn apply_burning(&mut self, dps: i32, dur: f32, src: Option<ActorId>) {
        self.burning = Some(match self.burning {
            Some(b) => Burning {
                dps: b.dps.max(dps),
                remaining_s: b.remaining_s.max(dur),
                src: src.or(b.src),
            },
            None => Burning {
                dps,
                remaining_s: dur,
                src,
            },
        });
    }
    pub fn apply_slow(&mut self, mul: f32, dur: f32) {
        let m = mul.clamp(0.0, 1.0);
        self.slow = Some(match self.slow {
            Some(s) => Slow {
                mul: s.mul.min(m),
                remaining_s: s.remaining_s.max(dur),
            },
            None => Slow {
                mul: m,
                remaining_s: dur,
            },
        });
    }
    pub fn apply_stun(&mut self, dur: f32) {
        self.stunned = Some(match self.stunned {
            Some(s) => Stunned {
                remaining_s: s.remaining_s.max(dur),
            },
            None => Stunned { remaining_s: dur },
        });
    }
}

#[inline]
fn hostile_default(a: Faction, b: Faction) -> bool {
    use Faction::*;
    matches!(
        (a, b),
        (Pc, Undead) | (Undead, Pc) | (Wizards, Undead) | (Undead, Wizards)
    )
}

// ----------------------------------------------------------------------------
// Projectile-related components
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct Projectile {
    pub kind: crate::ProjKind,
    pub ttl_s: f32,
    pub age_s: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Velocity {
    pub v: Vec3,
}

#[derive(Copy, Clone, Debug)]
pub struct Owner {
    pub id: ActorId,
}

#[derive(Copy, Clone, Debug)]
pub struct Homing {
    pub target: ActorId,
    pub turn_rate: f32,
    pub max_range_m: f32,
    pub reacquire: bool,
}

// Status effects --------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct Burning {
    pub dps: i32,
    pub remaining_s: f32,
    pub src: Option<crate::actor::ActorId>,
}

#[derive(Copy, Clone, Debug)]
pub struct Slow {
    /// Multiply base MoveSpeed.mps by this factor (0.0..=1.0).
    pub mul: f32,
    pub remaining_s: f32,
}

#[derive(Copy, Clone, Debug)]
pub struct Stunned {
    pub remaining_s: f32,
}

// Lifecycle -------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct DespawnAfter {
    pub seconds: f32,
}

// ----------------------------------------------------------------------------
// Intents (authoritative inputs)
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct IntentMove {
    pub dx: f32,
    pub dz: f32,
    pub run: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct IntentAim {
    pub yaw: f32,
}

// ----------------------------------------------------------------------------
// Casting-related components
// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Spellbook {
    pub known: Vec<crate::SpellId>,
}

#[derive(Copy, Clone, Debug)]
pub struct ResourcePool {
    pub mana: i32,
    pub max: i32,
    pub regen_per_s: f32,
    /// Fractional accumulator for mana regeneration to avoid truncation per tick
    pub mana_frac: f32,
}

#[derive(Clone, Debug)]
pub struct Cooldowns {
    pub gcd_s: f32,
    pub gcd_ready: f32,
    pub per_spell: HashMap<crate::SpellId, f32>,
}

#[derive(Default)]
pub struct CmdBuf {
    pub spawns: Vec<Components>,
    pub despawns: Vec<ActorId>,
}

impl WorldEcs {
    pub fn apply_cmds(&mut self, cmds: &mut CmdBuf) {
        for c in cmds.spawns.drain(..) {
            let _ = self.spawn_from_components(c);
        }
        if !cmds.despawns.is_empty() {
            self.ents.retain(|e| !cmds.despawns.contains(&e.id));
            cmds.despawns.clear();
        }
    }
}
