use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for the first Psion source-admission contract.
pub const PSION_CORPUS_ADMISSION_SCHEMA_VERSION: &str = "psion.corpus_admission.v1";
/// Stable policy identifier for the first Psion source-admission policy.
pub const PSION_CORPUS_ADMISSION_POLICY_ID: &str = "psion.corpus_admission";
/// Stable policy version for the first Psion source-admission policy.
pub const PSION_CORPUS_ADMISSION_POLICY_VERSION: &str = "v1";

/// Source families explicitly supported by the first Psion corpus-admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionCorpusSourceKind {
    /// Long-form teaching text with sustained conceptual coverage.
    Textbook,
    /// Normative or quasi-normative standard or language specification.
    Specification,
    /// Vendor or platform manual.
    Manual,
    /// Research or technical paper.
    Paper,
    /// High-quality technical documentation.
    TechnicalDocumentation,
    /// Small or reference-quality source code.
    Code,
    /// Incident review, debugging write-up, or engineering postmortem.
    Postmortem,
    /// Curated expert explanation or design discussion.
    ExpertDiscussion,
    /// Reference implementation or executable reference artifact.
    ReferenceImplementation,
}

/// Usage posture and rights boundary for one reviewed source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSourceRightsPosture {
    /// May be used for internal pretraining or SFT, but not internal serving.
    InternalTrainingOnly,
    /// May be used for internal training and internal serving.
    InternalTrainingAndInternalServing,
    /// Derived artifacts may support later redistribution claims when the rest of the stack agrees.
    RedistributionDerivedArtifactsAllowed,
    /// May only be used for evaluation or benchmark construction.
    EvaluationOnly,
    /// May only be exposed during tokenizer construction.
    TokenizerOnly,
    /// Must not be used by the Psion lane.
    Rejected,
}

/// High-level role a source plays in the learned-model lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSourceNormativeRole {
    /// The source defines a contract or invariant directly.
    Normative,
    /// The source explains, interprets, or teaches.
    Explanatory,
    /// The source is mainly historical and used for alternative design-space exposure.
    Historical,
    /// The source is executable or reference-like.
    ExecutableReference,
    /// The source mixes multiple roles and must be handled with care.
    Mixed,
}

/// Boundary anchor style needed for later contamination review and source removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionSourceBoundaryKind {
    /// Stable chapter/section anchors exist.
    ChapterSectionAnchors,
    /// Stable page ranges exist.
    PageRangeAnchors,
    /// Stable file-path or symbol anchors exist.
    FilePathAnchors,
    /// Stable record identifiers exist.
    RecordAnchors,
    /// No durable boundaries exist, so later split isolation would be weak.
    NoStableBoundaries,
}

/// Minimum explicit workflow for deciding whether a source can enter the Psion lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionAdmissionWorkflowStep {
    /// Intake and metadata completeness review.
    IntakeMetadataReview,
    /// Provenance and stable-reference review.
    ProvenanceReview,
    /// Rights and allowed-use review.
    RightsReview,
    /// Corpus-quality and scope-fit review.
    QualityReview,
    /// Pre-ingest contamination and boundary review.
    ContaminationPrecheck,
    /// Final maintained decision recording.
    FinalDecision,
}

/// Required metadata fields surfaced by the first Psion admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionAdmissionMetadataField {
    /// Stable source identifier.
    SourceId,
    /// Stable source-family identifier.
    SourceFamilyId,
    /// High-level source family.
    SourceKind,
    /// Human-readable title.
    Title,
    /// Stable author, editor, or origin organization.
    AuthorOrOrigin,
    /// Publication year, revision, or equivalent stable edition marker.
    PublicationYearOrRevision,
    /// Primary language.
    Language,
    /// Stable canonical reference or acquisition pointer.
    CanonicalReference,
    /// Stable content digest.
    ContentDigest,
    /// Rights posture.
    RightsPosture,
    /// Normative role.
    NormativeRole,
    /// Boundary anchor kind.
    BoundaryKind,
    /// Final review decision.
    ReviewDecision,
    /// Responsible reviewer.
    Reviewer,
    /// Short decision rationale.
    DecisionSummary,
}

