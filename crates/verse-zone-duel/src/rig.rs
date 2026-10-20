//! Each fighter's pose: a small skeleton, the weapon, and the shield, from
//! what the fighter is doing. The same pose draws the fighter
//! ([`crate::draw`]) and carries the blade and the body zones the hit test
//! sweeps ([`crate::hits`]), so what you see swing is what strikes.
//!
//! Motions are written in the fighter's own frame as `(right, up,
//! forward)` meters from the feet, then turned to the fighter's facing.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Mat3, Quat, Vec3};

use crate::rules::{
    Action, After, BASH_TOTAL, Dir, Fighter, Kit, PARRY_CLOSE, PARRY_OPEN, PARRY_TOTAL, STEP_TIME,
};

/// Bone lengths, m.
pub const UPPER_ARM: f32 = 0.31;
pub const FOREARM: f32 = 0.29;
pub const THIGH: f32 = 0.45;
pub const SHIN: f32 = 0.44;
pub const HIP_HEIGHT: f32 = 0.93;
pub const SHOULDER_HEIGHT: f32 = 1.44;
pub const HEAD_HEIGHT: f32 = 1.66;
/// How long a fall takes, s.
pub const FALL_TIME: f32 = 0.85;

/// A fighter's pose in world space. Index 0 is the left (shield) side and
/// 1 the right (weapon) side.
#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    pub pelvis: Vec3,
    pub chest: Vec3,
    pub neck: Vec3,
    pub head: Vec3,
    pub shoulders: [Vec3; 2],
    pub elbows: [Vec3; 2],
    pub hands: [Vec3; 2],
    pub hips: [Vec3; 2],
    pub knees: [Vec3; 2],
    pub feet: [Vec3; 2],
    /// The weapon's grip and the unit direction to its point or edge.
    pub grip: Vec3,
    pub blade: Vec3,
    pub length: f32,
    /// The shield's center and orientation: its local +Z is the face's
    /// outward normal and +Y its top.
    pub shield_at: Vec3,
    pub shield_rot: Quat,
    /// The body's orientation: facing, lean, and fall.
    pub body: Quat,
    /// The chest's orientation, with the twist of a swing.
    pub torso: Quat,
}

impl Rig {
    /// The weapon's point or edge.
    #[must_use]
    pub fn tip(&self) -> Vec3 {
        self.grip + self.blade * self.length
    }
}

/// `(right, up, forward)` to a world offset for `yaw`.
#[must_use]
pub fn local(yaw: f32, rfu: Vec3) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::new(-rfu.x, rfu.y, rfu.z)
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A swing's speed curve: slow off the chamber, fastest in the middle, and
/// slowing into the follow-through.
fn swing_ease(s: f32) -> f32 {
    let s = s.clamp(0.0, 1.0);
    // Between smoothstep and linear, so the blade is moving fast through
    // most of the release.
    0.5 * smooth(s) + 0.5 * s
}

fn dir_of(yaw_r: f32, elevation: f32) -> Vec3 {
    // `yaw_r` turns from forward toward the right.
    Vec3::new(
        yaw_r.sin() * elevation.cos(),
        elevation.sin(),
        yaw_r.cos() * elevation.cos(),
    )
}

fn bezier(a: Vec3, b: Vec3, c: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;
    a * u * u + b * 2.0 * u * t + c * t * t
}

/// The guard: the weapon held forward and up, point toward the opponent's
/// face.
#[must_use]
pub fn guard(kit: Kit) -> (Vec3, Vec3) {
    match kit {
        Kit::Knight => (
            Vec3::new(0.24, 1.12, 0.3),
            Vec3::new(-0.2, 0.62, 0.76).normalize(),
        ),
        // The axe rests higher, its head over the shield's rim.
        Kit::Prussian => (
            Vec3::new(0.26, 1.18, 0.26),
            Vec3::new(-0.1, 0.8, 0.55).normalize(),
        ),
    }
}

