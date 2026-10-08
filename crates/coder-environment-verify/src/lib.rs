//! Independent fresh verification of a sealed environment image (ENV-05).
//!
//! A [`VerifyJob`] boots the **exact** output image of one ready, current
//! build on a fresh machine of its own
//! ([`coder_working_computer::Purpose::EnvironmentVerify`]): never the setup
//! or builder computer, never restored from a checkpoint, with no
//! credentials. It then runs the recipe's frozen [`plan::CheckPlan`]
//! without repairing the candidate:
//!
//! - **Identity**: before any machine exists, the build's retained image
//!   manifest must reproduce the `BuildAttempt`'s manifest digest (recipe,
//!   source, base, runtime, platform, checkout, sanitization, snapshot) and
//!   the provider's image must still be `Ready` with that immutable
//!   snapshot. Restore readiness is the provider's typed hydration fact,
//!   not a boot or command exit. The verifier's machines must differ from
//!   the builder's.
//! - **Untouched baseline first**: on the candidate it proves the checkout
//!   is the pinned commit (fetching only when the plan says so), proves the
//!   lock files hash to the recipe's frozen digests, starts declared
//!   services under provider process ownership and requires their health
//!   rules, then runs readiness/browser and behavior checks. Nothing is
//!   installed on the baseline and nothing is ever captured from it.
//! - **Disposable idempotence fork**: only after the baseline passes, a
//!   second fresh boot of the same image fingerprints the declared
//!   inventory, reruns the build's exact install script and startup, and
//!   fingerprints again. Any change fails the run. Both invocations are
//!   retained.
//! - **Locked/offline**: with `offline` the verifier's commands see package
//!   managers offline and a dead proxy, so dependencies must already be in
//!   the image.
//! - **Failed, missing, or empty checks fail**: a non-zero exit, a timeout,
//!   a lost process, a missing or altered check artifact, a plan with no
//!   behavior check, or a check with no assertion result fails the run.
//! - **A changed plan invalidates the run**: a recipe revision (which
//!   stales the build) or an altered plan artifact cancels it.
//! - **Complete child results**: each machine's commands are a child
//!   evidence record archived into the run's ENV-02a record; the sealed
//!   status travels with the verdict onto the `VerificationAttempt`, whose
//!   transition lets only complete evidence pass and save.
//! - **Cleanup confirmed**: both machines are deleted and their usage
//!   retained before the verdict is recorded. Unknown cleanup stays visible,
//!   holds the verdict, and blocks new verifications of the environment.
//! - **Restart**: a lost reply reconciles by reading the command by
//!   identity. An owner restart loses the live evidence record, so the run
//!   ends incomplete (never passed) and its machines are still cleaned up.

