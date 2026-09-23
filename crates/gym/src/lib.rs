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
//! - [`questions`] — the question text, held apart from the items and
//!   carrying its own digest, so rewording a question produces a candidate
//!   against the same items rather than a different suite.
//! - [`gate`] — an acceptance rule that carries its own digest, so changing
//!   the rule produces a new rule rather than new history.
//! - [`ab`] — the loop that compares two doors over interleaved seed blocks,
//!   screens the result, and confirms the win on blocks nobody has drawn.
//! - [`admission`] — the frozen plan a cross-artifact candidate is admitted
//!   under, and the digested decision a registry activates from.
//! - [`views`] — the public benchmark and status snapshot: the page a
//!   stranger reads, the manifest its claims reproduce from, and the
//!   rendering where unknown reads as unknown and never as zero.
//! - [`calibrate`] — the reliability table that turns a raw signal into a
//!   probability, and the record that says what it was fitted against.
//! - [`coverage`] — the declared selection a run was meant to ask, and the
//!   accounting of whether the recorded rows cover it.
//! - [`commitment`] — the digested anchor a report is checked against after
//!   the store has left the writer's hands.
//! - [`eval`] — one run over a suite: what a door's answer becomes, and what
//!   a table of rows says afterwards.
//! - [`regress`] — one door against its own last recorded run, which is the
//!   question a repository with no CI asks before every push.
//! - [`spread`] — how far a metric moves when only the seed block moves,
//!   which is the floor a gate's thresholds are multiples of.
//!
//! What it deliberately does not own: how any model answers. The Gym scores
//! what comes back from `POST /v1/systemone` and knows nothing else about
//! the door.

pub mod ab;
pub mod admission;
pub mod build;
pub mod calibrate;
pub mod coder_briefing;
pub mod coder_calls;
pub mod coder_components;
pub mod coder_minitasks;
pub mod coder_policy;
pub mod coder_prompt;
pub mod coder_requirements;
pub mod commitment;
pub mod coverage;
pub mod eval;
pub mod gate;
pub mod jobs;
pub mod questions;
pub mod regress;
pub mod row;
pub mod spread;
pub mod store;
pub mod suite;
pub mod terminal_bench;
pub mod timeline;
pub mod views;

#[cfg(feature = "tui")]
pub mod terminal_bench_tui;
#[cfg(feature = "tui")]
pub mod tui;

/// The schema family every Gym document is tagged with.
pub const SCHEMA_PREFIX: &str = "openagents.gym";