/// The weapon at release progress `s` of a blow in `dir`: the grip and the
/// direction to the point, in the fighter's frame. `s = 0` is the chamber
/// the windup draws back to.
#[must_use]
pub fn swing(dir: Dir, s: f32) -> (Vec3, Vec3) {
    let e = swing_ease(s);
    match dir {
        Dir::Overhead => {
            // Down through the vertical plane, a little to the left.
            let a = (-35.0_f32).to_radians() + e * 165.0_f32.to_radians();
            let grip = Vec3::new(0.12, 1.78, -0.06).lerp(Vec3::new(0.02, 0.98, 0.5), e)
                + Vec3::Z * 0.12 * (PI * e).sin();
            (grip, Vec3::new(-0.1, a.cos(), a.sin()).normalize())
        }
        Dir::Right => {
            // The forehand: from high on the right, down across to the
            // left.
            let yaw = 105.0_f32.to_radians() - e * 175.0_f32.to_radians();
            let el = 40.0_f32.to_radians() - e * 55.0_f32.to_radians();
            let grip = bezier(
                Vec3::new(0.4, 1.62, -0.06),
                Vec3::new(0.22, 1.45, 0.55),
                Vec3::new(-0.22, 1.1, 0.4),
                e,
            );
            (grip, dir_of(yaw, el))
        }
        Dir::Left => {
            // The backhand: from over the left shoulder across to the
            // right.
            let yaw = -105.0_f32.to_radians() + e * 175.0_f32.to_radians();
            let el = 30.0_f32.to_radians() - e * 52.0_f32.to_radians();
            let grip = bezier(
                Vec3::new(-0.3, 1.52, 0.0),
                Vec3::new(-0.04, 1.34, 0.55),
                Vec3::new(0.42, 1.14, 0.34),
                e,
            );
            (grip, dir_of(yaw, el))
        }
        Dir::Thrust => {
            let e = 1.0 - (1.0 - s.clamp(0.0, 1.0)).powi(2);
            let grip = Vec3::new(0.24, 1.16, -0.18).lerp(Vec3::new(0.08, 1.22, 0.64), e);
            let d = Vec3::new(-0.14, 0.06, 1.0)
                .lerp(Vec3::new(-0.06, -0.03, 1.0), e)
                .normalize();
            (grip, d)
        }
    }
}

/// The parry: the blade brought across the front of the body, edge out.
fn parry_pose() -> (Vec3, Vec3) {
    (
        Vec3::new(0.06, 1.42, 0.46),
        Vec3::new(-0.92, 0.38, 0.1).normalize(),
    )
}

fn blend(a: (Vec3, Vec3), b: (Vec3, Vec3), t: f32) -> (Vec3, Vec3) {
    (a.0.lerp(b.0, t), a.1.lerp(b.1, t).normalize_or(b.1))
}

/// Torso twist for a blow in `dir` at the chamber (`windup`) or through
/// the release (`release`), radians toward the right.
fn twist(dir: Dir, phase: f32, release: bool) -> f32 {
    let (chamber, finish) = match dir {
        Dir::Overhead => (0.15, -0.1),
        Dir::Right => (0.55, -0.5),
        Dir::Left => (-0.45, 0.45),
        Dir::Thrust => (0.35, -0.3),
    };
    if release {
        chamber + (finish - chamber) * swing_ease(phase)
    } else {
        chamber * smooth(phase)
    }
}

/// What the weapon arm, the torso, and the body are doing.
struct Motion {
    weapon: (Vec3, Vec3),
    twist: f32,
    /// Forward lean, radians; negative leans back.
    lean: f32,
    /// How far the shield is driven forward, m.
    bash: f32,
    crouch: f32,
}

fn motion(f: &Fighter, now: f32) -> Motion {
    let t = f.elapsed(now);
    let g = guard(f.kit);
    let still = Motion {
        weapon: g,
        twist: 0.0,
        lean: 0.04,
        bash: 0.0,
        crouch: 0.04,
    };
    match f.action {
        Action::Guard | Action::Down => still,
        Action::Windup { dir, dur, .. } => {
            let p = (t / dur).min(1.0);
            Motion {
                weapon: blend(g, swing(dir, 0.0), smooth(p)),
                twist: twist(dir, p, false),
                lean: 0.04 - 0.06 * p,
                ..still
            }
        }
        Action::Release { dir, dur } => {
            let s = (t / dur).min(1.0);
            Motion {
                weapon: swing(dir, s),
                twist: twist(dir, s, true),
                lean: 0.02 + 0.18 * (PI * s).sin(),
                crouch: 0.04 + 0.06 * (PI * s).sin(),
                ..still
            }
        }
        Action::Recover {
            dir,
            from,
            dur,
            why,
        } => {
            let p = smooth(t / dur);
            // A blow stopped by a shield or a parry rebounds before it
            // comes back.
            let rebound = match why {
                After::Blocked | After::Parried => (1.0 - p) * 0.25,
                _ => 0.0,
            };
            let at = swing(dir, (from - rebound).max(0.0));
            let mut m = Motion {
                weapon: blend(at, g, p),
                twist: twist(dir, from, true) * (1.0 - p),
                lean: 0.04 + 0.1 * (1.0 - p),
                ..still
            };
            if why == After::Parried {
                m.lean = -0.12 * (1.0 - p);
            }
            m
        }
        Action::Parry => {
            let up = if t < PARRY_OPEN {
                smooth(t / PARRY_OPEN)
            } else if t < PARRY_CLOSE {
                1.0
            } else {
                1.0 - smooth((t - PARRY_CLOSE) / (PARRY_TOTAL - PARRY_CLOSE))
            };
            Motion {
                weapon: blend(g, parry_pose(), up),
                twist: -0.25 * up,
                ..still
            }
        }
        Action::Bash { .. } => {
            let s = (t / BASH_TOTAL).min(1.0);
            let push = if s < 0.35 {
                -0.12 * smooth(s / 0.35)
            } else if s < 0.55 {
                -0.12 + 0.52 * smooth((s - 0.35) / 0.2)
            } else {
                0.4 * (1.0 - smooth((s - 0.55) / 0.45))
            };
            Motion {
                bash: push,
                twist: -0.35 * push.max(0.0) / 0.4 + 0.2 * (-push).max(0.0) / 0.12,
                lean: 0.04 + 0.25 * push.max(0.0),
                ..still
            }
        }
        Action::Step { .. } => Motion {
            crouch: 0.04 + 0.08 * (PI * (t / STEP_TIME).min(1.0)).sin(),
            ..still
        },
        Action::Stagger { dur } => {
            let k = (1.0 - t / dur).clamp(0.0, 1.0);
            let jolt = (t * 18.0).min(1.0) * k;
            Motion {
                weapon: blend(
                    g,
                    (Vec3::new(0.38, 1.0, 0.1), Vec3::new(0.5, 0.3, 0.8)),
                    jolt,
                ),
                twist: 0.25 * jolt,
                lean: -0.28 * jolt,
                crouch: 0.08 * jolt,
                ..still
            }
        }
    }
}

