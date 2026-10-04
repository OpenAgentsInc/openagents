//! Seat postures authored on the pack character's skeleton when the zone
//! loads, so the studio's seats sit, read, lean, and gesture without new
//! clips in the pinned pack.
//!
//! The pack names no joints, so [`Skeleton::find`] finds the ones a posture
//! moves by the skeleton's shape: the pelvis is the joint with two mirrored
//! children whose chains reach the ground, the chest is the first joint up
//! the spine with three children, and each limb follows its longest chain.
//! A posture starts from the idle clip's pose at each key, then moves the
//! pelvis, turns the spine, and places the hands and feet with two-joint
//! reaches ([`Poser::reach`]). The result is an ordinary looping clip
//! ([`authored`]) that `verse_engine`'s playback blends like the pack's.
//!
//! The model space is the pack's: +Y up, the ground at zero, and the
//! character facing +Z.

use glam::{Mat4, Quat, Vec3};
use std::f32::consts::TAU;
use verse_engine::assets::{BoneKeys, Clip};

use super::studio::Posture;
use crate::zones::everglade_pack::{Clip as PackClip, Joint};

/// Height of a desk stool's seat, m: the pack's `props/Stool`.
pub const STOOL: f32 = 0.58;
/// Height of a workbench's top, m: the pack's `props/Workbench`.
pub const BENCH: f32 = 0.9;
/// How far forward of the standing point a seated seat sits, m, so its
/// hands reach the bench.
pub const SEAT_FORWARD: f32 = 0.2;
/// Keys per authored loop; typing taps four times a loop, so a tap has
/// eight keys.
const KEYS: usize = 32;

/// Three joints in a chain: a thigh, calf, and foot, or an upper arm,
/// forearm, and hand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limb {
    pub upper: usize,
    pub lower: usize,
    pub end: usize,
}

/// The joints a posture moves. Each pair is in +x, then -x order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skeleton {
    pub pelvis: usize,
    /// The first spine joint over the pelvis.
    pub spine: usize,
    /// The joint the neck and both arms hang from.
    pub chest: usize,
    pub neck: usize,
    pub head: usize,
    pub legs: [Limb; 2],
    pub arms: [Limb; 2],
}

/// One joint's local transform: translation, rotation, and scale.
type Local = (Vec3, Quat, Vec3);

fn rest(joint: &Joint) -> Local {
    (
        joint.translation.into(),
        Quat::from_array(joint.rotation).normalize(),
        joint.scale.into(),
    )
}

/// Model-space transforms of every joint for `locals`. Parents come
/// before children in the pack, so one pass suffices.
fn globals(joints: &[Joint], locals: &[Local]) -> Vec<Mat4> {
    let mut result: Vec<Mat4> = Vec::with_capacity(joints.len());
    for (i, joint) in joints.iter().enumerate() {
        let (t, r, s) = locals[i];
        let local = Mat4::from_scale_rotation_translation(s, r, t);
        let parent = usize::try_from(joint.parent).ok().filter(|&p| p < i);
        result.push(match parent {
            Some(p) => result[p] * local,
            None => local,
        });
    }
    result
}

