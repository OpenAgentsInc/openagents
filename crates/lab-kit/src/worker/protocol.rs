//! The job-only worker protocol; requests cannot select an account or executable.
use super::{Assignment, Capabilities, EffectiveRequirements, Worker};
use crate::{ArtifactRole, JobStatus, Version};
use serde::{Deserialize, Serialize};

fn lifetime() -> u64 {
    3_600_000
}
fn personal() -> String {
    "me".into()
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enroll {
    #[serde(default = "personal")]
    pub space: String,
    pub capabilities: Capabilities,
    #[serde(default = "lifetime")]
    pub lifetime_ms: u64,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Access {
    pub worker: Worker,
    pub experimental: bool,
    #[serde(default)]
    pub cad: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    #[serde(default)]
    pub wait_seconds: u64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Work {
    pub assignment: Assignment,
    pub version: Version,
    pub requirements: EffectiveRequirements,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Available {
    pub work: Option<Work>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub job: String,
    pub attempt: u64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub role: ArtifactRole,
    pub media_type: String,
    pub content_base64: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    pub job: String,
    pub attempt: u64,
    pub status: JobStatus,
    #[serde(default)]
    pub outputs: Vec<Output>,
}
