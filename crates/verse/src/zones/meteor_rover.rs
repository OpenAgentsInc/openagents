//! The CAD Lab rover in the Meteor Showcase: the scientific rover built by
//! talking in CAD Lab (#11627, `docs/cad-lab/mvp.md`), standing on the lot
//! between the houses as the swarm comes down, simulated rather than
//! animated (`verse --meteor-showcase --rover`, `meteor_rover_capture`).
//!
//! The rover is its CAD Lab version's own glTF (`assets/verse/cad/rover.glb`,
//! one node per solid, named by its instance) and mass properties
//! (`assets/verse/cad/rover.json`: each instance's mass, centre of mass and
//! inertia from the version's check report, and its joints). It stands as
//! rigid bodies in a [`physics::World`] of its own:
//!
//! - the body (chassis, avionics bay, cassette, dish, rails, cameras),
//!   whose instances share one body, with their masses, centres and
//!   inertias summed about its centre of mass;
//! - two rockers on hinges to the body and two bogies on hinges to the
//!   rockers, as the CAD joints place them, held level by soft servos (the
//!   differential the CAD model leaves out);
//! - six wheels on hinges to the arms, the mast with the camera head welded
//!   to the body, and the two solar wings welded to it. These joints are
//!   weak: a joint whose force or torque reached its limit in a step breaks,
//!   and the part flies free with sparks.
//!
//! A meteor's explosion near the rover pushes every part away from its
//! centre with an impulse that falls off to nothing at the blast's edge and
//! grows with the part's exposed area, so light, broad parts (wings, the
//! mast) are thrown harder than the heavy body and tear off at their joints.
//! A meteor that lands on the rover also gives up its own momentum
//! ([`METEOR_MASS`] × [`METEOR_SPEED`] along its flight) to the part it
//! strikes. What follows is the physics: the rover is knocked, tipped or
//! rolled by momentum, its parts tumble and slide on the ground, and a hard
//! landing raises dust.
//!
//! The parts collide with the ground only, not with each other or with the
//! houses' debris (each lives in its own world). Products of inertia are
//! dropped (the body frame is the rover's frame; they are under a tenth of
//! the principal moments for every body). Native only: the glTF is built
//! into the binary, which the browser build leaves out.

use glam::{DQuat, DVec3, Vec3};
use physics::body::Body;
use physics::collision::{Collider, Filter, Material, Shape};
use physics::joint::{Joint, JointId, JointKind, JointLimits, Servo};
use physics::world::{BodyId, World};
use serde_json::Value;
use std::sync::OnceLock;

use crate::pbr::LitVertex;

/// The rover's glTF and mass properties, exported from CAD Lab.
const GLB: &[u8] = include_bytes!("../../../../assets/verse/cad/rover.glb");
const SPEC: &str = include_str!("../../../../assets/verse/cad/rover.json");

/// The physics step, s.
pub const STEP: f64 = 1.0 / 240.0;
/// Gravity, m/s².
pub const GRAVITY: f64 = 9.81;
/// A meteor's mass, kg, and its speed when it lands, m/s: a burning rock
/// about 0.25 m across falling from 30 m in a second.
pub const METEOR_MASS: f64 = 18.0;
pub const METEOR_SPEED: f64 = 32.0;
/// How far from a meteor's landing point the rover counts as struck, m.
pub const STRUCK: f64 = 1.2;
/// The blast's push on a part per square metre it shows the blast at the
/// centre, N s/m² (falling off linearly to nothing at the blast's edge).
pub const BLAST_PRESSURE: f64 = 650.0;
/// A contact this hard (impulse in one step, N s) raises dust.
const DUST_IMPULSE: f64 = 1.5;

/// Collision groups: the rover's parts meet the ground, never each other.
const PARTS: Filter = Filter { group: 1, mask: 2 };
const GROUND: Filter = Filter { group: 2, mask: 1 };