use coder_environment::evidence::Sealed;
use coder_environment::{ImageIdentity, valid_id};
use coder_environment_build::ImageManifest;
use coder_working_computer::VerifyRole;
use coder_working_computer::provider::{CommandCursor, CommandSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod plan;
pub mod service;
pub mod store;

pub const SCHEMA: &str = "openagents.environment.verify_job.v1";
pub const MAX_HISTORY: usize = 128;
pub const MAX_REASON_BYTES: usize = 1024;
/// Bytes of a source, lock, or inventory report kept on the record.
pub const MAX_REPORT_BYTES: usize = 64 * 1024;

/// Everything the run is fixed to, retained before any allocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub environment: String,
    pub build_id: String,
    pub verification_id: String,
    pub image: ImageIdentity,
    pub manifest: ImageManifest,
    pub plan_digest: String,
    pub plan: plan::CheckPlan,
    pub install_digest: String,
    pub install_cwd: String,
    /// Lock path to its frozen digest (the recipe's inputs).
    pub locks: BTreeMap<String, String>,
    pub size: String,
    pub evidence_budget: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Baseline,
    Fork,
    /// Deleting both machines before the verdict is recorded.
    Cleanup,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case", deny_unknown_fields)]
pub enum Verdict {
    Passed,
    Failed { reason: String },
    Incomplete { reason: String },
    Cancelled { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cleanup {
    NotStarted,
    Requested,
    Complete {
        evidence: String,
    },
    /// The owner could not learn whether the machine was deleted.
    Unknown {
        reason: String,
    },
}
impl Cleanup {
    pub fn uncertain(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}

/// One verifier machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineRun {
    pub computer: String,
    pub role: VerifyRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    /// The provider reported every restored file on disk.
    pub hydrated: bool,
    pub cleanup: Cleanup,
    /// Meter-stop evidence per boot, or why usage is not known to stop.
    #[serde(default)]
    pub usage: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_uncertain: Option<String>,
}
impl MachineRun {
    fn new(computer: String, role: VerifyRole) -> Self {
        Self {
            computer,
            role,
            generation: None,
            resource: None,
            hydrated: false,
            cleanup: Cleanup::NotStarted,
            usage: vec![],
            usage_uncertain: None,
        }
    }
}

/// What the owner knows of one identified command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Run {
    NotStarted,
    /// Intent retained; the provider may or may not have it.
    Requested,
    Started {
        operation: String,
    },
    Exited {
        code: i64,
    },
    Lost,
    TimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum StepOutcome {
    Passed { detail: String },
    Failed { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRecord {
    pub id: String,
    pub role: VerifyRole,
    pub action: plan::Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<CommandSpec>,
    pub run: Run,
    pub cursor: CommandCursor,
    #[serde(default)]
    pub deadline_ms: u64,
    #[serde(default)]
    pub tally: plan::Tally,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub report: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<StepOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub phase: Phase,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyJob {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub request_id: String,
    pub fingerprint: String,
    pub inputs: Inputs,
    pub deadline_ms: u64,
    pub phase: Phase,
    pub baseline: MachineRun,
    pub fork: MachineRun,
    pub steps: Vec<StepRecord>,
    /// An outcome the owner could not observe; reconcile before acting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<String>,
    /// Decided, and recorded on the attempt once cleanup is confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    /// The sealed run evidence the verdict cites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Sealed>,
    /// An owner restart lost the live evidence record; the run cannot pass.
    #[serde(default)]
    pub evidence_lost: bool,
    pub history: Vec<Transition>,
}

impl VerifyJob {
    pub fn machine(&self, role: VerifyRole) -> &MachineRun {
        match role {
            VerifyRole::Baseline => &self.baseline,
            VerifyRole::Fork => &self.fork,
        }
    }
    pub fn machine_mut(&mut self, role: VerifyRole) -> &mut MachineRun {
        match role {
            VerifyRole::Baseline => &mut self.baseline,
            VerifyRole::Fork => &mut self.fork,
        }
    }
    pub fn step(&self, id: &str) -> Option<&StepRecord> {
        self.steps.iter().find(|s| s.id == id)
    }
    pub fn step_mut(&mut self, id: &str) -> Option<&mut StepRecord> {
        self.steps.iter_mut().find(|s| s.id == id)
    }
    /// The first unfinished step of a role.
    pub fn next_step(&self, role: VerifyRole) -> Option<&StepRecord> {
        self.steps
            .iter()
            .find(|s| s.role == role && s.outcome.is_none())
    }
    pub fn cleanup_uncertain(&self) -> bool {
        self.baseline.cleanup.uncertain() || self.fork.cleanup.uncertain()
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA
            || !valid_id(&self.id)
            || !valid_id(&self.baseline.computer)
            || !valid_id(&self.fork.computer)
            || self.baseline.computer == self.fork.computer
        {
            return Err("The verify job schema or identity is invalid.");
        }
        if self.baseline.role != VerifyRole::Baseline || self.fork.role != VerifyRole::Fork {
            return Err("The verify machines have the wrong roles.");
        }
        if self.inputs.manifest.digest() != self.inputs.image.manifest_digest
            || self.inputs.manifest.name != self.inputs.image.image_id
        {
            return Err("The retained manifest is not the image's.");
        }
        if self.history.len() > MAX_HISTORY {
            return Err("The verify job retains too much history.");
        }
        if self.phase == Phase::Done && self.verdict.is_none() {
            return Err("A finished verification needs a verdict.");
        }
        if self.verdict == Some(Verdict::Passed)
            && self
                .steps
                .iter()
                .any(|s| !matches!(s.outcome, Some(StepOutcome::Passed { .. })))
        {
            return Err("A passed verification needs every step passed.");
        }
        // The fork never runs before the baseline has passed.
        let baseline_passed = self
            .steps
            .iter()
            .filter(|s| s.role == VerifyRole::Baseline)
            .all(|s| matches!(s.outcome, Some(StepOutcome::Passed { .. })));
        if !baseline_passed
            && self
                .steps
                .iter()
                .any(|s| s.role == VerifyRole::Fork && s.run != Run::NotStarted)
        {
            return Err("The fork ran before the untouched baseline passed.");
        }
        Ok(())
    }

    /// Inputs, steps, verdicts, and sealed evidence never change once
    /// retained; history only grows.
    pub fn preserves_history_of(&self, next: &VerifyJob) -> bool {
        next.id == self.id
            && next.request_id == self.request_id
            && next.fingerprint == self.fingerprint
            && next.inputs == self.inputs
            && next.created_ms == self.created_ms
            && next.revision > self.revision
            && next.baseline.computer == self.baseline.computer
            && next.fork.computer == self.fork.computer
            && next.history.starts_with(&self.history)
            && next.steps.len() == self.steps.len()
            && self.steps.iter().zip(&next.steps).all(|(a, b)| {
                a.id == b.id
                    && a.action == b.action
                    && (a.outcome.is_none() || a.outcome == b.outcome)
                    && (a.spec.is_none() || a.spec == b.spec)
            })
            && (self.verdict.is_none() || next.verdict == self.verdict)
            && (self.evidence.is_none() || next.evidence == self.evidence)
            && (!self.evidence_lost || next.evidence_lost)
            && (self.phase != Phase::Done || next.phase == Phase::Done)
    }

    /// The `VerificationAttempt` state this job projects to while running.
    pub fn verification_state(&self) -> coder_environment::VerificationState {
        use coder_environment::VerificationState as V;
        if self.unresolved.is_some() {
            return V::NeedsReconciliation;
        }
        match self.phase {
            Phase::Baseline
                if !self.baseline.hydrated
                    || self
                        .steps
                        .iter()
                        .all(|s| s.role == VerifyRole::Fork || s.run == Run::NotStarted) =>
            {
                V::Restoring
            }
            _ => V::Checking,
        }
    }
}

#[cfg(test)]
mod tests;
