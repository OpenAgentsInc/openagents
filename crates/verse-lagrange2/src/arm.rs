//! The factory arm: Robot Lab's SO-101 at 100 times its size.
//!
//! The arm keeps the SO-101's five joints, their order, and their limits
//! (from the pinned `so101_new_calib.urdf`), and its link lengths scaled by
//! [`SCALE`]. Masses and motor ratings are not scaled geometrically (a solid
//! 100× arm would weigh hundreds of tonnes); they are sized like flight
//! arms (Canadarm2 is 17 m and about 1.5 t) and from the loads the arm
//! moves. The motor model is Robot Lab's: an effort-capped, speed-capped
//! position servo per joint.
//!
//! Frames: the arm stands on the factory deck at `base`, turned `yaw` about
//! the deck normal (+Y), so that at a zero pan angle it reaches along its
//! local +X. With every joint at zero the arm points straight up. The three
//! pitch joints turn about the local -Z axis, so a positive angle leans the
//! arm toward +X; the roll joint turns the tool about the link's own axis.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

/// How much bigger than the SO-101 the factory arm is.
pub const SCALE: f64 = 100.0;
/// Joint names, in chain order, as the SO-101 URDF names them.
pub const JOINTS: [&str; 5] = [
    "shoulder_pan",
    "shoulder_lift",
    "elbow_flex",
    "wrist_flex",
    "wrist_roll",
];
/// Each joint's zero as a geometric angle, rad. The SO-101's own zeros sit
/// at bends (its URDF turns the elbow and wrist frames by a quarter turn);
/// the factory arm puts the elbow's zero at a right-angle bend and the
/// wrist's at 45°, so the URDF's travel on either side of zero covers the
/// reach-out-and-point-down poses the factory works in. Joint values and
/// limits are measured from these zeros.
pub const ZERO: [f64; 5] = [
    0.0,
    0.0,
    std::f64::consts::FRAC_PI_2,
    std::f64::consts::FRAC_PI_4,
    0.0,
];
/// Joint limits from the SO-101 URDF, rad.
pub const LIMITS: [(f64, f64); 5] = [
    (-1.919_86, 1.919_86),
    (-1.745_33, 1.745_33),
    (-1.69, 1.69),
    (-1.658_06, 1.658_06),
    (-2.743_85, 2.841_21),
];
/// SO-101 link lengths, m: base to shoulder (pan offset plus lift offset
/// heights), upper arm, forearm, and wrist flex to the gripper frame.
pub const SO101_LENGTHS: [f64; 4] = [0.0624 + 0.0542, 0.116, 0.1350, 0.0611 + 0.0981];
/// Link masses, kg: turret, upper arm, forearm, wrist, and tool.
pub const MASSES: [f64; 5] = [4_000.0, 3_000.0, 3_000.0, 2_000.0, 1_000.0];
/// Joint effort limits, N m. The shoulder and elbow carry a 25 t tank at
/// 20 m with margin for 0.002 rad/s^2 (1,000 kN m would be needed only
/// for 0.0025 rad/s^2 at 25 m with the arm's own inertia).
pub const EFFORT: [f64; 5] = [600_000.0, 600_000.0, 450_000.0, 250_000.0, 120_000.0];
/// Joint speed limits, rad/s: slow, as flight arms move heavy loads, to
/// keep the momentum the factory's attitude control absorbs small.
pub const SPEED: [f64; 5] = [0.03, 0.03, 0.03, 0.04, 0.05];
/// Radius of the arm's links for clearance checks, m.
pub const LINK_RADIUS: f64 = 0.7;

/// A joint's servo command and limits.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointSpec {
    pub lower: f64,
    pub upper: f64,
    pub effort: f64,
    pub speed: f64,
}

/// One arm's geometry and mounting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmSpec {
    /// Base point on the deck, factory frame, m.
    pub base: DVec3,
    /// Turn of the base about +Y, rad: zero pan reaches along this heading.
    pub yaw: f64,
    /// Shoulder height above the base, m.
    pub shoulder: f64,
    pub upper: f64,
    pub forearm: f64,
    /// Wrist flex axis to the tool's grapple face, m.
    pub wrist: f64,
}

/// A joint vector: pan, lift, elbow, wrist, roll.
pub type Pose = [f64; 5];

/// Every link's frame for one pose, factory frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frames {
    /// Turret base, shoulder, elbow, wrist, and tool points.
    pub points: [DVec3; 5],
    /// Orientation of the turret, upper arm, forearm, wrist, and tool.
    pub orientations: [DQuat; 5],
}

impl ArmSpec {
    #[must_use]
    pub fn so101_scaled(base: DVec3, yaw: f64) -> Self {
        Self {
            base,
            yaw,
            shoulder: SO101_LENGTHS[0] * SCALE,
            upper: SO101_LENGTHS[1] * SCALE,
            forearm: SO101_LENGTHS[2] * SCALE,
            wrist: SO101_LENGTHS[3] * SCALE,
        }
    }