/// The shield's center and its outward normal and top, in the fighter's
/// frame.
fn shield_local(f: &Fighter, bash: f32) -> (Vec3, Vec3, Vec3) {
    let r = f.raise.clamp(0.0, 1.0);
    let lowered = (
        Vec3::new(-0.38, 0.72, -0.05),
        Vec3::new(-0.95, 0.0, 0.3).normalize(),
        Vec3::new(0.05, 1.0, 0.1).normalize(),
    );
    let a = f.cover;
    let h = f.high.clamp(0.0, 1.0);
    let normal = dir_of(-a, h * 0.5);
    let raised = (
        Vec3::new(-0.06 - 0.5 * a.sin(), 1.24 + 0.4 * h, 0.36 + 0.06 * a.cos()) + normal * bash,
        normal,
        // Tilted back over the head against a blow from above.
        Vec3::new(0.0, 1.0, -0.6 * h)
            .normalize()
            .lerp(Vec3::Y, 0.2)
            .normalize(),
    );
    let s = smooth(r);
    (
        lowered.0.lerp(raised.0, s),
        lowered.1.lerp(raised.1, s).normalize(),
        lowered.2.lerp(raised.2, s).normalize(),
    )
}

/// Two-bone reach from `root` toward `target`, bending toward `pole`:
/// returns the joint between and the end, which stops short of a target
/// out of reach.
#[must_use]
pub fn reach(root: Vec3, target: Vec3, a: f32, b: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp(1e-4, (a + b) * 0.999);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    // The joint's distance along `dir` and off it (law of cosines).
    let x = (a * a - b * b + d * d) / (2.0 * d);
    let y = (a * a - x * x).max(0.0).sqrt();
    let side = (pole - dir * pole.dot(dir)).normalize_or(dir.any_orthonormal_vector());
    (root + dir * x + side * y, end)
}

