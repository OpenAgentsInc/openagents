use std::{
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
    PSION_PLUGIN_BENCHMARK_PACKAGE_SCHEMA_VERSION, PsionPluginBenchmarkContaminationAttachment,
    PsionPluginBenchmarkExpectedResponseFormat, PsionPluginBenchmarkFamily,
    PsionPluginBenchmarkGraderInterface, PsionPluginBenchmarkItem, PsionPluginBenchmarkMetricKind,
    PsionPluginBenchmarkPackageContract, PsionPluginBenchmarkPackageError,
    PsionPluginBenchmarkPackageReceipt, PsionPluginBenchmarkPromptEnvelope,
    PsionPluginBenchmarkPromptFormat, PsionPluginBenchmarkReceiptPosture,
    PsionPluginBenchmarkTaskContract, PsionPluginDiscoverySelectionTask,
    PsionPluginExactRouteGrader, PsionPluginObservedMetric, PsionPluginSelectionDecisionGrader,
    PsionPluginSelectionNegativeCaseKind, record_psion_plugin_benchmark_package_receipt,
};

/// Stable schema version for the discovery-selection benchmark bundle.
pub const PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_discovery_selection_benchmark_bundle.v1";
/// Stable committed bundle ref for the discovery-selection benchmark family.
pub const PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_REF: &str = "fixtures/psion/benchmarks/psion_plugin_discovery_selection_benchmark_v1/psion_plugin_discovery_selection_benchmark_bundle.json";

