//! Clean environment builds (ENV-04).
//!
//! A [`BuildJob`] rebuilds one environment's pinned recipe revision on a
//! **fresh builder computer**
//! ([`coder_working_computer::Purpose::EnvironmentBuild`]): never the setup
//! computer, never a chat computer, never restored from a checkpoint. It
//! runs the recipe's exact install script, sanitizes the machine, quiesces
//! it, and captures an immutable output image under an owned name
//! ([`provider::Images`](coder_working_computer::provider::Images)).
//!
//! - **Frozen inputs** ([`Inputs`]): recipe revision and digest, source,
//!   base, runtime, platform, install script digest, credential names, the
//!   sanitization [`sanitize::Plan`], and the image name are fixed when the
//!   job starts and can never change on the record.
//! - **Stale builds**: a recipe edit makes every earlier build stale
//!   (`coder_environment::Environment::is_stale`). A build that becomes
//!   stale before capture is cancelled instead of spending a snapshot; a
//!   stale build can never be verified or saved.
//! - **Source**: before the install, the builder materializes the exact
//!   pinned commit with ephemeral auth and proves `HEAD` and a clean tree
//!   (`coder_environment_setup::source`); the typed report is retained on
//!   the job and in the image manifest.
//! - **Sanitization** runs before capture and gates it on a typed
//!   [`sanitize::Report`]: sign-ins (`~/.claude/.credentials.json`,
//!   `~/.codex/auth.json`, `gh`/npm/Cargo tokens, …), token-bearing Git
//!   configuration, private mounts, declared exclusions, and unkept explored
//!   state are gone, and every required path is present. Explored state
//!   survives only where the recipe declares it (`keep_explored`).
//! - **Image identity**: the name is derived from the environment, build,
//!   and recipe digest and is never reused; a capture reads it first and a
//!   name held by another source is refused, never replaced. Readiness is
//!   the provider's typed `Ready` state with an immutable snapshot ID, not a
//!   command exit or request acknowledgement. The sealed identity (name,
//!   snapshot, and [`ImageManifest`] digest) is recorded on the
//!   `BuildAttempt`.
//! - **Crash or lost reply** during any provider effect leaves the job
//!   `unresolved` (the `BuildAttempt` needs reconciliation). The next
//!   advance reads before acting: commands by identity, the image by name.
//!   A command is never started twice; a capture is re-issued only after a
//!   read shows the name absent, under the same owned name.
//! - **Usage and cleanup** are separate retained facts. Unknown cleanup
//!   stays visible and blocks new builds of the same environment until it
//!   is reconciled ([`service::Builder::cleanup`]).
//!
//! Every builder command goes through the ENV-02a evidence recorder,
//! redacted of the recipe's named credential values.

use coder_environment::{ArtifactPin, ImageIdentity, ImagePin, Platform, SourcePin, valid_id};
use coder_working_computer::provider::{CommandCursor, CommandSpec, ImageRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub mod sanitize;
pub mod service;
pub mod store;

pub const SCHEMA: &str = "openagents.environment.build_job.v1";
pub const MANIFEST_SCHEMA: &str = "openagents.environment.image_manifest.v1";
pub const MAX_HISTORY: usize = 128;
pub const MAX_SEGMENTS: usize = 64;
/// Captures re-issued after a read found the owned name absent.
pub const MAX_CAPTURE_ISSUES: u32 = 3;
pub const MAX_REASON_BYTES: usize = 1024;

/// The owned, never-reused image name for one build.
pub fn image_name(environment: &str, build: &str, recipe_digest: &str) -> String {
    let d =
        coder_environment::digest(format!("{environment}\0{build}\0{recipe_digest}").as_bytes());
    format!("oaenv-{build}-{}", &d[..24])
}

/// Everything the build is fixed to, retained before any allocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub environment: String,
    pub build_id: String,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub platform: Platform,
    pub install_digest: String,
    pub install_cwd: String,
    pub credential_names: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_credential: Option<String>,
    pub plan: sanitize::Plan,
    pub image_name: String,
    pub evidence_budget: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Provisioning,
    /// Putting the pinned source commit on the builder and proving it.
    Materializing,
    Installing,
    Sanitizing,
    /// Stopping the builder so the capture sees a quiet filesystem.
    Quiescing,
    Capturing,
    Ready,
    Failed,
    Cancelled,
}
impl Phase {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }
    /// The `BuildAttempt` state this phase projects to.
    pub fn build_state(self) -> coder_environment::BuildState {
        use coder_environment::BuildState as B;
        match self {
            Self::Provisioning | Self::Materializing => B::Provisioning,
            Self::Installing => B::Installing,
            Self::Sanitizing | Self::Quiescing => B::PreparingImage,
            Self::Capturing => B::SnapshotPending,
            Self::Ready => B::Ready,
            Self::Failed => B::Failed,
            Self::Cancelled => B::Cancelled,
        }
    }
}

/// One identified builder command and what the owner knows of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Run {
    /// Intent retained; the provider may or may not have it.
    Requested,
    Started {
        operation: String,
    },
    Exited {
        code: i64,
    },
    /// Gone without a recorded exit.
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub spec: CommandSpec,
    pub run: Run,
    pub cursor: CommandCursor,
    /// The evidence call currently receiving its output.
    pub call: String,
    /// Raw stdout kept for the sanitization report (bounded).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub report: String,
}

/// The capture under the job's owned name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub name: String,
    /// Capture calls issued, each after a read found the name absent.
    pub issued: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<ImageRecord>,
}

