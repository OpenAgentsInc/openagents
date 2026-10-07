use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for Probe GEPA text-bundle candidate manifests.
pub const PROBE_GEPA_CANDIDATE_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.probe_gepa_candidate_manifest.v1";

/// Stable schema version for Probe-facing GEPA candidate imports.
pub const PROBE_GEPA_PROBE_IMPORT_SCHEMA_VERSION: &str = "probe.prompt_candidate_import.v1";

/// Stable schema version for benchmark-cloud-facing candidate imports.
pub const PROBE_GEPA_BENCHMARK_CLOUD_IMPORT_SCHEMA_VERSION: &str =
    "benchmark_cloud.probe_candidate_import.v1";

/// Retained fixture path for the first Probe GEPA Stage 0/1 seed candidate.
pub const PROBE_GEPA_STAGE_0_1_CANDIDATE_FIXTURE_PATH: &str =
    "fixtures/probe/gepa/probe_gepa_candidate_manifest_stage_0_1_seed_v1.json";

/// Text bundle optimized by GEPA for Probe and Blueprint behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateComponents {
    pub probe_system_prompt: String,
    pub terminal_bench_global_playbook: String,
    pub signature_selection_policy: String,
    pub tool_menu_policy: String,
    pub patch_and_test_policy: String,
    pub failure_family_playbooks: BTreeMap<String, String>,
    pub closeout_policy: String,
}

/// Stable component hashes for the GEPA text bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateComponentHashes {
    pub probe_system_prompt: String,
    pub terminal_bench_global_playbook: String,
    pub signature_selection_policy: String,
    pub tool_menu_policy: String,
    pub patch_and_test_policy: String,
    pub failure_family_playbooks: BTreeMap<String, String>,
    pub closeout_policy: String,
}

/// Policy gate state owned by Psionic/Omega promotion gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaPolicyGateState {
    Pending,
    Passed,
    Failed,
    Blocked,
}

/// Optimizer-side acceptance, separate from runtime promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaOptimizerAcceptanceState {
    Draft,
    OptimizerAccepted,
    Rejected,
}

/// Runtime promotion state consumed by Probe/Omega gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaRuntimePromotionState {
    NotPromoted,
    Shadow,
    ReleaseCandidate,
    Active,
    Reverted,
}

/// User-visible lifecycle state named in the issue plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeGepaCandidatePromotionState {
    Draft,
    OptimizerAccepted,
    Shadow,
    ReleaseCandidate,
    Active,
    Rejected,
    Reverted,
}

/// Probe-side import refs. These are refs only, not new authority grants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaProbeImportRefs {
    pub schema_version: String,
    pub prompt_candidate_ref: String,
    pub blueprint_candidate_ref: String,
    pub tool_menu_candidate_ref: String,
    pub loop_policy_candidate_ref: String,
}

/// Benchmark-cloud-side import refs. These keep split and artifact authority external.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaBenchmarkCloudImportRefs {
    pub schema_version: String,
    pub split_refs: Vec<String>,
    pub benchmark_run_manifest_refs: Vec<String>,
    pub artifact_contract_refs: Vec<String>,
}

/// Safety posture attached to candidate text bundles.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateSafetyBoundary {
    pub no_new_runtime_authority: bool,
    pub inherited_runtime_authority_refs: Vec<String>,
    pub release_gate_ref: String,
    pub public_claim_upgrade_authority: bool,
}

/// Content-addressed Probe GEPA candidate manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateManifest {
    pub schema_version: String,
    pub candidate_id: String,
    pub parent_candidate_id: Option<String>,
    pub campaign_id: String,
    pub candidate_hash: String,
    pub manifest_hash: String,
    pub component_hashes: ProbeGepaCandidateComponentHashes,
    pub components: ProbeGepaCandidateComponents,
    pub target_suites: Vec<String>,
    pub target_failure_families: Vec<String>,
    pub split_refs: Vec<String>,
    pub optimizer_run_id: String,
    pub training_trace_digests: Vec<String>,
    pub evaluation_trace_digests: Vec<String>,
    pub policy_gate_state: ProbeGepaPolicyGateState,
    pub optimizer_acceptance_state: ProbeGepaOptimizerAcceptanceState,
    pub runtime_promotion_state: ProbeGepaRuntimePromotionState,
    pub promotion_state: ProbeGepaCandidatePromotionState,
    pub probe_import: ProbeGepaProbeImportRefs,
    pub benchmark_cloud_import: ProbeGepaBenchmarkCloudImportRefs,
    pub safety_boundary: ProbeGepaCandidateSafetyBoundary,
}

