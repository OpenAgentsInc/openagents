//! The L1 construction station: a crewed EVA with a cold-gas maneuvering
//! pack, free-flying rigid parts, and a keel jig where a ship frame begins.
//!
//! Scene axes: +Y is the ecliptic north pole, -Z points at the Sun, +Z at the
//! Earth, and +X completes a right-handed frame (the rotating frame's +y,
//! the direction of Earth's orbital motion). Units are SI.

use glam::{DQuat, DVec3};
use physics::{
    Body, BodyId, BodyKind, Collider, ColliderId, Composite, DebugKind, DebugLine, Filter,
    FixedStep, Imu, Joint, JointId, JointKind, Ledger, Material, Momentum, Plume as Exhaust,
    Reflection, Rope, RopeSettings, Sample, Shape, ThrusterSet, World,
};
use serde::{Deserialize, Serialize};

use crate::arrays::{ArrayFlex, Wing};
use crate::orbit::{StationOrbit, mean_motion};

/// Standard gravity, used only to convert specific impulse, m/s^2.
pub const G0: f64 = 9.806_65;
/// Suited astronaut plus maneuvering pack without propellant, kg.
pub const DRY_MASS: f64 = 230.0;
/// Full nitrogen load, kg.
pub const PROPELLANT: f64 = 20.0;
/// Net thrust available along any commanded direction, N: four of the
/// pack's thrusters at [`THRUSTER_FORCE`].
pub const THRUST: f64 = 40.0;
/// Force of one pack thruster, N.
pub const THRUSTER_FORCE: f64 = THRUST / 4.0;
/// Half-extents of the box whose corners carry the pack's 24 thrusters,
/// relative to the astronaut's center of mass, body frame, m.
pub const PACK_HALF: DVec3 = DVec3::new(0.35, 0.45, 0.3);
/// Attitude hold: natural frequency, rad/s, critically damped.
pub const ATTITUDE_FREQUENCY: f64 = 1.5;
/// Attitude errors and rates below these are left alone, rad and rad/s.
pub const ATTITUDE_DEADBAND: f64 = 0.004;
pub const RATE_DEADBAND: f64 = 0.002;
/// Fastest turn the attitude hold commands, rad/s.
pub const MAX_TURN_RATE: f64 = 0.6;
/// Torque the pack may spend steering its thrust through the center of mass
/// of a carried load, N m. Heavier or farther loads get less thrust so the
/// pair does not tumble.
pub const STEERING_TORQUE: f64 = 15.0;
/// Cold nitrogen specific impulse, s.
pub const ISP: f64 = 70.0;
/// Flight-control speed limit relative to the station, m/s.
pub const SPEED_LIMIT: f64 = 2.0;
/// Velocity errors below this are left alone (minimum impulse), m/s.
pub const VELOCITY_DEADBAND: f64 = 0.004;
/// Length of the safety tether from the airlock, m.
pub const EVA_RANGE: f64 = 140.0;
/// Largest tension the safety tether and the part lines hold, N.
pub const TETHER_TENSION: f64 = 3_000.0;
/// Beyond the tether length by this much, the emergency boundary returns
/// the astronaut or reels a part in, m. The tether itself normally stops
/// them first.
pub const TETHER_MARGIN: f64 = 2.0;
/// Refill port reach at the airlock, m.
pub const REFILL_RANGE: f64 = 3.5;
/// Refill rate at the airlock, kg/s.
pub const REFILL_RATE: f64 = 2.0;
/// Reach for grabbing a free part, m, measured from the hands.
pub const GRAB_RANGE: f64 = 3.0;
/// Latch capture distance, m, and maximum closing speed, m/s.
pub const LATCH_RANGE: f64 = 1.6;
pub const LATCH_SPEED: f64 = 0.35;
/// Latch alignment limit, rad (15 degrees). A part may also latch turned
/// half a turn about its keel.
pub const LATCH_ANGLE: f64 = 15.0 * std::f64::consts::PI / 180.0;
/// Largest spin rate that latches, rad/s.
pub const LATCH_SPIN: f64 = 0.05;
/// Orbital seconds per local second. The local rigid-body clock is real time.
pub const ORBIT_WARP: f64 = 3_600.0;
/// Parts that drift farther than this from the depot are reeled back, m.
pub const PART_TETHER: f64 = 120.0;
/// Fixed local physics step, s.
pub const PHYSICS_DT: f64 = 1.0 / 120.0;
/// Most physics steps one frame may run (0.1 s); longer frames drop time.
pub const MAX_STEPS_PER_FRAME: u32 = 12;
/// Layout version of [`StationState`].
pub const STATE_VERSION: u32 = 2;
/// Particles in the safety tether's rope and in each part line's rope.
pub const TETHER_PARTICLES: usize = 96;
pub const LINE_PARTICLES: usize = 48;
/// Linear density of the safety tether and the part lines, kg/m.
pub const LINE_DENSITY: f64 = 0.05;
/// Speed at which a line's reel takes in slack, m/s. It pays out freely.
pub const REEL_SPEED: f64 = 0.25;
/// Shortest line a reel leaves out, m.
pub const REEL_MIN: f64 = 0.5;
/// Station-keeping thruster pods at the truss tips.
pub const KEEPING_PODS: [DVec3; 2] = [DVec3::new(-30.8, 6.0, 0.0), DVec3::new(30.8, 6.0, 0.0)];
/// Thrust of each station-keeping pod, N.
pub const KEEPING_THRUST: f64 = 220.0;
/// How long each station-keeping burn fires in local time, s (its glow).
pub const KEEPING_BURN: f64 = 1.2;
/// Station mass, kg, for the acceleration a station-keeping burn gives the
/// structure (and so the solar array wings).
pub const STATION_MASS: f64 = 60_000.0;
/// How gas leaves the surfaces a plume strikes.
pub const PLUME_REFLECTION: Reflection = Reflection::DIFFUSE;
/// Plume onsets are kept this long for the renderer, s.
pub const PULSE_MEMORY: f64 = 1.0;
/// Index of the first station-keeping pod in [`PlumePulse::thruster`];
/// the pack's thrusters are 0 to 23.
pub const KEEPING_THRUSTER: u32 = 24;

/// The airlock refill port.
pub const AIRLOCK: DVec3 = DVec3::new(0.0, 6.0, 17.5);
/// The parts depot, where the next needed part waits.
pub const DEPOT: DVec3 = DVec3::new(-12.0, -6.0, 1.0);
/// Center line of the keel jig.
pub const JIG: DVec3 = DVec3::new(0.0, -6.0, 0.0);
/// Where a new EVA starts (body center), beside the airlock facing the station.
pub const SPAWN: DVec3 = DVec3::new(3.0, 5.3, 21.0);
/// Radius of the astronaut's collision capsule, m. The capsule runs head to
/// boots: 1.8 m tall around the body center.
pub const ASTRONAUT_RADIUS: f64 = 0.45;
/// Half the length of the capsule's core segment, m.
pub const ASTRONAUT_HALF_LENGTH: f64 = 0.45;

/// Collision groups: fixed station structure, the astronaut, free parts, and
/// parts fixed in the rack or on the jig.
pub mod layer {
    pub const STRUCTURE: u32 = 1;
    pub const ASTRONAUT: u32 = 2;
    pub const FREE: u32 = 4;
    pub const FIXED_PART: u32 = 8;
    pub const CARRIED: u32 = 16;
}

/// The glove's grip: a soft weld at the grabbed point, critically damped at
/// this natural frequency, rad/s.
pub const GRIP_FREQUENCY: f64 = 6.0;
/// Largest force and torque the grip holds before it slips, N and N m.
pub const GRIP_FORCE: f64 = 400.0;
pub const GRIP_TORQUE: f64 = 300.0;

/// Suit fabric and aluminum hardware.
pub const SURFACE: Material = Material {
    friction: 0.5,
    torsional: 0.01,
    restitution: 0.2,
};

/// Ship frame components, in keel order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    MainEngine,
    PropellantTank,
    KeelTrussAft,
    KeelTrussFore,
    RcsPod,
    AvionicsBay,
}

