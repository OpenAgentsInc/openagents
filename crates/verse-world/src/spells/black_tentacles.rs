//! Black Tentacles in the chamber and the spell playground.
//!
//! SRD 5.2.1: level 4 Conjuration, casting time Action, range 90 feet, a
//! 20-foot square of Difficult Terrain for Concentration, up to 1 minute.
//! Each creature in the square makes a Strength save on the cast, on
//! entering, and at the end of each of its turns there, once a turn; a
//! failure deals 3d6 Bludgeoning and Restrains it until the spell ends or an
//! Athletics check frees it.
//!
//! The tentacles, their saves, and their captures are
//! [`crate::black_tentacles`]. This module gives that mechanism the
//! chamber: each creature near the square has a massless kinematic body
//! that follows its character for the tentacles to wrap, the tentacles'
//! pull reaches a Restrained character as external motion, and the saves
//! and escapes roll through the spell world's seeded dice, so forced rolls
//! and checkpoints work as for every other spell.
use super::{SPELL_SAVE_DC, SpellWorld};
use crate::black_tentacles::{self as bt, BlackTentacles, Event, Kind, Roller, Test};
use crate::play::Game;
use glam::{DQuat, DVec3, Vec3};
use physics::{Body, BodyId, BodyKind, Collider, Filter, Shape};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Black Tentacles";
/// Row-two slot, Shift+7.
pub const SLOT: u8 = 6;
/// Without a selected creature ahead, the square is centered this far ahead
/// of the caster, m.
pub const AHEAD: f64 = 6.0;
/// A selected creature centers the square only within this angle of the
/// caster's facing, rad (20 degrees).
pub const AIM_CONE: f64 = 20.0 * std::f64::consts::PI / 180.0;
/// Chamber mana and cooldown: MMO tuning, not tabletop rules. Casting again
/// while concentrating on it dismisses the spell.
pub const COST: i32 = 4;
pub const COOLDOWN: f32 = 8.0;
/// Creatures this far outside the square's edge get a body for the
/// tentacles to reach for, m.
pub const TRACK_MARGIN: f64 = 3.0;
/// The agent casts it on at least this many hostiles in one square.
pub const AGENT_GROUP: usize = 2;
/// Height of a character's center above its feet, m.
const CENTER: f64 = 0.9;
/// A follower farther than this from its character moves there at once, m.
const SNAP: f64 = 2.0;
/// Collision group of the bodies that stand in for characters.
const PROXY_GROUP: u32 = 1 << 28;

/// The action-bar entry.
pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "black-tentacles",
    label: NAME,
    icon: "black-tentacles-icon",
    description: "20-ft square: tentacles grab, restrain, and drag",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// A scene creature the tentacles can reach, through the kinematic body that
/// follows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Proxy {
    pub actor: u64,
    pub body: BodyId,
    pub name: String,
    /// Strength modifier, also its Athletics bonus.
    pub strength: i32,
    pub alive: bool,
}

/// One cast of Black Tentacles in a spell world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub cast: u64,
    pub caster: u64,
    pub spell: BlackTentacles,
    pub proxies: Vec<Proxy>,
}

impl Active {
    fn proxy(&self, body: BodyId) -> Option<&Proxy> {
        self.proxies.iter().find(|p| p.body == body)
    }
    /// Whether `actor` is Restrained by this cast.
    pub fn restrained(&self, actor: u64) -> bool {
        self.proxies
            .iter()
            .any(|p| p.actor == actor && self.spell.restrained(p.body))
    }
}

/// Strength modifiers from SRD stat blocks; the Cultist has STR 11 and no
/// Athletics proficiency.
pub fn strength_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 0,
        // Not an SRD creature: a straw training dummy is as strong as a commoner.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 5,
        _ => 0,
    }
}

/// The spell world's dice as the tentacles' roller: tests roll through
/// [`super::Dice::save`] under the creature's actor, so forced rolls apply,
/// and every test is kept for the log.
struct SpellDice<'a> {
    dice: &'a mut super::Dice,
    proxies: &'a [Proxy],
    tests: Vec<super::Save>,
}