/// `f`'s pose at fight time `now`.
#[must_use]
pub fn rig(f: &Fighter, now: f32) -> Rig {
    let m = motion(f, now);
    let yaw = f.yaw;
    let at = |rfu: Vec3| f.pos + local(yaw, rfu);
    let dir = |rfu: Vec3| local(yaw, rfu);
    // The chest's twist and lean about the pelvis.
    let facing = Quat::from_rotation_y(yaw);
    let lean = Quat::from_axis_angle(dir(Vec3::X).normalize(), -m.lean);
    // A positive twist turns the chest toward the right, which is a
    // negative turn about +Y.
    let twist = Quat::from_rotation_y(-m.twist);
    let torso = lean * facing * twist;
    let pelvis = at(Vec3::new(0.0, HIP_HEIGHT - m.crouch, 0.0));
    let up_torso = |rfu: Vec3| pelvis + torso * Vec3::new(-rfu.x, rfu.y, rfu.z);
    let chest = up_torso(Vec3::new(0.0, 0.33, 0.0));
    let neck = up_torso(Vec3::new(0.0, 0.52, 0.0));
    let head = up_torso(Vec3::new(0.0, HEAD_HEIGHT - HIP_HEIGHT, 0.02));
    let shoulders = [
        up_torso(Vec3::new(-0.2, SHOULDER_HEIGHT - HIP_HEIGHT, 0.0)),
        up_torso(Vec3::new(0.2, SHOULDER_HEIGHT - HIP_HEIGHT, 0.0)),
    ];
    // The weapon, carried by the lean so a stoop brings it down with the
    // body.
    let lean_about = |p: Vec3| pelvis + lean * (p - pelvis);
    let grip = lean_about(at(m.weapon.0));
    let blade = (lean * dir(m.weapon.1)).normalize();
    let (s_at, s_normal, s_up) = shield_local(f, m.bash);
    let shield_at = lean_about(at(s_at));
    let normal = (lean * dir(s_normal)).normalize();
    let top = (lean * dir(s_up)).normalize();
    let x = top.cross(normal).normalize_or(Vec3::X);
    let y = normal.cross(x).normalize_or(Vec3::Y);
    let shield_rot = Quat::from_mat3(&Mat3::from_cols(x, y, normal)).normalize();
    // The shield arm holds the enarmes behind the shield's middle.
    let shield_hand = shield_at - normal * 0.07 + y * 0.02;
    let out = |side: f32| dir(Vec3::new(side, -0.9, -0.35));
    let (left_elbow, left_hand) = reach(shoulders[0], shield_hand, UPPER_ARM, FOREARM, out(-1.0));
    let (right_elbow, right_hand) = reach(shoulders[1], grip, UPPER_ARM, FOREARM, out(1.0));
    // A weapon too far for the arm is pulled back to the hand.
    let grip = right_hand;
    // Feet: the shield-side foot leads; walking swings them in turn.
    let stance = [Vec3::new(-0.15, 0.0, 0.2), Vec3::new(0.17, 0.0, -0.2)];
    let vel_local = Quat::from_rotation_y(-yaw) * f.vel;
    let moving = vel_local.length().min(2.5);
    let step_dir = if moving > 0.05 {
        Vec3::new(-vel_local.x, 0.0, vel_local.z) / vel_local.length()
    } else {
        Vec3::ZERO
    };
    let phase = f.stride * 2.6;
    let mut feet = [Vec3::ZERO; 2];
    let mut knees = [Vec3::ZERO; 2];
    let hips = [
        pelvis + dir(Vec3::new(-0.11, -0.04, 0.0)),
        pelvis + dir(Vec3::new(0.11, -0.04, 0.0)),
    ];
    let amount = (moving / 2.0).min(1.0);
    for i in 0..2 {
        let swing = (phase + i as f32 * PI).sin();
        let lift = (phase + i as f32 * PI).cos().max(0.0) * 0.09 * amount;
        let foot = stance[i] + step_dir * swing * 0.2 * amount + Vec3::Y * (0.06 + lift);
        feet[i] = at(foot);
        let (knee, end) = reach(
            hips[i],
            feet[i],
            THIGH,
            SHIN,
            dir(Vec3::new(if i == 0 { -0.15 } else { 0.15 }, 0.0, 1.0)),
        );
        knees[i] = knee;
        feet[i] = end;
    }
    let mut rig = Rig {
        pelvis,
        chest,
        neck,
        head,
        shoulders,
        elbows: [left_elbow, right_elbow],
        hands: [left_hand, right_hand],
        hips,
        knees,
        feet,
        grip,
        blade,
        length: f.kit.weapon().length,
        shield_at,
        shield_rot,
        body: facing,
        torso,
    };
    if f.action == Action::Down {
        // Toppled backward, away from the blow.
        let t = smooth(f.elapsed(now) / FALL_TIME);
        let pivot = f.pos - f.forward() * 0.1;
        let axis = dir(Vec3::X).normalize();
        let fall = Quat::from_axis_angle(axis, t * (FRAC_PI_2 - 0.12));
        let drop = Vec3::Y * 0.12 * t;
        let turn = |p: Vec3| pivot + fall * (p - pivot) + drop;
        transform(&mut rig, &turn, fall);
    }
    rig
}

fn transform(rig: &mut Rig, f: &impl Fn(Vec3) -> Vec3, rot: Quat) {
    for p in [
        &mut rig.pelvis,
        &mut rig.chest,
        &mut rig.neck,
        &mut rig.head,
        &mut rig.grip,
        &mut rig.shield_at,
    ] {
        *p = f(*p);
    }
    for list in [
        &mut rig.shoulders,
        &mut rig.elbows,
        &mut rig.hands,
        &mut rig.hips,
        &mut rig.knees,
        &mut rig.feet,
    ] {
        for p in list.iter_mut() {
            *p = f(*p);
        }
    }
    // Fallen limbs rest on the ground, not under it.
    for list in [&mut rig.feet, &mut rig.knees, &mut rig.hands] {
        for p in list.iter_mut() {
            p.y = p.y.max(0.06);
        }
    }
    rig.blade = (rot * rig.blade).normalize();
    rig.shield_rot = (rot * rig.shield_rot).normalize();
    rig.body = (rot * rig.body).normalize();
    rig.torso = (rot * rig.torso).normalize();
}