impl PartKind {
    pub const ALL: [Self; 6] = [
        Self::MainEngine,
        Self::PropellantTank,
        Self::KeelTrussAft,
        Self::KeelTrussFore,
        Self::RcsPod,
        Self::AvionicsBay,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MainEngine => "Main engine",
            Self::PropellantTank => "Propellant tank",
            Self::KeelTrussAft => "Aft keel truss",
            Self::KeelTrussFore => "Fore keel truss",
            Self::RcsPod => "RCS pod",
            Self::AvionicsBay => "Avionics bay",
        }
    }

    /// The part whose display name matches `name`, ignoring case and
    /// treating `-` as a space (`aft-keel-truss`).
    #[must_use]
    pub fn by_name(name: &str) -> Option<Self> {
        let name = name.replace('-', " ");
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.name().eq_ignore_ascii_case(&name))
    }

    /// Dry mass, kg.
    #[must_use]
    pub const fn mass(self) -> f64 {
        match self {
            Self::MainEngine => 450.0,
            Self::PropellantTank => 320.0,
            Self::KeelTrussAft | Self::KeelTrussFore => 180.0,
            Self::RcsPod => 140.0,
            Self::AvionicsBay => 90.0,
        }
    }

    /// Envelope edge lengths, m. The keel runs along z.
    #[must_use]
    pub fn size(self) -> DVec3 {
        match self {
            Self::MainEngine => DVec3::new(2.2, 2.2, 3.0),
            Self::PropellantTank => DVec3::new(2.6, 2.6, 3.6),
            Self::KeelTrussAft | Self::KeelTrussFore => DVec3::new(1.2, 1.2, 4.0),
            Self::RcsPod => DVec3::new(2.4, 1.0, 1.2),
            Self::AvionicsBay => DVec3::new(1.4, 1.4, 1.4),
        }
    }

    /// Collision shape: the tank is a capsule along the keel, the rest are
    /// boxes filling their envelopes.
    #[must_use]
    pub fn shape(self) -> Shape {
        match self {
            Self::PropellantTank => Shape::Capsule {
                radius: 1.3,
                half_length: 0.5,
            },
            _ => Shape::Cuboid {
                half: self.size() * 0.5,
            },
        }
    }

    #[must_use]
    pub fn inertia(self) -> DVec3 {
        match self {
            Self::PropellantTank => Body::shell_inertia(self.mass(), 1.3, 3.6),
            _ => Body::box_inertia(self.mass(), self.size()),
        }
    }

    /// Latch position on the jig, stacked from aft (-z) to fore (+z).
    #[must_use]
    pub fn slot(self) -> DVec3 {
        JIG + DVec3::new(
            0.0,
            0.0,
            match self {
                Self::MainEngine => -9.0,
                Self::PropellantTank => -5.7,
                Self::KeelTrussAft => -1.9,
                Self::KeelTrussFore => 2.1,
                Self::RcsPod => 4.7,
                Self::AvionicsBay => 6.1,
            },
        )
    }

    /// Stowage position in the depot rack.
    #[must_use]
    pub fn stowage(self) -> DVec3 {
        let i = Self::ALL.iter().position(|k| *k == self).unwrap_or(0) as f64;
        DEPOT + DVec3::new(0.0, 0.0, -5.0 + i * 2.2)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartState {
    Stowed,
    Carried,
    Drifting,
    Installed,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    /// The part's body in [`Station::world`].
    pub body: BodyId,
    pub collider: ColliderId,
    pub state: PartState,
    /// Line from the depot that keeps the part within [`PART_TETHER`].
    pub line: JointId,
    /// Weld to the jig once latched.
    #[serde(default)]
    pub latch: Option<JointId>,
}

impl PartState {
    /// How the world moves a part in this state.
    #[must_use]
    pub const fn body_kind(self) -> BodyKind {
        match self {
            Self::Stowed => BodyKind::Static,
            // A latched part is welded to the jig, so impacts load the weld.
            Self::Carried | Self::Drifting | Self::Installed => BodyKind::Dynamic,
        }
    }

    /// What a part in this state collides with. Racked and latched parts
    /// stop free parts but not the astronaut, who reaches in among them; a
    /// carried part hits structure and free parts but not its carrier or
    /// the rack and jig it is being fitted into.
    #[must_use]
    pub const fn filter(self) -> Filter {
        use layer::{ASTRONAUT, CARRIED, FIXED_PART, FREE, STRUCTURE};
        match self {
            Self::Stowed | Self::Installed => Filter {
                group: FIXED_PART,
                mask: FREE,
            },
            Self::Carried => Filter {
                group: CARRIED,
                mask: STRUCTURE | FREE,
            },
            Self::Drifting => Filter {
                group: FREE,
                mask: STRUCTURE | ASTRONAUT | FREE | FIXED_PART | CARRIED,
            },
        }
    }
}

/// An axis-aligned keep-out box, m.
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub min: DVec3,
    pub max: DVec3,
}

/// Solid station structure. Open lattice (the jig) is not listed.
pub const OBSTACLES: [Obstacle; 6] = [
    // Habitat module.
    Obstacle {
        min: DVec3::new(-2.2, 3.8, 2.0),
        max: DVec3::new(2.2, 8.2, 14.0),
    },
    // Node and airlock.
    Obstacle {
        min: DVec3::new(-1.7, 4.3, 14.0),
        max: DVec3::new(1.7, 7.7, 17.0),
    },
    // Main truss.
    Obstacle {
        min: DVec3::new(-30.0, 5.3, -0.7),
        max: DVec3::new(30.0, 6.7, 0.7),
    },
    // Solar arrays, face-on to the Sun.
    Obstacle {
        min: DVec3::new(-30.0, -0.5, -1.25),
        max: DVec3::new(-13.0, 12.5, -0.95),
    },
    Obstacle {
        min: DVec3::new(13.0, -0.5, -1.25),
        max: DVec3::new(30.0, 12.5, -0.95),
    },
    // Depot backboard.
    Obstacle {
        min: DVec3::new(-15.6, -9.0, -6.5),
        max: DVec3::new(-15.0, -3.0, 9.0),
    },
];

/// One frame of pilot input, already mapped into scene axes.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Command {
    /// Desired translation direction; zero holds position.
    pub direction: DVec3,
    /// Heading in radians, counterclockwise around +Y; +Z at zero.
    pub yaw: f64,
    /// A short climb along +Y (the jump gesture).
    pub climb: bool,
}

impl Command {
    /// Bitwise equality, so a NaN yaw ("keep heading") compares equal to itself.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        self.direction.to_array().map(f64::to_bits) == other.direction.to_array().map(f64::to_bits)
            && self.yaw.to_bits() == other.yaw.to_bits()
            && self.climb == other.climb
    }
}

/// Everything that changes the station from outside: the pilot's held
/// command and discrete actions from local controls or NIP-MV operators.
/// A saved state plus a tick-stamped list of inputs replays a session.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum Input {
    /// Hold this pilot command for the following steps.
    Pilot {
        command: Command,
    },
    Grab,
    Release,
    FlyTo {
        target: DVec3,
    },
    /// Cancel the autopilot target.
    Stop,
}

/// A saved station: restore it and continue, or replay inputs from it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StationState {
    pub version: u32,
    pub station: Station,
}

/// A thruster that started firing: the event a renderer seeds cosmetic
/// exhaust particles from. Pulses are deterministic, so a replay makes the
/// same ones.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlumePulse {
    /// Pack thrusters are 0 to 23 in [`Station::pack`] order;
    /// [`KEEPING_THRUSTER`] and the next are the −x and +x station-keeping
    /// pods.
    pub thruster: u32,
    /// Nozzle position when the pulse began, scene coordinates, m.
    pub pos: DVec3,
    /// Exhaust direction, unit (opposite the thrust).
    pub dir: DVec3,
    /// The physics step the thruster began firing in: the pulse starts at
    /// `tick × PHYSICS_DT`, one step before the state at that tick + 1.
    pub tick: u64,
    /// Thrust at onset, N.
    pub thrust: f64,
    /// Seed for the pulse's particles, from the thruster and the tick.
    pub seed: u64,
}

/// A line from the station with its rope: the safety tether or a part's
/// depot line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// The rigid tether joint, which stays authoritative for arrest,
    /// tension, and the ledger.
    pub joint: JointId,
    /// The body on the line's free end.
    pub body: BodyId,
    /// The joint's length, m: the most the reel pays out.
    pub max_length: f64,
    pub rope: Rope,
}

/// A rope as drawn: particle positions from the anchor on the station to
/// the free end, scene coordinates, m.
#[derive(Clone, Copy, Debug)]
pub struct RopeView<'a> {
    /// Positions after the last physics step.
    pub points: &'a [DVec3],
    /// Positions before it; interpolate with [`Station::alpha`].
    pub prev: &'a [DVec3],
    /// The rigid tether is pulling, or the anchors are a full rope length
    /// apart: the rope lies straight.
    pub taut: bool,
    /// Line tension, N: the tether joint's pull when it is taut, otherwise
    /// the rope's own pull on its ends.
    pub tension: f64,
    /// Rope length the reel has paid out, m.
    pub length: f64,
}

