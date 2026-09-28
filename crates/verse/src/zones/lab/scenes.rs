//! Physics Lab scenarios: small scenes on `physics::World`, one per mechanism.
//!
//! Nothing here draws or reads input. A scenario is built from its knob values
//! and stepped with a gravity flag and the same values, so a reset replays
//! bit for bit. Knobs marked live are read on every step; the others take
//! effect when the scenario rebuilds.

use glam::{DQuat, DVec3};
use physics::{
    Body, BodyId, BodyKind, Collider, ContactPoint, Filter, Joint, JointId, JointKind, Ledger,
    LedgerError, Material, NoField, Pid, Shape, ThrusterSet, Uniform, World,
};

/// Fixed step length, s.
pub(crate) const DT: f64 = 1.0 / 120.0;
/// Standard gravity, m/s^2.
pub(crate) const G: f64 = 9.81;
/// Half the stage floor's width, m.
pub(crate) const FLOOR_HALF: f64 = 3.5;

/// One knob: a closed list of options, so every value is reproducible.
#[derive(Clone, Copy)]
pub(crate) struct KnobDef {
    pub id: &'static str,
    pub label: &'static str,
    pub options: &'static [f64],
    pub default: usize,
    /// Read on every step; otherwise a change rebuilds the scenario.
    pub live: bool,
    pub format: fn(f64) -> String,
}

fn plain(v: f64) -> String {
    format!("{v}")
}
fn times_mu_n(v: f64) -> String {
    format!("{v:.2} × µN")
}
fn newton_metres(v: f64) -> String {
    format!("{v:.2} N·m")
}
fn metres(v: f64) -> String {
    format!("{v} m")
}
fn metres_per_second(v: f64) -> String {
    format!("{v} m/s")
}
fn radians_per_second(v: f64) -> String {
    format!("{v} rad/s")
}
fn degrees(v: f64) -> String {
    format!("{v}°")
}
fn millimetres(v: f64) -> String {
    if v < 0.0 {
        format!("{:.0} mm apart", -v)
    } else {
        format!("{v} mm")
    }
}
fn fraction(v: f64) -> String {
    format!("{:.0}% width", v * 100.0)
}
fn hertz(v: f64) -> String {
    format!("{v} Hz")
}
fn newtons(v: f64) -> String {
    if v.is_finite() {
        format!("{v} N")
    } else {
        "unlimited".into()
    }
}
fn sweep(v: f64) -> String {
    if v == 0.0 { "automatic" } else { "manual" }.into()
}
fn stack_mode(v: f64) -> String {
    if v == 0.0 { "stack" } else { "pile" }.into()
}
fn weld_mode(v: f64) -> String {
    if v == 0.0 { "hard" } else { "soft" }.into()
}
fn on_off(v: f64) -> String {
    if v == 0.0 { "off" } else { "on" }.into()
}
fn start_mode(v: f64) -> String {
    if v == 0.0 { "still" } else { "tumbling" }.into()
}
fn command_name(v: f64) -> String {
    COMMANDS[v as usize].into()
}

const COMMANDS: [&str; 5] = ["hold", "right 1 m", "up 0.5 m", "yaw 90°", "square"];

