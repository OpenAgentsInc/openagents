use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for canonical plugin-conditioned training records.
pub const PSION_PLUGIN_TRAINING_RECORD_SCHEMA_VERSION: &str =
    "psionic.psion.plugin_training_record.v1";

/// Plugin class carried through the plugin-conditioned training substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginClass {
    /// Host-native capability-free local deterministic plugin.
    HostNativeCapabilityFreeLocalDeterministic,
    /// Host-native networked read-only plugin.
    HostNativeNetworkedReadOnly,
    /// Host-native secret-backed or stateful plugin.
    HostNativeSecretBackedOrStateful,
    /// Digest-bound guest-artifact plugin.
    GuestArtifactDigestBound,
}

/// Controller surface that produced or anchored one training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginControllerSurface {
    /// Deterministic workflow controller above the starter-plugin bridge.
    DeterministicWorkflow,
    /// Router-owned `/v1/responses` plugin loop.
    RouterResponses,
    /// Local Apple FM plugin session lane.
    AppleFmSession,
    /// Weighted plugin controller lane.
    WeightedController,
    /// Synthetic or validator-only record used for schema checks.
    SyntheticValidation,
}

/// Route label carried by one plugin-conditioned training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginRouteLabel {
    /// The learned lane should answer in language.
    AnswerInLanguage,
    /// The learned lane should delegate to an admitted plugin.
    DelegateToAdmittedPlugin,
    /// The learned lane should ask for missing structured inputs before plugin use.
    RequestMissingStructureForPluginUse,
    /// The learned lane should refuse because the required plugin or capability is unsupported.
    RefuseUnsupportedPluginOrCapability,
}

/// Outcome label carried by one plugin-conditioned training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginOutcomeLabel {
    /// The record completed successfully.
    CompletedSuccess,
    /// Runtime returned a typed refusal.
    TypedRuntimeRefusal,
    /// The correct next move is to request missing structure.
    RequestMissingStructure,
    /// The correct next move is to refuse unsupported capability.
    RefusedUnsupportedCapability,
    /// Runtime execution failed outside the typed refusal surface.
    RuntimeFailure,
}

/// Invocation result state attached to one plugin invocation row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPluginInvocationStatus {
    /// Invocation completed successfully.
    Success,
    /// Invocation completed with a typed runtime refusal.
    TypedRefusal,
    /// Invocation failed outside the typed refusal surface.
    RuntimeFailure,
}

/// One admitted plugin row carried on a canonical training record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginAdmittedPluginRecord {
    /// Stable plugin identifier.
    pub plugin_id: String,
    /// Stable tool name projected through the shared bridge.
    pub tool_name: String,
    /// Plugin class carried by the current convergence posture.
    pub plugin_class: PsionPluginClass,
    /// Runtime capability class.
    pub capability_class: String,
    /// Runtime origin class.
    pub origin_class: String,
    /// Stable input schema identifier.
    pub input_schema_id: String,
    /// Stable result schema identifier.
    pub result_schema_id: String,
    /// Stable typed refusal schema identifiers.
    pub refusal_schema_ids: Vec<String>,
    /// Stable replay class identifier.
    pub replay_class_id: String,
}

/// One controller provenance block attached to a training record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPluginControllerContext {
    /// Controller surface that produced or anchored the record.
    pub controller_surface: PsionPluginControllerSurface,
    /// Stable source bundle ref.
    pub source_bundle_ref: String,
    /// Stable source bundle identifier.
    pub source_bundle_id: String,
    /// Stable source bundle digest.
    pub source_bundle_digest: String,
    /// Stable source case identifier.
    pub source_case_id: String,
    /// Optional workflow case identifier shared across lanes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_case_id: Option<String>,
    /// Short detail describing the controller provenance.
    pub detail: String,
}