/// Which body each instance rides on; any other instance rides on the body.
const BODIES: [(&str, &[&str]); 14] = [
    ("chassis", &[]),
    ("mast", &["mast", "camera_head"]),
    ("solar_panel_l", &["solar_panel_l"]),
    ("solar_panel_r", &["solar_panel_r"]),
    ("rocker_l", &["rocker_l"]),
    ("rocker_r", &["rocker_r"]),
    ("bogie_l", &["bogie_l"]),
    ("bogie_r", &["bogie_r"]),
    ("wheel_lf", &["wheel_lf"]),
    ("wheel_lm", &["wheel_lm"]),
    ("wheel_lr", &["wheel_lr"]),
    ("wheel_rf", &["wheel_rf"]),
    ("wheel_rm", &["wheel_rm"]),
    ("wheel_rr", &["wheel_rr"]),
];

/// A wheel's rolling radius and half width, m.
const WHEEL_RADIUS: f64 = 0.115;
const WHEEL_HALF_WIDTH: f64 = 0.04;

/// Weak joints: largest force, N, and torque, N m, before they break.
const WHEEL_BREAK: (f64, f64) = (2_600.0, 260.0);
const MAST_BREAK: (f64, f64) = (1_800.0, 420.0);
const WING_BREAK: (f64, f64) = (900.0, 140.0);

/// From the CAD frame (mm, X forward, Y left, Z up) to the rover's frame
/// in Verse (m, X forward, Y up, Z right): the glTF's own frame.
fn from_cad(p: [f64; 3]) -> DVec3 {
    DVec3::new(p[0], p[2], -p[1]) / 1000.0
}

/// A CAD inertia tensor (kg m², CAD axes) in the rover's frame.
fn tensor_from_cad(i: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    // R maps (x, y, z) to (x, z, -y): rows (1,0,0), (0,0,1), (0,-1,0).
    let r = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];
    let mut out = [[0.0; 3]; 3];
    for (a, row) in out.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            *cell = (0..3)
                .flat_map(|k| (0..3).map(move |l| (k, l)))
                .map(|(k, l)| r[a][k] * i[k][l] * r[b][l])
                .sum();
        }
    }
    out
}

/// One solid of the glTF, in the rover's frame.
struct Solid {
    instance: String,
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<u32>,
    color: [f32; 3],
}

/// The glTF the CAD Lab wrote: one node per solid, its instance in
/// `extras.instance`, positions, normals and indices, a colour per
/// material.
fn solids() -> &'static Vec<Solid> {
    static SOLIDS: OnceLock<Vec<Solid>> = OnceLock::new();
    SOLIDS.get_or_init(|| parse_glb(GLB).unwrap_or_default())
}

fn parse_glb(bytes: &[u8]) -> Option<Vec<Solid>> {
    let word = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    if bytes.get(..4)? != b"glTF" {
        return None;
    }
    let json_len = word(12)?;
    let json: Value = serde_json::from_slice(bytes.get(20..20 + json_len)?).ok()?;
    let bin = 20 + json_len + 8;
    let floats = |accessor: &Value| -> Option<Vec<f32>> {
        let view = &json["bufferViews"][accessor["bufferView"].as_u64()? as usize];
        let start = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let count = accessor["count"].as_u64()? as usize * 3;
        let raw = bytes.get(start..start + count * 4)?;
        Some(
            raw.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        )
    };
    let mut out = Vec::new();
    for node in json["nodes"].as_array()? {
        let mesh = &json["meshes"][node["mesh"].as_u64()? as usize];
        let primitive = &mesh["primitives"][0];
        let accessor = |name: &str| {
            &json["accessors"][primitive["attributes"][name].as_u64().unwrap_or(0) as usize]
        };
        let positions = floats(accessor("POSITION"))?;
        let normals = floats(accessor("NORMAL"))?;
        let index_accessor = &json["accessors"][primitive["indices"].as_u64()? as usize];
        let view = &json["bufferViews"][index_accessor["bufferView"].as_u64()? as usize];
        let start = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let count = index_accessor["count"].as_u64()? as usize;
        let indices = bytes
            .get(start..start + count * 4)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let material = &json["materials"][primitive["material"].as_u64().unwrap_or(0) as usize];
        let factor = &material["pbrMetallicRoughness"]["baseColorFactor"];
        let color = [0, 1, 2].map(|k| factor[k].as_f64().unwrap_or(0.6) as f32);
        let triple = |v: &[f32]| {
            v.chunks_exact(3)
                .map(|c| Vec3::new(c[0], c[1], c[2]))
                .collect()
        };
        out.push(Solid {
            instance: node["extras"]["instance"]
                .as_str()
                .unwrap_or("chassis")
                .to_owned(),
            positions: triple(&positions),
            normals: triple(&normals),
            indices,
            color,
        });
    }
    Some(out)
}

