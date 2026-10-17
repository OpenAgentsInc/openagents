//! The common resources: spaces, documents, versions, assets, artifacts,
//! jobs, notes and references. They are the same for every kind.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Where Labs live: a person's own space or a project. Access follows
/// project membership; the store and API decide it, never this value alone.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpaceRef {
    Personal { account: String },
    Project { project: String },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentMeta {
    pub id: String,
    /// A registered kind (`video`, `cad`, ...).
    pub kind: String,
    /// One of that kind's document schemas (`cut.v1`, `part.v1`, ...).
    pub schema: String,
    pub title: String,
    pub created_unix_ms: u64,
    /// The latest version number (0 before the first).
    pub latest: u64,
}

/// One immutable version. Made only by a kind's `apply` (through the store),
/// never written directly.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    pub document: String,
    pub number: u64,
    pub parent: Option<u64>,
    /// A person, or a chat turn (`chat:<id>`).
    pub author: String,
    pub summary: String,
    pub created_unix_ms: u64,
    pub schema: String,
    pub content: Value,
    /// [`crate::digest`] of `content`.
    pub content_sha256: String,
    /// The kind's check report for this content.
    #[serde(default)]
    pub report: Report,
    /// Every file the content uses.
    #[serde(default)]
    pub assets: Vec<AssetEntry>,
}

impl Version {
    pub fn reference(&self) -> Reference {
        Reference {
            document: self.document.clone(),
            version: self.number,
        }
    }

    /// Whether any input must stay on its enrolled host.
    pub fn has_private_local(&self) -> bool {
        self.assets
            .iter()
            .any(|a| a.class == AssetClass::PrivateLocal)
    }
}

/// What a kind's checks found. Problems that don't block saving (a warning
/// a person may accept) live here; blocking ones are refusals.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Kind-specific facts (mass, duration, sketch freedom, ...).
    #[serde(default)]
    pub facts: Value,
}

/// How a file may travel.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    /// May be stored and rendered anywhere the space allows.
    Public,
    /// Never leaves the host that has it (licensed or personal media:
    /// MetaHuman, Fab packs, fonts). Only an enrolled host renders it.
    PrivateLocal,
    /// May be stored and shown, never used to train or evaluate a model.
    NoTraining,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetEntry {
    /// A path on the host or an object in the space's storage.
    pub location: String,
    pub sha256: String,
    pub class: AssetClass,
}

/// A file made from (or attached to) a version.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub version: Reference,
    pub role: ArtifactRole,
    pub media_type: String,
    pub sha256: String,
    pub size: u64,
    pub class: AssetClass,
    /// Inherited by everything made from a marked input.
    pub no_training: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    Render,
    Poster,
    Frame,
    Strip,
    Export,
    Build,
    Replay,
    Source,
}

/// Work that makes artifacts. Its idempotency key is the version's digest,
/// the type and the parameters, so asking twice never runs it twice.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSpec {
    pub kind: String,
    /// Kind-defined: `render`, `rebuild`, `playtest`, `simulate`, ...
    #[serde(rename = "type")]
    pub job_type: String,
    pub version: Reference,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub requirements: Requirements,
}

impl JobSpec {
    /// The idempotency key: same version content, type and parameters, same key.
    pub fn key(&self, content_sha256: &str) -> String {
        crate::digest(&serde_json::json!({
            "kind": self.kind,
            "type": self.job_type,
            "content": content_sha256,
            "params": self.params,
        }))
    }
}

/// What a placement must offer to take a job.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    /// Minimum CPU capacity, in thousandths of one core (0 uses the runner default).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cpu_millis: u64,
    /// Minimum available memory (0 uses the runner default).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub memory_bytes: u64,
    #[serde(default)]
    pub gpu: bool,
    /// Programs the job runs (`ffmpeg`, `blender`, ...).
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub max_seconds: u64,
    /// Set when an input is [`AssetClass::PrivateLocal`].
    #[serde(default)]
    pub private_local: bool,
    /// Input classes required by this job, in addition to the version's assets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_classes: Vec<AssetClass>,
    /// Explicit use of inputs by a model; absent for ordinary rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_use: Option<ModelUse>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
    /// The worker was lost; the outcome is unknown until observed.
    Uncertain,
}

impl JobStatus {
    pub fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobState {
    pub id: String,
    pub spec: JobSpec,
    pub status: JobStatus,
    /// Plain words for a failure or a wait ("No computer that has these
    /// files is online").
    #[serde(default)]
    pub message: Option<String>,
    /// Seconds the job is expected to take, when known.
    #[serde(default)]
    pub estimate_seconds: Option<f64>,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

/// Where in a version a note points.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "at", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    None,
    Time {
        seconds: f64,
    },
    Frame {
        frame: u64,
    },
    Step {
        step: u64,
    },
    /// A kind's own stable reference (a CAD face `body:top`).
    Ref {
        reference: String,
    },
    Region {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub subject: Reference,
    pub locator: Locator,
    pub author: String,
    pub text: String,
    pub created_unix_ms: u64,
}

/// An immutable version, possibly of another kind's document (a CAD part
/// placed in a Robot Lab scene). Reading through it checks access to both.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub document: String,
    pub version: u64,
}

/// Purposes that must reject protected inputs, including old artifacts whose
/// producer did not yet set `no_training`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUse {
    Training,
    Evaluation,
    Trace,
}

pub fn check_model_use(
    version: &Version,
    artifacts: &[Artifact],
    _purpose: ModelUse,
) -> Result<(), crate::Refusal> {
    if version
        .assets
        .iter()
        .any(|a| matches!(a.class, AssetClass::PrivateLocal | AssetClass::NoTraining))
        || artifacts.iter().any(|a| {
            a.no_training || matches!(a.class, AssetClass::PrivateLocal | AssetClass::NoTraining)
        })
    {
        return Err(crate::Refusal::new(
            "no_training",
            "These files cannot be used to train or evaluate a model, or saved in a model trace.",
        ));
    }
    if artifacts.iter().any(|a| a.version != version.reference()) {
        return Err(crate::Refusal::new(
            "artifact_version",
            "Choose files from this saved version.",
        ));
    }
    Ok(())
}
