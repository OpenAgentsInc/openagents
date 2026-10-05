//! Owned chamber resources, projectiles, impacts, and presentation snapshots.
//! The retained Ruins adapter is a parity reference, not a dependency.
pub(crate) mod transfer;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileKind {
    Bow,
    SiegeBoulder,
    Firebolt,
    MagicMissile,
    Fireball,
}
/// How area spells treat a flight: Wind Wall, for example, deflects
/// ordinary arrows and bolts but not siege boulders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightTag {
    /// An arrow, bolt, or other ordinary ammunition.
    Ordinary,
    /// A boulder or other siege ammunition.
    Siege,
    /// A projectile a spell creates.
    Spell,
}
impl ProjectileKind {
    pub fn tag(self) -> FlightTag {
        match self {
            Self::Bow => FlightTag::Ordinary,
            Self::SiegeBoulder => FlightTag::Siege,
            Self::Firebolt | Self::MagicMissile | Self::Fireball => FlightTag::Spell,
        }
    }
}
impl From<Spell> for ProjectileKind {
    fn from(spell: Spell) -> Self {
        match spell {
            Spell::Firebolt => Self::Firebolt,
            Spell::MagicMissile => Self::MagicMissile,
            Spell::Fireball => Self::Fireball,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub id: u32,
    pub caster: u32,
    pub kind: ProjectileKind,
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
    #[serde(default)]
    pub deflections: u64,
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
    #[serde(default)]
    wind_dragged: bool,
    #[serde(default)]
    deflected: bool,
    view: Projectile,
    target: Option<u32>,
    until: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Burn {
    caster: u32,
    actor: u32,
    next: f32,
    remaining: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerState {
    resources: Player,
    mana_fraction: f32,
    ready: BTreeMap<Spell, f32>,
    global_ready: f32,
}
impl Default for PlayerState {
    fn default() -> Self {
        Self {
            resources: Player {
                hp: 200,
                max_hp: 200,
                mana: 20,
                max_mana: 20,
            },
            mana_fraction: 0.,
            ready: BTreeMap::new(),
            global_ready: 0.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Simulation {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    primary_absent: bool,
    elapsed: f32,
    players: BTreeMap<u32, PlayerState>,
    actors: BTreeMap<u32, Actor>,
    next_id: u32,
    motion_starts: BTreeMap<u32, [f32; 3]>,
    motion_paths: BTreeMap<u32, Vec<[f32; 3]>>,
    flights: Vec<Flight>,
    #[serde(default)]
    wind_walls: Vec<crate::wind_wall::Wall>,
    #[serde(default)]
    wind_gusts: Vec<crate::gust::Gust>,
    #[serde(skip)]
    spell_cover: BTreeMap<usize, physics::BodyId>,
    #[serde(default)]
    spell_hits: Vec<(physics::BodyId, ProjectileKind)>,
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
            || self.players.is_empty()
            || self.players.len() > 64 + usize::from(self.primary_absent)
            || self.primary_absent && self.players.get(&0).is_none_or(|p| p.resources.hp != 0)
            || !self.players.contains_key(&0)
            || self.actors.len() > 1024
            || self.flights.len() > 128
            || self.wind_walls.len() > 64
            || self.wind_gusts.len() > 64
            || self.burns.len() > 384
        {
            return Err("Invalid combat checkpoint".into());
        }
        for (id, state) in &self.players {
            let p = &state.resources;
            if !(200..=600).contains(&p.max_hp)
                || !(20..=60).contains(&p.max_mana)
                || !(0..=p.max_hp).contains(&p.hp)
                || !(0..=p.max_mana).contains(&p.mana)
                || !state.mana_fraction.is_finite()
                || !(0. ..1.).contains(&state.mana_fraction)
                || !state.global_ready.is_finite()
                || state.global_ready < 0.
                || state.ready.values().any(|at| !at.is_finite() || *at < 0.)
                || self
                    .actors
                    .get(id)
                    .is_none_or(|a| a.hp != p.hp || a.max_hp != p.max_hp || a.faction != "player")
            {
                return Err("Invalid checkpoint player resources".into());
            }
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
                || !matches!(actor.faction.as_str(), "player" | "undead" | "friendly")
                || (actor.faction == "player") != self.players.contains_key(id)
            {
                return Err("Invalid checkpoint actor".into());
            }
            ids.insert(*id);
        }
        for flight in &self.flights {
            valid_position(flight.view.pos)?;
            if !ids.insert(flight.view.id)
                || flight.view.id >= self.next_id
                || !self.players.contains_key(&flight.view.caster)
                || flight
                    .target
                    .is_some_and(|id| self.actors.get(&id).is_some_and(|a| a.faction != "undead"))
                || !Vec3::from(flight.view.vel).is_finite()
                || ((flight.deflected || flight.wind_dragged)
                    && flight.view.kind.tag() != FlightTag::Ordinary)
                || Vec3::from(flight.view.vel).length()
                    > if flight.deflected || flight.wind_dragged {
                        80.
                    } else {
                        30.
                    }
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
                || self.players.contains_key(id)
                || self.actors.get(id).is_none_or(|a| a.alive)
        }) || self.burns.iter().any(|b| {
            !b.next.is_finite()
                || b.next < 0.
                || !(1..=3).contains(&b.remaining)
                || !self.players.contains_key(&b.caster)
                || self
                    .actors
                    .get(&b.actor)
                    .is_none_or(|a| a.faction != "undead")
        }) {
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
            primary_absent: false,
            elapsed: 0.,
            players: BTreeMap::from([(0, PlayerState::default())]),
            actors: BTreeMap::new(),
            next_id: 1,
            motion_starts: BTreeMap::new(),
            motion_paths: BTreeMap::new(),
            flights: vec![],
            wind_walls: vec![],
            wind_gusts: vec![],
            spell_cover: BTreeMap::new(),
            spell_hits: vec![],
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
                hp: 200,
                max_hp: 200,
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
    pub(crate) fn set_spell_cover(
        &mut self,
        solids: Vec<physics::kinematic::Aabb>,
        panels: Vec<(physics::BodyId, physics::kinematic::Aabb)>,
    ) {
        self.colliders = solids;
        self.spell_cover.clear();
        for (body, bounds) in panels {
            self.spell_cover.insert(self.colliders.len(), body);
            self.colliders.push(bounds);
        }
    }
    pub(crate) fn take_spell_hits(&mut self) -> Vec<(physics::BodyId, ProjectileKind)> {
        std::mem::take(&mut self.spell_hits)
    }
    fn allocate(&mut self) -> Result<u32, String> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or("World actor IDs exhausted")?;
        Ok(id)
    }
    /// Assigns a friendly role during trusted scene construction, before stepping.
    pub(crate) fn mark_friendly(&mut self, id: u32) -> Result<(), String> {
        if self.elapsed != 0.0 || !self.flights.is_empty() || !self.burns.is_empty() {
            return Err("Friendly roles must be assigned before simulation starts".into());
        }
        let actor = self.actors.get_mut(&id).ok_or("Unknown friendly actor")?;
        if actor.faction != "undead" || !actor.alive {
            return Err("Invalid friendly actor role".into());
        }
        actor.faction = "friendly".into();
        Ok(())
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
    /// Adds a cooperative player to this simulation, without changing other players.
    /// The host must bind the returned actor ID to its authenticated controller.
    pub fn spawn_player(&mut self, pos: [f32; 3]) -> Result<u32, String> {
        valid_position(pos)?;
        if self.players.len() >= 64 + usize::from(self.primary_absent) || self.actors.len() >= 1024
        {
            return Err("Player capacity exceeded".into());
        }
        let id = self.allocate()?;
        self.players.insert(id, PlayerState::default());
        self.actors.insert(
            id,
            Actor {
                id,
                kind: "wizard".into(),
                faction: "player".into(),
                pos,
                yaw: 0.,
                hp: 200,
                max_hp: 200,
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
        if !(1..=10_000).contains(&damage)
            || self.actors.get(&id).is_some_and(|a| a.faction != "undead")
        {
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
    /// Applies host-derived equipment maxima without healing or resetting cooldowns.
    pub(crate) fn equipment_limits(&mut self, id: u32, hp: i32, mana: i32) -> Result<(), String> {
        if !(200..=600).contains(&hp) || !(20..=60).contains(&mana) {
            return Err("Invalid equipment resource limits".into());
        }
        let player = &mut self.players.get_mut(&id).ok_or("Unknown player")?.resources;
        player.max_hp = hp;
        player.max_mana = mana;
        player.hp = player.hp.min(hp);
        player.mana = player.mana.min(mana);
        let actor = self.actors.get_mut(&id).unwrap();
        actor.hp = player.hp;
        actor.max_hp = hp;
        Ok(())
    }
    /// Restores a living player's resources without changing cooldowns or other players.
    pub(crate) fn recover_resources(
        &mut self,
        id: u32,
        health: u32,
        mana: u32,
    ) -> Result<(), String> {
        if health > 600 || mana > 60 || (health == 0 && mana == 0) {
            return Err("Invalid recovery amounts".into());
        }
        let player = &mut self.players.get_mut(&id).ok_or("Unknown player")?.resources;
        if player.hp == 0 {
            return Err("The player is defeated".into());
        }
        let hp = (player.hp + health as i32).min(player.max_hp);
        let mp = (player.mana + mana as i32).min(player.max_mana);
        if hp == player.hp && mp == player.mana {
            return Err("Player resources are already full".into());
        }
        player.hp = hp;
        player.mana = mp;
        self.actors.get_mut(&id).unwrap().hp = hp;
        Ok(())
    }
    pub fn spend_chamber_mana(&mut self, cost: i32) -> Result<(), String> {
        self.spend_mana_for(0, cost)
    }
    pub fn spend_mana_for(&mut self, id: u32, cost: i32) -> Result<(), String> {
        if !(0..=20).contains(&cost) {
            return Err("Invalid mana debit".into());
        }
        let player = &mut self.players.get_mut(&id).ok_or("Unknown player")?.resources;
        if player.hp == 0 {
            return Err("The player is defeated".into());
        }
        if player.mana < cost {
            return Err("Not enough mana".into());
        }
        player.mana -= cost;
        self.counters.casts += 1;
        Ok(())
    }
    pub(super) fn respawn_player(&mut self, position: [f32; 3], yaw: f32) -> Result<(), String> {
        self.revive_player(0, position, yaw)
    }
    /// Revives one player after the owning adapter admits a new life and spawn.
    /// Other players' resources, flights, and burns remain intact.
    pub fn revive_player(&mut self, id: u32, position: [f32; 3], yaw: f32) -> Result<(), String> {
        if id == 0 && self.primary_absent {
            return Err("Primary simulation anchor has no character".into());
        }
        if self.players.get(&id).ok_or("Unknown player")?.resources.hp != 0 {
            return Err("The adventurer is still alive".into());
        }
        self.teleport_chamber_actor(id, position, yaw)?;
        let state = self.players.get_mut(&id).unwrap();
        let (hp, mana) = (state.resources.max_hp, state.resources.max_mana);
        *state = PlayerState::default();
        state.resources = Player {
            hp,
            max_hp: hp,
            mana,
            max_mana: mana,
        };
        state.global_ready = self.elapsed;
        let actor = self.actors.get_mut(&id).unwrap();
        actor.hp = state.resources.hp;
        actor.alive = true;
        self.deaths.remove(&id);
        self.flights.retain(|f| f.view.caster != id);
        self.burns.retain(|b| b.caster != id);
        Ok(())
    }
    pub fn chamber_player_damage(&mut self, damage: i32) -> Result<(), String> {
        self.player_damage_for(0, damage)
    }
    /// Applies authority-derived damage; network commands must not expose this mutation.
    pub fn player_damage_for(&mut self, id: u32, damage: i32) -> Result<(), String> {
        if !(0..=10_000).contains(&damage) {
            return Err("Invalid hostile damage".into());
        }
        let player = &mut self.players.get_mut(&id).ok_or("Unknown player")?.resources;
        player.hp = (player.hp - damage).max(0);
        let actor = self.actors.get_mut(&id).unwrap();
        actor.hp = player.hp;
        actor.alive = actor.hp > 0;
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
        self.cast_at(0, spell, origin, direction)
    }
    /// Derives release origin from an admitted actor; accepts aim, not client position.
    pub fn cast_for(
        &mut self,
        caster: u32,
        spell: Spell,
        direction: [f32; 3],
    ) -> Result<(), String> {
        let origin = self.release_origin(caster)?;
        self.cast_at(caster, spell, origin, direction)
    }
    fn release_origin(&self, caster: u32) -> Result<[f32; 3], String> {
        if !self.players.contains_key(&caster) {
            return Err("Unknown player".into());
        }
        Ok((Vec3::from(self.actors[&caster].pos) + Vec3::Y * 1.1).to_array())
    }
    pub(crate) fn cast_at(
        &mut self,
        caster: u32,
        spell: Spell,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Result<(), String> {
        valid_position(origin)?;
        let direction = Vec3::from(direction);
        if !direction.is_finite() || !(0.99..=1.01).contains(&direction.length_squared()) {
            return Err("Invalid projectile aim".into());
        }
        let state = self.players.get(&caster).ok_or("Unknown player")?;
        if self.elapsed
            < state
                .ready
                .get(&spell)
                .copied()
                .unwrap_or(0.)
                .max(state.global_ready)
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
            .filter(|a| a.faction == "undead" && a.alive)
            .filter_map(|a| {
                let offset = Vec3::from(a.pos) + Vec3::Y * 1.1 - start;
                let score = offset.normalize_or_zero().dot(direction);
                (score > 0.8).then_some((a.id, score, offset.length_squared()))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.2.total_cmp(&a.2)))
            .map(|a| a.0);
        self.spend_mana_for(caster, spell.cost())?;
        let state = self.players.get_mut(&caster).unwrap();
        state.ready.insert(spell, self.elapsed + spell.cooldown());
        state.global_ready = self.elapsed + if spell == Spell::Fireball { 0.5 } else { 0.3 };
        for _ in 0..count {
            let id = self.allocate()?;
            let speed = match spell {
                Spell::Firebolt => 24.,
                Spell::Fireball => 16.,
                Spell::MagicMissile => 18.,
            };
            self.flights.push(Flight {
                wind_dragged: false,
                deflected: false,
                view: Projectile {
                    id,
                    caster,
                    kind: spell.into(),
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
    /// Launches a non-homing arrow through the same continuous collision path as spells.
    pub fn launch_bow(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Result<(), String> {
        self.launch_bow_at(0, origin, direction)
    }
    pub fn launch_bow_for(&mut self, caster: u32, direction: [f32; 3]) -> Result<(), String> {
        let origin = self.release_origin(caster)?;
        self.launch_bow_at(caster, origin, direction)
    }
    pub(crate) fn launch_bow_at(
        &mut self,
        caster: u32,
        origin: [f32; 3],
        direction: [f32; 3],
    ) -> Result<(), String> {
        valid_position(origin)?;
        let direction = Vec3::from(direction);
        if !direction.is_finite()
            || !(0.99..=1.01).contains(&direction.length_squared())
            || self
                .players
                .get(&caster)
                .is_none_or(|p| p.resources.hp == 0)
        {
            return Err("Invalid bow launch".into());
        }
        if self.flights.len() >= 128 {
            return Err("Projectile budget exceeded".into());
        }
        let id = self.allocate()?;
        self.flights.push(Flight {
            wind_dragged: false,
            deflected: false,
            view: Projectile {
                id,
                caster,
                kind: ProjectileKind::Bow,
                pos: origin,
                vel: (direction * 24.).to_array(),
            },
            target: None,
            until: self.elapsed + 6.,
        });
        self.counters.projectiles += 1;
        Ok(())
    }
    /// Launch siege ammunition through the same bounded swept-flight path as a bow.
    pub fn launch_siege(&mut self, origin: [f32; 3], direction: [f32; 3]) -> Result<(), String> {
        self.launch_bow_at(0, origin, direction)?;
        self.flights.last_mut().unwrap().view.kind = ProjectileKind::SiegeBoulder;
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
    fn impact(
        &mut self,
        caster: u32,
        kind: ProjectileKind,
        point: Vec3,
        target: Option<u32>,
    ) -> Result<(), String> {
        self.effects.push(Effect {
            kind: match kind {
                ProjectileKind::Bow | ProjectileKind::SiegeBoulder => 3,
                ProjectileKind::Firebolt => 0,
                ProjectileKind::MagicMissile => 2,
                ProjectileKind::Fireball => 1,
            },
            pos: point.to_array(),
        });
        self.counters.hits += 1;
        if kind == ProjectileKind::Fireball {
            let ids: Vec<_> = self
                .actors
                .values()
                .filter(|a| {
                    a.faction == "undead"
                        && a.alive
                        && Vec3::from(a.pos).distance(point - Vec3::Y * 1.1) <= 6.096
                        && self.visible(point, Vec3::from(a.pos) + Vec3::Y * 1.1)
                })
                .map(|a| a.id)
                .collect();
            for id in ids {
                self.bow_impact(id, 15)?;
                self.burns.push(Burn {
                    caster,
                    actor: id,
                    next: self.elapsed + 1.,
                    remaining: 3,
                });
            }
        } else if let Some(id) = target {
            self.bow_impact(
                id,
                match kind {
                    ProjectileKind::Bow => 6,
                    ProjectileKind::SiegeBoulder => 12,
                    ProjectileKind::Firebolt => 8,
                    _ => 4,
                },
            )?;
        }
        Ok(())
    }
    fn motion_impact(
        &mut self,
        caster: u32,
        kind: ProjectileKind,
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
        let result = self.impact(caster, kind, point, target);
        for (id, pos) in final_positions {
            if let Some(actor) = self.actors.get_mut(&id) {
                actor.pos = pos;
            }
        }
        result
    }

    pub(crate) fn set_spell_gusts(&mut self, gusts: Vec<crate::gust::Gust>) {
        self.wind_gusts = gusts;
    }
    pub(crate) fn set_spell_wind_walls(&mut self, walls: Vec<crate::wind_wall::Wall>) {
        self.wind_walls = walls;
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
            if flight.view.kind == ProjectileKind::MagicMissile {
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
            if flight.view.kind.tag() == FlightTag::Ordinary {
                for gust in &self.wind_gusts {
                    let dv = gust.arrow_drag(
                        self.elapsed as f64,
                        start.as_dvec3(),
                        Vec3::from(flight.view.vel).as_dvec3(),
                        f64::from(dt),
                    );
                    if dv.length_squared() > 0. {
                        flight.view.vel = (Vec3::from(flight.view.vel) + dv.as_vec3()).to_array();
                        flight.wind_dragged = true;
                    }
                }
            }
            let delta = Vec3::from(flight.view.vel) * dt;
            if flight.deflected || flight.wind_dragged {
                flight.view.vel[1] -= 9.81 * dt;
            }
            let radius = if flight.view.kind == ProjectileKind::SiegeBoulder {
                0.6
            } else {
                0.06
            };
            let wall = physics::kinematic::sweep_box(
                start.as_dvec3(),
                glam::DVec3::splat(radius),
                delta.as_dvec3(),
                &self.colliders,
            )?;
            // An ordinary flight whose swept segment enters a wind wall is
            // turned upward at the exact entry point.
            let wind = if flight.view.kind.tag() == FlightTag::Ordinary && !flight.deflected {
                self.wind_walls
                    .iter()
                    .filter_map(|w| w.entry(start.as_dvec3(), (start + delta).as_dvec3(), 0.))
                    .reduce(f64::min)
            } else {
                None
            };
            let mut hit: Option<(f32, u32)> = None;
            for actor in self
                .actors
                .values()
                .filter(|a| !flight.deflected && a.faction == "undead" && a.alive)
            {
                let feet_start = motion_position(actor.id, actor.pos, starts, paths, from);
                let feet_end = motion_position(actor.id, actor.pos, starts, paths, to);
                let fraction = physics::continuous::sphere_capsule(
                    start.as_dvec3(),
                    (start + delta).as_dvec3(),
                    radius,
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
            if let Some(t) = wind.filter(|t| {
                wall.is_none_or(|w| *t < w.fraction) && hit.is_none_or(|(h, _)| *t <= f64::from(h))
            }) {
                // The rest of this substep is spent at the entry point.
                let point = start + delta * t as f32;
                flight.view.vel = crate::wind_wall::deflect(Vec3::from(flight.view.vel).as_dvec3())
                    .as_vec3()
                    .to_array();
                flight.view.pos = point.to_array();
                flight.deflected = true;
                flight.target = None;
                self.counters.deflections += 1;
                self.flights.push(flight);
            } else if let Some((t, id)) =
                hit.filter(|(t, _)| wall.is_none_or(|w| f64::from(*t) < w.fraction))
            {
                self.motion_impact(
                    flight.view.caster,
                    flight.view.kind,
                    start + delta * t,
                    Some(id),
                    starts,
                    paths,
                    from + (to - from) * t,
                )?;
            } else if let Some(wall) = wall {
                if let Some(body) = self.spell_cover.get(&wall.obstacle) {
                    self.spell_hits.push((*body, flight.view.kind));
                }
                self.motion_impact(
                    flight.view.caster,
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
        self.tick_at(dt, self.elapsed + dt, position, yaw)
    }
    /// Advances on the owning world's clock rather than accumulating another one.
    pub(super) fn tick_at(
        &mut self,
        dt: f32,
        at: f32,
        position: [f32; 3],
        yaw: f32,
    ) -> Result<(), String> {
        valid_position(position)?;
        if !yaw.is_finite() {
            return Err("Invalid actor facing".into());
        }
        self.validate_step(dt, at)?;
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
        self.advance_at(dt, at)
    }
    /// Advances every player and projectile on one shared authoritative clock.
    /// Owning adapters record admitted actor paths before this call.
    pub fn advance(&mut self, dt: f32) -> Result<(), String> {
        self.advance_at(dt, self.elapsed + dt)
    }
    fn validate_step(&self, dt: f32, at: f32) -> Result<(), String> {
        let tolerance = f32::EPSILON * at.abs().max(1.) * 4.;
        if !dt.is_finite()
            || !(0. ..=0.1).contains(&dt)
            || !at.is_finite()
            || at < self.elapsed
            || ((at - self.elapsed) - dt).abs() > tolerance
        {
            return Err("Invalid world step".into());
        }
        Ok(())
    }
    fn advance_at(&mut self, dt: f32, at: f32) -> Result<(), String> {
        self.validate_step(dt, at)?;
        let began = at - dt;
        for state in self.players.values_mut() {
            let p = &mut state.resources;
            if p.hp > 0 && p.mana < p.max_mana {
                state.mana_fraction += dt;
                if state.mana_fraction >= 1. {
                    p.mana += 1;
                    state.mana_fraction -= 1.;
                }
            } else {
                state.mana_fraction = 0.;
            }
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
            self.elapsed = if to == 1. { at } else { began + dt * to };
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
    pub(crate) fn player_resources(&self, id: u32) -> Option<&Player> {
        self.players.get(&id).map(|p| &p.resources)
    }
    pub(crate) fn player_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.players.keys().copied()
    }
    pub(crate) fn live_hostiles(&self) -> usize {
        self.actors
            .values()
            .filter(|actor| actor.faction == "undead" && actor.alive)
            .count()
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot_for(0)
            .expect("The local player remains admitted")
    }
    /// Projects one player's private resources and the shared public combat state.
    pub fn snapshot_for(&self, player: u32) -> Result<Snapshot, String> {
        let mut snapshot = self.private_snapshot(player)?;
        snapshot.actors = self.actors.values().cloned().collect();
        snapshot.projectiles = self.flights.iter().map(|f| f.view.clone()).collect();
        snapshot.effects = self.effects.clone();
        Ok(snapshot)
    }
    pub(crate) fn private_snapshot(&self, player: u32) -> Result<Snapshot, String> {
        let state = self.players.get(&player).ok_or("Unknown player")?;
        let player = &state.resources;
        Ok(Snapshot {
            elapsed: self.elapsed,
            player: player.clone(),
            abilities: Spell::ALL
                .into_iter()
                .map(|id| {
                    let cooldown_remaining = (state
                        .ready
                        .get(&id)
                        .copied()
                        .unwrap_or(0.)
                        .max(state.global_ready)
                        - self.elapsed)
                        .max(0.);
                    Ability {
                        id,
                        label: id.label().into(),
                        cost: id.cost(),
                        ready: player.hp > 0
                            && player.mana >= id.cost()
                            && cooldown_remaining == 0.,
                        cooldown_remaining,
                    }
                })
                .collect(),
            actors: vec![],
            projectiles: vec![],
            effects: vec![],
            counters: self.counters.clone(),
        })
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
    fn bow_hits_first_actor_once_and_replays_in_flight() {
        let (mut s, ids) =
            Simulation::chamber([0.; 3], &[([0., 0., 2.], 20), ([0., 0., 4.], 20)]).unwrap();
        s.launch_bow([0., 1.1, 0.], [0., 0., 1.]).unwrap();
        assert_eq!(s.snapshot().projectiles[0].kind, ProjectileKind::Bow);
        let mut restored: Simulation =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        for _ in 0..10 {
            s.tick(0.1, [0.; 3], 0.).unwrap();
            restored.tick(0.1, [0.; 3], 0.).unwrap();
        }
        assert_eq!(s.actors[&ids[0]].hp, 14);
        assert_eq!(s.actors[&ids[1]].hp, 20);
        assert_eq!(s.counters.hits, 1);
        assert!(s.flights.is_empty());
        assert_eq!(
            serde_json::to_vec(&s).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
    }

    #[test]
    fn bow_stops_at_cover_inserted_after_launch() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([0., 0., 2.], 20)]).unwrap();
        s.launch_bow([0., 1.1, 0.], [0., 0., 1.]).unwrap();
        s.set_colliders(vec![physics::kinematic::Aabb {
            min: glam::DVec3::new(-1., 0., 0.9),
            max: glam::DVec3::new(1., 3., 0.91),
        }]);
        s.tick(0.1, [0.; 3], 0.).unwrap();
        assert_eq!(s.actors[&ids[0]].hp, 20);
        assert!(s.flights.is_empty());
        assert_eq!(s.effects[0].kind, 3);
        assert!(s.effects[0].pos[2] < 0.9);
    }

    #[test]
    fn bow_can_miss_a_moving_target_and_expires_without_scheduled_damage() {
        let (mut s, ids) = Simulation::chamber([0.; 3], &[([0., 0., 2.], 20)]).unwrap();
        s.launch_bow([0., 1.1, 0.], [0., 0., 1.]).unwrap();
        s.place_chamber_actor(ids[0], [4., 0., 2.], 0.).unwrap();
        for _ in 0..61 {
            s.tick(0.1, [0.; 3], 0.).unwrap();
        }
        assert_eq!(s.actors[&ids[0]].hp, 20);
        assert_eq!(s.counters.hits, 0);
        assert!(s.flights.is_empty());
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

#[cfg(test)]
mod multiplayer_tests {
    use super::*;
    fn shared() -> (Simulation, u32, u32) {
        let (mut s, hostile) = Simulation::chamber([0.; 3], &[([0., 0., 3.], 100)]).unwrap();
        let second = s.spawn_player([0., 0., 1.]).unwrap();
        (s, second, hostile[0])
    }
    fn saved(s: &Simulation) -> Vec<u8> {
        serde_json::to_vec(s).unwrap()
    }
    fn step(s: &mut Simulation, count: usize) {
        for _ in 0..count {
            s.advance(1. / 30.).unwrap();
        }
    }
    #[test]
    fn simultaneous_casts_share_targets_but_not_resources_or_cooldowns() {
        let (mut s, second, hostile) = shared();
        s.cast_for(0, Spell::MagicMissile, [0., 0., 1.]).unwrap();
        s.cast_for(second, Spell::MagicMissile, [0., 0., 1.])
            .unwrap();
        assert_eq!(s.flights.len(), 6);
        assert_eq!(s.snapshot().player.mana, 18);
        assert_eq!(s.snapshot_for(second).unwrap().player.mana, 18);
        assert!(
            s.cast_for(second, Spell::MagicMissile, [0., 0., 1.])
                .is_err()
        );
        s.spend_mana_for(0, 5).unwrap();
        step(&mut s, 40);
        assert_eq!(s.actors[&hostile].hp, 76);
        assert_eq!(s.snapshot().player.mana, 14);
        assert_eq!(s.snapshot_for(second).unwrap().player.mana, 19);
        assert_eq!(s.actors[&second].hp, 200);
        s.validate().unwrap();
    }
    #[test]
    fn cooperative_players_do_not_intercept_arrows_or_receive_area_burns() {
        let (mut s, second, hostile) = shared();
        s.launch_bow_for(0, [0., 0., 1.]).unwrap();
        step(&mut s, 10);
        assert_eq!(s.actors[&hostile].hp, 94);
        s.cast_for(0, Spell::Fireball, [0., 0., 1.]).unwrap();
        step(&mut s, 120);
        assert_eq!(s.actors[&hostile].hp, 61);
        assert_eq!(s.actors[&0].hp, 200);
        assert_eq!(s.actors[&second].hp, 200);
        assert!(s.burns.is_empty());
        assert!(s.bow_impact(second, 6).is_err());
        s.validate().unwrap();
    }
    #[test]
    fn revival_cancels_only_that_casters_flights_and_preserves_other_players() {
        let (mut s, second, _) = shared();
        s.cast_for(0, Spell::MagicMissile, [0., 0., 1.]).unwrap();
        s.cast_for(second, Spell::MagicMissile, [0., 0., 1.])
            .unwrap();
        s.player_damage_for(0, 200).unwrap();
        s.player_damage_for(second, 45).unwrap();
        let other_before = serde_json::to_vec(&s.players[&second]).unwrap();
        assert!(s.cast_for(0, Spell::Firebolt, [0., 0., 1.]).is_err());
        s.revive_player(0, [-2., 0., 0.], 0.).unwrap();
        assert_eq!(
            other_before,
            serde_json::to_vec(&s.players[&second]).unwrap()
        );
        assert_eq!(s.flights.len(), 3);
        assert!(s.flights.iter().all(|f| f.view.caster == second));
        assert_eq!(s.snapshot().player.hp, 200);
        assert_eq!(s.snapshot().player.mana, 20);
        assert_eq!(s.snapshot_for(second).unwrap().player.hp, 155);
        assert_eq!(s.release_origin(0).unwrap(), [-2., 1.1, 0.]);
        s.validate().unwrap();
    }
    #[test]
    fn revival_preserves_another_casters_existing_burns() {
        let (mut s, second, hostile) = shared();
        s.cast_for(0, Spell::Fireball, [0., 0., 1.]).unwrap();
        s.cast_for(second, Spell::Fireball, [0., 0., 1.]).unwrap();
        step(&mut s, 10);
        assert_eq!(s.burns.len(), 2);
        assert_eq!(s.actors[&hostile].hp, 70);
        s.player_damage_for(0, 200).unwrap();
        s.revive_player(0, [-2., 0., 0.], 0.).unwrap();
        assert_eq!(s.burns.len(), 1);
        assert_eq!(s.burns[0].caster, second);
        step(&mut s, 100);
        assert_eq!(s.actors[&hostile].hp, 52);
        s.validate().unwrap();
    }
    #[test]
    fn explicit_second_player_casts_obey_cover_and_moving_target_collision() {
        let (mut s, second, hostile) = shared();
        s.set_colliders(vec![physics::kinematic::Aabb {
            min: glam::DVec3::new(-1., 0., 1.9),
            max: glam::DVec3::new(1., 3., 1.91),
        }]);
        s.cast_for(second, Spell::Firebolt, [0., 0., 1.]).unwrap();
        step(&mut s, 10);
        assert_eq!(s.actors[&hostile].hp, 100);
        assert!(s.flights.is_empty());
        s.set_colliders(vec![]);
        s.place_chamber_actor(hostile, [-2., 0., 1.5], 0.).unwrap();
        s.advance(0.).unwrap();
        s.cast_for(second, Spell::Firebolt, [0., 0., 1.]).unwrap();
        s.place_chamber_actor(hostile, [2., 0., 1.5], 0.).unwrap();
        s.advance(1. / 30.).unwrap();
        assert_eq!(s.actors[&hostile].hp, 92);
        s.validate().unwrap();
    }
    #[test]
    fn shared_clock_and_pending_paths_replay_both_players_exactly() {
        let (mut s, second, _) = shared();
        s.cast_for(0, Spell::Fireball, [0., 0., 1.]).unwrap();
        s.cast_for(second, Spell::MagicMissile, [0., 0., 1.])
            .unwrap();
        s.place_chamber_actor(second, [0.1, 0., 1.], 0.2).unwrap();
        s.record_motion_path(second, vec![[0., 0., 1.], [0.05, 0., 1.], [0.1, 0., 1.]])
            .unwrap();
        let mut restored: Simulation = serde_json::from_slice(&saved(&s)).unwrap();
        restored.validate().unwrap();
        step(&mut s, 120);
        step(&mut restored, 120);
        assert_eq!(saved(&s), saved(&restored));
        assert_eq!(s.snapshot().counters.hits, 4);
        assert_eq!(s.snapshot_for(second).unwrap().player.hp, 200);
    }
    #[test]
    fn invalid_player_requests_and_capacity_refusals_preserve_the_store() {
        let (mut s, second, _) = shared();
        let before = saved(&s);
        assert!(s.cast_for(u32::MAX, Spell::Fireball, [0., 0., 1.]).is_err());
        assert!(s.launch_bow_for(u32::MAX, [0., 0., 1.]).is_err());
        assert!(
            s.cast_for(second, Spell::Fireball, [f32::NAN, 0., 1.])
                .is_err()
        );
        assert!(s.player_damage_for(u32::MAX, 5).is_err());
        assert!(s.spend_mana_for(second, 21).is_err());
        assert!(s.revive_player(second, [0.; 3], 0.).is_err());
        assert!(s.advance(f32::NAN).is_err());
        assert_eq!(before, saved(&s));
        for _ in 2..64 {
            s.spawn_player([2., 0., 0.]).unwrap();
        }
        let before = saved(&s);
        assert!(s.spawn_player([0.; 3]).is_err());
        assert_eq!(before, saved(&s));
        s.validate().unwrap();
    }
    #[test]
    fn admission_rejects_corrupt_player_and_caster_checkpoint_state() {
        let (mut s, second, _) = shared();
        s.cast_for(second, Spell::Fireball, [0., 0., 1.]).unwrap();
        let valid = serde_json::to_value(&s).unwrap();
        for path in 0..4 {
            let mut bad = valid.clone();
            match path {
                0 => bad["players"]["0"]["resources"]["mana"] = 21.into(),
                1 => bad["actors"][second.to_string()]["hp"] = 199.into(),
                2 => bad["flights"][0]["view"]["caster"] = u32::MAX.into(),
                _ => bad["flights"][0]["target"] = second.into(),
            }
            let loaded: Simulation = serde_json::from_value(bad).unwrap();
            assert!(loaded.validate().is_err());
        }
    }
    #[test]
    fn friendly_npcs_reject_damage_and_do_not_intercept_spell_flights() {
        for spell in [Spell::Firebolt, Spell::Fireball, Spell::MagicMissile] {
            let (mut s, ids) =
                Simulation::chamber([0., 0., 0.], &[([0., 0., 2.], 100), ([0., 0., 4.], 100)])
                    .unwrap();
            s.mark_friendly(ids[0]).unwrap();
            let before = saved(&s);
            assert!(s.bow_impact(ids[0], 10).is_err());
            assert_eq!(saved(&s), before);
            s.cast_for(0, spell, [0., 0., 1.]).unwrap();
            step(&mut s, 180);
            assert_eq!(s.actors[&ids[0]].hp, 100);
            assert!(s.actors[&ids[1]].hp < 100);
            s.validate().unwrap();
            let restored: Simulation = serde_json::from_slice(&saved(&s)).unwrap();
            restored.validate().unwrap();
            assert_eq!(restored.actors[&ids[0]].faction, "friendly");
        }
    }
    #[test]
    fn friendly_roles_refuse_players_late_changes_and_hostile_checkpoint_effects() {
        let (mut s, ids) = Simulation::chamber([0., 0., 0.], &[([0., 0., 4.], 100)]).unwrap();
        let before = saved(&s);
        assert!(s.mark_friendly(0).is_err());
        assert!(s.mark_friendly(u32::MAX).is_err());
        assert_eq!(saved(&s), before);
        s.cast_for(0, Spell::MagicMissile, [0., 0., 1.]).unwrap();
        let pending = saved(&s);
        assert!(s.mark_friendly(ids[0]).is_err());
        assert_eq!(saved(&s), pending);
        let mut corrupt: Simulation = serde_json::from_slice(&pending).unwrap();
        corrupt.actors.get_mut(&ids[0]).unwrap().faction = "friendly".into();
        assert!(corrupt.validate().is_err());
        corrupt.flights.clear();
        corrupt.burns.push(Burn {
            caster: 0,
            actor: ids[0],
            next: 1.,
            remaining: 1,
        });
        assert!(corrupt.validate().is_err());
        corrupt.burns.clear();
        corrupt.actors.get_mut(&ids[0]).unwrap().faction = "unknown".into();
        assert!(corrupt.validate().is_err());
        step(&mut s, 1);
        assert!(s.mark_friendly(ids[0]).is_err());
    }
}
