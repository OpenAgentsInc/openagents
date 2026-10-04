//! Telekinesis in the chamber authority: targeting, the cast, steering
//! input, and the per-tick drive of a held creature.
use super::{
    Applied, CENTER, HAND_SPEED, Held, NAME, RANGE, Reason, Release, Size, Target, Telekinesis,
    follow,
};
use crate::play::Game;
use crate::spells::{FEET, SPELL_SAVE_DC, Track};
use glam::DVec3;
use physics::character::{GravityOverride, TERMINAL_SPEED};

/// Half-angle of the cone ahead of the caster that a cast picks its target
/// from, rad.
pub const AIM_CONE: f64 = 0.3;
/// Eye height of the caster, m: where sight lines start.
const EYE: f64 = 1.4;
/// Bounds of the steered hand, relative to the caster: nearest reach and
/// lowest and highest lift, m.
const MIN_REACH: f64 = 0.3;
const MIN_LIFT: f64 = -3.;

/// Strength modifiers from SRD stat blocks; the Cultist has STR 11.
pub fn strength_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 0,
        // Not an SRD creature: a straw training dummy is as strong as a commoner.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 4,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Choice {
    Actor { actor: u64, center: DVec3 },
    Prop { index: usize, center: DVec3 },
}

impl Choice {
    fn held(self, game: &Game) -> Held {
        match self {
            Self::Actor { actor, .. } => Held::Actor(actor),
            Self::Prop { index, .. } => Held::Body(game.spells.props[index].body),
        }
    }
    fn center(self) -> DVec3 {
        match self {
            Self::Actor { center, .. } | Self::Prop { center, .. } => center,
        }
    }
}

/// Living hostile creatures by scene actor, with their feet.
fn creatures(game: &Game) -> Vec<(u64, DVec3)> {
    let snapshot = game.snapshot();
    game.ids
        .iter()
        .filter_map(|(actor, id)| {
            snapshot
                .actors
                .iter()
                .find(|a| a.id == *id && a.alive && a.faction == "undead")
                .map(|a| (*actor, glam::Vec3::from(a.pos).as_dvec3()))
        })
        .collect()
}

/// The creature or loose object the caster faces: the one nearest the
/// facing within [`AIM_CONE`], in range and in sight. The held target wins
/// a tie, so a re-cast renews it.
fn choose(game: &Game) -> Option<Choice> {
    let feet = game.player.as_dvec3();
    let center = feet + DVec3::Y * CENTER;
    let eye = feet + DVec3::Y * EYE;
    let yaw = f64::from(game.yaw);
    let forward = DVec3::new(-yaw.sin(), 0., -yaw.cos());
    let colliders = &game.colliders;
    let visible = |p: DVec3| {
        physics::kinematic::sweep_box(eye, DVec3::splat(0.05), p - eye, colliders)
            .is_ok_and(|hit| hit.is_none())
    };
    let off_axis = |p: DVec3| {
        let d = p - feet;
        let flat = DVec3::new(d.x, 0., d.z);
        if flat.length() < 0.3 {
            return f64::INFINITY;
        }
        flat.normalize().dot(forward).clamp(-1., 1.).acos()
    };
    let held = game
        .spells
        .telekinesis
        .get(&game.player_actor())
        .and_then(Telekinesis::held);
    let mut candidates: Vec<Choice> = creatures(game)
        .into_iter()
        .map(|(actor, feet)| Choice::Actor {
            actor,
            center: feet + DVec3::Y * CENTER,
        })
        .collect();
    candidates.extend(
        (0..game.spells.props.len())
            .filter(|i| {
                let p = &game.spells.props[*i];
                !p.removed && !p.spec.secured
            })
            .map(|index| Choice::Prop {
                index,
                center: game.spells.prop_center(index),
            }),
    );
    candidates
        .into_iter()
        .filter(|c| c.center().distance(center) <= RANGE)
        .map(|c| {
            // Nearer targets win ties along the same line of sight.
            let mut angle = off_axis(c.center()) + 1e-4 * c.center().distance(center);
            if Some(c.held(game)) == held {
                angle -= 1e-3;
            }
            (angle, c)
        })
        .filter(|(angle, c)| *angle <= AIM_CONE && visible(c.center()))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c)
}

