//! Joints between two bodies, solved with contacts by sequential impulses.
//!
//! - [`JointKind::Point`] pins an anchor on each body together (MuJoCo's
//!   and Genesis's `connect`, `examples/rigid/closed_loop.py`).
//! - [`JointKind::Weld`] also holds their relative orientation
//!   (`add_weld_constraint` in `examples/rigid/suction_cup.py`).
//! - [`JointKind::Tether`] keeps the anchors at most a length apart and
//!   only pulls.
//!
//! A joint is hard (Baumgarte-corrected) or soft: a critically damped spring
//! set by natural frequency and damping ratio, solved implicitly so it is
//! stable at any stiffness. This is the grab in Genesis's mouse-interaction
//! viewer plugin: the impulse that lands the end-of-step velocity on the
//! spring-damper response, `(v + b C) / (1/m + s)` with softness
//! `s = 1 / (dt (c + dt k))` and bias rate `b = k / (c + dt k)` for
//! `k = m w^2` and `c = 2 m zeta w` per row.
//!
//! Force and torque limits cap the impulse; a joint that reached a limit in
//! the last step reports `saturated`, and its owner decides whether it
//! breaks.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::world::{BodyId, World};

/// Spring settings for a soft joint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spring {
    /// Natural frequency, rad/s.
    pub frequency: f64,
    /// 1 is critical damping.
    pub damping_ratio: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "joint", rename_all = "snake_case")]
pub enum JointKind {
    Point,
    /// Holds `b`'s orientation at `a`'s orientation times `relative`.
    Weld {
        relative: DQuat,
    },
    /// Anchors at most `length` apart; pulls only.
    Tether {
        length: f64,
    },
}

/// A joint between bodies `a` and `b`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Joint {
    pub a: BodyId,
    pub b: BodyId,
    /// Anchor on `a`, in `a`'s body frame, m.
    pub anchor_a: DVec3,
    /// Anchor on `b`, in `b`'s body frame, m.
    pub anchor_b: DVec3,
    pub kind: JointKind,
    /// `None` for a hard joint.
    pub spring: Option<Spring>,
    /// Largest force the joint transmits, N.
    pub max_force: f64,
    /// Largest torque a weld transmits, N m.
    pub max_torque: f64,
    /// Impulse on `b` in the last step, N s; `a` received the opposite.
    #[serde(default)]
    pub impulse: DVec3,
    /// Angular impulse on `b` in the last step, N m s.
    #[serde(default)]
    pub angular_impulse: DVec3,
    /// Whether a force or torque limit capped the last step.
    #[serde(default)]
    pub saturated: bool,
}

impl Joint {
    /// A hard, unlimited joint.
    #[must_use]
    pub fn new(a: BodyId, anchor_a: DVec3, b: BodyId, anchor_b: DVec3, kind: JointKind) -> Self {
        Self {
            a,
            b,
            anchor_a,
            anchor_b,
            kind,
            spring: None,
            max_force: f64::INFINITY,
            max_torque: f64::INFINITY,
            impulse: DVec3::ZERO,
            angular_impulse: DVec3::ZERO,
            saturated: false,
        }
    }

    /// A weld holding the bodies' current relative pose, anchored at the
    /// world point `at`.
    #[must_use]
    pub fn weld_here(world: &World, a: BodyId, b: BodyId, at: DVec3) -> Self {
        let (ba, bb) = (&world[a], &world[b]);
        Self::new(
            a,
            ba.orientation.inverse() * (at - ba.pos),
            b,
            bb.orientation.inverse() * (at - bb.pos),
            JointKind::Weld {
                relative: ba.orientation.inverse() * bb.orientation,
            },
        )
    }

    #[must_use]
    pub fn soft(mut self, frequency: f64, damping_ratio: f64) -> Self {
        self.spring = Some(Spring {
            frequency,
            damping_ratio,
        });
        self
    }

    #[must_use]
    pub fn limited(mut self, max_force: f64, max_torque: f64) -> Self {
        self.max_force = max_force;
        self.max_torque = max_torque;
        self
    }

    /// World anchor positions on `a` and `b`.
    #[must_use]
    pub fn anchors(&self, world: &World) -> (DVec3, DVec3) {
        (
            world[self.a].to_world(self.anchor_a),
            world[self.b].to_world(self.anchor_b),
        )
    }

    /// Orientation error of a weld as a world rotation vector from where `b`
    /// should be to where it is, rad; zero for other kinds.
    #[must_use]
    pub fn angle_error(&self, world: &World) -> DVec3 {
        let JointKind::Weld { relative } = self.kind else {
            return DVec3::ZERO;
        };
        let target = world[self.a].orientation * relative;
        let mut error = world[self.b].orientation * target.inverse();
        if error.w < 0.0 {
            error = -error;
        }
        let (axis, angle) = error.to_axis_angle();
        axis * angle
    }
}

/// Index of a joint in its world. Removed joints leave a gap, so ids stay
/// valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JointId(pub u32);

impl World {
    pub fn add_joint(&mut self, joint: Joint) -> JointId {
        self.joints.push(Some(joint));
        JointId(u32::try_from(self.joints.len() - 1).expect("fewer than 2^32 joints"))
    }

    /// Remove a joint; returns it if it was present.
    pub fn remove_joint(&mut self, id: JointId) -> Option<Joint> {
        self.joints.get_mut(id.0 as usize)?.take()
    }

    #[must_use]
    pub fn joint(&self, id: JointId) -> Option<&Joint> {
        self.joints.get(id.0 as usize)?.as_ref()
    }

    pub fn joint_mut(&mut self, id: JointId) -> Option<&mut Joint> {
        self.joints.get_mut(id.0 as usize)?.as_mut()
    }

    /// Present joints with their ids.
    pub fn joints(&self) -> impl Iterator<Item = (JointId, &Joint)> {
        self.joints
            .iter()
            .enumerate()
            .filter_map(|(i, j)| j.as_ref().map(|j| (JointId(i as u32), j)))
    }
}
