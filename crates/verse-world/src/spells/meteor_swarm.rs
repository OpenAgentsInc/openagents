//! Meteor Swarm in the chamber and the spell playground.
//!
//! The mechanics live in [`crate::meteor_swarm`]; this module runs them
//! through the [`SpellWorld`]. Meteors and debris are props, so they are
//! drawn, saved in checkpoints, and accounted in the momentum ledger like
//! every other body. Creatures take damage once through the chamber
//! simulation and are never moved by the spell.
//!
//! The four points are chosen for the caster: each meteor aims at the
//! largest group of visible hostiles (within 20 feet of each other) that no
//! earlier point covers, and points that would put the caster inside a
//! Sphere are skipped. When fewer than four groups remain, the remaining
//! points ring the first one. A point needs open sky: when the meteor's
//! flight to it would strike authored scenery (a ceiling, a pillar, or a
//! wall), the point is skipped, and with no usable point the cast is
//! refused with a message. A prop
//! overhead, such as an overhang or a Wall of Stone panel, does not refuse
//! it; the meteor detonates on it.
use super::{
    CHARACTER_HEIGHT, CHARACTER_RADIUS, FEET, GRAVITY, MAX_PROPS, PROP_ENTITY_BASE, SPELL_SAVE_DC,
    Save, SpellWorld, Target, Track,
};
use crate::meteor_swarm::{
    self as mechanics, Creature, Host, Impact, METEOR_MASS, METEOR_RADIUS, METEORS, MeteorSwarm,
    RADIUS, Unattended,
};
use crate::play::Game;
use glam::{DVec3, Vec3};
use physics::{BodyId, Momentum};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Meteor Swarm";
/// Row-two slot (Shift+8).
pub const SLOT: u8 = 7;
/// Chamber mana: MMO tuning, not tabletop rules.
pub const COST: i32 = 10;
/// Chamber cooldown, s: MMO tuning.
pub const COOLDOWN: f32 = 12.;
/// Hostiles within this distance of each other form one group that one
/// meteor aims at, m. Placement tuning, not an SRD rule.
pub const GROUP: f64 = 20. * FEET;
/// A point next to a creature moves this far toward the caster, so the
/// meteor lands beside it rather than on its head, m.
pub const STANDOFF: f64 = CHARACTER_RADIUS + METEOR_RADIUS + 0.25;
/// Points stay this far from the caster, so the caster is outside every
/// Sphere, m.
pub const SAFE_DISTANCE: f64 = RADIUS + 1.;
/// Smallest spacing between chosen points, m.
pub const SPACING: f64 = 2.;
/// Dexterity modifier of the player wizard: chamber tuning.
pub const PLAYER_DEXTERITY: i32 = 2;
/// Game ticks between flame cues on burning objects.
pub const FLAMES_EVERY: u64 = 12;
/// Burning objects that get a flame cue each time.
pub const MAX_FLAMES: usize = 4;
/// Small flame cues on burning objects alive at once. Each draws as about
/// seven render instances for 0.6 s, and a frame holds at most 256
/// instances, shared with characters, props, meteors, and blasts.
pub const MAX_FLAME_CUES: usize = 6;
/// Render instances the presentation draws for one falling meteor: a dark
/// core, a fire shell, a hot center, a flame trail, and smoke.
pub const METEOR_CORE_INSTANCES: usize = 3;
pub const METEOR_TRAIL_INSTANCES: usize = 7;
pub const METEOR_SMOKE_INSTANCES: usize = 2;
/// Spacing of trail flames behind a meteor, as seconds of its flight.
pub const METEOR_TRAIL_SPACING: f32 = 0.022;
/// How long a detonation's fireball shows, s.
pub const BLAST_SHOW: f32 = 1.2;
/// How long the fireball takes to swell to the Sphere's radius, s.
pub const BLAST_GROW: f32 = 0.3;
/// Render instances for one showing blast: the fireball, its flash, a
/// rising fire column, and smoke.
pub const BLAST_COLUMN_INSTANCES: usize = 5;
pub const BLAST_SMOKE_INSTANCES: usize = 3;
pub const BLAST_INSTANCES: usize = 2 + BLAST_COLUMN_INSTANCES + BLAST_SMOKE_INSTANCES;
/// Scorch decals drawn, most recent first, one instance each.
pub const MAX_SCORCHES: usize = 8;
/// How long the renderer draws an impact cue, s.
pub const CUE_LIFETIME: f32 = 0.6;
/// Detonations kept for presentation and evidence.
pub const MAX_IMPACTS: usize = 32;
/// Live casts at once.
pub const MAX_CASTS: usize = 8;
/// Presentation impact kind the renderer draws as a small flame.
const FLAME_CUE: u8 = 0;

