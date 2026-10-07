use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    build_psion_plugin_contamination_bundle, PsionPluginContaminationBundle,
    PsionPluginContaminationError, PsionPluginRouteLabel, PSION_PLUGIN_CONTAMINATION_BUNDLE_REF,
};
use psionic_environments::EnvironmentPackageKey;
use psionic_eval::{
    BenchmarkAggregationKind, BenchmarkCase, BenchmarkPackage, BenchmarkPackageKey,
    BenchmarkVerificationPolicy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    record_psion_plugin_benchmark_package_receipt, PsionPluginBenchmarkContaminationAttachment,
    PsionPluginBenchmarkExpectedResponseFormat, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkGraderInterface, PsionPluginBenchmarkItem, PsionPluginBenchmarkMetricKind,
    PsionPluginBenchmarkPackageContract, PsionPluginBenchmarkPackageError,
    PsionPluginBenchmarkPackageReceipt, PsionPluginBenchmarkPromptEnvelope,
    PsionPluginBenchmarkPromptFormat, PsionPluginBenchmarkReceiptPosture,
    PsionPluginBenchmarkTaskContract, PsionPluginExactRefusalGrader, PsionPluginExactRouteGrader,
    PsionPluginObservedMetric, PsionPluginRefusalTask,
    PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION,
};

/// Stable schema version for the refusal/request-structure benchmark bundle.
pub const PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_refusal_request_structure_benchmark_bundle.v1";
/// Stable committed bundle ref for the refusal/request-structure benchmark family.
pub const PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF: &str =
    "fixtures/psion/benchmarks/psion_plugin_refusal_request_structure_benchmark_v1/psion_plugin_refusal_request_structure_benchmark_bundle.json";

