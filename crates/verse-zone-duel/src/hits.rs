//! Hit detection: the blade's swept volume against the defender's shield
//! and body zones, through the `physics` crate's capsule queries.
//!
//! Each step the defender's pose becomes a small query scene: a sphere for
//! the head, capsules for the torso, the upper arms, forearms, thighs, and
//! shins, and the shield as a thin solid of its own outline. Points along
//! the striking part of the blade each sweep a small sphere from where they
//! were to where they are; the earliest contact of any of them is what the
//! blow met. A shield in the way meets the blade before the body behind
//! it.

use glam::{DQuat, DVec3, Vec3};
use physics::queries::{
    Capsule, CapsuleCollider, ColliderKey, Filter, Life, Mesh, MeshCollider, Pose, Scene, Triangle,
    Usage,
};

use crate::rig::Rig;
use crate::rules::{Kit, Weapon, Zone};

/// Thickness of the blade's swept spheres, m.
pub const BLADE_RADIUS: f32 = 0.03;
/// Points along the striking part of the weapon that sweep.
const SAMPLES: usize = 6;
/// The query scene's instance.
const INSTANCE: u64 = 7;
/// The shield's collider shape number.
const SHIELD: u32 = 100;
/// Half the shield's thickness, m.
pub const SHIELD_HALF_THICKNESS: f32 = 0.025;

/// What a blade met.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Shield,
    Body(Zone),
}

/// The first thing a blade met, where, and how far through the step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub struck: Struck,
    pub at: Vec3,
    pub fraction: f32,
}

/// A body zone's capsules on `rig`: segment ends and radius.
#[must_use]
pub fn zones(rig: &Rig) -> Vec<(Zone, Vec3, Vec3, f32)> {
    let up = rig.torso * Vec3::Y;
    vec![
        (Zone::Head, rig.head, rig.head + up * 0.04, 0.13),
        (
            Zone::Torso,
            rig.pelvis + up * 0.08,
            rig.neck - up * 0.08,
            0.19,
        ),
        (Zone::LeftArm, rig.shoulders[0], rig.elbows[0], 0.065),
        (Zone::LeftArm, rig.elbows[0], rig.hands[0], 0.055),
        (Zone::RightArm, rig.shoulders[1], rig.elbows[1], 0.065),
        (Zone::RightArm, rig.elbows[1], rig.hands[1], 0.055),
        (Zone::LeftLeg, rig.hips[0], rig.knees[0], 0.085),
        (Zone::LeftLeg, rig.knees[0], rig.feet[0], 0.065),
        (Zone::RightLeg, rig.hips[1], rig.knees[1], 0.085),
        (Zone::RightLeg, rig.knees[1], rig.feet[1], 0.065),
    ]
}

/// The outline of `kit`'s shield in its own plane (x right, y up), m: a
/// heater's flat top and point, or a round shield.
#[must_use]
pub fn outline(kit: Kit) -> Vec<[f32; 2]> {
    match kit {
        Kit::Knight => vec![
            [0.0, 0.37],
            [-0.29, 0.37],
            [-0.29, 0.08],
            [-0.24, -0.1],
            [-0.15, -0.25],
            [0.0, -0.38],
            [0.15, -0.25],
            [0.24, -0.1],
            [0.29, 0.08],
            [0.29, 0.37],
        ],
        Kit::Prussian => (0..20)
            .map(|i| {
                let a = i as f32 / 20.0 * std::f32::consts::TAU;
                [a.cos() * 0.4, a.sin() * 0.4]
            })
            .collect(),
    }
}

/// The shield as a closed solid in its own frame: the outline's front and
/// back faces and its rim.
fn shield_mesh(kit: Kit) -> Result<Mesh, String> {
    let points = outline(kit);
    let t = f64::from(SHIELD_HALF_THICKNESS);
    let p = |q: [f32; 2], z: f64| DVec3::new(f64::from(q[0]), f64::from(q[1]), z);
    let center = |z: f64| DVec3::new(0.0, 0.0, z);
    let mut triangles = Vec::new();
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        triangles.push(Triangle([center(t), p(a, t), p(b, t)]));
        triangles.push(Triangle([center(-t), p(b, -t), p(a, -t)]));
        triangles.push(Triangle([p(a, t), p(a, -t), p(b, -t)]));
        triangles.push(Triangle([p(a, t), p(b, -t), p(b, t)]));
    }
    triangles.retain(|tri| {
        (tri.0[1] - tri.0[0])
            .cross(tri.0[2] - tri.0[0])
            .length_squared()
            > 1e-14
    });
    Mesh::compile(triangles)
}