/// One detonation as presentation draws it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blast {
    pub center: DVec3,
    /// Scene time it detonated, s.
    pub at: f32,
}

/// The action-bar entry in [`super::CATALOG`].
pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "meteor-swarm",
    label: NAME,
    icon: "meteor-swarm-icon",
    description: "Four meteors strike groups of foes: 20d6 fire and 20d6 bludgeoning in 40-foot spheres",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// Dexterity modifiers from SRD stat blocks; the Cultist has DEX 12.
pub fn dexterity_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 1,
        // Not an SRD creature: a straw training dummy has no agility.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 2,
        _ => 0,
    }
}

/// SRD object hit points for an object without its own: the resilient
/// column of the Object Hit Points table. Huge objects are tracked in
/// sections, which these props do not have, so the spell can't break them.
pub fn object_hit_points(size: super::Size) -> Option<i32> {
    match size {
        super::Size::Tiny => Some(5),
        super::Size::Small => Some(10),
        super::Size::Medium => Some(18),
        super::Size::Large => Some(27),
        super::Size::Huge => None,
    }
}

/// One cast in flight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cast {
    pub cast: u64,
    pub caster: u64,
    pub swarm: MeteorSwarm,
}

/// Meteor Swarm state saved with the spell world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub casts: Vec<Cast>,
    /// Unattended objects a Sphere has reached, with hit points and fire.
    pub objects: Vec<Unattended>,
    /// Recent detonations: the Sphere centers presentation scorches.
    pub impacts: Vec<Impact>,
    /// Game ticks the hook has run, which paces flame cues.
    pub frames: u64,
    /// Recent detonations with their scene times, for fireballs and scorch.
    #[serde(default)]
    pub blasts: Vec<Blast>,
}

impl State {
    fn idle(&self, tick: u64) -> bool {
        self.casts.is_empty() && !self.objects.iter().any(|o| o.burning(tick))
    }

    pub fn validate(&self, world: &physics::World) -> Result<(), String> {
        let bodies = world.bodies().len();
        let body = |id: BodyId| (id.0 as usize) < bodies;
        if self.casts.len() > MAX_CASTS
            || self.objects.len() > MAX_PROPS * 2
            || self.impacts.len() > MAX_IMPACTS
            || self.blasts.len() > MAX_IMPACTS
            || self
                .blasts
                .iter()
                .any(|b| !b.center.is_finite() || !b.at.is_finite())
            || self.objects.iter().any(|o| !body(o.body))
            || self.casts.iter().any(|c| {
                c.swarm.meteors.len() != METEORS
                    || c.swarm.meteors.iter().any(|m| {
                        m.body.is_some_and(|b| !body(b))
                            || !m.point.is_finite()
                            || !m.last.is_finite()
                    })
            })
        {
            return Err("Invalid Meteor Swarm checkpoint".into());
        }
        Ok(())
    }

    /// Falling meteors: center and velocity, m and m/s.
    pub fn falling(&self, world: &physics::World) -> Vec<(DVec3, DVec3)> {
        self.casts
            .iter()
            .flat_map(|c| c.swarm.falling())
            .map(|b| (world[b].pos, world[b].vel))
            .collect()
    }

    /// Whether `body` is a meteor still falling, which presentation draws
    /// as fire rather than as a prop.
    pub fn is_falling_meteor(&self, body: BodyId) -> bool {
        self.casts
            .iter()
            .any(|c| c.swarm.falling().any(|b| b == body))
    }

    /// Detonations whose fireball shows at `time`.
    pub fn showing(&self, time: f32) -> impl Iterator<Item = &Blast> + '_ {
        self.blasts
            .iter()
            .filter(move |b| (0. ..BLAST_SHOW).contains(&(time - b.at)))
    }

    /// Render instances Meteor Swarm's own presentation draws at `time`.
    pub fn instances(&self, time: f32) -> usize {
        let meteors = self
            .casts
            .iter()
            .map(|c| c.swarm.falling().count())
            .sum::<usize>();
        meteors * (METEOR_CORE_INSTANCES + METEOR_TRAIL_INSTANCES + METEOR_SMOKE_INSTANCES)
            + self.showing(time).count() * BLAST_INSTANCES
            + self.blasts.len().min(MAX_SCORCHES)
    }

    /// Centers of recent detonations, for scorch marks.
    pub fn scorches(&self) -> impl Iterator<Item = DVec3> + '_ {
        self.impacts.iter().map(|i| i.center)
    }
}

