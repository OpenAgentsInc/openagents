//! Portable records shared by tasks and the background scheduler.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A Coder run a rule starts: an escalation or a `StartCoderRun` action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoderRun {
    pub title: String,
    /// The prompt, with the code-built briefing after it.
    pub prompt: String,
    /// The checkout it works in (absolute), or the host's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The existing Coder chat (its session id) the prompt is posted
    /// into, instead of a new run (#11177).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
}

/// An issue claim this computer holds that nothing works any more.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub repository: String,
    pub number: u64,
    pub task: String,
    /// Why it is stale, in a few words: "its task ended", "no run for 7 hours".
    pub why: String,
}

/// What Coder runs did over a span, for the daily summary.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Runs that ended in the span.
    pub ended: u64,
    pub succeeded: u64,
    pub failed: u64,
    /// What the priced runs cost, in micro-dollars; `None` when none was
    /// priced. Information, never a limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
    /// Runs whose cost is not known.
    #[serde(default)]
    pub unpriced: u64,
}

/// A failed check in a Coder run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    /// The test or check that failed.
    pub test: String,
    /// The failure's first lines, as the check printed them.
    pub output: String,
    pub task: String,
    pub at: u64,
}

/// What the task store says about one Coder task.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFact {
    pub id: String,
    /// Its worktree (the task's workspace path).
    pub worktree: PathBuf,
    /// Its per-task Cargo target directory (the layout before slots).
    pub target: PathBuf,
    /// Finished or cancelled, checks not running, group clear: the
    /// task store's own `ended` test.
    pub ended: bool,
    /// Its run or its checks failed.
    #[serde(default)]
    pub failed: bool,
    /// It was cancelled.
    #[serde(default)]
    pub cancelled: bool,
    /// It is queued or running.
    #[serde(default)]
    pub running: bool,
}

/// What recreates a removed worktree: its repository, branch, and commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Undo {
    pub repo: PathBuf,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub commit: String,
}
