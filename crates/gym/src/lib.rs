//! The Gym: the measurement and control plane for decision models.
//!
//! Jev, Kev, and Lev all answer one contract, and for a week they were
//! compared by hand — a suite, a split, a change, a number pasted into a
//! document. That works until it does not. It produced three real faults in
//! as many days: items silently dropped from a denominator so a door that
//! refused hard questions scored better; calibration records that could not
//! say which model they were fitted against, and sat on disk through two
//! model changes; and a gate and a suite replaced in one commit, leaving
//! their effects impossible to separate.
//!
//! None of those are measurement mistakes. They are record-keeping mistakes,
//! and the Gym is the record-keeping.
//!
//! What it owns:
//!
//! - [`suite`] — labelled items, pinned by a content digest, partitioned into
//!   development, calibration, and a locked set that is read once.
//! - [`row`] — one result per item per door per run, with the door's
//!   verifiable identity attached.
//! - [`store`] — an append-only file whose rows carry a receipt chain, so a
//!   worse result cannot be quietly removed and a better one cannot be
//!   quietly inserted.
//! - [`gate`] — an acceptance rule that carries its own digest, so changing
//!   the rule produces a new rule rather than new history.
//! - [`views`] — rendering where unknown reads as unknown and never as zero.
//!
//! What it deliberately does not own: how any model answers. The Gym scores
//! what comes back from `POST /v1/systemone` and knows nothing else about
//! the door.

pub mod gate;
pub mod row;
pub mod store;
pub mod suite;
pub mod views;

#[cfg(feature = "tui")]
pub mod tui;

/// The schema family every Gym document is tagged with.
pub const SCHEMA_PREFIX: &str = "openagents.gym";
