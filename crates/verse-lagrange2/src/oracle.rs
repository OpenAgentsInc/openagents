//! The arm's MuJoCo cross-check (sim-mapping's
//! `arms_match_mujoco_for_a_reference_move`): one factory arm, Robot Lab's
//! SO-101 at 100 times its size, carrying a loaded tank through a
//! reference move, written as MuJoCo XML with the same bodies, joints,
//! limits, and servo law as the `physics` arm the factory builds.
//!
//! MuJoCo 3.3.7 runs it offline through Robot Lab's native runner
//! (`cargo run -p robot-sim --features mujoco --example lagrange2_arm_oracle`)
//! and writes `oracle/arm-reference.json`. The test replays the same
//! targets in `physics` and compares the joint angles; nothing here links
//! MuJoCo.
//!
//! The servo: `physics` turns each target into a speed command
//! `kp (target - q) / (kd + dt kp)`, capped at the joint's speed, which a
//! stiff velocity constraint capped at the joint's effort then follows.
//! Robot Lab's runner commands `kp_m (target - q) / kd_m` through a
//! position actuator, which is a velocity servo of gain `kd_m` capped at
//! the same effort and integrated implicitly. With `kp_m / kd_m` equal to
//! `kp / (kd + dt kp)` the two command the same speed.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::arm::{self, ArmSpec, Pose};
use crate::factory::{self, DT, Motion, TOOL};
use crate::ship::{PartKind, Spacecraft};

/// The reference file's schema tag.
pub const SCHEMA: &str = "lagrange2.arm-reference.v1";
/// The checked-in reference.
pub const REFERENCE: &str = include_str!("../oracle/arm-reference.json");
/// The largest joint-angle difference accepted, rad: the articulated
/// spike's tolerance for its six-link chain (#11607,
/// crates/physics/oracle/README.md).
pub const TOLERANCE: f64 = 0.025;
/// MuJoCo's velocity gain for the factory servos, N m s/rad. MuJoCo's
/// implicit integrator takes the gain into the step's mass matrix even
/// while the effort cap clips the force, so a far stiffer gain (10^10)
/// stalls the joints whenever a speed error briefly saturates them; at
/// 10^8 the servo follows its command (`dt kd` is about 3 × 10^6 kg m^2,
/// the joints' own inertia) and the cap rarely engages.
pub const MUJOCO_KD: f64 = 1.0e8;
/// MuJoCo solver settings for the reference.
pub const MUJOCO_ITERATIONS: u32 = 100;

/// The physics servo gains the factory arm uses (`factory::build_arm`).
pub const SERVO_KP: f64 = 1.0e12;
pub const SERVO_KD: f64 = 1.0;
/// The factory servo's speed cap over the joint's speed limit.
pub const SPEED_MARGIN: f64 = 1.5;

/// The move: one arm on a fixed pedestal, holding a loaded tank, from its
/// parking pose to over the cradle at the transit height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceMove {
    pub spec: ArmSpec,
    pub motion: Motion,
    pub payload_kg: f64,
    /// Principal moments about the payload's center, its own frame.
    pub payload_inertia: DVec3,
    /// From the tool's grapple face to the payload's center, m.
    pub payload_drop: f64,
    /// Steps to record, and every how many steps.
    pub steps: usize,
    pub record_every: usize,
}

/// One recorded run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub schema: String,
    pub mujoco: String,
    /// SHA-256 of the MuJoCo library that ran it.
    pub library_sha256: String,
    pub timestep: f64,
    pub model_xml: String,
    pub record_every: usize,
    /// Joint angles after each recorded step: pan, lift, elbow, wrist,
    /// roll (measured from the arm's zeros, as `Factory::measured`).
    pub angles: Vec<Pose>,
}

impl ReferenceMove {
    /// Arm 1 of the sample factory, with the sample's port tank in its
    /// grapple, moving from its parking pose to over the tank's place at
    /// the transit height at full speed.
    #[must_use]
    pub fn sample() -> Self {
        let ship = Spacecraft::sample();
        let tank = ship
            .parts
            .iter()
            .find(|p| matches!(p.kind, PartKind::Tank { .. }))
            .expect("the sample has a tank");
        let [x, z] = factory::ARM_BASES[0];
        let spec = ArmSpec::so101_scaled(
            DVec3::new(x, factory::DECK + factory::PEDESTAL, z),
            -std::f64::consts::FRAC_PI_2,
        );
        let from = factory::home_pose(&spec).expect("the parking pose solves");
        let half = tank.shape.half_extents();
        let target = factory::CRADLE
            + tank.position
            + DVec3::Y * (factory::TRANSIT + 2.0 * half.y - factory::CRADLE.y - tank.position.y);
        let to = spec
            .solve(target, tank.yaw)
            .expect("the transit pose solves");
        let duration = (0..5)
            .map(|i| (to[i] - from[i]).abs() * 1.875 / arm::SPEED[i])
            .fold(2.0_f64, f64::max);
        let mass = tank.mass();
        Self {
            spec,
            motion: Motion {
                from,
                to,
                start: 0.0,
                duration,
            },
            payload_kg: mass,
            payload_inertia: tank.shape.inertia(mass),
            payload_drop: half.y,
            steps: (duration / DT).ceil() as usize + 300,
            record_every: 5,
        }
    }