impl RopeView<'_> {
    /// Particle `i` between the last two steps; `alpha` in [0, 1].
    #[must_use]
    pub fn point(&self, i: usize, alpha: f64) -> DVec3 {
        self.prev[i].lerp(self.points[i], alpha.clamp(0.0, 1.0))
    }
}

/// A visible thruster firing, for rendering only.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Plume {
    pub pos: [f64; 3],
    /// Exhaust direction (opposite the thrust), unit.
    pub dir: [f64; 3],
    pub age: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub orbit: crate::orbit::OrbitSnapshot,
    pub propellant_kg: f64,
    pub propellant_fraction: f64,
    pub delta_v_remaining_m_s: f64,
    pub speed_m_s: f64,
    pub mass_kg: f64,
    pub range_m: f64,
    pub carrying: Option<PartKind>,
    pub installed: usize,
    pub total: usize,
    pub next_part: Option<PartKind>,
    pub can_grab: bool,
    pub latch_ready: bool,
    pub latch_distance_m: Option<f64>,
    /// Alignment error of the carried part from its latch, degrees.
    pub latch_angle_deg: Option<f64>,
    /// Bodies the physics world is simulating (not asleep or fixed).
    pub awake_bodies: usize,
    /// Wall-clock time of the last physics step, ms, for profiling.
    pub step_ms: f64,
    /// Contact force on the astronaut and anything it holds in the last
    /// step, N.
    pub impact_n: f64,
    /// What the suit's accelerometer feels, in standard gravities.
    pub g_load: f64,
    /// The astronaut's rotation rate, degrees per second.
    pub spin_deg_s: f64,
    /// Distance to the nearest structure or part straight ahead, m.
    pub proximity_m: Option<f64>,
    pub refilling: bool,
    /// A station-keeping burn fired within the last second.
    pub keeping_active: bool,
    pub message: Option<String>,
}

/// The whole L1 construction simulation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub orbit: StationOrbit,
    /// Every local rigid body: the astronaut and the parts.
    pub world: World,
    /// Turns frame time into fixed [`PHYSICS_DT`] steps.
    pub clock: FixedStep,
    /// The suited astronaut; its `pos` is the body center, 0.9 m above the boots.
    pub astronaut: BodyId,
    /// Commanded heading, rad; the pack's attitude hold turns the body to it.
    /// The body's actual heading is [`Station::heading_yaw`].
    pub yaw: f64,
    pub propellant: f64,
    pub parts: Vec<Part>,
    pub plumes: Vec<Plume>,
    /// Remaining seconds of the station-keeping thruster glow.
    pub keeping_glow: f64,
    pub target: Option<DVec3>,
    pub message: Option<String>,
    pub refilling: bool,
    /// The pilot command held between frames.
    pub pilot: Command,
    /// Inputs applied since recording started, stamped with the world tick.
    #[serde(default)]
    pub journal: Option<Vec<(u64, Input)>>,
    /// The glove's grip on the carried part.
    #[serde(default)]
    pub grip: Option<JointId>,
    /// A fixed body at the station origin that carries the tether, line, and
    /// jig anchors.
    pub anchor: BodyId,
    /// The astronaut's safety tether from the airlock.
    pub tether: JointId,
    /// The suit's inertial measurement unit.
    #[serde(default)]
    pub imu: Imu,
    /// Thrusters that fired in the last step: world force and position.
    #[serde(skip)]
    pub fired: Vec<(DVec3, DVec3)>,
    /// Apply the L1 tidal field to local bodies. Off only for conservation
    /// tests, since the rotating-frame field is an external force.
    #[serde(default = "enabled")]
    pub tide: bool,
    /// External impulses since [`Station::reset_ledger`]: exhaust, contact
    /// with fixed structure, the safety tether, tether reel-in, latching,
    /// and plume impingement.
    #[serde(default)]
    pub ledger: Ledger,
    /// The safety tether's line first, then each part's depot line in
    /// [`PartKind::ALL`] order.
    #[serde(default)]
    pub lines: Vec<Line>,
    /// Solar array wings on the −x and +x sides.
    pub wings: [Wing; 2],
    /// Let the ropes pull the bodies they tie (two-way coupling). Off: the
    /// rigid tether joints carry every load and the ropes only follow.
    #[serde(default)]
    pub rope_coupling: bool,
    /// Let the pack's plumes push the part it carries. The forward and aft
    /// jets then blow into a load held at arm's length, so the pack cannot
    /// accelerate the pair away from the load or brake toward it; off by
    /// default so carrying stays flyable. Free parts are always pushed.
    #[serde(default)]
    pub impinge_carried: bool,
    /// Recent thruster onsets, oldest first.
    #[serde(default)]
    pub pulses: Vec<PlumePulse>,
    /// Exhaust direction of the current station-keeping burn, unit.
    #[serde(default)]
    pub keeping_exhaust: DVec3,
    /// Plume forces from the last step, on every collider they reached.
    #[serde(skip)]
    pub impingement: Vec<Sample>,
    /// Thrusters that fired in the last step, as a bit per
    /// [`PlumePulse::thruster`].
    #[serde(default)]
    firing: u32,
    climb: f64,
    plume_clock: f64,
}

impl Default for Station {
    fn default() -> Self {
        Self::new()
    }
}

