//! The factory: a free-flying deck at Earth–Moon L2 with four arms, parts
//! stowed on racks, and the cradle the ship is built in, all as rigid
//! bodies and joints in one `physics` world.
//!
//! Every arm is a chain of servoed hinges whose base hinge is anchored on
//! the factory body, so every move pushes the factory back: reaction
//! forces, momentum exchange, and the attitude disturbance are physics,
//! not animation. The factory holds its attitude with a control moment gyro
//! (CMG) model: the controller's torque on the deck is taken from the CMG's
//! stored angular momentum, so factory, arms, parts, and CMG together
//! conserve angular momentum exactly. A CMG that runs out of momentum is a
//! named failure.
//!
//! Frames: the factory frame is the factory body's frame, origin at its
//! center of mass, +Y the deck normal, +X along the cradle (the ship's
//! thrust axis). The world starts aligned with it. Near Earth–Moon L2 the
//! tidal gradient is about 5 × 10⁻¹¹ s⁻², about 2 × 10⁻⁹ m/s² across the
//! deck (docs/spacecraft-design/sim-mapping.md), so the field is left out.

use glam::{DQuat, DVec3};
use physics::{
    Body, BodyId, Joint, JointId, JointKind, JointLimits, Momentum, Servo, world::NoField,
};
use serde::{Deserialize, Serialize};

use crate::arm::{self, ArmSpec, Pose};
use crate::geometry::{Obb, capsule_hits};
use crate::ship::Spacecraft;

/// Physics step, s.
pub const DT: f64 = 1.0 / 30.0;
/// Deck top above the factory's center of mass, m.
pub const DECK: f64 = 2.0;
/// The deck's size, m.
pub const DECK_SIZE: DVec3 = DVec3::new(96.0, 4.0, 96.0);
/// Factory mass with its racks, power, and CMGs, kg.
pub const FACTORY_MASS: f64 = 800_000.0;
/// Where the ship frame's origin sits in the factory frame, m.
pub const CRADLE: DVec3 = DVec3::new(0.0, 8.0, 0.0);
/// Height of the tool above a grapple when approaching or leaving it, m.
pub const CLEAR: f64 = 3.0;
/// Distance of the racks from the deck's center across its long axis, m.
pub const RACK_X: f64 = 36.0;
/// Rack slots beside each arm: offsets along the deck's short axis, away
/// from the cradle, m.
pub const RACK_SLOTS: [f64; 3] = [-6.0, 6.0, 0.0];
/// Rack top above the deck, m.
pub const RACK: f64 = 1.0;
/// Length of the tool body past the roll joint, m.
pub const TOOL: f64 = 1.5;
/// The CMG's momentum capacity, N m s: about fifty times the ISS's set
/// (four 4,760 N m s wheels), for a deck forty times its mass.
pub const CMG_CAPACITY: f64 = 1.0e6;
/// The CMG's torque limit, N m.
pub const CMG_TORQUE: f64 = 4.0e5;
/// Rated force of the arm's grapple, N.
pub const GRAPPLE_FORCE: f64 = 2.0e6;

/// Height of the pedestals the arms stand on, m.
pub const PEDESTAL: f64 = 6.0;
/// Carried parts travel with their undersides this high above the deck's
/// center plane, clear of the ship and the racks, m.
pub const TRANSIT: f64 = 12.5;
/// Arm mountings: base (x, z) on the deck. Each faces the cradle.
pub const ARM_BASES: [[f64; 2]; 4] = [[-16.0, -15.0], [-16.0, 15.0], [16.0, -15.0], [16.0, 15.0]];

