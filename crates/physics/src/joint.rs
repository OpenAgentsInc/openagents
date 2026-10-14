//! Joints between two bodies, solved with contacts by sequential impulses.
//!
//! - [`JointKind::Point`] pins an anchor on each body together (MuJoCo's
//!   and Genesis's `connect`, `examples/rigid/closed_loop.py`).
//! - [`JointKind::Weld`] also holds their relative orientation
//!   (`add_weld_constraint` in `examples/rigid/suction_cup.py`).
//! - [`JointKind::Tether`] keeps the anchors at most a length apart and
//!   only pulls.
//! - [`JointKind::Hinge`] rotates about one axis; [`JointKind::Slider`]
//!   translates along one axis. Both support unilateral position limits
//!   and an implicit, effort-capped position [`Servo`].
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
    /// Rotation about `axis` in `a`'s frame, relative to `relative` at zero.
    Hinge {
        axis: DVec3,
        relative: DQuat,
        limits: Option<JointLimits>,
        servo: Option<Servo>,
    },
    /// Translation along `axis` in `a`'s frame, with fixed relative attitude.
    Slider {
        axis: DVec3,
        relative: DQuat,
        limits: Option<JointLimits>,
        servo: Option<Servo>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointLimits {
    /// Radians for a hinge, meters for a slider.
    pub lower: f64,
    pub upper: f64,
}

/// An implicit position PD servo. Effort is torque for a hinge and force
/// for a slider. `max_speed` caps commanded relative speed; a body already
/// above it decelerates under the effort cap instead of losing momentum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Servo {
    pub target: f64,
    pub kp: f64,
    pub kd: f64,
    pub max_effort: f64,
    pub max_speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointError {
    Bodies,
    Axis,
    Rotation,
    Limits,
    Servo,
    Inertia,
    Settings,
}

impl std::fmt::Display for JointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bodies => "Choose two different bodies in this world.",
            Self::Axis => "Use a finite unit joint axis.",
            Self::Rotation => "Use a unit rotation for the joint's zero position.",
            Self::Limits => "Use ordered finite limits; hinge limits must stay strictly between minus pi and pi.",
            Self::Servo => "Use positive servo settings and a target inside the joint limits.",
            Self::Inertia => "Use positive physical mass and inertia for each moving body.",
            Self::Settings => "Use finite anchors and positive joint force and spring settings.",
        })
    }
}