impl Roller for SpellDice<'_> {
    fn test(&mut self, body: BodyId, test: Test, modifier: i32, dc: i32) -> bt::Save {
        let actor = self
            .proxies
            .iter()
            .find(|p| p.body == body)
            .map_or(0, |p| p.actor);
        let ability = match test {
            Test::Strength => "Strength",
            Test::Athletics => "Strength (Athletics)",
        };
        let save = self.dice.save(actor, ability, modifier, dc);
        let result = bt::Save::new(save.roll as i32, modifier, dc);
        self.tests.push(save);
        result
    }
    fn roll(&mut self, sides: u32) -> u32 {
        self.dice.roll(sides)
    }
}

/// What the tentacles can act on: the living creatures' followers and every
/// prop still in the world.
fn targets(spells: &SpellWorld, active: &Active) -> Vec<bt::Target> {
    let creatures = active
        .proxies
        .iter()
        .filter(|p| p.alive)
        .map(|p| bt::Target {
            body: p.body,
            kind: Kind::Creature {
                strength: p.strength,
                athletics: p.strength,
            },
        });
    let props = spells
        .props
        .iter()
        .filter(|p| !p.removed)
        .map(|p| bt::Target {
            body: p.body,
            kind: Kind::Prop {
                secured: p.spec.secured,
            },
        });
    creatures.chain(props).collect()
}

fn trigger_text(trigger: bt::Trigger) -> &'static str {
    match trigger {
        bt::Trigger::Cast => "in the square",
        bt::Trigger::Enter => "enters",
        bt::Trigger::EndOfTurn => "ends its turn",
    }
}

/// Logs what the tentacles did and queues damage.
fn report(
    spells: &mut SpellWorld,
    active: &mut Active,
    events: Vec<Event>,
    mut tests: Vec<super::Save>,
    time: f32,
) {
    tests.reverse();
    for event in events {
        match event {
            Event::Save {
                body,
                trigger,
                save,
                damage,
            } => {
                let Some(proxy) = active.proxy(body).cloned() else {
                    continue;
                };
                let rolled = tests.pop();
                let text = format!(
                    "{} {}: STR save {} {:+} = {} vs DC {} {}",
                    proxy.name,
                    trigger_text(trigger),
                    save.d20,
                    save.modifier,
                    save.d20 + save.modifier,
                    save.dc,
                    match damage {
                        Some(d) => format!("fails; 3d6 = {d}, Restrained"),
                        None => "succeeds".into(),
                    }
                );
                spells.record(time, NAME, text, rolled);
                if let Some(d) = damage {
                    spells.tentacle_damage.push((proxy.actor, d));
                }
            }
            Event::Escape { body, check } => {
                let Some(proxy) = active.proxy(body).cloned() else {
                    continue;
                };
                let rolled = tests.pop();
                let text = format!(
                    "{}: Athletics {} {:+} = {} vs DC {} {}",
                    proxy.name,
                    check.d20,
                    check.modifier,
                    check.d20 + check.modifier,
                    check.dc,
                    if check.success {
                        "escapes"
                    } else {
                        "stays Restrained"
                    }
                );
                spells.record(time, NAME, text, rolled);
            }
            Event::Grabbed { body, tentacle } | Event::Stolen { body, tentacle } => {
                let name = spells
                    .props
                    .iter()
                    .find(|p| p.body == body)
                    .map_or("A prop".to_string(), |p| p.name.clone());
                let text = if matches!(event, Event::Grabbed { .. }) {
                    format!("Tentacle {} grabs {name} (no save)", tentacle + 1)
                } else {
                    format!("{name} is pulled free of tentacle {}", tentacle + 1)
                };
                spells.record(time, NAME, text, None);
            }
            Event::Wrapped { .. } => {}
            Event::Ended => {
                spells.record(time, NAME, "The tentacles release everything".into(), None)
            }
        }
    }
}

/// Before one fixed step of the spell world: saves, escapes, seeking, and the
/// tentacles' forces, with every impulse in the ledger.
pub(crate) fn before_step(spells: &mut SpellWorld, time: f32) {
    if spells.tentacles.is_empty() {
        return;
    }
    let mut actives = std::mem::take(&mut spells.tentacles);
    for active in &mut actives {
        let targets = targets(spells, active);
        let mut roller = SpellDice {
            dice: &mut spells.dice,
            proxies: &active.proxies,
            tests: Vec::new(),
        };
        let events = active.spell.before_step(
            &mut spells.world,
            &targets,
            &mut roller,
            Some(&mut spells.ledger),
        );
        let tests = roller.tests;
        report(spells, active, events, tests, time);
    }
    spells.tentacles = actives;
}