/// Input used to construct a content-addressed Probe GEPA candidate manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeGepaCandidateManifestInput {
    pub parent_candidate_id: Option<String>,
    pub campaign_id: String,
    pub components: ProbeGepaCandidateComponents,
    pub target_suites: Vec<String>,
    pub target_failure_families: Vec<String>,
    pub split_refs: Vec<String>,
    pub optimizer_run_id: String,
    pub training_trace_digests: Vec<String>,
    pub evaluation_trace_digests: Vec<String>,
    pub policy_gate_state: ProbeGepaPolicyGateState,
    pub optimizer_acceptance_state: ProbeGepaOptimizerAcceptanceState,
    pub runtime_promotion_state: ProbeGepaRuntimePromotionState,
    pub promotion_state: ProbeGepaCandidatePromotionState,
    pub probe_import: ProbeGepaProbeImportRefs,
    pub benchmark_cloud_import: ProbeGepaBenchmarkCloudImportRefs,
    pub safety_boundary: ProbeGepaCandidateSafetyBoundary,
}

#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeGepaCandidateManifestError {
    #[error("field `{field}` must be non-empty")]
    EmptyField { field: String },
    #[error("candidate `{candidate_id}` has unstable component hashes")]
    ComponentHashMismatch { candidate_id: String },
    #[error("candidate `{candidate_id}` hash mismatch: expected `{expected}`, found `{actual}`")]
    CandidateHashMismatch {
        candidate_id: String,
        expected: String,
        actual: String,
    },
    #[error(
        "candidate `{candidate_id}` manifest hash mismatch: expected `{expected}`, found `{actual}`"
    )]
    ManifestHashMismatch {
        candidate_id: String,
        expected: String,
        actual: String,
    },
    #[error("candidate `{candidate_id}` id does not match candidate hash `{candidate_hash}`")]
    CandidateIdMismatch {
        candidate_id: String,
        candidate_hash: String,
    },
    #[error("candidate `{candidate_id}` has unsafe text in `{field}`")]
    UnsafeCandidateText { candidate_id: String, field: String },
    #[error("candidate `{candidate_id}` requested runtime authority")]
    RuntimeAuthorityRequest { candidate_id: String },
    #[error("candidate `{candidate_id}` attempted to bypass release gates")]
    ReleaseGateBypass { candidate_id: String },
    #[error("candidate `{candidate_id}` has inconsistent promotion state")]
    InconsistentPromotionState { candidate_id: String },
    #[error("candidate manifest serialization failed: {message}")]
    Serialization { message: String },
}

#[derive(Serialize)]
struct CandidateHashPreimage<'a> {
    parent_candidate_id: &'a Option<String>,
    campaign_id: &'a str,
    component_hashes: &'a ProbeGepaCandidateComponentHashes,
    target_suites: &'a [String],
    target_failure_families: &'a [String],
    split_refs: &'a [String],
    optimizer_run_id: &'a str,
    training_trace_digests: &'a [String],
    evaluation_trace_digests: &'a [String],
    probe_import: &'a ProbeGepaProbeImportRefs,
    benchmark_cloud_import: &'a ProbeGepaBenchmarkCloudImportRefs,
    safety_boundary: &'a ProbeGepaCandidateSafetyBoundary,
}

#[derive(Serialize)]
struct ManifestHashPreimage<'a> {
    schema_version: &'a str,
    candidate_id: &'a str,
    parent_candidate_id: &'a Option<String>,
    campaign_id: &'a str,
    candidate_hash: &'a str,
    component_hashes: &'a ProbeGepaCandidateComponentHashes,
    components: &'a ProbeGepaCandidateComponents,
    target_suites: &'a [String],
    target_failure_families: &'a [String],
    split_refs: &'a [String],
    optimizer_run_id: &'a str,
    training_trace_digests: &'a [String],
    evaluation_trace_digests: &'a [String],
    policy_gate_state: ProbeGepaPolicyGateState,
    optimizer_acceptance_state: ProbeGepaOptimizerAcceptanceState,
    runtime_promotion_state: ProbeGepaRuntimePromotionState,
    promotion_state: ProbeGepaCandidatePromotionState,
    probe_import: &'a ProbeGepaProbeImportRefs,
    benchmark_cloud_import: &'a ProbeGepaBenchmarkCloudImportRefs,
    safety_boundary: &'a ProbeGepaCandidateSafetyBoundary,
}

