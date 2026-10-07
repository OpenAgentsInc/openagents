use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    PSION_PLUGIN_CONTAMINATION_BUNDLE_REF, PsionPluginContaminationBundle,
    PsionPluginContaminationError, PsionPluginRouteLabel, build_psion_plugin_contamination_bundle,
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
    PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION, PsionPluginArgumentConstructionTask,
    PsionPluginArgumentJsonValueType, PsionPluginArgumentSchemaGrader,
    PsionPluginBenchmarkContaminationAttachment, PsionPluginBenchmarkExpectedResponseFormat,
    PsionPluginBenchmarkFamily, PsionPluginBenchmarkGraderInterface, PsionPluginBenchmarkItem,
    PsionPluginBenchmarkMetricKind, PsionPluginBenchmarkPackageContract,
    PsionPluginBenchmarkPackageError, PsionPluginBenchmarkPackageReceipt,
    PsionPluginBenchmarkPromptEnvelope, PsionPluginBenchmarkPromptFormat,
    PsionPluginBenchmarkReceiptPosture, PsionPluginBenchmarkTaskContract,
    PsionPluginExactRefusalGrader, PsionPluginObservedMetric,
    record_psion_plugin_benchmark_package_receipt,
};

/// Stable schema version for the argument-construction benchmark bundle.
pub const PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_argument_construction_benchmark_bundle.v1";
/// Stable committed bundle ref for the argument-construction benchmark family.
pub const PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_REF: &str = "fixtures/psion/benchmarks/psion_plugin_argument_construction_benchmark_v1/psion_plugin_argument_construction_benchmark_bundle.json";

const FETCH_TEXT_TOOL_NAME: &str = "plugin_http_fetch_text";
const FETCH_TEXT_INPUT_SCHEMA_ID: &str = "plugin.http.fetch_text.input.v1";
const HTML_EXTRACT_TOOL_NAME: &str = "plugin_html_extract_readable";
const HTML_EXTRACT_INPUT_SCHEMA_ID: &str = "plugin.html.extract_readable.input.v1";
const CONTENT_TYPE_UNSUPPORTED_REASON: &str = "plugin.refusal.content_type_unsupported.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginArgumentConstructionBenchmarkBundle {
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