/// After one fixed step: root and capture impulses into the ledger, stolen
/// props, and the end of concentration after a minute.
pub(crate) fn after_step(spells: &mut SpellWorld, time: f32) {
    if spells.tentacles.is_empty() {
        return;
    }
    let mut actives = std::mem::take(&mut spells.tentacles);
    let mut ended = Vec::new();
    for active in &mut actives {
        let events = active
            .spell
            .after_step(&mut spells.world, Some(&mut spells.ledger));
        report(spells, active, events, Vec::new(), time);
        if active.spell.ended {
            ended.push(active.cast);
        }
    }
    spells.tentacles = actives;
    for cast in ended {
        spells.concentration.retain(|_, held| *held != cast);
        end(spells, cast, time);
    }
}

/// Ends a cast's tentacles: every segment, joint, and follower leaves the
/// world. Damage already owed stays queued for the game.
pub(crate) fn end(spells: &mut SpellWorld, cast: u64, time: f32) {
    let Some(index) = spells.tentacles.iter().position(|a| a.cast == cast) else {
        return;
    };
    let mut active = spells.tentacles.remove(index);
    let events = active
        .spell
        .end(&mut spells.world, Some(&mut spells.ledger));
    report(spells, &mut active, events, Vec::new(), time);
    for proxy in &active.proxies {
        spells.world.remove_body(proxy.body);
    }
}

/// Whether `actor` is Restrained by any Black Tentacles.
pub fn restrained(spells: &SpellWorld, actor: u64) -> bool {
    spells.tentacles.iter().any(|a| a.restrained(actor))
}

/// Movement speed scale for `actor` standing at `feet`: zero while
/// Restrained, halved over a square of tentacles (Difficult Terrain).
pub fn speed_scale(spells: &SpellWorld, actor: u64, feet: DVec3) -> f64 {
    if restrained(spells, actor) {
        return 0.;
    }
    spells
        .tentacles
        .iter()
        .map(|a| a.spell.speed_scale(feet))
        .fold(1., f64::min)
}

/// The square's ground point for a cast by the player, or why there is none.
fn aim(game: &Game) -> Result<DVec3, String> {
    let player = game.player.as_dvec3();
    let forward = DVec3::new(-(game.yaw.sin() as f64), 0., -(game.yaw.cos() as f64));
    let snapshot = game.snapshot();
    let selected = game.ids.get(&game.selected).and_then(|id| {
        let a = snapshot
            .actors
            .iter()
            .find(|a| a.id == *id && a.alive && a.faction == "undead")?;
        let at = Vec3::from(a.pos);
        let to = at.as_dvec3() - player;
        let flat = DVec3::new(to.x, 0., to.z);
        (flat.length() > 0.5
            && flat.normalize().angle_between(forward) <= AIM_CONE
            && to.length() <= bt::RANGE
            && game.attack_clear(game.player + Vec3::Y * 1.4, at + Vec3::Y * 1.1))
        .then_some(at.as_dvec3())
    });
    let at = selected.unwrap_or(player + forward * AHEAD);
    let colliders = &game.colliders;
    // Walkable ground: the highest collision top under a point, no higher
    // than a step above the aim.
    let ground = |x: f64, z: f64| {
        colliders
            .iter()
            .filter(|b| {
                (b.min.x..=b.max.x).contains(&x)
                    && (b.min.z..=b.max.z).contains(&z)
                    && b.max.y <= at.y + 1.0
            })
            .map(|b| b.max.y)
            .reduce(f64::max)
    };
    bt::place(player, at, ground).map_err(|refusal| {
        match refusal {
            bt::Refusal::OutOfRange => "The square is beyond 90 feet",
            bt::Refusal::NoGround => "There is no ground there",
            bt::Refusal::Uneven => "The ground there is not level enough",
        }
        .to_string()
    })
}