impl ProbeGepaCandidateComponents {
    #[must_use]
    pub fn component_hashes(&self) -> ProbeGepaCandidateComponentHashes {
        ProbeGepaCandidateComponentHashes {
            probe_system_prompt: stable_sha256(
                "probe_system_prompt",
                self.probe_system_prompt.as_str(),
            ),
            terminal_bench_global_playbook: stable_sha256(
                "terminal_bench_global_playbook",
                self.terminal_bench_global_playbook.as_str(),
            ),
            signature_selection_policy: stable_sha256(
                "signature_selection_policy",
                self.signature_selection_policy.as_str(),
            ),
            tool_menu_policy: stable_sha256("tool_menu_policy", self.tool_menu_policy.as_str()),
            patch_and_test_policy: stable_sha256(
                "patch_and_test_policy",
                self.patch_and_test_policy.as_str(),
            ),
            failure_family_playbooks: self
                .failure_family_playbooks
                .iter()
                .map(|(family, playbook)| {
                    (
                        family.clone(),
                        stable_sha256(
                            format!("failure_family_playbook:{family}").as_str(),
                            playbook,
                        ),
                    )
                })
                .collect(),
            closeout_policy: stable_sha256("closeout_policy", self.closeout_policy.as_str()),
        }
    }
}

impl ProbeGepaCandidateManifest {
    #[must_use]
    pub fn recomputed_component_hashes(&self) -> ProbeGepaCandidateComponentHashes {
        self.components.component_hashes()
    }

    pub fn recomputed_candidate_hash(&self) -> Result<String, ProbeGepaCandidateManifestError> {
        candidate_hash_from_parts(
            &self.parent_candidate_id,
            self.campaign_id.as_str(),
            &self.component_hashes,
            &self.target_suites,
            &self.target_failure_families,
            &self.split_refs,
            self.optimizer_run_id.as_str(),
            &self.training_trace_digests,
            &self.evaluation_trace_digests,
            &self.probe_import,
            &self.benchmark_cloud_import,
            &self.safety_boundary,
        )
    }

    pub fn recomputed_manifest_hash(&self) -> Result<String, ProbeGepaCandidateManifestError> {
        manifest_hash_from_parts(self)
    }
}