/// Reasons the policy requires a source to stay out of the admitted corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionAdmissionExclusionTrigger {
    /// Rights posture is missing or unresolved.
    MissingRightsPosture,
    /// Provenance cannot be checked or reproduced.
    UnverifiableProvenance,
    /// Stable digest or artifact identity is missing.
    MissingStableDigest,
    /// Boundaries are too weak for later split isolation or removal review.
    MissingStableBoundaries,
    /// The source would contaminate held-out or benchmark material.
    BenchmarkContaminationRisk,
    /// The source duplicates already-admitted content without canonical selection.
    NearDuplicateWithoutCanonicalSelection,
    /// The source is dominated by social chatter, SEO patterns, or low-signal repetition.
    LowSignalOrSocialNoise,
    /// The source is generated or synthetic without bounded parent lineage.
    GeneratedWithoutLineage,
    /// The source introduces sensitive or personal data outside the lane boundary.
    SensitiveOrPersonalData,
    /// The source family materially widens the lane away from systems reasoning.
    OutOfScopeForPsion,
    /// The source did not complete the required maintained review path.
    ReviewIncomplete,
}

/// Reasons a previously reviewed source must be reconsidered or removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionAdmissionRemovalTrigger {
    /// Rights posture changed or was revoked.
    RightsChangedOrRevoked,
    /// Provenance or acquisition history was invalidated.
    ProvenanceInvalidated,
    /// Stable digest or artifact identity no longer matches the admitted source.
    ContentDigestMismatch,
    /// Benchmark or held-out contamination was discovered after admission.
    BenchmarkContaminationDiscovered,
    /// The review decision or source classification was found to be incorrect.
    ReviewMisclassificationDiscovered,
    /// A source was superseded by a canonical replacement or retraction.
    SourceRetractedOrCanonicallyReplaced,
    /// Section or boundary metadata proved too weak for later isolation work.
    BoundaryMetadataInsufficient,
}

/// Final review decision for one source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionAdmissionReviewDecision {
    /// Fully admitted into the lane subject to the recorded rights posture.
    Admitted,
    /// Admitted only with scope restrictions such as tokenizer-only or eval-only use.
    AdmittedWithRestrictions,
    /// Rejected from the lane.
    Rejected,
}

/// Versioned Psion source-admission policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCorpusAdmissionPolicy {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable policy identifier.
    pub policy_id: String,
    /// Stable policy version.
    pub policy_version: String,
    /// Required metadata fields for admitted records.
    pub required_metadata_fields: Vec<PsionAdmissionMetadataField>,
    /// Supported source families.
    pub supported_source_kinds: Vec<PsionCorpusSourceKind>,
    /// Supported rights postures.
    pub supported_rights_postures: Vec<PsionSourceRightsPosture>,
    /// Required review workflow.
    pub review_workflow: Vec<PsionAdmissionWorkflowStep>,
    /// Explicit exclusion triggers.
    pub exclusion_triggers: Vec<PsionAdmissionExclusionTrigger>,
    /// Explicit removal triggers.
    pub removal_triggers: Vec<PsionAdmissionRemovalTrigger>,
}

