//! Identity types for Nostr marketplace participants.
//!
//! This module provides identity primitives for agents, creators, and providers
//! participating in the Nostr marketplace ecosystem.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "full")]
use chrono::{DateTime, Utc};

/// Errors that can occur when working with identities
#[derive(Error, Debug)]
pub enum IdentityError {
    /// Invalid public key format
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    /// Invalid Lightning address format
    #[error("invalid lightning address: {0}")]
    InvalidLightningAddress(String),

    /// Profile validation failed
    #[error("profile validation failed: {0}")]
    ProfileValidation(String),
}

/// Core Nostr identity containing public key.
///
/// The private key is stored separately in secure storage and never
/// exposed through this type.
///
/// # Examples
///
/// ```no_run
/// use nostr::identity::NostrIdentity;
///
/// // Create from hex public key
/// let hex_pubkey = "a".repeat(64);
/// let identity = NostrIdentity::new(&hex_pubkey).expect("valid hex pubkey");
/// assert_eq!(identity.pubkey(), hex_pubkey);
///
/// // Create from npub format
/// let identity = NostrIdentity::new("npub1234567890").expect("valid npub");
/// assert!(identity.pubkey().starts_with("npub1"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NostrIdentity {
    /// Public key in hex format (64 chars) or npub format
    pub pubkey: String,
}

impl NostrIdentity {
    /// Create a new NostrIdentity from a public key
    ///
    /// # Arguments
    /// * `pubkey` - Public key in hex (64 chars) or npub format
    pub fn new(pubkey: impl Into<String>) -> Result<Self, IdentityError> {
        let pubkey = pubkey.into();

        // Basic validation - should be either 64 hex chars or npub format
        if !Self::is_valid_pubkey(&pubkey) {
            return Err(IdentityError::InvalidPublicKey(pubkey));
        }

        Ok(Self { pubkey })
    }

    /// Validate a public key format
    fn is_valid_pubkey(pubkey: &str) -> bool {
        // Check for npub format or 64 hex characters
        pubkey.starts_with("npub1")
            || (pubkey.len() == 64 && pubkey.chars().all(|c| c.is_ascii_hexdigit()))
    }

    /// Get the public key as a string
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }
}

/// Profile information for a creator in the marketplace.
///
/// Creators publish skills, agents, and content to the marketplace.
/// Their identity is backed by a Nostr keypair for signing submissions.
#[cfg(feature = "full")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorProfile {
    /// Nostr identity for this creator
    pub identity: NostrIdentity,

    /// Display name
    pub name: String,

    /// Optional Lightning address for receiving payouts (e.g., user@domain.com)
    pub lightning_address: Option<String>,

    /// Whether this creator has been verified by the platform
    pub verified: bool,

    /// When this profile was created
    pub created_at: DateTime<Utc>,

    /// Optional bio/about text
    pub bio: Option<String>,

    /// Optional website URL
    pub website: Option<String>,

    /// Optional avatar image URL
    pub avatar_url: Option<String>,
}

#[cfg(feature = "full")]
impl CreatorProfile {
    /// Create a new creator profile
    pub fn new(identity: NostrIdentity, name: impl Into<String>) -> Self {
        Self {
            identity,
            name: name.into(),
            lightning_address: None,
            verified: false,
            created_at: Utc::now(),
            bio: None,
            website: None,
            avatar_url: None,
        }
    }

    /// Set the Lightning address for payouts
    pub fn with_lightning_address(
        mut self,
        address: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let address = address.into();

        // Basic Lightning address validation (should be email-like format)
        if !address.contains('@') || address.split('@').count() != 2 {
            return Err(IdentityError::InvalidLightningAddress(address));
        }

        self.lightning_address = Some(address);
        Ok(self)
    }

    /// Set the bio text
    pub fn with_bio(mut self, bio: impl Into<String>) -> Self {
        self.bio = Some(bio.into());
        self
    }

    /// Set the website URL
    pub fn with_website(mut self, website: impl Into<String>) -> Self {
        self.website = Some(website.into());
        self
    }

    /// Set the avatar URL
    pub fn with_avatar(mut self, avatar_url: impl Into<String>) -> Self {
        self.avatar_url = Some(avatar_url.into());
        self
    }

    /// Mark this creator as verified
    pub fn verify(mut self) -> Self {
        self.verified = true;
        self
    }
}

/// Wallet information for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    /// Lightning address or connection string
    pub lightning_address: Option<String>,

    /// Current balance in satoshis (if available)
    pub balance_sats: Option<u64>,

    /// Whether the wallet is connected and operational
    pub connected: bool,
}

impl WalletInfo {
    /// Create a new wallet info with no connection
    pub fn disconnected() -> Self {
        Self {
            lightning_address: None,
            balance_sats: None,
            connected: false,
        }
    }

    /// Create a new wallet info with a Lightning address
    pub fn connected(lightning_address: impl Into<String>) -> Self {
        Self {
            lightning_address: Some(lightning_address.into()),
            balance_sats: None,
            connected: true,
        }
    }
}

/// Reputation score for an agent based on marketplace activity
///
/// # Examples
///
/// ```no_run
/// use nostr::identity::ReputationScore;
///
/// let mut rep = ReputationScore::default();
/// rep.jobs_completed = 100;
/// rep.jobs_successful = 95;
/// rep.rating = 4.5;
/// rep.rating_count = 50;
///
/// assert_eq!(rep.success_rate(), 95.0);
/// assert!(rep.is_reputable());
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ReputationScore {
    /// Number of completed jobs
    pub jobs_completed: u64,

    /// Number of successful jobs (no errors)
    pub jobs_successful: u64,

    /// Average response time in seconds
    pub avg_response_time_secs: Option<f64>,

    /// Total earnings in satoshis
    pub total_earnings_sats: u64,

    /// Star rating (0.0 to 5.0)
    pub rating: f64,

    /// Number of ratings received
    pub rating_count: u64,
}

