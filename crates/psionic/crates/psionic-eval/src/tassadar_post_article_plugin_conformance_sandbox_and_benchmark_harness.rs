use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_sandbox::{
    build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report,
    TassadarPostArticlePluginBenchmarkHarnessReportRow,
    TassadarPostArticlePluginConformanceHarnessDependencyClass,
    TassadarPostArticlePluginConformanceHarnessReportRow,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError,
    TassadarPostArticlePluginIsolationNegativeReportRow,
    TassadarPostArticlePluginWorkflowHarnessReportRow,
    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-conformance-sandbox-and-benchmark-harness.sh";

const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginConformanceEvalDependencyClass {
    SandboxPrecedent,
    SupportingPrecedent,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceEvalMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub sandbox_report_id: String,
    pub sandbox_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub conformance_harness_id: String,
    pub benchmark_harness_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceEvalDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginConformanceEvalDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceEvalValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub sandbox_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginConformanceEvalMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticlePluginConformanceEvalDependencyRow>,
    pub conformance_rows: Vec<TassadarPostArticlePluginConformanceHarnessReportRow>,
    pub workflow_rows: Vec<TassadarPostArticlePluginWorkflowHarnessReportRow>,
    pub isolation_negative_rows: Vec<TassadarPostArticlePluginIsolationNegativeReportRow>,
    pub benchmark_rows: Vec<TassadarPostArticlePluginBenchmarkHarnessReportRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginConformanceEvalValidationRow>,
    pub contract_status: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus,
    pub contract_green: bool,
    pub conformance_sandbox_green: bool,
    pub cold_path_benchmarked: bool,
    pub warm_path_benchmarked: bool,
    pub pooled_path_benchmarked: bool,
    pub queued_path_benchmarked: bool,
    pub cancelled_path_benchmarked: bool,
    pub queue_saturation_explicit: bool,
    pub cancellation_latency_bounded: bool,
    pub evidence_overhead_explicit: bool,
    pub timeout_enforcement_measured: bool,
    pub receipt_integrity_and_envelope_compatibility_explicit: bool,
    pub operator_internal_only_posture: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError {
    #[error(transparent)]
    Sandbox(#[from] TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report(
) -> Result<
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError,
> {
    let sandbox =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report()?;

    let machine_identity_binding = TassadarPostArticlePluginConformanceEvalMachineIdentityBinding {
        machine_identity_id: sandbox
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: sandbox.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: sandbox.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: sandbox
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: sandbox
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: sandbox
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: sandbox
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: sandbox
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: sandbox
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        sandbox_report_id: sandbox.report_id.clone(),
        sandbox_report_digest: sandbox.report_digest.clone(),
        packet_abi_version: sandbox.machine_identity_binding.packet_abi_version.clone(),
        host_owned_runtime_api_id: sandbox
            .machine_identity_binding
            .host_owned_runtime_api_id
            .clone(),
        engine_abstraction_id: sandbox
            .machine_identity_binding
            .engine_abstraction_id
            .clone(),
        invocation_receipt_profile_id: sandbox
            .machine_identity_binding
            .invocation_receipt_profile_id
            .clone(),
        world_mount_envelope_compiler_id: sandbox
            .machine_identity_binding
            .world_mount_envelope_compiler_id
            .clone(),
        admissibility_contract_id: sandbox
            .machine_identity_binding
            .admissibility_contract_id
            .clone(),
        conformance_harness_id: sandbox
            .machine_identity_binding
            .conformance_harness_id
            .clone(),
        benchmark_harness_id: sandbox
            .machine_identity_binding
            .benchmark_harness_id
            .clone(),
        runtime_bundle_id: sandbox.machine_identity_binding.runtime_bundle_id.clone(),
        runtime_bundle_digest: sandbox
            .machine_identity_binding
            .runtime_bundle_digest
            .clone(),
        runtime_bundle_ref: sandbox.machine_identity_binding.runtime_bundle_ref.clone(),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` sandbox_report_id=`{}` and runtime_bundle_id=`{}` remain bound together.",
            sandbox.machine_identity_binding.machine_identity_id,
            sandbox.machine_identity_binding.canonical_route_id,
            sandbox.report_id,
            sandbox.machine_identity_binding.runtime_bundle_id,
        ),
    };

    let mut dependency_rows = vec![dependency_row(
        "sandbox_contract_green",
        TassadarPostArticlePluginConformanceEvalDependencyClass::SandboxPrecedent,
        sandbox.contract_green,
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
        Some(sandbox.report_id.clone()),
        Some(sandbox.report_digest.clone()),
        "the sandbox-owned conformance contract is green and carries the declared conformance, workflow, isolation, and benchmark rows.",
    )];
    dependency_rows.extend(sandbox.dependency_rows.iter().map(|row| {
        TassadarPostArticlePluginConformanceEvalDependencyRow {
            dependency_id: format!("sandbox.{}", row.dependency_id),
            dependency_class: match row.dependency_class {
                TassadarPostArticlePluginConformanceHarnessDependencyClass::DesignInput => {
                    TassadarPostArticlePluginConformanceEvalDependencyClass::DesignInput
                }
                _ => TassadarPostArticlePluginConformanceEvalDependencyClass::SupportingPrecedent,
            },
            satisfied: row.satisfied,
            source_ref: row.source_ref.clone(),
            bound_report_id: row.bound_report_id.clone(),
            bound_report_digest: row.bound_report_digest.clone(),
            detail: row.detail.clone(),
        }
    }));

    let conformance_sandbox_green = sandbox.contract_green;
    let cold_path_benchmarked = has_benchmark_row(&sandbox, "cold_instantiate");
    let warm_path_benchmarked = has_benchmark_row(&sandbox, "warm_invoke");
    let pooled_path_benchmarked = has_benchmark_row(&sandbox, "pooled_reuse");
    let queued_path_benchmarked = has_benchmark_row(&sandbox, "queued_saturation");
    let cancelled_path_benchmarked = has_benchmark_row(&sandbox, "cancelled_path");
    let queue_saturation_explicit = sandbox
        .benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "queued_saturation" && row.green && row.queue_depth > 0);
    let cancellation_latency_bounded = sandbox
        .benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "cancelled_path" && row.green && row.cancel_visible);
    let evidence_overhead_explicit = sandbox
        .benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "evidence_overhead" && row.green);
    let timeout_enforcement_measured = sandbox
        .benchmark_rows
        .iter()
        .any(|row| row.path_class_id == "timeout_enforcement" && row.green);
    let receipt_integrity_and_envelope_compatibility_explicit =
        sandbox.receipt_integrity_frozen && sandbox.envelope_compatibility_explicit;
    let operator_internal_only_posture = sandbox.operator_internal_only_posture
        && sandbox.rebase_claim_allowed
        && !sandbox.plugin_capability_claim_allowed
        && !sandbox.plugin_publication_allowed;

    let validation_rows = vec![
        validation_row(
            "sandbox_contract_green",
            sandbox.contract_green,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "the sandbox-owned conformance harness remains green and machine-legible.",
        ),
        validation_row(
            "supporting_material_dependencies_green",
            sandbox.dependency_rows.iter().all(|row| row.satisfied),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            ],
            "the supporting-material dependency rows carried through the sandbox report remain green.",
        ),
        validation_row(
            "roundtrip_and_refusal_rows_green",
            sandbox.conformance_rows.iter().all(|row| row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "roundtrip, refusal, timeout, memory-limit, packet-size, digest-mismatch, replay, and hot-swap rows remain green.",
        ),
        validation_row(
            "workflow_integrity_rows_green",
            sandbox.workflow_rows.iter().all(|row| row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "workflow integrity, refusal propagation, envelope intersection, hot-swap, and partial-cancellation replay rows remain green.",
        ),
        validation_row(
            "isolation_negative_rows_green",
            sandbox.isolation_negative_rows.iter().all(|row| row.green),
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "failure-domain, side-channel, and covert-channel negative rows remain green.",
        ),
        validation_row(
            "benchmark_paths_measured",
            cold_path_benchmarked
                && warm_path_benchmarked
                && pooled_path_benchmarked
                && queued_path_benchmarked
                && cancelled_path_benchmarked,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "cold, warm, pooled, queued, and cancelled benchmark paths remain explicit and green.",
        ),
        validation_row(
            "queue_and_cancellation_explicit",
            queue_saturation_explicit && cancellation_latency_bounded,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "queue saturation and cancellation latency remain explicit instead of being inferred from generic success rows.",
        ),
        validation_row(
            "evidence_overhead_and_timeout_explicit",
            evidence_overhead_explicit && timeout_enforcement_measured,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "evidence overhead and timeout enforcement remain explicit benchmark rows.",
        ),
        validation_row(
            "receipt_integrity_and_envelope_compatibility_explicit",
            receipt_integrity_and_envelope_compatibility_explicit,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "receipt integrity and envelope compatibility remain explicit across conformance and workflow rows.",
        ),
        validation_row(
            "host_scripted_conformance_only",
            sandbox.static_harness_only && sandbox.host_scripted_trace_only,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "the conformance harness stays static and host-scripted rather than model-owned sequencing.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            operator_internal_only_posture,
            &[TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF],
            "the eval-owned harness stays operator/internal-only and does not imply weighted plugin sequencing, publication, served/public universality, or arbitrary software capability.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && conformance_sandbox_green
        && cold_path_benchmarked
        && warm_path_benchmarked
        && pooled_path_benchmarked
        && queued_path_benchmarked
        && cancelled_path_benchmarked
        && queue_saturation_explicit
        && cancellation_latency_bounded
        && evidence_overhead_explicit
        && timeout_enforcement_measured
        && receipt_integrity_and_envelope_compatibility_explicit
        && operator_internal_only_posture;
    let contract_status = if contract_green {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus::Green
    } else {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus::Incomplete
    };
    let rebase_claim_allowed = contract_green;

    let mut report = TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_conformance_sandbox_and_benchmark_harness.eval_report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_CHECKER_REF,
        ),
        sandbox_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
        ),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: sandbox
            .supporting_material_refs
            .iter()
            .cloned()
            .chain(std::iter::once(String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            )))
            .collect(),
        machine_identity_binding,
        dependency_rows,
        conformance_rows: sandbox.conformance_rows.clone(),
        workflow_rows: sandbox.workflow_rows.clone(),
        isolation_negative_rows: sandbox.isolation_negative_rows.clone(),
        benchmark_rows: sandbox.benchmark_rows.clone(),
        validation_rows,
        contract_status,
        contract_green,
        conformance_sandbox_green,
        cold_path_benchmarked,
        warm_path_benchmarked,
        pooled_path_benchmarked,
        queued_path_benchmarked,
        cancelled_path_benchmarked,
        queue_saturation_explicit,
        cancellation_latency_bounded,
        evidence_overhead_explicit,
        timeout_enforcement_measured,
        receipt_integrity_and_envelope_compatibility_explicit,
        operator_internal_only_posture,
        rebase_claim_allowed,
        plugin_capability_claim_allowed: false,
        weighted_plugin_control_allowed: false,
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        deferred_issue_ids: vec![],
        claim_boundary: String::from(
            "this eval-owned harness freezes the canonical post-article plugin conformance sandbox and benchmark evidence above the admissibility contract. It keeps static host-scripted conformance rows, workflow rows, failure-domain and side-channel negative rows, and cold or warm or pooled or queued or cancelled benchmark paths machine-readable while keeping weighted plugin sequencing, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article plugin conformance eval report keeps contract_status={:?}, dependency_rows={}, conformance_rows={}, workflow_rows={}, isolation_negative_rows={}, benchmark_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.conformance_rows.len(),
        report.workflow_rows.len(),
        report.isolation_negative_rows.len(),
        report.benchmark_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginConformanceEvalDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginConformanceEvalDependencyRow {
    TassadarPostArticlePluginConformanceEvalDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginConformanceEvalValidationRow {
    TassadarPostArticlePluginConformanceEvalValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn has_benchmark_row(
    sandbox: &TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    path_class_id: &str,
) -> bool {
    sandbox
        .benchmark_rows
        .iter()
        .any(|row| row.path_class_id == path_class_id && row.green)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report,
        read_repo_json,
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report_path,
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report,
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport,
        TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
    };

    #[test]
    fn post_article_plugin_conformance_eval_report_keeps_frontier_explicit() {
        let report =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalStatus::Green
        );
        assert_eq!(
            report.report_id,
            "tassadar.post_article_plugin_conformance_sandbox_and_benchmark_harness.eval_report.v1"
        );
        assert_eq!(report.dependency_rows.len(), 7);
        assert_eq!(report.conformance_rows.len(), 9);
        assert_eq!(report.workflow_rows.len(), 5);
        assert_eq!(report.isolation_negative_rows.len(), 8);
        assert_eq!(report.benchmark_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 11);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.conformance_sandbox_green);
        assert!(report.cold_path_benchmarked);
        assert!(report.warm_path_benchmarked);
        assert!(report.pooled_path_benchmarked);
        assert!(report.queued_path_benchmarked);
        assert!(report.cancelled_path_benchmarked);
        assert!(report.queue_saturation_explicit);
        assert!(report.cancellation_latency_bounded);
        assert!(report.evidence_overhead_explicit);
        assert!(report.timeout_enforcement_measured);
        assert!(report.receipt_integrity_and_envelope_compatibility_explicit);
        assert!(report.operator_internal_only_posture);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_conformance_eval_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report()
                .expect("report");
        let committed: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_conformance_eval_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report.json",
        );
        let written =
            write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
