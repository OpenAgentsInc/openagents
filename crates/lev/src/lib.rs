//! The System One contract, answered by Apple's on-device foundation model.
//!
//! One state and a map of typed questions go in; one typed answer per
//! question comes out. The contract is the same one `crates/jev` sends to
//! TypeSafe and `crates/kev` serves from open weights, so a caller reaches
//! any of the three with a `base_url` change.
//!
//! What makes this implementation different is what Apple's framework
//! withholds. It returns text and typed structured values, and it exposes no
//! logits, no log-probabilities, and no hidden states. Kev's pointer head has
//! no position to read here and no head to run. So Lev derives a distribution
//! from behavior it can observe — a seeded sample ensemble, or a constrained
//! certainty band — and then earns the word "probability" by fitting a
//! calibration map against labelled outcomes. Without a fitted map for the
//! question family in hand, a Lev door refuses rather than reporting a number.
//!
//! Fitting that map, scoring a door against a suite, and deciding whether a
//! map earned the right to serve are the Gym's, in `crates/gym`. Lev reads
//! `gym::calibrate::Record` to serve a fitted map and writes none of its
//! own; the dependency runs one way.
//!
//! What a door serves is one document. [`manifest::Manifest`] names the
//! release, the artifact and its digest, the base model it is pinned to, the
//! contract shapes, the estimator, and the calibration records each admitted
//! family rests on. Checking a package, pinning it at startup, admitting a
//! family, and choosing an estimator were four decisions in three binaries
//! that answered to each other; they answer to the manifest now. See
//! `docs/lev/manifest.md`.
//!
//! And what a door may keep serving is a second document. Pinning happens at
//! load time, so it protects a door that restarts and does nothing for one
//! already running. [`policy`] is the other half: a snapshot a canonical
//! service publishes, a freshness window after which a door that cannot
//! reach that service stops serving its managed release, and revocations
//! aimed at a release or at a base model signature. The operating system
//! replacing the base is a standing revocation event, and the window is what
//! bounds how long a door can go on serving through one. See
//! `docs/lev/revocation.md`.
//!
//! See `docs/lev/` for the architecture, the measured behavior of the
//! runtime, and the calibration rule.

pub mod adapter;
pub mod admission;
pub mod api;
pub mod bridge;
pub mod error;
pub mod estimator;
pub mod manifest;
pub mod policy;
pub mod render;
pub mod schema;
#[cfg(feature = "serve")]
pub mod serve;
pub mod suite;

pub use api::{
    Answer, Extensions, NoulCriteria, Question, SystemOneRequest, SystemOneResponse, Usage,
};
pub use bridge::{Availability, Bridge, Call, Outcome, Sampling};
pub use error::{Refusal, RefusalCode, Result};
pub use estimator::{Estimator, Raw, confidence};
pub use manifest::{EvalRef, Manifest};
pub use policy::{Clock, Policy, Revocation, Snapshot, Standing};
pub use render::render;
pub use schema::{BANDS, Compiled, Kind, compile};