impl Skeleton {
    /// Finds the posed joints of a humanoid skeleton by its shape, or
    /// `None` when the skeleton is not one.
    #[must_use]
    pub fn find(joints: &[Joint]) -> Option<Self> {
        let n = joints.len();
        let locals: Vec<Local> = joints.iter().map(rest).collect();
        let at: Vec<Vec3> = globals(joints, &locals)
            .iter()
            .map(|m| m.w_axis.truncate())
            .collect();
        let mut children = vec![Vec::new(); n];
        for (i, joint) in joints.iter().enumerate() {
            if let Some(p) = usize::try_from(joint.parent).ok().filter(|&p| p < i) {
                children[p].push(i);
            }
        }
        // Descendant counts and the lowest point under each joint, children
        // first.
        let mut size = vec![1usize; n];
        let mut lowest: Vec<f32> = at.iter().map(|p| p.y).collect();
        for i in (0..n).rev() {
            if let Some(p) = usize::try_from(joints[i].parent).ok().filter(|&p| p < i) {
                size[p] += size[i];
                lowest[p] = lowest[p].min(lowest[i]);
            }
        }
        let main = |i: usize| children[i].iter().copied().max_by_key(|&c| size[c]);
        let limb = |upper: usize| -> Option<Limb> {
            let lower = main(upper)?;
            let end = main(lower)?;
            Some(Limb { upper, lower, end })
        };
        let pair = |a: usize, b: usize| if at[a].x >= at[b].x { [a, b] } else { [b, a] };
        for pelvis in 0..n {
            let kids = &children[pelvis];
            let legs: Vec<usize> = kids
                .iter()
                .copied()
                .filter(|&k| at[k].x.abs() > 0.03 && lowest[k] < at[pelvis].y - 0.5)
                .collect();
            let [a, b] = legs[..] else {
                continue;
            };
            if at[a].x.signum() == at[b].x.signum() {
                continue;
            }
            let Some(spine) = kids
                .iter()
                .copied()
                .filter(|k| !legs.contains(k) && size[*k] > 1)
                .max_by_key(|&k| size[k])
            else {
                continue;
            };
            let mut chest = spine;
            for _ in 0..8 {
                if children[chest].len() >= 3 {
                    break;
                }
                chest = main(chest)?;
            }
            if children[chest].len() < 3 {
                continue;
            }
            let neck = children[chest]
                .iter()
                .copied()
                .min_by(|&x, &y| at[x].x.abs().total_cmp(&at[y].x.abs()))?;
            let head = main(neck)?;
            let shoulders: Vec<usize> = children[chest]
                .iter()
                .copied()
                .filter(|&c| c != neck && size[c] > 3)
                .collect();
            let [l, r] = shoulders[..] else {
                continue;
            };
            let [l, r] = pair(l, r);
            let arm = |clavicle: usize| limb(main(clavicle)?);
            let [la, lb] = pair(a, b);
            return Some(Self {
                pelvis,
                spine,
                chest,
                neck,
                head,
                legs: [limb(la)?, limb(lb)?],
                arms: [arm(l)?, arm(r)?],
            });
        }
        None
    }
}

/// Samples a pack clip's local transforms at `time`, the rest pose for an
/// unkeyed channel.
fn sample(joints: &[Joint], clip: &PackClip, time: f32) -> Vec<Local> {
    fn pair<const N: usize>(keys: &[(f32, [f32; N])], t: f32) -> Option<([f32; N], [f32; N], f32)> {
        let first = keys.first()?;
        let i = keys.partition_point(|k| k.0 <= t).saturating_sub(1);
        let a = keys.get(i).unwrap_or(first);
        let b = keys.get(i + 1).unwrap_or(a);
        let f = if b.0 > a.0 {
            ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Some((a.1, b.1, f))
    }
    let mut locals: Vec<Local> = joints.iter().map(rest).collect();
    for track in &clip.tracks {
        let Some(local) = locals.get_mut(usize::from(track.joint)) else {
            continue;
        };
        if let Some((a, b, f)) = pair(&track.translation, time) {
            local.0 = Vec3::from(a).lerp(Vec3::from(b), f);
        }
        if let Some((a, b, f)) = pair(&track.rotation, time) {
            let q = |v: [f32; 4]| Quat::from_array(v).normalize();
            local.1 = q(a).slerp(q(b), f).normalize();
        }
        if let Some((a, b, f)) = pair(&track.scale, time) {
            local.2 = Vec3::from(a).lerp(Vec3::from(b), f);
        }
    }
    locals
}

/// A pose under construction: local transforms changed by model-space
/// moves.
struct Poser<'a> {
    joints: &'a [Joint],
    locals: Vec<Local>,
}