/// The spell world as the mechanics' host: meteors and debris are props.
struct Chamber<'a> {
    spells: &'a mut SpellWorld,
    query: &'a mut physics::queries::Scene,
    instance: u64,
    cast: u64,
    saves: Vec<Save>,
}

impl Chamber<'_> {
    fn index(&self, body: BodyId) -> Result<usize, String> {
        self.spells
            .props
            .iter()
            .position(|p| p.body == body)
            .ok_or_else(|| "Unknown Meteor Swarm body".to_string())
    }

    fn add(
        &mut self,
        name: &str,
        spec: super::PropSpec,
        center: DVec3,
        owner: Option<u64>,
    ) -> Result<usize, String> {
        let life = physics::queries::Life {
            instance: self.instance,
            entity: PROP_ENTITY_BASE + self.spells.props.len() as u64,
            generation: 0,
        };
        let index = self.spells.add_prop(life, name, spec, center, 0., owner)?;
        // Meteors and debris never block a character's movement.
        self.spells.props[index].passable = true;
        let prop = &self.spells.props[index];
        let half = prop.spec.dimensions * 0.5;
        let offset = -prop.spec.center_of_mass;
        self.query.insert(physics::queries::MeshCollider {
            key: prop.query_key(),
            layers: 1,
            usage: physics::queries::Usage::Trigger,
            mesh: physics::queries::Mesh::from_box(offset - half, offset + half)?,
        })?;
        Ok(index)
    }
}

impl Host for Chamber<'_> {
    fn world(&self) -> &physics::World {
        &self.spells.world
    }

    fn spawn_meteor(&mut self, start: DVec3, velocity: DVec3) -> Result<BodyId, String> {
        let spec = super::PropSpec {
            kind: super::PropKind::SpellBody,
            size: super::Size::Small,
            dimensions: DVec3::splat(2. * METEOR_RADIUS),
            mass: METEOR_MASS,
            material: super::Material::Stone,
            secured: false,
            flammable: false,
            hit_points: None,
            center_of_mass: DVec3::ZERO,
        };
        let index = self.add("Meteor", spec, start, Some(self.cast))?;
        let prop = self.spells.props[index].clone();
        // The swept test decides where it detonates; the solver never sees it.
        self.spells.world.collider_mut(prop.collider).filter = physics::Filter::NONE;
        self.spells.world[prop.body].vel = velocity;
        let momentum = Momentum::of(&self.spells.world[prop.body], self.spells.ledger.origin);
        self.spells.ledger.add(mechanics::LEDGER_TERM, momentum);
        Ok(prop.body)
    }

    fn remove_meteor(&mut self, body: BodyId) -> Result<(), String> {
        let index = self.index(body)?;
        self.spells.remove_prop(index)
    }

    fn impulse(&mut self, body: BodyId, impulse: DVec3, at: DVec3) -> Result<(), String> {
        let index = self.index(body)?;
        self.spells
            .impulse_prop(index, impulse, at, mechanics::LEDGER_TERM)
    }

    fn shatter(&mut self, body: BodyId) -> Result<Vec<BodyId>, String> {
        let index = self.index(body)?;
        let chunks = mechanics::debris(&self.spells.world, body);
        let parent = self.spells.props[index].clone();
        self.spells.remove_prop(index)?;
        if self.spells.props.len() + chunks.len() > MAX_PROPS {
            // Out of prop budget: the object is gone without debris, and the
            // ledger's "removed" term holds its momentum.
            return Ok(Vec::new());
        }
        let mut pieces = Vec::new();
        for (n, chunk) in chunks.iter().enumerate() {
            let spec = super::PropSpec {
                kind: parent.spec.kind,
                size: super::Size::Tiny,
                dimensions: chunk.half * 2.,
                mass: chunk.body.mass,
                material: parent.spec.material,
                secured: false,
                flammable: parent.spec.flammable,
                hit_points: None,
                center_of_mass: DVec3::ZERO,
            };
            let name = format!("{} debris {}", parent.name, n + 1);
            let piece = self.add(&name, spec, chunk.body.pos, None)?;
            let id = self.spells.props[piece].body;
            let body = &mut self.spells.world[id];
            body.orientation = chunk.body.orientation;
            body.prev_orientation = chunk.body.orientation;
            body.vel = chunk.body.vel;
            body.omega = chunk.body.omega;
            let momentum = Momentum::of(body, self.spells.ledger.origin);
            self.spells
                .ledger
                .add(&format!("{} debris", mechanics::LEDGER_TERM), momentum);
            pieces.push(id);
        }
        Ok(pieces)
    }

    fn damage_die(&mut self, sides: u32) -> u32 {
        self.spells.dice.roll(sides)
    }

    fn save_d20(&mut self, creature: &Creature) -> u32 {
        let save =
            self.spells
                .dice
                .save(creature.id, "Dexterity", creature.dexterity, SPELL_SAVE_DC);
        let roll = save.roll;
        self.saves.push(save);
        roll
    }
}

