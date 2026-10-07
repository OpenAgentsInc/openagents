use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_data::{PsionTokenizedCorpusManifest, PsionTokenizerArtifactBundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_public_work_assignment_contract, cross_provider_training_program_manifest,
    CrossProviderTrainingProgramManifestError, PublicWorkAssignmentContractError,
    PublicWorkAssignmentKind,
};

pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.public_dataset_authority_contract.v1";
pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_ID: &str =
    "psionic.public_dataset_authority_contract.v1";
pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/public_dataset_authority_contract_v1.json";
pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_CHECK_SCRIPT_PATH: &str =
    "scripts/check-public-dataset-authority-contract.sh";
pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_DOC_PATH: &str =
    "docs/PUBLIC_DATASET_AUTHORITY_REFERENCE.md";
pub const PUBLIC_DATASET_AUTHORITY_CONTRACT_TRAIN_SYSTEM_DOC_PATH: &str = "docs/TRAIN_SYSTEM.md";

const TOKENIZER_ARTIFACT_BUNDLE_FIXTURE_PATH: &str =
    "../../../fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json";
const TOKENIZED_CORPUS_FIXTURE_PATH: &str =
    "../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json";
const TOKENIZER_ARTIFACT_BUNDLE_FIXTURE_JSON: &str =
    include_str!("../../../fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json");
const TOKENIZED_CORPUS_FIXTURE_JSON: &str =
    include_str!("../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json");

