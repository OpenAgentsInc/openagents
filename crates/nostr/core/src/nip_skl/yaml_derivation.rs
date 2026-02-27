//! Deterministic SKILL payload hashing + SKL manifest derivation.

use super::manifest::{ManifestError, SkillManifest};
use crate::nip01::EventTemplate;
use serde_yaml::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors for frontmatter/payload derivation.
#[derive(Debug, Error)]
pub enum YamlDerivationError {
    #[error("missing YAML frontmatter block")]
    MissingFrontmatter,

    #[error("missing required frontmatter field: {0}")]
    MissingField(&'static str),

    #[error("invalid field type: {0}")]
    InvalidField(&'static str),

    #[error("yaml parse error: {0}")]
    YamlParse(String),

    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

/// Derived manifest artifact from a SKILL payload.
#[derive(Debug, Clone)]
pub struct DerivedManifest {
    pub manifest: SkillManifest,
    pub event_template: EventTemplate,
    pub normalized_payload_hash: String,
}

/// Normalize SKILL payload bytes for deterministic hashing:
/// - strip UTF-8 BOM
/// - normalize line endings to LF (`\n`)
pub fn normalize_skill_payload(payload: &str) -> Vec<u8> {
    let without_bom = payload.trim_start_matches('\u{feff}');
    let normalized = without_bom.replace("\r\n", "\n").replace('\r', "\n");
    normalized.into_bytes()
}

/// SHA-256 hash over normalized payload bytes.
pub fn hash_skill_payload(payload: &str) -> String {
    let normalized = normalize_skill_payload(payload);
    let mut hasher = Sha256::new();
    hasher.update(normalized);
    hex::encode(hasher.finalize())
}

/// Derive SKL manifest + event template from SKILL payload frontmatter.
///
/// Deterministic inputs:
/// - `payload`: SKILL payload text (frontmatter + body)
/// - `publisher_pubkey`: skill publisher pubkey for `skill_scope_id`
/// - `created_at`: caller-supplied timestamp for deterministic event construction
pub fn derive_manifest_from_skill_payload(
    payload: &str,
    publisher_pubkey: &str,
    created_at: u64,
) -> Result<DerivedManifest, YamlDerivationError> {
    let normalized = normalize_skill_payload(payload);
    let normalized_str = String::from_utf8(normalized.clone())
        .map_err(|_| YamlDerivationError::InvalidField("payload must be valid utf-8"))?;
    let (frontmatter, body) = split_frontmatter(&normalized_str)?;
    let map = parse_frontmatter(frontmatter)?;

    let identifier = string_field(&map, "d")
        .or_else(|| string_field(&map, "identifier"))
        .ok_or(YamlDerivationError::MissingField("d"))?;
    let name = string_field(&map, "name").ok_or(YamlDerivationError::MissingField("name"))?;
    let version =
        string_field(&map, "version").ok_or(YamlDerivationError::MissingField("version"))?;
    let description = string_field(&map, "description")
        .ok_or(YamlDerivationError::MissingField("description"))?;
    let expiry = u64_field(&map, "expiry").ok_or(YamlDerivationError::MissingField("expiry"))?;

    let capabilities = sequence_string_field(&map, "capabilities").unwrap_or_default();
    let payload_hash = hash_bytes(&normalized);

    let mut manifest = SkillManifest::new(
        identifier,
        name,
        version,
        description,
        payload_hash.clone(),
        capabilities,
        expiry,
    )
    .with_content(body.to_string());

    if let Some(author_npub) = string_field(&map, "author_npub") {
        manifest = manifest.with_author_npub(author_npub);
    }
    if let Some(author_pubkey) = string_field(&map, "author_pubkey") {
        manifest = manifest.with_author_pubkey(author_pubkey);
    }

    let event_template = manifest.to_event_template(publisher_pubkey, created_at)?;

    Ok(DerivedManifest {
        manifest,
        event_template,
        normalized_payload_hash: payload_hash,
    })
}

fn split_frontmatter(payload: &str) -> Result<(&str, &str), YamlDerivationError> {
    let start = "---\n";
    if !payload.starts_with(start) {
        return Err(YamlDerivationError::MissingFrontmatter);
    }

    let remaining = &payload[start.len()..];
    let end_marker = "\n---\n";
    let end_index = remaining
        .find(end_marker)
        .ok_or(YamlDerivationError::MissingFrontmatter)?;

    let frontmatter = &remaining[..end_index];
    let body = &remaining[end_index + end_marker.len()..];
    Ok((frontmatter, body))
}

fn parse_frontmatter(frontmatter: &str) -> Result<serde_yaml::Mapping, YamlDerivationError> {
    match serde_yaml::from_str::<Value>(frontmatter) {
        Ok(Value::Mapping(map)) => Ok(map),
        Ok(_) => Err(YamlDerivationError::InvalidField(
            "frontmatter must be a YAML mapping",
        )),
        Err(error) => Err(YamlDerivationError::YamlParse(error.to_string())),
    }
}

fn string_field(map: &serde_yaml::Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(|value| value.as_str().map(str::to_string))
}

fn u64_field(map: &serde_yaml::Mapping, key: &str) -> Option<u64> {
    map.get(Value::String(key.to_string())).and_then(|value| {
        if let Some(number) = value.as_u64() {
            Some(number)
        } else {
            value.as_str().and_then(|text| text.parse::<u64>().ok())
        }
    })
}

fn sequence_string_field(map: &serde_yaml::Mapping, key: &str) -> Option<Vec<String>> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_sequence)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
}

fn hash_bytes(payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_skill_payload_line_endings_and_bom() {
        let payload = "\u{feff}line1\r\nline2\rline3\n";
        let normalized = normalize_skill_payload(payload);
        assert_eq!(
            String::from_utf8(normalized).unwrap(),
            "line1\nline2\nline3\n"
        );
    }

    #[test]
    fn test_hash_skill_payload_is_stable_across_line_endings() {
        let a =
            "---\nd: skill\nname: Skill\nversion: 1.0.0\ndescription: test\nexpiry: 1\n---\nbody\n";
        let b = "---\r\nd: skill\r\nname: Skill\r\nversion: 1.0.0\r\ndescription: test\r\nexpiry: 1\r\n---\r\nbody\r\n";
        assert_eq!(hash_skill_payload(a), hash_skill_payload(b));
    }

    #[test]
    fn test_derive_manifest_from_skill_payload() {
        let payload = r#"---
d: research-assistant
name: Research Assistant
version: 1.4.2
description: Summarize technical docs
expiry: 1756000000
capabilities:
  - filesystem:read
  - http:outbound
author_npub: npub1author
---
Minor prompt hardening
"#;

        let derived =
            derive_manifest_from_skill_payload(payload, "publisherpubkey", 1_740_400_000).unwrap();

        assert_eq!(derived.manifest.identifier, "research-assistant");
        assert_eq!(derived.manifest.version, "1.4.2");
        assert_eq!(
            derived.manifest.author_npub,
            Some("npub1author".to_string())
        );
        assert_eq!(
            derived.event_template.kind,
            super::super::manifest::KIND_SKILL_MANIFEST
        );
        assert!(
            derived
                .event_template
                .tags
                .iter()
                .any(|tag| tag[0] == "manifest_hash" && tag[1] == derived.normalized_payload_hash)
        );
    }

    #[test]
    fn test_derive_manifest_is_deterministic_given_same_inputs() {
        let payload = r#"---
d: research-assistant
name: Research Assistant
version: 1.4.2
description: Summarize technical docs
expiry: 1756000000
capabilities:
  - http:outbound
---
Minor prompt hardening
"#;

        let first =
            derive_manifest_from_skill_payload(payload, "publisherpubkey", 1_740_400_000).unwrap();
        let second =
            derive_manifest_from_skill_payload(payload, "publisherpubkey", 1_740_400_000).unwrap();

        assert_eq!(
            first.normalized_payload_hash,
            second.normalized_payload_hash
        );
        assert_eq!(first.event_template.tags, second.event_template.tags);
        assert_eq!(first.event_template.content, second.event_template.content);
    }
}
