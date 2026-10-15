//! CAD solids in the zone's existing demolition physics world.
//! Mass and full inertia integrate the exported triangles; seams use the
//! same class 8.8 bolt capacities as the meteor rover.

use crate::pbr::LitVertex;
pub mod blast;
pub mod sync;
/// Pin and standing-load settings shared with the meteor scene.
pub const PIN_EFFICIENCY: f64 = 0.15;
pub const PIN_LENGTH: f64 = 0.04;
pub const DESIGN_FACTOR: f64 = 4.;

pub fn rover_pin_radius(joint: &str) -> f64 {
    if joint.starts_with("rocker") || joint.starts_with("bogie") {
        0.018
    } else if joint.ends_with("_spin") {
        0.030
    } else if joint.starts_with("mast") {
        0.020
    } else {
        0.006
    }
}

use glam::{DMat3, DQuat, DVec3, Vec3};
use physics::{Body, BodyId, Collider, Joint, JointId, JointKind, JointLimits, Shape, World};
use serde_json::Value;

/// The committed rover's suspension, including its modeled differential.
pub fn rover_joint_settings(name: &str) -> (Option<JointLimits>, Option<physics::Servo>) {
    let level = name.starts_with("rocker") || name.starts_with("bogie");
    let limit = if name.starts_with("rocker") {
        25f64
    } else {
        30f64
    }
    .to_radians();
    (
        level.then_some(JointLimits {
            lower: -limit,
            upper: limit,
        }),
        level.then_some(physics::Servo {
            target: 0.,
            kp: 4000.,
            kd: 300.,
            max_effort: 900.,
            max_speed: 4.,
        }),
    )
}

/// Yield strength of a CAD material, Pa (handbook values: 6061-T6
/// aluminium, Ti-6Al-4V, mild steel, FR-4, and so on).
#[must_use]
pub fn yield_strength(material: &str) -> f64 {
    1e6 * match material {
        "Titanium" => 880.0,
        "Steel" => 250.0,
        "Stainless Steel" => 215.0,
        "Brass" => 200.0,
        "Copper" => 70.0,
        "ABS Plastic" => 40.0,
        "PLA" => 50.0,
        "Nylon" => 70.0,
        "Polycarbonate" => 60.0,
        "Glass" => 50.0,
        "Carbon Fibre" => 600.0,
        "Rubber" => 15.0,
        "Circuit Board" => 300.0,
        "Solar Cell" => 50.0,
        "Gold" => 100.0,
        "Foam" => 1.0,
        "Kapton" => 69.0,
        // Aluminium
        _ => 276.0,
    }
}

/// The eigenvalues and eigenvectors (columns) of a symmetric matrix, by
/// Jacobi rotations; the vectors form a right-handed rotation.
fn principal(m: DMat3) -> (DVec3, DMat3) {
    let mut a = m.to_cols_array_2d();
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut largest) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > largest {
                largest = a[i][j].abs();
                p = i;
                q = j;
            }
        }
        if largest < 1e-15 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
        let t = if theta == 0.0 { 1.0 } else { t };
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        for k in 0..3 {
            let (akp, akq) = (a[k][p], a[k][q]);
            a[k][p] = c * akp - s * akq;
            a[k][q] = s * akp + c * akq;
        }
        for k in 0..3 {
            let (apk, aqk) = (a[p][k], a[q][k]);
            a[p][k] = c * apk - s * aqk;
            a[q][k] = s * apk + c * aqk;
        }
        for row in &mut v {
            let (vp, vq) = (row[p], row[q]);
            row[p] = c * vp - s * vq;
            row[q] = s * vp + c * vq;
        }
    }
    // Columns of v are the eigenvectors (v is stored row-major here).
    let mut basis = DMat3::from_cols(
        DVec3::new(v[0][0], v[1][0], v[2][0]),
        DVec3::new(v[0][1], v[1][1], v[2][1]),
        DVec3::new(v[0][2], v[1][2], v[2][2]),
    );
    if basis.determinant() < 0.0 {
        basis.z_axis = -basis.z_axis;
    }
    (DVec3::new(a[0][0], a[1][1], a[2][2]), basis)
}

/// One solid of the rover, at rest in the rover's frame.
#[derive(Clone, Debug)]
pub struct Solid {
    pub instance: String,
    pub material: String,
    pub mass: f64,
    /// Centre of mass, rover frame, m.
    pub com: DVec3,
    /// Rover frame from the body (principal) frame.
    pub frame: DQuat,
    pub inertia: DVec3,
    /// The box on the principal axes: its centre (body frame) and half
    /// extents, m.
    pub center: DVec3,
    pub half: DVec3,
    /// Rover-frame box, for contacts.
    pub lo: DVec3,
    pub hi: DVec3,
    /// Vertices and normals in the body frame, and the triangles.
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
    pub color: [f32; 3],
}

pub fn instance<'a>(spec: &'a Value, id: &str) -> Option<&'a Value> {
    spec["instances"].as_array()?.iter().find(|i| i["id"] == id)
}

