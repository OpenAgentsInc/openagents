//! Wall of Stone on physics.
//!
//! SRD 5.2.1: level 5 Evocation, casting time Action, range 120 feet,
//! concentration up to 10 minutes. Ten 10-by-10-foot panels 6 inches thick
//! (or 10-by-20-foot panels 3 inches thick), each contiguous with another,
//! merged with existing stone. A creature in the wall's space is pushed to
//! the side the caster chooses; one the wall would surround makes a
//! Dexterity save and on a success moves out with its reaction. Each panel
//! has AC 15 and 30 hit points per inch of thickness. Held for the full
//! duration, the wall is permanent.
//!
//! The mechanics live in [`crate::wall_of_stone`]: layouts, the placement
//! validator, joint limits, and the creature rules. This module makes each
//! panel a spell-owned prop in [`super::SpellWorld`], so the panels render,
//! carry characters, leave with concentration, and checkpoint with the
//! world. Joints break inside the fixed step when the solver holds them at
//! a limit. Standing panels are projectile cover, and a projectile that
//! stops on a panel damages it.
use super::{
    CHARACTER_HEIGHT, CHARACTER_RADIUS, FEET, Material, PROP_ENTITY_BASE, PropKind, PropSpec,
    SPELL_SAVE_DC, Size, SpellWorld, Target, Track,
};
use crate::play::Game;
use crate::wall_of_stone::creatures::{Creature, enclosed, push_out, shortest_exit};
use crate::wall_of_stone::rig::{
    BREAK_STEPS, Bond, BondKind, DEBRIS_GRIDS, FOOTING_FORCE, FOOTING_FREQUENCY, FOOTING_TORQUE,
    SEAM_FORCE, SEAM_GAP, SEAM_TORQUE, wake_near,
};
use crate::wall_of_stone::validate::{Plan, Refusal, Stone, validate};
use crate::wall_of_stone::{
    DEBRIS_LIFETIME, DURATION, DamageType, Form, Placement, SPEED, box_distance, shapes,
};
use glam::{DQuat, DVec3, Vec3};
use physics::{Body, BodyId, BodyKind, Joint, JointKind, Momentum};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Wall of Stone";
/// Row-two slot (Shift+2).
pub const SLOT: u8 = 1;
/// Chamber mana and cooldown: MMO tuning, not tabletop rules.
pub const COST: i32 = 2;
pub const COOLDOWN: f32 = 0.8;
pub const DESCRIPTION: &str =
    "Raise ten jointed stone panels that bear weight, block bolts, and break";
/// The row-two entry.
pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "wall-of-stone",
    label: NAME,
    icon: "wall-of-stone-icon",
    description: DESCRIPTION,
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};
/// Without an authored layout, the wall stands this far ahead of the
/// caster, across its facing, m.
pub const DEFAULT_DISTANCE: f64 = 4.0;
/// Panels in the default wall.
pub const DEFAULT_PANELS: usize = 3;
/// Fireball's radius against panels, m (20 feet), as against creatures.
pub const FIREBALL_RADIUS: f64 = 20. * FEET;
/// An impact this close to a panel's box struck it, m.
const STRIKE_REACH: f64 = 0.15;
/// Projectile cover approximates each panel by axis-aligned strips no longer
/// than this along the panel, m.
const COVER_STRIP: f64 = 0.8;
/// Raised walls kept for presentation and evidence.
const MAX_WALLS: usize = 32;

/// A panel layout, in world coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Layout {
    Straight {
        start: DVec3,
        direction: DVec3,
        count: usize,
        form: Form,
    },
    Bridge {
        from: DVec3,
        to: DVec3,
        lanes: usize,
        form: Form,
    },
    Ramp {
        bottom: DVec3,
        top: DVec3,
        lanes: usize,
        form: Form,
    },
    Enclosure {
        base: DVec3,
        form: Form,
    },
    Tower {
        base: DVec3,
        levels: usize,
        form: Form,
    },
}

impl Layout {
    pub fn panels(&self) -> Vec<Placement> {
        match *self {
            Self::Straight {
                start,
                direction,
                count,
                form,
            } => shapes::straight(start, direction, count.min(40), form),
            Self::Bridge {
                from,
                to,
                lanes,
                form,
            } => shapes::bridge(from, to, lanes.clamp(1, 4), form),
            Self::Ramp {
                bottom,
                top,
                lanes,
                form,
            } => shapes::ramp(bottom, top, lanes.clamp(1, 4), form),
            Self::Enclosure { base, form } => shapes::enclosure(base, form),
            Self::Tower { base, levels, form } => shapes::tower(base, levels.clamp(1, 4), form),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Straight { .. } => "wall",
            Self::Bridge { .. } => "bridge",
            Self::Ramp { .. } => "ramp",
            Self::Enclosure { .. } => "enclosure",
            Self::Tower { .. } => "tower",
        }
    }
}

/// What the next cast raises: a layout and the side creatures in its space
/// are pushed toward (a world direction).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub layout: Layout,
    pub side: DVec3,
}

/// One panel: the prop that carries it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelProp {
    pub prop: usize,
    pub form: Form,
    pub destroyed: bool,
}

/// One cast's wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Raised {
    pub cast: u64,
    pub caster: u64,
    pub layout: String,
    pub cast_at: f32,
    pub permanent: bool,
    pub vanished: bool,
    pub panels: Vec<PanelProp>,
    pub bonds: Vec<Bond>,
    /// The static anchor each footing pins to; `None` for seams.
    pub anchors: Vec<Option<BodyId>>,
    /// Debris props and when they despawn, scene seconds.
    pub debris: Vec<(usize, f32)>,
    /// Bonds that broke since the log last reported them.
    pub pending: Vec<usize>,
}

/// Wall of Stone's part of the spell world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub walls: Vec<Raised>,
    /// Authored layouts the next casts raise, in order. Trusted setup only;
    /// without one, a cast raises [`DEFAULT_PANELS`] panels ahead of the
    /// caster.
    pub queued: Vec<Order>,
}