impl PsionCorpusAdmissionPolicy {
    /// Returns the canonical first Psion source-admission policy.
    #[must_use]
    pub fn v1() -> Self {
        Self {
            schema_version: String::from(PSION_CORPUS_ADMISSION_SCHEMA_VERSION),
            policy_id: String::from(PSION_CORPUS_ADMISSION_POLICY_ID),
            policy_version: String::from(PSION_CORPUS_ADMISSION_POLICY_VERSION),
            required_metadata_fields: vec![
                PsionAdmissionMetadataField::SourceId,
                PsionAdmissionMetadataField::SourceFamilyId,
                PsionAdmissionMetadataField::SourceKind,
                PsionAdmissionMetadataField::Title,
                PsionAdmissionMetadataField::AuthorOrOrigin,
                PsionAdmissionMetadataField::PublicationYearOrRevision,
                PsionAdmissionMetadataField::Language,
                PsionAdmissionMetadataField::CanonicalReference,
                PsionAdmissionMetadataField::ContentDigest,
                PsionAdmissionMetadataField::RightsPosture,
                PsionAdmissionMetadataField::NormativeRole,
                PsionAdmissionMetadataField::BoundaryKind,
                PsionAdmissionMetadataField::ReviewDecision,
                PsionAdmissionMetadataField::Reviewer,
                PsionAdmissionMetadataField::DecisionSummary,
            ],
            supported_source_kinds: vec![
                PsionCorpusSourceKind::Textbook,
                PsionCorpusSourceKind::Specification,
                PsionCorpusSourceKind::Manual,
                PsionCorpusSourceKind::Paper,
                PsionCorpusSourceKind::TechnicalDocumentation,
                PsionCorpusSourceKind::Code,
                PsionCorpusSourceKind::Postmortem,
                PsionCorpusSourceKind::ExpertDiscussion,
                PsionCorpusSourceKind::ReferenceImplementation,
            ],
            supported_rights_postures: vec![
                PsionSourceRightsPosture::InternalTrainingOnly,
                PsionSourceRightsPosture::InternalTrainingAndInternalServing,
                PsionSourceRightsPosture::RedistributionDerivedArtifactsAllowed,
                PsionSourceRightsPosture::EvaluationOnly,
                PsionSourceRightsPosture::TokenizerOnly,
                PsionSourceRightsPosture::Rejected,
            ],
            review_workflow: vec![
                PsionAdmissionWorkflowStep::IntakeMetadataReview,
                PsionAdmissionWorkflowStep::ProvenanceReview,
                PsionAdmissionWorkflowStep::RightsReview,
                PsionAdmissionWorkflowStep::QualityReview,
                PsionAdmissionWorkflowStep::ContaminationPrecheck,
                PsionAdmissionWorkflowStep::FinalDecision,
            ],
            exclusion_triggers: vec![
                PsionAdmissionExclusionTrigger::MissingRightsPosture,
                PsionAdmissionExclusionTrigger::UnverifiableProvenance,
                PsionAdmissionExclusionTrigger::MissingStableDigest,
                PsionAdmissionExclusionTrigger::MissingStableBoundaries,
                PsionAdmissionExclusionTrigger::BenchmarkContaminationRisk,
                PsionAdmissionExclusionTrigger::NearDuplicateWithoutCanonicalSelection,
                PsionAdmissionExclusionTrigger::LowSignalOrSocialNoise,
                PsionAdmissionExclusionTrigger::GeneratedWithoutLineage,
                PsionAdmissionExclusionTrigger::SensitiveOrPersonalData,
                PsionAdmissionExclusionTrigger::OutOfScopeForPsion,
                PsionAdmissionExclusionTrigger::ReviewIncomplete,
            ],
            removal_triggers: vec![
                PsionAdmissionRemovalTrigger::RightsChangedOrRevoked,
                PsionAdmissionRemovalTrigger::ProvenanceInvalidated,
                PsionAdmissionRemovalTrigger::ContentDigestMismatch,
                PsionAdmissionRemovalTrigger::BenchmarkContaminationDiscovered,
                PsionAdmissionRemovalTrigger::ReviewMisclassificationDiscovered,
                PsionAdmissionRemovalTrigger::SourceRetractedOrCanonicallyReplaced,
                PsionAdmissionRemovalTrigger::BoundaryMetadataInsufficient,
            ],
        }
    }

    /// Validates the policy itself.
    pub fn validate(&self) -> Result<(), PsionCorpusAdmissionError> {
        if self.schema_version.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingPolicySchemaVersion);
        }
        if self.policy_id.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingPolicyId);
        }
        if self.policy_version.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingPolicyVersion);
        }
        if self.required_metadata_fields.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingRequiredMetadataFields);
        }
        if self.supported_source_kinds.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingSupportedSourceKinds);
        }
        if self.supported_rights_postures.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingSupportedRightsPostures);
        }
        if self.review_workflow.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingReviewWorkflow);
        }
        if self.exclusion_triggers.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingExclusionTriggers);
        }
        if self.removal_triggers.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingRemovalTriggers);
        }

        reject_duplicates(&self.required_metadata_fields, || {
            PsionCorpusAdmissionError::DuplicateRequiredMetadataField
        })?;
        reject_duplicates(&self.supported_source_kinds, || {
            PsionCorpusAdmissionError::DuplicateSupportedSourceKind
        })?;
        reject_duplicates(&self.supported_rights_postures, || {
            PsionCorpusAdmissionError::DuplicateSupportedRightsPosture
        })?;
        reject_duplicates(&self.review_workflow, || {
            PsionCorpusAdmissionError::DuplicateWorkflowStep
        })?;
        reject_duplicates(&self.exclusion_triggers, || {
            PsionCorpusAdmissionError::DuplicateExclusionTrigger
        })?;
        reject_duplicates(&self.removal_triggers, || {
            PsionCorpusAdmissionError::DuplicateRemovalTrigger
        })?;

        Ok(())
    }
}

