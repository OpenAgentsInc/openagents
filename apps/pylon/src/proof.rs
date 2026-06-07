use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::time::Instant;

use crate::render_byte_size;

const PROOF_RUNTIME_SCHEMA_VERSION: u32 = 1;
const DEFAULT_PROOF_NAMESPACE: &str = "authority";
const PROOF_PORT_BASE: u16 = 43_000;
const PROOF_PORT_SLOTS: u16 = 2_000;
const PROOF_PORT_STRIDE: u16 = 10;
const PROOF_ROUTE_TIMEOUT: Duration = Duration::from_secs(10);
const PROOF_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROOF_FLEET_DIAGNOSTIC_INTERVAL: Duration = Duration::from_secs(2);
const PROOF_ROUTE_PROBE_TIMEOUT: Duration = Duration::from_millis(750);
const PROOF_ROUTE_PROBE_TOTAL_TIMEOUT: Duration = Duration::from_secs(3);
const PROOF_ARTIFACT_BUCKET: &str = "gs://proof-local-artifacts";
const PROOF_ARTIFACT_UPLOAD_PREFIX: &str = "/upload";
const HOSTED_CS336_A1_STARTER_NETWORK_ID: &str = "trainnet.cs336.a1.demo";
const TEST_GCS_SERVICE_ACCOUNT_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC9YHg+P4UZig1h\nzoW/m8IbzylR9O6/9xrqmIzlSfA2S1Cz7w0P+viRoyzLBmYhTmI0p3RmNAMKWwph\nly6a0UkdsGbWsoKoWt8r+gB1zUyP+1tG4A7HDTcTnxG+T2dtJcwE/A0Y8rF4PKEt\nV0qTdHYjRZrEorBYKJdgUbdv1Pgkw0U9SuCJciRLs3SI3PPrKNhNyWERS5Ta0Hnr\nXtwzZ7e44KNJ8F8iMOgh70p0nLN/KtKl+2Gb/CuJh3Mfodkoc+sADKoofBXZct2+\nsGSw66S08q7WfuPkseaqxDlOgSfaHEjzTIMyoxvjyjRWjulVbUIz8i+JWSZUglfP\nIBsQcN1pAgMBAAECggEAAR3yRH5byNkVX4mXVscdkaBZQ35/6qLkz5cZ/3+VeXrA\nUP8uPYGoXQMOEfuoyfFhTZ0OTxRz0lVpmNX63oZ72kWS+jIPUqqeDt/YNwVeQIrp\nCAYGEwV8I+K+Si69sIm9kf2dYEJndw4Zd/QtYGrC+8R+vBaXRagvV2k0wggXVdzx\n7Wq5zqOz9QkeoG11hTkYAgTmVl5PBnAoRE/sNMtYUOf6JnQWmFpEwOTdTf+F8NL1\nFg+ecNH7tjoqsTBjD/lMSaA/kr10fUw4KoITkn2IvtuF2ZFZp2R/Viy9KnfsLyF7\nyb1NJSP2cn3gYp4+BEe5wOdQNO2+lZN7EQKmRzS7uwKBgQD7dtqD6pAw9VFSiLWN\nW8EcDevKOP48lOP++2esUCsXfip3Omn0lmyb+8i11GRz0QwiMywQ21p7sEUwn9HE\nTk2ZjPnaNdPN+i/vZ+RgcmHVeEzeTPNAXeAQ5zAlrJ8Ibh3239BeWHLxCa/p2nsD\nPL3dPXg/CQm68Ph/UjG9XiXSbwKBgQDAyuzEzrqgdc51x2Z40lcQ56zUZVVtW+A8\n485dS5VQMdwFglXzC5QTQ4T3zI1qT+Dd5ATtCkyMNpL07nC/9rQhI0+HTsRZE8P+\nKeSGIFOSvkA2ZwHWKKcctO8n1vOlAwJnjqYEJAZMIg01MtpOFRN0qrDd/9BDUbHi\nHO2smCRZpwKBgAn28r/Jer9F6VwQ6MjaOvPGpXJVAdYavFItWjVc0+hRapNg8DPu\nBg3EU3bJHNXuEcIFLxjX6GUAXi2IF8Lkq3SLPpdkDKmb4WxmPImJ3tCbvMgOWpFR\nZwCkeKb1iTPHUU6oHdSvQpbEoIDu1HMTZB6xQeOVkxoiVGaPNkNfyLXnAoGAeEXg\nQcNKUFJOM9HqzpNCN8ygWHzDN48qrDHeCvvdMYN5ZIJ0BkUB4qarrD+TNXCRszvO\nCuby7EIbmeuqsUdCBq5Vre7otT2MduJBq589I/3GZ2oJjkYcQt9pl2wU4aun81zd\nmxWyTAquPLL11+J0GcNmxYgSr/ymQY6Ug6kCfF8CgYEA2fSIcskydJ94TpX8Dpqm\nBwDXhRIZo6hkLjAqt6hHa7Fs/2qZXAeeX7/oxxfHBWqtPcTnp3N91xgfkPjarPeM\nth0qg1Cu4Y4ZyQfpaVaZB3aWIJB0PdWdMBZa/EUZDu9kFoaExF3BdzA2j7pmMDj4\nOZi9gzTa10z894ZuBJJkMPA=\n-----END PRIVATE KEY-----\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofCommand {
    Authority { command: ProofAuthorityCommand },
    Fleet { command: ProofFleetCommand },
    Run { command: ProofRunCommand },
    Doctor { command: ProofDoctorCommand },
    Internal { command: ProofInternalCommand },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofAuthorityCommand {
    Up {
        namespace: String,
        mode: ProofAuthorityMode,
        json: bool,
    },
    Status {
        namespace: String,
        json: bool,
    },
    Down {
        namespace: String,
        json: bool,
    },
    Reset {
        namespace: String,
        json: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofFleetCommand {
    Up {
        namespace: String,
        mode: ProofAuthorityMode,
        workers: usize,
        validators: usize,
        network_id: Option<String>,
        stale_worker_state: bool,
        stale_validator_state: bool,
        json: bool,
    },
    Status {
        namespace: String,
        json: bool,
    },
    Down {
        namespace: String,
        json: bool,
    },
    Reset {
        namespace: String,
        json: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRunCommand {
    lane: ProofLane,
    namespace: Option<String>,
    mode: ProofAuthorityMode,
    workers: usize,
    validators: usize,
    timeout_seconds: u64,
    stale_worker_state: bool,
    stale_validator_state: bool,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofDoctorCommand {
    namespace: String,
    json: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofInternalCommand {
    ArtifactStoreServe {
        listen_addr: SocketAddr,
        store_root: PathBuf,
        trace_path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofAuthorityMode {
    ProdShaped,
    DebugAuthority,
}

impl ProofAuthorityMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProdShaped => "prod_shaped",
            Self::DebugAuthority => "debug_authority",
        }
    }

    fn authority_binary(self) -> &'static str {
        match self {
            Self::ProdShaped => "nexus-relay",
            Self::DebugAuthority => "nexus-control",
        }
    }

    fn authority_package(self) -> &'static str {
        self.authority_binary()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofLane {
    Cs336A1,
    Cs336A1HostedStarter,
    Cs336A1StaleRecovery,
    Cs336A1ReplacementAttempt,
    A1MinimalDistributedLmLaunchA,
    A1MinimalDistributedLmLaunchB,
}

impl ProofLane {
    fn label(self) -> &'static str {
        match self {
            Self::Cs336A1 => "cs336-a1",
            Self::Cs336A1HostedStarter => "cs336-a1-hosted-starter",
            Self::Cs336A1StaleRecovery => "cs336-a1-stale-recovery",
            Self::Cs336A1ReplacementAttempt => "cs336-a1-replacement-attempt",
            Self::A1MinimalDistributedLmLaunchA => "a1-minimal-distributed-lm-launch-a",
            Self::A1MinimalDistributedLmLaunchB => "a1-minimal-distributed-lm-launch-b",
        }
    }

    fn run_prefix(self) -> &'static str {
        match self {
            Self::Cs336A1 => "run.cs336.a1.proof",
            Self::Cs336A1HostedStarter => "run.cs336.a1.proof.hosted.starter",
            Self::Cs336A1StaleRecovery => "run.cs336.a1.proof.stale",
            Self::Cs336A1ReplacementAttempt => "run.cs336.a1.proof.replace",
            Self::A1MinimalDistributedLmLaunchA => "run.a1_minimal_distributed_lm.proof.launch_a",
            Self::A1MinimalDistributedLmLaunchB => "run.a1_minimal_distributed_lm.proof.launch_b",
        }
    }

    fn display_name_prefix(self) -> &'static str {
        match self {
            Self::Cs336A1 => "Proof CS336 A1",
            Self::Cs336A1HostedStarter => "Proof CS336 A1 Hosted Starter",
            Self::Cs336A1StaleRecovery => "Proof CS336 A1 Stale Recovery",
            Self::Cs336A1ReplacementAttempt => "Proof CS336 A1 Replacement Attempt",
            Self::A1MinimalDistributedLmLaunchA => "Proof A1 Minimal Distributed LM Launch A",
            Self::A1MinimalDistributedLmLaunchB => "Proof A1 Minimal Distributed LM Launch B",
        }
    }

    const fn default_workers(self) -> usize {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter => 2,
            Self::Cs336A1StaleRecovery => 1,
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => 0,
        }
    }

    const fn default_validators(self) -> usize {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter => 1,
            Self::Cs336A1StaleRecovery => 1,
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => 0,
        }
    }

    const fn minimum_workers(self) -> usize {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter | Self::Cs336A1StaleRecovery => 1,
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => 0,
        }
    }

    const fn minimum_validators(self) -> usize {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter | Self::Cs336A1StaleRecovery => 1,
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => 0,
        }
    }

    const fn worker_fixture(self) -> Option<ProofNodeRuntimeFixture> {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter => None,
            Self::Cs336A1StaleRecovery => Some(ProofNodeRuntimeFixture::StaleWorkerLease),
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => None,
        }
    }

    const fn validator_fixture(self) -> Option<ProofNodeRuntimeFixture> {
        match self {
            Self::Cs336A1 | Self::Cs336A1HostedStarter => None,
            Self::Cs336A1StaleRecovery => Some(ProofNodeRuntimeFixture::StaleValidatorLease),
            Self::Cs336A1ReplacementAttempt
            | Self::A1MinimalDistributedLmLaunchA
            | Self::A1MinimalDistributedLmLaunchB => None,
        }
    }

    const fn uses_manual_authority_scenario(self) -> bool {
        matches!(self, Self::Cs336A1ReplacementAttempt)
    }

    const fn uses_a1_minimal_simulated_scenario(self) -> bool {
        matches!(
            self,
            Self::A1MinimalDistributedLmLaunchA | Self::A1MinimalDistributedLmLaunchB
        )
    }

    const fn uses_hosted_starter_autolaunch(self) -> bool {
        matches!(self, Self::Cs336A1HostedStarter)
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProofNodeRuntimeFixture {
    StaleWorkerLease,
    StaleValidatorLease,
    CloseoutObservePayoutWorker,
    CloseoutObservePayoutValidator,
}

impl ProofNodeRuntimeFixture {
    const fn fixture_id(self) -> &'static str {
        match self {
            Self::StaleWorkerLease => "cs336_a1_stale_worker_lease_v1",
            Self::StaleValidatorLease => "cs336_a1_stale_validator_lease_v1",
            Self::CloseoutObservePayoutWorker => "cs336_a1_closeout_observe_payout_worker_v1",
            Self::CloseoutObservePayoutValidator => "cs336_a1_closeout_observe_payout_validator_v1",
        }
    }

    const fn relative_path(self) -> &'static str {
        match self {
            Self::StaleWorkerLease => {
                "fixtures/proof/4368/stale_worker_runtime_state.template.json"
            }
            Self::StaleValidatorLease => {
                "fixtures/proof/4368/stale_validator_runtime_state.template.json"
            }
            Self::CloseoutObservePayoutWorker => {
                "fixtures/proof/4368/closeout_observe_payout_worker_runtime_state.template.json"
            }
            Self::CloseoutObservePayoutValidator => {
                "fixtures/proof/4368/closeout_observe_payout_validator_runtime_state.template.json"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProofFleetNodeRole {
    Worker,
    Validator,
}

impl ProofFleetNodeRole {
    const fn label(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Validator => "validator",
        }
    }

    const fn role_claim(self) -> super::PylonTrainingRoleClaim {
        match self {
            Self::Worker => super::PylonTrainingRoleClaim::Worker,
            Self::Validator => super::PylonTrainingRoleClaim::Validator,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofProcessRecord {
    binary: String,
    pid: Option<u32>,
    log_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofPersistedPaths {
    namespace_root: String,
    authority_env_path: String,
    relay_data_dir: String,
    receipt_log_path: String,
    kernel_state_path: String,
    treasury_state_path: String,
    treasury_wallet_dir: String,
    treasury_wallet_mnemonic_path: String,
    training_trn_identity_path: String,
    signer_credentials_path: String,
    artifact_store_root: String,
    artifact_trace_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthoritySurfaceUrls {
    authority_base_url: String,
    artifact_store_base_url: String,
    relay_ws_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofNamespacePorts {
    relay_http: u16,
    relay_upstream: u16,
    control_http: u16,
    artifact_store: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofNodePorts {
    admin: u16,
    checkpoint: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofArtifactSmokeReport {
    artifact_id: String,
    relative_object_path: String,
    expected_digest: String,
    payload_size_bytes: u64,
    trace_entry_count: usize,
    verified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthorityRuntimeState {
    schema_version: u32,
    namespace: String,
    mode: ProofAuthorityMode,
    started_at_ms: i64,
    admin_bearer_token: String,
    treasury_enabled: bool,
    ports: ProofNamespacePorts,
    paths: ProofPersistedPaths,
    urls: ProofAuthoritySurfaceUrls,
    authority_process: ProofProcessRecord,
    artifact_store_process: ProofProcessRecord,
    last_artifact_smoke: Option<ProofArtifactSmokeReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofFleetNodeRuntimeRecord {
    role: ProofFleetNodeRole,
    index: usize,
    node_label: String,
    payout_destination: String,
    home_dir: String,
    config_path: String,
    run_root: String,
    admin_url: String,
    checkpoint_serve_url: String,
    ports: ProofNodePorts,
    stale_retained_state_injected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retained_state_fixture_id: Option<String>,
    process: ProofProcessRecord,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofFleetRuntimeState {
    schema_version: u32,
    namespace: String,
    mode: ProofAuthorityMode,
    started_at_ms: i64,
    authority_started_at_ms: i64,
    authority_base_url: String,
    authority_relay_ws_url: Option<String>,
    network_id: String,
    run_slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    psionic_repo_root: Option<String>,
    nodes: Vec<ProofFleetNodeRuntimeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launched_run: Option<ProofRunLaunchResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofProcessStatus {
    binary: String,
    pid: Option<u32>,
    running: bool,
    log_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofRouteProbe {
    route_id: String,
    url: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofAuthorityStatusReport {
    configured: bool,
    namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<ProofAuthorityMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started_at_ms: Option<i64>,
    admin_auth_configured: bool,
    treasury_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ports: Option<ProofNamespacePorts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paths: Option<ProofPersistedPaths>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    urls: Option<ProofAuthoritySurfaceUrls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority_process: Option<ProofProcessStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_store_process: Option<ProofProcessStatus>,
    probes: Vec<ProofRouteProbe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_smoke: Option<ProofArtifactSmokeReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofFleetPaths {
    namespace_root: String,
    fleet_root: String,
    fleet_state_path: String,
    run_report_path: String,
    trace_path: String,
    summary_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofFleetNodeTrainingStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_runtime_process_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_failure_reason: Option<String>,
    recent_issue_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_issue_reason: Option<String>,
    pending_closeout_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    load_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofFleetNodeStatus {
    role: ProofFleetNodeRole,
    index: usize,
    node_label: String,
    payout_destination: String,
    home_dir: String,
    config_path: String,
    run_root: String,
    admin_url: String,
    checkpoint_serve_url: String,
    stale_retained_state_injected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retained_state_fixture_id: Option<String>,
    process: ProofProcessStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    training: Option<ProofFleetNodeTrainingStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofReplacementContributionTemplate {
    submission_receipt_digest: String,
    manifest_digest: String,
    object_digest: String,
    artifact_receipt_digest: String,
    provenance_bundle_digest: String,
    security_receipt_digest: String,
    validator_receipt_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    replay_receipt_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    validation_reason_codes: Vec<super::ComputeAdapterContributionValidationReasonCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    validator_disposition: Option<super::ComputeAdapterContributionDisposition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregation_eligibility: Option<super::ComputeAdapterAggregationEligibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local_step_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumed_token_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumed_example_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregation_weight_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregation_weight_value: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregation_weight_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    promotion_receipt_digest: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    metadata: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    held_out_average_score_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    benchmark_pass_rate_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    runtime_smoke_passed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregated_delta_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_aggregate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    promoted_checkpoint_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofFleetStatusReport {
    configured: bool,
    namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<ProofAuthorityMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    network_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    run_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paths: Option<ProofFleetPaths>,
    authority: ProofAuthorityStatusReport,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<ProofFleetNodeStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launched_run: Option<ProofRunLaunchResponse>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofStatsSnapshot {
    #[serde(default)]
    pylons_online_now: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
struct ProofObservedRunState {
    #[serde(default)]
    training_run_id: String,
    #[serde(default)]
    run_status: String,
    #[serde(default)]
    current_window_id: String,
    #[serde(default)]
    active_window_count: u64,
    #[serde(default)]
    pending_validation_window_count: u64,
    #[serde(default)]
    validator_challenges_open: u64,
    #[serde(default)]
    validator_challenges_queued: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_closeout_status: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofObservedWindowState {
    #[serde(default)]
    window_id: String,
    #[serde(default)]
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closeout_status: Option<String>,
    #[serde(default)]
    accepted_contributions: u32,
    #[serde(default)]
    validator_challenges_open: u64,
    #[serde(default)]
    validator_challenges_queued: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthorityTrainingRunDetailResponse {
    #[serde(default)]
    training_run_id: String,
    #[serde(default)]
    run: ProofObservedRunState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    windows: Vec<ProofObservedWindowState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    contributions: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    nodes: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    caveats: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofObservedTrainingRunDetail {
    training_run_id: String,
    run: ProofObservedRunState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    windows: Vec<ProofObservedWindowState>,
    contribution_count: usize,
    node_count: usize,
    caveat_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_caveat_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_caveat_severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_caveat_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_caveat_detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofRunLaunchResponse {
    launched_at_unix_ms: u64,
    launch_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launch_phase: Option<String>,
    training_run_id: String,
    lane_id: String,
    training_policy_ref: String,
    environment_ref: String,
    network_id: String,
    worker_target_count: u32,
    run_detail: ProofAuthorityTrainingRunDetailResponse,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalCounterMapping {
    public_label: String,
    internal_source_of_truth: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalCanonicalCounters {
    training_admitted_contributors: u64,
    training_assigned_contributors: u64,
    training_accepted_contributors: u64,
    training_model_progress_contributors: u64,
    training_weak_device_assigned_contributors: u64,
    training_weak_device_accepted_contributors: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalParticipantWorkProjection {
    participant_id: String,
    provider_id: String,
    assignment_id: String,
    work_unit_kind: String,
    work_class: String,
    progress_class: String,
    weak_device: bool,
    accepted: bool,
    artifact_kind: String,
    artifact_class: String,
    support_or_verifier_work: bool,
    model_progress_work: bool,
    enters_promoted_checkpoint_lineage: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalArtifactProjection {
    participant_id: String,
    assignment_id: String,
    direction: String,
    artifact_id: String,
    artifact_kind: String,
    artifact_class: String,
    signed_access_mode: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalCheckpointLineage {
    base_checkpoint_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    local_update_artifact_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aggregated_delta_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    accepted_aggregate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    promoted_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    validation_loss_before_bps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    validation_loss_after_bps: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalPayoutProjection {
    accepted_work_payout_count: u64,
    support_work_payout_count: u64,
    model_progress_payout_count: u64,
    total_projected_sats: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalPublicStatsProjection {
    run_id: String,
    training_accepted_contributors: u64,
    training_model_progress_contributors: u64,
    training_weak_device_accepted_contributors: u64,
    public_participant_label: String,
    public_model_progress_label: String,
    public_checkpoint_lineage_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofA1MinimalLaunchProjection {
    launch: String,
    run_id: String,
    run_definition_ref: String,
    tokenizer_digest: String,
    tokenized_dataset_digest: String,
    validation_set_digest: String,
    claim_warning: String,
    public_claim_copy: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    counter_mappings: Vec<ProofA1MinimalCounterMapping>,
    canonical_counters: ProofA1MinimalCanonicalCounters,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    participants: Vec<ProofA1MinimalParticipantWorkProjection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    artifacts: Vec<ProofA1MinimalArtifactProjection>,
    payout_projection: ProofA1MinimalPayoutProjection,
    public_stats_projection: ProofA1MinimalPublicStatsProjection,
    checkpoint_lineage: ProofA1MinimalCheckpointLineage,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofRunReport {
    namespace: String,
    lane: String,
    generated_at_ms: i64,
    timeout_seconds: u64,
    status: String,
    detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blocker_id: Option<String>,
    fleet: ProofFleetStatusReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launch: Option<ProofRunLaunchResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_run: Option<ProofObservedTrainingRunDetail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_failed_authority_write: Option<ProofAuthorityWriteFailureCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a1_minimal_projection: Option<ProofA1MinimalLaunchProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthorityWriteFailureCapture {
    source: String,
    observed_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_body: Option<String>,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofArtifactTraceEntry {
    recorded_at_ms: i64,
    operation: String,
    bucket: String,
    object_path: String,
    canonical_object_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload_size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofArtifactTraceSnapshot {
    trace_path: String,
    entry_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recent_entries: Vec<ProofArtifactTraceEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofRelayTransportView {
    relay_ws_url: Option<String>,
    relay_data_dir: Option<String>,
    authority_running: bool,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofNodeTransportView {
    role: ProofFleetNodeRole,
    index: usize,
    node_label: String,
    admin: ProofRouteProbe,
    checkpoint: ProofRouteProbe,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofTransportSplitView {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authority_front_door: Vec<ProofRouteProbe>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    artifact_store: Vec<ProofRouteProbe>,
    relay: ProofRelayTransportView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    node_surfaces: Vec<ProofNodeTransportView>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofNodeEligibilitySnapshot {
    eligibility: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hard_gate_reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofTraceNode {
    role: ProofFleetNodeRole,
    index: usize,
    node_label: String,
    payout_destination: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retained_state_fixture_id: Option<String>,
    eligibility: ProofNodeEligibilitySnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    training_status: Option<super::TrainingOperatorStatusReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    training_status_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofTraceArtifact {
    schema_version: String,
    namespace: String,
    lane: String,
    generated_at_ms: i64,
    status: String,
    detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blocker_id: Option<String>,
    fleet: ProofFleetStatusReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launch: Option<ProofRunLaunchResponse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_run: Option<ProofObservedTrainingRunDetail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_failed_authority_write: Option<ProofAuthorityWriteFailureCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a1_minimal_projection: Option<ProofA1MinimalLaunchProjection>,
    artifact_transport: ProofArtifactTraceSnapshot,
    transport: ProofTransportSplitView,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    node_traces: Vec<ProofTraceNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofSummaryArtifact {
    schema_version: String,
    namespace: String,
    lane: String,
    generated_at_ms: i64,
    status: String,
    detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blocker_id: Option<String>,
    first_red_stage: String,
    first_red_subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lease_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    membership_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closeout_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closeout_next_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    closeout_last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_failed_authority_write: Option<ProofAuthorityWriteFailureCapture>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    a1_minimal_projection: Option<ProofA1MinimalLaunchProjection>,
    trace_path: String,
    transport: ProofTransportSplitView,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofProcessEnvExpectation {
    key: String,
    expected_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    process_present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    matches_expected: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofProcessProvenance {
    component_id: String,
    expected_binary_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expected_binary_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    running_binary_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    running_binary_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    binary_matches_expected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    running: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    env_expectations: Vec<ProofProcessEnvExpectation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofAuthorityEnvFileReport {
    path: String,
    key_count: usize,
    keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofGitProvenance {
    workspace_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofDoctorReport {
    configured: bool,
    namespace: String,
    generated_at_ms: i64,
    fleet: ProofFleetStatusReport,
    transport: ProofTransportSplitView,
    artifact_transport: ProofArtifactTraceSnapshot,
    git: ProofGitProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority_env_file: Option<ProofAuthorityEnvFileReport>,
    current_executable: ProofProcessProvenance,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    supporting_binaries: Vec<ProofProcessProvenance>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    process_provenance: Vec<ProofProcessProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_trace_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    latest_summary_path: Option<String>,
}

#[derive(Clone)]
struct ArtifactStoreState {
    store_root: PathBuf,
    trace_path: PathBuf,
}

#[derive(Clone, Debug)]
struct ProofLayout {
    namespace_root: PathBuf,
    authority_env_path: PathBuf,
    fleet_root: PathBuf,
    relay_data_dir: PathBuf,
    receipt_log_path: PathBuf,
    kernel_state_path: PathBuf,
    treasury_state_path: PathBuf,
    treasury_wallet_dir: PathBuf,
    treasury_wallet_mnemonic_path: PathBuf,
    training_trn_identity_path: PathBuf,
    signer_credentials_path: PathBuf,
    artifact_store_root: PathBuf,
    artifact_trace_path: PathBuf,
    runtime_state_path: PathBuf,
    fleet_state_path: PathBuf,
    run_report_path: PathBuf,
    trace_path: PathBuf,
    summary_path: PathBuf,
    authority_log_path: PathBuf,
    artifact_store_log_path: PathBuf,
}

pub fn parse_proof_command(args: &[String], start_index: usize) -> Result<ProofCommand> {
    match args.get(start_index).map(String::as_str) {
        Some("authority") => Ok(ProofCommand::Authority {
            command: parse_proof_authority_command(args, start_index + 1)?,
        }),
        Some("fleet") => Ok(ProofCommand::Fleet {
            command: parse_proof_fleet_command(args, start_index + 1)?,
        }),
        Some("run") => Ok(ProofCommand::Run {
            command: parse_proof_run_command(args, start_index + 1)?,
        }),
        Some("doctor") => Ok(ProofCommand::Doctor {
            command: parse_proof_doctor_command(args, start_index + 1)?,
        }),
        Some("internal") => Ok(ProofCommand::Internal {
            command: parse_proof_internal_command(args, start_index + 1)?,
        }),
        Some(other) => bail!("unknown proof command: {other}"),
        None => bail!("missing proof subcommand"),
    }
}

pub async fn run_proof_command(
    config_path: &Path,
    command: ProofCommand,
) -> Result<Option<String>> {
    match command {
        ProofCommand::Authority { command } => {
            let report = match &command {
                ProofAuthorityCommand::Up {
                    namespace, mode, ..
                } => ensure_proof_authority_up(config_path, namespace.as_str(), *mode).await?,
                ProofAuthorityCommand::Status { namespace, .. } => {
                    collect_proof_status(config_path, namespace.as_str()).await?
                }
                ProofAuthorityCommand::Down { namespace, .. } => {
                    let _ = stop_proof_authority(config_path, namespace.as_str(), false).await?;
                    collect_proof_status(config_path, namespace.as_str()).await?
                }
                ProofAuthorityCommand::Reset { namespace, .. } => {
                    let _ = stop_proof_authority(config_path, namespace.as_str(), true).await?;
                    collect_proof_status(config_path, namespace.as_str()).await?
                }
            };
            let json = matches!(
                command,
                ProofAuthorityCommand::Up { json: true, .. }
                    | ProofAuthorityCommand::Status { json: true, .. }
                    | ProofAuthorityCommand::Down { json: true, .. }
                    | ProofAuthorityCommand::Reset { json: true, .. }
            );
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_proof_status_report(&report)))
        }
        ProofCommand::Fleet { command } => {
            let report = match &command {
                ProofFleetCommand::Up {
                    namespace,
                    mode,
                    workers,
                    validators,
                    network_id,
                    stale_worker_state,
                    stale_validator_state,
                    ..
                } => {
                    ensure_proof_fleet_up(
                        config_path,
                        namespace.as_str(),
                        *mode,
                        *workers,
                        *validators,
                        network_id.as_deref(),
                        *stale_worker_state,
                        *stale_validator_state,
                        None,
                        None,
                    )
                    .await?
                }
                ProofFleetCommand::Status { namespace, .. } => {
                    collect_proof_fleet_status(config_path, namespace.as_str()).await?
                }
                ProofFleetCommand::Down { namespace, .. } => {
                    let _ = stop_proof_fleet(config_path, namespace.as_str(), false).await?;
                    collect_proof_fleet_status(config_path, namespace.as_str()).await?
                }
                ProofFleetCommand::Reset { namespace, .. } => {
                    let _ = stop_proof_fleet(config_path, namespace.as_str(), true).await?;
                    collect_proof_fleet_status(config_path, namespace.as_str()).await?
                }
            };
            let json = matches!(
                command,
                ProofFleetCommand::Up { json: true, .. }
                    | ProofFleetCommand::Status { json: true, .. }
                    | ProofFleetCommand::Down { json: true, .. }
                    | ProofFleetCommand::Reset { json: true, .. }
            );
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_proof_fleet_status_report(&report)))
        }
        ProofCommand::Run { command } => {
            let report = run_proof_lane(config_path, &command).await?;
            if command.json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_proof_run_report(&report)))
        }
        ProofCommand::Doctor { command } => {
            let report =
                collect_proof_doctor_report(config_path, command.namespace.as_str()).await?;
            if command.json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_proof_doctor_report(&report)))
        }
        ProofCommand::Internal { command } => match command {
            ProofInternalCommand::ArtifactStoreServe {
                listen_addr,
                store_root,
                trace_path,
            } => {
                run_artifact_store_server(listen_addr, store_root, trace_path).await?;
                Ok(None)
            }
        },
    }
}

fn parse_proof_authority_command(
    args: &[String],
    start_index: usize,
) -> Result<ProofAuthorityCommand> {
    match args.get(start_index).map(String::as_str) {
        Some("up") => {
            let mut namespace = DEFAULT_PROOF_NAMESPACE.to_string();
            let mut json = false;
            let mut mode = ProofAuthorityMode::ProdShaped;
            let mut index = start_index + 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    "--mode" => {
                        let value = args.get(index + 1).ok_or_else(|| {
                            anyhow!("missing value for proof authority up --mode")
                        })?;
                        mode = parse_proof_authority_mode(value.as_str())?;
                        index += 2;
                    }
                    "--namespace" => {
                        namespace = parse_namespace_value(
                            args,
                            index + 1,
                            "proof authority up --namespace",
                        )?;
                        index += 2;
                    }
                    other => bail!("unexpected argument for proof authority up: {other}"),
                }
            }
            Ok(ProofAuthorityCommand::Up {
                namespace,
                mode,
                json,
            })
        }
        Some("status") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof authority status",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofAuthorityCommand::Status { namespace, json })
        }
        Some("down") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof authority down",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofAuthorityCommand::Down { namespace, json })
        }
        Some("reset") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof authority reset",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofAuthorityCommand::Reset { namespace, json })
        }
        Some(other) => bail!("unknown proof authority command: {other}"),
        None => bail!("missing proof authority command"),
    }
}

fn parse_proof_fleet_command(args: &[String], start_index: usize) -> Result<ProofFleetCommand> {
    match args.get(start_index).map(String::as_str) {
        Some("up") => {
            let mut namespace = DEFAULT_PROOF_NAMESPACE.to_string();
            let mut json = false;
            let mut mode = ProofAuthorityMode::ProdShaped;
            let mut workers = 1usize;
            let mut validators = 1usize;
            let mut network_id = None::<String>;
            let mut stale_worker_state = false;
            let mut stale_validator_state = false;
            let mut index = start_index + 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    "--mode" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for proof fleet up --mode"))?;
                        mode = parse_proof_authority_mode(value.as_str())?;
                        index += 2;
                    }
                    "--namespace" => {
                        namespace =
                            parse_namespace_value(args, index + 1, "proof fleet up --namespace")?;
                        index += 2;
                    }
                    "--workers" => {
                        workers = parse_usize_value(args, index + 1, "proof fleet up --workers")?;
                        index += 2;
                    }
                    "--validators" => {
                        validators =
                            parse_usize_value(args, index + 1, "proof fleet up --validators")?;
                        index += 2;
                    }
                    "--network-id" => {
                        network_id = Some(parse_nonempty_string_value(
                            args,
                            index + 1,
                            "proof fleet up --network-id",
                        )?);
                        index += 2;
                    }
                    "--stale-worker-state" => {
                        stale_worker_state = true;
                        index += 1;
                    }
                    "--stale-validator-state" => {
                        stale_validator_state = true;
                        index += 1;
                    }
                    other => bail!("unexpected argument for proof fleet up: {other}"),
                }
            }
            ensure!(workers > 0, "proof fleet up requires at least one worker");
            ensure!(
                validators > 0,
                "proof fleet up requires at least one validator"
            );
            Ok(ProofFleetCommand::Up {
                namespace,
                mode,
                workers,
                validators,
                network_id,
                stale_worker_state,
                stale_validator_state,
                json,
            })
        }
        Some("status") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof fleet status",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofFleetCommand::Status { namespace, json })
        }
        Some("down") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof fleet down",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofFleetCommand::Down { namespace, json })
        }
        Some("reset") => {
            let (namespace, json) = parse_namespace_and_json(
                args,
                start_index + 1,
                "proof fleet reset",
                DEFAULT_PROOF_NAMESPACE,
            )?;
            Ok(ProofFleetCommand::Reset { namespace, json })
        }
        Some(other) => bail!("unknown proof fleet command: {other}"),
        None => bail!("missing proof fleet command"),
    }
}

fn parse_proof_run_command(args: &[String], start_index: usize) -> Result<ProofRunCommand> {
    let lane = parse_proof_lane(
        args.get(start_index)
            .ok_or_else(|| anyhow!("missing proof run lane"))?,
    )?;
    let mut namespace = None::<String>;
    let mut json = false;
    let mut mode = ProofAuthorityMode::ProdShaped;
    let mut workers = lane.default_workers();
    let mut validators = lane.default_validators();
    let mut timeout_seconds = 45u64;
    let mut stale_worker_state = false;
    let mut stale_validator_state = false;
    let mut index = start_index + 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--mode" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow!("missing value for proof run --mode"))?;
                mode = parse_proof_authority_mode(value.as_str())?;
                index += 2;
            }
            "--namespace" => {
                namespace = Some(parse_namespace_value(
                    args,
                    index + 1,
                    "proof run --namespace",
                )?);
                index += 2;
            }
            "--workers" => {
                workers = parse_usize_value(args, index + 1, "proof run --workers")?;
                index += 2;
            }
            "--validators" => {
                validators = parse_usize_value(args, index + 1, "proof run --validators")?;
                index += 2;
            }
            "--timeout-seconds" => {
                timeout_seconds = parse_u64_value(args, index + 1, "proof run --timeout-seconds")?;
                index += 2;
            }
            "--stale-worker-state" => {
                stale_worker_state = true;
                index += 1;
            }
            "--stale-validator-state" => {
                stale_validator_state = true;
                index += 1;
            }
            other => bail!(
                "unexpected argument for proof run {}: {other}",
                lane.label()
            ),
        }
    }
    ensure!(
        workers >= lane.minimum_workers(),
        "proof run {} requires at least {} worker(s)",
        lane.label(),
        lane.minimum_workers()
    );
    ensure!(
        validators >= lane.minimum_validators(),
        "proof run {} requires at least {} validator(s)",
        lane.label(),
        lane.minimum_validators()
    );
    Ok(ProofRunCommand {
        lane,
        namespace,
        mode,
        workers,
        validators,
        timeout_seconds,
        stale_worker_state,
        stale_validator_state,
        json,
    })
}

fn parse_proof_doctor_command(args: &[String], start_index: usize) -> Result<ProofDoctorCommand> {
    let (namespace, json) =
        parse_namespace_and_json(args, start_index, "proof doctor", DEFAULT_PROOF_NAMESPACE)?;
    Ok(ProofDoctorCommand { namespace, json })
}

fn parse_proof_internal_command(
    args: &[String],
    start_index: usize,
) -> Result<ProofInternalCommand> {
    match (
        args.get(start_index).map(String::as_str),
        args.get(start_index + 1).map(String::as_str),
    ) {
        (Some("artifact-store"), Some("serve")) => {
            let mut listen_addr = None;
            let mut store_root = None;
            let mut trace_path = None;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--listen-addr" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --listen-addr"))?;
                        listen_addr = Some(
                            value
                                .parse::<SocketAddr>()
                                .with_context(|| format!("invalid proof listen addr `{value}`"))?,
                        );
                        index += 2;
                    }
                    "--store-root" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --store-root"))?;
                        store_root = Some(PathBuf::from(value));
                        index += 2;
                    }
                    "--trace-path" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --trace-path"))?;
                        trace_path = Some(PathBuf::from(value));
                        index += 2;
                    }
                    other => bail!(
                        "unexpected argument for proof internal artifact-store serve: {other}"
                    ),
                }
            }
            Ok(ProofInternalCommand::ArtifactStoreServe {
                listen_addr: listen_addr.ok_or_else(|| {
                    anyhow!("proof internal artifact-store serve requires --listen-addr")
                })?,
                store_root: store_root.ok_or_else(|| {
                    anyhow!("proof internal artifact-store serve requires --store-root")
                })?,
                trace_path: trace_path.ok_or_else(|| {
                    anyhow!("proof internal artifact-store serve requires --trace-path")
                })?,
            })
        }
        (Some(other), _) => bail!("unknown proof internal command: {other}"),
        _ => bail!("missing proof internal command"),
    }
}

fn parse_namespace_and_json(
    args: &[String],
    mut index: usize,
    context: &str,
    default_namespace: &str,
) -> Result<(String, bool)> {
    let mut namespace = default_namespace.to_string();
    let mut json = false;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => {
                json = true;
                index += 1;
            }
            "--namespace" => {
                namespace = parse_namespace_value(args, index + 1, context)?;
                index += 2;
            }
            other => bail!("unexpected argument for {context}: {other}"),
        }
    }
    Ok((namespace, json))
}

fn parse_namespace_value(args: &[String], value_index: usize, context: &str) -> Result<String> {
    let value = parse_nonempty_string_value(args, value_index, context)?;
    Ok(value)
}

fn parse_nonempty_string_value(
    args: &[String],
    value_index: usize,
    context: &str,
) -> Result<String> {
    let value = args
        .get(value_index)
        .ok_or_else(|| anyhow!("missing value for {context}"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{context} must not be empty");
    }
    Ok(trimmed.to_string())
}

fn parse_usize_value(args: &[String], value_index: usize, context: &str) -> Result<usize> {
    let value = args
        .get(value_index)
        .ok_or_else(|| anyhow!("missing value for {context}"))?;
    value
        .parse::<usize>()
        .with_context(|| format!("invalid integer for {context}: {value}"))
}

fn parse_u64_value(args: &[String], value_index: usize, context: &str) -> Result<u64> {
    let value = args
        .get(value_index)
        .ok_or_else(|| anyhow!("missing value for {context}"))?;
    value
        .parse::<u64>()
        .with_context(|| format!("invalid integer for {context}: {value}"))
}

fn parse_proof_authority_mode(value: &str) -> Result<ProofAuthorityMode> {
    match value {
        "prod-shaped" | "prod_shaped" => Ok(ProofAuthorityMode::ProdShaped),
        "debug-authority" | "debug_authority" => Ok(ProofAuthorityMode::DebugAuthority),
        other => bail!("unknown proof authority mode: {other}"),
    }
}

fn parse_proof_lane(value: &str) -> Result<ProofLane> {
    match value {
        "cs336-a1" | "cs336_a1" | "cs336/a1" => Ok(ProofLane::Cs336A1),
        "cs336-a1-hosted-starter" | "cs336_a1_hosted_starter" | "cs336/a1/hosted-starter" => {
            Ok(ProofLane::Cs336A1HostedStarter)
        }
        "cs336-a1-stale-recovery" | "cs336_a1_stale_recovery" | "cs336/a1/stale-recovery" => {
            Ok(ProofLane::Cs336A1StaleRecovery)
        }
        "cs336-a1-replacement-attempt"
        | "cs336_a1_replacement_attempt"
        | "cs336/a1/replacement-attempt" => Ok(ProofLane::Cs336A1ReplacementAttempt),
        "a1-minimal-distributed-lm-launch-a"
        | "a1_minimal_distributed_lm_launch_a"
        | "a1/minimal-distributed-lm/launch-a"
        | "a1-minimal-launch-a"
        | "a1_minimal_launch_a" => Ok(ProofLane::A1MinimalDistributedLmLaunchA),
        "a1-minimal-distributed-lm-launch-b"
        | "a1_minimal_distributed_lm_launch_b"
        | "a1/minimal-distributed-lm/launch-b"
        | "a1-minimal-launch-b"
        | "a1_minimal_launch_b" => Ok(ProofLane::A1MinimalDistributedLmLaunchB),
        other => bail!("unknown proof lane: {other}"),
    }
}

async fn ensure_proof_authority_up(
    config_path: &Path,
    namespace: &str,
    mode: ProofAuthorityMode,
) -> Result<ProofAuthorityStatusReport> {
    let layout = proof_layout(config_path, namespace);
    if let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? {
        let authority_running = process_is_running(&state.authority_process);
        let artifact_running = process_is_running(&state.artifact_store_process);
        if authority_running
            && artifact_running
            && state.mode == mode
            && state.namespace == namespace
        {
            return collect_proof_status(config_path, namespace).await;
        }
        let _ = stop_runtime_processes(&state).await;
    }

    ensure_layout_dirs(&layout)?;
    write_signer_credentials(layout.signer_credentials_path.as_path())?;

    let ports = proof_namespace_ports(namespace);
    let admin_bearer_token = format!("proof_admin_{}", super::random_token());
    let artifact_store_base_url = format!(
        "http://127.0.0.1:{}{}",
        ports.artifact_store, PROOF_ARTIFACT_UPLOAD_PREFIX
    );
    let authority_base_url = match mode {
        ProofAuthorityMode::ProdShaped => format!("http://127.0.0.1:{}", ports.relay_http),
        ProofAuthorityMode::DebugAuthority => format!("http://127.0.0.1:{}", ports.control_http),
    };
    let urls = ProofAuthoritySurfaceUrls {
        authority_base_url: authority_base_url.clone(),
        artifact_store_base_url: artifact_store_base_url.clone(),
        relay_ws_url: (mode == ProofAuthorityMode::ProdShaped)
            .then(|| format!("ws://127.0.0.1:{}/", ports.relay_http)),
    };
    let paths = ProofPersistedPaths {
        namespace_root: layout.namespace_root.display().to_string(),
        authority_env_path: layout.authority_env_path.display().to_string(),
        relay_data_dir: layout.relay_data_dir.display().to_string(),
        receipt_log_path: layout.receipt_log_path.display().to_string(),
        kernel_state_path: layout.kernel_state_path.display().to_string(),
        treasury_state_path: layout.treasury_state_path.display().to_string(),
        treasury_wallet_dir: layout.treasury_wallet_dir.display().to_string(),
        treasury_wallet_mnemonic_path: layout.treasury_wallet_mnemonic_path.display().to_string(),
        training_trn_identity_path: layout.training_trn_identity_path.display().to_string(),
        signer_credentials_path: layout.signer_credentials_path.display().to_string(),
        artifact_store_root: layout.artifact_store_root.display().to_string(),
        artifact_trace_path: layout.artifact_trace_path.display().to_string(),
    };

    let current_exe = current_executable_path()?;
    let artifact_args = vec![
        "proof".to_string(),
        "internal".to_string(),
        "artifact-store".to_string(),
        "serve".to_string(),
        "--listen-addr".to_string(),
        format!("127.0.0.1:{}", ports.artifact_store),
        "--store-root".to_string(),
        layout.artifact_store_root.display().to_string(),
        "--trace-path".to_string(),
        layout.artifact_trace_path.display().to_string(),
    ];
    let artifact_pid = spawn_logged_process(
        current_exe.as_path(),
        artifact_args.as_slice(),
        &[],
        layout.artifact_store_log_path.as_path(),
    )?;
    wait_for_route(
        format!("http://127.0.0.1:{}/healthz", ports.artifact_store).as_str(),
        &[StatusCode::OK],
    )
    .await?;

    let authority_binary =
        resolve_workspace_binary(mode.authority_binary(), mode.authority_package())?;
    let authority_env = authority_environment(
        mode,
        &ports,
        &layout,
        admin_bearer_token.as_str(),
        urls.relay_ws_url.as_deref(),
        artifact_store_base_url.as_str(),
    );
    write_env_manifest(layout.authority_env_path.as_path(), &authority_env)?;
    let authority_pid = spawn_logged_process(
        authority_binary.as_path(),
        &[],
        authority_env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>()
            .as_slice(),
        layout.authority_log_path.as_path(),
    )?;
    wait_for_route(
        format!("{authority_base_url}/healthz").as_str(),
        &[StatusCode::OK],
    )
    .await?;
    wait_for_proof_treasury_ready(authority_base_url.as_str()).await?;

    let mut state = ProofAuthorityRuntimeState {
        schema_version: PROOF_RUNTIME_SCHEMA_VERSION,
        namespace: namespace.to_string(),
        mode,
        started_at_ms: super::now_epoch_ms(),
        admin_bearer_token,
        treasury_enabled: true,
        ports,
        paths,
        urls,
        authority_process: ProofProcessRecord {
            binary: authority_binary.display().to_string(),
            pid: Some(authority_pid),
            log_path: layout.authority_log_path.display().to_string(),
        },
        artifact_store_process: ProofProcessRecord {
            binary: current_exe.display().to_string(),
            pid: Some(artifact_pid),
            log_path: layout.artifact_store_log_path.display().to_string(),
        },
        last_artifact_smoke: None,
    };
    let artifact_smoke = run_artifact_smoke(&state).await?;
    state.last_artifact_smoke = Some(artifact_smoke);
    save_runtime_state(layout.runtime_state_path.as_path(), &state)?;
    collect_proof_status(config_path, namespace).await
}

async fn collect_proof_status(
    config_path: &Path,
    namespace: &str,
) -> Result<ProofAuthorityStatusReport> {
    let layout = proof_layout(config_path, namespace);
    let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? else {
        return Ok(ProofAuthorityStatusReport {
            configured: false,
            namespace: namespace.to_string(),
            mode: None,
            started_at_ms: None,
            admin_auth_configured: false,
            treasury_enabled: false,
            ports: None,
            paths: None,
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        });
    };

    let authority_process = ProofProcessStatus {
        binary: state.authority_process.binary.clone(),
        pid: state.authority_process.pid,
        running: process_is_running(&state.authority_process),
        log_path: state.authority_process.log_path.clone(),
    };
    let artifact_store_process = ProofProcessStatus {
        binary: state.artifact_store_process.binary.clone(),
        pid: state.artifact_store_process.pid,
        running: process_is_running(&state.artifact_store_process),
        log_path: state.artifact_store_process.log_path.clone(),
    };
    let probes = collect_route_probes(&state).await;

    Ok(ProofAuthorityStatusReport {
        configured: true,
        namespace: state.namespace,
        mode: Some(state.mode),
        started_at_ms: Some(state.started_at_ms),
        admin_auth_configured: !state.admin_bearer_token.is_empty(),
        treasury_enabled: state.treasury_enabled,
        ports: Some(state.ports),
        paths: Some(state.paths),
        urls: Some(state.urls),
        authority_process: Some(authority_process),
        artifact_store_process: Some(artifact_store_process),
        probes,
        artifact_smoke: state.last_artifact_smoke,
    })
}

async fn stop_proof_authority(config_path: &Path, namespace: &str, reset: bool) -> Result<bool> {
    let layout = proof_layout(config_path, namespace);
    let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? else {
        return Ok(false);
    };
    stop_runtime_processes(&state).await?;
    if reset {
        if layout.namespace_root.exists() {
            fs::remove_dir_all(layout.namespace_root.as_path()).with_context(|| {
                format!(
                    "failed to remove proof namespace {}",
                    layout.namespace_root.display()
                )
            })?;
        }
    } else {
        let mut stopped = state;
        stopped.authority_process.pid = None;
        stopped.artifact_store_process.pid = None;
        save_runtime_state(layout.runtime_state_path.as_path(), &stopped)?;
    }
    Ok(true)
}

async fn ensure_proof_fleet_up(
    config_path: &Path,
    namespace: &str,
    mode: ProofAuthorityMode,
    workers: usize,
    validators: usize,
    network_id_override: Option<&str>,
    stale_worker_state: bool,
    stale_validator_state: bool,
    worker_fixture: Option<ProofNodeRuntimeFixture>,
    validator_fixture: Option<ProofNodeRuntimeFixture>,
) -> Result<ProofFleetStatusReport> {
    let authority = ensure_proof_authority_up(config_path, namespace, mode).await?;
    let layout = proof_layout(config_path, namespace);
    let authority_state =
        load_runtime_state(layout.runtime_state_path.as_path())?.ok_or_else(|| {
            anyhow!("proof authority runtime state missing for namespace {namespace}")
        })?;
    let network_id = network_id_override
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| proof_fleet_network_id(namespace));
    let run_slug = proof_fleet_run_slug(namespace);
    if let Some(existing) = load_fleet_state(layout.fleet_state_path.as_path())? {
        let existing_workers = existing
            .nodes
            .iter()
            .filter(|node| node.role == ProofFleetNodeRole::Worker)
            .count();
        let existing_validators = existing
            .nodes
            .iter()
            .filter(|node| node.role == ProofFleetNodeRole::Validator)
            .count();
        let all_running = existing
            .nodes
            .iter()
            .all(|node| process_is_running(&node.process));
        if existing.mode == mode
            && existing.authority_started_at_ms == authority_state.started_at_ms
            && existing.network_id == network_id
            && existing_workers == workers
            && existing_validators == validators
            && all_running
        {
            return collect_proof_fleet_status(config_path, namespace).await;
        }
        let _ = stop_proof_fleet(config_path, namespace, true).await?;
    }

    ensure_layout_dirs(&layout)?;
    fs::create_dir_all(layout.fleet_root.as_path()).with_context(|| {
        format!(
            "failed to create proof fleet root {}",
            layout.fleet_root.display()
        )
    })?;

    let current_exe = current_executable_path()?;
    let psionic_repo_root = proof_psionic_repo_root();
    let mut used_ports = BTreeSet::from([
        authority_state.ports.relay_http,
        authority_state.ports.relay_upstream,
        authority_state.ports.control_http,
        authority_state.ports.artifact_store,
    ]);
    let mut nodes = Vec::new();
    for index in 1..=workers {
        nodes.push(
            spawn_proof_fleet_node(
                namespace,
                &layout,
                &authority_state,
                ProofFleetNodeRole::Worker,
                index,
                network_id.as_str(),
                stale_worker_state,
                worker_fixture,
                current_exe.as_path(),
                psionic_repo_root.as_deref(),
                &mut used_ports,
            )
            .await?,
        );
    }
    for index in 1..=validators {
        nodes.push(
            spawn_proof_fleet_node(
                namespace,
                &layout,
                &authority_state,
                ProofFleetNodeRole::Validator,
                index,
                network_id.as_str(),
                stale_validator_state,
                validator_fixture,
                current_exe.as_path(),
                psionic_repo_root.as_deref(),
                &mut used_ports,
            )
            .await?,
        );
    }
    let state = ProofFleetRuntimeState {
        schema_version: PROOF_RUNTIME_SCHEMA_VERSION,
        namespace: namespace.to_string(),
        mode,
        started_at_ms: super::now_epoch_ms(),
        authority_started_at_ms: authority_state.started_at_ms,
        authority_base_url: authority_state.urls.authority_base_url.clone(),
        authority_relay_ws_url: authority_state.urls.relay_ws_url.clone(),
        network_id,
        run_slug,
        psionic_repo_root: psionic_repo_root.map(|value| value.display().to_string()),
        nodes,
        launched_run: None,
    };
    save_fleet_state(layout.fleet_state_path.as_path(), &state)?;
    let expected_nodes = u64::try_from(state.nodes.len()).unwrap_or(u64::MAX);
    let _ = wait_for_proof_pylons_online(
        authority
            .urls
            .as_ref()
            .map(|value| value.authority_base_url.as_str())
            .ok_or_else(|| anyhow!("proof authority URLs missing"))?,
        expected_nodes,
    )
    .await?;
    collect_proof_fleet_status(config_path, namespace).await
}

async fn collect_proof_fleet_status(
    config_path: &Path,
    namespace: &str,
) -> Result<ProofFleetStatusReport> {
    let layout = proof_layout(config_path, namespace);
    let authority = collect_proof_status(config_path, namespace).await?;
    let Some(state) = load_fleet_state(layout.fleet_state_path.as_path())? else {
        return Ok(ProofFleetStatusReport {
            configured: false,
            namespace: namespace.to_string(),
            mode: None,
            network_id: None,
            run_slug: None,
            paths: None,
            authority,
            nodes: Vec::new(),
            launched_run: None,
        });
    };

    let mut nodes = Vec::with_capacity(state.nodes.len());
    for node in &state.nodes {
        let config_path = PathBuf::from(node.config_path.as_str());
        let training = load_proof_node_training_status(config_path.as_path()).await;
        nodes.push(ProofFleetNodeStatus {
            role: node.role,
            index: node.index,
            node_label: node.node_label.clone(),
            payout_destination: node.payout_destination.clone(),
            home_dir: node.home_dir.clone(),
            config_path: node.config_path.clone(),
            run_root: node.run_root.clone(),
            admin_url: node.admin_url.clone(),
            checkpoint_serve_url: node.checkpoint_serve_url.clone(),
            stale_retained_state_injected: node.stale_retained_state_injected,
            retained_state_fixture_id: node.retained_state_fixture_id.clone(),
            process: ProofProcessStatus {
                binary: node.process.binary.clone(),
                pid: node.process.pid,
                running: process_is_running(&node.process),
                log_path: node.process.log_path.clone(),
            },
            training,
        });
    }

    Ok(ProofFleetStatusReport {
        configured: true,
        namespace: state.namespace,
        mode: Some(state.mode),
        network_id: Some(state.network_id),
        run_slug: Some(state.run_slug),
        paths: Some(ProofFleetPaths {
            namespace_root: layout.namespace_root.display().to_string(),
            fleet_root: layout.fleet_root.display().to_string(),
            fleet_state_path: layout.fleet_state_path.display().to_string(),
            run_report_path: layout.run_report_path.display().to_string(),
            trace_path: layout.trace_path.display().to_string(),
            summary_path: layout.summary_path.display().to_string(),
        }),
        authority,
        nodes,
        launched_run: state.launched_run,
    })
}

async fn stop_proof_fleet(config_path: &Path, namespace: &str, reset: bool) -> Result<bool> {
    let layout = proof_layout(config_path, namespace);
    let Some(state) = load_fleet_state(layout.fleet_state_path.as_path())? else {
        if reset && layout.fleet_root.exists() {
            fs::remove_dir_all(layout.fleet_root.as_path()).with_context(|| {
                format!(
                    "failed to remove proof fleet root {}",
                    layout.fleet_root.display()
                )
            })?;
        }
        return Ok(false);
    };
    for node in &state.nodes {
        if let Some(pid) = node.process.pid {
            stop_pid(pid).await?;
        }
    }
    if reset {
        if layout.fleet_root.exists() {
            fs::remove_dir_all(layout.fleet_root.as_path()).with_context(|| {
                format!(
                    "failed to remove proof fleet root {}",
                    layout.fleet_root.display()
                )
            })?;
        }
    } else {
        let mut stopped = state;
        for node in &mut stopped.nodes {
            node.process.pid = None;
        }
        save_fleet_state(layout.fleet_state_path.as_path(), &stopped)?;
    }
    Ok(true)
}

async fn run_proof_lane(config_path: &Path, command: &ProofRunCommand) -> Result<ProofRunReport> {
    let namespace = command
        .namespace
        .clone()
        .unwrap_or_else(|| generated_proof_namespace(command.lane));
    if command.lane.uses_manual_authority_scenario() {
        return run_manual_replacement_attempt_proof_lane(config_path, command, namespace).await;
    }
    if command.lane.uses_a1_minimal_simulated_scenario() {
        return run_a1_minimal_simulated_proof_lane(config_path, command, namespace).await;
    }
    run_standard_proof_lane(config_path, command, namespace).await
}

async fn run_a1_minimal_simulated_proof_lane(
    config_path: &Path,
    command: &ProofRunCommand,
    namespace: String,
) -> Result<ProofRunReport> {
    let projection = build_a1_minimal_launch_projection(command.lane, namespace.as_str())?;
    validate_a1_minimal_launch_projection(&projection)?;
    let observed_run = a1_minimal_observed_run(&projection);
    let fleet = a1_minimal_simulated_fleet_status(
        config_path,
        namespace.as_str(),
        command.mode,
        &projection,
    );
    let detail = format!(
        "{} local proof projected {} participant(s), {} model-progress participant(s), support_model_progress_mismatch=false",
        projection.launch,
        projection.canonical_counters.training_accepted_contributors,
        projection
            .canonical_counters
            .training_model_progress_contributors
    );
    let report = ProofRunReport {
        namespace,
        lane: command.lane.label().to_string(),
        generated_at_ms: super::now_epoch_ms(),
        timeout_seconds: command.timeout_seconds,
        status: "completed".to_string(),
        detail,
        blocker_id: None,
        fleet,
        launch: None,
        observed_run: Some(observed_run),
        first_failed_authority_write: None,
        a1_minimal_projection: Some(projection),
    };
    persist_proof_run_outputs(config_path, &report).await?;
    Ok(report)
}

fn build_a1_minimal_launch_projection(
    lane: ProofLane,
    namespace: &str,
) -> Result<ProofA1MinimalLaunchProjection> {
    let launch = match lane {
        ProofLane::A1MinimalDistributedLmLaunchA => "launch_a",
        ProofLane::A1MinimalDistributedLmLaunchB => "launch_b",
        _ => bail!("a1 minimal projection requested for non-A1 proof lane"),
    };
    let run_id = "a1_minimal_distributed_lm_001";
    let participants = match lane {
        ProofLane::A1MinimalDistributedLmLaunchA => vec![
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-weak-001",
                "assign.a1.launch_a.support.001",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::ValidationReplay,
                true,
                false,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-weak-002",
                "assign.a1.launch_a.support.002",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::EvaluationBatch,
                true,
                false,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-weak-003",
                "assign.a1.launch_a.support.003",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::CheckpointVerification,
                true,
                false,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-strong-001",
                "assign.a1.launch_a.local_update.001",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::LocalUpdate,
                false,
                true,
            ),
        ],
        ProofLane::A1MinimalDistributedLmLaunchB => vec![
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-strong-001",
                "assign.a1.launch_b.local_update.001",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::LocalUpdate,
                false,
                true,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-strong-002",
                "assign.a1.launch_b.local_update.002",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::LocalUpdate,
                false,
                true,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-strong-003",
                "assign.a1.launch_b.local_update.003",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::LocalUpdate,
                false,
                true,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-strong-004",
                "assign.a1.launch_b.local_update.004",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::LocalUpdate,
                false,
                true,
            ),
            a1_minimal_work_projection(
                run_id,
                "pylon-a1-weak-004",
                "assign.a1.launch_b.support.001",
                openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind::CloseoutVerification,
                true,
                false,
            ),
        ],
        _ => unreachable!(),
    };
    let canonical_counters = derive_a1_minimal_canonical_counters(&participants);
    let artifacts = participants
        .iter()
        .map(|participant| a1_minimal_output_artifact_projection(run_id, participant))
        .collect::<Vec<_>>();
    let local_update_artifact_ids = artifacts
        .iter()
        .filter(|artifact| artifact.artifact_kind == "local_update")
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let checkpoint_lineage = ProofA1MinimalCheckpointLineage {
        base_checkpoint_ref: "base://a1_minimal_distributed_lm/step-000000".to_string(),
        local_update_artifact_ids,
        aggregated_delta_digest: Some(sha256_prefixed_bytes(
            format!("a1-minimal:{launch}:{namespace}:aggregate").as_bytes(),
        )),
        accepted_aggregate_id: Some(format!("aggregate.a1_minimal.{launch}.{namespace}")),
        promoted_checkpoint_ref: Some(format!(
            "checkpoint://psion/a1_minimal_distributed_lm/{run_id}/{launch}/step-000001"
        )),
        validation_loss_before_bps: Some(5200),
        validation_loss_after_bps: Some(if launch == "launch_b" { 4700 } else { 5050 }),
    };
    let public_claim_copy = if launch == "launch_b" {
        "OpenAgents ran what we believe is the world's largest distributed language-model training run by number of model-progress participants: N distinct Pylons contributed accepted local-update work that advanced promoted checkpoint X for run Y.".to_string()
    } else {
        "OpenAgents ran what we believe is the world's largest distributed language-model training run by number of participants: N distinct Pylons contributed real compute through Psionic and completed accepted work for the same run, with run/window/checkpoint lineage published publicly.".to_string()
    };

    Ok(ProofA1MinimalLaunchProjection {
        launch: launch.to_string(),
        run_id: run_id.to_string(),
        run_definition_ref: "rundef.a1_minimal_distributed_lm.001.v1".to_string(),
        tokenizer_digest: "sha256:a1-minimal-tokenizer-fixture".to_string(),
        tokenized_dataset_digest: "sha256:a1-minimal-tokenized-dataset-fixture".to_string(),
        validation_set_digest: "sha256:a1-minimal-validation-set-fixture".to_string(),
        claim_warning: "The phrase \"by number of participants\" is allowed only when \"participant\" means accepted real compute work under one run id. It must never be inferred from online Pylons, seen-in-24h Pylons, sellable Pylons, generic payout totals, Discord members, downloads, or app sessions.".to_string(),
        public_claim_copy,
        counter_mappings: vec![
            ProofA1MinimalCounterMapping {
                public_label: "participants".to_string(),
                internal_source_of_truth: "training_accepted_contributors".to_string(),
            },
            ProofA1MinimalCounterMapping {
                public_label: "model-progress participants".to_string(),
                internal_source_of_truth: "training_model_progress_contributors".to_string(),
            },
        ],
        canonical_counters: canonical_counters.clone(),
        participants,
        artifacts,
        payout_projection: ProofA1MinimalPayoutProjection {
            accepted_work_payout_count: canonical_counters.training_accepted_contributors,
            support_work_payout_count: canonical_counters
                .training_accepted_contributors
                .saturating_sub(canonical_counters.training_model_progress_contributors),
            model_progress_payout_count: canonical_counters.training_model_progress_contributors,
            total_projected_sats: canonical_counters
                .training_accepted_contributors
                .saturating_mul(120),
        },
        public_stats_projection: ProofA1MinimalPublicStatsProjection {
            run_id: run_id.to_string(),
            training_accepted_contributors: canonical_counters.training_accepted_contributors,
            training_model_progress_contributors: canonical_counters
                .training_model_progress_contributors,
            training_weak_device_accepted_contributors: canonical_counters
                .training_weak_device_accepted_contributors,
            public_participant_label: "Participants".to_string(),
            public_model_progress_label: "Model-progress participants".to_string(),
            public_checkpoint_lineage_label: format!(
                "Checkpoint advanced by {} model-progress participants",
                canonical_counters.training_model_progress_contributors
            ),
        },
        checkpoint_lineage,
    })
}

fn a1_minimal_work_projection(
    _run_id: &str,
    participant_id: &str,
    assignment_id: &str,
    work_unit: openagents_kernel_core::compute::A1MinimalDistributedLmWorkUnitKind,
    weak_device: bool,
    enters_promoted_checkpoint_lineage: bool,
) -> ProofA1MinimalParticipantWorkProjection {
    let output_artifact_kind =
        openagents_kernel_core::pylon_training::pylon_training_a1_minimal_expected_output_artifact_kind(
            work_unit,
        );
    ProofA1MinimalParticipantWorkProjection {
        participant_id: participant_id.to_string(),
        provider_id: format!("provider.{participant_id}"),
        assignment_id: assignment_id.to_string(),
        work_unit_kind: work_unit.label().to_string(),
        work_class: work_unit.work_class().label().to_string(),
        progress_class: work_unit.progress_class_label().to_string(),
        weak_device,
        accepted: true,
        artifact_kind: output_artifact_kind.label().to_string(),
        artifact_class: output_artifact_kind.artifact_class().label().to_string(),
        support_or_verifier_work: work_unit.participation_only(),
        model_progress_work: work_unit.model_progress_bearing(),
        enters_promoted_checkpoint_lineage,
    }
}

fn a1_minimal_output_artifact_projection(
    run_id: &str,
    participant: &ProofA1MinimalParticipantWorkProjection,
) -> ProofA1MinimalArtifactProjection {
    ProofA1MinimalArtifactProjection {
        participant_id: participant.participant_id.clone(),
        assignment_id: participant.assignment_id.clone(),
        direction: "output".to_string(),
        artifact_id: format!(
            "oa.train_artifact.v1~kind~{}~network~trainnet.a1_minimal_distributed_lm.proof~run~{}~window~window.a1_minimal_distributed_lm.proof.0001~assignment~{}",
            participant.artifact_kind, run_id, participant.assignment_id
        ),
        artifact_kind: participant.artifact_kind.clone(),
        artifact_class: participant.artifact_class.clone(),
        signed_access_mode: "write".to_string(),
    }
}

fn derive_a1_minimal_canonical_counters(
    participants: &[ProofA1MinimalParticipantWorkProjection],
) -> ProofA1MinimalCanonicalCounters {
    let assigned = participants
        .iter()
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    let accepted = participants
        .iter()
        .filter(|participant| participant.accepted)
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    let weak_assigned = participants
        .iter()
        .filter(|participant| participant.weak_device)
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    let weak_accepted = participants
        .iter()
        .filter(|participant| participant.accepted && participant.weak_device)
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    let model_progress = participants
        .iter()
        .filter(|participant| {
            participant.accepted
                && participant.model_progress_work
                && participant.enters_promoted_checkpoint_lineage
        })
        .map(|participant| participant.participant_id.as_str())
        .collect::<BTreeSet<_>>();
    ProofA1MinimalCanonicalCounters {
        training_admitted_contributors: assigned.len() as u64,
        training_assigned_contributors: assigned.len() as u64,
        training_accepted_contributors: accepted.len() as u64,
        training_model_progress_contributors: model_progress.len() as u64,
        training_weak_device_assigned_contributors: weak_assigned.len() as u64,
        training_weak_device_accepted_contributors: weak_accepted.len() as u64,
    }
}

fn validate_a1_minimal_launch_projection(
    projection: &ProofA1MinimalLaunchProjection,
) -> Result<()> {
    ensure!(
        projection
            .participants
            .iter()
            .all(|participant| !(participant.support_or_verifier_work
                && participant.model_progress_work)),
        "a1_minimal_support_counted_as_model_progress"
    );
    let derived = derive_a1_minimal_canonical_counters(projection.participants.as_slice());
    ensure!(
        projection.canonical_counters == derived,
        "a1_minimal_canonical_counter_mismatch"
    );
    ensure!(
        projection
            .public_stats_projection
            .training_accepted_contributors
            == projection.canonical_counters.training_accepted_contributors,
        "a1_minimal_public_stats_participant_counter_mismatch"
    );
    ensure!(
        projection
            .public_stats_projection
            .training_model_progress_contributors
            == projection
                .canonical_counters
                .training_model_progress_contributors,
        "a1_minimal_public_stats_model_progress_counter_mismatch"
    );
    if projection.launch == "launch_b" {
        ensure!(
            projection
                .checkpoint_lineage
                .promoted_checkpoint_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            "a1_minimal_launch_b_promoted_checkpoint_missing"
        );
        ensure!(
            projection
                .canonical_counters
                .training_model_progress_contributors
                > 0,
            "a1_minimal_launch_b_model_progress_missing"
        );
    }
    Ok(())
}

fn a1_minimal_observed_run(
    projection: &ProofA1MinimalLaunchProjection,
) -> ProofObservedTrainingRunDetail {
    let window_id = format!("window.{}.proof.0001", projection.run_id);
    ProofObservedTrainingRunDetail {
        training_run_id: projection.run_id.clone(),
        run: ProofObservedRunState {
            training_run_id: projection.run_id.clone(),
            run_status: "running".to_string(),
            current_window_id: window_id.clone(),
            active_window_count: 0,
            pending_validation_window_count: 0,
            validator_challenges_open: 0,
            validator_challenges_queued: 0,
            latest_closeout_status: Some("rewarded".to_string()),
        },
        windows: vec![ProofObservedWindowState {
            window_id,
            status: "reconciled".to_string(),
            closeout_status: Some("rewarded".to_string()),
            accepted_contributions: projection
                .canonical_counters
                .training_accepted_contributors
                .min(u64::from(u32::MAX)) as u32,
            validator_challenges_open: 0,
            validator_challenges_queued: 0,
        }],
        contribution_count: projection.participants.len(),
        node_count: projection.participants.len(),
        caveat_count: 0,
        first_caveat_id: None,
        first_caveat_severity: None,
        first_caveat_title: None,
        first_caveat_detail: None,
    }
}

fn a1_minimal_simulated_fleet_status(
    config_path: &Path,
    namespace: &str,
    mode: ProofAuthorityMode,
    projection: &ProofA1MinimalLaunchProjection,
) -> ProofFleetStatusReport {
    let layout = proof_layout(config_path, namespace);
    ProofFleetStatusReport {
        configured: true,
        namespace: namespace.to_string(),
        mode: Some(mode),
        network_id: Some("trainnet.a1_minimal_distributed_lm.proof".to_string()),
        run_slug: Some(projection.run_id.clone()),
        paths: Some(ProofFleetPaths {
            namespace_root: layout.namespace_root.display().to_string(),
            fleet_root: layout.fleet_root.display().to_string(),
            fleet_state_path: layout.fleet_state_path.display().to_string(),
            run_report_path: layout.run_report_path.display().to_string(),
            trace_path: layout.trace_path.display().to_string(),
            summary_path: layout.summary_path.display().to_string(),
        }),
        authority: ProofAuthorityStatusReport {
            configured: true,
            namespace: namespace.to_string(),
            mode: Some(mode),
            started_at_ms: Some(super::now_epoch_ms()),
            admin_auth_configured: true,
            treasury_enabled: true,
            ports: None,
            paths: Some(ProofPersistedPaths {
                namespace_root: layout.namespace_root.display().to_string(),
                authority_env_path: layout.authority_env_path.display().to_string(),
                relay_data_dir: layout.relay_data_dir.display().to_string(),
                receipt_log_path: layout.receipt_log_path.display().to_string(),
                kernel_state_path: layout.kernel_state_path.display().to_string(),
                treasury_state_path: layout.treasury_state_path.display().to_string(),
                treasury_wallet_dir: layout.treasury_wallet_dir.display().to_string(),
                treasury_wallet_mnemonic_path: layout
                    .treasury_wallet_mnemonic_path
                    .display()
                    .to_string(),
                training_trn_identity_path: layout.training_trn_identity_path.display().to_string(),
                signer_credentials_path: layout.signer_credentials_path.display().to_string(),
                artifact_store_root: layout.artifact_store_root.display().to_string(),
                artifact_trace_path: layout.artifact_trace_path.display().to_string(),
            }),
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        },
        nodes: Vec::new(),
        launched_run: None,
    }
}

fn proof_run_status_from_detail(detail: Option<&ProofObservedTrainingRunDetail>) -> Option<&str> {
    detail.map(|value| value.run.run_status.as_str())
}

#[derive(Clone, Debug)]
struct ProofFleetDiagnosticScheduler {
    last_refresh_at: Instant,
    last_run_status: Option<String>,
    interval: Duration,
}

impl ProofFleetDiagnosticScheduler {
    fn new(last_refresh_at: Instant, initial_run_status: Option<&str>, interval: Duration) -> Self {
        Self {
            last_refresh_at,
            last_run_status: initial_run_status.map(ToString::to_string),
            interval,
        }
    }

    fn should_refresh(&self, now: Instant, run_status: Option<&str>, force: bool) -> bool {
        if force {
            return true;
        }
        if let Some(run_status) = run_status
            && self.last_run_status.as_deref() != Some(run_status)
        {
            return true;
        }
        now.duration_since(self.last_refresh_at) >= self.interval
    }

    fn mark_refreshed(&mut self, now: Instant, run_status: Option<&str>) {
        self.last_refresh_at = now;
        self.last_run_status = run_status.map(ToString::to_string);
    }
}

async fn refresh_proof_fleet_status_if_due(
    config_path: &Path,
    namespace: &str,
    scheduler: &mut ProofFleetDiagnosticScheduler,
    now: Instant,
    run_detail: Option<&ProofObservedTrainingRunDetail>,
    force: bool,
    fleet_status: &mut ProofFleetStatusReport,
) -> Result<()> {
    let run_status = proof_run_status_from_detail(run_detail);
    if scheduler.should_refresh(now, run_status, force) {
        *fleet_status = collect_proof_fleet_status(config_path, namespace).await?;
        scheduler.mark_refreshed(Instant::now(), run_status);
    }
    Ok(())
}

async fn run_standard_proof_lane(
    config_path: &Path,
    command: &ProofRunCommand,
    namespace: String,
) -> Result<ProofRunReport> {
    let lane_network_id = if command.lane.uses_hosted_starter_autolaunch() {
        HOSTED_CS336_A1_STARTER_NETWORK_ID.to_string()
    } else {
        proof_fleet_network_id(namespace.as_str())
    };
    let _fleet = ensure_proof_fleet_up(
        config_path,
        namespace.as_str(),
        command.mode,
        command.workers,
        command.validators,
        Some(lane_network_id.as_str()),
        command.stale_worker_state,
        command.stale_validator_state,
        command.lane.worker_fixture(),
        command.lane.validator_fixture(),
    )
    .await?;
    let layout = proof_layout(config_path, namespace.as_str());
    let authority_state =
        load_runtime_state(layout.runtime_state_path.as_path())?.ok_or_else(|| {
            anyhow!("proof authority runtime state missing for namespace {namespace}")
        })?;
    let mut first_failed_authority_write = None;
    let (training_run_id, launch, launch_detail) = if command.lane.uses_hosted_starter_autolaunch()
    {
        let training_run_id = wait_for_proof_hosted_starter_training_run_id(
            authority_state.urls.authority_base_url.as_str(),
            command.timeout_seconds,
        )
        .await?;
        let launch_detail = wait_for_proof_training_run_detail(
            authority_state.urls.authority_base_url.as_str(),
            training_run_id.as_str(),
            command.timeout_seconds,
        )
        .await?;
        (training_run_id, None, launch_detail)
    } else {
        let training_run_id = proof_lane_training_run_id(command.lane, namespace.as_str());
        let launch = launch_proof_lane(
            command.lane,
            authority_state.urls.authority_base_url.as_str(),
            authority_state.admin_bearer_token.as_str(),
            namespace.as_str(),
            lane_network_id.as_str(),
            training_run_id.as_str(),
            &mut first_failed_authority_write,
        )
        .await?;
        save_fleet_launch_record(config_path, namespace.as_str(), &launch)?;
        let launch_detail = summarize_training_run_detail_response(&launch.run_detail);
        (training_run_id, Some(launch), launch_detail)
    };
    let mut fleet_status = collect_proof_fleet_status(config_path, namespace.as_str()).await?;
    let mut diagnostic_scheduler = ProofFleetDiagnosticScheduler::new(
        Instant::now(),
        Some(launch_detail.run.run_status.as_str()),
        PROOF_FLEET_DIAGNOSTIC_INTERVAL,
    );
    if proof_run_status_is_terminal(&launch_detail.run) {
        let report = ProofRunReport {
            namespace,
            lane: command.lane.label().to_string(),
            generated_at_ms: super::now_epoch_ms(),
            timeout_seconds: command.timeout_seconds,
            status: "terminal".to_string(),
            detail: format!(
                "run reached terminal status {}",
                launch_detail.run.run_status
            ),
            blocker_id: None,
            fleet: fleet_status,
            launch,
            observed_run: Some(launch_detail),
            first_failed_authority_write,
            a1_minimal_projection: None,
        };
        persist_proof_run_outputs(config_path, &report).await?;
        return Ok(report);
    }

    let deadline = Instant::now() + Duration::from_secs(command.timeout_seconds);
    let mut last_detail = Some(launch_detail);
    loop {
        if let Some(detail) = fetch_proof_training_run_detail(
            authority_state.urls.authority_base_url.as_str(),
            training_run_id.as_str(),
        )
        .await?
        {
            if proof_run_status_is_terminal(&detail.run) {
                let report = ProofRunReport {
                    namespace,
                    lane: command.lane.label().to_string(),
                    generated_at_ms: super::now_epoch_ms(),
                    timeout_seconds: command.timeout_seconds,
                    status: "terminal".to_string(),
                    detail: format!("run reached terminal status {}", detail.run.run_status),
                    blocker_id: None,
                    fleet: fleet_status,
                    launch: launch.clone(),
                    observed_run: Some(detail),
                    first_failed_authority_write: first_failed_authority_write.clone(),
                    a1_minimal_projection: None,
                };
                persist_proof_run_outputs(config_path, &report).await?;
                return Ok(report);
            }
            last_detail = Some(detail);
        }
        refresh_proof_fleet_status_if_due(
            config_path,
            namespace.as_str(),
            &mut diagnostic_scheduler,
            Instant::now(),
            last_detail.as_ref(),
            false,
            &mut fleet_status,
        )
        .await?;
        if let Some((blocker_id, detail)) =
            detect_proof_run_blocker(&fleet_status, last_detail.as_ref())
        {
            let report = ProofRunReport {
                namespace,
                lane: command.lane.label().to_string(),
                generated_at_ms: super::now_epoch_ms(),
                timeout_seconds: command.timeout_seconds,
                status: "blocked".to_string(),
                detail,
                blocker_id: Some(blocker_id),
                fleet: fleet_status,
                launch: launch.clone(),
                observed_run: last_detail,
                first_failed_authority_write: first_failed_authority_write.clone(),
                a1_minimal_projection: None,
            };
            persist_proof_run_outputs(config_path, &report).await?;
            return Ok(report);
        }
        if let Some(detail) = last_detail.as_ref() {
            if let Some(completion_detail) = standard_proof_lane_completion_detail(
                &fleet_status,
                detail,
                command.workers,
                command.validators,
                command.lane.uses_hosted_starter_autolaunch(),
            ) {
                let report = ProofRunReport {
                    namespace,
                    lane: command.lane.label().to_string(),
                    generated_at_ms: super::now_epoch_ms(),
                    timeout_seconds: command.timeout_seconds,
                    status: "completed".to_string(),
                    detail: completion_detail,
                    blocker_id: None,
                    fleet: fleet_status,
                    launch: launch.clone(),
                    observed_run: last_detail,
                    first_failed_authority_write: first_failed_authority_write.clone(),
                    a1_minimal_projection: None,
                };
                persist_proof_run_outputs(config_path, &report).await?;
                return Ok(report);
            }
        }
        if Instant::now() >= deadline {
            refresh_proof_fleet_status_if_due(
                config_path,
                namespace.as_str(),
                &mut diagnostic_scheduler,
                Instant::now(),
                last_detail.as_ref(),
                true,
                &mut fleet_status,
            )
            .await?;
            let report = ProofRunReport {
                namespace,
                lane: command.lane.label().to_string(),
                generated_at_ms: super::now_epoch_ms(),
                timeout_seconds: command.timeout_seconds,
                status: "blocked".to_string(),
                detail: format!(
                    "timed out after {}s waiting for terminal state or first explicit blocker",
                    command.timeout_seconds
                ),
                blocker_id: Some("proof_run_timeout".to_string()),
                fleet: fleet_status,
                launch: launch.clone(),
                observed_run: last_detail,
                first_failed_authority_write,
                a1_minimal_projection: None,
            };
            persist_proof_run_outputs(config_path, &report).await?;
            return Ok(report);
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
}

async fn run_manual_replacement_attempt_proof_lane(
    config_path: &Path,
    command: &ProofRunCommand,
    namespace: String,
) -> Result<ProofRunReport> {
    let lane_network_id = proof_fleet_network_id(namespace.as_str());
    let _fleet = ensure_proof_fleet_up(
        config_path,
        namespace.as_str(),
        command.mode,
        command.workers,
        command.validators,
        Some(lane_network_id.as_str()),
        false,
        false,
        None,
        None,
    )
    .await?;
    let layout = proof_layout(config_path, namespace.as_str());
    let authority_state =
        load_runtime_state(layout.runtime_state_path.as_path())?.ok_or_else(|| {
            anyhow!("proof authority runtime state missing for namespace {namespace}")
        })?;
    let training_run_id = proof_lane_training_run_id(command.lane, namespace.as_str());
    let mut first_failed_authority_write = None;

    let primary_worker = replacement_attempt_node_admission_request(
        "proof-replacement-worker-a",
        lane_network_id.as_str(),
        "lnbc1proofreplacementa",
    );
    let _: super::PylonTrainingNodeAdmissionResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/nodes/admission",
        &primary_worker,
        &mut first_failed_authority_write,
        "replacement_worker_a_admission",
    )
    .await?;
    let _: super::PylonTrainingHeartbeatResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/heartbeats",
        &replacement_attempt_idle_heartbeat(
            primary_worker.node_pubkey_hex.as_str(),
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_a_heartbeat",
    )
    .await?;

    let launch = launch_proof_lane(
        command.lane,
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        namespace.as_str(),
        lane_network_id.as_str(),
        training_run_id.as_str(),
        &mut first_failed_authority_write,
    )
    .await?;
    save_fleet_launch_record(config_path, namespace.as_str(), &launch)?;
    let observed_launch = summarize_training_run_detail_response(&launch.run_detail);
    let window_id = observed_launch.run.current_window_id.trim().to_string();
    ensure!(
        !window_id.is_empty(),
        "replacement-attempt proof lane did not materialize a current window"
    );

    let primary_lease = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/leases/claim",
        &replacement_attempt_lease_request(
            primary_worker.node_pubkey_hex.as_str(),
            training_run_id.as_str(),
            lane_network_id.as_str(),
            super::PylonTrainingRoleClaim::Worker,
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_a_claim",
    )
    .await?;
    let _: super::PylonTrainingAssignmentAckResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/assignments/ack",
        &replacement_attempt_assignment_ack(
            primary_worker.node_pubkey_hex.as_str(),
            &primary_lease,
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_a_ack",
    )
    .await?;
    let _: super::PylonTrainingFailureNoticeResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/failures",
        &replacement_attempt_failure_notice(
            primary_worker.node_pubkey_hex.as_str(),
            &primary_lease,
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_a_failure",
    )
    .await?;

    let replacement_worker = replacement_attempt_node_admission_request(
        "proof-replacement-worker-b",
        lane_network_id.as_str(),
        "lnbc1proofreplacementb",
    );
    let _: super::PylonTrainingNodeAdmissionResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/nodes/admission",
        &replacement_worker,
        &mut first_failed_authority_write,
        "replacement_worker_b_admission",
    )
    .await?;
    let _: super::PylonTrainingHeartbeatResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/heartbeats",
        &replacement_attempt_idle_heartbeat(
            replacement_worker.node_pubkey_hex.as_str(),
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_b_heartbeat",
    )
    .await?;

    let replacement_lease = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/leases/claim",
        &replacement_attempt_lease_request(
            replacement_worker.node_pubkey_hex.as_str(),
            training_run_id.as_str(),
            lane_network_id.as_str(),
            super::PylonTrainingRoleClaim::Worker,
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_b_claim",
    )
    .await?;
    let _: super::PylonTrainingAssignmentAckResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/assignments/ack",
        &replacement_attempt_assignment_ack(
            replacement_worker.node_pubkey_hex.as_str(),
            &replacement_lease,
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_worker_b_ack",
    )
    .await?;

    let template = load_proof_replacement_contribution_template()?;
    let contribution = replacement_attempt_contribution_input(
        &template,
        &replacement_lease,
        lane_network_id.as_str(),
    );
    let replacement_assignment_detail = format!(
        "replacement attempt {} for {}",
        replacement_lease.assignment_id, window_id
    );
    if let Err(error) =
        proof_post_authority_json::<_, super::PylonTrainingWindowCoordinatorResponse>(
            authority_state.urls.authority_base_url.as_str(),
            authority_state.admin_bearer_token.as_str(),
            format!("/api/training/windows/{window_id}/seal").as_str(),
            &super::PylonSealTrainingWindowRequest {
                idempotency_key: format!(
                    "proof.replacement.seal.{}.{}",
                    namespace_slug(namespace.as_str()),
                    replacement_lease.assignment_id
                ),
                recorded_at_ms: super::now_epoch_ms(),
                window_id: window_id.clone(),
                contribution_outcomes: vec![contribution.clone()],
            },
            &mut first_failed_authority_write,
            "replacement_window_seal",
        )
        .await
    {
        let (fleet_status, run_detail_result) = tokio::join!(
            collect_proof_fleet_status(config_path, namespace.as_str()),
            fetch_proof_training_run_detail(
                authority_state.urls.authority_base_url.as_str(),
                training_run_id.as_str(),
            )
        );
        let fleet_status = fleet_status?;
        let observed_run = run_detail_result?.or(Some(observed_launch.clone()));
        let report = ProofRunReport {
            namespace,
            lane: command.lane.label().to_string(),
            generated_at_ms: super::now_epoch_ms(),
            timeout_seconds: command.timeout_seconds,
            status: "blocked".to_string(),
            detail: format!("{replacement_assignment_detail} blocked at seal: {error:#}"),
            blocker_id: Some("authority_write_failed".to_string()),
            fleet: fleet_status,
            launch: Some(launch),
            observed_run,
            first_failed_authority_write,
            a1_minimal_projection: None,
        };
        persist_proof_run_outputs(config_path, &report).await?;
        return Ok(report);
    }

    let validator = replacement_attempt_validator_admission_request(
        "proof-replacement-validator",
        lane_network_id.as_str(),
        "lnbc1proofreplacementvalidator",
    );
    let _: super::PylonTrainingNodeAdmissionResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/nodes/admission",
        &validator,
        &mut first_failed_authority_write,
        "replacement_validator_admission",
    )
    .await?;
    let _: super::PylonTrainingHeartbeatResponse = proof_post_authority_json(
        authority_state.urls.authority_base_url.as_str(),
        authority_state.admin_bearer_token.as_str(),
        "/api/training/heartbeats",
        &replacement_attempt_idle_heartbeat(
            validator.node_pubkey_hex.as_str(),
            super::now_epoch_ms(),
        ),
        &mut first_failed_authority_write,
        "replacement_validator_heartbeat",
    )
    .await?;
    for validation_index in 1..=2 {
        let validator_claim: super::PylonTrainingValidatorChallengeCoordinatorResponse =
            proof_post_authority_json(
                authority_state.urls.authority_base_url.as_str(),
                authority_state.admin_bearer_token.as_str(),
                "/api/training/validator-challenges/claim",
                &super::PylonClaimTrainingValidatorChallengeRequest {
                    idempotency_key: format!(
                        "proof.replacement.validator.claim.{}.{}.{}",
                        namespace_slug(namespace.as_str()),
                        replacement_lease.assignment_id,
                        validation_index
                    ),
                    requested_at_ms: super::now_epoch_ms(),
                    node_pubkey_hex: validator.node_pubkey_hex.clone(),
                    requested_network_id: Some(lane_network_id.clone()),
                    requested_training_run_id: Some(training_run_id.clone()),
                },
                &mut first_failed_authority_write,
                "replacement_validator_claim",
            )
            .await?;
        let validator_lease = validator_claim
            .lease
            .clone()
            .ok_or_else(|| anyhow!("replacement validator claim missing lease"))?;
        let finalized_at_ms = super::now_epoch_ms();
        let validator_result = replacement_attempt_validator_result(
            &validator_claim,
            &validator_lease,
            finalized_at_ms,
        );
        let _: super::PylonTrainingValidatorChallengeCoordinatorResponse =
            proof_post_authority_json(
                authority_state.urls.authority_base_url.as_str(),
                authority_state.admin_bearer_token.as_str(),
                format!(
                    "/api/training/validator-challenges/{}/finalize",
                    validator_claim.challenge_id
                )
                .as_str(),
                &super::PylonFinalizeTrainingValidatorChallengeRequest {
                    idempotency_key: format!(
                        "proof.replacement.validator.finalize.{}.{}",
                        namespace_slug(namespace.as_str()),
                        validator_claim.challenge_id
                    ),
                    recorded_at_ms: finalized_at_ms,
                    node_pubkey_hex: validator.node_pubkey_hex.clone(),
                    lease: validator_lease,
                    result: validator_result,
                    training_disposition: Some(
                        super::ComputeAdapterContributionDisposition::Accepted,
                    ),
                },
                &mut first_failed_authority_write,
                "replacement_validator_finalize",
            )
            .await?;
    }

    if let Err(error) =
        proof_post_authority_json::<_, super::PylonTrainingWindowCoordinatorResponse>(
            authority_state.urls.authority_base_url.as_str(),
            authority_state.admin_bearer_token.as_str(),
            format!("/api/training/windows/{window_id}/reconcile").as_str(),
            &super::PylonReconcileTrainingWindowRequest {
                idempotency_key: format!(
                    "proof.replacement.reconcile.{}.{}",
                    namespace_slug(namespace.as_str()),
                    replacement_lease.assignment_id
                ),
                recorded_at_ms: super::now_epoch_ms(),
                window_id: window_id.clone(),
                contribution_outcomes: vec![contribution],
                held_out_average_score_bps: template.held_out_average_score_bps,
                benchmark_pass_rate_bps: template.benchmark_pass_rate_bps,
                runtime_smoke_passed: template.runtime_smoke_passed,
                aggregated_delta_digest: template.aggregated_delta_digest.clone(),
                accepted_aggregate_id: template.accepted_aggregate_id.clone(),
                promoted_checkpoint_ref: template.promoted_checkpoint_ref.clone(),
            },
            &mut first_failed_authority_write,
            "replacement_window_reconcile",
        )
        .await
    {
        let (fleet_status, run_detail_result) = tokio::join!(
            collect_proof_fleet_status(config_path, namespace.as_str()),
            fetch_proof_training_run_detail(
                authority_state.urls.authority_base_url.as_str(),
                training_run_id.as_str(),
            )
        );
        let fleet_status = fleet_status?;
        let observed_run = run_detail_result?.or(Some(observed_launch.clone()));
        let report = ProofRunReport {
            namespace,
            lane: command.lane.label().to_string(),
            generated_at_ms: super::now_epoch_ms(),
            timeout_seconds: command.timeout_seconds,
            status: "blocked".to_string(),
            detail: format!("{replacement_assignment_detail} blocked at reconcile: {error:#}"),
            blocker_id: Some("authority_write_failed".to_string()),
            fleet: fleet_status,
            launch: Some(launch),
            observed_run,
            first_failed_authority_write,
            a1_minimal_projection: None,
        };
        persist_proof_run_outputs(config_path, &report).await?;
        return Ok(report);
    }

    let (fleet_status, run_detail_result) = tokio::join!(
        collect_proof_fleet_status(config_path, namespace.as_str()),
        fetch_proof_training_run_detail(
            authority_state.urls.authority_base_url.as_str(),
            training_run_id.as_str(),
        )
    );
    let fleet_status = fleet_status?;
    let observed_run = run_detail_result?.or(Some(observed_launch));
    let replacement_detail =
        format!("{replacement_assignment_detail} sealed and reconciled locally");
    if let Some((blocker_id, detail)) =
        detect_proof_run_blocker(&fleet_status, observed_run.as_ref())
    {
        let report = ProofRunReport {
            namespace,
            lane: command.lane.label().to_string(),
            generated_at_ms: super::now_epoch_ms(),
            timeout_seconds: command.timeout_seconds,
            status: "blocked".to_string(),
            detail: format!("{replacement_detail}; {detail}"),
            blocker_id: Some(blocker_id),
            fleet: fleet_status,
            launch: Some(launch),
            observed_run,
            first_failed_authority_write,
            a1_minimal_projection: None,
        };
        persist_proof_run_outputs(config_path, &report).await?;
        return Ok(report);
    }

    let report = ProofRunReport {
        namespace,
        lane: command.lane.label().to_string(),
        generated_at_ms: super::now_epoch_ms(),
        timeout_seconds: command.timeout_seconds,
        status: "completed".to_string(),
        detail: replacement_detail,
        blocker_id: None,
        fleet: fleet_status,
        launch: Some(launch),
        observed_run,
        first_failed_authority_write,
        a1_minimal_projection: None,
    };
    persist_proof_run_outputs(config_path, &report).await?;
    Ok(report)
}

fn replacement_attempt_node_admission_request(
    node_pubkey_hex: &str,
    network_id: &str,
    settlement_destination: &str,
) -> super::PylonTrainingNodeAdmissionRequest {
    let contributor_availability = super::ProviderAdapterTrainingContributorAvailability {
        contributor_supported: true,
        coordinator_match_supported: true,
        authority_receipt_supported: true,
        execution_backends: vec![
            super::ProviderAdapterTrainingExecutionBackend::OpenAdapterBackend,
        ],
        adapter_families: vec!["openagents.adapter.reference".to_string()],
        adapter_formats: vec!["openagents.adapter.delta.v1".to_string()],
        validator_policy_refs: vec!["policy://validator/mvp/v1".to_string()],
        checkpoint_families: vec!["decoder".to_string()],
        environment_refs: vec![super::PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string()],
        minimum_memory_gb: Some(16),
        available_memory_gb: Some(16),
        settlement_trigger: Some(
            super::ProviderAdapterTrainingSettlementTrigger::AcceptedSealedWindow,
        ),
    };
    let capability_tier = super::ProviderTrainingCapabilityTierProfile {
        tier: super::ProviderTrainingCapabilityTier::Tier2Trainer,
        backend_families: vec!["cpu".to_string()],
        accelerator_inventory: Vec::new(),
        memory_floor_gb: Some(16),
        available_memory_gb: Some(16),
        throughput_band: super::ProviderTrainingThroughputBand::Medium,
        lease_reliability: super::ProviderTrainingLeaseReliabilityClass::Steady,
        replay_capability: super::ProviderTrainingReplayCapability::ShortWindow,
        artifact_upload_latency_class: super::ProviderTrainingArtifactUploadLatencyClass::Moderate,
    };
    super::PylonTrainingNodeAdmissionRequest {
        idempotency_key: format!(
            "proof.replacement.admission.{}.{}",
            node_pubkey_hex,
            super::now_epoch_ms()
        ),
        requested_at_ms: super::now_epoch_ms(),
        node_pubkey_hex: node_pubkey_hex.to_string(),
        release_id: super::local_training_release_id(),
        node_label: Some(format!("proof-{node_pubkey_hex}")),
        role_claims: vec![super::PylonTrainingRoleClaim::Worker],
        allowed_networks: vec![network_id.to_string()],
        build_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        build_digest: Some(super::local_training_build_digest()),
        capability_envelope_v2: super::derive_training_capability_envelope_v2(
            &capability_tier,
            &contributor_availability,
            true,
        ),
        contributor_availability,
        capability_tier,
        host_telemetry: None,
        active_reputation_labels: Vec::new(),
        settlement_destination: Some(settlement_destination.to_string()),
    }
}

fn replacement_attempt_validator_admission_request(
    node_pubkey_hex: &str,
    network_id: &str,
    settlement_destination: &str,
) -> super::PylonTrainingNodeAdmissionRequest {
    let mut request = replacement_attempt_node_admission_request(
        node_pubkey_hex,
        network_id,
        settlement_destination,
    );
    request.role_claims = vec![super::PylonTrainingRoleClaim::Validator];
    request.node_label = Some(format!("proof-{node_pubkey_hex}-validator"));
    request
}

fn replacement_attempt_validator_result(
    claim: &super::PylonTrainingValidatorChallengeCoordinatorResponse,
    lease: &super::ComputeValidatorChallengeLease,
    finalized_at_ms: i64,
) -> super::ComputeValidatorChallengeResult {
    let result_digest = sha256_prefixed_bytes(
        format!(
            "proof.replacement.validator.result.{}.{}",
            claim.challenge_id, lease.attempt
        )
        .as_bytes(),
    );
    super::ComputeValidatorChallengeResult {
        challenge_id: claim.challenge_id.clone(),
        proof_bundle_digest: claim.challenge.request.context.proof_bundle_digest.clone(),
        protocol_id: claim.challenge.request.protocol.label().to_string(),
        attempt: lease.attempt,
        status: super::ComputeValidatorChallengeStatus::Verified,
        verdict: openagents_kernel_core::compute::ComputeValidatorChallengeVerdict::Verified,
        reason_code: None,
        detail: "proof replacement validator verdict".to_string(),
        created_at_ms: claim.challenge.request.context.created_at_ms,
        finalized_at_ms: finalized_at_ms.max(0) as u64,
        challenge_seed_digest: None,
        verified_row_count: Some(1),
        result_digest: result_digest.clone(),
        challenge_result_ref: format!(
            "validator_challenge_result:{}:{}",
            claim.challenge_id, lease.attempt
        ),
    }
}

fn replacement_attempt_idle_heartbeat(
    node_pubkey_hex: &str,
    recorded_at_ms: i64,
) -> super::PylonTrainingHeartbeatRequest {
    super::PylonTrainingHeartbeatRequest {
        idempotency_key: format!("proof.replacement.heartbeat.{node_pubkey_hex}.{recorded_at_ms}"),
        recorded_at_ms,
        node_pubkey_hex: node_pubkey_hex.to_string(),
        build_digest: super::local_training_build_digest(),
        training_run_id: "run.idle".to_string(),
        window_id: "window.idle".to_string(),
        assignment_id: format!("assignment.idle.{node_pubkey_hex}"),
        lease_id: format!("lease.idle.{node_pubkey_hex}"),
        desired_state: super::PylonTrainingSupervisorDesiredState::Running,
        process_state: super::PylonTrainingSupervisorProcessState::Running,
        last_heartbeat_at_ms: Some(recorded_at_ms),
        last_exit_code: None,
    }
}

fn replacement_attempt_lease_request(
    node_pubkey_hex: &str,
    training_run_id: &str,
    network_id: &str,
    role: super::PylonTrainingRoleClaim,
    requested_at_ms: i64,
) -> super::PylonTrainingRunLeaseRequest {
    super::PylonTrainingRunLeaseRequest {
        idempotency_key: format!("proof.replacement.lease.{node_pubkey_hex}.{requested_at_ms}"),
        requested_at_ms,
        node_pubkey_hex: node_pubkey_hex.to_string(),
        role,
        requested_network_id: Some(network_id.to_string()),
        requested_training_run_id: Some(training_run_id.to_string()),
        membership_revision: None,
    }
}

fn replacement_attempt_assignment_ack(
    node_pubkey_hex: &str,
    lease: &super::PylonTrainingRunLeaseResponse,
    acked_at_ms: i64,
) -> super::PylonTrainingAssignmentAckRequest {
    super::PylonTrainingAssignmentAckRequest {
        idempotency_key: format!(
            "proof.replacement.ack.{}.{}",
            lease.assignment_id, acked_at_ms
        ),
        acked_at_ms,
        node_pubkey_hex: node_pubkey_hex.to_string(),
        training_run_id: lease.training_run_id.clone(),
        window_id: lease.window_id.clone(),
        assignment_id: lease.assignment_id.clone(),
        lease_id: lease.lease_id.clone(),
        manifest_digest: lease.manifest_digest.clone(),
        manifest_path: None,
    }
}

fn replacement_attempt_failure_notice(
    node_pubkey_hex: &str,
    lease: &super::PylonTrainingRunLeaseResponse,
    reported_at_ms: i64,
) -> super::PylonTrainingFailureNoticeRequest {
    super::PylonTrainingFailureNoticeRequest {
        idempotency_key: format!(
            "proof.replacement.failure.{}.{}",
            lease.assignment_id, reported_at_ms
        ),
        reported_at_ms,
        node_pubkey_hex: node_pubkey_hex.to_string(),
        training_run_id: lease.training_run_id.clone(),
        window_id: lease.window_id.clone(),
        assignment_id: lease.assignment_id.clone(),
        lease_id: lease.lease_id.clone(),
        failure_reason: "proof replacement attempt fixture forced retry".to_string(),
        exit_code: Some(1),
        failure_receipt_path: None,
    }
}

fn replacement_attempt_contribution_input(
    template: &ProofReplacementContributionTemplate,
    lease: &super::PylonTrainingRunLeaseResponse,
    network_id: &str,
) -> super::PylonTrainingWindowContributionInput {
    super::PylonTrainingWindowContributionInput {
        contribution_id: sha256_prefixed_bytes(
            format!("proof.replacement.contribution.{}", lease.assignment_id).as_bytes(),
        ),
        assignment_id: lease.assignment_id.clone(),
        submission_receipt_digest: template.submission_receipt_digest.clone(),
        artifact_id: format!(
            "oa.train_artifact.v1~kind~local_update~network~{}~run~{}~window~{}~assignment~{}",
            network_id, lease.training_run_id, lease.window_id, lease.assignment_id
        ),
        manifest_digest: template.manifest_digest.clone(),
        object_digest: template.object_digest.clone(),
        artifact_receipt_digest: template.artifact_receipt_digest.clone(),
        provenance_bundle_digest: template.provenance_bundle_digest.clone(),
        security_receipt_digest: template.security_receipt_digest.clone(),
        replay_receipt_digest: template.replay_receipt_digest.clone(),
        validator_receipt_digest: template.validator_receipt_digest.clone(),
        validation_reason_codes: template.validation_reason_codes.clone(),
        validator_disposition: template.validator_disposition,
        aggregation_eligibility: template.aggregation_eligibility,
        local_step_count: template.local_step_count,
        consumed_token_count: template.consumed_token_count,
        consumed_example_count: template.consumed_example_count,
        aggregation_weight_basis: template.aggregation_weight_basis.clone(),
        aggregation_weight_value: template.aggregation_weight_value,
        aggregation_weight_bps: template.aggregation_weight_bps,
        promotion_receipt_digest: template.promotion_receipt_digest.clone(),
        metadata: template.metadata.clone(),
    }
}

async fn proof_post_authority_json<TReq, TResp>(
    authority_base_url: &str,
    admin_bearer_token: &str,
    path: &str,
    request: &TReq,
    first_failed_authority_write: &mut Option<ProofAuthorityWriteFailureCapture>,
    source: &str,
) -> Result<TResp>
where
    TReq: Serialize,
    TResp: DeserializeOwned,
{
    let url = format!(
        "{}/{}",
        authority_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let response = reqwest::Client::new()
        .post(url.as_str())
        .bearer_auth(admin_bearer_token)
        .json(request)
        .send()
        .await
        .with_context(|| format!("failed to post proof authority request to {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("failed to read proof authority response body")?;
    if !status.is_success() {
        first_failed_authority_write.get_or_insert_with(|| ProofAuthorityWriteFailureCapture {
            source: source.to_string(),
            observed_at_ms: super::now_epoch_ms(),
            method: Some("POST".to_string()),
            url: Some(url.clone()),
            status: Some(status.as_u16()),
            response_body: Some(body.clone()),
            detail: format!("proof authority write failed for {source}"),
        });
        bail!(
            "proof authority write {source} failed with status {}: {}",
            status.as_u16(),
            body.trim()
        );
    }
    serde_json::from_str::<TResp>(body.as_str())
        .with_context(|| format!("failed to decode proof authority response for {source}"))
}

async fn spawn_proof_fleet_node(
    namespace: &str,
    layout: &ProofLayout,
    authority: &ProofAuthorityRuntimeState,
    role: ProofFleetNodeRole,
    index: usize,
    network_id: &str,
    stale_retained_state: bool,
    retained_state_fixture: Option<ProofNodeRuntimeFixture>,
    current_exe: &Path,
    psionic_repo_root: Option<&Path>,
    used_ports: &mut BTreeSet<u16>,
) -> Result<ProofFleetNodeRuntimeRecord> {
    let node_root = proof_fleet_node_root(layout, role, index);
    if node_root.exists() {
        fs::remove_dir_all(node_root.as_path())
            .with_context(|| format!("failed to reset proof node root {}", node_root.display()))?;
    }
    fs::create_dir_all(node_root.as_path())
        .with_context(|| format!("failed to create proof node root {}", node_root.display()))?;
    let ports = allocate_proof_node_ports(namespace, role, index, used_ports)?;
    let config_path = node_root.join("config.json");
    let log_path = node_root.join("logs").join("serve.log");
    let node_label = format!(
        "proof-{}-{}-{}",
        namespace_slug(namespace),
        role.label(),
        index
    );
    let payout_destination = format!("lnbc1proof{}{}", role.label(), index);
    let mut config = super::default_config(node_root.as_path());
    config.node_label = node_label.clone();
    config.external_payout_target = Some(payout_destination.clone());
    config.admin_listen_addr = format!("127.0.0.1:{}", ports.admin);
    config.nexus_control_base_url = authority.urls.authority_base_url.clone();
    config.relay_urls = authority
        .urls
        .relay_ws_url
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    config.relay_auth_enabled = false;
    config.wallet_network = "regtest".to_string();
    config.wallet_api_key_env = None;
    config.training.allowed_networks = vec![network_id.to_string()];
    config.training.role_claims = vec![role.role_claim()];
    config.training.run_root = node_root.join("training");
    config.training.checkpoint_serve_addr = format!("127.0.0.1:{}", ports.checkpoint);
    config.training.nexus_authority_base_url = authority.urls.authority_base_url.clone();
    config.training.relay_urls = config.relay_urls.clone();
    config.training.validator_enabled = role == ProofFleetNodeRole::Validator;
    super::save_config(config_path.as_path(), &config)?;
    let config = super::ensure_local_setup(config_path.as_path())?;
    if let Some(retained_state_fixture) = retained_state_fixture {
        apply_proof_node_runtime_fixture(&config, retained_state_fixture, network_id)?;
    } else if stale_retained_state {
        inject_stale_training_runtime_state(&config, role, network_id)?;
    }
    let mut envs = vec![
        (
            super::ENV_PYLON_HOME.to_string(),
            node_root.display().to_string(),
        ),
        (
            super::ENV_TRAINING_NEXUS_BEARER_TOKEN.to_string(),
            authority.admin_bearer_token.clone(),
        ),
    ];
    if let Some(psionic_repo_root) = psionic_repo_root {
        envs.push((
            super::ENV_PSIONIC_REPO.to_string(),
            psionic_repo_root.display().to_string(),
        ));
    }
    let args = vec![
        "--config-path".to_string(),
        config_path.display().to_string(),
    ];
    let pid = spawn_logged_process(
        current_exe,
        args.as_slice(),
        envs.as_slice(),
        log_path.as_path(),
    )?;
    Ok(ProofFleetNodeRuntimeRecord {
        role,
        index,
        node_label,
        payout_destination,
        home_dir: node_root.display().to_string(),
        config_path: config_path.display().to_string(),
        run_root: config.training.run_root.display().to_string(),
        admin_url: format!("http://127.0.0.1:{}", ports.admin),
        checkpoint_serve_url: format!("http://127.0.0.1:{}", ports.checkpoint),
        ports,
        stale_retained_state_injected: stale_retained_state,
        retained_state_fixture_id: retained_state_fixture
            .map(|fixture| fixture.fixture_id().to_string()),
        process: ProofProcessRecord {
            binary: current_exe.display().to_string(),
            pid: Some(pid),
            log_path: log_path.display().to_string(),
        },
    })
}

async fn load_proof_node_training_status(
    config_path: &Path,
) -> Option<ProofFleetNodeTrainingStatus> {
    match super::load_training_status_report(config_path).await {
        Ok(report) => {
            let explicit_issues = report
                .recent_issues
                .iter()
                .filter(|issue| proof_training_issue_counts_as_explicit_blocker(issue))
                .collect::<Vec<_>>();
            Some(ProofFleetNodeTrainingStatus {
                current_run_id: report.current_run_id,
                active_window_id: report.active_window_id,
                active_runtime_process_state: report
                    .active_runtime
                    .as_ref()
                    .map(|runtime| runtime.process_state.clone()),
                last_failure_reason: report
                    .active_runtime
                    .as_ref()
                    .and_then(|runtime| runtime.last_failure_reason.clone()),
                recent_issue_count: explicit_issues.len(),
                first_issue_reason: explicit_issues.first().map(|issue| issue.reason.clone()),
                pending_closeout_count: report.pending_closeout_objects.len(),
                load_error: None,
            })
        }
        Err(error) => Some(ProofFleetNodeTrainingStatus {
            current_run_id: None,
            active_window_id: None,
            active_runtime_process_state: None,
            last_failure_reason: None,
            recent_issue_count: 0,
            first_issue_reason: None,
            pending_closeout_count: 0,
            load_error: Some(format!("{error:#}")),
        }),
    }
}

fn proof_training_issue_counts_as_explicit_blocker(
    issue: &super::TrainingOperatorIssueStatus,
) -> bool {
    if !issue.retryable {
        return true;
    }
    let kind = issue.kind.trim();
    let reason = issue.reason.trim();
    let blocking_class = issue.blocking_class.as_deref().map(str::trim);
    if kind == "artifact_bundle" && reason == "artifact_incomplete" {
        return false;
    }
    if kind == "closeout_progress_stalled" && blocking_class == Some("local_queue_replay") {
        return false;
    }
    true
}

fn detect_proof_run_blocker(
    fleet: &ProofFleetStatusReport,
    observed_run: Option<&ProofObservedTrainingRunDetail>,
) -> Option<(String, String)> {
    if fleet.authority.probes.iter().any(|probe| !probe.ok) {
        let probe = fleet.authority.probes.iter().find(|probe| !probe.ok)?;
        return Some((
            "authority_probe_failed".to_string(),
            format!(
                "authority probe {} failed: {}",
                probe.route_id, probe.detail
            ),
        ));
    }
    for node in &fleet.nodes {
        if !node.process.running {
            return Some((
                "fleet_node_exited".to_string(),
                format!(
                    "{} {} process is not running; inspect {}",
                    node.role.label(),
                    node.index,
                    node.process.log_path
                ),
            ));
        }
        if let Some(training) = node.training.as_ref() {
            if let Some(load_error) = training.load_error.as_deref() {
                return Some((
                    "fleet_node_status_unavailable".to_string(),
                    format!(
                        "{} {} training status failed to load: {}",
                        node.role.label(),
                        node.index,
                        load_error
                    ),
                ));
            }
            if let Some(reason) = training.last_failure_reason.as_deref() {
                return Some((
                    "fleet_node_failure".to_string(),
                    format!(
                        "{} {} reported failure: {}",
                        node.role.label(),
                        node.index,
                        reason
                    ),
                ));
            }
            if let Some(reason) = training.first_issue_reason.as_deref() {
                return Some((
                    "fleet_node_issue".to_string(),
                    format!(
                        "{} {} surfaced issue: {}",
                        node.role.label(),
                        node.index,
                        reason
                    ),
                ));
            }
        }
    }
    if let Some(observed) = observed_run {
        if let Some(detail) = critical_run_caveat_detail(observed) {
            return Some(("authority_run_caveat".to_string(), detail));
        }
    }
    None
}

fn critical_run_caveat_detail(observed: &ProofObservedTrainingRunDetail) -> Option<String> {
    let severity = observed
        .first_caveat_severity
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let normalized = severity.to_ascii_lowercase();
    if normalized != "critical" && normalized != "error" {
        return None;
    }
    if observed
        .first_caveat_id
        .as_deref()
        .is_some_and(|value| value == "payout_lag")
        && observed
            .first_caveat_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("0 accepted-work payout(s) need attention"))
    {
        return None;
    }
    if observed
        .first_caveat_id
        .as_deref()
        .is_some_and(|value| value == "validator_backlog")
    {
        return None;
    }
    let mut parts = Vec::new();
    if let Some(title) = observed
        .first_caveat_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(title.to_string());
    }
    if let Some(detail) = observed
        .first_caveat_detail
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(detail.to_string());
    }
    if parts.is_empty() {
        parts.push(format!(
            "run {} reported critical caveat {}",
            observed.training_run_id,
            observed
                .first_caveat_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("-")
        ));
    }
    Some(parts.join(": "))
}

fn proof_observed_caveats_block_completion(observed: &ProofObservedTrainingRunDetail) -> bool {
    observed.caveat_count != 0 && !proof_observed_first_caveat_is_completion_nonblocking(observed)
}

fn proof_observed_first_caveat_is_completion_nonblocking(
    observed: &ProofObservedTrainingRunDetail,
) -> bool {
    if observed
        .first_caveat_id
        .as_deref()
        .is_some_and(|value| value == "validator_backlog")
    {
        return true;
    }
    observed
        .first_caveat_id
        .as_deref()
        .is_some_and(|value| value == "payout_lag")
        && observed
            .first_caveat_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("0 accepted-work payout(s) need attention"))
}

fn proof_run_status_is_terminal(run: &ProofObservedRunState) -> bool {
    !matches!(
        run.run_status.as_str(),
        "queued" | "preparing" | "running" | "finalizing"
    ) && run.active_window_count == 0
        && run.pending_validation_window_count == 0
        && run.validator_challenges_open == 0
        && run.validator_challenges_queued == 0
}

fn standard_proof_lane_completion_detail(
    fleet: &ProofFleetStatusReport,
    observed: &ProofObservedTrainingRunDetail,
    expected_workers: usize,
    expected_validators: usize,
    allow_continuing_work: bool,
) -> Option<String> {
    if observed.run.active_window_count != 0
        || observed.run.pending_validation_window_count != 0
        || observed.run.validator_challenges_open != 0
        || observed.run.validator_challenges_queued != 0
        || proof_observed_caveats_block_completion(observed)
    {
        return None;
    }
    let reconciled_window = observed.windows.iter().find(|window| {
        window.status == "reconciled"
            && window.accepted_contributions > 0
            && matches!(
                window.closeout_status.as_deref(),
                Some("rewarded" | "paid" | "confirmed" | "settled")
            )
    })?;
    let worker_count =
        proof_completion_node_count(fleet, ProofFleetNodeRole::Worker, allow_continuing_work);
    let validator_count =
        proof_completion_node_count(fleet, ProofFleetNodeRole::Validator, allow_continuing_work);
    if worker_count < expected_workers || validator_count < expected_validators {
        return None;
    }
    let node_state_label = if allow_continuing_work {
        "healthy"
    } else {
        "quiesced"
    };
    Some(format!(
        "window {} reconciled with {} accepted contribution(s), closeout={}, workers_{}={}, validators_{}={}",
        reconciled_window.window_id,
        reconciled_window.accepted_contributions,
        reconciled_window.closeout_status.as_deref().unwrap_or("-"),
        node_state_label,
        worker_count,
        node_state_label,
        validator_count
    ))
}

fn proof_completion_node_count(
    fleet: &ProofFleetStatusReport,
    role: ProofFleetNodeRole,
    allow_continuing_work: bool,
) -> usize {
    if allow_continuing_work {
        proof_healthy_node_count(fleet, role)
    } else {
        proof_quiesced_node_count(fleet, role)
    }
}

fn proof_healthy_node_count(fleet: &ProofFleetStatusReport, role: ProofFleetNodeRole) -> usize {
    fleet
        .nodes
        .iter()
        .filter(|node| {
            node.role == role
                && node.process.running
                && node
                    .training
                    .as_ref()
                    .is_some_and(proof_node_training_is_healthy)
        })
        .count()
}

fn proof_quiesced_node_count(fleet: &ProofFleetStatusReport, role: ProofFleetNodeRole) -> usize {
    fleet
        .nodes
        .iter()
        .filter(|node| {
            node.role == role
                && node.process.running
                && node
                    .training
                    .as_ref()
                    .is_some_and(proof_node_training_is_quiesced)
        })
        .count()
}

fn proof_node_training_is_quiesced(training: &ProofFleetNodeTrainingStatus) -> bool {
    proof_node_training_is_healthy(training)
        && matches!(
            training.active_runtime_process_state.as_deref(),
            Some("stopped") | None
        )
}

fn proof_node_training_is_healthy(training: &ProofFleetNodeTrainingStatus) -> bool {
    training.load_error.is_none()
        && training.last_failure_reason.is_none()
        && training.recent_issue_count == 0
        && training.pending_closeout_count == 0
        && training.current_run_id.is_some()
        && training.active_window_id.is_some()
}

fn proof_psionic_repo_root() -> Option<PathBuf> {
    std::env::var(super::ENV_PSIONIC_REPO)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| {
            let candidate = workspace_root()
                .parent()
                .map(Path::to_path_buf)?
                .join("psionic");
            candidate.exists().then_some(candidate)
        })
}

fn proof_fleet_node_root(layout: &ProofLayout, role: ProofFleetNodeRole, index: usize) -> PathBuf {
    layout
        .fleet_root
        .join(format!("{}-{}", role.label(), index))
}

fn proof_fleet_network_id(namespace: &str) -> String {
    format!("trainnet.proof.{}", namespace_slug(namespace))
}

fn proof_fleet_run_slug(namespace: &str) -> String {
    format!("proof.{}", namespace_slug(namespace))
}

fn generated_proof_namespace(lane: ProofLane) -> String {
    format!("proof.{}.{}", lane.label(), super::now_epoch_ms())
}

fn proof_lane_training_run_id(lane: ProofLane, namespace: &str) -> String {
    format!("{}.{}", lane.run_prefix(), namespace_slug(namespace))
}

fn proof_fixture_path(relative_path: &str) -> PathBuf {
    workspace_root().join(relative_path)
}

fn load_proof_node_runtime_fixture(
    fixture: ProofNodeRuntimeFixture,
    network_id: &str,
) -> Result<super::PylonTrainingRuntimeState> {
    let path = proof_fixture_path(fixture.relative_path());
    let payload = fs::read_to_string(path.as_path())
        .with_context(|| format!("failed to read proof fixture {}", path.display()))?;
    let payload = payload.replace("__PROOF_NETWORK_ID__", network_id);
    serde_json::from_str::<super::PylonTrainingRuntimeState>(payload.as_str())
        .with_context(|| format!("failed to decode proof fixture {}", path.display()))
}

fn apply_proof_node_runtime_fixture(
    config: &super::PylonConfig,
    fixture: ProofNodeRuntimeFixture,
    network_id: &str,
) -> Result<()> {
    let state = load_proof_node_runtime_fixture(fixture, network_id)?;
    super::save_training_runtime_state(config, &state)
}

fn load_proof_replacement_contribution_template() -> Result<ProofReplacementContributionTemplate> {
    let path =
        proof_fixture_path("fixtures/proof/4368/replacement_attempt_contribution_template.json");
    let payload = fs::read_to_string(path.as_path())
        .with_context(|| format!("failed to read proof fixture {}", path.display()))?;
    serde_json::from_str::<ProofReplacementContributionTemplate>(payload.as_str())
        .with_context(|| format!("failed to decode proof fixture {}", path.display()))
}

fn inject_stale_training_runtime_state(
    config: &super::PylonConfig,
    role: ProofFleetNodeRole,
    network_id: &str,
) -> Result<()> {
    let mut state = super::load_or_create_training_runtime_state(config)?;
    let stale_at = super::now_epoch_ms() - 86_400_000;
    let role_label = role.label();
    let training_run_id = format!("run.stale.{role_label}");
    let window_id = format!("window.stale.{role_label}.0001");
    let assignment_id = format!("assign.stale.{role_label}.0001");
    let lease_id = format!("lease.stale.{role_label}.0001");
    state.lease_cache.insert(
        lease_id.clone(),
        super::PylonTrainingLeaseCacheEntry {
            lease_id,
            assignment_id,
            training_run_id: training_run_id.clone(),
            window_id: window_id.clone(),
            membership_revision: "members.rev.stale".to_string(),
            role: role.role_claim(),
            state: "acked".to_string(),
            manifest_digest: Some("sha256:proof-stale".to_string()),
            checkpoint_ref: Some("checkpoint://proof/stale".to_string()),
            expires_at_ms: Some(stale_at - 1_000),
            network_id: Some(network_id.to_string()),
            challenge_id: (role == ProofFleetNodeRole::Validator)
                .then(|| "challenge.stale.validator.0001".to_string()),
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt_path: None,
            validator_target_contribution_receipt_path: None,
            validator_target_contribution_artifact_manifest_path: None,
            validator_target_work_class: None,
            grouped_stage_input_transport_path: None,
            runtime_manifest_path: None,
            runtime_manifest_digest: None,
            runtime_lane_id: None,
            runtime_operation: None,
            runtime_work_class: None,
            updated_at_ms: stale_at,
        },
    );
    state.window_cache.insert(
        window_id.clone(),
        super::PylonTrainingWindowCacheEntry {
            window_id,
            training_run_id,
            state: "sealed".to_string(),
            manifest_digest: Some("sha256:proof-stale".to_string()),
            updated_at_ms: stale_at,
        },
    );
    state.last_authority_sync_at_ms = Some(stale_at);
    super::save_training_runtime_state(config, &state)
}

async fn wait_for_proof_pylons_online(
    authority_base_url: &str,
    expected_pylons: u64,
) -> Result<ProofStatsSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut last_snapshot = None::<ProofStatsSnapshot>;
    while Instant::now() < deadline {
        if let Some(snapshot) = fetch_proof_stats(authority_base_url).await? {
            if snapshot.pylons_online_now >= expected_pylons {
                return Ok(snapshot);
            }
            last_snapshot = Some(snapshot);
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    if let Some(snapshot) = last_snapshot {
        bail!(
            "timed out waiting for proof fleet online count {}; last observed {}",
            expected_pylons,
            snapshot.pylons_online_now
        );
    }
    bail!("timed out waiting for proof fleet stats route to respond")
}

async fn fetch_proof_stats(authority_base_url: &str) -> Result<Option<ProofStatsSnapshot>> {
    let url = format!("{authority_base_url}/api/stats");
    let response = reqwest::Client::new()
        .get(url.as_str())
        .send()
        .await
        .with_context(|| format!("failed to fetch proof stats from {url}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .with_context(|| format!("proof stats route returned failure status for {}", url))?;
    Ok(Some(
        response
            .json::<ProofStatsSnapshot>()
            .await
            .context("failed to decode proof stats snapshot")?,
    ))
}

async fn fetch_proof_stats_value(authority_base_url: &str) -> Result<Option<Value>> {
    let url = format!("{authority_base_url}/api/stats");
    let response = reqwest::Client::new()
        .get(url.as_str())
        .send()
        .await
        .with_context(|| format!("failed to fetch proof stats from {url}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let response = response
        .error_for_status()
        .with_context(|| format!("proof stats route returned failure status for {}", url))?;
    Ok(Some(
        response
            .json::<Value>()
            .await
            .context("failed to decode proof stats value")?,
    ))
}

fn proof_stats_hosted_starter_training_run_id(stats: &Value) -> Option<String> {
    [
        "/training_public_state/active_run_id",
        "/training_public_state/default_run_id",
    ]
    .iter()
    .filter_map(|pointer| stats.pointer(pointer).and_then(Value::as_str))
    .find(|training_run_id| training_run_id.contains("run.cs336.a1.starter."))
    .map(str::to_string)
    .or_else(|| {
        stats
            .pointer("/training_public_state/runs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|run| run.get("training_run_id").and_then(Value::as_str))
            .find(|training_run_id| training_run_id.contains("run.cs336.a1.starter."))
            .map(str::to_string)
    })
}

async fn wait_for_proof_hosted_starter_training_run_id(
    authority_base_url: &str,
    timeout_seconds: u64,
) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds.max(10));
    let mut last_stats = None::<Value>;
    while Instant::now() < deadline {
        if let Some(stats) = fetch_proof_stats_value(authority_base_url).await? {
            if let Some(training_run_id) = proof_stats_hosted_starter_training_run_id(&stats) {
                return Ok(training_run_id);
            }
            last_stats = Some(stats);
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!(
        "timed out waiting for hosted CS336 starter run to auto-launch; last stats={}",
        last_stats
            .map(|stats| stats.to_string())
            .unwrap_or_else(|| "none".to_string())
    )
}

async fn wait_for_proof_training_run_detail(
    authority_base_url: &str,
    training_run_id: &str,
    timeout_seconds: u64,
) -> Result<ProofObservedTrainingRunDetail> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds.max(10));
    while Instant::now() < deadline {
        if let Some(detail) =
            fetch_proof_training_run_detail(authority_base_url, training_run_id).await?
        {
            return Ok(detail);
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for proof training run detail for {training_run_id}")
}

async fn launch_proof_lane(
    lane: ProofLane,
    authority_base_url: &str,
    admin_bearer_token: &str,
    namespace: &str,
    network_id: &str,
    training_run_id: &str,
    first_failed_authority_write: &mut Option<ProofAuthorityWriteFailureCapture>,
) -> Result<ProofRunLaunchResponse> {
    match lane {
        ProofLane::Cs336A1
        | ProofLane::Cs336A1HostedStarter
        | ProofLane::Cs336A1StaleRecovery
        | ProofLane::Cs336A1ReplacementAttempt => {
            let url = format!("{authority_base_url}/v1/admin/training/demo-runs/cs336-a1/launch");
            let response = reqwest::Client::new()
                .post(url.as_str())
                .bearer_auth(admin_bearer_token)
                .json(&json!({
                    "training_run_id": training_run_id,
                    "display_name": format!("{} {}", lane.display_name_prefix(), namespace_slug(namespace)),
                    "network_id": network_id,
                    "reuse_existing_run": false,
                }))
                .send()
                .await
                .with_context(|| format!("failed to launch proof lane via {url}"))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .context("failed to read proof run launch response body")?;
            if !status.is_success() {
                first_failed_authority_write.get_or_insert_with(|| {
                    ProofAuthorityWriteFailureCapture {
                        source: "proof_run_launch".to_string(),
                        observed_at_ms: super::now_epoch_ms(),
                        method: Some("POST".to_string()),
                        url: Some(url.clone()),
                        status: Some(status.as_u16()),
                        response_body: Some(body.clone()),
                        detail: format!("proof lane launch failed for namespace {namespace}"),
                    }
                });
                bail!(
                    "proof lane launch failed for namespace {namespace} with status {}: {}",
                    status.as_u16(),
                    body.trim()
                );
            }
            serde_json::from_str::<ProofRunLaunchResponse>(body.as_str())
                .context("failed to decode proof run launch response")
        }
        ProofLane::A1MinimalDistributedLmLaunchA | ProofLane::A1MinimalDistributedLmLaunchB => {
            bail!("A1 minimal proof lanes use the local simulated projection path")
        }
    }
}

async fn fetch_proof_training_run_detail(
    authority_base_url: &str,
    training_run_id: &str,
) -> Result<Option<ProofObservedTrainingRunDetail>> {
    let url = format!("{authority_base_url}/api/training/runs/{training_run_id}?refresh=true");
    let response = reqwest::Client::new()
        .get(url.as_str())
        .send()
        .await
        .with_context(|| format!("failed to fetch proof training run detail from {url}"))?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND || proof_training_run_detail_status_is_retryable(status) {
        return Ok(None);
    }
    let response = response.error_for_status().with_context(|| {
        format!(
            "proof training run detail route returned failure status for {}",
            url
        )
    })?;
    let detail = response
        .json::<ProofAuthorityTrainingRunDetailResponse>()
        .await
        .context("failed to decode proof training run detail response")?;
    Ok(Some(summarize_training_run_detail_response(&detail)))
}

fn proof_training_run_detail_status_is_retryable(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn summarize_training_run_detail_response(
    detail: &ProofAuthorityTrainingRunDetailResponse,
) -> ProofObservedTrainingRunDetail {
    let first_caveat = detail.caveats.first();
    ProofObservedTrainingRunDetail {
        training_run_id: detail.training_run_id.clone(),
        run: detail.run.clone(),
        windows: detail.windows.clone(),
        contribution_count: detail.contributions.len(),
        node_count: detail.nodes.len(),
        caveat_count: detail.caveats.len(),
        first_caveat_id: extract_caveat_string(first_caveat, "caveat_id"),
        first_caveat_severity: extract_caveat_string(first_caveat, "severity"),
        first_caveat_title: extract_caveat_string(first_caveat, "title"),
        first_caveat_detail: extract_caveat_string(first_caveat, "detail"),
    }
}

fn extract_caveat_string(caveat: Option<&Value>, key: &str) -> Option<String> {
    caveat?
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn save_fleet_launch_record(
    config_path: &Path,
    namespace: &str,
    launch: &ProofRunLaunchResponse,
) -> Result<()> {
    let layout = proof_layout(config_path, namespace);
    let mut state = load_fleet_state(layout.fleet_state_path.as_path())?
        .ok_or_else(|| anyhow!("proof fleet state missing for namespace {namespace}"))?;
    state.launched_run = Some(launch.clone());
    save_fleet_state(layout.fleet_state_path.as_path(), &state)
}

fn load_fleet_state(path: &Path) -> Result<Option<ProofFleetRuntimeState>> {
    if !path.exists() {
        return Ok(None);
    }
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read proof fleet state {}", path.display()))?;
    let state = serde_json::from_str::<ProofFleetRuntimeState>(payload.as_str())
        .with_context(|| format!("failed to parse proof fleet state {}", path.display()))?;
    Ok(Some(state))
}

fn save_fleet_state(path: &Path, state: &ProofFleetRuntimeState) -> Result<()> {
    save_json_file_atomic(path, state, "proof fleet state")
}

fn save_proof_run_report(path: &Path, report: &ProofRunReport) -> Result<()> {
    save_json_file_atomic(path, report, "proof run report")
}

fn save_proof_trace_artifact(path: &Path, trace: &ProofTraceArtifact) -> Result<()> {
    save_json_file_atomic(path, trace, "proof trace artifact")
}

fn save_proof_summary_artifact(path: &Path, summary: &ProofSummaryArtifact) -> Result<()> {
    save_json_file_atomic(path, summary, "proof summary artifact")
}

async fn persist_proof_run_outputs(config_path: &Path, report: &ProofRunReport) -> Result<()> {
    let layout = proof_layout(config_path, report.namespace.as_str());
    save_proof_run_report(layout.run_report_path.as_path(), report)?;
    let trace = collect_proof_trace_artifact(config_path, report).await?;
    save_proof_trace_artifact(layout.trace_path.as_path(), &trace)?;
    let summary = build_proof_summary_artifact(&trace);
    save_proof_summary_artifact(layout.summary_path.as_path(), &summary)?;
    Ok(())
}

async fn collect_proof_trace_artifact(
    config_path: &Path,
    report: &ProofRunReport,
) -> Result<ProofTraceArtifact> {
    let layout = proof_layout(config_path, report.namespace.as_str());
    let node_traces = collect_proof_trace_nodes(&report.fleet)?;
    let training_run_id = report
        .observed_run
        .as_ref()
        .map(|value| value.training_run_id.as_str())
        .or_else(|| {
            report
                .launch
                .as_ref()
                .map(|value| value.training_run_id.as_str())
        });
    let transport =
        collect_proof_transport_split_view(&report.fleet, training_run_id, node_traces.as_slice())
            .await?;
    let artifact_transport =
        load_artifact_trace_snapshot(layout.artifact_trace_path.as_path(), 12)?;
    Ok(ProofTraceArtifact {
        schema_version: "openagents.proof.trace.v1".to_string(),
        namespace: report.namespace.clone(),
        lane: report.lane.clone(),
        generated_at_ms: super::now_epoch_ms(),
        status: report.status.clone(),
        detail: report.detail.clone(),
        blocker_id: report.blocker_id.clone(),
        fleet: report.fleet.clone(),
        launch: report.launch.clone(),
        observed_run: report.observed_run.clone(),
        first_failed_authority_write: report
            .first_failed_authority_write
            .clone()
            .or_else(|| infer_first_authority_write_failure(node_traces.as_slice())),
        a1_minimal_projection: report.a1_minimal_projection.clone(),
        artifact_transport,
        transport,
        node_traces,
    })
}

fn collect_proof_trace_nodes(fleet: &ProofFleetStatusReport) -> Result<Vec<ProofTraceNode>> {
    let mut traces = Vec::with_capacity(fleet.nodes.len());
    for node in &fleet.nodes {
        let config_path = Path::new(node.config_path.as_str());
        match super::load_training_status_report_local(config_path) {
            Ok(training_status) => traces.push(ProofTraceNode {
                role: node.role,
                index: node.index,
                node_label: node.node_label.clone(),
                payout_destination: node.payout_destination.clone(),
                retained_state_fixture_id: node.retained_state_fixture_id.clone(),
                eligibility: proof_node_eligibility(&training_status),
                training_status: Some(training_status),
                training_status_error: None,
            }),
            Err(error) => traces.push(ProofTraceNode {
                role: node.role,
                index: node.index,
                node_label: node.node_label.clone(),
                payout_destination: node.payout_destination.clone(),
                retained_state_fixture_id: node.retained_state_fixture_id.clone(),
                eligibility: ProofNodeEligibilitySnapshot {
                    eligibility: "unknown".to_string(),
                    hard_gate_reasons: Vec::new(),
                },
                training_status: None,
                training_status_error: Some(format!("{error:#}")),
            }),
        }
    }
    Ok(traces)
}

fn proof_node_eligibility(
    status: &super::TrainingOperatorStatusReport,
) -> ProofNodeEligibilitySnapshot {
    let hard_gate_reasons = status.blocked_label_keys.clone();
    let eligibility = if !hard_gate_reasons.is_empty() {
        "hard_gated"
    } else if status.contributor_supported {
        "eligible"
    } else {
        "unsupported"
    };
    ProofNodeEligibilitySnapshot {
        eligibility: eligibility.to_string(),
        hard_gate_reasons,
    }
}

async fn collect_proof_transport_split_view(
    fleet: &ProofFleetStatusReport,
    training_run_id: Option<&str>,
    node_traces: &[ProofTraceNode],
) -> Result<ProofTransportSplitView> {
    let client = reqwest::Client::new();
    let mut authority_front_door = Vec::new();
    let mut artifact_store = Vec::new();
    if let Some(urls) = fleet.authority.urls.as_ref() {
        authority_front_door.push(
            probe_route(
                &client,
                "authority_healthz",
                format!("{}/healthz", urls.authority_base_url.trim_end_matches('/')),
                reqwest::Method::GET,
                &[StatusCode::OK],
            )
            .await,
        );
        authority_front_door.push(
            probe_route(
                &client,
                "authority_stats",
                format!(
                    "{}/api/stats",
                    urls.authority_base_url.trim_end_matches('/')
                ),
                reqwest::Method::GET,
                &[StatusCode::OK],
            )
            .await,
        );
        authority_front_door.push(
            probe_route(
                &client,
                "authority_demo_launch_route",
                format!(
                    "{}/v1/admin/training/demo-runs/cs336-a1/launch",
                    urls.authority_base_url.trim_end_matches('/')
                ),
                reqwest::Method::GET,
                &[StatusCode::METHOD_NOT_ALLOWED, StatusCode::UNAUTHORIZED],
            )
            .await,
        );
        if let Some(training_run_id) = training_run_id {
            authority_front_door.push(
                probe_route(
                    &client,
                    "authority_training_run_detail",
                    format!(
                        "{}/api/training/runs/{training_run_id}",
                        urls.authority_base_url.trim_end_matches('/')
                    ),
                    reqwest::Method::GET,
                    &[StatusCode::OK, StatusCode::NOT_FOUND],
                )
                .await,
            );
        }
        artifact_store.push(
            probe_route(
                &client,
                "artifact_store_healthz",
                format!(
                    "{}/healthz",
                    urls.artifact_store_base_url.trim_end_matches("/upload")
                ),
                reqwest::Method::GET,
                &[StatusCode::OK],
            )
            .await,
        );
    }

    let relay = ProofRelayTransportView {
        relay_ws_url: fleet
            .authority
            .urls
            .as_ref()
            .and_then(|value| value.relay_ws_url.clone()),
        relay_data_dir: fleet
            .authority
            .paths
            .as_ref()
            .map(|value| value.relay_data_dir.clone()),
        authority_running: fleet
            .authority
            .authority_process
            .as_ref()
            .map(|value| value.running)
            .unwrap_or(false),
        detail: if fleet
            .authority
            .authority_process
            .as_ref()
            .map(|value| value.running)
            .unwrap_or(false)
        {
            "relay-facing authority process is running".to_string()
        } else {
            "relay-facing authority process is not running".to_string()
        },
    };

    let mut node_surfaces = Vec::with_capacity(fleet.nodes.len());
    for (node, trace) in fleet.nodes.iter().zip(node_traces.iter()) {
        let admin = probe_route(
            &client,
            format!("{}_{}_admin", node.role.label(), node.index).as_str(),
            format!(
                "{}/v1/training/status",
                node.admin_url.trim_end_matches('/')
            ),
            reqwest::Method::GET,
            &[StatusCode::OK],
        )
        .await;
        let checkpoint = probe_route(
            &client,
            format!("{}_{}_checkpoint", node.role.label(), node.index).as_str(),
            node_checkpoint_probe_url(node, trace.training_status.as_ref(), training_run_id),
            reqwest::Method::GET,
            &[StatusCode::OK, StatusCode::NOT_FOUND],
        )
        .await;
        node_surfaces.push(ProofNodeTransportView {
            role: node.role,
            index: node.index,
            node_label: node.node_label.clone(),
            admin,
            checkpoint,
        });
    }

    Ok(ProofTransportSplitView {
        authority_front_door,
        artifact_store,
        relay,
        node_surfaces,
    })
}

fn node_checkpoint_probe_url(
    node: &ProofFleetNodeStatus,
    training_status: Option<&super::TrainingOperatorStatusReport>,
    fallback_training_run_id: Option<&str>,
) -> String {
    if let Some(status) = training_status {
        if let Some(run_id) = status.current_run_id.as_deref() {
            return format!(
                "{}/runs/{run_id}/checkpoints/latest_pointer.json",
                node.checkpoint_serve_url.trim_end_matches('/')
            );
        }
    }
    if let Some(run_id) = fallback_training_run_id {
        return format!(
            "{}/runs/{run_id}/checkpoints/latest_pointer.json",
            node.checkpoint_serve_url.trim_end_matches('/')
        );
    }
    format!("{}/", node.checkpoint_serve_url.trim_end_matches('/'))
}

fn load_artifact_trace_snapshot(
    path: &Path,
    recent_limit: usize,
) -> Result<ProofArtifactTraceSnapshot> {
    if !path.is_file() {
        return Ok(ProofArtifactTraceSnapshot {
            trace_path: path.display().to_string(),
            entry_count: 0,
            recent_entries: Vec::new(),
        });
    }
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read proof artifact trace {}", path.display()))?;
    let mut entries = Vec::new();
    for line in payload.lines().filter(|line| !line.trim().is_empty()) {
        entries.push(
            serde_json::from_str::<ProofArtifactTraceEntry>(line).with_context(|| {
                format!(
                    "failed to decode proof artifact trace line in {}",
                    path.display()
                )
            })?,
        );
    }
    let keep_from = entries.len().saturating_sub(recent_limit);
    Ok(ProofArtifactTraceSnapshot {
        trace_path: path.display().to_string(),
        entry_count: entries.len(),
        recent_entries: entries.into_iter().skip(keep_from).collect(),
    })
}

fn infer_first_authority_write_failure(
    node_traces: &[ProofTraceNode],
) -> Option<ProofAuthorityWriteFailureCapture> {
    for node in node_traces {
        let Some(status) = node.training_status.as_ref() else {
            continue;
        };
        for issue in &status.recent_issues {
            if let Some((status_code, response_body)) =
                parse_status_body_from_reason(issue.reason.as_str())
            {
                return Some(ProofAuthorityWriteFailureCapture {
                    source: format!("{}_{}_issue", node.role.label(), node.index),
                    observed_at_ms: issue.observed_at_ms,
                    method: None,
                    url: None,
                    status: Some(status_code),
                    response_body: Some(response_body),
                    detail: issue.reason.clone(),
                });
            }
        }
        if let Some(active_runtime) = status.active_runtime.as_ref() {
            if let Some(reason) = active_runtime.last_failure_reason.as_deref() {
                if let Some((status_code, response_body)) = parse_status_body_from_reason(reason) {
                    return Some(ProofAuthorityWriteFailureCapture {
                        source: format!("{}_{}_runtime", node.role.label(), node.index),
                        observed_at_ms: active_runtime.updated_at_ms,
                        method: None,
                        url: None,
                        status: Some(status_code),
                        response_body: Some(response_body),
                        detail: reason.to_string(),
                    });
                }
            }
        }
    }
    None
}

fn parse_status_body_from_reason(reason: &str) -> Option<(u16, String)> {
    let (_, tail) = reason.split_once("failed with status ")?;
    let (status, body) = tail.split_once(':')?;
    let status = status.trim().parse::<u16>().ok()?;
    let body = body.trim();
    (!body.is_empty()).then(|| (status, body.to_string()))
}

#[derive(Clone, Debug)]
struct ProofSummarySignal {
    first_red_stage: String,
    first_red_subject: String,
    window_id: Option<String>,
    assignment_id: Option<String>,
    lease_id: Option<String>,
    membership_revision: Option<String>,
    closeout_stage: Option<String>,
    closeout_next_action: Option<String>,
    closeout_last_error: Option<String>,
}

fn build_proof_summary_artifact(trace: &ProofTraceArtifact) -> ProofSummaryArtifact {
    let signal = derive_proof_summary_signal(trace);
    let trace_path = trace
        .fleet
        .paths
        .as_ref()
        .map(|value| value.trace_path.clone())
        .unwrap_or_else(|| "-".to_string());
    ProofSummaryArtifact {
        schema_version: "openagents.proof.summary.v1".to_string(),
        namespace: trace.namespace.clone(),
        lane: trace.lane.clone(),
        generated_at_ms: trace.generated_at_ms,
        status: trace.status.clone(),
        detail: trace.detail.clone(),
        blocker_id: trace.blocker_id.clone(),
        first_red_stage: signal.first_red_stage,
        first_red_subject: signal.first_red_subject,
        window_id: signal.window_id,
        assignment_id: signal.assignment_id,
        lease_id: signal.lease_id,
        membership_revision: signal.membership_revision,
        closeout_stage: signal.closeout_stage,
        closeout_next_action: signal.closeout_next_action,
        closeout_last_error: signal.closeout_last_error,
        first_failed_authority_write: trace.first_failed_authority_write.clone(),
        a1_minimal_projection: trace.a1_minimal_projection.clone(),
        trace_path,
        transport: trace.transport.clone(),
    }
}

fn derive_proof_summary_signal(trace: &ProofTraceArtifact) -> ProofSummarySignal {
    match trace.blocker_id.as_deref() {
        Some("fleet_node_issue") | Some("fleet_node_failure") => {
            if let Some(signal) = derive_node_issue_signal(trace.node_traces.as_slice()) {
                return signal;
            }
        }
        Some("authority_run_caveat") => {
            return ProofSummarySignal {
                first_red_stage: "authority_caveat".to_string(),
                first_red_subject: trace
                    .observed_run
                    .as_ref()
                    .map(|value| value.training_run_id.clone())
                    .unwrap_or_else(|| trace.namespace.clone()),
                window_id: trace
                    .observed_run
                    .as_ref()
                    .map(|value| value.run.current_window_id.clone())
                    .filter(|value| !value.is_empty()),
                assignment_id: None,
                lease_id: None,
                membership_revision: None,
                closeout_stage: None,
                closeout_next_action: None,
                closeout_last_error: trace
                    .observed_run
                    .as_ref()
                    .and_then(|value| value.first_caveat_detail.clone()),
            };
        }
        Some("authority_probe_failed") => {
            return ProofSummarySignal {
                first_red_stage: "authority_probe".to_string(),
                first_red_subject: trace
                    .transport
                    .authority_front_door
                    .iter()
                    .find(|probe| !probe.ok)
                    .map(|probe| probe.route_id.clone())
                    .unwrap_or_else(|| "authority_probe".to_string()),
                window_id: None,
                assignment_id: None,
                lease_id: None,
                membership_revision: None,
                closeout_stage: None,
                closeout_next_action: None,
                closeout_last_error: None,
            };
        }
        Some("proof_run_timeout") => {
            return ProofSummarySignal {
                first_red_stage: "proof_run_timeout".to_string(),
                first_red_subject: trace.namespace.clone(),
                window_id: trace
                    .observed_run
                    .as_ref()
                    .map(|value| value.run.current_window_id.clone())
                    .filter(|value| !value.is_empty()),
                assignment_id: None,
                lease_id: None,
                membership_revision: None,
                closeout_stage: None,
                closeout_next_action: None,
                closeout_last_error: None,
            };
        }
        _ => {}
    }
    if trace.status == "terminal" {
        return ProofSummarySignal {
            first_red_stage: "terminal".to_string(),
            first_red_subject: trace
                .observed_run
                .as_ref()
                .map(|value| value.training_run_id.clone())
                .unwrap_or_else(|| trace.namespace.clone()),
            window_id: trace
                .observed_run
                .as_ref()
                .map(|value| value.run.current_window_id.clone())
                .filter(|value| !value.is_empty()),
            assignment_id: None,
            lease_id: None,
            membership_revision: None,
            closeout_stage: None,
            closeout_next_action: None,
            closeout_last_error: None,
        };
    }
    ProofSummarySignal {
        first_red_stage: trace
            .blocker_id
            .clone()
            .unwrap_or_else(|| trace.status.clone()),
        first_red_subject: trace.namespace.clone(),
        window_id: trace
            .observed_run
            .as_ref()
            .map(|value| value.run.current_window_id.clone())
            .filter(|value| !value.is_empty()),
        assignment_id: None,
        lease_id: None,
        membership_revision: None,
        closeout_stage: None,
        closeout_next_action: None,
        closeout_last_error: None,
    }
}

fn derive_node_issue_signal(node_traces: &[ProofTraceNode]) -> Option<ProofSummarySignal> {
    for node in node_traces {
        let status = node.training_status.as_ref()?;
        let active_runtime = status.active_runtime.as_ref();
        let leased_assignment = status.leased_assignment.as_ref();
        let current_window = status.current_window.as_ref();
        if let Some(progress) = status.recent_closeout_progress.first() {
            return Some(ProofSummarySignal {
                first_red_stage: format!("{}_closeout_{}", node.role.label(), progress.stage),
                first_red_subject: node.node_label.clone(),
                window_id: Some(progress.window_id.clone()),
                assignment_id: Some(progress.assignment_id.clone()),
                lease_id: active_runtime.map(|value| value.lease_id.clone()),
                membership_revision: active_runtime.map(|value| value.membership_revision.clone()),
                closeout_stage: Some(progress.stage.clone()),
                closeout_next_action: progress.next_action.clone(),
                closeout_last_error: progress.last_error.clone(),
            });
        }
        if !status.recent_issues.is_empty()
            || active_runtime
                .and_then(|value| value.last_failure_reason.as_ref())
                .is_some()
        {
            return Some(ProofSummarySignal {
                first_red_stage: format!("{}_issue", node.role.label()),
                first_red_subject: node.node_label.clone(),
                window_id: active_runtime
                    .map(|value| value.window_id.clone())
                    .or_else(|| current_window.map(|value| value.window_id.clone())),
                assignment_id: active_runtime
                    .map(|value| value.assignment_id.clone())
                    .or_else(|| leased_assignment.map(|value| value.assignment_id.clone())),
                lease_id: active_runtime
                    .map(|value| value.lease_id.clone())
                    .or_else(|| leased_assignment.map(|value| value.lease_id.clone())),
                membership_revision: active_runtime
                    .map(|value| value.membership_revision.clone())
                    .or_else(|| leased_assignment.map(|value| value.membership_revision.clone())),
                closeout_stage: None,
                closeout_next_action: None,
                closeout_last_error: status
                    .recent_issues
                    .first()
                    .map(|value| value.reason.clone())
                    .or_else(|| active_runtime.and_then(|value| value.last_failure_reason.clone())),
            });
        }
    }
    None
}

async fn collect_proof_doctor_report(
    config_path: &Path,
    namespace: &str,
) -> Result<ProofDoctorReport> {
    let layout = proof_layout(config_path, namespace);
    let fleet = collect_proof_fleet_status(config_path, namespace).await?;
    let node_traces = collect_proof_trace_nodes(&fleet)?;
    let training_run_id = fleet
        .launched_run
        .as_ref()
        .map(|value| value.training_run_id.as_str());
    let transport =
        collect_proof_transport_split_view(&fleet, training_run_id, node_traces.as_slice()).await?;
    let artifact_transport =
        load_artifact_trace_snapshot(layout.artifact_trace_path.as_path(), 12)?;
    let git = collect_git_provenance();
    let current_executable = current_executable_path()?;
    let current_executable_report = build_process_provenance(
        "oa_current_executable",
        current_executable.as_path(),
        Some(current_executable.as_path()),
        None,
        current_executable.is_file(),
        Vec::new(),
    )?;

    let mut supporting_binaries = Vec::new();
    for (binary, _package) in [
        ("nexus-relay", "nexus-relay"),
        ("nexus-control", "nexus-control"),
    ] {
        let expected_binary =
            locate_workspace_binary(binary)?.unwrap_or(expected_workspace_binary_path(binary)?);
        supporting_binaries.push(build_process_provenance(
            binary,
            expected_binary.as_path(),
            None,
            None,
            false,
            Vec::new(),
        )?);
    }

    let authority_state = load_runtime_state(layout.runtime_state_path.as_path())?;
    let authority_env_file = if layout.authority_env_path.is_file() {
        Some(load_authority_env_file_report(
            layout.authority_env_path.as_path(),
        )?)
    } else {
        None
    };
    let psionic_repo_root = load_fleet_state(layout.fleet_state_path.as_path())?
        .and_then(|value| value.psionic_repo_root);

    let mut process_provenance = Vec::new();
    if let Some(process) = fleet.authority.authority_process.as_ref() {
        let expected_binary = fleet
            .authority
            .mode
            .and_then(|mode| {
                locate_workspace_binary(mode.authority_binary())
                    .ok()
                    .flatten()
                    .or_else(|| expected_workspace_binary_path(mode.authority_binary()).ok())
            })
            .unwrap_or_else(|| PathBuf::from(process.binary.as_str()));
        process_provenance.push(build_process_provenance(
            "authority",
            expected_binary.as_path(),
            Some(Path::new(process.binary.as_str())),
            process.pid,
            process.running,
            Vec::new(),
        )?);
    }
    if let Some(process) = fleet.authority.artifact_store_process.as_ref() {
        process_provenance.push(build_process_provenance(
            "artifact_store",
            current_executable.as_path(),
            Some(Path::new(process.binary.as_str())),
            process.pid,
            process.running,
            Vec::new(),
        )?);
    }
    if let Some(authority_state) = authority_state.as_ref() {
        for node in &fleet.nodes {
            let env_expectations = expected_node_env_expectations(
                node,
                authority_state,
                fleet
                    .authority
                    .urls
                    .as_ref()
                    .map(|value| value.artifact_store_base_url.as_str()),
                psionic_repo_root.as_deref(),
            );
            process_provenance.push(build_process_provenance(
                format!("{}_{}", node.role.label(), node.index).as_str(),
                current_executable.as_path(),
                Some(Path::new(node.process.binary.as_str())),
                node.process.pid,
                node.process.running,
                env_expectations,
            )?);
        }
    }

    Ok(ProofDoctorReport {
        configured: fleet.configured || fleet.authority.configured,
        namespace: namespace.to_string(),
        generated_at_ms: super::now_epoch_ms(),
        fleet,
        transport,
        artifact_transport,
        git,
        authority_env_file,
        current_executable: current_executable_report,
        supporting_binaries,
        process_provenance,
        latest_trace_path: layout
            .trace_path
            .is_file()
            .then(|| layout.trace_path.display().to_string()),
        latest_summary_path: layout
            .summary_path
            .is_file()
            .then(|| layout.summary_path.display().to_string()),
    })
}

fn collect_git_provenance() -> ProofGitProvenance {
    ProofGitProvenance {
        workspace_root: workspace_root().display().to_string(),
        branch: run_git_capture(["branch", "--show-current"].as_slice()),
        commit: run_git_capture(["rev-parse", "HEAD"].as_slice()),
    }
}

fn run_git_capture(args: &[&str]) -> Option<String> {
    let output = StdCommand::new("git")
        .current_dir(workspace_root())
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn load_authority_env_file_report(path: &Path) -> Result<ProofAuthorityEnvFileReport> {
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read authority env file {}", path.display()))?;
    let mut keys = payload
        .lines()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim().to_string()))
        .filter(|key| !key.is_empty())
        .collect::<Vec<_>>();
    keys.sort();
    Ok(ProofAuthorityEnvFileReport {
        path: path.display().to_string(),
        key_count: keys.len(),
        keys,
    })
}

fn build_process_provenance(
    component_id: &str,
    expected_binary_path: &Path,
    running_binary_path: Option<&Path>,
    pid: Option<u32>,
    running: bool,
    env_expectations: Vec<(String, Option<String>)>,
) -> Result<ProofProcessProvenance> {
    let command_line = pid.and_then(read_process_command_line);
    let env_expectations =
        build_process_env_expectations(command_line.as_deref(), env_expectations);
    let expected_binary_digest = file_digest(expected_binary_path)?;
    let running_binary_digest = match running_binary_path {
        Some(path) => file_digest(path)?,
        None => None,
    };
    Ok(ProofProcessProvenance {
        component_id: component_id.to_string(),
        expected_binary_path: expected_binary_path.display().to_string(),
        expected_binary_digest,
        running_binary_path: running_binary_path.map(|path| path.display().to_string()),
        running_binary_digest,
        binary_matches_expected: running_binary_path.map(|path| path == expected_binary_path),
        pid,
        running,
        env_expectations,
    })
}

fn file_digest(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let payload =
        fs::read(path).with_context(|| format!("failed to read binary {}", path.display()))?;
    Ok(Some(sha256_prefixed_bytes(payload.as_slice())))
}

fn read_process_command_line(pid: u32) -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let pid_string = pid.to_string();
    let output = StdCommand::new("ps")
        .args(["eww", "-p", pid_string.as_str()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("PID") && !trimmed.starts_with("COMMAND")
        })
        .map(ToOwned::to_owned)
}

fn build_process_env_expectations(
    command_line: Option<&str>,
    expected: Vec<(String, Option<String>)>,
) -> Vec<ProofProcessEnvExpectation> {
    expected
        .into_iter()
        .map(|(key, expected_value)| {
            let process_present =
                command_line.map(|line| line.contains(format!("{key}=").as_str()));
            let matches_expected = match (command_line, expected_value) {
                (Some(line), Some(value)) => Some(line.contains(format!("{key}={value}").as_str())),
                (Some(line), None) => Some(line.contains(format!("{key}=").as_str())),
                (None, _) => None,
            };
            ProofProcessEnvExpectation {
                key,
                expected_present: true,
                process_present,
                matches_expected,
            }
        })
        .collect()
}

fn expected_node_env_expectations(
    node: &ProofFleetNodeStatus,
    authority_state: &ProofAuthorityRuntimeState,
    artifact_store_base_url: Option<&str>,
    psionic_repo_root: Option<&str>,
) -> Vec<(String, Option<String>)> {
    let mut expectations = vec![
        (
            super::ENV_PYLON_HOME.to_string(),
            Some(node.home_dir.clone()),
        ),
        (
            super::ENV_TRAINING_NEXUS_BEARER_TOKEN.to_string(),
            Some(authority_state.admin_bearer_token.clone()),
        ),
        (
            super::ENV_TRAINING_GCS_ENDPOINT.to_string(),
            artifact_store_base_url.map(ToOwned::to_owned),
        ),
        (
            super::ENV_TRAINING_GCS_BEARER_TOKEN.to_string(),
            Some("proof-local-artifact-store-token".to_string()),
        ),
    ];
    if let Some(psionic_repo_root) = psionic_repo_root {
        expectations.push((
            super::ENV_PSIONIC_REPO.to_string(),
            Some(psionic_repo_root.to_string()),
        ));
    }
    expectations
}

fn save_json_file_atomic<T: Serialize>(path: &Path, value: &T, label: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {} dir {}", label, parent.display()))?;
    }
    let temp_path = path.with_extension("tmp");
    fs::write(
        temp_path.as_path(),
        format!(
            "{}\n",
            serde_json::to_string_pretty(value)
                .with_context(|| format!("failed to encode {label}"))?
        ),
    )
    .with_context(|| format!("failed to write {label} {}", temp_path.display()))?;
    fs::rename(temp_path.as_path(), path)
        .with_context(|| format!("failed to finalize {label} {}", path.display()))?;
    Ok(())
}

async fn stop_runtime_processes(state: &ProofAuthorityRuntimeState) -> Result<()> {
    if let Some(pid) = state.authority_process.pid {
        stop_pid(pid).await?;
    }
    if let Some(pid) = state.artifact_store_process.pid {
        stop_pid(pid).await?;
    }
    Ok(())
}

async fn stop_pid(pid: u32) -> Result<()> {
    terminate_pid(pid, false)?;
    if let Err(error) = wait_for_pid_exit(pid).await {
        if super::training_supervisor_pid_is_running(pid) {
            terminate_pid(pid, true)?;
            wait_for_pid_exit(pid).await?;
        } else {
            return Err(error);
        }
    }
    Ok(())
}

async fn wait_for_pid_exit(pid: u32) -> Result<()> {
    let deadline = Instant::now() + PROOF_ROUTE_TIMEOUT;
    while Instant::now() < deadline {
        if !super::training_supervisor_pid_is_running(pid) {
            return Ok(());
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for pid {pid} to exit")
}

fn process_is_running(process: &ProofProcessRecord) -> bool {
    process
        .pid
        .is_some_and(super::training_supervisor_pid_is_running)
}

fn current_executable_path() -> Result<PathBuf> {
    std::env::current_exe().context("failed to resolve current executable")
}

fn resolve_workspace_binary(binary: &str, package: &str) -> Result<PathBuf> {
    let status = StdCommand::new("cargo")
        .current_dir(workspace_root())
        .args(["build", "-p", package, "--bin", binary])
        .status()
        .with_context(|| format!("failed to build supporting binary `{binary}`"))?;
    if !status.success() {
        bail!("cargo build failed for supporting binary `{binary}`");
    }
    locate_workspace_binary(binary)?
        .ok_or_else(|| anyhow!("supporting binary `{binary}` was not produced in target/debug"))
}

fn locate_workspace_binary(binary: &str) -> Result<Option<PathBuf>> {
    Ok(workspace_binary_candidates(binary)?
        .into_iter()
        .find(|candidate| candidate.is_file()))
}

fn expected_workspace_binary_path(binary: &str) -> Result<PathBuf> {
    let executable = platform_binary_name(binary);
    Ok(workspace_root()
        .join("target")
        .join("debug")
        .join(executable))
}

fn workspace_binary_candidates(binary: &str) -> Result<Vec<PathBuf>> {
    let executable = platform_binary_name(binary);
    let current_exe = current_executable_path()?;
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.parent() {
        candidates.push(parent.join(executable.as_str()));
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join(executable.as_str()));
        }
    }
    candidates.push(expected_workspace_binary_path(binary)?);
    Ok(candidates)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn platform_binary_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn spawn_logged_process(
    binary: &Path,
    args: &[String],
    envs: &[(String, String)],
    log_path: &Path,
) -> Result<u32> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log dir {}", parent.display()))?;
    }
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open log file {}", log_path.display()))?;
    let stderr_file = log_file
        .try_clone()
        .with_context(|| format!("failed to clone log file {}", log_path.display()))?;
    let mut command = StdCommand::new(binary);
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Proof services must survive the short-lived CLI process that launched
        // them, so move them into a fresh session before spawn.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("failed to spawn {}", binary.display()))?;
    Ok(child.id())
}

fn terminate_pid(pid: u32, force: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let pid_text = pid.to_string();
        let signal = if force { "-KILL" } else { "-TERM" };
        let status = StdCommand::new("kill")
            .args([signal, pid_text.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("failed to invoke kill {signal}"))?;
        if !status.success() && super::training_supervisor_pid_is_running(pid) {
            bail!("kill {signal} exited unsuccessfully for pid {pid}");
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let mut args = vec!["/PID", &pid.to_string(), "/T"];
        if force {
            args.push("/F");
        }
        let status = StdCommand::new("taskkill")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to invoke taskkill")?;
        if !status.success() {
            bail!("taskkill exited unsuccessfully for pid {pid}");
        }
        Ok(())
    }
}

fn ensure_layout_dirs(layout: &ProofLayout) -> Result<()> {
    for path in [
        layout.namespace_root.as_path(),
        layout.fleet_root.as_path(),
        layout.relay_data_dir.as_path(),
        layout.artifact_store_root.as_path(),
        layout
            .authority_log_path
            .parent()
            .ok_or_else(|| anyhow!("authority log path missing parent"))?,
        layout
            .kernel_state_path
            .parent()
            .ok_or_else(|| anyhow!("kernel state path missing parent"))?,
    ] {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn proof_layout(config_path: &Path, namespace: &str) -> ProofLayout {
    let base_root = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(super::default_home_dir);
    let namespace_root = base_root.join("proof").join("namespaces").join(namespace);
    ProofLayout {
        authority_env_path: namespace_root.join("authority").join("authority.env"),
        fleet_root: namespace_root.join("fleet"),
        relay_data_dir: namespace_root.join("authority").join("relay-data"),
        receipt_log_path: namespace_root.join("state").join("receipts.jsonl"),
        kernel_state_path: namespace_root.join("state").join("kernel-state.json"),
        treasury_state_path: namespace_root.join("state").join("treasury-state.json"),
        treasury_wallet_dir: namespace_root.join("state").join("treasury-wallet"),
        treasury_wallet_mnemonic_path: namespace_root.join("state").join("treasury.mnemonic"),
        training_trn_identity_path: namespace_root
            .join("state")
            .join("training-trn-identity.mnemonic"),
        signer_credentials_path: namespace_root.join("artifacts").join("gcs-signer.json"),
        artifact_store_root: namespace_root.join("artifacts").join("store"),
        artifact_trace_path: namespace_root.join("artifacts").join("object-trace.jsonl"),
        runtime_state_path: namespace_root.join("runtime-state.json"),
        fleet_state_path: namespace_root.join("fleet").join("fleet-state.json"),
        run_report_path: namespace_root.join("fleet").join("run-report.json"),
        trace_path: namespace_root
            .join("fleet")
            .join("authority-state-trace.json"),
        summary_path: namespace_root.join("fleet").join("proof-summary.json"),
        authority_log_path: namespace_root.join("logs").join("authority.log"),
        artifact_store_log_path: namespace_root.join("logs").join("artifact-store.log"),
        namespace_root,
    }
}

fn proof_namespace_ports(namespace: &str) -> ProofNamespacePorts {
    let slot = proof_hash_slot(namespace);
    let base = proof_slot_base(slot);
    ProofNamespacePorts {
        relay_http: base,
        relay_upstream: base + 1,
        control_http: base + 2,
        artifact_store: base + 3,
    }
}

fn proof_hash_slot(value: &str) -> u16 {
    let digest = Sha256::digest(value.as_bytes());
    u16::from_be_bytes([digest[0], digest[1]]) % PROOF_PORT_SLOTS
}

fn proof_slot_base(slot: u16) -> u16 {
    PROOF_PORT_BASE + slot * PROOF_PORT_STRIDE
}

fn allocate_proof_node_ports(
    namespace: &str,
    role: ProofFleetNodeRole,
    index: usize,
    used_ports: &mut BTreeSet<u16>,
) -> Result<ProofNodePorts> {
    let key = format!("{namespace}:{}:{index}", role.label());
    let mut slot = proof_hash_slot(key.as_str());
    for _ in 0..PROOF_PORT_SLOTS {
        let base = proof_slot_base(slot);
        let admin = base;
        let checkpoint = base + 1;
        if !used_ports.contains(&admin) && !used_ports.contains(&checkpoint) {
            used_ports.insert(admin);
            used_ports.insert(checkpoint);
            return Ok(ProofNodePorts { admin, checkpoint });
        }
        slot = (slot + 1) % PROOF_PORT_SLOTS;
    }
    bail!("failed to allocate deterministic proof node ports for {namespace}")
}

fn write_signer_credentials(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create signer dir {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(&json!({
        "client_email": "proof-local-signer@test.openagents.invalid",
        "private_key": TEST_GCS_SERVICE_ACCOUNT_PRIVATE_KEY,
    }))?;
    fs::write(path, format!("{payload}\n"))
        .with_context(|| format!("failed to write signer credentials {}", path.display()))
}

fn authority_environment(
    mode: ProofAuthorityMode,
    ports: &ProofNamespacePorts,
    layout: &ProofLayout,
    admin_bearer_token: &str,
    relay_ws_url: Option<&str>,
    artifact_store_base_url: &str,
) -> BTreeMap<String, String> {
    let authority_base_port = ports.control_http;
    let mut env = BTreeMap::from([
        (
            "NEXUS_CONTROL_ADMIN_BEARER_TOKEN".to_string(),
            admin_bearer_token.to_string(),
        ),
        (
            "NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL".to_string(),
            relay_ws_url
                .unwrap_or_else(|| "ws://127.0.0.1:0/")
                .to_string(),
        ),
        (
            "NEXUS_CONTROL_KERNEL_STATE_PATH".to_string(),
            layout.kernel_state_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{authority_base_port}"),
        ),
        (
            "NEXUS_CONTROL_RECEIPT_LOG_PATH".to_string(),
            layout.receipt_log_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_BUCKET_URI".to_string(),
            PROOF_ARTIFACT_BUCKET.to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_ENDPOINT".to_string(),
            artifact_store_base_url.to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_SIGNING_CREDENTIALS_PATH".to_string(),
            layout.signer_credentials_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_TRN_IDENTITY_PATH".to_string(),
            layout.training_trn_identity_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS".to_string(),
            "10000".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_ENABLED".to_string(),
            "true".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS".to_string(),
            "60".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW".to_string(),
            "120".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE".to_string(),
            "presence_only".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_STATE_PATH".to_string(),
            layout.treasury_state_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH".to_string(),
            layout.treasury_wallet_mnemonic_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_NETWORK".to_string(),
            "regtest".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_STATUS_REFRESH_SECONDS".to_string(),
            "3600".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_SIMULATED_WALLET_ENABLED".to_string(),
            "true".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_SIMULATED_WALLET_BALANCE_SATS".to_string(),
            "1000000".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR".to_string(),
            layout.treasury_wallet_dir.display().to_string(),
        ),
    ]);
    if mode == ProofAuthorityMode::ProdShaped {
        env.insert(
            "NEXUS_RELAY_DATA_DIR".to_string(),
            layout.relay_data_dir.display().to_string(),
        );
        env.insert(
            "NEXUS_RELAY_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{}", ports.relay_http),
        );
        env.insert(
            "NEXUS_RELAY_PUBLIC_WS_URL".to_string(),
            relay_ws_url.unwrap_or("ws://127.0.0.1:0/").to_string(),
        );
        env.insert(
            "NEXUS_RELAY_UPSTREAM_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{}", ports.relay_upstream),
        );
    }
    env
}

fn write_env_manifest(path: &Path, env: &BTreeMap<String, String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create env dir {}", parent.display()))?;
    }
    let mut lines = Vec::new();
    for (key, value) in env {
        lines.push(format!("{key}={value}"));
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .with_context(|| format!("failed to write env manifest {}", path.display()))
}

fn load_runtime_state(path: &Path) -> Result<Option<ProofAuthorityRuntimeState>> {
    if !path.is_file() {
        return Ok(None);
    }
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read proof runtime state {}", path.display()))?;
    let state = serde_json::from_str(payload.as_str())
        .with_context(|| format!("failed to parse proof runtime state {}", path.display()))?;
    Ok(Some(state))
}

fn save_runtime_state(path: &Path, state: &ProofAuthorityRuntimeState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime state dir {}", parent.display()))?;
    }
    let payload = serde_json::to_vec_pretty(state)?;
    let temp_path = path.with_extension("json.partial");
    fs::write(temp_path.as_path(), payload).with_context(|| {
        format!(
            "failed to write proof runtime state {}",
            temp_path.display()
        )
    })?;
    fs::rename(temp_path.as_path(), path)
        .with_context(|| format!("failed to finalize proof runtime state {}", path.display()))
}

async fn collect_route_probes(state: &ProofAuthorityRuntimeState) -> Vec<ProofRouteProbe> {
    match tokio::time::timeout(
        PROOF_ROUTE_PROBE_TOTAL_TIMEOUT,
        collect_route_probes_with_process_check(state),
    )
    .await
    {
        Ok(probes) => probes,
        Err(_) => vec![ProofRouteProbe {
            route_id: "route_probe_total_budget".to_string(),
            url: state.urls.authority_base_url.clone(),
            ok: false,
            status: None,
            detail: format!(
                "route probes timed out after {}ms total budget",
                PROOF_ROUTE_PROBE_TOTAL_TIMEOUT.as_millis()
            ),
        }],
    }
}

async fn collect_route_probes_with_process_check(
    state: &ProofAuthorityRuntimeState,
) -> Vec<ProofRouteProbe> {
    let authority_running = process_is_running(&state.authority_process);
    let artifact_running = process_is_running(&state.artifact_store_process);
    if !authority_running || !artifact_running {
        return vec![
            ProofRouteProbe {
                route_id: "healthz".to_string(),
                url: format!("{}/healthz", state.urls.authority_base_url),
                ok: false,
                status: None,
                detail: "authority process is not running".to_string(),
            },
            ProofRouteProbe {
                route_id: "artifact_store_healthz".to_string(),
                url: format!("http://127.0.0.1:{}/healthz", state.ports.artifact_store),
                ok: false,
                status: None,
                detail: "artifact store process is not running".to_string(),
            },
        ];
    }

    let client = reqwest::Client::new();
    let (p1, p2, p3, p4, p5) = tokio::join!(
        probe_route(
            &client,
            "healthz",
            format!("{}/healthz", state.urls.authority_base_url),
            reqwest::Method::GET,
            &[StatusCode::OK],
        ),
        probe_route(
            &client,
            "training_artifact_layout",
            format!(
                "{}/v1/kernel/compute/training/artifact-storage-layout",
                state.urls.authority_base_url
            ),
            reqwest::Method::GET,
            &[StatusCode::OK],
        ),
        probe_route(
            &client,
            "treasury_status",
            format!("{}/v1/treasury/status", state.urls.authority_base_url),
            reqwest::Method::GET,
            &[StatusCode::OK],
        ),
        probe_route(
            &client,
            "admin_demo_launch_route",
            format!(
                "{}/v1/admin/training/demo-runs/cs336-a1/launch",
                state.urls.authority_base_url
            ),
            reqwest::Method::GET,
            &[StatusCode::METHOD_NOT_ALLOWED, StatusCode::UNAUTHORIZED],
        ),
        probe_route(
            &client,
            "artifact_store_healthz",
            format!("http://127.0.0.1:{}/healthz", state.ports.artifact_store),
            reqwest::Method::GET,
            &[StatusCode::OK],
        ),
    );
    vec![p1, p2, p3, p4, p5]
}

async fn probe_route(
    client: &reqwest::Client,
    route_id: &str,
    url: String,
    method: reqwest::Method,
    expected: &[StatusCode],
) -> ProofRouteProbe {
    let response = tokio::time::timeout(
        PROOF_ROUTE_PROBE_TIMEOUT,
        client.request(method, url.as_str()).send(),
    )
    .await;
    match response {
        Ok(Ok(response)) => {
            let status = response.status();
            let ok = expected.contains(&status);
            let detail = if ok {
                format!("route reachable with {}", status.as_u16())
            } else {
                format!("unexpected status {}", status.as_u16())
            };
            ProofRouteProbe {
                route_id: route_id.to_string(),
                url,
                ok,
                status: Some(status.as_u16()),
                detail,
            }
        }
        Ok(Err(error)) => ProofRouteProbe {
            route_id: route_id.to_string(),
            url,
            ok: false,
            status: None,
            detail: error.to_string(),
        },
        Err(_) => ProofRouteProbe {
            route_id: route_id.to_string(),
            url,
            ok: false,
            status: None,
            detail: format!(
                "route probe timed out after {}ms",
                PROOF_ROUTE_PROBE_TIMEOUT.as_millis()
            ),
        },
    }
}

async fn wait_for_route(url: &str, expected: &[StatusCode]) -> Result<()> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + PROOF_ROUTE_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(response) = client.get(url).send().await {
            if expected.contains(&response.status()) {
                return Ok(());
            }
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for route {}", url);
}

async fn wait_for_proof_treasury_ready(authority_base_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{authority_base_url}/v1/treasury/status");
    let deadline = Instant::now() + PROOF_ROUTE_TIMEOUT;
    let mut last_detail = "treasury status not observed".to_string();
    while Instant::now() < deadline {
        match client.get(url.as_str()).send().await {
            Ok(response) if response.status() == StatusCode::OK => {
                let status: serde_json::Value = response.json().await.with_context(|| {
                    format!("failed to decode proof treasury status from {url}")
                })?;
                let degraded = status
                    .get("degraded_reason")
                    .and_then(serde_json::Value::as_str)
                    .filter(|value| !value.trim().is_empty());
                let wallet_status = status
                    .get("wallet_runtime_status")
                    .and_then(serde_json::Value::as_str);
                if degraded.is_none() && wallet_status == Some("connected") {
                    return Ok(());
                }
                last_detail = format!(
                    "degraded_reason={} wallet_runtime_status={}",
                    degraded.unwrap_or("none"),
                    wallet_status.unwrap_or("unknown")
                );
            }
            Ok(response) => {
                last_detail = format!("unexpected status {}", response.status().as_u16());
            }
            Err(error) => {
                last_detail = error.to_string();
            }
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for proof treasury readiness at {url}: {last_detail}");
}

async fn run_artifact_smoke(
    state: &ProofAuthorityRuntimeState,
) -> Result<ProofArtifactSmokeReport> {
    let artifact_id = synthetic_artifact_id(state.namespace.as_str());
    let resolver_url = format!(
        "{}/v1/kernel/compute/training/artifacts/{}",
        state.urls.authority_base_url, artifact_id
    );
    let client = reqwest::Client::new();
    let resolver = client
        .get(resolver_url.as_str())
        .send()
        .await
        .with_context(|| format!("failed to fetch resolver {resolver_url}"))?
        .error_for_status()
        .with_context(|| format!("resolver probe failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode artifact resolver response")?;
    let relative_object_path = resolver
        .get("relative_object_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("artifact resolver response missing relative_object_path"))?
        .to_string();

    let payload = serde_json::to_vec_pretty(&json!({
        "schema_version": "openagents.proof.runtime.smoke.v1",
        "namespace": state.namespace,
        "mode": state.mode.label(),
        "artifact_id": artifact_id,
    }))?;
    let expected_digest = sha256_prefixed_bytes(payload.as_slice());
    let size_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);

    let signed_access_url = format!(
        "{}/v1/kernel/compute/training/artifacts/{}/signed-access",
        state.urls.authority_base_url, artifact_id
    );
    let write_access = client
        .post(signed_access_url.as_str())
        .json(&json!({
            "mode": "write",
            "ttl_seconds": 300,
            "digest": expected_digest,
            "size_bytes": size_bytes,
        }))
        .send()
        .await
        .with_context(|| format!("failed to request write access for {artifact_id}"))?
        .error_for_status()
        .with_context(|| format!("write access failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode write access response")?;
    let write_url = write_access
        .get("signed_url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write access response missing signed_url"))?;
    client
        .put(write_url)
        .body(payload.clone())
        .send()
        .await
        .context("failed to upload artifact smoke payload")?
        .error_for_status()
        .context("artifact smoke upload returned non-success")?;

    let read_access = client
        .post(signed_access_url.as_str())
        .json(&json!({
            "mode": "read",
            "ttl_seconds": 300,
        }))
        .send()
        .await
        .with_context(|| format!("failed to request read access for {artifact_id}"))?
        .error_for_status()
        .with_context(|| format!("read access failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode read access response")?;
    let read_url = read_access
        .get("signed_url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("read access response missing signed_url"))?;
    let downloaded = client
        .get(read_url)
        .send()
        .await
        .context("failed to download artifact smoke payload")?
        .error_for_status()
        .context("artifact smoke read returned non-success")?
        .bytes()
        .await
        .context("failed to read artifact smoke payload bytes")?;
    if downloaded.as_ref() != payload.as_slice() {
        bail!("artifact smoke readback payload mismatched uploaded bytes");
    }

    let trace_entry_count = fs::read_to_string(state.paths.artifact_trace_path.as_str())
        .ok()
        .map(|contents| {
            contents
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        })
        .unwrap_or(0);
    Ok(ProofArtifactSmokeReport {
        artifact_id,
        relative_object_path,
        expected_digest,
        payload_size_bytes: size_bytes,
        trace_entry_count,
        verified_at_ms: super::now_epoch_ms(),
    })
}

fn synthetic_artifact_id(namespace: &str) -> String {
    let slug = namespace_slug(namespace);
    format!(
        "oa.train_artifact.v1~kind~local_update~network~trainnet.{slug}~run~run.{slug}~window~window.{slug}.0001~assignment~assign.{slug}.worker.1.attempt1"
    )
}

fn namespace_slug(namespace: &str) -> String {
    namespace
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '.',
        })
        .collect::<String>()
}

fn render_proof_status_report(report: &ProofAuthorityStatusReport) -> String {
    if !report.configured {
        return format!(
            "proof authority: configured=false namespace={} detail=run `oa proof authority up`",
            report.namespace
        );
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "proof authority: configured=true namespace={} mode={} treasury_enabled={} admin_auth_configured={}",
        report.namespace,
        report.mode.map(ProofAuthorityMode::label).unwrap_or("unknown"),
        report.treasury_enabled,
        report.admin_auth_configured
    ));
    if let Some(urls) = report.urls.as_ref() {
        lines.push(format!("authority_url: {}", urls.authority_base_url));
        lines.push(format!(
            "artifact_store_url: {}",
            urls.artifact_store_base_url
        ));
        if let Some(relay_ws_url) = urls.relay_ws_url.as_deref() {
            lines.push(format!("relay_ws_url: {relay_ws_url}"));
        }
    }
    if let Some(paths) = report.paths.as_ref() {
        lines.push(format!("kernel_state_path: {}", paths.kernel_state_path));
        lines.push(format!("receipt_log_path: {}", paths.receipt_log_path));
        lines.push(format!(
            "treasury_state_path: {}",
            paths.treasury_state_path
        ));
        lines.push(format!(
            "artifact_trace_path: {}",
            paths.artifact_trace_path
        ));
    }
    if let Some(process) = report.authority_process.as_ref() {
        lines.push(format!(
            "authority_process: running={} pid={} log={}",
            process.running,
            process
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            process.log_path
        ));
    }
    if let Some(process) = report.artifact_store_process.as_ref() {
        lines.push(format!(
            "artifact_store_process: running={} pid={} log={}",
            process.running,
            process
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            process.log_path
        ));
    }
    for probe in &report.probes {
        lines.push(format!(
            "probe {}: ok={} status={} detail={}",
            probe.route_id,
            probe.ok,
            probe
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "-".to_string()),
            probe.detail
        ));
    }
    if let Some(smoke) = report.artifact_smoke.as_ref() {
        lines.push(format!(
            "artifact_smoke: artifact_id={} bytes={} digest={} trace_entries={}",
            smoke.artifact_id,
            render_byte_size(smoke.payload_size_bytes),
            smoke.expected_digest,
            smoke.trace_entry_count
        ));
    }
    lines.join("\n")
}

fn render_proof_fleet_status_report(report: &ProofFleetStatusReport) -> String {
    if !report.configured {
        return format!(
            "proof fleet: configured=false namespace={} detail=run `oa proof fleet up --namespace {}`",
            report.namespace, report.namespace
        );
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "proof fleet: configured=true namespace={} mode={} network_id={} run_slug={}",
        report.namespace,
        report
            .mode
            .map(ProofAuthorityMode::label)
            .unwrap_or("unknown"),
        report.network_id.as_deref().unwrap_or("-"),
        report.run_slug.as_deref().unwrap_or("-")
    ));
    if let Some(paths) = report.paths.as_ref() {
        lines.push(format!("fleet_root: {}", paths.fleet_root));
        lines.push(format!("fleet_state_path: {}", paths.fleet_state_path));
        lines.push(format!("run_report_path: {}", paths.run_report_path));
        lines.push(format!("trace_path: {}", paths.trace_path));
        lines.push(format!("summary_path: {}", paths.summary_path));
    }
    lines.push(format!(
        "authority: configured={} authority_url={}",
        report.authority.configured,
        report
            .authority
            .urls
            .as_ref()
            .map(|urls| urls.authority_base_url.as_str())
            .unwrap_or("-")
    ));
    for node in &report.nodes {
        lines.push(format!(
            "node {} {}: running={} admin_url={} checkpoint_url={} config={} stale_state={} fixture={}",
            node.role.label(),
            node.index,
            node.process.running,
            node.admin_url,
            node.checkpoint_serve_url,
            node.config_path,
            node.stale_retained_state_injected,
            node.retained_state_fixture_id.as_deref().unwrap_or("-")
        ));
        if let Some(training) = node.training.as_ref() {
            lines.push(format!(
                "node {} {} training: current_run={} active_window={} process_state={} issues={} pending_closeouts={}",
                node.role.label(),
                node.index,
                training.current_run_id.as_deref().unwrap_or("-"),
                training.active_window_id.as_deref().unwrap_or("-"),
                training
                    .active_runtime_process_state
                    .as_deref()
                    .unwrap_or("-"),
                training.recent_issue_count,
                training.pending_closeout_count
            ));
            if let Some(reason) = training.last_failure_reason.as_deref() {
                lines.push(format!(
                    "node {} {} failure: {}",
                    node.role.label(),
                    node.index,
                    reason
                ));
            }
            if let Some(reason) = training.first_issue_reason.as_deref() {
                lines.push(format!(
                    "node {} {} issue: {}",
                    node.role.label(),
                    node.index,
                    reason
                ));
            }
            if let Some(load_error) = training.load_error.as_deref() {
                lines.push(format!(
                    "node {} {} load_error: {}",
                    node.role.label(),
                    node.index,
                    load_error
                ));
            }
        }
    }
    if let Some(launch) = report.launched_run.as_ref() {
        lines.push(format!(
            "launched_run: run_id={} launch_state={} run_status={} network_id={}",
            launch.training_run_id,
            launch.launch_state,
            launch.run_detail.run.run_status,
            launch.network_id
        ));
    }
    lines.join("\n")
}

fn render_proof_run_report(report: &ProofRunReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "proof run: status={} lane={} namespace={} detail={}",
        report.status, report.lane, report.namespace, report.detail
    ));
    if let Some(blocker_id) = report.blocker_id.as_deref() {
        lines.push(format!("blocker_id: {blocker_id}"));
    }
    if let Some(write) = report.first_failed_authority_write.as_ref() {
        lines.push(format!(
            "first_failed_authority_write: source={} status={} detail={}",
            write.source,
            write
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            write.detail
        ));
    }
    if let Some(launch) = report.launch.as_ref() {
        lines.push(format!(
            "launch: run_id={} launch_state={} launch_phase={} network_id={} worker_target_count={}",
            launch.training_run_id,
            launch.launch_state,
            launch.launch_phase.as_deref().unwrap_or("-"),
            launch.network_id,
            launch.worker_target_count
        ));
    }
    if let Some(observed) = report.observed_run.as_ref() {
        lines.push(format!(
            "observed_run: status={} current_window={} active_windows={} pending_validation={} validator_open={} validator_queued={} latest_closeout={} windows={} contributions={} nodes={} caveats={}",
            observed.run.run_status,
            observed.run.current_window_id,
            observed.run.active_window_count,
            observed.run.pending_validation_window_count,
            observed.run.validator_challenges_open,
            observed.run.validator_challenges_queued,
            observed.run.latest_closeout_status.as_deref().unwrap_or("-"),
            observed.windows.len(),
            observed.contribution_count,
            observed.node_count,
            observed.caveat_count
        ));
        if let Some(severity) = observed.first_caveat_severity.as_deref() {
            lines.push(format!(
                "observed_run caveat: severity={} title={} detail={}",
                severity,
                observed.first_caveat_title.as_deref().unwrap_or("-"),
                observed.first_caveat_detail.as_deref().unwrap_or("-")
            ));
        }
    }
    if let Some(projection) = report.a1_minimal_projection.as_ref() {
        lines.push(format!(
            "a1_minimal: launch={} run_id={} participants={} model_progress_participants={} weak_device_participants={}",
            projection.launch,
            projection.run_id,
            projection
                .canonical_counters
                .training_accepted_contributors,
            projection
                .canonical_counters
                .training_model_progress_contributors,
            projection
                .canonical_counters
                .training_weak_device_accepted_contributors
        ));
        lines.push(format!(
            "a1_minimal source_of_truth: participants=training_accepted_contributors model_progress_participants=training_model_progress_contributors"
        ));
        lines.push(format!(
            "a1_minimal checkpoint: promoted={} local_updates={}",
            projection
                .checkpoint_lineage
                .promoted_checkpoint_ref
                .as_deref()
                .unwrap_or("-"),
            projection
                .checkpoint_lineage
                .local_update_artifact_ids
                .len()
        ));
    }
    lines.push(render_proof_fleet_status_report(&report.fleet));
    lines.join("\n")
}

fn render_proof_doctor_report(report: &ProofDoctorReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "proof doctor: configured={} namespace={} branch={} commit={}",
        report.configured,
        report.namespace,
        report.git.branch.as_deref().unwrap_or("-"),
        report.git.commit.as_deref().unwrap_or("-")
    ));
    lines.push(format!(
        "current_executable: path={} digest={}",
        report.current_executable.expected_binary_path,
        report
            .current_executable
            .expected_binary_digest
            .as_deref()
            .unwrap_or("-")
    ));
    if let Some(path) = report.latest_trace_path.as_deref() {
        lines.push(format!("latest_trace_path: {path}"));
    }
    if let Some(path) = report.latest_summary_path.as_deref() {
        lines.push(format!("latest_summary_path: {path}"));
    }
    if let Some(env_file) = report.authority_env_file.as_ref() {
        lines.push(format!(
            "authority_env_file: path={} keys={}",
            env_file.path, env_file.key_count
        ));
    }
    for process in &report.process_provenance {
        lines.push(format!(
            "process {}: running={} pid={} matches_expected={} running_binary={}",
            process.component_id,
            process.running,
            process
                .pid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            process
                .binary_matches_expected
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            process.running_binary_path.as_deref().unwrap_or("-")
        ));
        for env in &process.env_expectations {
            lines.push(format!(
                "process {} env {}: present={} matches_expected={}",
                process.component_id,
                env.key,
                env.process_present
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                env.matches_expected
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ));
        }
    }
    for probe in &report.transport.authority_front_door {
        lines.push(format!(
            "transport authority {}: ok={} status={} detail={}",
            probe.route_id,
            probe.ok,
            probe
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            probe.detail
        ));
    }
    for probe in &report.transport.artifact_store {
        lines.push(format!(
            "transport artifact {}: ok={} status={} detail={}",
            probe.route_id,
            probe.ok,
            probe
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            probe.detail
        ));
    }
    lines.push(format!(
        "transport relay: running={} ws_url={} detail={}",
        report.transport.relay.authority_running,
        report
            .transport
            .relay
            .relay_ws_url
            .as_deref()
            .unwrap_or("-"),
        report.transport.relay.detail
    ));
    for node in &report.transport.node_surfaces {
        lines.push(format!(
            "transport node {} {} admin: ok={} status={} detail={}",
            node.role.label(),
            node.index,
            node.admin.ok,
            node.admin
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            node.admin.detail
        ));
        lines.push(format!(
            "transport node {} {} checkpoint: ok={} status={} detail={}",
            node.role.label(),
            node.index,
            node.checkpoint.ok,
            node.checkpoint
                .status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            node.checkpoint.detail
        ));
    }
    lines.push(render_proof_fleet_status_report(&report.fleet));
    lines.join("\n")
}

async fn run_artifact_store_server(
    listen_addr: SocketAddr,
    store_root: PathBuf,
    trace_path: PathBuf,
) -> Result<()> {
    fs::create_dir_all(store_root.as_path())
        .with_context(|| format!("failed to create artifact store {}", store_root.display()))?;
    if let Some(parent) = trace_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create trace dir {}", parent.display()))?;
    }
    let app = Router::new()
        .route("/healthz", get(artifact_store_healthz))
        .route(
            "/upload/{bucket}/{*object_path}",
            put(artifact_store_put).get(artifact_store_get),
        )
        .with_state(Arc::new(ArtifactStoreState {
            store_root,
            trace_path,
        }));
    let listener = TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("failed to bind artifact store {}", listen_addr))?;
    axum::serve(listener, app)
        .await
        .context("artifact store server exited unexpectedly")
}

async fn artifact_store_healthz() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "proof_artifact_store",
    }))
}

async fn artifact_store_put(
    State(state): State<Arc<ArtifactStoreState>>,
    AxumPath((bucket, object_path)): AxumPath<(String, String)>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let destination = state
        .store_root
        .join(bucket.as_str())
        .join(object_path.as_str());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(internal_error)?;
    }
    fs::write(destination.as_path(), body.as_ref()).map_err(internal_error)?;
    append_trace(
        state.trace_path.as_path(),
        "write",
        bucket.as_str(),
        object_path.as_str(),
        Some(body.as_ref()),
    )
    .map_err(internal_error)?;
    Ok(StatusCode::OK)
}

async fn artifact_store_get(
    State(state): State<Arc<ArtifactStoreState>>,
    AxumPath((bucket, object_path)): AxumPath<(String, String)>,
) -> Result<(StatusCode, Vec<u8>), (StatusCode, String)> {
    let source = state
        .store_root
        .join(bucket.as_str())
        .join(object_path.as_str());
    match fs::read(source.as_path()) {
        Ok(payload) => {
            append_trace(
                state.trace_path.as_path(),
                "read",
                bucket.as_str(),
                object_path.as_str(),
                None,
            )
            .map_err(internal_error)?;
            Ok((StatusCode::OK, payload))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((StatusCode::NOT_FOUND, Vec::new()))
        }
        Err(error) => Err(internal_error(error)),
    }
}

fn append_trace(
    trace_path: &Path,
    operation: &str,
    bucket: &str,
    object_path: &str,
    payload: Option<&[u8]>,
) -> Result<()> {
    let payload_digest = payload.map(sha256_prefixed_bytes);
    let payload_size_bytes = payload.and_then(|bytes| u64::try_from(bytes.len()).ok());
    let line = serde_json::to_string(&json!({
        "recorded_at_ms": super::now_epoch_ms(),
        "operation": operation,
        "bucket": bucket,
        "object_path": object_path,
        "canonical_object_uri": format!("gs://{bucket}/{object_path}"),
        "payload_digest": payload_digest,
        "payload_size_bytes": payload_size_bytes,
    }))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(trace_path)
        .with_context(|| format!("failed to open artifact trace {}", trace_path.display()))?;
    use std::io::Write as _;
    writeln!(file, "{line}")
        .with_context(|| format!("failed to append artifact trace {}", trace_path.display()))
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        PROOF_ARTIFACT_UPLOAD_PREFIX, ProofLane, ProofNodeRuntimeFixture,
        build_a1_minimal_launch_projection, detect_proof_run_blocker,
        load_proof_node_runtime_fixture, load_proof_replacement_contribution_template,
        parse_proof_command, parse_proof_lane, parse_status_body_from_reason,
        proof_namespace_ports, proof_training_run_detail_status_is_retryable,
        render_proof_status_report, run_artifact_store_server,
        validate_a1_minimal_launch_projection,
    };

    use std::time::Duration;

    use anyhow::{Result, anyhow, ensure};
    use axum::http::StatusCode;
    use serde_json::Value;
    use serde_json::json;
    use tempfile::tempdir;

    fn proof_test_authority_status(namespace: &str) -> super::ProofAuthorityStatusReport {
        super::ProofAuthorityStatusReport {
            configured: true,
            namespace: namespace.to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            started_at_ms: None,
            admin_auth_configured: true,
            treasury_enabled: true,
            ports: None,
            paths: None,
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        }
    }

    fn proof_test_process_status(running: bool) -> super::ProofProcessStatus {
        super::ProofProcessStatus {
            binary: "target/debug/oa".to_string(),
            pid: running.then_some(1234),
            running,
            log_path: "proof.log".to_string(),
        }
    }

    fn proof_test_node_status(
        role: super::ProofFleetNodeRole,
        runtime_state: Option<&str>,
    ) -> super::ProofFleetNodeStatus {
        super::ProofFleetNodeStatus {
            role,
            index: 1,
            node_label: format!("proof-{}", role.label()),
            payout_destination: format!("lnbc1proof{}", role.label()),
            home_dir: format!("/tmp/proof/{}", role.label()),
            config_path: format!("/tmp/proof/{}/config.json", role.label()),
            run_root: format!("/tmp/proof/{}/training", role.label()),
            admin_url: "http://127.0.0.1:1".to_string(),
            checkpoint_serve_url: "http://127.0.0.1:2".to_string(),
            stale_retained_state_injected: false,
            retained_state_fixture_id: None,
            process: proof_test_process_status(true),
            training: Some(super::ProofFleetNodeTrainingStatus {
                current_run_id: Some("run.proof".to_string()),
                active_window_id: Some("window.proof.0001".to_string()),
                active_runtime_process_state: runtime_state.map(ToString::to_string),
                last_failure_reason: None,
                recent_issue_count: 0,
                first_issue_reason: None,
                pending_closeout_count: 0,
                load_error: None,
            }),
        }
    }

    #[test]
    fn prod_shaped_authority_environment_keeps_relay_and_control_ports_separate() -> Result<()> {
        let temp = tempdir()?;
        let config_path = temp.path().join("config.json");
        let layout = super::proof_layout(config_path.as_path(), "proof.env.port-test");
        let ports = super::ProofNamespacePorts {
            relay_http: 41_000,
            relay_upstream: 41_001,
            control_http: 41_002,
            artifact_store: 41_003,
        };

        let env = super::authority_environment(
            super::ProofAuthorityMode::ProdShaped,
            &ports,
            &layout,
            "proof_admin_test",
            Some("ws://127.0.0.1:41000/"),
            "http://127.0.0.1:41003/upload",
        );

        assert_eq!(
            env.get("NEXUS_CONTROL_LISTEN_ADDR").map(String::as_str),
            Some("127.0.0.1:41002")
        );
        assert_eq!(
            env.get("NEXUS_RELAY_LISTEN_ADDR").map(String::as_str),
            Some("127.0.0.1:41000")
        );
        assert_eq!(
            env.get("NEXUS_RELAY_UPSTREAM_LISTEN_ADDR")
                .map(String::as_str),
            Some("127.0.0.1:41001")
        );
        Ok(())
    }

    fn proof_test_completed_observation() -> super::ProofObservedTrainingRunDetail {
        super::ProofObservedTrainingRunDetail {
            training_run_id: "run.proof".to_string(),
            run: super::ProofObservedRunState {
                training_run_id: "run.proof".to_string(),
                run_status: "running".to_string(),
                current_window_id: "window.proof.0002".to_string(),
                active_window_count: 0,
                pending_validation_window_count: 0,
                validator_challenges_open: 0,
                validator_challenges_queued: 0,
                latest_closeout_status: Some("rewarded".to_string()),
            },
            windows: vec![super::ProofObservedWindowState {
                window_id: "window.proof.0001".to_string(),
                status: "reconciled".to_string(),
                closeout_status: Some("rewarded".to_string()),
                accepted_contributions: 1,
                validator_challenges_open: 0,
                validator_challenges_queued: 0,
            }],
            contribution_count: 1,
            node_count: 1,
            caveat_count: 0,
            first_caveat_id: None,
            first_caveat_severity: None,
            first_caveat_title: None,
            first_caveat_detail: None,
        }
    }

    #[test]
    fn namespace_ports_are_stable() {
        let left = proof_namespace_ports("authority");
        let right = proof_namespace_ports("authority");
        assert_eq!(left, right);
        assert_ne!(left.relay_http, left.artifact_store);
    }

    #[tokio::test]
    async fn artifact_store_round_trips_and_traces_paths() -> Result<()> {
        let dir = tempdir()?;
        let store_root = dir.path().join("store");
        let trace_path = dir.path().join("trace.jsonl");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        drop(listener);
        let server = tokio::spawn(run_artifact_store_server(
            local_addr,
            store_root.clone(),
            trace_path.clone(),
        ));
        super::wait_for_route(
            format!("http://{local_addr}/healthz").as_str(),
            &[StatusCode::OK],
        )
        .await?;

        let client = reqwest::Client::new();
        let upload_url = format!(
            "http://{local_addr}{PROOF_ARTIFACT_UPLOAD_PREFIX}/bucket/networks/trainnet.alpha/runs/run.alpha/manifests/run_manifest.json"
        );
        let payload = br#"{"ok":true}"#.to_vec();
        client
            .put(upload_url.as_str())
            .body(payload.clone())
            .send()
            .await?
            .error_for_status()?;
        let downloaded = client
            .get(upload_url.as_str())
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        assert_eq!(downloaded.as_ref(), payload.as_slice());

        let trace: Value = serde_json::from_str(
            std::fs::read_to_string(trace_path.as_path())?
                .lines()
                .next()
                .ok_or_else(|| anyhow!("missing trace line"))?,
        )?;
        assert_eq!(
            trace["canonical_object_uri"],
            json!("gs://bucket/networks/trainnet.alpha/runs/run.alpha/manifests/run_manifest.json")
        );

        server.abort();
        Ok(())
    }

    #[test]
    fn proof_fleet_diagnostics_scheduler_skips_steady_state_before_interval() {
        let started_at = tokio::time::Instant::now();
        let scheduler = super::ProofFleetDiagnosticScheduler::new(
            started_at,
            Some("running"),
            super::PROOF_FLEET_DIAGNOSTIC_INTERVAL,
        );

        assert!(!scheduler.should_refresh(
            started_at + Duration::from_millis(200),
            Some("running"),
            false,
        ));
    }

    #[test]
    fn proof_fleet_diagnostics_scheduler_refreshes_after_interval() {
        let started_at = tokio::time::Instant::now();
        let scheduler = super::ProofFleetDiagnosticScheduler::new(
            started_at,
            Some("running"),
            super::PROOF_FLEET_DIAGNOSTIC_INTERVAL,
        );

        assert!(scheduler.should_refresh(
            started_at + super::PROOF_FLEET_DIAGNOSTIC_INTERVAL,
            Some("running"),
            false,
        ));
    }

    #[test]
    fn proof_fleet_diagnostics_scheduler_refreshes_on_status_transition() {
        let started_at = tokio::time::Instant::now();
        let scheduler = super::ProofFleetDiagnosticScheduler::new(
            started_at,
            Some("running"),
            super::PROOF_FLEET_DIAGNOSTIC_INTERVAL,
        );

        assert!(scheduler.should_refresh(
            started_at + Duration::from_millis(200),
            Some("completed"),
            false,
        ));
    }

    #[tokio::test]
    async fn proof_route_probe_reports_timeout_without_waiting_for_server() -> Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let _server = tokio::spawn(async move {
            if let Ok((_stream, _peer)) = listener.accept().await {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
        let client = reqwest::Client::new();

        let probe = tokio::time::timeout(
            super::PROOF_ROUTE_PROBE_TIMEOUT + Duration::from_secs(1),
            super::probe_route(
                &client,
                "slow_route",
                format!("http://{addr}/slow"),
                reqwest::Method::GET,
                &[StatusCode::OK],
            ),
        )
        .await
        .map_err(|_| anyhow!("probe_route did not enforce its own timeout"))?;

        ensure!(
            probe.route_id == "slow_route",
            "expected slow_route probe, got {}",
            probe.route_id
        );
        ensure!(!probe.ok, "expected timeout probe to report not ok");
        ensure!(
            probe.detail.contains("timed out"),
            "expected timeout detail, got {}",
            probe.detail
        );
        Ok(())
    }

    #[test]
    fn status_renderer_surfaces_unconfigured_detail() {
        let rendered = render_proof_status_report(&super::ProofAuthorityStatusReport {
            configured: false,
            namespace: "authority".to_string(),
            mode: None,
            started_at_ms: None,
            admin_auth_configured: false,
            treasury_enabled: false,
            ports: None,
            paths: None,
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        });
        assert!(rendered.contains("configured=false"));
    }

    #[test]
    fn detect_proof_run_blocker_ignores_zero_accepted_work_payout_lag() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.caveat".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.caveat".to_string()),
            run_slug: Some("proof.caveat".to_string()),
            paths: None,
            authority: super::ProofAuthorityStatusReport {
                configured: true,
                namespace: "proof.caveat".to_string(),
                mode: Some(super::ProofAuthorityMode::ProdShaped),
                started_at_ms: None,
                admin_auth_configured: true,
                treasury_enabled: true,
                ports: None,
                paths: None,
                urls: None,
                authority_process: None,
                artifact_store_process: None,
                probes: Vec::new(),
                artifact_smoke: None,
            },
            nodes: Vec::new(),
            launched_run: None,
        };
        let observed = super::ProofObservedTrainingRunDetail {
            training_run_id: "run.cs336.a1.proof.caveat".to_string(),
            run: super::ProofObservedRunState {
                training_run_id: "run.cs336.a1.proof.caveat".to_string(),
                run_status: "running".to_string(),
                current_window_id: "window.cs336.a1.proof.caveat.0001".to_string(),
                active_window_count: 1,
                pending_validation_window_count: 0,
                validator_challenges_open: 0,
                validator_challenges_queued: 0,
                latest_closeout_status: None,
            },
            windows: Vec::new(),
            contribution_count: 0,
            node_count: 0,
            caveat_count: 1,
            first_caveat_id: Some("payout_lag".to_string()),
            first_caveat_severity: Some("critical".to_string()),
            first_caveat_title: Some("Payout attention required".to_string()),
            first_caveat_detail: Some(
                "0 accepted-work payout(s) need attention // failed 24h 0 // skipped 24h 2."
                    .to_string(),
            ),
        };
        let blocker = detect_proof_run_blocker(&fleet, Some(&observed));
        assert!(blocker.is_none());
    }

    #[test]
    fn detect_proof_run_blocker_defers_active_validator_backlog() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.caveat".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.caveat".to_string()),
            run_slug: Some("proof.caveat".to_string()),
            paths: None,
            authority: super::ProofAuthorityStatusReport {
                configured: true,
                namespace: "proof.caveat".to_string(),
                mode: Some(super::ProofAuthorityMode::ProdShaped),
                started_at_ms: None,
                admin_auth_configured: true,
                treasury_enabled: true,
                ports: None,
                paths: None,
                urls: None,
                authority_process: None,
                artifact_store_process: None,
                probes: Vec::new(),
                artifact_smoke: None,
            },
            nodes: Vec::new(),
            launched_run: None,
        };
        let observed = super::ProofObservedTrainingRunDetail {
            training_run_id: "run.cs336.a1.proof.caveat".to_string(),
            run: super::ProofObservedRunState {
                training_run_id: "run.cs336.a1.proof.caveat".to_string(),
                run_status: "running".to_string(),
                current_window_id: "window.cs336.a1.proof.caveat.0001".to_string(),
                active_window_count: 1,
                pending_validation_window_count: 1,
                validator_challenges_open: 2,
                validator_challenges_queued: 2,
                latest_closeout_status: None,
            },
            windows: Vec::new(),
            contribution_count: 1,
            node_count: 2,
            caveat_count: 2,
            first_caveat_id: Some("validator_backlog".to_string()),
            first_caveat_severity: Some("critical".to_string()),
            first_caveat_title: Some("Validator backlog".to_string()),
            first_caveat_detail: Some(
                "1 pending window(s) // 2 open challenge(s) // 2 queued challenge(s).".to_string(),
            ),
        };
        let blocker = detect_proof_run_blocker(&fleet, Some(&observed));
        assert!(blocker.is_none());
    }

    #[test]
    fn standard_proof_lane_completion_ignores_global_validator_backlog_after_reward() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.global-backlog".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.global-backlog".to_string()),
            run_slug: Some("proof.global-backlog".to_string()),
            paths: None,
            authority: proof_test_authority_status("proof.global-backlog"),
            nodes: vec![
                proof_test_node_status(super::ProofFleetNodeRole::Worker, None),
                proof_test_node_status(super::ProofFleetNodeRole::Validator, Some("stopped")),
            ],
            launched_run: None,
        };
        let mut observed = proof_test_completed_observation();
        observed.caveat_count = 1;
        observed.first_caveat_id = Some("validator_backlog".to_string());
        observed.first_caveat_severity = Some("critical".to_string());
        observed.first_caveat_title = Some("Validator backlog".to_string());
        observed.first_caveat_detail = Some(
            "2 pending window(s) // 4 open challenge(s) // 4 queued challenge(s).".to_string(),
        );

        assert!(detect_proof_run_blocker(&fleet, Some(&observed)).is_none());
        let detail = super::standard_proof_lane_completion_detail(&fleet, &observed, 1, 1, false)
            .expect("global validator backlog should not block a rewarded observed proof run");
        assert!(detail.contains("window.proof.0001 reconciled"));
    }

    #[test]
    fn detect_proof_run_blocker_surfaces_accepted_work_payout_lag() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.caveat".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.caveat".to_string()),
            run_slug: Some("proof.caveat".to_string()),
            paths: None,
            authority: super::ProofAuthorityStatusReport {
                configured: true,
                namespace: "proof.caveat".to_string(),
                mode: Some(super::ProofAuthorityMode::ProdShaped),
                started_at_ms: None,
                admin_auth_configured: true,
                treasury_enabled: true,
                ports: None,
                paths: None,
                urls: None,
                authority_process: None,
                artifact_store_process: None,
                probes: Vec::new(),
                artifact_smoke: None,
            },
            nodes: Vec::new(),
            launched_run: None,
        };
        let observed = super::ProofObservedTrainingRunDetail {
            training_run_id: "run.cs336.a1.proof.caveat".to_string(),
            run: super::ProofObservedRunState {
                training_run_id: "run.cs336.a1.proof.caveat".to_string(),
                run_status: "running".to_string(),
                current_window_id: "window.cs336.a1.proof.caveat.0001".to_string(),
                active_window_count: 1,
                pending_validation_window_count: 0,
                validator_challenges_open: 0,
                validator_challenges_queued: 0,
                latest_closeout_status: None,
            },
            windows: Vec::new(),
            contribution_count: 0,
            node_count: 0,
            caveat_count: 1,
            first_caveat_id: Some("payout_lag".to_string()),
            first_caveat_severity: Some("critical".to_string()),
            first_caveat_title: Some("Payout attention required".to_string()),
            first_caveat_detail: Some(
                "1 accepted-work payout(s) need attention // failed 24h 0 // skipped 24h 0."
                    .to_string(),
            ),
        };
        let blocker = detect_proof_run_blocker(&fleet, Some(&observed))
            .expect("accepted-work payout caveat should surface as blocker");
        assert_eq!(blocker.0, "authority_run_caveat");
        assert!(blocker.1.contains("Payout attention required"));
    }

    #[test]
    fn standard_proof_lane_completion_accepts_cleared_worker_runtime() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.complete".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.complete".to_string()),
            run_slug: Some("proof.complete".to_string()),
            paths: None,
            authority: proof_test_authority_status("proof.complete"),
            nodes: vec![
                proof_test_node_status(super::ProofFleetNodeRole::Worker, None),
                proof_test_node_status(super::ProofFleetNodeRole::Validator, Some("stopped")),
            ],
            launched_run: None,
        };
        let observed = proof_test_completed_observation();

        let detail = super::standard_proof_lane_completion_detail(&fleet, &observed, 1, 1, false)
            .expect("cleared worker runtime should count as quiesced after rewarded closeout");

        assert!(detail.contains("window.proof.0001 reconciled"));
        assert!(detail.contains("workers_quiesced=1"));
        assert!(detail.contains("validators_quiesced=1"));
    }

    #[test]
    fn standard_proof_lane_completion_rejects_running_worker_runtime() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.running-worker".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.running-worker".to_string()),
            run_slug: Some("proof.running-worker".to_string()),
            paths: None,
            authority: proof_test_authority_status("proof.running-worker"),
            nodes: vec![
                proof_test_node_status(super::ProofFleetNodeRole::Worker, Some("running")),
                proof_test_node_status(super::ProofFleetNodeRole::Validator, Some("stopped")),
            ],
            launched_run: None,
        };
        let observed = proof_test_completed_observation();

        let detail = super::standard_proof_lane_completion_detail(&fleet, &observed, 1, 1, false);

        assert!(
            detail.is_none(),
            "running worker runtime must not be treated as quiesced"
        );
    }

    #[test]
    fn hosted_starter_completion_accepts_healthy_nodes_continuing_work() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.continuing-work".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.continuing-work".to_string()),
            run_slug: Some("proof.continuing-work".to_string()),
            paths: None,
            authority: proof_test_authority_status("proof.continuing-work"),
            nodes: vec![
                proof_test_node_status(super::ProofFleetNodeRole::Worker, Some("running")),
                proof_test_node_status(super::ProofFleetNodeRole::Validator, Some("running")),
            ],
            launched_run: None,
        };
        let observed = proof_test_completed_observation();

        let detail = super::standard_proof_lane_completion_detail(&fleet, &observed, 1, 1, true)
            .expect("hosted starter proof should complete after reward while online nodes continue later jobs");

        assert!(detail.contains("window.proof.0001 reconciled"));
        assert!(detail.contains("workers_healthy=1"));
        assert!(detail.contains("validators_healthy=1"));
    }

    #[test]
    fn standard_proof_lane_completion_waits_for_caveats_to_clear() {
        let fleet = super::ProofFleetStatusReport {
            configured: true,
            namespace: "proof.caveat-clearance".to_string(),
            mode: Some(super::ProofAuthorityMode::ProdShaped),
            network_id: Some("trainnet.proof.caveat-clearance".to_string()),
            run_slug: Some("proof.caveat-clearance".to_string()),
            paths: None,
            authority: proof_test_authority_status("proof.caveat-clearance"),
            nodes: vec![
                proof_test_node_status(super::ProofFleetNodeRole::Worker, None),
                proof_test_node_status(super::ProofFleetNodeRole::Validator, Some("stopped")),
            ],
            launched_run: None,
        };
        let mut observed = proof_test_completed_observation();
        observed.caveat_count = 1;
        observed.first_caveat_id = Some("payout_pending".to_string());
        observed.first_caveat_severity = Some("warning".to_string());

        let detail = super::standard_proof_lane_completion_detail(&fleet, &observed, 1, 1, false);

        assert!(
            detail.is_none(),
            "proof completion must wait until payout and other caveats clear"
        );
    }

    #[test]
    fn parse_proof_command_supports_doctor_namespace_json() {
        let args = vec![
            "oa".to_string(),
            "proof".to_string(),
            "doctor".to_string(),
            "--namespace".to_string(),
            "proof.alpha".to_string(),
            "--json".to_string(),
        ];
        let parsed = parse_proof_command(&args, 2).expect("proof doctor should parse");
        assert_eq!(
            parsed,
            super::ProofCommand::Doctor {
                command: super::ProofDoctorCommand {
                    namespace: "proof.alpha".to_string(),
                    json: true,
                },
            }
        );
    }

    #[test]
    fn parse_status_body_from_reason_extracts_status_and_body() {
        let parsed = parse_status_body_from_reason(
            "training authority run lease failed with status 404: {\"error\":\"kernel_error\"}",
        )
        .expect("status/body should parse");
        assert_eq!(parsed.0, 404);
        assert_eq!(parsed.1, "{\"error\":\"kernel_error\"}");
    }

    #[test]
    fn parse_proof_lane_supports_fixture_backed_variants() {
        assert_eq!(
            parse_proof_lane("cs336-a1-stale-recovery").expect("stale-recovery lane"),
            ProofLane::Cs336A1StaleRecovery
        );
        assert_eq!(
            parse_proof_lane("cs336-a1-hosted-starter").expect("hosted starter lane"),
            ProofLane::Cs336A1HostedStarter
        );
        assert_eq!(
            parse_proof_lane("cs336/a1/replacement-attempt").expect("replacement-attempt lane"),
            ProofLane::Cs336A1ReplacementAttempt
        );
        assert_eq!(
            parse_proof_lane("a1-minimal-distributed-lm-launch-a").expect("launch A lane"),
            ProofLane::A1MinimalDistributedLmLaunchA
        );
        assert_eq!(
            parse_proof_lane("a1_minimal_launch_b").expect("launch B lane"),
            ProofLane::A1MinimalDistributedLmLaunchB
        );
    }

    #[test]
    fn a1_minimal_launch_a_counts_support_as_participants_only() -> Result<()> {
        let projection = build_a1_minimal_launch_projection(
            ProofLane::A1MinimalDistributedLmLaunchA,
            "proof.a1",
        )?;
        validate_a1_minimal_launch_projection(&projection)?;

        assert_eq!(projection.launch, "launch_a");
        assert_eq!(
            projection.canonical_counters.training_accepted_contributors,
            4
        );
        assert_eq!(
            projection
                .canonical_counters
                .training_model_progress_contributors,
            1
        );
        assert_eq!(
            projection
                .canonical_counters
                .training_weak_device_accepted_contributors,
            3
        );
        assert_eq!(
            projection
                .public_stats_projection
                .training_accepted_contributors,
            4
        );
        assert_eq!(
            projection
                .public_stats_projection
                .training_model_progress_contributors,
            1
        );
        assert!(
            projection
                .participants
                .iter()
                .filter(|participant| participant.support_or_verifier_work)
                .all(|participant| !participant.model_progress_work
                    && participant.artifact_kind == "support_bundle")
        );
        assert!(
            projection
                .artifacts
                .iter()
                .any(|artifact| artifact.artifact_kind == "local_update")
        );
        assert!(projection.claim_warning.contains("online Pylons"));
        Ok(())
    }

    #[test]
    fn a1_minimal_launch_b_projects_promoted_checkpoint_lineage() -> Result<()> {
        let projection = build_a1_minimal_launch_projection(
            ProofLane::A1MinimalDistributedLmLaunchB,
            "proof.a1",
        )?;
        validate_a1_minimal_launch_projection(&projection)?;

        assert_eq!(projection.launch, "launch_b");
        assert_eq!(
            projection.canonical_counters.training_accepted_contributors,
            5
        );
        assert_eq!(
            projection
                .canonical_counters
                .training_model_progress_contributors,
            4
        );
        assert_eq!(
            projection
                .checkpoint_lineage
                .local_update_artifact_ids
                .len(),
            4
        );
        assert!(
            projection
                .checkpoint_lineage
                .promoted_checkpoint_ref
                .as_deref()
                .is_some_and(|value| value.contains("a1_minimal_distributed_lm"))
        );
        assert!(
            projection
                .public_stats_projection
                .public_checkpoint_lineage_label
                .contains("4 model-progress participants")
        );
        Ok(())
    }

    #[test]
    fn a1_minimal_projection_rejects_support_counted_as_model_progress() {
        let mut projection = build_a1_minimal_launch_projection(
            ProofLane::A1MinimalDistributedLmLaunchA,
            "proof.a1",
        )
        .expect("projection");
        let support = projection
            .participants
            .iter_mut()
            .find(|participant| participant.support_or_verifier_work)
            .expect("support participant");
        support.model_progress_work = true;
        let error = validate_a1_minimal_launch_projection(&projection)
            .expect_err("support model-progress mismatch must fail");
        assert!(
            error
                .to_string()
                .contains("a1_minimal_support_counted_as_model_progress")
                || error
                    .to_string()
                    .contains("a1_minimal_canonical_counter_mismatch")
        );
    }

    #[test]
    fn proof_training_run_detail_retry_statuses_do_not_abort_gate() {
        assert!(proof_training_run_detail_status_is_retryable(
            StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(proof_training_run_detail_status_is_retryable(
            StatusCode::BAD_GATEWAY
        ));
        assert!(proof_training_run_detail_status_is_retryable(
            StatusCode::GATEWAY_TIMEOUT
        ));
        assert!(proof_training_run_detail_status_is_retryable(
            StatusCode::TOO_MANY_REQUESTS
        ));
        assert!(!proof_training_run_detail_status_is_retryable(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(!proof_training_run_detail_status_is_retryable(
            StatusCode::OK
        ));
    }

    #[test]
    fn parse_proof_run_command_respects_lane_specific_minima() {
        let replacement_args = vec![
            "oa".to_string(),
            "proof".to_string(),
            "run".to_string(),
            "cs336-a1-replacement-attempt".to_string(),
            "--workers".to_string(),
            "0".to_string(),
            "--validators".to_string(),
            "0".to_string(),
        ];
        let parsed = parse_proof_command(&replacement_args, 2)
            .expect("replacement-attempt proof run should parse");
        match parsed {
            super::ProofCommand::Run { command } => {
                assert_eq!(command.lane, ProofLane::Cs336A1ReplacementAttempt);
                assert_eq!(command.workers, 0);
                assert_eq!(command.validators, 0);
            }
            other => panic!("expected proof run command, got {other:?}"),
        }

        let stale_args = vec![
            "oa".to_string(),
            "proof".to_string(),
            "run".to_string(),
            "cs336-a1-stale-recovery".to_string(),
            "--workers".to_string(),
            "0".to_string(),
            "--validators".to_string(),
            "1".to_string(),
        ];
        let error = parse_proof_command(&stale_args, 2)
            .expect_err("stale-recovery run should reject zero workers");
        assert!(error.to_string().contains("requires at least 1 worker"));
    }

    #[test]
    fn proof_runtime_fixtures_decode_and_substitute_network_ids() -> Result<()> {
        let network_id = "trainnet.proof.fixture-test";
        let stale_worker =
            load_proof_node_runtime_fixture(ProofNodeRuntimeFixture::StaleWorkerLease, network_id)?;
        let stale_worker_lease = stale_worker
            .lease_cache
            .get("lease.stale.worker.0001")
            .expect("stale worker lease");
        assert_eq!(stale_worker_lease.network_id.as_deref(), Some(network_id));

        let stale_validator = load_proof_node_runtime_fixture(
            ProofNodeRuntimeFixture::StaleValidatorLease,
            network_id,
        )?;
        let stale_validator_lease = stale_validator
            .lease_cache
            .get("lease.stale.validator.0001")
            .expect("stale validator lease");
        assert_eq!(
            stale_validator_lease.network_id.as_deref(),
            Some(network_id)
        );
        assert_eq!(
            stale_validator_lease.challenge_id.as_deref(),
            Some("challenge.stale.validator.0001")
        );

        let payout_worker = load_proof_node_runtime_fixture(
            ProofNodeRuntimeFixture::CloseoutObservePayoutWorker,
            network_id,
        )?;
        let payout_worker_progress = payout_worker
            .closeout_progress
            .get("assign.fixture.payout.worker.0001")
            .expect("payout worker progress");
        assert_eq!(payout_worker_progress.stage.label(), "accepted");
        assert_eq!(
            payout_worker_progress.payout_state.as_deref(),
            Some("failed")
        );

        let payout_validator = load_proof_node_runtime_fixture(
            ProofNodeRuntimeFixture::CloseoutObservePayoutValidator,
            network_id,
        )?;
        let payout_validator_progress = payout_validator
            .closeout_progress
            .get("assign.fixture.payout.validator.0001")
            .expect("payout validator progress");
        assert_eq!(payout_validator_progress.stage.label(), "accepted");
        assert_eq!(
            payout_validator_progress.payout_state.as_deref(),
            Some("failed")
        );

        Ok(())
    }

    #[test]
    fn replacement_attempt_contribution_fixture_decodes() -> Result<()> {
        let template = load_proof_replacement_contribution_template()?;
        assert_eq!(
            template.validator_disposition,
            Some(super::super::ComputeAdapterContributionDisposition::Accepted)
        );
        assert_eq!(
            template.aggregation_eligibility,
            Some(super::super::ComputeAdapterAggregationEligibility::Eligible)
        );
        assert_eq!(template.held_out_average_score_bps, Some(9400));
        assert_eq!(template.benchmark_pass_rate_bps, Some(9700));
        assert_eq!(template.runtime_smoke_passed, Some(true));
        Ok(())
    }
}
