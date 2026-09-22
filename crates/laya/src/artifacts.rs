//! Content identity computed from the bytes the runtime loads.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The digest and size of one loaded input, independent of its local path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    /// SHA-256 of the input bytes.
    pub sha256: String,
    /// Number of input bytes.
    pub bytes: u64,
}

/// Identity of all model inputs, including explicitly absent optional
/// inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIdentity {
    /// Version of the canonical identity projection.
    pub schema: String,
    /// SHA-256 of the schema and sorted file records.
    pub digest: String,
    /// Logical names such as `encoder/config`; no machine-local paths.
    pub files: BTreeMap<String, Option<FileIdentity>>,
}

/// Reads each input once so decoding and hashing use the same bytes.
#[derive(Default)]
pub(crate) struct ArtifactReader {
    files: BTreeMap<String, Option<FileIdentity>>,
}

impl ArtifactReader {
    pub(crate) fn read(&mut self, name: &str, path: &Path) -> Result<Vec<u8>> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Artifact(format!("read {}: {e}", path.display())))?;
        self.files.insert(
            name.to_string(),
            Some(FileIdentity {
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                bytes: bytes.len() as u64,
            }),
        );
        Ok(bytes)
    }

    pub(crate) fn finish(self) -> Result<ArtifactIdentity> {
        let schema = "openagents.laya.artifacts.v1".to_string();
        let bytes = serde_json::to_vec(&(&schema, &self.files))?;
        Ok(ArtifactIdentity {
            schema,
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            files: self.files,
        })
    }
}
