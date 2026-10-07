use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema for Qwen replacement-model conformance fixtures.
pub const QWEN_REPLACEMENT_CONFORMANCE_SCHEMA_VERSION: &str =
    "psionic.qwen_replacement_conformance.v1";

/// Tinker-recommended Qwen rows that Psionic must track for legal fine-tuning.
pub const REQUIRED_QWEN_REPLACEMENT_MODEL_IDS: &[&str] = &[
    "Qwen3.5-4B",
    "Qwen3.5-9B-Base",
    "Qwen3.5-35B-A3B-Base",
    "Qwen3.6-27B",
    "Qwen3.6-35B-A3B",
    "Qwen3.5-397B-A17B",
];

/// Artifact evidence level behind one conformance row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenReplacementArtifactEvidenceLevel {
    /// Real local artifact metadata was inspected.
    RealLocalArtifact,
    /// Hosted/Tinker metadata contract was inspected without local weights.
    HostedMetadataContract,
    /// Representative manifest only; sufficient for scheduling and refusal tests.
    RepresentativeManifest,
}

/// Weight/config packaging shape expected for one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenReplacementArtifactFormat {
    Gguf,
    SafeTensors,
    HostedTinker,
}

/// How Psionic admits the model family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenReplacementFamilyAdmissionDecision {
    /// Native qwen35 path with no version alias.
    DirectQwen35,
    /// Qwen3.6 is intentionally routed through qwen35-compatible runtime code.
    VersionedQwen36AliasToQwen35,
    /// Hosted-only large row whose local Psionic runtime is not scheduled yet.
    HostedOnly,
}

/// Role this model row can play in legal fine-tuning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenReplacementLegalRole {
    SmokeSftBase,
    RetainedScoreTarget,
    TeacherOrJudge,
    BaseModelResearch,
}

/// Vision handling claim for a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenReplacementVisionPosture {
    TextOnly,
    PromptProjectionOnly,
    NativeVisionExpected,
}

/// One Qwen replacement conformance fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenReplacementModelConformanceFixture {
    pub schema_version: String,
    pub public_model_id: String,
    pub artifact_format: QwenReplacementArtifactFormat,
    pub artifact_evidence_level: QwenReplacementArtifactEvidenceLevel,
    pub architecture_family: String,
    pub safetensors_config_family: Option<String>,
    pub tokenizer_pretokenizer: String,
    pub tokenizer_vocab_size: u32,
    pub chat_template_digest: String,
    pub context_window_tokens: u32,
    pub vision_posture: QwenReplacementVisionPosture,
    pub moe_expert_count: Option<u32>,
    pub moe_active_expert_count: Option<u32>,
    pub accepted_family_label: String,
    pub family_admission_decision: QwenReplacementFamilyAdmissionDecision,
    pub legal_role: QwenReplacementLegalRole,
    pub local_pylon_realistic: bool,
    pub tinker_hosted_large_row: bool,
    pub notes: String,
    pub fixture_digest: String,
}

