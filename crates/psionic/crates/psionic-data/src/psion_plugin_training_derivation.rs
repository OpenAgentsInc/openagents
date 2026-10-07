use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    StarterPluginAuthoringClass, StarterPluginCapabilityClass, StarterPluginInvocationStatus,
    StarterPluginOriginClass, starter_plugin_registration_by_tool_name,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PsionPluginAdmittedPluginRecord, PsionPluginControllerContext, PsionPluginControllerSurface,
    PsionPluginClass, PsionPluginInvocationRecord, PsionPluginInvocationStatus,
    PsionPluginOutcomeLabel, PsionPluginRouteLabel, PsionPluginTrainingRecord,
    TassadarMultiPluginTraceCorpusBundle, TassadarMultiPluginTraceCorpusError,
    TassadarMultiPluginTraceRecord, tassadar_multi_plugin_trace_corpus_bundle_path,
};

/// Stable schema version for the canonical plugin-trace derivation bundle.
pub const PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_training_derivation_bundle.v1";
/// Canonical committed output ref for the derivation bundle.
pub const PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_REF: &str =
    "fixtures/psion/plugins/datasets/psion_plugin_training_derivation_v1/psion_plugin_training_derivation_bundle.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginControllerSurfaceCountRow {
    /// Controller surface represented in the derived records.
    pub controller_surface: PsionPluginControllerSurface,
    /// Number of source trace records normalized for the surface.
    pub source_record_count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginTrainingDerivationBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable derivation bundle identifier.
    pub bundle_id: String,
    /// Stable source corpus ref.
    pub source_corpus_ref: String,
    /// Stable source corpus digest.
    pub source_corpus_digest: String,
    /// Per-surface source-record counts.
    pub controller_surface_counts: Vec<PsionPluginControllerSurfaceCountRow>,
    /// Derived canonical plugin-training records.
    pub records: Vec<PsionPluginTrainingRecord>,
    /// Plain-language claim boundary for the derivation bundle.
    pub claim_boundary: String,
    /// Short machine-readable summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionPluginTrainingDerivationBundle {
    /// Writes the derivation bundle to one JSON file.
    pub fn write_to_path(
        &self,
        output_path: impl AsRef<Path>,
    ) -> Result<(), PsionPluginTrainingDerivationError> {
        let output_path = output_path.as_ref();
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                PsionPluginTrainingDerivationError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(output_path, format!("{json}\n")).map_err(|error| {
            PsionPluginTrainingDerivationError::Write {
                path: output_path.display().to_string(),
                error,
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum PsionPluginTrainingDerivationError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("unknown controller lane `{lane_id}` in trace record `{record_id}`")]
    UnknownControllerLane { lane_id: String, record_id: String },
    #[error("no starter-plugin registration exists for tool `{tool_name}`")]
    UnknownToolRegistration { tool_name: String },
    #[error(
        "trace corpus drift for tool `{tool_name}` field `{field}` expected `{expected}` but found `{actual}`"
    )]
    TraceSchemaDrift {
        tool_name: String,
        field: String,
        expected: String,
        actual: String,
    },
    #[error(transparent)]
    TraceCorpus(#[from] TassadarMultiPluginTraceCorpusError),
    #[error(transparent)]
    TrainingRecord(#[from] crate::PsionPluginTrainingRecordError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn psion_plugin_training_derivation_bundle_path() -> PathBuf {
    repo_root().join(PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_REF)
}

pub fn build_psion_plugin_training_derivation_bundle(
) -> Result<PsionPluginTrainingDerivationBundle, PsionPluginTrainingDerivationError> {
    let corpus = load_committed_tassadar_multi_plugin_trace_corpus_bundle()?;
    build_psion_plugin_training_derivation_bundle_from_corpus(&corpus)
}

pub fn write_psion_plugin_training_derivation_bundle(
    output_path: impl AsRef<Path>,
) -> Result<PsionPluginTrainingDerivationBundle, PsionPluginTrainingDerivationError> {
    let bundle = build_psion_plugin_training_derivation_bundle()?;
    bundle.write_to_path(output_path)?;
    Ok(bundle)
}

pub fn build_psion_plugin_training_derivation_bundle_from_corpus(
    corpus: &TassadarMultiPluginTraceCorpusBundle,
) -> Result<PsionPluginTrainingDerivationBundle, PsionPluginTrainingDerivationError> {
    let mut records = corpus
        .trace_records
        .iter()
        .map(derive_training_record)
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|left, right| left.record_id.cmp(&right.record_id));

    let mut count_by_surface = BTreeMap::new();
    for record in &records {
        *count_by_surface
            .entry(record.controller_context.controller_surface)
            .or_insert(0_u32) += 1;
    }
    let controller_surface_counts = count_by_surface
        .into_iter()
        .map(|(controller_surface, source_record_count)| PsionPluginControllerSurfaceCountRow {
            controller_surface,
            source_record_count,
        })
        .collect::<Vec<_>>();

    let mut bundle = PsionPluginTrainingDerivationBundle {
        schema_version: String::from(PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_SCHEMA_VERSION),
        bundle_id: String::from("psion.plugin_training_derivation.bundle.v1"),
        source_corpus_ref: String::from(crate::TASSADAR_MULTI_PLUGIN_TRACE_CORPUS_BUNDLE_REF),
        source_corpus_digest: corpus.bundle_digest.clone(),
        controller_surface_counts,
        records,
        claim_boundary: String::from(
            "this derivation bundle normalizes deterministic workflow, router-owned plugin-loop, local Apple FM plugin-session, and the bounded deterministic guest-artifact trace from the committed Tassadar multi-plugin trace corpus into one canonical plugin-training record schema. It preserves plugin receipt identity, plugin class, guest-artifact digest-bound replay linkage, controller provenance, and route-or-outcome labels without inventing a second plugin API or implying trained-lane closure by itself.",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "plugin-training derivation bundle normalizes source_records={} into derived_records={} across controller_surfaces={}.",
        corpus.trace_records.len(),
        bundle.records.len(),
        bundle.controller_surface_counts.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_psion_plugin_training_derivation_bundle|",
        &bundle,
    );
    Ok(bundle)
}

fn derive_training_record(
    trace_record: &TassadarMultiPluginTraceRecord,
) -> Result<PsionPluginTrainingRecord, PsionPluginTrainingDerivationError> {
    let controller_surface = map_controller_surface(
        trace_record.lane_id.as_str(),
        trace_record.record_id.as_str(),
    )?;
    let admitted_plugins = trace_record
        .projected_tool_schema_rows
        .iter()
        .map(|row| {
            let registration = starter_plugin_registration_by_tool_name(row.tool_name.as_str())
                .ok_or_else(|| PsionPluginTrainingDerivationError::UnknownToolRegistration {
                    tool_name: row.tool_name.clone(),
                })?;
            check_match(
                row.tool_name.as_str(),
                "plugin_id",
                registration.plugin_id,
                row.plugin_id.as_str(),
            )?;
            check_match(
                row.tool_name.as_str(),
                "result_schema_id",
                registration.success_output_schema_id,
                row.result_schema_id.as_str(),
            )?;
            check_match(
                row.tool_name.as_str(),
                "replay_class_id",
                registration.replay_class_id,
                row.replay_class_id.as_str(),
            )?;
            let expected_refusal_schema_ids = registration
                .refusal_schema_ids
                .iter()
                .map(|schema_id| String::from(*schema_id))
                .collect::<Vec<_>>();
            if row.refusal_schema_ids != expected_refusal_schema_ids {
                return Err(PsionPluginTrainingDerivationError::TraceSchemaDrift {
                    tool_name: row.tool_name.clone(),
                    field: String::from("refusal_schema_ids"),
                    expected: format!("{expected_refusal_schema_ids:?}"),
                    actual: format!("{:?}", row.refusal_schema_ids),
                });
            }
            Ok(PsionPluginAdmittedPluginRecord {
                plugin_id: row.plugin_id.clone(),
                tool_name: row.tool_name.clone(),
                plugin_class: map_plugin_class(registration.authoring_class),
                capability_class: capability_class_label(registration.capability_class),
                origin_class: origin_class_label(registration.origin_class),
                input_schema_id: String::from(registration.input_schema_id),
                result_schema_id: row.result_schema_id.clone(),
                refusal_schema_ids: row.refusal_schema_ids.clone(),
                replay_class_id: row.replay_class_id.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let plugin_invocations = trace_record
        .step_rows
        .iter()
        .map(|step| {
            let (status, result_payload, refusal_schema_id) = match step.projected_result.status {
                StarterPluginInvocationStatus::Success => (
                    PsionPluginInvocationStatus::Success,
                    Some(step.projected_result.structured_payload.clone()),
                    None,
                ),
                StarterPluginInvocationStatus::Refusal => (
                    PsionPluginInvocationStatus::TypedRefusal,
                    None,
                    Some(step.projected_result.output_or_refusal_schema_id.clone()),
                ),
            };
            Ok(PsionPluginInvocationRecord {
                invocation_id: format!("{}.invoke.{}", trace_record.record_id, step.step_index),
                decision_ref: step.decision_ref.clone(),
                plugin_id: step.plugin_id.clone(),
                tool_name: step.tool_name.clone(),
                arguments: step.arguments.clone(),
                receipt_ref: step.projected_result.plugin_receipt.receipt_id.clone(),
                receipt_digest: step.projected_result.plugin_receipt.receipt_digest.clone(),
                status,
                result_payload,
                refusal_schema_id,
                detail: step.detail.clone(),
            })
        })
        .collect::<Result<Vec<_>, PsionPluginTrainingDerivationError>>()?;

    let route_label = if !plugin_invocations.is_empty() {
        PsionPluginRouteLabel::DelegateToAdmittedPlugin
    } else if trace_record.typed_refusal_preserved {
        PsionPluginRouteLabel::RefuseUnsupportedPluginOrCapability
    } else {
        PsionPluginRouteLabel::AnswerInLanguage
    };
    let outcome_label = if plugin_invocations
        .iter()
        .any(|invocation| invocation.status == PsionPluginInvocationStatus::TypedRefusal)
        || trace_record.typed_refusal_preserved
    {
        PsionPluginOutcomeLabel::TypedRuntimeRefusal
    } else {
        PsionPluginOutcomeLabel::CompletedSuccess
    };
    let final_response_text = if trace_record.final_message_text.trim().is_empty() {
        None
    } else {
        Some(trace_record.final_message_text.clone())
    };
    PsionPluginTrainingRecord::new(
        format!("psion_plugin_training.{}", trace_record.record_id),
        trace_record.directive_text.clone(),
        admitted_plugins,
        PsionPluginControllerContext {
            controller_surface,
            source_bundle_ref: trace_record.source_bundle_ref.clone(),
            source_bundle_id: trace_record.source_bundle_id.clone(),
            source_bundle_digest: trace_record.source_bundle_digest.clone(),
            source_case_id: trace_record.source_case_id.clone(),
            workflow_case_id: Some(trace_record.workflow_case_id.clone()),
            detail: trace_record.detail.clone(),
        },
        plugin_invocations,
        route_label,
        outcome_label,
        final_response_text,
        trace_record.detail.clone(),
    )
    .map_err(PsionPluginTrainingDerivationError::from)
}

fn map_controller_surface(
    lane_id: &str,
    record_id: &str,
) -> Result<PsionPluginControllerSurface, PsionPluginTrainingDerivationError> {
    match lane_id {
        "deterministic_workflow" => Ok(PsionPluginControllerSurface::DeterministicWorkflow),
        "router_responses" => Ok(PsionPluginControllerSurface::RouterResponses),
        "apple_fm_session" => Ok(PsionPluginControllerSurface::AppleFmSession),
        _ => Err(PsionPluginTrainingDerivationError::UnknownControllerLane {
            lane_id: String::from(lane_id),
            record_id: String::from(record_id),
        }),
    }
}

fn map_plugin_class(authoring_class: StarterPluginAuthoringClass) -> PsionPluginClass {
    match authoring_class {
        StarterPluginAuthoringClass::CapabilityFreeLocalDeterministic => {
            PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic
        }
        StarterPluginAuthoringClass::NetworkedReadOnly => {
            PsionPluginClass::HostNativeNetworkedReadOnly
        }
        StarterPluginAuthoringClass::GuestArtifactDigestBound => {
            PsionPluginClass::GuestArtifactDigestBound
        }
    }
}

fn capability_class_label(capability_class: StarterPluginCapabilityClass) -> String {
    match capability_class {
        StarterPluginCapabilityClass::LocalDeterministic => String::from("local_deterministic"),
        StarterPluginCapabilityClass::ReadOnlyNetwork => String::from("read_only_network"),
    }
}

fn origin_class_label(origin_class: StarterPluginOriginClass) -> String {
    match origin_class {
        StarterPluginOriginClass::OperatorBuiltin => String::from("operator_builtin"),
        StarterPluginOriginClass::UserAdded => String::from("user_added"),
    }
}

fn check_match(
    tool_name: &str,
    field: &str,
    expected: &str,
    actual: &str,
) -> Result<(), PsionPluginTrainingDerivationError> {
    if expected != actual {
        return Err(PsionPluginTrainingDerivationError::TraceSchemaDrift {
            tool_name: String::from(tool_name),
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-data crate dir")
}

fn load_committed_tassadar_multi_plugin_trace_corpus_bundle(
) -> Result<TassadarMultiPluginTraceCorpusBundle, PsionPluginTrainingDerivationError> {
    let path = tassadar_multi_plugin_trace_corpus_bundle_path();
    let bytes = fs::read(&path).map_err(|error| PsionPluginTrainingDerivationError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(PsionPluginTrainingDerivationError::from)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("derivation bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_SCHEMA_VERSION,
        build_psion_plugin_training_derivation_bundle,
        build_psion_plugin_training_derivation_bundle_from_corpus,
        load_committed_tassadar_multi_plugin_trace_corpus_bundle,
    };
    use crate::PsionPluginClass;

    #[test]
    fn derivation_bundle_builds_from_committed_corpus() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_training_derivation_bundle()?;
        assert_eq!(
            bundle.schema_version,
            PSION_PLUGIN_TRAINING_DERIVATION_BUNDLE_SCHEMA_VERSION
        );
        assert_eq!(bundle.records.len(), 7);
        assert_eq!(bundle.controller_surface_counts.len(), 3);
        assert!(
            bundle
                .records
                .iter()
                .all(|record| !record.record_digest.is_empty())
        );
        Ok(())
    }

    #[test]
    fn derivation_bundle_preserves_guest_artifact_class_and_receipt_linkage(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_psion_plugin_training_derivation_bundle()?;
        let guest_record = bundle
            .records
            .iter()
            .find(|record| {
                record
                    .admitted_plugins
                    .iter()
                    .any(|plugin| plugin.plugin_class == PsionPluginClass::GuestArtifactDigestBound)
            })
            .expect("guest artifact record");
        assert_eq!(guest_record.admitted_plugins.len(), 1);
        assert_eq!(guest_record.plugin_invocations.len(), 1);
        assert_eq!(
            guest_record.admitted_plugins[0].replay_class_id,
            "guest_artifact_digest_replay_only.v1"
        );
        assert_eq!(
            guest_record.plugin_invocations[0].receipt_ref,
            "receipt.plugin.example.echo_guest.bee52b6c5818aceb.v1"
        );
        assert_eq!(
            guest_record.plugin_invocations[0].receipt_digest,
            "384381469d85793219b83507292e3c0fc8ccf899d0823600666319f1f521c673"
        );
        Ok(())
    }

    #[test]
    fn derivation_rejects_schema_drift() -> Result<(), Box<dyn std::error::Error>> {
        let mut corpus = load_committed_tassadar_multi_plugin_trace_corpus_bundle()?;
        corpus.trace_records[0].projected_tool_schema_rows[0].result_schema_id =
            String::from("drifted.result.schema.v1");
        let error = build_psion_plugin_training_derivation_bundle_from_corpus(&corpus)
            .expect_err("schema drift should fail closed");
        assert!(error.to_string().contains("trace corpus drift for tool"));
        Ok(())
    }
}