    #[must_use]
    pub fn joint(i: usize) -> JointSpec {
        JointSpec {
            lower: LIMITS[i].0,
            upper: LIMITS[i].1,
            effort: EFFORT[i],
            speed: SPEED[i],
        }
    }

    /// Link lengths along each moving link's +Y axis: turret (to the
    /// shoulder), upper arm, forearm, wrist.
    #[must_use]
    pub fn lengths(&self) -> [f64; 4] {
        [self.shoulder, self.upper, self.forearm, self.wrist]
    }

    /// A link's zero-angle orientation relative to its parent link: the
    /// joint's `relative` rotation in the physics hinge.
    #[must_use]
    pub fn zero_relative(i: usize) -> DQuat {
        DQuat::from_axis_angle(-DVec3::Z, ZERO[i])
    }

    /// Orientation of each link for `pose`, factory frame.
    #[must_use]
    pub fn orientations(&self, pose: &Pose) -> [DQuat; 5] {
        let turret = DQuat::from_rotation_y(self.yaw + pose[0]);
        let pitch = |a: f64| DQuat::from_axis_angle(-DVec3::Z, a);
        let upper = turret * pitch(pose[1] + ZERO[1]);
        let forearm = upper * pitch(pose[2] + ZERO[2]);
        let wrist = forearm * pitch(pose[3] + ZERO[3]);
        let tool = wrist * DQuat::from_rotation_y(pose[4]);
        [turret, upper, forearm, wrist, tool]
    }

    /// Forward kinematics.
    #[must_use]
    pub fn frames(&self, pose: &Pose) -> Frames {
        let q = self.orientations(pose);
        let shoulder = self.base + DVec3::Y * self.shoulder;
        let elbow = shoulder + q[1] * DVec3::Y * self.upper;
        let wrist = elbow + q[2] * DVec3::Y * self.forearm;
        let tool = wrist + q[3] * DVec3::Y * self.wrist;
        Frames {
            points: [self.base, shoulder, elbow, wrist, tool],
            orientations: q,
        }
    }

    /// Joint angles that put the tool's grapple face at `tool` (factory
    /// frame) pointing straight down (-Y), turned so a part held with the
    /// nominal grip sits at `part_yaw`. The elbow bends up.
    ///
    /// # Errors
    /// The point is out of reach or needs a joint beyond its limits.
    pub fn solve(&self, tool: DVec3, part_yaw: f64) -> Result<Pose, String> {
        let d = tool - self.base;
        let heading = (-d.z).atan2(d.x);
        let pan = wrap(heading - self.yaw);
        let r = (d.x * d.x + d.z * d.z).sqrt();
        // The wrist sits one wrist length above the tool point.
        let wy = d.y + self.wrist - self.shoulder;
        let reach2 = r * r + wy * wy;
        let (l1, l2) = (self.upper, self.forearm);
        let cos_elbow = (reach2 - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
        if !(-1.0..=1.0).contains(&cos_elbow) {
            return Err(format!(
                "out of reach: the wrist would be {:.1} m from the shoulder",
                reach2.sqrt()
            ));
        }
        let elbow = cos_elbow.acos();
        // Angle of the shoulder-wrist line from vertical, toward +X.
        let alpha = r.atan2(wy);
        let beta = (l2 * elbow.sin()).atan2(l1 + l2 * elbow.cos());
        let lift = alpha - beta;
        let flex = std::f64::consts::PI - lift - elbow;
        let roll = wrap(self.yaw + pan - part_yaw);
        let pose = [pan, lift - ZERO[1], elbow - ZERO[2], flex - ZERO[3], roll];
        check_limits(&pose)?;
        Ok(pose)
    }

    /// The part's orientation when held with the nominal grip at `pose`.
    #[must_use]
    pub fn held_orientation(&self, pose: &Pose) -> DQuat {
        self.orientations(pose)[4] * nominal_grip()
    }
}

/// The grip that holds a part upright under a tool pointing down: the
/// part's frame relative to the tool's.
#[must_use]
pub fn nominal_grip() -> DQuat {
    DQuat::from_axis_angle(-DVec3::Z, std::f64::consts::PI).inverse()
}

/// Whether every joint is inside its limits.
///
/// # Errors
/// Names the first joint outside its range.
pub fn check_limits(pose: &Pose) -> Result<(), String> {
    for (i, (&q, &(lo, hi))) in pose.iter().zip(&LIMITS).enumerate() {
        if !(q.is_finite() && (lo..=hi).contains(&q)) {
            return Err(format!(
                "{} would be at {:.1}°, outside {:.1}° to {:.1}°",
                JOINTS[i],
                q.to_degrees(),
                lo.to_degrees(),
                hi.to_degrees()
            ));
        }
    }
    Ok(())
}

/// An angle in [-pi, pi).
#[must_use]
pub fn wrap(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}
