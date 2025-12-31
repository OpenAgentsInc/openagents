//! Skill licensing using NIP-SA types
//!
//! This module provides marketplace-specific wrappers around NIP-SA skill
//! license events (kinds 39220, 39221).

use nostr::{SkillDelivery, SkillDeliveryContent, SkillLicense, SkillLicenseContent};
use thiserror::Error;

/// Parameters for issuing a skill license
pub struct IssueLicenseParams {
    pub skill_id: String,
    pub skill_name: String,
    pub version: String,
    pub agent_pubkey: String,
    pub capabilities: Vec<String>,
    pub price_sats: u64,
    pub granted_at: u64,
}

/// Parameters for issuing an expiring skill license
pub struct IssueExpiringLicenseParams {
    pub skill_id: String,
    pub skill_name: String,
    pub version: String,
    pub agent_pubkey: String,
    pub capabilities: Vec<String>,
    pub price_sats: u64,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// Errors that can occur during skill licensing operations
#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("license expired: {0}")]
    LicenseExpired(String),

    #[error("license not found for skill: {0}")]
    LicenseNotFound(String),

    #[error("invalid license: {0}")]
    InvalidLicense(String),

    #[error("delivery verification failed: {0}")]
    DeliveryVerificationFailed(String),

    #[error("marketplace error: {0}")]
    Marketplace(String),
}

/// License manager for marketplace skill licensing
pub struct LicenseManager {
    // Future: add database connection, relay pool, etc.
}

impl LicenseManager {
    /// Create a new license manager
    pub fn new() -> Self {
        Self {}
    }

    /// Issue a new skill license
    ///
    /// This creates a SkillLicense event (kind:39220) that grants an agent
    /// access to a specific skill.
    pub fn issue_license(&self, params: IssueLicenseParams) -> Result<SkillLicense, LicenseError> {
        let content = SkillLicenseContent::new(
            params.skill_id,
            params.skill_name,
            params.version,
            params.granted_at,
            params.capabilities,
        );

        let license = SkillLicense::new(content, params.agent_pubkey, params.price_sats);

        Ok(license)
    }

    /// Issue a license with expiration
    pub fn issue_expiring_license(
        &self,
        params: IssueExpiringLicenseParams,
    ) -> Result<SkillLicense, LicenseError> {
        let content = SkillLicenseContent::new(
            params.skill_id,
            params.skill_name,
            params.version,
            params.granted_at,
            params.capabilities,
        )
        .with_expires_at(params.expires_at);

        let license = SkillLicense::new(content, params.agent_pubkey, params.price_sats);

        Ok(license)
    }

    /// Verify a license is valid for the current time
    pub fn verify_license(
        &self,
        license: &SkillLicense,
        current_time: u64,
    ) -> Result<(), LicenseError> {
        license
            .validate(current_time)
            .map_err(|e| LicenseError::InvalidLicense(e.to_string()))
    }

    /// Create a skill delivery event
    ///
    /// This creates a SkillDelivery event (kind:39221) that delivers the
    /// encrypted skill content to the licensed agent.
    pub fn create_delivery(
        &self,
        skill_id: impl Into<String>,
        content: impl Into<String>,
        content_type: impl Into<String>,
        content_hash: impl Into<String>,
        license_id: impl Into<String>,
    ) -> Result<SkillDelivery, LicenseError> {
        let delivery_content =
            SkillDeliveryContent::new(skill_id, content, content_type, content_hash);

        let delivery = SkillDelivery::new(delivery_content, license_id);

        Ok(delivery)
    }

    /// Verify a skill delivery matches the expected hash
    pub fn verify_delivery(
        &self,
        delivery: &SkillDelivery,
        expected_hash: &str,
    ) -> Result<(), LicenseError> {
        delivery
            .content
            .verify_hash(expected_hash)
            .map_err(|e| LicenseError::DeliveryVerificationFailed(e.to_string()))
    }
}

impl Default for LicenseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_license() {
        let manager = LicenseManager::new();

        let license = manager
            .issue_license(IssueLicenseParams {
                skill_id: "skill-123".to_string(),
                skill_name: "web-scraper".to_string(),
                version: "1.0.0".to_string(),
                agent_pubkey: "agent-pubkey".to_string(),
                capabilities: vec!["fetch".to_string(), "parse".to_string()],
                price_sats: 1000,
                granted_at: 1703000000,
            })
            .unwrap();

        assert_eq!(license.content.skill_id, "skill-123");
        assert_eq!(license.agent_pubkey, "agent-pubkey");
        assert_eq!(license.price_sats, 1000);
        assert!(license.content.expires_at.is_none());
    }

    #[test]
    fn test_issue_expiring_license() {
        let manager = LicenseManager::new();

        let license = manager
            .issue_expiring_license(IssueExpiringLicenseParams {
                skill_id: "skill-123".to_string(),
                skill_name: "web-scraper".to_string(),
                version: "1.0.0".to_string(),
                agent_pubkey: "agent-pubkey".to_string(),
                capabilities: vec!["fetch".to_string()],
                price_sats: 1000,
                granted_at: 1703000000,
                expires_at: 1703086400,
            })
            .unwrap();

        assert_eq!(license.content.expires_at, Some(1703086400));
    }

    #[test]
    fn test_verify_license_valid() {
        let manager = LicenseManager::new();

        let license = manager
            .issue_license(IssueLicenseParams {
                skill_id: "skill-123".to_string(),
                skill_name: "web-scraper".to_string(),
                version: "1.0.0".to_string(),
                agent_pubkey: "agent-pubkey".to_string(),
                capabilities: vec!["fetch".to_string()],
                price_sats: 1000,
                granted_at: 1703000000,
            })
            .unwrap();

        assert!(manager.verify_license(&license, 1703000000).is_ok());
    }

    #[test]
    fn test_verify_license_expired() {
        let manager = LicenseManager::new();

        let license = manager
            .issue_expiring_license(IssueExpiringLicenseParams {
                skill_id: "skill-123".to_string(),
                skill_name: "web-scraper".to_string(),
                version: "1.0.0".to_string(),
                agent_pubkey: "agent-pubkey".to_string(),
                capabilities: vec!["fetch".to_string()],
                price_sats: 1000,
                granted_at: 1703000000,
                expires_at: 1703086400,
            })
            .unwrap();

        assert!(manager.verify_license(&license, 1703100000).is_err());
    }

    #[test]
    fn test_create_delivery() {
        let manager = LicenseManager::new();

        let delivery = manager
            .create_delivery(
                "skill-123",
                "fn main() {}",
                "rust",
                "abc123",
                "license-event-id",
            )
            .unwrap();

        assert_eq!(delivery.content.skill_id, "skill-123");
        assert_eq!(delivery.license_id, "license-event-id");
    }

    #[test]
    fn test_verify_delivery_valid() {
        let manager = LicenseManager::new();

        let delivery = manager
            .create_delivery(
                "skill-123",
                "fn main() {}",
                "rust",
                "abc123",
                "license-event-id",
            )
            .unwrap();

        assert!(manager.verify_delivery(&delivery, "abc123").is_ok());
    }

    #[test]
    fn test_verify_delivery_invalid() {
        let manager = LicenseManager::new();

        let delivery = manager
            .create_delivery(
                "skill-123",
                "fn main() {}",
                "rust",
                "abc123",
                "license-event-id",
            )
            .unwrap();

        assert!(manager.verify_delivery(&delivery, "wrong-hash").is_err());
    }
}