/// One receipt-backed plugin invocation row attached to a training record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginInvocationRecord {
    /// Stable invocation identifier.
    pub invocation_id: String,
    /// Stable controller decision reference.
    pub decision_ref: String,
    /// Invoked plugin identifier.
    pub plugin_id: String,
    /// Invoked tool name.
    pub tool_name: String,
    /// Arguments payload preserved for the invocation.
    pub arguments: Value,
    /// Stable runtime receipt reference.
    pub receipt_ref: String,
    /// Stable runtime receipt digest.
    pub receipt_digest: String,
    /// Receipt-backed invocation status.
    pub status: PsionPluginInvocationStatus,
    /// Result payload returned on successful invocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_payload: Option<Value>,
    /// Refusal schema identifier returned on typed refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_schema_id: Option<String>,
    /// Short detail for the invocation row.
    pub detail: String,
}

/// Canonical plugin-conditioned training record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionPluginTrainingRecord {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable record identifier.
    pub record_id: String,
    /// Task prompt or directive text.
    pub directive_text: String,
    /// Explicit admitted plugin set for the task.
    pub admitted_plugins: Vec<PsionPluginAdmittedPluginRecord>,
    /// Controller provenance and source bundle identity.
    pub controller_context: PsionPluginControllerContext,
    /// Receipt-backed plugin invocation rows.
    pub plugin_invocations: Vec<PsionPluginInvocationRecord>,
    /// Route label for the task.
    pub route_label: PsionPluginRouteLabel,
    /// Outcome label for the task.
    pub outcome_label: PsionPluginOutcomeLabel,
    /// Optional final response text preserved for success cases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_response_text: Option<String>,
    /// Short detail for the record.
    pub detail: String,
    /// Stable digest over the record.
    pub record_digest: String,
}

impl PsionPluginTrainingRecord {
    /// Creates and validates one canonical plugin-conditioned training record.
    pub fn new(
        record_id: impl Into<String>,
        directive_text: impl Into<String>,
        admitted_plugins: Vec<PsionPluginAdmittedPluginRecord>,
        controller_context: PsionPluginControllerContext,
        plugin_invocations: Vec<PsionPluginInvocationRecord>,
        route_label: PsionPluginRouteLabel,
        outcome_label: PsionPluginOutcomeLabel,
        final_response_text: Option<String>,
        detail: impl Into<String>,
    ) -> Result<Self, PsionPluginTrainingRecordError> {
        let mut record = Self {
            schema_version: String::from(PSION_PLUGIN_TRAINING_RECORD_SCHEMA_VERSION),
            record_id: record_id.into(),
            directive_text: directive_text.into(),
            admitted_plugins,
            controller_context,
            plugin_invocations,
            route_label,
            outcome_label,
            final_response_text,
            detail: detail.into(),
            record_digest: String::new(),
        };
        record.validate()?;
        record.record_digest = record.stable_digest();
        Ok(record)
    }