const RSS_PARSE_PLUGIN_ID: &str = "plugin.feed.rss_atom_parse";
const HTML_EXTRACT_PLUGIN_ID: &str = "plugin.html.extract_readable";
const HTTP_FETCH_PLUGIN_ID: &str = "plugin.http.fetch_text";
const TEXT_URL_EXTRACT_PLUGIN_ID: &str = "plugin.text.url_extract";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginDiscoverySelectionBenchmarkBundle {
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

impl PsionPluginDiscoverySelectionBenchmarkBundle {
    /// Writes the bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginDiscoverySelectionBenchmarkError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginDiscoverySelectionBenchmarkError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginDiscoverySelectionBenchmarkError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }

    /// Validates the bundle against the shared contamination bundle.
    pub fn validate_against_contamination(
        &self,
        contamination: &PsionPluginContaminationBundle,
    ) -> Result<(), PsionPluginDiscoverySelectionBenchmarkError> {
        if self.schema_version != PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION {
            return Err(
                PsionPluginDiscoverySelectionBenchmarkError::SchemaVersionMismatch {
                    expected: String::from(
                        PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
                    ),
                    actual: self.schema_version.clone(),
                },
            );
        }
        self.package.validate_against_contamination(contamination)?;
        self.receipt
            .validate_against_package(&self.package, contamination)?;
        if self.package.package_family != PsionPluginBenchmarkFamily::DiscoverySelection {
            return Err(PsionPluginDiscoverySelectionBenchmarkError::PackageFamilyMismatch);
        }
        ensure_nonempty(self.summary.as_str(), "discovery_selection_bundle.summary")?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(PsionPluginDiscoverySelectionBenchmarkError::DigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginDiscoverySelectionBenchmarkError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("the discovery bundle must carry the discovery-selection package family")]
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
pub fn psion_plugin_discovery_selection_benchmark_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_REF)
}

pub fn build_psion_plugin_discovery_selection_benchmark_bundle()
-> Result<PsionPluginDiscoverySelectionBenchmarkBundle, PsionPluginDiscoverySelectionBenchmarkError>
{
    let contamination = build_psion_plugin_contamination_bundle()?;
    build_psion_plugin_discovery_selection_benchmark_bundle_from_contamination(&contamination)
}

pub fn write_psion_plugin_discovery_selection_benchmark_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginDiscoverySelectionBenchmarkBundle, PsionPluginDiscoverySelectionBenchmarkError>
{
    let bundle = build_psion_plugin_discovery_selection_benchmark_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_discovery_selection_benchmark_bundle_from_contamination(
    contamination: &PsionPluginContaminationBundle,
) -> Result<PsionPluginDiscoverySelectionBenchmarkBundle, PsionPluginDiscoverySelectionBenchmarkError>
{
    let prompt_format = selection_prompt_format();
    let grader_interfaces = vec![
        selection_grader(
            "selection_direct_answer_v1",
            PsionPluginRouteLabel::AnswerInLanguage,
            Vec::new(),
            false,
            "Direct-answer route should stay in language without plugin delegation.",
        ),
        selection_grader(
            "selection_single_http_fetch_v1",
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            vec![String::from(HTTP_FETCH_PLUGIN_ID)],
            true,
            "Single-plugin selection should choose the bounded HTTP fetch plugin.",
        ),
        selection_grader(
            "selection_multi_plugin_feed_then_fetch_v1",
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            vec![
                String::from(RSS_PARSE_PLUGIN_ID),
                String::from(HTTP_FETCH_PLUGIN_ID),
            ],
            true,
            "Multi-plugin selection should choose the feed parser before the fetch plugin.",
        ),
        selection_grader(
            "selection_wrong_tool_rejection_v1",
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            vec![String::from(RSS_PARSE_PLUGIN_ID)],
            true,
            "Wrong-tool negatives are scored separately from unsupported-capability refusal.",
        ),
        PsionPluginUnsupportedToolRefusalGrader::grader_interface(),
    ];
    let items = discovery_selection_items(contamination);
    let benchmark_package = BenchmarkPackage::new(
        BenchmarkPackageKey::new(
            "benchmark://openagents/psion/plugin_discovery_selection",
            "v1",
        ),
        "Psion Plugin Discovery And Selection",
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
        package_id: String::from("psion.plugin.discovery_selection.v1"),
        package_family: PsionPluginBenchmarkFamily::DiscoverySelection,
        benchmark_package,
        prompt_formats: vec![prompt_format],
        grader_interfaces,
        items,
        summary: String::from(
            "Discovery package covers direct-answer, single-plugin, multi-plugin, wrong-tool, and unsupported-tool selection under the bounded host-native plugin set.",
        ),
        package_digest: String::new(),
    };
    package.package_digest = stable_package_digest(&package);
    let receipt = record_psion_plugin_benchmark_package_receipt(
        "receipt.psion.plugin.discovery_selection.reference.v1",
        &package,
        contamination,
        vec![
            metric(
                PsionPluginBenchmarkMetricKind::RouteAccuracyBps,
                10_000,
                "Reference route labels are fully aligned for the bounded package.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::SelectionAccuracyBps,
                10_000,
                "Reference plugin-selection labels are fully aligned for the bounded package.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::WrongToolRejectionAccuracyBps,
                10_000,
                "Wrong-tool negatives remain distinct from unsupported-capability refusal.",
            ),
            metric(
                PsionPluginBenchmarkMetricKind::UnsupportedToolRefusalAccuracyBps,
                10_000,
                "Unsupported-tool cases route to explicit refusal instead of wrong-tool delegation.",
            ),
        ],
        "Reference receipt for the first plugin discovery-and-selection benchmark package.",
    )?;
    let mut bundle = PsionPluginDiscoverySelectionBenchmarkBundle {
        schema_version: String::from(
            PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
        ),
        package,
        receipt,
        summary: String::from(
            "Discovery benchmark bundle freezes one authored package plus one shared receipt for direct-answer, plugin-delegate, wrong-tool, and unsupported-tool selection decisions.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_bundle_digest(&bundle);
    bundle.validate_against_contamination(contamination)?;
    Ok(bundle)
}

struct PsionPluginUnsupportedToolRefusalGrader;

impl PsionPluginUnsupportedToolRefusalGrader {
    fn grader_interface() -> PsionPluginBenchmarkGraderInterface {
        PsionPluginBenchmarkGraderInterface::ExactRoute(PsionPluginExactRouteGrader {
            grader_id: String::from("selection_unsupported_tool_refusal_v1"),
            expected_route: PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
            detail: String::from(
                "Unsupported-tool discovery cases must refuse unsupported capability instead of choosing a wrong admitted tool.",
            ),
        })
    }
}

fn discovery_selection_items(
    contamination: &PsionPluginContaminationBundle,
) -> Vec<PsionPluginBenchmarkItem> {
    let admitted_all = vec![
        String::from(RSS_PARSE_PLUGIN_ID),
        String::from(HTML_EXTRACT_PLUGIN_ID),
        String::from(HTTP_FETCH_PLUGIN_ID),
        String::from(TEXT_URL_EXTRACT_PLUGIN_ID),
    ];
    let admitted_without_fetch = vec![
        String::from(RSS_PARSE_PLUGIN_ID),
        String::from(HTML_EXTRACT_PLUGIN_ID),
        String::from(TEXT_URL_EXTRACT_PLUGIN_ID),
    ];
    vec![
        item(
            contamination,
            "plugin_discovery_direct_answer_v1",
            "selection_direct_answer_v1",
            "benchmark://openagents/psion/plugin_discovery/direct_answer_v1",
            PsionPluginDiscoverySelectionTask {
                admitted_plugin_ids: admitted_all.clone(),
                direct_answer_allowed: true,
                expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                expected_plugin_ids: Vec::new(),
                negative_case_kind: None,
            },
            "Direct-answer case should stay in language even when multiple plugins are admitted.",
        ),
        item(
            contamination,
            "plugin_discovery_single_fetch_v1",
            "selection_single_http_fetch_v1",
            "benchmark://openagents/psion/plugin_discovery/single_fetch_v1",
            PsionPluginDiscoverySelectionTask {
                admitted_plugin_ids: admitted_all.clone(),
                direct_answer_allowed: false,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                expected_plugin_ids: vec![String::from(HTTP_FETCH_PLUGIN_ID)],
                negative_case_kind: None,
            },
            "Single-plugin case should choose the bounded HTTP fetch plugin from the admitted set.",
        ),
        item(
            contamination,
            "plugin_discovery_multi_sequence_v1",
            "selection_multi_plugin_feed_then_fetch_v1",
            "benchmark://openagents/psion/plugin_discovery/multi_sequence_v1",
            PsionPluginDiscoverySelectionTask {
                admitted_plugin_ids: admitted_all.clone(),
                direct_answer_allowed: false,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                expected_plugin_ids: vec![
                    String::from(RSS_PARSE_PLUGIN_ID),
                    String::from(HTTP_FETCH_PLUGIN_ID),
                ],
                negative_case_kind: None,
            },
            "Multi-plugin case should select a bounded two-plugin sequence under one admitted set.",
        ),
        item(
            contamination,
            "plugin_discovery_wrong_tool_negative_v1",
            "selection_wrong_tool_rejection_v1",
            "benchmark://openagents/psion/plugin_discovery/wrong_tool_negative_v1",
            PsionPluginDiscoverySelectionTask {
                admitted_plugin_ids: admitted_all.clone(),
                direct_answer_allowed: false,
                expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                expected_plugin_ids: vec![String::from(RSS_PARSE_PLUGIN_ID)],
                negative_case_kind: Some(PsionPluginSelectionNegativeCaseKind::WrongToolChoice),
            },
            "Wrong-tool negative remains separate from unsupported-tool refusal.",
        ),
        item(
            contamination,
            "plugin_discovery_unsupported_tool_negative_v1",
            "selection_unsupported_tool_refusal_v1",
            "benchmark://openagents/psion/plugin_discovery/unsupported_tool_negative_v1",
            PsionPluginDiscoverySelectionTask {
                admitted_plugin_ids: admitted_without_fetch,
                direct_answer_allowed: false,
                expected_route: PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability,
                expected_plugin_ids: Vec::new(),
                negative_case_kind: Some(
                    PsionPluginSelectionNegativeCaseKind::UnsupportedToolChoice,
                ),
            },
            "Unsupported-tool negative should refuse unsupported capability instead of selecting a wrong admitted tool.",
        ),
    ]
}

fn item(
    contamination: &PsionPluginContaminationBundle,
    item_id: &str,
    grader_id: &str,
    authored_prompt_ref: &str,
    task: PsionPluginDiscoverySelectionTask,
    detail: &str,
) -> PsionPluginBenchmarkItem {
    let prompt_digest = digest_text(item_id);
    PsionPluginBenchmarkItem {
        item_id: String::from(item_id),
        family: PsionPluginBenchmarkFamily::DiscoverySelection,
        prompt_format_id: String::from("plugin_selection_decision_v1"),
        grader_id: String::from(grader_id),
        prompt_digest,
        contamination_attachment: PsionPluginBenchmarkContaminationAttachment {
            contamination_bundle_ref: String::from(PSION_PLUGIN_CONTAMINATION_BUNDLE_REF),
            contamination_bundle_digest: contamination.bundle_digest.clone(),
            authored_prompt_ref: Some(String::from(authored_prompt_ref)),
            parent_lineage_ids: Vec::new(),
            source_case_ids: Vec::new(),
            receipt_refs: Vec::new(),
            detail: String::from(
                "Discovery benchmark item is benchmark-authored and explicitly marked as such instead of pretending it came from held-out runtime lineage.",
            ),
        },
        receipt_posture: PsionPluginBenchmarkReceiptPosture {
            execution_evidence_required: false,
            required_receipt_refs: Vec::new(),
            forbid_unseen_execution_claims: true,
            detail: String::from(
                "Discovery benchmark scores route and selection choice without claiming execution happened.",
            ),
        },
        task: PsionPluginBenchmarkTaskContract::DiscoverySelection(task),
        detail: String::from(detail),
    }
}

fn selection_prompt_format() -> PsionPluginBenchmarkPromptFormat {
    PsionPluginBenchmarkPromptFormat {
        format_id: String::from("plugin_selection_decision_v1"),
        system_instruction_ref: String::from(
            "prompt://psion/plugin_benchmark/system/plugin-selection",
        ),
        user_template_ref: String::from("prompt://psion/plugin_benchmark/user/plugin-selection"),
        envelope: PsionPluginBenchmarkPromptEnvelope::StructuredPluginSelectionJson,
        expected_response_format:
            PsionPluginBenchmarkExpectedResponseFormat::PluginSelectionDecisionJson,
        preserve_receipt_boundaries: true,
        detail: String::from(
            "Selection prompts force the model to emit one explicit route decision and one explicit plugin-selection payload.",
        ),
    }
}

fn selection_grader(
    grader_id: &str,
    expected_route: PsionPluginRouteLabel,
    expected_plugin_ids: Vec<String>,
    order_matters: bool,
    detail: &str,
) -> PsionPluginBenchmarkGraderInterface {
    PsionPluginBenchmarkGraderInterface::SelectionDecision(PsionPluginSelectionDecisionGrader {
        grader_id: String::from(grader_id),
        expected_route,
        expected_plugin_ids,
        order_matters,
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
    hasher.update(b"psion_plugin_discovery_prompt|");
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionPluginDiscoverySelectionBenchmarkError> {
    if value.trim().is_empty() {
        return Err(PsionPluginDiscoverySelectionBenchmarkError::MissingField {
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

fn stable_bundle_digest(bundle: &PsionPluginDiscoverySelectionBenchmarkBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    let encoded = serde_json::to_vec(&canonical).expect("bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_psion_plugin_discovery_selection_benchmark_bundle|");
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION,
        build_psion_plugin_discovery_selection_benchmark_bundle,
    };
    use psionic_data::PsionPluginRouteLabel;

    #[test]
    fn discovery_selection_bundle_builds() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_discovery_selection_benchmark_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_DISCOVERY_SELECTION_BENCHMARK_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.package.items.len(), 5);
        assert_eq!(bundle.receipt.observed_metrics.len(), 4);
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::DiscoverySelection(
                crate::PsionPluginDiscoverySelectionTask {
                    expected_route: PsionPluginRouteLabel::AnswerInLanguage,
                    ..
                }
            )
        )));
        assert!(bundle.package.items.iter().any(|item| matches!(
            item.task,
            crate::PsionPluginBenchmarkTaskContract::DiscoverySelection(
                crate::PsionPluginDiscoverySelectionTask {
                    expected_route: PsionPluginRouteLabel::DelegateToAdmittedPlugin,
                    ..
                }
            )
        )));
        Ok(())
    }
}
