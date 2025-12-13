//! Agent manifest - declarative agent definition.

use super::{
    AgentCapabilities, AgentEconomics, AgentId, AgentIdError, AgentKeypair, AgentRequirements,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("invalid version: {0}")]
    InvalidVersion(String),
    #[error("signature verification failed: {0}")]
    SignatureVerification(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("identity error: {0}")]
    Identity(#[from] AgentIdError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<AgentId>,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AgentAuthor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(default)]
    pub requirements: AgentRequirements,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economics: Option<AgentEconomics>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<AgentCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

impl AgentManifest {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            schema_version: default_schema_version(),
            id: None,
            name: name.into(),
            version: version.into(),
            description: None,
            author: None,
            license: None,
            homepage: None,
            repository: None,
            capabilities: AgentCapabilities::default(),
            requirements: AgentRequirements::default(),
            economics: None,
            tags: Vec::new(),
            category: None,
            created_at: None,
            updated_at: None,
            signature: None,
            metadata: HashMap::new(),
        }
    }

    pub fn builder() -> AgentManifestBuilder {
        AgentManifestBuilder::default()
    }

    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    pub fn to_json(&self) -> Result<String, ManifestError> {
        serde_json::to_string_pretty(self).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    pub fn canonical_form(&self) -> Result<Vec<u8>, ManifestError> {
        let mut manifest = self.clone();
        manifest.signature = None;
        serde_json::to_vec(&manifest).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    pub fn sign(&mut self, keypair: &AgentKeypair) -> Result<(), ManifestError> {
        self.id = Some(keypair.agent_id());
        let canonical = self.canonical_form()?;
        let signature = keypair.sign(&canonical)?;
        self.signature = Some(hex::encode(signature));
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<bool, ManifestError> {
        let Some(id) = &self.id else {
            return Err(ManifestError::SignatureVerification(
                "manifest has no ID".to_string(),
            ));
        };
        let Some(sig_hex) = &self.signature else {
            return Err(ManifestError::SignatureVerification(
                "manifest has no signature".to_string(),
            ));
        };
        let sig_bytes = hex::decode(sig_hex)
            .map_err(|e| ManifestError::SignatureVerification(e.to_string()))?;
        if sig_bytes.len() != 64 {
            return Err(ManifestError::SignatureVerification(
                "invalid signature length".to_string(),
            ));
        }
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&sig_bytes);
        let canonical = self.canonical_form()?;
        Ok(AgentKeypair::verify(id, &canonical, &signature))
    }

    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.is_empty() {
            return Err(ManifestError::MissingField("name".to_string()));
        }
        if self.version.is_empty() {
            return Err(ManifestError::MissingField("version".to_string()));
        }
        Ok(())
    }

    pub fn can_handle_job(&self, kind: u16) -> bool {
        self.capabilities.can_handle_job(kind)
    }

    pub fn has_skill(&self, skill: &str) -> bool {
        self.capabilities.has_skill(skill)
    }

    pub fn display_name(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }
}

impl Default for AgentManifest {
    fn default() -> Self {
        Self::new("unnamed", "0.0.0")
    }
}

#[derive(Default)]
pub struct AgentManifestBuilder {
    manifest: AgentManifest,
}

impl AgentManifestBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.manifest.name = name.into();
        self
    }
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.manifest.version = version.into();
        self
    }
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.manifest.description = Some(description.into());
        self
    }
    pub fn author(mut self, author: AgentAuthor) -> Self {
        self.manifest.author = Some(author);
        self
    }
    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.manifest.license = Some(license.into());
        self
    }
    pub fn homepage(mut self, url: impl Into<String>) -> Self {
        self.manifest.homepage = Some(url.into());
        self
    }
    pub fn repository(mut self, url: impl Into<String>) -> Self {
        self.manifest.repository = Some(url.into());
        self
    }
    pub fn capabilities(mut self, capabilities: AgentCapabilities) -> Self {
        self.manifest.capabilities = capabilities;
        self
    }
    pub fn requirements(mut self, requirements: AgentRequirements) -> Self {
        self.manifest.requirements = requirements;
        self
    }
    pub fn economics(mut self, economics: AgentEconomics) -> Self {
        self.manifest.economics = Some(economics);
        self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.manifest.tags.push(tag.into());
        self
    }
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.manifest.tags = tags;
        self
    }
    pub fn category(mut self, category: AgentCategory) -> Self {
        self.manifest.category = Some(category);
        self
    }
    pub fn metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.manifest.metadata.insert(key.into(), value);
        self
    }
    pub fn build(mut self) -> Result<AgentManifest, ManifestError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if self.manifest.created_at.is_none() {
            self.manifest.created_at = Some(now);
        }
        self.manifest.updated_at = Some(now);
        self.manifest.validate()?;
        Ok(self.manifest)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npub: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl AgentAuthor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            npub: None,
            email: None,
            url: None,
        }
    }
    pub fn with_npub(mut self, npub: impl Into<String>) -> Self {
        self.npub = Some(npub.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCategory {
    Coding,
    Research,
    Writing,
    Data,
    Image,
    Audio,
    Video,
    Automation,
    Security,
    Finance,
    General,
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_builder() {
        let manifest = AgentManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .description("A test agent")
            .build()
            .unwrap();
        assert_eq!(manifest.name, "test-agent");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_manifest_validation() {
        let result = AgentManifest::builder().version("1.0.0").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_manifest_signing() {
        let keypair = AgentKeypair::from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();
        let mut manifest = AgentManifest::builder()
            .name("agent")
            .version("1.0.0")
            .build()
            .unwrap();
        manifest.sign(&keypair).unwrap();
        assert!(manifest.is_signed());
        assert!(manifest.verify_signature().unwrap());
    }
}
