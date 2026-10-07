//! Immutable identities for a nonspendable native projection of shared custody.
use crate::{
    funding_units::Conversion,
    purchase::{CommercialRef, CommercialSource},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    pub binding: String,
    pub binding_digest: String,
    pub origin: String,
    pub native_origin: String,
    pub node: String,
    pub pool: String,
    pub socket: PathBuf,
    pub conversion: Conversion,
    pub commercial: CommercialRef,
    pub source: CommercialSource,
}
impl Mode {
    /// Compare native custody, without replacing either historical review.
    pub fn same_native(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.native_origin == other.native_origin
            && self.node == other.node
            && self.pool == other.pool
            && self.socket == other.socket
            && self.source == other.source
            && self.commercial.customer == other.commercial.customer
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub intent: String,
    pub digest: String,
    pub mode: Mode,
}