/// One source reviewed under the Psion admission policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionSourceAdmissionRecord {
    /// Stable source identifier.
    pub source_id: String,
    /// Stable source-family identifier for mixture, ablation, and later removal work.
    pub source_family_id: String,
    /// High-level source family.
    pub source_kind: PsionCorpusSourceKind,
    /// Human-readable title.
    pub title: String,
    /// Author, editor, publisher, or organization.
    pub author_or_origin: String,
    /// Publication year, version, edition, or revision marker.
    pub publication_year_or_revision: String,
    /// Primary language.
    pub language: String,
    /// Stable acquisition pointer, repo path, catalog ref, or URL.
    pub canonical_reference: String,
    /// Stable digest over the reviewed source payload.
    pub content_digest: String,
    /// Rights posture.
    pub rights_posture: PsionSourceRightsPosture,
    /// Normative role.
    pub normative_role: PsionSourceNormativeRole,
    /// Boundary anchor kind.
    pub boundary_kind: PsionSourceBoundaryKind,
    /// Final review decision.
    pub review_decision: PsionAdmissionReviewDecision,
    /// Responsible reviewer.
    pub reviewer: String,
    /// Short decision summary.
    pub decision_summary: String,
    /// Optional note for later policy or source-family review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl PsionSourceAdmissionRecord {
    fn missing_field(
        &self,
        field: PsionAdmissionMetadataField,
    ) -> Option<PsionAdmissionMetadataField> {
        let missing = match field {
            PsionAdmissionMetadataField::SourceId => self.source_id.trim().is_empty(),
            PsionAdmissionMetadataField::SourceFamilyId => self.source_family_id.trim().is_empty(),
            PsionAdmissionMetadataField::SourceKind => false,
            PsionAdmissionMetadataField::Title => self.title.trim().is_empty(),
            PsionAdmissionMetadataField::AuthorOrOrigin => self.author_or_origin.trim().is_empty(),
            PsionAdmissionMetadataField::PublicationYearOrRevision => {
                self.publication_year_or_revision.trim().is_empty()
            }
            PsionAdmissionMetadataField::Language => self.language.trim().is_empty(),
            PsionAdmissionMetadataField::CanonicalReference => {
                self.canonical_reference.trim().is_empty()
            }
            PsionAdmissionMetadataField::ContentDigest => self.content_digest.trim().is_empty(),
            PsionAdmissionMetadataField::RightsPosture => false,
            PsionAdmissionMetadataField::NormativeRole => false,
            PsionAdmissionMetadataField::BoundaryKind => false,
            PsionAdmissionMetadataField::ReviewDecision => false,
            PsionAdmissionMetadataField::Reviewer => self.reviewer.trim().is_empty(),
            PsionAdmissionMetadataField::DecisionSummary => self.decision_summary.trim().is_empty(),
        };
        missing.then_some(field)
    }
}

/// Versioned reviewed-source manifest bound to one Psion admission policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCorpusAdmissionManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable policy identifier.
    pub policy_id: String,
    /// Stable policy version.
    pub policy_version: String,
    /// Reviewed source records.
    pub sources: Vec<PsionSourceAdmissionRecord>,
}

