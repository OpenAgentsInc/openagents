use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_a1_minimal_distributed_lm_lane_contract,
    canonical_a1_minimal_distributed_lm_tokenized_dataset_digest,
    canonical_a1_minimal_distributed_lm_tokenizer_digest,
    canonical_a1_minimal_distributed_lm_validation_set_digest,
    TrainingExecutionValidatorDisposition, A1_MINIMAL_DISTRIBUTED_LM_LANE_ID,
};

pub const A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_CATALOG_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.support_artifact_catalog.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_RECEIPT_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.support_artifact_receipt.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_CATALOG_FIXTURE_PATH: &str =
    "fixtures/psion/a1_minimal_distributed_lm/support_artifact_catalog_v1.json";

#[derive(Debug, Error)]
pub enum A1MinimalDistributedLmSupportArtifactError {
    #[error("A1 minimal distributed LM support artifact catalog is invalid: {detail}")]
    Invalid { detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A1MinimalDistributedLmSupportArtifactKind {
    TokenizedShardValidation,
    ValidationReplay,
    CheckpointVerification,
    EvalBatch,
    ArtifactRematerialization,
    IndependentScoredTrainingWindow,
}

impl A1MinimalDistributedLmSupportArtifactKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TokenizedShardValidation => "tokenized_shard_validation",
            Self::ValidationReplay => "validation_replay",
            Self::CheckpointVerification => "checkpoint_verification",
            Self::EvalBatch => "eval_batch",
            Self::ArtifactRematerialization => "artifact_rematerialization",
            Self::IndependentScoredTrainingWindow => "independent_scored_training_window",
        }
    }

    #[must_use]
    pub const fn public_label(self) -> &'static str {
        match self {
            Self::TokenizedShardValidation => "tokenized shard validation",
            Self::ValidationReplay => "validation replay",
            Self::CheckpointVerification => "checkpoint verification",
            Self::EvalBatch => "eval batch",
            Self::ArtifactRematerialization => "artifact rematerialization",
            Self::IndependentScoredTrainingWindow => "independent scored training window",
        }
    }

    #[must_use]
    pub const fn existing_psionic_work_class(self) -> &'static str {
        match self {
            Self::TokenizedShardValidation
            | Self::ValidationReplay
            | Self::CheckpointVerification
            | Self::ArtifactRematerialization => "validation_replay",
            Self::EvalBatch => "evaluation",
            Self::IndependentScoredTrainingWindow => "small_model_local_training",
        }
    }

    #[must_use]
    pub const fn requires_tokenizer(self) -> bool {
        matches!(
            self,
            Self::TokenizedShardValidation
                | Self::ValidationReplay
                | Self::EvalBatch
                | Self::IndependentScoredTrainingWindow
        )
    }

    #[must_use]
    pub const fn requires_tokenized_dataset(self) -> bool {
        matches!(
            self,
            Self::TokenizedShardValidation
                | Self::ValidationReplay
                | Self::IndependentScoredTrainingWindow
        )
    }

    #[must_use]
    pub const fn requires_validation_set(self) -> bool {
        matches!(self, Self::EvalBatch)
    }

    #[must_use]
    pub const fn requires_base_checkpoint(self) -> bool {
        matches!(
            self,
            Self::ValidationReplay
                | Self::CheckpointVerification
                | Self::EvalBatch
                | Self::IndependentScoredTrainingWindow
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmSupportArtifactFamily {
    pub support_artifact_kind: A1MinimalDistributedLmSupportArtifactKind,
    pub schema_version: String,
    pub public_work_label: String,
    pub existing_psionic_work_class_mapping: String,
    pub participant_eligible_on_acceptance: bool,
    pub model_progress_participant_by_default: bool,
    pub required_inputs: Vec<String>,
    pub required_outputs: Vec<String>,
    pub required_artifact_refs: Vec<String>,
    pub validator_acceptance_checks: Vec<String>,
    pub rejection_checks: Vec<String>,
    pub closeout_counter_source: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmSupportArtifactRef {
    pub artifact_role: String,
    pub artifact_ref: String,
    pub artifact_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmSupportArtifactReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub worker_id: String,
    pub pylon_provider_id: String,
    pub support_artifact_kind: A1MinimalDistributedLmSupportArtifactKind,
    pub existing_psionic_work_class: String,
    pub tokenizer_digest: Option<String>,
    pub tokenized_dataset_digest: Option<String>,
    pub validation_set_digest: Option<String>,
    pub base_checkpoint_ref: Option<String>,
    pub input_refs: Vec<A1MinimalDistributedLmSupportArtifactRef>,
    pub output_refs: Vec<A1MinimalDistributedLmSupportArtifactRef>,
    pub required_validator_checks: Vec<String>,
    pub validator_disposition: TrainingExecutionValidatorDisposition,
    pub validator_verdict_binding: String,
    pub closeout_verdict_binding: String,
    pub participant_eligible: bool,
    pub accepted_participant_work: bool,
    pub model_progress_participant: bool,
    pub participant_counter_source: String,
    pub model_progress_counter_source: String,
    pub claim_boundary: String,
    pub detail: String,
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmSupportArtifactCatalog {
    pub schema_version: String,
    pub lane_id: String,
    pub tokenizer_digest: String,
    pub tokenized_dataset_digest: String,
    pub validation_set_digest: String,
    pub base_checkpoint_ref: String,
    pub participant_counter_source: String,
    pub model_progress_counter_source: String,
    pub support_families: Vec<A1MinimalDistributedLmSupportArtifactFamily>,
    pub retained_example_receipts: Vec<A1MinimalDistributedLmSupportArtifactReceipt>,
    pub claim_boundary: String,
    pub catalog_digest: String,
}

impl A1MinimalDistributedLmSupportArtifactCatalog {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.catalog_digest.clear();
        stable_digest(
            b"psion_a1_minimal_distributed_lm_support_artifact_catalog|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
        ensure_eq(
            self.schema_version.as_str(),
            A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_CATALOG_SCHEMA_VERSION,
            "catalog.schema_version",
        )?;
        ensure_eq(
            self.lane_id.as_str(),
            A1_MINIMAL_DISTRIBUTED_LM_LANE_ID,
            "catalog.lane_id",
        )?;
        ensure_eq(
            self.tokenizer_digest.as_str(),
            expected_tokenizer_digest()?.as_str(),
            "catalog.tokenizer_digest",
        )?;
        ensure_eq(
            self.tokenized_dataset_digest.as_str(),
            expected_tokenized_dataset_digest()?.as_str(),
            "catalog.tokenized_dataset_digest",
        )?;
        ensure_eq(
            self.validation_set_digest.as_str(),
            expected_validation_set_digest()?.as_str(),
            "catalog.validation_set_digest",
        )?;
        ensure_eq(
            self.base_checkpoint_ref.as_str(),
            expected_base_checkpoint_ref()?.as_str(),
            "catalog.base_checkpoint_ref",
        )?;
        ensure_eq(
            self.participant_counter_source.as_str(),
            "training_accepted_contributors",
            "catalog.participant_counter_source",
        )?;
        ensure_eq(
            self.model_progress_counter_source.as_str(),
            "training_model_progress_contributors",
            "catalog.model_progress_counter_source",
        )?;
        ensure_nonempty(self.claim_boundary.as_str(), "catalog.claim_boundary")?;
        if !self
            .claim_boundary
            .contains("support/verifier work can count as participants")
            || !self
                .claim_boundary
                .contains("does not count as model-progress participants")
        {
            return invalid(String::from(
                "catalog claim boundary must distinguish participant support work from model-progress participant work",
            ));
        }

        let expected_kinds = expected_support_artifact_kinds();
        let mut family_kinds = BTreeSet::new();
        for family in &self.support_families {
            family.validate()?;
            family_kinds.insert(family.support_artifact_kind);
        }
        if family_kinds != expected_kinds {
            return invalid(format!(
                "support family set drifted: expected {:?}, got {:?}",
                expected_kinds, family_kinds
            ));
        }

        let mut receipt_kinds = BTreeSet::new();
        for receipt in &self.retained_example_receipts {
            let family = self
                .support_families
                .iter()
                .find(|candidate| candidate.support_artifact_kind == receipt.support_artifact_kind)
                .ok_or_else(|| A1MinimalDistributedLmSupportArtifactError::Invalid {
                    detail: format!(
                        "receipt `{}` has no matching support artifact family",
                        receipt.receipt_id
                    ),
                })?;
            receipt.validate_against_catalog(self, family)?;
            receipt_kinds.insert(receipt.support_artifact_kind);
        }
        if receipt_kinds != expected_kinds {
            return invalid(format!(
                "retained example receipt set drifted: expected {:?}, got {:?}",
                expected_kinds, receipt_kinds
            ));
        }

        ensure_sha256_uri(self.catalog_digest.as_str(), "catalog.catalog_digest")?;
        if self.catalog_digest != self.stable_digest() {
            return invalid(String::from(
                "catalog_digest drifted from canonical contents",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmSupportArtifactFamily {
    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
        ensure_eq(
            self.schema_version.as_str(),
            support_artifact_schema_version(self.support_artifact_kind).as_str(),
            "family.schema_version",
        )?;
        ensure_eq(
            self.public_work_label.as_str(),
            self.support_artifact_kind.public_label(),
            "family.public_work_label",
        )?;
        ensure_eq(
            self.existing_psionic_work_class_mapping.as_str(),
            self.support_artifact_kind.existing_psionic_work_class(),
            "family.existing_psionic_work_class_mapping",
        )?;
        if !self.participant_eligible_on_acceptance {
            return invalid(format!(
                "support family `{}` must remain participant eligible on accepted closeout",
                self.support_artifact_kind.label()
            ));
        }
        if self.model_progress_participant_by_default {
            return invalid(format!(
                "support family `{}` must not count as model-progress participant work by default",
                self.support_artifact_kind.label()
            ));
        }
        ensure_vec_nonempty(self.required_inputs.as_slice(), "family.required_inputs")?;
        ensure_vec_nonempty(self.required_outputs.as_slice(), "family.required_outputs")?;
        ensure_vec_nonempty(
            self.required_artifact_refs.as_slice(),
            "family.required_artifact_refs",
        )?;
        ensure_vec_nonempty(
            self.validator_acceptance_checks.as_slice(),
            "family.validator_acceptance_checks",
        )?;
        ensure_vec_nonempty(self.rejection_checks.as_slice(), "family.rejection_checks")?;
        ensure_eq(
            self.closeout_counter_source.as_str(),
            "training_accepted_contributors",
            "family.closeout_counter_source",
        )?;
        for required_check in [
            "assignment_binding_matches",
            "artifact_digest_matches_payload",
            "validator_verdict_explicit",
            "closeout_verdict_explicit",
        ] {
            if !self
                .validator_acceptance_checks
                .iter()
                .any(|check| check == required_check)
            {
                return invalid(format!(
                    "support family `{}` lost required validator check `{required_check}`",
                    self.support_artifact_kind.label()
                ));
            }
        }
        ensure_kind_required_ref(
            self.support_artifact_kind.requires_tokenizer(),
            self.required_artifact_refs.as_slice(),
            "tokenizer_digest",
            self.support_artifact_kind,
        )?;
        ensure_kind_required_ref(
            self.support_artifact_kind.requires_tokenized_dataset(),
            self.required_artifact_refs.as_slice(),
            "tokenized_dataset_digest",
            self.support_artifact_kind,
        )?;
        ensure_kind_required_ref(
            self.support_artifact_kind.requires_validation_set(),
            self.required_artifact_refs.as_slice(),
            "validation_set_digest",
            self.support_artifact_kind,
        )?;
        ensure_kind_required_ref(
            self.support_artifact_kind.requires_base_checkpoint(),
            self.required_artifact_refs.as_slice(),
            "base_checkpoint_ref",
            self.support_artifact_kind,
        )?;
        ensure_nonempty(self.detail.as_str(), "family.detail")?;
        Ok(())
    }
}

impl A1MinimalDistributedLmSupportArtifactReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_digest(
            b"psion_a1_minimal_distributed_lm_support_artifact_receipt|",
            &clone,
        )
    }

    pub fn validate_against_catalog(
        &self,
        catalog: &A1MinimalDistributedLmSupportArtifactCatalog,
        family: &A1MinimalDistributedLmSupportArtifactFamily,
    ) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
        ensure_eq(
            self.schema_version.as_str(),
            A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_RECEIPT_SCHEMA_VERSION,
            "receipt.schema_version",
        )?;
        ensure_eq(
            self.lane_id.as_str(),
            catalog.lane_id.as_str(),
            "receipt.lane_id",
        )?;
        ensure_nonempty(self.receipt_id.as_str(), "receipt.receipt_id")?;
        ensure_nonempty(self.run_id.as_str(), "receipt.run_id")?;
        ensure_nonempty(self.window_id.as_str(), "receipt.window_id")?;
        ensure_nonempty(self.assignment_id.as_str(), "receipt.assignment_id")?;
        ensure_nonempty(self.worker_id.as_str(), "receipt.worker_id")?;
        ensure_nonempty(self.pylon_provider_id.as_str(), "receipt.pylon_provider_id")?;
        ensure_eq(
            self.support_artifact_kind.label(),
            family.support_artifact_kind.label(),
            "receipt.support_artifact_kind",
        )?;
        ensure_eq(
            self.existing_psionic_work_class.as_str(),
            family.existing_psionic_work_class_mapping.as_str(),
            "receipt.existing_psionic_work_class",
        )?;
        ensure_optional_ref(
            self.support_artifact_kind.requires_tokenizer(),
            self.tokenizer_digest.as_deref(),
            catalog.tokenizer_digest.as_str(),
            "receipt.tokenizer_digest",
        )?;
        ensure_optional_ref(
            self.support_artifact_kind.requires_tokenized_dataset(),
            self.tokenized_dataset_digest.as_deref(),
            catalog.tokenized_dataset_digest.as_str(),
            "receipt.tokenized_dataset_digest",
        )?;
        ensure_optional_ref(
            self.support_artifact_kind.requires_validation_set(),
            self.validation_set_digest.as_deref(),
            catalog.validation_set_digest.as_str(),
            "receipt.validation_set_digest",
        )?;
        ensure_optional_ref(
            self.support_artifact_kind.requires_base_checkpoint(),
            self.base_checkpoint_ref.as_deref(),
            catalog.base_checkpoint_ref.as_str(),
            "receipt.base_checkpoint_ref",
        )?;
        validate_artifact_refs(self.input_refs.as_slice(), "receipt.input_refs")?;
        validate_artifact_refs(self.output_refs.as_slice(), "receipt.output_refs")?;
        if self.required_validator_checks != family.validator_acceptance_checks {
            return invalid(format!(
                "receipt `{}` validator checks drifted from family `{}`",
                self.receipt_id,
                family.support_artifact_kind.label()
            ));
        }
        if self.validator_disposition != TrainingExecutionValidatorDisposition::Accepted {
            return invalid(format!(
                "support receipt `{}` must stay accepted in the retained examples",
                self.receipt_id
            ));
        }
        ensure_nonempty(
            self.validator_verdict_binding.as_str(),
            "receipt.validator_verdict_binding",
        )?;
        ensure_nonempty(
            self.closeout_verdict_binding.as_str(),
            "receipt.closeout_verdict_binding",
        )?;
        if !self.participant_eligible || !self.accepted_participant_work {
            return invalid(format!(
                "support receipt `{}` must count as accepted participant work",
                self.receipt_id
            ));
        }
        if self.model_progress_participant {
            return invalid(format!(
                "support receipt `{}` must not claim model-progress participant credit",
                self.receipt_id
            ));
        }
        ensure_eq(
            self.participant_counter_source.as_str(),
            catalog.participant_counter_source.as_str(),
            "receipt.participant_counter_source",
        )?;
        ensure_eq(
            self.model_progress_counter_source.as_str(),
            catalog.model_progress_counter_source.as_str(),
            "receipt.model_progress_counter_source",
        )?;
        ensure_nonempty(self.claim_boundary.as_str(), "receipt.claim_boundary")?;
        if !self.claim_boundary.contains("participant")
            || !self.claim_boundary.contains("not model-progress")
        {
            return invalid(format!(
                "support receipt `{}` claim boundary must distinguish participant from model-progress participant",
                self.receipt_id
            ));
        }
        ensure_nonempty(self.detail.as_str(), "receipt.detail")?;
        ensure_sha256_uri(self.receipt_digest.as_str(), "receipt.receipt_digest")?;
        if self.receipt_digest != self.stable_digest() {
            return invalid(format!(
                "support receipt `{}` digest drifted from canonical contents",
                self.receipt_id
            ));
        }
        Ok(())
    }
}

#[must_use]
pub fn canonical_a1_minimal_distributed_lm_support_artifact_catalog(
) -> A1MinimalDistributedLmSupportArtifactCatalog {
    let tokenizer_digest = expected_tokenizer_digest()
        .expect("canonical A1 minimal distributed LM tokenizer digest should resolve");
    let tokenized_dataset_digest = expected_tokenized_dataset_digest()
        .expect("canonical A1 minimal distributed LM tokenized dataset digest should resolve");
    let validation_set_digest = expected_validation_set_digest()
        .expect("canonical A1 minimal distributed LM validation set digest should resolve");
    let base_checkpoint_ref = expected_base_checkpoint_ref()
        .expect("canonical A1 minimal distributed LM base checkpoint should resolve");
    let support_families: Vec<_> = expected_support_artifact_kinds()
        .into_iter()
        .map(canonical_support_family)
        .collect();
    let mut retained_example_receipts = Vec::new();
    for (index, family) in support_families.iter().enumerate() {
        retained_example_receipts.push(canonical_support_receipt(
            family,
            index,
            tokenizer_digest.as_str(),
            tokenized_dataset_digest.as_str(),
            validation_set_digest.as_str(),
            base_checkpoint_ref.as_str(),
        ));
    }
    let mut catalog = A1MinimalDistributedLmSupportArtifactCatalog {
        schema_version: String::from(
            A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_CATALOG_SCHEMA_VERSION,
        ),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        tokenizer_digest,
        tokenized_dataset_digest,
        validation_set_digest,
        base_checkpoint_ref,
        participant_counter_source: String::from("training_accepted_contributors"),
        model_progress_counter_source: String::from("training_model_progress_contributors"),
        support_families,
        retained_example_receipts,
        claim_boundary: String::from(
            "A1 minimal distributed LM support/verifier work can count as participants only after accepted Nexus closeout under one run id. It does not count as model-progress participants unless a later accepted local update directly enters aggregate and checkpoint promotion.",
        ),
        catalog_digest: String::new(),
    };
    catalog.catalog_digest = catalog.stable_digest();
    catalog
}

pub fn write_a1_minimal_distributed_lm_support_artifact_catalog(
    output_path: impl AsRef<Path>,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            A1MinimalDistributedLmSupportArtifactError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let catalog = canonical_a1_minimal_distributed_lm_support_artifact_catalog();
    catalog.validate()?;
    let mut bytes = serde_json::to_vec_pretty(&catalog)?;
    bytes.push(b'\n');
    fs::write(output_path, bytes).map_err(|error| {
        A1MinimalDistributedLmSupportArtifactError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn canonical_support_family(
    kind: A1MinimalDistributedLmSupportArtifactKind,
) -> A1MinimalDistributedLmSupportArtifactFamily {
    A1MinimalDistributedLmSupportArtifactFamily {
        support_artifact_kind: kind,
        schema_version: support_artifact_schema_version(kind),
        public_work_label: String::from(kind.public_label()),
        existing_psionic_work_class_mapping: String::from(kind.existing_psionic_work_class()),
        participant_eligible_on_acceptance: true,
        model_progress_participant_by_default: false,
        required_inputs: required_inputs(kind),
        required_outputs: required_outputs(kind),
        required_artifact_refs: required_artifact_refs(kind),
        validator_acceptance_checks: vec![
            String::from("assignment_binding_matches"),
            String::from("artifact_digest_matches_payload"),
            String::from("validator_verdict_explicit"),
            String::from("closeout_verdict_explicit"),
            String::from("participant_counter_is_training_accepted_contributors"),
            String::from("model_progress_counter_not_incremented"),
        ],
        rejection_checks: vec![
            String::from("stale_assignment"),
            String::from("digest_mismatch"),
            String::from("missing_artifact"),
            String::from("wrong_run_id"),
            String::from("checkpoint_or_dataset_mismatch"),
        ],
        closeout_counter_source: String::from("training_accepted_contributors"),
        detail: format!(
            "{} is accepted-work eligible for participant counting after Nexus closeout, but it carries zero model-progress participant weight unless it is later converted into accepted local-update aggregate input.",
            kind.public_label()
        ),
    }
}

fn canonical_support_receipt(
    family: &A1MinimalDistributedLmSupportArtifactFamily,
    index: usize,
    tokenizer_digest: &str,
    tokenized_dataset_digest: &str,
    validation_set_digest: &str,
    base_checkpoint_ref: &str,
) -> A1MinimalDistributedLmSupportArtifactReceipt {
    let kind = family.support_artifact_kind;
    let ordinal = index + 1;
    let mut receipt = A1MinimalDistributedLmSupportArtifactReceipt {
        schema_version: String::from(
            A1_MINIMAL_DISTRIBUTED_LM_SUPPORT_ARTIFACT_RECEIPT_SCHEMA_VERSION,
        ),
        receipt_id: format!("a1-minimal-support-{}-receipt-v1", kind.label()),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        run_id: String::from("a1_minimal_distributed_lm_001"),
        window_id: format!("support-window-{ordinal:03}"),
        assignment_id: format!("support-assignment-{ordinal:03}"),
        worker_id: format!("support-worker-{ordinal:03}"),
        pylon_provider_id: format!("pylon-provider-support-{ordinal:03}"),
        support_artifact_kind: kind,
        existing_psionic_work_class: family.existing_psionic_work_class_mapping.clone(),
        tokenizer_digest: kind.requires_tokenizer().then(|| String::from(tokenizer_digest)),
        tokenized_dataset_digest: kind
            .requires_tokenized_dataset()
            .then(|| String::from(tokenized_dataset_digest)),
        validation_set_digest: kind
            .requires_validation_set()
            .then(|| String::from(validation_set_digest)),
        base_checkpoint_ref: kind
            .requires_base_checkpoint()
            .then(|| String::from(base_checkpoint_ref)),
        input_refs: example_input_refs(kind),
        output_refs: example_output_refs(kind),
        required_validator_checks: family.validator_acceptance_checks.clone(),
        validator_disposition: TrainingExecutionValidatorDisposition::Accepted,
        validator_verdict_binding: format!("accepted_support_artifact:{}", kind.label()),
        closeout_verdict_binding: format!("nexus_closeout_accepted_support:{}", kind.label()),
        participant_eligible: true,
        accepted_participant_work: true,
        model_progress_participant: false,
        participant_counter_source: String::from("training_accepted_contributors"),
        model_progress_counter_source: String::from("training_model_progress_contributors"),
        claim_boundary: format!(
            "This accepted {} receipt can count as one participant for run a1_minimal_distributed_lm_001. It is not model-progress participant work and must not advance checkpoint lineage or training_model_progress_contributors by itself.",
            kind.public_label()
        ),
        detail: format!(
            "Retained schema example for {} support work: explicit inputs, outputs, digests, validator disposition, and Nexus closeout binding are sufficient for acceptance or rejection without reading logs.",
            kind.public_label()
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn support_artifact_schema_version(kind: A1MinimalDistributedLmSupportArtifactKind) -> String {
    format!(
        "psion.a1_minimal_distributed_lm.support_artifact.{}.v1",
        kind.label()
    )
}

fn required_inputs(kind: A1MinimalDistributedLmSupportArtifactKind) -> Vec<String> {
    match kind {
        A1MinimalDistributedLmSupportArtifactKind::TokenizedShardValidation => vec![
            "raw_shard_digest",
            "tokenized_shard_digest",
            "tokenizer_digest",
            "tokenized_dataset_digest",
        ],
        A1MinimalDistributedLmSupportArtifactKind::ValidationReplay => vec![
            "contribution_receipt_digest",
            "artifact_manifest_digest",
            "validator_challenge_id",
            "base_checkpoint_ref",
        ],
        A1MinimalDistributedLmSupportArtifactKind::CheckpointVerification => vec![
            "base_checkpoint_ref",
            "checkpoint_digest",
            "checkpoint_lineage",
        ],
        A1MinimalDistributedLmSupportArtifactKind::EvalBatch => vec![
            "validation_set_digest",
            "checkpoint_digest",
            "tokenizer_digest",
        ],
        A1MinimalDistributedLmSupportArtifactKind::ArtifactRematerialization => {
            vec!["artifact_digest", "materialization_recipe_digest"]
        }
        A1MinimalDistributedLmSupportArtifactKind::IndependentScoredTrainingWindow => vec![
            "tokenizer_digest",
            "tokenized_dataset_digest",
            "base_checkpoint_ref",
            "training_window_seed",
        ],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

fn required_outputs(kind: A1MinimalDistributedLmSupportArtifactKind) -> Vec<String> {
    match kind {
        A1MinimalDistributedLmSupportArtifactKind::TokenizedShardValidation => vec![
            "round_trip_report",
            "token_count_report",
            "validator_verdict",
        ],
        A1MinimalDistributedLmSupportArtifactKind::ValidationReplay => vec![
            "validator_score_receipt",
            "quality_drift_signal",
            "rollback_signal",
        ],
        A1MinimalDistributedLmSupportArtifactKind::CheckpointVerification => vec![
            "checkpoint_surface_report",
            "checkpoint_digest_verdict",
            "lineage_verdict",
        ],
        A1MinimalDistributedLmSupportArtifactKind::EvalBatch => {
            vec!["eval_loss", "eval_batch_digest", "metric_receipt"]
        }
        A1MinimalDistributedLmSupportArtifactKind::ArtifactRematerialization => {
            vec!["rematerialized_artifact_digest", "byte_equality_verdict"]
        }
        A1MinimalDistributedLmSupportArtifactKind::IndependentScoredTrainingWindow => {
            vec![
                "loss_before",
                "loss_after",
                "score_bps",
                "training_window_receipt",
            ]
        }
    }
    .into_iter()
    .map(String::from)
    .collect()
}

fn required_artifact_refs(kind: A1MinimalDistributedLmSupportArtifactKind) -> Vec<String> {
    let mut refs = vec![
        String::from("assignment_id"),
        String::from("run_id"),
        String::from("validator_verdict_binding"),
        String::from("closeout_verdict_binding"),
    ];
    if kind.requires_tokenizer() {
        refs.push(String::from("tokenizer_digest"));
    }
    if kind.requires_tokenized_dataset() {
        refs.push(String::from("tokenized_dataset_digest"));
    }
    if kind.requires_validation_set() {
        refs.push(String::from("validation_set_digest"));
    }
    if kind.requires_base_checkpoint() {
        refs.push(String::from("base_checkpoint_ref"));
    }
    refs
}

fn example_input_refs(
    kind: A1MinimalDistributedLmSupportArtifactKind,
) -> Vec<A1MinimalDistributedLmSupportArtifactRef> {
    required_inputs(kind)
        .into_iter()
        .map(|role| example_ref("input", kind, role.as_str()))
        .collect()
}

fn example_output_refs(
    kind: A1MinimalDistributedLmSupportArtifactKind,
) -> Vec<A1MinimalDistributedLmSupportArtifactRef> {
    required_outputs(kind)
        .into_iter()
        .map(|role| example_ref("output", kind, role.as_str()))
        .collect()
}

fn example_ref(
    direction: &str,
    kind: A1MinimalDistributedLmSupportArtifactKind,
    role: &str,
) -> A1MinimalDistributedLmSupportArtifactRef {
    A1MinimalDistributedLmSupportArtifactRef {
        artifact_role: String::from(role),
        artifact_ref: format!(
            "fixture://a1_minimal_distributed_lm/support/{}/{direction}/{role}",
            kind.label()
        ),
        artifact_digest: digest_for(&[kind.label(), direction, role]),
        detail: format!(
            "Retained {direction} reference `{role}` for the {} support artifact family.",
            kind.public_label()
        ),
    }
}

fn expected_support_artifact_kinds() -> BTreeSet<A1MinimalDistributedLmSupportArtifactKind> {
    [
        A1MinimalDistributedLmSupportArtifactKind::TokenizedShardValidation,
        A1MinimalDistributedLmSupportArtifactKind::ValidationReplay,
        A1MinimalDistributedLmSupportArtifactKind::CheckpointVerification,
        A1MinimalDistributedLmSupportArtifactKind::EvalBatch,
        A1MinimalDistributedLmSupportArtifactKind::ArtifactRematerialization,
        A1MinimalDistributedLmSupportArtifactKind::IndependentScoredTrainingWindow,
    ]
    .into_iter()
    .collect()
}

fn expected_tokenizer_digest() -> Result<String, A1MinimalDistributedLmSupportArtifactError> {
    canonical_a1_minimal_distributed_lm_tokenizer_digest().map_err(|error| {
        A1MinimalDistributedLmSupportArtifactError::Invalid {
            detail: format!("canonical tokenizer digest invalid: {error}"),
        }
    })
}

fn expected_tokenized_dataset_digest() -> Result<String, A1MinimalDistributedLmSupportArtifactError>
{
    canonical_a1_minimal_distributed_lm_tokenized_dataset_digest().map_err(|error| {
        A1MinimalDistributedLmSupportArtifactError::Invalid {
            detail: format!("canonical tokenized dataset digest invalid: {error}"),
        }
    })
}

fn expected_validation_set_digest() -> Result<String, A1MinimalDistributedLmSupportArtifactError> {
    canonical_a1_minimal_distributed_lm_validation_set_digest().map_err(|error| {
        A1MinimalDistributedLmSupportArtifactError::Invalid {
            detail: format!("canonical validation set digest invalid: {error}"),
        }
    })
}

fn expected_base_checkpoint_ref() -> Result<String, A1MinimalDistributedLmSupportArtifactError> {
    Ok(canonical_a1_minimal_distributed_lm_lane_contract()
        .checkpoint_family
        .base_checkpoint_ref)
}

fn validate_artifact_refs(
    refs: &[A1MinimalDistributedLmSupportArtifactRef],
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    if refs.is_empty() {
        return invalid(format!("field `{field}` must not be empty"));
    }
    for artifact_ref in refs {
        ensure_nonempty(
            artifact_ref.artifact_role.as_str(),
            format!("{field}[].artifact_role").as_str(),
        )?;
        ensure_nonempty(
            artifact_ref.artifact_ref.as_str(),
            format!("{field}[].artifact_ref").as_str(),
        )?;
        ensure_sha256_uri(
            artifact_ref.artifact_digest.as_str(),
            format!("{field}[].artifact_digest").as_str(),
        )?;
        ensure_nonempty(
            artifact_ref.detail.as_str(),
            format!("{field}[].detail").as_str(),
        )?;
    }
    Ok(())
}

fn ensure_kind_required_ref(
    required: bool,
    refs: &[String],
    ref_name: &str,
    kind: A1MinimalDistributedLmSupportArtifactKind,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    if required && !refs.iter().any(|candidate| candidate == ref_name) {
        return invalid(format!(
            "support family `{}` must carry required ref `{ref_name}`",
            kind.label()
        ));
    }
    Ok(())
}

fn ensure_optional_ref(
    required: bool,
    actual: Option<&str>,
    expected: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    match (required, actual) {
        (true, Some(actual)) => ensure_eq(actual, expected, field),
        (true, None) => invalid(format!("field `{field}` is required for this support kind")),
        (false, Some(actual)) => {
            ensure_nonempty(actual, field)?;
            Ok(())
        }
        (false, None) => Ok(()),
    }
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    if value.trim().is_empty() {
        return invalid(format!("field `{field}` must not be empty"));
    }
    Ok(())
}

fn ensure_vec_nonempty(
    values: &[String],
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return invalid(format!("field `{field}` must contain nonempty values"));
    }
    Ok(())
}

fn ensure_sha256_uri(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    ensure_nonempty(value, field)?;
    let Some(hex) = value.strip_prefix("sha256:") else {
        return invalid(format!("field `{field}` must use sha256:<hex> form"));
    };
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return invalid(format!(
            "field `{field}` must contain a 64-hex sha256 digest"
        ));
    }
    Ok(())
}

fn ensure_eq(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmSupportArtifactError> {
    if actual != expected {
        return invalid(format!(
            "field `{field}` must be `{expected}` but was `{actual}`"
        ));
    }
    Ok(())
}

fn digest_for(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_a1_minimal_distributed_lm_support_artifact_example|");
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("A1 minimal distributed LM support artifact payload should serialize"),
    );
    format!("sha256:{:x}", hasher.finalize())
}

fn invalid<T>(detail: String) -> Result<T, A1MinimalDistributedLmSupportArtifactError> {
    Err(A1MinimalDistributedLmSupportArtifactError::Invalid { detail })
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_a1_minimal_distributed_lm_support_artifact_catalog,
        A1MinimalDistributedLmSupportArtifactCatalog,
    };

    fn fixture_catalog() -> A1MinimalDistributedLmSupportArtifactCatalog {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/a1_minimal_distributed_lm/support_artifact_catalog_v1.json"
        ))
        .expect("A1 minimal distributed LM support artifact catalog should parse")
    }

    #[test]
    fn a1_minimal_distributed_lm_support_catalog_fixture_validates() {
        fixture_catalog()
            .validate()
            .expect("A1 minimal distributed LM support artifact catalog should validate");
    }

    #[test]
    fn a1_minimal_distributed_lm_support_catalog_matches_canonical() {
        assert_eq!(
            fixture_catalog(),
            canonical_a1_minimal_distributed_lm_support_artifact_catalog()
        );
    }

    #[test]
    fn a1_minimal_distributed_lm_support_receipts_count_as_participants_only() {
        let catalog = fixture_catalog();
        assert_eq!(catalog.retained_example_receipts.len(), 6);
        for receipt in &catalog.retained_example_receipts {
            assert!(receipt.participant_eligible);
            assert!(receipt.accepted_participant_work);
            assert!(!receipt.model_progress_participant);
            assert_eq!(
                receipt.participant_counter_source,
                "training_accepted_contributors"
            );
            assert_eq!(
                receipt.model_progress_counter_source,
                "training_model_progress_contributors"
            );
        }
    }

    #[test]
    fn a1_minimal_distributed_lm_support_receipt_rejects_model_progress_claim() {
        let catalog = fixture_catalog();
        let mut receipt = catalog.retained_example_receipts[0].clone();
        receipt.model_progress_participant = true;
        receipt.receipt_digest = receipt.stable_digest();
        let family = catalog
            .support_families
            .iter()
            .find(|candidate| candidate.support_artifact_kind == receipt.support_artifact_kind)
            .expect("receipt should have a family");
        receipt
            .validate_against_catalog(&catalog, family)
            .expect_err("support receipt must reject model-progress participant credit");
    }
}