pub fn parse_glb(bytes: &[u8], spec: &Value) -> Option<Vec<Solid>> {
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
        let start =
            bin.checked_add(usize::try_from(view["byteOffset"].as_u64().unwrap_or(0)).ok()?)?;
        let count = usize::try_from(accessor["count"].as_u64()?)
            .ok()?
            .checked_mul(3)?;
        if count > 1_350_000 {
            return None;
        }
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
        let triple3 = |v: Vec<f32>| -> Vec<DVec3> {
            v.chunks_exact(3)
                .map(|c| DVec3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2])))
                .collect()
        };
        let positions = triple3(floats(accessor("POSITION"))?);
        let normals = triple3(floats(accessor("NORMAL"))?);
        let index_accessor = &json["accessors"][primitive["indices"].as_u64()? as usize];
        let view = &json["bufferViews"][index_accessor["bufferView"].as_u64()? as usize];
        let start =
            bin.checked_add(usize::try_from(view["byteOffset"].as_u64().unwrap_or(0)).ok()?)?;
        let count = usize::try_from(index_accessor["count"].as_u64()?).ok()?;
        if count > 450_000 {
            return None;
        }
        let indices: Vec<u32> = bytes
            .get(start..start + count * 4)?
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let material = &json["materials"][primitive["material"].as_u64().unwrap_or(0) as usize];
        let factor = &material["pbrMetallicRoughness"]["baseColorFactor"];
        let color = [0, 1, 2].map(|k| factor[k].as_f64().unwrap_or(0.6) as f32);
        let name = node["extras"]["instance"]
            .as_str()
            .unwrap_or("chassis")
            .to_owned();
        if positions
            .iter()
            .chain(&normals)
            .any(|p| !p.is_finite() || p.abs().max_element() > 5000.)
            || indices.iter().any(|i| *i as usize >= positions.len())
        {
            return None;
        }
        let info = instance(spec, &name);
        let density = info
            .and_then(|i| i["density_g_cm3"].as_f64())
            .unwrap_or(2.7)
            * 1000.0;
        if !density.is_finite() || !(1.0..=30_000.).contains(&density) {
            return None;
        }
        let material = info
            .and_then(|i| i["material"].as_str())
            .unwrap_or("Aluminum")
            .to_owned();
        // Mass properties from the triangles (divergence theorem), as CAD
        // Lab measures them.
        let (mut volume, mut first, mut second) = (0.0, DVec3::ZERO, [0.0f64; 6]);
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| positions[tri[k] as usize]);
            let v = a.dot(b.cross(c)) / 6.0;
            volume += v;
            first += v * (a + b + c) / 4.0;
            let (aa, bb, cc) = (a.to_array(), b.to_array(), c.to_array());
            let sq = |k: usize| {
                aa[k] * aa[k]
                    + bb[k] * bb[k]
                    + cc[k] * cc[k]
                    + aa[k] * bb[k]
                    + aa[k] * cc[k]
                    + bb[k] * cc[k]
            };
            let cross = |i: usize, j: usize| {
                2.0 * (aa[i] * aa[j] + bb[i] * bb[j] + cc[i] * cc[j])
                    + aa[i] * bb[j]
                    + aa[j] * bb[i]
                    + aa[i] * cc[j]
                    + aa[j] * cc[i]
                    + bb[i] * cc[j]
                    + bb[j] * cc[i]
            };
            for k in 0..3 {
                second[k] += v * sq(k) / 10.0;
            }
            second[3] += v * cross(0, 1) / 20.0;
            second[4] += v * cross(1, 2) / 20.0;
            second[5] += v * cross(2, 0) / 20.0;
        }
        if volume <= 1e-12 {
            continue;
        }
        let mass = density * volume;
        let com = first / volume;
        let s = second.map(|x| x * density);
        let (cx, cy, cz) = (com.x, com.y, com.z);
        let ixx = s[1] + s[2] - mass * (cy * cy + cz * cz);
        let iyy = s[0] + s[2] - mass * (cx * cx + cz * cz);
        let izz = s[0] + s[1] - mass * (cx * cx + cy * cy);
        let ixy = -(s[3] - mass * cx * cy);
        let iyz = -(s[4] - mass * cy * cz);
        let izx = -(s[5] - mass * cz * cx);
        let tensor = DMat3::from_cols(
            DVec3::new(ixx, ixy, izx),
            DVec3::new(ixy, iyy, iyz),
            DVec3::new(izx, iyz, izz),
        );
        let (moments, basis) = principal(tensor);
        let frame = DQuat::from_mat3(&basis).normalize();
        let to_body = |p: DVec3| frame.inverse() * (p - com);
        let body: Vec<DVec3> = positions.iter().map(|p| to_body(*p)).collect();
        let (blo, bhi) = body.iter().fold(
            (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let (lo, hi) = positions.iter().fold(
            (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        out.push(Solid {
            instance: name,
            material,
            mass,
            com,
            frame,
            inertia: moments.max(DVec3::splat(mass * 1e-12)),
            center: (blo + bhi) / 2.0,
            half: ((bhi - blo) / 2.0).max(DVec3::splat(0.003)),
            lo,
            hi,
            positions: body.iter().map(|p| p.as_vec3()).collect(),
            normals: normals
                .iter()
                .map(|n| (frame.inverse() * *n).as_vec3())
                .collect(),
            indices,
            color,
        });
    }
    Some(out)
}

/// What a joint can carry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Capacity {
    /// A pin: its shear force and bending torque, N and N m.
    Pin { force: f64, torque: f64 },
    /// A bolted seam: `bolts` bolts of `size`, the seam's normal on body
    /// `a` (body frame), and the pattern's capacities: shear and tension,
    /// N, and bending and twisting, N m.
    Bolted {
        bolts: u32,
        size: &'static str,
        normal: DVec3,
        shear: f64,
        tension: f64,
        bending: f64,
        twist: f64,
    },
}

impl Capacity {
    /// The share of the capacity a force and torque on the joint take
    /// (both in body `a`'s frame), at `scale`: 1 or more breaks it. A pin
    /// fails in shear or bending, whichever comes first. A bolt pattern
    /// takes shear with twist, and tension with bending, combined by the
    /// usual interaction rule: (V/Vc + Mt/Mtc)² + (T/Tc + Mb/Mbc)² = 1.
    #[must_use]
    pub fn used(&self, force: DVec3, torque: DVec3, scale: f64) -> f64 {
        match *self {
            Capacity::Pin {
                force: f,
                torque: t,
            } => (force.length() / f).max(torque.length() / t) / scale,
            Capacity::Bolted {
                normal,
                shear,
                tension,
                bending,
                twist,
                ..
            } => {
                let pull = force.dot(normal).abs();
                let slide = (force - normal * force.dot(normal)).length();
                let turn = torque.dot(normal).abs();
                let bend = (torque - normal * torque.dot(normal)).length();
                let a = slide / shear + turn / twist;
                let b = pull / tension + bend / bending;
                (a * a + b * b).sqrt() / scale
            }
        }
    }
}

/// The bolts of a seam by its length (its patch's longest side), m: how
/// many, and of what size. Class 8.8 steel (800 MPa ultimate).
#[must_use]
pub fn bolts_for(span: f64) -> (u32, &'static str) {
    if span < 0.03 {
        (2, "M3")
    } else if span < 0.08 {
        (2, "M4")
    } else if span < 0.2 {
        (4, "M4")
    } else if span < 0.35 {
        (6, "M5")
    } else {
        (8, "M6")
    }
}

/// One bolt's tensile and shear strength, N: 800 MPa (class 8.8) over its
/// tensile stress area (ISO 898-1: M3 5.03, M4 8.78, M5 14.2, M6 20.1 mm²),
/// shear at 0.6 of tension.
#[must_use]
pub fn bolt_strength(size: &str) -> (f64, f64) {
    let area = match size {
        "M3" => 5.03,
        "M4" => 8.78,
        "M5" => 14.2,
        _ => 20.1,
    } * 1e-6;
    let tension = 800e6 * area;
    (tension, 0.6 * tension)
}

/// A parsed assembly, independent of files and rendering adapters.
#[derive(Clone, Debug)]
pub struct Model {
    pub solids: Vec<Solid>,
    pub spec: Value,
}

impl Model {
    pub fn from_solids(solids: Vec<Solid>, spec: Value) -> Self {
        Self { solids, spec }
    }
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 64 << 20 {
            return Err("This CAD model is too large.".into());
        }
        let n = bytes
            .get(12..16)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or("This file is not a CAD Lab model.")? as usize;
        let json: Value = serde_json::from_slice(
            bytes
                .get(20..20usize.saturating_add(n))
                .ok_or("This CAD model is incomplete.")?,
        )
        .map_err(|_| "This CAD model is incomplete.")?;
        let spec = json["extras"]["physics"].clone();
        if bytes.get(..4) != Some(b"glTF") || bytes.get(4..8) != Some(&2u32.to_le_bytes()) {
            return Err("This file is not a CAD Lab model.".into());
        }
        let solids = parse_glb(bytes, &spec)
            .filter(|s| !s.is_empty())
            .ok_or("This CAD model has no closed solids.")?;
        if solids.len() > 256 || solids.iter().map(|s| s.indices.len() / 3).sum::<usize>() > 150_000
        {
            return Err("This CAD model has too many parts or triangles.".into());
        }
        Ok(Self { solids, spec })
    }
}