#[derive(Debug, Error)]
pub enum PublicDatasetAuthorityContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    PublicWork(#[from] PublicWorkAssignmentContractError),
    #[error("failed to parse fixture `{fixture}`: {error}")]
    FixtureDeserialize {
        fixture: &'static str,
        error: serde_json::Error,
    },
    #[error("public dataset authority contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicDataReceiptDisposition {
    Admitted,
    RefusedDuplicate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicDatasetPage {
    pub page_id: String,
    pub shard_id: String,
    pub split_name: String,
    pub sequence_start_index: u32,
    pub sequence_end_index: u32,
    pub page_token_count: u32,
    pub detail: String,
}

impl PublicDatasetPage {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psionic_public_dataset_page|", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicDatasetPageProof {
    pub proof_id: String,
    pub page_id: String,
    pub shard_digest: String,
    pub source_lineage_digest: String,
    pub tokenizer_digest: String,
    pub packing_policy_digest: String,
    pub replay_identity: String,
    pub page_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicAntiReplayDataReceipt {
    pub receipt_id: String,
    pub assignment_id: String,
    pub page_id: String,
    pub claim_fingerprint: String,
    pub disposition: PublicDataReceiptDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior_receipt_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicDatasetAuthorityPaths {
    pub fixture_path: String,
    pub check_script_path: String,
    pub reference_doc_path: String,
    pub train_system_doc_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicDatasetAuthorityContract {
    pub schema_version: String,
    pub contract_id: String,
    pub dataset_family_id: String,
    pub program_manifest_digest: String,
    pub public_work_assignment_contract_digest: String,
    pub tokenized_corpus_dataset_id: String,
    pub tokenized_corpus_dataset_version: String,
    pub tokenizer_bundle_schema_version: String,
    pub tokenizer_id: String,
    pub tokenizer_version: String,
    pub tokenizer_digest: String,
    pub tokenizer_config_digest: String,
    pub replay_identity: String,
    pub packing_policy_digest: String,
    pub dataset_pages: Vec<PublicDatasetPage>,
    pub page_proofs: Vec<PublicDatasetPageProof>,
    pub anti_replay_receipts: Vec<PublicAntiReplayDataReceipt>,
    pub authority_paths: PublicDatasetAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl PublicDatasetAuthorityContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_public_dataset_authority_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), PublicDatasetAuthorityContractError> {
        let manifest = cross_provider_training_program_manifest()?;
        let public_work = canonical_public_work_assignment_contract()?;
        let tokenizer_bundle = canonical_psion_tokenizer_artifact_bundle()?;
        let tokenized_corpus = canonical_psion_tokenized_corpus_manifest()?;

        let page_by_id = self
            .dataset_pages
            .iter()
            .map(|page| (page.page_id.as_str(), page))
            .collect::<BTreeMap<_, _>>();
        let assignment_by_id = public_work
            .assignments
            .iter()
            .map(|assignment| (assignment.assignment_id.as_str(), assignment))
            .collect::<BTreeMap<_, _>>();
        let shard_by_id = tokenized_corpus
            .shards
            .iter()
            .map(|shard| (shard.shard_id.as_str(), shard))
            .collect::<BTreeMap<_, _>>();
        let expected_page_ids = public_work
            .assignments
            .iter()
            .map(|assignment| assignment.dataset_page_selector.as_str())
            .collect::<BTreeSet<_>>();

        if self.schema_version != PUBLIC_DATASET_AUTHORITY_CONTRACT_SCHEMA_VERSION {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    PUBLIC_DATASET_AUTHORITY_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != PUBLIC_DATASET_AUTHORITY_CONTRACT_ID {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.dataset_family_id != manifest.dataset_family_id
            || self.program_manifest_digest != manifest.program_manifest_digest
            || self.public_work_assignment_contract_digest != public_work.contract_digest
        {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("program or public-work binding drifted"),
            });
        }
        if self.tokenized_corpus_dataset_id != tokenized_corpus.dataset_id
            || self.tokenized_corpus_dataset_version != tokenized_corpus.dataset_version
            || self.tokenizer_bundle_schema_version != tokenizer_bundle.schema_version
            || self.tokenizer_id != tokenizer_bundle.tokenizer_id
            || self.tokenizer_version != tokenizer_bundle.tokenizer_version
            || self.tokenizer_digest != tokenizer_bundle.tokenizer.tokenizer_digest
            || self.tokenizer_config_digest != tokenizer_bundle.tokenizer_config_digest
            || self.replay_identity != tokenized_corpus.replay_contract.stable_dataset_identity
            || self.packing_policy_digest
                != stable_digest(
                    b"psionic_xtrain_dataset_packing_policy|",
                    &tokenized_corpus.packing_policy,
                )
        {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("tokenizer or tokenized corpus binding drifted"),
            });
        }
        if self.authority_paths.fixture_path != PUBLIC_DATASET_AUTHORITY_CONTRACT_FIXTURE_PATH
            || self.authority_paths.check_script_path
                != PUBLIC_DATASET_AUTHORITY_CONTRACT_CHECK_SCRIPT_PATH
            || self.authority_paths.reference_doc_path != PUBLIC_DATASET_AUTHORITY_CONTRACT_DOC_PATH
            || self.authority_paths.train_system_doc_path
                != PUBLIC_DATASET_AUTHORITY_CONTRACT_TRAIN_SYSTEM_DOC_PATH
        {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }

        let actual_page_ids = self
            .dataset_pages
            .iter()
            .map(|page| page.page_id.as_str())
            .collect::<BTreeSet<_>>();
        if actual_page_ids != expected_page_ids {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("dataset page set drifted from public assignments"),
            });
        }

        let mut page_ids = BTreeSet::new();
        for page in &self.dataset_pages {
            if !page_ids.insert(page.page_id.as_str()) {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!("duplicate page `{}`", page.page_id),
                });
            }
            let shard = shard_by_id.get(page.shard_id.as_str()).ok_or_else(|| {
                PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "page `{}` references unknown shard `{}`",
                        page.page_id, page.shard_id
                    ),
                }
            })?;
            if shard.split_name != page.split_name
                || page.sequence_start_index >= page.sequence_end_index
                || page.sequence_end_index > shard.sequence_count as u32
                || page.page_token_count == 0
            {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!("page `{}` drifted from tokenized shard truth", page.page_id),
                });
            }
        }

        let mut proof_ids = BTreeSet::new();
        for proof in &self.page_proofs {
            if !proof_ids.insert(proof.proof_id.as_str()) {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!("duplicate page proof `{}`", proof.proof_id),
                });
            }
            let page = page_by_id.get(proof.page_id.as_str()).ok_or_else(|| {
                PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "page proof `{}` references unknown page `{}`",
                        proof.proof_id, proof.page_id
                    ),
                }
            })?;
            let shard = shard_by_id.get(page.shard_id.as_str()).ok_or_else(|| {
                PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "page proof `{}` lost shard `{}`",
                        proof.proof_id, page.shard_id
                    ),
                }
            })?;
            let expected_source_lineage_digest = stable_digest(
                b"psionic_xtrain_dataset_source_lineage|",
                &shard.source_lineage,
            );
            if proof.shard_digest != shard.shard_digest
                || proof.source_lineage_digest != expected_source_lineage_digest
                || proof.tokenizer_digest != self.tokenizer_digest
                || proof.packing_policy_digest != self.packing_policy_digest
                || proof.replay_identity != self.replay_identity
                || proof.page_digest != page.stable_digest()
            {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "page proof `{}` lost shard, tokenizer, packing, replay, or page binding",
                        proof.proof_id
                    ),
                });
            }
        }
        if self.page_proofs.len() != self.dataset_pages.len() {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("every dataset page must retain one page proof"),
            });
        }

        let mut receipt_ids = BTreeSet::new();
        let mut admitted_receipts = 0_u16;
        let mut refused_duplicate_receipts = 0_u16;
        for receipt in &self.anti_replay_receipts {
            if !receipt_ids.insert(receipt.receipt_id.as_str()) {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!("duplicate anti-replay receipt `{}`", receipt.receipt_id),
                });
            }
            let assignment = assignment_by_id
                .get(receipt.assignment_id.as_str())
                .ok_or_else(|| PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "anti-replay receipt `{}` references unknown assignment `{}`",
                        receipt.receipt_id, receipt.assignment_id
                    ),
                })?;
            if assignment.dataset_page_selector != receipt.page_id
                || !page_by_id.contains_key(receipt.page_id.as_str())
                || receipt.claim_fingerprint.is_empty()
            {
                return Err(PublicDatasetAuthorityContractError::InvalidContract {
                    detail: format!(
                        "anti-replay receipt `{}` drifted from assignment/page truth",
                        receipt.receipt_id
                    ),
                });
            }
            match receipt.disposition {
                PublicDataReceiptDisposition::Admitted => {
                    admitted_receipts += 1;
                    if receipt.prior_receipt_id.is_some()
                        || assignment.assignment_kind != PublicWorkAssignmentKind::PublicMinerTrain
                    {
                        return Err(PublicDatasetAuthorityContractError::InvalidContract {
                            detail: format!(
                                "admitted anti-replay receipt `{}` drifted",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
                PublicDataReceiptDisposition::RefusedDuplicate => {
                    refused_duplicate_receipts += 1;
                    let prior_receipt_id =
                        receipt.prior_receipt_id.as_deref().ok_or_else(|| {
                            PublicDatasetAuthorityContractError::InvalidContract {
                                detail: format!(
                                    "duplicate refusal `{}` lost prior receipt binding",
                                    receipt.receipt_id
                                ),
                            }
                        })?;
                    let prior_receipt = self
                        .anti_replay_receipts
                        .iter()
                        .find(|candidate| candidate.receipt_id == prior_receipt_id)
                        .ok_or_else(|| PublicDatasetAuthorityContractError::InvalidContract {
                            detail: format!(
                                "duplicate refusal `{}` references unknown prior receipt `{}`",
                                receipt.receipt_id, prior_receipt_id
                            ),
                        })?;
                    if prior_receipt.page_id != receipt.page_id
                        || prior_receipt.claim_fingerprint != receipt.claim_fingerprint
                    {
                        return Err(PublicDatasetAuthorityContractError::InvalidContract {
                            detail: format!(
                                "duplicate refusal `{}` lost duplicate-work binding",
                                receipt.receipt_id
                            ),
                        });
                    }
                }
            }
        }
        if admitted_receipts != 4 || refused_duplicate_receipts != 1 {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from(
                    "expected four admitted miner receipts and one refused duplicate receipt",
                ),
            });
        }

        if self.contract_digest != self.stable_digest() {
            return Err(PublicDatasetAuthorityContractError::InvalidContract {
                detail: String::from("contract_digest does not match the stable digest"),
            });
        }

        Ok(())
    }
}