impl State {
    /// Rejects a checkpoint whose wall state is out of bounds, refers to
    /// missing props or bodies, or holds a non-finite value.
    pub fn validate(&self, spells: &SpellWorld) -> Result<(), String> {
        let bodies = spells.world.bodies().len();
        let finite_joint = |bond: &Bond| {
            spells.world.joint(bond.joint).is_none_or(|j| {
                j.max_force.is_finite()
                    && j.max_torque.is_finite()
                    && j.impulse.is_finite()
                    && j.angular_impulse.is_finite()
                    && j.point.is_finite()
            })
        };
        if self.walls.len() > MAX_WALLS
            || self.queued.len() > 16
            || self.queued.iter().any(|o| !o.side.is_finite())
            || self.walls.iter().any(|w| {
                !w.cast_at.is_finite()
                    || w.cast > spells.casts
                    || w.panels.len() > 40
                    || w.bonds.len() != w.anchors.len()
                    || w.panels.iter().any(|p| p.prop >= spells.props.len())
                    || w.debris
                        .iter()
                        .any(|(prop, until)| *prop >= spells.props.len() || !until.is_finite())
                    || w.anchors.iter().flatten().any(|a| a.0 as usize >= bodies)
                    || w.pending.iter().any(|i| *i >= w.bonds.len())
                    || w.bonds
                        .iter()
                        .any(|b| b.broken.is_some_and(|t| !t.is_finite()) || !finite_joint(b))
            })
        {
            return Err("Invalid Wall of Stone checkpoint".into());
        }
        Ok(())
    }

    /// Queues the layout the next cast raises.
    pub fn author(&mut self, layout: Layout, side: DVec3) -> Result<(), String> {
        if self.queued.len() >= 16 || !side.is_finite() {
            return Err("Invalid Wall of Stone layout".into());
        }
        self.queued.push(Order { layout, side });
        Ok(())
    }
}

fn default_order(feet: DVec3, yaw: f32) -> Order {
    let yaw = f64::from(yaw);
    let facing = DVec3::new(-yaw.sin(), 0., -yaw.cos());
    let across = DVec3::new(-facing.z, 0., facing.x);
    let length = Form::Thick.size().x * DEFAULT_PANELS as f64;
    Order {
        layout: Layout::Straight {
            start: feet + facing * DEFAULT_DISTANCE - across * (length * 0.5),
            direction: across,
            count: DEFAULT_PANELS,
            form: Form::Thick,
        },
        side: facing,
    }
}

/// Static scene boxes and secured stone props: what a wall may merge with.
fn stone(game: &Game) -> Vec<Stone> {
    let mut out: Vec<Stone> = game
        .colliders
        .iter()
        .map(|a| Stone::aabb(a.min, a.max))
        .collect();
    for (i, p) in game.spells.props.iter().enumerate() {
        if !p.removed && p.spec.secured && p.spec.material == Material::Stone {
            out.push(Stone {
                center: game.spells.prop_center(i),
                half: p.spec.dimensions * 0.5,
                orientation: game.spells.world[p.body].orientation,
            });
        }
    }
    out
}

fn refusal(r: &Refusal) -> String {
    match r {
        Refusal::Empty => "The wall has no panels".into(),
        Refusal::OverBudget { .. } => "The wall needs more than ten panels".into(),
        Refusal::MixedForms => "Every panel must be the same size".into(),
        Refusal::OutOfRange { .. } => "The wall is out of range".into(),
        Refusal::Disconnected { .. } => "Each panel must share an edge with another".into(),
        Refusal::Unsupported => "The wall must merge with existing stone".into(),
        Refusal::SpanTooLong { span } => {
            format!("A {:.0}-foot span is longer than 20 feet", span / FEET)
        }
        Refusal::NeedsHalfPanels { .. } => {
            "A span over 20 feet needs half-size panels for supports".into()
        }
    }
}

/// Runs an admitted cast: validates the layout, pushes creatures out of its
/// space, resolves enclosure saves, then raises the panels.
pub fn cast(game: &mut Game) -> Result<(), String> {
    if game.colliders.is_empty() {
        return Err("Wall of Stone needs stone to merge with".into());
    }
    let caster = game.player_actor();
    let feet = game.player.as_dvec3();
    let order = game
        .spells
        .wall_of_stone
        .queued
        .first()
        .cloned()
        .unwrap_or_else(|| default_order(feet, game.yaw));
    let panels = order.layout.panels();
    let rock = stone(game);
    let plan = validate(&panels, &rock, feet).map_err(|r| refusal(&r))?;
    if !game.spells.wall_of_stone.queued.is_empty() {
        game.spells.wall_of_stone.queued.remove(0);
    }
    let cast = game.spells.begin_cast(caster, true)?;
    displace_creatures(game, &plan.panels, &rock, order.side)?;
    let raised = raise(game, &plan, cast, caster, order.layout.name())?;
    game.spells.record(
        game.time,
        NAME,
        format!(
            "{} raised: {} panels, {} welds, span {:.0} ft{}",
            order.layout.name(),
            raised.panels.len(),
            plan.seams.len(),
            plan.span / FEET,
            if plan.uses_supports {
                ", half-size supports"
            } else {
                ""
            }
        ),
        None,
    );
    let walls = &mut game.spells.wall_of_stone.walls;
    walls.push(raised);
    while walls.len() > MAX_WALLS {
        match walls.iter().position(|w| w.vanished) {
            Some(i) => {
                walls.remove(i);
            }
            None => break,
        }
    }
    game.spells.sync_query_poses(&mut game.query_scene)?;
    Ok(())
}

/// Dexterity modifiers from SRD stat blocks; the Cultist has DEX 12.
pub fn dexterity_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 1,
        _ => 0,
    }
}

fn sweep(game: &Game, actor: u64, from: DVec3, to: DVec3) -> Result<DVec3, String> {
    let life = game.actor_life(actor).ok_or("Unknown creature")?;
    let mut filter = physics::queries::Filter::blocking(life.instance);
    filter.ignore = Some(physics::queries::Life {
        instance: life.instance,
        entity: life.actor,
        generation: life.generation,
    });
    let delta = DVec3::new(to.x - from.x, 0., to.z - from.z);
    physics::character::slide(
        &game.query_scene,
        filter,
        physics::character::Settings::default(),
        from,
        delta,
        true,
    )
}

/// Moves a creature to `to`, which the character sweep admitted.
fn place(game: &mut Game, actor: u64, from: DVec3, to: DVec3) -> Result<(), String> {
    if actor == game.player_actor() {
        game.player = to.as_vec3();
        game.character = physics::character::Character::new(to);
        return game
            .simulation
            .place_chamber_actor(0, game.player.to_array(), game.yaw);
    }
    let id = *game.ids.get(&actor).ok_or("Unknown creature")?;
    let yaw = game
        .scene
        .actors
        .iter()
        .find(|a| a.id == actor)
        .map_or(0., |a| a.yaw);
    game.controls.displace(id, (to - from).as_vec3());
    game.simulation
        .place_chamber_actor(id, to.as_vec3().to_array(), yaw)?;
    game.npc_characters
        .insert(actor, physics::character::Character::new(to));
    Ok(())
}

