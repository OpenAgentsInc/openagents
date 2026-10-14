//! The CAD Lab rover in the Meteor Showcase: the scientific rover built by
//! talking in CAD Lab (#11627, `docs/cad-lab/mvp.md`), standing on the lawn
//! in front of the houses as the swarm comes down, simulated rather than
//! animated (`meteor_rover_capture`, #11644).
//!
//! # Bodies
//!
//! Every solid of the rover's CAD version is a rigid body of its own: 51
//! of them, from its glTF (`assets/verse/cad/rover.glb`, one node per
//! solid, named by its instance). A solid's mass, centre of mass and full
//! inertia tensor come from its own triangles and its instance's material
//! density (`assets/verse/cad/rover.json`, from the version's check
//! report), the way CAD Lab measures them; the tensor is diagonalised, so
//! the body's frame is the solid's principal axes and nothing (products
//! of inertia included) is dropped. Their masses sum to the CAD mass.
//!
//! # Joints
//!
//! Every connection of the assembly is a joint:
//!
//! - **Articulated:** the CAD joints stay hinges at their CAD origin and
//!   axis: the rockers' and bogies' pivots (held level by a soft servo,
//!   the differential the CAD model leaves out), the six wheels' axles, the
//!   camera head's pan and the avionics lid's hinge. Each is a pin, and
//!   breaks as one: its force limit is the pin's shear strength,
//!   [`PIN_EFFICIENCY`] × 0.6 σᵧ × πr² (the pin's radius r from the CAD
//!   part: the 36 mm shafts, the 60 mm hubs, the 40 mm mast, a 12 mm lid
//!   hinge pin), and its torque limit that force × the pin's
//!   [`PIN_LENGTH`].
//! - **Bolted:** every pair of solids that touch within one rigid group
//!   (the solids not separated by an articulated joint, from the
//!   assembly's parents) is a bolted seam: a fixed joint held by bolts.
//!   The seam's length (its contact patch's longest side, from the overlap
//!   of the two solids' boxes) sets the bolts ([`bolts_for`]):
//!
//!   | seam length | bolts |
//!   | --- | --- |
//!   | under 30 mm | 2 × M3 |
//!   | 30–80 mm | 2 × M4 |
//!   | 80–200 mm | 4 × M4 |
//!   | 200–350 mm | 6 × M5 |
//!   | 350 mm and over | 8 × M6 |
//!
//!   Each bolt is class 8.8 steel ([`bolt_strength`]): 800 MPa over its
//!   tensile stress area (M3 5.03, M4 8.78, M5 14.2, M6 20.1 mm²), so an M5
//!   holds 11.4 kN in tension and 6.8 kN in shear (0.6 of tension). The
//!   pattern of N bolts over a seam of length s holds N × that in shear
//!   and tension, N × Ft × s / 4 in bending (half the bolts in tension at
//!   half the pattern's length) and N × Fv × s / 2 in twist. Shear with
//!   twist and tension with bending combine by the interaction rule
//!   (V/Vc + Mt/Mtc)² + (T/Tc + Mb/Mbc)² = 1 ([`Capacity::used`]). A solid
//!   that touches nothing in its group is joined to the nearest solid of
//!   the group by two M3s.
//!
//! Every joint is also built to carry [`DESIGN_FACTOR`] times what it
//! carries standing still under gravity (measured in a settling run when
//! the rover is made), as a structure is designed with a margin over its
//! static load: its capacities are scaled up where they fall short of it.
//!
//! A joint breaks in the step its load reaches its capacity (sparks at its
//! anchor). The solver doesn't cap the joints, so the loads recorded are
//! what they carried: in a blow, on a hard landing, or as a piece tumbles.
//! Each joint's peak load and the largest share of its capacity it used
//! are kept for the report ([`Rover::joints`]).
//!
//! # What hits it
//!
//! A meteor's blast pushes every solid away from its centre with
//! [`BLAST_PRESSURE`] × the area the solid shows the blast (its box's
//! cross-section facing the centre), falling off to nothing at the blast's
//! edge; shading by other parts is not modelled. A meteor that lands on
//! the rover bursts on the first solid its flight meets, which takes its
//! whole momentum ([`METEOR_MASS`] × [`METEOR_SPEED`] along its flight) as
//! one sharp impulse in a step. The joints round that solid take the shock
//! first, and what they pass on loads the next ones.
//!
//! # Collisions
//!
//! Solids collide with the ground, with the houses' debris near them
//! (mirrored in as moving boxes each step: they push the rover's parts,
//! the parts don't push them back), and with each other once they are in
//! different pieces: each piece still held together by its joints is a
//! collision group of its own, recomputed whenever a joint breaks, so
//! loose parts bounce off the body and pile up on each other. Colliders
//! are each solid's box on its principal axes, and a capsule along the
//! axle for a wheel. A part flying faster than 40 m/s meets only the
//! ground until it slows: it would cross another part within a step.
//!
//! Native only: the glTF is built into the binary, which the browser build
//! leaves out.

use glam::{DMat3, DQuat, DVec3, Vec3};
use physics::body::{Body, BodyKind};
use physics::collision::{Collider, ColliderId, Filter, Material, Shape};
use physics::joint::{Joint, JointId, JointKind, JointLimits, Servo};
use physics::world::{BodyId, World};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::pbr::LitVertex;

