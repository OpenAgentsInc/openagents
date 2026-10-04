//! The wall in a [`World`]: dynamic granite panels, welded to each other
//! along shared edges and pinned to the stone they merge with.
//!
//! The panels of one wall are one casting, so a seam carries force and
//! bending, in proportion to its length ([`SEAM_FORCE`], [`SEAM_TORQUE`]).
//! Where a panel merges with existing stone, the joint holds it in place
//! but carries no bending ([`FOOTING_FORCE`]): the deck of a bridge rests on
//! its abutments through contacts, so the halves of a bridge whose middle
//! panel is destroyed tip off them and fall. A joint the solver holds at
//! its limit for [`BREAK_STEPS`] steps breaks, so collapse comes from the
//! solver rather than from a rule.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use physics::{Body, BodyId, Collider, Joint, JointId, JointKind, Material, Shape, World};

use super::validate::Plan;
use super::{DEBRIS_LIFETIME, DURATION, DamageType, Form};

/// Largest force a seam carries per meter of shared edge, N/m.
pub const SEAM_FORCE: f64 = 120_000.0;
/// Largest bending moment a seam carries per meter of shared edge, N m/m.
/// An intact 20-foot bridge carrying a 300 kg creature at its center loads
/// a 10-foot seam to about 15 000 N m/m (the solver reports up to about
/// twice that); a 30-foot cantilever loads its root to about
/// 170 000 N m/m.
pub const SEAM_TORQUE: f64 = 100_000.0;
/// Gap left between the colliders of neighbouring panels, m. Welded panels
/// that also touched would have their contacts fight the weld; the gap is
/// on the panel's +x end and +y edge, so a panel's base still rests on
/// whatever it stands on.
pub const SEAM_GAP: f64 = 0.005;
/// Largest force the pin between a panel and stone carries, N. A bridge
/// half tipping off its abutment pulls about 37 500 N on it.
pub const FOOTING_FORCE: f64 = 20_000.0;
/// Natural frequency of the pin between a panel and stone, rad/s. Soft, so
/// the pin and the contacts beside it share the load instead of fighting
/// over it; it sags about 1 cm under a hanging panel.
pub const FOOTING_FREQUENCY: f64 = std::f64::consts::TAU * 5.0;
/// Consecutive steps at a limit before a joint breaks.
pub const BREAK_STEPS: u32 = 6;
/// Granite on granite.
pub const GRANITE: Material = Material {
    friction: 0.7,
    torsional: 0.0,
    restitution: 0.05,
};
/// Debris chunk counts and their grids along and across a panel.
const DEBRIS_GRIDS: [(usize, usize); 3] = [(2, 2), (3, 2), (4, 2)];

/// One panel of the wall.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub form: Form,
    pub body: BodyId,
    pub hit_points: i32,
    pub destroyed: bool,
}

/// What a joint holds together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "bond", rename_all = "snake_case")]
pub enum BondKind {
    Seam { a: usize, b: usize },
    Footing { panel: usize, stone: usize },
}

/// One joint of the wall.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bond {
    pub kind: BondKind,
    pub joint: JointId,
    /// Consecutive steps at its limit.
    pub strained: u32,
    /// When it broke or was removed, s.
    pub broken: Option<f64>,
}

impl Bond {
    #[must_use]
    pub fn intact(&self) -> bool {
        self.broken.is_none()
    }

    #[must_use]
    pub fn touches(&self, panel: usize) -> bool {
        match self.kind {
            BondKind::Seam { a, b } => a == panel || b == panel,
            BondKind::Footing { panel: p, .. } => p == panel,
        }
    }
}

/// A chunk of a destroyed panel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Debris {
    pub body: BodyId,
    /// When it despawns, s.
    pub until: f64,
    pub removed: bool,
}

/// Something the wall did, for presentation and evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A joint broke under load.
    Broke {
        bond: usize,
    },
    /// A panel reached 0 hit points.
    Destroyed {
        panel: usize,
        debris: Vec<BodyId>,
    },
    DebrisGone {
        body: BodyId,
    },
    /// Concentration ended early; the panels are gone.
    Vanished,
    /// Concentration held for the full duration.
    Permanent,
}

/// A raised wall. It serializes with its world, so a checkpoint restores
/// broken joints, damage, and debris exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub cast_at: f64,
    pub permanent: bool,
    pub vanished: bool,
    pub panels: Vec<Panel>,
    pub bonds: Vec<Bond>,
    pub debris: Vec<Debris>,
}

