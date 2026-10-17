//! Bounded, operator-reviewed SPACE qualification. An assessment never changes
//! deployment settings, grants entitlement, or enables customer charges.
use crate::economics;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path},
};

pub const SCHEMA: &str = "openagents.computers.space-qualification.v1";
pub const RECEIPT_SCHEMA: &str = "openagents.computers.space-case-receipt.v1";

macro_rules! cases {
    ($($case:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all="snake_case")]
        pub enum Case { $($case),+ }
        impl Case { pub const ALL: &'static [Self] = &[$(Self::$case),+]; }
    };
}
cases!(
    PublicSdk,
    WebTaskAndFollowup,
    AccountIsolation,
    ActualBootAndShape,
    StageTimings,
    MemoryAndWorkspace,
    CancellationAndChildren,
    ApiRestart,
    ReconcilerRestart,
    LostCreateReply,
    LostCommandReply,
    AttachInterruption,
    DuplicateDelivery,
    DelayedLifecycle,
    WorkerLossAndSingleWriter,
    CrossHostMemoryRestore,
    CheckpointRecovery,
    DatabaseRestore,
    StorageOutage,
    ProviderOutage,
    CredentialRevocation,
    EgressBypass,
    CapacityExhaustion,
    ConcurrentWake,
    QuotasAndAccounting,
    Retention,
    AccountDeletion,
    OrphanCleanup,
    UpgradeAndRollback,
    DisabledPlacement,
    LegacyBoat,
    UnsupportedControls,
    Monitoring,
    FixtureCleanup
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Passed,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    pub status: Status,
    pub artifact: Option<Artifact>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub web_commit: String,
    pub computers_commit: String,
    pub sdk_commit: String,
    pub native_commit: String,
    pub native_image_sha256: String,
    pub template: String,
    pub runtime_sha256: String,
    pub policy_sha256: String,
    pub schema_sha256: String,
    pub vcpus: u32,
    pub memory_mib: u64,
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}
impl Identity {
    fn valid(&self) -> bool {
        let template = self.template.rsplit_once("@sha256:");
        [
            &self.web_commit,
            &self.computers_commit,
            &self.sdk_commit,
            &self.native_commit,
        ]
        .iter()
        .all(|v| hex(v, 40))
            && [
                &self.native_image_sha256,
                &self.runtime_sha256,
                &self.policy_sha256,
                &self.schema_sha256,
            ]
            .iter()
            .all(|v| hex(v, 64))
            && template.is_some_and(|(name, digest)| relative(name) && hex(digest, 64))
            && (1..=4096).contains(&self.vcpus)
            && (128..=4194304).contains(&self.memory_mib)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cold,
    Snapshot,
    Wake,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cache {
    Cold,
    Warm,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timings {
    pub create_ms: Option<u64>,
    pub reconcile_ms: Option<u64>,
    pub readiness_ms: Option<u64>,
    pub attach_ms: Option<u64>,
    pub repository_ms: Option<u64>,
    pub model_ms: Option<u64>,
    /// End-to-end duration through independently verified useful output.
    pub first_useful_ms: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub kind: Kind,
    pub cache: Cache,
    pub concurrency: u32,
    pub succeeded: bool,
    pub timings: Timings,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub minimum_samples: u32,
    pub required_concurrency: u32,
    pub snapshot_p50_ms: u64,
    pub snapshot_p90_ms: u64,
    pub wake_p90_ms: u64,
    pub maximum_error_permille: u16,
    pub maximum_service_cost_per_useful_task_micros: u64,
    pub currency: String,
    pub maximum_age_seconds: u64,
}
impl Limits {
    fn valid(&self) -> bool {
        (20..=10000).contains(&self.minimum_samples)
            && (2..=4096).contains(&self.required_concurrency)
            && (1..2000).contains(&self.snapshot_p50_ms)
            && self.snapshot_p90_ms >= self.snapshot_p50_ms
            && self.wake_p90_ms > 0
            && self.maximum_error_permille <= 1000
            && self.maximum_service_cost_per_useful_task_micros > 0
            && (1..=86400).contains(&self.maximum_age_seconds)
            && self.currency.len() == 3
            && self.currency.bytes().all(|b| b.is_ascii_uppercase())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advertised {
    pub fork: bool,
    pub terminal: bool,
    pub preview: bool,
    pub cross_host_memory_restore: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub schema: String,
    pub identity: Option<Identity>,
    pub observed_unix: Option<u64>,
    pub advertised: Option<Advertised>,
    /// Exact hashed account owners from the deployment registry, never `*`.
    pub accounts: BTreeSet<String>,
    pub dedicated_test_accounts: bool,
    pub private_management: bool,
    pub rollback_ready: bool,
    pub pricing_enabled: bool,
    pub limits: Option<Limits>,
    pub cases: BTreeMap<Case, Proof>,
    pub trials: Vec<Trial>,
    pub economics: Option<economics::Window>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Distribution {
    pub attempts: usize,
    pub failures: usize,
    pub successful_samples: usize,
    pub p50_ms: Option<u64>,
    pub p90_ms: Option<u64>,
    pub cold_cache_samples: usize,
    pub warm_cache_samples: usize,
    pub maximum_concurrency: u32,
    pub observed_error_permille: Option<u64>,
}
fn distribution(trials: &[Trial], kind: Kind) -> Distribution {
    let trials: Vec<_> = trials.iter().filter(|t| t.kind == kind).collect();
    let mut values: Vec<_> = trials
        .iter()
        .filter(|t| t.succeeded)
        .filter_map(|t| t.timings.first_useful_ms)
        .collect();
    values.sort_unstable();
    let percentile =
        |p: usize| (!values.is_empty()).then(|| values[(values.len() * p).div_ceil(100) - 1]);
    Distribution {
        attempts: trials.len(),
        failures: trials.iter().filter(|t| !t.succeeded).count(),
        successful_samples: values.len(),
        p50_ms: percentile(50),
        p90_ms: percentile(90),
        cold_cache_samples: trials
            .iter()
            .filter(|t| t.succeeded && t.cache == Cache::Cold)
            .count(),
        warm_cache_samples: trials
            .iter()
            .filter(|t| t.succeeded && t.cache == Cache::Warm)
            .count(),
        maximum_concurrency: trials
            .iter()
            .filter(|t| t.succeeded)
            .map(|t| t.concurrency)
            .max()
            .unwrap_or(0),
        observed_error_permille: (!trials.is_empty()).then(|| {
            (trials.iter().filter(|t| !t.succeeded).count() as u64 * 1000)
                .div_ceil(trials.len() as u64)
        }),
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Assessment {
    pub schema: &'static str,
    pub ready_for_dedicated_tests: bool,
    pub ready_for_named_group: bool,
    pub broad_admission: bool,
    pub pricing_enabled: bool,
    pub incomplete_cases: Vec<Case>,
    pub failed_cases: Vec<Case>,
    pub invalid_artifacts: Vec<Case>,
    pub blockers: Vec<&'static str>,
    pub snapshot: Distribution,
    pub wake: Distribution,
    pub observed_error_permille: Option<u64>,
    pub economics: Option<economics::Report>,
}
fn relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.-".contains(&b))
        && Path::new(value)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub identity: Identity,
    pub observed_unix: u64,
    pub cases: BTreeMap<Case, Status>,
    /// Scrubbed raw checks or archive reviewed by the operator.
    pub source: Artifact,
}
fn artifact_bytes(root: &Path, artifact: &Artifact, remaining: &mut u64) -> Option<Vec<u8>> {
    if !relative(&artifact.path) || !hex(&artifact.sha256, 64) {
        return None;
    }
    let mut path = root.to_owned();
    for component in Path::new(&artifact.path).components() {
        path.push(component);
        if std::fs::symlink_metadata(&path).map_or(true, |m| m.file_type().is_symlink()) {
            return None;
        }
    }
    let file = std::fs::File::open(&path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > 16 * 1024 * 1024 || meta.len() > *remaining {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    *remaining = remaining.checked_sub(bytes.len() as u64)?;
    if bytes.len() > 16 * 1024 * 1024 || format!("{:x}", Sha256::digest(&bytes)) != artifact.sha256
    {
        return None;
    }
    Some(bytes)
}

impl Packet {
    pub fn template() -> serde_json::Value {
        let cases: BTreeMap<_, _> = Case::ALL
            .iter()
            .map(|case| {
                (
                    case,
                    Proof {
                        status: Status::Pending,
                        artifact: None,
                    },
                )
            })
            .collect();
        serde_json::json!({"schema":SCHEMA,"identity":null,"observed_unix":null,"advertised":null,"accounts":[],"dedicated_test_accounts":true,
            "private_management":false,"rollback_ready":false,"pricing_enabled":false,"limits":null,
            "cases":cases,"trials":[],"economics":null})
    }
    pub fn assess(&self, root: &Path) -> Result<Assessment, &'static str> {
        if self.schema != SCHEMA
            || self.accounts.len() > 8
            || self.trials.len() > 20000
            || self
                .accounts
                .iter()
                .any(|a| !a.strip_prefix("account:").is_some_and(|v| hex(v, 64)))
            || self.identity.as_ref().is_some_and(|i| !i.valid())
            || self.limits.as_ref().is_some_and(|l| !l.valid())
            || self.trials.iter().any(|t| {
                t.concurrency == 0
                    || t.concurrency > 4096
                    || t.succeeded && t.timings.first_useful_ms.is_none_or(|ms| ms == 0)
            })
        {
            return Err("The qualification packet is invalid.");
        }
        let mut incomplete_cases = Vec::new();
        let mut failed_cases = Vec::new();
        let mut invalid_artifacts = Vec::new();
        let mut receipts: BTreeMap<(String, String), Option<Receipt>> = BTreeMap::new();
        let mut sources = BTreeSet::new();
        let mut remaining = 128 * 1024 * 1024;
        for case in Case::ALL {
            if *case == Case::CrossHostMemoryRestore
                && self
                    .advertised
                    .as_ref()
                    .is_some_and(|a| !a.cross_host_memory_restore)
            {
                continue;
            }
            match self.cases.get(case) {
                None
                | Some(Proof {
                    status: Status::Pending,
                    ..
                }) => incomplete_cases.push(*case),
                Some(Proof {
                    status: Status::Failed,
                    ..
                }) => failed_cases.push(*case),
                Some(Proof {
                    status: Status::Passed,
                    artifact,
                }) => {
                    let valid = artifact.as_ref().is_some_and(|a| {
                        let receipt = receipts
                            .entry((a.path.clone(), a.sha256.clone()))
                            .or_insert_with(|| {
                                let bytes = artifact_bytes(root, a, &mut remaining)?;
                                if bytes.len() > 1024 * 1024 {
                                    return None;
                                }
                                let receipt: Receipt = serde_json::from_slice(&bytes).ok()?;
                                if receipt.schema != RECEIPT_SCHEMA
                                    || self.identity.as_ref() != Some(&receipt.identity)
                                    || self.observed_unix != Some(receipt.observed_unix)
                                {
                                    return None;
                                }
                                let source =
                                    (receipt.source.path.clone(), receipt.source.sha256.clone());
                                if !sources.contains(&source) {
                                    artifact_bytes(root, &receipt.source, &mut remaining)?;
                                    sources.insert(source);
                                }
                                Some(receipt)
                            });
                        receipt
                            .as_ref()
                            .is_some_and(|r| r.cases.get(case) == Some(&Status::Passed))
                    });
                    if !valid {
                        invalid_artifacts.push(*case);
                    }
                }
            }
        }
        let mut blockers = Vec::new();
        if self.identity.is_none() {
            blockers.push("missing_identity");
        }
        if !self.private_management {
            blockers.push("management_not_private");
        }
        if !self.rollback_ready {
            blockers.push("rollback_not_ready");
        }
        if self.pricing_enabled {
            blockers.push("pricing_requires_owner_decision");
        }
        if self.accounts.is_empty() {
            blockers.push("missing_named_accounts");
        }
        let ready_for_dedicated_tests =
            blockers.is_empty() && self.dedicated_test_accounts && self.accounts.len() <= 2;
        match &self.advertised {
            None => blockers.push("missing_capability_observation"),
            Some(a) if a.fork || a.terminal || a.preview => {
                blockers.push("unsupported_controls_advertised")
            }
            Some(_) => {}
        }
        if !incomplete_cases.is_empty() {
            blockers.push("incomplete_cases");
        }
        if !failed_cases.is_empty() {
            blockers.push("failed_cases");
        }
        if !invalid_artifacts.is_empty() {
            blockers.push("missing_or_changed_artifacts");
        }
        let snapshot = distribution(&self.trials, Kind::Snapshot);
        let wake = distribution(&self.trials, Kind::Wake);
        let failures = self.trials.iter().filter(|t| !t.succeeded).count() as u64;
        let observed_error_permille =
            (!self.trials.is_empty()).then(|| (failures * 1000).div_ceil(self.trials.len() as u64));
        let economics = self.economics.as_ref().map(|e| e.report()).transpose()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "The qualification clock is unavailable.")?
            .as_secs();
        if let Some(limits) = &self.limits {
            if self
                .observed_unix
                .is_none_or(|at| at > now || now - at > limits.maximum_age_seconds)
            {
                blockers.push("observation_time_missing_or_expired");
            }
            if snapshot.successful_samples < limits.minimum_samples as usize
                || wake.successful_samples < limits.minimum_samples as usize
            {
                blockers.push("insufficient_samples");
            }
            if snapshot.cold_cache_samples == 0
                || snapshot.warm_cache_samples == 0
                || snapshot.maximum_concurrency < limits.required_concurrency
                || wake.maximum_concurrency < limits.required_concurrency
            {
                blockers.push("missing_cache_or_concurrency_coverage");
            }
            if snapshot.p50_ms.is_none_or(|n| n > limits.snapshot_p50_ms)
                || snapshot.p90_ms.is_none_or(|n| n > limits.snapshot_p90_ms)
                || wake.p90_ms.is_none_or(|n| n > limits.wake_p90_ms)
            {
                blockers.push("latency_budget_exceeded");
            }
            if [
                observed_error_permille,
                snapshot.observed_error_permille,
                wake.observed_error_permille,
            ]
            .iter()
            .any(|rate| rate.is_none_or(|n| n > u64::from(limits.maximum_error_permille)))
            {
                blockers.push("error_budget_exceeded");
            }
            if economics.as_ref().is_none_or(|e| {
                e.currency != limits.currency
                    || e.guest_vcpu_ms.is_none()
                    || e.command_wall_ms.is_none()
                    || e.service_cost_per_useful_task_micros
                        .is_none_or(|n| n > limits.maximum_service_cost_per_useful_task_micros)
            }) {
                blockers.push("cost_or_capacity_measurement_missing_or_over_budget");
            }
        } else {
            blockers.push("missing_operator_budgets");
        }
        Ok(Assessment {
            schema: "openagents.computers.space-assessment.v1",
            ready_for_dedicated_tests,
            ready_for_named_group: blockers.is_empty(),
            broad_admission: false,
            pricing_enabled: false,
            incomplete_cases,
            failed_cases,
            invalid_artifacts,
            blockers,
            snapshot,
            wake,
            observed_error_permille,
            economics,
        })
    }
}

#[cfg(test)]
mod tests;