/// Living creatures that could stand in the wall's space: the caster and
/// every living NPC, with their feet.
fn creatures(game: &Game) -> Vec<(u64, DVec3)> {
    let snapshot = game.snapshot();
    let mut out = vec![];
    if snapshot.player.hp > 0 {
        out.push((game.player_actor(), game.player.as_dvec3()));
    }
    for (actor, id) in &game.ids {
        if snapshot.actors.iter().any(|a| a.id == *id && a.alive) {
            if let Some(feet) = game.actor_position(*actor) {
                out.push((*actor, feet.as_dvec3()));
            }
        }
    }
    out
}

fn displace_creatures(
    game: &mut Game,
    panels: &[Placement],
    rock: &[Stone],
    side: DVec3,
) -> Result<(), String> {
    for (actor, feet) in creatures(game) {
        let name = game.actor_name(actor);
        let body = Creature {
            feet,
            radius: CHARACTER_RADIUS,
            height: CHARACTER_HEIGHT,
        };
        if let Some(wanted) = push_out(panels, body, side) {
            let to = sweep(game, actor, feet, wanted)?;
            place(game, actor, feet, to)?;
            let moved = DVec3::new(to.x - feet.x, 0., to.z - feet.z).length();
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{name} is in the wall's space: pushed {:.1} ft aside",
                    moved / FEET
                ),
                None,
            );
            game.spells.track(Track {
                label: name,
                target: Target::Actor(actor),
                spell: NAME.into(),
                at: game.time,
                start: feet,
                requested: moved,
            });
            continue;
        }
        if !enclosed(panels, rock, body) {
            continue;
        }
        let model = game
            .scene
            .actors
            .iter()
            .find(|a| a.id == actor)
            .map(|a| a.model.clone())
            .unwrap_or_default();
        let save = game.spells.dice.save(
            actor,
            "Dexterity",
            dexterity_modifier(&model),
            SPELL_SAVE_DC,
        );
        let exit = shortest_exit(panels, rock, body).filter(|(_, d)| *d <= SPEED);
        let line = |outcome: &str| {
            format!(
                "{name} enclosed: DEX save {} {:+} = {} vs DC {} {outcome}",
                save.roll, save.modifier, save.total, save.dc,
            )
        };
        match exit.filter(|_| save.success) {
            Some((wanted, distance)) => {
                let to = sweep(game, actor, feet, wanted)?;
                place(game, actor, feet, to)?;
                game.spells.record(
                    game.time,
                    NAME,
                    line(&format!("succeeds; escapes {:.1} ft", distance / FEET)),
                    Some(save.clone()),
                );
                game.spells.track(Track {
                    label: name,
                    target: Target::Actor(actor),
                    spell: NAME.into(),
                    at: game.time,
                    start: feet,
                    requested: distance,
                });
            }
            None => game.spells.record(
                game.time,
                NAME,
                line(if save.success {
                    "succeeds, but no way out"
                } else {
                    "fails; trapped"
                }),
                Some(save.clone()),
            ),
        }
    }
    Ok(())
}

fn panel_spec(form: Form) -> PropSpec {
    PropSpec {
        kind: PropKind::SpellBody,
        size: Size::Huge,
        dimensions: form.size(),
        mass: form.mass(),
        material: Material::Stone,
        secured: false,
        flammable: false,
        hit_points: Some(form.hit_points()),
        center_of_mass: DVec3::ZERO,
    }
}

/// Adds a spell-owned prop at a full orientation, with its query collider.
/// `gap` shrinks the rigid collider so welded neighbours do not also touch.
fn add_prop(
    game: &mut Game,
    name: &str,
    spec: PropSpec,
    center: DVec3,
    orientation: DQuat,
    cast: u64,
    gap: DVec3,
) -> Result<usize, String> {
    let life = physics::queries::Life {
        instance: game.player_life().instance,
        entity: PROP_ENTITY_BASE + game.spells.props.len() as u64,
        generation: 0,
    };
    let index = game
        .spells
        .add_prop(life, name, spec, center, 0., Some(cast))?;
    let prop = game.spells.props[index].clone();
    let body = &mut game.spells.world[prop.body];
    body.orientation = orientation;
    body.prev_orientation = orientation;
    let collider = game.spells.world.collider_mut(prop.collider);
    collider.shape = physics::Shape::Cuboid {
        half: (prop.spec.dimensions - gap) * 0.5,
    };
    collider.offset = -gap * 0.5;
    let half = prop.spec.dimensions * 0.5;
    game.query_scene.insert(physics::queries::MeshCollider {
        key: prop.query_key(),
        layers: 1,
        usage: physics::queries::Usage::Blocking,
        mesh: physics::queries::Mesh::from_box(-half, half)?,
    })?;
    Ok(index)
}

fn raise(
    game: &mut Game,
    plan: &Plan,
    cast: u64,
    caster: u64,
    layout: &str,
) -> Result<Raised, String> {
    let mut panels = vec![];
    for (i, p) in plan.panels.iter().enumerate() {
        let prop = add_prop(
            game,
            &format!("Stone panel {}", i + 1),
            panel_spec(p.form),
            p.center,
            p.orientation,
            cast,
            DVec3::new(SEAM_GAP, SEAM_GAP, 0.),
        )?;
        panels.push(PanelProp {
            prop,
            form: p.form,
            destroyed: false,
        });
    }
    let body = |game: &Game, i: usize| game.spells.props[panels[i].prop].body;
    let mut bonds = vec![];
    let mut anchors = vec![];
    for seam in &plan.seams {
        let (a, b) = (body(game, seam.a), body(game, seam.b));
        let joint = Joint::weld_here(&game.spells.world, a, b, seam.at)
            .limited(SEAM_FORCE * seam.length, SEAM_TORQUE * seam.length);
        bonds.push(Bond {
            kind: BondKind::Seam {
                a: seam.a,
                b: seam.b,
            },
            joint: game.spells.world.add_joint(joint),
            strained: 0,
            broken: None,
        });
        anchors.push(None);
    }
    for footing in &plan.footings {
        // A static anchor at the contact, so the pin's debug line stays at
        // the contact instead of reaching to the middle of the floor.
        let anchor = game
            .spells
            .world
            .add(Body::new(1., DVec3::ONE, footing.at).with_kind(BodyKind::Static));
        let panel = body(game, footing.panel);
        let world = &game.spells.world;
        let joint = Joint::new(
            anchor,
            DVec3::ZERO,
            panel,
            world[panel].orientation.inverse() * (footing.at - world[panel].pos),
            JointKind::Point,
        )
        .limited(FOOTING_FORCE, FOOTING_TORQUE)
        .soft(FOOTING_FREQUENCY, 1.0);
        bonds.push(Bond {
            kind: BondKind::Footing {
                panel: footing.panel,
                stone: footing.stone,
            },
            joint: game.spells.world.add_joint(joint),
            strained: 0,
            broken: None,
        });
        anchors.push(Some(anchor));
    }
    Ok(Raised {
        cast,
        caster,
        layout: layout.into(),
        cast_at: game.time,
        permanent: false,
        vanished: false,
        panels,
        bonds,
        anchors,
        debris: vec![],
        pending: vec![],
    })
}

