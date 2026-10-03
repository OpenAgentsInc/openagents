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
#[derive(Clone, Copy)]
struct Local {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}
fn sample(model: &Model, animation: u16, time: f32) -> Vec<Local> {
    let clip = model
        .clips
        .iter()
        .find(|c| c.id == animation)
        .or_else(|| model.clips.iter().find(|c| c.id == 0));
    let time = clip.map_or(0.0, |c| {
        if !time.is_finite() || c.duration <= 0.0 {
            0.0
        } else if c.id == 1 {
            time.clamp(0.0, c.duration)
        } else {
            time.max(0.0) % c.duration
        }
    });
    model
        .bones
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let rest = model.skin.as_ref().map(|s| s.rest[i]);
            let default_translation = rest.map_or([0.; 3], |r| r.translation);
            let default_rotation = rest.map_or([0., 0., 0., 1.], |r| r.rotation);
            let default_scale = rest.map_or([1.; 3], |r| r.scale);
            let keys = clip.and_then(|c| c.bones.iter().find(|k| k.bone == i));
            keys.map_or(
                Local {
                    translation: default_translation.into(),
                    rotation: Quat::from_array(default_rotation),
                    scale: default_scale.into(),
                },
                |k| {
                    let (a, b, f) = pair(&k.rotation, time, default_rotation);
                    let q = |v| {
                        let q = Quat::from_array(v);
                        if q.length_squared() > 0.00001 {
                            q.normalize()
                        } else {
                            Quat::IDENTITY
                        }
                    };
                    Local {
                        translation: vector(&k.translation, time, default_translation),
                        rotation: q(a).slerp(q(b), f),
                        scale: vector(&k.scale, time, default_scale),
                    }
                },
            )
        })
        .collect()
}
fn matrices(model: &Model, locals: &[Local]) -> Vec<Mat4> {
    let mut result = vec![Mat4::IDENTITY; model.bones.len().max(1)];
    for (i, bone) in model.bones.iter().enumerate() {
        let p = Vec3::from(bone.pivot);
        let l = locals[i];
        let local = if model.skin.is_some() {
            Mat4::from_scale_rotation_translation(l.scale, l.rotation, l.translation)
        } else {
            Mat4::from_translation(p + l.translation)
                * Mat4::from_quat(l.rotation)
                * Mat4::from_scale(l.scale)
                * Mat4::from_translation(-p)
        };
        result[i] = if bone.parent >= 0 {
            result[bone.parent as usize] * local
        } else {
            local
        };
    }
    if let Some(skin) = &model.skin {
        let basis = Mat4::from_cols_array(&skin.basis);
        let inverse = basis.inverse();
        for (i, matrix) in result.iter_mut().enumerate() {
            *matrix = basis * *matrix * Mat4::from_cols_array(&skin.inverse_bind[i]) * inverse;
        }
    }
    result
}
/// Produce model-space skin matrices; a missing clip uses the rest pose.
pub fn pose(model: &Model, animation: u16, time: f32) -> Vec<Mat4> {
    matrices(model, &sample(model, animation, time))
}
/// Interruptible clip blending in local space, before parent transforms accumulate.
#[derive(Default)]
pub struct Playback {
    clip: Option<u16>,
    clock: f32,
    changed: f32,
    from: Vec<Local>,
    current: Vec<Local>,
}
impl Playback {
    pub fn update(&mut self, model: &Model, animation: u16, time: f32, clock: f32) -> Vec<Mat4> {
        let target = sample(model, animation, time);
        if self.current.len() != target.len() || clock < self.clock {
            self.clip = None;
            self.current = target.clone();
        }
        if self.clip != Some(animation) {
            self.from = self.current.clone();
            self.changed = clock;
            if self.clip.is_none() {
                self.changed -= 0.25;
            }
            self.clip = Some(animation);
        }
        let duration = if animation == 1 { 0.12 } else { 0.22 };
        let f = ((clock - self.changed) / duration).clamp(0.0, 1.0);
        let f = f * f * (3.0 - 2.0 * f);
        self.current = self
            .from
            .iter()
            .zip(target)
            .map(|(a, b)| Local {
                translation: a.translation.lerp(b.translation, f),
                rotation: a.rotation.slerp(b.rotation, f),
                scale: a.scale.lerp(b.scale, f),
            })
            .collect();
        self.clock = clock;
        matrices(model, &self.current)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{Bone, BoneKeys, Clip};
    #[test]
    fn inverse_bind_cancels_rest_hierarchy_and_preserves_joint_pivot() {
        use crate::assets::{RestPose, Skin};
        let rest = RestPose {
            translation: [0., 2., 0.],
            rotation: Quat::IDENTITY.to_array(),
            scale: [1.; 3],
        };
        let basis = Mat4::from_rotation_x(0.7) * Mat4::from_scale(Vec3::splat(2.));
        let mut model = Model {
            source: String::new(),
            source_sha256: String::new(),
            height: 2.,
            surfaces: vec![],
            attachments: vec![],
            bones: vec![
                Bone {
                    parent: -1,
                    pivot: [0.; 3],
                },
                Bone {
                    parent: 0,
                    pivot: [0.; 3],
                },
            ],
            clips: vec![],
            skin: Some(Skin {
                names: vec!["root".into(), "hand".into()],
                rest: vec![rest, rest],
                inverse_bind: vec![
                    Mat4::from_translation(Vec3::Y * -2.).to_cols_array(),
                    Mat4::from_translation(Vec3::Y * -4.).to_cols_array(),
                ],
                basis: basis.to_cols_array(),
            }),
        };
        assert!(
            pose(&model, 0, 0.)
                .iter()
                .all(|m| m.abs_diff_eq(Mat4::IDENTITY, 1e-5))
        );
        model.clips.push(Clip {
            id: 4,
            duration: 1.,
            bones: vec![BoneKeys {
                bone: 0,
                translation: vec![],
                scale: vec![],
                rotation: vec![(
                    0.,
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
                )],
            }],
        });
        let pivot = basis.transform_point3(Vec3::Y * 2.);
        assert!(
            pose(&model, 4, 0.)[0]
                .transform_point3(pivot)
                .abs_diff_eq(pivot, 1e-5)
        );
        let hand = basis.transform_point3(Vec3::Y * 4.);
        let expected = basis.transform_point3(Vec3::new(-2., 2., 0.));
        assert!(
            pose(&model, 4, 0.)[1]
                .transform_point3(hand)
                .abs_diff_eq(expected, 1e-5)
        );
    }
    #[test]
    fn interrupted_transitions_preserve_pose_and_rotation_length() {
        let model = Model {
            skin: None,
            source: String::new(),
            source_sha256: String::new(),
            height: 1.0,
            surfaces: vec![],
            attachments: vec![],
            bones: vec![Bone {
                parent: -1,
                pivot: [0.0; 3],
            }],
            clips: [0, 4, 52]
                .into_iter()
                .enumerate()
                .map(|(i, id)| Clip {
                    id,
                    duration: 1.0,
                    bones: vec![BoneKeys {
                        bone: 0,
                        translation: vec![(0.0, [i as f32, 0.0, 0.0])],
                        rotation: vec![(0.0, Quat::from_rotation_y(i as f32).to_array())],
                        scale: vec![],
                    }],
                })
                .collect(),
        };
        let mut playback = Playback::default();
        let idle = playback.update(&model, 0, 0.0, 0.0)[0];
        let start = playback.update(&model, 4, 0.0, 0.1)[0];
        assert!(idle.abs_diff_eq(start, 0.00001));
        let middle = playback.update(&model, 4, 0.1, 0.21)[0];
        assert!((middle.w_axis.x - 0.5).abs() < 0.001);
        let interrupt = playback.update(&model, 52, 0.0, 0.21)[0];
        assert!(middle.abs_diff_eq(interrupt, 0.00001));
        let blended = playback.update(&model, 52, 0.1, 0.32)[0];
        assert!((blended.x_axis.truncate().length() - 1.0).abs() < 0.00001);
        let end = playback.update(&model, 52, 0.3, 0.5)[0];
        assert!((end.w_axis.x - 2.0).abs() < 0.001);
        let reset = playback.update(&model, 0, 0.0, 0.0)[0];
        assert!(reset.abs_diff_eq(idle, 0.00001));
    }
    #[test]
    fn death_holds_the_final_pose_while_idle_keeps_looping() {
        let mut model = Model {
            skin: None,
            source: String::new(),
            source_sha256: String::new(),
            surfaces: vec![],
            height: 1.0,
            attachments: vec![],
            bones: vec![Bone {
                parent: -1,
                pivot: [0.0; 3],
            }],
            clips: vec![Clip {
                id: 1,
                duration: 2.0,
                bones: vec![BoneKeys {
                    bone: 0,
                    translation: vec![(0.0, [0.0; 3]), (2.0, [2.0, 0.0, 0.0])],
                    rotation: vec![],
                    scale: vec![],
                }],
            }],
        };
        for time in [2.0, 4.0, 12.0] {
            assert!((pose(&model, 1, time)[0].transform_point3(Vec3::ZERO).x - 2.0).abs() < 0.001);
        }
        model.clips[0].id = 0;
        assert!(pose(&model, 0, 4.0)[0].transform_point3(Vec3::ZERO).x.abs() < 0.001);
    }
    #[test]
    fn child_inherits_interpolated_parent_motion() {
        let model = Model {
            skin: None,
            source: String::new(),
            source_sha256: String::new(),
            surfaces: vec![],
            height: 1.0,
            attachments: vec![],
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
