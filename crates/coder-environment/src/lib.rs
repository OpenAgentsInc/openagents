//! Repository environment owner (ENV-01).
//!
//! An [`Environment`] joins a project and a source pin with a draft recipe,
//! its immutable recipe revisions, build and verification attempts, saved
//! immutable [`EnvironmentVersion`]s, and the project's selected version.
//! Every change goes through [`transition::apply`], a pure function that
//! returns a typed outcome; [`store::Store`] retains records atomically so
//! the state survives restart. See
//! `docs/cloud/example-cursor-cloud-agent-onboarding/environment-onboarding.md`.
//!
//! This crate owns records, their transitions, and the evidence record a
//! verification cites ([`evidence`]: complete, redacted command capture fed
//! from Boat frames and linked into ATIF). Execution, provider machines, and
//! source materialization stay in `coder-cloud` and `boat`; attempts link to
//! their runs by identity.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub mod capture;
pub mod evidence;
pub mod promotion;
pub mod store;
pub mod transition;

pub use promotion::{
    Candidate, HistoryRow, Review, ReviewStamp, SelectionChange, SelectionKind, VersionPin,
};
pub use transition::{Applied, Command, Effect, Refusal, apply};

pub const SCHEMA: &str = "openagents.environment.v1";
pub const RECIPE_SCHEMA: &str = "openagents.environment.recipe.v1";
/// Bounds that keep one retained record small enough to read whole.
pub const MAX_RECIPE_REVISIONS: usize = 512;
pub const MAX_ATTEMPTS: usize = 512;
pub const MAX_VERSIONS: usize = 256;
pub const MAX_REQUESTS: usize = 2048;
pub const MAX_HISTORY: usize = 64;
pub const MAX_REASON_BYTES: usize = 1024;

/// Lowercase hex SHA-256 of `bytes`.
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Opaque identity: letters, numbers, `_`, `-`, `.`, at most 128 bytes.
pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}
pub fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn valid_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn valid_relative_path(value: &str) -> bool {
    valid_text(value, 1024)
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// The project that owns an environment, as admitted by the operator profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectLink {
    pub workspace: String,
    pub project: String,
}