/// The action-bar cast: fills a 20-foot square ahead of the caster, on the
/// selected creature when it is ahead. Casting again while concentrating on
/// it dismisses it.
pub(crate) fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player_actor();
    if let Some(active) = game.spells.tentacles.iter().find(|a| a.caster == caster) {
        let cast = active.cast;
        end(&mut game.spells, cast, game.time);
        game.spells.end_cast(cast)?;
        return Ok(());
    }
    if game.colliders.is_empty() {
        return Err("Black Tentacles needs collision ground".into());
    }
    let center = aim(game)?;
    let cast = game.spells.begin_cast(caster, true)?;
    let seed = game.spells.dice.roll(u32::MAX) as u64 ^ cast;
    let (spell, events) = BlackTentacles::cast(
        &mut game.spells.world,
        center,
        DVec3::new(0., -super::GRAVITY, 0.),
        SPELL_SAVE_DC,
        seed,
        &[],
        &mut |sides: u32| sides,
    );
    debug_assert!(events.is_empty());
    game.spells.tentacles.push(Active {
        cast,
        caster,
        spell,
        proxies: Vec::new(),
    });
    game.spells.record(
        game.time,
        NAME,
        format!(
            "20-ft square at ({:.1}, {:.1}): Difficult Terrain, STR saves",
            center.x, center.z
        ),
        None,
    );
    // Creatures already in the square save on the cast.
    sync(game, 0)?;
    let time = game.time;
    let spells = &mut game.spells;
    let mut active = spells.tentacles.pop().expect("just cast");
    let targets = targets(spells, &active);
    let mut roller = SpellDice {
        dice: &mut spells.dice,
        proxies: &active.proxies,
        tests: Vec::new(),
    };
    let mut events = Vec::new();
    for target in &targets {
        if let Kind::Creature { strength, .. } = target.kind {
            events.extend(active.spell.save_on_cast(
                &spells.world,
                target.body,
                strength,
                &mut roller,
            ));
        }
    }
    let tests = roller.tests;
    report(spells, &mut active, events, tests, time);
    spells.tentacles.push(active);
    apply(game, 0.)
}

/// Before the spell world steps: give each living creature near a square a
/// follower body, and move followers to their characters over the coming
/// `steps`. Followers of dead creatures let go.
pub(crate) fn sync(game: &mut Game, steps: usize) -> Result<(), String> {
    if game.spells.tentacles.is_empty() {
        return Ok(());
    }
    let span = (steps.max(1) as f64) * game.spells.world.dt;
    let snapshot = game.snapshot();
    let creatures: Vec<(u64, DVec3, String, i32)> = game
        .scene
        .actors
        .iter()
        .filter(|a| !a.friendly)
        .filter_map(|a| {
            let id = game.ids.get(&a.id)?;
            let s = snapshot
                .actors
                .iter()
                .find(|s| s.id == *id && s.alive && s.faction == "undead")?;
            let feet = game
                .npc_characters
                .get(&a.id)
                .map_or(Vec3::from(s.pos).as_dvec3(), |c| c.feet);
            Some((a.id, feet, a.name.clone(), strength_modifier(&a.model)))
        })
        .collect();
    let spells = &mut game.spells;
    let mut actives = std::mem::take(&mut spells.tentacles);
    for active in &mut actives {
        for proxy in &mut active.proxies {
            let alive = creatures.iter().any(|c| c.0 == proxy.actor);
            if proxy.alive && !alive {
                active.spell.forget(&mut spells.world, proxy.body);
            }
            proxy.alive = alive;
        }
        for (actor, feet, name, strength) in &creatures {
            let center = *feet + DVec3::Y * CENTER;
            let near = (center - active.spell.center).abs();
            let reach = bt::SIDE / 2. + TRACK_MARGIN;
            let known = active.proxies.iter().position(|p| p.actor == *actor);
            let Some(index) = known else {
                if near.x > reach || near.z > reach || active.proxies.len() >= 64 {
                    continue;
                }
                let mut body = Body::new(0., DVec3::ONE, center).with_kind(BodyKind::Kinematic);
                body.orientation = DQuat::IDENTITY;
                let id = spells.world.add(body);
                spells.world.add_collider(
                    Collider::new(
                        id,
                        Shape::Capsule {
                            radius: super::CHARACTER_RADIUS,
                            half_length: CENTER - super::CHARACTER_RADIUS,
                        },
                    )
                    .at(DVec3::ZERO, DQuat::from_rotation_arc(DVec3::Z, DVec3::Y))
                    .with_filter(Filter {
                        group: PROXY_GROUP,
                        mask: bt::TENTACLE_GROUP,
                    }),
                );
                active.proxies.push(Proxy {
                    actor: *actor,
                    body: id,
                    name: name.clone(),
                    strength: *strength,
                    alive: true,
                });
                continue;
            };
            let body = &mut spells.world[active.proxies[index].body];
            if body.pos.distance(center) > SNAP || steps == 0 {
                body.pos = center;
                body.prev_pos = center;
                body.vel = DVec3::ZERO;
            } else {
                body.vel = (center - body.pos) / span;
            }
            body.wake();
        }
    }
    spells.tentacles = actives;
    Ok(())
}

