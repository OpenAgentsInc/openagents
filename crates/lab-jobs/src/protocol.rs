//! The job-only worker protocol; requests cannot select an account or executable.
use crate::{
    Capabilities, EffectiveRequirements,
    authority::{Assignment, Worker},
};
use lab_kit::{ArtifactRole, JobStatus, Version};
use serde::{Deserialize, Serialize};

fn lifetime() -> u64 {
    3_600_000
}
fn personal() -> String {
    "me".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Enroll {
    #[serde(default = "personal")]
    pub space: String,
    pub capabilities: Capabilities,
    #[serde(default = "lifetime")]
    pub lifetime_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Access {
    pub worker: Worker,
    pub experimental: bool,
    #[serde(default)]
    pub cad: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    #[serde(default)]
    pub wait_seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Work {
    pub assignment: Assignment,
    pub version: Version,
    pub requirements: EffectiveRequirements,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Available {
    pub work: Option<Work>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub job: String,
    pub attempt: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub role: ArtifactRole,
    pub media_type: String,
    pub content_base64: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    pub job: String,
    pub attempt: u64,
    pub status: JobStatus,
    #[serde(default)]
    pub outputs: Vec<Output>,
}
