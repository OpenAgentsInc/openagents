use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    build_psion_plugin_contamination_bundle, PsionPluginContaminationBundle,
    PsionPluginContaminationError, PsionPluginContaminationItemKind, PsionPluginRouteLabel,
    PSION_PLUGIN_CONTAMINATION_BUNDLE_REF,
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
    PsionPluginBenchmarkTaskContract, PsionPluginContinuationPosture, PsionPluginObservedMetric,
    PsionPluginSequenceMode, PsionPluginSequencePlanGrader, PsionPluginSequencingTask,
    PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION,
};

/// Stable schema version for the sequencing benchmark bundle.
pub const PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_sequencing_benchmark_bundle.v1";
/// Stable committed bundle ref for the sequencing benchmark family.
pub const PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_REF: &str =
    "fixtures/psion/benchmarks/psion_plugin_sequencing_benchmark_v1/psion_plugin_sequencing_benchmark_bundle.json";

const FETCH_TEXT_TOOL_NAME: &str = "plugin_http_fetch_text";
const HTML_EXTRACT_TOOL_NAME: &str = "plugin_html_extract_readable";
const RSS_PARSE_TOOL_NAME: &str = "plugin_feed_rss_atom_parse";
const TEXT_URL_EXTRACT_TOOL_NAME: &str = "plugin_text_url_extract";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginSequencingBenchmarkBundle {
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

impl PsionPluginSequencingBenchmarkBundle {
    /// Writes the bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginSequencingBenchmarkError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginSequencingBenchmarkError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginSequencingBenchmarkError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the bundle against the shared contamination bundle.
    pub fn validate_against_contamination(
        &self,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginSequencingBenchmarkError> {
        if self.schema_version != PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION {
            return Err(PsionPluginSequencingBenchmarkError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        self.package.validate_against_contamination(contamination)?;
        self.receipt
            .validate_against_package(&self.package, contamination)?;
        if self.package.package_family != PsionPluginBenchmarkFamily::SequencingMultiCall {
            return Err(PsionPluginSequencingBenchmarkError::PackageFamilyMismatch);
        }
        ensure_nonempty(self.summary.as_str(), "sequencing_bundle.summary")?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginSequencingBenchmarkError::DigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginSequencingBenchmarkError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("the sequencing bundle must carry the sequencing package family")]
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
pub fn psion_plugin_sequencing_benchmark_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_REF)
}

pub fn build_psion_plugin_sequencing_benchmark_bundle(
) -> Result<PsionPluginSequencingBenchmarkBundle, PsionPluginSequencingBenchmarkError> {
    let contamination = build_psion_plugin_contamination_bundle()?;
    build_psion_plugin_sequencing_benchmark_bundle_from_contamination(&contamination)
}

pub fn write_psion_plugin_sequencing_benchmark_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginSequencingBenchmarkBundle, PsionPluginSequencingBenchmarkError> {
    let bundle = build_psion_plugin_sequencing_benchmark_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_sequencing_benchmark_bundle_from_contamination(
    contamination: &PsionPluginContaminationBundle,
) -> Result<PsionPluginSequencingBenchmarkBundle, PsionPluginSequencingBenchmarkError> {
    let runtime_refusal_row = held_out_fetch_refusal_row(contamination)?;
    let prompt_format = sequencing_prompt_format();
    let grader_interfaces = vec![
        sequencing_grader(
            "sequence_fetch_then_extract_readable_v1",
            vec![
                String::from(FETCH_TEXT_TOOL_NAME),
                String::from(HTML_EXTRACT_TOOL_NAME),
            ],
            PsionPluginSequenceMode::Serial,
            PsionPluginContinuationPosture::StopAfterGoalSatisfied,
            "Readable extraction requires a serial fetch-then-extract plan and should stop after the requested article text is produced.",
        ),
        sequencing_grader(
            "sequence_fetch_then_parse_feed_v1",
            vec![
                String::from(FETCH_TEXT_TOOL_NAME),
                String::from(RSS_PARSE_TOOL_NAME),
            ],
            PsionPluginSequenceMode::Serial,
            PsionPluginContinuationPosture::StopAfterGoalSatisfied,
            "Feed parsing requires a serial fetch-then-parse plan and should stop when the feed entries are available.",
        ),
        sequencing_grader(
            "sequence_parallel_fetch_and_url_extract_v1",
            vec![
                String::from(FETCH_TEXT_TOOL_NAME),
                String::from(TEXT_URL_EXTRACT_TOOL_NAME),
            ],
            PsionPluginSequenceMode::Parallelizable,
            PsionPluginContinuationPosture::StopAfterGoalSatisfied,
            "Independent fetch and URL-extraction work should not be overconstrained to one serial order.",
        ),
        sequencing_grader(
            "sequence_full_pipeline_continue_v1",
            vec![
                String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                String::from(FETCH_TEXT_TOOL_NAME),
                String::from(HTML_EXTRACT_TOOL_NAME),
            ],
            PsionPluginSequenceMode::Serial,
            PsionPluginContinuationPosture::ContinueUntilAdmissionSetExhausted,
            "Full-pipeline cases should continue through all requested bounded stages instead of stopping after the first partial success.",
        ),
        sequencing_grader(
            "sequence_runtime_refusal_stop_v1",
            vec![
                String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                String::from(FETCH_TEXT_TOOL_NAME),
            ],
            PsionPluginSequenceMode::Serial,
            PsionPluginContinuationPosture::StopOnTypedRuntimeRefusal,
            "Receipt-backed refusal cases must stop on the typed runtime refusal boundary rather than continuing past it.",
        ),
    ];
    let items = sequencing_items(contamination, runtime_refusal_row);
    let benchmark_package = BenchmarkPackage::new(
        BenchmarkPackageKey::new("benchmark://openagents/psion/plugin_sequencing", "v1"),
        "Psion Plugin Sequencing And Multi-Call",
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
        package_id: String::from("psion.plugin.sequencing.v1"),
        package_family: PsionPluginBenchmarkFamily::SequencingMultiCall,
        benchmark_package,
        prompt_formats: vec![prompt_format],
        grader_interfaces,
        items,
        summary: String::from(
            "Sequencing package covers serial pipelines, parallelizable multi-call work, continuation posture, and one held-out typed-runtime-refusal stop boundary.",
        ),
        package_digest: String::new(),
    };
    package.package_digest = stable_package_digest(&package);
    let receipt = record_psion_plugin_benchmark_package_receipt(
        "receipt.psion.plugin.sequencing.reference.v1",
        &package,
        contamination,
        vec![
            metric(
                PsionPluginBenchmarkMetricKind::SequencePlanAccuracyBps,
                10_000,
                "Reference sequence plans stay aligned for serial and parallelizable multi-call cases.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::ContinuationBoundaryAccuracyBps,
                10_000,
                "Reference continuation choices keep stop-after-goal, continue-until-exhausted, and stop-on-refusal boundaries distinct.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::TypedRuntimeRefusalAccuracyBps,
                10_000,
                "Receipt-backed refusal stop boundaries remain visible as a separate scored surface.",
            ),
        ],
        "Reference receipt for the first plugin sequencing and multi-call benchmark package.",
    )?;
    let mut bundle = PsionPluginSequencingBenchmarkBundle {
        schema_version: String::from(PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION),
        package,
        receipt,
        summary: String::from(
            "Sequencing benchmark bundle freezes benchmark-authored serial and parallelizable plans plus one held-out receipt-backed refusal-stop case.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_contamination(contamination)?;
    if !bundle.package.items.iter().any(|item| {
        matches!(
            item.task,
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(PsionPluginSequencingTask {
                sequence_mode: PsionPluginSequenceMode::Parallelizable,
                ..
            })
        )
    }) {
        return Err(PsionPluginSequencingBenchmarkError::MissingField {
            field: String::from("sequencing_bundle.parallelizable_item"),
        });
    }
    if !bundle.package.items.iter().any(|item| {
        matches!(
            item.task,
            PsionPluginBenchmarkTaskContract::SequencingMultiCall(PsionPluginSequencingTask {
                continuation_posture: PsionPluginContinuationPosture::StopOnTypedRuntimeRefusal,
                ..
            })
        )
    }) {
        return Err(PsionPluginSequencingBenchmarkError::MissingField {
            field: String::from("sequencing_bundle.stop_on_typed_runtime_refusal_item"),
        });
    }
    Ok(bundle)
}

fn held_out_fetch_refusal_row<'a>(
    contamination: &'a PsionPluginContaminationBundle,
) -> Result<&'a psionic_data::PsionPluginParentLineageRow, PsionPluginSequencingBenchmarkError> {
    contamination
        .parent_lineage_rows
        .iter()
        .find(|row| {
            row.item_kind == PsionPluginContaminationItemKind::HeldOutEvalRecord
                && row.source_trace.source_case_id == "web_content_intake_fetch_refusal"
        })
        .ok_or_else(|| PsionPluginSequencingBenchmarkError::MissingField {
            field: String::from("contamination_bundle.held_out.web_content_intake_fetch_refusal"),
        })
}

fn sequencing_items(
    contamination: &PsionPluginContaminationBundle,
    runtime_refusal_row: &psionic_data::PsionPluginParentLineageRow,
) -> Vec<PsionPluginBenchmarkItem> {
    vec![
        authored_item(
            contamination,
            "plugin_sequence_fetch_then_extract_readable_v1",
            "sequence_fetch_then_extract_readable_v1",
            "benchmark://openagents/psion/plugin_sequencing/fetch_then_extract_readable_v1",
            PsionPluginSequencingTask {
                admitted_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(HTML_EXTRACT_TOOL_NAME),
                ],
                expected_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(HTML_EXTRACT_TOOL_NAME),
                ],
                sequence_mode: PsionPluginSequenceMode::Serial,
                continuation_posture: PsionPluginContinuationPosture::StopAfterGoalSatisfied,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Readable extraction should fetch first and then extract readable text before stopping.",
        ),
        authored_item(
            contamination,
            "plugin_sequence_fetch_then_parse_feed_v1",
            "sequence_fetch_then_parse_feed_v1",
            "benchmark://openagents/psion/plugin_sequencing/fetch_then_parse_feed_v1",
            PsionPluginSequencingTask {
                admitted_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(RSS_PARSE_TOOL_NAME),
                ],
                expected_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(RSS_PARSE_TOOL_NAME),
                ],
                sequence_mode: PsionPluginSequenceMode::Serial,
                continuation_posture: PsionPluginContinuationPosture::StopAfterGoalSatisfied,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Feed parsing should fetch the feed text before running the parser and then stop.",
        ),
        authored_item(
            contamination,
            "plugin_sequence_parallel_fetch_and_url_extract_v1",
            "sequence_parallel_fetch_and_url_extract_v1",
            "benchmark://openagents/psion/plugin_sequencing/parallel_fetch_and_url_extract_v1",
            PsionPluginSequencingTask {
                admitted_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                ],
                expected_tool_names: vec![
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                ],
                sequence_mode: PsionPluginSequenceMode::Parallelizable,
                continuation_posture: PsionPluginContinuationPosture::StopAfterGoalSatisfied,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Independent fetch and URL-extraction work should stay parallelizable instead of being needlessly serialized.",
        ),
        authored_item(
            contamination,
            "plugin_sequence_full_pipeline_continue_v1",
            "sequence_full_pipeline_continue_v1",
            "benchmark://openagents/psion/plugin_sequencing/full_pipeline_continue_v1",
            PsionPluginSequencingTask {
                admitted_tool_names: vec![
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(HTML_EXTRACT_TOOL_NAME),
                ],
                expected_tool_names: vec![
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                    String::from(FETCH_TEXT_TOOL_NAME),
                    String::from(HTML_EXTRACT_TOOL_NAME),
                ],
                sequence_mode: PsionPluginSequenceMode::Serial,
                continuation_posture:
                    PsionPluginContinuationPosture::ContinueUntilAdmissionSetExhausted,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Full-pipeline requests should continue through each requested bounded stage instead of stopping after the first partial result.",
        ),
        runtime_refusal_item(
            contamination,
            runtime_refusal_row,
            "plugin_sequence_runtime_refusal_stop_v1",
            PsionPluginSequencingTask {
                admitted_tool_names: vec![
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                    String::from(FETCH_TEXT_TOOL_NAME),
                ],
                expected_tool_names: vec![
                    String::from(TEXT_URL_EXTRACT_TOOL_NAME),
                    String::from(FETCH_TEXT_TOOL_NAME),
                ],
                sequence_mode: PsionPluginSequenceMode::Serial,
                continuation_posture: PsionPluginContinuationPosture::StopOnTypedRuntimeRefusal,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Held-out refusal-stop case keeps the sequence and stop boundary tied to real runtime receipts.",
        ),
    ]
}

fn authored_item(
    contamination: &PsionPluginContaminationBundle,
    item_id: &str,
    grader_id: &str,
    authored_prompt_ref: &str,
    task: PsionPluginSequencingTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::SequencingMultiCall,
        prompt_format_id: String::from("plugin_sequence_plan_v1"),
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
                "Sequencing benchmark item is benchmark-authored and explicitly marked as authored provenance rather than claimed held-out execution lineage.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: false,
            required_receipt_refs: Vec::new(),
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Benchmark-authored sequencing cases score plan structure and continuation without claiming runtime execution happened.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::SequencingMultiCall(task),
        detail: String::from(detail),
    }
}

fn runtime_refusal_item(
    contamination: &PsionPluginContaminationBundle,
    runtime_refusal_row: &psionic_data::PsionPluginParentLineageRow,
    item_id: &str,
    task: PsionPluginSequencingTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::SequencingMultiCall,
        prompt_format_id: String::from("plugin_sequence_plan_v1"),
        grader_id: String::from("sequence_runtime_refusal_stop_v1"),
        prompt_digest: digest_text(item_id),
        contamination_attachment: PsionPluginBenchmarkContaminationAttachment {
            contamination_bundle_ref: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_REF),
            contamination_bundle_digest: contamination.bundle_digest.clone(),
            authored_prompt_ref: None,
            parent_lineage_ids: vec![runtime_refusal_row.lineage_id.clone()],
            source_case_ids: vec![runtime_refusal_row.source_trace.source_case_id.clone()],
            receipt_refs: runtime_refusal_row.receipt_refs.clone(),
            detail: String::from(
                "Runtime-refusal sequencing case is bound to one held-out lineage row so the stop boundary remains receipt-backed.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: true,
            required_receipt_refs: runtime_refusal_row.receipt_refs.clone(),
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Runtime-refusal sequencing case requires the held-out sequence receipts because the stop boundary is part of the scored truth.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::SequencingMultiCall(task),
        detail: String::from(detail),
    }
}

fn sequencing_prompt_format() -> PsionPluginBenchmarkPromptFormat {
    PsionPluginBenchmarkPromptFormat {
        format_id: String::from("plugin_sequence_plan_v1"),
        system_instruction_ref: String::from(
            "prompt://psion/plugin_benchmark/system/plugin-sequence-plan",
        ),
        user_template_ref: String::from(
            "prompt://psion/plugin_benchmark/user/plugin-sequence-plan",
        ),
        envelope: PsionPluginBenchmarkPromptEnvelope::StructuredPluginSequenceJson,
        expected_response_format:
            PsionPluginBenchmarkExpectedResponseFormat::PluginSequencePlanJson,
        preserve_receipt_boundaries: true,
        detail: String::from(
            "Sequencing prompts force one explicit multi-call plan, one sequence-mode declaration, and one continuation posture.",
        ),
    }
}

fn sequencing_grader(
    grader_id: &str,
    expected_tool_names: Vec<String>,
    sequence_mode: PsionPluginSequenceMode,
    continuation_posture: PsionPluginContinuationPosture,
    detail: &str,
) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::SequencePlan(PsionPluginSequencePlanGrader {
        grader_id: String::from(grader_id),
        expected_tool_names,
        sequence_mode,
        continuation_posture,
        detail: String::from(detail),
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
    hasher.update(b"psion_plugin_sequence_prompt|");
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginSequencingBenchmarkError> {
    if value.trim().is_empty() {
        return Err(PsionPluginSequencingBenchmarkError::MissingField {
            field: String::from(field),
        });
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

fn stable_bundle_digest(bundle: &PsionPluginSequencingBenchmarkBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_sequencing_benchmark_bundle|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_psion_plugin_sequencing_benchmark_bundle,
        PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION,
    };
    use crate::{PsionPluginContinuationPosture, PsionPluginSequenceMode};

    #[test]
    fn sequencing_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_sequencing_benchmark_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_SEQUENCING_BENCHMARK_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.package.items.len(), 5);
        assert_eq!(bundle.receipt.observed_metrics.len(), 3);
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::SequencingMultiCall(
                crate::PsionPluginSequencingTask {
                    sequence_mode: PsionPluginSequenceMode::Parallelizable,
                    ..
                }
            )
        )));
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::SequencingMultiCall(
                crate::PsionPluginSequencingTask {
                    continuation_posture: PsionPluginContinuationPosture::StopOnTypedRuntimeRefusal,
                    ..
                }
            )
        )));
        Ok(())
    }
}