/// After the spell world steps: damage from failed saves, and the
/// tentacles' pull on each Restrained character as external motion over
/// `dt` seconds.
pub(crate) fn apply(game: &mut Game, dt: f64) -> Result<(), String> {
    let owed = std::mem::take(&mut game.spells.tentacle_damage);
    for (actor, damage) in owed {
        let Some(id) = game.ids.get(&actor).copied() else {
            continue;
        };
        if game.snapshot().actors.iter().any(|a| a.id == id && a.alive) {
            game.simulation.bow_impact(id, damage)?;
        }
    }
    if dt <= 0. {
        return Ok(());
    }
    let mut pulls = Vec::new();
    for active in &game.spells.tentacles {
        for proxy in active.proxies.iter().filter(|p| p.alive) {
            if !active.spell.restrained(proxy.body) {
                continue;
            }
            let Some(character) = game.npc_characters.get(&proxy.actor) else {
                continue;
            };
            let pull = active.spell.pull(
                proxy.body,
                character.feet + DVec3::Y * CENTER,
                character.external,
            );
            pulls.push((proxy.actor, DVec3::new(pull.x, 0., pull.z)));
        }
    }
    for (actor, pull) in pulls {
        let mass = super::model_size(
            game.scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map_or("", |a| a.model.as_str()),
        )
        .creature_mass();
        if let Some(character) = game.npc_characters.get_mut(&actor) {
            character.add_velocity(pull / mass * dt);
        }
    }
    Ok(())
}

/// The hostile the agent should center the square on: one with at least
/// [`AGENT_GROUP`] living hostiles, the boss aside, inside a square around
/// it, within range and in sight. `None` while the agent already holds the
/// spell, or unless the game lets the agent cast it
/// ([`super::SpellWorld::agent_tentacles`]).
pub fn agent_target(game: &Game) -> Option<u64> {
    if !game.spells.agent_tentacles
        || game
            .spells
            .tentacles
            .iter()
            .any(|a| a.caster == game.player_actor())
    {
        return None;
    }
    let frame = game.frame();
    let hostiles: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| {
            a.actor.nameplate && !a.actor.friendly && a.health > 0 && a.actor.model != "claude"
        })
        .collect();
    let half = (bt::SIDE / 2.) as f32;
    hostiles
        .iter()
        .filter(|a| {
            a.actor.position.distance(game.player) <= bt::RANGE as f32
                && game.attack_clear(
                    game.player + Vec3::Y * 1.4,
                    a.actor.position + Vec3::Y * 1.1,
                )
        })
        .map(|a| {
            let count = hostiles
                .iter()
                .filter(|o| {
                    let d = o.actor.position - a.actor.position;
                    d.x.abs() <= half && d.z.abs() <= half
                })
                .count();
            (a.actor.id, count)
        })
        .filter(|(_, count)| *count >= AGENT_GROUP)
        .max_by(|(a, ac), (b, bc)| ac.cmp(bc).then(b.cmp(a)))
        .map(|(id, _)| id)
}