pub fn build_probe_gepa_candidate_manifest(
    input: ProbeGepaCandidateManifestInput,
) -> Result<ProbeGepaCandidateManifest, ProbeGepaCandidateManifestError> {
    validate_input_text(&input)?;
    let component_hashes = input.components.component_hashes();
    let candidate_hash = candidate_hash_from_parts(
        &input.parent_candidate_id,
        input.campaign_id.as_str(),
        &component_hashes,
        &input.target_suites,
        &input.target_failure_families,
        &input.split_refs,
        input.optimizer_run_id.as_str(),
        &input.training_trace_digests,
        &input.evaluation_trace_digests,
        &input.probe_import,
        &input.benchmark_cloud_import,
        &input.safety_boundary,
    )?;
    let candidate_id = format!(
        "probe_gepa_candidate.{}",
        short_hash(candidate_hash.as_str())
    );
    let mut manifest = ProbeGepaCandidateManifest {
        schema_version: PROBE_GEPA_CANDIDATE_MANIFEST_SCHEMA_VERSION.to_string(),
        candidate_id,
        parent_candidate_id: input.parent_candidate_id,
        campaign_id: input.campaign_id,
        candidate_hash,
        manifest_hash: String::new(),
        component_hashes,
        components: input.components,
        target_suites: input.target_suites,
        target_failure_families: input.target_failure_families,
        split_refs: input.split_refs,
        optimizer_run_id: input.optimizer_run_id,
        training_trace_digests: input.training_trace_digests,
        evaluation_trace_digests: input.evaluation_trace_digests,
        policy_gate_state: input.policy_gate_state,
        optimizer_acceptance_state: input.optimizer_acceptance_state,
        runtime_promotion_state: input.runtime_promotion_state,
        promotion_state: input.promotion_state,
        probe_import: input.probe_import,
        benchmark_cloud_import: input.benchmark_cloud_import,
        safety_boundary: input.safety_boundary,
    };
    manifest.manifest_hash = manifest_hash_from_parts(&manifest)?;
    validate_probe_gepa_candidate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_probe_gepa_candidate_manifest(
    manifest: &ProbeGepaCandidateManifest,
) -> Result<(), ProbeGepaCandidateManifestError> {
    ensure_nonempty(manifest.schema_version.as_str(), "schema_version")?;
    ensure_nonempty(manifest.candidate_id.as_str(), "candidate_id")?;
    ensure_nonempty(manifest.campaign_id.as_str(), "campaign_id")?;
    ensure_nonempty(manifest.candidate_hash.as_str(), "candidate_hash")?;
    ensure_nonempty(manifest.manifest_hash.as_str(), "manifest_hash")?;
    ensure_nonempty(manifest.optimizer_run_id.as_str(), "optimizer_run_id")?;
    ensure_nonempty(
        manifest.probe_import.schema_version.as_str(),
        "probe_import.schema_version",
    )?;
    ensure_nonempty(
        manifest.benchmark_cloud_import.schema_version.as_str(),
        "benchmark_cloud_import.schema_version",
    )?;
    ensure_nonempty(
        manifest.safety_boundary.release_gate_ref.as_str(),
        "safety_boundary.release_gate_ref",
    )?;
    ensure_nonempty_vec(&manifest.target_suites, "target_suites")?;
    ensure_nonempty_vec(&manifest.target_failure_families, "target_failure_families")?;
    ensure_nonempty_vec(&manifest.split_refs, "split_refs")?;
    ensure_nonempty_vec(
        &manifest.benchmark_cloud_import.split_refs,
        "benchmark_cloud_import.split_refs",
    )?;
    ensure_nonempty_vec(
        &manifest.benchmark_cloud_import.artifact_contract_refs,
        "benchmark_cloud_import.artifact_contract_refs",
    )?;

    validate_component_text(manifest.candidate_id.as_str(), &manifest.components)?;

    if manifest.recomputed_component_hashes() != manifest.component_hashes {
        return Err(ProbeGepaCandidateManifestError::ComponentHashMismatch {
            candidate_id: manifest.candidate_id.clone(),
        });
    }

    let expected_candidate_hash = manifest.recomputed_candidate_hash()?;
    if expected_candidate_hash != manifest.candidate_hash {
        return Err(ProbeGepaCandidateManifestError::CandidateHashMismatch {
            candidate_id: manifest.candidate_id.clone(),
            expected: expected_candidate_hash,
            actual: manifest.candidate_hash.clone(),
        });
    }

    let expected_candidate_id = format!(
        "probe_gepa_candidate.{}",
        short_hash(manifest.candidate_hash.as_str())
    );
    if expected_candidate_id != manifest.candidate_id {
        return Err(ProbeGepaCandidateManifestError::CandidateIdMismatch {
            candidate_id: manifest.candidate_id.clone(),
            candidate_hash: manifest.candidate_hash.clone(),
        });
    }

    let expected_manifest_hash = manifest.recomputed_manifest_hash()?;
    if expected_manifest_hash != manifest.manifest_hash {
        return Err(ProbeGepaCandidateManifestError::ManifestHashMismatch {
            candidate_id: manifest.candidate_id.clone(),
            expected: expected_manifest_hash,
            actual: manifest.manifest_hash.clone(),
        });
    }

    validate_safety_boundary(manifest)?;
    validate_promotion_state(manifest)?;
    Ok(())
}

#[must_use]
pub fn canonical_probe_gepa_stage_0_1_candidate_input() -> ProbeGepaCandidateManifestInput {
    let mut failure_family_playbooks = BTreeMap::new();
    failure_family_playbooks.insert(
        "service_readiness".to_string(),
        "Confirm service ports, readiness endpoints, and process lifetime before declaring success.".to_string(),
    );
    failure_family_playbooks.insert(
        "parser_correctness".to_string(),
        "Use bounded parser tests and preserve hostile-input cases before patch acceptance."
            .to_string(),
    );
    failure_family_playbooks.insert(
        "runner_supervision".to_string(),
        "Treat stalled commands as task evidence, retain partial artifacts, and close out with timeout state.".to_string(),
    );

    ProbeGepaCandidateManifestInput {
        parent_candidate_id: None,
        campaign_id: "probe_gepa.terminal_bench.stage_0_1".to_string(),
        components: ProbeGepaCandidateComponents {
            probe_system_prompt: "Run as Probe under benchmark authority. Preserve evidence, use selected Blueprint signatures, and do not widen runtime authority.".to_string(),
            terminal_bench_global_playbook: "For Terminal-Bench, inspect the task, make the smallest correct patch, run task-local tests, and emit closeout evidence even on failure.".to_string(),
            signature_selection_policy: "Use assignment-selected Blueprint signatures first. If a needed signature is missing, record a lookup miss instead of inventing a new authority path.".to_string(),
            tool_menu_policy: "Use only the assignment tool menu. Prefer read, edit, shell, and test tools that are explicitly admitted for the task sandbox.".to_string(),
            patch_and_test_policy: "Patch only task-scoped files, run the verifier or nearest local test, and preserve command receipts for failed, timed-out, and successful attempts.".to_string(),
            failure_family_playbooks,
            closeout_policy: "Always emit probe-run-record and probe-closeout refs with selected signatures, tool menu, artifact refs, resource refs, and failure classification.".to_string(),
        },
        target_suites: vec!["terminal_bench_2".to_string(), "probe_retained_fixtures".to_string()],
        target_failure_families: vec![
            "service_readiness".to_string(),
            "parser_correctness".to_string(),
            "runner_supervision".to_string(),
        ],
        split_refs: vec![
            "benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1".to_string(),
        ],
        optimizer_run_id: "psionic_gepa_optimizer.probe.stage_0_1.seed".to_string(),
        training_trace_digests: vec!["sha256:probe-gepa-stage-0-retained-trace-seed".to_string()],
        evaluation_trace_digests: vec!["sha256:probe-gepa-stage-1-validation-trace-seed".to_string()],
        policy_gate_state: ProbeGepaPolicyGateState::Pending,
        optimizer_acceptance_state: ProbeGepaOptimizerAcceptanceState::Draft,
        runtime_promotion_state: ProbeGepaRuntimePromotionState::NotPromoted,
        promotion_state: ProbeGepaCandidatePromotionState::Draft,
        probe_import: ProbeGepaProbeImportRefs {
            schema_version: PROBE_GEPA_PROBE_IMPORT_SCHEMA_VERSION.to_string(),
            prompt_candidate_ref: "probe.prompt_candidate.stage_0_1.seed".to_string(),
            blueprint_candidate_ref: "probe.blueprint_candidate.stage_0_1.seed".to_string(),
            tool_menu_candidate_ref: "probe.tool_menu_candidate.stage_0_1.seed".to_string(),
            loop_policy_candidate_ref: "probe.loop_policy_candidate.stage_0_1.seed".to_string(),
        },
        benchmark_cloud_import: ProbeGepaBenchmarkCloudImportRefs {
            schema_version: PROBE_GEPA_BENCHMARK_CLOUD_IMPORT_SCHEMA_VERSION.to_string(),
            split_refs: vec![
                "benchmark_split_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1".to_string(),
            ],
            benchmark_run_manifest_refs: vec![
                "benchmark_run_manifest.terminal_bench_2.probe_gepa.stage_0_1.v1".to_string(),
            ],
            artifact_contract_refs: vec![
                "openagents.benchmark_artifact_manifest.v1".to_string(),
                "openagents.benchmark_proof_bundle.v1".to_string(),
                "probe.benchmark_closeout.v1".to_string(),
            ],
        },
        safety_boundary: ProbeGepaCandidateSafetyBoundary {
            no_new_runtime_authority: true,
            inherited_runtime_authority_refs: vec![
                "runtime_authority.inherited_from_probe_assignment_refs".to_string(),
            ],
            release_gate_ref: "release_gate.omega.probe_blueprint_candidate_promotion.v1".to_string(),
            public_claim_upgrade_authority: false,
        },
    }
}

pub fn canonical_probe_gepa_stage_0_1_candidate_manifest()
-> Result<ProbeGepaCandidateManifest, ProbeGepaCandidateManifestError> {
    build_probe_gepa_candidate_manifest(canonical_probe_gepa_stage_0_1_candidate_input())
}

fn candidate_hash_from_parts(
    parent_candidate_id: &Option<String>,
    campaign_id: &str,
    component_hashes: &ProbeGepaCandidateComponentHashes,
    target_suites: &[String],
    target_failure_families: &[String],
    split_refs: &[String],
    optimizer_run_id: &str,
    training_trace_digests: &[String],
    evaluation_trace_digests: &[String],
    probe_import: &ProbeGepaProbeImportRefs,
    benchmark_cloud_import: &ProbeGepaBenchmarkCloudImportRefs,
    safety_boundary: &ProbeGepaCandidateSafetyBoundary,
) -> Result<String, ProbeGepaCandidateManifestError> {
    stable_json_sha256(&CandidateHashPreimage {
        parent_candidate_id,
        campaign_id,
        component_hashes,
        target_suites,
        target_failure_families,
        split_refs,
        optimizer_run_id,
        training_trace_digests,
        evaluation_trace_digests,
        probe_import,
        benchmark_cloud_import,
        safety_boundary,
    })
}

fn manifest_hash_from_parts(
    manifest: &ProbeGepaCandidateManifest,
) -> Result<String, ProbeGepaCandidateManifestError> {
    stable_json_sha256(&ManifestHashPreimage {
        schema_version: manifest.schema_version.as_str(),
        candidate_id: manifest.candidate_id.as_str(),
        parent_candidate_id: &manifest.parent_candidate_id,
        campaign_id: manifest.campaign_id.as_str(),
        candidate_hash: manifest.candidate_hash.as_str(),
        component_hashes: &manifest.component_hashes,
        components: &manifest.components,
        target_suites: &manifest.target_suites,
        target_failure_families: &manifest.target_failure_families,
        split_refs: &manifest.split_refs,
        optimizer_run_id: manifest.optimizer_run_id.as_str(),
        training_trace_digests: &manifest.training_trace_digests,
        evaluation_trace_digests: &manifest.evaluation_trace_digests,
        policy_gate_state: manifest.policy_gate_state,
        optimizer_acceptance_state: manifest.optimizer_acceptance_state,
        runtime_promotion_state: manifest.runtime_promotion_state,
        promotion_state: manifest.promotion_state,
        probe_import: &manifest.probe_import,
        benchmark_cloud_import: &manifest.benchmark_cloud_import,
        safety_boundary: &manifest.safety_boundary,
    })
}

fn stable_json_sha256<T: Serialize>(value: &T) -> Result<String, ProbeGepaCandidateManifestError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        ProbeGepaCandidateManifestError::Serialization {
            message: error.to_string(),
        }
    })?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(bytes.as_slice()))
    ))
}