/// Every creature a Sphere can reach: the adventurer and living hostiles.
fn creatures(game: &Game) -> Vec<Creature> {
    let snapshot = game.snapshot();
    let mut out = Vec::new();
    if snapshot.player.hp > 0 {
        out.push(Creature {
            id: game.player_actor(),
            feet: game.player.as_dvec3(),
            radius: CHARACTER_RADIUS,
            height: CHARACTER_HEIGHT,
            dexterity: PLAYER_DEXTERITY,
        });
    }
    for (actor, id) in &game.ids {
        if let Some(a) = snapshot
            .actors
            .iter()
            .find(|a| a.id == *id && a.alive && a.faction == "undead")
        {
            let model = game
                .scene
                .actors
                .iter()
                .find(|s| s.id == *actor)
                .map_or("", |s| s.model.as_str());
            out.push(Creature {
                id: *actor,
                feet: Vec3::from(a.pos).as_dvec3(),
                radius: CHARACTER_RADIUS,
                height: CHARACTER_HEIGHT,
                dexterity: dexterity_modifier(model),
            });
        }
    }
    out
}

/// Whether a meteor reaches `point` without striking authored scenery,
/// such as a ceiling, a pillar, or a wall, on the way. Props never refuse
/// a point: a meteor that meets one detonates on it.
pub fn sky_clear(spells: &SpellWorld, caster: DVec3, point: DVec3) -> bool {
    mechanics::path_clear(&spells.world, caster, point, GRAVITY, |body| {
        !spells.props.iter().any(|p| p.body == body)
    })
}

/// Four points for a caster at `caster` facing `facing`: the largest
/// uncovered hostile groups first, then a ring around the first point.
/// `usable` says whether the caster can see a point under open sky.
///
/// # Errors
///
/// Returns why no four points could be placed.
pub fn choose_points(
    caster: DVec3,
    facing: DVec3,
    hostiles: &[(u64, DVec3)],
    usable: impl Fn(DVec3) -> bool,
) -> Result<[DVec3; METEORS], String> {
    let flat = |v: DVec3| DVec3::new(v.x, 0., v.z);
    let apart = |a: DVec3, b: DVec3| flat(a - b).length();
    let fits = |p: DVec3, points: &[DVec3]| {
        apart(p, caster) >= SAFE_DISTANCE
            && points.iter().all(|q| apart(p, *q) >= SPACING)
            && usable(p)
    };
    let mut uncovered: Vec<(u64, DVec3)> = hostiles.to_vec();
    let mut points: Vec<DVec3> = Vec::new();
    let mut blocked = false;
    while points.len() < METEORS && !uncovered.is_empty() {
        let mut best: Option<(DVec3, Vec<u64>)> = None;
        for (_, p) in &uncovered {
            let group: Vec<(u64, DVec3)> = uncovered
                .iter()
                .filter(|(_, q)| apart(*p, *q) <= GROUP)
                .copied()
                .collect();
            let mut point = group.iter().map(|(_, q)| *q).sum::<DVec3>() / group.len() as f64;
            if hostiles.iter().any(|(_, q)| apart(point, *q) < STANDOFF) {
                point += flat(caster - point).normalize_or_zero() * STANDOFF;
            }
            if !fits(point, &points) {
                blocked |= apart(point, caster) >= SAFE_DISTANCE;
                continue;
            }
            if best.as_ref().is_none_or(|(_, b)| group.len() > b.len()) {
                best = Some((point, group.iter().map(|(id, _)| *id).collect()));
            }
        }
        let Some((point, group)) = best else { break };
        uncovered.retain(|(id, q)| !group.contains(id) && apart(*q, point) > GROUP);
        points.push(point);
    }
    let Some(&anchor) = points.first() else {
        return Err(if blocked {
            "Meteor Swarm needs open sky you can see above its targets".into()
        } else {
            "No hostile in sight beyond 40 feet of you".into()
        });
    };
    let forward = flat(facing).normalize_or(DVec3::Z);
    'fill: for distance in [1.25 * RADIUS, 1.75 * RADIUS, 0.75 * RADIUS] {
        for step in 0..12 {
            if points.len() == METEORS {
                break 'fill;
            }
            let angle = step as f64 * std::f64::consts::TAU / 12.;
            let p = anchor + glam::DQuat::from_rotation_y(angle) * forward * distance;
            if fits(p, &points) {
                points.push(p);
            }
        }
    }
    points
        .try_into()
        .map_err(|_| "No room for four meteors you can see under open sky".to_string())
}