const MANIFOLD_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "sweep",
        label: "Sweep",
        options: &[0.0, 1.0],
        default: 0,
        live: true,
        format: sweep,
    },
    KnobDef {
        id: "tilt",
        label: "Tilt",
        options: &[0.0, 2.0, 5.0, 10.0, 20.0, 30.0],
        default: 2,
        live: true,
        format: degrees,
    },
    KnobDef {
        id: "yaw",
        label: "Yaw",
        options: &[0.0, 15.0, 30.0, 45.0],
        default: 0,
        live: true,
        format: degrees,
    },
    KnobDef {
        id: "depth",
        label: "Penetration",
        options: &[-50.0, 1.0, 10.0, 30.0, 60.0],
        default: 1,
        live: true,
        format: millimetres,
    },
    KnobDef {
        id: "slide",
        label: "Slide",
        options: &[0.0, 0.25, 0.5, 0.75, 0.9],
        default: 0,
        live: true,
        format: fraction,
    },
];
const FRICTION_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "load",
        label: "Load",
        options: &[0.0, 0.25, 0.5, 0.75, 0.9, 1.1, 1.2, 1.5],
        default: 2,
        live: true,
        format: times_mu_n,
    },
    KnobDef {
        id: "friction",
        label: "Friction µ",
        options: &[0.2, 0.4, 0.6, 0.8, 1.0],
        default: 2,
        live: false,
        format: plain,
    },
];
const TORSION_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "torque",
        label: "Torque",
        options: &[0.1, 0.25, 0.45, 0.55, 1.0],
        default: 2,
        live: true,
        format: newton_metres,
    },
    KnobDef {
        id: "torsional",
        label: "Torsion coef",
        options: &[0.0, 0.01, 0.02, 0.05, 0.1],
        default: 3,
        live: false,
        format: metres,
    },
];
const TUNNEL_KNOBS: &[KnobDef] = &[KnobDef {
    id: "speed",
    label: "Speed",
    options: &[2.0, 10.0, 40.0, 100.0, 200.0],
    default: 2,
    live: false,
    format: metres_per_second,
}];
const MOMENTUM_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "speed",
        label: "Cube speed",
        options: &[1.0, 2.0, 4.0, 8.0],
        default: 2,
        live: false,
        format: metres_per_second,
    },
    KnobDef {
        id: "spin",
        label: "Cube spin",
        options: &[0.0, 1.0, 3.0],
        default: 1,
        live: false,
        format: radians_per_second,
    },
];
const STACK_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "mode",
        label: "Mode",
        options: &[0.0, 1.0],
        default: 0,
        live: false,
        format: stack_mode,
    },
    KnobDef {
        id: "count",
        label: "Boxes",
        options: &[1.0, 3.0, 5.0, 8.0, 12.0],
        default: 2,
        live: false,
        format: plain,
    },
    KnobDef {
        id: "restitution",
        label: "Restitution",
        options: &[0.0, 0.2, 0.5, 0.8],
        default: 0,
        live: false,
        format: plain,
    },
    KnobDef {
        id: "friction",
        label: "Friction µ",
        options: &[0.2, 0.5, 0.8, 1.0],
        default: 1,
        live: false,
        format: plain,
    },
    KnobDef {
        id: "iterations",
        label: "Iterations",
        options: &[4.0, 10.0, 20.0, 40.0],
        default: 2,
        live: true,
        format: plain,
    },
    KnobDef {
        id: "sleep",
        label: "Island sleep",
        options: &[0.0, 1.0],
        default: 1,
        live: true,
        format: on_off,
    },
];
const GRIP_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "frequency",
        label: "Frequency",
        options: &[0.5, 1.0, 2.0, 4.0, 8.0],
        default: 2,
        live: true,
        format: hertz,
    },
    KnobDef {
        id: "damping",
        label: "Damping",
        options: &[0.1, 0.3, 0.7, 1.0, 2.0],
        default: 3,
        live: true,
        format: plain,
    },
    KnobDef {
        id: "limit",
        label: "Force limit",
        options: &[20.0, 40.0, 60.0, 100.0, 300.0, f64::INFINITY],
        default: 4,
        live: true,
        format: newtons,
    },
];
const TETHER_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "length",
        label: "Tether",
        options: &[0.6, 1.0, 1.4],
        default: 1,
        live: false,
        format: metres,
    },
    KnobDef {
        id: "impact",
        label: "Impact",
        options: &[3.0, 6.0, 12.0],
        default: 1,
        live: false,
        format: metres_per_second,
    },
    KnobDef {
        id: "weld",
        label: "Weld",
        options: &[0.0, 1.0],
        default: 0,
        live: false,
        format: weld_mode,
    },
];
const THRUSTER_KNOBS: &[KnobDef] = &[
    KnobDef {
        id: "command",
        label: "Command",
        options: &[0.0, 1.0, 2.0, 3.0, 4.0],
        default: 4,
        live: true,
        format: command_name,
    },
    KnobDef {
        id: "start",
        label: "Start",
        options: &[0.0, 1.0],
        default: 1,
        live: false,
        format: start_mode,
    },
];

/// The nine scenarios, in menu order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Manifold,
    Friction,
    Torsion,
    Tunneling,
    Momentum,
    Stack,
    SoftGrip,
    Tether,
    Thrusters,
}

impl Kind {
    pub const ALL: [Self; 9] = [
        Self::Manifold,
        Self::Friction,
        Self::Torsion,
        Self::Tunneling,
        Self::Momentum,
        Self::Stack,
        Self::SoftGrip,
        Self::Tether,
        Self::Thrusters,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Manifold => "Box-on-box manifold",
            Self::Friction => "Friction breakaway",
            Self::Torsion => "Torsional friction",
            Self::Tunneling => "Tunneling",
            Self::Momentum => "Zero-g momentum",
            Self::Stack => "Stack and pile",
            Self::SoftGrip => "Soft grip",
            Self::Tether => "Tether and weld",
            Self::Thrusters => "Thrusters",
        }
    }

    /// Stage lettering: A–Z, 0–9, and spaces only.
    pub const fn sign(self) -> &'static str {
        match self {
            Self::Manifold => "CONTACT MANIFOLD",
            Self::Friction => "FRICTION BREAKAWAY",
            Self::Torsion => "TORSIONAL FRICTION",
            Self::Tunneling => "TUNNELING",
            Self::Momentum => "ZERO G MOMENTUM",
            Self::Stack => "STACK AND PILE",
            Self::SoftGrip => "SOFT GRIP",
            Self::Tether => "TETHER AND WELD",
            Self::Thrusters => "THRUSTERS",
        }
    }

    /// Whether the scenario starts under uniform gravity.
    pub const fn gravity(self) -> bool {
        !matches!(self, Self::Tunneling | Self::Momentum | Self::Thrusters)
    }

    pub(crate) const fn knobs(self) -> &'static [KnobDef] {
        match self {
            Self::Manifold => MANIFOLD_KNOBS,
            Self::Friction => FRICTION_KNOBS,
            Self::Torsion => TORSION_KNOBS,
            Self::Tunneling => TUNNEL_KNOBS,
            Self::Momentum => MOMENTUM_KNOBS,
            Self::Stack => STACK_KNOBS,
            Self::SoftGrip => GRIP_KNOBS,
            Self::Tether => TETHER_KNOBS,
            Self::Thrusters => THRUSTER_KNOBS,
        }
    }

    /// The default option index of every knob.
    pub fn defaults(self) -> Vec<usize> {
        self.knobs().iter().map(|k| k.default).collect()
    }
}

