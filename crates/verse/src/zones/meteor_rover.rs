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
//! A meteor's energy comes from the damage the scene shows: the ground
//! burst that gives [`blast::DAMAGE_OVERPRESSURE`] (50 kPa, enough to
//! flatten brick houses) at its damage radius, 5.2 m in the showcase: a
//! free-air charge of 2.58 kg TNT, 6.0 MJ released at the ground, a 1.33 kg
//! impactor at [`IMPACT_SPEED`]. Its blast reaches every solid as a
//! Friedlander pulse from the Kinney–Graham curves at the solid's distance:
//! the reflected overpressure on the face it shows, fading as the wave
//! clears round it, or the wind's drag, whichever is greater, times the
//! area it shows, integrated through each step as a real force with
//! nothing capping it. Solids shadowed by another get [`SHIELDED`] of it.
//! A meteor whose flight meets the rover strikes the first solid on it
//! inelastically; above [`DISRUPTION`] the solid breaks into fragments, and
//! the energy it took goes off as a burst where it hit. Air drag slows
//! everything that flies. While a pulse crosses the rover the physics
//! steps at 21 µs ([`FINE`]) and each joint's load is averaged over
//! [`WINDOW`] substeps, so the joints feel the pulse's real peak. The hand
//! calculations are in
//! `docs/verse/meteor-showcase-handoff.md`.
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
//! ground from then on: it would cross another part within a step, and
//! meeting the others again as it slows, from inside them, would throw
//! it.
//!
//! Native only: the glTF is built into the binary, which the browser build
//! leaves out.

use super::everglade::cad_physics::rover_pin_radius as pin_radius;
pub use super::everglade::cad_physics::{Capacity, bolt_strength, bolts_for, yield_strength};
pub use super::everglade::cad_physics::{DESIGN_FACTOR, PIN_EFFICIENCY, PIN_LENGTH};
use super::everglade::cad_physics::{Solid, instance, parse_glb};
use glam::{DMat3, DQuat, DVec3, Vec3};
use physics::body::{Body, BodyKind};
use physics::collision::{Collider, ColliderId, Filter, Material, Shape};
use physics::joint::{Joint, JointId, JointKind};
use physics::world::{BodyId, World};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::pbr::LitVertex;

pub use super::everglade::cad_physics::blast;

/// The rover's glTF and assembly data, exported from CAD Lab.
const GLB: &[u8] = include_bytes!("../../../../assets/verse/cad/rover.glb");
const SPEC: &str = include_str!("../../../../assets/verse/cad/rover.json");

/// The physics step, s.
pub const STEP: f64 = 1.0 / 240.0;
/// How close a resting body's lowest point must be to what holds it up, m
/// ([`Rover::resting`]): the parts collide as boxes (the wheels as
/// capsules), so a thin or round part may rest a few centimetres off its
/// box's corner.
pub const REST_CONTACT: f64 = 0.05;
/// Gravity, m/s².
pub const GRAVITY: f64 = 9.81;
/// The meteor's speed when it reaches the ground, m/s: hypervelocity,
/// still carrying a good part of its entry speed (a stony or iron body
/// that has slowed from 11–20 km/s but is far from terminal velocity).
/// Its mass follows from its energy ([`Rover::blast`]).
pub const IMPACT_SPEED: f64 = 3_000.0;
/// How far from a meteor's landing point the rover counts as struck, m.
pub const STRUCK: f64 = 1.2;
/// Specific energy that breaks a solid into fragments, J/kg: the
/// catastrophic-disruption threshold Q* of metal targets struck at
/// kilometres a second is of order 10⁴ J/kg (Fujiwara et al. 1989, and
/// Holsapple's scaling for strength-dominated bodies).
pub const DISRUPTION: f64 = 1.0e4;
/// The share of the energy a disrupted solid takes that goes into its
/// fragments flying apart (the rest heats and deforms them): a few per
/// cent in impact-disruption experiments.
pub const SPREAD: f64 = 0.03;
/// Drag coefficient of a part, flat or blunt, in the blast's wind and
/// flying through the air.
pub const DRAG: f64 = 1.2;
/// The share of the blast a part gets when another part stands between
/// it and the burst: the wave diffracts round small obstacles, so the
/// shadow is partial.
pub const SHIELDED: f64 = 0.5;
/// How long a burst's wave is followed, s: past every part in reach.
const BURST_LIFE: f64 = 0.25;
/// Substeps per step while a blast pulse crosses the rover: 1/240 s in
/// 200 is 21 µs, short against the pulse's positive phase (0.1–3 ms here),
/// so joints carry its peak.
pub const FINE: usize = 200;
/// Substeps a joint's load is averaged over while stepping finely: 10 ×
/// 21 µs is 0.21 ms, the order of the rover's parts' own response time
/// (a 0.3 m aluminium panel's first bending mode is about 1 kHz), so a
/// joint feels the pulse's peak as the structure passes it on, not each
/// solver iteration's spike.
pub const WINDOW: usize = 10;
/// The longest a burst steps finely, s.
const FINE_LONGEST: f64 = 0.02;
/// How long the rover settles under gravity when it is made, s.
const SETTLE: f64 = 0.5;
/// A contact this hard (impulse in one step, N s) raises dust.
const DUST_IMPULSE: f64 = 4.0;
/// The least time between two puffs of dust, s.
const DUST_EVERY: f64 = 0.4;
/// The ground's slab is this deep below the surface (half), m: deep
/// enough that nothing crosses it in a step.
const GROUND_DEPTH: f64 = 50.0;
/// Where unused debris boxes wait, m below the rover, under the ground.
const PARKED: f64 = 400.0;
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

/// From the CAD frame (mm, X forward, Y left, Z up) to the rover's frame
/// in Verse (m, X forward, Y up, Z right): the glTF's own frame.
fn from_cad(p: [f64; 3]) -> DVec3 {
    DVec3::new(p[0], p[2], -p[1]) / 1000.0
}

fn triple(v: &Value) -> [f64; 3] {
    [0, 1, 2].map(|k| v[k].as_f64().unwrap_or(0.0))
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
    /// The impulse and angular impulse it carried since its load was last
    /// checked, body `a`'s frame.
    carried: (DVec3, DVec3),
}

/// Something worth an effect: sparks where a joint broke (`rover_sparks`),
/// a puff of dust where a part landed hard (`rover_landing`); both fire
/// once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub effect: &'static str,
    pub at: Vec3,
}