static PUBLIC_DATASET_AUTHORITY_CONTRACT_CACHE: std::sync::OnceLock<
    PublicDatasetAuthorityContract,
> = std::sync::OnceLock::new();

pub fn canonical_public_dataset_authority_contract(
) -> Result<PublicDatasetAuthorityContract, PublicDatasetAuthorityContractError> {
    if let Some(contract) = PUBLIC_DATASET_AUTHORITY_CONTRACT_CACHE.get() {
        return Ok(contract.clone());
    }
    let manifest = cross_provider_training_program_manifest()?;
    let public_work = canonical_public_work_assignment_contract()?;
    let tokenizer_bundle = canonical_psion_tokenizer_artifact_bundle()?;
    let tokenized_corpus = canonical_psion_tokenized_corpus_manifest()?;

    let packing_policy_digest = stable_digest(
        b"psionic_xtrain_dataset_packing_policy|",
        &tokenized_corpus.packing_policy,
    );
    let train_shard = tokenized_corpus
        .shards
        .iter()
        .find(|shard| shard.shard_id == "psion_train_shard_0001")
        .expect("canonical tokenized corpus must retain psion_train_shard_0001");
    let validation_shard = tokenized_corpus
        .shards
        .iter()
        .find(|shard| shard.shard_id == "psion_validation_shard_0001")
        .expect("canonical tokenized corpus must retain psion_validation_shard_0001");

    let dataset_pages = vec![
        dataset_page("dataset.page.train.0001_0004", "psion_train_shard_0001", "train", 0, 4, 15_360, "The first public miner page covers the first four train-sequence slots in the canonical tokenized corpus."),
        dataset_page("dataset.page.train.0005_0008", "psion_train_shard_0001", "train", 4, 8, 15_360, "The second public miner page covers the next four train-sequence slots in the canonical tokenized corpus."),
        dataset_page("dataset.page.validation.challenge.0001", "psion_validation_shard_0001", "validation", 0, 4, 16_384, "The first validator challenge page covers the first four validation sequences."),
        dataset_page("dataset.page.validation.challenge.0002", "psion_validation_shard_0001", "validation", 4, 8, 16_384, "The second validator challenge page covers the next four validation sequences."),
        dataset_page("dataset.page.train.0009_0012", "psion_train_shard_0001", "train", 8, 12, 15_360, "The third public miner page covers train-sequence slots eight through eleven."),
        dataset_page("dataset.page.train.0013_0016", "psion_train_shard_0001", "train", 12, 16, 15_360, "The fourth public miner page covers train-sequence slots twelve through fifteen."),
        dataset_page("dataset.page.validation.challenge.0003", "psion_validation_shard_0001", "validation", 8, 12, 16_384, "The third validator challenge page covers validation-sequence slots eight through eleven."),
        dataset_page("dataset.page.validation.challenge.0004", "psion_validation_shard_0001", "validation", 12, 16, 16_384, "The fourth validator challenge page covers validation-sequence slots twelve through fifteen."),
    ];

    let page_proofs = dataset_pages
        .iter()
        .map(|page| {
            let shard = match page.shard_id.as_str() {
                "psion_train_shard_0001" => train_shard,
                "psion_validation_shard_0001" => validation_shard,
                _ => unreachable!("canonical dataset page references an unknown shard"),
            };
            PublicDatasetPageProof {
                proof_id: format!("proof.{}", page.page_id),
                page_id: page.page_id.clone(),
                shard_digest: shard.shard_digest.clone(),
                source_lineage_digest: stable_digest(
                    b"psionic_xtrain_dataset_source_lineage|",
                    &shard.source_lineage,
                ),
                tokenizer_digest: tokenizer_bundle.tokenizer.tokenizer_digest.clone(),
                packing_policy_digest: packing_policy_digest.clone(),
                replay_identity: tokenized_corpus
                    .replay_contract
                    .stable_dataset_identity
                    .clone(),
                page_digest: page.stable_digest(),
                detail: format!(
                    "The network retains the page proof for `{}` against shard `{}`.",
                    page.page_id, page.shard_id
                ),
            }
        })
        .collect::<Vec<_>>();

    let anti_replay_receipts = vec![
        anti_replay_receipt(
            "anti_replay.assignment.public_miner.window1230.google",
            "assignment.public_miner.window1230.google",
            "dataset.page.train.0001_0004",
            "claim.public_miner.window1230.google.v1",
            PublicDataReceiptDisposition::Admitted,
            None,
            "The first Google miner claim is admitted against its canonical page proof.",
        ),
        anti_replay_receipt(
            "anti_replay.assignment.public_miner.window1230.local_mlx",
            "assignment.public_miner.window1230.local_mlx",
            "dataset.page.train.0005_0008",
            "claim.public_miner.window1230.local_mlx.v1",
            PublicDataReceiptDisposition::Admitted,
            None,
            "The first Apple MLX miner claim is admitted against its canonical page proof.",
        ),
        anti_replay_receipt(
            "anti_replay.assignment.public_miner.window1231.google",
            "assignment.public_miner.window1231.google",
            "dataset.page.train.0009_0012",
            "claim.public_miner.window1231.google.v1",
            PublicDataReceiptDisposition::Admitted,
            None,
            "The second Google miner claim is admitted against its canonical page proof.",
        ),
        anti_replay_receipt(
            "anti_replay.assignment.public_miner.window1231.local_mlx",
            "assignment.public_miner.window1231.local_mlx",
            "dataset.page.train.0013_0016",
            "claim.public_miner.window1231.local_mlx.v1",
            PublicDataReceiptDisposition::Admitted,
            None,
            "The second Apple MLX miner claim is admitted against its canonical page proof.",
        ),
        anti_replay_receipt(
            "anti_replay.assignment.public_miner.window1230.google.duplicate",
            "assignment.public_miner.window1230.google",
            "dataset.page.train.0001_0004",
            "claim.public_miner.window1230.google.v1",
            PublicDataReceiptDisposition::RefusedDuplicate,
            Some("anti_replay.assignment.public_miner.window1230.google"),
            "A replayed Google miner claim with the same fingerprint is refused as duplicate work rather than being treated as fresh contribution.",
        ),
    ];

    let mut contract = PublicDatasetAuthorityContract {
        schema_version: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_ID),
        dataset_family_id: manifest.dataset_family_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        public_work_assignment_contract_digest: public_work.contract_digest.clone(),
        tokenized_corpus_dataset_id: tokenized_corpus.dataset_id.clone(),
        tokenized_corpus_dataset_version: tokenized_corpus.dataset_version.clone(),
        tokenizer_bundle_schema_version: tokenizer_bundle.schema_version.clone(),
        tokenizer_id: tokenizer_bundle.tokenizer_id.clone(),
        tokenizer_version: tokenizer_bundle.tokenizer_version.clone(),
        tokenizer_digest: tokenizer_bundle.tokenizer.tokenizer_digest.clone(),
        tokenizer_config_digest: tokenizer_bundle.tokenizer_config_digest.clone(),
        replay_identity: tokenized_corpus
            .replay_contract
            .stable_dataset_identity
            .clone(),
        packing_policy_digest,
        dataset_pages,
        page_proofs,
        anti_replay_receipts,
        authority_paths: PublicDatasetAuthorityPaths {
            fixture_path: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_FIXTURE_PATH),
            check_script_path: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_CHECK_SCRIPT_PATH),
            reference_doc_path: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_DOC_PATH),
            train_system_doc_path: String::from(PUBLIC_DATASET_AUTHORITY_CONTRACT_TRAIN_SYSTEM_DOC_PATH),
        },
        claim_boundary: String::from(
            "This contract freezes the first public data-truth surface above deterministic public work: page definitions, page proofs, tokenizer and packing digests, replay identity, admitted miner data receipts, and one refused duplicate claim. It does not yet claim content-addressed artifact exchange or the full public miner execution protocol.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    let _ = PUBLIC_DATASET_AUTHORITY_CONTRACT_CACHE.set(contract.clone());
    Ok(contract)
}

