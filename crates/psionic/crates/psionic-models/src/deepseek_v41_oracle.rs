//! Golden activations from the DeepSeek-V4.1-Flash reference (V0 oracle,
//! #12371).
//!
//! The pinned checkpoint's own PyTorch reference ran four fixed prompts with
//! greedy decoding and recorded selected layers at five positions per prompt.
//! The full dumps live in Cloud Storage; the repository keeps their manifest,
//! the first and last 64 values of every tensor, and some small tensors whole,
//! gzip-compressed under `src/testdata/deepseek_v41/oracle/`. Each tensor's SHA-256 covers its raw
//! little-endian, row-major bytes, so a value read from the bucket or computed
//! by Psionic can be checked against it.
//!
//! The dump tool and its positions are described in
//! `scripts/bench/deepseek-v41/oracle_dump.py`.

use std::{
    collections::BTreeMap,
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Schema label of the repository fixture manifest.
pub const ORACLE_FIXTURES_SCHEMA: &str = "openagents.deepseek_v41.oracle_fixtures.v1";

/// The checked-in fixture directory.
#[must_use]
pub fn oracle_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/testdata/deepseek_v41/oracle")
}

/// Errors from reading the oracle fixtures.
#[derive(Debug, thiserror::Error)]
pub enum V41OracleError {
    /// A file couldn't be read.
    #[error("reading {path}: {message}")]
    Io {
        /// The file.
        path: String,
        /// The error.
        message: String,
    },
    /// The manifest or slices didn't parse, or have the wrong schema.
    #[error("invalid oracle fixtures: {0}")]
    Invalid(String),
    /// The manifest has no tensor of that name.
    #[error("no oracle tensor named `{0}`")]
    UnknownTensor(String),
}

/// One recorded tensor.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct V41OracleTensor {
    /// `PROMPT.MODE.LABEL.SITE`, for example `p1-chat-thinking.decode.last.layers.20.attn.out`.
    pub name: String,
    /// The rank file in the bucket that holds it.
    pub file: String,
    /// Shape.
    pub shape: Vec<u64>,
    /// PyTorch dtype name, such as `bfloat16`, `float32`, `int32`, or `float8_e4m3fn`.
    pub dtype: String,
    /// Bytes per element.
    pub element_bytes: u64,
    /// Total bytes.
    pub bytes: u64,
    /// SHA-256 of the raw bytes, lowercase hex.
    pub sha256: String,
    /// Where `full.bin` holds the whole tensor, when it does.
    #[serde(default)]
    pub full: Option<V41OracleFullRange>,
}

/// A byte range of `full.bin`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct V41OracleFullRange {
    /// Offset.
    pub offset: u64,
    /// Length.
    pub bytes: u64,
}

/// One prompt's run.
#[derive(Clone, Debug, Deserialize)]
pub struct V41OraclePrompt {
    /// Prompt id, the first part of every tensor name.
    pub id: String,
    /// Token ids of the rendered prompt.
    pub prompt_token_ids: Vec<u32>,
    /// Greedy tokens generated: 32 for the four main prompts, 8 for the edge cases.
    pub n_gen: usize,
    /// The greedy tokens.
    pub generated_ids: Vec<u32>,
    /// Their text, cut at the end-of-sentence token.
    #[serde(default)]
    pub generated_text: Option<String>,
    /// The answer generated to the end-of-sentence token (at most 2,048 tokens), for the
    /// first two prompts.
    #[serde(default)]
    pub complete_answer: Option<serde_json::Value>,
    /// Per forward step: position, argmax, top-64 logits, and the full vector's SHA-256.
    pub steps: Vec<V41OracleStep>,
}

/// One forward step of a prompt (0 is the prefill).
#[derive(Clone, Debug, Deserialize)]
pub struct V41OracleStep {
    /// Step.
    pub step: u32,
    /// Position of the last input token.
    pub position: u64,
    /// Greedy next token.
    pub argmax: u32,
    /// SHA-256 of the f32 logits.
    pub logits_sha256: String,
    /// Top-64 token ids, highest first.
    pub top_ids: Vec<u32>,
    /// Their logits.
    pub top_values: Vec<f32>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    schema: String,
    revision: String,
    bucket: String,
    full_file: String,
    full_sha256: String,
    prompts: Vec<V41OraclePrompt>,
    tensors: Vec<V41OracleTensor>,
}

#[derive(Debug, Deserialize)]
struct RawSlice {
    head: String,
    tail: String,
}

/// The repository's oracle fixtures, loaded.
#[derive(Debug)]
pub struct V41OracleFixtures {
    /// Checkpoint revision.
    pub revision: String,
    /// Where the full dumps live.
    pub bucket: String,
    /// The prompts.
    pub prompts: Vec<V41OraclePrompt>,
    tensors: BTreeMap<String, V41OracleTensor>,
    slices: BTreeMap<String, (Vec<u8>, Vec<u8>)>,
    full: Vec<u8>,
    full_sha256: String,
}