impl Station {
    #[must_use]
    pub fn new() -> Self {
        use layer::{ASTRONAUT, CARRIED, FREE, STRUCTURE};
        let mut world = World::new(PHYSICS_DT);
        let astronaut = world.add(Body::new(DRY_MASS + PROPELLANT, DVec3::splat(40.0), SPAWN));
        world.add_collider(
            Collider::new(
                astronaut,
                Shape::Capsule {
                    radius: ASTRONAUT_RADIUS,
                    half_length: ASTRONAUT_HALF_LENGTH,
                },
            )
            .at(
                DVec3::ZERO,
                DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2),
            )
            .with_filter(Filter {
                group: ASTRONAUT,
                mask: STRUCTURE | FREE,
            })
            .with_material(SURFACE),
        );
        for o in &OBSTACLES {
            let body = world
                .add(Body::new(1.0, DVec3::ONE, (o.min + o.max) * 0.5).with_kind(BodyKind::Static));
            world.add_collider(
                Collider::new(
                    body,
                    Shape::Cuboid {
                        half: (o.max - o.min) * 0.5,
                    },
                )
                .with_filter(Filter {
                    group: STRUCTURE,
                    mask: ASTRONAUT | FREE | CARRIED,
                })
                .with_material(SURFACE),
            );
        }
        // Colliders 4 and 5 are the solar arrays.
        let arrays = [ColliderId(4), ColliderId(5)];
        let wings = arrays.map(|id| {
            let collider = world.colliders()[id.0 as usize];
            let side = world[collider.body].pos.x.signum();
            Wing::new(side, id, collider.body)
        });
        let anchor = world.add(Body::new(1.0, DVec3::ONE, DVec3::ZERO).with_kind(BodyKind::Static));
        let tether = world.add_joint(
            Joint::new(
                anchor,
                AIRLOCK,
                astronaut,
                DVec3::ZERO,
                JointKind::Tether { length: EVA_RANGE },
            )
            .limited(TETHER_TENSION, 0.0),
        );
        let parts = PartKind::ALL
            .iter()
            .map(|&kind| {
                let body = world.add(
                    Body::new(kind.mass(), kind.inertia(), kind.stowage())
                        .with_kind(PartState::Stowed.body_kind()),
                );
                let collider = world.add_collider(
                    Collider::new(body, kind.shape())
                        .with_filter(PartState::Stowed.filter())
                        .with_material(SURFACE),
                );
                let line = world.add_joint(
                    Joint::new(
                        anchor,
                        DEPOT,
                        body,
                        DVec3::ZERO,
                        JointKind::Tether {
                            length: PART_TETHER,
                        },
                    )
                    .limited(TETHER_TENSION, 0.0),
                );
                Part {
                    kind,
                    body,
                    collider,
                    state: PartState::Stowed,
                    line,
                    latch: None,
                }
            })
            .collect();
        let mut station = Self {
            orbit: StationOrbit::new(),
            world,
            clock: FixedStep::new(PHYSICS_DT, MAX_STEPS_PER_FRAME),
            astronaut,
            yaw: std::f64::consts::PI,
            propellant: PROPELLANT,
            parts,
            plumes: Vec::new(),
            keeping_glow: 0.0,
            target: None,
            message: None,
            refilling: false,
            pilot: Command::default(),
            journal: None,
            grip: None,
            imu: Imu::default(),
            fired: Vec::new(),
            anchor,
            tether,
            tide: true,
            ledger: Ledger::default(),
            lines: Vec::new(),
            wings,
            rope_coupling: false,
            impinge_carried: false,
            pulses: Vec::new(),
            keeping_exhaust: DVec3::ZERO,
            impingement: Vec::new(),
            firing: 0,
            climb: 0.0,
            plume_clock: 0.0,
        };
        station.lines.push(Line {
            joint: tether,
            body: astronaut,
            max_length: EVA_RANGE,
            rope: Rope::new(AIRLOCK, SPAWN, 1.0, TETHER_PARTICLES, line_settings()),
        });
        for part in station.parts.clone() {
            station.lines.push(Line {
                joint: part.line,
                body: part.body,
                max_length: PART_TETHER,
                rope: Rope::new(
                    DEPOT,
                    part.kind.stowage(),
                    1.0,
                    LINE_PARTICLES,
                    line_settings(),
                ),
            });
        }
        station.settle_lines();
        station.face(std::f64::consts::PI);
        station.reset_ledger();
        station
    }

    /// The pack's thrusters: three at each corner of [`PACK_HALF`].
    #[must_use]
    pub fn pack() -> ThrusterSet {
        ThrusterSet::box_corners(PACK_HALF, THRUSTER_FORCE)
    }

    /// Point the astronaut at `yaw` at once, at rest in rotation, and make
    /// it the commanded heading. For spawning and scripted setups.
    pub fn face(&mut self, yaw: f64) {
        self.yaw = yaw;
        let astronaut = self.astronaut_mut();
        astronaut.orientation = DQuat::from_rotation_y(yaw);
        astronaut.prev_orientation = astronaut.orientation;
        astronaut.omega = DVec3::ZERO;
    }

    /// Unit direction the astronaut faces.
    #[must_use]
    pub fn facing(&self) -> DVec3 {
        self.astronaut().orientation * DVec3::Z
    }

    /// The astronaut's actual heading about +Y, rad; +Z at zero.
    #[must_use]
    pub fn heading_yaw(&self) -> f64 {
        let f = self.facing();
        f.x.atan2(f.z)
    }

    /// The astronaut's body.
    #[must_use]
    pub fn astronaut(&self) -> &Body {
        &self.world[self.astronaut]
    }

    pub fn astronaut_mut(&mut self) -> &mut Body {
        &mut self.world[self.astronaut]
    }

    /// A part's body.
    #[must_use]
    pub fn body(&self, part: &Part) -> &Body {
        &self.world[part.body]
    }

    /// Fraction of a physics step accumulated since the last one, for
    /// interpolating rendered poses with [`Body::interpolated`].
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.clock.alpha()
    }

    fn set_state(&mut self, index: usize, state: PartState) {
        let part = &mut self.parts[index];
        part.state = state;
        let (body, collider) = (part.body, part.collider);
        self.world[body].kind = state.body_kind();
        self.world.collider_mut(collider).filter = state.filter();
    }

    /// Whether `body` belongs to the free system the ledger follows: the
    /// astronaut, or a carried or drifting part.
    fn in_system(&self, body: BodyId) -> bool {
        body == self.astronaut
            || self.parts.iter().any(|p| {
                p.body == body && matches!(p.state, PartState::Carried | PartState::Drifting)
            })
    }

    /// Record the pull of the safety tether and the part lines on the free
    /// system as the `tether` term, and say when the safety tether is taut.
    fn account_tethers(&mut self) {
        let mut lines = vec![(self.tether, self.astronaut)];
        lines.extend(self.parts.iter().map(|p| (p.line, p.body)));
        for (id, body) in lines {
            if !self.in_system(body) {
                continue;
            }
            let Some(joint) = self.world.joint(id) else {
                continue;
            };
            if joint.impulse == DVec3::ZERO {
                continue;
            }
            let (impulse, at) = (joint.impulse, joint.point);
            self.ledger.add_impulse("tether", impulse, at);
            if id == self.tether {
                self.message = Some("Safety tether taut".into());
            }
        }
    }

    /// Record contact impulses between the free system and fixed bodies
    /// (station structure, racked and latched parts) as the `structure` term.
    fn account_contacts(&mut self) {
        // A system body put to sleep against fixed structure gave its last
        // trace of momentum to that structure.
        for (id, removed) in self.world.slept.clone() {
            if self.in_system(id) && removed != Momentum::ZERO {
                self.ledger.add(
                    "structure",
                    Momentum {
                        linear: -removed.linear,
                        angular: -removed.angular,
                    },
                );
            }
        }
        let contacts = std::mem::take(&mut self.world.contacts);
        for c in &contacts {
            let (a, b) = (self.in_system(c.body_a), self.in_system(c.body_b));
            if a == b {
                continue;
            }
            let sign = if b { 1.0 } else { -1.0 };
            self.ledger
                .add_impulse("structure", c.impulse * sign, c.point);
            self.ledger.add(
                "structure",
                Momentum {
                    linear: DVec3::ZERO,
                    angular: c.twist * sign,
                },
            );
        }
        self.world.contacts = contacts;
    }

    /// Save everything needed to continue or replay from here.
    #[must_use]
    pub fn save(&self) -> StationState {
        StationState {
            version: STATE_VERSION,
            station: self.clone(),
        }
    }

    /// Restore a saved station.
    ///
    /// # Errors
    ///
    /// Returns a message when the saved layout version is not supported.
    pub fn restore(state: StationState) -> Result<Self, String> {
        if state.version != STATE_VERSION {
            return Err(format!(
                "station state version {} is not the supported version {STATE_VERSION}",
                state.version
            ));
        }
        state.station.world.check_version()?;
        Ok(state.station)
    }

    /// Start recording inputs for replay, from the current tick.
    pub fn record(&mut self) {
        self.journal = Some(vec![(
            self.world.tick,
            Input::Pilot {
                command: self.pilot,
            },
        )]);
    }

    fn log(&mut self, input: Input) {
        let tick = self.world.tick;
        if let Some(journal) = &mut self.journal {
            journal.push((tick, input));
        }
    }

    /// Apply one input now. Actions return the part they affected.
    ///
    /// # Errors
    ///
    /// Returns the refusal message when an action is not possible.
    pub fn apply(&mut self, input: Input) -> Result<Option<PartKind>, String> {
        self.log(input);
        match input {
            Input::Pilot { command } => {
                self.pilot = command;
                Ok(None)
            }
            Input::Grab => self.grab().map(Some),
            Input::Release => self.release().map(Some),
            Input::FlyTo { target } => self.fly_to(target).map(|()| None),
            Input::Stop => {
                self.target = None;
                Ok(None)
            }
        }
    }

    /// Run `journal` from `start` until the world reaches `until`: at each
    /// tick, apply the inputs stamped with it, then take one step.
    #[must_use]
    pub fn replay(start: &Self, journal: &[(u64, Input)], until: u64) -> Self {
        let mut station = start.clone();
        station.journal = None;
        let mut next = journal.iter().peekable();
        while station.world.tick < until {
            while let Some((_, input)) = next.next_if(|(tick, _)| *tick <= station.world.tick) {
                // A refused action was refused live too; replay it the same way.
                let _ = station.apply(*input);
            }
            let command = station.pilot;
            station.advance(&command);
        }
        station
    }

    /// Relative acceleration near L1: the linearized restricted three-body
    /// field, with Coriolis terms, mapped into scene axes. At station scale it
    /// is about 1e-11 m/s^2; it is kept so free parts obey the real field.
    #[must_use]
    pub fn field(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        tide(self.orbit.l1.c2, pos, vel)
    }

    #[must_use]
    pub fn carried(&self) -> Option<usize> {
        self.parts
            .iter()
            .position(|p| p.state == PartState::Carried)
    }

    /// The astronaut's own mass with its propellant, kg.
    #[must_use]
    pub fn own_mass(&self) -> f64 {
        DRY_MASS + self.propellant
    }

    /// Total mass the pack must accelerate, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        DRY_MASS
            + self.propellant
            + self
                .carried()
                .map_or(0.0, |i| self.body(&self.parts[i]).mass)
    }

    /// Where the gloves are: in front of the chest.
    #[must_use]
    pub fn hands(&self) -> DVec3 {
        self.astronaut().to_world(DVec3::new(0.0, 0.2, 1.0))
    }

    /// Ideal rocket equation for the propellant left, m/s.
    #[must_use]
    pub fn delta_v_remaining(&self) -> f64 {
        let wet = self.mass();
        ISP * G0 * (wet / (wet - self.propellant)).ln()
    }

    /// The next part the depot will release.
    #[must_use]
    pub fn next_part(&self) -> Option<PartKind> {
        self.parts
            .iter()
            .find(|p| p.state == PartState::Stowed)
            .map(|p| p.kind)
    }

    /// A named place to fly to: `depot` (beside the next stowed part, or
    /// the empty depot), `jig`, `airlock`, `spawn`, or a part name, which
    /// lands beside that part's stowage.
    #[must_use]
    pub fn landmark(&self, name: &str) -> Option<DVec3> {
        let beside = |stowage: DVec3| stowage + DVec3::new(2.0, 0.0, 0.0);
        Some(match name {
            "depot" => self
                .next_part()
                .map_or(DEPOT, |kind| beside(kind.stowage())),
            "jig" => JIG,
            "airlock" => AIRLOCK,
            "spawn" => SPAWN,
            _ => beside(PartKind::by_name(name)?.stowage()),
        })
    }

    /// Fly the pack toward `target` under the same speed and thrust limits.
    pub fn fly_to(&mut self, target: DVec3) -> Result<(), String> {
        if !target.is_finite() || target.distance(AIRLOCK) > EVA_RANGE {
            return Err("That point is beyond the safety tether".into());
        }
        self.target = Some(target);
        Ok(())
    }

    /// Advance by `frame` real seconds of frame time: whole
    /// [`PHYSICS_DT`] steps under `command`, with the remainder carried to the
    /// next frame. At most [`MAX_STEPS_PER_FRAME`] steps run; the clock
    /// counts time beyond them in `clock.dropped`.
    pub fn step(&mut self, frame: f64, command: &Command) {
        if !command.same(&self.pilot) {
            let _ = self.apply(Input::Pilot { command: *command });
        }
        for _ in 0..self.clock.advance(frame) {
            self.advance(command);
        }
    }

    /// One fixed step: local physics by [`PHYSICS_DT`] and the orbit by
    /// `PHYSICS_DT * ORBIT_WARP` mission seconds.
    pub fn advance(&mut self, command: &Command) {
        let dt = PHYSICS_DT;
        let tick = self.world.tick;
        if self.orbit.advance(dt * ORBIT_WARP) > 0 {
            self.keeping_glow = KEEPING_BURN;
            let dv = self.orbit.last_dv;
            // Rotating-frame axes (x Sun to Earth, y along track, z north)
            // to scene axes; the exhaust leaves opposite the velocity change.
            self.keeping_exhaust = -DVec3::new(dv.y, dv.z, dv.x).normalize_or_zero();
        }
        let keeping = self.keeping_glow > 0.0 && self.keeping_exhaust != DVec3::ZERO;
        self.keeping_glow = (self.keeping_glow - dt).max(0.0);
        if command.yaw.is_finite() {
            self.yaw = command.yaw;
        }
        if command.climb {
            self.climb = 1.0;
        }
        let manual = command.direction.length_squared() > 1e-6 || self.climb > 0.0;
        if manual {
            self.target = None;
        }
        let mut desired = if command.direction.length_squared() > 1e-6 {
            command.direction.normalize() * SPEED_LIMIT
        } else {
            DVec3::ZERO
        };
        if self.climb > 0.0 {
            desired.y = SPEED_LIMIT;
            self.climb -= dt;
        }
        let group = self.group();
        let mass = group.mass;
        // A force through the group's center of mass needs torque from the
        // pack in proportion to the lever arm; keep that within budget.
        let arm = (group.com - self.astronaut().pos).length();
        let thrust = if arm > 1e-6 {
            THRUST.min(STEERING_TORQUE / arm)
        } else {
            THRUST
        };
        let accel_limit = thrust / mass;
        let (pos, vel) = (self.astronaut().pos, group.vel);
        if !manual && let Some(target) = self.target {
            let offset = target - pos;
            let distance = offset.length();
            if distance < 0.3 && vel.length() < 0.05 {
                self.target = None;
            } else {
                // Leave margin so the braking burn fits inside the thrust limit.
                let speed = SPEED_LIMIT.min((accel_limit * distance).sqrt());
                desired = offset.normalize_or_zero() * speed;
            }
        }
        let error = desired - vel;
        let force = if error.length() > VELOCITY_DEADBAND {
            (error / dt).clamp_length_max(accel_limit) * mass
        } else {
            DVec3::ZERO
        };
        let torque = (group.com - self.astronaut().pos).cross(force) + self.attitude_torque(&group);
        let fired = self.fire(force, torque, dt);
        let mass = self.own_mass();
        self.astronaut_mut().mass = mass;
        self.impingement = self.impinge(&fired, keeping, dt);
        let (c2, tidal) = (self.orbit.l1.c2, self.tide);
        self.world.step(&move |p, v| {
            if tidal { tide(c2, p, v) } else { DVec3::ZERO }
        });
        // The structure accelerates with a station-keeping burn; the wings
        // lag it.
        let base = if keeping {
            -self.keeping_exhaust * (2.0 * KEEPING_THRUST / STATION_MASS)
        } else {
            DVec3::ZERO
        };
        for wing in &mut self.wings {
            wing.step(&self.impingement, base, dt);
        }
        self.account_contacts();
        let astronaut = *self.astronaut();
        let field = if self.tide {
            tide(c2, astronaut.pos, astronaut.vel)
        } else {
            DVec3::ZERO
        };
        self.imu.read(&astronaut, field, dt);
        self.check_grip();
        self.account_tethers();
        // Emergency boundary: only if the tether gave way past its tension.
        let before = self.momentum();
        let astronaut = &mut self.world[self.astronaut];
        let out = astronaut.pos - AIRLOCK;
        let range = out.length();
        if range > EVA_RANGE + TETHER_MARGIN {
            let out = out / range;
            astronaut.pos = AIRLOCK + out * (EVA_RANGE + TETHER_MARGIN);
            let radial = astronaut.vel.dot(out);
            if radial > 0.0 {
                astronaut.vel -= out * radial;
            }
        }
        self.account("tether", before);
        let (pos, vel) = (self.astronaut().pos, self.astronaut().vel);
        self.refilling = pos.distance(AIRLOCK) <= REFILL_RANGE
            && vel.length() < 0.6
            && self.propellant < PROPELLANT;
        if self.refilling {
            // Gas from the station tank starts at rest, so the pack slows.
            let added = (REFILL_RATE * dt).min(PROPELLANT - self.propellant);
            let mass = self.own_mass();
            self.propellant += added;
            let astronaut = self.astronaut_mut();
            astronaut.vel *= mass / (mass + added);
            astronaut.mass = mass + added;
        }
        self.step_lines(dt);
        self.record_pulses(&fired, keeping, tick);
        let fired: Vec<(DVec3, DVec3)> = fired.iter().map(|(_, f, at)| (*f, *at)).collect();
        self.emit_plumes(&fired, dt);
        self.fired = fired;
        self.settle_parts();
    }

    /// Contact force on the astronaut and anything it holds, N.
    #[must_use]
    pub fn impact(&self) -> f64 {
        let mut total = self.world.contact_force(self.astronaut).0;
        if let Some(i) = self.carried() {
            total += self.world.contact_force(self.parts[i].body).0;
        }
        total.length()
    }

    /// Distance to the nearest structure or part along the astronaut's
    /// facing, within 50 m, ignoring the astronaut and what it holds.
    #[must_use]
    pub fn proximity(&self) -> Option<f64> {
        let held = self.carried().map(|i| self.parts[i].body);
        let astronaut = self.astronaut;
        self.world
            .raycast(self.astronaut().pos, self.facing(), 50.0, &|c| {
                c.body != astronaut && Some(c.body) != held && c.filter.group != 0
            })
            .map(|hit| hit.distance)
    }

    /// Lines for a physics overlay: contacts, their impulses, joints (the
    /// grip, tethers, and latches), and the pack's firing thrusters.
    #[must_use]
    pub fn debug_lines(&self) -> Vec<DebugLine> {
        let mut lines = self.world.debug_lines();
        lines.extend(self.fired.iter().map(|(force, at)| DebugLine {
            from: *at,
            to: *at + *force * 0.05,
            kind: DebugKind::Thrust,
        }));
        lines
    }

    /// The astronaut and anything it holds, as one body.
    #[must_use]
    pub fn group(&self) -> Composite {
        let astronaut = self.astronaut();
        match self.carried() {
            Some(i) => Composite::of([astronaut, self.body(&self.parts[i])]),
            None => Composite::of([astronaut]),
        }
    }

    /// World-frame torque the attitude hold wants: a critically damped
    /// spring toward the commanded heading, level, for the whole group.
    fn attitude_torque(&self, group: &Composite) -> DVec3 {
        let body = self.astronaut();
        let mut error = DQuat::from_rotation_y(self.yaw) * body.orientation.inverse();
        if error.w < 0.0 {
            error = -error;
        }
        let (axis, angle) = error.to_axis_angle();
        let rate = group.omega();
        if angle < ATTITUDE_DEADBAND && rate.length() < RATE_DEADBAND {
            return DVec3::ZERO;
        }
        // Rate-limited: turn toward the heading at most MAX_TURN_RATE, and
        // damp the rate toward that. Unsaturated, this is w^2 angle - 2 w rate.
        let w = ATTITUDE_FREQUENCY;
        let wanted = axis * (angle * w / 2.0).min(MAX_TURN_RATE);
        let accel = (wanted - rate) * (2.0 * w);
        group.inertia * accel
    }

    /// Fire the pack toward a world-frame `force` and `torque` within the
    /// propellant left. The gas leaves each thruster at the exhaust velocity
    /// relative to the pack; the ledger records what the system receives
    /// (the thrust impulses, minus the gas's share of the pack's momentum).
    /// Returns each firing thruster's index, world force, and position.
    fn fire(&mut self, force: DVec3, torque: DVec3, dt: f64) -> Vec<(u32, DVec3, DVec3)> {
        if self.propellant <= 0.0 || (force == DVec3::ZERO && torque == DVec3::ZERO) {
            return Vec::new();
        }
        let pack = Self::pack();
        let inverse = self.astronaut().orientation.inverse();
        let mut throttles = pack.allocate(inverse * force, inverse * torque);
        let burn = |throttles: &[f64]| {
            pack.thrusters
                .iter()
                .zip(throttles)
                .map(|(t, u)| t.max_force * u)
                .sum::<f64>()
                * dt
                / (ISP * G0)
        };
        let mut used = burn(&throttles);
        if used > self.propellant {
            let scale = self.propellant / used;
            throttles.iter_mut().for_each(|u| *u *= scale);
            used = self.propellant;
        }
        if used <= 0.0 {
            return Vec::new();
        }
        let (pos, vel) = (self.astronaut().pos, self.astronaut().vel);
        self.propellant -= used;
        let mass = self.own_mass();
        let astronaut = self.astronaut_mut();
        astronaut.mass = mass;
        let fired = pack.apply(astronaut, &throttles);
        self.ledger.add_impulse("exhaust", -vel * used, pos);
        for (force, at) in &fired {
            self.ledger.add_impulse("exhaust", *force * dt, *at);
        }
        // `apply` returns the firing thrusters in order.
        throttles
            .iter()
            .enumerate()
            .filter(|(_, u)| **u > 0.0)
            .zip(fired)
            .map(|((i, _), (force, at))| (i as u32, force, at))
            .collect()
    }

    /// Plume impingement for one step, applied before the world steps.
    ///
    /// Pack plumes push drifting parts (and the carried part with
    /// [`Station::impinge_carried`]), never the astronaut they come from. Gas that strikes anything did not escape, so its
    /// momentum is added back to `exhaust` (which is minus what the escaping
    /// gas carries away); what it gave fixed structure leaves the free
    /// system under `impingement`, with the matching sign. Station-keeping
    /// plumes come from the station, so what they give the free system,
    /// the astronaut included, enters under `impingement`. Every sample,
    /// on any collider, is returned for the array wings.
    fn impinge(&mut self, fired: &[(u32, DVec3, DVec3)], keeping: bool, dt: f64) -> Vec<Sample> {
        let mut all = Vec::new();
        let astronaut = self.astronaut;
        let held = if self.impinge_carried {
            None
        } else {
            self.carried().map(|i| self.parts[i].body)
        };
        for (_, force, at) in fired {
            let thrust = force.length();
            if thrust <= 0.0 {
                continue;
            }
            let plume = Exhaust::nitrogen(*at, -*force / thrust, thrust);
            let samples = self.world.impinge(&plume, PLUME_REFLECTION, &|c| {
                c.body != astronaut && Some(c.body) != held
            });
            for sample in &samples {
                self.ledger
                    .add_impulse("exhaust", sample.force * dt, sample.point);
                if self.in_system(sample.body) {
                    self.world[sample.body].apply_force_at(sample.force, sample.point);
                } else {
                    self.ledger
                        .add_impulse("impingement", -sample.force * dt, sample.point);
                }
            }
            all.extend(samples);
        }
        if keeping {
            for pod in KEEPING_PODS {
                let plume = Self::keeping_plume(pod, self.keeping_exhaust);
                let samples = self.world.impinge(&plume, PLUME_REFLECTION, &|_| true);
                for sample in &samples {
                    if self.in_system(sample.body) {
                        self.world[sample.body].apply_force_at(sample.force, sample.point);
                        self.ledger
                            .add_impulse("impingement", sample.force * dt, sample.point);
                    }
                }
                all.extend(samples);
            }
        }
        all
    }

    /// A station-keeping pod's plume: hot monopropellant exhaust (γ about
    /// 1.25, so a narrower `cos⁸` lobe) from the pod's face.
    #[must_use]
    pub fn keeping_plume(pod: DVec3, exhaust: DVec3) -> Exhaust {
        Exhaust {
            exponent: 8.0,
            range: 60.0,
            ..Exhaust::nitrogen(pod + exhaust * 0.35, exhaust, KEEPING_THRUST)
        }
    }

    /// Note thrusters that began firing this step.
    fn record_pulses(&mut self, fired: &[(u32, DVec3, DVec3)], keeping: bool, tick: u64) {
        let mut firing = 0_u32;
        let mut started = Vec::new();
        for (index, force, at) in fired {
            firing |= 1 << index;
            let thrust = force.length();
            if thrust > 0.0 {
                started.push((*index, *at, -*force / thrust, thrust));
            }
        }
        if keeping {
            for (k, pod) in KEEPING_PODS.iter().enumerate() {
                let index = KEEPING_THRUSTER + k as u32;
                firing |= 1 << index;
                let plume = Self::keeping_plume(*pod, self.keeping_exhaust);
                started.push((index, plume.origin, plume.axis, plume.thrust));
            }
        }
        for (index, pos, dir, thrust) in started {
            if self.firing & (1 << index) == 0 {
                self.pulses.push(PlumePulse {
                    thruster: index,
                    pos,
                    dir,
                    tick,
                    thrust,
                    seed: pulse_seed(index, tick),
                });
            }
        }
        self.firing = firing;
        let memory = (PULSE_MEMORY / PHYSICS_DT).round() as u64;
        let now = self.world.tick;
        self.pulses.retain(|p| p.tick + memory >= now);
    }

    /// Recent thruster onsets, oldest first, from the last
    /// [`PULSE_MEMORY`] seconds. The rendered instant is
    /// `(world.tick − 1 + alpha) × PHYSICS_DT`, so a pulse's age there is
    /// that minus `tick × PHYSICS_DT`.
    #[must_use]
    pub fn plume_pulses(&self) -> &[PlumePulse] {
        &self.pulses
    }

    /// Every line's rope: the safety tether first, then each part's depot
    /// line in [`PartKind::ALL`] order. Points run from the station anchor
    /// (the airlock or the depot) to the free end (the astronaut's or the
    /// part's center of mass), in scene coordinates, which the station's
    /// physics world uses directly.
    #[must_use]
    pub fn ropes(&self) -> Vec<RopeView<'_>> {
        self.lines
            .iter()
            .map(|line| {
                let pulling = self
                    .world
                    .joint(line.joint)
                    .map_or(0.0, |j| j.impulse.length() / PHYSICS_DT);
                RopeView {
                    points: &line.rope.pos,
                    prev: &line.rope.prev,
                    taut: pulling > 0.0 || line.rope.taut(1e-3),
                    tension: if pulling > 0.0 {
                        pulling
                    } else {
                        line.rope.tension(PHYSICS_DT)
                    },
                    length: line.rope.length,
                }
            })
            .collect()
    }

    /// The deflected shape of solar array wing `wing`: 0 is the −x wing and
    /// 1 the +x wing. Interpolated with [`Station::alpha`]. See
    /// [`ArrayFlex::displacement`].
    #[must_use]
    pub fn array_flex(&self, wing: usize) -> ArrayFlex {
        self.wings[wing.min(1)].flex(self.alpha())
    }

    /// Lay every line's rope straight from its anchor to its end, at rest,
    /// with the reel taken in to the gap. For scripted setups that move
    /// bodies directly.
    pub fn settle_lines(&mut self) {
        for i in 0..self.lines.len() {
            let Some(joint) = self.world.joint(self.lines[i].joint).copied() else {
                continue;
            };
            let (a, b) = joint.anchors(&self.world);
            let line = &mut self.lines[i];
            let length = a.distance(b).clamp(REEL_MIN, line.max_length);
            let count = line.rope.pos.len();
            line.rope = Rope::new(a, b, length, count, line.rope.settings);
        }
    }

    /// After the world step: reel each line to its free end and step its
    /// rope. With [`Station::rope_coupling`], a rope whose free end is in
    /// the free system pulls it, the rope's momentum joins the system, and
    /// the pull at the station anchor enters the ledger under `tether`.
    fn step_lines(&mut self, dt: f64) {
        let (c2, tidal) = (self.orbit.l1.c2, self.tide);
        let field = move |p: DVec3, v: DVec3| {
            if tidal { tide(c2, p, v) } else { DVec3::ZERO }
        };
        let origin = self.ledger.origin;
        for i in 0..self.lines.len() {
            let Some(joint) = self.world.joint(self.lines[i].joint).copied() else {
                continue;
            };
            let coupled = self.rope_coupling && self.in_system(self.lines[i].body);
            let (a, b) = joint.anchors(&self.world);
            let line = &mut self.lines[i];
            let before = line.rope.momentum(origin);
            line.rope.length = if joint.impulse == DVec3::ZERO {
                reel(line.rope.length, a.distance(b), line.max_length, dt)
            } else {
                line.max_length
            };
            // Paid-out line starts at rest on the reel.
            let reeled = line.rope.momentum(origin) - before;
            line.rope.coupled = coupled;
            line.rope.step_between(
                &mut self.world,
                (joint.a, joint.anchor_a),
                (joint.b, joint.anchor_b),
                &field,
            );
            if self.rope_coupling {
                let rope = &self.lines[i].rope;
                if reeled != Momentum::ZERO {
                    self.ledger.add("reel", reeled);
                }
                let mut pinned = vec![0];
                if !coupled {
                    pinned.push(1);
                }
                for end in pinned {
                    self.ledger.add(
                        "tether",
                        Momentum {
                            linear: -rope.end_impulse[end],
                            angular: -rope.end_moment[end],
                        },
                    );
                }
            }
        }
    }

    /// Momentum of the free system, about the station origin: the astronaut
    /// with its propellant, and every part that is not stowed or installed.
    #[must_use]
    pub fn momentum(&self) -> Momentum {
        let mut astronaut = *self.astronaut();
        astronaut.mass = DRY_MASS + self.propellant;
        let mut total = Momentum::of(&astronaut, self.ledger.origin);
        for part in &self.parts {
            if matches!(part.state, PartState::Stowed | PartState::Installed) {
                continue;
            }
            total += Momentum::of(self.body(part), self.ledger.origin);
        }
        if self.rope_coupling {
            for line in &self.lines {
                total += line.rope.momentum(self.ledger.origin);
            }
        }
        total
    }

    /// Start a fresh momentum ledger from the current state.
    pub fn reset_ledger(&mut self) {
        self.ledger = Ledger::new(DVec3::ZERO, self.momentum());
    }

    /// Record the momentum change since `before` as external term `term`.
    fn account(&mut self, term: &str, before: Momentum) {
        let change = self.momentum() - before;
        if change != Momentum::ZERO {
            self.ledger.add(term, change);
        }
    }

    fn emit_plumes(&mut self, fired: &[(DVec3, DVec3)], dt: f64) {
        for p in &mut self.plumes {
            p.age += dt;
        }
        self.plumes.retain(|p| p.age < 0.35);
        self.plume_clock -= dt;
        if self.plume_clock > 0.0 {
            return;
        }
        self.plume_clock = 0.03;
        for (force, at) in fired {
            if force.length() < 0.5 || self.plumes.len() >= 64 {
                continue;
            }
            let exhaust = -force.normalize();
            self.plumes.push(Plume {
                pos: (*at + exhaust * 0.1).to_array(),
                dir: exhaust.to_array(),
                age: 0.0,
            });
        }
    }

    /// After a world step: reel in a drifting part whose line gave way.
    fn settle_parts(&mut self) {
        for i in 0..self.parts.len() {
            if self.parts[i].state != PartState::Drifting {
                continue;
            }
            let kind = self.parts[i].kind;
            let before = self.momentum();
            let body = &mut self.world[self.parts[i].body];
            if body.pos.distance(DEPOT) > PART_TETHER + TETHER_MARGIN {
                *body = Body::new(kind.mass(), kind.inertia(), kind.stowage());
                self.set_state(i, PartState::Stowed);
                self.message = Some(format!(
                    "Tether reeled the {} back to the depot",
                    kind.name().to_lowercase()
                ));
            }
            self.account("reel", before);
        }
    }

    /// Take the nearest free part, or the next part at the depot. The glove
    /// closes on the point of the part nearest the hands and holds it with a
    /// soft weld ([`GRIP_FREQUENCY`]), so capture is an internal impulse and
    /// momentum is conserved; a heavy part drags on the astronaut.
    pub fn grab(&mut self) -> Result<PartKind, String> {
        if self.carried().is_some() {
            return Err("Hands are full".into());
        }
        let hands = self.hands();
        let free = self
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| p.state == PartState::Drifting)
            .map(|(i, p)| (i, reach(p, self.body(p), hands)))
            .filter(|(_, d)| *d <= GRAB_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        let index = match free {
            Some(i) => i,
            None => {
                let next = self
                    .parts
                    .iter()
                    .position(|p| p.state == PartState::Stowed)
                    .ok_or("Every part is out of the depot")?;
                let part = &self.parts[next];
                if reach(part, self.body(part), hands) > GRAB_RANGE + 1.5 {
                    return Err(format!(
                        "Fly to the depot for the {}",
                        self.parts[next].kind.name().to_lowercase()
                    ));
                }
                next
            }
        };
        let kind = self.parts[index].kind;
        // Reach from the hands toward the part's center; the glove closes where
        // that ray meets the part, or on its nearest point if the hands are
        // already inside it.
        let target = self.parts[index].collider;
        let body = self.parts[index].body;
        let center = self.world[body].pos;
        let grabbed = self
            .world
            .raycast(hands, center - hands, GRAB_RANGE + 10.0, &|c| {
                c.body == body
            })
            .filter(|hit| hit.distance > 0.0)
            .map_or_else(
                || self.world.colliders()[target.0 as usize].closest_point(&self.world, hands),
                |hit| hit.point,
            );
        self.set_state(index, PartState::Carried);
        let grip = Joint::weld_here(&self.world, self.astronaut, self.parts[index].body, grabbed)
            .soft(GRIP_FREQUENCY, 1.0)
            .limited(GRIP_FORCE, GRIP_TORQUE);
        self.grip = Some(self.world.add_joint(grip));
        self.target = None;
        self.message = Some(format!(
            "Holding the {} ({:.0} kg)",
            kind.name().to_lowercase(),
            kind.mass()
        ));
        Ok(kind)
    }

    /// Let go of the grip, if any.
    fn let_go(&mut self) {
        if let Some(grip) = self.grip.take() {
            self.world.remove_joint(grip);
        }
    }

    /// After a world step: a grip pulled past its force or torque limit
    /// slips, and the part floats free with the motion it has.
    fn check_grip(&mut self) {
        let Some(grip) = self.grip else { return };
        if !self.world.joint(grip).is_some_and(|j| j.saturated) {
            return;
        }
        self.let_go();
        if let Some(index) = self.carried() {
            self.set_state(index, PartState::Drifting);
            self.message = Some(format!(
                "The grip slipped; the {} floats free",
                self.parts[index].kind.name().to_lowercase()
            ));
        }
    }

    /// Move the astronaut and anything it holds by `offset`, as one rigid
    /// group, without changing velocities. For scripted setups.
    pub fn translate(&mut self, offset: DVec3) {
        let mut ids = vec![self.astronaut];
        ids.extend(self.carried().map(|i| self.parts[i].body));
        for id in ids {
            let body = &mut self.world[id];
            body.pos += offset;
            body.prev_pos += offset;
        }
        self.settle_lines();
    }

    /// Set the velocity of the astronaut and anything it holds. For
    /// scripted setups.
    pub fn set_velocity(&mut self, vel: DVec3) {
        let mut ids = vec![self.astronaut];
        ids.extend(self.carried().map(|i| self.parts[i].body));
        for id in ids {
            self.world[id].vel = vel;
            self.world[id].omega = DVec3::ZERO;
        }
    }

    /// How far the carried part is from latching: distance to its slot, m;
    /// alignment error, rad (the nearer of the slot's two keel-symmetric
    /// orientations); speed, m/s; and spin, rad/s. Also the orientation it
    /// would latch at.
    #[must_use]
    pub fn latch_error(&self) -> Option<(f64, f64, f64, f64, DQuat)> {
        let part = &self.parts[self.carried()?];
        let body = self.body(part);
        let (angle, seat) = [
            DQuat::IDENTITY,
            DQuat::from_rotation_z(std::f64::consts::PI),
        ]
        .into_iter()
        .map(|q| (body.orientation.angle_between(q), q))
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
        Some((
            body.pos.distance(part.kind.slot()),
            angle,
            body.vel.length(),
            body.omega.length(),
            seat,
        ))
    }

    /// Whether releasing now would latch the carried part.
    #[must_use]
    pub fn latch_ready(&self) -> bool {
        self.latch_error()
            .is_some_and(|(distance, angle, speed, spin, _)| {
                distance <= LATCH_RANGE
                    && angle <= LATCH_ANGLE
                    && speed <= LATCH_SPEED
                    && spin <= LATCH_SPIN
            })
    }

    /// Distance from the carried part to its latch, m.
    #[must_use]
    pub fn latch_distance(&self) -> Option<f64> {
        let part = &self.parts[self.carried()?];
        Some(self.body(part).pos.distance(part.kind.slot()))
    }

    /// Let go. Within latch range and below latch speed, the part locks
    /// into the jig; otherwise it floats free with its own motion.
    pub fn release(&mut self) -> Result<PartKind, String> {
        let index = self.carried().ok_or("Nothing is held")?;
        let (distance, angle, speed, spin, seat) = self.latch_error().ok_or("Nothing is held")?;
        let kind = self.parts[index].kind;
        let ready = self.latch_ready();
        self.let_go();
        if ready {
            let before = self.momentum();
            self.set_state(index, PartState::Installed);
            let weld = Joint::new(
                self.anchor,
                kind.slot(),
                self.parts[index].body,
                DVec3::ZERO,
                JointKind::Weld { relative: seat },
            );
            self.parts[index].latch = Some(self.world.add_joint(weld));
            self.account("latch", before);
            let installed = self
                .parts
                .iter()
                .filter(|p| p.state == PartState::Installed)
                .count();
            self.message = Some(if installed == self.parts.len() {
                "Keel complete: the ship frame is latched on the jig".into()
            } else {
                format!(
                    "{} latched · {installed} of {}",
                    kind.name(),
                    self.parts.len()
                )
            });
        } else {
            // The part keeps the motion the grip left it: its own velocity
            // and spin.
            self.set_state(index, PartState::Drifting);
            let name = kind.name().to_lowercase();
            self.message = Some(if distance > LATCH_RANGE {
                format!("The {name} floats free")
            } else if speed > LATCH_SPEED {
                format!("Too fast to latch; the {name} floats free")
            } else if spin > LATCH_SPIN {
                format!("Spinning too fast to latch; the {name} floats free")
            } else {
                format!(
                    "Misaligned by {:.0} degrees; the {name} floats free",
                    angle.to_degrees()
                )
            });
        }
        Ok(kind)
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let carrying = self.carried().map(|i| self.parts[i].kind);
        let hands = self.hands();
        let latch_distance = self.latch_distance();
        let can_grab = carrying.is_none()
            && (self.parts.iter().any(|p| {
                p.state == PartState::Drifting && reach(p, self.body(p), hands) <= GRAB_RANGE
            }) || self
                .parts
                .iter()
                .find(|p| p.state == PartState::Stowed)
                .is_some_and(|p| reach(p, self.body(p), hands) <= GRAB_RANGE + 1.5));
        Snapshot {
            orbit: self.orbit.snapshot(),
            propellant_kg: self.propellant,
            propellant_fraction: self.propellant / PROPELLANT,
            delta_v_remaining_m_s: self.delta_v_remaining(),
            speed_m_s: self.astronaut().vel.length(),
            mass_kg: self.mass(),
            range_m: self.astronaut().pos.distance(AIRLOCK),
            carrying,
            installed: self
                .parts
                .iter()
                .filter(|p| p.state == PartState::Installed)
                .count(),
            total: self.parts.len(),
            next_part: self.next_part(),
            can_grab,
            latch_ready: self.latch_ready(),
            latch_angle_deg: self.latch_error().map(|e| e.1.to_degrees()),
            awake_bodies: self.world.stats.awake,
            step_ms: self.world.stats.total.as_secs_f64() * 1_000.0,
            impact_n: self.impact(),
            g_load: self.imu.reading.specific_force.length() / G0,
            spin_deg_s: self.astronaut().omega.length().to_degrees(),
            proximity_m: self.proximity(),
            latch_distance_m: latch_distance,
            refilling: self.refilling,
            keeping_active: self.keeping_glow > 0.0,
            message: self.message.clone(),
        }
    }
}

