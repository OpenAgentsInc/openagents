//! Canonical scope parsing and hashing for NIP-AC.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors for scope parsing/hash operations.
#[derive(Debug, Error)]
pub enum ScopeHashError {
    #[error("invalid scope tag")]
    InvalidScopeTag,

    #[error("invalid scope type: {0}")]
    InvalidScopeType(String),

    #[error("missing scope id")]
    MissingScopeId,

    #[error("invalid SKL skill scope id: {0}")]
    InvalidSkillScopeId(String),

    #[error("invalid constraints hash; expected 64 lowercase hex chars")]
    InvalidConstraintsHash,

    #[error("missing constraints hash for skill scope")]
    MissingConstraintsHash,

    #[error("missing required SKL `a` tag for skill scope")]
    MissingSkillAddressTag,

    #[error("missing required pinned manifest `e` tag for skill scope")]
    MissingManifestEventTag,

    #[error("skill address mismatch: expected {expected}, got {actual}")]
    SkillAddressMismatch { expected: String, actual: String },
}

/// Supported scope types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeType {
    Nip90,
    L402,
    Skill,
    Other(String),
}

impl ScopeType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Nip90 => "nip90",
            Self::L402 => "l402",
            Self::Skill => "skill",
            Self::Other(value) => value.as_str(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, ScopeHashError> {
        match value.to_lowercase().as_str() {
            "nip90" => Ok(Self::Nip90),
            "l402" => Ok(Self::L402),
            "skill" => Ok(Self::Skill),
            other if !other.is_empty() => Ok(Self::Other(other.to_string())),
            _ => Err(ScopeHashError::InvalidScopeType(value.to_string())),
        }
    }
}

/// Parsed SKL canonical skill scope id (`33400:<pubkey>:<d-tag>:<version>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillScopeId {
    pub publisher_pubkey: String,
    pub d_tag: String,
    pub version: String,
}

impl SkillScopeId {
    pub fn parse(value: &str) -> Result<Self, ScopeHashError> {
        let parts: Vec<&str> = value.split(':').collect();
        if parts.len() != 4 || parts[0] != "33400" {
            return Err(ScopeHashError::InvalidSkillScopeId(value.to_string()));
        }
        if parts[1].trim().is_empty() || parts[2].trim().is_empty() || parts[3].trim().is_empty() {
            return Err(ScopeHashError::InvalidSkillScopeId(value.to_string()));
        }
        Ok(Self {
            publisher_pubkey: parts[1].to_string(),
            d_tag: parts[2].to_string(),
            version: parts[3].to_string(),
        })
    }

    pub fn to_canonical_string(&self) -> String {
        format!(
            "33400:{}:{}:{}",
            self.publisher_pubkey, self.d_tag, self.version
        )
    }
}

/// Scope reference carried by AC events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeReference {
    pub scope_type: ScopeType,
    pub scope_id: String,
    pub constraints_hash: Option<String>,
}

impl ScopeReference {
    pub fn new(scope_type: ScopeType, scope_id: impl Into<String>) -> Self {
        Self {
            scope_type,
            scope_id: scope_id.into(),
            constraints_hash: None,
        }
    }

    pub fn with_constraints_hash(mut self, constraints_hash: impl Into<String>) -> Self {
        self.constraints_hash = Some(constraints_hash.into());
        self
    }

    pub fn validate(&self) -> Result<(), ScopeHashError> {
        if self.scope_id.trim().is_empty() {
            return Err(ScopeHashError::MissingScopeId);
        }

        if let Some(constraints_hash) = &self.constraints_hash
            && !is_lower_hex_64(constraints_hash)
        {
            return Err(ScopeHashError::InvalidConstraintsHash);
        }

        if self.scope_type == ScopeType::Skill {
            SkillScopeId::parse(&self.scope_id)?;
            if self.constraints_hash.is_none() {
                return Err(ScopeHashError::MissingConstraintsHash);
            }
        }

        Ok(())
    }

    /// Canonical compact scope value used in `["scope", "<type>", "<value>"]` tags.
    pub fn canonical_value(&self) -> String {
        if let Some(constraints_hash) = &self.constraints_hash {
            format!("{}:{}", self.scope_id, constraints_hash)
        } else {
            self.scope_id.clone()
        }
    }

    pub fn to_scope_tag(&self) -> Result<Vec<String>, ScopeHashError> {
        self.validate()?;
        Ok(vec![
            "scope".to_string(),
            self.scope_type.as_str().to_string(),
            self.canonical_value(),
        ])
    }