#[derive(Clone, Debug)]
struct Seam {
    a: usize,
    b: usize,
    id: JointId,
    capacity: Capacity,
    scale: f64,
    carried: (DVec3, DVec3),
    peak_use: f64,
    broken: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Load {
    body: usize,
    wave: blast::Wave,
    area: f64,
    clearing: f64,
    direction: DVec3,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Burst {
    start: f64,
    until: f64,
    loads: Vec<Load>,
}

/// Read-only measurements for captures and physics checks.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Report {
    pub mass_kg: f64,
    pub positions: Vec<[f64; 3]>,
    pub velocities: Vec<[f64; 3]>,
    pub orientations: Vec<[f64; 4]>,
    pub broken: usize,
    pub joints: usize,
    pub joint_use: f64,
}

/// Bodies registered in the demolition world, with breakable joints.
/// Removing a seam leaves the original solid bodies as loose debris.
#[derive(Clone, Debug)]
pub struct Rig {
    pub bodies: Vec<BodyId>,
    colliders: Vec<physics::ColliderId>,
    seams: Vec<Seam>,
    intimate: std::collections::BTreeSet<(usize, usize)>,
    bursts: Vec<Burst>,
    load_time: f64,
    pub model: std::sync::Arc<Model>,
    scale: f64,
    pub fingerprint: String,
}

fn vector(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap_or(0.),
        v[2].as_f64().unwrap_or(0.),
        -v[1].as_f64().unwrap_or(0.),
    )
}

/// Shared CAD-to-body builder for ordinary placements and the meteor rover.
pub fn insert_solids(
    world: &mut World,
    solids: &[Solid],
    origin: DVec3,
    turn: DQuat,
    scale: f64,
    rounded_wheels: bool,
    filter: physics::Filter,
    material: physics::Material,
) -> (Vec<BodyId>, Vec<physics::ColliderId>) {
    let mut bodies = Vec::new();
    let mut colliders = Vec::new();
    for s in solids {
        let mut body = Body::new(
            s.mass * scale.powi(3),
            s.inertia * scale.powi(5),
            origin + turn * s.com * scale,
        );
        body.orientation = turn * s.frame;
        body.prev_orientation = body.orientation;
        let id = world.add(body);
        bodies.push(id);
        // Support samples in 26 fixed directions keep collision hulls small.
        // Mass and inertia still use every original triangle.
        let mut points = Vec::new();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let d = Vec3::new(x as f32, y as f32, z as f32);
                    if d == Vec3::ZERO {
                        continue;
                    }
                    if let Some(p) = s
                        .positions
                        .iter()
                        .max_by(|a, b| a.dot(d).total_cmp(&b.dot(d)))
                    {
                        let p = p.as_dvec3() * scale;
                        if !points.contains(&p) {
                            points.push(p);
                        }
                    }
                }
            }
        }

        let shape = physics::collision::hull::ConvexHull::new(&points).map(|h| Shape::Hull {
            hull: physics::collision::hull::HullRef::new(h),
        });
        let (mut shape, mut offset) = match shape {
            Ok(s) => (s, DVec3::ZERO),
            Err(_) => (
                Shape::Cuboid {
                    half: s.half * scale,
                },
                s.center * scale,
            ),
        };
        let mut rotation = DQuat::IDENTITY;
        if rounded_wheels {
            shape = Shape::Cuboid {
                half: s.half * scale,
            };
            offset = s.center * scale;
            if s.instance.starts_with("wheel") {
                let axis = if s.inertia.x >= s.inertia.y && s.inertia.x >= s.inertia.z {
                    0
                } else if s.inertia.y >= s.inertia.z {
                    1
                } else {
                    2
                };
                let half = s.half * scale;
                let radius = (0..3)
                    .filter(|k| *k != axis)
                    .map(|k| half[k])
                    .fold(0., f64::max);
                let along = half[axis];
                rotation = match axis {
                    0 => DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
                    1 => DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
                    _ => DQuat::IDENTITY,
                };
                shape = Shape::Capsule {
                    radius: radius * 0.95,
                    half_length: (along - radius * 0.95).max(0.005 * scale).min(along),
                };
                offset = DVec3::ZERO;
            }
        }
        colliders.push(
            world.add_collider(
                Collider::new(id, shape)
                    .at(offset, rotation)
                    .with_filter(filter)
                    .with_material(material),
            ),
        );
    }
    (bodies, colliders)
}