/// Runs an admitted Telekinesis: picks the faced target, starts
/// concentration if this caster holds none, and applies the spell.
pub(crate) fn cast(game: &mut Game) -> Result<(), String> {
    if game.colliders.is_empty() {
        return Err("Telekinesis needs a collision profile".into());
    }
    let caster = game.player_actor();
    let choice = choose(game).ok_or("Nothing to move within 60 feet in front of you")?;
    let feet = game.player.as_dvec3();
    let active = game.spells.telekinesis.get(&caster).is_some_and(|t| {
        game.spells.concentration.get(&caster) == Some(&t.cast) && !t.ended(&game.spells.world)
    });
    if !active {
        if let Some(mut old) = game.spells.telekinesis.remove(&caster) {
            old.end(&mut game.spells.world);
            if let Some(character) = old
                .suspended
                .and_then(|actor| game.npc_characters.get_mut(&actor))
            {
                character.gravity = None;
            }
        }
        let cast = game.spells.begin_cast(caster, true)?;
        let gravity = DVec3::new(0., -crate::spells::GRAVITY, 0.);
        let mut tk = Telekinesis::cast(&mut game.spells.world, choice.center(), gravity);
        tk.cast = cast;
        tk.caster = caster;
        game.spells.telekinesis.insert(caster, tk);
    }
    let time = game.time;
    let mut tk = game
        .spells
        .telekinesis
        .remove(&caster)
        .ok_or("Telekinesis lost its caster")?;
    tk.origin = feet;
    tk.yaw = f64::from(game.yaw);
    let center = tk.center();
    let mut releases = vec![];
    let result = match choice {
        Choice::Actor { actor, center: at } => {
            let model = game
                .scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map(|a| a.model.clone())
                .unwrap_or_default();
            let save =
                game.spells
                    .dice
                    .save(actor, "Strength", strength_modifier(&model), SPELL_SAVE_DC);
            let size = Size::from(crate::spells::model_size(&model));
            let applied = tk.apply_creature(
                &mut game.spells.world,
                center,
                actor,
                at,
                size,
                save.success,
                &mut releases,
            );
            if let Ok(applied) = applied {
                let name = game.actor_name(actor);
                game.spells.record(
                    time,
                    NAME,
                    format!(
                        "{name}: STR save {} {:+} = {} vs DC {} {}",
                        save.roll,
                        save.modifier,
                        save.total,
                        save.dc,
                        if applied == Applied::Resisted {
                            "succeeds; not moved"
                        } else {
                            "fails; Restrained and lifted for one round"
                        }
                    ),
                    Some(save.clone()),
                );
                game.spells.track(Track {
                    label: name,
                    target: crate::spells::Target::Actor(actor),
                    spell: NAME.into(),
                    at: time,
                    start: at - DVec3::Y * CENTER,
                    requested: 0.,
                });
            }
            applied
        }
        Choice::Prop { index, center: at } => {
            let prop = game.spells.props[index].clone();
            let applied = tk.apply(
                &mut game.spells.world,
                center,
                prop.body,
                Size::from(prop.spec.size),
                Target::Object,
                false,
                &mut releases,
            );
            if applied.is_ok() {
                game.spells.record(
                    time,
                    NAME,
                    format!(
                        "{}: gripped ({:?}, {:.0} kg; no save for an unattended object)",
                        prop.name, prop.spec.size, prop.spec.mass
                    ),
                    None,
                );
                if !game
                    .spells
                    .tracks
                    .iter()
                    .any(|t| t.spell == NAME && t.target == crate::spells::Target::Prop(index))
                {
                    game.spells.track(Track {
                        label: prop.name.clone(),
                        target: crate::spells::Target::Prop(index),
                        spell: NAME.into(),
                        at: time,
                        start: at,
                        requested: 0.,
                    });
                }
            }
            applied
        }
    };
    if result == Ok(Applied::Gripped) {
        tk.aim_at_hand(&game.spells.world);
    }
    tk.released.extend(releases);
    game.spells.telekinesis.insert(caster, tk);
    report(game)?;
    result.map(|_| ()).map_err(|refusal| match refusal {
        super::Refusal::OutOfRange => "Target is beyond 60 feet".into(),
        super::Refusal::TooLarge => "Telekinesis moves Huge or smaller targets".into(),
        super::Refusal::Ended => "Concentration has ended".into(),
        super::Refusal::Immovable => "That cannot be moved".into(),
    })
}

/// Routes the caster's movement input to the hand while the hand holds
/// something and has path left; returns what still walks the caster.
/// Forward and back move the hand along the facing, strafing raises and
/// lowers it, both at [`HAND_SPEED`].
pub fn steer_input(game: &mut Game, movement: [f32; 2], dt: f32) -> [f32; 2] {
    let caster = game.player_actor();
    let Some(tk) = game.spells.telekinesis.get_mut(&caster) else {
        return movement;
    };
    if !tk.steering() {
        return movement;
    }
    let dt = f64::from(dt);
    let axes = movement.map(|v| f64::from(v.clamp(-1., 1.)));
    tk.reach = (tk.reach + axes[1] * HAND_SPEED * dt).clamp(MIN_REACH, RANGE);
    tk.lift = (tk.lift + axes[0] * HAND_SPEED * dt).clamp(MIN_LIFT, RANGE);
    [0.; 2]
}

/// Lets go of whatever the caster's hand holds; returns whether it held
/// anything. Jumping while holding lets go instead of jumping.
pub fn let_go(game: &mut Game) -> bool {
    let caster = game.player_actor();
    let Some(tk) = game.spells.telekinesis.get_mut(&caster) else {
        return false;
    };
    match tk.release(&mut game.spells.world, Reason::Let) {
        Some(release) => {
            tk.released.push(release);
            true
        }
        None => false,
    }
}