/// After each fixed step: footing impulses go into the ledger, and joints
/// the solver held at a limit for [`BREAK_STEPS`] steps break.
pub(crate) fn after_world_step(spells: &mut SpellWorld) {
    let SpellWorld {
        world,
        ledger,
        wall_of_stone,
        ..
    } = spells;
    let slept: Vec<BodyId> = world.slept.iter().map(|(id, _)| *id).collect();
    for wall in wall_of_stone.walls.iter_mut().filter(|w| !w.vanished) {
        for (index, bond) in wall.bonds.iter_mut().enumerate() {
            if !bond.intact() {
                continue;
            }
            let Some(joint) = world.joint(bond.joint).copied() else {
                // Its panel left the world, taking the joint.
                bond.broken = Some(world.time());
                continue;
            };
            if matches!(bond.kind, BondKind::Footing { .. })
                && (world[joint.b].responds() || slept.contains(&joint.b))
            {
                // The anchor is static: the pin's impulse is external.
                ledger.add_impulse("static joint", joint.impulse, joint.point);
                ledger.add(
                    "static joint",
                    Momentum {
                        linear: DVec3::ZERO,
                        angular: joint.angular_impulse,
                    },
                );
            }
            bond.strained = if joint.saturated {
                bond.strained + 1
            } else {
                0
            };
            if bond.strained >= BREAK_STEPS {
                world.remove_joint(bond.joint);
                bond.broken = Some(world.time());
                wall.pending.push(index);
            }
        }
    }
}

fn bond_label(bond: &Bond) -> String {
    match bond.kind {
        BondKind::Seam { a, b } => format!("weld between panels {} and {}", a + 1, b + 1),
        BondKind::Footing { panel, .. } => format!("footing of panel {}", panel + 1),
    }
}

/// Once per chamber tick, before stepping: reports broken joints, removes
/// a wall whose concentration ended, despawns old debris, and makes a wall
/// held for [`DURATION`] permanent.
pub(crate) fn after_tick(spells: &mut SpellWorld, time: f32) -> Result<(), String> {
    for w in 0..spells.wall_of_stone.walls.len() {
        let wall = &mut spells.wall_of_stone.walls[w];
        if wall.vanished {
            continue;
        }
        let pending = std::mem::take(&mut wall.pending);
        let lines: Vec<String> = pending
            .iter()
            .map(|&i| format!("{} broke under load", bond_label(&wall.bonds[i])))
            .collect();
        for line in lines {
            spells.record(time, NAME, line, None);
        }
        let wall = &spells.wall_of_stone.walls[w];
        let (cast, cast_at) = (wall.cast, wall.cast_at);
        let held = wall.permanent || spells.concentration.values().any(|c| *c == cast);
        if !held {
            vanish(spells, w)?;
            spells.record(
                time,
                NAME,
                "Concentration ends: the wall vanishes".into(),
                None,
            );
            continue;
        }
        // Panels another spell removed are destroyed.
        let wall = &mut spells.wall_of_stone.walls[w];
        for panel in &mut wall.panels {
            if !panel.destroyed && spells.props[panel.prop].removed {
                panel.destroyed = true;
            }
        }
        let expired: Vec<usize> = wall
            .debris
            .iter()
            .filter(|(prop, until)| time >= *until && !spells.props[*prop].removed)
            .map(|(prop, _)| *prop)
            .collect();
        for prop in expired {
            spells.remove_prop(prop)?;
        }
        if !spells.wall_of_stone.walls[w].permanent && time - cast_at >= DURATION as f32 {
            let wall = &mut spells.wall_of_stone.walls[w];
            wall.permanent = true;
            let props: Vec<usize> = wall.panels.iter().map(|p| p.prop).collect();
            spells.concentration.retain(|_, c| *c != cast);
            for prop in props {
                spells.props[prop].owner = None;
            }
            spells.record(
                time,
                NAME,
                "Concentration held 10 minutes: the wall is permanent".into(),
                None,
            );
        }
    }
    Ok(())
}

fn vanish(spells: &mut SpellWorld, w: usize) -> Result<(), String> {
    let wall = spells.wall_of_stone.walls[w].clone();
    for panel in &wall.panels {
        let body = spells.props[panel.prop].body;
        if !spells.props[panel.prop].removed {
            spells.remove_prop(panel.prop)?;
        }
        // What rested on the panel falls.
        wake_near(&mut spells.world, body);
    }
    for (prop, _) in &wall.debris {
        if !spells.props[*prop].removed {
            spells.remove_prop(*prop)?;
        }
    }
    for anchor in wall.anchors.iter().flatten() {
        spells.world.remove_body(*anchor);
    }
    let time = spells.world.time();
    let wall = &mut spells.wall_of_stone.walls[w];
    for bond in &mut wall.bonds {
        if bond.intact() {
            bond.broken = Some(time);
        }
    }
    wall.vanished = true;
    Ok(())
}

/// Axis-aligned strips covering every standing panel, for projectile cover.
pub fn cover(spells: &SpellWorld) -> Vec<physics::kinematic::Aabb> {
    let mut out = vec![];
    for wall in spells.wall_of_stone.walls.iter().filter(|w| !w.vanished) {
        for panel in &wall.panels {
            let prop = &spells.props[panel.prop];
            if prop.removed {
                continue;
            }
            let body = &spells.world[prop.body];
            let half = prop.spec.dimensions * 0.5;
            let strips = (prop.spec.dimensions.x / COVER_STRIP).ceil().max(1.) as usize;
            for s in 0..strips {
                let x0 = -half.x + prop.spec.dimensions.x * s as f64 / strips as f64;
                let x1 = -half.x + prop.spec.dimensions.x * (s + 1) as f64 / strips as f64;
                let mut min = DVec3::splat(f64::INFINITY);
                let mut max = DVec3::splat(f64::NEG_INFINITY);
                for corner in 0..8 {
                    let local = DVec3::new(
                        if corner & 1 == 0 { x0 } else { x1 },
                        if corner & 2 == 0 { -half.y } else { half.y },
                        if corner & 4 == 0 { -half.z } else { half.z },
                    );
                    let p = body.pos + body.orientation * local;
                    min = min.min(p);
                    max = max.max(p);
                }
                out.push(physics::kinematic::Aabb { min, max });
            }
        }
    }
    out
}