/// Where a part is held.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Holder {
    /// On its rack, by the rack's clamp.
    Rack(JointId),
    /// In an arm's grapple.
    Arm { arm: usize, grip: JointId },
    /// Latched into the ship.
    Latched(JointId),
    /// Held by the arm's grapple and latched both, between latch and
    /// release.
    Both {
        arm: usize,
        grip: JointId,
        latch: JointId,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactoryPart {
    /// Index into the ship document's parts.
    pub index: usize,
    pub body: BodyId,
    pub holder: Holder,
    /// Rack position and yaw, factory frame.
    pub rack: DVec3,
    pub rack_yaw: f64,
    /// Where the ship document puts it, factory frame.
    pub design: DVec3,
    pub design_yaw: f64,
    /// The arm assigned to place it.
    pub arm: usize,
    pub half: DVec3,
}

/// A joint-space move with a minimum-jerk profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    pub from: Pose,
    pub to: Pose,
    pub start: f64,
    pub duration: f64,
}

impl Motion {
    /// The reference pose at time `t`.
    #[must_use]
    pub fn at(&self, t: f64) -> Pose {
        let tau = ((t - self.start) / self.duration).clamp(0.0, 1.0);
        let s = tau * tau * tau * (10.0 - 15.0 * tau + 6.0 * tau * tau);
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * s)
    }

    #[must_use]
    pub fn done(&self, t: f64) -> bool {
        t >= self.start + self.duration
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arm {
    pub spec: ArmSpec,
    /// Turret, upper arm, forearm, wrist, tool.
    pub links: [BodyId; 5],
    pub joints: [JointId; 5],
    pub motion: Option<Motion>,
    /// Bumped when the arm's controller restarts; the sequencer compares
    /// it to notice a reset.
    pub epoch: u32,
    /// Seconds each joint's motor has been at its effort limit.
    pub saturated: [f64; 5],
    pub home: Pose,
}

/// The control moment gyro set: stored angular momentum, world frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cmg {
    pub h: DVec3,
    pub capacity: f64,
    pub max_torque: f64,
    /// Whether the controller wanted more momentum than the CMGs hold.
    pub saturated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attitude {
    pub enabled: bool,
    pub target: DQuat,
    pub kp: f64,
    pub kd: f64,
}

/// What a clearance check found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub a: String,
    pub b: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Factory {
    pub world: physics::World,
    pub body: BodyId,
    pub arms: Vec<Arm>,
    pub parts: Vec<FactoryPart>,
    pub ship: Spacecraft,
    pub cmg: Cmg,
    pub attitude: Attitude,
}

const LINK_NAMES: [&str; 5] = ["turret", "upper arm", "forearm", "wrist", "tool"];
/// Width of the arm links' mass boxes, m.
pub const LINK_MASS_WIDTH: f64 = 0.9;

impl Factory {
    /// The factory with `ship`'s parts on its racks. Parts go to the arm
    /// nearest their place in the ship that can reach it, two or three to
    /// a rack.
    ///
    /// # Errors
    /// The ship fails its checks or a part is out of every arm's reach.
    pub fn new(ship: Spacecraft) -> Result<Self, String> {
        if let Some(finding) = ship.check().into_iter().next() {
            return Err(format!("The ship does not pass its checks: {finding}"));
        }
        let mut world = physics::World::new(DT);
        world.sleep.enabled = false;
        let inertia = Body::box_inertia(FACTORY_MASS, DECK_SIZE);
        let body = world.add(Body::new(FACTORY_MASS, inertia, DVec3::ZERO));
        let specs: Vec<ArmSpec> = ARM_BASES
            .iter()
            .map(|&[x, z]| {
                // Zero pan reaches toward the cradle line.
                let yaw = if z < 0.0 {
                    -std::f64::consts::FRAC_PI_2
                } else {
                    std::f64::consts::FRAC_PI_2
                };
                ArmSpec::so101_scaled(DVec3::new(x, DECK + PEDESTAL, z), yaw)
            })
            .collect();
        let mut arms = Vec::new();
        for spec in specs {
            let home = home_pose(&spec)?;
            arms.push(build_arm(&mut world, body, spec, home)?);
        }
        let order = ship
            .assembly_order()
            .ok_or("The ship's attachments do not form a tree")?;
        // Assign arms: parts that few arms reach go first, each to the
        // least-loaded, then nearest, arm that reaches above its place.
        let reaches = |i: usize, index: usize| {
            let part = &ship.parts[index];
            let grapple = CRADLE + part.position + DVec3::Y * part.shape.half_extents().y;
            let spec: &ArmSpec = &arms[i].spec;
            spec.solve(grapple, part.yaw).is_ok()
                && spec.solve(grapple + DVec3::Y * CLEAR, part.yaw).is_ok()
        };
        let reach: Vec<Vec<usize>> = (0..ship.parts.len())
            .map(|index| (0..arms.len()).filter(|&i| reaches(i, index)).collect())
            .collect();
        let mut by_freedom = order.clone();
        by_freedom.sort_by_key(|&index| reach[index].len());
        let mut load = [0usize; 4];
        let mut assigned = vec![usize::MAX; ship.parts.len()];
        for &index in &by_freedom {
            let design = CRADLE + ship.parts[index].position;
            let distance = |i: usize| {
                let base = arms[i].spec.base;
                (base.x - design.x).hypot(base.z - design.z)
            };
            let arm = reach[index]
                .iter()
                .copied()
                .filter(|&i| load[i] < RACK_SLOTS.len())
                .min_by(|&a, &b| {
                    load[a]
                        .cmp(&load[b])
                        .then(distance(a).total_cmp(&distance(b)))
                        .then(a.cmp(&b))
                })
                .ok_or_else(|| {
                    format!(
                        "No arm reaches {}'s place in the ship",
                        ship.parts[index].id
                    )
                })?;
            load[arm] += 1;
            assigned[index] = arm;
        }
        let mut slots = [0usize; 4];
        let mut parts = Vec::new();
        for &index in &order {
            let part = &ship.parts[index];
            let design = CRADLE + part.position;
            let half = part.shape.half_extents();
            let arm = assigned[index];
            let spec = arms[arm].spec;
            let slot = slots[arm];
            slots[arm] += 1;
            // Racks stand outboard of each arm, about a quarter turn from
            // the cradle, parts lying along the deck's long axis, turned
            // end for end if that keeps the wrist's roll in its range.
            let rack = DVec3::new(
                spec.base.x.signum() * RACK_X,
                DECK + RACK + half.y,
                spec.base.z + spec.base.z.signum() * RACK_SLOTS[slot],
            );
            let grapple = rack + DVec3::Y * half.y;
            let rack_yaw = [0.0, std::f64::consts::PI]
                .into_iter()
                .find(|&yaw| spec.solve(grapple, yaw).is_ok())
                .unwrap_or(0.0);
            let orientation = DQuat::from_rotation_y(rack_yaw);
            let mass = part.mass();
            // The parts are symmetric about their own axes: the measured
            // tensor's off-diagonal terms are tessellation residue, under
            // 1e-4 of the diagonal.
            let local = part.local_inertia(mass);
            let principal = DVec3::new(local.x_axis.x, local.y_axis.y, local.z_axis.z);
            let mut pb = Body::new(mass, principal, rack);
            pb.orientation = orientation;
            pb.prev_orientation = orientation;
            let pid = world.add(pb);
            let clamp = world.add_joint(Joint::weld_here(&world, body, pid, rack));
            parts.push(FactoryPart {
                index,
                body: pid,
                holder: Holder::Rack(clamp),
                rack,
                rack_yaw,
                design,
                design_yaw: part.yaw,
                arm,
                half,
            });
        }
        Ok(Self {
            world,
            body,
            arms,
            parts,
            ship,
            cmg: Cmg {
                h: DVec3::ZERO,
                capacity: CMG_CAPACITY,
                max_torque: CMG_TORQUE,
                saturated: false,
            },
            attitude: Attitude {
                enabled: true,
                target: DQuat::IDENTITY,
                // About 0.15 rad/s bandwidth on the deck's 1.2e9 kg m^2.
                kp: 2.7e7,
                kd: 3.6e8,
            },
        })
    }

    /// The sample ship's factory.
    #[must_use]
    pub fn sample() -> Self {
        Self::new(Spacecraft::sample()).expect("the sample ship builds")
    }

    #[must_use]
    pub fn time(&self) -> f64 {
        self.world.time()
    }

    /// A factory-frame point in the world.
    #[must_use]
    pub fn to_world(&self, p: DVec3) -> DVec3 {
        self.world[self.body].to_world(p)
    }

    /// A world point in the factory frame.
    #[must_use]
    pub fn to_factory(&self, p: DVec3) -> DVec3 {
        let f = &self.world[self.body];
        f.orientation.inverse() * (p - f.pos)
    }

    /// A world orientation in the factory frame.
    #[must_use]
    pub fn orientation_in_factory(&self, q: DQuat) -> DQuat {
        self.world[self.body].orientation.inverse() * q
    }

    /// Measured joint angles of `arm`.
    #[must_use]
    pub fn measured(&self, arm: usize) -> Pose {
        let joints = self.arms[arm].joints;
        std::array::from_fn(|i| {
            self.world
                .joint(joints[i])
                .and_then(|j| j.coordinate(&self.world))
                .unwrap_or(0.0)
        })
    }

    /// Start a move of `arm` from where it is to `to`, as fast as its
    /// speed limits allow scaled by `pace` (1 is full speed).
    pub fn move_arm(&mut self, arm: usize, to: Pose, pace: f64) {
        let from = self.measured(arm);
        // A minimum-jerk profile peaks at 1.875 times the mean speed.
        let duration = (0..5)
            .map(|i| (to[i] - from[i]).abs() * 1.875 / (arm::SPEED[i] * pace))
            .fold(2.0_f64, f64::max);
        self.arms[arm].motion = Some(Motion {
            from,
            to,
            start: self.time(),
            duration,
        });
    }

    /// Stop `arm` where it is.
    pub fn hold_arm(&mut self, arm: usize) {
        let here = self.measured(arm);
        self.arms[arm].motion = None;
        for (i, &q) in here.iter().enumerate() {
            let _ = self.world.set_joint_target(self.arms[arm].joints[i], q);
        }
    }

    /// A controller restart: the arm loses its move and brakes where it is.
    pub fn reset_controller(&mut self, arm: usize) {
        self.hold_arm(arm);
        self.arms[arm].epoch += 1;
    }

    /// The arm's reference pose now, or `None` when it is not moving.
    #[must_use]
    pub fn reference(&self, arm: usize) -> Option<Pose> {
        self.arms[arm].motion.map(|m| m.at(self.time()))
    }

    /// One physics step: arm servos follow their moves, the CMGs hold the
    /// factory's attitude, and the world advances.
    pub fn step(&mut self) {
        let t = self.time() + DT;
        for a in 0..self.arms.len() {
            if let Some(motion) = self.arms[a].motion {
                let q = motion.at(t);
                for (i, &target) in q.iter().enumerate() {
                    let _ = self.world.set_joint_target(self.arms[a].joints[i], target);
                }
            }
        }
        if self.attitude.enabled {
            let f = &self.world[self.body];
            let mut error = f.orientation * self.attitude.target.inverse();
            if error.w < 0.0 {
                error = -error;
            }
            let (axis, angle) = error.to_axis_angle();
            let wanted = -(axis * angle) * self.attitude.kp - f.omega_world() * self.attitude.kd;
            let torque = wanted.clamp_length_max(self.cmg.max_torque);
            // The deck's torque comes out of the CMGs' stored momentum.
            let h = self.cmg.h - torque * DT;
            if h.length() > self.cmg.capacity {
                self.cmg.saturated = true;
            } else {
                self.cmg.h = h;
                self.world[self.body].apply_torque(torque);
            }
        }
        self.world.step(&NoField);
        for arm in &mut self.arms {
            for (i, &joint) in arm.joints.iter().enumerate() {
                let effort = self
                    .world
                    .joint(joint)
                    .map_or(0.0, |j| j.motor_effort.abs());
                if effort >= arm::EFFORT[i] * 0.999 {
                    arm.saturated[i] += DT;
                } else {
                    arm.saturated[i] = 0.0;
                }
            }
        }
    }

    /// Momentum of every body plus the CMGs' stored momentum, about the
    /// world origin. Nothing outside acts on the factory, so this is
    /// constant.
    #[must_use]
    pub fn momentum(&self) -> Momentum {
        let mut m = self.world.momentum(DVec3::ZERO);
        m.angular += self.cmg.h;
        m
    }

    /// The part's position and attitude errors from its place in the ship,
    /// factory frame: meters and radians.
    #[must_use]
    pub fn placement_error(&self, part: usize) -> (f64, f64) {
        let p = &self.parts[part];
        let body = &self.world[p.body];
        let at = self.to_factory(body.pos);
        let q = self.orientation_in_factory(body.orientation);
        let want = DQuat::from_rotation_y(p.design_yaw);
        (at.distance(p.design), q.angle_between(want))
    }

    /// Relative speed of the part to the factory at its center, m/s.
    #[must_use]
    pub fn relative_speed(&self, part: usize) -> f64 {
        let f = &self.world[self.body];
        let b = &self.world[self.parts[part].body];
        let frame = f.vel + f.omega_world().cross(b.pos - f.pos);
        (b.vel - frame).length()
    }

    /// The grapple point on top of a part centered at `center` (factory
    /// frame).
    #[must_use]
    pub fn grapple(&self, part: usize, center: DVec3) -> DVec3 {
        center + DVec3::Y * self.parts[part].half.y
    }

    /// Close the arm's grapple on the part, then open the rack clamp.
    ///
    /// # Errors
    /// The part is not on its rack or the tool is too far from its
    /// grapple.
    pub fn grasp(&mut self, arm: usize, part: usize) -> Result<(), String> {
        let Holder::Rack(clamp) = self.parts[part].holder else {
            return Err("The part is not on its rack".into());
        };
        let tool = self.arms[arm].links[4];
        let tip = self.world[tool].to_world(DVec3::Y * TOOL / 2.0);
        let top = self.world[self.parts[part].body].to_world(DVec3::Y * self.parts[part].half.y);
        if tip.distance(top) > 0.1 {
            return Err(format!(
                "The grapple is {:.2} m from the part's fixture",
                tip.distance(top)
            ));
        }
        let grip = self.world.add_joint(
            Joint::weld_here(&self.world, tool, self.parts[part].body, tip)
                .limited(GRAPPLE_FORCE, GRAPPLE_FORCE * 2.0),
        );
        self.world.remove_joint(clamp);
        self.parts[part].holder = Holder::Arm { arm, grip };
        Ok(())
    }

    /// Close the part's latch at its place in the ship. The latch pulls the
    /// part into exact alignment from within its capture envelope.
    ///
    /// # Errors
    /// The part is not held, or its parent is not latched yet.
    pub fn latch(&mut self, part: usize) -> Result<(), String> {
        let Holder::Arm { arm, grip } = self.parts[part].holder else {
            return Err("The part is not in an arm".into());
        };
        let spec = &self.ship.parts[self.parts[part].index];
        let at = CRADLE + spec.attach.at;
        let (parent_body, parent_center, parent_yaw) = match &spec.attach.parent {
            None => (self.body, DVec3::ZERO, 0.0),
            Some(id) => {
                let p = self
                    .parts
                    .iter()
                    .find(|p| self.ship.parts[p.index].id == *id)
                    .ok_or("The parent part is missing")?;
                if !matches!(p.holder, Holder::Latched(_)) {
                    return Err(format!("{id} is not latched yet"));
                }
                (p.body, p.design, p.design_yaw)
            }
        };
        let parent_q = DQuat::from_rotation_y(parent_yaw);
        let own_q = DQuat::from_rotation_y(self.parts[part].design_yaw);
        let joint = Joint::new(
            parent_body,
            parent_q.inverse() * (at - parent_center),
            self.parts[part].body,
            own_q.inverse() * (at - self.parts[part].design),
            JointKind::Weld {
                relative: parent_q.inverse() * own_q,
            },
        )
        .limited(spec.attach.rated_force_n, spec.attach.rated_moment_nm);
        let latch = self.world.add_joint(joint);
        self.parts[part].holder = Holder::Both { arm, grip, latch };
        Ok(())
    }

    /// Open the arm's grapple on a latched part.
    ///
    /// # Errors
    /// The part is not both held and latched.
    pub fn release(&mut self, part: usize) -> Result<(), String> {
        let Holder::Both { grip, latch, .. } = self.parts[part].holder else {
            return Err("The part is not latched in an arm".into());
        };
        self.world.remove_joint(grip);
        self.parts[part].holder = Holder::Latched(latch);
        Ok(())
    }

    /// The latch's joint, if the part is latched.
    #[must_use]
    pub fn latch_joint(&self, part: usize) -> Option<&Joint> {
        match self.parts[part].holder {
            Holder::Latched(j) | Holder::Both { latch: j, .. } => self.world.joint(j),
            _ => None,
        }
    }

    /// The part's box, factory frame, as it is now.
    #[must_use]
    pub fn part_box(&self, part: usize) -> Obb {
        let b = &self.world[self.parts[part].body];
        Obb {
            center: self.to_factory(b.pos),
            orientation: self.orientation_in_factory(b.orientation),
            half: self.parts[part].half,
        }
    }

    /// The part's box at a pose given as center and yaw, factory frame.
    #[must_use]
    pub fn part_box_at(&self, part: usize, center: DVec3, yaw: f64) -> Obb {
        Obb {
            center,
            orientation: DQuat::from_rotation_y(yaw),
            half: self.parts[part].half,
        }
    }

    /// Link segments of an arm at `pose`, factory frame, from its design
    /// kinematics: (name, from, to).
    #[must_use]
    pub fn link_segments(&self, arm: usize, pose: &Pose) -> Vec<(&'static str, DVec3, DVec3)> {
        let f = self.arms[arm].spec.frames(pose);
        (0..4)
            .map(|i| (LINK_NAMES[i], f.points[i], f.points[i + 1]))
            .collect()
    }

    /// Link segments of an arm as its bodies are now, factory frame.
    #[must_use]
    pub fn link_segments_now(&self, arm: usize) -> Vec<(&'static str, DVec3, DVec3)> {
        let a = &self.arms[arm];
        let lengths = link_lengths(&a.spec);
        (0..5)
            .map(|i| {
                let b = &self.world[a.links[i]];
                let half = DVec3::Y * lengths[i] / 2.0;
                (
                    LINK_NAMES[i],
                    self.to_factory(b.to_world(-half)),
                    self.to_factory(b.to_world(half)),
                )
            })
            .collect()
    }

    /// Clearance of the scene with arm `arm` at `pose` holding `held` (a
    /// part index, placed at `held_box`), against every other part, the
    /// other arms as they are now, and the deck. `ignore` names parts the
    /// held part may touch (its parent at the latch). The tool may touch
    /// the part it holds.
    #[must_use]
    pub fn clearance(
        &self,
        arm: usize,
        pose: &Pose,
        held: Option<(usize, Obb)>,
        ignore: &[usize],
    ) -> Option<Contact> {
        let segments = self.link_segments(arm, pose);
        let r = arm::LINK_RADIUS;
        let name = |i: usize| self.ship.parts[self.parts[i].index].id.clone();
        for p in 0..self.parts.len() {
            if held.is_some_and(|(h, _)| h == p) {
                continue;
            }
            let obb = self.part_box(p).shrunk(0.05);
            for &(link, a, b) in &segments {
                if capsule_hits(a, b, r, &obb) {
                    return Some(Contact {
                        a: format!("arm {} {link}", arm + 1),
                        b: name(p),
                    });
                }
            }
            if let Some((h, held_box)) = held
                && !ignore.contains(&p)
                && held_box.shrunk(0.05).overlaps(&obb)
            {
                return Some(Contact {
                    a: name(h),
                    b: name(p),
                });
            }
        }
        // The deck: no link or held part below it.
        let deck = Obb {
            center: DVec3::new(0.0, DECK - DECK_SIZE.y / 2.0, 0.0),
            orientation: DQuat::IDENTITY,
            half: DECK_SIZE / 2.0,
        };
        for &(link, a, b) in segments.iter().skip(1) {
            if capsule_hits(a, b, r * 0.5, &deck.shrunk(0.1)) {
                return Some(Contact {
                    a: format!("arm {} {link}", arm + 1),
                    b: "deck".into(),
                });
            }
        }
        if let Some((h, held_box)) = held
            && held_box.shrunk(0.05).overlaps(&deck)
        {
            return Some(Contact {
                a: name(h),
                b: "deck".into(),
            });
        }
        // The other arms where they stand.
        for other in 0..self.arms.len() {
            if other == arm {
                continue;
            }
            for (ln, oa, ob) in self.link_segments_now(other) {
                for &(link, a, b) in &segments {
                    if crate::geometry::segment_distance(a, b, oa, ob) < 2.0 * r {
                        return Some(Contact {
                            a: format!("arm {} {link}", arm + 1),
                            b: format!("arm {} {ln}", other + 1),
                        });
                    }
                }
                if let Some((h, held_box)) = held
                    && capsule_hits(oa, ob, r, &held_box.shrunk(0.05))
                {
                    return Some(Contact {
                        a: name(h),
                        b: format!("arm {} {ln}", other + 1),
                    });
                }
            }
        }
        None
    }

    /// The same check for the scene as it is now (bodies, not design
    /// kinematics).
    #[must_use]
    pub fn clearance_now(&self, arm: usize, ignore: &[usize]) -> Option<Contact> {
        let pose = self.measured(arm);
        let held = self.parts.iter().position(|p| {
            matches!(p.holder, Holder::Arm { arm: a, .. } | Holder::Both { arm: a, .. } if a == arm)
        });
        self.clearance(arm, &pose, held.map(|h| (h, self.part_box(h))), ignore)
    }

    /// Whether every part is held by a rack, an arm, or a latch.
    #[must_use]
    pub fn all_restrained(&self) -> bool {
        self.parts.iter().all(|p| {
            let id = match p.holder {
                Holder::Rack(j) | Holder::Latched(j) => vec![j],
                Holder::Arm { grip, .. } => vec![grip],
                Holder::Both { grip, latch, .. } => vec![grip, latch],
            };
            id.iter().all(|&j| self.world.joint(j).is_some())
        })
    }

    /// Mass properties of the latched parts, in the ship frame: what the
    /// assembled ship weighs, from the bodies the arms put together.
    #[must_use]
    pub fn assembled(&self) -> crate::ship::MassProperties {
        let bodies: Vec<&Body> = self
            .parts
            .iter()
            .filter(|p| matches!(p.holder, Holder::Latched(_) | Holder::Both { .. }))
            .map(|p| &self.world[p.body])
            .collect();
        let c = physics::Composite::of(bodies.iter().copied());
        let r = glam::DMat3::from_quat(self.world[self.body].orientation);
        crate::ship::MassProperties {
            mass: c.mass,
            com: self.to_factory(c.com) - CRADLE,
            inertia: r.transpose() * c.inertia * r,
        }
    }

    /// Every body together: the deck, the arms, and the parts.
    #[must_use]
    pub fn system(&self) -> physics::Composite {
        let mut bodies: Vec<&Body> = vec![&self.world[self.body]];
        for arm in &self.arms {
            bodies.extend(arm.links.iter().map(|&b| &self.world[b]));
        }
        bodies.extend(self.parts.iter().map(|p| &self.world[p.body]));
        physics::Composite::of(bodies.iter().copied())
    }

    /// The factory's attitude error from its target, rad.
    #[must_use]
    pub fn attitude_error(&self) -> f64 {
        self.world[self.body]
            .orientation
            .angle_between(self.attitude.target)
    }
}

/// Lengths of the five arm bodies: turret, upper arm, forearm, wrist, tool.
#[must_use]
pub fn link_lengths(spec: &ArmSpec) -> [f64; 5] {
    [
        spec.shoulder,
        spec.upper,
        spec.forearm,
        spec.wrist - TOOL,
        TOOL,
    ]
}

/// The arm's parking pose: the tool 8 m out toward the cradle and 18 m
/// above the deck, pointing down.
pub(crate) fn home_pose(spec: &ArmSpec) -> Result<Pose, String> {
    let heading = DQuat::from_rotation_y(spec.yaw) * DVec3::X;
    spec.solve(spec.base + heading * 8.0 + DVec3::Y * 16.0, spec.yaw)
}

pub(crate) fn build_arm(
    world: &mut physics::World,
    factory: BodyId,
    spec: ArmSpec,
    home: Pose,
) -> Result<Arm, String> {
    let frames = spec.frames(&home);
    let lengths = link_lengths(&spec);
    // Link start points: base, shoulder, elbow, wrist flex, roll joint.
    let q = frames.orientations;
    let roll_point = frames.points[4] - q[3] * DVec3::Y * TOOL;
    let starts = [
        frames.points[0],
        frames.points[1],
        frames.points[2],
        frames.points[3],
        roll_point,
    ];
    let mut links = [BodyId(0); 5];
    for i in 0..5 {
        let mass = arm::MASSES[i];
        let size = DVec3::new(LINK_MASS_WIDTH, lengths[i], LINK_MASS_WIDTH);
        let center = starts[i] + q[i] * DVec3::Y * (lengths[i] / 2.0);
        let mut body = Body::new(mass, Body::box_inertia(mass, size), center);
        body.orientation = q[i];
        body.prev_orientation = q[i];
        links[i] = world.add(body);
    }
    let mut joints = [JointId(0); 5];
    for i in 0..5 {
        let (parent, anchor_a, axis, relative) = if i == 0 {
            (
                factory,
                spec.base,
                DVec3::Y,
                DQuat::from_rotation_y(spec.yaw),
            )
        } else {
            let axis = if i == 4 { DVec3::Y } else { -DVec3::Z };
            (
                links[i - 1],
                DVec3::Y * (lengths[i - 1] / 2.0),
                axis,
                ArmSpec::zero_relative(i),
            )
        };
        let j = arm::ArmSpec::joint(i);
        let joint = Joint::new(
            parent,
            anchor_a,
            links[i],
            -DVec3::Y * (lengths[i] / 2.0),
            JointKind::Hinge {
                axis,
                relative,
                limits: Some(JointLimits {
                    lower: j.lower,
                    upper: j.upper,
                }),
                servo: Some(Servo {
                    target: home[i],
                    kp: 1.0e12,
                    kd: 1.0,
                    max_effort: j.effort,
                    max_speed: j.speed * 1.5,
                }),
            },
        );
        joints[i] = world
            .try_add_joint(joint)
            .map_err(|e| format!("arm joint {}: {e}", arm::JOINTS[i]))?;
    }
    Ok(Arm {
        spec,
        links,
        joints,
        motion: None,
        epoch: 0,
        saturated: [0.0; 5],
        home,
    })
}