/// A solid as the rover simulates it: one of the CAD solids, or a
/// fragment of one.
#[derive(Clone)]
struct Part {
    instance: String,
    material: String,
    mass: f64,
    /// Its box: centre (body frame) and half extents, m.
    center: DVec3,
    half: DVec3,
    /// Vertices and normals in the mesh frame (the CAD solid's body
    /// frame); a vertex sits at `p - shift` in this part's body frame.
    shift: Vec3,
    positions: std::sync::Arc<Vec<Vec3>>,
    normals: std::sync::Arc<Vec<Vec3>>,
    indices: std::sync::Arc<Vec<u32>>,
    color: [f32; 3],
    /// Broken into fragments: its body is out of the world.
    gone: bool,
}

/// A blast wave on its way: where and when it went off, and its free-air
/// TNT charge, kg.
#[derive(Clone, Copy, Debug)]
struct Burst {
    at: DVec3,
    charge: f64,
    start: f64,
    /// Until when its pulse is still crossing the rover: the physics steps
    /// finely till then.
    fine_until: f64,
}

/// The rover on the lot: its world, its bodies (one per solid), its joints,
/// and what happened to it.
pub struct Rover {
    world: World,
    /// The solver's position correction at the full step.
    erp: f64,
    /// One per body.
    parts: Vec<Part>,
    bursts: Vec<Burst>,
    fragmented: Vec<String>,
    struck: Vec<(String, f64, f64)>,
    /// Each body's impulse from outside, and their sum, N s.
    received: Vec<DVec3>,
    ledger: DVec3,
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
    #[cfg_attr(not(test), allow(dead_code))]
    ground: ColliderId,
    origin: DVec3,
    caster: DVec3,
    carry: f64,
    events: Vec<Event>,
    /// Every blast it felt: when (s), where, and its damage radius (m).
    felt: Vec<(f64, Vec3, f32)>,
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
        let ground_body = world.add(
            Body::new(1.0, DVec3::ONE, origin - DVec3::Y * GROUND_DEPTH)
                .with_kind(BodyKind::Static),
        );
        let ground = world.add_collider(
            Collider::new(
                ground_body,
                Shape::Cuboid {
                    half: DVec3::new(300.0, GROUND_DEPTH, 300.0),
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
        let (bodies, colliders) = super::everglade::cad_physics::insert_solids(
            &mut world,
            &model.solids,
            origin,
            turn,
            1.,
            true,
            Filter {
                group: 1 << 2,
                mask: !(1 << 2),
            },
            Material {
                friction: 0.85,
                torsional: 0.01,
                restitution: 0.2,
            },
        );
        let colliders = colliders.into_iter().map(|c| vec![c]).collect::<Vec<_>>();
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
                    carried: (DVec3::ZERO, DVec3::ZERO),
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
            let (limits, servo) = super::everglade::cad_physics::rover_joint_settings(name);
            let kind = JointKind::Hinge {
                axis: qa.inverse() * axis_world,
                relative,
                limits,
                servo,
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
                Body::new(1.0, DVec3::ONE, origin - DVec3::Y * (PARKED + k as f64))
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
                    mask: !(DEBRIS_BIT | GROUND_BIT | FAST_BIT),
                })
                .with_material(Material {
                    friction: 0.7,
                    torsional: 0.01,
                    restitution: 0.1,
                }),
            );
            debris.push(id);
        }
        let parts: Vec<Part> = model
            .solids
            .iter()
            .map(|s| Part {
                instance: s.instance.clone(),
                material: s.material.clone(),
                mass: s.mass,
                center: s.center,
                half: s.half,
                shift: Vec3::ZERO,
                positions: std::sync::Arc::new(s.positions.clone()),
                normals: std::sync::Arc::new(s.normals.clone()),
                indices: std::sync::Arc::new(s.indices.clone()),
                color: s.color,
                gone: false,
            })
            .collect();
        let received = vec![DVec3::ZERO; parts.len()];
        let erp = world.solver.erp;
        let mut rover = Rover {
            erp,
            world,
            parts,
            bursts: Vec::new(),
            fragmented: Vec::new(),
            struck: Vec::new(),
            received,
            ledger: DVec3::ZERO,
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
            felt: Vec::new(),
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
        let dt = self.world.dt;
        (q * joint.impulse / dt, q * joint.angular_impulse / dt)
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

    /// A part's box in the world: its min and max corners.
    fn bounds(&self, k: usize) -> (DVec3, DVec3) {
        let s = &self.parts[k];
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
        if self.parts[i].gone || self.parts[j].gone {
            return false;
        }
        let (a, b) = (self.bounds(i), self.bounds(j));
        (0..3).all(|k| a.0[k] < b.1[k] + 0.01 && b.0[k] < a.1[k] + 0.01)
    }

    /// Gives each piece still held together by its joints a collision bit
    /// of its own, so pieces collide with each other but a piece's own
    /// solids don't. Solids that were joined and still overlap where they
    /// broke apart (a rail inside the body's box, a tube in its tray, a
    /// part's fragments) don't collide until they are clear of each other:
    /// their boxes stand for hollow parts, and pushing them apart from
    /// inside would throw them.
    fn regroup(&mut self) {
        let n = self.bodies.len();
        let roots = self.pieces();
        // Solids of different pieces that overlap and don't collide now (a
        // fast part that flew into another, or two pieces that shared a
        // collision bit) stay apart until clear: colliding them from inside
        // would throw them.
        for i in 0..n {
            for j in i + 1..n {
                if roots[i] == roots[j] || self.intimate.contains(&(i, j)) || !self.overlap(i, j) {
                    continue;
                }
                let filter =
                    |k: usize| self.world.colliders()[self.colliders[k][0].0 as usize].filter;
                if !filter(i).allows(filter(j)) {
                    self.intimate.insert((i, j));
                }
            }
        }
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
            if self.parts[k].gone {
                continue;
            }
            let root = find(&mut parent, k);
            let next = bits.len() as u32;
            let bit = *bits.entry(root).or_insert(1 << (2 + next % PIECE_BITS));
            let filter = if self.fast[k] {
                Filter {
                    group: FAST_BIT,
                    mask: GROUND_BIT,
                }
            } else {
                Filter {
                    group: bit,
                    mask: !(bit | FAST_BIT),
                }
            };
            for &c in &self.colliders[k] {
                self.world.collider_mut(c).filter = filter;
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
                    body.pos = self.origin - DVec3::Y * (PARKED + k as f64);
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

    fn live(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.parts.len()).filter(|&k| !self.parts[k].gone)
    }

    fn body(&self, k: usize) -> &Body {
        &self.world.bodies()[self.bodies[k].0 as usize]
    }

    /// The body's centre: the centre of mass of its heaviest piece (the
    /// solids still joined together), where it is now.
    #[must_use]
    pub fn center(&self) -> Vec3 {
        let roots = self.pieces();
        let mut mass: BTreeMap<usize, f64> = BTreeMap::new();
        for k in self.live() {
            *mass.entry(roots[k]).or_default() += self.parts[k].mass;
        }
        let Some((&root, &total)) = mass.iter().max_by(|a, b| a.1.total_cmp(b.1)) else {
            return Vec3::ZERO;
        };
        let sum: DVec3 = self
            .live()
            .filter(|&k| roots[k] == root)
            .map(|k| self.body(k).pos * self.parts[k].mass)
            .sum();
        (sum / total).as_vec3()
    }

    /// The rover's total linear momentum, kg m/s.
    #[must_use]
    pub fn momentum(&self) -> DVec3 {
        self.live().map(|k| self.body(k).momentum()).sum()
    }

    /// The rover's total kinetic energy, J.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        self.live()
            .map(|k| {
                let b = self.body(k);
                0.5 * b.mass * b.vel.length_squared() + 0.5 * (b.inertia * b.omega).dot(b.omega)
            })
            .sum()
    }

    /// The rover's total mass, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.live().map(|k| self.body(k).mass).sum()
    }