impl PsionCorpusAdmissionManifest {
    /// Validates the reviewed-source manifest against a concrete policy.
    pub fn validate_against_policy(
        &self,
        policy: &PsionCorpusAdmissionPolicy,
    ) -> Result<(), PsionCorpusAdmissionError> {
        policy.validate()?;
        if self.schema_version.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingManifestSchemaVersion);
        }
        if self.policy_id.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingManifestPolicyId);
        }
        if self.policy_version.trim().is_empty() {
            return Err(PsionCorpusAdmissionError::MissingManifestPolicyVersion);
        }
        if self.schema_version != policy.schema_version {
            return Err(PsionCorpusAdmissionError::SchemaVersionMismatch {
                expected: policy.schema_version.clone(),
                actual: self.schema_version.clone(),
            });
        }
        if self.policy_id != policy.policy_id {
            return Err(PsionCorpusAdmissionError::PolicyIdMismatch {
                expected: policy.policy_id.clone(),
                actual: self.policy_id.clone(),
            });
        }
        if self.policy_version != policy.policy_version {
            return Err(PsionCorpusAdmissionError::PolicyVersionMismatch {
                expected: policy.policy_version.clone(),
                actual: self.policy_version.clone(),
            });
        }
        if self.sources.is_empty() {
            return Err(PsionCorpusAdmissionError::MissingSourceRecords);
        }

        let supported_kinds = policy
            .supported_source_kinds
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let supported_postures = policy
            .supported_rights_postures
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut source_ids = BTreeSet::new();
        for source in &self.sources {
            if !source_ids.insert(source.source_id.clone()) {
                return Err(PsionCorpusAdmissionError::DuplicateSourceId {
                    source_id: source.source_id.clone(),
                });
            }
            if !supported_kinds.contains(&source.source_kind) {
                return Err(PsionCorpusAdmissionError::UnsupportedSourceKind {
                    source_id: source.source_id.clone(),
                    source_kind: source.source_kind,
                });
            }
            if !supported_postures.contains(&source.rights_posture) {
                return Err(PsionCorpusAdmissionError::UnsupportedRightsPosture {
                    source_id: source.source_id.clone(),
                    rights_posture: source.rights_posture,
                });
            }
            for field in &policy.required_metadata_fields {
                if let Some(missing_field) = source.missing_field(*field) {
                    return Err(PsionCorpusAdmissionError::MissingRequiredField {
                        source_id: source.source_id.clone(),
                        field: missing_field,
                    });
                }
            }
            match (source.review_decision, source.rights_posture) {
                (PsionAdmissionReviewDecision::Rejected, PsionSourceRightsPosture::Rejected) => {}
                (PsionAdmissionReviewDecision::Rejected, rights_posture) => {
                    return Err(
                        PsionCorpusAdmissionError::RejectedDecisionRequiresRejectedPosture {
                            source_id: source.source_id.clone(),
                            rights_posture,
                        },
                    );
                }
                (PsionAdmissionReviewDecision::Admitted, PsionSourceRightsPosture::Rejected) => {
                    return Err(
                        PsionCorpusAdmissionError::AdmittedSourceCannotUseRejectedPosture {
                            source_id: source.source_id.clone(),
                        },
                    );
                }
                (
                    PsionAdmissionReviewDecision::Admitted,
                    PsionSourceRightsPosture::EvaluationOnly
                    | PsionSourceRightsPosture::TokenizerOnly,
                ) => {
                    return Err(
                        PsionCorpusAdmissionError::RestrictedPostureRequiresRestrictedDecision {
                            source_id: source.source_id.clone(),
                            rights_posture: source.rights_posture,
                        },
                    );
                }
                (
                    PsionAdmissionReviewDecision::AdmittedWithRestrictions,
                    PsionSourceRightsPosture::Rejected,
                ) => {
                    return Err(
                        PsionCorpusAdmissionError::RestrictedDecisionCannotUseRejectedPosture {
                            source_id: source.source_id.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        Ok(())
    }
}

/// Validation failures for the Psion source-admission contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionCorpusAdmissionError {
    /// The policy omitted its schema version.
    #[error("Psion corpus-admission policy is missing `schema_version`")]
    MissingPolicySchemaVersion,
    /// The policy omitted its stable identifier.
    #[error("Psion corpus-admission policy is missing `policy_id`")]
    MissingPolicyId,
    /// The policy omitted its version tag.
    #[error("Psion corpus-admission policy is missing `policy_version`")]
    MissingPolicyVersion,
    /// The policy omitted required metadata fields.
    #[error(
        "Psion corpus-admission policy requires at least one `required_metadata_fields` entry"
    )]
    MissingRequiredMetadataFields,
    /// The policy omitted supported source kinds.
    #[error("Psion corpus-admission policy requires at least one `supported_source_kinds` entry")]
    MissingSupportedSourceKinds,
    /// The policy omitted supported rights postures.
    #[error(
        "Psion corpus-admission policy requires at least one `supported_rights_postures` entry"
    )]
    MissingSupportedRightsPostures,
    /// The policy omitted its workflow.
    #[error("Psion corpus-admission policy requires at least one `review_workflow` step")]
    MissingReviewWorkflow,
    /// The policy omitted its exclusion triggers.
    #[error("Psion corpus-admission policy requires explicit `exclusion_triggers`")]
    MissingExclusionTriggers,
    /// The policy omitted its removal triggers.
    #[error("Psion corpus-admission policy requires explicit `removal_triggers`")]
    MissingRemovalTriggers,
    /// One required metadata field appears twice.
    #[error("Psion corpus-admission policy repeats one `required_metadata_fields` entry")]
    DuplicateRequiredMetadataField,
    /// One supported source kind appears twice.
    #[error("Psion corpus-admission policy repeats one `supported_source_kinds` entry")]
    DuplicateSupportedSourceKind,
    /// One supported rights posture appears twice.
    #[error("Psion corpus-admission policy repeats one `supported_rights_postures` entry")]
    DuplicateSupportedRightsPosture,
    /// One workflow step appears twice.
    #[error("Psion corpus-admission policy repeats one `review_workflow` step")]
    DuplicateWorkflowStep,
    /// One exclusion trigger appears twice.
    #[error("Psion corpus-admission policy repeats one `exclusion_triggers` entry")]
    DuplicateExclusionTrigger,
    /// One removal trigger appears twice.
    #[error("Psion corpus-admission policy repeats one `removal_triggers` entry")]
    DuplicateRemovalTrigger,
    /// The manifest omitted its schema version.
    #[error("Psion source-admission manifest is missing `schema_version`")]
    MissingManifestSchemaVersion,
    /// The manifest omitted its policy id.
    #[error("Psion source-admission manifest is missing `policy_id`")]
    MissingManifestPolicyId,
    /// The manifest omitted its policy version.
    #[error("Psion source-admission manifest is missing `policy_version`")]
    MissingManifestPolicyVersion,
    /// Manifest and policy schema versions do not match.
    #[error(
        "Psion source-admission manifest schema version mismatch: expected `{expected}`, found `{actual}`"
    )]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version in the manifest.
        actual: String,
    },
    /// Manifest and policy ids do not match.
    #[error("Psion source-admission manifest policy id mismatch: expected `{expected}`, found `{actual}`")]
    PolicyIdMismatch {
        /// Expected policy id.
        expected: String,
        /// Actual policy id.
        actual: String,
    },
    /// Manifest and policy versions do not match.
    #[error(
        "Psion source-admission manifest policy version mismatch: expected `{expected}`, found `{actual}`"
    )]
    PolicyVersionMismatch {
        /// Expected policy version.
        expected: String,
        /// Actual policy version.
        actual: String,
    },
    /// The manifest omitted source records.
    #[error("Psion source-admission manifest requires at least one reviewed `sources` entry")]
    MissingSourceRecords,
    /// One source id was repeated.
    #[error("Psion source-admission manifest repeats source id `{source_id}`")]
    DuplicateSourceId {
        /// Duplicated source identifier.
        source_id: String,
    },
    /// One reviewed source omitted a required field.
    #[error("Psion source `{source_id}` is missing required field `{field:?}`")]
    MissingRequiredField {
        /// Source identifier.
        source_id: String,
        /// Missing required field.
        field: PsionAdmissionMetadataField,
    },
    /// A record uses a source kind the policy does not allow.
    #[error("Psion source `{source_id}` uses unsupported source kind `{source_kind:?}`")]
    UnsupportedSourceKind {
        /// Source identifier.
        source_id: String,
        /// Unsupported source kind.
        source_kind: PsionCorpusSourceKind,
    },
    /// A record uses a rights posture the policy does not allow.
    #[error("Psion source `{source_id}` uses unsupported rights posture `{rights_posture:?}`")]
    UnsupportedRightsPosture {
        /// Source identifier.
        source_id: String,
        /// Unsupported rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Rejected decisions must keep the rejected posture explicit.
    #[error(
        "Psion source `{source_id}` is marked rejected but keeps non-rejected posture `{rights_posture:?}`"
    )]
    RejectedDecisionRequiresRejectedPosture {
        /// Source identifier.
        source_id: String,
        /// Non-rejected rights posture.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Admitted sources may not use the rejected posture.
    #[error("Psion source `{source_id}` is admitted but uses the rejected rights posture")]
    AdmittedSourceCannotUseRejectedPosture {
        /// Source identifier.
        source_id: String,
    },
    /// Restricted-only postures must stay explicit in the review decision.
    #[error(
        "Psion source `{source_id}` uses restricted posture `{rights_posture:?}` but is not marked `admitted_with_restrictions`"
    )]
    RestrictedPostureRequiresRestrictedDecision {
        /// Source identifier.
        source_id: String,
        /// Restricted posture that requires an explicit restricted decision.
        rights_posture: PsionSourceRightsPosture,
    },
    /// Restricted decisions may not silently use the rejected posture.
    #[error(
        "Psion source `{source_id}` is `admitted_with_restrictions` but uses rejected posture"
    )]
    RestrictedDecisionCannotUseRejectedPosture {
        /// Source identifier.
        source_id: String,
    },
}

