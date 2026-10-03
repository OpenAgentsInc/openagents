//! Continuous skeletal animation evaluated from imported source keyframes.
use crate::assets::Model;
use glam::{Mat4, Quat, Vec3};

fn pair<const N: usize>(
    keys: &[(f32, [f32; N])],
    t: f32,
    default: [f32; N],
) -> ([f32; N], [f32; N], f32) {
    if keys.is_empty() {
        return (default, default, 0.0);
    }
    let i = keys.partition_point(|k| k.0 <= t).saturating_sub(1);
    let a = keys[i];
    let b = keys.get(i + 1).copied().unwrap_or(a);
    (
        a.1,
        b.1,
        if b.0 > a.0 {
            ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
        } else {
            0.0
        },
    )
}
fn vector(keys: &[(f32, [f32; 3])], t: f32, default: [f32; 3]) -> Vec3 {
    let (a, b, f) = pair(keys, t, default);
    Vec3::from(a).lerp(Vec3::from(b), f)
}
/// Produce model-space skin matrices; a missing clip uses the rest pose.
pub fn pose(model: &Model, animation: u16, time: f32) -> Vec<Mat4> {
    let clip = model
        .clips
        .iter()
        .find(|c| c.id == animation)
        .or_else(|| model.clips.iter().find(|c| c.id == 0));
    let time = clip.map_or(0.0, |c| {
        if time.is_finite() {
            time.max(0.0) % c.duration
        } else {
            0.0
        }
    });
    let mut result = vec![Mat4::IDENTITY; model.bones.len().max(1)];
    for (i, bone) in model.bones.iter().enumerate() {
        let keys = clip.and_then(|c| c.bones.iter().find(|k| k.bone == i));
        let pivot = Vec3::from(bone.pivot);
        let local = if let Some(k) = keys {
            let (a, b, f) = pair(&k.rotation, time, [0.0, 0.0, 0.0, 1.0]);
            let q = |v| {
                let q = Quat::from_array(v);
                if q.length_squared() > 0.00001 {
                    q.normalize()
                } else {
                    Quat::IDENTITY
                }
            };
            Mat4::from_translation(pivot + vector(&k.translation, time, [0.0; 3]))
                * Mat4::from_quat(q(a).slerp(q(b), f))
                * Mat4::from_scale(vector(&k.scale, time, [1.0; 3]))
                * Mat4::from_translation(-pivot)
        } else {
            Mat4::IDENTITY
        };
        result[i] = if bone.parent >= 0 {
            result[bone.parent as usize] * local
        } else {
            local
        };
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{Bone, BoneKeys, Clip};
    #[test]
    fn child_inherits_interpolated_parent_motion() {
        let model = Model {
            source: String::new(),
            source_sha256: String::new(),
            surfaces: vec![],
            height: 1.0,
            bones: vec![
                Bone {
                    parent: -1,
                    pivot: [0.0; 3],
                },
                Bone {
                    parent: 0,
                    pivot: [0.0; 3],
                },
            ],
            clips: vec![Clip {
                id: 0,
                duration: 2.0,
                bones: vec![BoneKeys {
                    bone: 0,
                    translation: vec![(0.0, [0.0; 3]), (2.0, [2.0, 0.0, 0.0])],
                    rotation: vec![],
                    scale: vec![],
                }],
            }],
        };
        assert!((pose(&model, 0, 1.0)[1].transform_point3(Vec3::ZERO) - Vec3::X).length() < 1e-5);
    }
}