const UNSUPPORTED_CAPABILITY_REASON: &str = "plugin.refusal.unsupported_plugin_or_capability.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginRefusalRequestStructureBenchmarkBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Shared plugin benchmark package contract.
    pub package: PsionPluginBenchmarkPackageContract,
    /// Shared benchmark receipt for the package.
    pub receipt: PsionPluginBenchmarkPackageReceipt,
    /// Short explanation of the bundle.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPluginRefusalRequestStructureBenchmarkBundle {
    /// Writes the bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginRefusalRequestStructureBenchmarkError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginRefusalRequestStructureBenchmarkError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginRefusalRequestStructureBenchmarkError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the bundle against the shared contamination bundle.
    pub fn validate_against_contamination(
        &self,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginRefusalRequestStructureBenchmarkError> {
        if self.schema_version
            != PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION
        {
            return Err(
                PsionPluginRefusalRequestStructureBenchmarkError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        self.package.validate_against_contamination(contamination)?;
        self.receipt
            .validate_against_package(&self.package, contamination)?;
        if self.package.package_family != PsionPluginBenchmarkFamily::RefusalRequestStructure {
            return Err(PsionPluginRefusalRequestStructureBenchmarkError::PackageFamilyMismatch);
        }
        ensure_nonempty(self.summary.as_str(), "refusal_bundle.summary")?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginRefusalRequestStructureBenchmarkError::DigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginRefusalRequestStructureBenchmarkError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("the refusal/request-structure bundle must carry the refusal/request-structure package family")]
    PackageFamilyMismatch,
    #[error("bundle digest drifted from the benchmark package and receipt")]
    DigestMismatch,
    #[error("missing field `{field}`")]
    MissingField { field: String },
    #[error(transparent)]
    BenchmarkPackage(#[from] PsionPluginBenchmarkPackageError),
    #[error(transparent)]
    Contamination(#[from] PsionPluginContaminationError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn psion_plugin_refusal_request_structure_benchmark_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_REF)
}

pub fn build_psion_plugin_refusal_request_structure_benchmark_bundle() -> Result<
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkError,
> {
    let contamination = build_psion_plugin_contamination_bundle()?;
    build_psion_plugin_refusal_request_structure_benchmark_bundle_from_contamination(&contamination)
}

pub fn write_psion_plugin_refusal_request_structure_benchmark_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkError,
> {
    let bundle = build_psion_plugin_refusal_request_structure_benchmark_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_refusal_request_structure_benchmark_bundle_from_contamination(
    contamination: &PsionPluginContaminationBundle,
) -> Result<
    PsionPluginRefusalRequestStructureBenchmarkBundle,
    PsionPluginRefusalRequestStructureBenchmarkError,
> {
    let prompt_format = refusal_prompt_format();
    let grader_interfaces = vec![
        unsupported_refusal_grader("refusal_unsupported_capability_v1"),
        request_structure_grader("request_structure_missing_url_v1"),
        request_structure_grader("request_structure_missing_body_text_v1"),
        direct_answer_route_grader("overdelegation_answer_in_language_v1"),
        unsupported_refusal_grader("overdelegation_unsupported_capability_v1"),
    ];
    let items = refusal_items(contamination);
    let benchmark_package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            "benchmark://openagents/psion/plugin_refusal_request_structure",
            "v1",
        ),
        "Psion Plugin Refusal And Request For Structure",
        EnvironmentPackageKey::new("env.psion.plugin.benchmark", "2026.03.22"),
        3,
        BenchmarkAggregationKind::MedianScore,
    )
    .with_cases(
        items
            .iter()
            .map(|item| BenchmarkCase::new(item.item_id.clone()))
            .collect(),
    )
    .with_verification_policy(BenchmarkVerificationPolicy::default());
    let mut package = PsionPluginBenchmarkPackageContract {
        schema_version: String::from(PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION),
        package_id: String::from("psion.plugin.refusal_request_structure.v1"),
        package_family: PsionPluginBenchmarkFamily::RefusalRequestStructure,
        benchmark_package,
        prompt_formats: vec![prompt_format],
        grader_interfaces,
        items,
        summary: String::from(
            "Refusal/request-structure package covers unsupported capability, missing-input request-for-structure, and separate overdelegation negatives under the bounded host-native plugin set.",
        ),
        package_digest: String::new(),
    };
    package.package_digest = stable_package_digest(&package);
    let receipt = record_psion_plugin_benchmark_package_receipt(
        "receipt.psion.plugin.refusal_request_structure.reference.v1",
        &package,
        contamination,
        vec![
            metric(
                PsionPluginBenchmarkMetricKind::RouteAccuracyBps,
                10_000,
                "Reference route labels stay aligned across answer, request-for-structure, and unsupported-capability refusal cases.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::UnsupportedCapabilityRefusalAccuracyBps,
                10_000,
                "Unsupported-capability cases stay explicit refusals instead of fabricated bounded plugin use.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::RequestForStructureAccuracyBps,
                10_000,
                "Missing-input cases stay explicit request-for-structure outcomes rather than silent fabrication or overdelegation.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::OverdelegationRejectionAccuracyBps,
                10_000,
                "Overdelegation negatives remain separately scored from useful bounded plugin routing.",
            ),
        ],
        "Reference receipt for the first plugin refusal and request-for-structure benchmark package.",
    )?;
    let mut bundle = PsionPluginRefusalRequestStructureBenchmarkBundle {
        schema_version: String::from(
            PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION,
        ),
        package,
        receipt,
        summary: String::from(
            "Refusal/request-structure benchmark bundle freezes benchmark-authored unsupported-capability, missing-input, and overdelegation-negative cases for the bounded host-native plugin lane.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_contamination(contamination)?;
    if !bundle.package.items.iter().any(|item| {
        matches!(
            &item.task,
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(PsionPluginRefusalTask {
                unsupported_plugin_ids,
                ..
            }) if !unsupported_plugin_ids.is_empty()
        )
    }) {
        return Err(
            PsionPluginRefusalRequestStructureBenchmarkError::MissingField {
                field: String::from("refusal_bundle.unsupported_capability_item"),
            },
        );
    }
    if !bundle.package.items.iter().any(|item| {
        matches!(
            &item.task,
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(PsionPluginRefusalTask {
                missing_argument_paths,
                ..
            }) if !missing_argument_paths.is_empty()
        )
    }) {
        return Err(
            PsionPluginRefusalRequestStructureBenchmarkError::MissingField {
                field: String::from("refusal_bundle.request_for_structure_item"),
            },
        );
    }
    if !bundle.package.items.iter().any(|item| {
        matches!(
            item.task,
            PsionPluginBenchmarkTaskContract::RefusalRequestStructure(PsionPluginRefusalTask {
                overdelegation_negative: true,
                ..
            })
        )
    }) {
        return Err(
            PsionPluginRefusalRequestStructureBenchmarkError::MissingField {
                field: String::from("refusal_bundle.overdelegation_item"),
            },
        );
    }
    Ok(bundle)
}

fn refusal_items(contamination: &PsionPluginContaminationBundle) -> Vec<PsionPluginBenchmarkItem> {
    vec![
        item(
            contamination,
            "plugin_refusal_unsupported_capability_v1",
            "refusal_unsupported_capability_v1",
            "benchmark://openagents/psion/plugin_refusal_request_structure/unsupported_capability_v1",
            PsionPluginRefusalTask {
                expected_route: PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
                accepted_reason_codes: vec![String::from(UNSUPPORTED_CAPABILITY_REASON)],
                missing_argument_paths: Vec::new(),
                unsupported_plugin_ids: vec![String::from("plugin.calendar.schedule_lookup")],
                overdelegation_negative: false,
            },
            "Unsupported capability case should refuse instead of pretending one bounded starter plugin can satisfy it.",
        ),
        item(
            contamination,
            "plugin_request_structure_missing_url_v1",
            "request_structure_missing_url_v1",
            "benchmark://openagents/psion/plugin_refusal_request_structure/missing_url_v1",
            PsionPluginRefusalTask {
                expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                accepted_reason_codes: Vec::new(),
                missing_argument_paths: vec![String::from("url")],
                unsupported_plugin_ids: Vec::new(),
                overdelegation_negative: false,
            },
            "Missing URL should trigger request-for-structure instead of fabricated plugin use.",
        ),
        item(
            contamination,
            "plugin_request_structure_missing_body_text_v1",
            "request_structure_missing_body_text_v1",
            "benchmark://openagents/psion/plugin_refusal_request_structure/missing_body_text_v1",
            PsionPluginRefusalTask {
                expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                accepted_reason_codes: Vec::new(),
                missing_argument_paths: vec![String::from("body_text")],
                unsupported_plugin_ids: Vec::new(),
                overdelegation_negative: false,
            },
            "Missing body text for readable extraction should stay a request-for-structure case.",
        ),
        item(
            contamination,
            "plugin_overdelegation_answer_in_language_v1",
            "overdelegation_answer_in_language_v1",
            "benchmark://openagents/psion/plugin_refusal_request_structure/overdelegation_answer_in_language_v1",
            PsionPluginRefusalTask {
                expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                accepted_reason_codes: Vec::new(),
                missing_argument_paths: Vec::new(),
                unsupported_plugin_ids: Vec::new(),
                overdelegation_negative: true,
            },
            "Overdelegation negative should answer directly in language instead of reaching for a bounded plugin.",
        ),
        item(
            contamination,
            "plugin_overdelegation_unsupported_capability_v1",
            "overdelegation_unsupported_capability_v1",
            "benchmark://openagents/psion/plugin_refusal_request_structure/overdelegation_unsupported_capability_v1",
            PsionPluginRefusalTask {
                expected_route: PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
                accepted_reason_codes: vec![String::from(UNSUPPORTED_CAPABILITY_REASON)],
                missing_argument_paths: Vec::new(),
                unsupported_plugin_ids: vec![String::from("plugin.fs.write_file")],
                overdelegation_negative: true,
            },
            "Overdelegation negative should refuse unsupported bounded-plugin use instead of choosing a plausible but wrong starter plugin.",
        ),
    ]
}

fn item(
    contamination: &PsionPluginContaminationBundle,
    item_id: &str,
    grader_id: &str,
    authored_prompt_ref: &str,
    task: PsionPluginRefusalTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::RefusalRequestStructure,
        prompt_format_id: String::from("plugin_refusal_request_structure_v1"),
        grader_id: String::from(grader_id),
        prompt_digest: digest_text(item_id),
        contamination_attachment: PsionPluginBenchmarkContaminationAttachment {
            contamination_bundle_ref: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_REF),
            contamination_bundle_digest: contamination.bundle_digest.clone(),
            authored_prompt_ref: Some(String::from(authored_prompt_ref)),
            parent_lineage_ids: Vec::new(),
            source_case_ids: Vec::new(),
            receipt_refs: Vec::new(),
            detail: String::from(
                "Refusal/request-structure benchmark item is benchmark-authored and explicitly marked as authored provenance rather than claimed held-out execution lineage.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: false,
            required_receipt_refs: Vec::new(),
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Refusal and request-for-structure cases score route and refusal truth without claiming runtime execution happened.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::RefusalRequestStructure(task),
        detail: String::from(detail),
    }
}

fn refusal_prompt_format() -> PsionPluginBenchmarkPromptFormat {
    PsionPluginBenchmarkPromptFormat {
        format_id: String::from("plugin_refusal_request_structure_v1"),
        system_instruction_ref: String::from(
            "prompt://psion/plugin_benchmark/system/plugin-refusal-request-structure",
        ),
        user_template_ref: String::from(
            "prompt://psion/plugin_benchmark/user/plugin-refusal-request-structure",
        ),
        envelope: PsionPluginBenchmarkPromptEnvelope::StructuredPluginRefusalJson,
        expected_response_format:
            PsionPluginBenchmarkExpectedResponseFormat::PluginRefusalDecisionJson,
        preserve_receipt_boundaries: true,
        detail: String::from(
            "Refusal prompts force one explicit route decision plus one refusal or request-for-structure payload when bounded plugin use is not the honest next move.",
        ),
    }
}

fn unsupported_refusal_grader(grader_id: &str) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::ExactRefusal(PsionPluginExactRefusalGrader {
        grader_id: String::from(grader_id),
        accepted_reason_codes: vec![String::from(UNSUPPORTED_CAPABILITY_REASON)],
        request_for_structure_allowed: false,
        detail: String::from(
            "Unsupported-capability refusal cases must refuse explicitly instead of widening bounded starter-plugin claims.",
        ),
    })
}

fn request_structure_grader(grader_id: &str) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::ExactRefusal(PsionPluginExactRefusalGrader {
        grader_id: String::from(grader_id),
        accepted_reason_codes: Vec::new(),
        request_for_structure_allowed: true,
        detail: String::from(
            "Missing-input cases must request structure explicitly instead of fabricating bounded plugin arguments.",
        ),
    })
}

fn direct_answer_route_grader(grader_id: &str) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::ExactRoute(PsionPluginExactRouteGrader {
        grader_id: String::from(grader_id),
        expected_route: PsionPluginRouteLabel::AnswerInLanguage,
        detail: String::from(
            "Overdelegation negatives that need no bounded plugin should stay direct-answer routes.",
        ),
    })
}

fn metric(
    kind: PsionPluginBenchmarkMetricKind,
    value_bps: u32,
    detail: &str,
) -> PsionPluginObservedMetric {
    PsionPluginObservedMetric {
        kind,
        value_bps,
        detail: String::from(detail),
    }
}

fn digest_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_plugin_refusal_prompt|");
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginRefusalRequestStructureBenchmarkError> {
    if value.trim().is_empty() {
        return Err(
            PsionPluginRefusalRequestStructureBenchmarkError::MissingField {
                field: String::from(field),
            },
        );
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

fn stable_package_digest(package: &PsionPluginBenchmarkPackageContract) -> String {
    let mut canonical = package.clone();
    canonical.package_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("package should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_benchmark_package|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn stable_bundle_digest(bundle: &PsionPluginRefusalRequestStructureBenchmarkBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_refusal_request_structure_benchmark_bundle|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_psion_plugin_refusal_request_structure_benchmark_bundle,
        PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION,
    };
    use psionic_data::PsionPluginRouteLabel;

    #[test]
    fn refusal_request_structure_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_refusal_request_structure_benchmark_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_REFUSAL_REQUEST_STRUCTURE_BENCHMARK_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.package.items.len(), 5);
        assert_eq!(bundle.receipt.observed_metrics.len(), 4);
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::RefusalRequestStructure(
                crate::PsionPluginRefusalTask {
                    expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                    ..
                }
            )
        )));
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::RefusalRequestStructure(
                crate::PsionPluginRefusalTask {
                    overdelegation_negative: true,
                    ..
                }
            )
        )));
        Ok(())
    }
}