/// The rover's glTF and assembly data, exported from CAD Lab.
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
/// The blast's push per square metre a solid shows it at the centre, N s/m²
/// (falling off linearly to nothing at the blast's edge).
pub const BLAST_PRESSURE: f64 = 350.0;
/// The share of a pin's shear strength it holds under a sudden blow
/// (notches, bushings, the eye around it).
pub const PIN_EFFICIENCY: f64 = 0.15;
/// A pin's length, for the bending torque that breaks it, m.
pub const PIN_LENGTH: f64 = 0.04;
/// How many times its load standing still every joint is built to carry.
pub const DESIGN_FACTOR: f64 = 4.0;
/// How long the rover settles under gravity when it is made, s.
const SETTLE: f64 = 0.5;
/// The blast's gas speed at its centre, m/s: no part is pushed faster than
/// the gas that pushes it (falling off with the push).
pub const BLAST_SPEED: f64 = 25.0;
/// A contact this hard (impulse in one step, N s) raises dust.
const DUST_IMPULSE: f64 = 4.0;
/// The least time between two puffs of dust, s.
const DUST_EVERY: f64 = 0.4;
/// Debris boxes mirrored into the rover's world, at most.
const DEBRIS: usize = 40;
/// Two solids' boxes this close count as touching, m.
const TOUCH: f64 = 0.002;

/// Collision bits: the ground, the houses' debris, then one per piece.
const GROUND_BIT: u32 = 1;
const DEBRIS_BIT: u32 = 2;
const PIECE_BITS: u32 = 29;
/// A part flying faster than this (m/s) crosses another in under a step
/// and only meets the ground: colliding at that speed with the step's
/// look-ahead throws both apart unphysically.
const FAST: f64 = 40.0;
const FAST_BIT: u32 = 1 << 31;

fn find(p: &mut [usize], mut i: usize) -> usize {
    while p[i] != i {
        p[i] = p[p[i]];
        i = p[i];
    }
    i
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

/// A pin's radius for a CAD joint, m, by what the CAD part is.
fn pin_radius(joint: &str) -> f64 {
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

/// From the CAD frame (mm, X forward, Y left, Z up) to the rover's frame
/// in Verse (m, X forward, Y up, Z right): the glTF's own frame.
fn from_cad(p: [f64; 3]) -> DVec3 {
    DVec3::new(p[0], p[2], -p[1]) / 1000.0
}

fn triple(v: &Value) -> [f64; 3] {
    [0, 1, 2].map(|k| v[k].as_f64().unwrap_or(0.0))
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
struct Solid {
    instance: String,
    material: String,
    mass: f64,
    /// Centre of mass, rover frame, m.
    com: DVec3,
    /// Rover frame from the body (principal) frame.
    frame: DQuat,
    inertia: DVec3,
    /// The box on the principal axes: its centre (body frame) and half
    /// extents, m.
    center: DVec3,
    half: DVec3,
    /// Rover-frame box, for contacts.
    lo: DVec3,
    hi: DVec3,
    /// Vertices and normals in the body frame, and the triangles.
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<u32>,
    color: [f32; 3],
}

/// The whole model: solids, and the assembly's instances and joints.
struct Model {
    solids: Vec<Solid>,
    spec: Value,
}

fn model() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| {
        let spec: Value = serde_json::from_str(SPEC).unwrap_or(Value::Null);
        let solids = parse_glb(GLB, &spec).unwrap_or_default();
        Model { solids, spec }
    })
}

fn instance<'a>(spec: &'a Value, id: &str) -> Option<&'a Value> {
    spec["instances"].as_array()?.iter().find(|i| i["id"] == id)
}

fn parse_glb(bytes: &[u8], spec: &Value) -> Option<Vec<Solid>> {
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
        let triple3 = |v: Vec<f32>| -> Vec<DVec3> {
            v.chunks_exact(3)
                .map(|c| DVec3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2])))
                .collect()
        };
        let positions = triple3(floats(accessor("POSITION"))?);
        let normals = triple3(floats(accessor("NORMAL"))?);
        let index_accessor = &json["accessors"][primitive["indices"].as_u64()? as usize];
        let view = &json["bufferViews"][index_accessor["bufferView"].as_u64()? as usize];
        let start = bin + view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let count = index_accessor["count"].as_u64()? as usize;
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
        let info = instance(spec, &name);
        let density = info
            .and_then(|i| i["density_g_cm3"].as_f64())
            .unwrap_or(2.7)
            * 1000.0;
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
            inertia: moments.max(DVec3::splat(1e-7)),
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

/// The rigid group of an instance: the nearest of it and its ancestors
/// that a CAD joint moves, else the body.
fn group_of(spec: &Value, id: &str) -> String {
    let moved: Vec<&str> = spec["joints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|j| j["child"].as_str())
        .collect();
    let mut at = Some(id.to_owned());
    while let Some(i) = at {
        if moved.contains(&i.as_str()) {
            return i;
        }
        at = instance(spec, &i)
            .and_then(|v| v["parent"].as_str())
            .map(str::to_owned);
    }
    "chassis".into()
}

/// What a joint is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A pin of the CAD assembly (pivot, axle, pan, hinge).
    Pin,
    /// A bolted or bonded seam between two touching solids.
    Fastened,
}

/// A joint of the rover and how it has fared.
#[derive(Clone, Debug)]
pub struct Link {
    pub name: String,
    pub kind: Kind,
    /// The two solids, by instance.
    pub a: String,
    pub b: String,
    /// What it can carry.
    pub capacity: Capacity,
    /// Its capacities times this: at least [`DESIGN_FACTOR`] over its
    /// standing load.
    pub scale: f64,
    /// The largest force and torque it carried in a step, N and N m, and
    /// the largest share of its capacity (1 breaks it).
    pub peak_force: f64,
    pub peak_torque: f64,
    pub peak_use: f64,
    /// When it broke, s, if it did.
    pub broke: Option<f64>,
    id: JointId,
    bodies: (usize, usize),
    /// Its anchor on body `a`, body frame.
    anchor: DVec3,
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

/// Something worth an effect: sparks where a joint broke (`rover_sparks`),
/// a puff of dust where a part landed hard (`rover_landing`); both fire
/// once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub effect: &'static str,
    pub at: Vec3,
}

