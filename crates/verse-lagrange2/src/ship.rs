//! The spacecraft document: a typed, versioned description of a ship built
//! from parts, with its derived mass properties and the checks every edit
//! must pass.
//!
//! Frames: the ship frame has +X forward along the thrust axis, +Y up when
//! the ship lies in the factory's cradle, +Z to starboard. A part's frame is
//! centered on the part; cylinders run along the part's X axis. Parts are
//! turned only about Y (yaw), as the factory arms place them.
//!
//! Every part attaches to one parent through a latch at a point in the ship
//! frame, except the root, which latches to the factory's cradle. The
//! attachments form a tree, which is also the assembly precedence: a part
//! goes on after its parent.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::cad_feed::{self, MassFeed};

/// Standard gravity, m/s^2 (exact by definition).
pub const G0: f64 = 9.806_65;
/// The document's schema tag, as Robot Lab's `robot.v1` and its siblings.
pub const SCHEMA: &str = "spacecraft.v1";

/// What a part is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PartKind {
    Truss,
    Module,
    /// A propellant tank, loaded with `propellant_kg`.
    Tank {
        propellant_kg: f64,
    },
    /// A main engine firing along the part's +X axis (its exhaust leaves
    /// along -X), pushing the ship forward.
    Engine {
        thrust_n: f64,
        isp_s: f64,
    },
}

/// The subsystem a part's mass is booked against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subsystem {
    Structure,
    Propulsion,
    Habitat,
    Power,
    Thermal,
}

/// Design maturity, which sets the mass contingency the budget carries
/// (AIAA S-120 style classes; see docs/spacecraft-design/design-method.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Maturity {
    /// Flown hardware: 2%.
    Flown,
    /// Modified from flown hardware: 10%.
    Modified,
    /// A new design: 20%.
    New,
}

impl Maturity {
    #[must_use]
    pub fn contingency(self) -> f64 {
        match self {
            Self::Flown => 0.02,
            Self::Modified => 0.10,
            Self::New => 0.20,
        }
    }
}

/// A part's shape, which sets its inertia and what the arms grasp.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Shape {
    /// A solid box with these full edge lengths, m.
    Box { size: DVec3 },
    /// A solid cylinder along the part's X axis, m.
    Cylinder { radius: f64, length: f64 },
}

impl Shape {
    /// Principal moments about the part's center, part frame, kg m^2.
    #[must_use]
    pub fn inertia(self, mass: f64) -> DVec3 {
        match self {
            Self::Box { size } => physics::Body::box_inertia(mass, size),
            Self::Cylinder { radius, length } => {
                let across = mass * (3.0 * radius * radius + length * length) / 12.0;
                DVec3::new(mass * radius * radius / 2.0, across, across)
            }
        }
    }

    /// Half extents of the shape's bounding box, part frame, m.
    #[must_use]
    pub fn half_extents(self) -> DVec3 {
        match self {
            Self::Box { size } => size / 2.0,
            Self::Cylinder { radius, length } => DVec3::new(length / 2.0, radius, radius),
        }
    }
}

/// A part's latch to its parent: where, and what it is rated to carry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attach {
    /// The parent part's id, or `None` for the factory's cradle.
    pub parent: Option<String>,
    /// The latch point, ship frame, m.
    pub at: DVec3,
    /// Rated force, N.
    pub rated_force_n: f64,
    /// Rated bending moment, N m.
    pub rated_moment_nm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    #[serde(flatten)]
    pub kind: PartKind,
    pub subsystem: Subsystem,
    pub maturity: Maturity,
    /// Mass without propellant, kg.
    pub dry_mass_kg: f64,
    #[serde(flatten)]
    pub shape: Shape,
    /// Center of the part, ship frame, m.
    pub position: DVec3,
    /// Turn about the ship's Y axis, rad.
    #[serde(default)]
    pub yaw: f64,
    pub attach: Attach,
    /// The dry structure's measured mass properties, when the part was
    /// built in CAD Lab: then `position` is its measured center of mass and
    /// its inertia is the measured tensor rather than the shape's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cad: Option<CadMass>,
}