impl Poser<'_> {
    fn globals(&self) -> Vec<Mat4> {
        globals(self.joints, &self.locals)
    }

    fn at(&self, joint: usize) -> Vec3 {
        self.globals()[joint].w_axis.truncate()
    }

    fn parent(&self, joint: usize) -> Option<usize> {
        usize::try_from(self.joints[joint].parent)
            .ok()
            .filter(|&p| p < joint)
    }

    /// Turns `joint`, and everything it carries, by model-space `delta`
    /// about the joint.
    fn turn(&mut self, joint: usize, delta: Quat) {
        let parent = self.parent(joint).map_or(Quat::IDENTITY, |p| {
            self.globals()[p].to_scale_rotation_translation().1
        });
        let local = &mut self.locals[joint].1;
        *local = (parent.inverse() * delta * parent * *local).normalize();
    }

    /// Moves `joint`, and everything it carries, by model-space `delta`.
    fn shift(&mut self, joint: usize, delta: Vec3) {
        let inverse = self
            .parent(joint)
            .map_or(Mat4::IDENTITY, |p| self.globals()[p].inverse());
        self.locals[joint].0 += inverse.transform_vector3(delta);
    }

    /// Turns `limb`'s two upper joints so its end reaches `target`, or as
    /// near as the limb's length allows, bending toward `pole`.
    fn reach(&mut self, limb: Limb, target: Vec3, pole: Vec3) {
        let (s, e, w) = (self.at(limb.upper), self.at(limb.lower), self.at(limb.end));
        let (a, b) = ((e - s).length(), (w - e).length());
        if a < 1e-2 || b < 1e-2 || !target.is_finite() {
            return;
        }
        let to = target - s;
        let d = to
            .length()
            .clamp((a - b).abs() + 1e-3, a + b - 1e-3)
            .max(1e-3);
        let dir = to.normalize_or(Vec3::NEG_Y);
        let cos = ((a * a + d * d - b * b) / (2.0 * a * d)).clamp(-1.0, 1.0);
        let sin = (1.0 - cos * cos).max(0.0).sqrt();
        let bend = (pole - dir * pole.dot(dir)).normalize_or(dir.any_orthonormal_vector());
        let elbow = s + (dir * cos + bend * sin) * a;
        let upper =
            Quat::from_rotation_arc((e - s).normalize_or(dir), (elbow - s).normalize_or(dir));
        self.turn(limb.upper, upper);
        let (e, w) = (self.at(limb.lower), self.at(limb.end));
        let lower = Quat::from_rotation_arc(
            (w - e).normalize_or(dir),
            (s + dir * d - e).normalize_or(dir),
        );
        self.turn(limb.lower, lower);
    }
}