/// Casts Meteor Swarm from the adventurer's place and facing.
pub fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player.as_dvec3();
    let facing = DVec3::new(-(game.yaw as f64).sin(), 0., -(game.yaw as f64).cos());
    let eye = game.player + Vec3::Y * 1.4;
    let hostiles: Vec<(u64, DVec3)> = creatures(game)
        .into_iter()
        .filter(|c| c.id != game.player_actor() && c.feet.distance(caster) <= mechanics::RANGE)
        .filter(|c| game.attack_clear(eye, c.feet.as_vec3() + Vec3::Y * 1.1))
        .map(|c| (c.id, c.feet))
        .collect();
    let usable = |p: DVec3| {
        game.attack_clear(eye, p.as_vec3() + Vec3::Y * 0.5) && sky_clear(&game.spells, caster, p)
    };
    let points = choose_points(caster, facing, &hostiles, usable)?;
    if game.spells.meteor_swarm.casts.len() >= MAX_CASTS {
        return Err("Too many meteors in the sky".into());
    }
    let player = game.player_actor();
    let cast = game.spells.begin_cast(player, false)?;
    let instance = game.admission.actor().instance;
    let mut host = Chamber {
        spells: &mut game.spells,
        query: &mut game.query_scene,
        instance,
        cast,
        saves: Vec::new(),
    };
    let swarm = MeteorSwarm::cast(&mut host, caster, points, GRAVITY, SPELL_SAVE_DC, |_| true)?;
    let damage = swarm.damage;
    // Every object in the world when it is cast can be reached.
    register_objects(game);
    game.spells.meteor_swarm.casts.push(Cast {
        cast,
        caster: player,
        swarm,
    });
    let feet = |p: DVec3| p.distance(caster) / FEET;
    game.spells.record(
        game.time,
        NAME,
        format!(
            "Four meteors at {:.0}, {:.0}, {:.0}, {:.0} ft; rolled {} fire + {} bludgeoning",
            feet(points[0]),
            feet(points[1]),
            feet(points[2]),
            feet(points[3]),
            damage.fire,
            damage.bludgeoning
        ),
        None,
    );
    Ok(())
}

/// Adds live props the registry does not know yet, with SRD object hit
/// points and their flammability. Meteors are never objects.
fn register_objects(game: &mut Game) {
    let spells = &mut game.spells;
    let meteors: Vec<BodyId> = spells
        .meteor_swarm
        .casts
        .iter()
        .flat_map(|c| c.swarm.meteors.iter().filter_map(|m| m.body))
        .collect();
    for prop in spells.props.iter().filter(|p| !p.removed) {
        if meteors.contains(&prop.body)
            || prop.name == "Meteor"
            || spells
                .meteor_swarm
                .objects
                .iter()
                .any(|o| o.body == prop.body)
        {
            continue;
        }
        spells.meteor_swarm.objects.push(Unattended::new(
            prop.body,
            prop.hit_points
                .or_else(|| object_hit_points(prop.spec.size)),
            prop.spec.flammable,
        ));
    }
}

/// Runs after the spell world steps each game tick: sweeps and detonates
/// meteors, applies their damage, burns flammable objects, and adds the
/// flame and blast cues presentation draws.
pub(crate) fn after_step(game: &mut Game) -> Result<(), String> {
    let tick = game.spells.world.tick;
    if game.spells.meteor_swarm.idle(tick) {
        return Ok(());
    }
    let mut state = std::mem::take(&mut game.spells.meteor_swarm);
    let result = advance(game, &mut state);
    game.spells.meteor_swarm = state;
    result
}

