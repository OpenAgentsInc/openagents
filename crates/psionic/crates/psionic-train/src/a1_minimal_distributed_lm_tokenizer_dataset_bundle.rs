use std::{fs, path::Path};

use psionic_data::{
    train_cs336_a1_byte_pair_encoding_from_text, Cs336A1BytePairEncodingArtifacts,
    Cs336A1BytePairEncodingError,
};
use psionic_models::{
    Cs336A1BytePairTokenizer, Cs336A1BytePairTokenizerError, TokenId, TokenizerBoundary,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::A1_MINIMAL_DISTRIBUTED_LM_LANE_ID;

pub const A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.tokenizer_dataset_bundle.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/psion/tokenized/a1_minimal_distributed_lm_tokenizer_dataset_bundle_v1.json";
pub const A1_MINIMAL_DISTRIBUTED_LM_CORPUS_FIXTURE_PATH: &str =
    "fixtures/training/a1_minimal_distributed_lm_corpus.txt";
pub const A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_REQUESTED_VOCAB_SIZE: u32 = 272;

#[derive(Debug, Error)]
pub enum A1MinimalDistributedLmTokenizerDatasetBundleError {
    #[error("A1 minimal distributed LM tokenizer/dataset bundle is invalid: {detail}")]
    InvalidBundle { detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Bpe(#[from] Cs336A1BytePairEncodingError),
    #[error(transparent)]
    Tokenizer(#[from] Cs336A1BytePairTokenizerError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmCorpusSource {
    pub source_id: String,
    pub source_family_id: String,
    pub source_kind: String,
    pub corpus_path: String,
    pub corpus_digest: String,
    pub byte_count: usize,
    pub line_count: usize,
    pub license_or_origin: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmTokenizedShard {
    pub shard_id: String,
    pub split_name: String,
    pub split_kind: String,
    pub storage_ref: String,
    pub source_shard_digest: String,
    pub tokenizer_digest: String,
    pub source_sample_count: usize,
    pub token_count: usize,
    pub sample_token_counts: Vec<usize>,
    pub min_sample_tokens: usize,
    pub max_sample_tokens: usize,
    pub tokens: Vec<u32>,
    pub sample_text_digests: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmReplaySample {
    pub sample_id: String,
    pub split_name: String,
    pub raw_text: String,
    pub encoded_tokens: Vec<u32>,
    pub decoded_text: String,
    pub round_trip_matches: bool,
    pub sample_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmTokenizerDatasetBundle {
    pub schema_version: String,
    pub lane_id: String,
    pub tokenizer_vocab_size_requested: u32,
    pub corpus_source: A1MinimalDistributedLmCorpusSource,
    pub tokenizer_artifacts: Cs336A1BytePairEncodingArtifacts,
    pub tokenizer_digest: String,
    pub training_shards: Vec<A1MinimalDistributedLmTokenizedShard>,
    pub validation_shards: Vec<A1MinimalDistributedLmTokenizedShard>,
    pub training_dataset_digest: String,
    pub validation_dataset_digest: String,
    pub bundle_digest: String,
    pub replay_samples: Vec<A1MinimalDistributedLmReplaySample>,
    pub claim_boundary: String,
}

impl A1MinimalDistributedLmTokenizerDatasetBundle {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_tokenizer_dataset_bundle|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
        if self.schema_version != A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_SCHEMA_VERSION
        {
            return invalid_bundle(format!(
                "schema_version must stay `{}` but was `{}`",
                A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_SCHEMA_VERSION,
                self.schema_version
            ));
        }
        if self.lane_id != A1_MINIMAL_DISTRIBUTED_LM_LANE_ID {
            return invalid_bundle(format!(
                "lane_id must stay `{}` but was `{}`",
                A1_MINIMAL_DISTRIBUTED_LM_LANE_ID, self.lane_id
            ));
        }
        if self.tokenizer_vocab_size_requested
            != A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_REQUESTED_VOCAB_SIZE
        {
            return invalid_bundle(String::from("tokenizer requested vocab size drifted"));
        }
        if self.tokenizer_artifacts.vocab_size != self.tokenizer_vocab_size_requested {
            return invalid_bundle(String::from(
                "tokenizer artifact vocab size must match requested vocab size",
            ));
        }
        let expected_tokenizer_digest = tokenizer_digest_from_artifacts(&self.tokenizer_artifacts);
        if self.tokenizer_digest != expected_tokenizer_digest {
            return invalid_bundle(String::from(
                "tokenizer_digest must match tokenizer artifacts",
            ));
        }
        self.corpus_source.validate()?;
        validate_shards(
            self.training_shards.as_slice(),
            "training",
            self.tokenizer_digest.as_str(),
        )?;
        validate_shards(
            self.validation_shards.as_slice(),
            "validation",
            self.tokenizer_digest.as_str(),
        )?;
        if self.training_dataset_digest
            != tokenized_dataset_digest("training", self.training_shards.as_slice())
        {
            return invalid_bundle(String::from("training_dataset_digest drifted"));
        }
        if self.validation_dataset_digest
            != tokenized_dataset_digest("validation", self.validation_shards.as_slice())
        {
            return invalid_bundle(String::from("validation_dataset_digest drifted"));
        }
        ensure_sha256_uri(self.bundle_digest.as_str(), "bundle_digest")?;
        if self.bundle_digest != self.stable_digest() {
            return invalid_bundle(String::from("bundle_digest does not match stable digest"));
        }
        validate_replay_samples(self)?;
        if !self.claim_boundary.contains("not distributed BPE")
            || !self.claim_boundary.contains("not OpenWebText leaderboard")
        {
            return invalid_bundle(String::from(
                "claim_boundary must exclude distributed BPE and OpenWebText leaderboard claims",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmCorpusSource {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
        ensure_nonempty(self.source_id.as_str(), "corpus_source.source_id")?;
        ensure_nonempty(
            self.source_family_id.as_str(),
            "corpus_source.source_family_id",
        )?;
        ensure_nonempty(self.source_kind.as_str(), "corpus_source.source_kind")?;
        ensure_nonempty(self.corpus_path.as_str(), "corpus_source.corpus_path")?;
        ensure_sha256_uri(self.corpus_digest.as_str(), "corpus_source.corpus_digest")?;
        ensure_nonempty(
            self.license_or_origin.as_str(),
            "corpus_source.license_or_origin",
        )?;
        if self.byte_count == 0 || self.line_count == 0 {
            return invalid_bundle(String::from("corpus source must be nonempty"));
        }
        Ok(())
    }
}

#[must_use]
pub fn canonical_a1_minimal_distributed_lm_corpus_text() -> String {
    canonical_a1_minimal_distributed_lm_samples()
        .iter()
        .map(|sample| sample.text)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle() -> Result<
    A1MinimalDistributedLmTokenizerDatasetBundle,
    A1MinimalDistributedLmTokenizerDatasetBundleError,
> {
    let corpus_text = canonical_a1_minimal_distributed_lm_corpus_text();
    let samples = canonical_a1_minimal_distributed_lm_samples();
    let tokenizer_artifacts = train_cs336_a1_byte_pair_encoding_from_text(
        corpus_text.as_str(),
        A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_REQUESTED_VOCAB_SIZE,
        &[],
    )?;
    let tokenizer_digest = tokenizer_digest_from_artifacts(&tokenizer_artifacts);
    let tokenizer = tokenizer_from_artifacts(&tokenizer_artifacts)?;
    let training_samples = samples
        .iter()
        .copied()
        .filter(|sample| sample.split == "training")
        .collect::<Vec<_>>();
    let validation_samples = samples
        .iter()
        .copied()
        .filter(|sample| sample.split == "validation")
        .collect::<Vec<_>>();
    let training_shards = vec![tokenized_shard(
        "train-shard-0000",
        "training",
        "training",
        training_samples.as_slice(),
        tokenizer_digest.as_str(),
        &tokenizer,
    )?];
    let validation_shards = vec![tokenized_shard(
        "validation-shard-0000",
        "validation",
        "validation",
        validation_samples.as_slice(),
        tokenizer_digest.as_str(),
        &tokenizer,
    )?];
    let replay_samples = validation_samples
        .iter()
        .enumerate()
        .map(|(index, sample)| replay_sample(index, *sample, &tokenizer))
        .collect::<Result<Vec<_>, _>>()?;
    let training_dataset_digest = tokenized_dataset_digest("training", training_shards.as_slice());
    let validation_dataset_digest =
        tokenized_dataset_digest("validation", validation_shards.as_slice());
    let mut bundle = A1MinimalDistributedLmTokenizerDatasetBundle {
        schema_version: String::from(
            A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_SCHEMA_VERSION,
        ),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        tokenizer_vocab_size_requested: A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_REQUESTED_VOCAB_SIZE,
        corpus_source: A1MinimalDistributedLmCorpusSource {
            source_id: String::from("a1_minimal_distributed_lm_tiny_corpus_v1"),
            source_family_id: String::from("a1_minimal_distributed_lm_tiny_corpus"),
            source_kind: String::from("committed_fixture"),
            corpus_path: String::from(A1_MINIMAL_DISTRIBUTED_LM_CORPUS_FIXTURE_PATH),
            corpus_digest: sha256_uri_digest(
                b"psion_a1_minimal_distributed_lm_corpus_text|",
                &corpus_text,
            ),
            byte_count: corpus_text.len(),
            line_count: samples.len(),
            license_or_origin: String::from(
                "synthetic Psionic fixture text written for deterministic A1 minimal distributed LM tests",
            ),
        },
        tokenizer_artifacts,
        tokenizer_digest,
        training_shards,
        validation_shards,
        training_dataset_digest,
        validation_dataset_digest,
        bundle_digest: String::new(),
        replay_samples,
        claim_boundary: String::from(
            "This bundle freezes a tiny synthetic corpus, one CS336 A1 BPE tokenizer, one tokenized training shard, and one validation shard for a1_minimal_distributed_lm_001. It is not distributed BPE, not OpenWebText leaderboard training, and not broad pretraining.",
        ),
    };
    bundle.bundle_digest = bundle.stable_digest();
    bundle.validate()?;
    Ok(bundle)
}

pub fn canonical_a1_minimal_distributed_lm_tokenizer_digest(
) -> Result<String, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Ok(canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?.tokenizer_digest)
}

pub fn canonical_a1_minimal_distributed_lm_tokenized_dataset_digest(
) -> Result<String, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Ok(canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?.training_dataset_digest)
}

pub fn canonical_a1_minimal_distributed_lm_validation_set_digest(
) -> Result<String, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Ok(canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?.validation_dataset_digest)
}

pub fn canonical_a1_minimal_distributed_lm_tokenizer_vocab_size(
) -> Result<u32, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Ok(
        canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?
            .tokenizer_artifacts
            .vocab_size,
    )
}

pub fn write_a1_minimal_distributed_lm_tokenizer_dataset_bundle(
    output_root: impl AsRef<Path>,
) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
    let output_root = output_root.as_ref();
    let corpus_path = output_root.join(A1_MINIMAL_DISTRIBUTED_LM_CORPUS_FIXTURE_PATH);
    let bundle_path =
        output_root.join(A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_FIXTURE_PATH);
    if let Some(parent) = corpus_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            A1MinimalDistributedLmTokenizerDatasetBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            A1MinimalDistributedLmTokenizerDatasetBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    fs::write(
        corpus_path.as_path(),
        canonical_a1_minimal_distributed_lm_corpus_text(),
    )
    .map_err(
        |error| A1MinimalDistributedLmTokenizerDatasetBundleError::Write {
            path: corpus_path.display().to_string(),
            error,
        },
    )?;

    let bundle = canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?;
    let mut bytes = serde_json::to_vec_pretty(&bundle)?;
    bytes.push(b'\n');
    fs::write(bundle_path.as_path(), bytes).map_err(|error| {
        A1MinimalDistributedLmTokenizerDatasetBundleError::Write {
            path: bundle_path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

#[derive(Clone, Copy)]
struct CorpusSample {
    split: &'static str,
    text: &'static str,
}

fn canonical_a1_minimal_distributed_lm_samples() -> Vec<CorpusSample> {
    vec![
        CorpusSample {
            split: "training",
            text: "Mira counted seven blue sparks before the little rover found the cave.",
        },
        CorpusSample {
            split: "training",
            text: "Inside, the rover mapped each stone and sent a careful note home.",
        },
        CorpusSample {
            split: "training",
            text: "The note said the cave was small, dry, and bright enough for seeds.",
        },
        CorpusSample {
            split: "training",
            text: "By morning, Mira and the rover had planted the first moon garden.",
        },
        CorpusSample {
            split: "validation",
            text: "Mira checked the moon garden and the rover counted every new leaf.",
        },
        CorpusSample {
            split: "validation",
            text: "The careful note still matched the map of the small bright cave.",
        },
    ]
}

fn tokenizer_from_artifacts(
    artifacts: &Cs336A1BytePairEncodingArtifacts,
) -> Result<Cs336A1BytePairTokenizer, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Ok(Cs336A1BytePairTokenizer::from_vocab_and_merges(
        artifacts.vocabulary_bytes()?.as_slice(),
        artifacts.merge_pairs_bytes()?.as_slice(),
        artifacts.special_tokens.as_slice(),
    )?)
}

fn tokenized_shard(
    shard_id: &str,
    split_name: &str,
    split_kind: &str,
    samples: &[CorpusSample],
    tokenizer_digest: &str,
    tokenizer: &Cs336A1BytePairTokenizer,
) -> Result<A1MinimalDistributedLmTokenizedShard, A1MinimalDistributedLmTokenizerDatasetBundleError>
{
    if samples.is_empty() {
        return invalid_bundle(format!("split `{split_name}` must contain samples"));
    }
    let mut tokens = Vec::new();
    let mut sample_token_counts = Vec::new();
    let mut sample_text_digests = Vec::new();
    for sample in samples {
        let encoded = encode_to_u32(tokenizer, sample.text);
        sample_token_counts.push(encoded.len());
        sample_text_digests.push(sample_text_digest(sample));
        tokens.extend(encoded);
    }
    let min_sample_tokens = sample_token_counts.iter().copied().min().unwrap_or(0);
    let max_sample_tokens = sample_token_counts.iter().copied().max().unwrap_or(0);
    Ok(A1MinimalDistributedLmTokenizedShard {
        shard_id: String::from(shard_id),
        split_name: String::from(split_name),
        split_kind: String::from(split_kind),
        storage_ref: format!(
            "embedded://{}#{shard_id}",
            A1_MINIMAL_DISTRIBUTED_LM_TOKENIZER_DATASET_BUNDLE_FIXTURE_PATH
        ),
        source_shard_digest: source_shard_digest(split_name, samples),
        tokenizer_digest: String::from(tokenizer_digest),
        source_sample_count: samples.len(),
        token_count: tokens.len(),
        sample_token_counts,
        min_sample_tokens,
        max_sample_tokens,
        tokens,
        sample_text_digests,
    })
}

fn replay_sample(
    index: usize,
    sample: CorpusSample,
    tokenizer: &Cs336A1BytePairTokenizer,
) -> Result<A1MinimalDistributedLmReplaySample, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    let encoded_tokens = encode_to_u32(tokenizer, sample.text);
    let decoded_text = decode_from_u32(tokenizer, encoded_tokens.as_slice());
    let round_trip_matches = decoded_text == sample.text;
    if !round_trip_matches {
        return invalid_bundle(format!(
            "replay sample `{}` failed tokenizer round-trip",
            sample.text
        ));
    }
    Ok(A1MinimalDistributedLmReplaySample {
        sample_id: format!("{}-replay-{index:04}", sample.split),
        split_name: String::from(sample.split),
        raw_text: String::from(sample.text),
        encoded_tokens,
        decoded_text,
        round_trip_matches,
        sample_digest: sample_text_digest(&sample),
    })
}

fn encode_to_u32(tokenizer: &Cs336A1BytePairTokenizer, text: &str) -> Vec<u32> {
    tokenizer
        .encode(text)
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect()
}

fn decode_from_u32(tokenizer: &Cs336A1BytePairTokenizer, tokens: &[u32]) -> String {
    let token_ids = tokens.iter().copied().map(TokenId).collect::<Vec<_>>();
    tokenizer.decode(token_ids.as_slice())
}

fn validate_shards(
    shards: &[A1MinimalDistributedLmTokenizedShard],
    expected_split: &str,
    tokenizer_digest: &str,
) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
    if shards.is_empty() {
        return invalid_bundle(format!(
            "split `{expected_split}` must have at least one shard"
        ));
    }
    for shard in shards {
        ensure_nonempty(shard.shard_id.as_str(), "tokenized_shard.shard_id")?;
        ensure_nonempty(shard.storage_ref.as_str(), "tokenized_shard.storage_ref")?;
        ensure_sha256_uri(
            shard.source_shard_digest.as_str(),
            "tokenized_shard.source_shard_digest",
        )?;
        if shard.split_name != expected_split || shard.split_kind != expected_split {
            return invalid_bundle(format!("shard `{}` split drifted", shard.shard_id));
        }
        if shard.tokenizer_digest != tokenizer_digest {
            return invalid_bundle(format!(
                "shard `{}` tokenizer_digest drifted",
                shard.shard_id
            ));
        }
        if shard.tokens.is_empty()
            || shard.token_count != shard.tokens.len()
            || shard.source_sample_count != shard.sample_token_counts.len()
            || shard.source_sample_count != shard.sample_text_digests.len()
        {
            return invalid_bundle(format!("shard `{}` counts are invalid", shard.shard_id));
        }
        if shard.sample_token_counts.iter().any(|count| *count == 0) {
            return invalid_bundle(format!(
                "shard `{}` contains an empty tokenized sample",
                shard.shard_id
            ));
        }
        if shard.min_sample_tokens != shard.sample_token_counts.iter().copied().min().unwrap_or(0)
            || shard.max_sample_tokens
                != shard.sample_token_counts.iter().copied().max().unwrap_or(0)
        {
            return invalid_bundle(format!(
                "shard `{}` min/max sample token counts drifted",
                shard.shard_id
            ));
        }
        for digest in &shard.sample_text_digests {
            ensure_sha256_uri(digest.as_str(), "tokenized_shard.sample_text_digest")?;
        }
    }
    Ok(())
}

fn validate_replay_samples(
    bundle: &A1MinimalDistributedLmTokenizerDatasetBundle,
) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
    if bundle.replay_samples.is_empty() {
        return invalid_bundle(String::from("bundle must include replay samples"));
    }
    if !bundle
        .replay_samples
        .iter()
        .any(|sample| sample.split_name == "validation")
    {
        return invalid_bundle(String::from("bundle must include validation replay"));
    }
    let tokenizer = tokenizer_from_artifacts(&bundle.tokenizer_artifacts)?;
    for sample in &bundle.replay_samples {
        ensure_nonempty(sample.sample_id.as_str(), "replay_sample.sample_id")?;
        ensure_nonempty(sample.split_name.as_str(), "replay_sample.split_name")?;
        ensure_nonempty(sample.raw_text.as_str(), "replay_sample.raw_text")?;
        ensure_sha256_uri(sample.sample_digest.as_str(), "replay_sample.sample_digest")?;
        let encoded_tokens = encode_to_u32(&tokenizer, sample.raw_text.as_str());
        if sample.encoded_tokens != encoded_tokens {
            return invalid_bundle(format!(
                "replay sample `{}` encoded tokens drifted",
                sample.sample_id
            ));
        }
        let decoded_text = decode_from_u32(&tokenizer, sample.encoded_tokens.as_slice());
        if sample.decoded_text != decoded_text
            || !sample.round_trip_matches
            || sample.decoded_text != sample.raw_text
        {
            return invalid_bundle(format!(
                "replay sample `{}` failed round-trip validation",
                sample.sample_id
            ));
        }
    }
    Ok(())
}

fn tokenized_dataset_digest(
    split_name: &str,
    shards: &[A1MinimalDistributedLmTokenizedShard],
) -> String {
    sha256_uri_digest(
        b"psion_a1_minimal_distributed_lm_tokenized_dataset|",
        &(split_name, shards),
    )
}

fn source_shard_digest(split_name: &str, samples: &[CorpusSample]) -> String {
    let texts = samples.iter().map(|sample| sample.text).collect::<Vec<_>>();
    sha256_uri_digest(
        b"psion_a1_minimal_distributed_lm_source_shard|",
        &(split_name, texts),
    )
}

fn sample_text_digest(sample: &CorpusSample) -> String {
    sha256_uri_digest(
        b"psion_a1_minimal_distributed_lm_sample_text|",
        &(sample.split, sample.text),
    )
}

fn tokenizer_digest_from_artifacts(artifacts: &Cs336A1BytePairEncodingArtifacts) -> String {
    format!("sha256:{}", artifacts.tokenizer_digest.tokenizer_digest)
}

fn sha256_uri_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("A1 minimal distributed LM tokenizer/dataset payload should serialize"),
    );
    format!("sha256:{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
    if value.trim().is_empty() {
        return invalid_bundle(format!("field `{field}` must not be empty"));
    }
    Ok(())
}

fn ensure_sha256_uri(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmTokenizerDatasetBundleError> {
    ensure_nonempty(value, field)?;
    let Some(hex) = value.strip_prefix("sha256:") else {
        return invalid_bundle(format!("field `{field}` must use sha256:<hex> form"));
    };
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return invalid_bundle(format!(
            "field `{field}` must contain a 64-hex sha256 digest"
        ));
    }
    Ok(())
}

fn invalid_bundle<T>(
    detail: String,
) -> Result<T, A1MinimalDistributedLmTokenizerDatasetBundleError> {
    Err(A1MinimalDistributedLmTokenizerDatasetBundleError::InvalidBundle { detail })
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle,
        A1MinimalDistributedLmTokenizerDatasetBundle,
        A1MinimalDistributedLmTokenizerDatasetBundleError,
    };

    fn fixture_bundle() -> A1MinimalDistributedLmTokenizerDatasetBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenized/a1_minimal_distributed_lm_tokenizer_dataset_bundle_v1.json"
        ))
        .expect("A1 minimal distributed LM tokenizer/dataset bundle fixture should parse")
    }

    #[test]
    fn a1_minimal_distributed_lm_tokenizer_dataset_fixture_validates() {
        fixture_bundle()
            .validate()
            .expect("A1 minimal distributed LM tokenizer/dataset bundle should validate");
    }

    #[test]
    fn a1_minimal_distributed_lm_tokenizer_dataset_canonical_matches_fixture() {
        assert_eq!(
            fixture_bundle(),
            canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()
                .expect("canonical bundle should build")
        );
    }

    #[test]
    fn a1_minimal_distributed_lm_validation_replay_round_trips() {
        let bundle = fixture_bundle();
        assert!(bundle
            .replay_samples
            .iter()
            .filter(|sample| sample.split_name == "validation")
            .all(|sample| sample.round_trip_matches && sample.raw_text == sample.decoded_text));
    }

    #[test]
    fn a1_minimal_distributed_lm_rejects_shard_tokenizer_drift() {
        let mut bundle = fixture_bundle();
        bundle.training_shards[0].tokenizer_digest =
            String::from("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        bundle.bundle_digest = bundle.stable_digest();
        let error = bundle
            .validate()
            .expect_err("shard tokenizer drift should be rejected");
        assert!(matches!(
            error,
            A1MinimalDistributedLmTokenizerDatasetBundleError::InvalidBundle { .. }
        ));
    }
}
