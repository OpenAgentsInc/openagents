//! Agent manifest - declarative agent definition.
//!
//! A manifest is a complete, serializable definition of an agent that can be:
//! - Published to registries
//! - Shared between users
//! - Used to instantiate agent runtimes
//! - Verified for authenticity (signed by creator)
//!
//! # Example
//!
//! ```rust,ignore
//! use agent::core::*;
//!
//! let manifest = AgentManifest::builder()
//!     .name("code-reviewer")
//!     .version("1.0.0")
//!     .description("Reviews code for bugs and style issues")
//!     .capabilities(AgentCapabilities::builder()
//!         .add_skill(SKILL_CODE_REVIEW)
//!         .add_job_kind(KIND_JOB_TEXT_GENERATION)
//!         .build())
//!     .requirements(AgentRequirements::builder()
//!         .environment(ExecutionEnvironment::Hybrid {
//!             prefer: ExecutionPreference::PreferLocal,
//!         })
//!         .build())
//!     .economics(AgentEconomics::builder()
//!         .pricing(PricingModel::per_job(10000))
//!         .build())
//!     .build()?;
//! ```

use super::{AgentCapabilities, AgentEconomics, AgentId, AgentIdError, AgentKeypair, AgentRequirements};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur with manifests.
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

/// A complete, declarative definition of an agent.
///
/// Manifests can be serialized to JSON/YAML, published to registries,
/// and used to instantiate agent runtimes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    /// Manifest schema version.
    #[serde(default = "default_schema_version")]
    pub schema_version: String,

    /// Agent identity (Nostr public key).
    ///
    /// This is optional for draft manifests but required for published agents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<AgentId>,

    /// Human-readable name.
    pub name: String,

    /// Semantic version.
    pub version: String,

    /// Description of what this agent does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Agent author/creator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<AgentAuthor>,

    /// License.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Homepage URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// Repository URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Capabilities this agent provides.
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Execution requirements.
    #[serde(default)]
    pub requirements: AgentRequirements,

    /// Economic configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economics: Option<AgentEconomics>,

    /// Tags for discovery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<AgentCategory>,

    /// Creation timestamp (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,

    /// Last updated timestamp (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,

    /// Cryptographic signature (from creator's keypair).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,

    /// Additional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

impl AgentManifest {
    /// Create a new manifest with minimal required fields.
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

    /// Create a builder for constructing a manifest.
    pub fn builder() -> AgentManifestBuilder {
        AgentManifestBuilder::default()
    }

    /// Parse a manifest from JSON.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Serialize the manifest to JSON.
    pub fn to_json(&self) -> Result<String, ManifestError> {
        serde_json::to_string_pretty(self).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Parse a manifest from YAML.
    #[cfg(feature = "yaml")]
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        serde_yaml::from_str(yaml).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Get the canonical form for signing.
    ///
    /// This excludes the signature field itself.
    pub fn canonical_form(&self) -> Result<Vec<u8>, ManifestError> {
        let mut manifest = self.clone();
        manifest.signature = None;
        serde_json::to_vec(&manifest).map_err(|e| ManifestError::Serialization(e.to_string()))
    }

    /// Sign the manifest with a keypair.
    pub fn sign(&mut self, keypair: &AgentKeypair) -> Result<(), ManifestError> {
        // Set the ID from the keypair
        self.id = Some(keypair.agent_id());

        // Get canonical form and sign
        let canonical = self.canonical_form()?;
        let signature = keypair.sign(&canonical)?;
        self.signature = Some(hex::encode(signature));

        Ok(())
    }

    /// Verify the signature.
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

    /// Check if this manifest is signed.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Validate the manifest.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.is_empty() {
            return Err(ManifestError::MissingField("name".to_string()));
        }

        if self.version.is_empty() {
            return Err(ManifestError::MissingField("version".to_string()));
        }

        // Validate semver if we have the semver feature
        #[cfg(feature = "semver")]
        {
            semver::Version::parse(&self.version)
                .map_err(|e| ManifestError::InvalidVersion(e.to_string()))?;
        }

        Ok(())
    }

    /// Check if this agent can handle a job kind.
    pub fn can_handle_job(&self, kind: u16) -> bool {
        self.capabilities.can_handle_job(kind)
    }

    /// Check if this agent has a specific skill.
    pub fn has_skill(&self, skill: &str) -> bool {
        self.capabilities.has_skill(skill)
    }

    /// Get display name (name + version).
    pub fn display_name(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }
}

/// Builder for AgentManifest.
#[derive(Default)]
pub struct AgentManifestBuilder {
    manifest: AgentManifest,
}

impl Default for AgentManifest {
    fn default() -> Self {
        Self::new("unnamed", "0.0.0")
    }
}

impl AgentManifestBuilder {
    /// Set the agent ID.
    pub fn id(mut self, id: AgentId) -> Self {
        self.manifest.id = Some(id);
        self
    }