fn advance(game: &mut Game, state: &mut State) -> Result<(), String> {
    state.frames += 1;
    let creatures = creatures(game);
    let instance = game.admission.actor().instance;
    let mut impacts = Vec::new();
    let mut saves = Vec::new();
    {
        let mut host = Chamber {
            spells: &mut game.spells,
            query: &mut game.query_scene,
            instance,
            cast: 0,
            saves: Vec::new(),
        };
        for cast in &mut state.casts {
            host.cast = cast.cast;
            impacts.extend(
                cast.swarm
                    .after_step(&mut host, &creatures, &mut state.objects)?,
            );
        }
        mechanics::burn(&mut host, &mut state.objects)?;
        saves.append(&mut host.saves);
    }
    state.casts.retain(|c| !c.swarm.finished());
    for impact in &impacts {
        resolve(game, impact, &saves)?;
        state.blasts.push(Blast {
            center: impact.center,
            at: game.time,
        });
    }
    let excess = state.blasts.len().saturating_sub(MAX_IMPACTS);
    state.blasts.drain(..excess);
    state.impacts.extend(impacts);
    let excess = state.impacts.len().saturating_sub(MAX_IMPACTS);
    state.impacts.drain(..excess);
    // Prop hit points follow the registry.
    for object in &state.objects {
        if let Some(prop) = game.spells.props.iter_mut().find(|p| p.body == object.body) {
            if object.hp.is_some() {
                prop.hit_points = object.hp;
            }
        }
    }
    state.objects.retain(|o| !game.spells.world[o.body].removed);
    cues(game, state);
    Ok(())
}

/// Applies one detonation's creature damage and writes its log lines.
fn resolve(game: &mut Game, impact: &Impact, saves: &[Save]) -> Result<(), String> {
    let name_of = |game: &Game, body: BodyId| {
        game.spells
            .props
            .iter()
            .find(|p| p.body == body)
            .map(|p| p.name.clone())
    };
    let place = if impact.obstructed {
        match impact.struck {
            Some(body) => format!(
                "early on {}",
                name_of(game, body).unwrap_or_else(|| "the scenery".into())
            ),
            None => "early on a creature".into(),
        }
    } else {
        "at its point".into()
    };
    let broke = impact.objects.iter().filter(|o| o.broke).count();
    let ignited = impact.objects.iter().filter(|o| o.ignited).count();
    game.spells.record(
        game.time,
        NAME,
        format!(
            "Meteor {} detonates {place}, {:.1} ft up: {} objects hit, {broke} broke, {ignited} ignite",
            impact.meteor + 1,
            impact.center.y / FEET,
            impact.objects.len(),
        ),
        None,
    );
    let player = game.player_actor();
    for hit in &impact.creatures {
        let save = saves.iter().rev().find(|s| s.target == hit.id).cloned();
        let name = game.actor_name(hit.id);
        let total = hit.damage.total();
        let feet = game.actor_position(hit.id);
        if hit.id == player {
            let lost = total.min(game.snapshot().player.hp);
            if lost > 0 {
                game.simulation.chamber_player_damage(lost)?;
            }
        } else if let Some(&id) = game.ids.get(&hit.id) {
            if game.snapshot().actors.iter().any(|a| a.id == id && a.alive) {
                game.simulation.bow_impact(id, total.clamp(1, 10_000))?;
            }
        }
        game.spells.record(
            game.time,
            NAME,
            format!(
                "{name}: DEX save {} {:+} = {} vs DC {} {}; {} fire + {} bludgeoning",
                hit.save.d20,
                hit.save.modifier,
                hit.save.d20 + hit.save.modifier,
                hit.save.dc,
                if hit.save.success {
                    "succeeds"
                } else {
                    "fails"
                },
                hit.damage.fire,
                hit.damage.bludgeoning
            ),
            save,
        );
        if let Some(feet) = feet {
            // Measured so the overlay and evidence show it never moved.
            game.spells.track(Track {
                label: name,
                target: Target::Actor(hit.id),
                spell: NAME.into(),
                at: game.time,
                start: feet.as_dvec3(),
                requested: 0.,
            });
        }
    }
    Ok(())
}

/// Small flame cues alive now, of every source.
pub fn live_flame_cues(game: &Game) -> usize {
    game.impacts
        .iter()
        .filter(|(_, at, kind)| *kind == FLAME_CUE && game.time - at < CUE_LIFETIME)
        .count()
}

/// Flame cues on a rotating handful of burning objects, within
/// [`MAX_FLAME_CUES`].
fn cues(game: &mut Game, state: &State) {
    let room = MAX_FLAME_CUES.saturating_sub(live_flame_cues(game));
    if state.frames % FLAMES_EVERY != 0 || room == 0 {
        return;
    }
    let tick = game.spells.world.tick;
    let burning: Vec<Vec3> = state
        .objects
        .iter()
        .filter(|o| o.burning(tick) && !game.spells.world[o.body].removed)
        .map(|o| game.spells.world[o.body].pos.as_vec3() + Vec3::Y * 0.2)
        .collect();
    if burning.is_empty() {
        return;
    }
    let start = (state.frames / FLAMES_EVERY) as usize * MAX_FLAMES % burning.len();
    for n in 0..MAX_FLAMES.min(burning.len()).min(room) {
        let at = burning[(start + n) % burning.len()];
        game.impacts.push((at, game.time, FLAME_CUE));
    }
}

