//! The mass-properties feed (`cad-mass-feed.v1`): a CAD Lab version's
//! measured parts and their ports, as the typed record Lagrange 2's
//! spacecraft document reads (docs/spacecraft-design/sim-mapping.md, "The
//! spacecraft document"; docs/cad-lab/README.md).
//!
//! Every value is SI in the assembly's frame (X forward, Y left, Z up,
//! metres) at the version's joint values: each part's mass, centre of mass
//! and inertia tensor about its centre of mass, and a port per joint on
//! both instances it joins (where it is, its axis, its kind and limits).
//! The total is the parallel-axis sum of the parts, so a document built
//! from the feed starts with the CAD's own numbers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::doc;

/// The record's kind.
pub const KIND: &str = "cad-mass-feed.v1";

/// Where the numbers came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The CAD Lab document's id, e.g. `rover`.
    pub doc: String,
    pub version: u64,
    /// The version's digest (`version.json`).
    pub digest: String,
}

/// Mass properties, SI, assembly axes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mass {
    pub mass_kg: f64,
    /// Centre of mass, m.
    pub center_of_mass_m: [f64; 3],
    /// About the centre of mass, kg m², rows.
    pub inertia_kg_m2: [[f64; 3]; 3],
}

/// A joint seen from one of the instances it joins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    /// The assembly joint's id.
    pub joint: String,
    /// fastened, revolute, slider or cylindrical.
    pub kind: String,
    /// The instance on the other side.
    pub mate: String,
    /// Whether this instance is the joint's parent.
    pub parent: bool,
    /// The joint's origin, m, assembly frame.
    pub origin_m: [f64; 3],
    /// Its unit axis, assembly frame.
    pub axis: [f64; 3],
    /// Its values: degrees for turns, mm for slides, as the assembly has
    /// them.
    pub values: Vec<f64>,
    /// [min, max] per value, in the same units.
    pub limits: Vec<[f64; 2]>,
}

/// One placed instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// The instance id.
    pub id: String,
    /// The part it places.
    pub part: String,
    pub title: String,
    pub material: String,
    #[serde(flatten)]
    pub mass: Mass,
    pub ports: Vec<Port>,
}

/// A version's mass-properties feed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassFeed {
    pub kind: String,
    pub source: Source,
    pub total: Mass,
    pub parts: Vec<Part>,
}

impl Mass {
    /// From a report entry (kg, mm, kg m²).
    fn from_report(entry: &Value) -> Option<Self> {
        let com = doc::triple(&entry["center_of_mass_mm"])?;
        let mut inertia = [[0.0; 3]; 3];
        for (i, row) in inertia.iter_mut().enumerate() {
            *row = doc::triple(&entry["inertia_kg_m2"][i])?;
        }
        Some(Self {
            mass_kg: entry["mass_kg"].as_f64()?,
            center_of_mass_m: com.map(|x| x / 1000.0),
            inertia_kg_m2: inertia,
        })
    }

    /// The parallel-axis sum of `parts` about their joint centre of mass.
    #[must_use]
    pub fn sum(parts: &[Mass]) -> Self {
        let m: f64 = parts.iter().map(|p| p.mass_kg).sum();
        if m <= 0.0 {
            return Self {
                mass_kg: 0.0,
                center_of_mass_m: [0.0; 3],
                inertia_kg_m2: [[0.0; 3]; 3],
            };
        }
        let mut c = [0.0; 3];
        for p in parts {
            for (k, x) in c.iter_mut().enumerate() {
                *x += p.mass_kg * p.center_of_mass_m[k] / m;
            }
        }
        let mut i = [[0.0; 3]; 3];
        for p in parts {
            let d = [0, 1, 2].map(|k| p.center_of_mass_m[k] - c[k]);
            let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            for (a, row) in i.iter_mut().enumerate() {
                for (b, cell) in row.iter_mut().enumerate() {
                    let delta = if a == b { 1.0 } else { 0.0 };
                    *cell += p.inertia_kg_m2[a][b] + p.mass_kg * (dd * delta - d[a] * d[b]);
                }
            }
        }
        Self {
            mass_kg: m,
            center_of_mass_m: c,
            inertia_kg_m2: i,
        }
    }
}

/// The feed of a version: `document` (assembly.v1 or part.v1) and its
/// check `report`.
///
/// # Errors
/// The report lacks an instance's mass properties.
pub fn feed(source: Source, document: &Value, report: &Value) -> Result<MassFeed, String> {
    let assembly = doc::as_assembly(document);
    let placed = crate::build::placements(&assembly);
    let entries: Vec<&Value> = report["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let mut parts = Vec::new();
    for entry in &entries {
        let id = entry["id"].as_str().unwrap_or_default().to_owned();
        let part = entry["part"].as_str().unwrap_or_default().to_owned();
        let mass = Mass::from_report(entry)
            .ok_or_else(|| format!("The report has no mass properties for {id}."))?;
        let spec = &assembly["parts"][&part];
        let mut ports = Vec::new();
        for joint in assembly["joints"].as_array().into_iter().flatten() {
            let (child, parent) = (
                joint["child"].as_str().unwrap_or_default(),
                joint["parent"].as_str().unwrap_or_default(),
            );
            if id != child && id != parent {
                continue;
            }
            // Origin and axis are in the parent instance's frame.
            let frame = placed
                .get(parent)
                .copied()
                .unwrap_or(crate::xf::Xf::IDENTITY);
            let origin = frame.apply(doc::triple(&joint["origin"]).unwrap_or([0.0; 3]));
            let axis = frame.rotate(doc::triple(&joint["axis"]).unwrap_or([0.0, 0.0, 1.0]));
            let n = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2])
                .sqrt()
                .max(1e-12);
            ports.push(Port {
                joint: joint["id"].as_str().unwrap_or_default().to_owned(),
                kind: joint["type"].as_str().unwrap_or_default().to_owned(),
                mate: if id == child { parent } else { child }.to_owned(),
                parent: id == parent,
                origin_m: origin.map(|x| x / 1000.0),
                axis: axis.map(|x| x / n),
                values: doc::joint_values(joint),
                limits: joint["limits"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|l| Some([l[0].as_f64()?, l[1].as_f64()?]))
                    .collect(),
            });
        }
        parts.push(Part {
            id,
            title: spec["title"].as_str().unwrap_or(&part).to_owned(),
            material: spec["material"]
                .as_str()
                .unwrap_or(doc::DEFAULT_MATERIAL)
                .to_owned(),
            part,
            mass,
            ports,
        });
    }
    let total = Mass::sum(&parts.iter().map(|p| p.mass).collect::<Vec<_>>());
    Ok(MassFeed {
        kind: KIND.into(),
        source,
        total,
        parts,
    })
}
