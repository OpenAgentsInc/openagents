//! Repo index job type (`oa.repo_index.v1`).
//!
//! This job type produces a verifiable repository snapshot manifest (file digests) plus optional
//! index artifacts (symbols/embeddings/etc). Phase-1 verification is objective and focuses on
//! deterministic snapshot hashing + internal consistency.

use crate::hash::{HashError, canonical_hash};
use crate::provenance::Provenance;
use crate::verification::Verification;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

use super::{JobRequest, JobResponse};

/// Index artifact classes that a provider may produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoIndexType {
    Digests,
    Symbols,
    Embeddings,
    Dependencies,
}

fn default_index_types() -> Vec<RepoIndexType> {
    vec![RepoIndexType::Digests]
}

/// A single file digest entry for a repository snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoFileDigest {
    /// Path relative to repository root (POSIX style).
    pub path: String,
    /// SHA-256 hex digest of the file bytes.
    pub sha256: String,
    /// File size in bytes.
    pub bytes: u64,
}

/// Optional index artifact metadata (the artifact payload is out-of-band).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoIndexArtifact {
    pub index_type: RepoIndexType,
    pub sha256: String,
    pub bytes: u64,
}

/// Request to produce a repository index snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndexRequest {
    /// Repository clone URL (or other opaque locator).
    pub repo_url: String,
    /// Git ref to index (branch/tag/commit).
    pub git_ref: String,
    /// Expected tree hash for the snapshot. This is a deterministic hash over the file-digest
    /// manifest (see [`compute_tree_sha256`]).
    pub expected_tree_sha256: String,
    /// Which indexes to produce; defaults to `digests` only.
    #[serde(default = "default_index_types")]
    pub index_types: Vec<RepoIndexType>,
    /// Verification settings (objective by default).
    #[serde(default)]
    pub verification: Verification,
}

impl Default for RepoIndexRequest {
    fn default() -> Self {
        Self {
            repo_url: String::new(),
            git_ref: String::new(),
            expected_tree_sha256: String::new(),
            index_types: default_index_types(),
            verification: Verification::objective(),
        }
    }
}

impl JobRequest for RepoIndexRequest {
    const JOB_TYPE: &'static str = "oa.repo_index.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}

/// Response for a repository index snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndexResponse {
    /// Tree hash computed from the returned digest manifest.
    pub tree_sha256: String,
    /// File digest manifest for the snapshot.
    #[serde(default)]
    pub digests: Vec<RepoFileDigest>,
    /// Optional additional artifact metadata (payloads are out-of-band).
    #[serde(default)]
    pub artifacts: Vec<RepoIndexArtifact>,
    /// Provenance for how the indexes were produced.
    pub provenance: Provenance,
}

impl JobResponse for RepoIndexResponse {
    type Request = RepoIndexRequest;

    fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

/// Deterministically compute a repository "tree" hash from a digest manifest.
///
/// This is intended to be portable across implementations by relying on canonical JSON hashing.
pub fn compute_tree_sha256(digests: &[RepoFileDigest]) -> Result<String, HashError> {
    let mut sorted = digests.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    canonical_hash(&sorted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_hash_is_deterministic_across_digest_order() {
        let a = RepoFileDigest {
            path: "a.txt".to_string(),
            sha256: "0".repeat(64),
            bytes: 1,
        };
        let b = RepoFileDigest {
            path: "b.txt".to_string(),
            sha256: "1".repeat(64),
            bytes: 2,
        };

        let hash1 = compute_tree_sha256(&[a.clone(), b.clone()]).expect("hash");
        let hash2 = compute_tree_sha256(&[b, a]).expect("hash");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn request_job_type_and_schema_version_are_stable() {
        assert_eq!(RepoIndexRequest::JOB_TYPE, "oa.repo_index.v1");
        assert_eq!(RepoIndexRequest::SCHEMA_VERSION.to_string(), "1.0.0");
    }
}