/// The playground recording: four meteors over an open-sky courtyard with
/// crate stacks, a burning barrel pyramid, a wooden fence, a stone-block
/// tower, and an overhang that catches the last meteor; six dummies, two
/// in overlapping Spheres; and the second impact again at 0.25x.
pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    Scenario {
        key: "meteor-swarm",
        title: NAME,
        srd: mechanics::SRD_LINE,
        seed: 459,
        live: 10.,
        replay: (2.95, 4.45),
        setup: |scene, _| {
            for (id, name, x, z) in [
                (101, "Dummy A1 (overlap)", -4., -16.),
                (102, "Dummy A2", -9., -19.),
                (103, "Dummy B1 (overlap)", 4., -21.),
                (104, "Dummy C1", -12., 8.),
                (105, "Dummy C2", -9., 10.),
                (106, "Dummy D (overhang)", 17., 12.),
            ] {
                // Enough health to take the full damage and stay standing.
                scene
                    .actors
                    .push(creature(id, name, "dummy", Vec3::new(x, 0., z), 0., 300));
            }
            Ok(())
        },
        populate: |game, _| {
            use super::{PropKind, PropSpec};
            let crate_spec = PropSpec::reference(PropKind::Crate);
            // Props are few and large: every piece and every flame is a
            // render instance, and a frame holds at most 256.
            // A crate pyramid (3, 2, 1) beside the first point.
            for row in 0..3 {
                for i in 0..3 - row {
                    let x = (i as f32 - (2 - row) as f32 * 0.5) * 0.61;
                    game.spawn_prop(
                        &format!("Crate {}-{}", row + 1, i + 1),
                        crate_spec.clone(),
                        Vec3::new(-4.4 + x, 0.3 + 0.602 * row as f32, -19.0),
                        0.,
                    )?;
                }
            }
            // A barrel pyramid (2, 1) east of the third point.
            let barrel = PropSpec::reference(PropKind::Barrel);
            for (name, x, y) in [
                ("Barrel 1-1", 6.275, 0.45),
                ("Barrel 1-2", 6.925, 0.45),
                ("Barrel 2-1", 6.6, 1.352),
            ] {
                game.spawn_prop(name, barrel.clone(), Vec3::new(x, y, -20.3), 0.)?;
            }
            // A wooden fence of three boards west of it.
            let board = PropSpec {
                dimensions: glam::DVec3::new(0.12, 1.1, 1.2),
                mass: 16.,
                ..PropSpec::reference(PropKind::Crate)
            };
            for n in 0..3 {
                game.spawn_prop(
                    &format!("Fence board {}", n + 1),
                    board.clone(),
                    Vec3::new(1.4, 0.55, -21.6 + 1.2 * n as f32),
                    0.,
                )?;
            }
            // A tower of eight 30 cm stone bricks (65 kg each, Small) 1.5 m
            // beyond the second point, on the far side from the meteor's
            // approach. The blast throws each at about 4.5 m/s, the upper
            // bricks, farther from the center, a little less, so the tower
            // comes apart instead of sliding as one piece.
            let brick = PropSpec {
                size: super::Size::Small,
                dimensions: glam::DVec3::splat(0.3),
                mass: 65.,
                ..PropSpec::reference(PropKind::StoneBlock)
            };
            for n in 0..8 {
                game.spawn_prop(
                    &format!("Tower brick {}", n + 1),
                    brick.clone(),
                    Vec3::new(-11.8, 0.15 + 0.302 * n as f32, 9.7),
                    0.,
                )?;
            }
            // An overhang on two posts over the last dummy.
            let post = PropSpec {
                dimensions: glam::DVec3::new(0.5, 3., 0.5),
                mass: 1_500.,
                // Sturdy enough to stand through the blast.
                hit_points: Some(5_000),
                ..PropSpec::reference(PropKind::StoneBlock)
            }
            .secured();
            for (n, (x, z)) in [(14.9, 13.), (17.9, 9.8)].into_iter().enumerate() {
                game.spawn_prop(
                    &format!("Post {}", n + 1),
                    post.clone(),
                    Vec3::new(x, 1.5, z),
                    0.,
                )?;
            }
            let slab = PropSpec {
                dimensions: glam::DVec3::new(3.6, 0.4, 3.6),
                mass: 6_000.,
                size: super::Size::Large,
                hit_points: Some(5_000),
                ..PropSpec::reference(PropKind::StoneBlock)
            }
            .secured();
            game.spawn_prop("Overhang", slab, Vec3::new(16.4, 3.2, 11.4), 0.)?;
            Ok(())
        },
        script: || {
            vec![
                Cue {
                    at: 0.5,
                    step: Step::Face(0.),
                },
                Cue {
                    at: 1.0,
                    step: Step::Cast(crate::play::Ability::Spell(SLOT)),
                },
            ]
        },
        // Meteors detonate about 1.84 s after their spawn: A at 2.84 s,
        // C at 3.09 s, B at 3.34 s, and D, early on the overhang, at 3.55 s.
        camera: || {
            let shoulder = (Vec3::new(9., 3., 5.), Vec3::new(-1., 1.5, -14.));
            let sky = (Vec3::new(2., 1.8, -8.), Vec3::new(12., 50., 8.));
            let falling = (Vec3::new(1., 6., -4.), Vec3::new(2., 22., -6.));
            let south = (Vec3::new(-1., 7.5, -5.5), Vec3::new(-1.5, 1., -18.5));
            let overhang = (Vec3::new(9., 4.5, 4.), Vec3::new(16.3, 2.4, 11.4));
            let tower = (Vec3::new(-4., 3.5, 3.), Vec3::new(-11.5, 1., 9.5));
            let wide = (Vec3::new(16., 7., -10.), Vec3::new(-1., 1.5, -18.5));
            [
                (0., shoulder),
                (0.9, shoulder),
                (1.15, sky),
                (2.0, sky),
                (2.45, falling),
                (2.6, south),
                (3.44, south),
                (3.46, overhang),
                (4.9, overhang),
                (5.0, tower),
                (7.0, tower),
                (7.1, wide),
                (10., wide),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        // The second impact (C) and the tower falling, at 0.25x.
        replay_camera: (Vec3::new(-4., 3.5, 3.), Vec3::new(-11.5, 1., 9.5)),
        check: |game| {
            let state = &game.spells.meteor_swarm;
            if state.impacts.len() != METEORS {
                return Err(format!("{} meteors detonated", state.impacts.len()));
            }
            let early: Vec<_> = state.impacts.iter().filter(|i| i.obstructed).collect();
            if early.len() != 1
                || early[0].struck.is_none_or(|b| {
                    !game
                        .spells
                        .props
                        .iter()
                        .any(|p| p.body == b && p.name == "Overhang")
                })
            {
                return Err("Only the overhang should stop a meteor early".into());
            }
            let saves: Vec<u64> = game
                .spells
                .log
                .iter()
                .filter_map(|r| r.save.as_ref().map(|s| s.target))
                .collect();
            for dummy in 101..=106 {
                if saves.iter().filter(|t| **t == dummy).count() != 1 {
                    return Err(format!("Dummy {dummy} was not affected exactly once"));
                }
            }
            let overlap = [101u64, 103]
                .iter()
                .filter(|id| {
                    let feet = game.actor_position(**id).unwrap_or_default().as_dvec3();
                    state
                        .impacts
                        .iter()
                        .filter(|i| {
                            Creature {
                                id: **id,
                                feet,
                                radius: CHARACTER_RADIUS,
                                height: CHARACTER_HEIGHT,
                                dexterity: 0,
                            }
                            .closest_point(i.center)
                            .distance(i.center)
                                <= RADIUS
                        })
                        .count()
                        >= 2
                })
                .count();
            if overlap != 2 {
                return Err(format!("{overlap} dummies stood in two Spheres"));
            }
            for track in game.spells.tracks.iter().filter(|t| t.spell == NAME) {
                let moved = crate::playground::measure(game, track).map_or(0., |(_, d)| d);
                if moved > 0.05 {
                    return Err(format!("{} moved {moved:.2} m", track.label));
                }
            }
            let broke: usize = state
                .impacts
                .iter()
                .map(|i| i.objects.iter().filter(|o| o.broke).count())
                .sum();
            let ignited: usize = state
                .impacts
                .iter()
                .map(|i| i.objects.iter().filter(|o| o.ignited).count())
                .sum();
            if broke < 10 || ignited < 6 {
                return Err(format!("{broke} objects broke and {ignited} ignited"));
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE || error.angular > super::LEDGER_TOLERANCE {
                return Err(format!("Ledger residual {error:?}"));
            }
            Ok(())
        },
    }
}

#[cfg(test)]
mod tests;