/// The rover on the lot: its world, its bodies (one per solid), its joints,
/// and what happened to it.
pub struct Rover {
    world: World,
    bodies: Vec<BodyId>,
    /// Each body's colliders.
    colliders: Vec<Vec<ColliderId>>,
    /// Each body's exposed box, for the blast (body frame half extents).
    links: Vec<Link>,
    debris: Vec<BodyId>,
    /// Pairs of solids that broke apart and still overlap.
    intimate: std::collections::BTreeSet<(usize, usize)>,
    /// Which parts are flying faster than [`FAST`].
    fast: Vec<bool>,
    last_dust: f64,
    ground: ColliderId,
    origin: DVec3,
    caster: DVec3,
    carry: f64,
    events: Vec<Event>,
    hardest: Option<(f64, f64)>,
    elapsed: f64,
}

impl Rover {
    /// The rover standing at `origin` (on the ground) facing `heading`
    /// (radians about Y; 0 faces +X), with meteors flying in from `caster`.
    #[must_use]
    pub fn new(origin: Vec3, heading: f32, caster: Vec3) -> Self {
        let origin = origin.as_dvec3();
        let heading = f64::from(heading);
        let turn = DQuat::from_rotation_y(heading);
        let model = model();
        let spec = &model.spec;
        let mut world = World::new(STEP);
        let ground_body =
            world.add(Body::new(1.0, DVec3::ONE, origin - DVec3::Y).with_kind(BodyKind::Static));
        let ground = world.add_collider(
            Collider::new(
                ground_body,
                Shape::Cuboid {
                    half: DVec3::new(300.0, 1.0, 300.0),
                },
            )
            .with_filter(Filter {
                group: GROUND_BIT,
                mask: !GROUND_BIT,
            })
            .with_material(Material {
                friction: 0.8,
                torsional: 0.02,
                restitution: 0.1,
            }),
        );
        let mut bodies = Vec::new();
        let mut colliders = Vec::new();
        for s in &model.solids {
            let mut body = Body::new(s.mass, s.inertia, origin + turn * s.com);
            body.orientation = (turn * s.frame).normalize();
            let id = world.add(body);
            bodies.push(id);
            let wheel = s.instance.starts_with("wheel");
            let mut mine = Vec::new();
            let shape = if wheel {
                // A capsule along the axle: the principal axis with the
                // largest moment.
                let axis = if s.inertia.x >= s.inertia.y && s.inertia.x >= s.inertia.z {
                    0
                } else if s.inertia.y >= s.inertia.z {
                    1
                } else {
                    2
                };
                let radius = [s.half.x, s.half.y, s.half.z]
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| *k != axis)
                    .map(|(_, h)| *h)
                    .fold(0.0, f64::max);
                let along = [s.half.x, s.half.y, s.half.z][axis];
                let rotation = match axis {
                    0 => DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
                    1 => DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
                    _ => DQuat::IDENTITY,
                };
                (
                    Shape::Capsule {
                        radius: radius * 0.95,
                        half_length: (along - radius * 0.95).max(0.005).min(along),
                    },
                    rotation,
                )
            } else {
                (Shape::Cuboid { half: s.half }, DQuat::IDENTITY)
            };
            mine.push(
                world.add_collider(
                    Collider::new(id, shape.0)
                        .at(if wheel { DVec3::ZERO } else { s.center }, shape.1)
                        .with_filter(Filter {
                            group: 1 << 2,
                            mask: !(1 << 2),
                        })
                        .with_material(Material {
                            friction: 0.85,
                            torsional: 0.01,
                            restitution: 0.2,
                        }),
                ),
            );
            colliders.push(mine);
        }
        let rest = |k: usize| {
            (
                origin + turn * model.solids[k].com,
                (turn * model.solids[k].frame).normalize(),
            )
        };
        let mut links: Vec<Link> = Vec::new();
        let mut join = |world: &mut World,
                        a: usize,
                        b: usize,
                        at: DVec3,
                        kind: JointKind,
                        link: Kind,
                        name: String,
                        capacity: Capacity| {
            let (pa, qa) = rest(a);
            let (pb, qb) = rest(b);
            let anchor_a = qa.inverse() * (at - pa);
            let anchor_b = qb.inverse() * (at - pb);
            let joint = Joint::new(bodies[a], anchor_a, bodies[b], anchor_b, kind);
            if let Ok(id) = world.try_add_joint(joint) {
                links.push(Link {
                    name,
                    kind: link,
                    a: model.solids[a].instance.clone(),
                    b: model.solids[b].instance.clone(),
                    capacity,
                    scale: 1.0,
                    peak_force: 0.0,
                    peak_torque: 0.0,
                    peak_use: 0.0,
                    broke: None,
                    id,
                    bodies: (a, b),
                    anchor: anchor_a,
                });
            }
        };
        let main_of = |id: &str| -> Option<usize> {
            model
                .solids
                .iter()
                .enumerate()
                .filter(|(_, s)| s.instance == id)
                .max_by(|x, y| x.1.mass.total_cmp(&y.1.mass))
                .map(|(k, _)| k)
        };
        let weaker = |a: usize, b: usize| {
            yield_strength(&model.solids[a].material).min(yield_strength(&model.solids[b].material))
        };
        // The CAD joints: pins.
        for j in spec["joints"].as_array().into_iter().flatten() {
            let (Some(parent), Some(child), Some(name)) =
                (j["parent"].as_str(), j["child"].as_str(), j["id"].as_str())
            else {
                continue;
            };
            let (Some(a), Some(b)) = (main_of(parent), main_of(child)) else {
                continue;
            };
            let at = origin + turn * from_cad(triple(&j["origin_mm"]));
            let axis_world = turn * from_cad(triple(&j["axis"])).normalize_or(DVec3::Z) * 1000.0;
            let axis_world = axis_world.normalize();
            let (_, qa) = rest(a);
            let (_, qb) = rest(b);
            let relative = qa.inverse() * qb;
            let r = pin_radius(name);
            let force = PIN_EFFICIENCY * 0.6 * weaker(a, b) * std::f64::consts::PI * r * r;
            let torque = force * PIN_LENGTH;
            let level = name.starts_with("rocker") || name.starts_with("bogie");
            let limit = if name.starts_with("rocker") {
                25f64
            } else {
                30.0
            }
            .to_radians();
            let kind = JointKind::Hinge {
                axis: qa.inverse() * axis_world,
                relative,
                limits: level.then_some(JointLimits {
                    lower: -limit,
                    upper: limit,
                }),
                servo: level.then_some(Servo {
                    target: 0.0,
                    kp: 4_000.0,
                    kd: 300.0,
                    max_effort: 900.0,
                    max_speed: 4.0,
                }),
            };
            join(
                &mut world,
                a,
                b,
                at,
                kind,
                Kind::Pin,
                name.to_owned(),
                Capacity::Pin { force, torque },
            );
        }
        // Fastened seams: touching solids in one rigid group.
        let groups: Vec<String> = model
            .solids
            .iter()
            .map(|s| group_of(spec, &s.instance))
            .collect();
        let n = model.solids.len();
        let mut joined = vec![false; n];
        let seam = |i: usize, j: usize| -> Option<(f64, DVec3, f64, DVec3)> {
            let (a, b) = (&model.solids[i], &model.solids[j]);
            let lo = a.lo.max(b.lo);
            let hi = a.hi.min(b.hi);
            let o = hi - lo + DVec3::splat(TOUCH);
            if o.x < 0.0 || o.y < 0.0 || o.z < 0.0 {
                return None;
            }
            let o = o.max(DVec3::splat(1e-6));
            let (area, normal) = if o.x <= o.y && o.x <= o.z {
                (o.y * o.z, DVec3::X)
            } else if o.y <= o.z {
                (o.x * o.z, DVec3::Y)
            } else {
                (o.x * o.y, DVec3::Z)
            };
            Some((area.max(1e-4), (lo + hi) / 2.0, o.max_element(), normal))
        };
        let mut fasten =
            |world: &mut World, i: usize, j: usize, middle: DVec3, span: f64, normal: DVec3| {
                // Bolted: bolts by the seam's length, the pattern's
                // capacities from the bolts and their spacing.
                let (count, size) = bolts_for(span);
                let (tension, shear) = bolt_strength(size);
                let n = f64::from(count);
                let span = span.max(0.01);
                let (_, qi) = rest(i);
                let (_, qj) = rest(j);
                let capacity = Capacity::Bolted {
                    bolts: count,
                    size,
                    normal: qi.inverse() * (turn * normal),
                    shear: n * shear,
                    tension: n * tension,
                    // Half the bolts in tension at half the pattern's length.
                    bending: n * tension * span / 4.0,
                    twist: n * shear * span / 2.0,
                };
                let name = format!(
                    "{} to {}",
                    model.solids[i].instance, model.solids[j].instance
                );
                join(
                    world,
                    i,
                    j,
                    origin + turn * middle,
                    JointKind::Weld {
                        relative: qi.inverse() * qj,
                    },
                    Kind::Fastened,
                    name,
                    capacity,
                );
            };
        for i in 0..n {
            for j in i + 1..n {
                if groups[i] != groups[j] {
                    continue;
                }
                if let Some((_, middle, span, normal)) = seam(i, j) {
                    fasten(&mut world, i, j, middle, span, normal);
                    joined[i] = true;
                    joined[j] = true;
                }
            }
        }
        // A solid touching nothing in its group joins the nearest one.
        for i in 0..n {
            if joined[i] {
                continue;
            }
            let nearest = (0..n)
                .filter(|&j| j != i && groups[j] == groups[i])
                .min_by(|&x, &y| {
                    model.solids[x]
                        .com
                        .distance(model.solids[i].com)
                        .total_cmp(&model.solids[y].com.distance(model.solids[i].com))
                });
            if let Some(j) = nearest {
                let middle = (model.solids[i].com + model.solids[j].com) / 2.0;
                fasten(&mut world, i, j, middle, 0.01, DVec3::Y);
            }
        }
        world.solver.iterations = world.solver.iterations.max(30);
        // The debris boxes, parked under the ground until used.
        let mut debris = Vec::new();
        for k in 0..DEBRIS {
            let id = world.add(
                Body::new(1.0, DVec3::ONE, origin - DVec3::Y * (60.0 + k as f64))
                    .with_kind(BodyKind::Kinematic),
            );
            world.add_collider(
                Collider::new(
                    id,
                    Shape::Cuboid {
                        half: DVec3::splat(0.1),
                    },
                )
                .with_filter(Filter {
                    group: DEBRIS_BIT,
                    mask: !(DEBRIS_BIT | GROUND_BIT),
                })
                .with_material(Material {
                    friction: 0.7,
                    torsional: 0.01,
                    restitution: 0.1,
                }),
            );
            debris.push(id);
        }
        let mut rover = Rover {
            world,
            bodies,
            colliders,
            links,
            debris,
            intimate: std::collections::BTreeSet::new(),
            fast: Vec::new(),
            last_dust: f64::NEG_INFINITY,
            ground,
            origin,
            caster: caster.as_dvec3(),
            carry: 0.0,
            events: Vec::new(),
            hardest: None,
            elapsed: 0.0,
        };
        rover.fast = vec![false; rover.bodies.len()];
        rover.regroup();
        rover.settle();
        rover
    }

    /// Lets the rover settle under gravity with its joints unbreakable,
    /// then gives each joint the larger of its strength model and
    /// [`DESIGN_FACTOR`] times the load it carried standing.
    fn settle(&mut self) {
        let steps = (SETTLE / STEP).round() as usize;
        let mut used = vec![0.0f64; self.links.len()];
        let field = |_: DVec3, _: DVec3| DVec3::new(0.0, -GRAVITY, 0.0);
        for step in 0..steps {
            self.world.step(&field);
            // The last half: the load standing, not the first jolt.
            if step >= steps / 2 {
                for (k, link) in self.links.iter().enumerate() {
                    let (force, torque) = self.load(link);
                    used[k] = used[k].max(link.capacity.used(force, torque, 1.0));
                }
            }
        }
        for (k, link) in self.links.iter_mut().enumerate() {
            link.scale = (DESIGN_FACTOR * used[k]).max(1.0);
        }
        for b in self.world.bodies_mut() {
            b.vel = DVec3::ZERO;
            b.omega = DVec3::ZERO;
        }
    }

    /// The force and torque a joint carried in the last step, in its body
    /// `a`'s frame, N and N m.
    fn load(&self, link: &Link) -> (DVec3, DVec3) {
        let Some(joint) = self.world.joint(link.id) else {
            return (DVec3::ZERO, DVec3::ZERO);
        };
        let q = self.world.bodies()[self.bodies[link.bodies.0].0 as usize]
            .orientation
            .inverse();
        (q * joint.impulse / STEP, q * joint.angular_impulse / STEP)
    }

    /// The pieces: each body's root, from the joints still holding.
    fn pieces(&self) -> Vec<usize> {
        let n = self.bodies.len();
        let mut parent: Vec<usize> = (0..n).collect();
        for link in self.links.iter().filter(|l| l.broke.is_none()) {
            let (a, b) = (
                find(&mut parent, link.bodies.0),
                find(&mut parent, link.bodies.1),
            );
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
        (0..n).map(|k| find(&mut parent, k)).collect()
    }

    /// A body's box in the world: its min and max corners.
    fn bounds(&self, k: usize) -> (DVec3, DVec3) {
        let s = &model().solids[k];
        let b = &self.world.bodies()[self.bodies[k].0 as usize];
        let center = b.pos + b.orientation * s.center;
        let m = DMat3::from_quat(b.orientation);
        let half = DVec3::new(
            m.row(0).abs().dot(s.half),
            m.row(1).abs().dot(s.half),
            m.row(2).abs().dot(s.half),
        );
        (center - half, center + half)
    }

    fn overlap(&self, i: usize, j: usize) -> bool {
        let (a, b) = (self.bounds(i), self.bounds(j));
        (0..3).all(|k| a.0[k] < b.1[k] + 0.01 && b.0[k] < a.1[k] + 0.01)
    }

    /// Gives each piece still held together by its joints a collision bit
    /// of its own, so pieces collide with each other but a piece's own
    /// solids don't. Solids that were joined and still overlap where they
    /// broke apart (a rail inside the body's box, a tube in its tray) don't
    /// collide until they are clear of each other: their boxes stand for
    /// hollow parts, and pushing them apart from inside would throw them.
    fn regroup(&mut self) {
        let n = self.bodies.len();
        let roots = self.pieces();
        let mut parent: Vec<usize> = (0..n).collect();
        for k in 0..n {
            let (a, b) = (find(&mut parent, k), find(&mut parent, roots[k]));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
        for &(i, j) in &self.intimate {
            let (a, b) = (find(&mut parent, i), find(&mut parent, j));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
        let mut bits: BTreeMap<usize, u32> = BTreeMap::new();
        for k in 0..n {
            let root = find(&mut parent, k);
            let next = bits.len() as u32;
            let bit = *bits.entry(root).or_insert(1 << (2 + next % PIECE_BITS));
            if self.fast[k] {
                for &c in &self.colliders[k] {
                    self.world.collider_mut(c).filter = Filter {
                        group: FAST_BIT,
                        mask: GROUND_BIT | DEBRIS_BIT,
                    };
                }
                continue;
            }
            for &c in &self.colliders[k] {
                self.world.collider_mut(c).filter = Filter {
                    group: bit,
                    mask: !bit,
                };
            }
        }
    }

    /// The houses' debris near the rover this step: boxes (centre,
    /// rotation, velocity, half extents) it collides with.
    pub fn set_debris(&mut self, boxes: &[(DVec3, DQuat, DVec3, DVec3)]) {
        for (k, &id) in self.debris.iter().enumerate() {
            let body = &mut self.world.bodies_mut()[id.0 as usize];
            match boxes.get(k) {
                Some(&(pos, rot, vel, _)) => {
                    body.pos = pos;
                    body.orientation = rot;
                    body.vel = vel;
                }
                None => {
                    body.pos = self.origin - DVec3::Y * (60.0 + k as f64);
                    body.vel = DVec3::ZERO;
                }
            }
        }
        let ids: Vec<ColliderId> = (0..self.world.colliders().len() as u32)
            .map(ColliderId)
            .filter(|c| {
                self.debris
                    .contains(&self.world.colliders()[c.0 as usize].body)
            })
            .collect();
        for (k, c) in ids.into_iter().enumerate() {
            if let Some(&(_, _, _, half)) = boxes.get(k) {
                self.world.collider_mut(c).shape = Shape::Cuboid { half };
            }
        }
    }

    /// The body's centre: the chassis solid, where it is now.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        let model = model();
        let k = model
            .solids
            .iter()
            .enumerate()
            .filter(|(_, s)| s.instance == "chassis")
            .max_by(|a, b| a.1.mass.total_cmp(&b.1.mass))
            .map_or(0, |(k, _)| k);
        self.world.bodies()[self.bodies[k].0 as usize].pos.as_vec3()
    }

    /// The rover's total linear momentum, kg m/s.
    #[must_use]
    pub fn momentum(&self) -> DVec3 {
        self.bodies
            .iter()
            .map(|id| self.world.bodies()[id.0 as usize].momentum())
            .sum()
    }

    /// The rover's total mass, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.bodies
            .iter()
            .map(|id| self.world.bodies()[id.0 as usize].mass)
            .sum()
    }

    /// How many solids (bodies) and joints it has.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (self.bodies.len(), self.links.len())
    }

    /// Every joint, with its limits, peak loads and whether it broke.
    #[must_use]
    pub fn joints(&self) -> &[Link] {
        &self.links
    }

    /// The instances whose solids broke free of the body, in order.
    #[must_use]
    pub fn broken(&self) -> Vec<String> {
        let mut out: Vec<(f64, String)> = self
            .links
            .iter()
            .filter_map(|l| l.broke.map(|t| (t, l.name.clone())))
            .collect();
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.into_iter().map(|(_, n)| n).collect()
    }

    /// The hardest knock: when, s, and the largest speed it gave a part,
    /// m/s.
    #[must_use]
    pub fn hardest(&self) -> Option<(f64, f64)> {
        self.hardest
    }

    /// The sparks and dust since the last call.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn velocities(&self) -> Vec<DVec3> {
        self.bodies
            .iter()
            .map(|id| self.world.bodies()[id.0 as usize].vel)
            .collect()
    }

    fn note_knock(&mut self, before: &[DVec3]) {
        let after = self.velocities();
        let knock = before
            .iter()
            .zip(&after)
            .map(|(a, b)| (*b - *a).length())
            .fold(0.0, f64::max);
        if knock > 0.05 && self.hardest.is_none_or(|(_, k)| knock > k) {
            self.hardest = Some((self.elapsed, knock));
        }
    }

    /// A meteor's blast at `at` with radius `radius`: every solid in reach
    /// is pushed by the area it shows the blast. Returns the impulses given
    /// (solid's instance, N s).
    pub fn push(&mut self, at: Vec3, radius: f32) -> Vec<(String, DVec3)> {
        let at = at.as_dvec3();
        let radius = f64::from(radius).max(0.1);
        let model = model();
        let before = self.velocities();
        let mut given = Vec::new();
        for (k, id) in self.bodies.iter().enumerate() {
            let s = &model.solids[k];
            let body = &mut self.world.bodies_mut()[id.0 as usize];
            let center = body.pos + body.orientation * s.center;
            let away = center - at;
            let d = away.length();
            if d >= radius {
                continue;
            }
            let dir = (away.normalize_or(DVec3::Y) + DVec3::Y * 0.6).normalize();
            let local = (body.orientation.inverse() * dir).abs();
            let h = s.half;
            let area = 4.0 * (h.y * h.z * local.x + h.x * h.z * local.y + h.x * h.y * local.z);
            let fall = 1.0 - d / radius;
            let impulse = dir * (BLAST_PRESSURE * area * fall).min(s.mass * BLAST_SPEED * fall);
            body.wake();
            body.apply_impulse_at(impulse, center);
            given.push((s.instance.clone(), impulse));
        }
        self.note_knock(&before);
        given
    }

    /// A meteor of mass `mass` (kg) and momentum `momentum` (N s) flying
    /// along `flight` that lands at `at`: it bursts on the first solid its
    /// flight meets (within [`STRUCK`] of the landing), which takes its
    /// whole momentum as one sharp impulse in a step. Returns that solid
    /// and the impulse.
    pub fn strike(
        &mut self,
        at: DVec3,
        flight: DVec3,
        mass: f64,
        momentum: f64,
    ) -> Vec<(String, DVec3)> {
        let model = model();
        let flight = flight.normalize_or(DVec3::NEG_Y);
        let from = at - flight * 3.0;
        let before = self.velocities();
        // Every box the flight enters, in order.
        let mut met: Vec<(f64, usize)> = Vec::new();
        for (k, id) in self.bodies.iter().enumerate() {
            let s = &model.solids[k];
            let body = &self.world.bodies()[id.0 as usize];
            let to_local = |p: DVec3| body.orientation.inverse() * (p - body.pos) - s.center;
            let (o, d) = (to_local(from), body.orientation.inverse() * flight);
            let (mut t0, mut t1) = (0.0f64, 3.0 + STRUCK);
            let mut hit = true;
            for axis in 0..3 {
                let (oa, da, ha) = (o[axis], d[axis], s.half[axis]);
                if da.abs() < 1e-12 {
                    if oa.abs() > ha {
                        hit = false;
                    }
                    continue;
                }
                let (mut a, mut b) = ((-ha - oa) / da, (ha - oa) / da);
                if a > b {
                    std::mem::swap(&mut a, &mut b);
                }
                t0 = t0.max(a);
                t1 = t1.min(b);
            }
            if hit && t0 <= t1 {
                met.push((t0, k));
            }
        }
        met.sort_by(|a, b| a.0.total_cmp(&b.0));
        let _ = mass;
        let mut given = Vec::new();
        // The meteor bursts on the first solid its flight meets: its whole
        // momentum, in one step, into that solid. The joints round it take
        // the shock.
        for (t, k) in met.into_iter().take(1) {
            let point = from + flight * t;
            let impulse = flight * momentum;
            let body = &mut self.world.bodies_mut()[self.bodies[k].0 as usize];
            body.wake();
            body.apply_impulse_at(impulse, point);
            if given.is_empty() {
                self.events.push(Event {
                    effect: "rover_sparks",
                    at: point.as_vec3(),
                });
            }
            given.push((model.solids[k].instance.clone(), impulse));
        }
        self.note_knock(&before);
        given
    }

    /// A meteor landing at `at` with blast radius `radius`: its blast, and
    /// its own momentum when it landed on the rover.
    pub fn blast(&mut self, at: Vec3, radius: f32) -> Vec<(String, DVec3)> {
        let mut given = self.push(at, radius);
        let horizontal = DVec3::new(
            f64::from(at.x) - self.caster.x,
            0.0,
            f64::from(at.z) - self.caster.z,
        )
        .normalize_or(DVec3::X);
        let flight = (horizontal * 0.55 + DVec3::NEG_Y * 0.83).normalize();
        given.extend(self.strike(
            at.as_dvec3(),
            flight,
            METEOR_MASS,
            METEOR_MASS * METEOR_SPEED,
        ));
        given
    }

    /// Advances `dt` seconds in fixed steps, breaking the joints that
    /// reached their limits and noting hard landings.
    pub fn step(&mut self, dt: f32) {
        self.carry += f64::from(dt.max(0.0));
        while self.carry >= STEP {
            self.carry -= STEP;
            self.tick(true);
        }
    }

    fn tick(&mut self, gravity: bool) {
        let g = if gravity { GRAVITY } else { 0.0 };
        let field = move |_: DVec3, _: DVec3| DVec3::new(0.0, -g, 0.0);
        self.world.step(&field);
        self.elapsed += STEP;
        let mut snapped = false;
        let before = self.pieces();
        for k in 0..self.links.len() {
            if self.links[k].broke.is_some() {
                continue;
            }
            let (force, torque) = self.load(&self.links[k]);
            let link = &mut self.links[k];
            link.peak_force = link.peak_force.max(force.length());
            link.peak_torque = link.peak_torque.max(torque.length());
            let used = link.capacity.used(force, torque, link.scale);
            link.peak_use = link.peak_use.max(used);
            if used >= 1.0 {
                link.broke = Some(self.elapsed);
                let (id, a, anchor) = (link.id, link.bodies.0, link.anchor);
                let body = &self.world.bodies()[self.bodies[a].0 as usize];
                let at = body.pos + body.orientation * anchor;
                self.world.remove_joint(id);
                self.events.push(Event {
                    effect: "rover_sparks",
                    at: at.as_vec3(),
                });
                snapped = true;
            }
        }
        let fast: Vec<bool> = self
            .bodies
            .iter()
            .map(|id| self.world.bodies()[id.0 as usize].vel.length() > FAST)
            .collect();
        let mut changed = fast != self.fast;
        self.fast = fast;
        if snapped {
            let after = self.pieces();
            let n = self.bodies.len();
            for i in 0..n {
                for j in i + 1..n {
                    if before[i] == before[j] && after[i] != after[j] && self.overlap(i, j) {
                        self.intimate.insert((i, j));
                    }
                }
            }
            changed = true;
        }
        let clear: Vec<(usize, usize)> = self
            .intimate
            .iter()
            .copied()
            .filter(|&(i, j)| !self.overlap(i, j))
            .collect();
        for pair in clear {
            self.intimate.remove(&pair);
            changed = true;
        }
        if changed {
            self.regroup();
        }
        // A hard landing raises one puff of dust, at most every
        // [`DUST_EVERY`] seconds, so the dust doesn't hide what landed.
        if self.elapsed - self.last_dust >= DUST_EVERY
            && let Some(c) = self.world.contacts.iter().find(|c| {
                c.impulse.length() > DUST_IMPULSE
                    && (self.bodies.contains(&c.body_a) || self.bodies.contains(&c.body_b))
            })
        {
            self.last_dust = self.elapsed;
            self.events.push(Event {
                effect: "rover_landing",
                at: c.point.as_vec3(),
            });
        }
    }

    /// The rover's triangles where its solids are now, lit physically.
    #[must_use]
    pub fn lit(&self) -> Vec<LitVertex> {
        let model = model();
        let mut out = Vec::new();
        for (k, id) in self.bodies.iter().enumerate() {
            let s = &model.solids[k];
            let body = &self.world.bodies()[id.0 as usize];
            let rotation = body.orientation.as_quat();
            let pos = body.pos.as_vec3();
            for tri in s.indices.chunks_exact(3) {
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize);
                let [pa, pb, pc] = [a, b, c].map(|i| pos + rotation * s.positions[i]);
                let tangent = (pb - pa).normalize_or(Vec3::X);
                for (p, i) in [(pa, a), (pb, b), (pc, c)] {
                    out.push(LitVertex {
                        pos: p.to_array(),
                        normal: (rotation * s.normals[i]).normalize_or(Vec3::Y).to_array(),
                        tangent: tangent.to_array(),
                        local: s.positions[i].to_array(),
                        color: s.color,
                        params: [0.15, 0.55, 0.0, 1.0],
                    });
                }
            }
        }
        out
    }

    /// Each solid's pose: its instance, position and rotation.
    #[must_use]
    pub fn poses(&self) -> Vec<(String, DVec3, DQuat)> {
        let model = model();
        self.bodies
            .iter()
            .enumerate()
            .map(|(k, id)| {
                let b = &self.world.bodies()[id.0 as usize];
                (model.solids[k].instance.clone(), b.pos, b.orientation)
            })
            .collect()
    }

    /// Lifts the rover off the ground and turns gravity off, for a test of
    /// momentum: only its own joints and collisions act between its parts.
    #[cfg(test)]
    fn float(&mut self) {
        self.world.collider_mut(self.ground).filter = Filter { group: 0, mask: 0 };
    }

    #[cfg(test)]
    fn step_free(&mut self, steps: usize) {
        for _ in 0..steps {
            self.tick(false);
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

    fn settled() -> Rover {
        let mut r = rover();
        for _ in 0..60 {
            r.step(1.0 / 60.0);
        }
        r
    }

    #[test]
    fn every_solid_is_a_body_and_they_weigh_what_cad_lab_measured() {
        let r = rover();
        let (bodies, joints) = r.counts();
        assert_eq!(bodies, 51);
        assert!(joints >= 50, "{joints} joints");
        // CAD Lab measured 87.94 kg for v38.
        assert!((r.mass() - 87.94).abs() < 0.05, "{}", r.mass());
        // The CAD joints are pins; the seams are fastened.
        assert_eq!(
            r.joints().iter().filter(|l| l.kind == Kind::Pin).count(),
            12
        );
        // Seams are bolted by their size: the bay on the deck with more
        // and bigger bolts than a solar wing's mount.
        let bolts = |a: &str, b: &str| {
            r.joints()
                .iter()
                .filter(|l| (l.a == a && l.b == b) || (l.a == b && l.b == a))
                .find_map(|l| match l.capacity {
                    Capacity::Bolted {
                        bolts,
                        size,
                        tension,
                        ..
                    } => Some((bolts, size, tension)),
                    Capacity::Pin { .. } => None,
                })
        };
        let bay = bolts("chassis", "avionics_bay").expect("the bay is bolted down");
        let wing = bolts("chassis", "solar_panel_l").expect("the wing is bolted on");
        assert!(bay.0 >= 6 && bay.2 > 2.0 * wing.2, "{bay:?} {wing:?}");
        assert_eq!(bolts_for(0.32), (6, "M5"));
        // One M5's tension: 800 MPa over 14.2 mm².
        assert!((bolt_strength("M5").0 - 11_360.0).abs() < 1.0);
    }

    #[test]
    fn it_stands_whole_until_something_hits_it() {
        let r = settled();
        assert!(r.broken().is_empty(), "{:?}", r.broken());
        let start = rover().center();
        assert!((r.center() - start).length() < 0.08);
    }

    #[test]
    fn momentum_is_conserved_through_a_strike_and_what_breaks_after() {
        let mut r = rover();
        r.float();
        let before = r.momentum();
        let at = r.center() + Vec3::new(0.1, 0.4, 0.0);
        let given = r.blast(at, 5.0);
        let impulses: DVec3 = given.iter().map(|(_, j)| *j).sum();
        let after = r.momentum();
        assert!((after - before - impulses).length() < 1e-9 * impulses.length().max(1.0));
        // Joints and the parts' collisions are internal: the rover keeps
        // its momentum while it flies apart.
        r.step_free(240);
        assert!(!r.broken().is_empty());
        let later = r.momentum();
        assert!(
            (later - after).length() < 1e-6 * after.length(),
            "{after} -> {later}"
        );
    }

    #[test]
    fn a_direct_hit_breaks_every_joint_loaded_past_its_limit_and_no_other() {
        let mut r = settled();
        let at = r.center().as_dvec3() + DVec3::new(0.0, 0.2, 0.0);
        let flight = DVec3::new(0.5, -0.85, 0.1);
        let hit = r.strike(at, flight, METEOR_MASS, METEOR_MASS * METEOR_SPEED);
        assert!(!hit.is_empty());
        for _ in 0..30 {
            r.step(1.0 / 60.0);
        }
        let broke: Vec<&Link> = r.joints().iter().filter(|l| l.broke.is_some()).collect();
        assert!(!broke.is_empty(), "nothing broke");
        for l in r.joints() {
            assert_eq!(
                l.broke.is_some(),
                l.peak_use >= 1.0,
                "{} {:?}: {:.2} of its capacity",
                l.name,
                l.kind,
                l.peak_use
            );
        }
    }

    #[test]
    fn a_weak_hit_breaks_nothing() {
        let mut r = settled();
        let at = r.center().as_dvec3() + DVec3::new(0.0, 0.2, 0.0);
        assert!(
            !r.strike(at, DVec3::new(0.5, -0.85, 0.1), 1.0, 1.0 * 2.0)
                .is_empty()
        );
        for _ in 0..60 {
            r.step(1.0 / 60.0);
        }
        assert!(r.broken().is_empty(), "{:?}", r.broken());
    }

    #[test]
    fn the_same_strike_does_exactly_the_same_twice() {
        let run = || {
            let mut r = settled();
            let at = r.center() + Vec3::new(0.6, -0.4, 0.3);
            r.blast(at, 6.0);
            for _ in 0..120 {
                r.step(1.0 / 60.0);
            }
            (r.poses(), r.broken())
        };
        let (poses, broken) = run();
        let again = run();
        assert_eq!(poses, again.0);
        assert_eq!(broken, again.1);
    }
}
