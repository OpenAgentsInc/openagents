use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::keys::import_metadata as import_keys;
use crate::{CadError, CadResult};

/// Typed STEP import metadata contract stored in CadDocument.metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadStepImportMetadata {
    pub format: String,
    pub import_hash: String,
    pub solid_count: usize,
    pub shell_count: usize,
    pub face_count: usize,
}

impl CadStepImportMetadata {
    pub fn new(
        import_hash: impl Into<String>,
        solid_count: usize,
        shell_count: usize,
        face_count: usize,
    ) -> CadResult<Self> {
        let metadata = Self {
            format: "step".to_string(),
            import_hash: import_hash.into(),
            solid_count,
            shell_count,
            face_count,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn encode_into(&self, metadata: &mut BTreeMap<String, String>) {
        metadata.insert(import_keys::FORMAT.owned(), self.format.clone());
        metadata.insert(import_keys::HASH.owned(), self.import_hash.clone());
        metadata.insert(
            import_keys::SOLID_COUNT.owned(),
            self.solid_count.to_string(),
        );
        metadata.insert(
            import_keys::SHELL_COUNT.owned(),
            self.shell_count.to_string(),
        );
        metadata.insert(import_keys::FACE_COUNT.owned(), self.face_count.to_string());
    }

    pub fn decode_from(metadata: &BTreeMap<String, String>) -> CadResult<Self> {
        let format = required_value(metadata, import_keys::FORMAT)?;
        let import_hash = required_value(metadata, import_keys::HASH)?;
        let solid_count = parse_required_usize(metadata, import_keys::SOLID_COUNT)?;
        let shell_count = parse_required_usize(metadata, import_keys::SHELL_COUNT)?;
        let face_count = parse_required_usize(metadata, import_keys::FACE_COUNT)?;
        let decoded = Self {
            format,
            import_hash,
            solid_count,
            shell_count,
            face_count,
        };
        decoded.validate()?;
        Ok(decoded)
    }

    pub fn validate(&self) -> CadResult<()> {
        if self.format != "step" {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "step import metadata format must be step; got {}",
                    self.format
                ),
            });
        }
        if self.import_hash.trim().is_empty() {
            return Err(CadError::ParseFailed {
                reason: "step import metadata requires non-empty import hash".to_string(),
            });
        }
        if self.solid_count == 0 {
            return Err(CadError::ParseFailed {
                reason: "step import metadata requires solid_count > 0".to_string(),
            });
        }
        if self.shell_count < self.solid_count {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "step import metadata requires shell_count >= solid_count; got shells={} solids={}",
                    self.shell_count, self.solid_count
                ),
            });
        }
        if self.face_count == 0 {
            return Err(CadError::ParseFailed {
                reason: "step import metadata requires face_count > 0".to_string(),
            });
        }
        Ok(())
    }
}

fn required_value(
    metadata: &BTreeMap<String, String>,
    key: crate::keys::CadMapKey,
) -> CadResult<String> {
    metadata
        .get(key.as_str())
        .cloned()
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("step import metadata missing required key {}", key.as_str()),
        })
}

fn parse_required_usize(
    metadata: &BTreeMap<String, String>,
    key: crate::keys::CadMapKey,
) -> CadResult<usize> {
    let raw = required_value(metadata, key)?;
    raw.parse::<usize>().map_err(|error| CadError::ParseFailed {
        reason: format!(
            "step import metadata key {} must be usize; got {} ({error})",
            key.as_str(),
            raw
        ),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::CadStepImportMetadata;
    use crate::keys::import_metadata as import_keys;

    #[test]
    fn encode_decode_roundtrip_is_deterministic() {
        let metadata =
            CadStepImportMetadata::new("abc123", 2, 3, 8).expect("metadata should build");
        let mut map = BTreeMap::new();
        metadata.encode_into(&mut map);
        let decoded = CadStepImportMetadata::decode_from(&map).expect("metadata should decode");
        assert_eq!(decoded, metadata);
    }

    #[test]
    fn decode_rejects_missing_required_key() {
        let mut map = BTreeMap::new();
        map.insert(import_keys::FORMAT.owned(), "step".to_string());
        map.insert(import_keys::HASH.owned(), "abc123".to_string());
        map.insert(import_keys::SOLID_COUNT.owned(), "1".to_string());
        map.insert(import_keys::SHELL_COUNT.owned(), "1".to_string());
        let error =
            CadStepImportMetadata::decode_from(&map).expect_err("missing face count should fail");
        assert!(
            error.to_string().contains(import_keys::FACE_COUNT.as_str()),
            "error should identify missing key"
        );
    }

    #[test]
    fn decode_rejects_invalid_counts() {
        let mut map = BTreeMap::new();
        map.insert(import_keys::FORMAT.owned(), "step".to_string());
        map.insert(import_keys::HASH.owned(), "abc123".to_string());
        map.insert(import_keys::SOLID_COUNT.owned(), "2".to_string());
        map.insert(import_keys::SHELL_COUNT.owned(), "1".to_string());
        map.insert(import_keys::FACE_COUNT.owned(), "8".to_string());
        let error = CadStepImportMetadata::decode_from(&map)
            .expect_err("shell count < solid count should fail");
        assert!(error.to_string().contains("shell_count"));
    }
}
