//! The L1 construction station: a crewed EVA with a cold-gas maneuvering
//! pack, free-flying rigid parts, and a keel jig where a ship frame begins.
//!
//! Scene axes: +Y is the ecliptic north pole, -Z points at the Sun, +Z at the
//! Earth, and +X completes a right-handed frame (the rotating frame's +y,
//! the direction of Earth's orbital motion). Units are SI.

use glam::{DQuat, DVec3};
use serde::Serialize;

use crate::body::RigidBody;
use crate::orbit::{StationOrbit, mean_motion};

/// Standard gravity, used only to convert specific impulse, m/s^2.
pub const G0: f64 = 9.806_65;
/// Suited astronaut plus maneuvering pack without propellant, kg.
pub const DRY_MASS: f64 = 230.0;
/// Full nitrogen load, kg.
pub const PROPELLANT: f64 = 20.0;
/// Net thrust available along any commanded direction, N.
pub const THRUST: f64 = 40.0;
/// Cold nitrogen specific impulse, s.
pub const ISP: f64 = 70.0;
/// Flight-control speed limit relative to the station, m/s.
pub const SPEED_LIMIT: f64 = 2.0;
/// Velocity errors below this are left alone (minimum impulse), m/s.
pub const VELOCITY_DEADBAND: f64 = 0.004;
/// Safety tether range from the station center of mass, m.
pub const EVA_RANGE: f64 = 140.0;
/// Refill port reach at the airlock, m.
pub const REFILL_RANGE: f64 = 3.5;
/// Refill rate at the airlock, kg/s.
pub const REFILL_RATE: f64 = 2.0;
/// Reach for grabbing a free part, m, measured from the hands.
pub const GRAB_RANGE: f64 = 3.0;
/// Latch capture distance, m, and maximum closing speed, m/s.
pub const LATCH_RANGE: f64 = 1.6;
pub const LATCH_SPEED: f64 = 0.35;
/// Orbital seconds per local second. The local rigid-body clock is real time.
pub const ORBIT_WARP: f64 = 3_600.0;
/// Parts that drift farther than this from the depot are reeled back, m.
pub const PART_TETHER: f64 = 120.0;

/// The airlock refill port.
pub const AIRLOCK: DVec3 = DVec3::new(0.0, 6.0, 17.5);
/// The parts depot, where the next needed part waits.
pub const DEPOT: DVec3 = DVec3::new(-12.0, -6.0, 1.0);
/// Center line of the keel jig.
pub const JIG: DVec3 = DVec3::new(0.0, -6.0, 0.0);
/// Where a new EVA starts (body center), beside the airlock facing the station.
pub const SPAWN: DVec3 = DVec3::new(3.0, 5.3, 21.0);
/// Radius of the astronaut's collision sphere around the body center, m.
pub const ASTRONAUT_RADIUS: f64 = 0.9;

