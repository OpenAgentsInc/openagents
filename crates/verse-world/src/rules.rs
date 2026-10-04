//! Owned chamber resources, projectiles, impacts, and presentation snapshots.
//! The retained Ruins adapter is a parity reference, not a dependency.
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spell {
    Firebolt,
    MagicMissile,
    Fireball,
}
impl Spell {
    pub const ALL: [Self; 3] = [Self::Firebolt, Self::MagicMissile, Self::Fireball];
    fn cost(self) -> i32 {
        match self {
            Self::Firebolt => 0,
            Self::MagicMissile => 2,
            Self::Fireball => 5,
        }
    }
    fn cooldown(self) -> f32 {
        match self {
            Self::Firebolt => 0.3,
            Self::MagicMissile => 1.5,
            Self::Fireball => 2.,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Firebolt => "Firebolt",
            Self::MagicMissile => "Magic missile",
            Self::Fireball => "Fireball",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ability {
    pub id: Spell,
    pub label: String,
    pub cost: i32,
    pub ready: bool,
    pub cooldown_remaining: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    pub id: u32,
    pub kind: String,
    pub faction: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub id: u32,
    pub kind: Spell,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub kind: u8,
    pub pos: [f32; 3],
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Counters {
    pub casts: u64,
    pub projectiles: u64,
    pub hits: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub elapsed: f32,
    pub player: Player,
    pub abilities: Vec<Ability>,
    pub actors: Vec<Actor>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub counters: Counters,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Flight {
    view: Projectile,
    target: Option<u32>,
    until: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Burn {
    actor: u32,
    next: f32,
    remaining: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Simulation {
    elapsed: f32,
    player: Player,
    mana_fraction: f32,
    actors: BTreeMap<u32, Actor>,
    next_id: u32,
    ready: BTreeMap<Spell, f32>,
    global_ready: f32,
    flights: Vec<Flight>,
    burns: Vec<Burn>,
    deaths: BTreeMap<u32, f32>,
    effects: Vec<Effect>,
    counters: Counters,
    // Static solids come from the validated scene profile after checkpoint load.
    #[serde(skip)]
    colliders: Vec<physics::kinematic::Aabb>,
}
fn valid_position(pos: [f32; 3]) -> Result<(), String> {
    if pos.iter().any(|p| !p.is_finite() || p.abs() > 10_000.) {
        Err("Invalid world position".into())
    } else {
        Ok(())
    }
}
impl Simulation {
    pub fn validate(&self) -> Result<(), String> {
        if !self.elapsed.is_finite()
            || self.elapsed < 0.
            || self.player.max_hp != 100
            || self.player.max_mana != 20
            || !(0..=100).contains(&self.player.hp)
            || !(0..=20).contains(&self.player.mana)
            || !self.mana_fraction.is_finite()
            || !(0. ..1.).contains(&self.mana_fraction)
            || self.actors.len() > 1024
            || self.flights.len() > 128
            || self.burns.len() > 384
            || !self.global_ready.is_finite()
            || self.global_ready < 0.
            || self.ready.values().any(|at| !at.is_finite() || *at < 0.)
        {
            return Err("Invalid combat checkpoint".into());
        }
        let player = self.actors.get(&0).ok_or("Missing checkpoint player")?;
        if player.hp != self.player.hp {
            return Err("Checkpoint player health disagrees".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for (id, actor) in &self.actors {
            valid_position(actor.pos)?;
            if *id != actor.id
                || *id >= self.next_id
                || !actor.yaw.is_finite()
                || !(1..=1_000_000).contains(&actor.max_hp)
                || !(0..=actor.max_hp).contains(&actor.hp)
                || actor.alive != (actor.hp > 0)
            {
                return Err("Invalid checkpoint actor".into());
            }
            ids.insert(*id);
        }
        for flight in &self.flights {
            valid_position(flight.view.pos)?;
            if !ids.insert(flight.view.id)
                || flight.view.id >= self.next_id
                || !Vec3::from(flight.view.vel).is_finite()
                || Vec3::from(flight.view.vel).length() > 30.
                || !flight.until.is_finite()
                || flight.until < 0.
            {
                return Err("Invalid checkpoint projectile".into());
            }
        }
        if self.deaths.iter().any(|(id, at)| {
            !at.is_finite()
                || *at > self.elapsed
                || *at < 0.
                || self.actors.get(id).is_none_or(|a| a.alive)
        }) || self
            .burns
            .iter()
            .any(|b| !b.next.is_finite() || !(1..=3).contains(&b.remaining))
        {
            return Err("Invalid checkpoint lifetime".into());
        }
        Ok(())
    }
    pub fn chamber(
        position: [f32; 3],
        hostiles: &[([f32; 3], i32)],
    ) -> Result<(Self, Vec<u32>), String> {
        valid_position(position)?;
        if hostiles.len() > 256 {
            return Err("Chamber actor budget exceeded".into());
        }
        let mut s = Self {
            elapsed: 0.,
            player: Player {
                hp: 100,
                max_hp: 100,
                mana: 20,
                max_mana: 20,
            },
            mana_fraction: 0.,
            actors: BTreeMap::new(),
            next_id: 1,
            ready: BTreeMap::new(),
            global_ready: 0.,
            flights: vec![],
            burns: vec![],
            deaths: BTreeMap::new(),
            effects: vec![],
            counters: Counters::default(),
            colliders: vec![],
        };
        s.actors.insert(
            0,
            Actor {
                id: 0,
                kind: "wizard".into(),
                faction: "player".into(),
                pos: position,
                yaw: 0.,
                hp: 100,
                max_hp: 100,
                alive: true,
            },
        );
        let mut ids = vec![];
        for (pos, hp) in hostiles {
            ids.push(s.spawn_chamber_actor(*pos, *hp)?);
        }
        Ok((s, ids))
    }
    pub fn set_colliders(&mut self, solids: Vec<physics::kinematic::Aabb>) {
        self.colliders = solids;
    }
    fn allocate(&mut self) -> Result<u32, String> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or("World actor IDs exhausted")?;
        Ok(id)
    }
    pub fn spawn_chamber_actor(&mut self, pos: [f32; 3], hp: i32) -> Result<u32, String> {
        valid_position(pos)?;
        if !(1..=1_000_000).contains(&hp) || self.actors.len() >= 1024 {
            return Err("Invalid chamber actor".into());
        }
        let id = self.allocate()?;
        self.actors.insert(
            id,
            Actor {
                id,
                kind: "wizard".into(),
                faction: "undead".into(),
                pos,
                yaw: 0.,
                hp,
                max_hp: hp,
                alive: true,
            },
        );
        Ok(id)
    }
    pub fn place_chamber_actor(&mut self, id: u32, pos: [f32; 3], yaw: f32) -> Result<(), String> {
        valid_position(pos)?;
        if !yaw.is_finite() {
            return Err("Invalid actor facing".into());
        }
        let actor = self.actors.get_mut(&id).ok_or("Unknown chamber actor")?;
        actor.pos = pos;
        actor.yaw = yaw;
        Ok(())
    }
    pub fn bow_impact(&mut self, id: u32, damage: i32) -> Result<(), String> {
        if !(1..=10_000).contains(&damage) || id == 0 {
            return Err("Invalid directed damage".into());
        }
        let actor = self.actors.get_mut(&id).ok_or("Unknown impact target")?;
        if actor.alive {
            actor.hp = (actor.hp - damage).max(0);
            actor.alive = actor.hp > 0;
            if !actor.alive {
                self.deaths.insert(id, self.elapsed);
            }
        }
        Ok(())
    }
    pub fn spend_chamber_mana(&mut self, cost: i32) -> Result<(), String> {
        if !(0..=20).contains(&cost) {
            return Err("Invalid mana debit".into());
        }
        if self.player.hp == 0 {
            return Err("The player is defeated".into());
        }
        if self.player.mana < cost {
            return Err("Not enough mana".into());
        }
        self.player.mana -= cost;
        self.counters.casts += 1;
        Ok(())
    }
    pub fn chamber_player_damage(&mut self, damage: i32) -> Result<(), String> {
        if !(0..=10_000).contains(&damage) {
            return Err("Invalid hostile damage".into());
        }
        self.player.hp = (self.player.hp - damage).max(0);
        if let Some(actor) = self.actors.get_mut(&0) {
            actor.hp = self.player.hp;
            actor.alive = actor.hp > 0;
        }
        Ok(())
    }
    pub fn cooldown_duration(&self, spell: Spell) -> f32 {
        spell.cooldown()
    }
    pub fn cast(
        &mut self,
        spell: Spell,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Result<(), String> {
        valid_position(origin)?;
        let direction = Vec3::from(direction);
        if !direction.is_finite() || !(0.99..=1.01).contains(&direction.length_squared()) {
            return Err("Invalid projectile aim".into());
        }
        if self.elapsed
            < self
                .ready
                .get(&spell)
                .copied()
                .unwrap_or(0.)
                .max(self.global_ready)
        {
            return Err("Spell is cooling down".into());
        }
        let count = if spell == Spell::MagicMissile { 3 } else { 1 };
        if self.flights.len() + count > 128 || self.next_id.checked_add(count as u32).is_none() {
            return Err("Projectile budget exceeded".into());
        }
        let start = Vec3::from(origin);
        let target = self
            .actors
            .values()
            .filter(|a| a.id != 0 && a.alive)
            .filter_map(|a| {
                let offset = Vec3::from(a.pos) + Vec3::Y * 1.1 - start;
                let score = offset.normalize_or_zero().dot(direction);
                (score > 0.8).then_some((a.id, score, offset.length_squared()))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.2.total_cmp(&a.2)))
            .map(|a| a.0);
        self.spend_chamber_mana(spell.cost())?;
        self.ready.insert(spell, self.elapsed + spell.cooldown());
        self.global_ready = self.elapsed + if spell == Spell::Fireball { 0.5 } else { 0.3 };
        for _ in 0..count {
            let id = self.allocate()?;
            let speed = match spell {
                Spell::Firebolt => 24.,
                Spell::Fireball => 16.,
                Spell::MagicMissile => 18.,
            };
            self.flights.push(Flight {
                view: Projectile {
                    id,
                    kind: spell,
                    pos: origin,
                    vel: (direction * speed).to_array(),
                },
                target,
                until: self.elapsed + 6.,
            });
            self.counters.projectiles += 1;
        }
        Ok(())
    }
    fn visible(&self, start: Vec3, end: Vec3) -> bool {
        physics::kinematic::sweep_box(
            start.as_dvec3(),
            glam::DVec3::splat(0.01),
            (end - start).as_dvec3(),
            &self.colliders,
        )
        .is_ok_and(|hit| hit.is_none())
    }
    fn impact(&mut self, kind: Spell, point: Vec3, target: Option<u32>) -> Result<(), String> {
        self.effects.push(Effect {
            kind: match kind {
                Spell::Firebolt => 0,
                Spell::MagicMissile => 2,
                Spell::Fireball => 1,
            },
            pos: point.to_array(),
        });
        self.counters.hits += 1;
        if kind == Spell::Fireball {
            let ids: Vec<_> = self
                .actors
                .values()
                .filter(|a| {
                    a.id != 0
                        && a.alive
                        && Vec3::from(a.pos).distance(point - Vec3::Y * 1.1) <= 6.096
                        && self.visible(point, Vec3::from(a.pos) + Vec3::Y * 1.1)
                })
                .map(|a| a.id)
                .collect();
            for id in ids {
                self.bow_impact(id, 15)?;
                self.burns.push(Burn {
                    actor: id,
                    next: self.elapsed + 1.,
                    remaining: 3,
                });
            }
        } else if let Some(id) = target {
            self.bow_impact(id, if kind == Spell::Firebolt { 8 } else { 4 })?;
        }
        Ok(())
    }
    pub fn tick(&mut self, dt: f32, position: [f32; 3], yaw: f32) -> Result<(), String> {
        valid_position(position)?;
        if !dt.is_finite() || !(0. ..=0.1).contains(&dt) || !yaw.is_finite() {
            return Err("Invalid world step".into());
        }
        self.place_chamber_actor(0, position, yaw)?;
        self.elapsed += dt;
        if self.player.hp > 0 && self.player.mana < self.player.max_mana {
            self.mana_fraction += dt;
            if self.mana_fraction >= 1. {
                self.player.mana += 1;
                self.mana_fraction -= 1.;
            }
        } else {
            self.mana_fraction = 0.;
        }
        self.effects.clear();
        for mut flight in std::mem::take(&mut self.flights) {
            if self.elapsed >= flight.until {
                continue;
            }
            let start = Vec3::from(flight.view.pos);
            if flight.view.kind == Spell::MagicMissile {
                if let Some(actor) = flight
                    .target
                    .and_then(|id| self.actors.get(&id))
                    .filter(|a| a.alive)
                {
                    flight.view.vel =
                        ((Vec3::from(actor.pos) + Vec3::Y * 1.1 - start).normalize_or_zero() * 18.)
                            .to_array();
                } else {
                    continue;
                }
            }
            let delta = Vec3::from(flight.view.vel) * dt;
            let wall = physics::kinematic::sweep_box(
                start.as_dvec3(),
                glam::DVec3::splat(0.06),
                delta.as_dvec3(),
                &self.colliders,
            )?;
            let mut hit: Option<(f32, u32)> = None;
            for actor in self.actors.values().filter(|a| a.id != 0 && a.alive) {
                let center = Vec3::from(actor.pos) + Vec3::Y * 1.1;
                let offset = start - center;
                let aa = delta.length_squared();
                let bb = offset.dot(delta);
                let cc = offset.length_squared() - 0.65f32.powi(2);
                let discriminant = bb * bb - aa * cc;
                let fraction = if cc <= 0. {
                    Some(0.)
                } else if aa > 0. && discriminant >= 0. {
                    let t = (-bb - discriminant.sqrt()) / aa;
                    (0. ..=1.).contains(&t).then_some(t)
                } else {
                    None
                };
                if let Some(t) = fraction {
                    if hit.is_none_or(|(old, _)| t < old) {
                        hit = Some((t, actor.id));
                    }
                }
            }
            if let Some((t, id)) =
                hit.filter(|(t, _)| wall.is_none_or(|w| f64::from(*t) < w.fraction))
            {
                self.impact(flight.view.kind, start + delta * t, Some(id))?;
            } else if let Some(wall) = wall {
                self.impact(flight.view.kind, start + delta * wall.fraction as f32, None)?;
            } else {
                flight.view.pos = (start + delta).to_array();
                self.flights.push(flight);
            }
        }
        for mut burn in std::mem::take(&mut self.burns) {
            if !self.actors.get(&burn.actor).is_some_and(|a| a.alive) {
                continue;
            }
            if self.elapsed >= burn.next {
                self.bow_impact(burn.actor, 6)?;
                burn.next += 1.;
                burn.remaining -= 1;
            }
            if burn.remaining > 0 {
                self.burns.push(burn);
            }
        }
        let expired: Vec<_> = self
            .deaths
            .iter()
            .filter(|(_, at)| self.elapsed - **at >= 2.)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.actors.remove(&id);
            self.deaths.remove(&id);
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            elapsed: self.elapsed,
            player: self.player.clone(),
            abilities: Spell::ALL
                .into_iter()
                .map(|id| {
                    let cooldown_remaining = (self
                        .ready
                        .get(&id)
                        .copied()
                        .unwrap_or(0.)
                        .max(self.global_ready)
                        - self.elapsed)
                        .max(0.);
                    Ability {
                        id,
                        label: id.label().into(),
                        cost: id.cost(),
                        ready: self.player.hp > 0
                            && self.player.mana >= id.cost()
                            && cooldown_remaining == 0.,
                        cooldown_remaining,
                    }
                })
                .collect(),
            actors: self.actors.values().cloned().collect(),
            projectiles: self.flights.iter().map(|f| f.view.clone()).collect(),
            effects: self.effects.clone(),
            counters: self.counters.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swept_projectile_hits_thin_cover_before_actor() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([0., 0., 2.], 100)]).unwrap();
        s.set_colliders(vec![physics::kinematic::Aabb {
            min: glam::DVec3::new(-1., 0., 0.9),
            max: glam::DVec3::new(1., 3., 0.91),
        }]);
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.tick(0.1, [0.; 3], 0.).unwrap();
        assert!(s.snapshot().projectiles.is_empty());
        assert_eq!(s.actors[&ids[0]].hp, 100);
    }
    #[test]
    fn checkpoint_replays_pending_projectiles_and_cooldowns() {
        let (mut s, _) = Simulation::chamber([0.; 3], &[([0., 0., 8.], 100)]).unwrap();
        s.cast(Spell::MagicMissile, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        let bytes = serde_json::to_vec(&s).unwrap();
        let mut restored: Simulation = serde_json::from_slice(&bytes).unwrap();
        for _ in 0..60 {
            s.tick(1. / 30., [0.; 3], 0.).unwrap();
            restored.tick(1. / 30., [0.; 3], 0.).unwrap();
        }
        assert_eq!(
            serde_json::to_vec(&s.snapshot()).unwrap(),
            serde_json::to_vec(&restored.snapshot()).unwrap()
        );
        assert_eq!(s.snapshot().counters.hits, 3);
    }
}