    /// Set the name.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.manifest.name = name.into();
        self
    }

    /// Set the version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.manifest.version = version.into();
        self
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.manifest.description = Some(description.into());
        self
    }

    /// Set the author.
    pub fn author(mut self, author: AgentAuthor) -> Self {
        self.manifest.author = Some(author);
        self
    }

    /// Set the license.
    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.manifest.license = Some(license.into());
        self
    }

    /// Set the homepage.
    pub fn homepage(mut self, url: impl Into<String>) -> Self {
        self.manifest.homepage = Some(url.into());
        self
    }

    /// Set the repository.
    pub fn repository(mut self, url: impl Into<String>) -> Self {
        self.manifest.repository = Some(url.into());
        self
    }

    /// Set capabilities.
    pub fn capabilities(mut self, capabilities: AgentCapabilities) -> Self {
        self.manifest.capabilities = capabilities;
        self
    }

    /// Set requirements.
    pub fn requirements(mut self, requirements: AgentRequirements) -> Self {
        self.manifest.requirements = requirements;
        self
    }

    /// Set economics.
    pub fn economics(mut self, economics: AgentEconomics) -> Self {
        self.manifest.economics = Some(economics);
        self
    }

    /// Add a tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.manifest.tags.push(tag.into());
        self
    }

    /// Set tags.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.manifest.tags = tags;
        self
    }

    /// Set category.
    pub fn category(mut self, category: AgentCategory) -> Self {
        self.manifest.category = Some(category);
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.manifest.metadata.insert(key.into(), value);
        self
    }

    /// Build the manifest.
    pub fn build(mut self) -> Result<AgentManifest, ManifestError> {
        // Set timestamps
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if self.manifest.created_at.is_none() {
            self.manifest.created_at = Some(now);
        }
        self.manifest.updated_at = Some(now);

        // Validate
        self.manifest.validate()?;

        Ok(self.manifest)
    }
}

/// Agent author information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthor {
    /// Author name.
    pub name: String,

    /// Author's Nostr public key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npub: Option<String>,

    /// Author email.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Author website.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl AgentAuthor {
    /// Create a new author.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            npub: None,
            email: None,
            url: None,
        }
    }

    /// Set the Nostr public key.
    pub fn with_npub(mut self, npub: impl Into<String>) -> Self {
        self.npub = Some(npub.into());
        self
    }

    /// Set the email.
    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }
}

/// Agent categories for discovery.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCategory {
    /// Coding and development.
    Coding,
    /// Research and analysis.
    Research,
    /// Writing and content.
    Writing,
    /// Data processing.
    Data,
    /// Image generation/processing.
    Image,
    /// Audio processing.
    Audio,
    /// Video processing.
    Video,
    /// Automation and workflows.
    Automation,
    /// Security and analysis.
    Security,
    /// Finance and trading.
    Finance,
    /// General purpose.
    General,
    /// Custom category.
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ExecutionEnvironment, PricingModel};

    #[test]
    fn test_manifest_builder() {
        let manifest = AgentManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .description("A test agent")
            .author(AgentAuthor::new("Test Author"))
            .license("MIT")
            .category(AgentCategory::Coding)
            .tag("test")
            .tag("example")
            .build()
            .unwrap();

        assert_eq!(manifest.name, "test-agent");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, Some("A test agent".to_string()));
        assert!(manifest.created_at.is_some());
    }

    #[test]
    fn test_manifest_validation() {
        // Missing name
        let result = AgentManifest::builder().version("1.0.0").build();
        assert!(result.is_err());

        // Missing version
        let result = AgentManifest::builder().name("test").build();
        assert!(result.is_err());

        // Valid
        let result = AgentManifest::builder()
            .name("test")
            .version("1.0.0")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_manifest_signing() {
        let keypair = AgentKeypair::from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        )
        .unwrap();

        let mut manifest = AgentManifest::builder()
            .name("signed-agent")
            .version("1.0.0")
            .build()
            .unwrap();

        // Sign
        manifest.sign(&keypair).unwrap();
        assert!(manifest.is_signed());
        assert_eq!(manifest.id, Some(keypair.agent_id()));

        // Verify
        assert!(manifest.verify_signature().unwrap());

        // Tamper and verify fails
        manifest.name = "tampered".to_string();
        assert!(!manifest.verify_signature().unwrap());
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = AgentManifest::builder()
            .name("test-agent")
            .version("1.0.0")
            .description("A test agent")
            .requirements(
                AgentRequirements::builder()
                    .environment(ExecutionEnvironment::Local)
                    .build(),
            )
            .economics(
                AgentEconomics::builder()
                    .pricing(PricingModel::per_job(10000))
                    .build(),
            )
            .build()
            .unwrap();

        let json = manifest.to_json().unwrap();
        assert!(json.contains("test-agent"));
        assert!(json.contains("1.0.0"));

        let parsed = AgentManifest::from_json(&json).unwrap();
        assert_eq!(parsed.name, manifest.name);
        assert_eq!(parsed.version, manifest.version);
    }
}
