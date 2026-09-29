//! A finished run, as the runner hands it to the engine.
//!
//! The engine spawns nothing. The runner (`openagents ext eval run`, the
//! hosted runner, or a test) runs each case in each arm, and hands over one
//! [`RunRecord`] per attempt: how it ended, its trajectory, the files it
//! created, and what it cost.

use std::path::PathBuf;

use serde::Serialize;

use crate::artifact::ArtifactRef;
use crate::case::RunFailure;
use crate::trajectory::Trajectory;

/// One side of a comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Arm {
    /// The extension admitted.
    Subject,
    /// Nothing admitted.
    Baseline,
}

impl Arm {
    /// The word a report writes.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Baseline => "baseline",
        }
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", content = "reason", rename_all = "snake_case")]
pub enum RunOutcome {
    /// The turn finished; its graders decide whether it passed.
    Completed,
    /// The run stopped with an error, which fails every grader with that
    /// reason.
    Errored(RunFailure),
    /// The operator stopped it. Not scored.
    Cancelled,
    /// Nobody knows how it ended. Not scored.
    Unknown,
}

impl RunOutcome {
    /// The NIP-EVAL outcome word: `completed`, `refused`, `failed`,
    /// `cancelled`, or `unknown`.
    #[must_use]
    pub const fn coverage_word(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Errored(failure) if failure.refused() => "refused",
            Self::Errored(_) => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    /// Whether the run is scored: completed, or ended with an error that
    /// becomes a grader failure. A cancelled or unknown run counts as
    /// neither a pass nor a failure.
    #[must_use]
    pub const fn scored(self) -> bool {
        matches!(self, Self::Completed | Self::Errored(_))
    }
}

/// One attempt of one case in one arm.
#[derive(Clone, Debug)]
pub struct RunRecord {
    /// The case name.
    pub case: String,
    /// The arm.
    pub arm: Arm,
    /// The attempt, from 1 to the case's runs.
    pub attempt: u32,
    /// How it ended.
    pub outcome: RunOutcome,
    /// The run's ATIF trajectory, when it wrote one.
    pub trajectory: Option<Trajectory>,
    /// The paths of files the run created, relative to its workspace.
    pub created_files: Vec<String>,
    /// The run's workspace after the run, for `{ file = ... }` focus.
    pub workspace: Option<PathBuf>,
    /// What the run cost, when known.
    pub cost_usd: Option<f64>,
    /// Wall seconds, when known.
    pub seconds: Option<f64>,
    /// Receipts the run left, such as Wasm invocation receipts.
    pub receipts: Vec<ArtifactRef>,
}

impl RunRecord {
    /// A record with nothing observed yet.
    #[must_use]
    pub fn new(case: &str, arm: Arm, attempt: u32, outcome: RunOutcome) -> Self {
        Self {
            case: case.to_string(),
            arm,
            attempt,
            outcome,
            trajectory: None,
            created_files: Vec::new(),
            workspace: None,
            cost_usd: None,
            seconds: None,
            receipts: Vec::new(),
        }
    }
}

/// Where a run's files live under a results directory:
/// `runs/<case>/<arm>-<attempt>`. `report.html` links trajectories there.
#[must_use]
pub fn run_path(case: &str, arm: Arm, attempt: u32) -> String {
    format!("runs/{case}/{}-{attempt}", arm.word())
}
