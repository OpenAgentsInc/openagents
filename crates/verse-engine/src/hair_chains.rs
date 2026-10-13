//! Secondary motion for hair (and any dangling chain of joints), baked into
//! clips: spring chains driven by a wind field (docs/verse/draconia/parity.md).
//!
//! A character carries chains of extra joints, named `hair_<chain>_<link>`
//! (link 0 is a child of the head, each next link a child of the one
//! before), with the strands skinned along them. [`bake`] plays a source
//! clip, simulates each chain as a string of particles (Verlet integration,
//! a spring pulling every particle back toward where the animated head
//! carries it, gravity, wind with gusts and turbulence, fixed link lengths,
//! and collision with spheres on the body), and writes the result as
//! rotation keys on the chain joints in a new clip, beside the source
//! clip's own keys. A clip is what every Verse renderer already plays, so
//! the hair moves in any scene that plays the baked clip.
//!
//! Space: the model's skeleton space, glTF's: +Y up. A character built
//! like Draconia faces +Z there.
use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};

use crate::assets::{BoneKeys, Clip, Model};
use crate::motion::{Binding, Mode};

/// The wind, in skeleton space (m/s): a steady part, gusts that swell and
/// fade, and turbulence that varies over space and time.
#[derive(Clone, Copy, Debug)]
pub struct Wind {
    pub velocity: Vec3,
    /// Gust strength as a share of `velocity`.
    pub gust: f32,
    /// Gusts per second.
    pub gust_rate: f32,
    /// Turbulent velocity, m/s.
    pub turbulence: f32,
    /// Air rushing out from (positive) or in toward a vertical axis through
    /// `center`, m/s, sideways and backward only (skeleton -Z is behind a
    /// character facing +Z): a blast from below throws hair up and out.
    pub outward: f32,
    pub center: Vec3,
}
impl Wind {
    pub const CALM: Self = Self {
        velocity: Vec3::ZERO,
        gust: 0.0,
        gust_rate: 0.0,
        turbulence: 0.0,
        outward: 0.0,
        center: Vec3::ZERO,
    };
    /// The wind's velocity at `p` and time `t`.
    #[must_use]
    pub fn at(&self, p: Vec3, t: f32) -> Vec3 {
        let g = 1.0
            + self.gust
                * (0.5 + 0.5 * (t * self.gust_rate * std::f32::consts::TAU).sin())
                * (0.6 + 0.4 * (t * self.gust_rate * 2.7 + 1.3).sin());
        let n = |a: f32, b: f32, c: f32| {
            (p.x * a + t * 1.7).sin() * (p.y * b - t * 1.3).cos() + (p.z * c + t * 2.3).sin() * 0.5
        };
        let turb = Vec3::new(n(9.0, 7.0, 5.0), n(6.0, 11.0, 8.0), n(7.0, 5.0, 10.0));
        // Outward from the axis, never forward past her face (+Z): a blast
        // throws hair up, out and back.
        let radial =
            Vec3::new(p.x - self.center.x, 0.0, (p.z - self.center.z).min(0.0)).normalize_or_zero();
        self.velocity * g + turb * self.turbulence + radial * self.outward * g
    }
}

/// How the chains respond.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    /// Pull back toward the animated pose, 1/s² (higher is stiffer hair).
    pub stiffness: f32,
    /// Velocity kept per second (0 to 1; lower is more damped).
    pub damping: f32,
    /// Gravity, m/s², along -Y.
    pub gravity: f32,
    /// How strongly the strands follow the air (1/s): air drag over mass.
    pub drag: f32,
    /// Simulation steps per second.
    pub rate: f32,
}
impl Default for Spring {
    fn default() -> Self {
        Self {
            stiffness: 30.0,
            damping: 0.15,
            gravity: 9.81,
            drag: 5.0,
            rate: 240.0,
        }
    }
}

/// A sphere the chains can't enter, fixed to a joint: `center` is in the
/// skeleton's rest space.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub joint: usize,
    pub center: Vec3,
    pub radius: f32,
}