/// A part's dry mass properties as CAD Lab measured them
/// (`cad-mass-feed.v1`, [`crate::cad_feed`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadMass {
    /// `cad:DOC/vN#INSTANCE`.
    pub source: String,
    /// The CAD document version's SHA-256.
    pub digest: String,
    /// The measured dry mass, kg; the inertia scales with `dry_mass_kg`
    /// over this if an edit changes the mass.
    pub mass_kg: f64,
    /// Inertia about the center of mass, part frame, kg m^2, rows.
    pub inertia_kg_m2: [[f64; 3]; 3],
}

impl Part {
    /// Loaded mass: dry plus propellant, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.dry_mass_kg + self.propellant()
    }

    #[must_use]
    pub fn propellant(&self) -> f64 {
        match self.kind {
            PartKind::Tank { propellant_kg } => propellant_kg,
            _ => 0.0,
        }
    }

    #[must_use]
    pub fn orientation(&self) -> DQuat {
        DQuat::from_rotation_y(self.yaw)
    }

    /// Inertia tensor about the part's center at mass `mass`, ship frame.
    #[must_use]
    pub fn inertia_at(&self, mass: f64) -> DMat3 {
        let r = DMat3::from_quat(self.orientation());
        r * self.local_inertia(mass) * r.transpose()
    }

    /// Inertia tensor about the part's center at mass `mass`, part frame.
    /// A part built in CAD carries its measured dry tensor (scaled to the
    /// document's dry mass); propellant, and every part without CAD
    /// numbers, takes its shape's solid inertia.
    #[must_use]
    pub fn local_inertia(&self, mass: f64) -> DMat3 {
        match &self.cad {
            Some(cad) if cad.mass_kg > 0.0 => {
                let rows = cad.inertia_kg_m2;
                let measured =
                    DMat3::from_cols_array_2d(&rows).transpose() * (self.dry_mass_kg / cad.mass_kg);
                let fluid = (mass - self.dry_mass_kg).max(0.0);
                measured + DMat3::from_diagonal(self.shape.inertia(fluid))
            }
            _ => DMat3::from_diagonal(self.shape.inertia(mass)),
        }
    }
}

/// System-level limits the checks enforce.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirements {
    /// The most the loaded ship may weigh including contingency, kg (the
    /// requirement named "mass budget").
    pub max_mass_kg: f64,
    /// The farthest the thrust line may pass from the center of mass, m
    /// (the requirement named "thrust line").
    pub max_thrust_offset_m: f64,
    /// The test burn the ship must be able to fly, m/s.
    pub test_burn_m_s: f64,
}

/// A typed, versioned spacecraft design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spacecraft {
    pub schema: String,
    pub name: String,
    /// Bumped by every accepted edit.
    pub revision: u32,
    pub requirements: Requirements,
    pub parts: Vec<Part>,
}

/// Derived mass properties of the loaded ship.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct MassProperties {
    pub mass: f64,
    /// Ship frame, m.
    pub com: DVec3,
    /// About the center of mass, ship frame, kg m^2.
    pub inertia: DMat3,
}

/// One failed check: the requirement's name and what failed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub requirement: String,
    pub detail: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.requirement, self.detail)
    }
}

/// The load on one latch during the test burn.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LatchLoad {
    pub part: String,
    pub force_n: f64,
    pub moment_nm: f64,
    /// Fraction of the rating left on the worse of force and moment.
    pub margin: f64,
}

/// Every derived number the checks use.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub mass: MassProperties,
    pub dry_mass_kg: f64,
    pub propellant_kg: f64,
    /// Loaded mass plus the dry-mass contingency, kg.
    pub budget_mass_kg: f64,
    /// `1 - budget_mass / max_mass`.
    pub mass_margin: f64,
    pub thrust_n: f64,
    pub isp_s: f64,
    /// Rocket-equation velocity change on all the propellant, m/s.
    pub delta_v_m_s: f64,
    /// Distance from the center of mass to the thrust line, m.
    pub thrust_offset_m: f64,
    /// Acceleration at ignition, m/s^2.
    pub acceleration_m_s2: f64,
    pub latch_loads: Vec<LatchLoad>,
}