/// Before projectiles fly: standing panels join the static cover they stop
/// at.
pub(crate) fn before_projectiles(game: &mut Game) {
    let mut boxes = game.colliders.clone();
    boxes.extend(cover(&game.spells));
    game.simulation.set_colliders(boxes);
}

/// After projectiles fly: an impact on a panel damages it. Fireball damages
/// every panel within its radius.
pub(crate) fn after_projectiles(game: &mut Game) -> Result<(), String> {
    if game.spells.wall_of_stone.walls.iter().all(|w| w.vanished) {
        return Ok(());
    }
    for effect in game.snapshot().effects {
        let point = Vec3::from(effect.pos).as_dvec3();
        // The chamber profile's projectile damage, as against creatures.
        let (damage, kind, source) = match effect.kind {
            0 => (8, DamageType::Fire, "Fire Bolt"),
            1 => (15, DamageType::Fire, "Fireball"),
            2 => (4, DamageType::Force, "Magic Missile"),
            3 => (6, DamageType::Piercing, "Arrow"),
            _ => continue,
        };
        if effect.kind == 1 {
            for (w, p) in standing(game) {
                let placement = pose(game, w, p);
                if box_distance(
                    placement.center,
                    placement.orientation,
                    placement.half(),
                    point,
                ) <= FIREBALL_RADIUS
                {
                    damage_panel(game, w, p, damage, kind, source)?;
                }
            }
            continue;
        }
        struck(game, point, damage, kind, source)?;
    }
    Ok(())
}

fn standing(game: &Game) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for (w, wall) in game.spells.wall_of_stone.walls.iter().enumerate() {
        if wall.vanished {
            continue;
        }
        for (p, panel) in wall.panels.iter().enumerate() {
            if !panel.destroyed && !game.spells.props[panel.prop].removed {
                out.push((w, p));
            }
        }
    }
    out
}

fn pose(game: &Game, w: usize, p: usize) -> Placement {
    let panel = game.spells.wall_of_stone.walls[w].panels[p];
    let body = &game.spells.world[game.spells.props[panel.prop].body];
    Placement {
        form: panel.form,
        center: body.pos,
        orientation: body.orientation,
    }
}

/// A projectile stopped at `point`: the nearest standing panel within reach
/// takes its damage. Returns whether a panel was struck.
pub fn struck(
    game: &mut Game,
    point: DVec3,
    damage: i32,
    kind: DamageType,
    source: &str,
) -> Result<bool, String> {
    let nearest = standing(game)
        .into_iter()
        .map(|(w, p)| {
            let placement = pose(game, w, p);
            (
                box_distance(
                    placement.center,
                    placement.orientation,
                    placement.half(),
                    point,
                ),
                w,
                p,
            )
        })
        .filter(|(d, _, _)| *d <= STRIKE_REACH)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, w, p)) = nearest else {
        return Ok(false);
    };
    damage_panel(game, w, p, damage, kind, source)?;
    Ok(true)
}

/// Damages a panel; at 0 hit points it breaks into debris. Poison and
/// Psychic do nothing.
pub fn damage_panel(
    game: &mut Game,
    w: usize,
    p: usize,
    amount: i32,
    kind: DamageType,
    source: &str,
) -> Result<(), String> {
    let panel = game.spells.wall_of_stone.walls[w].panels[p];
    if panel.destroyed || !kind.harms_panels() || amount <= 0 {
        return Ok(());
    }
    let prop = &mut game.spells.props[panel.prop];
    let left = (prop.hit_points.unwrap_or(panel.form.hit_points()) - amount).max(0);
    prop.hit_points = Some(left);
    game.spells.record(
        game.time,
        NAME,
        format!(
            "{source} hits panel {}: {amount} {}, {left}/{} HP",
            p + 1,
            format!("{kind:?}").to_lowercase(),
            panel.form.hit_points()
        ),
        None,
    );
    if left == 0 {
        destroy(game, w, p)?;
    }
    Ok(())
}

/// Removes a panel and its joints and breaks it into 4 to 8 chunks with
/// the panel's total mass and momentum, each gone after
/// [`DEBRIS_LIFETIME`].
pub fn destroy(game: &mut Game, w: usize, p: usize) -> Result<(), String> {
    let time = game.time;
    let wall = &mut game.spells.wall_of_stone.walls[w];
    let panel = wall.panels[p];
    wall.panels[p].destroyed = true;
    let cast = wall.cast;
    let id = game.spells.props[panel.prop].body;
    let body = game.spells.world[id];
    wake_near(&mut game.spells.world, id);
    let now = game.spells.world.time();
    let wall = &mut game.spells.wall_of_stone.walls[w];
    for bond in &mut wall.bonds {
        if bond.intact() && bond.touches(p) {
            game.spells.world.remove_joint(bond.joint);
            bond.broken = Some(now);
        }
    }
    for (bond, anchor) in wall.bonds.iter().zip(&wall.anchors) {
        if let (BondKind::Footing { panel: owner, .. }, Some(anchor)) = (bond.kind, anchor) {
            if owner == p {
                game.spells.world.remove_body(*anchor);
            }
        }
    }
    game.spells.remove_prop(panel.prop)?;
    let (nx, ny) = DEBRIS_GRIDS[(game.spells.dice.roll(3) - 1) as usize % DEBRIS_GRIDS.len()];
    let size = panel.form.size();
    let chunk = DVec3::new(size.x / nx as f64, size.y / ny as f64, size.z);
    let mass = body.mass / (nx * ny) as f64;
    let omega = body.omega_world();
    let mut count = 0;
    for j in 0..ny {
        for i in 0..nx {
            let local = DVec3::new(
                -size.x * 0.5 + chunk.x * (i as f64 + 0.5),
                -size.y * 0.5 + chunk.y * (j as f64 + 0.5),
                0.,
            );
            let pos = body.to_world(local);
            let spec = PropSpec {
                kind: PropKind::SpellBody,
                size: Size::Medium,
                dimensions: chunk,
                mass,
                material: Material::Stone,
                secured: false,
                flammable: false,
                hit_points: None,
                center_of_mass: DVec3::ZERO,
            };
            let index = add_prop(
                game,
                &format!("Panel {} debris", p + 1),
                spec,
                pos,
                body.orientation,
                cast,
                chunk * 0.04,
            )?;
            let piece = game.spells.props[index].body;
            let b = &mut game.spells.world[piece];
            b.vel = body.vel + omega.cross(pos - body.pos);
            b.omega = body.omega;
            let origin = game.spells.ledger.origin;
            let gained = Momentum::of(&game.spells.world[piece], origin);
            game.spells.ledger.add("spell:wall of stone", gained);
            game.spells.wall_of_stone.walls[w]
                .debris
                .push((index, time + DEBRIS_LIFETIME as f32));
            count += 1;
        }
    }
    game.spells.record(
        time,
        NAME,
        format!("Panel {} destroyed: {count} chunks of debris", p + 1),
        None,
    );
    game.spells.sync_query_poses(&mut game.query_scene)?;
    Ok(())
}