/// Unit facing on the XZ plane for a yaw; +Z at zero.
#[must_use]
pub fn heading(yaw: f64) -> DVec3 {
    DVec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// The station's attitude: pitched 30° about its truss (x) axis so the Sun
/// stands 30° above the −z axis. Radiators on ±x stay edge-on to the Sun and
/// the fixed arrays still see cos 30° of full sunlight, while module sides and
/// the truss catch light instead of standing exactly along the Sun line.
pub const PITCH: f64 = 30.0 * std::f64::consts::PI / 180.0;

/// Maps a vector from rotating-frame scene axes (`y`, `z`, `x` of the
/// rotating frame: along-track, north, Sun to Earth) into the station body
/// axes that the scene, the physics world, and the renderer use.
#[must_use]
pub fn attitude() -> DQuat {
    DQuat::from_rotation_x(PITCH)
}

/// Relative acceleration near L1 for Richardson's `c2`: the linearized
/// restricted three-body field with Coriolis terms. Positions, velocities,
/// and the result are in station body axes; the field is evaluated in the
/// rotating frame's axes through [`attitude`].
#[must_use]
pub fn tide(c2: f64, pos: DVec3, vel: DVec3) -> DVec3 {
    let n = mean_motion();
    let q = attitude();
    let (pos, vel) = (q.inverse() * pos, q.inverse() * vel);
    q * DVec3::new(
        -2.0 * n * vel.z + n * n * (1.0 - c2) * pos.x,
        -n * n * c2 * pos.y,
        2.0 * n * vel.x + n * n * (1.0 + 2.0 * c2) * pos.z,
    )
}

/// Rope settings for the safety tether and the part lines: light webbing
/// that barely stretches and hardly resists bending.
fn line_settings() -> RopeSettings {
    RopeSettings {
        linear_density: LINE_DENSITY,
        ..RopeSettings::default()
    }
}

/// A reel's paid-out length: it pays out as fast as the end moves away,
/// takes in slack at [`REEL_SPEED`], and holds between [`REEL_MIN`] and
/// the line's full length.
#[must_use]
pub fn reel(length: f64, distance: f64, max_length: f64, dt: f64) -> f64 {
    (length - REEL_SPEED * dt)
        .max(distance)
        .clamp(REEL_MIN, max_length)
}

/// A pulse's particle seed: SplitMix64 of the thruster and the tick.
#[must_use]
pub fn pulse_seed(thruster: u32, tick: u64) -> u64 {
    let mut z = (u64::from(thruster) << 48 ^ tick).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Distance from the hands to a part's nearest envelope surface, roughly.
fn reach(part: &Part, body: &Body, hands: DVec3) -> f64 {
    (body.pos.distance(hands) - part.kind.size().max_element() * 0.5).max(0.0)
}

const fn enabled() -> bool {
    true
}