impl Rig {
    pub fn insert(
        world: &mut World,
        model: std::sync::Arc<Model>,
        origin: DVec3,
        heading: f64,
        scale: f64,
        fingerprint: String,
    ) -> Self {
        let turn = DQuat::from_rotation_y(heading);
        let bottom = model
            .solids
            .iter()
            .map(|s| s.lo.y)
            .fold(f64::INFINITY, f64::min);
        let shift = origin - turn * DVec3::Y * bottom * scale;
        let (bodies, colliders) = insert_solids(
            world,
            &model.solids,
            shift,
            turn,
            scale,
            false,
            physics::Filter::ALL,
            physics::Material {
                friction: 0.85,
                torsional: 0.01,
                restitution: 0.1,
            },
        );
        let mut rig = Self {
            bodies,
            colliders,
            seams: Vec::new(),
            intimate: Default::default(),
            bursts: Vec::new(),
            load_time: 0.,
            model,
            scale,
            fingerprint,
        };
        let solid = |name: &str| {
            rig.model
                .solids
                .iter()
                .enumerate()
                .filter(|(_, s)| s.instance == name)
                .max_by(|a, b| a.1.mass.total_cmp(&b.1.mass))
                .map(|(i, _)| i)
                .or_else(|| {
                    rig.model
                        .solids
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| {
                            let mut at = s.instance.as_str();
                            for _ in 0..256 {
                                if at == name {
                                    return true;
                                }
                                let Some(parent) = instance(&rig.model.spec, at)
                                    .and_then(|i| i["parent"].as_str())
                                else {
                                    return false;
                                };
                                at = parent;
                            }
                            false
                        })
                        .max_by(|a, b| a.1.mass.total_cmp(&b.1.mass))
                        .map(|(i, _)| i)
                })
        };
        let explicit: Vec<_> = rig.model.spec["joints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|j| {
                Some((
                    solid(j["parent"].as_str()?)?,
                    solid(j["child"].as_str()?)?,
                    j.clone(),
                ))
            })
            .collect();
        let mut articulated = std::collections::BTreeSet::new();
        let mut moved_names = std::collections::BTreeSet::new();
        for (a, b, j) in &explicit {
            if matches!(
                j["type"].as_str(),
                Some("revolute" | "hinge" | "prismatic" | "slider")
            ) || (j["type"].is_null() && j["origin_mm"].is_array())
            {
                articulated.insert((*a, *b));
                moved_names.insert(j["child"].as_str().unwrap_or_default().to_owned());
                let qa = world[rig.bodies[*a]].orientation;
                let qb = world[rig.bodies[*b]].orientation;
                let at = if j["origin_mm"].is_array() {
                    shift + turn * vector(&j["origin_mm"]) * scale / 1000.
                } else {
                    (world[rig.bodies[*a]].pos + world[rig.bodies[*b]].pos) * 0.5
                };
                let axis = qa.inverse() * turn * vector(&j["axis"]).normalize_or(DVec3::Y);
                let slider = matches!(j["type"].as_str(), Some("prismatic" | "slider"));
                let mut limits = j["limits"]
                    .as_array()
                    .filter(|x| x.len() == 2)
                    .and_then(|v| {
                        Some(JointLimits {
                            lower: v[0].as_f64()?
                                * if slider {
                                    scale / 1000.
                                } else {
                                    std::f64::consts::PI / 180.
                                },
                            upper: v[1].as_f64()?
                                * if slider {
                                    scale / 1000.
                                } else {
                                    std::f64::consts::PI / 180.
                                },
                        })
                    });
                let mut servo: Option<physics::Servo> =
                    serde_json::from_value(j["servo"].clone()).ok();
                let mut relative = qa.inverse() * qb;
                if !slider && let Some(l) = &mut limits {
                    if l.upper - l.lower >= std::f64::consts::TAU - 1e-9 {
                        limits = None;
                    } else {
                        let offset = (l.lower + l.upper) * 0.5;
                        relative = DQuat::from_axis_angle(axis, offset) * relative;
                        if let Some(s) = &mut servo {
                            s.target -= offset;
                        }
                        l.lower -= offset;
                        l.upper -= offset;
                    }
                }
                let kind = if slider {
                    JointKind::Slider {
                        axis,
                        relative,
                        limits,
                        servo,
                    }
                } else {
                    JointKind::Hinge {
                        axis,
                        relative,
                        limits,
                        servo,
                    }
                };
                let radius = j["pin_radius_m"]
                    .as_f64()
                    .filter(|r| r.is_finite() && *r > 0. && *r <= 1.)
                    .map(|r| r * scale)
                    .unwrap_or_else(|| {
                        (rig.model.solids[*a]
                            .half
                            .min_element()
                            .min(rig.model.solids[*b].half.min_element())
                            * scale
                            * 0.25)
                            .clamp(0.001, 0.03)
                    });
                let strength = yield_strength(&rig.model.solids[*a].material)
                    .min(yield_strength(&rig.model.solids[*b].material));
                let force =
                    PIN_EFFICIENCY * 0.6 * strength * std::f64::consts::PI * radius * radius;
                rig.join(
                    world,
                    *a,
                    *b,
                    at,
                    kind,
                    Capacity::Pin {
                        force,
                        torque: force * PIN_LENGTH * scale,
                    },
                );
            }
        }
        // Each articulated child's descendants are a rigid group; fastened
        // parts stay with their parent. Multiple solids of a part share it.
        let group_of = |name: &str| {
            let mut at = name.to_owned();
            for _ in 0..256 {
                if moved_names.contains(&at) {
                    return at;
                }
                let Some(parent) =
                    instance(&rig.model.spec, &at).and_then(|i| i["parent"].as_str())
                else {
                    return String::new();
                };
                at = parent.to_owned();
            }
            at
        };
        let groups: Vec<_> = rig
            .model
            .solids
            .iter()
            .map(|s| group_of(&s.instance))
            .collect();
        let mut seams = std::collections::BTreeSet::new();
        for (a, b, _) in &explicit {
            if !articulated.contains(&(*a, *b)) {
                seams.insert(((*a).min(*b), (*a).max(*b)));
            }
        }
        for b in 1..rig.bodies.len() {
            let Some(a) = (0..b).filter(|a| groups[*a] == groups[b]).min_by(|a, c| {
                rig.model.solids[*a]
                    .com
                    .distance_squared(rig.model.solids[b].com)
                    .total_cmp(
                        &rig.model.solids[*c]
                            .com
                            .distance_squared(rig.model.solids[b].com),
                    )
            }) else {
                continue;
            };
            seams.insert((a, b));
        }
        for b in 1..rig.bodies.len() {
            for a in 0..b {
                let overlap = rig.model.solids[a].hi.min(rig.model.solids[b].hi)
                    - rig.model.solids[a].lo.max(rig.model.solids[b].lo);
                if groups[a] == groups[b] && overlap.min_element() >= -0.002 {
                    seams.insert((a, b));
                }
            }
        }
        for (a, b) in seams {
            let sa = &rig.model.solids[a];
            let sb = &rig.model.solids[b];
            let overlap = (sa.hi.min(sb.hi) - sa.lo.max(sb.lo)) * scale;
            let touching = overlap.min_element() >= -0.002 * scale;
            let span = if touching {
                overlap.max_element().max(0.01)
            } else {
                0.01
            };
            let (bolts, size) = bolts_for(span);
            let (tension, shear) = bolt_strength(size);
            let qa = world[rig.bodies[a]].orientation;
            let normal = (qa.inverse() * turn * (sb.com - sa.com)).normalize_or(DVec3::Y);
            let capacity = Capacity::Bolted {
                bolts,
                size,
                normal,
                shear: shear * f64::from(bolts),
                tension: tension * f64::from(bolts),
                bending: tension * f64::from(bolts) * span / 4.,
                twist: shear * f64::from(bolts) * span / 2.,
            };
            let at = (world[rig.bodies[a]].pos + world[rig.bodies[b]].pos) * 0.5;
            let relative = qa.inverse() * world[rig.bodies[b]].orientation;
            rig.join(world, a, b, at, JointKind::Weld { relative }, capacity);
        }
        for a in 0..rig.bodies.len() {
            for b in a + 1..rig.bodies.len() {
                world.set_pair_collision(rig.bodies[a], rig.bodies[b], false);
            }
        }
        rig.regroup(world);
        rig.calibrate(world, origin);
        rig
    }
    /// Measures the standing load in an isolated copy, without moving the
    /// placed bodies. Every joint carries at least four times that load.
    fn calibrate(&mut self, world: &World, origin: DVec3) {
        let mut probe = World::new(world.dt);
        let ground = probe.add(
            Body::new(1., DVec3::ONE, origin - DVec3::Y * 50.).with_kind(physics::BodyKind::Static),
        );
        probe.add_collider(Collider::new(
            ground,
            Shape::Cuboid {
                half: DVec3::new(300., 50., 300.),
            },
        ));
        let mut ids = std::collections::BTreeMap::new();
        for id in &self.bodies {
            let mut body = world[*id];
            body.prev_pos = body.pos;
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
            body.sleeping = false;
            ids.insert(id.0, probe.add(body));
        }
        for id in &self.colliders {
            let mut c = world.colliders()[id.0 as usize];
            c.body = ids[&c.body.0];
            probe.add_collider(c);
        }
        for a in &self.bodies {
            for b in &self.bodies {
                if a < b && !world.pair_collides(*a, *b) {
                    probe.set_pair_collision(ids[&a.0], ids[&b.0], false);
                }
            }
        }
        let joints: Vec<_> = self
            .seams
            .iter()
            .filter_map(|s| {
                world.joints().find(|(id, _)| *id == s.id).map(|(_, j)| {
                    let mut j = *j;
                    j.a = ids[&j.a.0];
                    j.b = ids[&j.b.0];
                    probe.add_joint(j)
                })
            })
            .collect();
        let mut scales = vec![1f64; self.seams.len()];
        for _ in 0..(2. / probe.dt).ceil() as usize {
            probe.step(&physics::Uniform(DVec3::NEG_Y * 9.81));
            for (i, id) in joints.iter().enumerate() {
                if let Some(j) = probe
                    .joints()
                    .find(|(found, _)| found == id)
                    .map(|(_, j)| *j)
                {
                    let q = probe[j.a].orientation.inverse();
                    scales[i] = scales[i].max(
                        DESIGN_FACTOR
                            * self.seams[i].capacity.used(
                                q * j.impulse / probe.dt,
                                q * j.angular_impulse / probe.dt,
                                1.,
                            ),
                    );
                }
            }
        }
        for (seam, scale) in self.seams.iter_mut().zip(&scales) {
            seam.scale = *scale;
        }
    }

    /// The meteor scene's Kinney–Graham pressure and Friedlander pulse.
    pub fn blast(&mut self, world: &World, at: DVec3, radius: f64, strength: f64, now: f64) {
        if radius <= 0. || strength <= 0. {
            return;
        }
        let charge = blast::charge_for(radius) * strength * strength;
        let mut loads = Vec::new();
        let mut until = now;
        for (k, (id, s)) in self.bodies.iter().zip(&self.model.solids).enumerate() {
            let b = world[*id];
            let center = b.to_world(s.center * self.scale);
            let distance = center.distance(at).max(0.05);
            if distance > radius * 4. {
                continue;
            }
            let direction = (center - at).normalize_or(DVec3::Y);
            let local = (b.orientation.inverse() * direction).abs();
            let half = s.half * self.scale;
            let area = 4.
                * (half.y * half.z * local.x
                    + half.x * half.z * local.y
                    + half.x * half.y * local.z);
            let face = if local.x >= local.y && local.x >= local.z {
                half.y.min(half.z)
            } else if local.y >= local.z {
                half.x.min(half.z)
            } else {
                half.x.min(half.y)
            };
            let shielded =
                self.bodies
                    .iter()
                    .zip(&self.model.solids)
                    .enumerate()
                    .any(|(other, (id, s))| {
                        if other == k {
                            return false;
                        }
                        let b = world[*id];
                        let o = b.orientation.inverse() * (at - b.pos) - s.center * self.scale;
                        let d = b.orientation.inverse() * direction;
                        let half = s.half * self.scale;
                        let (mut near, mut far) = (0f64, distance - 0.05);
                        for axis in 0..3 {
                            if d[axis].abs() < 1e-12 {
                                if o[axis].abs() > half[axis] {
                                    return false;
                                }
                                continue;
                            }
                            let a = (-half[axis] - o[axis]) / d[axis];
                            let c = (half[axis] - o[axis]) / d[axis];
                            near = near.max(a.min(c));
                            far = far.min(a.max(c));
                        }
                        near <= far && near > 0.02
                    });
            let area = area * if shielded { 0.5 } else { 1. };
            let wave = blast::Wave::at(distance, charge);
            until = until.max(now + wave.arrival + wave.duration);
            loads.push(Load {
                body: k,
                wave,
                area,
                clearing: wave.clearing(face),
                direction,
            });
        }
        self.bursts.push(Burst {
            start: now,
            until,
            loads,
        });
    }
    pub fn needs_fine(&self, now: f64) -> bool {
        self.bursts
            .iter()
            .any(|b| now < b.until && now - b.start < 0.02)
    }
    pub fn press(&mut self, world: &mut World, now: f64, h: f64) {
        for burst in &self.bursts {
            for load in &burst.loads {
                let t = now - burst.start - load.wave.arrival;
                let impulse = load.wave.net_impulse(t, t + h, load.clearing, 1.2) * load.area;
                if impulse <= 0. {
                    continue;
                }
                let id = self.bodies[load.body];
                let point = world[id].to_world(self.model.solids[load.body].center * self.scale);
                world.wake(id);
                world[id].apply_impulse_at(load.direction * impulse, point);
            }
        }
        self.bursts.retain(|b| now + h < b.until);
    }

    fn join(
        &mut self,
        world: &mut World,
        a: usize,
        b: usize,
        at: DVec3,
        kind: JointKind,
        capacity: Capacity,
    ) {
        let indices = (a, b);
        let (a, b) = (self.bodies[a], self.bodies[b]);
        let joint = Joint::new(
            a,
            world[a].orientation.inverse() * (at - world[a].pos),
            b,
            world[b].orientation.inverse() * (at - world[b].pos),
            kind,
        );
        if let Ok(id) = world.try_add_joint(joint) {
            self.seams.push(Seam {
                a: indices.0,
                b: indices.1,
                id,
                capacity,
                scale: 1.,
                carried: (DVec3::ZERO, DVec3::ZERO),
                peak_use: 0.,
                broken: false,
            });
        }
    }
    /// Breaks overloaded joints after every solver step, retaining debris.
    pub fn after_step(&mut self, world: &mut World) {
        let mut changed = false;
        self.load_time += world.dt;
        let check = self.load_time >= 1. / 4800. - 1e-12;
        for seam in &mut self.seams {
            if seam.broken {
                continue;
            }
            let Some(j) = world
                .joints()
                .find(|(id, _)| *id == seam.id)
                .map(|(_, j)| *j)
            else {
                seam.broken = true;
                changed = true;
                continue;
            };
            seam.carried.0 += j.impulse;
            seam.carried.1 += j.angular_impulse;
            if !check {
                continue;
            }
            let q = world[j.a].orientation.inverse();
            let used = seam.capacity.used(
                q * seam.carried.0 / self.load_time,
                q * seam.carried.1 / self.load_time,
                seam.scale,
            );
            seam.peak_use = seam.peak_use.max(used);
            seam.carried = (DVec3::ZERO, DVec3::ZERO);
            if used >= 1. {
                world.remove_joint(seam.id);
                seam.broken = true;
                changed = true;
            }
        }
        if check {
            self.load_time = 0.;
        }
        if changed || (check && !self.intimate.is_empty()) {
            self.regroup(world);
        }
    }
    fn regroup(&mut self, world: &mut World) {
        fn root(parents: &mut [usize], mut i: usize) -> usize {
            while parents[i] != i {
                parents[i] = parents[parents[i]];
                i = parents[i];
            }
            i
        }
        let mut parents: Vec<_> = (0..self.bodies.len()).collect();
        for seam in self.seams.iter().filter(|s| !s.broken) {
            let (a, b) = (root(&mut parents, seam.a), root(&mut parents, seam.b));
            parents[b] = a;
        }
        let roots: Vec<_> = (0..self.bodies.len())
            .map(|i| root(&mut parents, i))
            .collect();
        let boxes: Vec<_> = self
            .bodies
            .iter()
            .zip(&self.model.solids)
            .map(|(id, s)| {
                let b = world[*id];
                let center = b.to_world(s.center * self.scale);
                let m = DMat3::from_quat(b.orientation);
                let h = (m.x_axis.abs() * s.half.x
                    + m.y_axis.abs() * s.half.y
                    + m.z_axis.abs() * s.half.z)
                    * self.scale;
                (center - h, center + h)
            })
            .collect();
        for a in 0..self.bodies.len() {
            for b in a + 1..self.bodies.len() {
                let overlap = (boxes[a].1 - boxes[b].0)
                    .min(boxes[b].1 - boxes[a].0)
                    .min_element()
                    > -0.003;
                if roots[a] != roots[b]
                    && overlap
                    && !world.pair_collides(self.bodies[a], self.bodies[b])
                {
                    self.intimate.insert((a, b));
                }
                if !overlap {
                    self.intimate.remove(&(a, b));
                }
                world.set_pair_collision(
                    self.bodies[a],
                    self.bodies[b],
                    roots[a] != roots[b] && !self.intimate.contains(&(a, b)),
                );
            }
        }
    }

    pub fn report(&self, world: &World) -> Report {
        Report {
            mass_kg: self.bodies.iter().map(|id| world[*id].mass).sum(),
            orientations: self
                .bodies
                .iter()
                .map(|id| world[*id].orientation.to_array())
                .collect(),
            positions: self
                .bodies
                .iter()
                .map(|id| world[*id].pos.to_array())
                .collect(),
            velocities: self
                .bodies
                .iter()
                .map(|id| world[*id].vel.to_array())
                .collect(),
            broken: self.broken(),
            joints: self.seams.len(),
            joint_use: self.seams.iter().map(|s| s.peak_use).fold(0., f64::max),
        }
    }
    pub fn broken(&self) -> usize {
        self.seams.iter().filter(|s| s.broken).count()
    }
    pub fn remove(&self, world: &mut World) {
        for id in &self.bodies {
            world.remove_body(*id);
        }
    }
    /// Keeps CAD bodies and joint states when the town rebuilds its world.
    pub fn rehome(&mut self, old: &World, world: &mut World) {
        let shift = world.time() - old.time();
        for burst in &mut self.bursts {
            burst.start += shift;
            burst.until += shift;
        }
        let mut map = std::collections::BTreeMap::new();
        for id in &mut self.bodies {
            let next = world.add(old[*id]);
            map.insert(id.0, next);
            *id = next;
        }
        for id in &mut self.colliders {
            let mut c = old.colliders()[id.0 as usize];
            c.body = map[&c.body.0];
            *id = world.add_collider(c);
        }
        for seam in &mut self.seams {
            if seam.broken {
                continue;
            }
            if let Some(mut j) = old.joints().find(|(id, _)| *id == seam.id).map(|(_, j)| *j) {
                j.a = map[&j.a.0];
                j.b = map[&j.b.0];
                seam.id = world.add_joint(j);
            }
        }
        self.regroup(world);
    }
    /// Contacts at the avatar's 0.45 m blocker radius, with a small margin.
    pub fn avatar_contacts(&self, world: &World, feet: Vec3, direction: DVec3) -> Vec<BodyId> {
        self.colliders
            .iter()
            .filter_map(|id| {
                let c = &world.colliders()[id.0 as usize];
                let body = world[c.body];
                if body.removed
                    || direction.dot(DVec3::new(
                        body.pos.x - f64::from(feet.x),
                        0.,
                        body.pos.z - f64::from(feet.z),
                    )) <= 0.
                {
                    return None;
                }
                let (center, rotation) = c.pose(world);
                let half = c.shape.half_extents();
                let m = DMat3::from_quat(rotation);
                let h = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
                let (lo, hi) = (center - h, center + h);
                if lo.y > f64::from(feet.y) + 1.7 || hi.y < f64::from(feet.y) - 0.1 {
                    return None;
                }
                let point = DVec3::new(
                    f64::from(feet.x).clamp(lo.x, hi.x),
                    0.,
                    f64::from(feet.z).clamp(lo.z, hi.z),
                );
                (point.distance(DVec3::new(f64::from(feet.x), 0., f64::from(feet.z))) < 0.5)
                    .then_some(c.body)
            })
            .collect()
    }

    /// The same height-aware blockers the avatar uses for other solids.
    pub fn blocks(&self, world: &World, feet: Vec3) -> Vec<(crate::controller::Footprint, f32)> {
        self.colliders
            .iter()
            .filter_map(|id| {
                let c = &world.colliders()[id.0 as usize];
                if world[c.body].removed {
                    return None;
                }
                let (center, rotation) = c.pose(world);
                let half = c.shape.half_extents();
                let m = DMat3::from_quat(rotation);
                let h = m.x_axis.abs() * half.x + m.y_axis.abs() * half.y + m.z_axis.abs() * half.z;
                let (lo, hi) = (center - h, center + h);
                if lo.y > f64::from(feet.y) + 1.7 || hi.y < f64::from(feet.y) - 0.1 {
                    return None;
                }
                Some((
                    crate::controller::Footprint {
                        min: [lo.x as f32, lo.z as f32],
                        max: [hi.x as f32, hi.z as f32],
                    },
                    hi.y as f32,
                ))
            })
            .collect()
    }

    pub fn vertices(&self, world: &World) -> Vec<LitVertex> {
        let mut out = Vec::new();
        for (s, id) in self.model.solids.iter().zip(&self.bodies) {
            let b = &world[*id];
            if b.removed {
                continue;
            }
            for tri in s.indices.chunks_exact(3) {
                let tangent = (s.positions[tri[1] as usize] - s.positions[tri[0] as usize])
                    .normalize_or(Vec3::X);
                for i in tri {
                    let i = *i as usize;
                    let p = s.positions[i];
                    let n = s.normals.get(i).copied().unwrap_or(Vec3::Y);
                    out.push(LitVertex {
                        pos: b.to_world(p.as_dvec3() * self.scale).as_vec3().to_array(),
                        normal: (b.orientation * n.as_dvec3()).as_vec3().to_array(),
                        tangent: (b.orientation * tangent.as_dvec3()).as_vec3().to_array(),
                        local: p.to_array(),
                        color: s.color,
                        params: [0.15, 0.55, 0., 1.],
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cube(at: DVec3) -> Solid {
        let half = DVec3::splat(0.1);
        let hull = physics::collision::hull::ConvexHull::cuboid(half).unwrap();
        let mut indices = Vec::new();
        for f in hull.faces() {
            for k in 1..f.vertices.len() - 1 {
                indices.extend([f.vertices[0], f.vertices[k], f.vertices[k + 1]]);
            }
        }
        Solid {
            instance: format!("part{}", at.x),
            material: "Aluminum".into(),
            mass: 2.7 * 8.,
            com: at,
            frame: DQuat::IDENTITY,
            inertia: Body::box_inertia(21.6, half * 2.),
            center: DVec3::ZERO,
            half,
            lo: at - half,
            hi: at + half,
            positions: hull.vertices().iter().map(|p| p.as_vec3()).collect(),
            normals: vec![Vec3::Y; hull.vertices().len()],
            indices,
            color: [0.7, 0.3, 0.1],
        }
    }
    fn model() -> std::sync::Arc<Model> {
        std::sync::Arc::new(Model {
            solids: vec![cube(DVec3::ZERO), cube(DVec3::X * 0.2)],
            spec: Value::Null,
        })
    }
    fn floor(world: &mut World) {
        let id = world.add(
            Body::new(1., DVec3::ONE, DVec3::NEG_Y * 0.5).with_kind(physics::BodyKind::Static),
        );
        world.add_collider(Collider::new(
            id,
            Shape::Cuboid {
                half: DVec3::new(80., 0.5, 80.),
            },
        ));
    }
    #[test]
    fn triangle_mass_matches_checked_rover_and_retains_full_tensor() {
        let bytes = include_bytes!("../../../../../assets/verse/cad/rover.glb");
        let spec: Value =
            serde_json::from_str(include_str!("../../../../../assets/verse/cad/rover.json"))
                .unwrap();
        let solids = parse_glb(bytes, &spec).unwrap();
        let expected: f64 = spec["instances"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v["mass_kg"].as_f64())
            .sum();
        let mass: f64 = solids.iter().map(|s| s.mass).sum();
        assert!(
            (mass - expected).abs() < expected * 0.002,
            "{mass} vs {expected}"
        );
        let tensor = DMat3::from_cols(
            DVec3::new(3., 0.5, 0.2),
            DVec3::new(0.5, 4., 0.3),
            DVec3::new(0.2, 0.3, 5.),
        );
        let (d, q) = principal(tensor);
        let rebuilt = q * DMat3::from_diagonal(d) * q.transpose();
        assert!(
            (rebuilt - tensor)
                .to_cols_array()
                .iter()
                .all(|x| x.abs() < 1e-10)
        );
    }
    #[test]
    fn hinge_limits_keep_the_exported_pose_even_across_a_half_turn() {
        let mut model = Model::from_solids(
            vec![cube(DVec3::ZERO), cube(DVec3::X * 0.2)],
            serde_json::json!({
            "instances":[{"id":"part0"},{"id":"part0.2","parent":"part0"}],
            "joints":[{"id":"hinge","parent":"part0","child":"part0.2","type":"revolute","axis":[0,0,1],"origin_mm":[100,0,0],"limits":[-270,45]}]}),
        );
        model.solids[1].instance = "part0.2".into();
        let mut world = World::new(1. / 120.);
        floor(&mut world);
        let rig = Rig::insert(
            &mut world,
            std::sync::Arc::new(model),
            DVec3::ZERO,
            0.,
            1.,
            "hinged".into(),
        );
        let joint = world
            .joints()
            .find(|(_, j)| matches!(j.kind, JointKind::Hinge { .. }))
            .unwrap()
            .1;
        let angle = joint.coordinate(&world).unwrap();
        let JointKind::Hinge {
            limits: Some(limits),
            ..
        } = joint.kind
        else {
            panic!("missing hinge limits");
        };
        assert!(angle >= limits.lower && angle <= limits.upper);
        assert!((angle - 112.5f64.to_radians()).abs() < 1e-8);
        assert_eq!(rig.seams.len(), 1);
    }

    #[test]
    fn joints_between_subassemblies_keep_their_descendants_together() {
        let mut solids = vec![
            cube(DVec3::ZERO),
            cube(DVec3::X * 0.2),
            cube(DVec3::X),
            cube(DVec3::X * 1.2),
        ];
        for (s, id) in solids
            .iter_mut()
            .zip(["base.block", "base.cap", "arm.lower", "arm.upper"])
        {
            s.instance = id.into();
        }
        let model = Model::from_solids(
            solids,
            serde_json::json!({"instances":[{"id":"base"},{"id":"arm"},{"id":"base.block","parent":"base"},{"id":"base.cap","parent":"base"},{"id":"arm.lower","parent":"arm"},{"id":"arm.upper","parent":"arm"}],"joints":[{"id":"pivot","type":"revolute","parent":"base","child":"arm","origin_mm":[500,0,100],"axis":[0,0,1],"limits":[-30,30]}]}),
        );
        let mut world = World::new(1. / 120.);
        floor(&mut world);
        let rig = Rig::insert(
            &mut world,
            std::sync::Arc::new(model),
            DVec3::ZERO,
            0.,
            1.,
            "arm".into(),
        );
        assert_eq!(
            world
                .joints()
                .filter(|(_, j)| matches!(j.kind, JointKind::Hinge { .. }))
                .count(),
            1
        );
        assert_eq!(rig.seams.len(), 3);
    }

    #[test]
    fn checked_rover_stands_before_a_spell_or_blast() {
        let bytes = include_bytes!("../../../../../assets/verse/cad/rover.glb");
        let spec: Value =
            serde_json::from_str(include_str!("../../../../../assets/verse/cad/rover.json"))
                .unwrap();
        let model = std::sync::Arc::new(Model {
            solids: parse_glb(bytes, &spec).unwrap(),
            spec,
        });
        for origin in [
            DVec3::ZERO,
            DVec3::new(-2.5, 0., -28.),
            DVec3::new(2.5, 0., -28.),
        ] {
            let mut w = World::new(1. / 120.);
            floor(&mut w);
            let mut rig = Rig::insert(&mut w, model.clone(), origin, 0., 1., "rover".into());
            for _ in 0..240 {
                w.step(&physics::Uniform(DVec3::NEG_Y * 9.81));
                rig.after_step(&mut w);
            }
            assert_eq!(
                rig.broken(),
                0,
                "rover joints failed under gravity: {:?}",
                rig.seams
                    .iter()
                    .filter(|s| s.broken)
                    .map(|s| (
                        &rig.model.solids[s.a].instance,
                        &rig.model.solids[s.b].instance,
                        s.scale,
                        s.peak_use
                    ))
                    .collect::<Vec<_>>()
            );
            assert!(w[rig.bodies[0]].pos.y > -0.01);
        }
    }

    #[test]
    fn a_placed_body_rests_on_the_zones_raised_terrain() {
        use super::super::demolition::site::Site;
        let mut site = Site::new(Vec::new(), 42);
        site.set_cad_terrain(|_, _| 2.);
        site.put_cad("hill", "v1", model(), DVec3::new(100., 4., 100.), 0., 1.);
        for _ in 0..240 {
            site.tick(1. / 120.);
        }
        let p = site.cad_report("hill").unwrap().positions[0];
        assert!((p[1] - 2.1).abs() < 0.04, "{p:?}");
    }

    #[test]
    fn falls_rests_rises_and_runs_deterministically() {
        let run = || {
            let mut w = World::new(1. / 240.);
            floor(&mut w);
            let mut r = Rig::insert(&mut w, model(), DVec3::Y * 2., 0.3, 1., "one".into());
            let start = w[r.bodies[0]].pos.y;
            for _ in 0..960 {
                w.step(&physics::Uniform(DVec3::NEG_Y * 9.81));
                r.after_step(&mut w);
            }
            assert!(w[r.bodies[0]].pos.y < start - 1.);
            assert!(
                (w[r.bodies[0]].pos.y - 0.1).abs() < 0.04,
                "{:?}",
                w[r.bodies[0]].pos
            );
            assert_eq!(r.broken(), 0);
            let rest = w[r.bodies[0]].pos.y;
            let gravity = verse_world::reverse_gravity::Gravity {
                cylinder: verse_world::reverse_gravity::Cylinder::at(DVec3::ZERO),
                active: true,
            };
            for _ in 0..120 {
                for id in &r.bodies {
                    w.wake(*id);
                }
                w.step(&|p, v| gravity.at(p, v));
                r.after_step(&mut w);
            }
            assert!(w[r.bodies[0]].pos.y > rest + 0.5);
            r.bodies.iter().map(|id| w[*id].pos).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
    #[test]
    fn weak_impulse_keeps_bolts_strong_impulse_breaks_and_debris_stays() {
        let mut w = World::new(1. / 240.);
        let mut r = Rig::insert(&mut w, model(), DVec3::Y * 2., 0., 1., "one".into());
        w[r.bodies[1]].vel = DVec3::Y * 0.001;
        w.step(&physics::NoField);
        r.after_step(&mut w);
        assert_eq!(r.broken(), 0);
        w[r.bodies[1]].vel = DVec3::Y * 100.;
        w.step(&physics::NoField);
        r.after_step(&mut w);
        assert_eq!(r.broken(), 1);
        assert!(r.bodies.iter().all(|id| !w[*id].removed));
    }
    #[test]
    fn pushing_at_the_avatars_blocker_edge_moves_a_placed_assembly() {
        use super::super::demolition::site::Site;
        let mut site = Site::new(Vec::new(), 42);
        site.put_cad("cart", "v1", model(), DVec3::ZERO, 0., 1.);
        for _ in 0..240 {
            site.tick(1. / 120.);
        }
        let initial = site.cad_report("cart").unwrap().positions[0][0];
        for _ in 0..60 {
            let x = site.cad_report("cart").unwrap().positions[0][0] as f32;
            site.cad_forces(
                None,
                Vec::new(),
                Vec3::new(x - 0.55, 0., 0.),
                Vec3::X,
                1. / 120.,
            );
            site.tick(1. / 120.);
        }
        assert!(site.cad_report("cart").unwrap().positions[0][0] > initial + 0.05);
    }

    #[test]
    fn site_blasts_and_replacements_use_the_existing_world() {
        use super::super::demolition::site::Site;
        let mut site = Site::new(Vec::new(), 42);
        site.put_cad("a", "v1", model(), DVec3::ZERO, 0., 1.);
        site.put_cad("b", "v1", model(), DVec3::X * 3., 0., 1.);
        for _ in 0..240 {
            site.tick(1. / 120.);
        }
        assert_eq!(site.cad_report("a").unwrap().broken, 0);
        site.explode(Vec3::new(-0.4, 0.4, 0.), 5.2, 1, 0.01);
        site.tick(1. / 120.);
        assert_eq!(site.cad_report("a").unwrap().broken, 0);
        site.explode(Vec3::new(-0.4, 0.4, 0.), 5.2, 1000, 17.);
        site.tick(1. / 120.);
        assert!(
            site.cad_report("a").unwrap().broken > 0,
            "{:?}",
            site.cad_report("a")
        );
        let b = site.cad_report("b").unwrap().positions;
        site.put_cad("a", "v2", model(), DVec3::X * -3., 0., 1.);
        assert_eq!(site.cad_report("a").unwrap().broken, 0);
        assert_eq!(site.cad_report("b").unwrap().positions, b);
        site.retain_cad(&["b".to_owned()].into());
        assert!(site.cad_report("a").is_none());
        site.reset();
        assert_eq!(site.cad_report("b").unwrap().positions, b);
    }

    #[test]
    fn two_copies_and_world_rebuild_keep_independent_bodies() {
        let mut w = World::new(1. / 240.);
        let mut a = Rig::insert(&mut w, model(), DVec3::X * -2., 0., 1., "a".into());
        let b = Rig::insert(&mut w, model(), DVec3::X * 2., 0., 1., "b".into());
        assert!(a.bodies.iter().all(|id| !b.bodies.contains(id)));
        let before = a.bodies.iter().map(|id| w[*id].pos).collect::<Vec<_>>();
        w.tick = 240;
        a.blast(&w, DVec3::new(-3., 0.3, 0.), 5.2, 1., w.time());
        let mut next = World::new(w.dt);
        floor(&mut next);
        a.rehome(&w, &mut next);
        assert!(
            a.bursts[0].start.abs() < 1e-10,
            "burst did not follow the rebuilt world's clock"
        );
        assert_eq!(
            before,
            a.bodies.iter().map(|id| next[*id].pos).collect::<Vec<_>>()
        );
        a.remove(&mut next);
        assert!(a.bodies.iter().all(|id| next[*id].removed));
    }
}