fn key(shape: u32) -> ColliderKey {
    ColliderKey {
        life: Life {
            instance: INSTANCE,
            entity: 1,
            generation: 0,
        },
        shape,
    }
}

fn d(v: Vec3) -> DVec3 {
    v.as_dvec3()
}

/// The defender's query scene.
fn scene(rig: &Rig, kit: Kit) -> Option<(Scene, Vec<Zone>)> {
    let mut scene = Scene::default();
    let mut names = Vec::new();
    for (i, (zone, a, b, r)) in zones(rig).into_iter().enumerate() {
        scene
            .insert_capsule(CapsuleCollider {
                key: key(i as u32),
                capsule: Capsule {
                    a: d(a),
                    b: d(b),
                    radius: f64::from(r),
                },
                layers: 1,
                usage: Usage::Damage,
            })
            .ok()?;
        names.push(zone);
    }
    scene
        .insert(MeshCollider {
            key: key(SHIELD),
            layers: 1,
            usage: Usage::Blocking,
            mesh: shield_mesh(kit).ok()?,
        })
        .ok()?;
    let q = rig.shield_rot;
    scene
        .set_pose(
            key(SHIELD),
            Pose {
                position: d(rig.shield_at),
                rotation: DQuat::from_xyzw(
                    f64::from(q.x),
                    f64::from(q.y),
                    f64::from(q.z),
                    f64::from(q.w),
                )
                .normalize(),
            },
        )
        .ok()?;
    Some((scene, names))
}

fn filter() -> Filter {
    Filter {
        instance: INSTANCE,
        layers: u32::MAX,
        ignore: None,
        exclude: None,
        usages: Usage::Blocking.bit() | Usage::Damage.bit(),
        limit: 32,
    }
}

/// Sweeps the attacker's weapon from its pose `before` to its pose `after`
/// against `defender`, whose kit sets the shield's shape. Returns the
/// earliest contact, the shield winning a tie with the body behind it.
#[must_use]
pub fn sweep(
    defender: &Rig,
    kit: Kit,
    before: &Rig,
    after: &Rig,
    weapon: Weapon,
) -> Option<Contact> {
    let (scene, names) = scene(defender, kit)?;
    let filter = filter();
    let mut best: Option<Contact> = None;
    let (from, to) = weapon.strikes;
    for i in 0..SAMPLES {
        let k = from + (to - from) * i as f32 / (SAMPLES - 1) as f32;
        let p0 = before.grip + before.blade * before.length * k;
        let p1 = after.grip + after.blade * after.length * k;
        let capsule = Capsule {
            a: d(p0),
            b: d(p0),
            radius: f64::from(BLADE_RADIUS),
        };
        let Ok(results) = scene.sweep(capsule, d(p1 - p0), filter) else {
            continue;
        };
        for hit in results.hits {
            let struck = if hit.collider.shape == SHIELD {
                Struck::Shield
            } else {
                match names.get(hit.collider.shape as usize) {
                    Some(zone) => Struck::Body(*zone),
                    None => continue,
                }
            };
            let fraction = hit.fraction as f32;
            let better = match best {
                None => true,
                Some(b) => {
                    fraction < b.fraction - 1e-4
                        || (fraction <= b.fraction + 1e-4
                            && struck == Struck::Shield
                            && b.struck != Struck::Shield)
                }
            };
            if better {
                best = Some(Contact {
                    struck,
                    at: hit.position.as_vec3(),
                    fraction,
                });
            }
        }
    }
    best
}

/// Whether the attacker's shield face, driven forward in a bash, meets the
/// defender: returns where.
#[must_use]
pub fn bash(attacker: &Rig, defender: &Rig, kit: Kit) -> Option<Vec3> {
    let (scene, _) = scene(defender, kit)?;
    let face = attacker.shield_rot * Vec3::Z;
    let at = attacker.shield_at + face * 0.06;
    let capsule = Capsule {
        a: d(at - attacker.shield_rot * Vec3::Y * 0.15),
        b: d(at + attacker.shield_rot * Vec3::Y * 0.15),
        radius: 0.2,
    };
    let results = scene.overlap(capsule, filter()).ok()?;
    results.hits.first().map(|h| h.position.as_vec3())
}
