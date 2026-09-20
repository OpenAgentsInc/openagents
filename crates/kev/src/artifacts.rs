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

/// Identity of all model inputs, including explicitly absent optional inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactIdentity {
    /// Version of the canonical identity projection.
    pub schema: String,
    /// SHA-256 of the schema and sorted file records.
    pub digest: String,
    /// Logical names such as `base/config.json`; no machine-local paths.
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

    pub(crate) fn optional(&mut self, name: &str, path: &Path) -> Result<Option<Vec<u8>>> {
        match std::fs::metadata(path) {
            Ok(_) => self.read(name, path).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.files.insert(name.to_string(), None);
                Ok(None)
            }
            Err(error) => Err(Error::Artifact(format!("read {}: {error}", path.display()))),
        }
    }

    pub(crate) fn finish(self) -> Result<ArtifactIdentity> {
        let schema = "openagents.kev.artifacts.v1".to_string();
        let bytes = serde_json::to_vec(&(&schema, &self.files))?;
        Ok(ArtifactIdentity {
            schema,
            digest: format!("sha256:{:x}", Sha256::digest(bytes)),
            files: self.files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_uses_loaded_bytes_and_logical_names() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        for directory in [first.path(), second.path()] {
            std::fs::write(directory.join("weights"), b"same input").unwrap();
            std::fs::write(directory.join("config"), b"same configuration").unwrap();
        }
        let mut a = ArtifactReader::default();
        a.read("adapter/config", &first.path().join("config"))
            .unwrap();
        let loaded = a
            .read("adapter/weights", &first.path().join("weights"))
            .unwrap();
        std::fs::write(first.path().join("weights"), b"replaced after load").unwrap();
        let a = a.finish().unwrap();
        let mut b = ArtifactReader::default();
        b.read("adapter/weights", &second.path().join("weights"))
            .unwrap();
        b.read("adapter/config", &second.path().join("config"))
            .unwrap();
        assert_eq!(a, b.finish().unwrap());
        assert_eq!(
            a.files["adapter/weights"].as_ref().unwrap().sha256,
            format!("{:x}", Sha256::digest(loaded))
        );
        let mut replacement = ArtifactReader::default();
        replacement
            .read("adapter/config", &first.path().join("config"))
            .unwrap();
        replacement
            .read("adapter/weights", &first.path().join("weights"))
            .unwrap();
        assert_ne!(a.digest, replacement.finish().unwrap().digest);
    }

    #[test]
    fn missing_optional_metadata_is_part_of_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("metadata");
        let mut absent = ArtifactReader::default();
        assert!(
            absent
                .optional("adapter/head_meta.json", &path)
                .unwrap()
                .is_none()
        );
        let absent = absent.finish().unwrap();
        std::fs::write(&path, b"{}").unwrap();
        let mut present = ArtifactReader::default();
        present.optional("adapter/head_meta.json", &path).unwrap();
        assert_ne!(absent.digest, present.finish().unwrap().digest);
    }
}