impl QwenReplacementModelConformanceFixture {
    /// Returns the stable digest over this fixture.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.fixture_digest.clear();
        stable_digest(b"psionic_qwen_replacement_conformance_fixture|", &clone)
    }

    /// Validates the row and prevents silent Qwen3.6-as-Qwen3.5 admission.
    pub fn validate(&self) -> Result<(), QwenReplacementConformanceError> {
        if self.schema_version != QWEN_REPLACEMENT_CONFORMANCE_SCHEMA_VERSION {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("schema version drifted"),
            });
        }
        for (field, value) in [
            ("public_model_id", self.public_model_id.as_str()),
            ("architecture_family", self.architecture_family.as_str()),
            (
                "tokenizer_pretokenizer",
                self.tokenizer_pretokenizer.as_str(),
            ),
            ("chat_template_digest", self.chat_template_digest.as_str()),
            ("accepted_family_label", self.accepted_family_label.as_str()),
            ("notes", self.notes.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(QwenReplacementConformanceError::InvalidFixture {
                    model_id: self.public_model_id.clone(),
                    detail: format!("{field} must be present"),
                });
            }
        }
        if self.tokenizer_vocab_size == 0 || self.context_window_tokens == 0 {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("tokenizer vocabulary and context window must be non-zero"),
            });
        }
        if self.public_model_id.starts_with("Qwen3.6-")
            && self.family_admission_decision
                != QwenReplacementFamilyAdmissionDecision::VersionedQwen36AliasToQwen35
        {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from(
                    "Qwen3.6 rows must record an explicit versioned qwen36 alias decision",
                ),
            });
        }
        if self.public_model_id.starts_with("Qwen3.6-")
            && !self.accepted_family_label.contains("qwen36")
        {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("Qwen3.6 rows must not silently reuse the plain qwen35 label"),
            });
        }
        if matches!(
            self.family_admission_decision,
            QwenReplacementFamilyAdmissionDecision::DirectQwen35
        ) && self.accepted_family_label != "qwen35"
        {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("direct qwen35 rows must use accepted_family_label=qwen35"),
            });
        }
        if matches!(
            self.family_admission_decision,
            QwenReplacementFamilyAdmissionDecision::HostedOnly
        ) && !self.tinker_hosted_large_row
        {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("hosted-only admission must mark tinker_hosted_large_row"),
            });
        }
        if self.fixture_digest != self.stable_digest() {
            return Err(QwenReplacementConformanceError::InvalidFixture {
                model_id: self.public_model_id.clone(),
                detail: String::from("fixture digest drifted"),
            });
        }
        Ok(())
    }
}

/// Probe facts extracted from a candidate artifact or representative manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenReplacementArtifactProbe {
    pub public_model_id: String,
    pub artifact_format: QwenReplacementArtifactFormat,
    pub architecture_family: String,
    pub tokenizer_pretokenizer: String,
    pub chat_template_digest: String,
    pub context_window_tokens: u32,
    pub moe_expert_count: Option<u32>,
    pub moe_active_expert_count: Option<u32>,
}

/// Conformance report across the required replacement set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenReplacementConformanceReport {
    pub schema_version: String,
    pub report_id: String,
    pub required_model_ids: Vec<String>,
    pub fixtures: Vec<QwenReplacementModelConformanceFixture>,
    pub qwen36_alias_decision: String,
    pub serious_retained_target_model_id: String,
    pub smoke_model_id: String,
    pub report_digest: String,
}