    /// Joint targets for step `k` (applied before the step).
    #[must_use]
    pub fn target(&self, k: usize) -> Pose {
        self.motion.at(DT * (k + 1) as f64)
    }

    /// The servo MuJoCo needs for joint `i`: (kp, kd, effort, speed).
    #[must_use]
    pub fn mujoco_servo(i: usize) -> (f64, f64, f64, f64) {
        let ratio = SERVO_KP / (SERVO_KD + DT * SERVO_KP);
        (
            MUJOCO_KD * ratio,
            MUJOCO_KD,
            arm::EFFORT[i],
            arm::SPEED[i] * SPEED_MARGIN,
        )
    }

    /// The arm as MuJoCo XML, in the factory's frame (+Y up), no gravity.
    #[must_use]
    pub fn mjcf(&self) -> String {
        let lengths = factory::link_lengths(&self.spec);
        let q = |r: DQuat| format!("{} {} {} {}", r.w, r.x, r.y, r.z);
        let v = |p: DVec3| format!("{} {} {}", p.x, p.y, p.z);
        let names = ["turret", "upper_arm", "forearm", "wrist", "tool"];
        let mut body = String::new();
        for i in 0..5 {
            let (pos, quat, axis) = if i == 0 {
                (
                    self.spec.base,
                    DQuat::from_rotation_y(self.spec.yaw),
                    DVec3::Y,
                )
            } else {
                let axis = if i == 4 { DVec3::Y } else { -DVec3::Z };
                (DVec3::Y * lengths[i - 1], ArmSpec::zero_relative(i), axis)
            };
            let (lo, hi) = arm::LIMITS[i];
            let mass = arm::MASSES[i];
            let inertia = physics::Body::box_inertia(
                mass,
                DVec3::new(
                    factory::LINK_MASS_WIDTH,
                    lengths[i],
                    factory::LINK_MASS_WIDTH,
                ),
            );
            body += &format!(
                "<body name=\"{}\" pos=\"{}\" quat=\"{}\"><joint name=\"{}\" type=\"hinge\" axis=\"{}\" limited=\"true\" range=\"{lo} {hi}\"/><inertial pos=\"{}\" mass=\"{mass}\" diaginertia=\"{}\"/>",
                names[i],
                v(pos),
                q(quat),
                arm::JOINTS[i],
                v(axis),
                v(DVec3::Y * (lengths[i] / 2.0)),
                v(inertia),
            );
        }
        // The payload hangs from the grapple face at the tool's end (the
        // tool body's frame is at its roll joint), turned by the nominal
        // grip (a half turn about Z, which leaves its principal moments on
        // the same axes).
        body += &format!(
            "<body name=\"payload\" pos=\"{}\" quat=\"{}\"><inertial pos=\"0 0 0\" mass=\"{}\" diaginertia=\"{}\"/></body>",
            v(DVec3::Y * (TOOL + self.payload_drop)),
            q(arm::nominal_grip()),
            self.payload_kg,
            v(self.payload_inertia),
        );
        body += &"</body>".repeat(5);
        let actuators: String = arm::JOINTS
            .iter()
            .map(|j| format!("<position joint=\"{j}\" kp=\"1\"/>"))
            .collect();
        format!(
            "<mujoco model=\"lagrange2-arm\"><compiler angle=\"radian\"/><option timestep=\"{DT}\" gravity=\"0 0 0\" integrator=\"implicitfast\" iterations=\"{MUJOCO_ITERATIONS}\" tolerance=\"1e-12\"/><worldbody>{body}</worldbody><actuator>{actuators}</actuator></mujoco>"
        )
    }
}

impl Reference {
    /// The checked-in reference.
    ///
    /// # Errors
    /// It doesn't parse or isn't this schema.
    pub fn checked_in() -> Result<Self, String> {
        let reference: Self = serde_json::from_str(REFERENCE).map_err(|e| e.to_string())?;
        if reference.schema != SCHEMA {
            return Err(format!("expected {SCHEMA}, found {}", reference.schema));
        }
        Ok(reference)
    }
}