/// Before the spell world steps: refresh the caster's place and facing, and
/// end a Telekinesis whose concentration another spell or death took.
pub(crate) fn before_step(game: &mut Game) -> Result<(), String> {
    let feet = game.player.as_dvec3();
    let yaw = f64::from(game.yaw);
    let concentration = game.spells.concentration.clone();
    let world = &mut game.spells.world;
    for (caster, tk) in game.spells.telekinesis.iter_mut() {
        if *caster == game.admission.actor().actor {
            tk.origin = feet;
            tk.yaw = yaw;
        }
        if concentration.get(caster) != Some(&tk.cast) && !tk.ended(world) {
            let release = tk.end(world);
            tk.released.extend(release);
        }
    }
    Ok(())
}

/// After the spell world steps: drive held creatures toward the hand,
/// release creatures that left range or whose hold expired, restore the
/// gravity of released ones, and log what happened.
pub(crate) fn after_step(game: &mut Game, steps: usize) -> Result<(), String> {
    let dt = steps as f64 * game.physics_clock.dt;
    let living = creatures(game);
    let casters: Vec<u64> = game.spells.telekinesis.keys().copied().collect();
    for caster in casters {
        let Some(mut tk) = game.spells.telekinesis.remove(&caster) else {
            continue;
        };
        let world = &mut game.spells.world;
        if let Some(hold) = tk.creature {
            let feet = living
                .iter()
                .find(|(actor, _)| *actor == hold.actor)
                .map(|(_, feet)| *feet);
            let character = game.npc_characters.get_mut(&hold.actor);
            match (feet, character) {
                (Some(feet), Some(character)) => {
                    let center = feet + DVec3::Y * CENTER;
                    if center.distance(tk.center()) > RANGE {
                        let release = tk.release(world, Reason::OutOfRange);
                        tk.released.extend(release);
                    } else if world.tick >= hold.hold_until {
                        let release = tk.release(world, Reason::HoldExpired);
                        tk.released.extend(release);
                    } else {
                        character.gravity = Some(GravityOverride {
                            scale: 0.,
                            terminal: TERMINAL_SPEED,
                        });
                        let now = DVec3::new(
                            character.external.x,
                            character.vertical_speed,
                            character.external.z,
                        );
                        let hand = &world[tk.hand];
                        let wanted = follow(center, now, hand.pos, hand.vel, dt);
                        character.add_velocity(wanted - now);
                        tk.suspended = Some(hold.actor);
                    }
                }
                _ => {
                    let release = tk.release(world, Reason::Ended);
                    tk.released.extend(release);
                }
            }
        }
        let holding = tk.creature.map(|c| c.actor);
        if let Some(actor) = tk.suspended
            && holding != Some(actor)
        {
            if let Some(character) = game.npc_characters.get_mut(&actor) {
                character.gravity = None;
            }
            tk.suspended = None;
        }
        game.spells.telekinesis.insert(caster, tk);
    }
    report(game)?;
    let ended: Vec<u64> = game
        .spells
        .telekinesis
        .iter()
        .filter(|(_, tk)| tk.ended(&game.spells.world) && tk.released.is_empty())
        .map(|(caster, _)| *caster)
        .collect();
    for caster in ended {
        game.spells.telekinesis.remove(&caster);
    }
    Ok(())
}

/// Logs releases and a spent budget for the overlay and the evidence.
fn report(game: &mut Game) -> Result<(), String> {
    let time = game.time;
    let casters: Vec<u64> = game.spells.telekinesis.keys().copied().collect();
    for caster in casters {
        let (releases, spent, center) = {
            let tk = game
                .spells
                .telekinesis
                .get_mut(&caster)
                .ok_or("Telekinesis lost its caster")?;
            let spent = tk.held().is_some() && !tk.steering() && !tk.spent_reported;
            if spent {
                tk.spent_reported = true;
            }
            (std::mem::take(&mut tk.released), spent, tk.center())
        };
        for release in releases {
            let text = describe(game, &release, center);
            game.spells.record(time, NAME, text, None);
        }
        if spent {
            game.spells.record(
                time,
                NAME,
                "hand moved its 30 ft; it freezes and the target stays suspended".into(),
                None,
            );
        }
    }
    Ok(())
}

fn describe(game: &Game, release: &Release, caster: DVec3) -> String {
    let (name, at) = match (release.body, release.actor) {
        (Some(body), _) => (
            game.spells
                .props
                .iter()
                .find(|p| p.body == body)
                .map_or_else(|| "Body".to_string(), |p| p.name.clone()),
            Some(game.spells.world[body].pos),
        ),
        (None, Some(actor)) => (
            game.actor_name(actor),
            game.actor_position(actor)
                .map(|p| p.as_dvec3() + DVec3::Y * CENTER),
        ),
        (None, None) => ("Target".to_string(), None),
    };
    let distance = at.map_or(String::new(), |p| {
        format!(", {:.1} ft away", p.distance(caster) / FEET)
    });
    match release.reason {
        Reason::Let if release.body.is_some() => format!(
            "{name}: released at {:.1} m/s ({}{distance})",
            release.velocity.length(),
            release.reason.text()
        ),
        Reason::HoldExpired => format!("{name}: {}; it falls", release.reason.text()),
        _ => format!("{name}: released ({}{distance})", release.reason.text()),
    }
}
