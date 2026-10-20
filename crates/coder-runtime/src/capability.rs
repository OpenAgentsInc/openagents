//! The capability contract, shared.
//!
//! The manifest, the registry, the trust store, and the bounded probe all
//! live in `crates/capability` so Coder and CoderBench read the same
//! contract and cannot drift. This module re-exports it: what used to be
//! `coder::capability::Manifest` is `capability::Manifest`, unchanged in
//! shape, and `coder::capability` is the same door it was.
//!
//! What the extraction adds is the separation the audit asked for:
//! [`Registry`](capability::Registry) reads without running, [`Entry`]'s
//! probe runs only under a [`Trust`](capability::Trust) approval that
//! names the manifest's digest and the adapter's pinned identity, and
//! [`Presence`](capability::Presence) answers `unprobed` and `unknown`
//! as states of their own rather than letting a failed or forbidden run
//! stand in for a found one. `capability-trust` is the operator's
//! approval path; nothing in a repository can grant it.

pub use ::capability::*;