/// Scenario-specific bodies and bookkeeping.
pub(crate) enum Rig {
    Manifold {
        top: BodyId,
        /// Tilt and yaw, rad; penetration and slide, m.
        pose: [f64; 4],
        points: Vec<ContactPoint>,
    },
    Friction {
        block: BodyId,
        /// Where the load started, after settling.
        start: Option<DVec3>,
        /// Tangential load this step, N.
        load: f64,
    },
    Torsion {
        ball: BodyId,
        /// Torque this step, N m.
        torque: f64,
        start: Option<DQuat>,
    },
    Tunneling {
        shots: Vec<BodyId>,
        trails: Vec<Vec<DVec3>>,
        period: f64,
    },
    Momentum {
        ledger: Ledger,
        origin: DVec3,
        error: LedgerError,
    },
    Stack {
        boxes: Vec<BodyId>,
    },
    SoftGrip {
        hand: BodyId,
        part: BodyId,
        joint: JointId,
    },
    Tether {
        pivot: DVec3,
        bob: BodyId,
        anchor: DVec3,
        tether: JointId,
        length: f64,
        pair: [BodyId; 2],
        weld: JointId,
        ball: BodyId,
        speed: f64,
        launched: bool,
    },
    Thrusters {
        craft: BodyId,
        set: ThrusterSet,
        throttles: Vec<f64>,
        position: Pid,
        attitude: Pid,
        home: DVec3,
        /// Where the controller is steering, and its heading, rad.
        target: (DVec3, f64),
    },
}

/// A built scenario: its world, rig, and elapsed time.
pub(crate) struct Scene {
    pub kind: Kind,
    pub world: World,
    pub rig: Rig,
    /// Simulated time since the last build, s.
    pub time: f64,
    /// How many times the scenario looped back to its start on its own.
    pub loops: u32,
}

fn value(kind: Kind, params: &[usize], id: &str) -> f64 {
    let (def, index) = kind
        .knobs()
        .iter()
        .zip(params)
        .find(|(k, _)| k.id == id)
        .expect("every scenario reads only its own knobs");
    def.options[(*index).min(def.options.len() - 1)]
}

fn floor(world: &mut World, material: Material) -> BodyId {
    let id = world
        .add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static));
    world.add_collider(
        Collider::new(
            id,
            Shape::Cuboid {
                half: DVec3::new(FLOOR_HALF, 0.5, FLOOR_HALF),
            },
        )
        .with_material(material),
    );
    id
}

fn cuboid(world: &mut World, mass: f64, half: DVec3, pos: DVec3, material: Material) -> BodyId {
    let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), pos));
    world.add_collider(Collider::new(id, Shape::Cuboid { half }).with_material(material));
    id
}

fn sphere(world: &mut World, mass: f64, radius: f64, pos: DVec3, material: Material) -> BodyId {
    let id = world.add(Body::new(
        mass,
        DVec3::splat(0.4 * mass * radius * radius),
        pos,
    ));
    world.add_collider(Collider::new(id, Shape::Sphere { radius }).with_material(material));
    id
}

fn fixed(world: &mut World, pos: DVec3) -> BodyId {
    world.add(Body::new(1.0, DVec3::ONE, pos).with_kind(BodyKind::Static))
}

fn step_world(world: &mut World, gravity: bool) {
    if gravity {
        world.step(&Uniform(DVec3::new(0.0, -G, 0.0)));
    } else {
        world.step(&NoField);
    }
}