impl PsionPluginArgumentConstructionBenchmarkBundle {
    /// Writes the bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginArgumentConstructionBenchmarkError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginArgumentConstructionBenchmarkError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginArgumentConstructionBenchmarkError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the bundle against the shared contamination bundle.
    pub fn validate_against_contamination(
        &self,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginArgumentConstructionBenchmarkError> {
        if self.schema_version != PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION
        {
            return Err(
                PsionPluginArgumentConstructionBenchmarkError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        self.package.validate_against_contamination(contamination)?;
        self.receipt
            .validate_against_package(&self.package, contamination)?;
        if self.package.package_family != PsionPluginBenchmarkFamily::ArgumentConstruction {
            return Err(PsionPluginArgumentConstructionBenchmarkError::PackageFamilyMismatch);
        }
        ensure_nonempty(
            self.summary.as_str(),
            "argument_construction_bundle.summary",
        )?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginArgumentConstructionBenchmarkError::DigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginArgumentConstructionBenchmarkError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("the argument-construction bundle must carry the argument-construction package family")]
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
pub fn psion_plugin_argument_construction_benchmark_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_REF)
}

pub fn build_psion_plugin_argument_construction_benchmark_bundle() -> Result<
    PsionPluginArgumentConstructionBenchmarkBundle,
    PsionPluginArgumentConstructionBenchmarkError,
> {
    let contamination = build_psion_plugin_contamination_bundle()?;
    build_psion_plugin_argument_construction_benchmark_bundle_from_contamination(&contamination)
}

pub fn write_psion_plugin_argument_construction_benchmark_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    PsionPluginArgumentConstructionBenchmarkBundle,
    PsionPluginArgumentConstructionBenchmarkError,
> {
    let bundle = build_psion_plugin_argument_construction_benchmark_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_argument_construction_benchmark_bundle_from_contamination(
    contamination: &PsionPluginContaminationBundle,
) -> Result<
    PsionPluginArgumentConstructionBenchmarkBundle,
    PsionPluginArgumentConstructionBenchmarkError,
> {
    let runtime_refusal_row = held_out_fetch_refusal_row(contamination)?;
    if !runtime_refusal_row
        .receipt_refs
        .iter()
        .any(|receipt_ref| receipt_ref.contains("plugin.http.fetch_text"))
    {
        return Err(
            PsionPluginArgumentConstructionBenchmarkError::MissingField {
                field: String::from("held_out_fetch_refusal.receipt_refs.plugin_http_fetch_text"),
            },
        );
    }

    let prompt_format = argument_prompt_format();
    let grader_interfaces = vec![
        argument_grader(
            "argument_fetch_text_success_v1",
            FETCH_TEXT_TOOL_NAME,
            vec![String::from("url")],
            vec![(
                String::from("url"),
                PsionPluginArgumentJsonValueType::String,
            )],
            Vec::new(),
            false,
            "Fetch-text success cases must emit one string `url` field and no schema drift.",
        ),
        argument_grader(
            "argument_html_extract_success_v1",
            HTML_EXTRACT_TOOL_NAME,
            vec![
                String::from("source_url"),
                String::from("content_type"),
                String::from("body_text"),
            ],
            vec![
                (
                    String::from("body_text"),
                    PsionPluginArgumentJsonValueType::String,
                ),
                (
                    String::from("content_type"),
                    PsionPluginArgumentJsonValueType::String,
                ),
                (
                    String::from("source_url"),
                    PsionPluginArgumentJsonValueType::String,
                ),
            ],
            Vec::new(),
            false,
            "Readable-extraction success cases must preserve the three required string fields.",
        ),
        argument_grader(
            "argument_fetch_missing_url_request_structure_v1",
            FETCH_TEXT_TOOL_NAME,
            vec![String::from("url")],
            vec![(
                String::from("url"),
                PsionPluginArgumentJsonValueType::String,
            )],
            Vec::new(),
            true,
            "Missing-input cases must request structure instead of fabricating a fetch URL.",
        ),
        argument_grader(
            "argument_fetch_invalid_url_type_request_structure_v1",
            FETCH_TEXT_TOOL_NAME,
            vec![String::from("url")],
            vec![(
                String::from("url"),
                PsionPluginArgumentJsonValueType::String,
            )],
            Vec::new(),
            true,
            "Wrong-type URL cases must request corrected structure instead of emitting a malformed packet.",
        ),
        argument_grader(
            "argument_html_malformed_structure_request_structure_v1",
            HTML_EXTRACT_TOOL_NAME,
            vec![
                String::from("source_url"),
                String::from("content_type"),
                String::from("body_text"),
            ],
            vec![
                (
                    String::from("body_text"),
                    PsionPluginArgumentJsonValueType::String,
                ),
                (
                    String::from("content_type"),
                    PsionPluginArgumentJsonValueType::String,
                ),
                (
                    String::from("source_url"),
                    PsionPluginArgumentJsonValueType::String,
                ),
            ],
            Vec::new(),
            true,
            "Malformed multi-field structure cases must request a corrected flat packet before plugin use.",
        ),
        runtime_refusal_grader(),
    ];
    let items = argument_items(contamination, runtime_refusal_row);
    let benchmark_package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            "benchmark://openagents/psion/plugin_argument_construction",
            "v1",
        ),
        "Psion Plugin Argument Construction",
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
        package_id: String::from("psion.plugin.argument_construction.v1"),
        package_family: PsionPluginBenchmarkFamily::ArgumentConstruction,
        benchmark_package,
        prompt_formats: vec![prompt_format],
        grader_interfaces,
        items,
        summary: String::from(
            "Argument package covers schema-correct success, missing-input request-for-structure, malformed-structure correction, and one held-out runtime-refusal case under the bounded host-native plugin set.",
        ),
        package_digest: String::new(),
    };
    package.package_digest = stable_package_digest(&package);

    let receipt = record_psion_plugin_benchmark_package_receipt(
        "receipt.psion.plugin.argument_construction.reference.v1",
        &package,
        contamination,
        vec![
            metric(
                PsionPluginBenchmarkMetricKind::RouteAccuracyBps,
                10_000,
                "Reference route labels stay aligned across delegate and request-for-structure argument cases.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::ArgumentSchemaAccuracyBps,
                10_000,
                "Reference argument packets satisfy the required path and type expectations for the bounded starter-plugin schemas.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::RequestForStructureAccuracyBps,
                10_000,
                "Missing-input and malformed-structure cases remain distinct request-for-structure outcomes rather than silent packet fabrication.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::TypedRuntimeRefusalAccuracyBps,
                10_000,
                "Held-out runtime-refusal cases preserve the typed refusal boundary separately from model packet errors.",
            ),
        ],
        "Reference receipt for the first plugin argument-construction benchmark package.",
    )?;

    let mut bundle = PsionPluginArgumentConstructionBenchmarkBundle {
        schema_version: String::from(
            PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
        ),
        package,
        receipt,
        summary: String::from(
            "Argument-construction benchmark bundle freezes benchmark-authored schema-correct and request-for-structure cases plus one held-out execution-backed runtime-refusal case.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_contamination(contamination)?;
    if !bundle
        .package
        .items
        .iter()
        .any(|item| item.receipt_posture.execution_evidence_required)
    {
        return Err(
            PsionPluginArgumentConstructionBenchmarkError::MissingField {
                field: String::from(
                    "argument_construction_bundle.execution_backed_runtime_refusal_item",
                ),
            },
        );
    }
    if !bundle.package.items.iter().any(|item| {
        matches!(
            item.task,
            PsionPluginBenchmarkTaskContract::ArgumentConstruction(
                PsionPluginArgumentConstructionTask {
                    expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                    ..
                }
            )
        )
    }) {
        return Err(
            PsionPluginArgumentConstructionBenchmarkError::MissingField {
                field: String::from("argument_construction_bundle.request_missing_structure_item"),
            },
        );
    }
    Ok(bundle)
}

fn held_out_fetch_refusal_row<'a>(
    contamination: &'a PsionPluginContaminationBundle,
) -> Result<
    &'a psionic_data::PsionPluginParentLineageRow,
    PsionPluginArgumentConstructionBenchmarkError,
> {
    contamination
        .parent_lineage_rows
        .iter()
        .find(|row| row.source_trace.source_case_id == "web_content_intake_fetch_refusal")
        .ok_or_else(
            || PsionPluginArgumentConstructionBenchmarkError::MissingField {
                field: String::from(
                    "contamination_bundle.held_out.web_content_intake_fetch_refusal",
                ),
            },
        )
}

fn argument_items(
    contamination: &PsionPluginContaminationBundle,
    runtime_refusal_row: &psionic_data::PsionPluginParentLineageRow,
) -> Vec<PsionPluginBenchmarkItem> {
    vec![
        authored_item(
            contamination,
            "plugin_argument_fetch_text_success_v1",
            "argument_fetch_text_success_v1",
            "benchmark://openagents/psion/plugin_argument_construction/fetch_text_success_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(FETCH_TEXT_TOOL_NAME),
                input_schema_id: String::from(FETCH_TEXT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![String::from("url")],
                missing_argument_paths: Vec::new(),
                malformed_argument_paths: Vec::new(),
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Fetch-text success case should emit one schema-correct string URL packet.",
        ),
        authored_item(
            contamination,
            "plugin_argument_html_extract_success_v1",
            "argument_html_extract_success_v1",
            "benchmark://openagents/psion/plugin_argument_construction/html_extract_success_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(HTML_EXTRACT_TOOL_NAME),
                input_schema_id: String::from(HTML_EXTRACT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![
                    String::from("source_url"),
                    String::from("content_type"),
                    String::from("body_text"),
                ],
                missing_argument_paths: Vec::new(),
                malformed_argument_paths: Vec::new(),
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Readable-extraction success case should keep all three required string fields.",
        ),
        authored_item(
            contamination,
            "plugin_argument_missing_url_request_structure_v1",
            "argument_fetch_missing_url_request_structure_v1",
            "benchmark://openagents/psion/plugin_argument_construction/missing_url_request_structure_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(FETCH_TEXT_TOOL_NAME),
                input_schema_id: String::from(FETCH_TEXT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![String::from("url")],
                missing_argument_paths: vec![String::from("url")],
                malformed_argument_paths: Vec::new(),
                expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
            },
            "Missing required URL should trigger request-for-structure instead of fabricated arguments.",
        ),
        authored_item(
            contamination,
            "plugin_argument_invalid_url_type_request_structure_v1",
            "argument_fetch_invalid_url_type_request_structure_v1",
            "benchmark://openagents/psion/plugin_argument_construction/invalid_url_type_request_structure_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(FETCH_TEXT_TOOL_NAME),
                input_schema_id: String::from(FETCH_TEXT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![String::from("url")],
                missing_argument_paths: Vec::new(),
                malformed_argument_paths: vec![String::from("url")],
                expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
            },
            "Wrong-type URL input stays distinct from missing-input cases and must request corrected structure.",
        ),
        authored_item(
            contamination,
            "plugin_argument_html_malformed_structure_request_structure_v1",
            "argument_html_malformed_structure_request_structure_v1",
            "benchmark://openagents/psion/plugin_argument_construction/html_malformed_structure_request_structure_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(HTML_EXTRACT_TOOL_NAME),
                input_schema_id: String::from(HTML_EXTRACT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![
                    String::from("source_url"),
                    String::from("content_type"),
                    String::from("body_text"),
                ],
                missing_argument_paths: Vec::new(),
                malformed_argument_paths: vec![
                    String::from("content_type"),
                    String::from("body_text"),
                ],
                expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
            },
            "Malformed multi-field structure should stay distinct from single-field type errors.",
        ),
        runtime_refusal_item(
            contamination,
            runtime_refusal_row,
            "plugin_argument_typed_runtime_refusal_v1",
            PsionPluginArgumentConstructionTask {
                tool_name: String::from(FETCH_TEXT_TOOL_NAME),
                input_schema_id: String::from(FETCH_TEXT_INPUT_SCHEMA_ID),
                required_argument_paths: vec![String::from("url")],
                missing_argument_paths: Vec::new(),
                malformed_argument_paths: Vec::new(),
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            },
            "Held-out content-type refusal keeps the execution-backed typed runtime-refusal path separate from argument-schema mistakes.",
        ),
    ]
}

fn authored_item(
    contamination: &PsionPluginContaminationBundle,
    item_id: &str,
    grader_id: &str,
    authored_prompt_ref: &str,
    task: PsionPluginArgumentConstructionTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::ArgumentConstruction,
        prompt_format_id: String::from("plugin_argument_construction_v1"),
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
                "Argument benchmark item is benchmark-authored and explicitly marked as authored provenance rather than claimed held-out execution lineage.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: false,
            required_receipt_refs: Vec::new(),
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Benchmark-authored argument cases score packet construction and route choice without claiming runtime execution happened.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::ArgumentConstruction(task),
        detail: String::from(detail),
    }
}

fn runtime_refusal_item(
    contamination: &PsionPluginContaminationBundle,
    runtime_refusal_row: &psionic_data::PsionPluginParentLineageRow,
    item_id: &str,
    task: PsionPluginArgumentConstructionTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    let required_receipt_ref = runtime_refusal_row
        .receipt_refs
        .iter()
        .find(|receipt_ref| receipt_ref.contains("plugin.http.fetch_text"))
        .cloned()
        .expect("held-out fetch-refusal row should contain the fetch-text receipt ref");
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::ArgumentConstruction,
        prompt_format_id: String::from("plugin_argument_construction_v1"),
        grader_id: String::from("argument_fetch_typed_runtime_refusal_v1"),
        prompt_digest: digest_text(item_id),
        contamination_attachment: PsionPluginBenchmarkContaminationAttachment {
            contamination_bundle_ref: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_REF),
            contamination_bundle_digest: contamination.bundle_digest.clone(),
            authored_prompt_ref: None,
            parent_lineage_ids: vec![runtime_refusal_row.lineage_id.clone()],
            source_case_ids: vec![runtime_refusal_row.source_trace.source_case_id.clone()],
            receipt_refs: runtime_refusal_row.receipt_refs.clone(),
            detail: String::from(
                "Runtime-refusal argument case is bound to one held-out lineage row so the typed refusal stays receipt-backed.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: true,
            required_receipt_refs: vec![required_receipt_ref],
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Runtime-refusal argument case requires the held-out fetch receipt because the typed refusal is part of the scored truth.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::ArgumentConstruction(task),
        detail: String::from(detail),
    }
}

fn argument_prompt_format() -> PsionPluginBenchmarkPromptFormat {
    PsionPluginBenchmarkPromptFormat {
        format_id: String::from("plugin_argument_construction_v1"),
        system_instruction_ref: String::from(
            "prompt://psion/plugin_benchmark/system/plugin-argument-construction",
        ),
        user_template_ref: String::from(
            "prompt://psion/plugin_benchmark/user/plugin-argument-construction",
        ),
        envelope: PsionPluginBenchmarkPromptEnvelope::StructuredPluginArgumentsJson,
        expected_response_format: PsionPluginBenchmarkExpectedResponseFormat::PluginArgumentsJson,
        preserve_receipt_boundaries: true,
        detail: String::from(
            "Argument-construction prompts force an explicit route decision plus one typed plugin packet or a request-for-structure outcome.",
        ),
    }
}

fn argument_grader(
    grader_id: &str,
    tool_name: &str,
    required_argument_paths: Vec<String>,
    required_argument_types: Vec<(String, PsionPluginArgumentJsonValueType)>,
    forbidden_argument_paths: Vec<String>,
    request_for_structure_allowed: bool,
    detail: &str,
) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::ArgumentSchema(PsionPluginArgumentSchemaGrader {
        grader_id: String::from(grader_id),
        tool_name: String::from(tool_name),
        required_argument_paths,
        required_argument_types: required_argument_types
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
        forbidden_argument_paths,
        request_for_structure_allowed,
        detail: String::from(detail),
    })
}

fn runtime_refusal_grader() -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::ExactRefusal(PsionPluginExactRefusalGrader {
        grader_id: String::from("argument_fetch_typed_runtime_refusal_v1"),
        accepted_reason_codes: vec![String::from(CONTENT_TYPE_UNSUPPORTED_REASON)],
        request_for_structure_allowed: false,
        detail: String::from(
            "Typed runtime refusal cases preserve the content-type refusal boundary separately from model-side argument mistakes.",
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
    hasher.update(b"psion_plugin_argument_prompt|");
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginArgumentConstructionBenchmarkError> {
    if value.trim().is_empty() {
        return Err(
            PsionPluginArgumentConstructionBenchmarkError::MissingField {
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

fn stable_bundle_digest(bundle: &PsionPluginArgumentConstructionBenchmarkBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_argument_construction_benchmark_bundle|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
        build_psion_plugin_argument_construction_benchmark_bundle,
    };
    use psionic_data::PsionPluginRouteLabel;

    #[test]
    fn argument_construction_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_argument_construction_benchmark_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_ARGUMENT_CONSTRUCTION_BENCHMARK_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.package.items.len(), 6);
        assert_eq!(bundle.receipt.observed_metrics.len(), 4);
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::ArgumentConstruction(
                crate::PsionPluginArgumentConstructionTask {
                    expected_route: PsionPluginRouteLabel::RequestMissingStructureForPluginUse,
                    ..
                }
            )
        )));
        assert!(
            bundle
                .package
                .items
                .iter()
                .any(|item| item.receipt_posture.execution_evidence_required)
        );
        Ok(())
    }
}