    /// Validates the canonical training record.
    pub fn validate(&self) -> Result<(), PsionPluginTrainingRecordError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "training_record.schema_version",
        )?;
        if self.schema_version != PSION_PLUGIN_TRAINING_RECORD_SCHEMA_VERSION {
            return Err(PsionPluginTrainingRecordError::SchemaVersionMismatch {
                expected: String::from(PSION_PLUGIN_TRAINING_RECORD_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.record_id.as_str(), "training_record.record_id")?;
        ensure_nonempty(
            self.directive_text.as_str(),
            "training_record.directive_text",
        )?;
        ensure_nonempty(self.detail.as_str(), "training_record.detail")?;
        validate_controller_context(&self.controller_context)?;

        if self.admitted_plugins.is_empty() {
            return Err(PsionPluginTrainingRecordError::MissingField {
                field: String::from("training_record.admitted_plugins"),
            });
        }
        let mut seen_plugin_ids = BTreeSet::new();
        let mut seen_tool_names = BTreeSet::new();
        for plugin in &self.admitted_plugins {
            validate_admitted_plugin(plugin)?;
            if !seen_plugin_ids.insert(plugin.plugin_id.as_str()) {
                return Err(
                    PsionPluginTrainingRecordError::DuplicateAdmittedPluginId {
                        plugin_id: plugin.plugin_id.clone(),
                    },
                );
            }
            if !seen_tool_names.insert(plugin.tool_name.as_str()) {
                return Err(
                    PsionPluginTrainingRecordError::DuplicateAdmittedToolName {
                        tool_name: plugin.tool_name.clone(),
                    },
                );
            }
        }

        if matches!(
            self.route_label,
            PsionPluginRouteLabel::DelegateToAdmittedPlugin
        ) && self.plugin_invocations.is_empty()
        {
            return Err(
                PsionPluginTrainingRecordError::MissingInvocationRowsForDelegation {
                    record_id: self.record_id.clone(),
                },
            );
        }

        let mut seen_invocations = BTreeSet::new();
        for invocation in &self.plugin_invocations {
            validate_invocation_row(invocation)?;
            if !seen_invocations.insert(invocation.invocation_id.as_str()) {
                return Err(PsionPluginTrainingRecordError::DuplicateInvocationId {
                    invocation_id: invocation.invocation_id.clone(),
                });
            }
            let admitted_plugin = self
                .admitted_plugins
                .iter()
                .find(|plugin| plugin.plugin_id == invocation.plugin_id)
                .ok_or_else(|| {
                    PsionPluginTrainingRecordError::InvocationTargetsUnknownPlugin {
                        invocation_id: invocation.invocation_id.clone(),
                        plugin_id: invocation.plugin_id.clone(),
                    }
                })?;
            if admitted_plugin.tool_name != invocation.tool_name {
                return Err(PsionPluginTrainingRecordError::FieldMismatch {
                    field: format!(
                        "training_record.plugin_invocations[{}].tool_name",
                        invocation.invocation_id
                    ),
                    expected: admitted_plugin.tool_name.clone(),
                    actual: invocation.tool_name.clone(),
                });
            }
            if let Some(refusal_schema_id) = &invocation.refusal_schema_id {
                if !admitted_plugin
                    .refusal_schema_ids
                    .iter()
                    .any(|schema_id| schema_id == refusal_schema_id)
                {
                    return Err(
                        PsionPluginTrainingRecordError::UnknownInvocationRefusalSchema {
                            invocation_id: invocation.invocation_id.clone(),
                            refusal_schema_id: refusal_schema_id.clone(),
                        },
                    );
                }
            }
        }

        match self.outcome_label {
            PsionPluginOutcomeLabel::CompletedSuccess => {
                let final_text = self.final_response_text.as_deref().ok_or_else(|| {
                    PsionPluginTrainingRecordError::MissingField {
                        field: String::from("training_record.final_response_text"),
                    }
                })?;
                ensure_nonempty(final_text, "training_record.final_response_text")?;
            }
            _ => {
                if let Some(final_text) = self.final_response_text.as_deref() {
                    ensure_nonempty(final_text, "training_record.final_response_text")?;
                }
            }
        }

        Ok(())
    }

    /// Returns a stable digest over the record.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digest_record = self.clone();
        digest_record.record_digest.clear();
        stable_digest(b"psionic_psion_plugin_training_record|", &digest_record)
    }
}

/// Validation failure for one canonical plugin-conditioned training record.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionPluginTrainingRecordError {
    #[error("field `{field}` is missing")]
    MissingField { field: String },
    #[error("expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch { expected: String, actual: String },
    #[error("field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("duplicate admitted plugin id `{plugin_id}`")]
    DuplicateAdmittedPluginId { plugin_id: String },
    #[error("duplicate admitted tool name `{tool_name}`")]
    DuplicateAdmittedToolName { tool_name: String },
    #[error("delegation record `{record_id}` is missing plugin invocation rows")]
    MissingInvocationRowsForDelegation { record_id: String },
    #[error("duplicate invocation id `{invocation_id}`")]
    DuplicateInvocationId { invocation_id: String },
    #[error("invocation `{invocation_id}` targets unknown plugin `{plugin_id}`")]
    InvocationTargetsUnknownPlugin {
        invocation_id: String,
        plugin_id: String,
    },
    #[error(
        "invocation `{invocation_id}` references refusal schema `{refusal_schema_id}` outside the admitted plugin contract"
    )]
    UnknownInvocationRefusalSchema {
        invocation_id: String,
        refusal_schema_id: String,
    },
}

