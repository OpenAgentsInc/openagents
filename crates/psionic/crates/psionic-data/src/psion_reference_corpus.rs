use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DatasetIterationMode, DatasetPackingMode, DatasetShardOrdering, DatasetSplitKind,
    OverlongSequencePosture, PsionAdmissionReviewDecision, PsionBenchmarkIsolationError,
    PsionContaminationViolationConsequence, PsionCorpusAdmissionError,
    PsionCorpusAdmissionManifest, PsionCorpusAdmissionPolicy, PsionCorpusSourceKind,
    PsionExclusionManifest, PsionLifecycleChangeTrigger, PsionNearDuplicateReviewPolicy,
    PsionRawSourceDocument, PsionRawSourceImportKind, PsionRawSourceIngestionError,
    PsionRawSourceManifest, PsionRawSourceNormalizationProfile, PsionRawSourceNormalizationStep,
    PsionRawSourceRecord, PsionRawSourceSectionBoundary, PsionSectionDisjointnessMode,
    PsionSectionRangeDisjointnessRule, PsionSourceAdmissionRecord, PsionSourceBoundaryKind,
    PsionSourceLifecycleError, PsionSourceLifecycleManifest, PsionSourceLifecycleRecord,
    PsionSourceLifecycleState, PsionSourceNormativeRole, PsionSourceRightsPosture,
    PsionTokenizedCorpusError, PsionTokenizedCorpusManifest, PsionTokenizedPackingPolicy,
    PsionTokenizedReplayContract, PsionTokenizedShardManifest, PsionTokenizedShardSourceLineage,
    PsionTokenizedSourceFamilyBinding, PsionTokenizedSplitManifest, PsionTokenizerArtifactBundle,
    PsionTokenizerExposureAuditRow, PsionTokenizerExposureRecord, PsionTokenizerSourceReference,
    PsionTokenizerTrainingAlgorithm, PsionTokenizerTrainingConfig, PsionTokenizerTrainingError,
    PsionTokenizerTrainingManifest, TokenizerFamily,
};

/// Stable repo-relative source root for the Psion reference corpus.
pub const PSION_REFERENCE_CORPUS_SOURCE_ROOT: &str = "fixtures/psion/reference_corpus/raw_sources";
/// Stable preprocessing version for the reference corpus.
pub const PSION_REFERENCE_PREPROCESSING_VERSION: &str = "psion_reference_preprocess_v1";
/// Stable tokenizer id for the reference corpus.
pub const PSION_REFERENCE_TOKENIZER_ID: &str = "psion_reference_sentencepiece";
/// Stable tokenizer version for the reference corpus.
pub const PSION_REFERENCE_TOKENIZER_VERSION: &str = "v1";
/// Stable dataset id for the reference tokenized corpus.
pub const PSION_REFERENCE_DATASET_ID: &str = "psion_reference_tokenized";
/// Stable dataset version for the reference tokenized corpus.
pub const PSION_REFERENCE_DATASET_VERSION: &str = "v1";
/// Stable dataset identity for replay and checkpoints.
pub const PSION_REFERENCE_DATASET_IDENTITY: &str = "psion_reference_tokenized@v1";
/// Stable packing policy id for the reference corpus.
pub const PSION_REFERENCE_PACKING_POLICY_ID: &str = "psion_reference_packing";
/// Stable packing policy version for the reference corpus.
pub const PSION_REFERENCE_PACKING_POLICY_VERSION: &str = "v1";
/// Stable maximum sequence length emitted by the reference corpus.
pub const PSION_REFERENCE_MAX_SEQUENCE_TOKENS: u32 = 32;

const SPECIAL_TOKENS: [&str; 4] = ["<pad>", "<bos>", "<eos>", "<unk>"];

#[derive(Clone, Copy)]
struct ReferenceDocumentBlueprint {
    document_id: &'static str,
    relative_path: &'static str,
    import_kind: PsionRawSourceImportKind,
    import_reference: &'static str,
}

#[derive(Clone, Copy)]
struct ReferenceSourceBlueprint {
    source_id: &'static str,
    source_family_id: &'static str,
    source_kind: PsionCorpusSourceKind,
    title: &'static str,
    author_or_origin: &'static str,
    publication_year_or_revision: &'static str,
    language: &'static str,
    canonical_reference: &'static str,
    rights_posture: PsionSourceRightsPosture,
    normative_role: PsionSourceNormativeRole,
    boundary_kind: PsionSourceBoundaryKind,
    review_decision: PsionAdmissionReviewDecision,
    reviewer: &'static str,
    decision_summary: &'static str,
    notes: Option<&'static str>,
    lifecycle_state: PsionSourceLifecycleState,
    last_transition_trigger: PsionLifecycleChangeTrigger,
    lifecycle_rationale: &'static str,
    tokenizer_exposed: bool,
    model_training_exposed: bool,
    benchmark_exposed: bool,
    documents: &'static [ReferenceDocumentBlueprint],
}

const ARCH_TEXTBOOK_DOCS: [ReferenceDocumentBlueprint; 2] = [
    ReferenceDocumentBlueprint {
        document_id: "arch_textbook_foster_1985:chapter_01",
        relative_path: "arch_textbook_foster_1985/chapter_01.txt",
        import_kind: PsionRawSourceImportKind::PdfExtractedText,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/arch_textbook_foster_1985/chapter_01.txt",
    },
    ReferenceDocumentBlueprint {
        document_id: "arch_textbook_foster_1985:chapter_02",
        relative_path: "arch_textbook_foster_1985/chapter_02.txt",
        import_kind: PsionRawSourceImportKind::PdfExtractedText,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/arch_textbook_foster_1985/chapter_02.txt",
    },
];

const DISTRIBUTED_SCHEDULER_DOCS: [ReferenceDocumentBlueprint; 2] = [
    ReferenceDocumentBlueprint {
        document_id: "distributed_scheduler_notes_v1:notes_01",
        relative_path: "distributed_scheduler_notes_v1/notes_01.txt",
        import_kind: PsionRawSourceImportKind::MarkdownSnapshot,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/distributed_scheduler_notes_v1/notes_01.txt",
    },
    ReferenceDocumentBlueprint {
        document_id: "distributed_scheduler_notes_v1:notes_02",
        relative_path: "distributed_scheduler_notes_v1/notes_02.txt",
        import_kind: PsionRawSourceImportKind::MarkdownSnapshot,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/distributed_scheduler_notes_v1/notes_02.txt",
    },
];

const WASM_SPEC_DOCS: [ReferenceDocumentBlueprint; 2] = [
    ReferenceDocumentBlueprint {
        document_id: "wasm_core_spec_release_2:chapter_01",
        relative_path: "wasm_core_spec_release_2/chapter_01.txt",
        import_kind: PsionRawSourceImportKind::HtmlSnapshot,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/wasm_core_spec_release_2/chapter_01.txt",
    },
    ReferenceDocumentBlueprint {
        document_id: "wasm_core_spec_release_2:chapter_02",
        relative_path: "wasm_core_spec_release_2/chapter_02.txt",
        import_kind: PsionRawSourceImportKind::HtmlSnapshot,
        import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/wasm_core_spec_release_2/chapter_02.txt",
    },
];

const VENDOR_MANUAL_DOCS: [ReferenceDocumentBlueprint; 1] = [ReferenceDocumentBlueprint {
    document_id: "vendor_manual_private_scan_v1:manual_01",
    relative_path: "vendor_manual_private_scan_v1/manual_01.txt",
    import_kind: PsionRawSourceImportKind::PdfExtractedText,
    import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/vendor_manual_private_scan_v1/manual_01.txt",
}];