/// The playground recording, about 15 s live plus a 0.25x replay of the
/// first grab:
///
/// 1. The wizard casts on an empty square ahead; the tentacles rise and writhe.
/// 2. Three dummies walk in at half speed: A and B fail and are wrapped, C
///    passes on a forced roll and keeps walking.
/// 3. A Thunderwave slides a barrel into the square; tentacles grab and lift it.
/// 4. A second Thunderwave hits Restrained B: the tentacles stretch and pull
///    it back.
/// 5. B escapes on a forced Athletics roll and walks out.
/// 6. The wizard dismisses the spell; everything drops free.
pub fn scenario() -> crate::playground::Scenario {
    use crate::play::Ability;
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::FRAC_PI_2;
    /// Where the wizard stands, facing -x; the square is centered `AHEAD`
    /// meters ahead.
    const CASTER: Vec3 = Vec3::new(1.8, 0., -5.);
    /// Dummy B's first wrap point, roughly: where its path enters the square.
    const B_HELD: Vec3 = Vec3::new(-1.5, 0., -8.05);
    fn yaw_to(from: Vec3, to: Vec3) -> f32 {
        let d = to - from;
        (-d.x).atan2(-d.z)
    }
    Scenario {
        key: "black-tentacles",
        title: NAME,
        srd: "Level 4 Conjuration | Range 90 ft (20-ft square) | STR save | 3d6 Bludgeoning, Restrained",
        seed: 458,
        live: 15.,
        replay: (2.2, 4.2),
        setup: |scene, _| {
            scene.actors[0].position = CASTER;
            for (id, name, x) in [
                (101, "Dummy A", -4.2),
                (102, "Dummy B", -3.4),
                (103, "Dummy C", -6.4),
            ] {
                scene.actors.push(creature(
                    id,
                    name,
                    "dummy",
                    Vec3::new(x, 0., -10.6),
                    std::f32::consts::PI,
                    100,
                ));
            }
            Ok(())
        },
        populate: |game, _| {
            let spec = super::PropSpec::reference(super::PropKind::Barrel);
            game.spawn_prop("Barrel", spec, CASTER + Vec3::new(-2.0, 0.45, 0.), 0.)?;
            for (id, goal) in [
                (101, Vec3::new(-4.2, 0., 3.)),
                (102, Vec3::new(3., 0., -2.)),
                (103, Vec3::new(-6.4, 0., 3.)),
            ] {
                let life = game.actor_life(id).ok_or("A dummy has no life")?;
                game.direct_npc_navigation(life, goal, 1.)?;
            }
            // A fails, then fails every escape. B fails, fails its
            // Constitution save against the second wave, escapes a round
            // after it was seized, and passes the save that ends that turn.
            // C always passes.
            for (dummy, rolls) in [
                (101, &[3, 2, 2][..]),
                (102, &[4, 3, 19, 18][..]),
                (103, &[18, 18, 18][..]),
            ] {
                for roll in rolls {
                    game.spells.dice.force_save(dummy, *roll)?;
                }
            }
            Ok(())
        },
        script: || {
            let mut cues = vec![
                Cue {
                    at: 0.3,
                    step: Step::Face(FRAC_PI_2),
                },
                Cue {
                    at: 0.6,
                    step: Step::Cast(Ability::Spell(SLOT)),
                },
                // Shoves the barrel 10 ft into the square.
                Cue {
                    at: 3.6,
                    step: Step::Cast(Ability::Thunderwave),
                },
                Cue {
                    at: 7.6,
                    step: Step::Face(yaw_to(CASTER, B_HELD)),
                },
                Cue {
                    at: 8.2,
                    step: Step::Cast(Ability::Thunderwave),
                },
                Cue {
                    at: 13.,
                    step: Step::Cast(Ability::Spell(SLOT)),
                },
            ];
            // Three steps toward B, so it is inside the wave's Cube, then
            // back out of its path.
            for i in 0..3 {
                cues.push(Cue {
                    at: 7.7 + i as f32 / 30.,
                    step: Step::Move([0., 1.]),
                });
            }
            for i in 0..9 {
                cues.push(Cue {
                    at: 8.6 + i as f32 / 30.,
                    step: Step::Move([0., -1.]),
                });
            }
            cues
        },
        camera: || {
            let wide = (Vec3::new(6.0, 6.0, 3.0), Vec3::new(-4.4, 0.6, -6.3));
            let side = (Vec3::new(4.5, 4.2, -13.5), Vec3::new(-2.6, 0.6, -5.6));
            [
                (0., wide),
                (6.8, wide),
                (7.6, side),
                (11.5, side),
                (12.3, wide),
                (15., wide),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        replay_camera: (Vec3::new(3.0, 3.4, -1.5), Vec3::new(-3.0, 0.6, -8.0)),
        check: |game| {
            let log: Vec<&str> = game.spells.log.iter().map(|r| r.text.as_str()).collect();
            let has = |needle: &str| log.iter().any(|t| t.contains(needle));
            for (needle, what) in [
                ("Dummy A enters: STR save 3", "Dummy A failed on entry"),
                ("Dummy B enters: STR save 4", "Dummy B failed on entry"),
                ("Dummy C enters: STR save 18", "Dummy C passed on entry"),
                ("grabs Barrel", "a tentacle grabbed the barrel"),
                ("Dummy B: Athletics 19", "Dummy B escaped"),
                ("release everything", "the spell ended"),
            ] {
                if !has(needle) {
                    return Err(format!("Expected {what}; log: {log:?}"));
                }
            }
            if !game.spells.tentacles.is_empty() || game.spells.world.joints().next().is_some() {
                return Err("Tentacle bodies or joints outlived the spell".into());
            }
            let b = game.actor_position(102).ok_or("Dummy B is gone")?;
            if b.x < -1.152 + 0.3 {
                return Err(format!("Dummy B did not walk out: {b}"));
            }
            // B was held against the wave: it ends far short of 10 ft from
            // where the wave found it before it escaped.
            let wave = game
                .spells
                .tracks
                .iter()
                .find(|t| t.label == "Dummy B")
                .ok_or("The second wave missed Dummy B")?;
            if wave.requested <= 0. {
                return Err("Dummy B saved against the wave".into());
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
mod tests {
    use super::*;
    use crate::play::Ability;
    use crate::playground::{creature, hall};

    fn hall_game(dummies: &[(u64, Vec3)]) -> Game {
        let hall = hall().unwrap();
        let mut scene = hall.scene.clone();
        scene.actors[0].position = Vec3::new(4., 0., 0.);
        for (id, at) in dummies {
            scene
                .actors
                .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
        }
        let mut game = Game::new(scene).unwrap();
        // Facing -x: the square is centered at (-2, 0, 0), or on a selected
        // dummy straight ahead.
        game.face(std::f32::consts::FRAC_PI_2).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.).round() as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    fn cast(game: &mut Game) {
        game.activate(Ability::Spell(SLOT)).unwrap();
    }

    fn health(game: &Game, actor: u64) -> u32 {
        game.frame()
            .actors
            .iter()
            .find(|a| a.actor.id == actor)
            .unwrap()
            .health
    }

    #[test]
    fn the_catalog_puts_it_on_shift_seven() {
        let spell = crate::spells::spell_in_slot(SLOT).unwrap();
        assert_eq!(spell.key, "black-tentacles");
        assert_eq!(spell.icon, "black-tentacles-icon");
        assert_eq!(Ability::Spell(SLOT).label(), NAME);
        assert!(crate::playground::scenario(spell.key).is_some());
    }

    #[test]
    fn a_failed_save_on_the_cast_deals_three_d6_and_holds_the_dummy() {
        let at = Vec3::new(-2., 0., 0.5);
        let mut game = hall_game(&[(2, at)]);
        game.spells.dice.force_save(2, 2).unwrap();
        cast(&mut game);
        let save = game.spells.log.iter().find_map(|r| r.save.clone()).unwrap();
        assert!(!save.success && save.forced && save.roll == 2);
        let lost = 100 - health(&game, 2);
        assert!((3..=18).contains(&lost), "{lost}");
        assert!(restrained(&game.spells, 2));
        settle(&mut game, 2.);
        let active = &game.spells.tentacles[0];
        let proxy = active.proxies.iter().find(|p| p.actor == 2).unwrap();
        assert!((2..=3).contains(&active.spell.captures(proxy.body).len()));
        assert!(game.actor_position(2).unwrap().distance(at) < 0.05);
    }

    #[test]
    fn a_thunderwave_push_does_not_free_a_restrained_dummy() {
        let at = Vec3::new(0.6, 0., 0.);
        let mut game = hall_game(&[(2, at)]);
        game.spells.dice.force_save(2, 1).unwrap();
        cast(&mut game);
        settle(&mut game, 2.);
        assert!(restrained(&game.spells, 2));
        // The wizard at x = 4 faces -x: the dummy is 2.8 m into the Cube.
        game.spells.dice.force_save(2, 1).unwrap();
        game.activate(Ability::Thunderwave).unwrap();
        let mut farthest = 0f32;
        for _ in 0..90 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            farthest = farthest.max(game.actor_position(2).unwrap().distance(at));
        }
        assert!(
            farthest > 0.1 && farthest < 1.5,
            "{farthest}: free, it would slide 3 m"
        );
        assert!(restrained(&game.spells, 2));
        assert!(game.actor_position(2).unwrap().distance(at) < 0.6);
        let error = game.spells.ledger_error();
        assert!(error.linear < crate::spells::LEDGER_TOLERANCE, "{error:?}");
    }

    #[test]
    fn walking_inside_is_half_speed_and_saves_come_on_entry() {
        let start = Vec3::new(-2., 0., -6.);
        let mut game = hall_game(&[(2, start)]);
        game.spells.dice.force_save(2, 20).unwrap();
        cast(&mut game);
        let life = game.actor_life(2).unwrap();
        game.direct_npc_navigation(life, Vec3::new(-2., 0., 6.), 1.)
            .unwrap();
        settle(&mut game, 1.);
        let z0 = game.actor_position(2).unwrap().z;
        settle(&mut game, 1.);
        let outside = game.actor_position(2).unwrap().z - z0;
        assert!((outside - 1.).abs() < 0.1, "{outside}");
        while game.actor_position(2).unwrap().z < -2. {
            settle(&mut game, 0.1);
        }
        let entry = game
            .spells
            .log
            .iter()
            .find(|r| r.text.contains("enters"))
            .unwrap();
        assert!(entry.save.as_ref().unwrap().success);
        let z0 = game.actor_position(2).unwrap().z;
        settle(&mut game, 1.);
        let inside = game.actor_position(2).unwrap().z - z0;
        assert!((inside - 0.5).abs() < 0.06, "{inside}");
    }

    #[test]
    fn dismissing_removes_every_tentacle_body_and_joint_and_drops_props() {
        let mut game = hall_game(&[(2, Vec3::new(-2.5, 0., 0.5))]);
        let barrel = game
            .spawn_prop(
                "Barrel",
                super::super::PropSpec::reference(super::super::PropKind::Barrel),
                Vec3::new(-1.2, 0.45, -1.),
                0.,
            )
            .unwrap();
        game.spells.dice.force_save(2, 1).unwrap();
        cast(&mut game);
        settle(&mut game, 4.);
        assert!(game.spells.world.joints().count() > 9 * bt::SEGMENTS * 2);
        let body = game.spells.props[barrel].body;
        assert!(!game.spells.tentacles[0].spell.captures(body).is_empty());
        let (owned, _) = game.spells.tentacles[0].spell.owned();
        let followers: Vec<BodyId> = game.spells.tentacles[0]
            .proxies
            .iter()
            .map(|p| p.body)
            .collect();
        settle(&mut game, COOLDOWN);
        cast(&mut game);
        assert!(game.spells.tentacles.is_empty());
        assert_eq!(game.spells.world.joints().count(), 0);
        assert!(
            owned
                .iter()
                .chain(&followers)
                .all(|b| game.spells.world[*b].removed)
        );
        assert!(!restrained(&game.spells, 2));
        assert!(game.spells.concentration.is_empty());
        settle(&mut game, 2.);
        assert!(game.spells.prop_center(barrel).y < 0.5);
        let error = game.spells.ledger_error();
        assert!(error.linear < crate::spells::LEDGER_TOLERANCE, "{error:?}");
        assert!(error.angular < crate::spells::LEDGER_TOLERANCE, "{error:?}");
    }

    #[test]
    fn a_checkpoint_mid_grab_replays_identically() {
        let mut game = hall_game(&[(2, Vec3::new(-2., 0., 0.4)), (3, Vec3::new(-2.5, 0., -6.))]);
        game.spawn_prop(
            "Barrel",
            super::super::PropSpec::reference(super::super::PropKind::Barrel),
            Vec3::new(-1., 0.45, -1.5),
            0.,
        )
        .unwrap();
        game.spells.dice.force_save(2, 1).unwrap();
        cast(&mut game);
        let life = game.actor_life(3).unwrap();
        game.direct_npc_navigation(life, Vec3::new(-2.5, 0., 6.), 1.)
            .unwrap();
        settle(&mut game, 1.5);
        let active = &game.spells.tentacles[0];
        let held = active.proxies.iter().find(|p| p.actor == 2).unwrap().body;
        assert!(!active.spell.captures(held).is_empty());
        let saved = game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        for _ in 0..240 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }

    #[test]
    fn the_agent_aims_at_a_group_and_not_while_concentrating() {
        let mut game = hall_game(&[
            (2, Vec3::new(-2., 0., 0.5)),
            (3, Vec3::new(-3., 0., -0.5)),
            (4, Vec3::new(-14., 0., 6.)),
        ]);
        assert_eq!(agent_target(&game), None);
        game.spells.agent_tentacles = true;
        let target = agent_target(&game).unwrap();
        assert!(target == 2 || target == 3, "{target}");
        cast(&mut game);
        assert_eq!(agent_target(&game), None);
    }
}