/// The posture `posture` at `phase` (0 to 1 over its loop), from `base`,
/// the idle pose at the same moment.
fn pose(
    skeleton: &Skeleton,
    joints: &[Joint],
    base: Vec<Local>,
    posture: Posture,
    phase: f32,
) -> Vec<Local> {
    let mut p = Poser {
        joints,
        locals: base,
    };
    let wave = |cycles: f32, offset: f32| (phase * cycles * TAU + offset).sin();
    let side = |arm: Limb, p: &Poser<'_>| p.at(arm.upper).x.signum();
    let [left, right] = skeleton.arms;
    match posture {
        Posture::Stand => {}
        Posture::Sit | Posture::Type => {
            let hip = p.at(skeleton.pelvis);
            let feet: Vec<Vec3> = skeleton.legs.iter().map(|l| p.at(l.end)).collect();
            p.shift(
                skeleton.pelvis,
                Vec3::new(0.0, STOOL + 0.1 - hip.y, SEAT_FORWARD - 0.03 - hip.z),
            );
            for (leg, foot) in skeleton.legs.iter().zip(feet) {
                let thigh = p.at(leg.upper);
                let target = Vec3::new(thigh.x * 1.3, foot.y, thigh.z + 0.4);
                p.reach(*leg, target, Vec3::new(0.0, 0.4, 1.0));
            }
            let lean = if posture == Posture::Type { 0.14 } else { 0.04 };
            p.turn(
                skeleton.spine,
                Quat::from_rotation_x(lean + 0.01 * wave(1.0, 0.0)),
            );
            for (i, arm) in [left, right].into_iter().enumerate() {
                let sign = side(arm, &p);
                let shoulder = p.at(arm.upper);
                let target = if posture == Posture::Type {
                    // Hands on the bench, each tapping in turn.
                    let tap = wave(4.0, i as f32 * std::f32::consts::PI).max(0.0);
                    Vec3::new(
                        shoulder.x * 0.55,
                        BENCH + 0.03 + 0.025 * tap,
                        shoulder.z + 0.4,
                    )
                } else {
                    // Hands resting on the thighs.
                    let knee = p.at(skeleton.legs[i].lower);
                    knee.lerp(p.at(skeleton.legs[i].upper), 0.4) + Vec3::Y * 0.08
                };
                p.reach(arm, target, Vec3::new(sign, -0.4, -0.6));
            }
        }
        Posture::Read => {
            p.turn(skeleton.spine, Quat::from_rotation_x(0.08));
            p.turn(skeleton.head, Quat::from_rotation_x(0.35));
            let chest = p.at(skeleton.chest);
            for (i, arm) in [left, right].into_iter().enumerate() {
                let sign = side(arm, &p);
                // A book held open before the chest; one hand turns a page
                // once a loop.
                let turn = if i == 0 {
                    0.06 * wave(1.0, 0.0).max(0.0)
                } else {
                    0.0
                };
                let target = Vec3::new(sign * (0.12 - turn), chest.y - 0.15, chest.z + 0.3);
                p.reach(arm, target, Vec3::new(sign, -1.0, -0.3));
            }
        }
        Posture::Lean => {
            // Bent over the ring's rail, hands on the knees.
            let feet: Vec<Vec3> = skeleton.legs.iter().map(|l| p.at(l.end)).collect();
            p.shift(skeleton.pelvis, Vec3::new(0.0, -0.08, -0.06));
            for (leg, foot) in skeleton.legs.iter().zip(feet) {
                p.reach(*leg, foot, Vec3::new(0.0, 0.0, 1.0));
            }
            p.turn(
                skeleton.spine,
                Quat::from_rotation_x(0.42 + 0.015 * wave(1.0, 0.0)),
            );
            for (i, arm) in [left, right].into_iter().enumerate() {
                let sign = side(arm, &p);
                let knee = p.at(skeleton.legs[i].lower);
                p.reach(
                    arm,
                    knee + Vec3::new(0.0, 0.2, 0.05),
                    Vec3::new(sign, 0.0, -1.0),
                );
            }
        }
        Posture::Wait => {
            // Hands clasped before the hips, weight shifting slowly.
            p.turn(
                skeleton.pelvis,
                Quat::from_rotation_z(0.03 * wave(1.0, 0.0)),
            );
            let hip = p.at(skeleton.pelvis);
            for arm in [left, right] {
                let sign = side(arm, &p);
                let target = Vec3::new(sign * 0.03, hip.y + 0.1, hip.z + 0.2);
                p.reach(arm, target, Vec3::new(sign, 0.0, -0.6));
            }
        }
        Posture::Think => {
            // One hand at the chin, the other arm across the waist.
            p.turn(skeleton.head, Quat::from_rotation_x(-0.12));
            let head = p.at(skeleton.head);
            let hip = p.at(skeleton.pelvis);
            let chin = Vec3::new(head.x, head.y - 0.06, head.z + 0.12);
            let sign = side(right, &p);
            p.reach(right, chin, Vec3::new(sign, -1.0, 0.0));
            let sign = side(left, &p);
            p.reach(
                left,
                Vec3::new(-sign * 0.05, hip.y + 0.15, hip.z + 0.22),
                Vec3::new(sign, -0.5, -0.5),
            );
        }
        Posture::Work => {
            // Both hands at the bench, one striking twice a loop.
            p.turn(skeleton.spine, Quat::from_rotation_x(0.12));
            for (i, arm) in [left, right].into_iter().enumerate() {
                let sign = side(arm, &p);
                let shoulder = p.at(arm.upper);
                let lift = if i == 1 {
                    0.14 * wave(2.0, 0.0).max(0.0)
                } else {
                    0.0
                };
                let target = Vec3::new(shoulder.x * 0.7, BENCH + 0.05 + lift, shoulder.z + 0.4);
                p.reach(arm, target, Vec3::new(sign, -0.5, -0.5));
            }
        }
        Posture::Talk => {
            // One open hand rising and falling with the words.
            let shoulder = p.at(right.upper);
            let sign = side(right, &p);
            let target = shoulder + Vec3::new(sign * 0.18, -0.18 + 0.07 * wave(2.0, 0.0), 0.3);
            p.reach(right, target, Vec3::new(sign, -1.0, 0.0));
            p.turn(skeleton.head, Quat::from_rotation_z(0.05 * wave(1.0, 0.5)));
        }
    }
    p.locals
}

