//! Portable Lab worker resources, placement policy, and job-only requests.
use crate::{AssetClass, JobSpec, KindManifest, Refusal, Requirements, SpaceRef, Version};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The service binds this value to a worker's enrollment, never a poll argument.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Server,
    Cloud,
    Enrolled,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub worker: String,
    pub placement: Placement,
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub gpu: bool,
    pub tools: BTreeSet<String>,
    /// Supported pairs, for example `video/render`.
    pub jobs: BTreeSet<String>,
    /// Private input checksums already present on this enrolled host.
    pub local_assets: BTreeSet<String>,
    pub max_seconds: u64,
}

impl Capabilities {
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.worker.is_empty()
            || self.worker.len() > 128
            || self.cpu_millis == 0
            || self.cpu_millis > 1_024_000
            || self.memory_bytes < 1024 * 1024
            || self.memory_bytes > 16 * 1024_u64.pow(4)
            || !(1..=86_400).contains(&self.max_seconds)
            || self.tools.len() > 64
            || self.jobs.is_empty()
            || self.jobs.len() > 64
            || self.local_assets.len() > 512
            || self.tools.iter().any(|s| !identifier(s))
            || self.jobs.iter().any(|s| {
                s.split_once('/')
                    .is_none_or(|(kind, job)| !identifier(kind) || !identifier(job))
            })
            || self.local_assets.iter().any(|s| !checksum(s))
            || (self.placement != Placement::Enrolled && !self.local_assets.is_empty())
        {
            return Err(Refusal::new(
                "worker_settings",
                "Check the worker's tools and limits.",
            ));
        }
        Ok(())
    }
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_+.".contains(&b))
}

fn checksum(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveRequirements {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub gpu: bool,
    pub tools: BTreeSet<String>,
    pub max_seconds: u64,
    pub private_local: bool,
    pub local_assets: BTreeSet<String>,
    pub no_training: bool,
    pub job: String,
}

impl EffectiveRequirements {
    /// Combine the kind's policy and immutable inputs before considering a worker.
    /// A caller cannot remove a requirement by omitting it from the job request.
    pub fn for_job(
        spec: &JobSpec,
        version: &Version,
        kind: &KindManifest,
    ) -> Result<Self, Refusal> {
        if spec.kind != kind.kind
            || !kind.job_types.contains(&spec.job_type)
            || spec.version != version.reference()
        {
            return Err(Refusal::new(
                "job_kind",
                "Choose work this document supports.",
            ));
        }
        let default = Requirements::default();
        let policy = kind
            .job_requirements
            .get(&spec.job_type)
            .unwrap_or(&default);
        let requested = &spec.requirements;
        for purpose in [policy.model_use, requested.model_use]
            .into_iter()
            .flatten()
        {
            crate::check_model_use(version, &[], purpose)?;
        }
        let limit = if policy.max_seconds == 0 {
            3600
        } else {
            policy.max_seconds
        };
        let duration = if requested.max_seconds == 0 {
            limit.min(600)
        } else {
            requested.max_seconds
        };
        if duration == 0 || duration > limit || limit > 86_400 {
            return Err(Refusal::new(
                "job_duration",
                "Choose a shorter run for this work.",
            ));
        }
        let tools: BTreeSet<_> = requested
            .tools
            .iter()
            .chain(&policy.tools)
            .cloned()
            .collect();
        if tools.len() > 64 || tools.iter().any(|s| !identifier(s)) {
            return Err(Refusal::new(
                "job_tools",
                "Choose tools this worker supports.",
            ));
        }
        let classes = requested.asset_classes.iter().chain(&policy.asset_classes);
        let private_local = requested.private_local
            || policy.private_local
            || classes.clone().any(|c| *c == AssetClass::PrivateLocal)
            || version.has_private_local();
        let no_training = private_local
            || classes.clone().any(|c| *c == AssetClass::NoTraining)
            || version
                .assets
                .iter()
                .any(|a| a.class == AssetClass::NoTraining);
        let local_assets: BTreeSet<_> = version
            .assets
            .iter()
            .filter(|a| a.class == AssetClass::PrivateLocal)
            .map(|a| a.sha256.clone())
            .collect();
        if local_assets.iter().any(|s| !checksum(s)) {
            return Err(Refusal::new(
                "asset_checksum",
                "Check the private files' checksums.",
            ));
        }
        let cpu_millis = requested.cpu_millis.max(policy.cpu_millis).max(1000);
        let memory_bytes = requested
            .memory_bytes
            .max(policy.memory_bytes)
            .max(64 * 1024 * 1024);
        if cpu_millis > 1_024_000 || memory_bytes > 16 * 1024_u64.pow(4) {
            return Err(Refusal::new(
                "job_resources",
                "Choose a smaller computer for this work.",
            ));
        }
        Ok(Self {
            cpu_millis,
            memory_bytes,
            gpu: requested.gpu || policy.gpu,
            tools,
            max_seconds: duration,
            private_local,
            local_assets,
            no_training,
            job: format!("{}/{}", spec.kind, spec.job_type),
        })
    }

    pub fn eligible(&self, worker: &Capabilities) -> bool {
        worker.validate().is_ok()
            && worker.jobs.contains(&self.job)
            && worker.cpu_millis >= self.cpu_millis
            && worker.memory_bytes >= self.memory_bytes
            && (!self.gpu || worker.gpu)
            && worker.max_seconds >= self.max_seconds
            && self.tools.is_subset(&worker.tools)
            && (!self.private_local
                || (worker.placement == Placement::Enrolled
                    && self.local_assets.is_subset(&worker.local_assets)))
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Decision {
    Assigned {
        worker: String,
        placement: Placement,
    },
    Queued {
        message: String,
    },
}

/// `workers` contains only current authorized, online workers with a free slot.
/// Sorting makes the answer independent of heartbeat and inventory order.
pub fn select(requirements: &EffectiveRequirements, workers: &[Capabilities]) -> Decision {
    match workers
        .iter()
        .filter(|w| requirements.eligible(w))
        .min_by_key(|w| (w.placement, &w.worker))
    {
        Some(w) => Decision::Assigned {
            worker: w.worker.clone(),
            placement: w.placement,
        },
        None => Decision::Queued {
            message: if requirements.private_local {
                "No computer with these files is online."
            } else {
                "No computer with the tools and space for this work is available."
            }
            .into(),
        },
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lease {
    pub job: crate::JobState,
    pub worker: String,
    pub attempt: u64,
    pub expires_unix_ms: u64,
    pub started: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub job: String,
    pub attempt: u64,
    pub worker: String,
    pub account: String,
    pub space: SpaceRef,
    pub placement: Placement,
    pub started_unix_ms: Option<u64>,
    pub observed_until_unix_ms: u64,
    /// Time between service observations, not CPU usage or provider billable time.
    pub observed_wall_ms: Option<u64>,
    pub outcome: crate::JobStatus,
    pub charge: Option<u64>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    pub id: String,
    pub account: String,
    pub space: SpaceRef,
    pub capabilities: Capabilities,
    pub expires_unix_ms: u64,
    pub revoked: bool,
    /// The issuing app/login-key session reference, never its bearer token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_session: Option<String>,
}

/// Returned once to the enrolling account. Debug output never contains the key.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
pub struct Enrolled {
    pub worker: Worker,
    pub token: String,
}
impl std::fmt::Debug for Enrolled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enrolled")
            .field("worker", &self.worker)
            .finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub lease: Lease,
    /// The service records this before returning permission to execute.
    pub started_unix_ms: Option<u64>,
}

pub mod protocol;