/// Exact source baseline: commit plus the admitted snapshot digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    pub revision: String,
    pub digest: String,
}
impl SourcePin {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_revision(&self.revision) || !valid_digest(&self.digest) {
            return Err("The source pin needs an exact commit and digest.");
        }
        if self
            .repository
            .as_deref()
            .is_some_and(|r| !valid_text(r, 512))
        {
            return Err("The source repository label is invalid.");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Boat,
    /// The optional dedicated GCE image adapter (ENV-09): one isolated
    /// instance per setup, builder, or verifier, and GCE images pinned by
    /// their numeric ID. Never the shared Coder pool.
    Gce,
}

/// A resolved, never-moving provider image identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImagePin {
    pub provider: Provider,
    pub image_id: String,
    pub digest: String,
}
/// A sealed builder output image.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageIdentity {
    pub provider: Provider,
    pub image_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    pub manifest_digest: String,
}
impl ImageIdentity {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_text(&self.image_id, 256)
            || self
                .snapshot_id
                .as_deref()
                .is_some_and(|s| !valid_text(s, 256))
            || !valid_digest(&self.manifest_digest)
        {
            return Err("The output image identity is incomplete.");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPin {
    pub revision: String,
    pub digest: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
}
/// Install script: retained as a blob by digest, run from `cwd` in the source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub cwd: String,
    pub digest: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedArtifact {
    pub name: String,
    pub digest: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub services: Vec<NamedArtifact>,
    pub readiness: Vec<NamedArtifact>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain: Option<String>,
    /// Lock file path (relative to the source root) to its digest.
    pub locks: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub profile: String,
    pub plan_digest: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub deadline_seconds: u64,
    pub concurrent_machines: u32,
    pub total_machine_allocations: u32,
    pub output_bytes: u64,
}

/// The install/start recipe. Inert data: it grants no privilege, credential,
/// network recipient, or spend by itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema: String,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub platform: Platform,
    pub install: Script,
    #[serde(default)]
    pub start: Start,
    #[serde(default)]
    pub inputs: Inputs,
    #[serde(default)]
    pub credential_names: BTreeSet<String>,
    pub qualification: Qualification,
    pub limits: Limits,
    /// What the clean build captures into its image (ENV-04). Omitted when
    /// empty, so recipes without one keep their digest.
    #[serde(default, skip_serializing_if = "capture::Capture::is_empty")]
    pub capture: capture::Capture,
}
impl Recipe {
    /// Canonical digest: fields serialize in declaration order and maps/sets
    /// in key order, so equal recipes always share one digest.
    pub fn digest(&self) -> String {
        digest(&serde_json::to_vec(self).expect("recipe encodes"))
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != RECIPE_SCHEMA {
            return Err("The recipe schema is not supported.");
        }
        if !valid_text(&self.base.image_id, 256) || !valid_digest(&self.base.digest) {
            return Err("The recipe base needs a resolved image identity and digest.");
        }
        if !valid_text(&self.runtime.revision, 128) || !valid_digest(&self.runtime.digest) {
            return Err("The recipe runtime needs an exact revision and digest.");
        }
        if self.platform.os != "linux" || self.platform.architecture != "x86_64" {
            return Err("Only linux x86_64 environments are qualified.");
        }
        if !(self.install.cwd == "." || valid_relative_path(&self.install.cwd))
            || !valid_digest(&self.install.digest)
        {
            return Err("The install script needs a source-relative cwd and digest.");
        }
        let start = self.start.services.iter().chain(&self.start.readiness);
        if self.start.services.len() + self.start.readiness.len() > 64 {
            return Err("The recipe declares too many services or checks.");
        }
        for item in start {
            if !valid_id(&item.name) || !valid_digest(&item.digest) {
                return Err("A service or readiness check is invalid.");
            }
        }
        if self
            .inputs
            .toolchain
            .as_deref()
            .is_some_and(|d| !valid_digest(d))
            || self.inputs.locks.len() > 128
            || self
                .inputs
                .locks
                .iter()
                .any(|(p, d)| !valid_relative_path(p) || !valid_digest(d))
        {
            return Err("The recipe inputs are invalid.");
        }
        if self.credential_names.len() > 32
            || self.credential_names.iter().any(|name| {
                name.is_empty()
                    || name.len() > 128
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
            })
        {
            return Err("Credential names must be uppercase environment names.");
        }
        if !valid_id(&self.qualification.profile) || !valid_digest(&self.qualification.plan_digest)
        {
            return Err("The recipe needs a frozen qualification plan.");
        }
        let l = &self.limits;
        if l.deadline_seconds == 0
            || l.concurrent_machines == 0
            || l.total_machine_allocations < l.concurrent_machines
            || l.output_bytes == 0
        {
            return Err("The recipe limits are invalid.");
        }
        self.capture.validate()
    }
}

/// One immutable recipe revision. Revision 1 is the first draft.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeRevision {
    pub revision: u64,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_digest: Option<String>,
    pub recipe: Recipe,
    pub created_ms: u64,
}

/// The run executing an attempt (a `coder-cloud` job and its task).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLink {
    pub cloud_job: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildState {
    Requested,
    Provisioning,
    Installing,
    PreparingImage,
    SnapshotPending,
    Ready,
    Failed,
    Cancelled,
    NeedsReconciliation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    Requested,
    Restoring,
    Checking,
    Passed,
    Failed,
    Incomplete,
    Cancelled,
    NeedsReconciliation,
}

/// Shared ordering for both attempt lifecycles.
pub trait Stage: Copy + Eq + std::fmt::Debug {
    const RECONCILE: Self;
    /// Position in the forward lifecycle; terminal states share the top rank.
    fn rank(self) -> u8;
    fn terminal(self) -> bool;
}
impl Stage for BuildState {
    const RECONCILE: Self = Self::NeedsReconciliation;
    fn rank(self) -> u8 {
        match self {
            Self::Requested => 0,
            Self::Provisioning => 1,
            Self::Installing => 2,
            Self::PreparingImage => 3,
            Self::SnapshotPending => 4,
            Self::Ready | Self::Failed | Self::Cancelled => 9,
            Self::NeedsReconciliation => 8,
        }
    }
    fn terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed | Self::Cancelled)
    }
}
impl Stage for VerificationState {
    const RECONCILE: Self = Self::NeedsReconciliation;
    fn rank(self) -> u8 {
        match self {
            Self::Requested => 0,
            Self::Restoring => 1,
            Self::Checking => 2,
            Self::Passed | Self::Failed | Self::Incomplete | Self::Cancelled => 9,
            Self::NeedsReconciliation => 8,
        }
    }
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Passed | Self::Failed | Self::Incomplete | Self::Cancelled
        )
    }
}

/// One retained state change. Unknown outcomes stay in this history even
/// after reconciliation resolves them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step<S> {
    pub state: S,
    pub at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