fn validate_admitted_plugin(
    plugin: &PsionPluginAdmittedPluginRecord,
) -> Result<(), PsionPluginTrainingRecordError> {
    ensure_nonempty(
        plugin.plugin_id.as_str(),
        "training_record.admitted_plugins[].plugin_id",
    )?;
    ensure_nonempty(
        plugin.tool_name.as_str(),
        "training_record.admitted_plugins[].tool_name",
    )?;
    ensure_nonempty(
        plugin.capability_class.as_str(),
        "training_record.admitted_plugins[].capability_class",
    )?;
    ensure_nonempty(
        plugin.origin_class.as_str(),
        "training_record.admitted_plugins[].origin_class",
    )?;
    ensure_nonempty(
        plugin.input_schema_id.as_str(),
        "training_record.admitted_plugins[].input_schema_id",
    )?;
    ensure_nonempty(
        plugin.result_schema_id.as_str(),
        "training_record.admitted_plugins[].result_schema_id",
    )?;
    ensure_nonempty(
        plugin.replay_class_id.as_str(),
        "training_record.admitted_plugins[].replay_class_id",
    )?;
    if plugin.refusal_schema_ids.is_empty() {
        return Err(PsionPluginTrainingRecordError::MissingField {
            field: String::from("training_record.admitted_plugins[].refusal_schema_ids"),
        });
    }
    let mut seen_refusal_schema_ids = BTreeSet::new();
    for refusal_schema_id in &plugin.refusal_schema_ids {
        ensure_nonempty(
            refusal_schema_id.as_str(),
            "training_record.admitted_plugins[].refusal_schema_ids[]",
        )?;
        if !seen_refusal_schema_ids.insert(refusal_schema_id.as_str()) {
            return Err(PsionPluginTrainingRecordError::FieldMismatch {
                field: String::from("training_record.admitted_plugins[].refusal_schema_ids"),
                expected: String::from("unique refusal schema ids"),
                actual: refusal_schema_id.clone(),
            });
        }
    }
    Ok(())
}

fn validate_controller_context(
    context: &PsionPluginControllerContext,
) -> Result<(), PsionPluginTrainingRecordError> {
    ensure_nonempty(
        context.source_bundle_ref.as_str(),
        "training_record.controller_context.source_bundle_ref",
    )?;
    ensure_nonempty(
        context.source_bundle_id.as_str(),
        "training_record.controller_context.source_bundle_id",
    )?;
    ensure_nonempty(
        context.source_bundle_digest.as_str(),
        "training_record.controller_context.source_bundle_digest",
    )?;
    ensure_nonempty(
        context.source_case_id.as_str(),
        "training_record.controller_context.source_case_id",
    )?;
    ensure_nonempty(
        context.detail.as_str(),
        "training_record.controller_context.detail",
    )?;
    if let Some(workflow_case_id) = &context.workflow_case_id {
        ensure_nonempty(
            workflow_case_id.as_str(),
            "training_record.controller_context.workflow_case_id",
        )?;
    }
    Ok(())
}

