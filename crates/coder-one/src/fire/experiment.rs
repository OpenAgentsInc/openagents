//! The fire loop's development experiment: the one registered way for a
//! fire loop arm to run components that aren't admitted.
//!
//! `bench/terminal-bench/fire/protocol.md` says what the runs are and what
//! their results may be used for: development leads on in-sample tasks,
//! never admission. A manifest names the experiment and the protocol's
//! digest in `executor.microluna.lean.experiment`.

pub use crate::checks::oracle::Experiment;

/// The registered fire loop experiments. A test holds each protocol file
/// to its digest.
pub const EXPERIMENTS: &[Experiment] = &[Experiment {
    id: "fire-loop-development",
    protocol: "bench/terminal-bench/fire/protocol.md",
    protocol_sha256: "b0d64946dc2e5f98caabf45307911117bc5b81fd3003b9b24d7730f169fc2fc3",
}];

/// The registered fire loop experiment with this id and protocol digest.
#[must_use]
pub fn preregistered(id: &str, protocol_sha256: &str) -> Option<&'static Experiment> {
    EXPERIMENTS
        .iter()
        .find(|e| e.id == id && e.protocol_sha256 == protocol_sha256)
}