/// An outcome the owner could not observe; blocks dependent operations
/// until a definite observation reconciles it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unresolved<S> {
    pub reason: String,
    pub prior: S,
    pub since_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildAttempt {
    pub id: String,
    pub request_id: String,
    /// Exact frozen inputs: recipe revision, recipe digest, and source.
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunLink>,
    pub state: BuildState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<Unresolved<BuildState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageIdentity>,
    pub history: Vec<Step<BuildState>>,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationAttempt {
    pub id: String,
    pub request_id: String,
    pub build_id: String,
    /// The sealed candidate this verifier targets.
    pub image: ImageIdentity,
    pub plan_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunLink>,
    pub state: VerificationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<Unresolved<VerificationState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_digest: Option<String>,
    /// The sealed status of the evidence `evidence_digest` names. A
    /// version saves only from a passed attempt whose evidence is
    /// complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_status: Option<evidence::EvidenceStatus>,
    pub history: Vec<Step<VerificationState>>,
    pub created_ms: u64,
}

/// A saved environment. Never changes once written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentVersion {
    pub id: String,
    pub number: u64,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub image: ImageIdentity,
    pub build_id: String,
    pub build_run: RunLink,
    pub verification_id: String,
    pub verification_run: RunLink,
    pub plan_digest: String,
    pub evidence_digest: String,
    /// The review that approved this exact candidate (ENV-06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewStamp>,
    pub created_ms: u64,
}

/// The project's selected version, fenced by its own revision.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
}

/// The original fingerprint and result of an idempotent request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEntry {
    pub fingerprint: String,
    pub effect: Effect,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub schema: String,
    pub id: String,
    /// Record revision; increases on every retained change.
    pub revision: u64,
    pub project: ProjectLink,
    pub source: SourcePin,
    /// Current draft revision; `recipes.last()` holds its contents.
    pub draft_revision: u64,
    pub recipes: Vec<RecipeRevision>,
    pub builds: Vec<BuildAttempt>,
    pub verifications: Vec<VerificationAttempt>,
    pub versions: Vec<EnvironmentVersion>,
    pub selection: Selection,
    /// Every move of `selection`, oldest first (ENV-06).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selections: Vec<SelectionChange>,
    pub requests: BTreeMap<String, RequestEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_ms: Option<u64>,
    pub created_ms: u64,
}

impl Environment {
    /// A new environment with its first draft recipe at revision 1.
    pub fn new(
        id: &str,
        project: ProjectLink,
        source: SourcePin,
        recipe: Recipe,
        now_ms: u64,
    ) -> Result<Self, Refusal> {
        if !valid_id(id) || !valid_id(&project.workspace) || !valid_id(&project.project) {
            return Err(Refusal::Invalid(
                "Environment and project identities are invalid.",
            ));
        }
        source.validate().map_err(Refusal::Invalid)?;
        recipe.validate().map_err(Refusal::Invalid)?;
        Ok(Self {
            schema: SCHEMA.into(),
            id: id.into(),
            revision: 1,
            project,
            source,
            draft_revision: 1,
            recipes: vec![RecipeRevision {
                revision: 1,
                digest: recipe.digest(),
                parent_digest: None,
                recipe,
                created_ms: now_ms,
            }],
            builds: vec![],
            verifications: vec![],
            versions: vec![],
            selection: Selection::default(),
            selections: vec![],
            requests: BTreeMap::new(),
            retired_ms: None,
            created_ms: now_ms,
        })
    }
    pub fn draft(&self) -> &RecipeRevision {
        self.recipes.last().expect("environment has a draft")
    }
    pub fn recipe(&self, revision: u64) -> Option<&RecipeRevision> {
        self.recipes.iter().find(|r| r.revision == revision)
    }
    pub fn build(&self, id: &str) -> Option<&BuildAttempt> {
        self.builds.iter().find(|b| b.id == id)
    }
    pub fn verification(&self, id: &str) -> Option<&VerificationAttempt> {
        self.verifications.iter().find(|v| v.id == id)
    }
    pub fn version(&self, id: &str) -> Option<&EnvironmentVersion> {
        self.versions.iter().find(|v| v.id == id)
    }
    /// A build is stale once the draft has moved past the recipe revision
    /// it was frozen to: it can no longer be verified or saved.
    pub fn is_stale(&self, build: &BuildAttempt) -> bool {
        build.recipe_revision != self.draft_revision
    }
    /// Every build a later recipe edit has made stale.
    pub fn stale_builds(&self) -> Vec<&str> {
        self.builds
            .iter()
            .filter(|b| self.is_stale(b))
            .map(|b| b.id.as_str())
            .collect()
    }
    pub fn active(&self) -> Option<&EnvironmentVersion> {
        self.selection
            .active
            .as_deref()
            .and_then(|id| self.version(id))
    }
    /// Every attempt whose outcome is unknown, for projection and reconcile.
    pub fn unresolved(&self) -> Vec<&str> {
        self.builds
            .iter()
            .filter(|b| b.unresolved.is_some())
            .map(|b| b.id.as_str())
            .chain(
                self.verifications
                    .iter()
                    .filter(|v| v.unresolved.is_some())
                    .map(|v| v.id.as_str()),
            )
            .collect()
    }