/// An edit to the design. Accepted only if the edited design passes every
/// check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum Edit {
    SetDryMass {
        part: String,
        kg: f64,
    },
    SetPropellant {
        part: String,
        kg: f64,
    },
    MovePart {
        part: String,
        position: DVec3,
    },
    AddPart {
        part: Part,
    },
    RemovePart {
        part: String,
    },
    /// Make a part `factor` times its volume, in place: each dimension
    /// grows by the cube root of `factor`, and its dry mass and any
    /// propellant scale with the volume. The part leaves its CAD document
    /// (`cad` is cleared): its inertia comes from its shape.
    ScalePart {
        part: String,
        factor: f64,
    },
    /// Several edits checked together, as one revision (both tanks at once
    /// keep the thrust line where one alone would not).
    Batch {
        edits: Vec<Edit>,
    },
}

/// What a part is beyond its measured mass: its role, propellant or
/// engine, maturity, the envelope the arms grasp and clear, and its
/// latch's rating. [`Spacecraft::from_feed`] joins it to a CAD feed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartSpec {
    /// The CAD instance's id.
    pub id: String,
    pub kind: PartKind,
    pub subsystem: Subsystem,
    pub maturity: Maturity,
    pub shape: Shape,
    pub rated_force_n: f64,
    pub rated_moment_nm: f64,
}

/// The sample tug's parts table.
#[must_use]
pub fn sample_parts() -> Vec<PartSpec> {
    let spec = |id: &str,
                kind: PartKind,
                subsystem: Subsystem,
                maturity: Maturity,
                shape: Shape,
                force: f64,
                moment: f64| PartSpec {
        id: id.into(),
        kind,
        subsystem,
        maturity,
        shape,
        rated_force_n: force,
        rated_moment_nm: moment,
    };
    let keel = Shape::Box {
        size: DVec3::new(16.0, 2.4, 2.4),
    };
    let tank = Shape::Cylinder {
        radius: 2.0,
        length: 10.0,
    };
    let tank_kind = PartKind::Tank {
        propellant_kg: 22_000.0,
    };
    use Maturity::{Flown, Modified, New};
    use Subsystem::{Habitat, Power, Propulsion, Structure, Thermal};
    vec![
        spec(
            "keel-aft",
            PartKind::Truss,
            Structure,
            Modified,
            keel,
            600_000.0,
            900_000.0,
        ),
        spec(
            "keel-fwd",
            PartKind::Truss,
            Structure,
            Modified,
            keel,
            300_000.0,
            600_000.0,
        ),
        spec(
            "engine",
            PartKind::Engine {
                thrust_n: 120_000.0,
                isp_s: 330.0,
            },
            Propulsion,
            Flown,
            Shape::Cylinder {
                radius: 2.2,
                length: 6.0,
            },
            300_000.0,
            600_000.0,
        ),
        spec(
            "tank-port",
            tank_kind,
            Propulsion,
            Modified,
            tank,
            150_000.0,
            250_000.0,
        ),
        spec(
            "tank-starboard",
            tank_kind,
            Propulsion,
            Modified,
            tank,
            150_000.0,
            250_000.0,
        ),
        spec(
            "habitat",
            PartKind::Module,
            Habitat,
            New,
            Shape::Cylinder {
                radius: 2.5,
                length: 10.0,
            },
            150_000.0,
            250_000.0,
        ),
        spec(
            "power",
            PartKind::Module,
            Power,
            Modified,
            Shape::Box {
                size: DVec3::new(6.0, 2.0, 4.0),
            },
            80_000.0,
            150_000.0,
        ),
        spec(
            "radiator",
            PartKind::Module,
            Thermal,
            Modified,
            Shape::Box {
                size: DVec3::new(6.0, 0.6, 4.0),
            },
            80_000.0,
            150_000.0,
        ),
    ]
}