/// The model's hair chains: joint indices per chain, root first, from the
/// names `hair_<chain>_<link>`, chains in order.
#[must_use]
pub fn chains(model: &Model) -> Vec<Vec<usize>> {
    let Some(skin) = &model.skin else {
        return vec![];
    };
    let mut found: BTreeMap<u32, BTreeMap<u32, usize>> = BTreeMap::new();
    for (i, name) in skin.names.iter().enumerate() {
        let Some(rest) = name.strip_prefix("hair_") else {
            continue;
        };
        let Some((c, k)) = rest.split_once('_') else {
            continue;
        };
        if let (Ok(c), Ok(k)) = (c.parse::<u32>(), k.parse::<u32>()) {
            found.entry(c).or_default().insert(k, i);
        }
    }
    found
        .into_values()
        .map(|links| links.into_values().collect())
        .filter(|c: &Vec<usize>| c.len() >= 2)
        .collect()
}

/// Spheres for a Universal-rig humanoid in its rest pose: the head, neck,
/// chest, shoulders and upper arms, from its joint positions.
#[must_use]
pub fn humanoid_spheres(model: &Model) -> Vec<Sphere> {
    let Some(skin) = &model.skin else {
        return vec![];
    };
    let rest = rest_globals(model);
    let at = |name: &str| skin.names.iter().position(|n| n == name);
    let mut head = vec![];
    // The head from its own skin: the box around the skin-shaded vertices
    // bound mostly to `Head`, as a skull sphere and a face sphere in front.
    if let Some(h) = at("Head") {
        let inverse = Mat4::from_cols_array(&skin.basis).inverse();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for s in model
            .surfaces
            .iter()
            .filter(|s| s.material.shading == crate::material::Shading::Skin as u8)
        {
            for v in &s.vertices {
                if (0..4).any(|k| v.joints[k] as usize == h && v.weights[k] > 0.5) {
                    let p = inverse.transform_point3(Vec3::from(v.position));
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
        if lo.x < hi.x {
            let c = (lo + hi) * 0.5;
            let e = hi - lo;
            head.push(Sphere {
                joint: h,
                center: c + Vec3::new(0.0, e.y * 0.08, -e.z * 0.08),
                radius: e.x.min(e.z) * 0.5,
            });
            head.push(Sphere {
                joint: h,
                center: c + Vec3::new(0.0, -e.y * 0.12, e.z * 0.18),
                radius: e.x * 0.42,
            });
        } else {
            for (offset, radius) in [
                (Vec3::new(0.0, 0.095, 0.0), 0.098),
                (Vec3::new(0.0, 0.04, 0.035), 0.085),
            ] {
                head.push(Sphere {
                    joint: h,
                    center: rest[h].transform_point3(Vec3::ZERO) + offset,
                    radius,
                });
            }
        }
    }
    let mut out = head;
    let mut add = |name: &str, offset: Vec3, radius: f32| {
        if let Some(j) = at(name) {
            out.push(Sphere {
                joint: j,
                center: rest[j].transform_point3(Vec3::ZERO) + offset,
                radius,
            });
        }
    };
    add("neck_01", Vec3::new(0.0, 0.03, 0.0), 0.06);
    add("spine_03", Vec3::new(0.0, 0.06, -0.01), 0.14);
    add("spine_02", Vec3::new(0.0, 0.0, -0.01), 0.14);
    add("spine_01", Vec3::new(0.0, 0.0, -0.01), 0.13);
    add("pelvis", Vec3::new(0.0, 0.0, 0.0), 0.15);
    for side in ["l", "r"] {
        add(&format!("clavicle_{side}"), Vec3::ZERO, 0.06);
        add(&format!("upperarm_{side}"), Vec3::ZERO, 0.055);
    }
    out
}

fn locals(model: &Model, clip: u16, t: f32) -> Vec<(Vec3, Quat, Vec3)> {
    crate::animation::sample(
        model,
        Binding {
            clip,
            mode: Mode::Loop,
            transition_seconds: 0.0,
        },
        t,
    )
    .into_iter()
    .map(|l| (l.translation, l.rotation, l.scale))
    .collect()
}

fn globals(model: &Model, locals: &[(Vec3, Quat, Vec3)]) -> Vec<Mat4> {
    let mut g = vec![Mat4::IDENTITY; model.bones.len()];
    for (i, b) in model.bones.iter().enumerate() {
        let (t, r, s) = locals[i];
        let l = Mat4::from_scale_rotation_translation(s, r, t);
        g[i] = if b.parent >= 0 {
            g[b.parent as usize] * l
        } else {
            l
        };
    }
    g
}

fn rest_globals(model: &Model) -> Vec<Mat4> {
    let skin = model.skin.as_ref().expect("skinned");
    let l: Vec<_> = skin
        .rest
        .iter()
        .map(|r| {
            (
                Vec3::from(r.translation),
                Quat::from_array(r.rotation).normalize(),
                Vec3::from(r.scale),
            )
        })
        .collect();
    globals(model, &l)
}

/// Bakes `source`'s motion plus the hair's response to `wind` (a function
/// of film time, so a scene can script a gust) into a new clip `id` of
/// `duration` seconds at `fps` keys a second. Hair joints get rotation
/// keys; every other joint keeps the source clip's keys. `settle` seconds
/// are simulated before the clip starts, so it opens at rest in the wind.
///
/// # Errors
///
/// Returns a message when the model has no skin, no hair chains, no clip
/// `source`, or the settings are out of range.
#[allow(clippy::too_many_arguments)]
pub fn bake(
    model: &Model,
    source: u16,
    id: u16,
    duration: f32,
    fps: f32,
    settle: f32,
    spring: Spring,
    spheres: &[Sphere],
    wind: impl Fn(f32) -> Wind,
) -> Result<Clip, String> {
    let skin = model.skin.as_ref().ok_or("The model has no skin")?;
    let chains = chains(model);
    if chains.is_empty() {
        return Err("The model has no hair chains (joints hair_<chain>_<link>)".into());
    }
    let src = model
        .clips
        .iter()
        .find(|c| c.id == source)
        .ok_or("The model has no source clip")?;
    if !(duration > 0.0 && duration <= 600.0)
        || !(1.0..=240.0).contains(&fps)
        || !(0.0..=30.0).contains(&settle)
        || !(spring.rate >= fps && spring.rate <= 2000.0)
        || !(0.0..=1.0).contains(&spring.damping)
    {
        return Err("Hair bake settings are out of range".into());
    }
    let rest = rest_globals(model);
    let src_duration = src.duration.max(1e-3);
    let src_time = |t: f32| t.rem_euclid(src_duration);
    // Particles: each chain's joints, then a tip one link past the last.
    struct Chain {
        joints: Vec<usize>,
        lengths: Vec<f32>,
        p: Vec<Vec3>,
        prev: Vec<Vec3>,
    }
    let animated = |t: f32| globals(model, &locals(model, source, src_time(t)));
    let points = |g: &[Mat4], joints: &[usize]| -> Vec<Vec3> {
        let mut pts: Vec<Vec3> = joints
            .iter()
            .map(|&j| g[j].transform_point3(Vec3::ZERO))
            .collect();
        let n = pts.len();
        let last = pts[n - 1] - pts[n - 2];
        pts.push(pts[n - 1] + last);
        pts
    };
    let start = -settle;
    let g0 = animated(start);
    let mut state: Vec<Chain> = chains
        .iter()
        .map(|joints| {
            let p = points(&g0, joints);
            let lengths = p.windows(2).map(|w| (w[1] - w[0]).length()).collect();
            Chain {
                joints: joints.clone(),
                lengths,
                prev: p.clone(),
                p,
            }
        })
        .collect();
    let dt = 1.0 / spring.rate;
    let keep = spring.damping.powf(dt);
    let frames = (duration * fps).ceil() as usize + 1;
    let mut keys: BTreeMap<usize, Vec<(f32, [f32; 4])>> = BTreeMap::new();
    let mut t = start;
    for frame in 0..frames {
        let ft = frame as f32 / fps;
        // Step the simulation up to this frame's time.
        while t < ft - 1e-6 {
            t += dt;
            let g = animated(t);
            let posed_spheres: Vec<(Vec3, f32)> = spheres
                .iter()
                .map(|s| {
                    let m = g[s.joint] * rest[s.joint].inverse();
                    (m.transform_point3(s.center), s.radius)
                })
                .collect();
            let w = wind(t);
            for c in &mut state {
                let target = points(&g, &c.joints);
                // The root follows the head exactly.
                c.p[0] = target[0];
                c.prev[0] = target[0];
                for k in 1..c.p.len() {
                    let v = (c.p[k] - c.prev[k]) * keep;
                    let air = w.at(c.p[k], t) - v / dt;
                    let a = (target[k] - c.p[k]) * spring.stiffness
                        + Vec3::NEG_Y * spring.gravity
                        + air * spring.drag;
                    c.prev[k] = c.p[k];
                    c.p[k] += v + a * dt * dt;
                }
                for _ in 0..4 {
                    for k in 1..c.p.len() {
                        let d = c.p[k] - c.p[k - 1];
                        let len = d.length().max(1e-6);
                        c.p[k] = c.p[k - 1] + d * (c.lengths[k - 1] / len);
                    }
                    for k in 1..c.p.len() {
                        for &(center, radius) in &posed_spheres {
                            let d = c.p[k] - center;
                            let len = d.length();
                            if len < radius && len > 1e-6 {
                                c.p[k] = center + d * (radius / len);
                            }
                        }
                    }
                }
            }
        }
        // Rotations: turn each link so its segment points at its simulated
        // successor, from the parent's adjusted frame.
        let g = animated(ft);
        let mut world_rot: BTreeMap<usize, Quat> = BTreeMap::new();
        for c in &state {
            let target = points(&g, &c.joints);
            for (k, &j) in c.joints.iter().enumerate() {
                let parent = model.bones[j].parent;
                let parent_rot = if parent >= 0 {
                    world_rot
                        .get(&(parent as usize))
                        .copied()
                        .unwrap_or_else(|| g[parent as usize].to_scale_rotation_translation().1)
                } else {
                    Quat::IDENTITY
                };
                let animated_rot = g[j].to_scale_rotation_translation().1;
                let from = (target[k + 1] - target[k]).normalize_or_zero();
                let to = (c.p[k + 1] - c.p[k]).normalize_or_zero();
                let turn = if from.length_squared() > 0.5 && to.length_squared() > 0.5 {
                    Quat::from_rotation_arc(from, to)
                } else {
                    Quat::IDENTITY
                };
                let world = (turn * animated_rot).normalize();
                world_rot.insert(j, world);
                let local = (parent_rot.inverse() * world).normalize();
                keys.entry(j).or_default().push((ft, local.to_array()));
            }
        }
    }
    let hair: std::collections::BTreeSet<usize> = chains.iter().flatten().copied().collect();
    let mut bones: Vec<BoneKeys> = src
        .bones
        .iter()
        .filter(|b| !hair.contains(&b.bone))
        .cloned()
        .collect();
    // Source keys beyond the source clip's length wrap, as it loops.
    if duration > src_duration {
        for b in &mut bones {
            let wrap = |keys: &mut Vec<(f32, [f32; 4])>| {
                let base = keys.clone();
                let mut k = 1.0;
                while k * src_duration < duration {
                    keys.extend(base.iter().map(|(t, v)| (t + k * src_duration, *v)));
                    k += 1.0;
                }
            };
            wrap(&mut b.rotation);
            let wrap3 = |keys: &mut Vec<(f32, [f32; 3])>| {
                let base = keys.clone();
                let mut k = 1.0;
                while k * src_duration < duration {
                    keys.extend(base.iter().map(|(t, v)| (t + k * src_duration, *v)));
                    k += 1.0;
                }
            };
            wrap3(&mut b.translation);
            wrap3(&mut b.scale);
        }
    }
    // Keys strictly increasing in time, within the clip.
    fn tidy<const N: usize>(keys: &mut Vec<(f32, [f32; N])>, end: f32) {
        let mut last = f32::NEG_INFINITY;
        keys.retain(|k| {
            let ok = k.0 > last + 1e-5 && k.0 <= end + 1e-4;
            if ok {
                last = k.0;
            }
            ok
        });
    }
    for b in &mut bones {
        tidy(&mut b.rotation, duration);
        tidy(&mut b.translation, duration);
        tidy(&mut b.scale, duration);
    }
    for (bone, rotation) in keys {
        bones.push(BoneKeys {
            bone,
            translation: vec![],
            rotation: {
                let mut r = rotation;
                tidy(&mut r, duration);
                r
            },
            scale: vec![],
        });
    }
    let _ = skin;
    Ok(Clip {
        id,
        duration,
        bones,
    })
}

/// Aims `joints` (eyes: joints whose rest frame is the head's, looking
/// along the head's +Z) at `target` (skeleton space) on every key of clip
/// `clip`, sampled at `fps`, turning each at most `limit` radians from
/// where the head points it. The eyes then hold the target as the head
/// moves, the way a gaze locks onto a lens.
///
/// # Errors
///
/// Returns a message when the model has no skin or no clip `clip`.
pub fn aim(
    model: &mut Model,
    clip: u16,
    joints: &[usize],
    target: Vec3,
    fps: f32,
    limit: f32,
) -> Result<(), String> {
    model.skin.as_ref().ok_or("The model has no skin")?;
    let index = model
        .clips
        .iter()
        .position(|c| c.id == clip)
        .ok_or("The model has no clip to aim")?;
    let duration = model.clips[index].duration.max(1e-3);
    let rest = rest_globals(model);
    let frames = (duration * fps).ceil() as usize + 1;
    let mut keys: BTreeMap<usize, Vec<(f32, [f32; 4])>> = BTreeMap::new();
    for f in 0..frames {
        let t = (f as f32 / fps).min(duration);
        let l = locals(model, clip, t);
        let g = globals(model, &l);
        for &j in joints {
            let parent = model.bones[j].parent;
            let p = if parent >= 0 {
                g[parent as usize]
            } else {
                Mat4::IDENTITY
            };
            let (_, prot, _) = p.to_scale_rotation_translation();
            let (_, rrest, _) = rest[j].to_scale_rotation_translation();
            let world = g[j].to_scale_rotation_translation().1;
            let forward_local = rrest.inverse() * Vec3::Z;
            let forward = (world * forward_local).normalize();
            let to = (target - g[j].transform_point3(Vec3::ZERO)).normalize_or_zero();
            let mut turn = Quat::from_rotation_arc(forward, to);
            let (axis, angle) = turn.to_axis_angle();
            if angle > limit {
                turn = Quat::from_axis_angle(axis, limit);
            }
            let local = (prot.inverse() * turn * world).normalize();
            keys.entry(j).or_default().push((t, local.to_array()));
        }
    }
    let c = &mut model.clips[index];
    c.bones.retain(|b| !joints.contains(&b.bone));
    for (bone, rotation) in keys {
        c.bones.push(BoneKeys {
            bone,
            translation: vec![],
            rotation,
            scale: vec![],
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{Bone, RestPose, Skin};

    /// A head and one hanging chain of three links, 5 cm apart, along -Y.
    fn model() -> Model {
        let rest = |t: [f32; 3]| RestPose {
            translation: t,
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        };
        let names = ["Head", "hair_0_0", "hair_0_1", "hair_0_2"];
        Model {
            graph: None,
            markers: vec![],
            states: Default::default(),
            skin: Some(Skin {
                names: names.iter().map(|s| s.to_string()).collect(),
                rest: vec![
                    rest([0.0, 1.6, 0.0]),
                    rest([0.0, 0.0, -0.1]),
                    rest([0.0, -0.05, 0.0]),
                    rest([0.0, -0.05, 0.0]),
                ],
                inverse_bind: vec![Mat4::IDENTITY.to_cols_array(); 4],
                basis: Mat4::IDENTITY.to_cols_array(),
            }),
            source: String::new(),
            source_sha256: String::new(),
            surfaces: vec![],
            bones: vec![
                Bone {
                    parent: -1,
                    pivot: [0.0; 3],
                },
                Bone {
                    parent: 0,
                    pivot: [0.0; 3],
                },
                Bone {
                    parent: 1,
                    pivot: [0.0; 3],
                },
                Bone {
                    parent: 2,
                    pivot: [0.0; 3],
                },
            ],
            clips: vec![Clip {
                id: 0,
                duration: 1.0,
                bones: vec![],
            }],
            height: 1.8,
            attachments: vec![],
        }
    }

    fn tip(model: &Model, clip: &Clip, t: f32) -> Vec3 {
        let mut m = model.clone();
        m.clips.push(clip.clone());
        let g = globals(&m, &locals(&m, clip.id, t));
        g[3].transform_point3(Vec3::ZERO)
    }

    #[test]
    fn aimed_eyes_look_at_the_target() {
        let mut m = model();
        aim(&mut m, 0, &[1], Vec3::new(1.0, 1.6, 1.0), 30.0, 1.5).unwrap();
        let g = globals(&m, &locals(&m, 0, 0.2));
        let forward = g[1].transform_vector3(Vec3::Z).normalize();
        let to = (Vec3::new(1.0, 1.6, 1.0) - g[1].transform_point3(Vec3::ZERO)).normalize();
        assert!(forward.dot(to) > 0.999, "{forward} {to}");
    }

    #[test]
    fn finds_chains_by_name() {
        assert_eq!(chains(&model()), vec![vec![1, 2, 3]]);
    }

    #[test]
    fn calm_hair_hangs_at_rest_and_wind_blows_it_downwind() {
        let m = model();
        let calm = bake(&m, 0, 9, 1.0, 30.0, 2.0, Spring::default(), &[], |_| {
            Wind::CALM
        })
        .unwrap();
        let rest = tip(&m, &calm, 0.0);
        // Gravity and the spring balance a little below the rest pose,
        // and the chain keeps its lengths.
        assert!(rest.y < 1.6 - 0.09, "{rest}");
        assert!((rest - Vec3::new(0.0, 1.6, -0.1)).length() < 0.105);
        let windy = bake(&m, 0, 9, 1.0, 30.0, 2.0, Spring::default(), &[], |_| Wind {
            velocity: Vec3::new(6.0, 0.0, 0.0),
            ..Wind::CALM
        })
        .unwrap();
        let blown = tip(&m, &windy, 0.5);
        assert!(blown.x > rest.x + 0.02, "{blown} vs {rest}");
    }

    #[test]
    fn an_upward_gust_lifts_the_chain() {
        let m = model();
        let gust = bake(&m, 0, 9, 1.0, 30.0, 1.0, Spring::default(), &[], |t| Wind {
            velocity: if t > 0.2 {
                Vec3::new(4.0, 14.0, 0.0)
            } else {
                Vec3::ZERO
            },
            ..Wind::CALM
        })
        .unwrap();
        let (a, b) = (tip(&m, &gust, 0.0), tip(&m, &gust, 0.9));
        assert!(b.y > a.y + 0.03, "{a} -> {b}");
    }

    #[test]
    fn spheres_keep_the_chain_out() {
        let m = model();
        let ball = [Sphere {
            joint: 0,
            center: Vec3::new(0.0, 1.5, -0.08),
            radius: 0.06,
        }];
        let clip = bake(&m, 0, 9, 0.5, 30.0, 1.0, Spring::default(), &ball, |_| {
            Wind::CALM
        })
        .unwrap();
        let t = tip(&m, &clip, 0.4);
        assert!((t - Vec3::new(0.0, 1.5, -0.08)).length() >= 0.059, "{t}");
    }

    #[test]
    fn refuses_models_without_chains_and_bad_settings() {
        let mut m = model();
        assert!(
            bake(&m, 0, 9, 1.0, 30.0, 0.0, Spring::default(), &[], |_| {
                Wind::CALM
            })
            .is_ok()
        );
        assert!(
            bake(&m, 0, 9, -1.0, 30.0, 0.0, Spring::default(), &[], |_| {
                Wind::CALM
            })
            .is_err()
        );
        assert!(
            bake(&m, 7, 9, 1.0, 30.0, 0.0, Spring::default(), &[], |_| {
                Wind::CALM
            })
            .is_err()
        );
        for k in 1..4 {
            m.skin.as_mut().unwrap().names[k] = format!("strand{k}");
        }
        assert!(
            bake(&m, 0, 9, 1.0, 30.0, 0.0, Spring::default(), &[], |_| {
                Wind::CALM
            })
            .is_err()
        );
    }
}