    /// Structural checks for a retained record.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA || !valid_id(&self.id) {
            return Err("Environment schema or identity mismatch.");
        }
        if self.recipes.is_empty()
            || self.recipes.len() > MAX_RECIPE_REVISIONS
            || self.builds.len() > MAX_ATTEMPTS
            || self.verifications.len() > MAX_ATTEMPTS
            || self.versions.len() > MAX_VERSIONS
            || self.requests.len() > MAX_REQUESTS
            || self.selections.len() > promotion::MAX_SELECTIONS
        {
            return Err("The environment record exceeds its bounds.");
        }
        for (i, r) in self.recipes.iter().enumerate() {
            let parent = i.checked_sub(1).map(|p| self.recipes[p].digest.clone());
            if r.revision != i as u64 + 1
                || r.digest != r.recipe.digest()
                || r.parent_digest != parent
            {
                return Err("The recipe revision chain is broken.");
            }
        }
        if self.draft_revision != self.recipes.len() as u64 {
            return Err("The draft revision does not match its recipe chain.");
        }
        for (i, v) in self.versions.iter().enumerate() {
            if v.number != i as u64 + 1 || v.id != version_id(v.number) {
                return Err("The version sequence is broken.");
            }
        }
        if self
            .selection
            .active
            .as_deref()
            .is_some_and(|id| self.version(id).is_none())
        {
            return Err("The selected version does not exist.");
        }
        for (i, s) in self.selections.iter().enumerate() {
            let previous = i
                .checked_sub(1)
                .map(|p| self.selections[p].version_id.clone());
            if self.version(&s.version_id).is_none()
                || s.previous != previous
                || i.checked_sub(1)
                    .is_some_and(|p| self.selections[p].revision >= s.revision)
            {
                return Err("The selection history is broken.");
            }
        }
        if let Some(last) = self.selections.last() {
            if last.revision != self.selection.revision
                || self.selection.active.as_deref() != Some(last.version_id.as_str())
            {
                return Err("The selection history disagrees with the selection.");
            }
        }
        Ok(())
    }

    /// Whether `next` keeps everything immutable in `self`: recipe revisions,
    /// saved versions, and accepted request results only ever grow.
    pub fn preserves_history_of(&self, next: &Environment) -> bool {
        next.id == self.id
            && next.project == self.project
            && next.source == self.source
            && next.created_ms == self.created_ms
            && next.revision > self.revision
            && next.selection.revision >= self.selection.revision
            && (self.retired_ms.is_none() || next.retired_ms == self.retired_ms)
            && next.recipes.starts_with(&self.recipes)
            && next.versions.starts_with(&self.versions)
            && next.selections.starts_with(&self.selections)
            && next.builds.len() >= self.builds.len()
            && next.verifications.len() >= self.verifications.len()
            && self.builds.iter().zip(&next.builds).all(|(a, b)| {
                (a.id.as_str(), a.request_id.as_str(), a.recipe_revision)
                    == (b.id.as_str(), b.request_id.as_str(), b.recipe_revision)
                    && a.recipe_digest == b.recipe_digest
                    && a.source == b.source
                    && (!a.state.terminal() || a == b)
            })
            && self
                .verifications
                .iter()
                .zip(&next.verifications)
                .all(|(a, b)| {
                    (a.id.as_str(), a.request_id.as_str(), a.build_id.as_str())
                        == (b.id.as_str(), b.request_id.as_str(), b.build_id.as_str())
                        && a.image == b.image
                        && a.plan_digest == b.plan_digest
                        && (!a.state.terminal() || a == b)
                })
            && self
                .requests
                .iter()
                .all(|(k, v)| next.requests.get(k) == Some(v))
    }
}

pub(crate) fn version_id(number: u64) -> String {
    format!("v{number}")
}

#[cfg(test)]
mod tests;
