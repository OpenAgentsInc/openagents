//! CAD Lab's mass-properties feed (`cad-mass-feed.v1`, written by
//! `cad_export` with format `feed`; `cad::feed` in the `cad` crate), read
//! here without linking the CAD kernel, so the ship document stays
//! buildable for the web and phones.
//!
//! The feed is SI in the CAD assembly's axes: X forward, Y left, Z up. The
//! ship frame is X forward, Y up, Z starboard, so a CAD vector (x, y, z)
//! is (x, z, -y) in the ship.
//!
//! The `cad` crate's test `the_lagrange_tug_feed_is_its_documents` checks
//! that the checked-in feed is what CAD measures for the checked-in
//! document, and these types read exactly its fields.

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

/// The record's kind.
pub const KIND: &str = "cad-mass-feed.v1";

/// The sample ship's CAD document, `cad/lagrange-tug.json`.
pub const TUG_DOCUMENT: &str = include_str!("../cad/lagrange-tug.json");
/// Its feed, `cad/lagrange-tug.mass.json`.
pub const TUG_FEED: &str = include_str!("../cad/lagrange-tug.mass.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub doc: String,
    pub version: u64,
    pub digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mass {
    pub mass_kg: f64,
    pub center_of_mass_m: [f64; 3],
    pub inertia_kg_m2: [[f64; 3]; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub joint: String,
    pub kind: String,
    pub mate: String,
    pub parent: bool,
    pub origin_m: [f64; 3],
    pub axis: [f64; 3],
    pub values: Vec<f64>,
    pub limits: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub id: String,
    pub part: String,
    pub title: String,
    pub material: String,
    #[serde(flatten)]
    pub mass: Mass,
    pub ports: Vec<Port>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassFeed {
    pub kind: String,
    pub source: Source,
    pub total: Mass,
    pub parts: Vec<Part>,
}

/// A CAD-axes vector in the ship frame.
#[must_use]
pub fn to_ship(v: [f64; 3]) -> DVec3 {
    DVec3::new(v[0], v[2], -v[1])
}

/// A CAD-axes tensor (rows) in the ship frame.
#[must_use]
pub fn tensor_to_ship(rows: [[f64; 3]; 3]) -> DMat3 {
    let cad = DMat3::from_cols_array_2d(&rows).transpose();
    // Columns are the CAD axes' images in the ship frame.
    let r = DMat3::from_cols(
        to_ship([1.0, 0.0, 0.0]),
        to_ship([0.0, 1.0, 0.0]),
        to_ship([0.0, 0.0, 1.0]),
    );
    r * cad * r.transpose()
}

impl MassFeed {
    /// Parse a feed.
    ///
    /// # Errors
    /// It isn't a `cad-mass-feed.v1` record.
    pub fn parse(text: &str) -> Result<Self, String> {
        let feed: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if feed.kind != KIND {
            return Err(format!("expected {KIND}, found {}", feed.kind));
        }
        Ok(feed)
    }

    /// The sample ship's feed.
    #[must_use]
    pub fn tug() -> Self {
        Self::parse(TUG_FEED).expect("the checked-in tug feed parses")
    }

    #[must_use]
    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }
}