const EVAL_PACK_DOCS: [ReferenceDocumentBlueprint; 1] = [ReferenceDocumentBlueprint {
    document_id: "spec_quiz_eval_pack_v1:records",
    relative_path: "spec_quiz_eval_pack_v1/records.txt",
    import_kind: PsionRawSourceImportKind::RecordStreamSnapshot,
    import_reference: "repo://fixtures/psion/reference_corpus/raw_sources/spec_quiz_eval_pack_v1/records.txt",
}];

const REFERENCE_SOURCES: [ReferenceSourceBlueprint; 5] = [
    ReferenceSourceBlueprint {
        source_id: "arch_textbook_foster_1985",
        source_family_id: "computer_architecture_history",
        source_kind: PsionCorpusSourceKind::Textbook,
        title: "Computer Architecture Through Bottlenecks",
        author_or_origin: "M. Foster",
        publication_year_or_revision: "1985",
        language: "en",
        canonical_reference: "repo://fixtures/psion/reference_corpus/raw_sources/arch_textbook_foster_1985",
        rights_posture: PsionSourceRightsPosture::InternalTrainingOnly,
        normative_role: PsionSourceNormativeRole::Historical,
        boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
        review_decision: PsionAdmissionReviewDecision::Admitted,
        reviewer: "psion-reference-review",
        decision_summary: "Historical architecture textbook retained for bottleneck and tradeoff language.",
        notes: None,
        lifecycle_state: PsionSourceLifecycleState::Admitted,
        last_transition_trigger: PsionLifecycleChangeTrigger::ReviewMisclassificationCorrected,
        lifecycle_rationale: "Admitted into the bounded reference corpus for architecture reasoning language.",
        tokenizer_exposed: true,
        model_training_exposed: true,
        benchmark_exposed: false,
        documents: &ARCH_TEXTBOOK_DOCS,
    },
    ReferenceSourceBlueprint {
        source_id: "distributed_scheduler_notes_v1",
        source_family_id: "technical_runtime_docs",
        source_kind: PsionCorpusSourceKind::TechnicalDocumentation,
        title: "Distributed Scheduler Notes",
        author_or_origin: "OpenAgents Runtime Team",
        publication_year_or_revision: "2026-03",
        language: "en",
        canonical_reference: "repo://fixtures/psion/reference_corpus/raw_sources/distributed_scheduler_notes_v1",
        rights_posture: PsionSourceRightsPosture::InternalTrainingOnly,
        normative_role: PsionSourceNormativeRole::Explanatory,
        boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
        review_decision: PsionAdmissionReviewDecision::Admitted,
        reviewer: "psion-reference-review",
        decision_summary: "Short technical runtime notes retained for scheduler and tradeoff vocabulary.",
        notes: None,
        lifecycle_state: PsionSourceLifecycleState::Admitted,
        last_transition_trigger: PsionLifecycleChangeTrigger::ReviewMisclassificationCorrected,
        lifecycle_rationale: "Admitted into the bounded reference corpus for technical-doc held-out tracking.",
        tokenizer_exposed: true,
        model_training_exposed: true,
        benchmark_exposed: false,
        documents: &DISTRIBUTED_SCHEDULER_DOCS,
    },
    ReferenceSourceBlueprint {
        source_id: "wasm_core_spec_release_2",
        source_family_id: "normative_specs",
        source_kind: PsionCorpusSourceKind::Specification,
        title: "WebAssembly Core Specification Release 2 Reference Slice",
        author_or_origin: "W3C WebAssembly Working Group",
        publication_year_or_revision: "release-2",
        language: "en",
        canonical_reference: "repo://fixtures/psion/reference_corpus/raw_sources/wasm_core_spec_release_2",
        rights_posture: PsionSourceRightsPosture::InternalTrainingAndInternalServing,
        normative_role: PsionSourceNormativeRole::Normative,
        boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
        review_decision: PsionAdmissionReviewDecision::Admitted,
        reviewer: "psion-reference-review",
        decision_summary: "Normative spec slice retained for definition and boundary language.",
        notes: None,
        lifecycle_state: PsionSourceLifecycleState::Admitted,
        last_transition_trigger: PsionLifecycleChangeTrigger::ReviewMisclassificationCorrected,
        lifecycle_rationale: "Admitted into the bounded reference corpus for normative-spec coverage.",
        tokenizer_exposed: true,
        model_training_exposed: true,
        benchmark_exposed: false,
        documents: &WASM_SPEC_DOCS,
    },
    ReferenceSourceBlueprint {
        source_id: "vendor_manual_private_scan_v1",
        source_family_id: "historical_vendor_manuals",
        source_kind: PsionCorpusSourceKind::Manual,
        title: "Vendor Service Manual Scan",
        author_or_origin: "Legacy Vendor",
        publication_year_or_revision: "scan-v1",
        language: "en",
        canonical_reference: "repo://fixtures/psion/reference_corpus/raw_sources/vendor_manual_private_scan_v1",
        rights_posture: PsionSourceRightsPosture::TokenizerOnly,
        normative_role: PsionSourceNormativeRole::Mixed,
        boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
        review_decision: PsionAdmissionReviewDecision::AdmittedWithRestrictions,
        reviewer: "psion-reference-review",
        decision_summary: "Restricted to tokenizer exposure only for niche vendor terms.",
        notes: Some("Tokenizer-only source."),
        lifecycle_state: PsionSourceLifecycleState::Restricted,
        last_transition_trigger: PsionLifecycleChangeTrigger::ReclassifiedToRestrictedUse,
        lifecycle_rationale: "Restricted to tokenizer exposure because the rights posture excludes model training.",
        tokenizer_exposed: true,
        model_training_exposed: false,
        benchmark_exposed: false,
        documents: &VENDOR_MANUAL_DOCS,
    },
    ReferenceSourceBlueprint {
        source_id: "spec_quiz_eval_pack_v1",
        source_family_id: "evaluation_only_benchmark_material",
        source_kind: PsionCorpusSourceKind::ExpertDiscussion,
        title: "Psion Evaluation Quiz Pack",
        author_or_origin: "OpenAgents Eval Team",
        publication_year_or_revision: "2026-03",
        language: "en",
        canonical_reference: "repo://fixtures/psion/reference_corpus/raw_sources/spec_quiz_eval_pack_v1/records.txt",
        rights_posture: PsionSourceRightsPosture::EvaluationOnly,
        normative_role: PsionSourceNormativeRole::Explanatory,
        boundary_kind: PsionSourceBoundaryKind::RecordAnchors,
        review_decision: PsionAdmissionReviewDecision::AdmittedWithRestrictions,
        reviewer: "psion-reference-review",
        decision_summary: "Held out as evaluation-only benchmark material.",
        notes: Some("Evaluation-only held-out source."),
        lifecycle_state: PsionSourceLifecycleState::EvaluationOnly,
        last_transition_trigger: PsionLifecycleChangeTrigger::ReclassifiedToEvaluationOnly,
        lifecycle_rationale: "Kept evaluation-only to preserve benchmark isolation.",
        tokenizer_exposed: false,
        model_training_exposed: false,
        benchmark_exposed: true,
        documents: &EVAL_PACK_DOCS,
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferenceEncodedSequence {
    pub sequence_id: String,
    pub source_id: String,
    pub source_family_id: String,
    pub split_name: String,
    pub split_kind: DatasetSplitKind,
    pub token_ids: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferenceShardArtifact {
    pub schema_version: String,
    pub shard_id: String,
    pub split_name: String,
    pub split_kind: DatasetSplitKind,
    pub shard_digest: String,
    pub sequences: Vec<PsionReferenceEncodedSequence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferenceVocabularyArtifact {
    pub schema_version: String,
    pub tokenizer_id: String,
    pub tokenizer_version: String,
    pub tokens: Vec<String>,
    pub vocabulary_digest: String,
}

#[derive(Clone, Debug)]
pub struct PsionReferenceCorpusBundle {
    pub admission_manifest: PsionCorpusAdmissionManifest,
    pub lifecycle_manifest: PsionSourceLifecycleManifest,
    pub exclusion_manifest: PsionExclusionManifest,
    pub raw_source_manifest: PsionRawSourceManifest,
    pub tokenizer_training_manifest: PsionTokenizerTrainingManifest,
    pub tokenizer_bundle: PsionTokenizerArtifactBundle,
    pub tokenized_corpus_manifest: PsionTokenizedCorpusManifest,
    pub vocabulary_artifact: PsionReferenceVocabularyArtifact,
    pub shard_artifacts: Vec<PsionReferenceShardArtifact>,
}

impl PsionReferenceCorpusBundle {
    pub fn validate(&self) -> Result<(), PsionReferenceCorpusError> {
        let policy = PsionCorpusAdmissionPolicy::v1();
        self.admission_manifest
            .validate_against_policy(&policy)
            .map_err(PsionReferenceCorpusError::Admission)?;
        self.lifecycle_manifest
            .validate_against_admission(&self.admission_manifest)
            .map_err(PsionReferenceCorpusError::Lifecycle)?;
        self.exclusion_manifest
            .validate_against_lifecycle(&self.lifecycle_manifest)
            .map_err(PsionReferenceCorpusError::Isolation)?;
        self.raw_source_manifest
            .validate_against_lifecycle(&self.admission_manifest, &self.lifecycle_manifest)
            .map_err(PsionReferenceCorpusError::RawSource)?;
        self.tokenizer_training_manifest
            .validate_against_inputs(
                &self.raw_source_manifest,
                &self.lifecycle_manifest,
                &self.exclusion_manifest,
            )
            .map_err(PsionReferenceCorpusError::Tokenizer)?;
        self.tokenizer_bundle
            .validate_against_manifest(
                &self.tokenizer_training_manifest,
                &self.raw_source_manifest,
                &self.lifecycle_manifest,
                &self.exclusion_manifest,
            )
            .map_err(PsionReferenceCorpusError::Tokenizer)?;
        self.tokenized_corpus_manifest
            .validate_against_inputs(
                &self.tokenizer_bundle,
                &self.raw_source_manifest,
                &self.lifecycle_manifest,
                &self.exclusion_manifest,
            )
            .map_err(PsionReferenceCorpusError::TokenizedCorpus)?;
        for shard in &self.shard_artifacts {
            if stable_digest(b"psion_reference_shard|", &shard.sequences) != shard.shard_digest {
                return Err(PsionReferenceCorpusError::DigestMismatch {
                    detail: format!("shard `{}` digest drifted", shard.shard_id),
                });
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn shard(&self, split_kind: DatasetSplitKind) -> Option<&PsionReferenceShardArtifact> {
        self.shard_artifacts
            .iter()
            .find(|artifact| artifact.split_kind == split_kind)
    }

    #[must_use]
    pub fn token_id(&self, token: &str) -> Option<u32> {
        self.vocabulary_artifact
            .tokens
            .iter()
            .position(|candidate| candidate == token)
            .and_then(|index| u32::try_from(index).ok())
    }

    pub fn write_to_dir(&self, output_dir: &Path) -> Result<(), PsionReferenceCorpusError> {
        fs::create_dir_all(output_dir).map_err(PsionReferenceCorpusError::Io)?;
        fs::create_dir_all(output_dir.join("tokenized")).map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_source_admission_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.admission_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_source_lifecycle_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.lifecycle_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_exclusion_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.exclusion_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_raw_source_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.raw_source_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_tokenizer_training_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.tokenizer_training_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_tokenizer_artifact_bundle_v1.json"),
            serde_json::to_vec_pretty(&self.tokenizer_bundle)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        fs::write(
            output_dir.join("psion_reference_vocabulary_v1.json"),
            serde_json::to_vec_pretty(&self.vocabulary_artifact)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        for shard in &self.shard_artifacts {
            let file_name = match shard.split_kind {
                DatasetSplitKind::Train => "train_shard_0001.json",
                DatasetSplitKind::Validation => "validation_shard_0001.json",
                DatasetSplitKind::HeldOut => "held_out_shard_0001.json",
                _ => continue,
            };
            fs::write(
                output_dir.join("tokenized").join(file_name),
                serde_json::to_vec_pretty(shard)
                    .map_err(PsionReferenceCorpusError::Serialization)?,
            )
            .map_err(PsionReferenceCorpusError::Io)?;
        }
        fs::write(
            output_dir.join("psion_tokenized_corpus_manifest_v1.json"),
            serde_json::to_vec_pretty(&self.tokenized_corpus_manifest)
                .map_err(PsionReferenceCorpusError::Serialization)?,
        )
        .map_err(PsionReferenceCorpusError::Io)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionReferenceCorpusError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Admission(#[from] PsionCorpusAdmissionError),
    #[error(transparent)]
    Lifecycle(#[from] PsionSourceLifecycleError),
    #[error(transparent)]
    Isolation(#[from] PsionBenchmarkIsolationError),
    #[error(transparent)]
    RawSource(#[from] PsionRawSourceIngestionError),
    #[error(transparent)]
    Tokenizer(#[from] PsionTokenizerTrainingError),
    #[error(transparent)]
    TokenizedCorpus(#[from] PsionTokenizedCorpusError),
    #[error("reference corpus parse error: {detail}")]
    Parse { detail: String },
    #[error("reference corpus digest mismatch: {detail}")]
    DigestMismatch { detail: String },
    #[error("reference corpus serialization failed: {0}")]
    Serialization(serde_json::Error),
}

#[derive(Clone)]
struct ParsedSection {
    section_id: String,
    title: String,
    text: String,
}

struct LoadedDocument {
    blueprint: ReferenceDocumentBlueprint,
    raw_text: String,
    normalized_text: String,
    sections: Vec<ParsedSection>,
}

struct LoadedSource {
    blueprint: ReferenceSourceBlueprint,
    documents: Vec<LoadedDocument>,
    source_raw_digest: String,
    source_normalized_digest: String,
}

pub fn build_psion_reference_corpus(
    repo_root: &Path,
) -> Result<PsionReferenceCorpusBundle, PsionReferenceCorpusError> {
    let loaded_sources = load_reference_sources(repo_root)?;
    let admission_manifest = build_admission_manifest(&loaded_sources)?;
    let lifecycle_manifest = build_lifecycle_manifest(&loaded_sources, &admission_manifest)?;
    let exclusion_manifest = build_exclusion_manifest(&loaded_sources, &lifecycle_manifest)?;
    let raw_source_manifest =
        build_raw_source_manifest(&loaded_sources, &admission_manifest, &lifecycle_manifest)?;
    let vocabulary_artifact = build_vocabulary_artifact(&loaded_sources);
    let tokenizer_training_manifest = build_tokenizer_training_manifest(
        &loaded_sources,
        &raw_source_manifest,
        &exclusion_manifest,
        vocabulary_artifact.tokens.len() as u32,
    );
    let tokenizer_bundle = PsionTokenizerArtifactBundle::build_from_manifest(
        &tokenizer_training_manifest,
        &raw_source_manifest,
        &lifecycle_manifest,
        &exclusion_manifest,
    )?;
    let shard_artifacts =
        build_shard_artifacts(&loaded_sources, &raw_source_manifest, &vocabulary_artifact)?;
    let tokenized_corpus_manifest = build_tokenized_corpus_manifest(
        &raw_source_manifest,
        &lifecycle_manifest,
        &exclusion_manifest,
        &tokenizer_bundle,
        &shard_artifacts,
    )?;
    let bundle = PsionReferenceCorpusBundle {
        admission_manifest,
        lifecycle_manifest,
        exclusion_manifest,
        raw_source_manifest,
        tokenizer_training_manifest,
        tokenizer_bundle,
        tokenized_corpus_manifest,
        vocabulary_artifact,
        shard_artifacts,
    };
    bundle.validate()?;
    Ok(bundle)
}

/// Builds one repo-owned reconstructed corpus bundle for the canonical actual
/// pretraining lane.
///
/// This does not claim byte-exact recovery of the canonical datastream-backed
/// shard payloads. It materializes a bounded local reconstruction from the
/// frozen actual-lane admission, lifecycle, tokenizer, tokenized-corpus, and
/// sampling manifests plus the repo-owned raw-source snapshots that remain
/// available in-tree for bring-up and rehearsal.
pub fn build_psion_actual_pretraining_reconstructed_corpus(
    repo_root: &Path,
) -> Result<PsionReferenceCorpusBundle, PsionReferenceCorpusError> {
    let loaded_sources = load_reference_sources(repo_root)?;
    let admission_manifest: PsionCorpusAdmissionManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/corpus_admission/psion_source_admission_manifest_v1.json",
    )?;
    let lifecycle_manifest: PsionSourceLifecycleManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json",
    )?;
    let exclusion_manifest: PsionExclusionManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/isolation/psion_exclusion_manifest_v1.json",
    )?;
    let raw_source_manifest: PsionRawSourceManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/ingestion/psion_raw_source_manifest_v1.json",
    )?;
    let tokenizer_training_manifest: PsionTokenizerTrainingManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/tokenizer/psion_tokenizer_training_manifest_v1.json",
    )?;
    let tokenizer_bundle: PsionTokenizerArtifactBundle = load_json_fixture(
        repo_root,
        "fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json",
    )?;
    let tokenized_corpus_manifest: PsionTokenizedCorpusManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json",
    )?;
    let vocabulary_artifact =
        build_vocabulary_artifact_for_tokenizer(&loaded_sources, &tokenizer_training_manifest);
    let shard_artifacts = build_actual_pretraining_shard_artifacts(
        &loaded_sources,
        &raw_source_manifest,
        &tokenizer_training_manifest,
        &tokenized_corpus_manifest,
        &exclusion_manifest,
        &vocabulary_artifact,
    )?;
    let bundle = PsionReferenceCorpusBundle {
        admission_manifest,
        lifecycle_manifest,
        exclusion_manifest,
        raw_source_manifest,
        tokenizer_training_manifest,
        tokenizer_bundle,
        tokenized_corpus_manifest,
        vocabulary_artifact,
        shard_artifacts,
    };
    bundle.validate()?;
    Ok(bundle)
}

fn load_reference_sources(
    repo_root: &Path,
) -> Result<Vec<LoadedSource>, PsionReferenceCorpusError> {
    let source_root = repo_root.join(PSION_REFERENCE_CORPUS_SOURCE_ROOT);
    let mut loaded = Vec::new();
    for blueprint in REFERENCE_SOURCES {
        let mut documents = Vec::new();
        let mut source_raw_parts = Vec::new();
        let mut source_normalized_parts = Vec::new();
        for document_blueprint in blueprint.documents {
            let path = source_root.join(document_blueprint.relative_path);
            let raw_text = fs::read_to_string(&path)?;
            let normalized_text = normalize_text(&raw_text);
            let sections = parse_sections(&normalized_text)?;
            if sections.is_empty() {
                return Err(PsionReferenceCorpusError::Parse {
                    detail: format!(
                        "document `{}` did not contain any section markers",
                        document_blueprint.document_id
                    ),
                });
            }
            source_raw_parts.push(format!(
                "{}:{}",
                document_blueprint.document_id,
                raw_text.trim()
            ));
            source_normalized_parts.push(format!(
                "{}:{}",
                document_blueprint.document_id,
                normalized_text.trim()
            ));
            documents.push(LoadedDocument {
                blueprint: *document_blueprint,
                raw_text,
                normalized_text,
                sections,
            });
        }
        loaded.push(LoadedSource {
            blueprint,
            documents,
            source_raw_digest: stable_digest(b"psion_reference_source_raw|", &source_raw_parts),
            source_normalized_digest: stable_digest(
                b"psion_reference_source_normalized|",
                &source_normalized_parts,
            ),
        });
    }
    loaded.sort_by_key(|source| source.blueprint.source_id);
    Ok(loaded)
}

fn build_admission_manifest(
    loaded_sources: &[LoadedSource],
) -> Result<PsionCorpusAdmissionManifest, PsionReferenceCorpusError> {
    let policy = PsionCorpusAdmissionPolicy::v1();
    let mut sources = loaded_sources
        .iter()
        .map(|source| PsionSourceAdmissionRecord {
            source_id: String::from(source.blueprint.source_id),
            source_family_id: String::from(source.blueprint.source_family_id),
            source_kind: source.blueprint.source_kind,
            title: String::from(source.blueprint.title),
            author_or_origin: String::from(source.blueprint.author_or_origin),
            publication_year_or_revision: String::from(
                source.blueprint.publication_year_or_revision,
            ),
            language: String::from(source.blueprint.language),
            canonical_reference: String::from(source.blueprint.canonical_reference),
            content_digest: source.source_raw_digest.clone(),
            rights_posture: source.blueprint.rights_posture,
            normative_role: source.blueprint.normative_role,
            boundary_kind: source.blueprint.boundary_kind,
            review_decision: source.blueprint.review_decision,
            reviewer: String::from(source.blueprint.reviewer),
            decision_summary: String::from(source.blueprint.decision_summary),
            notes: source.blueprint.notes.map(String::from),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let manifest = PsionCorpusAdmissionManifest {
        schema_version: policy.schema_version.clone(),
        policy_id: policy.policy_id.clone(),
        policy_version: policy.policy_version.clone(),
        sources,
    };
    manifest.validate_against_policy(&policy)?;
    Ok(manifest)
}

fn build_lifecycle_manifest(
    loaded_sources: &[LoadedSource],
    admission_manifest: &PsionCorpusAdmissionManifest,
) -> Result<PsionSourceLifecycleManifest, PsionReferenceCorpusError> {
    let mut sources = loaded_sources
        .iter()
        .map(|source| PsionSourceLifecycleRecord {
            source_id: String::from(source.blueprint.source_id),
            lifecycle_state: source.blueprint.lifecycle_state,
            admission_rights_posture: source.blueprint.rights_posture,
            current_rights_posture: source.blueprint.rights_posture,
            last_transition_event_id: format!("{}:initial", source.blueprint.source_id),
            last_transition_trigger: source.blueprint.last_transition_trigger,
            rationale: String::from(source.blueprint.lifecycle_rationale),
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let manifest = PsionSourceLifecycleManifest {
        schema_version: String::from(crate::PSION_SOURCE_LIFECYCLE_SCHEMA_VERSION),
        policy_id: admission_manifest.policy_id.clone(),
        policy_version: admission_manifest.policy_version.clone(),
        admission_schema_version: admission_manifest.schema_version.clone(),
        sources,
    };
    manifest.validate_against_admission(admission_manifest)?;
    Ok(manifest)
}

fn build_exclusion_manifest(
    loaded_sources: &[LoadedSource],
    lifecycle_manifest: &PsionSourceLifecycleManifest,
) -> Result<PsionExclusionManifest, PsionReferenceCorpusError> {
    let held_out_source_ids = vec![String::from("spec_quiz_eval_pack_v1")];
    let training_excluded_source_ids = vec![
        String::from("spec_quiz_eval_pack_v1"),
        String::from("vendor_manual_private_scan_v1"),
    ];
    let benchmark_excluded_source_ids = loaded_sources
        .iter()
        .filter(|source| !source.blueprint.benchmark_exposed)
        .map(|source| String::from(source.blueprint.source_id))
        .collect::<Vec<_>>();
    let mut tokenizer_exposure = loaded_sources
        .iter()
        .map(|source| PsionTokenizerExposureRecord {
            source_id: String::from(source.blueprint.source_id),
            tokenizer_exposed: source.blueprint.tokenizer_exposed,
            model_training_exposed: source.blueprint.model_training_exposed,
            benchmark_exposed: source.blueprint.benchmark_exposed,
            detail: format!(
                "{} exposure keeps tokenizer={}, train={}, benchmark={} explicit.",
                source.blueprint.source_id,
                source.blueprint.tokenizer_exposed,
                source.blueprint.model_training_exposed,
                source.blueprint.benchmark_exposed
            ),
        })
        .collect::<Vec<_>>();
    tokenizer_exposure.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let section_range_rules = vec![
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::Textbook,
            required_boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::ChapterSectionDisjoint,
            rationale: String::from(
                "Textbooks stay chapter-section anchored in the reference corpus.",
            ),
        },
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::Specification,
            required_boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::ChapterSectionDisjoint,
            rationale: String::from(
                "Specifications stay chapter-section anchored in the reference corpus.",
            ),
        },
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::Manual,
            required_boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::ChapterSectionDisjoint,
            rationale: String::from(
                "Manuals stay chapter-section anchored even when tokenizer-only.",
            ),
        },
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::Paper,
            required_boundary_kind: PsionSourceBoundaryKind::PageRangeAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::PageRangeDisjoint,
            rationale: String::from("Papers would require page-range disjointness if added later."),
        },
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::TechnicalDocumentation,
            required_boundary_kind: PsionSourceBoundaryKind::ChapterSectionAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::ChapterSectionDisjoint,
            rationale: String::from(
                "Technical docs stay chapter-section anchored in the reference corpus.",
            ),
        },
        PsionSectionRangeDisjointnessRule {
            source_kind: PsionCorpusSourceKind::ExpertDiscussion,
            required_boundary_kind: PsionSourceBoundaryKind::RecordAnchors,
            disjointness_mode: PsionSectionDisjointnessMode::EntireSourceDisjoint,
            rationale: String::from(
                "Evaluation-only records stay entirely disjoint from train-visible sources.",
            ),
        },
    ];
    let manifest = PsionExclusionManifest {
        schema_version: String::from(crate::PSION_BENCHMARK_ISOLATION_SCHEMA_VERSION),
        lifecycle_schema_version: lifecycle_manifest.schema_version.clone(),
        held_out_source_ids,
        training_excluded_source_ids,
        benchmark_excluded_source_ids,
        tokenizer_exposure,
        section_range_rules,
        near_duplicate_review_policy: PsionNearDuplicateReviewPolicy {
            review_required_before_training: true,
            review_required_before_benchmark_publication: true,
        },
        contamination_violation_consequences: vec![
            PsionContaminationViolationConsequence::InvalidateAffectedBenchmark,
            PsionContaminationViolationConsequence::TriggerCapabilityMatrixReview,
            PsionContaminationViolationConsequence::TriggerBenchmarkRebuildReview,
        ],
    };
    manifest.validate_against_lifecycle(lifecycle_manifest)?;
    Ok(manifest)
}

fn build_raw_source_manifest(
    loaded_sources: &[LoadedSource],
    admission_manifest: &PsionCorpusAdmissionManifest,
    lifecycle_manifest: &PsionSourceLifecycleManifest,
) -> Result<PsionRawSourceManifest, PsionReferenceCorpusError> {
    let normalization_profile = PsionRawSourceNormalizationProfile {
        preprocessing_version: String::from(PSION_REFERENCE_PREPROCESSING_VERSION),
        normalization_steps: vec![
            PsionRawSourceNormalizationStep::UnicodeNfc,
            PsionRawSourceNormalizationStep::NormalizeLineEndings,
            PsionRawSourceNormalizationStep::StripBoilerplate,
            PsionRawSourceNormalizationStep::PreserveSectionAnchors,
        ],
        preserves_document_boundaries: true,
        preserves_section_boundaries: true,
    };
    let mut sources = Vec::new();
    for loaded_source in loaded_sources {
        let mut documents = Vec::new();
        for (document_index, loaded_document) in loaded_source.documents.iter().enumerate() {
            let mut section_boundaries = Vec::new();
            for (section_index, section) in loaded_document.sections.iter().enumerate() {
                section_boundaries.push(PsionRawSourceSectionBoundary {
                    section_id: section.section_id.clone(),
                    boundary_kind: loaded_source.blueprint.boundary_kind,
                    order_index: u32::try_from(section_index + 1)
                        .expect("section index should fit in u32"),
                    title: section.title.clone(),
                    start_anchor: section.section_id.clone(),
                    end_anchor: section.section_id.clone(),
                    normalized_section_digest: stable_digest(
                        b"psion_reference_section|",
                        &section.text,
                    ),
                });
            }
            documents.push(PsionRawSourceDocument {
                document_id: String::from(loaded_document.blueprint.document_id),
                order_index: u32::try_from(document_index + 1)
                    .expect("document index should fit in u32"),
                import_kind: loaded_document.blueprint.import_kind,
                import_reference: String::from(loaded_document.blueprint.import_reference),
                raw_document_digest: stable_digest(
                    b"psion_reference_raw_document|",
                    &loaded_document.raw_text,
                ),
                normalized_document_digest: stable_digest(
                    b"psion_reference_normalized_document|",
                    &loaded_document.normalized_text,
                ),
                section_boundaries,
            });
        }
        sources.push(PsionRawSourceRecord {
            source_id: String::from(loaded_source.blueprint.source_id),
            source_family_id: String::from(loaded_source.blueprint.source_family_id),
            source_kind: loaded_source.blueprint.source_kind,
            current_rights_posture: loaded_source.blueprint.rights_posture,
            lifecycle_state: loaded_source.blueprint.lifecycle_state,
            source_raw_digest: loaded_source.source_raw_digest.clone(),
            source_normalized_digest: loaded_source.source_normalized_digest.clone(),
            documents,
            normalization_detail: format!(
                "{} normalized with stable section markers preserved.",
                loaded_source.blueprint.source_id
            ),
        });
    }
    Ok(PsionRawSourceManifest::new(
        normalization_profile,
        admission_manifest,
        lifecycle_manifest,
        sources,
    )?)
}

fn build_vocabulary_artifact(loaded_sources: &[LoadedSource]) -> PsionReferenceVocabularyArtifact {
    let mut counts = BTreeMap::<String, u32>::new();
    for source in loaded_sources {
        if !source.blueprint.tokenizer_exposed {
            continue;
        }
        for document in &source.documents {
            for token in tokenize_text(&document.normalized_text) {
                *counts.entry(token).or_insert(0) += 1;
            }
        }
    }
    let mut tokens = SPECIAL_TOKENS
        .iter()
        .map(|token| String::from(*token))
        .collect::<Vec<_>>();
    let mut vocabulary_tokens = counts.into_iter().collect::<Vec<_>>();
    vocabulary_tokens.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    tokens.extend(vocabulary_tokens.into_iter().map(|(token, _)| token));
    PsionReferenceVocabularyArtifact {
        schema_version: String::from("psion.reference_vocabulary.v1"),
        tokenizer_id: String::from(PSION_REFERENCE_TOKENIZER_ID),
        tokenizer_version: String::from(PSION_REFERENCE_TOKENIZER_VERSION),
        vocabulary_digest: stable_digest(b"psion_reference_vocabulary|", &tokens),
        tokens,
    }
}

fn build_vocabulary_artifact_for_tokenizer(
    loaded_sources: &[LoadedSource],
    tokenizer_training_manifest: &PsionTokenizerTrainingManifest,
) -> PsionReferenceVocabularyArtifact {
    let admitted_source_ids = tokenizer_training_manifest
        .admitted_sources
        .iter()
        .map(|source| source.source_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut counts = BTreeMap::<String, u32>::new();
    for source in loaded_sources {
        if !admitted_source_ids.contains(source.blueprint.source_id) {
            continue;
        }
        for document in &source.documents {
            for token in tokenize_text(&document.normalized_text) {
                *counts.entry(token).or_insert(0) += 1;
            }
        }
    }
    let mut tokens = tokenizer_training_manifest
        .training_config
        .special_tokens
        .clone();
    let mut vocabulary_tokens = counts.into_iter().collect::<Vec<_>>();
    vocabulary_tokens.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    tokens.extend(vocabulary_tokens.into_iter().map(|(token, _)| token));
    PsionReferenceVocabularyArtifact {
        schema_version: String::from("psion.reference_vocabulary.v1"),
        tokenizer_id: tokenizer_training_manifest.tokenizer_id.clone(),
        tokenizer_version: tokenizer_training_manifest.tokenizer_version.clone(),
        vocabulary_digest: stable_digest(b"psion_reference_vocabulary|", &tokens),
        tokens,
    }
}

fn build_tokenizer_training_manifest(
    loaded_sources: &[LoadedSource],
    raw_source_manifest: &PsionRawSourceManifest,
    exclusion_manifest: &PsionExclusionManifest,
    target_vocab_size: u32,
) -> PsionTokenizerTrainingManifest {
    let raw_source_map = raw_source_manifest
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut admitted_sources = Vec::new();
    let mut excluded_sources = Vec::new();
    let mut exposure_report = Vec::new();
    for source in loaded_sources {
        let raw_source = raw_source_map
            .get(source.blueprint.source_id)
            .expect("loaded source should be present in raw manifest");
        let reference = PsionTokenizerSourceReference {
            source_id: String::from(source.blueprint.source_id),
            source_family_id: String::from(source.blueprint.source_family_id),
            source_normalized_digest: raw_source.source_normalized_digest.clone(),
        };
        if source.blueprint.tokenizer_exposed {
            admitted_sources.push(reference);
        } else {
            excluded_sources.push(reference);
        }
        let exposure = exclusion_manifest
            .tokenizer_exposure
            .iter()
            .find(|row| row.source_id == source.blueprint.source_id)
            .expect("exclusion manifest should carry every source");
        exposure_report.push(PsionTokenizerExposureAuditRow {
            source_id: exposure.source_id.clone(),
            tokenizer_exposed: exposure.tokenizer_exposed,
            tokenizer_only_exposure: exposure.tokenizer_exposed && !exposure.model_training_exposed,
            model_training_exposed: exposure.model_training_exposed,
            detail: exposure.detail.clone(),
        });
    }
    admitted_sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    excluded_sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    exposure_report.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    PsionTokenizerTrainingManifest {
        schema_version: String::from(crate::PSION_TOKENIZER_TRAINING_MANIFEST_SCHEMA_VERSION),
        tokenizer_id: String::from(PSION_REFERENCE_TOKENIZER_ID),
        tokenizer_version: String::from(PSION_REFERENCE_TOKENIZER_VERSION),
        raw_source_schema_version: raw_source_manifest.schema_version.clone(),
        exclusion_schema_version: exclusion_manifest.schema_version.clone(),
        preprocessing_version: String::from(PSION_REFERENCE_PREPROCESSING_VERSION),
        training_config: PsionTokenizerTrainingConfig {
            tokenizer_family: TokenizerFamily::SentencePiece,
            algorithm: PsionTokenizerTrainingAlgorithm::SentencePieceUnigram,
            target_vocab_size,
            character_coverage_bps: 9990,
            lowercase_ascii: true,
            special_tokens: SPECIAL_TOKENS
                .iter()
                .map(|token| String::from(*token))
                .collect(),
        },
        admitted_sources,
        excluded_sources,
        exposure_report,
    }
}

fn build_shard_artifacts(
    loaded_sources: &[LoadedSource],
    raw_source_manifest: &PsionRawSourceManifest,
    vocabulary_artifact: &PsionReferenceVocabularyArtifact,
) -> Result<Vec<PsionReferenceShardArtifact>, PsionReferenceCorpusError> {
    let token_map = vocabulary_artifact
        .tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            (
                token.clone(),
                u32::try_from(index).expect("token index should fit in u32"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut train_sequences = Vec::new();
    let mut validation_sequences = Vec::new();
    let mut held_out_sequences = Vec::new();
    let raw_source_map = raw_source_manifest
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for source in loaded_sources {
        let raw_source = raw_source_map
            .get(source.blueprint.source_id)
            .expect("loaded source should exist in raw source manifest");
        let mut source_sequences = Vec::new();
        for document in &source.documents {
            for section in &document.sections {
                let mut token_ids = vec![token_id(&token_map, "<bos>")];
                token_ids.extend(tokenize_text(&section.text).into_iter().map(|token| {
                    token_map
                        .get(&token)
                        .copied()
                        .unwrap_or(token_id(&token_map, "<unk>"))
                }));
                token_ids.push(token_id(&token_map, "<eos>"));
                if token_ids.len() > usize::try_from(PSION_REFERENCE_MAX_SEQUENCE_TOKENS).unwrap() {
                    token_ids
                        .truncate(usize::try_from(PSION_REFERENCE_MAX_SEQUENCE_TOKENS).unwrap());
                    if let Some(last) = token_ids.last_mut() {
                        *last = token_id(&token_map, "<eos>");
                    }
                }
                source_sequences.push(PsionReferenceEncodedSequence {
                    sequence_id: format!("{}:{}", source.blueprint.source_id, section.section_id),
                    source_id: String::from(source.blueprint.source_id),
                    source_family_id: String::from(source.blueprint.source_family_id),
                    split_name: String::new(),
                    split_kind: DatasetSplitKind::Custom,
                    token_ids,
                });
            }
        }
        if source.blueprint.benchmark_exposed {
            for mut sequence in source_sequences {
                sequence.split_name = String::from("held_out");
                sequence.split_kind = DatasetSplitKind::HeldOut;
                held_out_sequences.push(sequence);
            }
            continue;
        }
        if !source.blueprint.model_training_exposed {
            continue;
        }
        let validation_index = source_sequences.len().saturating_sub(1);
        for (index, mut sequence) in source_sequences.into_iter().enumerate() {
            if index == validation_index {
                sequence.split_name = String::from("validation");
                sequence.split_kind = DatasetSplitKind::Validation;
                validation_sequences.push(sequence);
            } else {
                sequence.split_name = String::from("train");
                sequence.split_kind = DatasetSplitKind::Train;
                train_sequences.push(sequence);
            }
        }
        let _ = raw_source;
    }
    train_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));
    validation_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));
    held_out_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));
    let shard_artifacts = vec![
        build_shard_artifact(
            "psion_reference_train_shard_0001",
            "train",
            DatasetSplitKind::Train,
            train_sequences,
        ),
        build_shard_artifact(
            "psion_reference_validation_shard_0001",
            "validation",
            DatasetSplitKind::Validation,
            validation_sequences,
        ),
        build_shard_artifact(
            "psion_reference_held_out_shard_0001",
            "held_out",
            DatasetSplitKind::HeldOut,
            held_out_sequences,
        ),
    ];
    Ok(shard_artifacts)
}

fn build_actual_pretraining_shard_artifacts(
    loaded_sources: &[LoadedSource],
    raw_source_manifest: &PsionRawSourceManifest,
    tokenizer_training_manifest: &PsionTokenizerTrainingManifest,
    tokenized_corpus_manifest: &PsionTokenizedCorpusManifest,
    exclusion_manifest: &PsionExclusionManifest,
    vocabulary_artifact: &PsionReferenceVocabularyArtifact,
) -> Result<Vec<PsionReferenceShardArtifact>, PsionReferenceCorpusError> {
    let token_map = vocabulary_artifact
        .tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            (
                token.clone(),
                u32::try_from(index).expect("token index should fit in u32"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let max_sequence_tokens = usize::try_from(
        tokenized_corpus_manifest.packing_policy.max_sequence_tokens,
    )
    .map_err(|_| PsionReferenceCorpusError::Parse {
        detail: String::from("actual pretraining packing max_sequence_tokens did not fit in usize"),
    })?;
    let raw_source_map = raw_source_manifest
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let held_out_source_ids = exclusion_manifest
        .held_out_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let training_excluded_source_ids = exclusion_manifest
        .training_excluded_source_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let training_visible_source_ids = tokenizer_training_manifest
        .exposure_report
        .iter()
        .filter(|row| row.model_training_exposed)
        .map(|row| row.source_id.as_str())
        .collect::<BTreeSet<_>>();

    let mut train_sequences = Vec::new();
    let mut validation_sequences = Vec::new();
    let mut held_out_sequences = Vec::new();

    for source in loaded_sources {
        let mut source_sequences = Vec::new();
        for document in &source.documents {
            for section in &document.sections {
                let mut token_ids = vec![token_id(&token_map, "<bos>")];
                token_ids.extend(tokenize_text(&section.text).into_iter().map(|token| {
                    token_map
                        .get(&token)
                        .copied()
                        .unwrap_or(token_id(&token_map, "<unk>"))
                }));
                token_ids.push(token_id(&token_map, "<eos>"));
                if token_ids.len() > max_sequence_tokens {
                    token_ids.truncate(max_sequence_tokens);
                    if let Some(last) = token_ids.last_mut() {
                        *last = token_id(&token_map, "<eos>");
                    }
                }
                source_sequences.push(PsionReferenceEncodedSequence {
                    sequence_id: format!("{}:{}", source.blueprint.source_id, section.section_id),
                    source_id: String::from(source.blueprint.source_id),
                    source_family_id: String::from(source.blueprint.source_family_id),
                    split_name: String::new(),
                    split_kind: DatasetSplitKind::Custom,
                    token_ids,
                });
            }
        }

        if held_out_source_ids.contains(source.blueprint.source_id) {
            for mut sequence in source_sequences {
                sequence.split_name = String::from("held_out");
                sequence.split_kind = DatasetSplitKind::HeldOut;
                held_out_sequences.push(sequence);
            }
            continue;
        }

        if training_excluded_source_ids.contains(source.blueprint.source_id)
            || !training_visible_source_ids.contains(source.blueprint.source_id)
        {
            continue;
        }

        let validation_index = source_sequences.len().saturating_sub(1);
        for (index, mut sequence) in source_sequences.into_iter().enumerate() {
            if index == validation_index {
                sequence.split_name = String::from("validation");
                sequence.split_kind = DatasetSplitKind::Validation;
                validation_sequences.push(sequence);
            } else {
                sequence.split_name = String::from("train");
                sequence.split_kind = DatasetSplitKind::Train;
                train_sequences.push(sequence);
            }
        }

        let _ = raw_source_map;
    }

    train_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));
    validation_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));
    held_out_sequences.sort_by(|left, right| left.sequence_id.cmp(&right.sequence_id));

    Ok(vec![
        build_shard_artifact(
            "psion_actual_train_shard_0001",
            "train",
            DatasetSplitKind::Train,
            train_sequences,
        ),
        build_shard_artifact(
            "psion_actual_validation_shard_0001",
            "validation",
            DatasetSplitKind::Validation,
            validation_sequences,
        ),
        build_shard_artifact(
            "psion_actual_held_out_shard_0001",
            "held_out",
            DatasetSplitKind::HeldOut,
            held_out_sequences,
        ),
    ])
}

fn build_shard_artifact(
    shard_id: &str,
    split_name: &str,
    split_kind: DatasetSplitKind,
    sequences: Vec<PsionReferenceEncodedSequence>,
) -> PsionReferenceShardArtifact {
    let mut artifact = PsionReferenceShardArtifact {
        schema_version: String::from("psion.reference_shard_artifact.v1"),
        shard_id: String::from(shard_id),
        split_name: String::from(split_name),
        split_kind,
        shard_digest: String::new(),
        sequences,
    };
    artifact.shard_digest = stable_digest(b"psion_reference_shard|", &artifact.sequences);
    artifact
}

fn build_tokenized_corpus_manifest(
    raw_source_manifest: &PsionRawSourceManifest,
    lifecycle_manifest: &PsionSourceLifecycleManifest,
    exclusion_manifest: &PsionExclusionManifest,
    tokenizer_bundle: &PsionTokenizerArtifactBundle,
    shard_artifacts: &[PsionReferenceShardArtifact],
) -> Result<PsionTokenizedCorpusManifest, PsionReferenceCorpusError> {
    let mut source_family_bindings = Vec::new();
    let mut family_map = BTreeMap::<String, BTreeSet<String>>::new();
    let raw_source_map = raw_source_manifest
        .sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut splits = Vec::new();
    let mut shards = Vec::new();
    for artifact in shard_artifacts {
        let mut split_source_families = BTreeSet::new();
        let mut lineage_rows = Vec::new();
        let mut seen_sources = BTreeSet::new();
        let mut token_count = 0_u64;
        let mut min_sequence_tokens = u32::MAX;
        let mut max_sequence_tokens = 0_u32;
        for sequence in &artifact.sequences {
            token_count = token_count.saturating_add(sequence.token_ids.len() as u64);
            min_sequence_tokens = min_sequence_tokens.min(sequence.token_ids.len() as u32);
            max_sequence_tokens = max_sequence_tokens.max(sequence.token_ids.len() as u32);
            split_source_families.insert(sequence.source_family_id.clone());
            family_map
                .entry(sequence.source_family_id.clone())
                .or_default()
                .insert(sequence.source_id.clone());
            if seen_sources.insert(sequence.source_id.clone()) {
                let raw_source = raw_source_map
                    .get(sequence.source_id.as_str())
                    .expect("sequence source should exist in raw source manifest");
                lineage_rows.push(PsionTokenizedShardSourceLineage {
                    source_id: sequence.source_id.clone(),
                    source_family_id: sequence.source_family_id.clone(),
                    source_normalized_digest: raw_source.source_normalized_digest.clone(),
                });
            }
        }
        if artifact.sequences.is_empty() {
            return Err(PsionReferenceCorpusError::Parse {
                detail: format!(
                    "shard `{}` did not contain any sequences",
                    artifact.shard_id
                ),
            });
        }
        lineage_rows.sort_by(|left, right| left.source_id.cmp(&right.source_id));
        let split_name = artifact.split_name.clone();
        shards.push(PsionTokenizedShardManifest {
            shard_id: artifact.shard_id.clone(),
            split_name: split_name.clone(),
            split_kind: artifact.split_kind,
            storage_ref: format!(
                "tokenized/{}.json",
                split_name.replace('-', "_").replace(" ", "_") + "_shard_0001"
            ),
            shard_digest: artifact.shard_digest.clone(),
            sequence_count: artifact.sequences.len() as u64,
            token_count,
            min_sequence_tokens,
            max_sequence_tokens,
            tokenizer_digest: tokenizer_bundle.tokenizer.tokenizer_digest.clone(),
            source_manifest_schema_version: raw_source_manifest.schema_version.clone(),
            preprocessing_version: String::from(PSION_REFERENCE_PREPROCESSING_VERSION),
            packing_policy_version: String::from(PSION_REFERENCE_PACKING_POLICY_VERSION),
            source_lineage: lineage_rows,
        });
        splits.push(PsionTokenizedSplitManifest {
            split_name: split_name.clone(),
            kind: artifact.split_kind,
            shard_ids: vec![artifact.shard_id.clone()],
            source_family_ids: split_source_families.into_iter().collect(),
            sequence_count: artifact.sequences.len() as u64,
            token_count,
        });
    }
    for (source_family_id, source_ids) in family_map {
        source_family_bindings.push(PsionTokenizedSourceFamilyBinding {
            source_family_id,
            source_ids: source_ids.into_iter().collect(),
        });
    }
    Ok(PsionTokenizedCorpusManifest::new(
        PSION_REFERENCE_DATASET_ID,
        PSION_REFERENCE_DATASET_VERSION,
        PsionTokenizedPackingPolicy {
            policy_id: String::from(PSION_REFERENCE_PACKING_POLICY_ID),
            policy_version: String::from(PSION_REFERENCE_PACKING_POLICY_VERSION),
            packing_mode: DatasetPackingMode::PackIntoContextWindow,
            max_sequence_tokens: PSION_REFERENCE_MAX_SEQUENCE_TOKENS,
            overlong_sequence_posture: OverlongSequencePosture::Drop,
        },
        PsionTokenizedReplayContract {
            iteration_mode: DatasetIterationMode::SinglePass,
            shard_ordering: DatasetShardOrdering::ManifestOrder,
            deterministic_shuffle_seed: 11,
            stable_dataset_identity: String::from(PSION_REFERENCE_DATASET_IDENTITY),
        },
        source_family_bindings,
        splits,
        shards,
        tokenizer_bundle,
        raw_source_manifest,
        lifecycle_manifest,
        exclusion_manifest,
    )?)
}

fn normalize_text(raw_text: &str) -> String {
    raw_text.replace("\r\n", "\n").trim().to_string()
}

fn parse_sections(normalized_text: &str) -> Result<Vec<ParsedSection>, PsionReferenceCorpusError> {
    let mut sections = Vec::new();
    let mut current_id = None::<String>;
    let mut current_title = None::<String>;
    let mut current_lines = Vec::<String>::new();
    for line in normalized_text.lines() {
        if let Some((section_id, title)) = parse_section_marker(line) {
            if let (Some(previous_id), Some(previous_title)) =
                (current_id.take(), current_title.take())
            {
                sections.push(ParsedSection {
                    section_id: previous_id,
                    title: previous_title,
                    text: current_lines.join(" ").trim().to_string(),
                });
                current_lines.clear();
            }
            current_id = Some(section_id);
            current_title = Some(title);
            continue;
        }
        if !line.trim().is_empty() {
            current_lines.push(line.trim().to_string());
        }
    }
    if let (Some(section_id), Some(title)) = (current_id.take(), current_title.take()) {
        sections.push(ParsedSection {
            section_id,
            title,
            text: current_lines.join(" ").trim().to_string(),
        });
    }
    if sections.iter().any(|section| section.text.is_empty()) {
        return Err(PsionReferenceCorpusError::Parse {
            detail: String::from("one or more section markers had empty content"),
        });
    }
    Ok(sections)
}

fn parse_section_marker(line: &str) -> Option<(String, String)> {
    let marker = line.trim();
    let marker = marker.strip_prefix("[[SECTION:")?;
    let marker = marker.strip_suffix("]]")?;
    let (section_id, title) = marker.split_once('|')?;
    Some((String::from(section_id.trim()), String::from(title.trim())))
}

fn tokenize_text(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn token_id(token_map: &BTreeMap<String, u32>, token: &str) -> u32 {
    token_map
        .get(token)
        .copied()
        .expect("special token should be present in the reference vocabulary")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("reference corpus digest serialization should succeed"),
    );
    hex::encode(hasher.finalize())
}

fn load_json_fixture<T: serde::de::DeserializeOwned>(
    repo_root: &Path,
    relative_path: &str,
) -> Result<T, PsionReferenceCorpusError> {
    let payload = fs::read_to_string(repo_root.join(relative_path))?;
    serde_json::from_str(&payload).map_err(PsionReferenceCorpusError::Serialization)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::path::PathBuf;

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate should live under workspace root")
            .parent()
            .expect("workspace root should exist")
            .to_path_buf()
    }

    #[test]
    fn reference_corpus_builds_validated_psion_artifacts() {
        let bundle = build_psion_reference_corpus(repo_root().as_path())
            .expect("reference corpus should build");
        bundle.validate().expect("reference corpus should validate");
        assert!(
            bundle
                .tokenized_corpus_manifest
                .splits
                .iter()
                .any(|split| split.kind == DatasetSplitKind::HeldOut)
        );
        assert!(
            bundle
                .tokenizer_training_manifest
                .admitted_sources
                .iter()
                .any(|source| source.source_id == "vendor_manual_private_scan_v1")
        );
    }

    #[test]
    fn reference_corpus_writer_materializes_shards_and_manifests() {
        let bundle = build_psion_reference_corpus(repo_root().as_path())
            .expect("reference corpus should build");
        let output_dir = std::env::temp_dir().join(format!(
            "psion_reference_corpus_test_{}",
            std::process::id()
        ));
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).expect("old output dir should be removable");
        }
        bundle
            .write_to_dir(output_dir.as_path())
            .expect("writer should materialize the reference corpus");
        assert!(
            output_dir
                .join("psion_raw_source_manifest_v1.json")
                .exists()
        );
        assert!(
            output_dir
                .join("psion_tokenized_corpus_manifest_v1.json")
                .exists()
        );
        assert!(output_dir.join("tokenized/train_shard_0001.json").exists());
        fs::remove_dir_all(output_dir).expect("test output should be removable");
    }

    #[test]
    fn actual_pretraining_reconstructed_corpus_builds_validated_inputs() {
        let bundle = build_psion_actual_pretraining_reconstructed_corpus(repo_root().as_path())
            .expect("actual pretraining corpus should build");
        bundle
            .validate()
            .expect("actual pretraining corpus should validate");
        assert_eq!(
            bundle
                .tokenized_corpus_manifest
                .replay_contract
                .stable_dataset_identity,
            "psion_corpus_tokenized@v1"
        );
        assert_eq!(
            bundle.tokenizer_bundle.tokenizer.tokenizer_digest,
            "sha256:psion_sentencepiece_seed_tokenizer_digest_v1"
        );
        assert!(
            bundle
                .shard_artifacts
                .iter()
                .any(|artifact| artifact.split_kind == DatasetSplitKind::Train
                    && !artifact.sequences.is_empty()),
            "actual pretraining reconstructed corpus should retain non-empty train sequences"
        );
    }
}