/// The authored clip for `posture`, playing as clip `id` over `idle`'s
/// loop, or `None` for [`Posture::Stand`], which plays `idle` itself.
#[must_use]
pub fn authored(
    skeleton: &Skeleton,
    joints: &[Joint],
    idle: &PackClip,
    posture: Posture,
    id: u16,
) -> Option<Clip> {
    if posture == Posture::Stand {
        return None;
    }
    let duration = if idle.duration.is_finite() && idle.duration > 0.0 {
        idle.duration
    } else {
        3.0
    };
    let frames: Vec<(f32, Vec<Local>)> = (0..=KEYS)
        .map(|k| {
            let phase = k as f32 / KEYS as f32;
            let time = phase * duration;
            let base = sample(joints, idle, time.min(duration - 1e-4));
            (time, pose(skeleton, joints, base, posture, phase))
        })
        .collect();
    let bones = (0..joints.len())
        .map(|bone| {
            // A channel that never changes keeps one key.
            fn keys<const N: usize>(all: Vec<(f32, [f32; N])>) -> Vec<(f32, [f32; N])> {
                let same = all
                    .iter()
                    .all(|k| k.1.iter().zip(&all[0].1).all(|(a, b)| (a - b).abs() < 1e-6));
                if same {
                    all.into_iter().take(1).collect()
                } else {
                    all
                }
            }
            BoneKeys {
                bone,
                translation: keys(
                    frames
                        .iter()
                        .map(|(t, l)| (*t, l[bone].0.to_array()))
                        .collect(),
                ),
                rotation: keys(
                    frames
                        .iter()
                        .map(|(t, l)| (*t, l[bone].1.to_array()))
                        .collect(),
                ),
                scale: keys(
                    frames
                        .iter()
                        .map(|(t, l)| (*t, l[bone].2.to_array()))
                        .collect(),
                ),
            }
        })
        .collect();
    Some(Clip {
        id,
        duration,
        bones,
    })
}

