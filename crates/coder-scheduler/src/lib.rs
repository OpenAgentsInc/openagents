//! The deterministic backlog scheduler.
//!
//! `coder-scheduler` is the deterministic foundation under a supervised
//! delegation backlog. It answers one question — *which pinned tasks may
//! run next* — as a pure function of the catalog, the durable state, and
//! the host's capacity, and it keeps the durable record that answer is
//! drawn from.
//!
//! # The pieces
//!
//! - [`catalog`] — the versioned, self-digested task catalog. Every task
//!   pins its stable id, its issue, the base and input it was cut
//!   against, its dependencies, its declared read and write footprints,
//!   its priority, and its resource requirements.
//! - [`resources`] — the resource vector a task holds while admitted:
//!   executor slots, CPU units, a memory reservation, the exclusive
//!   quiet-host lane, and the integration lane.
//! - [`plan`] — the pure selection function. Given a catalog, a
//!   capacity, the current states, and any externally owned paths, it
//!   returns the admissions and, for every task it does not admit, the
//!   reasons why.
//! - [`ledger`] — the durable record. One JSON document under one OS
//!   file lock, replaced atomically, with checked transitions, unique
//!   attempt ids, compare-and-set ownership, and conservative crash
//!   recovery: an attempt that was in flight when the writer died is
//!   `unknown`, stays blocked, and is never replayed automatically.
//! - [`simulate`] — a deterministic discrete-event comparison of
//!   fixed-wave and completion-driven scheduling over the same catalog.
//!
//! # What this crate is not
//!
//! It is not a daemon, an executor, or a delegation implementation. It
//! does not spawn work, does not watch a queue, and does not talk to a
//! network. A caller runs a plan, dispatches the admissions itself,
//! records outcomes through the ledger, and reviews results before
//! accepting them — the ledger's `completed` state is reachable only
//! through an explicit accept, because a settled result is a claim until
//! someone verifies it.
//!
//! It is also not an isolation boundary. The footprint conflict check is
//! a scheduling rule — two admitted tasks are never planned over
//! overlapping writes — not a mechanism that prevents a misbehaving task
//! from writing where it said it would not. Enforcement belongs to the
//! host's execution boundary, and this crate claims none.

pub mod catalog;
pub mod ledger;
pub mod plan;
pub mod resources;
pub mod simulate;
