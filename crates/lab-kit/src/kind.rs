//! The contract each Lab kind implements. Pure: no network, files or
//! engines, so the server, workers and every client run the same rules.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Artifact, Card, Change, JobSpec, JobState, Refusal, Report, Version, View};

/// What a kind declares about itself. Served by `GET /labs/kinds`, and
/// enough for a chat to offer its tools and for a client to draw its cards.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindManifest {
    /// Stable id (`video`, `cad`, ...). Display names may change; this doesn't.
    pub kind: String,
    pub name: String,
    pub schemas: Vec<String>,
    /// Every tool name starts with this (`lab_`, `cad_`, ...), so prompts and
    /// older clients keep working.
    pub tool_prefix: String,
    /// The argument a tool names its document by (`document`, `doc`, `game`).
    pub doc_arg: String,
    pub tools: Vec<ToolDecl>,
    pub job_types: Vec<String>,
    /// Minimum resources and maximum run time for each supported job type.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub job_requirements: std::collections::BTreeMap<String, crate::Requirements>,
    pub views: Vec<View>,
    pub limits: Limits,
}

/// A tool for a model loop, as OpenAI-style function declarations write it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDecl {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_doc_bytes: u64,
    pub max_docs_per_space: u64,
    pub max_versions_per_doc: u64,
    pub max_args_bytes: u64,
    pub runs_per_hour: u64,
}

impl Default for Limits {
    /// CAD on openagents.com's limits, the first hosted kind's.
    fn default() -> Self {
        Self {
            max_doc_bytes: 512 * 1024,
            max_docs_per_space: 100,
            max_versions_per_doc: 300,
            max_args_bytes: 256 * 1024,
            runs_per_hour: 120,
        }
    }
}

/// Facts that need media or heavy compute (a clip's still stretches, a
/// bot's playtest). A job computes and caches them, so `check` stays pure.
pub type Measurements = Value;

/// A new version before the store numbers and saves it.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub content: Value,
    pub summary: String,
    pub report: Report,
    pub assets: Vec<crate::AssetEntry>,
}

/// What a caller wants made from a version.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "want", rename_all = "snake_case", deny_unknown_fields)]
pub enum Want {
    /// What the card needs to show this version (a preview, a hero image).
    Card,
    /// A named job type with its parameters (`render` at `review`).
    Job {
        #[serde(rename = "type")]
        job_type: String,
        #[serde(default)]
        params: Value,
    },
}

pub trait LabKind {
    const KIND: &'static str;

    fn manifest() -> KindManifest;

    /// Check content. A blocking problem is a refusal; others are warnings.
    fn check(content: &Value, measurements: &Measurements) -> Result<Report, Refusal>;

    /// Turn a change on `base` into a draft, checked. `summary` is the
    /// person's or chat's one line.
    fn apply(
        base: &Version,
        change: &Change,
        summary: &str,
        measurements: &Measurements,
    ) -> Result<Draft, Refusal>;

    fn jobs_for(version: &Version, want: &Want) -> Vec<JobSpec>;

    fn card(version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card;
}