    /// How many solids (bodies, fragments included) and joints it has.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (self.live().count(), self.links.len())
    }

    /// Every joint, with its limits, peak loads and whether it broke.
    #[must_use]
    pub fn joints(&self) -> &[Link] {
        &self.links
    }

    /// The joints that broke, in order.
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

    /// What each strike hit: the solid, the momentum it took, N s, and
    /// the energy it took, J.
    #[must_use]
    pub fn struck(&self) -> &[(String, f64, f64)] {
        &self.struck
    }

    /// The mass of the piece (the solids still joined together) that the
    /// largest solid of `instance` is in, kg.
    #[must_use]
    pub fn piece_mass(&self, instance: &str) -> f64 {
        let roots = self.pieces();
        let Some(k) = self
            .live()
            .filter(|&k| self.parts[k].instance == instance)
            .max_by(|&a, &b| self.parts[a].mass.total_cmp(&self.parts[b].mass))
        else {
            return 0.0;
        };
        self.live()
            .filter(|&o| roots[o] == roots[k])
            .map(|o| self.parts[o].mass)
            .sum()
    }

    /// The solids a strike broke into fragments.
    #[must_use]
    pub fn fragmented(&self) -> &[String] {
        &self.fragmented
    }

    /// The total impulse given to each solid from outside so far (blasts
    /// and strikes), by instance, N s.
    #[must_use]
    pub fn received(&self, instance: &str) -> DVec3 {
        (0..self.parts.len())
            .filter(|&k| self.parts[k].instance == instance)
            .map(|k| self.received[k])
            .sum()
    }

    /// The sum of every impulse given from outside (blasts, strikes and
    /// air drag), N s.
    #[must_use]
    pub fn ledger(&self) -> DVec3 {
        self.ledger
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

    fn give(&mut self, k: usize, impulse: DVec3, at: DVec3) {
        let id = self.bodies[k];
        let body = &mut self.world.bodies_mut()[id.0 as usize];
        let before = body.vel;
        body.wake();
        body.apply_impulse_at(impulse, at);
        let knock = (body.vel - before).length();
        self.received[k] += impulse;
        self.ledger += impulse;
        if knock > 0.05 && self.hardest.is_none_or(|(_, v)| knock > v) {
            self.hardest = Some((self.elapsed, knock));
        }
    }

    /// The area part `k` shows a blast arriving along `dir` (its box's
    /// cross-section), m².
    fn area(&self, k: usize, dir: DVec3) -> f64 {
        self.face(k, dir).0
    }

    /// The area part `k` shows a wave arriving along `dir` (its box's
    /// cross-section), m², and the smaller half side of the face it
    /// mostly shows, m (for the wave's clearing time).
    fn face(&self, k: usize, dir: DVec3) -> (f64, f64) {
        let local = (self.body(k).orientation.inverse() * dir).abs();
        let h = self.parts[k].half;
        let area = 4.0 * (h.y * h.z * local.x + h.x * h.z * local.y + h.x * h.y * local.z);
        let face = if local.x >= local.y && local.x >= local.z {
            h.y.min(h.z)
        } else if local.y >= local.z {
            h.x.min(h.z)
        } else {
            h.x.min(h.y)
        };
        (area, face)
    }

    /// Air drag on every part flying through still air this step:
    /// ½ ρ C_d A v², never more than stops it.
    fn drag(&mut self, h: f64) {
        let mut pushes = Vec::new();
        for k in self.live() {
            let b = self.body(k);
            let speed = b.vel.length();
            if speed < 0.5 {
                continue;
            }
            let along = b.vel / speed;
            let slow = (0.5 * blast::AIR_DENSITY * DRAG * self.area(k, along) * speed * speed * h
                / b.mass)
                .min(speed);
            pushes.push((k, -along * slow * b.mass));
        }
        for (k, impulse) in pushes {
            let body = &mut self.world.bodies_mut()[self.bodies[k].0 as usize];
            body.vel += impulse / body.mass;
            self.ledger += impulse;
        }
    }

    /// Where the segment from `from` along unit `dir` first enters part
    /// `k`'s box, as a distance, if within `length`.
    fn ray(&self, k: usize, from: DVec3, dir: DVec3, length: f64) -> Option<f64> {
        let s = &self.parts[k];
        let b = self.body(k);
        let o = b.orientation.inverse() * (from - b.pos) - s.center;
        let d = b.orientation.inverse() * dir;
        let (mut t0, mut t1) = (0.0f64, length);
        for axis in 0..3 {
            let (oa, da, ha) = (o[axis], d[axis], s.half[axis]);
            if da.abs() < 1e-12 {
                if oa.abs() > ha {
                    return None;
                }
                continue;
            }
            let (mut a, mut c) = ((-ha - oa) / da, (ha - oa) / da);
            if a > c {
                std::mem::swap(&mut a, &mut c);
            }
            t0 = t0.max(a);
            t1 = t1.min(c);
        }
        (t0 <= t1).then_some(t0)
    }

    /// A meteor landing at `at` whose blast reaches `radius` (the scene's
    /// damage radius). Its energy is the ground burst whose free-air
    /// equivalent charge ([`blast::charge_for`]) gives
    /// [`blast::DAMAGE_OVERPRESSURE`] at that radius: E = W × 4.184 MJ/kg
    /// ÷ [`blast::SURFACE`]; its mass is 2E/v² at [`IMPACT_SPEED`]. When
    /// its flight meets the rover it strikes the first solid there
    /// ([`Rover::strike`]), and the energy that solid took goes off where
    /// it hit; the rest goes off on the ground.
    pub fn blast(&mut self, at: Vec3, radius: f32) {
        self.felt.push((self.elapsed, at, radius));
        let charge = blast::charge_for(f64::from(radius));
        let horizontal = DVec3::new(
            f64::from(at.x) - self.caster.x,
            0.0,
            f64::from(at.z) - self.caster.z,
        )
        .normalize_or(DVec3::X);
        let flight = (horizontal * 0.55 + DVec3::NEG_Y * 0.83).normalize();
        let energy = charge * blast::TNT / blast::SURFACE;
        let mass = 2.0 * energy / (IMPACT_SPEED * IMPACT_SPEED);
        let at = at.as_dvec3();
        match self.hit(at, flight, mass, IMPACT_SPEED) {
            Some((_, point, deposited)) => {
                let share = (deposited / energy).clamp(0.0, 1.0);
                self.burst(point, charge * share);
                if share < 1.0 {
                    self.burst(at, charge * (1.0 - share));
                }
            }
            None => self.burst(at, charge),
        }
    }

    /// A charge of `charge` kg TNT going off at `at` now (for tests and
    /// for anything else that explodes near the rover).
    pub fn burst(&mut self, at: DVec3, charge: f64) {
        // The pulse has passed every solid once the last one's positive
        // phase is over.
        let pulse = self
            .live()
            .map(|k| {
                let b = self.body(k);
                let wave = blast::Wave::at((b.pos - at).length().max(0.05), charge);
                wave.arrival + wave.duration
            })
            .fold(0.0, f64::max);
        self.bursts.push(Burst {
            at,
            charge,
            start: self.elapsed,
            fine_until: self.elapsed + pulse.min(FINE_LONGEST) + STEP / FINE as f64,
        });
    }

    /// An impactor of `mass` kg at `speed` m/s along `flight` landing at
    /// `at`. It strikes the first solid its flight meets (within
    /// [`STRUCK`] of the landing), a perfectly inelastic collision over
    /// the step: the solid takes the impactor's momentum in the share of
    /// its mass, m/(m + mᵢ), and the kinetic energy lost, ½ mᵢ v² ×
    /// m/(m + mᵢ), goes into it. When that energy per kilogram of the
    /// solid passes the catastrophic-disruption threshold [`DISRUPTION`],
    /// the solid breaks into four fragments that share its momentum and
    /// fly apart with [`SPREAD`] of that energy. Returns the solid it
    /// struck.
    pub fn strike(&mut self, at: DVec3, flight: DVec3, mass: f64, speed: f64) -> Option<String> {
        self.hit(at, flight, mass, speed).map(|(name, _, _)| name)
    }

    /// [`Rover::strike`], with where it hit and the energy the solid took.
    fn hit(
        &mut self,
        at: DVec3,
        flight: DVec3,
        mass: f64,
        speed: f64,
    ) -> Option<(String, DVec3, f64)> {
        let flight = flight.normalize_or(DVec3::NEG_Y);
        let from = at - flight * 3.0;
        let first = self
            .live()
            .filter_map(|k| self.ray(k, from, flight, 3.0 + STRUCK).map(|t| (t, k)))
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        let (t, k) = first;
        let point = from + flight * t;
        let target = self.parts[k].mass;
        let share = target / (target + mass);
        let momentum = flight * mass * speed * share;
        let deposited = 0.5 * mass * speed * speed * share;
        let name = self.parts[k].instance.clone();
        self.events.push(Event {
            effect: "rover_sparks",
            at: point.as_vec3(),
        });
        self.struck
            .push((name.clone(), momentum.length(), deposited));
        if deposited / target >= DISRUPTION {
            self.fragment(k, momentum, deposited, point);
        } else {
            self.give(k, momentum, point);
        }
        Some((name, point, deposited))
    }

    /// Notes the solids that were in one piece `before` and are not now,
    /// and still overlap: they don't collide until clear.
    fn parted(&mut self, before: &[usize]) {
        let after = self.pieces();
        let n = before.len();
        for i in 0..n {
            for j in i + 1..n {
                if before[i] == before[j] && after[i] != after[j] && self.overlap(i, j) {
                    self.intimate.insert((i, j));
                }
            }
        }
    }

    /// Breaks part `k` into four fragments, by two planes through its
    /// centre across its two longest sides. Each takes a quarter of its
    /// mass and its triangles there and a quarter of `momentum`, and flies
    /// out from the centre with [`SPREAD`] of the `deposited` energy
    /// between them.
    fn fragment(&mut self, k: usize, momentum: DVec3, deposited: f64, point: DVec3) {
        let part = self.parts[k].clone();
        let (pos, orientation, vel, omega) = {
            let b = self.body(k);
            (b.pos, b.orientation, b.vel, b.omega)
        };
        // Its joints go with it.
        let before = self.pieces();
        let now = self.elapsed;
        for link in self
            .links
            .iter_mut()
            .filter(|l| l.broke.is_none() && (l.bodies.0 == k || l.bodies.1 == k))
        {
            link.broke = Some(now);
            link.peak_use = link.peak_use.max(1.0);
        }
        self.world.remove_body(self.bodies[k]);
        self.parts[k].gone = true;
        self.parted(&before);
        self.fragmented.push(part.instance.clone());
        let h = part.half;
        let mut axes = [0usize, 1, 2];
        axes.sort_by(|&a, &b| h[b].total_cmp(&h[a]));
        let (u, v) = (axes[0], axes[1]);
        let first = self.parts.len();
        let spread = (2.0 * SPREAD * deposited / part.mass).sqrt();
        for (su, sv) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let mut offset = DVec3::ZERO;
            offset[u] = su * h[u] / 2.0;
            offset[v] = sv * h[v] / 2.0;
            let mut half = h;
            half[u] /= 2.0;
            half[v] /= 2.0;
            let center = part.center + offset;
            let inside = |p: Vec3| {
                let d = (p - part.shift).as_dvec3() - part.center;
                d[u] * su >= 0.0 && d[v] * sv >= 0.0
            };
            let indices: Vec<u32> = part
                .indices
                .chunks_exact(3)
                .filter(|t| {
                    let c = (part.positions[t[0] as usize]
                        + part.positions[t[1] as usize]
                        + part.positions[t[2] as usize])
                        / 3.0;
                    inside(c)
                })
                .flatten()
                .copied()
                .collect();
            let mass = part.mass / 4.0;
            let inertia = Body::box_inertia(mass, half * 2.0);
            let mut body = Body::new(mass, inertia, pos + orientation * center);
            body.orientation = orientation;
            let mut out = DVec3::ZERO;
            out[u] = su;
            out[v] = sv;
            body.vel = vel + momentum / part.mass + orientation * out.normalize() * spread;
            body.omega = omega;
            let id = self.world.add(body);
            let collider = self.world.add_collider(
                Collider::new(id, Shape::Cuboid { half })
                    .with_filter(Filter {
                        group: FAST_BIT,
                        mask: GROUND_BIT,
                    })
                    .with_material(Material {
                        friction: 0.85,
                        torsional: 0.01,
                        restitution: 0.2,
                    }),
            );
            self.bodies.push(id);
            self.colliders.push(vec![collider]);
            self.fast.push(true);
            self.received.push(DVec3::ZERO);
            self.parts.push(Part {
                instance: part.instance.clone(),
                material: part.material.clone(),
                mass,
                center: DVec3::ZERO,
                shift: part.shift + center.as_vec3(),
                half,
                positions: part.positions.clone(),
                normals: part.normals.clone(),
                indices: std::sync::Arc::new(indices),
                color: part.color,
                gone: false,
            });
        }
        // The impactor's momentum is the fragments' (the ejecta's spread
        // sums to nothing).
        self.ledger += momentum;
        for f in first..self.parts.len() {
            self.received[f] = momentum / 4.0;
        }
        let knock = (momentum / part.mass).length() + spread;
        if self.hardest.is_none_or(|(_, v)| knock > v) {
            self.hardest = Some((self.elapsed, knock));
        }
        self.events.push(Event {
            effect: "rover_sparks",
            at: point.as_vec3(),
        });
        // Fragments overlap each other and their neighbours where they are
        // born: they collide only once clear.
        for f in first..self.parts.len() {
            for o in 0..self.parts.len() {
                if o != f && self.overlap(f, o) {
                    self.intimate.insert((o.min(f), o.max(f)));
                }
            }
        }
        self.regroup();
    }

    /// The blasts' push in this step on every part: each part's share of
    /// the Friedlander pulse at its distance, through the step, as a force.
    fn press(&mut self, h: f64) {
        let mut pushes: Vec<(usize, DVec3, DVec3)> = Vec::new();
        for burst in &self.bursts {
            for k in self.live() {
                let s = &self.parts[k];
                let b = self.body(k);
                let center = b.pos + b.orientation * s.center;
                let away = center - burst.at;
                let distance = away.length().max(0.05);
                let dir = away / distance;
                let wave = blast::Wave::at(distance, burst.charge);
                // The pulse's window this step, from its arrival.
                let t1 = self.elapsed + h - burst.start - wave.arrival;
                let t0 = t1 - h;
                if t1 <= 0.0 || t0 >= wave.duration {
                    continue;
                }
                let (area, face) = self.face(k, dir);
                let per_area = wave.net_impulse(t0, t1, wave.clearing(face), DRAG);
                if per_area <= 0.0 {
                    continue;
                }
                // A part behind another, seen from the burst, is shielded (not by
                // one the burst is inside or on: what it struck).
                let shielded = self.live().any(|o| {
                    o != k
                        && self
                            .ray(o, burst.at, dir, distance - 0.05)
                            .is_some_and(|t| t > 0.02)
                });
                let shade = if shielded { SHIELDED } else { 1.0 };
                let impulse = dir * per_area * area * shade;
                pushes.push((k, impulse, center));
            }
        }
        for (k, impulse, at) in pushes {
            self.give(k, impulse, at);
        }
        let now = self.elapsed;
        self.bursts.retain(|b| now - b.start < BURST_LIFE);
    }

    /// Advances `dt` seconds in fixed steps, breaking the joints that
    /// reached their limits and noting hard landings.
    pub fn step(&mut self, dt: f32) {
        self.carry += f64::from(dt.max(0.0));
        while self.carry >= STEP {
            self.carry -= STEP;
            self.advance(true);
        }
    }

    /// One step of [`STEP`]: in [`FINE`] substeps while a blast pulse is
    /// crossing the rover, so the joints feel its real peak, not its
    /// average over the step.
    fn advance(&mut self, gravity: bool) {
        if self.bursts.iter().any(|b| self.elapsed < b.fine_until) {
            let h = STEP / FINE as f64;
            // No position correction while the pulse passes (at most
            // `FINE_LONGEST`): correcting a joint's drift within 21 µs would
            // read as a load hundreds of times its real one. The velocity
            // constraints still hold every joint and contact.
            self.world.solver.erp = 0.0;
            for i in 0..FINE {
                let check = (i + 1) % WINDOW == 0 || i + 1 == FINE;
                let span = h
                    * (if (i + 1) % WINDOW == 0 {
                        WINDOW
                    } else {
                        FINE % WINDOW
                    }) as f64;
                self.tick(h, gravity, check.then_some(span));
            }
            self.world.solver.erp = self.erp;
        } else {
            self.tick(STEP, gravity, Some(STEP));
        }
    }

    /// One physics step of `h` s. When `check` is the time since the
    /// joints' loads were last checked, each joint's load is what it
    /// carried over that span, and a joint at its limit breaks.
    fn tick(&mut self, h: f64, gravity: bool, check: Option<f64>) {
        self.world.dt = h;
        self.press(h);
        self.drag(h);
        let g = if gravity { GRAVITY } else { 0.0 };
        let field = move |_: DVec3, _: DVec3| DVec3::new(0.0, -g, 0.0);
        self.world.step(&field);
        self.elapsed += h;
        for k in 0..self.links.len() {
            if self.links[k].broke.is_some() {
                continue;
            }
            let Some(joint) = self.world.joint(self.links[k].id) else {
                continue;
            };
            let q = self.body(self.links[k].bodies.0).orientation.inverse();
            let (linear, angular) = (q * joint.impulse, q * joint.angular_impulse);
            let carried = &mut self.links[k].carried;
            carried.0 += linear;
            carried.1 += angular;
        }
        let Some(span) = check else {
            return;
        };
        let mut snapped = false;
        let before = self.pieces();
        for k in 0..self.links.len() {
            if self.links[k].broke.is_some() {
                continue;
            }
            let (force, torque) = (
                self.links[k].carried.0 / span,
                self.links[k].carried.1 / span,
            );
            self.links[k].carried = (DVec3::ZERO, DVec3::ZERO);
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
        // Once a part has flown faster than `FAST` it meets only the ground
        // from then on: re-enabling its collisions with the other pieces as
        // it slows, wherever it has flown through them, throws it.
        let fast: Vec<bool> = (0..self.bodies.len())
            .map(|k| !self.parts[k].gone && (self.fast[k] || self.body(k).vel.length() > FAST))
            .collect();
        let mut changed = fast != self.fast;
        self.fast = fast;
        if snapped {
            self.parted(&before);
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
        let mut out = Vec::new();
        for k in self.live() {
            let s = &self.parts[k];
            let body = self.body(k);
            let rotation = body.orientation.as_quat();
            let pos = body.pos.as_vec3();
            for tri in s.indices.chunks_exact(3) {
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| i as usize);
                let [pa, pb, pc] = [a, b, c].map(|i| pos + rotation * (s.positions[i] - s.shift));
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

    /// Repaints the rover: each solid (and any fragment broken from it)
    /// takes the color `paint` gives for its instance and its CAD color,
    /// linear RGB. A livery for a film; the physics is untouched.
    pub fn repaint(&mut self, paint: impl Fn(&str, [f32; 3]) -> [f32; 3]) {
        for part in &mut self.parts {
            part.color = paint(&part.instance, part.color);
        }
    }

    /// How each body still in the world rests: its index, instance, its
    /// lowest point's height above the ground (m; negative is sunk), how
    /// fast it still moves up or down (m/s; a freed wheel may roll on
    /// level ground, but nothing settled falls or bounces), and whether
    /// something holds it up (the ground within [`REST_CONTACT`], or
    /// another body's box within that of its own).
    #[must_use]
    pub fn resting(&self) -> Vec<(usize, String, f64, f64, bool)> {
        let boxes: Vec<(usize, DVec3, DVec3)> = self
            .live()
            .map(|k| {
                let b = self.body(k);
                let p = &self.parts[k];
                let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
                for &i in p.indices.iter() {
                    let v = b.pos + b.orientation * (p.positions[i as usize] - p.shift).as_dvec3();
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
                (k, lo, hi)
            })
            .collect();
        boxes
            .iter()
            .map(|&(k, lo, hi)| {
                let height = lo.y - self.origin.y;
                let on_ground = height <= REST_CONTACT;
                let on_part = boxes.iter().any(|&(j, olo, ohi)| {
                    j != k
                        && lo.y <= ohi.y + REST_CONTACT
                        && lo.y >= olo.y - REST_CONTACT
                        && lo.x <= ohi.x + REST_CONTACT
                        && hi.x >= olo.x - REST_CONTACT
                        && lo.z <= ohi.z + REST_CONTACT
                        && hi.z >= olo.z - REST_CONTACT
                });
                let speed = self.body(k).vel.y.abs();
                (
                    k,
                    self.parts[k].instance.clone(),
                    height,
                    speed,
                    on_ground || on_part,
                )
            })
            .collect()
    }

    /// Each solid's color, linear RGB, with its instance.
    #[must_use]
    pub fn colors(&self) -> Vec<(String, [f32; 3])> {
        self.parts
            .iter()
            .map(|p| (p.instance.clone(), p.color))
            .collect()
    }

    /// Every blast the rover felt, in order: when (s since it was made),
    /// where the meteor landed, and the blast's damage radius (m).
    #[must_use]
    pub fn blasts(&self) -> &[(f64, Vec3, f32)] {
        &self.felt
    }

    /// The direction a meteor landing at `at` flies in: down at a slant,
    /// away from the caster (the flight [`Self::blast`] strikes with).
    #[must_use]
    pub fn flight(&self, at: Vec3) -> Vec3 {
        let horizontal = DVec3::new(
            f64::from(at.x) - self.caster.x,
            0.0,
            f64::from(at.z) - self.caster.z,
        )
        .normalize_or(DVec3::X);
        (horizontal * 0.55 + DVec3::NEG_Y * 0.83)
            .normalize()
            .as_vec3()
    }

    /// Every body still in the world (a whole solid, or a fragment broken
    /// from one): its index (stable for its life; fragments get new ones),
    /// its solid's instance, and its pose.
    #[must_use]
    pub fn bodies(&self) -> Vec<(usize, String, DVec3, DQuat)> {
        self.live()
            .map(|k| {
                let b = self.body(k);
                (k, self.parts[k].instance.clone(), b.pos, b.orientation)
            })
            .collect()
    }

    /// Body `k`'s triangles in its own frame (a vertex `p` is at
    /// `pos + rotation * p` in the world, with [`Self::bodies`]' pose):
    /// positions, normals, a triangle list, and its color (linear RGB).
    #[must_use]
    pub fn body_mesh(&self, k: usize) -> Option<(Vec<Vec3>, Vec<Vec3>, Vec<u32>, [f32; 3])> {
        let part = self.parts.get(k)?;
        let mut remap = std::collections::HashMap::new();
        let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
        for &i in part.indices.iter() {
            let next = remap.len() as u32;
            let j = *remap.entry(i).or_insert_with(|| {
                positions.push(part.positions[i as usize] - part.shift);
                normals.push(part.normals[i as usize]);
                next
            });
            indices.push(j);
        }
        Some((positions, normals, indices, part.color))
    }

    /// Each solid's pose: its instance, position and rotation.
    #[must_use]
    pub fn poses(&self) -> Vec<(String, DVec3, DQuat)> {
        self.live()
            .map(|k| {
                let b = self.body(k);
                (self.parts[k].instance.clone(), b.pos, b.orientation)
            })
            .collect()
    }

    /// Lifts the rover off the ground and turns gravity off, for a test of
    /// momentum: only its own joints and collisions act between its parts.
    #[cfg(test)]
    fn float(&mut self) {
        self.world.collider_mut(self.ground).filter = Filter { group: 0, mask: 0 };
        for c in 0..self.world.colliders().len() as u32 {
            if self
                .debris
                .contains(&self.world.colliders()[c as usize].body)
            {
                self.world.collider_mut(ColliderId(c)).filter = Filter { group: 0, mask: 0 };
            }
        }
    }

    #[cfg(test)]
    fn step_free(&mut self, steps: usize) {
        for _ in 0..steps {
            self.advance(false);
        }
    }
}

/// An sRGB hex color (`e8892f`) in linear RGB.
#[must_use]
pub fn linear(hex: &str) -> Option<[f32; 3]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let mut out = [0.0; 3];
    for (k, c) in out.iter_mut().enumerate() {
        let v = f32::from(u8::from_str_radix(hex.get(2 * k..2 * k + 2)?, 16).ok()?) / 255.0;
        *c = if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        };
    }
    Some(out)
}

/// A livery for [`Rover::repaint`]: each `KEY=RRGGBB` matches a solid by
/// its CAD color (an sRGB hex such as `e8892f`) or its instance name (such
/// as `chassis`) and gives its new color; an instance name wins.
#[derive(Clone, Debug, Default)]
pub struct Livery(Vec<(String, Option<[f32; 3]>, [f32; 3])>);

impl Livery {
    /// Parses `KEY=RRGGBB` specs.
    ///
    /// # Errors
    /// Returns a message for a spec that is not `KEY=RRGGBB`.
    pub fn parse(specs: &[String]) -> Result<Self, String> {
        specs
            .iter()
            .map(|spec| {
                let (key, to) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--paint {spec}: expected KEY=RRGGBB"))?;
                let to = linear(to).ok_or_else(|| format!("--paint {spec}: bad color"))?;
                Ok((key.to_owned(), linear(key), to))
            })
            .collect::<Result<_, String>>()
            .map(Self)
    }

    /// The color for `instance`, now `color`.
    #[must_use]
    pub fn paint(&self, instance: &str, color: [f32; 3]) -> [f32; 3] {
        if let Some((_, _, to)) = self.0.iter().find(|(key, _, _)| key == instance) {
            return *to;
        }
        let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 0.01);
        self.0
            .iter()
            .find(|(_, from, _)| from.is_some_and(|f| near(f, color)))
            .map_or(color, |(_, _, to)| *to)
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
    fn a_livery_matches_by_instance_before_color() {
        let livery = Livery::parse(&["e8892f=ff0000".into(), "chassis=00ff00".into()]).unwrap();
        let orange = linear("e8892f").unwrap();
        assert_eq!(livery.paint("rail_l", orange), [1.0, 0.0, 0.0]);
        assert_eq!(livery.paint("chassis", orange), [0.0, 1.0, 0.0]);
        assert_eq!(livery.paint("rail_l", [0.5; 3]), [0.5; 3]);
        assert!(Livery::parse(&["nonsense".into()]).is_err());
    }

    #[test]
    fn a_body_mesh_and_pose_rebuild_its_triangles_and_blasts_are_logged() {
        let mut r = rover();
        for (k, _, pos, rot) in r.bodies() {
            let (positions, _, indices, _) = r.body_mesh(k).unwrap();
            assert_eq!(indices.len() % 3, 0);
            assert!(indices.iter().all(|&i| (i as usize) < positions.len()));
            let world = pos.as_vec3() + rot.as_quat() * positions[0];
            assert!(world.distance(pos.as_vec3()) < 2.0);
        }
        r.blast(Vec3::new(-56.0, 0.0, 30.0), 5.2);
        assert_eq!(r.blasts().len(), 1);
        assert!(r.flight(Vec3::new(-56.0, 0.0, 30.0)).y < -0.5);
    }

    #[test]
    fn a_livery_repaints_the_solids_and_leaves_the_physics_alone() {
        let mut r = rover();
        let mass = r.mass();
        r.repaint(|instance, color| {
            if instance == "chassis" {
                [0.5, 0.02, 0.02]
            } else {
                color
            }
        });
        let colors = r.colors();
        assert!(
            colors
                .iter()
                .filter(|(i, _)| i == "chassis")
                .all(|(_, c)| *c == [0.5, 0.02, 0.02])
        );
        assert!(
            colors
                .iter()
                .any(|(i, c)| i != "chassis" && *c != [0.5, 0.02, 0.02])
        );
        assert_eq!(r.mass(), mass);
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

    /// The scene's meteor: the charge that flattens the houses out to the
    /// showcase's 5.2 m damage radius.
    fn charge() -> f64 {
        blast::charge_for(5.2)
    }

    /// Where the showcase's meteor on the rover lands: 0.25 m past its
    /// centre, away from the caster, so its flight crosses the deck.
    fn on(r: &Rover) -> DVec3 {
        let c = r
            .poses()
            .into_iter()
            .find(|p| p.0 == "chassis")
            .map_or(DVec3::ZERO, |p| p.1);
        let toward = DVec3::new(r.caster.x - c.x, 0.0, r.caster.z - c.z).normalize();
        DVec3::new(c.x, r.origin.y, c.z) - toward * 0.25
    }

    #[test]
    fn momentum_and_energy_add_up_through_a_blast_and_what_breaks_after() {
        let mut r = rover();
        r.float();
        let before = r.momentum();
        let at = on(&r);
        r.blast(at.as_vec3(), 5.2);
        r.step_free(240);
        assert!(!r.broken().is_empty());
        // The only outside pushes are the blast and the air's drag; the
        // joints and the parts' collisions are internal.
        let ledger = r.ledger();
        let after = r.momentum();
        assert!(
            (after - before - ledger).length() < 1e-6 * ledger.length().max(1.0),
            "{before} + {ledger} -> {after}"
        );
        // The parts carry off less energy than the meteor brought.
        let energy = r.kinetic_energy();
        assert!(
            energy > 0.0 && energy < charge() * blast::TNT / blast::SURFACE * 0.2,
            "{energy} J"
        );
    }

    #[test]
    fn a_direct_hit_shatters_what_it_strikes_and_conserves_momentum() {
        let mut r = rover();
        r.float();
        let before = r.momentum();
        let at = r.center().as_dvec3() + DVec3::new(0.0, 0.2, 0.0);
        let flight = DVec3::new(0.5, -0.85, 0.1).normalize();
        let energy = charge() * blast::TNT / blast::SURFACE;
        let mass = 2.0 * energy / (IMPACT_SPEED * IMPACT_SPEED);
        let hit = r.strike(at, flight, mass, IMPACT_SPEED).expect("it hit");
        assert_eq!(r.fragmented(), [hit.clone()]);
        let (_, momentum, deposited) = r.struck()[0].clone();
        // An inelastic hit: no more momentum or energy than the impactor's.
        assert!(momentum < mass * IMPACT_SPEED && deposited < energy);
        assert!((r.momentum() - before - flight * momentum).length() < 1e-6 * momentum);
        r.step_free(120);
        assert!((r.momentum() - before - r.ledger()).length() < 1e-6 * momentum);
        for l in r.joints() {
            assert_eq!(
                l.broke.is_some(),
                l.peak_use >= 1.0,
                "{} {:.2}",
                l.name,
                l.peak_use
            );
        }
    }

    #[test]
    fn after_the_scenes_meteor_every_part_comes_to_rest_on_something() {
        let mut r = settled();
        let at = on(&r);
        r.blast(at.as_vec3(), 5.2);
        // The chassis's fragments go some 50 m up and fall back; all of
        // it has landed and settled well within 30 s.
        for _ in 0..(30 * 60) {
            r.step(1.0 / 60.0);
        }
        for (k, name, height, speed, held) in r.resting() {
            assert!(
                height >= -REST_CONTACT,
                "{name} ({k}) sinks {height:.3} m into the ground"
            );
            assert!(
                held,
                "{name} ({k}) hangs {height:.3} m up with nothing under it"
            );
            assert!(
                speed < 0.05,
                "{name} ({k}) still falls or bounces at {speed:.3} m/s"
            );
        }
    }

    #[test]
    fn the_scenes_meteor_shatters_the_deck_and_tears_off_panels_dish_and_mast() {
        let mut r = settled();
        let at = on(&r);
        r.blast(at.as_vec3(), 5.2);
        for _ in 0..60 {
            r.step(1.0 / 60.0);
        }
        assert_eq!(r.fragmented(), ["chassis"]);
        // Each comes away on its own or with a few small parts: its piece
        // weighs a few kilograms, not the rover's 88.
        for part in [
            "solar_panel_l",
            "solar_panel_r",
            "antenna",
            "mast",
            "avionics_bay",
        ] {
            let held = r.piece_mass(part);
            assert!(
                held < 8.0,
                "{part} is in a piece of {held:.1} kg: {:?}",
                r.broken()
            );
        }
        let broke = |name: &str| {
            r.joints()
                .iter()
                .any(|l| l.name == name && l.broke.is_some())
        };
        assert!(broke("lid_hinge"), "{:?}", r.broken());
        for l in r.joints() {
            assert_eq!(
                l.broke.is_some(),
                l.peak_use >= 1.0,
                "{} {:.2}",
                l.name,
                l.peak_use
            );
        }
    }

    /// The doc's hand calculations (`docs/verse/meteor-showcase-handoff.md`):
    /// the impulse each blast gives the solar panel, the dish and the
    /// avionics bay, from the Kinney–Graham numbers at their distances with
    /// a triangular pulse, N s. The simulation integrates the Friedlander
    /// pulse; they agree within 20%.
    #[test]
    fn the_blast_loads_match_the_hand_calculations() {
        let hand = [
            ("solar_panel_l", [11.28, 10.51]),
            ("antenna", [124.7, 6.56]),
            ("avionics_bay", [79.05, 19.26]),
        ];
        for burst in 0..2 {
            let mut r = settled();
            // The scene's two bursts, on the deck where the meteor struck it
            // and on the ground, without the strike: the blast alone.
            let deck = DVec3::new(-55.92894, 0.49990, 7.95285);
            let ground = DVec3::new(-56.20319, 0.0, 8.13864);
            let charge = charge();
            let share = 0.878_659;
            if burst == 0 {
                r.burst(deck, charge * share);
            } else {
                r.burst(ground, charge * (1.0 - share));
            }
            for _ in 0..4 {
                r.advance(true);
            }
            for (part, loads) in hand {
                let got = r.received(part).length();
                let ratio = got / loads[burst];
                println!(
                    "{part}, burst {burst}: {got:.2} N s, {:.2} by hand",
                    loads[burst]
                );
                assert!(
                    (0.8..=1.2).contains(&ratio),
                    "{part}, burst {burst}: {got:.2} N s against {:.2} by hand",
                    loads[burst]
                );
            }
        }
    }

    /// The blast alone, with the deck not struck: the scene's meteor
    /// landing on the ground 1 m from the deck tears both wings off at
    /// their mounts; 4 m away they hold (the doc's hand calculation:
    /// the mounts' bending from the wing's inertia as the deck is thrown).
    #[test]
    fn the_blast_alone_tears_the_wings_off_close_by_and_not_far() {
        let wings = |d: f64| {
            let mut r = settled();
            let c = r
                .poses()
                .into_iter()
                .find(|p| p.0 == "chassis")
                .map_or(DVec3::ZERO, |p| p.1);
            let toward = DVec3::new(r.caster.x - c.x, 0.0, r.caster.z - c.z).normalize();
            r.burst(DVec3::new(c.x, r.origin.y, c.z) + toward * d, charge());
            for _ in 0..60 {
                r.step(1.0 / 60.0);
            }
            assert!(r.fragmented().is_empty());
            (r.piece_mass("solar_panel_l"), r.piece_mass("solar_panel_r"))
        };
        let (l, r) = wings(1.0);
        assert!(l < 5.0 && r < 5.0, "1 m: {l:.1} and {r:.1} kg");
        let (l, r) = wings(4.0);
        assert!(l > 50.0 && r > 50.0, "4 m: {l:.1} and {r:.1} kg");
    }

    #[test]
    fn a_weak_blast_breaks_nothing() {
        let mut r = settled();
        // 1 kPa side on at the rover: a charge whose 1 kPa ring is 20 m out.
        let charge = (20.0 / blast::scaled_distance_for(1_000.0)).powi(3);
        let at = r.center().as_dvec3() + DVec3::new(-20.0, -0.3, 0.0);
        r.burst(at, charge);
        for _ in 0..60 {
            r.step(1.0 / 60.0);
        }
        assert!(r.broken().is_empty(), "{:?}", r.broken());
        assert!(r.ledger().length() > 0.0);
    }

    #[test]
    fn the_same_blast_does_exactly_the_same_twice() {
        let run = || {
            let mut r = settled();
            let at = r.center() + Vec3::new(0.6, -0.4, 0.3);
            r.blast(at, 5.2);
            for _ in 0..120 {
                r.step(1.0 / 60.0);
            }
            (r.poses(), r.broken(), r.fragmented().to_vec())
        };
        let (poses, broken, fragments) = run();
        let again = run();
        assert_eq!(poses, again.0);
        assert_eq!(broken, again.1);
        assert_eq!(fragments, again.2);
    }
}
