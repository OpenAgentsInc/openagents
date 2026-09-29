//! Exact-byte references, in the shape the shared contracts define.
//!
//! `nostr::contracts::ArtifactRef` is the validator; this is the same
//! object as a serializable value, so a report can carry it and a test can
//! hand it back to `nostr::contracts::parse_artifact` to check it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The media type of every JSON document this crate writes.
pub const JSON: &str = "application/json";
/// The media type of a case's Markdown files.
pub const MARKDOWN: &str = "text/markdown";
/// The media type of a case's `case.toml`.
pub const TOML: &str = "application/toml";
/// The media type of `report.html`.
pub const HTML: &str = "text/html";

/// `{digest, size, media_type, schema?}`: the identity of exact bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    /// `sha256:` and 64 lowercase hex digits over the exact bytes.
    pub digest: String,
    /// The exact length of those bytes.
    pub size: u64,
    /// A lowercase media type. It grants no execution authority.
    pub media_type: String,
    /// The schema of a structured artifact; absent for plain bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// The EventRef of a declaration that binds these bytes, such as the
    /// NIP-EXT release that published a suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<serde_json::Value>,
}

impl ArtifactRef {
    /// The reference to `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8], media_type: &str, schema: Option<&str>) -> Self {
        Self {
            digest: nostr::contracts::digest_bytes(bytes),
            size: bytes.len() as u64,
            media_type: media_type.to_string(),
            schema: schema.map(str::to_string),
            event: None,
        }
    }

    /// The 64 hex digits after `sha256:`, which a `3189` publication's `x`
    /// tag carries.
    #[must_use]
    pub fn hex(&self) -> &str {
        self.digest.strip_prefix("sha256:").unwrap_or(&self.digest)
    }

    /// The reference as a JSON value.
    #[must_use]
    pub fn value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// Pretty JSON with a trailing newline: the bytes every document this crate
/// writes is digested over.
#[must_use]
pub fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_passes_the_shared_contract() {
        let reference = ArtifactRef::of(b"hello", JSON, Some("openagents.eval-report.v1"));
        let parsed = nostr::contracts::parse_artifact(&reference.value()).expect("valid");
        assert_eq!(parsed.digest, reference.digest);
        assert_eq!(parsed.size, 5);
        nostr::contracts::check_artifact_bytes(&parsed, b"hello").expect("the bytes match");
        assert_eq!(reference.hex().len(), 64);
    }

    #[test]
    fn plain_bytes_carry_no_schema() {
        let reference = ArtifactRef::of(b"x", MARKDOWN, None);
        assert!(reference.value().get("schema").is_none());
        nostr::contracts::parse_artifact(&reference.value()).expect("valid");
    }
}
