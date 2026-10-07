use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const LEGAL_BENCHMARK_DATASET_SHARD_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.dataset_shard_manifest.v1";
pub const LEGAL_BENCHMARK_WORKER_SHARD_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.worker_shard_receipt.v1";
pub const LEGAL_BENCHMARK_SHARD_AGGREGATION_SCHEMA_VERSION: &str =
    "psionic.legal_benchmark.shard_aggregation_report.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalBenchmarkDatasetShardConfig {
    pub dataset_jsonl: PathBuf,
    pub shard_count: u32,
    pub out_dir: PathBuf,
    pub dataset_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalBenchmarkDatasetShardBuildResult {
    pub manifest_path: PathBuf,
    pub manifest: LegalBenchmarkDatasetShardManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkArtifactRef {
    pub artifact_id: String,
    pub artifact_type: String,
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkDatasetLock {
    pub lock_id: String,
    pub source_dataset_path: String,
    pub source_dataset_file_sha256: String,
    pub dataset_global_hash: String,
    pub mutable_dataset_allowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkDatasetShardDescriptor {
    pub shard_id: String,
    pub shard_index: u32,
    pub shard_count: u32,
    pub example_count: usize,
    pub example_ids: Vec<String>,
    pub shard_hash: String,
    pub local_artifact: LegalBenchmarkArtifactRef,
    pub uploaded_artifact: LegalBenchmarkArtifactRef,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkDatasetShardManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub source_dataset: LegalBenchmarkArtifactRef,
    pub dataset_global_hash: String,
    pub example_count: usize,
    pub shard_count: u32,
    pub sharding_rule: String,
    pub immutable_dataset_lock: LegalBenchmarkDatasetLock,
    pub shards: Vec<LegalBenchmarkDatasetShardDescriptor>,
    pub manifest_hash: String,
}

impl LegalBenchmarkDatasetShardManifest {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.manifest_hash.clear();
        stable_json_digest(b"psionic_legal_benchmark_dataset_shard_manifest|", &clone)
    }

    pub fn validate_digest(&self) -> Result<(), LegalBenchmarkDatasetShardError> {
        let expected = self.stable_digest();
        if self.manifest_hash != expected {
            return Err(LegalBenchmarkDatasetShardError::ManifestMismatch {
                expected,
                observed: self.manifest_hash.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkShardAssignment {
    pub assignment_id: String,
    pub worker_id: String,
    pub shard_id: String,
    pub shard_index: u32,
    pub attempt: u32,
    pub manifest_hash: String,
    pub dataset_global_hash: String,
    pub shard_hash: String,
    pub artifact: LegalBenchmarkArtifactRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkWorkerShardStatus {
    Accepted,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkWorkerShardReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub worker_id: String,
    pub assignment: LegalBenchmarkShardAssignment,
    pub status: LegalBenchmarkWorkerShardStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    pub credit_key: String,
    pub receipt_hash: String,
}

impl LegalBenchmarkWorkerShardReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_hash.clear();
        stable_json_digest(b"psionic_legal_benchmark_worker_shard_receipt|", &clone)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalBenchmarkWorkerShardRunConfig {
    pub manifest_json: PathBuf,
    pub expected_manifest_hash: String,
    pub shard_index: u32,
    pub worker_id: String,
    pub attempt: u32,
    pub receipt_json: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkShardAggregationReport {
    pub schema_version: String,
    pub manifest_hash: String,
    pub dataset_global_hash: String,
    pub expected_shard_count: u32,
    pub accepted_shard_count: usize,
    pub missing_shards: Vec<u32>,
    pub duplicate_submissions: Vec<String>,
    pub verified: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LegalBenchmarkShardExample {
    example_id: String,
    value: Value,
}

#[derive(Debug, Error)]
pub enum LegalBenchmarkDatasetShardError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("dataset shard argument error: {0}")]
    InvalidArgument(String),
    #[error("dataset example at line {line} is missing stable example_id")]
    MissingExampleId { line: usize },
    #[error("dataset example_id is duplicated: {example_id}")]
    DuplicateExampleId { example_id: String },
    #[error("artifact hash mismatch for {path}: expected {expected}, observed {observed}")]
    ArtifactHashMismatch {
        path: PathBuf,
        expected: String,
        observed: String,
    },
    #[error("artifact size mismatch for {path}: expected {expected}, observed {observed}")]
    ArtifactSizeMismatch {
        path: PathBuf,
        expected: u64,
        observed: u64,
    },
    #[error("required artifact is missing: {path}")]
    MissingArtifact { path: PathBuf },
    #[error("manifest hash mismatch: expected {expected}, observed {observed}")]
    ManifestMismatch { expected: String, observed: String },
    #[error("shard {shard_index} is missing from manifest")]
    MissingShard { shard_index: u32 },
    #[error("worker shard receipt cannot be accepted: {0}")]
    InvalidWorkerShardReceipt(String),
}

pub fn build_legal_benchmark_dataset_shards(
    config: &LegalBenchmarkDatasetShardConfig,
) -> Result<LegalBenchmarkDatasetShardBuildResult, LegalBenchmarkDatasetShardError> {
    if config.shard_count == 0 {
        return Err(LegalBenchmarkDatasetShardError::InvalidArgument(
            "shard count must be greater than zero".to_string(),
        ));
    }
    if config.dataset_id.trim().is_empty() {
        return Err(LegalBenchmarkDatasetShardError::InvalidArgument(
            "dataset id must be present".to_string(),
        ));
    }

    let examples = read_sorted_examples(config.dataset_jsonl.as_path())?;
    fs::create_dir_all(config.out_dir.as_path()).map_err(|source| {
        LegalBenchmarkDatasetShardError::Io {
            path: config.out_dir.clone(),
            source,
        }
    })?;
    let artifact_store = config.out_dir.join("artifacts");
    fs::create_dir_all(artifact_store.as_path()).map_err(|source| {
        LegalBenchmarkDatasetShardError::Io {
            path: artifact_store.clone(),
            source,
        }
    })?;

    let canonical_dataset_bytes = jsonl_bytes(&examples)?;
    let dataset_global_hash = sha256_hex(canonical_dataset_bytes.as_slice());
    let source_dataset = artifact_ref_for_file(
        "source-dataset-jsonl",
        "source_dataset_jsonl",
        config.dataset_jsonl.as_path(),
    )?;
    let mut shards = shard_examples(&examples, config.shard_count)?;
    let mut shard_descriptors = Vec::new();
    for (index, shard_examples) in shards.iter_mut().enumerate() {
        let shard_index = u32::try_from(index).map_err(|_| {
            LegalBenchmarkDatasetShardError::InvalidArgument(
                "shard index overflowed u32".to_string(),
            )
        })?;
        let shard_id = format!("{}.shard.{shard_index:05}", config.dataset_id);
        let shard_path = config.out_dir.join(format!("shard-{shard_index:05}.jsonl"));
        let bytes = jsonl_bytes(shard_examples)?;
        write_bytes(shard_path.as_path(), bytes.as_slice())?;
        let local_artifact = artifact_ref_for_file(
            format!("{shard_id}.local").as_str(),
            "dataset_shard_jsonl",
            shard_path.as_path(),
        )?;
        let uploaded_artifact = upload_legal_benchmark_artifact(
            shard_path.as_path(),
            artifact_store.as_path(),
            format!("{shard_id}.transport").as_str(),
            "dataset_shard_jsonl",
        )?;
        shard_descriptors.push(LegalBenchmarkDatasetShardDescriptor {
            shard_id,
            shard_index,
            shard_count: config.shard_count,
            example_count: shard_examples.len(),
            example_ids: shard_examples
                .iter()
                .map(|example| example.example_id.clone())
                .collect(),
            shard_hash: local_artifact.sha256.clone(),
            local_artifact,
            uploaded_artifact,
        });
    }

    let mut manifest = LegalBenchmarkDatasetShardManifest {
        schema_version: String::from(LEGAL_BENCHMARK_DATASET_SHARD_MANIFEST_SCHEMA_VERSION),
        dataset_id: config.dataset_id.clone(),
        source_dataset: source_dataset.clone(),
        dataset_global_hash: dataset_global_hash.clone(),
        example_count: examples.len(),
        shard_count: config.shard_count,
        sharding_rule: String::from("sort_by_example_id_then_sha256_example_id_mod_shard_count"),
        immutable_dataset_lock: LegalBenchmarkDatasetLock {
            lock_id: format!("{}.lock.{}", config.dataset_id, dataset_global_hash),
            source_dataset_path: source_dataset.path.clone(),
            source_dataset_file_sha256: source_dataset.sha256,
            dataset_global_hash,
            mutable_dataset_allowed: false,
        },
        shards: shard_descriptors,
        manifest_hash: String::new(),
    };
    manifest.manifest_hash = manifest.stable_digest();
    let manifest_path = config.out_dir.join("dataset_shard_manifest.json");
    write_json(manifest_path.as_path(), &manifest)?;
    Ok(LegalBenchmarkDatasetShardBuildResult {
        manifest_path,
        manifest,
    })
}

pub fn load_legal_benchmark_dataset_shard_manifest(
    path: impl AsRef<Path>,
) -> Result<LegalBenchmarkDatasetShardManifest, LegalBenchmarkDatasetShardError> {
    read_json(path.as_ref())
}

pub fn verify_legal_benchmark_dataset_shards(
    manifest: &LegalBenchmarkDatasetShardManifest,
) -> Result<(), LegalBenchmarkDatasetShardError> {
    manifest.validate_digest()?;
    if manifest.shards.len()
        != usize::try_from(manifest.shard_count).map_err(|_| {
            LegalBenchmarkDatasetShardError::InvalidArgument(
                "shard count overflowed usize".to_string(),
            )
        })?
    {
        return Err(LegalBenchmarkDatasetShardError::InvalidArgument(
            "manifest shard count does not match shard list".to_string(),
        ));
    }
    let mut seen_indices = BTreeSet::new();
    for shard in &manifest.shards {
        if !seen_indices.insert(shard.shard_index) {
            return Err(LegalBenchmarkDatasetShardError::InvalidArgument(format!(
                "duplicate shard index {}",
                shard.shard_index
            )));
        }
        verify_legal_benchmark_artifact(&shard.local_artifact)?;
        verify_legal_benchmark_artifact(&shard.uploaded_artifact)?;
        if shard.local_artifact.sha256 != shard.shard_hash {
            return Err(LegalBenchmarkDatasetShardError::ArtifactHashMismatch {
                path: PathBuf::from(&shard.local_artifact.path),
                expected: shard.shard_hash.clone(),
                observed: shard.local_artifact.sha256.clone(),
            });
        }
    }
    Ok(())
}

pub fn upload_legal_benchmark_artifact(
    source_path: impl AsRef<Path>,
    store_dir: impl AsRef<Path>,
    artifact_id: &str,
    artifact_type: &str,
) -> Result<LegalBenchmarkArtifactRef, LegalBenchmarkDatasetShardError> {
    let source_path = source_path.as_ref();
    let bytes = read_bytes(source_path)?;
    let digest = sha256_hex(bytes.as_slice());
    let file_name = format!("{}-{digest}.artifact", sanitize_artifact_id(artifact_id));
    let store_dir = store_dir.as_ref();
    fs::create_dir_all(store_dir).map_err(|source| LegalBenchmarkDatasetShardError::Io {
        path: store_dir.to_path_buf(),
        source,
    })?;
    let destination = store_dir.join(file_name);
    write_bytes(destination.as_path(), bytes.as_slice())?;
    artifact_ref_for_file(artifact_id, artifact_type, destination.as_path())
}

pub fn download_legal_benchmark_artifact(
    artifact: &LegalBenchmarkArtifactRef,
    destination: impl AsRef<Path>,
) -> Result<LegalBenchmarkArtifactRef, LegalBenchmarkDatasetShardError> {
    verify_legal_benchmark_artifact(artifact)?;
    let bytes = read_bytes(Path::new(&artifact.path))?;
    let destination = destination.as_ref();
    write_bytes(destination, bytes.as_slice())?;
    let downloaded = artifact_ref_for_file(
        format!("{}.download", artifact.artifact_id).as_str(),
        artifact.artifact_type.as_str(),
        destination,
    )?;
    if downloaded.sha256 != artifact.sha256 {
        return Err(LegalBenchmarkDatasetShardError::ArtifactHashMismatch {
            path: destination.to_path_buf(),
            expected: artifact.sha256.clone(),
            observed: downloaded.sha256.clone(),
        });
    }
    Ok(downloaded)
}

pub fn verify_legal_benchmark_artifact(
    artifact: &LegalBenchmarkArtifactRef,
) -> Result<(), LegalBenchmarkDatasetShardError> {
    let path = PathBuf::from(&artifact.path);
    if !path.is_file() {
        return Err(LegalBenchmarkDatasetShardError::MissingArtifact { path });
    }
    let bytes = read_bytes(path.as_path())?;
    let observed_hash = sha256_hex(bytes.as_slice());
    if observed_hash != artifact.sha256 {
        return Err(LegalBenchmarkDatasetShardError::ArtifactHashMismatch {
            path,
            expected: artifact.sha256.clone(),
            observed: observed_hash,
        });
    }
    let observed_len = u64::try_from(bytes.len()).map_err(|_| {
        LegalBenchmarkDatasetShardError::InvalidArgument(
            "artifact length overflowed u64".to_string(),
        )
    })?;
    if observed_len != artifact.byte_len {
        return Err(LegalBenchmarkDatasetShardError::ArtifactSizeMismatch {
            path,
            expected: artifact.byte_len,
            observed: observed_len,
        });
    }
    Ok(())
}

pub fn run_legal_benchmark_worker_shard(
    config: &LegalBenchmarkWorkerShardRunConfig,
) -> Result<LegalBenchmarkWorkerShardReceipt, LegalBenchmarkDatasetShardError> {
    let manifest = load_legal_benchmark_dataset_shard_manifest(config.manifest_json.as_path())?;
    manifest.validate_digest()?;
    if manifest.manifest_hash != config.expected_manifest_hash {
        return Err(LegalBenchmarkDatasetShardError::ManifestMismatch {
            expected: config.expected_manifest_hash.clone(),
            observed: manifest.manifest_hash,
        });
    }
    let shard = manifest
        .shards
        .iter()
        .find(|candidate| candidate.shard_index == config.shard_index)
        .ok_or(LegalBenchmarkDatasetShardError::MissingShard {
            shard_index: config.shard_index,
        })?;
    verify_legal_benchmark_artifact(&shard.local_artifact)?;
    let assignment =
        shard_assignment_for_worker(&manifest, shard, config.worker_id.as_str(), config.attempt);
    let mut receipt =
        worker_shard_receipt(assignment, LegalBenchmarkWorkerShardStatus::Accepted, None);
    receipt.receipt_hash = receipt.stable_digest();
    if let Some(path) = &config.receipt_json {
        write_json(path.as_path(), &receipt)?;
    }
    Ok(receipt)
}

pub fn failed_legal_benchmark_worker_shard_receipt(
    manifest: &LegalBenchmarkDatasetShardManifest,
    shard_index: u32,
    worker_id: &str,
    attempt: u32,
    failure_reason: &str,
) -> Result<LegalBenchmarkWorkerShardReceipt, LegalBenchmarkDatasetShardError> {
    let shard = manifest
        .shards
        .iter()
        .find(|candidate| candidate.shard_index == shard_index)
        .ok_or(LegalBenchmarkDatasetShardError::MissingShard { shard_index })?;
    let assignment = shard_assignment_for_worker(manifest, shard, worker_id, attempt);
    let mut receipt = worker_shard_receipt(
        assignment,
        LegalBenchmarkWorkerShardStatus::Failed,
        Some(failure_reason.to_string()),
    );
    receipt.receipt_hash = receipt.stable_digest();
    Ok(receipt)
}

pub fn reassign_legal_benchmark_failed_shard(
    manifest: &LegalBenchmarkDatasetShardManifest,
    failed_receipt: &LegalBenchmarkWorkerShardReceipt,
    new_worker_id: &str,
) -> Result<LegalBenchmarkShardAssignment, LegalBenchmarkDatasetShardError> {
    if failed_receipt.status != LegalBenchmarkWorkerShardStatus::Failed {
        return Err(LegalBenchmarkDatasetShardError::InvalidWorkerShardReceipt(
            "only failed shard receipts can be reassigned".to_string(),
        ));
    }
    if failed_receipt.assignment.manifest_hash != manifest.manifest_hash {
        return Err(LegalBenchmarkDatasetShardError::ManifestMismatch {
            expected: manifest.manifest_hash.clone(),
            observed: failed_receipt.assignment.manifest_hash.clone(),
        });
    }
    let shard_index = failed_receipt.assignment.shard_index;
    let shard = manifest
        .shards
        .iter()
        .find(|candidate| candidate.shard_index == shard_index)
        .ok_or(LegalBenchmarkDatasetShardError::MissingShard { shard_index })?;
    Ok(shard_assignment_for_worker(
        manifest,
        shard,
        new_worker_id,
        failed_receipt.assignment.attempt.saturating_add(1),
    ))
}

pub fn aggregate_legal_benchmark_worker_shard_receipts(
    manifest: &LegalBenchmarkDatasetShardManifest,
    receipts: &[LegalBenchmarkWorkerShardReceipt],
) -> Result<LegalBenchmarkShardAggregationReport, LegalBenchmarkDatasetShardError> {
    manifest.validate_digest()?;
    let mut accepted_by_shard = BTreeMap::new();
    let mut duplicate_submissions = Vec::new();
    for receipt in receipts {
        if receipt.receipt_hash != receipt.stable_digest() {
            return Err(LegalBenchmarkDatasetShardError::InvalidWorkerShardReceipt(
                format!("receipt {} hash drifted", receipt.receipt_id),
            ));
        }
        if receipt.assignment.manifest_hash != manifest.manifest_hash {
            return Err(LegalBenchmarkDatasetShardError::ManifestMismatch {
                expected: manifest.manifest_hash.clone(),
                observed: receipt.assignment.manifest_hash.clone(),
            });
        }
        if receipt.status != LegalBenchmarkWorkerShardStatus::Accepted {
            continue;
        }
        let shard = manifest
            .shards
            .iter()
            .find(|candidate| candidate.shard_index == receipt.assignment.shard_index)
            .ok_or(LegalBenchmarkDatasetShardError::MissingShard {
                shard_index: receipt.assignment.shard_index,
            })?;
        if receipt.assignment.shard_hash != shard.shard_hash {
            return Err(LegalBenchmarkDatasetShardError::ArtifactHashMismatch {
                path: PathBuf::from(&shard.local_artifact.path),
                expected: shard.shard_hash.clone(),
                observed: receipt.assignment.shard_hash.clone(),
            });
        }
        if accepted_by_shard
            .insert(receipt.assignment.shard_index, receipt.credit_key.clone())
            .is_some()
        {
            duplicate_submissions.push(receipt.credit_key.clone());
        }
    }
    let missing_shards = manifest
        .shards
        .iter()
        .filter_map(|shard| {
            if accepted_by_shard.contains_key(&shard.shard_index) {
                None
            } else {
                Some(shard.shard_index)
            }
        })
        .collect::<Vec<_>>();
    Ok(LegalBenchmarkShardAggregationReport {
        schema_version: String::from(LEGAL_BENCHMARK_SHARD_AGGREGATION_SCHEMA_VERSION),
        manifest_hash: manifest.manifest_hash.clone(),
        dataset_global_hash: manifest.dataset_global_hash.clone(),
        expected_shard_count: manifest.shard_count,
        accepted_shard_count: accepted_by_shard.len(),
        missing_shards,
        duplicate_submissions,
        verified: accepted_by_shard.len() == manifest.shards.len(),
    })
}

fn read_sorted_examples(
    dataset_jsonl: &Path,
) -> Result<Vec<LegalBenchmarkShardExample>, LegalBenchmarkDatasetShardError> {
    let content = fs::read_to_string(dataset_jsonl).map_err(|source| {
        LegalBenchmarkDatasetShardError::Io {
            path: dataset_jsonl.to_path_buf(),
            source,
        }
    })?;
    let mut examples = Vec::new();
    let mut seen = BTreeSet::new();
    for (line_index, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(line).map_err(|source| {
            LegalBenchmarkDatasetShardError::Json {
                path: dataset_jsonl.to_path_buf(),
                source,
            }
        })?;
        let example_id = value
            .get("example_id")
            .and_then(Value::as_str)
            .ok_or(LegalBenchmarkDatasetShardError::MissingExampleId {
                line: line_index + 1,
            })?
            .to_string();
        if !seen.insert(example_id.clone()) {
            return Err(LegalBenchmarkDatasetShardError::DuplicateExampleId { example_id });
        }
        examples.push(LegalBenchmarkShardExample { example_id, value });
    }
    examples.sort_by(|left, right| left.example_id.cmp(&right.example_id));
    Ok(examples)
}

fn shard_examples(
    examples: &[LegalBenchmarkShardExample],
    shard_count: u32,
) -> Result<Vec<Vec<LegalBenchmarkShardExample>>, LegalBenchmarkDatasetShardError> {
    let len = usize::try_from(shard_count).map_err(|_| {
        LegalBenchmarkDatasetShardError::InvalidArgument("shard count overflowed usize".to_string())
    })?;
    let mut shards = vec![Vec::new(); len];
    for example in examples {
        let shard_index = deterministic_shard_index(example.example_id.as_str(), shard_count)?;
        let shard_index = usize::try_from(shard_index).map_err(|_| {
            LegalBenchmarkDatasetShardError::InvalidArgument(
                "shard index overflowed usize".to_string(),
            )
        })?;
        shards[shard_index].push(example.clone());
    }
    Ok(shards)
}

fn deterministic_shard_index(
    example_id: &str,
    shard_count: u32,
) -> Result<u32, LegalBenchmarkDatasetShardError> {
    if shard_count == 0 {
        return Err(LegalBenchmarkDatasetShardError::InvalidArgument(
            "shard count must be greater than zero".to_string(),
        ));
    }
    let digest = Sha256::digest(example_id.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let value = u64::from_be_bytes(bytes);
    Ok((value % u64::from(shard_count)) as u32)
}

fn jsonl_bytes(
    examples: &[LegalBenchmarkShardExample],
) -> Result<Vec<u8>, LegalBenchmarkDatasetShardError> {
    let mut bytes = Vec::new();
    for example in examples {
        let mut line = serde_json::to_vec(&example.value).map_err(|source| {
            LegalBenchmarkDatasetShardError::Json {
                path: PathBuf::from("<canonical-jsonl>"),
                source,
            }
        })?;
        bytes.append(&mut line);
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn shard_assignment_for_worker(
    manifest: &LegalBenchmarkDatasetShardManifest,
    shard: &LegalBenchmarkDatasetShardDescriptor,
    worker_id: &str,
    attempt: u32,
) -> LegalBenchmarkShardAssignment {
    LegalBenchmarkShardAssignment {
        assignment_id: format!(
            "{}.{}.attempt.{attempt}",
            shard.shard_id,
            sanitize_artifact_id(worker_id)
        ),
        worker_id: worker_id.to_string(),
        shard_id: shard.shard_id.clone(),
        shard_index: shard.shard_index,
        attempt,
        manifest_hash: manifest.manifest_hash.clone(),
        dataset_global_hash: manifest.dataset_global_hash.clone(),
        shard_hash: shard.shard_hash.clone(),
        artifact: shard.uploaded_artifact.clone(),
    }
}

fn worker_shard_receipt(
    assignment: LegalBenchmarkShardAssignment,
    status: LegalBenchmarkWorkerShardStatus,
    failure_reason: Option<String>,
) -> LegalBenchmarkWorkerShardReceipt {
    let credit_key = format!(
        "{}:{}:{}",
        assignment.dataset_global_hash, assignment.shard_id, assignment.shard_hash
    );
    LegalBenchmarkWorkerShardReceipt {
        schema_version: String::from(LEGAL_BENCHMARK_WORKER_SHARD_RECEIPT_SCHEMA_VERSION),
        receipt_id: format!(
            "receipt.{}.{}.{}",
            assignment.shard_id,
            sanitize_artifact_id(assignment.worker_id.as_str()),
            assignment.attempt
        ),
        worker_id: assignment.worker_id.clone(),
        assignment,
        status,
        failure_reason,
        credit_key,
        receipt_hash: String::new(),
    }
}

fn artifact_ref_for_file(
    artifact_id: &str,
    artifact_type: &str,
    path: &Path,
) -> Result<LegalBenchmarkArtifactRef, LegalBenchmarkDatasetShardError> {
    let bytes = read_bytes(path)?;
    let byte_len = u64::try_from(bytes.len()).map_err(|_| {
        LegalBenchmarkDatasetShardError::InvalidArgument(
            "artifact length overflowed u64".to_string(),
        )
    })?;
    Ok(LegalBenchmarkArtifactRef {
        artifact_id: artifact_id.to_string(),
        artifact_type: artifact_type.to_string(),
        path: path.display().to_string(),
        sha256: sha256_hex(bytes.as_slice()),
        byte_len,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, LegalBenchmarkDatasetShardError> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(bytes.as_slice()).map_err(|source| {
        LegalBenchmarkDatasetShardError::Json {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), LegalBenchmarkDatasetShardError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| {
        LegalBenchmarkDatasetShardError::Json {
            path: path.to_path_buf(),
            source,
        }
    })?;
    write_bytes(path, bytes.as_slice())
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, LegalBenchmarkDatasetShardError> {
    fs::read(path).map_err(|source| LegalBenchmarkDatasetShardError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), LegalBenchmarkDatasetShardError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| LegalBenchmarkDatasetShardError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| LegalBenchmarkDatasetShardError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sanitize_artifact_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn legal_dataset_sharding_is_deterministic() {
        let temp = tempdir().expect("tempdir");
        let dataset = temp.path().join("legal-sft-v1.jsonl");
        write_sample_dataset(&dataset, &["case-c", "case-a", "case-b", "case-d"]);

        let first = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset.clone(),
            shard_count: 3,
            out_dir: temp.path().join("first"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("first shard build");
        let second = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset,
            shard_count: 3,
            out_dir: temp.path().join("second"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("second shard build");

        assert_eq!(
            first.manifest.dataset_global_hash,
            second.manifest.dataset_global_hash
        );
        assert_eq!(
            first
                .manifest
                .shards
                .iter()
                .map(|shard| (&shard.example_ids, &shard.shard_hash))
                .collect::<Vec<_>>(),
            second
                .manifest
                .shards
                .iter()
                .map(|shard| (&shard.example_ids, &shard.shard_hash))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn worker_refuses_manifest_hash_mismatch() {
        let temp = tempdir().expect("tempdir");
        let dataset = temp.path().join("legal-sft-v1.jsonl");
        write_sample_dataset(&dataset, &["case-a", "case-b"]);
        let result = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset,
            shard_count: 2,
            out_dir: temp.path().join("shards"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("shard build");

        let error = run_legal_benchmark_worker_shard(&LegalBenchmarkWorkerShardRunConfig {
            manifest_json: result.manifest_path,
            expected_manifest_hash: String::from("wrong-manifest-hash"),
            shard_index: 0,
            worker_id: String::from("pylon.worker.1"),
            attempt: 0,
            receipt_json: None,
        })
        .expect_err("manifest mismatch should fail");

        assert!(matches!(
            error,
            LegalBenchmarkDatasetShardError::ManifestMismatch { .. }
        ));
    }

    #[test]
    fn aggregator_detects_missing_shard_artifacts() {
        let temp = tempdir().expect("tempdir");
        let dataset = temp.path().join("legal-sft-v1.jsonl");
        write_sample_dataset(&dataset, &["case-a", "case-b"]);
        let result = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset,
            shard_count: 2,
            out_dir: temp.path().join("shards"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("shard build");
        let shard_path = PathBuf::from(&result.manifest.shards[0].local_artifact.path);
        fs::remove_file(shard_path).expect("remove shard file");

        let error = verify_legal_benchmark_dataset_shards(&result.manifest)
            .expect_err("missing shard should fail");
        assert!(matches!(
            error,
            LegalBenchmarkDatasetShardError::MissingArtifact { .. }
        ));
    }

    #[test]
    fn failed_worker_shard_can_be_reassigned() {
        let temp = tempdir().expect("tempdir");
        let dataset = temp.path().join("legal-sft-v1.jsonl");
        write_sample_dataset(&dataset, &["case-a", "case-b"]);
        let result = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset,
            shard_count: 2,
            out_dir: temp.path().join("shards"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("shard build");
        let failed = failed_legal_benchmark_worker_shard_receipt(
            &result.manifest,
            1,
            "pylon.worker.failed",
            0,
            "transient worker exit",
        )
        .expect("failed receipt");
        let reassigned =
            reassign_legal_benchmark_failed_shard(&result.manifest, &failed, "pylon.worker.retry")
                .expect("reassigned");

        assert_eq!(reassigned.shard_index, 1);
        assert_eq!(reassigned.attempt, 1);
        assert_eq!(reassigned.worker_id, "pylon.worker.retry");
        assert_eq!(
            reassigned.credit_key_like(),
            failed.assignment.credit_key_like()
        );
    }

    #[test]
    fn duplicate_shard_submissions_are_detected_without_duplicate_credit() {
        let temp = tempdir().expect("tempdir");
        let dataset = temp.path().join("legal-sft-v1.jsonl");
        write_sample_dataset(&dataset, &["case-a", "case-b", "case-c"]);
        let result = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
            dataset_jsonl: dataset,
            shard_count: 1,
            out_dir: temp.path().join("shards"),
            dataset_id: String::from("legal-sft-v1"),
        })
        .expect("shard build");
        let first = run_legal_benchmark_worker_shard(&LegalBenchmarkWorkerShardRunConfig {
            manifest_json: result.manifest_path.clone(),
            expected_manifest_hash: result.manifest.manifest_hash.clone(),
            shard_index: 0,
            worker_id: String::from("pylon.worker.1"),
            attempt: 0,
            receipt_json: None,
        })
        .expect("first receipt");
        let duplicate = run_legal_benchmark_worker_shard(&LegalBenchmarkWorkerShardRunConfig {
            manifest_json: result.manifest_path,
            expected_manifest_hash: result.manifest.manifest_hash.clone(),
            shard_index: 0,
            worker_id: String::from("pylon.worker.2"),
            attempt: 0,
            receipt_json: None,
        })
        .expect("duplicate receipt");

        let report =
            aggregate_legal_benchmark_worker_shard_receipts(&result.manifest, &[first, duplicate])
                .expect("aggregate");

        assert_eq!(report.accepted_shard_count, 1);
        assert_eq!(report.duplicate_submissions.len(), 1);
        assert!(report.verified);
    }

    #[test]
    fn artifact_upload_download_roundtrip_verifies_hash() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("artifact.jsonl");
        fs::write(&source, b"{\"example_id\":\"case-a\"}\n").expect("write source");
        let artifact = upload_legal_benchmark_artifact(
            &source,
            temp.path().join("store"),
            "artifact.case-a",
            "dataset_shard_jsonl",
        )
        .expect("upload");
        let downloaded =
            download_legal_benchmark_artifact(&artifact, temp.path().join("copy.jsonl"))
                .expect("download");

        assert_eq!(downloaded.sha256, artifact.sha256);
        assert_eq!(downloaded.byte_len, artifact.byte_len);
    }

    fn write_sample_dataset(path: &Path, ids: &[&str]) {
        let lines = ids
            .iter()
            .map(|id| {
                serde_json::to_string(&json!({
                    "schema_version": "legal_sft_v1",
                    "example_id": id,
                    "messages": [{"role": "user", "content": format!("task {id}")}],
                    "answer_files": [],
                    "training_tags": ["fixture"]
                }))
                .expect("serialize sample row")
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{lines}\n")).expect("write sample dataset");
    }

    trait CreditKeyLike {
        fn credit_key_like(&self) -> String;
    }

    impl CreditKeyLike for LegalBenchmarkShardAssignment {
        fn credit_key_like(&self) -> String {
            format!(
                "{}:{}:{}",
                self.dataset_global_hash, self.shard_id, self.shard_hash
            )
        }
    }
}