fn validate_invocation_row(
    invocation: &PsionPluginInvocationRecord,
) -> Result<(), PsionPluginTrainingRecordError> {
    ensure_nonempty(
        invocation.invocation_id.as_str(),
        "training_record.plugin_invocations[].invocation_id",
    )?;
    ensure_nonempty(
        invocation.decision_ref.as_str(),
        "training_record.plugin_invocations[].decision_ref",
    )?;
    ensure_nonempty(
        invocation.plugin_id.as_str(),
        "training_record.plugin_invocations[].plugin_id",
    )?;
    ensure_nonempty(
        invocation.tool_name.as_str(),
        "training_record.plugin_invocations[].tool_name",
    )?;
    ensure_nonempty(
        invocation.receipt_ref.as_str(),
        "training_record.plugin_invocations[].receipt_ref",
    )?;
    ensure_nonempty(
        invocation.receipt_digest.as_str(),
        "training_record.plugin_invocations[].receipt_digest",
    )?;
    ensure_nonempty(
        invocation.detail.as_str(),
        "training_record.plugin_invocations[].detail",
    )?;
    if let Some(refusal_schema_id) = &invocation.refusal_schema_id {
        ensure_nonempty(
            refusal_schema_id.as_str(),
            "training_record.plugin_invocations[].refusal_schema_id",
        )?;
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPluginTrainingRecordError> {
    if value.trim().is_empty() {
        return Err(PsionPluginTrainingRecordError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("training record should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn admitted_plugin(plugin_id: &str, tool_name: &str) -> PsionPluginAdmittedPluginRecord {
        PsionPluginAdmittedPluginRecord {
            plugin_id: String::from(plugin_id),
            tool_name: String::from(tool_name),
            plugin_class: PsionPluginClass::HostNativeCapabilityFreeLocalDeterministic,
            capability_class: String::from("local_deterministic_text"),
            origin_class: String::from("host_native"),
            input_schema_id: format!("{plugin_id}.input.v1"),
            result_schema_id: format!("{plugin_id}.result.v1"),
            refusal_schema_ids: vec![format!("{plugin_id}.refusal.v1")],
            replay_class_id: String::from("starter_plugin.runtime_receipt.v1"),
        }
    }

    fn controller_context() -> PsionPluginControllerContext {
        PsionPluginControllerContext {
            controller_surface: PsionPluginControllerSurface::DeterministicWorkflow,
            source_bundle_ref: String::from("fixtures/tassadar/example_bundle.json"),
            source_bundle_id: String::from("bundle.example.v1"),
            source_bundle_digest: String::from("bundle_digest"),
            source_case_id: String::from("case_a"),
            workflow_case_id: Some(String::from("workflow_a")),
            detail: String::from("deterministic workflow anchor"),
        }
    }

    fn invocation(plugin_id: &str, tool_name: &str) -> PsionPluginInvocationRecord {
        PsionPluginInvocationRecord {
            invocation_id: String::from("invoke_a"),
            decision_ref: String::from("decision_a"),
            plugin_id: String::from(plugin_id),
            tool_name: String::from(tool_name),
            arguments: serde_json::json!({"text": "hello"}),
            receipt_ref: String::from("receipt_a"),
            receipt_digest: String::from("receipt_digest"),
            status: PsionPluginInvocationStatus::Success,
            result_payload: Some(serde_json::json!({"words": 1})),
            refusal_schema_id: None,
            detail: String::from("successful invocation"),
        }
    }

    #[test]
    fn training_record_validates() {
        let record = PsionPluginTrainingRecord::new(
            "record_a",
            "Count the words in this text.",
            vec![admitted_plugin("plugin.text.stats", "plugin_text_stats")],
            controller_context(),
            vec![invocation("plugin.text.stats", "plugin_text_stats")],
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            PsionPluginOutcomeLabel::CompletedSuccess,
            Some(String::from("The text contains one word.")),
            "delegation over admitted plugin",
        )
        .expect("record should validate");
        assert!(!record.record_digest.is_empty());
    }

    #[test]
    fn delegation_requires_invocation_rows() {
        let error = PsionPluginTrainingRecord::new(
            "record_a",
            "Count the words in this text.",
            vec![admitted_plugin("plugin.text.stats", "plugin_text_stats")],
            controller_context(),
            Vec::new(),
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            PsionPluginOutcomeLabel::CompletedSuccess,
            Some(String::from("The text contains one word.")),
            "delegation over admitted plugin",
        )
        .expect_err("delegation should require invocation rows");
        assert_eq!(
            error,
            PsionPluginTrainingRecordError::MissingInvocationRowsForDelegation {
                record_id: String::from("record_a")
            }
        );
    }

    #[test]
    fn invocation_must_target_admitted_plugin() {
        let error = PsionPluginTrainingRecord::new(
            "record_a",
            "Count the words in this text.",
            vec![admitted_plugin("plugin.text.stats", "plugin_text_stats")],
            controller_context(),
            vec![invocation(
                "plugin.http.fetch_text",
                "plugin_http_fetch_text",
            )],
            PsionPluginRouteLabel::DelegateToAdmittedPlugin,
            PsionPluginOutcomeLabel::CompletedSuccess,
            Some(String::from("The text contains one word.")),
            "delegation over admitted plugin",
        )
        .expect_err("invocation should target admitted plugin");
        assert_eq!(
            error,
            PsionPluginTrainingRecordError::InvocationTargetsUnknownPlugin {
                invocation_id: String::from("invoke_a"),
                plugin_id: String::from("plugin.http.fetch_text")
            }
        );
    }
}