pub fn write_public_dataset_authority_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), PublicDatasetAuthorityContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PublicDatasetAuthorityContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_public_dataset_authority_contract()?;
    let bytes = serde_json::to_vec_pretty(&contract)?;
    fs::write(output_path, bytes).map_err(|error| PublicDatasetAuthorityContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn dataset_page(
    page_id: &str,
    shard_id: &str,
    split_name: &str,
    sequence_start_index: u32,
    sequence_end_index: u32,
    page_token_count: u32,
    detail: &str,
) -> PublicDatasetPage {
    PublicDatasetPage {
        page_id: String::from(page_id),
        shard_id: String::from(shard_id),
        split_name: String::from(split_name),
        sequence_start_index,
        sequence_end_index,
        page_token_count,
        detail: String::from(detail),
    }
}

fn anti_replay_receipt(
    receipt_id: &str,
    assignment_id: &str,
    page_id: &str,
    claim_fingerprint: &str,
    disposition: PublicDataReceiptDisposition,
    prior_receipt_id: Option<&str>,
    detail: &str,
) -> PublicAntiReplayDataReceipt {
    PublicAntiReplayDataReceipt {
        receipt_id: String::from(receipt_id),
        assignment_id: String::from(assignment_id),
        page_id: String::from(page_id),
        claim_fingerprint: String::from(claim_fingerprint),
        disposition,
        prior_receipt_id: prior_receipt_id.map(String::from),
        detail: String::from(detail),
    }
}

fn canonical_psion_tokenizer_artifact_bundle(
) -> Result<PsionTokenizerArtifactBundle, PublicDatasetAuthorityContractError> {
    serde_json::from_str(TOKENIZER_ARTIFACT_BUNDLE_FIXTURE_JSON).map_err(|error| {
        PublicDatasetAuthorityContractError::FixtureDeserialize {
            fixture: TOKENIZER_ARTIFACT_BUNDLE_FIXTURE_PATH,
            error,
        }
    })
}

fn canonical_psion_tokenized_corpus_manifest(
) -> Result<PsionTokenizedCorpusManifest, PublicDatasetAuthorityContractError> {
    serde_json::from_str(TOKENIZED_CORPUS_FIXTURE_JSON).map_err(|error| {
        PublicDatasetAuthorityContractError::FixtureDeserialize {
            fixture: TOKENIZED_CORPUS_FIXTURE_PATH,
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect(
            "stable digest serialization must succeed for public dataset authority contract",
        ),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_public_dataset_authority_contract, PublicDataReceiptDisposition,
        PublicDatasetAuthorityContractError,
    };

    #[test]
    fn canonical_public_dataset_authority_contract_is_valid(
    ) -> Result<(), PublicDatasetAuthorityContractError> {
        let contract = canonical_public_dataset_authority_contract()?;
        contract.validate()
    }

    #[test]
    fn duplicate_receipt_must_retain_duplicate_binding(
    ) -> Result<(), PublicDatasetAuthorityContractError> {
        let mut contract = canonical_public_dataset_authority_contract()?;
        let duplicate = contract
            .anti_replay_receipts
            .iter_mut()
            .find(|receipt| {
                receipt.receipt_id
                    == "anti_replay.assignment.public_miner.window1230.google.duplicate"
            })
            .expect("duplicate anti-replay receipt must exist");
        duplicate.disposition = PublicDataReceiptDisposition::Admitted;
        duplicate.prior_receipt_id = None;
        let error = contract
            .validate()
            .expect_err("duplicate anti-replay receipt cannot flip to admitted");
        assert!(matches!(
            error,
            PublicDatasetAuthorityContractError::InvalidContract { .. }
        ));
        Ok(())
    }
}
