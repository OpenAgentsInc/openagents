//! What the host does for a rule that this crate cannot do by itself
//! (phase 3): start a Coder run, read and release issue claims, probe and
//! restart the relay or the host, read what Coder runs cost and which
//! checks failed, open or update an issue, and run a plugin's background
//! action. The program that starts the host gives the runner one
//! ([`crate::runner::start_full`]); tests give it a stand-in. Every method
//! has a default that says it is not available here, so a host that
//! cannot do one still runs every other rule.

use serde::{Deserialize, Serialize};

use crate::rule::Watched;

/// A Coder run a rule starts: an escalation or a `StartCoderRun` action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoderRun {
    pub title: String,
    /// The prompt, with the code-built briefing after it.
    pub prompt: String,
    /// The checkout it works in (absolute), or the host's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
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

/// The host's side of the phase 3 actions.
pub trait Services: Send + Sync {
    /// Start a Coder run; its task id.
    ///
    /// # Errors
    /// It could not start.
    fn start_coder_run(&self, _run: &CoderRun) -> Result<String, String> {
        Err("Coder runs cannot be started from here".into())
    }

    /// This computer's stale issue claims.
    ///
    /// # Errors
    /// The task store or the tracker could not be read.
    fn stale_claims(&self, _idle_hours: u64, _now: u64) -> Result<Vec<Claim>, String> {
        Err("issue claims cannot be read from here".into())
    }

    /// Release one claim with a comment saying why.
    ///
    /// # Errors
    /// The tracker refused.
    fn release_claim(&self, _claim: &Claim) -> Result<(), String> {
        Err("issue claims cannot be released from here".into())
    }

    /// Whether the relay or the host answers now.
    ///
    /// # Errors
    /// Why it does not.
    fn probe(&self, target: Watched) -> Result<(), String> {
        Err(format!("the {} cannot be probed from here", target.name()))
    }

    /// Restart through the service manager; what was done.
    ///
    /// # Errors
    /// It could not.
    fn restart(&self, target: Watched) -> Result<String, String> {
        Err(format!(
            "the {} cannot be restarted from here",
            target.name()
        ))
    }

    /// What Coder runs did since `since`.
    ///
    /// # Errors
    /// The task store could not be read.
    fn usage(&self, _since: u64) -> Result<Usage, String> {
        Err("Coder runs cannot be read from here".into())
    }

    /// Checks that failed in Coder runs since `since`.
    ///
    /// # Errors
    /// The task store could not be read.
    fn failures(&self, _since: u64) -> Result<Vec<Failure>, String> {
        Err("Coder checks cannot be read from here".into())
    }

    /// Open an issue (or comment on `existing`); its URL or number.
    ///
    /// # Errors
    /// The tracker refused.
    fn report_issue(
        &self,
        _title: &str,
        _body: &str,
        _existing: Option<&str>,
    ) -> Result<String, String> {
        Err("issues cannot be opened from here".into())
    }

    /// Run an installed plugin's background action, read-only; its reply.
    ///
    /// # Errors
    /// No such plugin, or it failed.
    fn run_plugin(&self, plugin: &str, _input: &str) -> Result<String, String> {
        Err(format!("plugin {plugin} cannot run from here"))
    }
}

/// A host that does none of it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nothing;

impl Services for Nothing {}