/// What the image's identity digest covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageManifest {
    pub schema: String,
    pub environment: String,
    pub build_id: String,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub platform: Platform,
    pub plan_digest: String,
    /// The proven checkout the install ran in.
    pub checkout: coder_environment_setup::source::Report,
    pub report: sanitize::Report,
    pub name: String,
    pub snapshot: String,
    pub builder: String,
}
impl ImageManifest {
    pub fn digest(&self) -> String {
        coder_environment::digest(&serde_json::to_vec(self).expect("manifest encodes"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Cleanup {
    NotStarted,
    Requested,
    Complete {
        evidence: String,
    },
    Failed {
        reason: String,
    },
    /// The owner could not learn whether the builder was deleted.
    Unknown {
        reason: String,
    },
}
impl Cleanup {
    pub fn uncertain(&self) -> bool {
        matches!(self, Self::Failed { .. } | Self::Unknown { .. })
    }
}

/// Metered usage of the builder, from each boot's meter-stop fact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub evidence: Vec<String>,
    /// Why the usage is not known to have stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertain: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<coder_environment::evidence::Sealed>,
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
pub struct BuildJob {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub created_ms: u64,
    pub updated_ms: u64,
    pub request_id: String,
    pub fingerprint: String,
    pub inputs: Inputs,
    pub computer: String,
    pub deadline_ms: u64,
    pub phase: Phase,
    /// An outcome the owner could not observe; reconcile before acting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Step>,
    /// What the source step proved about the checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<coder_environment_setup::source::Report>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install: Option<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sanitize: Option<Step>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<sanitize::Report>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<Capture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub usage: Usage,
    pub cleanup: Cleanup,
    pub segments: Vec<Segment>,
    pub history: Vec<Transition>,
}

impl BuildJob {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA || !valid_id(&self.id) || !valid_id(&self.computer) {
            return Err("The build job schema or identity is invalid.");
        }
        if !valid_id(&self.inputs.environment) || !valid_id(&self.inputs.build_id) {
            return Err("The build job names an invalid environment or build.");
        }
        if self.inputs.image_name
            != image_name(
                &self.inputs.environment,
                &self.inputs.build_id,
                &self.inputs.recipe_digest,
            )
        {
            return Err("The image name is not the build's owned name.");
        }
        if self
            .capture
            .as_ref()
            .is_some_and(|c| c.name != self.inputs.image_name)
        {
            return Err("A capture used a name the build does not own.");
        }
        if self.history.len() > MAX_HISTORY || self.segments.len() > MAX_SEGMENTS {
            return Err("The build job retains too much history.");
        }
        if self.phase == Phase::Ready && self.image.is_none() {
            return Err("A ready build needs its sealed image.");
        }
        if let Some(i) = &self.image {
            i.validate()?;
            if i.image_id != self.inputs.image_name || i.snapshot_id.is_none() {
                return Err("The sealed image must be the owned name and an immutable snapshot.");
            }
        }
        Ok(())
    }

    /// Inputs, owned names, sealed images, and terminal outcomes never
    /// change; history only grows.
    pub fn preserves_history_of(&self, next: &BuildJob) -> bool {
        next.id == self.id
            && next.request_id == self.request_id
            && next.fingerprint == self.fingerprint
            && next.inputs == self.inputs
            && next.computer == self.computer
            && next.created_ms == self.created_ms
            && next.revision > self.revision
            && (!self.phase.terminal() || next.phase == self.phase)
            && (self.image.is_none() || next.image == self.image)
            && next.history.starts_with(&self.history)
            && next.segments.len() >= self.segments.len()
            && (self.source.is_none() || next.source.is_some())
            && (self.checkout.is_none() || next.checkout == self.checkout)
            && (self.install.is_none() || next.install.is_some())
            && (self.sanitize.is_none() || next.sanitize.is_some())
            && self.capture.as_ref().is_none_or(|c| {
                next.capture
                    .as_ref()
                    .is_some_and(|n| n.name == c.name && n.issued >= c.issued)
            })
    }

    /// The manifest of the sealed image, reconstructed from the retained
    /// capture; its digest equals the `BuildAttempt` image's manifest
    /// digest. A verifier uses it to agree on the exact image identity.
    pub fn manifest(&self) -> Option<ImageManifest> {
        let record = self.capture.as_ref()?.record.as_ref()?;
        self.image.as_ref()?;
        Some(ImageManifest {
            schema: MANIFEST_SCHEMA.into(),
            environment: self.inputs.environment.clone(),
            build_id: self.inputs.build_id.clone(),
            recipe_revision: self.inputs.recipe_revision,
            recipe_digest: self.inputs.recipe_digest.clone(),
            source: self.inputs.source.clone(),
            base: self.inputs.base.clone(),
            runtime: self.inputs.runtime.clone(),
            platform: self.inputs.platform.clone(),
            plan_digest: self.inputs.plan.digest(),
            checkout: self.checkout.clone().unwrap_or_default(),
            report: self.report.clone().unwrap_or_default(),
            name: record.name.clone(),
            snapshot: record.snapshot.clone()?,
            builder: record.source.clone(),
        })
    }

    /// The `BuildAttempt` state this job projects to.
    pub fn build_state(&self) -> coder_environment::BuildState {
        if self.unresolved.is_some() && !self.phase.terminal() {
            coder_environment::BuildState::NeedsReconciliation
        } else {
            self.phase.build_state()
        }
    }
}

#[cfg(test)]
mod tests;