fn panel_body(world: &mut World, form: Form, center: DVec3, orientation: glam::DQuat) -> BodyId {
    let size = form.size();
    let mut body = Body::new(form.mass(), Body::box_inertia(form.mass(), size), center);
    body.orientation = orientation;
    body.prev_orientation = orientation;
    let id = world.add(body);
    world.add_collider(
        Collider::new(
            id,
            Shape::Cuboid {
                half: size * 0.5 - DVec3::new(SEAM_GAP, SEAM_GAP, 0.0) * 0.5,
            },
        )
        .at(
            -DVec3::new(SEAM_GAP, SEAM_GAP, 0.0) * 0.5,
            glam::DQuat::IDENTITY,
        )
        .with_material(GRANITE),
    );
    id
}

/// Wake every body whose colliders come near `body`'s, so whatever slept
/// resting on a panel falls when the panel goes.
fn wake_near(world: &mut World, body: BodyId) {
    let reach = world.solver.margin + 0.05;
    let near: Vec<BodyId> = {
        let colliders = world.colliders();
        let own: Vec<(DVec3, f64)> = colliders
            .iter()
            .filter(|c| c.body == body)
            .map(|c| (c.pose(world).0, c.shape.bound()))
            .collect();
        colliders
            .iter()
            .filter(|c| c.body != body && world[c.body].sleeping)
            .filter(|c| {
                let (at, bound) = (c.pose(world).0, c.shape.bound());
                own.iter().any(|(p, r)| p.distance(at) <= r + bound + reach)
            })
            .map(|c| c.body)
            .collect()
    };
    for id in near {
        world.wake(id);
    }
}

impl Wall {
    /// Raise an admitted wall at `time`. `stone` maps each stone index in
    /// the plan to the static body that carries its collider.
    #[must_use]
    pub fn raise(world: &mut World, plan: &Plan, stone: &[BodyId], time: f64) -> Self {
        let panels: Vec<Panel> = plan
            .panels
            .iter()
            .map(|p| Panel {
                form: p.form,
                body: panel_body(world, p.form, p.center, p.orientation),
                hit_points: p.form.hit_points(),
                destroyed: false,
            })
            .collect();
        let mut bonds = Vec::new();
        for seam in &plan.seams {
            let joint = Joint::weld_here(world, panels[seam.a].body, panels[seam.b].body, seam.at)
                .limited(SEAM_FORCE * seam.length, SEAM_TORQUE * seam.length);
            bonds.push(Bond {
                kind: BondKind::Seam {
                    a: seam.a,
                    b: seam.b,
                },
                joint: world.add_joint(joint),
                strained: 0,
                broken: None,
            });
        }
        for footing in &plan.footings {
            let (rock, panel) = (stone[footing.stone], panels[footing.panel].body);
            let joint = Joint::new(
                rock,
                world[rock].orientation.inverse() * (footing.at - world[rock].pos),
                panel,
                world[panel].orientation.inverse() * (footing.at - world[panel].pos),
                JointKind::Point,
            )
            .limited(FOOTING_FORCE, f64::INFINITY)
            .soft(FOOTING_FREQUENCY, 1.0);
            bonds.push(Bond {
                kind: BondKind::Footing {
                    panel: footing.panel,
                    stone: footing.stone,
                },
                joint: world.add_joint(joint),
                strained: 0,
                broken: None,
            });
        }
        Self {
            cast_at: time,
            permanent: false,
            vanished: false,
            panels,
            bonds,
            debris: Vec::new(),
        }
    }

    /// Update after each world step at `time`: break joints held at their
    /// limit, despawn old debris, and make the wall permanent once
    /// concentration has lasted the full duration.
    pub fn after_step(&mut self, world: &mut World, time: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for (index, bond) in self.bonds.iter_mut().enumerate() {
            if !bond.intact() {
                continue;
            }
            let saturated = world.joint(bond.joint).is_some_and(|j| j.saturated);
            bond.strained = if saturated { bond.strained + 1 } else { 0 };
            if bond.strained >= BREAK_STEPS {
                world.remove_joint(bond.joint);
                bond.broken = Some(time);
                events.push(Event::Broke { bond: index });
            }
        }
        for chunk in &mut self.debris {
            if !chunk.removed && time >= chunk.until {
                world.remove_body(chunk.body);
                chunk.removed = true;
                events.push(Event::DebrisGone { body: chunk.body });
            }
        }
        if !self.permanent && !self.vanished && time - self.cast_at >= DURATION {
            self.permanent = true;
            events.push(Event::Permanent);
        }
        events
    }

    /// Joints still holding.
    pub fn intact(&self) -> impl Iterator<Item = (usize, &Bond)> {
        self.bonds.iter().enumerate().filter(|(_, b)| b.intact())
    }