/// Ship frame components, in keel order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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

    #[must_use]
    pub fn inertia(self) -> DVec3 {
        match self {
            Self::PropellantTank => RigidBody::shell_inertia(self.mass(), 1.3, 3.6),
            _ => RigidBody::box_inertia(self.mass(), self.size()),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartState {
    Stowed,
    Carried,
    Drifting,
    Installed,
}

#[derive(Clone, Copy, Debug)]
pub struct Part {
    pub kind: PartKind,
    pub body: RigidBody,
    pub state: PartState,
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
#[derive(Clone, Copy, Debug, Default)]
pub struct Command {
    /// Desired translation direction; zero holds position.
    pub direction: DVec3,
    /// Heading in radians, counterclockwise around +Y; +Z at zero.
    pub yaw: f64,
    /// A short climb along +Y (the jump gesture).
    pub climb: bool,
}

/// A visible thruster firing, for rendering only.
#[derive(Clone, Copy, Debug, Serialize)]
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
    pub refilling: bool,
    /// A station-keeping burn fired within the last second.
    pub keeping_active: bool,
    pub message: Option<String>,
}

/// The whole L1 construction simulation.
#[derive(Clone, Debug)]
pub struct Station {
    pub orbit: StationOrbit,
    /// The suited astronaut; `pos` is the body center, 0.9 m above the boots.
    pub astronaut: RigidBody,
    pub yaw: f64,
    pub propellant: f64,
    pub parts: Vec<Part>,
    pub plumes: Vec<Plume>,
    /// Remaining seconds of the station-keeping thruster glow.
    pub keeping_glow: f64,
    pub target: Option<DVec3>,
    pub message: Option<String>,
    pub refilling: bool,
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
        let parts = PartKind::ALL
            .iter()
            .map(|&kind| Part {
                kind,
                body: RigidBody::new(kind.mass(), kind.inertia(), kind.stowage()),
                state: PartState::Stowed,
            })
            .collect();
        Self {
            orbit: StationOrbit::new(),
            astronaut: RigidBody::new(DRY_MASS + PROPELLANT, DVec3::splat(40.0), SPAWN),
            yaw: std::f64::consts::PI,
            propellant: PROPELLANT,
            parts,
            plumes: Vec::new(),
            keeping_glow: 0.0,
            target: None,
            message: None,
            refilling: false,
            climb: 0.0,
            plume_clock: 0.0,
        }
    }

    /// Relative acceleration near L1: the linearized restricted three-body
    /// field, with Coriolis terms, mapped into scene axes. At station scale it
    /// is about 1e-11 m/s^2; it is kept so free parts obey the real field.
    #[must_use]
    pub fn field(&self, pos: DVec3, vel: DVec3) -> DVec3 {
        let n = mean_motion();
        let c2 = self.orbit.l1.c2;
        DVec3::new(
            -2.0 * n * vel.z + n * n * (1.0 - c2) * pos.x,
            -n * n * c2 * pos.y,
            2.0 * n * vel.x + n * n * (1.0 + 2.0 * c2) * pos.z,
        )
    }

    fn carried(&self) -> Option<usize> {
        self.parts
            .iter()
            .position(|p| p.state == PartState::Carried)
    }

    /// Total mass the pack must accelerate, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        DRY_MASS + self.propellant + self.carried().map_or(0.0, |i| self.parts[i].body.mass)
    }

    /// Where the gloves are: in front of the chest.
    #[must_use]
    pub fn hands(&self) -> DVec3 {
        self.astronaut.pos + heading(self.yaw) * 1.0 + DVec3::Y * 0.2
    }

    fn carry_point(&self, kind: PartKind) -> DVec3 {
        self.astronaut.pos
            + heading(self.yaw) * (1.0 + kind.size().z.max(kind.size().x) * 0.5)
            + DVec3::Y * 0.2
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
        if !target.is_finite() || target.length() > EVA_RANGE {
            return Err("That point is beyond the safety tether".into());
        }
        self.target = Some(target);
        Ok(())
    }

    /// Advance local physics by `dt` real seconds and the orbit by
    /// `dt * ORBIT_WARP` mission seconds.
    pub fn step(&mut self, dt: f64, command: &Command) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.1);
        if self.orbit.advance(dt * ORBIT_WARP) > 0 {
            self.keeping_glow = 1.2;
        }
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
        let mass = self.mass();
        let accel_limit = THRUST / mass;
        if !manual && let Some(target) = self.target {
            let offset = target - self.astronaut.pos;
            let distance = offset.length();
            if distance < 0.3 && self.astronaut.vel.length() < 0.05 {
                self.target = None;
            } else {
                // Leave margin so the braking burn fits inside the thrust limit.
                let speed = SPEED_LIMIT.min((accel_limit * distance).sqrt());
                desired = offset.normalize_or_zero() * speed;
            }
        }
        let error = desired - self.astronaut.vel;
        let mut thrust = DVec3::ZERO;
        if error.length() > VELOCITY_DEADBAND && self.propellant > 0.0 {
            let wanted = error / dt;
            thrust = wanted.clamp_length_max(accel_limit);
            let used = (thrust.length() * mass / (ISP * G0) * dt).min(self.propellant);
            self.propellant -= used;
        }
        let field = self.field(self.astronaut.pos, self.astronaut.vel);
        self.astronaut.vel += (thrust + field) * dt;
        self.astronaut.pos += self.astronaut.vel * dt;
        self.astronaut.mass = self.mass();
        collide(&mut self.astronaut, ASTRONAUT_RADIUS);
        let range = self.astronaut.pos.length();
        if range > EVA_RANGE {
            let out = self.astronaut.pos / range;
            self.astronaut.pos = out * EVA_RANGE;
            let radial = self.astronaut.vel.dot(out);
            if radial > 0.0 {
                self.astronaut.vel -= out * radial;
            }
            self.message = Some("Safety tether taut".into());
        }
        self.refilling = self.astronaut.pos.distance(AIRLOCK) <= REFILL_RANGE
            && self.astronaut.vel.length() < 0.6
            && self.propellant < PROPELLANT;
        if self.refilling {
            self.propellant = (self.propellant + REFILL_RATE * dt).min(PROPELLANT);
        }
        self.emit_plumes(thrust, dt);
        self.step_parts(dt);
    }

    fn emit_plumes(&mut self, thrust: DVec3, dt: f64) {
        for p in &mut self.plumes {
            p.age += dt;
        }
        self.plumes.retain(|p| p.age < 0.35);
        self.plume_clock -= dt;
        if thrust.length() < 1e-3 || self.plume_clock > 0.0 || self.plumes.len() >= 64 {
            return;
        }
        self.plume_clock = 0.03;
        let exhaust = -thrust.normalize();
        let pack = self.astronaut.pos + DVec3::Y * 0.3 - heading(self.yaw) * 0.45;
        self.plumes.push(Plume {
            pos: (pack + exhaust * 0.5).to_array(),
            dir: exhaust.to_array(),
            age: 0.0,
        });
    }

    fn step_parts(&mut self, dt: f64) {
        let carried = self.carried();
        if let Some(i) = carried {
            let kind = self.parts[i].kind;
            let at = self.carry_point(kind);
            let body = &mut self.parts[i].body;
            body.pos = at;
            body.vel = self.astronaut.vel;
            body.orientation = DQuat::from_rotation_y(self.yaw);
            body.omega = DVec3::ZERO;
        }
        for i in 0..self.parts.len() {
            if self.parts[i].state != PartState::Drifting {
                continue;
            }
            let field = self.field(self.parts[i].body.pos, self.parts[i].body.vel);
            let part = &mut self.parts[i];
            part.body.vel += field * dt;
            part.body.pos += part.body.vel * dt;
            part.body.rotate(dt);
            let radius = part.kind.size().min_element() * 0.5;
            collide(&mut part.body, radius);
            if part.body.pos.distance(DEPOT) > PART_TETHER {
                part.state = PartState::Stowed;
                part.body =
                    RigidBody::new(part.kind.mass(), part.kind.inertia(), part.kind.stowage());
                self.message = Some(format!(
                    "Tether reeled the {} back to the depot",
                    part.kind.name().to_lowercase()
                ));
            }
        }
    }

    /// Take the nearest free part, or the next part at the depot.
    /// Capture is perfectly inelastic, so combined momentum is conserved.
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
            .map(|(i, p)| (i, reach(p, hands)))
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
                if reach(&self.parts[next], hands) > GRAB_RANGE + 1.5 {
                    return Err(format!(
                        "Fly to the depot for the {}",
                        self.parts[next].kind.name().to_lowercase()
                    ));
                }
                next
            }
        };
        let part = &mut self.parts[index];
        let m_a = self.astronaut.mass;
        let m_p = part.body.mass;
        self.astronaut.vel = (self.astronaut.vel * m_a + part.body.vel * m_p) / (m_a + m_p);
        part.state = PartState::Carried;
        self.target = None;
        self.message = Some(format!(
            "Holding the {} ({:.0} kg)",
            part.kind.name().to_lowercase(),
            m_p
        ));
        let kind = part.kind;
        self.astronaut.mass = self.mass();
        Ok(kind)
    }

    /// Distance from the carried part to its latch, m.
    #[must_use]
    pub fn latch_distance(&self) -> Option<f64> {
        let part = &self.parts[self.carried()?];
        Some(part.body.pos.distance(part.kind.slot()))
    }

    /// Let go. Within latch range and below latch speed, the part locks
    /// into the jig; otherwise it floats free and tumbles.
    pub fn release(&mut self) -> Result<PartKind, String> {
        let index = self.carried().ok_or("Nothing is held")?;
        let distance = self.latch_distance().unwrap_or(f64::MAX);
        let part = &mut self.parts[index];
        let kind = part.kind;
        if distance <= LATCH_RANGE && self.astronaut.vel.length() <= LATCH_SPEED {
            part.state = PartState::Installed;
            part.body.pos = kind.slot();
            part.body.vel = DVec3::ZERO;
            part.body.orientation = DQuat::IDENTITY;
            part.body.omega = DVec3::ZERO;
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
            part.state = PartState::Drifting;
            part.body.vel = self.astronaut.vel;
            // Real hands never let go perfectly; a small residual rate
            // shows the free rigid-body motion.
            part.body.omega = DVec3::new(0.012, 0.035, -0.02);
            self.message = Some(if distance <= LATCH_RANGE {
                format!(
                    "Too fast to latch; the {} floats free",
                    kind.name().to_lowercase()
                )
            } else {
                format!("The {} floats free", kind.name().to_lowercase())
            });
        }
        self.astronaut.mass = self.mass();
        Ok(kind)
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let carrying = self.carried().map(|i| self.parts[i].kind);
        let hands = self.hands();
        let latch_distance = self.latch_distance();
        let can_grab = carrying.is_none()
            && (self
                .parts
                .iter()
                .any(|p| p.state == PartState::Drifting && reach(p, hands) <= GRAB_RANGE)
                || self
                    .parts
                    .iter()
                    .find(|p| p.state == PartState::Stowed)
                    .is_some_and(|p| reach(p, hands) <= GRAB_RANGE + 1.5));
        Snapshot {
            orbit: self.orbit.snapshot(),
            propellant_kg: self.propellant,
            propellant_fraction: self.propellant / PROPELLANT,
            delta_v_remaining_m_s: self.delta_v_remaining(),
            speed_m_s: self.astronaut.vel.length(),
            mass_kg: self.mass(),
            range_m: self.astronaut.pos.length(),
            carrying,
            installed: self
                .parts
                .iter()
                .filter(|p| p.state == PartState::Installed)
                .count(),
            total: self.parts.len(),
            next_part: self.next_part(),
            can_grab,
            latch_ready: latch_distance.is_some_and(|d| d <= LATCH_RANGE)
                && self.astronaut.vel.length() <= LATCH_SPEED,
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

/// Distance from the hands to a part's nearest envelope surface, roughly.
fn reach(part: &Part, hands: DVec3) -> f64 {
    (part.body.pos.distance(hands) - part.kind.size().max_element() * 0.5).max(0.0)
}

/// Push a sphere out of solid structure and remove its closing velocity,
/// with a soft 0.2 restitution.
fn collide(body: &mut RigidBody, radius: f64) {
    for o in &OBSTACLES {
        let nearest = body.pos.clamp(o.min, o.max);
        let offset = body.pos - nearest;
        let distance = offset.length();
        if distance >= radius {
            continue;
        }
        let (surface, normal) = if distance > 1e-9 {
            (nearest, offset / distance)
        } else {
            // Inside: leave through the shallowest face.
            let center = (o.min + o.max) * 0.5;
            let half = (o.max - o.min) * 0.5;
            let d = body.pos - center;
            let depth = half - d.abs();
            let mut surface = body.pos;
            let normal = if depth.x <= depth.y && depth.x <= depth.z {
                surface.x = center.x + half.x * d.x.signum();
                DVec3::X * d.x.signum()
            } else if depth.y <= depth.z {
                surface.y = center.y + half.y * d.y.signum();
                DVec3::Y * d.y.signum()
            } else {
                surface.z = center.z + half.z * d.z.signum();
                DVec3::Z * d.z.signum()
            };
            (surface, normal)
        };
        body.pos = surface + normal * radius;
        let closing = body.vel.dot(normal);
        if closing < 0.0 {
            body.vel -= normal * closing * 1.2;
        }
    }
}
