//! Bounded CAD body snapshots and inputs for an account's shared simulation.
use super::Rig;
use glam::{DQuat, DVec3, Vec3};
use physics::{BodyKind, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub doc: String,
    pub version: u64,
    pub hash: String,
    pub generation: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec3,
    pub rotation: DQuat,
    pub velocity: DVec3,
    pub spin: DVec3,
    pub sleeping: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub bodies: Vec<Body>,
    pub broken: Vec<bool>,
    time: f64,
    bursts: Vec<super::Burst>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub binding: Binding,
    pub state: State,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub account: String,
    pub revision: u64,
    pub objects: Vec<Object>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forces {
    pub player: Vec3,
    pub push: Vec3,
    pub gravity: Option<verse_world::reverse_gravity::Gravity>,
    pub winds: Vec<verse_world::wind_wall::Wall>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blast {
    pub center: Vec3,
    pub radius: f32,
    pub speed: f32,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub forces: Forces,
    pub blasts: Vec<Blast>,
}
impl Inputs {
    pub fn valid(&self) -> bool {
        let f = &self.forces;
        f.player.is_finite()
            && f.player.abs().max_element() <= 10_000.
            && f.push.is_finite()
            && f.push.length() <= 100.
            && f.winds.len() <= 4
            && f.winds.iter().all(|w| {
                w.path.len() >= 2
                    && w.path.len() <= 64
                    && w.base.is_finite()
                    && w.path
                        .iter()
                        .all(|p| p.is_finite() && p.abs().max_element() <= 10_000.)
            })
            && f.gravity.is_none_or(|g| {
                g.cylinder.base.is_finite()
                    && g.cylinder.base.abs().max_element() <= 10_000.
                    && (0.1..=16.).contains(&g.cylinder.radius)
                    && (0.1..=31.).contains(&g.cylinder.height)
            })
            && self.blasts.len() <= 16
            && self.blasts.iter().all(|b| {
                b.center.is_finite()
                    && b.center.abs().max_element() <= 10_000.
                    && (0.01..=100.).contains(&b.radius)
                    && (0.01..=1000.).contains(&b.speed)
            })
    }
}
impl Rig {
    pub fn shared_state(&self, world: &World) -> State {
        State {
            bodies: self
                .bodies
                .iter()
                .map(|id| {
                    let b = world[*id];
                    Body {
                        position: b.pos,
                        rotation: b.orientation,
                        velocity: b.vel,
                        spin: b.omega,
                        sleeping: b.sleeping,
                    }
                })
                .collect(),
            broken: self.seams.iter().map(|s| s.broken).collect(),
            time: world.time(),
            bursts: self.bursts.clone(),
        }
    }
    pub fn valid_state(&self, state: &State) -> bool {
        state.time.is_finite()
            && state.bursts.len() <= 256
            && state.bursts.iter().all(|b| {
                b.start.is_finite()
                    && b.until.is_finite()
                    && b.loads.len() <= self.bodies.len()
                    && b.loads.iter().all(|l| {
                        l.body < self.bodies.len()
                            && l.area.is_finite()
                            && l.clearing.is_finite()
                            && l.direction.is_finite()
                    })
            })
            && state.bodies.len() == self.bodies.len()
            && state.broken.len() == self.seams.len()
            && state.bodies.iter().all(|b| {
                b.position.is_finite()
                    && b.position.abs().max_element() <= 100_000.
                    && b.rotation.is_finite()
                    && (b.rotation.length_squared() - 1.).abs() < 1e-5
                    && b.velocity.is_finite()
                    && b.velocity.length() <= 10_000.
                    && b.spin.is_finite()
                    && b.spin.length() <= 10_000.
            })
            && self
                .seams
                .iter()
                .zip(&state.broken)
                .all(|(old, broken)| !old.broken || *broken)
    }
    pub fn apply_state(
        &mut self,
        world: &mut World,
        state: &State,
        replica: bool,
    ) -> Result<(), String> {
        if !self.valid_state(state) {
            return Err("This model's movement could not be read.".into());
        }
        for (id, state) in self.bodies.iter().zip(&state.bodies) {
            let b = &mut world[*id];
            b.pos = state.position;
            b.prev_pos = b.pos;
            b.orientation = state.rotation;
            b.prev_orientation = b.orientation;
            b.vel = state.velocity;
            b.omega = state.spin;
            b.sleeping = state.sleeping;
            b.kind = if replica {
                BodyKind::Static
            } else {
                BodyKind::Dynamic
            };
            b.force = DVec3::ZERO;
            b.torque = DVec3::ZERO;
        }
        for (seam, broken) in self.seams.iter_mut().zip(&state.broken) {
            if *broken && !seam.broken {
                world.remove_joint(seam.id);
                seam.broken = true;
            }
        }
        self.bursts = if replica {
            Vec::new()
        } else {
            let shift = world.time() - state.time;
            state
                .bursts
                .iter()
                .cloned()
                .map(|mut b| {
                    b.start += shift;
                    b.until += shift;
                    b
                })
                .collect()
        };
        self.regroup(world);
        Ok(())
    }
}