impl std::error::Error for JointError {}

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
    #[serde(with = "limit")]
    pub max_force: f64,
    /// Largest torque a weld transmits, N m.
    #[serde(with = "limit")]
    pub max_torque: f64,
    /// Impulse on `b` in the last step, N s; `a` received the opposite.
    #[serde(default)]
    pub impulse: DVec3,
    /// World point where `impulse` acted on `b`, m.
    #[serde(default)]
    pub point: DVec3,
    /// Angular impulse on `b` in the last step, N m s.
    #[serde(default)]
    pub angular_impulse: DVec3,
    /// Whether a force or torque limit capped the last step.
    #[serde(default)]
    pub saturated: bool,
    /// Absolute effort supplied by position limits in the last step, N or N m.
    #[serde(default)]
    pub limit_effort: f64,
    /// Signed motor effort on `b` in the last step, N or N m.
    #[serde(default)]
    pub motor_effort: f64,
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
            point: DVec3::ZERO,
            angular_impulse: DVec3::ZERO,
            saturated: false,
            limit_effort: 0.0,
            motor_effort: 0.0,
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
        let relative = match self.kind {
            JointKind::Weld { relative } | JointKind::Slider { relative, .. } => relative,
            JointKind::Hinge { axis, relative, .. } => {
                let parent = world[self.a].orientation;
                let q = parent.inverse() * world[self.b].orientation * relative.inverse();
                let twist = hinge_twist(q, axis);
                return parent * rotation_vector(q * twist.inverse());
            }
            _ => return DVec3::ZERO,
        };
        let target = world[self.a].orientation * relative;
        rotation_vector(world[self.b].orientation * target.inverse())
    }

    /// Hinge angle in [-pi, pi] or slider displacement along the parent axis.
    /// Unlimited multi-turn hinges require a separate unwrapped coordinate.
    pub fn coordinate(&self, world: &World) -> Option<f64> {
        match self.kind {
            JointKind::Hinge { axis, relative, .. } => {
                let q = world[self.a].orientation.inverse()
                    * world[self.b].orientation
                    * relative.inverse();
                let twist = hinge_twist(q, axis);
                let vector = DVec3::new(twist.x, twist.y, twist.z);
                let angle = 2.0 * vector.dot(axis).atan2(twist.w);
                Some(
                    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                        - std::f64::consts::PI,
                )
            }
            JointKind::Slider { axis, .. } => {
                let (a, b) = self.anchors(world);
                Some((b - a).dot(world[self.a].orientation * axis))
            }
            _ => None,
        }
    }

    pub fn validate(&self, world: &World) -> Result<(), JointError> {
        let bodies = world.bodies();
        if self.a == self.b
            || [self.a, self.b]
                .iter()
                .any(|id| bodies.get(id.0 as usize).is_none_or(|body| body.removed))
        {
            return Err(JointError::Bodies);
        }
        for id in [self.a, self.b] {
            let body = &world[id];
            if !body.pos.is_finite()
                || !body.orientation.is_finite()
                || (body.orientation.length_squared() - 1.0).abs() > 1e-9
            {
                return Err(JointError::Rotation);
            }
            if body.kind == crate::BodyKind::Dynamic
                && (!body.mass.is_finite()
                    || body.mass <= 0.0
                    || !body.inertia.is_finite()
                    || body.inertia.min_element() <= 0.0
                    || 2.0 * body.inertia.max_element()
                        > body.inertia.element_sum() * (1.0 + 1e-12))
            {
                return Err(JointError::Inertia);
            }
        }
        let bound = |n: f64| (n.is_finite() || n == f64::INFINITY) && n > 0.0;
        if !self.anchor_a.is_finite()
            || !self.anchor_b.is_finite()
            || !bound(self.max_force)
            || !bound(self.max_torque)
            || self.spring.is_some_and(|s| {
                !s.frequency.is_finite()
                    || s.frequency <= 0.0
                    || !s.damping_ratio.is_finite()
                    || s.damping_ratio < 0.0
            })
        {
            return Err(JointError::Settings);
        }
        let rotation = |q: DQuat| q.is_finite() && (q.length_squared() - 1.0).abs() <= 1e-9;
        match self.kind {
            JointKind::Hinge {
                axis,
                relative,
                limits,
                servo,
            }
            | JointKind::Slider {
                axis,
                relative,
                limits,
                servo,
            } => {
                if !axis.is_finite() || (axis.length_squared() - 1.0).abs() > 1e-9 {
                    return Err(JointError::Axis);
                }
                if !rotation(relative) {
                    return Err(JointError::Rotation);
                }
                if limits.is_some_and(|l| {
                    !l.lower.is_finite()
                        || !l.upper.is_finite()
                        || l.lower >= l.upper
                        || (matches!(self.kind, JointKind::Hinge { .. })
                            && (l.lower <= -std::f64::consts::PI
                                || l.upper >= std::f64::consts::PI))
                }) {
                    return Err(JointError::Limits);
                }
                if servo.is_some_and(|s| {
                    !s.target.is_finite()
                        || [s.kp, s.kd, s.max_effort, s.max_speed]
                            .iter()
                            .any(|n| !n.is_finite() || *n <= 0.0 || *n > 1e12)
                        || (matches!(self.kind, JointKind::Hinge { .. })
                            && s.target.abs() >= std::f64::consts::PI)
                        || limits.is_some_and(|l| s.target < l.lower || s.target > l.upper)
                }) {
                    return Err(JointError::Servo);
                }
            }
            JointKind::Weld { relative } if !rotation(relative) => {
                return Err(JointError::Rotation);
            }
            JointKind::Tether { length } if !length.is_finite() || length <= 0.0 => {
                return Err(JointError::Limits);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn set_target(&mut self, target: f64) -> Result<(), JointError> {
        if matches!(self.kind, JointKind::Hinge { .. }) && target.abs() >= std::f64::consts::PI {
            return Err(JointError::Servo);
        }
        let (servo, limits) = match &mut self.kind {
            JointKind::Hinge { servo, limits, .. } | JointKind::Slider { servo, limits, .. } => {
                (servo, limits)
            }
            _ => return Err(JointError::Servo),
        };
        if !target.is_finite() || limits.is_some_and(|l| target < l.lower || target > l.upper) {
            return Err(JointError::Servo);
        }
        servo.as_mut().ok_or(JointError::Servo)?.target = target;
        Ok(())
    }
}

fn hinge_twist(q: DQuat, axis: DVec3) -> DQuat {
    let p = axis * DVec3::new(q.x, q.y, q.z).dot(axis);
    let twist = DQuat::from_xyzw(p.x, p.y, p.z, q.w);
    if twist.length_squared() > 1e-20 {
        twist.normalize()
    } else {
        DQuat::IDENTITY
    }
}

fn rotation_vector(mut error: DQuat) -> DVec3 {
    if error.w < 0.0 {
        error = -error;
    }
    let (axis, angle) = error.to_axis_angle();
    axis * angle
}

/// Index of a joint in its world. Removed joints leave a gap, so ids stay
/// valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct JointId(pub u32);

impl World {
    pub fn try_add_joint(&mut self, joint: Joint) -> Result<JointId, JointError> {
        joint.validate(self)?;
        Ok(self.add_joint(joint))
    }

    /// Change a motor target and wake its entire connected island.
    pub fn set_joint_target(&mut self, id: JointId, target: f64) -> Result<(), JointError> {
        let joint = self.joint_mut(id).ok_or(JointError::Bodies)?;
        joint.set_target(target)?;
        let (a, b) = (joint.a, joint.b);
        self.wake_joint_island(a, b);
        Ok(())
    }

    pub(crate) fn wake_joint_island(&mut self, a: BodyId, b: BodyId) {
        let mut awake = std::collections::BTreeSet::from([a, b]);
        loop {
            let previous = awake.len();
            for joint in self.joints.iter().flatten() {
                if awake.contains(&joint.a) || awake.contains(&joint.b) {
                    awake.insert(joint.a);
                    awake.insert(joint.b);
                }
            }
            if awake.len() == previous {
                break;
            }
        }
        for id in awake {
            self.wake(id);
        }
    }

    pub(crate) fn wake_active_servos(&mut self) {
        let active: Vec<_> = self
            .joints
            .iter()
            .flatten()
            .filter_map(|joint| {
                let servo = match joint.kind {
                    JointKind::Hinge { servo, .. } | JointKind::Slider { servo, .. } => servo?,
                    _ => return None,
                };
                ((joint.coordinate(self)? - servo.target).abs() > 1e-6)
                    .then_some((joint.a, joint.b))
            })
            .collect();
        for (a, b) in active {
            self.wake_joint_island(a, b);
        }
    }

    /// Add a joint, waking both bodies.
    pub fn add_joint(&mut self, joint: Joint) -> JointId {
        self.wake(joint.a);
        self.wake(joint.b);
        self.joints.push(Some(joint));
        JointId(u32::try_from(self.joints.len() - 1).expect("fewer than 2^32 joints"))
    }

    /// Remove a joint, waking both bodies; returns it if it was present.
    pub fn remove_joint(&mut self, id: JointId) -> Option<Joint> {
        let joint = self.joints.get_mut(id.0 as usize)?.take()?;
        self.wake(joint.a);
        self.wake(joint.b);
        Some(joint)
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

/// JSON null denotes an unlimited positive force or torque bound.
mod limit {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if *value == f64::INFINITY {
            serializer.serialize_none()
        } else if value.is_finite() {
            serializer.serialize_some(value)
        } else {
            Err(serde::ser::Error::custom("Invalid joint limit"))
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(f64::INFINITY))
    }
}