impl Spacecraft {
    /// The checked-in sample: a 98 t tug with two tanks, an engine, a
    /// habitat, and power and radiator panels on a two-piece keel.
    ///
    /// Its parts' dry masses, centers of mass, inertia tensors, latch
    /// points, and attachment tree come from CAD Lab: the document
    /// `cad/lagrange-tug.json` and its feed ([`cad_feed::TUG_FEED`]).
    /// [`sample_parts`] adds what CAD does not know.
    #[must_use]
    pub fn sample() -> Self {
        Self::from_feed(
            &MassFeed::tug(),
            "Lagrange tug",
            Requirements {
                max_mass_kg: 115_000.0,
                max_thrust_offset_m: 0.25,
                test_burn_m_s: 150.0,
            },
            &sample_parts(),
            DVec3::new(-8.0, -1.2, 0.0),
        )
        .expect("the sample ship's feed matches its parts table")
    }

    /// A ship built from a CAD mass feed: one part per feed instance, in
    /// the order of `specs`, each with the feed's dry mass, center of mass
    /// (the part's position), and inertia; a part latches to the instance
    /// its CAD joint names as parent, at that joint's origin. The one part
    /// with no parent latches to the factory's cradle at `cradle_latch`.
    ///
    /// # Errors
    /// The feed and the table name different parts, or a part has more
    /// than one parent.
    pub fn from_feed(
        feed: &MassFeed,
        name: &str,
        requirements: Requirements,
        specs: &[PartSpec],
        cradle_latch: DVec3,
    ) -> Result<Self, String> {
        if feed.parts.len() != specs.len() {
            return Err(format!(
                "the feed has {} parts and the table {}",
                feed.parts.len(),
                specs.len()
            ));
        }
        let mut parts = Vec::new();
        for spec in specs {
            let measured = feed
                .part(&spec.id)
                .ok_or_else(|| format!("the feed has no part {}", spec.id))?;
            let mut parents = measured.ports.iter().filter(|p| !p.parent);
            let (parent, at) = match (parents.next(), parents.next()) {
                (None, _) => (None, cradle_latch),
                (Some(port), None) => (Some(port.mate.clone()), cad_feed::to_ship(port.origin_m)),
                _ => return Err(format!("{} has two parents", spec.id)),
            };
            let inertia = cad_feed::tensor_to_ship(measured.mass.inertia_kg_m2);
            parts.push(Part {
                id: spec.id.clone(),
                kind: spec.kind,
                subsystem: spec.subsystem,
                maturity: spec.maturity,
                dry_mass_kg: measured.mass.mass_kg,
                shape: spec.shape,
                position: cad_feed::to_ship(measured.mass.center_of_mass_m),
                yaw: 0.0,
                attach: Attach {
                    parent,
                    at,
                    rated_force_n: spec.rated_force_n,
                    rated_moment_nm: spec.rated_moment_nm,
                },
                cad: Some(CadMass {
                    source: format!(
                        "cad:{}/v{}#{}",
                        feed.source.doc, feed.source.version, measured.id
                    ),
                    digest: feed.source.digest.clone(),
                    mass_kg: measured.mass.mass_kg,
                    inertia_kg_m2: inertia.transpose().to_cols_array_2d(),
                }),
            });
        }
        Ok(Self {
            schema: SCHEMA.into(),
            name: name.into(),
            revision: 1,
            requirements,
            parts,
        })
    }