impl Default for ReputationScore {
    fn default() -> Self {
        Self {
            jobs_completed: 0,
            jobs_successful: 0,
            avg_response_time_secs: None,
            total_earnings_sats: 0,
            rating: 0.0,
            rating_count: 0,
        }
    }
}

impl ReputationScore {
    /// Calculate success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.jobs_completed == 0 {
            return 0.0;
        }
        (self.jobs_successful as f64 / self.jobs_completed as f64) * 100.0
    }

    /// Check if this agent has a good reputation (>= 4.0 rating and >= 80% success rate)
    pub fn is_reputable(&self) -> bool {
        self.rating >= 4.0 && self.success_rate() >= 80.0 && self.jobs_completed >= 10
    }
}

/// Identity for an agent participating in the marketplace.
///
/// Agents process jobs, earn reputation, and manage a wallet for payments.
///
/// # Examples
///
/// ```no_run
/// use nostr::identity::{NostrIdentity, AgentIdentity, WalletInfo};
///
/// let pubkey = "a".repeat(64);
/// let identity = NostrIdentity::new(&pubkey).expect("valid pubkey");
///
/// let agent = AgentIdentity::new(identity, "Code Assistant")
///     .with_description("Helps with programming tasks")
///     .add_job_kind(5050)  // Code generation
///     .add_job_kind(5051); // Code review
///
/// assert_eq!(agent.name, "Code Assistant");
/// assert!(agent.supports_job_kind(5050));
/// assert!(!agent.supports_job_kind(9999));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Nostr identity for this agent
    pub identity: NostrIdentity,

    /// Wallet information for receiving payments
    pub wallet: WalletInfo,

    /// Reputation score based on marketplace activity
    pub reputation: ReputationScore,

    /// Agent display name
    pub name: String,

    /// Optional description of agent capabilities
    pub description: Option<String>,

    /// Supported DVM job kinds (NIP-90 kinds 5000-5999)
    pub supported_job_kinds: Vec<u32>,
}

impl AgentIdentity {
    /// Create a new agent identity
    pub fn new(identity: NostrIdentity, name: impl Into<String>) -> Self {
        Self {
            identity,
            wallet: WalletInfo::disconnected(),
            reputation: ReputationScore::default(),
            name: name.into(),
            description: None,
            supported_job_kinds: vec![],
        }
    }

    /// Set the wallet information
    pub fn with_wallet(mut self, wallet: WalletInfo) -> Self {
        self.wallet = wallet;
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a supported job kind
    pub fn add_job_kind(mut self, kind: u32) -> Self {
        if !self.supported_job_kinds.contains(&kind) {
            self.supported_job_kinds.push(kind);
        }
        self
    }

    /// Check if this agent supports a specific job kind
    pub fn supports_job_kind(&self, kind: u32) -> bool {
        self.supported_job_kinds.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nostr_identity_hex() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();
        assert_eq!(identity.pubkey(), pubkey);
    }

    #[test]
    fn test_nostr_identity_npub() {
        let identity = NostrIdentity::new("npub1234567890").unwrap();
        assert_eq!(identity.pubkey(), "npub1234567890");
    }

    #[test]
    fn test_nostr_identity_invalid() {
        assert!(NostrIdentity::new("invalid").is_err());
        assert!(NostrIdentity::new("abc").is_err());
    }

    #[test]
    fn test_wallet_info() {
        let wallet = WalletInfo::disconnected();
        assert!(!wallet.connected);
        assert!(wallet.lightning_address.is_none());

        let wallet = WalletInfo::connected("user@domain.com");
        assert!(wallet.connected);
        assert_eq!(wallet.lightning_address.unwrap(), "user@domain.com");
    }

    #[test]
    fn test_reputation_score() {
        let mut rep = ReputationScore::default();
        assert_eq!(rep.success_rate(), 0.0);
        assert!(!rep.is_reputable());

        rep.jobs_completed = 100;
        rep.jobs_successful = 95;
        rep.rating = 4.5;
        rep.rating_count = 50;

        assert_eq!(rep.success_rate(), 95.0);
        assert!(rep.is_reputable());
    }

    #[test]
    fn test_agent_identity() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();

        let agent = AgentIdentity::new(identity, "Test Agent")
            .with_description("A test agent")
            .add_job_kind(5001) // Summarization
            .add_job_kind(5002); // Translation

        assert_eq!(agent.name, "Test Agent");
        assert!(agent.supports_job_kind(5001));
        assert!(agent.supports_job_kind(5002));
        assert!(!agent.supports_job_kind(5000));
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_creator_profile() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();

        let creator = CreatorProfile::new(identity, "Test Creator")
            .with_bio("A test creator bio")
            .with_website("https://example.com")
            .with_lightning_address("creator@domain.com")
            .unwrap()
            .verify();

        assert_eq!(creator.name, "Test Creator");
        assert!(creator.verified);
        assert_eq!(creator.bio.unwrap(), "A test creator bio");
        assert_eq!(creator.lightning_address.unwrap(), "creator@domain.com");
    }

    #[cfg(feature = "full")]
    #[test]
    fn test_invalid_lightning_address() {
        let pubkey = "a".repeat(64);
        let identity = NostrIdentity::new(&pubkey).unwrap();

        let creator = CreatorProfile::new(identity.clone(), "Test Creator");
        assert!(creator.with_lightning_address("invalid").is_err());

        let creator2 = CreatorProfile::new(identity, "Test Creator");
        assert!(creator2.with_lightning_address("user@domain.com").is_ok());
    }
}
