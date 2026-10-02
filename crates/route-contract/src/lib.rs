//! The agentic execution router's contract, frozen at phase 0 (#10205).
//!
//! One crate every client of the router reads: OpenAgents Terminal, the
//! `openagents` CLI, the desktop app, the phone, and the future HTTP API
//! ([plan](../../../docs/api/2026-10-02-agentic-execution-router.md),
//! sections 4 to 6 and 13). It holds data and pure checks only: no I/O, no
//! Jev, no task store. It depends on `serde`, `serde_json`, and `sha2`, so a
//! thin surface can use it without linking Coder.
//!
//! - [`snapshot`]: the immutable admission snapshot (identity, input, route,
//!   placement, effects with the macOS deny set, disclosure, resources,
//!   money with the payer per resource, evidence, defaults applied), and the
//!   widening check a fallback or a continuation must pass.
//! - [`route`]: the route result (answer, local command, plugin or program,
//!   Coder with a dispatch plan of N runs, standing rule, missing capability,
//!   clarification, refusal).
//! - [`offer`]: an immutable, expiring proposal bound to a digest of its
//!   terms.
//! - [`lifecycle`]: the domain lifecycle and its total mapping onto the task
//!   owner's existing `(status, execution, checks)` dispositions. The task
//!   owner's journal stays the only execution state machine.
//! - [`record`]: what one message's route became (phase 1, #10207): the
//!   route result, the snapshot that admitted it, the router's own moves,
//!   each task's projected lifecycle, and per-run cost and wall time.
//! - [`eval`]: the labeled evaluation split for route families
//!   (`fixtures/route-families-v1.json`).
//!
//! Every document carries a versioned `schema`. Immutable definitions are
//! named by [`Digest`]: SHA-256 over canonical JSON (object keys sorted, no
//! whitespace), so a digest does not move with serde's map ordering.

pub mod digest;
pub mod eval;
pub mod lifecycle;
pub mod offer;
pub mod record;
pub mod route;
pub mod snapshot;

pub use digest::{Digest, digest_of};
pub use lifecycle::{Lifecycle, Projection, TaskDisposition};
pub use offer::Offer;
pub use record::{Observation, RouteRecord, RunOutcome};
pub use route::{DispatchPlan, RouteFamily, RouteResult};
pub use snapshot::AdmissionSnapshot;

/// The contract version these schemas belong to. A breaking change to any
/// document below is a new version with new schema strings, never an edit
/// in place.
pub const CONTRACT_VERSION: u32 = 1;

/// The admission snapshot's schema.
pub const SNAPSHOT_SCHEMA: &str = "openagents.route.admission-snapshot.v1";
/// The route result's schema.
pub const ROUTE_SCHEMA: &str = "openagents.route.result.v1";
/// An offer's schema.
pub const OFFER_SCHEMA: &str = "openagents.route.offer.v1";
/// A lifecycle transition record's schema.
pub const TRANSITION_SCHEMA: &str = "openagents.route.transition.v1";
/// A route record's schema ([`record`]), added in phase 1 beside the
/// frozen documents.
pub const RECORD_SCHEMA: &str = "openagents.route.record.v1";
/// The evaluation split's schema.
pub const EVAL_SCHEMA: &str = "openagents.route.eval-split.v1";

#[cfg(test)]
mod tests;