    #[must_use]
    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.id == id)
    }

    /// Mass properties of the loaded ship: the parallel-axis sum of its
    /// parts.
    #[must_use]
    pub fn mass_properties(&self) -> MassProperties {
        self.mass_properties_with(|p| p.mass())
    }

    /// Mass properties with each part's mass given by `mass`, for a ship
    /// whose tanks have partly drained. Inertia scales with each part's mass.
    pub fn mass_properties_with(&self, mass: impl Fn(&Part) -> f64) -> MassProperties {
        let masses: Vec<f64> = self.parts.iter().map(&mass).collect();
        let total: f64 = masses.iter().sum();
        let com = self
            .parts
            .iter()
            .zip(&masses)
            .map(|(p, m)| p.position * *m)
            .sum::<DVec3>()
            / total;
        let mut inertia = DMat3::ZERO;
        for (p, &m) in self.parts.iter().zip(&masses) {
            let d = p.position - com;
            inertia += p.inertia_at(m)
                + (DMat3::IDENTITY * d.length_squared()
                    - DMat3::from_cols(d * d.x, d * d.y, d * d.z))
                    * m;
        }
        MassProperties {
            mass: total,
            com,
            inertia,
        }
    }

    /// The engines: (part, thrust N, Isp s).
    fn engines(&self) -> impl Iterator<Item = (&Part, f64, f64)> {
        self.parts.iter().filter_map(|p| match p.kind {
            PartKind::Engine { thrust_n, isp_s } => Some((p, thrust_n, isp_s)),
            _ => None,
        })
    }

    /// The thrust axis: a point on it (the engine's center) and its
    /// direction, ship frame.
    #[must_use]
    pub fn thrust_line(&self) -> Option<(DVec3, DVec3)> {
        let (engine, _, _) = self.engines().next()?;
        Some((engine.position, engine.orientation() * DVec3::X))
    }

    /// The parts on the side of `part`'s latch away from the engine: the
    /// mass that latch has to push during a burn.
    pub(crate) fn pushed_side(&self, part: usize) -> BTreeSet<usize> {
        let mut subtree = BTreeSet::from([part]);
        loop {
            let before = subtree.len();
            for (i, p) in self.parts.iter().enumerate() {
                if let Some(parent) = p.attach.parent.as_deref().and_then(|id| self.index(id))
                    && subtree.contains(&parent)
                {
                    subtree.insert(i);
                }
            }
            if subtree.len() == before {
                break;
            }
        }
        let engine_inside = self
            .engines()
            .any(|(e, _, _)| self.index(&e.id).is_some_and(|i| subtree.contains(&i)));
        if engine_inside {
            (0..self.parts.len())
                .filter(|i| !subtree.contains(i))
                .collect()
        } else {
            subtree
        }
    }

    /// Latch loads at ignition: each latch accelerates the parts on its far
    /// side from the engine, `F = m a`, and carries the moment of that force
    /// about the latch, `(com - latch) x F`.
    #[must_use]
    pub fn latch_loads(&self, acceleration: DVec3) -> Vec<LatchLoad> {
        let mut loads = Vec::new();
        for (i, part) in self.parts.iter().enumerate() {
            if part.attach.parent.is_none() {
                continue;
            }
            let side = self.pushed_side(i);
            let mass: f64 = side.iter().map(|&j| self.parts[j].mass()).sum();
            if mass <= 0.0 {
                continue;
            }
            let com = side
                .iter()
                .map(|&j| self.parts[j].position * self.parts[j].mass())
                .sum::<DVec3>()
                / mass;
            let force = acceleration * mass;
            let moment = (com - part.attach.at).cross(force).length();
            let margin = (1.0 - force.length() / part.attach.rated_force_n)
                .min(1.0 - moment / part.attach.rated_moment_nm);
            loads.push(LatchLoad {
                part: part.id.clone(),
                force_n: force.length(),
                moment_nm: moment,
                margin,
            });
        }
        loads
    }

    /// Every derived number.
    #[must_use]
    pub fn report(&self) -> Report {
        let mass = self.mass_properties();
        let dry: f64 = self.parts.iter().map(|p| p.dry_mass_kg).sum();
        let propellant: f64 = self.parts.iter().map(Part::propellant).sum();
        let contingency: f64 = self
            .parts
            .iter()
            .map(|p| p.dry_mass_kg * p.maturity.contingency())
            .sum();
        let budget = mass.mass + contingency;
        let (thrust, isp) = self.engines().fold((0.0, 0.0), |(t, i), (_, thrust, isp)| {
            (t + thrust, i + thrust * isp)
        });
        let isp = if thrust > 0.0 { isp / thrust } else { 0.0 };
        let delta_v = if mass.mass > propellant {
            isp * G0 * (mass.mass / (mass.mass - propellant)).ln()
        } else {
            f64::INFINITY
        };
        let (offset, direction) = match self.thrust_line() {
            Some((point, dir)) => {
                let d = mass.com - point;
                ((d - dir * d.dot(dir)).length(), dir)
            }
            None => (f64::INFINITY, DVec3::X),
        };
        let acceleration = thrust / mass.mass;
        Report {
            mass,
            dry_mass_kg: dry,
            propellant_kg: propellant,
            budget_mass_kg: budget,
            mass_margin: 1.0 - budget / self.requirements.max_mass_kg,
            thrust_n: thrust,
            isp_s: isp,
            delta_v_m_s: delta_v,
            thrust_offset_m: offset,
            acceleration_m_s2: acceleration,
            latch_loads: self.latch_loads(direction * acceleration),
        }
    }

    /// Every failed check, in a fixed order. Empty means the design is
    /// valid.
    #[must_use]
    pub fn check(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        if self.schema != SCHEMA {
            fail(
                &mut findings,
                "schema",
                format!("expected {SCHEMA}, found {}", self.schema),
            );
        }
        // The parts: unique ids, physical numbers.
        let mut ids = BTreeSet::new();
        for p in &self.parts {
            if !ids.insert(p.id.as_str()) {
                fail(&mut findings, "part ids", format!("{} appears twice", p.id));
            }
            let positive = |v: f64| v.is_finite() && v > 0.0;
            let shape_ok = match p.shape {
                Shape::Box { size } => size.is_finite() && size.min_element() > 0.0,
                Shape::Cylinder { radius, length } => positive(radius) && positive(length),
            };
            let kind_ok = match p.kind {
                PartKind::Tank { propellant_kg } => {
                    propellant_kg.is_finite() && propellant_kg >= 0.0
                }
                PartKind::Engine { thrust_n, isp_s } => positive(thrust_n) && positive(isp_s),
                _ => true,
            };
            if !(positive(p.dry_mass_kg)
                && shape_ok
                && kind_ok
                && p.position.is_finite()
                && p.yaw.is_finite()
                && positive(p.attach.rated_force_n)
                && positive(p.attach.rated_moment_nm))
            {
                fail(
                    &mut findings,
                    "physical values",
                    format!("{} needs positive, finite mass, size, and ratings", p.id),
                );
            }
        }
        // The attachment tree: one root on the cradle, every parent present,
        // no cycles, everything connected.
        let roots: Vec<_> = self
            .parts
            .iter()
            .filter(|p| p.attach.parent.is_none())
            .collect();
        if roots.len() != 1 {
            fail(
                &mut findings,
                "assembly tree",
                format!(
                    "exactly one part latches to the cradle; found {}",
                    roots.len()
                ),
            );
        }
        for p in &self.parts {
            if let Some(parent) = &p.attach.parent
                && self.index(parent).is_none()
            {
                fail(
                    &mut findings,
                    "assembly tree",
                    format!("{} attaches to {parent}, which is not in the ship", p.id),
                );
            }
        }
        if findings.is_empty() && self.assembly_order().is_none() {
            fail(
                &mut findings,
                "assembly tree",
                "the attachments loop or leave parts unconnected".into(),
            );
        }
        if self.engines().count() == 0 {
            fail(&mut findings, "propulsion", "the ship has no engine".into());
        }
        if !findings.is_empty() {
            return findings;
        }
        let report = self.report();
        if report.mass_margin < 0.0 {
            fail(
                &mut findings,
                "mass budget",
                format!(
                    "{:.0} kg with contingency exceeds the {:.0} kg budget",
                    report.budget_mass_kg, self.requirements.max_mass_kg
                ),
            );
        }
        if report.thrust_offset_m > self.requirements.max_thrust_offset_m {
            fail(
                &mut findings,
                "thrust line",
                format!(
                    "the thrust line passes {:.2} m from the center of mass; the limit is {:.2} m",
                    report.thrust_offset_m, self.requirements.max_thrust_offset_m
                ),
            );
        }
        if report.delta_v_m_s < self.requirements.test_burn_m_s {
            fail(
                &mut findings,
                "test burn",
                format!(
                    "{:.0} m/s of propellant is short of the {:.0} m/s test burn",
                    report.delta_v_m_s, self.requirements.test_burn_m_s
                ),
            );
        }
        for load in &report.latch_loads {
            if load.margin < 0.0 {
                fail(
                    &mut findings,
                    "latch load",
                    format!(
                        "{}'s latch carries {:.0} N and {:.0} N m at ignition, over its rating",
                        load.part, load.force_n, load.moment_nm
                    ),
                );
            }
        }
        findings
    }

    /// Parts in an order where each comes after its parent, or `None` if
    /// the attachments loop or leave a part unconnected. Ties keep the
    /// document's order, so the order is deterministic.
    #[must_use]
    pub fn assembly_order(&self) -> Option<Vec<usize>> {
        let mut placed: Vec<usize> = Vec::new();
        let mut done = vec![false; self.parts.len()];
        while placed.len() < self.parts.len() {
            let next = self.parts.iter().enumerate().position(|(i, p)| {
                !done[i]
                    && match &p.attach.parent {
                        None => true,
                        Some(parent) => self.index(parent).is_some_and(|j| done[j]),
                    }
            })?;
            done[next] = true;
            placed.push(next);
        }
        Some(placed)
    }

    /// Apply an edit and return the next revision, or the first failed
    /// requirement. The document itself is unchanged either way.
    ///
    /// # Errors
    /// The edit names a missing part or leaves the design failing a check.
    pub fn edit(&self, edit: &Edit) -> Result<Self, Finding> {
        let mut next = self.clone();
        next.apply(edit)?;
        next.revision += 1;
        match next.check().into_iter().next() {
            Some(finding) => Err(finding),
            None => Ok(next),
        }
    }

    /// One edit's change, unchecked: [`Self::edit`] checks the result.
    fn apply(&mut self, edit: &Edit) -> Result<(), Finding> {
        let next = self;
        let missing = |id: &str| Finding {
            requirement: "edit".into(),
            detail: format!("there is no part named {id}"),
        };
        let index = |ship: &Self, id: &str| ship.index(id).ok_or_else(|| missing(id));
        match edit {
            Edit::Batch { edits } => {
                for edit in edits {
                    next.apply(edit)?;
                }
            }
            Edit::SetDryMass { part, kg } => {
                let i = index(&next, part)?;
                next.parts[i].dry_mass_kg = *kg;
            }
            Edit::SetPropellant { part, kg } => {
                let i = index(&next, part)?;
                match &mut next.parts[i].kind {
                    PartKind::Tank { propellant_kg } => *propellant_kg = *kg,
                    _ => {
                        return Err(Finding {
                            requirement: "edit".into(),
                            detail: format!("{part} is not a tank"),
                        });
                    }
                }
            }
            Edit::MovePart { part, position } => {
                let i = index(&next, part)?;
                let shift = *position - next.parts[i].position;
                next.parts[i].position = *position;
                next.parts[i].attach.at += shift;
            }
            Edit::AddPart { part } => next.parts.push(part.clone()),
            Edit::RemovePart { part } => {
                let i = index(&next, part)?;
                next.parts.remove(i);
            }
            Edit::ScalePart { part, factor } => {
                let i = index(&next, part)?;
                if !(factor.is_finite() && *factor > 0.0) {
                    return Err(Finding {
                        requirement: "edit".into(),
                        detail: format!("{part} can't be scaled by {factor}"),
                    });
                }
                let linear = factor.cbrt();
                let p = &mut next.parts[i];
                // A resized part no longer matches its CAD document, so
                // its inertia comes from its shape again.
                p.cad = None;
                p.dry_mass_kg *= factor;
                if let PartKind::Tank { propellant_kg } = &mut p.kind {
                    *propellant_kg *= factor;
                }
                p.shape = match p.shape {
                    Shape::Box { size } => Shape::Box {
                        size: size * linear,
                    },
                    Shape::Cylinder { radius, length } => Shape::Cylinder {
                        radius: radius * linear,
                        length: length * linear,
                    },
                };
            }
        }
        Ok(())
    }

    /// A stable 64-bit content hash (FNV-1a of the canonical JSON), which
    /// names this exact revision.
    #[must_use]
    pub fn version(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in json.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    /// Subsystem mass totals with contingency, kg.
    #[must_use]
    pub fn budget_by_subsystem(&self) -> BTreeMap<Subsystem, f64> {
        let mut totals = BTreeMap::new();
        for p in &self.parts {
            *totals.entry(p.subsystem).or_insert(0.0) +=
                p.dry_mass_kg * (1.0 + p.maturity.contingency()) + p.propellant();
        }
        totals
    }
}

fn fail(findings: &mut Vec<Finding>, requirement: &str, detail: String) {
    findings.push(Finding {
        requirement: requirement.into(),
        detail,
    });
}