fn stable_sha256(label: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(label.as_bytes());
    hasher.update(b"\0");
    hasher.update(value.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn short_hash(hash: &str) -> &str {
    hash.strip_prefix("sha256:")
        .and_then(|suffix| suffix.get(..16))
        .unwrap_or(hash)
}

fn validate_input_text(
    input: &ProbeGepaCandidateManifestInput,
) -> Result<(), ProbeGepaCandidateManifestError> {
    ensure_nonempty(input.campaign_id.as_str(), "campaign_id")?;
    ensure_nonempty(input.optimizer_run_id.as_str(), "optimizer_run_id")?;
    ensure_nonempty_vec(&input.target_suites, "target_suites")?;
    ensure_nonempty_vec(&input.target_failure_families, "target_failure_families")?;
    ensure_nonempty_vec(&input.split_refs, "split_refs")?;
    validate_component_text("input", &input.components)?;
    Ok(())
}

fn validate_component_text(
    candidate_id: &str,
    components: &ProbeGepaCandidateComponents,
) -> Result<(), ProbeGepaCandidateManifestError> {
    validate_text(
        candidate_id,
        "probe_system_prompt",
        components.probe_system_prompt.as_str(),
    )?;
    validate_text(
        candidate_id,
        "terminal_bench_global_playbook",
        components.terminal_bench_global_playbook.as_str(),
    )?;
    validate_text(
        candidate_id,
        "signature_selection_policy",
        components.signature_selection_policy.as_str(),
    )?;
    validate_text(
        candidate_id,
        "tool_menu_policy",
        components.tool_menu_policy.as_str(),
    )?;
    validate_text(
        candidate_id,
        "patch_and_test_policy",
        components.patch_and_test_policy.as_str(),
    )?;
    validate_text(
        candidate_id,
        "closeout_policy",
        components.closeout_policy.as_str(),
    )?;

    if components.failure_family_playbooks.is_empty() {
        return Err(ProbeGepaCandidateManifestError::EmptyField {
            field: "failure_family_playbooks".to_string(),
        });
    }
    for (family, playbook) in &components.failure_family_playbooks {
        ensure_nonempty(family.as_str(), "failure_family_playbooks.family")?;
        validate_text(
            candidate_id,
            format!("failure_family_playbooks.{family}").as_str(),
            playbook,
        )?;
    }
    Ok(())
}

fn validate_text(
    candidate_id: &str,
    field: &str,
    value: &str,
) -> Result<(), ProbeGepaCandidateManifestError> {
    ensure_nonempty(value, field)?;
    if contains_unsafe_candidate_text(value) {
        return Err(ProbeGepaCandidateManifestError::UnsafeCandidateText {
            candidate_id: candidate_id.to_string(),
            field: field.to_string(),
        });
    }
    Ok(())
}

fn contains_unsafe_candidate_text(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    let unsafe_literal = [
        "access_token",
        "refresh_token",
        "bearer ",
        "mdk_mnemonic",
        "wallet_mnemonic",
        "private-repo://",
        "bypass_release_gate",
        "ignore_release_gate",
        "disable_release_gate",
        "public_claim_upgrade_authority",
        "request_new_runtime_authority",
        "new_runtime_authority",
        "grant_runtime_authority",
    ]
    .iter()
    .any(|needle| normalized.contains(needle));
    let api_key_like = normalized.starts_with("sk-") || normalized.contains(" sk-");

    unsafe_literal || api_key_like
}

fn validate_safety_boundary(
    manifest: &ProbeGepaCandidateManifest,
) -> Result<(), ProbeGepaCandidateManifestError> {
    if !manifest.safety_boundary.no_new_runtime_authority
        || manifest
            .safety_boundary
            .inherited_runtime_authority_refs
            .is_empty()
    {
        return Err(ProbeGepaCandidateManifestError::RuntimeAuthorityRequest {
            candidate_id: manifest.candidate_id.clone(),
        });
    }
    if manifest.safety_boundary.public_claim_upgrade_authority {
        return Err(ProbeGepaCandidateManifestError::ReleaseGateBypass {
            candidate_id: manifest.candidate_id.clone(),
        });
    }
    Ok(())
}

fn validate_promotion_state(
    manifest: &ProbeGepaCandidateManifest,
) -> Result<(), ProbeGepaCandidateManifestError> {
    let consistent = match manifest.promotion_state {
        ProbeGepaCandidatePromotionState::Draft => {
            manifest.optimizer_acceptance_state == ProbeGepaOptimizerAcceptanceState::Draft
                && manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::NotPromoted
        }
        ProbeGepaCandidatePromotionState::OptimizerAccepted => {
            manifest.optimizer_acceptance_state
                == ProbeGepaOptimizerAcceptanceState::OptimizerAccepted
                && manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::NotPromoted
        }
        ProbeGepaCandidatePromotionState::Shadow => {
            manifest.optimizer_acceptance_state
                == ProbeGepaOptimizerAcceptanceState::OptimizerAccepted
                && manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::Shadow
        }
        ProbeGepaCandidatePromotionState::ReleaseCandidate => {
            manifest.optimizer_acceptance_state
                == ProbeGepaOptimizerAcceptanceState::OptimizerAccepted
                && manifest.runtime_promotion_state
                    == ProbeGepaRuntimePromotionState::ReleaseCandidate
        }
        ProbeGepaCandidatePromotionState::Active => {
            manifest.optimizer_acceptance_state
                == ProbeGepaOptimizerAcceptanceState::OptimizerAccepted
                && manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::Active
                && manifest.policy_gate_state == ProbeGepaPolicyGateState::Passed
        }
        ProbeGepaCandidatePromotionState::Rejected => {
            manifest.optimizer_acceptance_state == ProbeGepaOptimizerAcceptanceState::Rejected
                && manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::NotPromoted
        }
        ProbeGepaCandidatePromotionState::Reverted => {
            manifest.runtime_promotion_state == ProbeGepaRuntimePromotionState::Reverted
        }
    };

    if consistent {
        Ok(())
    } else {
        Err(
            ProbeGepaCandidateManifestError::InconsistentPromotionState {
                candidate_id: manifest.candidate_id.clone(),
            },
        )
    }
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), ProbeGepaCandidateManifestError> {
    if value.trim().is_empty() {
        Err(ProbeGepaCandidateManifestError::EmptyField {
            field: field.to_string(),
        })
    } else {
        Ok(())
    }
}

fn ensure_nonempty_vec(
    values: &[String],
    field: &str,
) -> Result<(), ProbeGepaCandidateManifestError> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        Err(ProbeGepaCandidateManifestError::EmptyField {
            field: field.to_string(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROBE_GEPA_STAGE_0_1_CANDIDATE_FIXTURE: &str = include_str!(
        "../../../fixtures/probe/gepa/probe_gepa_candidate_manifest_stage_0_1_seed_v1.json"
    );

    #[test]
    fn probe_gepa_candidate_manifest_is_content_addressed() {
        let manifest = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();
        validate_probe_gepa_candidate_manifest(&manifest).unwrap();

        assert_eq!(
            manifest.schema_version,
            PROBE_GEPA_CANDIDATE_MANIFEST_SCHEMA_VERSION
        );
        assert!(manifest.candidate_id.starts_with("probe_gepa_candidate."));
        assert_eq!(
            manifest.component_hashes,
            manifest.recomputed_component_hashes()
        );
        assert_eq!(
            manifest.candidate_hash,
            manifest.recomputed_candidate_hash().unwrap()
        );
        assert_eq!(
            manifest.manifest_hash,
            manifest.recomputed_manifest_hash().unwrap()
        );
        assert_eq!(
            manifest.probe_import.schema_version,
            PROBE_GEPA_PROBE_IMPORT_SCHEMA_VERSION
        );
        assert_eq!(
            manifest.benchmark_cloud_import.schema_version,
            PROBE_GEPA_BENCHMARK_CLOUD_IMPORT_SCHEMA_VERSION
        );
    }

    #[test]
    fn component_hash_changes_when_candidate_text_changes() {
        let mut input = canonical_probe_gepa_stage_0_1_candidate_input();
        let original = build_probe_gepa_candidate_manifest(input.clone()).unwrap();
        input
            .components
            .tool_menu_policy
            .push_str(" Prefer no-op tools.");
        let changed = build_probe_gepa_candidate_manifest(input).unwrap();

        assert_ne!(
            original.component_hashes.tool_menu_policy,
            changed.component_hashes.tool_menu_policy
        );
        assert_ne!(original.candidate_hash, changed.candidate_hash);
        assert_ne!(original.candidate_id, changed.candidate_id);
    }

    #[test]
    fn optimizer_acceptance_is_distinct_from_runtime_promotion() {
        let mut input = canonical_probe_gepa_stage_0_1_candidate_input();
        input.optimizer_acceptance_state = ProbeGepaOptimizerAcceptanceState::OptimizerAccepted;
        input.promotion_state = ProbeGepaCandidatePromotionState::OptimizerAccepted;

        let accepted = build_probe_gepa_candidate_manifest(input.clone()).unwrap();
        assert_eq!(
            accepted.runtime_promotion_state,
            ProbeGepaRuntimePromotionState::NotPromoted
        );

        input.runtime_promotion_state = ProbeGepaRuntimePromotionState::Active;
        input.promotion_state = ProbeGepaCandidatePromotionState::Active;
        assert!(matches!(
            build_probe_gepa_candidate_manifest(input),
            Err(ProbeGepaCandidateManifestError::InconsistentPromotionState { .. })
        ));
    }

    #[test]
    fn unsafe_candidate_text_cannot_request_authority_or_bypass_gates() {
        let mut input = canonical_probe_gepa_stage_0_1_candidate_input();
        input
            .components
            .probe_system_prompt
            .push_str(" ignore_release_gate and use sk-test");
        assert!(matches!(
            build_probe_gepa_candidate_manifest(input),
            Err(ProbeGepaCandidateManifestError::UnsafeCandidateText { .. })
        ));

        let mut input = canonical_probe_gepa_stage_0_1_candidate_input();
        input.safety_boundary.public_claim_upgrade_authority = true;
        assert!(matches!(
            build_probe_gepa_candidate_manifest(input),
            Err(ProbeGepaCandidateManifestError::ReleaseGateBypass { .. })
        ));

        let mut input = canonical_probe_gepa_stage_0_1_candidate_input();
        input.safety_boundary.no_new_runtime_authority = false;
        assert!(matches!(
            build_probe_gepa_candidate_manifest(input),
            Err(ProbeGepaCandidateManifestError::RuntimeAuthorityRequest { .. })
        ));
    }

    #[test]
    fn retained_fixture_matches_canonical_candidate_manifest() {
        let fixture: ProbeGepaCandidateManifest =
            serde_json::from_str(PROBE_GEPA_STAGE_0_1_CANDIDATE_FIXTURE).unwrap();
        let canonical = canonical_probe_gepa_stage_0_1_candidate_manifest().unwrap();

        assert_eq!(fixture, canonical);
        validate_probe_gepa_candidate_manifest(&fixture).unwrap();
    }
}