fn read(path: &Path) -> Result<Vec<u8>, V41OracleError> {
    let io = |error: std::io::Error| V41OracleError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    };
    let bytes = fs::read(path).map_err(io)?;
    if path.extension().is_some_and(|ext| ext == "gz") {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut out)
            .map_err(io)?;
        Ok(out)
    } else {
        Ok(bytes)
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

impl V41OracleFixtures {
    /// Loads `manifest.json.gz`, `slices.json.gz`, and `full.bin.gz` from `dir`.
    ///
    /// # Errors
    ///
    /// Refuses unreadable files, the wrong schema, and malformed hex.
    pub fn load(dir: &Path) -> Result<Self, V41OracleError> {
        let manifest: RawManifest = serde_json::from_slice(&read(&dir.join("manifest.json.gz"))?)
            .map_err(|error| V41OracleError::Invalid(error.to_string()))?;
        if manifest.schema != ORACLE_FIXTURES_SCHEMA {
            return Err(V41OracleError::Invalid(format!(
                "schema is `{}`, wants `{ORACLE_FIXTURES_SCHEMA}`",
                manifest.schema
            )));
        }
        let raw_slices: BTreeMap<String, RawSlice> =
            serde_json::from_slice(&read(&dir.join("slices.json.gz"))?)
                .map_err(|error| V41OracleError::Invalid(error.to_string()))?;
        let mut slices = BTreeMap::new();
        for (name, slice) in raw_slices {
            let decode = |text: &str| {
                hex::decode(text)
                    .map_err(|error| V41OracleError::Invalid(format!("{name}: {error}")))
            };
            slices.insert(name.clone(), (decode(&slice.head)?, decode(&slice.tail)?));
        }
        Ok(Self {
            revision: manifest.revision,
            bucket: manifest.bucket,
            prompts: manifest.prompts,
            tensors: manifest
                .tensors
                .into_iter()
                .map(|tensor| (tensor.name.clone(), tensor))
                .collect(),
            slices,
            full: read(&dir.join(format!("{}.gz", manifest.full_file)))?,
            full_sha256: manifest.full_sha256,
        })
    }

    /// Loads the checked-in fixtures.
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn load_checked_in() -> Result<Self, V41OracleError> {
        Self::load(&oracle_fixture_dir())
    }

    /// Every tensor, by name.
    pub fn tensors(&self) -> impl Iterator<Item = &V41OracleTensor> {
        self.tensors.values()
    }

    /// One tensor's entry.
    ///
    /// # Errors
    ///
    /// Returns [`V41OracleError::UnknownTensor`].
    pub fn tensor(&self, name: &str) -> Result<&V41OracleTensor, V41OracleError> {
        self.tensors
            .get(name)
            .ok_or_else(|| V41OracleError::UnknownTensor(name.to_owned()))
    }

    /// The whole tensor's bytes, when the repository keeps it whole.
    #[must_use]
    pub fn full_bytes(&self, name: &str) -> Option<&[u8]> {
        let range = self.tensors.get(name)?.full?;
        let start = usize::try_from(range.offset).ok()?;
        let end = start.checked_add(usize::try_from(range.bytes).ok()?)?;
        self.full.get(start..end)
    }

    /// The first and last 64 values' bytes of a tensor the repository keeps
    /// only in part. For a tensor kept whole, slice [`Self::full_bytes`].
    #[must_use]
    pub fn head_tail(&self, name: &str) -> Option<(&[u8], &[u8])> {
        self.slices
            .get(name)
            .map(|(head, tail)| (head.as_slice(), tail.as_slice()))
    }

    /// Whether `bytes` hash to the tensor's recorded SHA-256.
    ///
    /// # Errors
    ///
    /// Returns [`V41OracleError::UnknownTensor`].
    pub fn matches(&self, name: &str, bytes: &[u8]) -> Result<bool, V41OracleError> {
        Ok(sha256_hex(bytes) == self.tensor(name)?.sha256)
    }

    /// Checks `full.bin` against its hash and every whole tensor against its
    /// SHA-256, and that every other tensor has its head and tail. Returns
    /// the names that fail.
    #[must_use]
    pub fn verify(&self) -> Vec<String> {
        let mut failures = Vec::new();
        if sha256_hex(&self.full) != self.full_sha256 {
            failures.push("full.bin".to_owned());
        }
        for tensor in self.tensors.values() {
            let ok = match self.full_bytes(&tensor.name) {
                Some(bytes) => sha256_hex(bytes) == tensor.sha256,
                None => self.head_tail(&tensor.name).is_some_and(|(head, tail)| {
                    let want = (tensor.element_bytes * 64).min(tensor.bytes);
                    head.len() as u64 == want && tail.len() as u64 == want
                }),
            };
            if !ok {
                failures.push(tensor.name.clone());
            }
        }
        failures
    }
}

#[cfg(test)]
#[path = "deepseek_v41_oracle_tests.rs"]
mod tests;