/// The mass properties and joints the CAD Lab measured.
fn spec() -> &'static Value {
    static SPEC_JSON: OnceLock<Value> = OnceLock::new();
    SPEC_JSON.get_or_init(|| serde_json::from_str(SPEC).unwrap_or(Value::Null))
}

fn body_of(instance: &str) -> &'static str {
    BODIES
        .iter()
        .find(|(_, members)| members.contains(&instance))
        .map_or("chassis", |(name, _)| name)
}

/// All body names.
fn body_names() -> Vec<&'static str> {
    BODIES.iter().map(|(n, _)| *n).collect()
}

/// Something worth an effect: sparks where a joint broke, dust where a part
/// landed hard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub effect: &'static str,
    pub at: Vec3,
}

/// A part of the rover: its body and its solids, posed relative to the
/// body's rest pose.
struct Part {
    name: &'static str,
    body: BodyId,
    /// Rest centre of mass in the rover's frame, m.
    rest: DVec3,
    /// Exposed area for the blast, m², from its box.
    area: f64,
    /// Indices into [`solids`] riding on it.
    solids: Vec<usize>,
}

/// A weak joint and what it holds.
struct Weak {
    id: JointId,
    part: &'static str,
}

/// The rover on the lot: its world, its parts, and what happened to it.
pub struct Rover {
    world: World,
    parts: Vec<Part>,
    weak: Vec<Weak>,
    /// Where the rover stands and which way it faces (radians about Y).
    origin: DVec3,
    heading: f64,
    /// Where meteors come from, for their direction of flight.
    caster: DVec3,
    carry: f64,
    events: Vec<Event>,
    /// The hardest knock so far: its time, s, and the speed it gave the
    /// body, m/s.
    hardest: Option<(f64, f64)>,
    broken: Vec<&'static str>,
    elapsed: f64,
}

/// A body's mass properties summed from its instances, in the rover frame:
/// mass, centre of mass, and the principal moments about it (products
/// dropped).
fn mass_of(instances: &[&Value]) -> (f64, DVec3, DVec3) {
    let mut mass = 0.0;
    let mut first = DVec3::ZERO;
    for i in instances {
        let m = i["mass_kg"].as_f64().unwrap_or(0.0);
        let c = from_cad(triple(&i["com_mm"]));
        mass += m;
        first += c * m;
    }
    if mass <= 0.0 {
        return (1.0, DVec3::ZERO, DVec3::splat(0.01));
    }
    let com = first / mass;
    let mut total = [[0.0; 3]; 3];
    for i in instances {
        let m = i["mass_kg"].as_f64().unwrap_or(0.0);
        let d = from_cad(triple(&i["com_mm"])) - com;
        let local = tensor_from_cad(&matrix(&i["inertia_kg_m2"]));
        let d2 = d.length_squared();
        let da = d.to_array();
        for a in 0..3 {
            for b in 0..3 {
                let delta = if a == b { 1.0 } else { 0.0 };
                total[a][b] += local[a][b] + m * (d2 * delta - da[a] * da[b]);
            }
        }
    }
    let inertia = DVec3::new(total[0][0], total[1][1], total[2][2]).max(DVec3::splat(1e-4));
    (mass, com, inertia)
}

