//! What the host does for a rule that this crate cannot do by itself
//! (phase 3): start a Coder run, read and release issue claims, probe and
//! restart the relay or the host, read what Coder runs cost and which
//! checks failed, open or update an issue, and run a plugin's background
//! action. The program that starts the host gives the runner one
//! ([`crate::runner::start_full`]); tests give it a stand-in. Every method
//! has a default that says it is not available here, so a host that
//! cannot do one still runs every other rule.

use crate::rule::Watched;

pub use crate::records::{Claim, CoderRun, Failure, Usage};

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

    /// Refit the decision thresholds from joined run outcomes, adopting
    /// only fits that pass their held-out check; one line saying what
    /// changed.
    ///
    /// # Errors
    /// The outcomes or the adopted-settings file could not be read or
    /// written.
    fn recalibrate(&self, _dry_run: bool) -> Result<String, String> {
        Err("decision thresholds cannot be refitted from here".into())
    }
}

/// A host that does none of it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nothing;

impl Services for Nothing {}