impl QwenReplacementConformanceReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_replacement_conformance_report|", &clone)
    }

    /// Validates the report and fixture coverage.
    pub fn validate(&self) -> Result<(), QwenReplacementConformanceError> {
        if self.schema_version != QWEN_REPLACEMENT_CONFORMANCE_SCHEMA_VERSION {
            return Err(QwenReplacementConformanceError::InvalidReport {
                detail: String::from("schema version drifted"),
            });
        }
        let required = REQUIRED_QWEN_REPLACEMENT_MODEL_IDS
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let actual = self
            .fixtures
            .iter()
            .map(|fixture| fixture.public_model_id.as_str())
            .collect::<BTreeSet<_>>();
        if required != actual {
            return Err(QwenReplacementConformanceError::InvalidReport {
                detail: String::from("report does not cover exactly the required Qwen models"),
            });
        }
        for fixture in &self.fixtures {
            fixture.validate()?;
        }
        if self.serious_retained_target_model_id != "Qwen3.6-35B-A3B" {
            return Err(QwenReplacementConformanceError::InvalidReport {
                detail: String::from("serious retained target must stay Qwen3.6-35B-A3B"),
            });
        }
        if self.smoke_model_id != "Qwen3.5-4B" {
            return Err(QwenReplacementConformanceError::InvalidReport {
                detail: String::from("smoke model must stay Qwen3.5-4B"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(QwenReplacementConformanceError::InvalidReport {
                detail: String::from("report digest drifted"),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum QwenReplacementConformanceError {
    #[error("invalid Qwen replacement fixture for {model_id}: {detail}")]
    InvalidFixture { model_id: String, detail: String },
    #[error("invalid Qwen replacement report: {detail}")]
    InvalidReport { detail: String },
    #[error("unsupported Qwen replacement artifact for {model_id}: {detail}")]
    UnsupportedArtifact { model_id: String, detail: String },
}

/// Returns the canonical replacement conformance report.
pub fn canonical_qwen_replacement_conformance_report()
-> Result<QwenReplacementConformanceReport, QwenReplacementConformanceError> {
    let fixtures = canonical_qwen_replacement_conformance_fixtures();
    let mut report = QwenReplacementConformanceReport {
        schema_version: String::from(QWEN_REPLACEMENT_CONFORMANCE_SCHEMA_VERSION),
        report_id: String::from("qwen-replacement-legal-finetune-conformance-v1"),
        required_model_ids: REQUIRED_QWEN_REPLACEMENT_MODEL_IDS
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        fixtures,
        qwen36_alias_decision: String::from(
            "Qwen3.6 rows are admitted as qwen36_alias_qwen35 until real artifact metadata requires a separate runtime branch.",
        ),
        serious_retained_target_model_id: String::from("Qwen3.6-35B-A3B"),
        smoke_model_id: String::from("Qwen3.5-4B"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Returns canonical fixtures for all required replacement rows.
#[must_use]
pub fn canonical_qwen_replacement_conformance_fixtures()
-> Vec<QwenReplacementModelConformanceFixture> {
    vec![
        fixture(
            "Qwen3.5-4B",
            QwenReplacementArtifactFormat::SafeTensors,
            QwenReplacementArtifactEvidenceLevel::RepresentativeManifest,
            "qwen35",
            Some("Qwen3ForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen35-4b-chat-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::TextOnly,
            None,
            None,
            "qwen35",
            QwenReplacementFamilyAdmissionDecision::DirectQwen35,
            QwenReplacementLegalRole::SmokeSftBase,
            true,
            false,
            "First local smoke SFT base. Uses representative safetensors metadata until the local artifact is materialized.",
        ),
        fixture(
            "Qwen3.5-9B-Base",
            QwenReplacementArtifactFormat::SafeTensors,
            QwenReplacementArtifactEvidenceLevel::HostedMetadataContract,
            "qwen35",
            Some("Qwen3ForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen35-9b-base-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::TextOnly,
            None,
            None,
            "qwen35",
            QwenReplacementFamilyAdmissionDecision::DirectQwen35,
            QwenReplacementLegalRole::BaseModelResearch,
            true,
            false,
            "Small base-model research row for tokenizer/template and adapter-shape experiments.",
        ),
        fixture(
            "Qwen3.5-35B-A3B-Base",
            QwenReplacementArtifactFormat::SafeTensors,
            QwenReplacementArtifactEvidenceLevel::HostedMetadataContract,
            "qwen35_moe",
            Some("Qwen3MoeForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen35-35b-a3b-base-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::TextOnly,
            Some(128),
            Some(8),
            "qwen35",
            QwenReplacementFamilyAdmissionDecision::DirectQwen35,
            QwenReplacementLegalRole::BaseModelResearch,
            false,
            true,
            "Base MoE row for architecture research and hosted/Tinker fine-tune planning.",
        ),
        fixture(
            "Qwen3.6-27B",
            QwenReplacementArtifactFormat::SafeTensors,
            QwenReplacementArtifactEvidenceLevel::RepresentativeManifest,
            "qwen36",
            Some("Qwen3ForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen36-27b-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::TextOnly,
            None,
            None,
            "qwen36_alias_qwen35",
            QwenReplacementFamilyAdmissionDecision::VersionedQwen36AliasToQwen35,
            QwenReplacementLegalRole::RetainedScoreTarget,
            false,
            true,
            "Dense Qwen3.6 retained-score fallback target. Alias is explicit and must be revisited when real artifact metadata lands.",
        ),
        fixture(
            "Qwen3.6-35B-A3B",
            QwenReplacementArtifactFormat::HostedTinker,
            QwenReplacementArtifactEvidenceLevel::HostedMetadataContract,
            "qwen36_moe",
            Some("Qwen3MoeForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen36-35b-a3b-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::NativeVisionExpected,
            Some(128),
            Some(8),
            "qwen36_alias_qwen35",
            QwenReplacementFamilyAdmissionDecision::VersionedQwen36AliasToQwen35,
            QwenReplacementLegalRole::RetainedScoreTarget,
            false,
            true,
            "First serious legal retained-score target after the 4B smoke. Hosted metadata is enough to schedule, not enough to claim local serving support.",
        ),
        fixture(
            "Qwen3.5-397B-A17B",
            QwenReplacementArtifactFormat::HostedTinker,
            QwenReplacementArtifactEvidenceLevel::HostedMetadataContract,
            "qwen35_moe_large",
            Some("Qwen3MoeForCausalLM"),
            "qwen35",
            248_320,
            "sha256:qwen35-397b-a17b-template-pending-real-artifact",
            262_144,
            QwenReplacementVisionPosture::NativeVisionExpected,
            Some(256),
            Some(17),
            "hosted_qwen35_large",
            QwenReplacementFamilyAdmissionDecision::HostedOnly,
            QwenReplacementLegalRole::TeacherOrJudge,
            false,
            true,
            "Hosted teacher/judge row for distillation and non-thinking-mode comparisons; not a local Pylon target.",
        ),
    ]
}

/// Admits an artifact probe against the canonical replacement fixtures.
pub fn admit_qwen_replacement_artifact_probe(
    probe: &QwenReplacementArtifactProbe,
) -> Result<QwenReplacementModelConformanceFixture, QwenReplacementConformanceError> {
    let fixture = canonical_qwen_replacement_conformance_fixtures()
        .into_iter()
        .find(|fixture| fixture.public_model_id == probe.public_model_id)
        .ok_or_else(|| QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("model id is not in the required replacement set"),
        })?;
    fixture.validate()?;
    if fixture.artifact_format != probe.artifact_format {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("artifact format drifted from conformance fixture"),
        });
    }
    if fixture.architecture_family != probe.architecture_family {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("architecture family drifted from conformance fixture"),
        });
    }
    if fixture.tokenizer_pretokenizer != probe.tokenizer_pretokenizer {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("tokenizer pretokenizer drifted from conformance fixture"),
        });
    }
    if fixture.chat_template_digest != probe.chat_template_digest {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("chat template digest drifted from conformance fixture"),
        });
    }
    if fixture.context_window_tokens != probe.context_window_tokens {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("context window drifted from conformance fixture"),
        });
    }
    if fixture.moe_expert_count != probe.moe_expert_count
        || fixture.moe_active_expert_count != probe.moe_active_expert_count
    {
        return Err(QwenReplacementConformanceError::UnsupportedArtifact {
            model_id: probe.public_model_id.clone(),
            detail: String::from("MoE expert facts drifted from conformance fixture"),
        });
    }
    Ok(fixture)
}

fn fixture(
    public_model_id: &str,
    artifact_format: QwenReplacementArtifactFormat,
    artifact_evidence_level: QwenReplacementArtifactEvidenceLevel,
    architecture_family: &str,
    safetensors_config_family: Option<&str>,
    tokenizer_pretokenizer: &str,
    tokenizer_vocab_size: u32,
    chat_template_digest: &str,
    context_window_tokens: u32,
    vision_posture: QwenReplacementVisionPosture,
    moe_expert_count: Option<u32>,
    moe_active_expert_count: Option<u32>,
    accepted_family_label: &str,
    family_admission_decision: QwenReplacementFamilyAdmissionDecision,
    legal_role: QwenReplacementLegalRole,
    local_pylon_realistic: bool,
    tinker_hosted_large_row: bool,
    notes: &str,
) -> QwenReplacementModelConformanceFixture {
    let mut fixture = QwenReplacementModelConformanceFixture {
        schema_version: String::from(QWEN_REPLACEMENT_CONFORMANCE_SCHEMA_VERSION),
        public_model_id: String::from(public_model_id),
        artifact_format,
        artifact_evidence_level,
        architecture_family: String::from(architecture_family),
        safetensors_config_family: safetensors_config_family.map(String::from),
        tokenizer_pretokenizer: String::from(tokenizer_pretokenizer),
        tokenizer_vocab_size,
        chat_template_digest: String::from(chat_template_digest),
        context_window_tokens,
        vision_posture,
        moe_expert_count,
        moe_active_expert_count,
        accepted_family_label: String::from(accepted_family_label),
        family_admission_decision,
        legal_role,
        local_pylon_realistic,
        tinker_hosted_large_row,
        notes: String::from(notes),
        fixture_digest: String::new(),
    };
    fixture.fixture_digest = fixture.stable_digest();
    fixture
}

fn stable_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).expect("payload should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_report_covers_required_replacement_models() {
        let report = canonical_qwen_replacement_conformance_report().expect("report");
        assert_eq!(
            report.fixtures.len(),
            REQUIRED_QWEN_REPLACEMENT_MODEL_IDS.len()
        );
        assert_eq!(report.report_digest, report.stable_digest());
        let ids = report
            .fixtures
            .iter()
            .map(|fixture| fixture.public_model_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(ids.contains("Qwen3.6-35B-A3B"));
        assert!(ids.contains("Qwen3.5-4B"));
    }

    #[test]
    fn qwen36_rows_record_versioned_alias_decision() {
        let report = canonical_qwen_replacement_conformance_report().expect("report");
        let qwen36 = report
            .fixtures
            .iter()
            .filter(|fixture| fixture.public_model_id.starts_with("Qwen3.6-"))
            .collect::<Vec<_>>();
        assert_eq!(qwen36.len(), 2);
        assert!(qwen36.iter().all(|fixture| {
            fixture.family_admission_decision
                == QwenReplacementFamilyAdmissionDecision::VersionedQwen36AliasToQwen35
                && fixture.accepted_family_label == "qwen36_alias_qwen35"
        }));
    }

    #[test]
    fn qwen36_plain_qwen35_label_is_refused() {
        let mut fixture = canonical_qwen_replacement_conformance_fixtures()
            .into_iter()
            .find(|fixture| fixture.public_model_id == "Qwen3.6-35B-A3B")
            .expect("qwen36");
        fixture.accepted_family_label = String::from("qwen35");
        fixture.fixture_digest = fixture.stable_digest();
        let error = fixture
            .validate()
            .expect_err("silent qwen35 label must refuse");
        assert!(error.to_string().contains("plain qwen35"));
    }

    #[test]
    fn qwen36_representative_probe_is_admitted_with_alias_label() {
        let fixture = canonical_qwen_replacement_conformance_fixtures()
            .into_iter()
            .find(|fixture| fixture.public_model_id == "Qwen3.6-35B-A3B")
            .expect("qwen36");
        let probe = QwenReplacementArtifactProbe {
            public_model_id: fixture.public_model_id.clone(),
            artifact_format: fixture.artifact_format,
            architecture_family: fixture.architecture_family.clone(),
            tokenizer_pretokenizer: fixture.tokenizer_pretokenizer.clone(),
            chat_template_digest: fixture.chat_template_digest.clone(),
            context_window_tokens: fixture.context_window_tokens,
            moe_expert_count: fixture.moe_expert_count,
            moe_active_expert_count: fixture.moe_active_expert_count,
        };
        let admitted = admit_qwen_replacement_artifact_probe(&probe).expect("admitted");
        assert_eq!(admitted.accepted_family_label, "qwen36_alias_qwen35");
        assert_eq!(
            admitted.legal_role,
            QwenReplacementLegalRole::RetainedScoreTarget
        );
    }
}