fn reject_duplicates<T, F>(entries: &[T], error: F) -> Result<(), PsionCorpusAdmissionError>
where
    T: Copy + Ord,
    F: FnOnce() -> PsionCorpusAdmissionError + Copy,
{
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(*entry) {
            return Err(error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_fixture() -> PsionCorpusAdmissionPolicy {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/corpus_admission/psion_corpus_admission_policy_v1.json"
        ))
        .expect("policy fixture should parse")
    }

    fn manifest_fixture() -> PsionCorpusAdmissionManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json"
        ))
        .expect("manifest fixture should parse")
    }

    #[test]
    fn v1_policy_matches_canonical_helper() {
        let policy = policy_fixture();
        policy.validate().expect("fixture policy should validate");
        assert_eq!(policy, PsionCorpusAdmissionPolicy::v1());
        assert!(policy
            .supported_rights_postures
            .contains(&PsionSourceRightsPosture::InternalTrainingAndInternalServing));
        assert!(policy
            .supported_rights_postures
            .contains(&PsionSourceRightsPosture::RedistributionDerivedArtifactsAllowed));
    }

    #[test]
    fn manifest_fixture_validates_against_policy() {
        let policy = policy_fixture();
        let manifest = manifest_fixture();
        manifest
            .validate_against_policy(&policy)
            .expect("fixture manifest should validate");
        assert!(manifest.sources.iter().any(|source| {
            source.rights_posture == PsionSourceRightsPosture::TokenizerOnly
                && source.review_decision == PsionAdmissionReviewDecision::AdmittedWithRestrictions
        }));
        assert!(manifest.sources.iter().any(|source| {
            source.rights_posture == PsionSourceRightsPosture::Rejected
                && source.review_decision == PsionAdmissionReviewDecision::Rejected
        }));
    }

    #[test]
    fn manifest_rejects_missing_required_title_for_admitted_source() {
        let policy = policy_fixture();
        let mut manifest = manifest_fixture();
        manifest.sources[0].title.clear();
        let error = manifest
            .validate_against_policy(&policy)
            .expect_err("missing title should fail validation");
        assert!(matches!(
            error,
            PsionCorpusAdmissionError::MissingRequiredField {
                field: PsionAdmissionMetadataField::Title,
                ..
            }
        ));
    }

    #[test]
    fn manifest_rejects_restricted_posture_without_restricted_decision() {
        let policy = policy_fixture();
        let mut manifest = manifest_fixture();
        let source = manifest
            .sources
            .iter_mut()
            .find(|source| source.rights_posture == PsionSourceRightsPosture::TokenizerOnly)
            .expect("fixture should contain tokenizer-only source");
        source.review_decision = PsionAdmissionReviewDecision::Admitted;
        let error = manifest
            .validate_against_policy(&policy)
            .expect_err("restricted posture should require restricted decision");
        assert!(matches!(
            error,
            PsionCorpusAdmissionError::RestrictedPostureRequiresRestrictedDecision { .. }
        ));
    }
}