    pub fn from_scope_tag(tag: &[String]) -> Result<Self, ScopeHashError> {
        if tag.first().map(String::as_str) != Some("scope") || tag.len() < 3 {
            return Err(ScopeHashError::InvalidScopeTag);
        }

        let scope_type = ScopeType::parse(tag[1].as_str())?;
        let (scope_id, constraints_hash) = split_scope_value(&scope_type, tag[2].as_str())?;

        let scope = Self {
            scope_type,
            scope_id,
            constraints_hash,
        };
        scope.validate()?;
        Ok(scope)
    }
}

/// Canonical scope hash used for deterministic envelope targeting.
pub fn canonical_scope_hash(scope: &ScopeReference) -> Result<String, ScopeHashError> {
    scope.validate()?;
    let canonical = format!(
        "{}|{}|{}",
        scope.scope_type.as_str(),
        scope.scope_id,
        scope.constraints_hash.clone().unwrap_or_default()
    );
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

/// Validate SKL linkage tags (`a` + pinned manifest `e`) for skill scopes.
pub fn validate_skill_scope_links(
    scope: &ScopeReference,
    skill_address: Option<&str>,
    manifest_event_id: Option<&str>,
) -> Result<(), ScopeHashError> {
    scope.validate()?;
    if scope.scope_type != ScopeType::Skill {
        return Ok(());
    }

    let skill_scope = SkillScopeId::parse(&scope.scope_id)?;
    let expected_skill_address = format!(
        "33400:{}:{}",
        skill_scope.publisher_pubkey, skill_scope.d_tag
    );
    let provided_skill_address = skill_address.ok_or(ScopeHashError::MissingSkillAddressTag)?;
    if provided_skill_address != expected_skill_address {
        return Err(ScopeHashError::SkillAddressMismatch {
            expected: expected_skill_address,
            actual: provided_skill_address.to_string(),
        });
    }

    let manifest_event_id = manifest_event_id.ok_or(ScopeHashError::MissingManifestEventTag)?;
    if manifest_event_id.trim().is_empty() {
        return Err(ScopeHashError::MissingManifestEventTag);
    }

    Ok(())
}

fn split_scope_value(
    scope_type: &ScopeType,
    raw_value: &str,
) -> Result<(String, Option<String>), ScopeHashError> {
    if raw_value.trim().is_empty() {
        return Err(ScopeHashError::MissingScopeId);
    }

    if scope_type != &ScopeType::Skill {
        return Ok((raw_value.to_string(), None));
    }

    // Skill scope values may include a trailing constraints hash:
    // `33400:<pubkey>:<d-tag>:<version>:<constraints_hash>`
    let parts: Vec<&str> = raw_value.split(':').collect();
    if parts.len() == 4 {
        return Ok((raw_value.to_string(), None));
    }
    if parts.len() == 5 {
        let scope_id = parts[..4].join(":");
        let constraints_hash = parts[4].to_string();
        if !is_lower_hex_64(&constraints_hash) {
            return Err(ScopeHashError::InvalidConstraintsHash);
        }
        return Ok((scope_id, Some(constraints_hash)));
    }

    Err(ScopeHashError::InvalidSkillScopeId(raw_value.to_string()))
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn test_parse_skill_scope_id() {
        let parsed = SkillScopeId::parse("33400:skillpub:research-assistant:1.4.2").unwrap();
        assert_eq!(parsed.publisher_pubkey, "skillpub");
        assert_eq!(parsed.d_tag, "research-assistant");
        assert_eq!(parsed.version, "1.4.2");
    }

    #[test]
    fn test_scope_reference_from_scope_tag_skill_with_constraints() {
        let tag = vec![
            "scope".to_string(),
            "skill".to_string(),
            format!("33400:skillpub:research-assistant:1.4.2:{}", HASH),
        ];
        let scope = ScopeReference::from_scope_tag(&tag).unwrap();
        assert_eq!(scope.scope_type, ScopeType::Skill);
        assert_eq!(scope.scope_id, "33400:skillpub:research-assistant:1.4.2");
        assert_eq!(scope.constraints_hash, Some(HASH.to_string()));
    }

    #[test]
    fn test_scope_hash_deterministic() {
        let scope = ScopeReference::new(ScopeType::Nip90, "job-id").with_constraints_hash(HASH);
        let hash_a = canonical_scope_hash(&scope).unwrap();
        let hash_b = canonical_scope_hash(&scope).unwrap();
        assert_eq!(hash_a, hash_b);
    }

    #[test]
    fn test_skill_scope_links_validation() {
        let scope =
            ScopeReference::new(ScopeType::Skill, "33400:skillpub:research-assistant:1.4.2")
                .with_constraints_hash(HASH);
        assert!(
            validate_skill_scope_links(
                &scope,
                Some("33400:skillpub:research-assistant"),
                Some("manifest-event-id"),
            )
            .is_ok()
        );
    }
}