/// The playground recording.
///
/// 1. A bridge over the 3 m chasm, and the wizard walks across.
/// 2. A ramp of half-size panels on supports up to the 6 m ledge, and the
///    wizard walks up it.
/// 3. A five-panel bridge over a 20-foot gap between two stone blocks; Fire
///    Bolts from the ledge break its middle panel, and the halves tip off
///    their abutments as their footings tear (joint lines turn red at a
///    limit).
/// 4. A wall raised through a dummy's space pushes it to the chosen side.
/// 5. An enclosure around a dummy: a forced Dexterity save, and it escapes.
/// 6. A tower beside the ledge; the wizard steps onto its roof and casts
///    again, which ends concentration: the tower vanishes and the wizard
///    falls 20 feet (2d6).
/// 7. The collapse again at 0.25×.
pub fn scenario() -> crate::playground::Scenario {
    use crate::play::Ability;
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::FRAC_PI_2;
    Scenario {
        key: "wall-of-stone",
        title: NAME,
        srd: "Level 5 Evocation | Range 120 ft | ten 10x10 ft panels, 6 in, AC 15, 180 HP | DEX save if enclosed | Concentration, 10 min",
        seed: 453,
        live: 16.5,
        replay: (10.0, 11.3),
        setup: |scene, _| {
            let wizard = &mut scene.actors[0];
            wizard.position = Vec3::new(-4.5, 0., 20.);
            wizard.yaw = 0.;
            for (id, name, at) in [
                // Beside the bridge, clear of where its halves and debris
                // fall, so the bolts' line to it crosses the middle panel.
                (104, "Dummy (target)", Vec3::new(-7.16, 0., -5.0)),
                (105, "Dummy (wall)", Vec3::new(1.0, 0., 6.75)),
                (106, "Dummy (enclosed)", Vec3::new(4.0, 0., 11.0)),
            ] {
                scene
                    .actors
                    .push(creature(id, name, "dummy", at, FRAC_PI_2, 100));
            }
            Ok(())
        },
        populate: |game, _| {
            // Two stone blocks 20 feet apart, 2 m tall.
            for (name, min_x, max_x) in [
                ("West abutment", -14.0f32, -11.812f32),
                ("East abutment", -5.716, -3.5),
            ] {
                let mut spec = PropSpec::reference(PropKind::StoneBlock).secured();
                spec.dimensions = DVec3::new(f64::from(max_x - min_x), 2.0, 3.4);
                spec.mass = 2_700. * spec.dimensions.x * spec.dimensions.y * spec.dimensions.z;
                game.spawn_prop(name, spec, Vec3::new((min_x + max_x) * 0.5, 1.0, -2.6), 0.)?;
            }
            let orders = [
                (
                    Layout::Bridge {
                        from: DVec3::new(-4.5, 0., 18.548),
                        to: DVec3::new(-4.5, 0., 12.452),
                        lanes: 1,
                        form: Form::Thick,
                    },
                    DVec3::X,
                ),
                (
                    Layout::Ramp {
                        bottom: DVec3::new(-5.6, 0., 2.5),
                        top: DVec3::new(-16., 6., 2.5),
                        lanes: 2,
                        form: Form::Half,
                    },
                    DVec3::X,
                ),
                (
                    Layout::Bridge {
                        from: DVec3::new(-12.574, 2.0, -2.6),
                        to: DVec3::new(-4.954, 2.0, -2.6),
                        lanes: 1,
                        form: Form::HalfThin,
                    },
                    DVec3::Z,
                ),
                (
                    Layout::Straight {
                        start: DVec3::new(-1.048, 0., 7.05),
                        direction: DVec3::X,
                        count: 2,
                        form: Form::Thick,
                    },
                    DVec3::Z,
                ),
                (
                    Layout::Enclosure {
                        base: DVec3::new(4.0, 0., 11.0),
                        form: Form::Thick,
                    },
                    DVec3::X,
                ),
                (
                    Layout::Tower {
                        base: DVec3::new(-14.4, 0., 2.5),
                        levels: 2,
                        form: Form::Thick,
                    },
                    DVec3::X,
                ),
                (
                    Layout::Straight {
                        start: DVec3::new(6.0, 0., -6.0),
                        direction: DVec3::X,
                        count: 1,
                        form: Form::Half,
                    },
                    DVec3::Z,
                ),
            ];
            for (layout, side) in orders {
                game.spells.wall_of_stone.author(layout, side)?;
            }
            game.spells.dice.force_save(106, 15)?;
            game.selected = 104;
            Ok(())
        },
        script: || {
            let mut cues = vec![];
            let at = |cues: &mut Vec<Cue>, at: f32, step: Step| cues.push(Cue { at, step });
            let walk = |cues: &mut Vec<Cue>, from: f32, until: f32| {
                let mut t = from;
                while t < until {
                    cues.push(Cue {
                        at: t,
                        step: Step::Move([0., 1.]),
                    });
                    t += 1. / 30.;
                }
            };
            let spell = Step::Cast(Ability::Spell(SLOT));
            // 1. Bridge the chasm and cross it.
            at(&mut cues, 0.1, Step::Face(0.));
            at(&mut cues, 0.3, spell);
            walk(&mut cues, 0.4, 3.13);
            // 2. Ramp to the ledge and climb it.
            at(&mut cues, 1.6, spell);
            at(&mut cues, 3.25, Step::Face(FRAC_PI_2));
            walk(&mut cues, 3.3, 5.35);
            // 3. A bridge between the blocks; Fire Bolts at its middle.
            at(&mut cues, 5.5, spell);
            for k in 0..14 {
                at(
                    &mut cues,
                    6.0 + k as f32 / 3.,
                    Step::Cast(Ability::FireBolt),
                );
            }
            // 4. A wall through a dummy; 5. an enclosure; 6. a tower.
            at(&mut cues, 11.6, spell);
            at(&mut cues, 12.6, spell);
            at(&mut cues, 13.6, spell);
            at(&mut cues, 13.8, Step::Face(-FRAC_PI_2));
            walk(&mut cues, 13.85, 14.35);
            // Casting again ends concentration on the tower.
            at(&mut cues, 14.9, spell);
            cues
        },
        camera: || {
            [
                (0., (6.0, 4.5, 23.0), (-4.5, 0.6, 15.5)),
                (1.3, (6.0, 4.5, 23.0), (-4.5, 0.6, 15.5)),
                (2.4, (4.0, 6.5, 12.0), (-9.0, 2.5, 2.5)),
                (5.2, (4.0, 6.5, 12.0), (-11.0, 3.5, 2.5)),
                (6.0, (0.0, 7.0, 8.0), (-9.5, 2.0, -2.6)),
                (11.2, (0.0, 7.0, 8.0), (-9.5, 1.5, -2.6)),
                (11.5, (6.0, 5.0, 16.0), (2.5, 0.8, 8.5)),
                (13.2, (6.0, 5.0, 16.0), (2.5, 0.8, 8.5)),
                (13.6, (-6.0, 7.5, 12.0), (-15.0, 3.5, 2.5)),
                (16.5, (-6.0, 7.5, 12.0), (-15.0, 3.0, 2.5)),
            ]
            .into_iter()
            .map(|(at, eye, target)| Shot {
                at,
                eye: Vec3::from(eye),
                target: Vec3::from(target),
            })
            .collect()
        },
        replay_camera: (Vec3::new(-1.0, 5.5, 6.0), Vec3::new(-8.8, 1.5, -2.6)),
        check: |game| {
            let log: Vec<&str> = game.spells.log.iter().map(|r| r.text.as_str()).collect();
            let has = |needle: &str| log.iter().any(|l| l.contains(needle));
            let raised = log.iter().filter(|l| l.contains(" raised: ")).count();
            if raised != 7 {
                return Err(format!("{raised} of 7 walls were raised: {log:?}"));
            }
            if !has("Panel 3 destroyed") {
                return Err(format!("The middle panel survived: {log:?}"));
            }
            let torn = log
                .iter()
                .filter(|l| l.contains("footing of panel") && l.contains("broke"))
                .count();
            if torn < 2 {
                return Err(format!("Only {torn} footings tore: {log:?}"));
            }
            let pushed = game
                .spells
                .tracks
                .iter()
                .find(|t| t.label == "Dummy (wall)")
                .and_then(|t| crate::playground::measure(game, t))
                .map(|(_, d)| d)
                .ok_or("The wall did not push its dummy")?;
            if pushed < 0.5 {
                return Err(format!("The dummy moved only {pushed:.2} m"));
            }
            if !has("escapes") {
                return Err(format!("The enclosed dummy did not escape: {log:?}"));
            }
            if !log
                .iter()
                .any(|l| l.contains("Adventurer fell") && l.contains("2d6"))
            {
                return Err(format!("The wizard did not fall from the tower: {log:?}"));
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE {
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

    fn hall_game(caster: Vec3, dummies: &[(u64, Vec3)]) -> Game {
        let hall = hall().unwrap();
        let mut scene = hall.scene.clone();
        scene.actors[0].position = caster;
        for (id, at) in dummies {
            scene
                .actors
                .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
        }
        let mut game = Game::new(scene).unwrap();
        game.face(0.).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.).round() as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    fn wall(game: &mut Game, layout: Layout, side: DVec3) {
        game.spells.wall_of_stone.author(layout, side).unwrap();
        game.activate(Ability::Spell(SLOT)).unwrap();
    }

    fn chasm_bridge() -> Layout {
        Layout::Bridge {
            from: DVec3::new(-4.5, 0., 18.548),
            to: DVec3::new(-4.5, 0., 12.452),
            lanes: 1,
            form: Form::Thick,
        }
    }

    #[test]
    fn the_spell_is_on_the_second_row() {
        let def = Ability::Spell(SLOT).catalog().expect("in the catalog");
        assert_eq!(def.key, "wall-of-stone");
        assert_eq!(def.icon, "wall-of-stone-icon");
        assert!(crate::playground::scenario("wall-of-stone").is_some());
    }

    #[test]
    fn a_bridge_carries_the_wizard_and_vanishes_when_concentration_moves_on() {
        let mut game = hall_game(Vec3::new(-4.5, 0., 20.), &[(2, Vec3::new(20., 0., -20.))]);
        wall(&mut game, chasm_bridge(), DVec3::X);
        assert_eq!(game.spells.wall_of_stone.walls[0].panels.len(), 2);
        // Walk across the 3 m chasm on the panels.
        for _ in 0..40 {
            game.tick(1. / 30., [0., 1.]).unwrap();
        }
        let wizard = game.player;
        assert!(wizard.z < 13.5 && wizard.y.abs() < 0.01, "{wizard}");
        // A second cast ends concentration on the first.
        wall(
            &mut game,
            Layout::Straight {
                start: DVec3::new(6.0, 0., -6.0),
                direction: DVec3::X,
                count: 1,
                form: Form::Half,
            },
            DVec3::Z,
        );
        settle(&mut game, 0.2);
        let first = &game.spells.wall_of_stone.walls[0];
        assert!(first.vanished);
        assert!(
            first
                .panels
                .iter()
                .all(|p| game.spells.props[p.prop].removed)
        );
        assert!(game.spells.log.iter().any(|r| r.text.contains("vanishes")));
    }

    #[test]
    fn a_refused_layout_spends_nothing() {
        let mut game = hall_game(Vec3::new(0., 0., 0.), &[(2, Vec3::new(20., 0., -20.))]);
        let mana = game.snapshot().player.mana;
        game.spells
            .wall_of_stone
            .author(
                Layout::Straight {
                    start: DVec3::new(0., 30., 0.),
                    direction: DVec3::X,
                    count: 2,
                    form: Form::Thick,
                },
                DVec3::Z,
            )
            .unwrap();
        let error = game.activate(Ability::Spell(SLOT)).unwrap_err();
        assert!(error.contains("merge with existing stone"), "{error}");
        assert_eq!(game.snapshot().player.mana, mana);
        assert!(game.spells.wall_of_stone.walls.is_empty());
    }

    #[test]
    fn a_wall_through_a_dummy_pushes_it_to_the_chosen_side() {
        let dummy = Vec3::new(1.0, 0., 6.75);
        let mut game = hall_game(Vec3::new(-6., 0., 0.), &[(2, dummy)]);
        wall(
            &mut game,
            Layout::Straight {
                start: DVec3::new(-1.048, 0., 7.05),
                direction: DVec3::X,
                count: 2,
                form: Form::Thick,
            },
            DVec3::Z,
        );
        settle(&mut game, 0.5);
        let now = game.actor_position(2).unwrap();
        let clear = 7.05 + Form::Thick.size().z * 0.5 + CHARACTER_RADIUS;
        assert!(f64::from(now.z) > clear, "{now}");
        assert!((now.x - dummy.x).abs() < 1e-3);
    }

    #[test]
    fn an_enclosed_dummy_saves_to_escape_or_is_trapped() {
        for (roll, escapes) in [(15, true), (2, false)] {
            let dummy = Vec3::new(4.0, 0., 11.0);
            let mut game = hall_game(Vec3::new(-6., 0., 0.), &[(2, dummy)]);
            game.spells.dice.force_save(2, roll).unwrap();
            wall(
                &mut game,
                Layout::Enclosure {
                    base: dummy.as_dvec3(),
                    form: Form::Thick,
                },
                DVec3::X,
            );
            settle(&mut game, 0.5);
            let moved = game.actor_position(2).unwrap().distance(dummy);
            let save = game.spells.log.iter().find_map(|r| r.save.clone()).unwrap();
            assert_eq!(save.ability, "Dexterity");
            if escapes {
                assert!(save.success);
                assert!((1.8..2.2).contains(&moved), "{moved}");
            } else {
                assert!(!save.success);
                assert!(moved < 1e-3, "{moved}");
            }
        }
    }

    #[test]
    fn a_fire_bolt_at_a_dummy_behind_the_wall_strikes_a_panel() {
        let mut game = hall_game(Vec3::new(0., 0., 0.), &[(2, Vec3::new(0., 0., -9.))]);
        game.selected = 2;
        // Facing -z; the default wall stands 4 m ahead, across the line.
        game.activate(Ability::Spell(SLOT)).unwrap();
        settle(&mut game, 0.2);
        game.activate(Ability::FireBolt).unwrap();
        settle(&mut game, 1.0);
        let hit = game
            .spells
            .wall_of_stone
            .walls
            .iter()
            .flat_map(|w| &w.panels)
            .any(|p| game.spells.props[p.prop].hit_points == Some(172));
        assert!(hit, "{:?}", game.spells.log);
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == 2)
            .unwrap()
            .health;
        assert_eq!(health, 100);
    }

    #[test]
    fn concentration_held_ten_minutes_makes_the_wall_permanent() {
        let mut game = hall_game(Vec3::new(-4.5, 0., 20.), &[(2, Vec3::new(20., 0., -20.))]);
        wall(&mut game, chasm_bridge(), DVec3::X);
        game.spells.wall_of_stone.walls[0].cast_at = game.time - DURATION as f32;
        settle(&mut game, 0.1);
        assert!(game.spells.wall_of_stone.walls[0].permanent);
        settle(&mut game, 1.0);
        wall(
            &mut game,
            Layout::Straight {
                start: DVec3::new(6.0, 0., -6.0),
                direction: DVec3::X,
                count: 1,
                form: Form::Half,
            },
            DVec3::Z,
        );
        settle(&mut game, 0.2);
        let first = &game.spells.wall_of_stone.walls[0];
        assert!(!first.vanished);
        assert!(
            first
                .panels
                .iter()
                .all(|p| !game.spells.props[p.prop].removed)
        );
    }

    #[test]
    fn checkpoints_round_trip_with_intact_and_breaking_joints() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        let mut saved = 0;
        // Intact (the bridge just raised), mid-collapse, and after.
        for at in [5.7, 10.3, 10.8] {
            while run.game.time < at {
                run.advance().unwrap();
            }
            run.game
                .spells
                .wall_of_stone
                .validate(&run.game.spells)
                .unwrap();
            let bytes = run.game.checkpoint().unwrap();
            let mut restored = Game::restore(&bytes).unwrap();
            let mut live = Game::restore(&bytes).unwrap();
            for _ in 0..15 {
                live.tick(1. / 30., [0.; 2]).unwrap();
                restored.tick(1. / 30., [0.; 2]).unwrap();
            }
            assert_eq!(live.checkpoint().unwrap(), restored.checkpoint().unwrap());
            saved += 1;
        }
        assert_eq!(saved, 3);
    }

    #[test]
    fn the_collapse_replays_identically_from_a_checkpoint_with_broken_welds() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while run.game.time < 10.6 {
            run.advance().unwrap();
        }
        let wall = &run.game.spells.wall_of_stone.walls[2];
        assert!(wall.bonds.iter().any(|b| !b.intact()));
        let saved = run.game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        for _ in 0..45 {
            run.game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(
            run.game.checkpoint().unwrap(),
            restored.checkpoint().unwrap()
        );
    }

    #[test]
    fn chamber_walls_are_cover_and_cultist_fire_breaks_panels() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.; 2]).unwrap();
        }
        let start = game.player + Vec3::Y * 1.2;
        game.activate(Ability::Spell(SLOT)).unwrap();
        let facing = Vec3::new(-game.yaw.sin(), 0., -game.yaw.cos());
        // A bolt toward the caster from beyond the wall stops at it.
        let from = start + facing * 8.;
        let cover = game
            .projectile_cover(from, start - from, 0.08)
            .unwrap()
            .expect("the wall is cover");
        let point = (from + (start - from) * cover as f32).as_dvec3();
        assert!(struck(&mut game, point, 8, DamageType::Force, "Cultist bolt").unwrap());
        let wall = &game.spells.wall_of_stone.walls[0];
        assert!(
            wall.panels
                .iter()
                .any(|p| game.spells.props[p.prop].hit_points == Some(172))
        );
        // Enough fire breaks the panel into debris.
        let (w, p) = (0, 1);
        damage_panel(&mut game, w, p, 500, DamageType::Fire, "Cultist bolt").unwrap();
        let wall = &game.spells.wall_of_stone.walls[0];
        assert!(wall.panels[p].destroyed);
        assert!((4..=8).contains(&wall.debris.len()));
        settle(&mut game, 1.0);
        let error = game.spells.ledger_error();
        assert!(error.linear < super::super::LEDGER_TOLERANCE, "{error:?}");
    }
}
