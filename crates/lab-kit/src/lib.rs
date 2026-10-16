//! The shared Lab contract ([Labs plan](../../docs/labs/README.md)).
//!
//! Every Lab kind (Video, CAD, Game, Robot, Ship, and later ones) keeps typed
//! documents with immutable versions, accepts changes only through its own
//! checks, makes artifacts with jobs, takes notes at a place in a version,
//! and shows a card. This crate holds what they all share and the
//! [`LabKind`] trait each implements. It does no IO: the store
//! (`lab-store`), the API (`/v1/labs`), the SDK and every app use these
//! types, and a client can check a change before sending it.

pub mod api;
pub mod card;
pub mod change;
pub mod digest;
pub mod echo;
pub mod kind;
pub mod model;

pub use card::{Card, CardState, Compare, NoteBrief, View};
pub use change::{Addressing, Change, PatchOp, apply_patch};
pub use digest::{canonical_json, digest};
pub use kind::{Draft, KindManifest, LabKind, Limits, Measurements, ToolDecl, Want};
pub use model::*;

/// A change a kind won't accept, with the sentence every surface shows.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct Refusal {
    /// A stable code clients can branch on (`beat_grid`, `stale_base`, ...).
    pub code: String,
    /// Plain words, shown as is.
    pub message: String,
}

impl Refusal {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