fn triple(v: &Value) -> [f64; 3] {
    [0, 1, 2].map(|k| v[k].as_f64().unwrap_or(0.0))
}

fn matrix(v: &Value) -> [[f64; 3]; 3] {
    [0, 1, 2].map(|r| triple(&v[r]))
}

impl Rover {
    /// The rover standing at `origin` (on the ground) facing `heading`
    /// (radians about Y; 0 faces +X), with meteors flying in from `caster`.
    #[must_use]
    pub fn new(origin: Vec3, heading: f32, caster: Vec3) -> Self {
        let origin = origin.as_dvec3();
        let heading = f64::from(heading);
        let turn = DQuat::from_rotation_y(heading);
        let at = |p: DVec3| origin + turn * p;
        let mut world = World::new(STEP);
        // The ground: a broad static slab whose top is the lot.
        let ground = world.add(
            Body::new(1.0, DVec3::ONE, origin - DVec3::Y * 1.0)
                .with_kind(physics::body::BodyKind::Static),
        );
        world.add_collider(
            Collider::new(
                ground,
                Shape::Cuboid {
                    half: DVec3::new(300.0, 1.0, 300.0),
                },
            )
            .with_filter(GROUND)
            .with_material(Material {
                friction: 0.8,
                torsional: 0.02,
                restitution: 0.1,
            }),
        );
        let spec = spec();
        let instances: Vec<&Value> = spec["instances"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let solids = solids();
        let mut parts = Vec::new();
        for name in body_names() {
            let members: Vec<&Value> = instances
                .iter()
                .copied()
                .filter(|i| body_of(i["id"].as_str().unwrap_or_default()) == name)
                .collect();
            let (mass, com, inertia) = mass_of(&members);
            let mut body = Body::new(mass, inertia, at(com));
            body.orientation = turn;
            let id = world.add(body);
            let mine: Vec<usize> = solids
                .iter()
                .enumerate()
                .filter(|(_, s)| body_of(&s.instance) == name)
                .map(|(k, _)| k)
                .collect();
            // Colliders: a box round each solid, a capsule along the axle
            // for a wheel, and capsules along the arms.
            let mut area = 0.0;
            for &k in &mine {
                let s = &solids[k];
                let (lo, hi) = s.positions.iter().fold(
                    (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                    |(lo, hi), p| (lo.min(*p), hi.max(*p)),
                );
                let (lo, hi) = (lo.as_dvec3(), hi.as_dvec3());
                let half = ((hi - lo) / 2.0).max(DVec3::splat(0.005));
                let center = (lo + hi) / 2.0 - com;
                area += 4.0 * (half.x * half.y + half.y * half.z + half.x * half.z) / 3.0;
                let shape = if name.starts_with("wheel") {
                    Shape::Capsule {
                        radius: WHEEL_RADIUS,
                        half_length: WHEEL_HALF_WIDTH,
                    }
                } else {
                    Shape::Cuboid { half }
                };
                if name.starts_with("rocker") || name.starts_with("bogie") {
                    continue;
                }
                world.add_collider(
                    Collider::new(id, shape)
                        .at(
                            if name.starts_with("wheel") {
                                DVec3::ZERO
                            } else {
                                center
                            },
                            DQuat::IDENTITY,
                        )
                        .with_filter(PARTS)
                        .with_material(Material {
                            friction: 0.9,
                            torsional: 0.01,
                            restitution: 0.15,
                        }),
                );
            }
            parts.push(Part {
                name,
                body: id,
                rest: com,
                area,
                solids: mine,
            });
        }
        let body_id = |name: &str| parts.iter().find(|p| p.name == name).map(|p| p.body);
        let rest = |name: &str| {
            parts
                .iter()
                .find(|p| p.name == name)
                .map_or(DVec3::ZERO, |p| p.rest)
        };
        // The arms' capsules, from the joints' points.
        let joints: Vec<&Value> = spec["joints"]
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let point = |id: &str| {
            joints
                .iter()
                .find(|j| j["id"] == id)
                .map(|j| from_cad(triple(&j["origin_mm"])))
        };
        for (arm, ends) in [
            (
                "rocker_l",
                ["rocker_joint_l", "wheel_lf_spin", "bogie_joint_l"],
            ),
            (
                "rocker_r",
                ["rocker_joint_r", "wheel_rf_spin", "bogie_joint_r"],
            ),
            (
                "bogie_l",
                ["bogie_joint_l", "wheel_lm_spin", "wheel_lr_spin"],
            ),
            (
                "bogie_r",
                ["bogie_joint_r", "wheel_rm_spin", "wheel_rr_spin"],
            ),
        ] {
            let (Some(id), Some(pivot)) = (body_id(arm), point(ends[0])) else {
                continue;
            };
            for end in &ends[1..] {
                let Some(to) = point(end) else { continue };
                let middle = (pivot + to) / 2.0 - rest(arm);
                let along = to - pivot;
                let rotation = DQuat::from_rotation_arc(DVec3::Z, along.normalize_or(DVec3::Z));
                world.add_collider(
                    Collider::new(
                        id,
                        Shape::Capsule {
                            radius: 0.025,
                            half_length: along.length() / 2.0,
                        },
                    )
                    .at(middle, rotation)
                    .with_filter(PARTS),
                );
            }
        }
        let mut weak = Vec::new();
        let mut join = |world: &mut World,
                        a: &str,
                        b: &str,
                        at_rover: DVec3,
                        kind: JointKind,
                        limit: Option<(f64, f64)>,
                        part: &'static str| {
            let (Some(ia), Some(ib)) = (body_id(a), body_id(b)) else {
                return;
            };
            let anchor_a = at_rover - rest(a);
            let anchor_b = at_rover - rest(b);
            let mut joint = Joint::new(ia, anchor_a, ib, anchor_b, kind);
            if let Some((force, torque)) = limit {
                joint = joint.limited(force, torque);
            }
            if let Ok(id) = world.try_add_joint(joint) {
                if limit.is_some() {
                    weak.push(Weak { id, part });
                }
            }
        };
        // Axles: the CAD joints' axes are its +Y, the rover frame's -Z.
        let axle = DVec3::NEG_Z;
        let level = |limit: f64| JointKind::Hinge {
            axis: axle,
            relative: DQuat::IDENTITY,
            limits: Some(JointLimits {
                lower: -limit,
                upper: limit,
            }),
            servo: Some(Servo {
                target: 0.0,
                kp: 4_000.0,
                kd: 300.0,
                max_effort: 900.0,
                max_speed: 4.0,
            }),
        };
        let spin = JointKind::Hinge {
            axis: axle,
            relative: DQuat::IDENTITY,
            limits: None,
            servo: None,
        };
        for j in &joints {
            let id = j["id"].as_str().unwrap_or_default();
            let (Some(origin), Some(parent), Some(child)) =
                (point(id), j["parent"].as_str(), j["child"].as_str())
            else {
                continue;
            };
            let (pa, pb) = (body_of(parent), body_of(child));
            if pa == pb {
                continue;
            }
            match id {
                i if i.starts_with("rocker_joint") => join(
                    &mut world,
                    pa,
                    pb,
                    origin,
                    level(25f64.to_radians()),
                    None,
                    "rocker",
                ),
                i if i.starts_with("bogie_joint") => join(
                    &mut world,
                    pa,
                    pb,
                    origin,
                    level(30f64.to_radians()),
                    None,
                    "bogie",
                ),
                i if i.ends_with("_spin") => {
                    let part = BODIES
                        .iter()
                        .map(|(n, _)| *n)
                        .find(|n| *n == pb)
                        .unwrap_or("wheel");
                    join(&mut world, pa, pb, origin, spin, Some(WHEEL_BREAK), part);
                }
                _ => {}
            }
        }
        // The mast and the wings are welded where they meet the body.
        let weld = JointKind::Weld {
            relative: DQuat::IDENTITY,
        };
        let base = from_cad([250.0, -130.0, 500.0]);
        join(
            &mut world,
            "chassis",
            "mast",
            base,
            weld,
            Some(MAST_BREAK),
            "mast",
        );
        for (wing, y) in [("solar_panel_l", 230.0), ("solar_panel_r", -230.0)] {
            join(
                &mut world,
                "chassis",
                wing,
                from_cad([60.0, y, 520.0]),
                weld,
                Some(WING_BREAK),
                if y > 0.0 {
                    "solar_panel_l"
                } else {
                    "solar_panel_r"
                },
            );
        }
        world.solver.iterations = world.solver.iterations.max(24);
        Rover {
            world,
            parts,
            weak,
            origin,
            heading,
            caster: caster.as_dvec3(),
            carry: 0.0,
            events: Vec::new(),
            hardest: None,
            broken: Vec::new(),
            elapsed: 0.0,
        }
    }

    /// Where the body's centre of mass is now.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        self.parts.first().map_or(Vec3::ZERO, |p| {
            self.world.bodies()[p.body.0 as usize].pos.as_vec3()
        })
    }

    /// The rover's total linear momentum, kg m/s.
    #[must_use]
    pub fn momentum(&self) -> DVec3 {
        self.parts
            .iter()
            .map(|p| self.world.bodies()[p.body.0 as usize].momentum())
            .sum()
    }

    /// The parts that have broken off, in order.
    #[must_use]
    pub fn broken(&self) -> &[&'static str] {
        &self.broken
    }

    /// The hardest knock: when, s, and the body's speed after it, m/s.
    #[must_use]
    pub fn hardest(&self) -> Option<(f64, f64)> {
        self.hardest
    }

    /// The sparks and dust since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// A meteor's explosion at `at` with blast radius `radius`: a push on
    /// every part in reach, and, when it landed on the rover, its momentum
    /// into the part nearest the landing. Returns the impulses given, each
    /// with the part's name, N s, for the momentum ledger.
    pub fn blast(&mut self, at: Vec3, radius: f32) -> Vec<(&'static str, DVec3)> {
        let at = at.as_dvec3();
        let radius = f64::from(radius).max(0.1);
        let mut given = Vec::new();
        let center_before = self.world.bodies()[self.parts[0].body.0 as usize].vel;
        for part in &self.parts {
            let body = &mut self.world.bodies_mut()[part.body.0 as usize];
            let away = body.pos - at;
            let d = away.length();
            if d >= radius {
                continue;
            }
            // Outward and a little up, as a ground burst throws.
            let dir = (away.normalize_or(DVec3::Y) + DVec3::Y * 0.6).normalize();
            let impulse = dir * BLAST_PRESSURE * part.area * (1.0 - d / radius);
            body.wake();
            body.apply_impulse_at(impulse, body.pos);
            given.push((part.name, impulse));
        }
        // A landing on the rover: the meteor's own momentum, along its
        // flight from the caster's side and down, into the nearest part.
        let horizontal =
            DVec3::new(at.x - self.caster.x, 0.0, at.z - self.caster.z).normalize_or(DVec3::X);
        let flight = (horizontal * 0.55 + DVec3::NEG_Y * 0.83).normalize();
        let near = self
            .parts
            .iter()
            .map(|p| (p, self.world.bodies()[p.body.0 as usize].pos.distance(at)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((part, d)) = near
            && d < STRUCK
        {
            let impulse = flight * METEOR_MASS * METEOR_SPEED;
            let body = &mut self.world.bodies_mut()[part.body.0 as usize];
            let hit = body.pos + (at - body.pos).clamp_length_max(0.2);
            body.wake();
            body.apply_impulse_at(impulse, hit);
            given.push((part.name, impulse));
            self.events.push(Event {
                effect: "scorch_embers",
                at: hit.as_vec3(),
            });
        }
        let after = self.world.bodies()[self.parts[0].body.0 as usize].vel;
        let knock = (after - center_before).length();
        if knock > 0.05 && self.hardest.is_none_or(|(_, k)| knock > k) {
            self.hardest = Some((self.elapsed, knock));
        }
        given
    }

    /// Advances `dt` seconds in fixed steps; breaks the weak joints that
    /// reached their limits and notes hard landings.
    pub fn step(&mut self, dt: f32) {
        self.carry += f64::from(dt.max(0.0));
        let gravity = |_: DVec3, _: DVec3| DVec3::new(0.0, -GRAVITY, 0.0);
        while self.carry >= STEP {
            self.carry -= STEP;
            self.world.step(&gravity);
            self.elapsed += STEP;
            let mut snapped = Vec::new();
            for (k, w) in self.weak.iter().enumerate() {
                if let Some(joint) = self.world.joint(w.id)
                    && joint.saturated
                {
                    snapped.push((k, joint.point));
                }
            }
            for (k, at) in snapped.into_iter().rev() {
                let w = self.weak.remove(k);
                self.world.remove_joint(w.id);
                self.broken.push(w.part);
                self.events.push(Event {
                    effect: "scorch_embers",
                    at: at.as_vec3(),
                });
            }
            let parts: Vec<BodyId> = self.parts.iter().map(|p| p.body).collect();
            let mut dust = Vec::new();
            for c in &self.world.contacts {
                if (parts.contains(&c.body_a) || parts.contains(&c.body_b))
                    && c.impulse.length() > DUST_IMPULSE
                {
                    dust.push(c.point.as_vec3());
                }
            }
            for at in dust.into_iter().take(2) {
                if self
                    .events
                    .iter()
                    .filter(|e| e.effect == "debris_dust")
                    .count()
                    < 6
                {
                    self.events.push(Event {
                        effect: "debris_dust",
                        at,
                    });
                }
            }
        }
    }

    /// The rover's triangles where its parts are now, lit physically.
    #[must_use]
    pub fn lit(&self) -> Vec<LitVertex> {
        let solids = solids();
        let mut out = Vec::new();
        let turn = DQuat::from_rotation_y(self.heading);
        for part in &self.parts {
            let body = &self.world.bodies()[part.body.0 as usize];
            // rover frame -> world at rest: origin + turn * p. The body's
            // motion since: (body.orientation * turn⁻¹) about its centre.
            let spin = (body.orientation * turn.inverse()).as_quat();
            let rest = (self.origin + turn * part.rest).as_vec3();
            let place = |p: Vec3| -> Vec3 {
                let world_rest = self.origin.as_vec3() + turn.as_quat() * p;
                body.pos.as_vec3() + spin * (world_rest - rest)
            };
            let rotate = |n: Vec3| -> Vec3 { spin * (turn.as_quat() * n) };
            for &k in &part.solids {
                let s = &solids[k];
                for tri in s.indices.chunks_exact(3) {
                    let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize);
                    let (pa, pb, pc) = (
                        place(s.positions[a]),
                        place(s.positions[b]),
                        place(s.positions[c]),
                    );
                    let tangent = (pb - pa).normalize_or(Vec3::X);
                    for (p, i) in [(pa, a), (pb, b), (pc, c)] {
                        out.push(LitVertex {
                            pos: p.to_array(),
                            normal: rotate(s.normals[i]).normalize_or(Vec3::Y).to_array(),
                            tangent: tangent.to_array(),
                            local: s.positions[i].to_array(),
                            color: s.color,
                            params: [0.15, 0.55, 0.0, 1.0],
                        });
                    }
                }
            }
        }
        out
    }

    /// Each part's pose: name, position, rotation (for tests and replays).
    #[must_use]
    pub fn poses(&self) -> Vec<(&'static str, DVec3, DQuat)> {
        self.parts
            .iter()
            .map(|p| {
                let b = &self.world.bodies()[p.body.0 as usize];
                (p.name, b.pos, b.orientation)
            })
            .collect()
    }

    /// The rover's total mass, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.parts
            .iter()
            .map(|p| self.world.bodies()[p.body.0 as usize].mass)
            .sum()
    }

    /// Turns gravity's pull and the ground off for a test of momentum: the
    /// rover floats, and only its own joints act between its parts.
    #[cfg(test)]
    fn float(&mut self) {
        for c in 0..self.world.colliders().len() {
            self.world
                .collider_mut(physics::collision::ColliderId(c as u32))
                .filter = Filter { group: 0, mask: 0 };
        }
    }

    #[cfg(test)]
    fn step_free(&mut self, steps: usize) {
        let none = |_: DVec3, _: DVec3| DVec3::ZERO;
        for _ in 0..steps {
            self.world.step(&none);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rover() -> Rover {
        Rover::new(
            Vec3::new(-56.0, 0.0, 8.0),
            0.4,
            Vec3::new(-25.0, 0.0, -13.0),
        )
    }

    #[test]
    fn the_rover_is_its_cad_version_with_its_measured_mass() {
        let r = rover();
        assert_eq!(r.parts.len(), 14);
        // CAD Lab measured 87.94 kg for v38.
        assert!((r.mass() - 87.94).abs() < 0.05, "{}", r.mass());
        assert!(!r.lit().is_empty());
        assert_eq!(solids().len(), 51);
    }

    #[test]
    fn it_stands_on_its_wheels_until_something_hits_it() {
        let mut r = rover();
        let start = r.center();
        for _ in 0..120 {
            r.step(1.0 / 60.0);
        }
        let settled = r.center();
        assert!((settled - start).length() < 0.08, "{start} -> {settled}");
        assert!(r.broken().is_empty());
    }

    #[test]
    fn momentum_is_conserved_when_a_meteor_lands_on_it() {
        let mut r = rover();
        r.float();
        let before = r.momentum();
        let at = r.center() + Vec3::new(0.1, 0.3, 0.0);
        let given = r.blast(at, 5.0);
        let impulses: DVec3 = given.iter().map(|(_, j)| *j).sum();
        let after = r.momentum();
        assert!((after - before - impulses).length() < 1e-9 * impulses.length().max(1.0));
        // The meteor's own momentum is among them.
        assert!(
            given
                .iter()
                .any(|(_, j)| (j.length() - METEOR_MASS * METEOR_SPEED).abs() < 1e-6)
        );
        // Its joints are internal: floating free, the rover keeps it.
        r.step_free(240);
        let later = r.momentum();
        assert!(
            (later - after).length() < 1e-6 * after.length(),
            "{after} -> {later}"
        );
    }

    #[test]
    fn a_close_blast_throws_it_and_tears_parts_off_the_same_way_every_time() {
        let run = || {
            let mut r = rover();
            for _ in 0..30 {
                r.step(1.0 / 60.0);
            }
            let at = r.center() + Vec3::new(0.6, -0.4, 0.3);
            r.blast(at, 6.0);
            for _ in 0..180 {
                r.step(1.0 / 60.0);
            }
            (r.poses(), r.broken().to_vec(), r.hardest())
        };
        let (poses, broken, hardest) = run();
        assert!(!broken.is_empty(), "nothing broke off");
        assert!(hardest.is_some_and(|(_, v)| v > 1.0), "{hardest:?}");
        // Deterministic: the same blast does exactly the same.
        let again = run();
        assert_eq!(poses, again.0);
        assert_eq!(broken, again.1);
    }
}