    /// Damage a panel. Poison and Psychic do nothing; at 0 hit points the
    /// panel breaks into debris. `seed` picks the chunk count.
    pub fn damage(
        &mut self,
        world: &mut World,
        panel: usize,
        amount: i32,
        kind: DamageType,
        time: f64,
        seed: u64,
    ) -> Vec<Event> {
        let Some(p) = self.panels.get_mut(panel) else {
            return Vec::new();
        };
        if p.destroyed || self.vanished || !kind.harms_panels() || amount <= 0 {
            return Vec::new();
        }
        p.hit_points = (p.hit_points - amount).max(0);
        if p.hit_points > 0 {
            return Vec::new();
        }
        vec![self.destroy(world, panel, time, seed)]
    }

    /// Remove a panel and its joints and break it into 4 to 8 chunks with
    /// the panel's total mass and momentum.
    pub fn destroy(&mut self, world: &mut World, panel: usize, time: f64, seed: u64) -> Event {
        let p = &mut self.panels[panel];
        p.destroyed = true;
        p.hit_points = 0;
        let (form, id) = (p.form, p.body);
        for bond in &mut self.bonds {
            if bond.intact() && bond.touches(panel) {
                world.remove_joint(bond.joint);
                bond.broken = Some(time);
            }
        }
        let body = world[id];
        wake_near(world, id);
        world.remove_body(id);
        let (nx, ny) = DEBRIS_GRIDS[(seed % DEBRIS_GRIDS.len() as u64) as usize];
        let size = form.size();
        let chunk = DVec3::new(size.x / nx as f64, size.y / ny as f64, size.z);
        let mass = body.mass / (nx * ny) as f64;
        let omega = body.omega_world();
        let mut debris = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let local = DVec3::new(
                    -size.x * 0.5 + chunk.x * (i as f64 + 0.5),
                    -size.y * 0.5 + chunk.y * (j as f64 + 0.5),
                    0.0,
                );
                let pos = body.to_world(local);
                let mut piece = Body::new(mass, Body::box_inertia(mass, chunk), pos);
                piece.orientation = body.orientation;
                piece.prev_orientation = body.orientation;
                piece.vel = body.vel + omega.cross(pos - body.pos);
                piece.omega = body.omega;
                let piece = world.add(piece);
                // Slightly smaller, so neighbouring chunks start apart.
                world.add_collider(
                    Collider::new(
                        piece,
                        Shape::Cuboid {
                            half: chunk * 0.5 * 0.96,
                        },
                    )
                    .with_material(GRANITE),
                );
                self.debris.push(Debris {
                    body: piece,
                    until: time + DEBRIS_LIFETIME,
                    removed: false,
                });
                debris.push(piece);
            }
        }
        Event::Destroyed { panel, debris }
    }

    /// Concentration ended at `time`. Before the full duration the wall
    /// vanishes, with its debris, and whatever rested on it falls; a
    /// permanent wall stays.
    pub fn end(&mut self, world: &mut World, time: f64) -> Vec<Event> {
        if self.permanent || self.vanished {
            return Vec::new();
        }
        if time - self.cast_at >= DURATION {
            self.permanent = true;
            return vec![Event::Permanent];
        }
        for bond in &mut self.bonds {
            if bond.intact() {
                world.remove_joint(bond.joint);
                bond.broken = Some(time);
            }
        }
        for panel in &mut self.panels {
            if !panel.destroyed {
                wake_near(world, panel.body);
                world.remove_body(panel.body);
            }
        }
        for chunk in &mut self.debris {
            if !chunk.removed {
                world.remove_body(chunk.body);
                chunk.removed = true;
            }
        }
        self.vanished = true;
        vec![Event::Vanished]
    }

    /// The panel whose body is `body`, if it stands.
    #[must_use]
    pub fn panel_of(&self, body: BodyId) -> Option<usize> {
        self.panels
            .iter()
            .position(|p| p.body == body && !p.destroyed && !self.vanished)
    }

    /// Joint stress for debug lines: each intact joint's anchors and the
    /// fraction of its force or torque limit it carried in the last step.
    #[must_use]
    pub fn stress(&self, world: &World) -> Vec<(DVec3, DVec3, f64)> {
        self.intact()
            .filter_map(|(_, bond)| {
                let joint = world.joint(bond.joint)?;
                let (a, b) = joint.anchors(world);
                let dt = world.dt;
                let force = joint.impulse.length() / dt / joint.max_force;
                let torque = if joint.max_torque.is_finite() {
                    joint.angular_impulse.length() / dt / joint.max_torque
                } else {
                    0.0
                };
                Some((a, b, force.max(torque)))
            })
            .collect()
    }
}
