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
    motion_starts: BTreeMap<u32, [f32; 3]>,
    motion_paths: BTreeMap<u32, Vec<[f32; 3]>>,
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
fn motion_position(
    id: u32,
    final_pos: [f32; 3],
    starts: &BTreeMap<u32, [f32; 3]>,
    paths: &BTreeMap<u32, Vec<[f32; 3]>>,
    fraction: f32,
) -> Vec3 {
    if let Some(path) = paths.get(&id) {
        let segment = fraction.clamp(0., 1.) * (path.len() - 1) as f32;
        let index = (segment.floor() as usize).min(path.len() - 2);
        return Vec3::from(path[index]).lerp(Vec3::from(path[index + 1]), segment - index as f32);
    }
    Vec3::from(starts.get(&id).copied().unwrap_or(final_pos)).lerp(Vec3::from(final_pos), fraction)
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
        for (id, pos) in &self.motion_starts {
            valid_position(*pos)?;
            if !self.actors.contains_key(id) {
                return Err("Checkpoint motion refers to a removed actor".into());
            }
        }
        for (id, path) in &self.motion_paths {
            if !(2..=13).contains(&path.len())
                || self.motion_starts.get(id) != path.first()
                || self
                    .actors
                    .get(id)
                    .is_none_or(|actor| path.last() != Some(&actor.pos))
            {
                return Err("Invalid checkpoint character trajectory".into());
            }
            for point in path {
                valid_position(*point)?;
            }
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
            motion_starts: BTreeMap::new(),
            motion_paths: BTreeMap::new(),
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
        self.motion_starts.entry(id).or_insert(actor.pos);
        actor.pos = pos;
        self.motion_paths.remove(&id);
        actor.yaw = yaw;
        Ok(())
    }
    /// Retains the controller's admitted substep positions for continuous collision.
    pub fn record_motion_path(&mut self, id: u32, path: Vec<[f32; 3]>) -> Result<(), String> {
        if !(2..=13).contains(&path.len())
            || self
                .actors
                .get(&id)
                .is_none_or(|actor| path.last() != Some(&actor.pos))
        {
            return Err("Invalid admitted character trajectory".into());
        }
        for point in &path {
            valid_position(*point)?;
        }
        self.motion_starts.insert(id, path[0]);
        self.motion_paths.insert(id, path);
        Ok(())
    }

    /// Places an admitted teleport without sweeping through the skipped space.
    pub fn teleport_chamber_actor(
        &mut self,
        id: u32,
        pos: [f32; 3],
        yaw: f32,
    ) -> Result<(), String> {
        self.place_chamber_actor(id, pos, yaw)?;
        self.motion_starts.insert(id, pos);
        self.motion_paths.remove(&id);
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
    fn motion_impact(
        &mut self,
        kind: Spell,
        point: Vec3,
        target: Option<u32>,
        starts: &BTreeMap<u32, [f32; 3]>,
        paths: &BTreeMap<u32, Vec<[f32; 3]>>,
        fraction: f32,
    ) -> Result<(), String> {
        let final_positions: BTreeMap<_, _> = self
            .actors
            .iter()
            .map(|(id, actor)| (*id, actor.pos))
            .collect();
        for (id, actor) in &mut self.actors {
            actor.pos = motion_position(*id, actor.pos, starts, paths, fraction).to_array();
        }
        let result = self.impact(kind, point, target);
        for (id, pos) in final_positions {
            if let Some(actor) = self.actors.get_mut(&id) {
                actor.pos = pos;
            }
        }
        result
    }

    fn advance_flights(
        &mut self,
        dt: f32,
        starts: &BTreeMap<u32, [f32; 3]>,
        paths: &BTreeMap<u32, Vec<[f32; 3]>>,
        from: f32,
        to: f32,
    ) -> Result<(), String> {
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
                    flight.view.vel = ((motion_position(actor.id, actor.pos, starts, paths, from)
                        + Vec3::Y * 1.1
                        - start)
                        .normalize_or_zero()
                        * 18.)
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
                let feet_start = motion_position(actor.id, actor.pos, starts, paths, from);
                let feet_end = motion_position(actor.id, actor.pos, starts, paths, to);
                let fraction = physics::continuous::sphere_capsule(
                    start.as_dvec3(),
                    (start + delta).as_dvec3(),
                    0.06,
                    feet_start.as_dvec3(),
                    feet_end.as_dvec3(),
                    0.35,
                    1.8,
                )?
                .map(|t| t as f32);
                if let Some(t) = fraction {
                    if hit.is_none_or(|(old, _)| t < old) {
                        hit = Some((t, actor.id));
                    }
                }
            }
            if let Some((t, id)) =
                hit.filter(|(t, _)| wall.is_none_or(|w| f64::from(*t) < w.fraction))
            {
                self.motion_impact(
                    flight.view.kind,
                    start + delta * t,
                    Some(id),
                    starts,
                    paths,
                    from + (to - from) * t,
                )?;
            } else if let Some(wall) = wall {
                self.motion_impact(
                    flight.view.kind,
                    start + delta * wall.fraction as f32,
                    None,
                    starts,
                    paths,
                    from + (to - from) * wall.fraction as f32,
                )?;
            } else {
                flight.view.pos = (start + delta).to_array();
                self.flights.push(flight);
            }
        }
        Ok(())
    }

    pub fn tick(&mut self, dt: f32, position: [f32; 3], yaw: f32) -> Result<(), String> {
        valid_position(position)?;
        if !dt.is_finite() || !(0. ..=0.1).contains(&dt) || !yaw.is_finite() {
            return Err("Invalid world step".into());
        }
        // The adapter may already have recorded the player controller's path.
        if self
            .actors
            .get(&0)
            .is_none_or(|actor| actor.pos != position)
        {
            self.place_chamber_actor(0, position, yaw)?;
        } else {
            self.actors.get_mut(&0).unwrap().yaw = yaw;
        }
        let began = self.elapsed;
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
        let starts = std::mem::take(&mut self.motion_starts);
        let paths = std::mem::take(&mut self.motion_paths);
        let steps = (dt * 120.).ceil().max(1.) as usize;
        let mut boundaries: Vec<f32> = (0..=steps).map(|step| step as f32 / steps as f32).collect();
        for path in paths.values() {
            let count = path.len() - 1;
            boundaries.extend((1..count).map(|step| step as f32 / count as f32));
        }
        boundaries.sort_by(f32::total_cmp);
        boundaries.dedup();
        for window in boundaries.windows(2) {
            let (from, to) = (window[0], window[1]);
            self.elapsed = began + dt * to;
            self.advance_flights(dt * (to - from), &starts, &paths, from, to)?;
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
            self.motion_starts.remove(&id);
            self.motion_paths.remove(&id);
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
    fn moving_actor_crossing_is_hit_and_replays_pending_motion() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([-2., 0., 0.5], 100)]).unwrap();
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.place_chamber_actor(ids[0], [2., 0., 0.5], 0.).unwrap();
        let mut restored: Simulation =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        restored.validate().unwrap();
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        restored.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 92);
        assert!(s.flights.is_empty());
        assert_eq!(s.actors[&ids[0]].pos, [2., 0., 0.5]);
        assert!(s.motion_starts.is_empty());
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
    }

    #[test]
    fn crossing_behind_thin_cover_does_not_receive_damage() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([-2., 0., 0.5], 100)]).unwrap();
        s.set_colliders(vec![physics::kinematic::Aabb {
            min: glam::DVec3::new(-1., 0., 0.15),
            max: glam::DVec3::new(1., 3., 0.16),
        }]);
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.place_chamber_actor(ids[0], [2., 0., 0.5], 0.).unwrap();
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 100);
        assert_eq!(s.effects.len(), 1);
        assert!(s.effects[0].pos[2] < 0.15);
    }

    #[test]
    fn admitted_teleport_does_not_sweep_the_skipped_space() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([-2., 0., 0.5], 100)]).unwrap();
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.teleport_chamber_actor(ids[0], [2., 0., 0.5], 0.).unwrap();
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 100);
        assert_eq!(s.flights.len(), 1);
    }

    #[test]
    fn homing_flights_do_not_transfer_to_a_replacement_actor() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([0., 0., 8.], 10)]).unwrap();
        s.cast(Spell::MagicMissile, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.bow_impact(ids[0], 10).unwrap();
        let replacement = s.spawn_chamber_actor([0., 0., 8.], 10).unwrap();
        assert_ne!(replacement, ids[0]);
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert!(s.flights.is_empty());
        assert_eq!(s.actors[&replacement].hp, 10);
    }

    #[test]
    fn curved_controller_path_hits_when_endpoint_chord_misses() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([2., 0., 0.4], 100)]).unwrap();
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.record_motion_path(ids[0], vec![[2., 0., 0.4], [0., 0., 0.4], [2., 0., 0.4]])
            .unwrap();
        let mut restored: Simulation =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        restored.validate().unwrap();
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        restored.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 92);
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
    }

    #[test]
    fn controller_path_avoids_false_endpoint_chord_contact() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([-2., 0., 0.4], 100)]).unwrap();
        s.cast(Spell::Firebolt, [0., 1.1, 0.], [0., 0., 1.])
            .unwrap();
        s.place_chamber_actor(ids[0], [2., 0., 0.4], 0.).unwrap();
        s.record_motion_path(
            ids[0],
            vec![[-2., 0., 0.4], [-2., 0., 2.], [2., 0., 2.], [2., 0., 0.4]],
        )
        .unwrap();
        s.tick(1. / 30., [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 100);
        assert_eq!(s.flights.len(), 1);
        assert!(s.record_motion_path(ids[0], vec![[0.; 3]; 14]).is_err());
        assert!(s.record_motion_path(ids[0], vec![[0.; 3]; 2]).is_err());
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