/// The model-space positions of `joints` under `clip` at the key nearest
/// `time`, for tests.
#[cfg(test)]
pub(crate) fn posed(joints: &[Joint], clip: &Clip, time: f32) -> Vec<Vec3> {
    fn nearest<const N: usize>(keys: &[(f32, [f32; N])], time: f32) -> Option<[f32; N]> {
        keys.iter()
            .min_by(|a, b| (a.0 - time).abs().total_cmp(&(b.0 - time).abs()))
            .map(|k| k.1)
    }
    let mut locals: Vec<Local> = joints.iter().map(rest).collect();
    for keys in &clip.bones {
        if let Some(t) = nearest(&keys.translation, time) {
            locals[keys.bone].0 = t.into();
        }
        if let Some(r) = nearest(&keys.rotation, time) {
            locals[keys.bone].1 = Quat::from_array(r);
        }
    }
    globals(joints, &locals)
        .iter()
        .map(|m| m.w_axis.truncate())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character() -> &'static crate::zones::everglade_pack::Character {
        super::super::tests::pack()
            .character
            .as_ref()
            .expect("the pack's character")
    }

    #[test]
    fn the_skeleton_is_found_by_its_shape() {
        let joints = &character().joints;
        let skeleton = Skeleton::find(joints).expect("a humanoid skeleton");
        let locals: Vec<Local> = joints.iter().map(rest).collect();
        let at: Vec<Vec3> = globals(joints, &locals)
            .iter()
            .map(|m| m.w_axis.truncate())
            .collect();
        // The head is over the chest, the chest over the pelvis, and each
        // pair of limbs is mirrored.
        assert!(at[skeleton.head].y > at[skeleton.chest].y);
        assert!(at[skeleton.chest].y > at[skeleton.pelvis].y);
        for limbs in [skeleton.legs, skeleton.arms] {
            assert!(at[limbs[0].upper].x > 0.0 && at[limbs[1].upper].x < 0.0);
            for limb in limbs {
                let ids = [limb.upper, limb.lower, limb.end];
                assert!(ids.windows(2).all(|w| w[0] != w[1]));
            }
        }
        for leg in skeleton.legs {
            assert!(at[leg.end].y < 0.2, "a foot at {}", at[leg.end]);
        }
        // A skeleton too small to be a humanoid's has none.
        assert_eq!(Skeleton::find(&joints[..2]), None);
    }

    #[test]
    fn a_seated_seat_sits_on_its_stool_and_types_at_the_bench() {
        let character = character();
        let joints = &character.joints;
        let skeleton = Skeleton::find(joints).unwrap();
        let idle = character.clip("idle").unwrap();
        assert!(authored(&skeleton, joints, idle, Posture::Stand, 30).is_none());
        let wait = authored(&skeleton, joints, idle, Posture::Wait, 31).unwrap();
        let standing = posed(joints, &wait, 0.0);
        for posture in [Posture::Sit, Posture::Type] {
            let clip = authored(&skeleton, joints, idle, posture, 32).unwrap();
            assert!(
                clip.bones
                    .iter()
                    .all(|b| { b.rotation.iter().all(|k| Quat::from_array(k.1).is_finite()) })
            );
            let at = posed(joints, &clip, clip.duration / 3.0);
            let hip = at[skeleton.pelvis];
            // The hips rest on the stool, over its middle.
            assert!(
                (hip.y - (STOOL + 0.1)).abs() < 0.03,
                "{posture:?} hips at {hip}"
            );
            assert!((hip.z - (SEAT_FORWARD - 0.03)).abs() < 0.03, "{hip}");
            assert!(hip.y < standing[skeleton.pelvis].y - 0.2);
            // The knees come forward and the feet stay on the ground.
            for leg in skeleton.legs {
                let knee = at[leg.lower];
                assert!(knee.z > hip.z + 0.25, "{posture:?} knee at {knee}");
                assert!((at[leg.end].y - standing[leg.end].y).abs() < 0.05);
            }
            if posture == Posture::Type {
                // The hands are out over the bench's height.
                for arm in skeleton.arms {
                    let hand = at[arm.end];
                    assert!((hand.y - BENCH).abs() < 0.15, "a hand at {hand}");
                    assert!(hand.z > at[skeleton.chest].z + 0.2, "a hand at {hand}");
                }
            }
        }
    }

    #[test]
    fn every_posture_moves_from_idle_and_loops() {
        let character = character();
        let joints = &character.joints;
        let skeleton = Skeleton::find(joints).unwrap();
        let idle = character.clip("idle").unwrap();
        let base: Vec<Vec3> = globals(joints, &sample(joints, idle, 0.0))
            .iter()
            .map(|m| m.w_axis.truncate())
            .collect();
        for posture in Posture::ALL.into_iter().filter(|p| *p != Posture::Stand) {
            let clip = authored(&skeleton, joints, idle, posture, 40).unwrap();
            assert!((clip.duration - idle.duration).abs() < 1e-6);
            let start = posed(joints, &clip, 0.0);
            let end = posed(joints, &clip, clip.duration);
            for (a, b) in start.iter().zip(&end) {
                assert!(a.distance(*b) < 0.02, "{posture:?} does not loop");
            }
            let moved = [skeleton.arms[0].end, skeleton.arms[1].end, skeleton.head]
                .iter()
                .any(|&j| start[j].distance(base[j]) > 0.02);
            assert!(moved, "{posture:?} looks like idle");
        }
    }
}