/// A deterministic value in [0, 1) from an integer seed.
fn hash01(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x5eed_1ab5;
    x ^= x >> 31;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// World rotation vector that turns `from` into `to`, rad.
fn rotation_error(from: DQuat, to: DQuat) -> DVec3 {
    let mut q = to * from.inverse();
    if q.w < 0.0 {
        q = -q;
    }
    let (axis, angle) = q.to_axis_angle();
    if angle.is_finite() {
        axis * angle
    } else {
        DVec3::ZERO
    }
}

impl Scene {
    /// Build `kind` from knob option indexes.
    pub fn new(kind: Kind, params: &[usize]) -> Self {
        let mut world = World::new(DT);
        let v = |id| value(kind, params, id);
        let rig = match kind {
            Kind::Manifold => {
                let base = world.add(
                    Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 0.5, 0.0))
                        .with_kind(BodyKind::Static),
                );
                world.add_collider(Collider::new(
                    base,
                    Shape::Cuboid {
                        half: DVec3::splat(0.5),
                    },
                ));
                // Detection needs one side that can respond; the pose is
                // scripted and the world never steps, so a unit dynamic body
                // stands in for a kinematic one.
                let top = world.add(Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 1.5, 0.0)));
                world.add_collider(Collider::new(
                    top,
                    Shape::Cuboid {
                        half: DVec3::splat(0.5),
                    },
                ));
                Rig::Manifold {
                    top,
                    pose: [0.0; 4],
                    points: Vec::new(),
                }
            }
            Kind::Friction => {
                let material = Material {
                    friction: v("friction"),
                    torsional: 0.0,
                    restitution: 0.0,
                };
                floor(&mut world, material);
                let block = cuboid(
                    &mut world,
                    2.0,
                    DVec3::splat(0.3),
                    DVec3::new(-2.8, 0.3, 0.0),
                    material,
                );
                Rig::Friction {
                    block,
                    start: None,
                    load: 0.0,
                }
            }
            Kind::Torsion => {
                let material = Material {
                    friction: 1.0,
                    torsional: v("torsional"),
                    restitution: 0.0,
                };
                floor(&mut world, material);
                let ball = sphere(&mut world, 1.0, 0.3, DVec3::new(0.0, 0.3, 0.0), material);
                Rig::Torsion {
                    ball,
                    torque: 0.0,
                    start: None,
                }
            }
            Kind::Tunneling => {
                let speed = v("speed");
                let panel = fixed(&mut world, DVec3::new(0.0, 1.2, 0.0));
                world.add_collider(Collider::new(
                    panel,
                    Shape::Cuboid {
                        half: DVec3::new(0.02, 1.0, 1.5),
                    },
                ));
                let shapes = [
                    (Shape::Sphere { radius: 0.12 }, -0.9, DQuat::IDENTITY),
                    (
                        Shape::Cuboid {
                            half: DVec3::splat(0.12),
                        },
                        0.0,
                        DQuat::IDENTITY,
                    ),
                    (
                        Shape::Capsule {
                            radius: 0.08,
                            half_length: 0.2,
                        },
                        0.9,
                        DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2),
                    ),
                ];
                let shots = shapes
                    .iter()
                    .map(|(shape, z, rotation)| {
                        let mut body =
                            Body::new(1.0, DVec3::splat(0.01), DVec3::new(-3.0, 1.2, *z));
                        body.orientation = *rotation;
                        body.prev_orientation = *rotation;
                        body.vel = DVec3::X * speed;
                        let id = world.add(body);
                        world.add_collider(Collider::new(id, *shape));
                        id
                    })
                    .collect::<Vec<_>>();
                Rig::Tunneling {
                    trails: vec![Vec::new(); shots.len()],
                    shots,
                    period: (3.0 / speed + 1.0).max(1.6),
                }
            }
            Kind::Momentum => {
                let lift = DVec3::new(-0.4, 1.4, 0.0);
                let mut cube = Body::new(
                    10.0,
                    Body::box_inertia(10.0, DVec3::splat(0.4)),
                    DVec3::new(-1.6, 0.1, 0.0) + lift,
                );
                cube.vel = DVec3::X * v("speed");
                cube.omega = DVec3::new(0.0, 0.3, 1.0) * v("spin");
                let cube = world.add(cube);
                world.add_collider(Collider::new(
                    cube,
                    Shape::Cuboid {
                        half: DVec3::splat(0.2),
                    },
                ));
                let tank = world.add(Body::new(
                    30.0,
                    Body::shell_inertia(30.0, 0.3, 1.2),
                    DVec3::new(0.5, 0.0, 0.2) + lift,
                ));
                world.add_collider(Collider::new(
                    tank,
                    Shape::Capsule {
                        radius: 0.3,
                        half_length: 0.3,
                    },
                ));
                let truss = world.add(Body::new(
                    15.0,
                    Body::box_inertia(15.0, DVec3::new(0.3, 0.3, 2.0)),
                    DVec3::new(1.4, 0.0, 0.0) + lift,
                ));
                world.add_collider(Collider::new(
                    truss,
                    Shape::Cuboid {
                        half: DVec3::new(0.15, 0.15, 1.0),
                    },
                ));
                let origin = DVec3::new(0.0, 1.4, 0.0);
                Rig::Momentum {
                    ledger: Ledger::new(origin, world.momentum(origin)),
                    origin,
                    error: LedgerError {
                        linear: 0.0,
                        angular: 0.0,
                    },
                }
            }
            Kind::Stack => {
                let material = Material {
                    friction: v("friction"),
                    torsional: 0.0,
                    restitution: v("restitution"),
                };
                floor(&mut world, material);
                let half = DVec3::splat(0.2);
                let count = v("count") as usize;
                let pile = v("mode") != 0.0;
                let boxes = (0..count)
                    .map(|i| {
                        let seed = i as u64;
                        let (pos, orientation) = if pile {
                            (
                                DVec3::new(
                                    (hash01(seed * 3) - 0.5) * 1.2,
                                    0.8 + 0.5 * i as f64,
                                    (hash01(seed * 3 + 1) - 0.5) * 1.2,
                                ),
                                DQuat::from_rotation_y(hash01(seed * 3 + 2) * 6.0)
                                    * DQuat::from_rotation_x(0.4),
                            )
                        } else {
                            (
                                DVec3::new(0.0, 0.2 + 0.401 * i as f64, 0.0),
                                DQuat::IDENTITY,
                            )
                        };
                        let id = cuboid(&mut world, 1.0, half, pos, material);
                        world[id].orientation = orientation;
                        world[id].prev_orientation = orientation;
                        id
                    })
                    .collect();
                Rig::Stack { boxes }
            }
            Kind::SoftGrip => {
                floor(&mut world, Material::default());
                let hand = world.add(
                    Body::new(1.0, DVec3::ONE, DVec3::new(0.0, 2.4, 0.0))
                        .with_kind(BodyKind::Kinematic),
                );
                // The hand is drawn from this collider but touches nothing.
                world.add_collider(
                    Collider::new(
                        hand,
                        Shape::Cuboid {
                            half: DVec3::new(0.3, 0.1, 0.3),
                        },
                    )
                    .with_filter(Filter::NONE),
                );
                let rest = DVec3::new(0.0, 2.4 - 0.1 - 0.3, 0.0);
                let part = cuboid(
                    &mut world,
                    5.0,
                    DVec3::splat(0.3),
                    rest + DVec3::new(0.35, -0.25, 0.0),
                    Material::default(),
                );
                let tipped = DQuat::from_rotation_z(0.45);
                world[part].orientation = tipped;
                world[part].prev_orientation = tipped;
                let joint = world.add_joint(Joint::new(
                    hand,
                    DVec3::new(0.0, -0.1, 0.0),
                    part,
                    DVec3::new(0.0, 0.3, 0.0),
                    JointKind::Weld {
                        relative: DQuat::IDENTITY,
                    },
                ));
                Rig::SoftGrip { hand, part, joint }
            }
            Kind::Tether => {
                floor(&mut world, Material::default());
                // Pendulum: a rigid bob pinned 1.2 m below a fixed pivot.
                let pivot = DVec3::new(-2.0, 2.8, 0.0);
                let pivot_body = fixed(&mut world, pivot);
                let arm = 1.2;
                let swing = DQuat::from_rotation_z(1.0);
                let mut bob = Body::new(
                    1.0,
                    DVec3::splat(0.4 * 0.15 * 0.15),
                    pivot - swing * DVec3::new(0.0, arm, 0.0),
                );
                bob.orientation = swing;
                bob.prev_orientation = swing;
                let bob = world.add(bob);
                world.add_collider(Collider::new(bob, Shape::Sphere { radius: 0.15 }));
                world.add_joint(Joint::new(
                    pivot_body,
                    DVec3::ZERO,
                    bob,
                    DVec3::new(0.0, arm, 0.0),
                    JointKind::Point,
                ));
                // Tether: a box dropped with slack is caught when the line
                // goes taut.
                let length = v("length");
                let anchor = DVec3::new(0.0, 3.0, 0.0);
                let anchor_body = fixed(&mut world, anchor);
                let hanging = cuboid(
                    &mut world,
                    2.0,
                    DVec3::splat(0.15),
                    anchor + DVec3::new(0.35, -0.3, 0.0),
                    Material::default(),
                );
                let tether = world.add_joint(Joint::new(
                    anchor_body,
                    DVec3::ZERO,
                    hanging,
                    DVec3::new(0.0, 0.15, 0.0),
                    JointKind::Tether { length },
                ));
                // Weld: an inverted T of two boxes that must move as one when
                // a ball strikes its upright.
                let (foot_filter, post_filter, ball_filter) = (
                    Filter {
                        group: 0b0010,
                        mask: 0b1001,
                    },
                    Filter {
                        group: 0b0100,
                        mask: 0b1001,
                    },
                    Filter {
                        group: 0b1000,
                        mask: u32::MAX,
                    },
                );
                let foot = world.add(Body::new(
                    4.0,
                    Body::box_inertia(4.0, DVec3::new(0.8, 0.3, 0.3)),
                    DVec3::new(1.9, 0.15, 0.0),
                ));
                world.add_collider(
                    Collider::new(
                        foot,
                        Shape::Cuboid {
                            half: DVec3::new(0.4, 0.15, 0.15),
                        },
                    )
                    .with_filter(foot_filter),
                );
                let post = world.add(Body::new(
                    3.0,
                    Body::box_inertia(3.0, DVec3::new(0.3, 0.8, 0.3)),
                    DVec3::new(1.9, 0.7, 0.0),
                ));
                world.add_collider(
                    Collider::new(
                        post,
                        Shape::Cuboid {
                            half: DVec3::new(0.15, 0.4, 0.15),
                        },
                    )
                    .with_filter(post_filter),
                );
                let mut weld = Joint::weld_here(&world, foot, post, DVec3::new(1.9, 0.3, 0.0));
                if v("weld") != 0.0 {
                    weld = weld.soft(std::f64::consts::TAU * 3.0, 0.7);
                }
                let weld = world.add_joint(weld);
                let ball = world.add(
                    Body::new(
                        3.0,
                        DVec3::splat(0.4 * 3.0 * 0.15 * 0.15),
                        DVec3::new(3.3, 0.8, 0.0),
                    )
                    .with_kind(BodyKind::Kinematic),
                );
                world.add_collider(
                    Collider::new(ball, Shape::Sphere { radius: 0.15 }).with_filter(ball_filter),
                );
                Rig::Tether {
                    pivot,
                    bob,
                    anchor,
                    tether,
                    length,
                    pair: [foot, post],
                    weld,
                    ball,
                    speed: v("impact"),
                    launched: false,
                }
            }
            Kind::Thrusters => {
                let half = DVec3::new(0.3, 0.4, 0.3);
                let home = DVec3::new(0.0, 1.6, 0.0);
                let mut craft = Body::new(6.0, Body::box_inertia(6.0, half * 2.0), home);
                if v("start") != 0.0 {
                    craft.omega = DVec3::new(0.8, 1.4, -0.6);
                    craft.orientation = DQuat::from_rotation_x(0.5) * DQuat::from_rotation_y(0.7);
                    craft.prev_orientation = craft.orientation;
                }
                let craft = world.add(craft);
                world.add_collider(Collider::new(craft, Shape::Cuboid { half }));
                Rig::Thrusters {
                    craft,
                    set: ThrusterSet::box_corners(half, 25.0),
                    throttles: vec![0.0; 24],
                    position: Pid::new(4.0, 0.4, 4.0),
                    // No integral: it winds up while recovering from a
                    // tumble.
                    attitude: Pid::new(64.0, 0.0, 16.0),
                    home,
                    target: (home, 0.0),
                }
            }
        };
        Self {
            kind,
            world,
            rig,
            time: 0.0,
            loops: 0,
        }
    }

    /// Advance one fixed step.
    pub fn step(&mut self, gravity: bool, params: &[usize]) {
        let kind = self.kind;
        let v = |id| value(kind, params, id);
        let t = self.time;
        let g = if gravity { G } else { 0.0 };
        let mut restart = false;
        match &mut self.rig {
            Rig::Manifold { top, pose, points } => {
                *pose = if v("sweep") == 0.0 {
                    auto_sweep(t)
                } else {
                    [
                        v("tilt").to_radians(),
                        v("yaw").to_radians(),
                        v("depth") / 1000.0,
                        v("slide"),
                    ]
                };
                let [tilt, yaw, depth, slide] = *pose;
                let q = DQuat::from_rotation_y(yaw) * DQuat::from_rotation_z(tilt);
                let lowest = (0..8)
                    .map(|i| {
                        let c = DVec3::new(
                            if i & 1 == 0 { -0.5 } else { 0.5 },
                            if i & 2 == 0 { -0.5 } else { 0.5 },
                            if i & 4 == 0 { -0.5 } else { 0.5 },
                        );
                        (q * c).y
                    })
                    .fold(f64::INFINITY, f64::min);
                let body = &mut self.world[*top];
                body.prev_pos = body.pos;
                body.prev_orientation = body.orientation;
                body.orientation = q;
                body.pos = DVec3::new(slide, 1.0 - depth - lowest, 0.0);
                *points = self
                    .world
                    .detect(&|_, _| 0.0)
                    .into_iter()
                    .flat_map(|m| m.points)
                    .collect();
                self.world.tick += 1;
            }
            Rig::Friction { block, start, load } => {
                let settle = 0.5;
                if t >= settle {
                    let mu = v("friction");
                    *load = v("load") * mu * self.world[*block].mass * G;
                    self.world[*block].apply_force(DVec3::X * *load);
                    if start.is_none() {
                        *start = Some(self.world[*block].pos);
                    }
                }
                step_world(&mut self.world, gravity);
                restart =
                    self.world[*block].pos.x > FLOOR_HALF - 0.6 || self.world[*block].pos.y < -2.0;
            }
            Rig::Torsion {
                ball,
                torque,
                start,
            } => {
                if t >= 0.5 {
                    *torque = v("torque");
                    self.world[*ball].apply_torque(DVec3::Y * *torque);
                    if start.is_none() {
                        *start = Some(self.world[*ball].orientation);
                    }
                }
                step_world(&mut self.world, gravity);
                restart = self.world[*ball].omega_world().length() > 40.0;
            }
            Rig::Tunneling {
                shots,
                trails,
                period,
            } => {
                step_world(&mut self.world, gravity);
                for (id, trail) in shots.iter().zip(trails.iter_mut()) {
                    trail.push(self.world[*id].pos);
                    if trail.len() > 90 {
                        trail.remove(0);
                    }
                }
                restart = t >= *period;
            }
            Rig::Momentum {
                ledger,
                origin,
                error,
            } => {
                // Gravity is an external impulse on the system; record it so
                // the ledger stays balanced when it is on.
                for body in self.world.bodies() {
                    if body.kind == BodyKind::Dynamic && g != 0.0 {
                        ledger.add_impulse(
                            "gravity",
                            DVec3::new(0.0, -g, 0.0) * body.mass * DT,
                            body.pos,
                        );
                    }
                }
                step_world(&mut self.world, gravity);
                *error = ledger.error(self.world.momentum(*origin));
                restart = t >= 7.0;
            }
            Rig::Stack { .. } => {
                self.world.solver.iterations = v("iterations") as u32;
                let sleep = v("sleep") != 0.0;
                if self.world.sleep.enabled && !sleep {
                    for i in 0..self.world.bodies().len() {
                        self.world.wake(BodyId(i as u32));
                    }
                }
                self.world.sleep.enabled = sleep;
                step_world(&mut self.world, gravity);
            }
            Rig::SoftGrip { hand, joint, .. } => {
                // The hand holds still for two seconds, then sways.
                let omega = std::f64::consts::TAU / 4.0;
                let sway = if t >= 2.0 {
                    0.7 * omega * (omega * (t - 2.0)).sin()
                } else {
                    0.0
                };
                self.world[*hand].vel = DVec3::X * sway;
                if let Some(joint) = self.world.joint_mut(*joint) {
                    joint.spring = Some(physics::Spring {
                        frequency: std::f64::consts::TAU * v("frequency"),
                        damping_ratio: v("damping"),
                    });
                    joint.max_force = v("limit");
                    joint.max_torque = v("limit") * 0.5;
                }
                step_world(&mut self.world, gravity);
            }
            Rig::Tether {
                ball,
                speed,
                launched,
                pair,
                ..
            } => {
                if !*launched && t >= 1.5 {
                    // Aim a ballistic shot at the upright's middle.
                    let from = self.world[*ball].pos;
                    let aim = self.world[pair[1]].pos;
                    let flight = (from.x - aim.x).abs() / *speed;
                    let body = &mut self.world[*ball];
                    body.kind = BodyKind::Dynamic;
                    body.vel =
                        DVec3::new(-*speed, (aim.y - from.y) / flight + 0.5 * g * flight, 0.0);
                    *launched = true;
                }
                step_world(&mut self.world, gravity);
                restart = t >= 9.0;
            }
            Rig::Thrusters {
                craft,
                set,
                throttles,
                position,
                attitude,
                home,
                target,
            } => {
                let command = v("command") as usize;
                *target = match command {
                    1 => (*home + DVec3::X, 0.0),
                    2 => (*home + DVec3::Y * 0.5, 0.0),
                    3 => (*home, std::f64::consts::FRAC_PI_2),
                    4 => {
                        let corner = ((t / 3.0) as usize) % 4;
                        let offset = [
                            DVec3::new(-0.8, 0.0, 0.0),
                            DVec3::new(-0.8, 0.0, 0.8),
                            DVec3::new(0.8, 0.0, 0.8),
                            DVec3::new(0.8, 0.0, 0.0),
                        ][corner];
                        (*home + offset, 0.0)
                    }
                    _ => (*home, 0.0),
                };
                let body = self.world[*craft];
                let accel = position.update(target.0 - body.pos, DT) + DVec3::Y * g;
                let spin = attitude.update(
                    rotation_error(body.orientation, DQuat::from_rotation_y(target.1)),
                    DT,
                );
                let force = body.orientation.inverse() * (accel * body.mass);
                // Body-frame torque from the principal inertia.
                let torque = body.inertia * (body.orientation.inverse() * spin);
                *throttles = set.allocate(force, torque);
                set.apply(&mut self.world[*craft], throttles);
                step_world(&mut self.world, gravity);
            }
        }
        self.time += DT;
        if restart {
            let loops = self.loops + 1;
            *self = Self::new(kind, params);
            self.loops = loops;
        }
    }

    /// One or two lines describing what the mechanism is doing now.
    pub fn readout(&self, params: &[usize], gravity: bool) -> Vec<String> {
        let kind = self.kind;
        let v = |id| value(kind, params, id);
        let w = &self.world;
        let g = if gravity { G } else { 0.0 };
        // Each line stays under about 38 characters to fit a phone's HUD.
        match &self.rig {
            Rig::Manifold { pose, points, .. } => {
                let deepest = points
                    .iter()
                    .map(|p| -p.separation)
                    .fold(f64::NEG_INFINITY, f64::max);
                vec![
                    format!(
                        "Tilt {:.0}° yaw {:.0}° in {:.0} mm slide {:.2}",
                        pose[0].to_degrees(),
                        pose[1].to_degrees(),
                        pose[2] * 1000.0,
                        pose[3]
                    ),
                    if points.is_empty() {
                        "No contact".into()
                    } else {
                        format!(
                            "{} points · deepest {:.1} mm",
                            points.len(),
                            deepest * 1000.0
                        )
                    },
                ]
            }
            Rig::Friction { block, start, load } => {
                let body = &w[*block];
                let limit = v("friction") * body.mass * g;
                let state = match start {
                    None => "Settling".to_owned(),
                    Some(s) if body.vel.length() > 1e-3 => format!(
                        "Slipping {:.2} m/s · moved {:.2} m",
                        body.vel.length(),
                        (body.pos - *s).length()
                    ),
                    Some(s) => format!(
                        "Holding · moved {:.1} mm",
                        (body.pos - *s).length() * 1000.0
                    ),
                };
                vec![format!("Load {load:.1} N · limit µN {limit:.1} N"), state]
            }
            Rig::Torsion {
                ball,
                torque,
                start,
            } => {
                let body = &w[*ball];
                let limit = v("torsional") * body.mass * g;
                let rpm = body.omega_world().y * 60.0 / std::f64::consts::TAU;
                let turned = start.map_or(0.0, |s| s.angle_between(body.orientation).to_degrees());
                vec![
                    format!("Torque {torque:.2} · limit {limit:.2} N·m"),
                    if rpm.abs() > 0.1 {
                        format!("Slipping at {rpm:.0} rpm")
                    } else {
                        format!("Holding · turned {turned:.1}°")
                    },
                ]
            }
            Rig::Tunneling { shots, .. } => {
                let speed = v("speed");
                let passed = shots.iter().filter(|id| w[**id].pos.x > 0.0).count();
                vec![
                    format!("{speed} m/s: {:.2} m per step", speed * DT),
                    format!(
                        "4 cm panel · {} of {} stopped",
                        shots.len() - passed,
                        shots.len()
                    ),
                ]
            }
            Rig::Momentum { origin, error, .. } => {
                let now = w.momentum(*origin);
                vec![
                    format!(
                        "p {:.2} kg·m/s · L {:.2} kg·m²/s",
                        now.linear.length(),
                        now.angular.length()
                    ),
                    format!(
                        "Ledger error {:.0e} p, {:.0e} L",
                        error.linear, error.angular
                    ),
                ]
            }
            Rig::Stack { boxes } => {
                let asleep = boxes.iter().filter(|id| w[**id].sleeping).count();
                let top = boxes
                    .iter()
                    .map(|id| w[*id].pos.y + 0.2)
                    .fold(0.0, f64::max);
                vec![
                    format!(
                        "{} awake · {asleep} asleep · top {top:.2} m",
                        boxes.len() - asleep
                    ),
                    format!(
                        "{} contacts · {} iterations",
                        w.contacts.len(),
                        w.solver.iterations
                    ),
                ]
            }
            Rig::SoftGrip { joint, .. } => {
                let Some(j) = w.joint(*joint) else {
                    return vec![];
                };
                let (a, b) = j.anchors(w);
                let force = j.impulse.length() / DT;
                vec![
                    format!(
                        "Error {:.0} mm {:.1}° · {force:.0} N",
                        a.distance(b) * 1000.0,
                        j.angle_error(w).length().to_degrees()
                    ),
                    if j.saturated {
                        format!("Slipping at the {} limit", newtons(v("limit")))
                    } else {
                        format!("Holding under {}", newtons(v("limit")))
                    },
                ]
            }
            Rig::Tether {
                pivot,
                bob,
                anchor,
                tether,
                length,
                weld,
                ..
            } => {
                let swing = w[*bob].pos - *pivot;
                let angle = swing.x.atan2(-swing.y).to_degrees();
                let reach = w
                    .joint(*tether)
                    .map_or(0.0, |j| j.anchors(w).1.distance(*anchor));
                let (gap, bend) = w.joint(*weld).map_or((0.0, 0.0), |j| {
                    let (a, b) = j.anchors(w);
                    (a.distance(b), j.angle_error(w).length())
                });
                vec![
                    format!(
                        "Swing {angle:.0}° · tether {reach:.2}/{length:.1} {}",
                        if reach >= length - 0.01 {
                            "taut"
                        } else {
                            "slack"
                        }
                    ),
                    format!(
                        "Weld error {:.1} mm · {:.2}°",
                        gap * 1000.0,
                        bend.to_degrees()
                    ),
                ]
            }
            Rig::Thrusters {
                craft,
                throttles,
                target,
                ..
            } => {
                let body = &w[*craft];
                let firing = throttles.iter().filter(|u| **u > 1e-3).count();
                let thrust: f64 = throttles.iter().sum::<f64>() / throttles.len() as f64;
                vec![
                    format!("{firing} of 24 firing · {:.0}% thrust", thrust * 100.0),
                    format!(
                        "Error {:.2} m · {:.1}°",
                        body.pos.distance(target.0),
                        rotation_error(body.orientation, DQuat::from_rotation_y(target.1))
                            .length()
                            .to_degrees()
                    ),
                ]
            }
        }
    }

    /// Whether every body state is finite.
    #[cfg(test)]
    pub fn finite(&self) -> bool {
        self.world.bodies().iter().all(|b| {
            b.pos.is_finite()
                && b.vel.is_finite()
                && b.orientation.is_finite()
                && b.omega.is_finite()
        })
    }

    /// A body to draw highlighted: holding green, slipping red.
    pub fn highlight(&self, id: BodyId) -> Option<bool> {
        match &self.rig {
            Rig::Friction {
                block,
                start: Some(_),
                ..
            } if *block == id => Some(self.world[id].vel.length() <= 1e-3),
            Rig::Torsion {
                ball,
                start: Some(_),
                ..
            } if *ball == id => Some(self.world[id].omega_world().length() <= 0.02),
            Rig::SoftGrip { part, joint, .. } if *part == id => {
                Some(!self.world.joint(*joint).is_some_and(|j| j.saturated))
            }
            _ => None,
        }
    }
}

/// The scripted sweep: flat, rock on an edge, yaw, press in, slide off,
/// lift apart, and repeat every twelve seconds.
fn auto_sweep(t: f64) -> [f64; 4] {
    let phase = t % 12.0;
    let k = (phase % 2.0) / 2.0;
    let bump = (k * std::f64::consts::PI).sin();
    let rest = 0.001;
    match (phase / 2.0) as u32 {
        0 => [0.0, 0.0, rest, 0.0],
        1 => [10f64.to_radians() * bump, 0.0, rest, 0.0],
        2 => [0.0, 45f64.to_radians() * bump, rest, 0.0],
        3 => [0.0, 20f64.to_radians(), rest + 0.06 * bump, 0.0],
        4 => [0.0, 0.0, rest, 0.9 * bump],
        _ => [0.0, 0.0, rest - 0.05 * bump, 0.0],
    }
}
